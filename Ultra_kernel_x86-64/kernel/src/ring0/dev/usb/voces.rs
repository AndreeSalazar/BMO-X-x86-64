//! **LAS VOCES DEL ORQUESTADOR, en el kernel**: el banco que la app presta, las
//! ordenes que manda y la mezcla que corre en el hilo del bus (2026-09-22).
//!
//! [carril]  AMARILLO  lee el banco de la app por el physmap, dentro de la
//!                     medida que se juzgo al prestarlo
//! [consumo] APARATO   solo mientras suena una voz: sin voces, `hay()` es falso
//!                     y la trama sigue el camino de siempre
//!
//! [cuesta]  MAQUINA -- lee memoria de otro proceso por el physmap. La base y
//!           la medida del banco las da `fisica_de` (el juez de siempre) y cada
//!           muestra se lee con `get` dentro de esa medida.
//!
//! [riesgo]  AJENO RELOJ
//!           AJENO: las muestras las escribe la app, y la app puede escribir lo
//!           que quiera -- se leen como numeros, nunca como direcciones, y lo
//!           peor que suena es ruido. RELOJ: la mezcla corre en el latido del
//!           bus; dieciseis voces por trama son ~800 sumas por milisegundo.
//!
//! # La pieza entera, y por que es del orquestador
//!
//! Ver la cabecera de `bmo_amplificador::voces`: hasta hoy una app tenia que
//! mandar PCM ya mezclado y llegar SIEMPRE a tiempo, y la que se atascaba
//! cortaba. Con las voces la app DECLARA -- tocar, ajustar, callar -- y el que
//! marca el tiempo es el orquestador. Es el mismo reparto que TypeSafe escribe
//! para sus modelos de decision: *"el codigo es el responsable de la accion"*.
//!
//! # Quien escribe que (la regla de los escritores)
//!
//! ```text
//!    la app (su syscall)   el BANCO, en atomicos, y las ORDENES, en la cola
//!    el hilo del bus       las VOCES: saca las ordenes, mezcla, y publica
//!                          cuales suenan (`SONANDO`)
//! ```
//!
//! Ningun estado tiene dos escritores. Por eso tocar no toca las voces: deja
//! una orden. Y por eso soltar el banco --la app se muere-- no vacia la cola
//! desde otro sitio: sube la GENERACION del banco, y el bus, al verla, calla
//! todo y tira las ordenes que eran del banco anterior.

use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use bmo_amplificador::voces::{self, Formato, Rechazo, Sonido, Voces, MAX_VOCES, PLENO_VOZ};

use crate::ring0::cabina;

// ===================================================================
//  EL BANCO: lo escribe la app, lo lee el bus
// ===================================================================

/// El pid del banco. `0` = no hay banco.
static BANCO_PID: AtomicU32 = AtomicU32::new(0);
static BANCO_FISICA: AtomicU64 = AtomicU64::new(0);
static BANCO_BYTES: AtomicU64 = AtomicU64::new(0);
/// **La generacion del banco**: sube cada vez que el banco cambia o se suelta.
/// El bus la compara con la suya y, si no coincide, calla todas las voces:
/// una voz del banco de antes no puede sonar con las muestras del de ahora.
static BANCO_GEN: AtomicU32 = AtomicU32::new(0);

// ===================================================================
//  LAS ORDENES: las empuja la app, las saca el bus
// ===================================================================

#[derive(Clone, Copy)]
enum Orden {
    Nada,
    /// Canal, generacion del banco para la que se pidio, y el sonido.
    Tocar(u8, u32, Sonido),
    Ajustar(u8, u16, u16),
    Callar(u8),
    CallarTodas,
}

/// UN productor --el syscall del propietario del sonido, que es uno-- y UN
/// consumidor, el bus. 64 sitios: DOOM empieza como mucho unos pocos sonidos
/// por tic, y el bus drena cada 4 ms.
static ORDENES: bmo_cola::Cola<Orden, 64> = bmo_cola::Cola::nueva(Orden::Nada);

