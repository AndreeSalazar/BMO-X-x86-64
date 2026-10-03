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

//! # Y el ATRIL DEL FONDO (2026-10-03)
//!
//! El propietario: *"como si fuera fondo [...] que relaja al usuario hasta
//! que aparece una notificacion para avisar"*, y que siga sonando mientras
//! juega. Con UN banco eso no se puede: el banco es del que tiene el sonido
//! (DOOM), y el escritorio no puede tocar nada mientras DOOM lo tenga.
//!
//! Por eso hay DOS atriles con la misma pieza dentro:
//!
//! ```text
//!    APP     el banco de quien RECLAMO el sonido (DOOM, `musica`)
//!    FONDO   el banco del ESCRITORIO (quien tiene la pantalla): la musica
//!            de fondo en los canales 0..8 y los AVISOS en 8..16
//! ```
//!
//! Y en la mezcla, el fondo pasa por el AGACHE
//! (`bmo_amplificador::agacha`) antes de sumarse: baja 15 dB mientras suena
//! un aviso, 8 mientras suena la app, y vuelve sola. Los avisos se suman
//! DESPUES, sin agachar: son los que mandan. Es la misma regla que el mando
//! del maestro: lo del escritorio lo toca solo el escritorio.

use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use bmo_amplificador::agacha::{Agacha, AGACHE_AVISO, AGACHE_JUEGO};
use bmo_amplificador::voces::{self, Formato, Rechazo, Sonido, Voces, MAX_VOCES, PLENO_VOZ};

use crate::ring0::cabina;

#[derive(Clone, Copy)]
enum Orden {
    Nada,
    /// Canal, generacion del banco para la que se pidio, y el sonido.
    Tocar(u8, u32, Sonido),
    Ajustar(u8, u16, u16),
    Callar(u8),
    CallarTodas,
}

/// Los canales del FONDO que son AVISOS: no se agachan y hacen agacharse.
pub const AVISOS: u32 = 0xFF00;
/// Los del FONDO que son musica.
pub const MUSICA: u32 = 0x00FF;

/// **Un atril**: un banco prestado, la cola de sus ordenes y sus cuentas.
///
/// # Quien escribe que (la regla de los escritores)
///
/// ```text
///    la app (su syscall)   el BANCO, en atomicos, y las ORDENES, en la cola
///    el hilo del bus       las VOCES: saca las ordenes, mezcla, y publica
///                          cuales suenan (`sonando`)
/// ```
///
/// Ningun estado tiene dos escritores. Por eso tocar no toca las voces: deja
/// una orden. Y por eso soltar el banco --la app se muere-- no vacia la cola
/// desde otro sitio: sube la GENERACION del banco, y el bus, al verla, calla
/// todo y tira las ordenes que eran del banco anterior.
pub struct Atril {
    /// Con que nombre sale en CABINA.
    modulo: &'static str,
    /// Su sitio en [`VOCES`] y [`GEN_VISTA`].
    indice: usize,
    /// El pid del banco. `0` = no hay banco.
    pid: AtomicU32,
    fisica: AtomicU64,
    bytes: AtomicU64,
    /// **La generacion del banco**: sube cada vez que el banco cambia o se
    /// suelta. El bus la compara con la suya y, si no coincide, calla todas
    /// las voces: una voz del banco de antes no puede sonar con las muestras
    /// del de ahora.
    gen: AtomicU32,
    /// UN productor --el syscall del propietario del atril, que es uno-- y UN
    /// consumidor, el bus. 64 sitios: DOOM empieza como mucho unos pocos
    /// sonidos por tic, y el bus drena cada 4 ms.
    ordenes: bmo_cola::Cola<Orden, 64>,
    /// Canales con un `tocar` pedido y aun no aplicado. Sin esto, preguntar
    /// *"suena?"* justo despues de tocar contestaria que no, y DOOM volveria
    /// a pedir el sonido. Lo pone la app y lo quita el bus: dos escritores,
    /// pero cada uno con su operacion atomica sobre bits distintos en el
    /// tiempo.
    pendientes: AtomicU32,
    /// Canales que sonaban en la ultima trama. Lo escribe SOLO el bus.
    sonando: AtomicU32,
    /// Cuentas para el `save`: tocadas y rechazadas (con su motivo en CABINA).
    tocadas: AtomicU64,
    rechazadas: AtomicU64,
}

/// **El atril de la APP**: el de siempre, el de quien reclamo el sonido.
pub static APP: Atril = Atril::nuevo("audio", 0);
/// **El atril del FONDO**: el del escritorio.
pub static FONDO: Atril = Atril::nuevo("fondo", 1);

// ===================================================================
//  LAS VOCES: las mueve SOLO el bus
// ===================================================================

static mut VOCES: [Voces; 2] = [Voces::nuevas(), Voces::nuevas()]; // [escribe] bombeo
/// La generacion del banco que el bus tiene vista, por atril.
static mut GEN_VISTA: [u32; 2] = [0, 0]; // [escribe] bombeo
/// **El agache del fondo**, y la frecuencia para la que se hizo.
static mut AGACHE: Agacha = Agacha::nueva(48_000); // [escribe] bombeo
static mut AGACHE_HZ: u32 = 48_000; // [escribe] bombeo
/// El cubo donde se mezcla la musica de fondo antes de agacharla: una trama
/// del tubo cabe de sobra (la de `maestro::SUMA`).
const CUBO_MAX: usize = 512;
static mut CUBO: [i32; CUBO_MAX] = [0; CUBO_MAX]; // [escribe] bombeo

impl Atril {
    const fn nuevo(modulo: &'static str, indice: usize) -> Atril {
        Atril {
            modulo,
            indice,
            pid: AtomicU32::new(0),
            fisica: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            gen: AtomicU32::new(0),
            ordenes: bmo_cola::Cola::nueva(Orden::Nada),
            pendientes: AtomicU32::new(0),
            sonando: AtomicU32::new(0),
            tocadas: AtomicU64::new(0),
            rechazadas: AtomicU64::new(0),
        }
    }

    fn rechazo(&self, que: Rechazo) {
        self.rechazadas.fetch_add(1, Ordering::SeqCst);
        let porque = match que {
            Rechazo::Canal => "voz: ese canal no existe (hay 16)",
            Rechazo::Vacio => "voz: un sonido de cero muestras",
            Rechazo::Frecuencia => "voz: la frecuencia no tiene sentido (1 a 192 kHz)",
            Rechazo::FueraDelBanco => "voz: el sonido se SALE del banco que presto la app",
        };
        cabina::warn(self.modulo, porque, 0);
    }

    /// El pid del banco (0 = no hay).
    pub fn pid(&self) -> u32 {
        self.pid.load(Ordering::SeqCst)
    }

    // ---------------------------------------------------------------
    //  LO QUE LLAMA LA APP (desde `AUDIO_OP_VOZ` o `TASK_OP_AUDIO_FONDO`)
    // ---------------------------------------------------------------