/// Canales con un `tocar` pedido y aun no aplicado. Sin esto, preguntar
/// *"suena?"* justo despues de tocar contestaria que no, y DOOM volveria a
/// pedir el sonido. Lo pone la app y lo quita el bus: dos escritores, pero
/// cada uno con su operacion atomica sobre bits distintos en el tiempo.
static PENDIENTES: AtomicU32 = AtomicU32::new(0);
/// Canales que sonaban en la ultima trama. Lo escribe SOLO el bus.
static SONANDO: AtomicU32 = AtomicU32::new(0);
/// Cuentas para el `save`: tocadas, rechazadas (con su motivo en CABINA) y
/// ordenes que no cupieron en la cola.
static TOCADAS: AtomicU64 = AtomicU64::new(0);
static RECHAZADAS: AtomicU64 = AtomicU64::new(0);

// ===================================================================
//  LAS VOCES: las mueve SOLO el bus
// ===================================================================

static mut VOCES: Voces = Voces::nuevas(); // [escribe] bombeo
/// La generacion del banco que el bus tiene vista.
static mut GEN_VISTA: u32 = 0; // [escribe] bombeo

fn rechazo(que: Rechazo) {
    RECHAZADAS.fetch_add(1, Ordering::SeqCst);
    let porque = match que {
        Rechazo::Canal => "voz: ese canal no existe (hay 16)",
        Rechazo::Vacio => "voz: un sonido de cero muestras",
        Rechazo::Frecuencia => "voz: la frecuencia no tiene sentido (1 a 192 kHz)",
        Rechazo::FueraDelBanco => "voz: el sonido se SALE del banco que presto la app",
    };
    cabina::warn("audio", porque, 0);
}

// ===================================================================
//  LO QUE LLAMA LA APP (desde `AUDIO_OP_VOZ`)
// ===================================================================

/// **Prestar el banco**: el bloque `va` de `pid`, entero. Devuelve sus bytes,
/// o `0` si esa memoria no es suya. Un banco nuevo calla lo que sonaba.
pub fn banco(pid: u32, va: u64) -> u64 {
    let Some(bytes) = crate::ring0::obj::memory::bytes_de_bloque(pid, va) else {
        cabina::warn("audio", "voz: el banco no es un bloque de quien lo presta, pid", pid as u64);
        return 0;
    };
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, bytes) else {
        cabina::warn("audio", "voz: el banco no es memoria de quien lo presta, pid", pid as u64);
        return 0;
    };
    BANCO_FISICA.store(fisica, Ordering::SeqCst);
    BANCO_BYTES.store(bytes, Ordering::SeqCst);
    BANCO_PID.store(pid, Ordering::SeqCst);
    BANCO_GEN.fetch_add(1, Ordering::SeqCst);
    PENDIENTES.store(0, Ordering::SeqCst);
    cabina::bytes("audio", "voz: BANCO prestado por la app, bytes", bytes);
    bytes
}

/// **Tocar**: `a1` = inicio en bytes `[0..32)` | muestras `[32..64)`;
/// `a2` = canal `[0..8)` | formato `[8..10)` | pista `[10..18)` | izq
/// `[18..27)` | der `[27..36)` | hz `[36..56)`. Devuelve `1` si queda pedida
/// y `0` si el juez dijo que no (el motivo, en CABINA).
pub fn tocar(pid: u32, a1: u64, a2: u64) -> u64 {
    if BANCO_PID.load(Ordering::SeqCst) != pid || pid == 0 {
        RECHAZADAS.fetch_add(1, Ordering::SeqCst);
        cabina::warn("audio", "voz: tocar sin haber prestado un banco, pid", pid as u64);
        return 0;
    }
    let canal = (a2 & 0xFF) as usize;
    let Some(formato) = Formato::de((a2 >> 8) & 0x3) else {
        RECHAZADAS.fetch_add(1, Ordering::SeqCst);
        return 0;
    };
    let s = Sonido {
        inicio: (a1 & 0xFFFF_FFFF) as u32,
        muestras: (a1 >> 32) as u32,
        formato,
        pista: ((a2 >> 10) & 0xFF) as u8,
        izq: (((a2 >> 18) & 0x1FF) as u16).min(PLENO_VOZ),
        der: (((a2 >> 27) & 0x1FF) as u16).min(PLENO_VOZ),
        hz: ((a2 >> 36) & 0xF_FFFF) as u32,
    };
    // ** EL MISMO JUEZ que usara el bus, y aqui: el NO se dice en el acto y
    // con su motivo. La frecuencia de salida no importa para juzgar, basta
    // con que no sea cero.
    if let Err(r) = voces::comprobar(canal, &s, BANCO_BYTES.load(Ordering::SeqCst), 48_000) {
        rechazo(r);
        return 0;
    }
    let gen = BANCO_GEN.load(Ordering::SeqCst);
    if !ORDENES.empujar(Orden::Tocar(canal as u8, gen, s)) {
        return 0;
    }
    PENDIENTES.fetch_or(1 << canal, Ordering::SeqCst);
    TOCADAS.fetch_add(1, Ordering::SeqCst);
    1
}