    /// **Prestar el banco**: el bloque `va` de `pid`, entero. Devuelve sus
    /// bytes, o `0` si esa memoria no es suya. Un banco nuevo calla lo que
    /// sonaba.
    pub fn banco(&self, pid: u32, va: u64) -> u64 {
        let Some(bytes) = crate::ring0::obj::memory::bytes_de_bloque(pid, va) else {
            cabina::warn(self.modulo, "voz: el banco no es un bloque de quien lo presta, pid", pid as u64);
            return 0;
        };
        let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, va, bytes) else {
            cabina::warn(self.modulo, "voz: el banco no es memoria de quien lo presta, pid", pid as u64);
            return 0;
        };
        self.fisica.store(fisica, Ordering::SeqCst);
        self.bytes.store(bytes, Ordering::SeqCst);
        self.pid.store(pid, Ordering::SeqCst);
        self.gen.fetch_add(1, Ordering::SeqCst);
        self.pendientes.store(0, Ordering::SeqCst);
        cabina::bytes(self.modulo, "voz: BANCO prestado por la app, bytes", bytes);
        bytes
    }

    /// **Tocar** un sonido ya leido de la puerta. Devuelve `1` si queda
    /// pedido y `0` si el juez dijo que no (el motivo, en CABINA).
    pub fn tocar_sonido(&self, pid: u32, canal: usize, s: Sonido) -> u64 {
        if self.pid.load(Ordering::SeqCst) != pid || pid == 0 {
            self.rechazadas.fetch_add(1, Ordering::SeqCst);
            cabina::warn(self.modulo, "voz: tocar sin haber prestado un banco, pid", pid as u64);
            return 0;
        }
        // ** EL MISMO JUEZ que usara el bus, y aqui: el NO se dice en el acto
        // y con su motivo. La frecuencia de salida no importa para juzgar,
        // basta con que no sea cero.
        if let Err(r) = voces::comprobar(canal, &s, self.bytes.load(Ordering::SeqCst), 48_000) {
            self.rechazo(r);
            return 0;
        }
        let gen = self.gen.load(Ordering::SeqCst);
        if !self.ordenes.empujar(Orden::Tocar(canal as u8, gen, s)) {
            return 0;
        }
        self.pendientes.fetch_or(1 << canal, Ordering::SeqCst);
        self.tocadas.fetch_add(1, Ordering::SeqCst);
        1
    }

    /// **Ajustar** el volumen y el lado de una voz que suena. Llega por la
    /// rampa de `bmo_amplificador::voces`: no salta.
    pub fn ajustar_lados(&self, pid: u32, canal: u64, izq: u16, der: u16) -> u64 {
        if self.pid.load(Ordering::SeqCst) != pid || canal as usize >= MAX_VOCES {
            return 0;
        }
        self.ordenes.empujar(Orden::Ajustar(canal as u8, izq.min(PLENO_VOZ), der.min(PLENO_VOZ))) as u64
    }

    /// **Callar** un canal, o todos con `canal = 0xFF`.
    pub fn callar(&self, pid: u32, canal: u64) -> u64 {
        if self.pid.load(Ordering::SeqCst) != pid {
            return 0;
        }
        let orden = if canal == 0xFF {
            self.pendientes.store(0, Ordering::SeqCst);
            Orden::CallarTodas
        } else if (canal as usize) < MAX_VOCES {
            self.pendientes.fetch_and(!(1 << canal), Ordering::SeqCst);
            Orden::Callar(canal as u8)
        } else {
            return 0;
        };
        self.ordenes.empujar(orden) as u64
    }

    /// **Suena?** Cuenta tambien lo pedido y aun no aplicado.
    pub fn suena(&self, canal: u64) -> u64 {
        if canal as usize >= MAX_VOCES {
            return 0;
        }
        let bits = self.sonando.load(Ordering::SeqCst) | self.pendientes.load(Ordering::SeqCst);
        ((bits >> canal) & 1) as u64
    }

    /// **El propietario suelta el banco, o se muere.** No se toca ni la cola ni las
    /// voces --son de otros escritores--: sube la generacion y el bus hace el
    /// resto.
    pub fn soltar(&self, pid: u32) {
        if pid != 0 && self.pid.load(Ordering::SeqCst) == pid {
            self.pid.store(0, Ordering::SeqCst);
            // ** Y la medida a cero: el `save` decia `banco 16777216 B` con
            // `banco de 0 pid`, un numero sin propietario que parece un banco vivo.
            // Nadie la lee sin mirar antes el pid (el bus y
            // `block_returned`), asi que ponerla a cero no cambia lo que
            // suena, cambia lo que se CUENTA.
            self.bytes.store(0, Ordering::SeqCst);
            self.gen.fetch_add(1, Ordering::SeqCst);
            self.pendientes.store(0, Ordering::SeqCst);
            cabina::count(self.modulo, "voz: banco SOLTADO, pid", pid as u64);
        }
    }

    /// **La app devolvio un bloque suyo al asignador.** Si era (o pisaba) el
    /// banco, el banco se suelta AQUI: sin esto el bus seguiria leyendo unos
    /// marcos que ya son de otro, y lo que sonaria es la memoria de otro
    /// proceso.
    ///
    /// [!] Lo que esto NO cierra, dicho: si el hilo del bus estaba A MITAD de
    /// mezclar una trama cuando se devolvio el bloque, esa trama (1 ms como
    /// mucho) termina leyendo los marcos ya limpiados -- `memory::soltar` los
    /// pone a cero antes de devolverlos, asi que lo normal es que suene
    /// silencio. Cerrarlo del todo pide que el marco no vuelva al asignador
    /// hasta que el bus lo diga.
    pub fn block_returned(&self, pid: u32, fisica: u64, bytes: u64) {
        if pid == 0 || self.pid.load(Ordering::SeqCst) != pid {
            return;
        }
        let base = self.fisica.load(Ordering::SeqCst);
        let fin = base.saturating_add(self.bytes.load(Ordering::SeqCst));
        if base < fisica.saturating_add(bytes) && fisica < fin {
            self.soltar(pid);
        }
    }

    // ---------------------------------------------------------------
    //  LO QUE HACE EL BUS (desde `maestro::componer`, una vez por trama)
    // ---------------------------------------------------------------

    /// **Saca las ordenes y las aplica.** `hz_salida` es la del aparato.
    ///
    /// # Safety
    /// Solo desde el hilo del bus: es el unico escritor de las voces.
    pub unsafe fn atender(&self, hz_salida: u32) {
        let voces = &mut (*addr_of_mut!(VOCES))[self.indice];
        let vista = &mut (*addr_of_mut!(GEN_VISTA))[self.indice];
        let gen = self.gen.load(Ordering::SeqCst);
        if gen != *vista {
            voces.callar_todas();
            *vista = gen;
        }
        let bytes = if self.pid.load(Ordering::SeqCst) != 0 { self.bytes.load(Ordering::SeqCst) } else { 0 };
        while let Some(o) = self.ordenes.sacar() {
            match o {
                Orden::Nada => {}
                Orden::Tocar(c, g, s) => {
                    self.pendientes.fetch_and(!(1u32 << c), Ordering::SeqCst);
                    // Una orden del banco de antes no suena con el de ahora.
                    if g == gen && bytes > 0 {
                        if let Err(r) = voces.tocar(c as usize, s, bytes, hz_salida) {
                            self.rechazo(r);
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
        self.sonando.store(voces.activas(), Ordering::SeqCst);
    }

    /// Suena alguna voz?
    ///
    /// # Safety
    /// Desde el hilo del bus.
    pub unsafe fn hay(&self) -> bool {
        (*addr_of_mut!(VOCES))[self.indice].hay()
    }

    /// Las voces que suenan, en un mapa de bits.
    ///
    /// # Safety
    /// Desde el hilo del bus.
    pub unsafe fn activas(&self) -> u32 {
        (*addr_of_mut!(VOCES))[self.indice].activas()
    }

    /// **Mezcla las voces de `mascara` en `acc`**, sumando a lo que hubiera.
    ///
    /// # Safety
    /// Desde el hilo del bus. El banco se lee por el physmap con la base y la
    /// medida que dio el juez al prestarlo.
    pub unsafe fn mezclar(&self, acc: &mut [i32], canales: usize, mascara: u32) {
        let voces = &mut (*addr_of_mut!(VOCES))[self.indice];
        if self.pid.load(Ordering::SeqCst) == 0 {
            voces.callar_todas();
            self.sonando.store(0, Ordering::SeqCst);
            return;
        }
        let fisica = self.fisica.load(Ordering::SeqCst);
        let bytes = self.bytes.load(Ordering::SeqCst) as usize;
        let banco = core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(fisica) as *const u8, bytes);
        voces.mezclar_solo(banco, acc, canales, mascara);
        self.sonando.store(voces.activas(), Ordering::SeqCst);
    }

    // ---------------------------------------------------------------
    //  LO QUE SE LEE: `OP_INFO`, sin handle
    // ---------------------------------------------------------------

    /// `[0..16)` canales que suenan | `[16..48)` bytes del banco | `[48..64)`
    /// pid del banco (0 = no hay).
    pub fn info(&self) -> u64 {
        (self.sonando.load(Ordering::SeqCst) as u64 & 0xFFFF)
            | ((self.bytes.load(Ordering::SeqCst).min(0xFFFF_FFFF)) << 16)
            | ((self.pid.load(Ordering::SeqCst) as u64 & 0xFFFF) << 48)
    }

    /// `[0..32)` voces tocadas | `[32..48)` rechazadas por el juez |
    /// `[48..64)` ordenes que no cupieron en la cola.
    pub fn info_cuenta(&self) -> u64 {
        self.tocadas.load(Ordering::SeqCst).min(0xFFFF_FFFF)
            | (self.rechazadas.load(Ordering::SeqCst).min(0xFFFF) << 32)
            | ((self.ordenes.perdidos() as u64).min(0xFFFF) << 48)
    }
}

// ===================================================================
//  EL ATRIL DE LA APP, por su puerta de siempre (`AUDIO_OP_VOZ`)
// ===================================================================

/// Prestar el banco de la APP.
pub fn banco(pid: u32, va: u64) -> u64 {
    APP.banco(pid, va)
}

/// **Tocar** en el atril de la APP: `a1` = inicio en bytes `[0..32)` |
/// muestras `[32..64)`; `a2` = canal `[0..8)` | formato `[8..10)` | pista
/// `[10..18)` | izq `[18..27)` | der `[27..36)` | hz `[36..56)` | BUCLE
/// `[56]`.
pub fn tocar(pid: u32, a1: u64, a2: u64) -> u64 {
    let canal = (a2 & 0xFF) as usize;
    let Some(formato) = Formato::de((a2 >> 8) & 0x3) else {
        APP.rechazadas.fetch_add(1, Ordering::SeqCst);
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
        bucle: (a2 >> 56) & 1 != 0,
    };
    APP.tocar_sonido(pid, canal, s)
}

/// **Ajustar** en el atril de la APP: `lados` = izq `[0..16)` | der `[16..32)`.
pub fn ajustar(pid: u32, canal: u64, lados: u64) -> u64 {
    APP.ajustar_lados(pid, canal, (lados & 0xFFFF) as u16, ((lados >> 16) & 0xFFFF) as u16)
}

/// Callar en el atril de la APP.
pub fn callar(pid: u32, canal: u64) -> u64 {
    APP.callar(pid, canal)
}

/// Suena, en el atril de la APP.
pub fn suena(canal: u64) -> u64 {
    APP.suena(canal)
}

/// **Un proceso suelta el sonido, o se muere**: suelta su banco en los DOS
/// atriles. El del fondo no tiene puerta de soltar el sonido -- se suelta al
/// morir o con su propia orden --, y el ARMADO del fondo se apaga con el.
pub fn soltar(pid: u32) {
    APP.soltar(pid);
    if FONDO.pid() == pid && pid != 0 {
        FONDO.soltar(pid);
        super::audio::armar_fondo(false);
    }
}

/// Un bloque devuelto al asignador: lo miran los dos atriles.
pub fn block_returned(pid: u32, fisica: u64, bytes: u64) {
    APP.block_returned(pid, fisica, bytes);
    let tenia = FONDO.pid() == pid && pid != 0;
    FONDO.block_returned(pid, fisica, bytes);
    if tenia && FONDO.pid() == 0 {
        super::audio::armar_fondo(false);
    }
}

// ===================================================================
//  EL ATRIL DEL FONDO, por `TASK_OP_AUDIO_FONDO` (solo el escritorio)
// ===================================================================

/// **La puerta del fondo.** `a0` = orden `[0..4)` | canal `[4..8)` | izq
/// `[8..17)` | der `[17..26)` | BUCLE `[26]`; `a1` segun la orden.
///
/// ```text
///    1 BANCO    a1 = la VA del bloque. Arma el tubo si no lo estaba
///    2 TOCAR    a1 = inicio en bytes [0..32) | muestras [32..64).
///               Siempre S16 a 48 kHz: el escritorio compone asi
///    3 AJUSTAR  izq y der de `a0`, por la rampa
///    4 CALLAR   el canal de `a0`, o todos con canal 15 y a1 = 1
///    5 SUENA    el canal de `a0`
///    6 SOLTAR   el banco, y desarma el tubo si era por el fondo
/// ```
///
/// Los canales 8..16 son AVISOS ([`AVISOS`]): no se agachan y agachan al
/// resto.
pub fn fondo(pid: u32, a0: u64, a1: u64) -> u64 {
    let orden = a0 & 0xF;
    let canal = ((a0 >> 4) & 0xF) as usize;
    let izq = (((a0 >> 8) & 0x1FF) as u16).min(PLENO_VOZ);
    let der = (((a0 >> 17) & 0x1FF) as u16).min(PLENO_VOZ);
    match orden {
        1 => {
            let b = FONDO.banco(pid, a1);
            if b > 0 && !super::audio::armar_fondo(true) {
                // Sin tubo no hay donde sonar: se dice, y el banco se suelta
                // para no dejar uno prestado que nunca se lee.
                FONDO.soltar(pid);
                return 0;
            }
            b
        }
        2 => {
            let s = Sonido {
                inicio: (a1 & 0xFFFF_FFFF) as u32,
                muestras: (a1 >> 32) as u32,
                formato: Formato::S16,
                pista: 0,
                izq,
                der,
                hz: 48_000,
                bucle: (a0 >> 26) & 1 != 0,
            };
            FONDO.tocar_sonido(pid, canal, s)
        }
        3 => FONDO.ajustar_lados(pid, canal as u64, izq, der),
        4 => FONDO.callar(pid, if a1 == 1 { 0xFF } else { canal as u64 }),
        5 => FONDO.suena(canal as u64),
        6 => {
            let era = FONDO.pid() == pid;
            FONDO.soltar(pid);
            if era {
                super::audio::armar_fondo(false);
            }
            era as u64
        }
        _ => 0,
    }
}

/// **El fondo en la trama**: la musica (canales 0..8) a su cubo, agachada
/// segun lo que suene, sumada a `acc`; y los avisos (8..16) encima, sin
/// agachar. `app_suena` dice si la app sono en esta trama.
///
/// # Safety
/// Desde el hilo del bus.
pub unsafe fn mezclar_fondo(acc: &mut [i32], canales: usize, hz: u32, app_suena: bool) {
    let n = acc.len().min(CUBO_MAX);
    let cubo = &mut (&mut *addr_of_mut!(CUBO))[..n];
    cubo.fill(0);
    FONDO.mezclar(cubo, canales, MUSICA);
    let ag = &mut *addr_of_mut!(AGACHE);
    if *addr_of_mut!(AGACHE_HZ) != hz && hz != 0 {
        *ag = Agacha::nueva(hz);
        *addr_of_mut!(AGACHE_HZ) = hz;
    }
    let pedido = if FONDO.activas() & AVISOS != 0 {
        AGACHE_AVISO
    } else if app_suena {
        AGACHE_JUEGO
    } else {
        0
    };
    ag.bloque(pedido, cubo, canales);
    for (a, c) in acc[..n].iter_mut().zip(cubo.iter()) {
        *a += *c;
    }
    FONDO.mezclar(&mut acc[..n], canales, AVISOS);
}

/// Lo bajado por el agache ahora, en 1/256 dB (para el `save`).
pub fn agachado() -> i32 {
    // SAFETY: una lectura de un `i32` alineado; si el bus esta escribiendo,
    // se lee el de antes o el de despues, y los dos son verdad.
    unsafe { (*addr_of_mut!(AGACHE)).actual() }
}

// ===================================================================
//  LO QUE LLAMA EL BUS para la APP (los nombres de siempre)
// ===================================================================

/// # Safety
/// Desde el hilo del bus.
pub unsafe fn atender(hz_salida: u32) {
    APP.atender(hz_salida);
    FONDO.atender(hz_salida);
}

/// Suena alguna voz, de la app o del fondo?
///
/// # Safety
/// Desde el hilo del bus.
pub unsafe fn hay() -> bool {
    APP.hay() || FONDO.hay()
}

/// `INFO_AUDIO_FONDO`: `[0..16)` canales del fondo que suenan | `[16..32)` lo
/// agachado ahora (`i16`, 1/256 dB) | `[48..64)` pid del banco (0 = apagado).
pub fn info_fondo() -> u64 {
    let i = FONDO.info();
    (i & 0xFFFF) | (((agachado() as i16 as u16) as u64) << 16) | (i & (0xFFFF << 48))
}

/// `INFO_AUDIO_VOCES`, del atril de la APP.
pub fn info() -> u64 {
    APP.info()
}

/// `INFO_AUDIO_VOCES_CUENTA`, del atril de la APP.
pub fn info_cuenta() -> u64 {
    APP.info_cuenta()
}