/// **Ajustar** el volumen y el lado de una voz que suena: `canal`, y `lados` =
/// izq `[0..16)` | der `[16..32)`.
pub fn ajustar(pid: u32, canal: u64, lados: u64) -> u64 {
    if BANCO_PID.load(Ordering::SeqCst) != pid || canal as usize >= MAX_VOCES {
        return 0;
    }
    let izq = ((lados & 0xFFFF) as u16).min(PLENO_VOZ);
    let der = (((lados >> 16) & 0xFFFF) as u16).min(PLENO_VOZ);
    ORDENES.empujar(Orden::Ajustar(canal as u8, izq, der)) as u64
}

/// **Callar** un canal, o todos con `canal = 0xFF`.
pub fn callar(pid: u32, canal: u64) -> u64 {
    if BANCO_PID.load(Ordering::SeqCst) != pid {
        return 0;
    }
    let orden = if canal == 0xFF {
        PENDIENTES.store(0, Ordering::SeqCst);
        Orden::CallarTodas
    } else if (canal as usize) < MAX_VOCES {
        PENDIENTES.fetch_and(!(1 << canal), Ordering::SeqCst);
        Orden::Callar(canal as u8)
    } else {
        return 0;
    };
    ORDENES.empujar(orden) as u64
}

/// **Suena?** Cuenta tambien lo pedido y aun no aplicado.
pub fn suena(canal: u64) -> u64 {
    if canal as usize >= MAX_VOCES {
        return 0;
    }
    let bits = SONANDO.load(Ordering::SeqCst) | PENDIENTES.load(Ordering::SeqCst);
    ((bits >> canal) & 1) as u64
}

/// **El propietario suelta el banco, o se muere.** No se toca ni la cola ni las
/// voces --son de otros escritores--: sube la generacion y el bus hace el resto.
pub fn soltar(pid: u32) {
    if pid != 0 && BANCO_PID.load(Ordering::SeqCst) == pid {
        BANCO_PID.store(0, Ordering::SeqCst);
        BANCO_GEN.fetch_add(1, Ordering::SeqCst);
        PENDIENTES.store(0, Ordering::SeqCst);
        cabina::count("audio", "voz: banco SOLTADO, pid", pid as u64);
    }
}

/// **La app devolvio un bloque suyo al asignador.** Si era (o pisaba) el
/// banco, el banco se suelta AQUI: sin esto el bus seguiria leyendo unos
/// marcos que ya son de otro, y lo que sonaria es la memoria de otro proceso.
///
/// [!] Lo que esto NO cierra, dicho: si el hilo del bus estaba A MITAD de
/// mezclar una trama cuando se devolvio el bloque, esa trama (1 ms como mucho)
/// termina leyendo los marcos ya limpiados -- `memory::soltar` los pone a cero
/// antes de devolverlos, asi que lo normal es que suene silencio. Cerrarlo del
/// todo pide que el marco no vuelva al asignador hasta que el bus lo diga.
pub fn block_returned(pid: u32, fisica: u64, bytes: u64) {
    if pid == 0 || BANCO_PID.load(Ordering::SeqCst) != pid {
        return;
    }
    let base = BANCO_FISICA.load(Ordering::SeqCst);
    let fin = base.saturating_add(BANCO_BYTES.load(Ordering::SeqCst));
    if base < fisica.saturating_add(bytes) && fisica < fin {
        soltar(pid);
    }
}

// ===================================================================
//  LO QUE HACE EL BUS (desde `maestro::componer`, una vez por trama)
// ===================================================================

/// **Saca las ordenes y las aplica.** `hz_salida` es la del aparato.
///
/// # Safety
/// Solo desde el hilo del bus: es el unico escritor de las voces.
pub unsafe fn atender(hz_salida: u32) {
    let voces = &mut *addr_of_mut!(VOCES);
    let gen = BANCO_GEN.load(Ordering::SeqCst);
    if gen != GEN_VISTA {
        voces.callar_todas();
        GEN_VISTA = gen;
    }
    let bytes = if BANCO_PID.load(Ordering::SeqCst) != 0 { BANCO_BYTES.load(Ordering::SeqCst) } else { 0 };
    while let Some(o) = ORDENES.sacar() {
        match o {
            Orden::Nada => {}
            Orden::Tocar(c, g, s) => {
                PENDIENTES.fetch_and(!(1u32 << c), Ordering::SeqCst);
                // Una orden del banco de antes no suena con el de ahora.
                if g == gen && bytes > 0 {
                    if let Err(r) = voces.tocar(c as usize, s, bytes, hz_salida) {
                        rechazo(r);
                    }
                }
            }
            Orden::Ajustar(c, i, d) => voces.ajustar(c as usize, i, d),
            Orden::Callar(c) => voces.callar(c as usize),
            Orden::CallarTodas => voces.callar_todas(),
        }
    }
    if bytes == 0 {
        voces.callar_todas();
    }
    SONANDO.store(voces.activas(), Ordering::SeqCst);
}

/// Suena alguna voz?
///
/// # Safety
/// Desde el hilo del bus.
pub unsafe fn hay() -> bool {
    (*addr_of_mut!(VOCES)).hay()
}

/// **Mezcla las voces en `acc`**, sumando a lo que hubiera.
///
/// # Safety
/// Desde el hilo del bus. El banco se lee por el physmap con la base y la
/// medida que dio el juez al prestarlo.
pub unsafe fn mezclar(acc: &mut [i32], canales: usize) {
    let voces = &mut *addr_of_mut!(VOCES);
    if BANCO_PID.load(Ordering::SeqCst) == 0 {
        voces.callar_todas();
        SONANDO.store(0, Ordering::SeqCst);
        return;
    }
    let fisica = BANCO_FISICA.load(Ordering::SeqCst);
    let bytes = BANCO_BYTES.load(Ordering::SeqCst) as usize;
    let banco = core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(fisica) as *const u8, bytes);
    voces.mezclar(banco, acc, canales);
    SONANDO.store(voces.activas(), Ordering::SeqCst);
}

// ===================================================================
//  LO QUE SE LEE: `OP_INFO`, sin handle
// ===================================================================

/// `INFO_AUDIO_VOCES`: `[0..16)` canales que suenan | `[16..48)` bytes del
/// banco | `[48..64)` pid del banco (0 = no hay).
pub fn info() -> u64 {
    (SONANDO.load(Ordering::SeqCst) as u64 & 0xFFFF)
        | ((BANCO_BYTES.load(Ordering::SeqCst).min(0xFFFF_FFFF)) << 16)
        | ((BANCO_PID.load(Ordering::SeqCst) as u64 & 0xFFFF) << 48)
}

/// `INFO_AUDIO_VOCES_CUENTA`: `[0..32)` voces tocadas | `[32..48)` rechazadas
/// por el juez | `[48..64)` ordenes que no cupieron en la cola.
pub fn info_cuenta() -> u64 {
    TOCADAS.load(Ordering::SeqCst).min(0xFFFF_FFFF)
        | (RECHAZADAS.load(Ordering::SeqCst).min(0xFFFF) << 32)
        | ((ORDENES.perdidos() as u64).min(0xFFFF) << 48)
}
