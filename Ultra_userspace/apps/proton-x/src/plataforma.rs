//! **La plataforma de BMO-X** para las DLL de la casa (`bmo-proton-x-casa`).
//!
//! Lo que `kernel32`, `user32` y `gdi32` de la casa piden al sistema, con la
//! puerta de BMO-X debajo:
//!
//! ```text
//!    escribir     la consola de quien nos lanzo (sin el \r de un \r\n)
//!    salir        dice con que salio el .exe, y el proceso se va
//!    superficie   un bloque con la cabecera BSUP y un buzon de 64 ranuras,
//!                 SIN ofrecer: la ventana existe y todavia no se ve
//!    mostrar      ofrecerla al escritorio (MEM_OP_OFRECER a quien nos lanzo)
//!    presentar    subir la secuencia: el dibujo esta entero (R-APP4)
//!                 (05-10: con el CARTEL ROJO encima si la casa aviso de algo,
//!                 `cartel.rs`; `escribir` le pasa cada linea)
//!    evento       el siguiente del buzon, o 0
//!    dormir       4 ms: un .exe esperando teclas no gasta CPU (R21)
//!    poner_gs     el TEB del hilo de Windows que va a correr (P4):
//!                 TASK_OP_PON_GS, que no toca el MSR si no cambia
//!    ahora_ns     `rdtsc` y la frecuencia que publica el kernel (INFO_TSC_HZ)
//!    dibujar      los lotes de D3D12: por la 3060 (la puerta, P3b4c), o con
//!                 los sombreadores NATIVOS de la casa si no se puede
//!    sellar_codigo  un bloque, los bytes y MEM_OP_SELLAR (W^X); soltarlo es
//!                 MEM_OP_SOLTAR: de los ocho bloques vivos, el codigo gasta uno
//!    leer_fichero   Archivo::leer_de + leer_en por el bloque de PASO (V3,
//!                 07-10): ENTERO, en trozos de 1 MiB, sin pedir bloque
//!    escribir_fichero  Archivo::create + escribir_de por el bloque de PASO;
//!                 a ESTRATOS, el de paso si cabe
//!    trozos       A LA CARTA (01-10): Archivo::reflejar + saltar + leer_en,
//!                 sin traerse el fichero: los grandes de un juego (Cyberpunk
//!                 abrio uno de 46 MB con un monton de 25)
//! ```
//!
//! La superficie es la MISMA que pide una app de INTI o de C
//! (`runtime/superficie/roja.inti`, `superficie/amarilla.h`): el escritorio no
//! sabe que dentro hay un `.exe` de Windows, y no tiene por que saberlo.

use bmo_proton_x_casa::{Plataforma, Superficie};
use bmo_userland as bmo;

/// Ranuras del buzon: potencia de dos, como pide el escritorio.
const RANURAS: u64 = 64;

pub fn de_bmo() -> Plataforma {
    Plataforma {
        escribir,
        salir,
        superficie,
        mostrar,
        presentar,
        evento,
        dormir,
        poner_gs,
        ahora_ns,
        dibujar: super::la3060::dibujar,
        sellar_codigo,
        soltar_codigo,
        leer_fichero,
        escribir_fichero,
        memoria,
        fecha,
        listar,
        carpetas: Some(CARPETAS),
        reserva: Some(RESERVA),
        trozos: Some(TROZOS),
        sonido: Some(super::sonido::SONIDO),
        cuaderno: Some(bmo_proton_x_casa::Cuaderno { abrir: abrir_cuaderno, sellar_hasta: sellar_cuaderno }),
    }
}

/// **El cuaderno de codigo** (V4 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10): UN
/// bloque que vive lo que el proceso, sellado por tramos
/// (`MEM_OP_SELLAR_HASTA`). Uno de los ocho bloques vivos, para siempre; a
/// cambio, la VA de bloques ya no se gasta con cada sombreador.
struct Cuaderno(core::cell::UnsafeCell<Option<bmo::Memoria>>);
// SAFETY: una tarea, y los hilos de la casa son cooperativos.
// [hilos] cerrojo -- el cuaderno de codigo: lo sella quien dibuje primero
unsafe impl Sync for Cuaderno {}
static CUADERNO: Cuaderno = Cuaderno(core::cell::UnsafeCell::new(None));

/// El mas grande que de el kernel (64 MiB es su tope por bloque; la RAM
/// tiene que estar SEGUIDA, asi que si no, menos).
fn abrir_cuaderno() -> Option<(u64, usize)> {
    // SAFETY: ver `Cuaderno`; nadie guarda la referencia.
    let c = unsafe { &mut *CUADERNO.0.get() };
    if c.is_none() {
        *c = [64usize, 32, 16, 8].iter().find_map(|&mib| bmo::Memoria::request((mib as u64) << 20));
    }
    let m = c.as_ref()?;
    Some((m.base() as u64, m.bytes() as usize))
}

fn sellar_cuaderno(base: u64, hasta: usize) -> bool {
    // SAFETY: ver `Cuaderno`.
    let c = unsafe { &*CUADERNO.0.get() };
    c.as_ref().is_some_and(|m| m.base() as u64 == base && m.sellar_hasta(hasta as u64).is_ok())
}

/// Los bloques de codigo sellados (uno vivo, casi siempre: la casa suelta el
/// anterior al sellar el siguiente).
struct Codigo(core::cell::UnsafeCell<alloc::vec::Vec<bmo::Memoria>>);
// SAFETY: una tarea, y los hilos de la casa son cooperativos.
// [hilos] cerrojo -- los bloques de codigo sellados
unsafe impl Sync for Codigo {}
static CODIGO: Codigo = Codigo(core::cell::UnsafeCell::new(alloc::vec::Vec::new()));

fn sellar_codigo(bytes: &[u8]) -> Option<u64> {
    let m = bmo::Memoria::request(bytes.len() as u64)?;
    // SAFETY: el bloque recien pedido mide al menos `bytes.len()` y es nuestro.
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), m.base(), bytes.len()) };
    // Si el kernel dice que no, `m` se suelta al salir (su Drop).
    m.sellar().ok()?;
    let base = m.base() as u64;
    // SAFETY: ver `Codigo`.
    unsafe { (*CODIGO.0.get()).push(m) };
    Some(base)
}

/// **Con el bloque de PASO** (V3 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10): el
/// de A LA CARTA ([`PASO`], uno y se queda), tambien para los ficheros
/// enteros. Antes cada `leer_fichero` y cada `escribir_fichero` pedia un
/// bloque y lo soltaba, y la VA de un bloque soltado NO vuelve (el kernel no
/// la reusa, a proposito): Cyberpunk agoto los 512 MiB de VA de bloques
/// (`SIN SITIO`, 06-10). `None` si no hay bloque de paso.
fn con_paso<R>(f: impl FnOnce(&bmo::Memoria) -> R) -> Option<R> {
    // SAFETY: ver `Global`; `f` no toca `CARTA` (lee o escribe el bloque y
    // un fichero suyo), y nadie guarda la referencia.
    let e = unsafe { &mut *CARTA.0.get() };
    if e.paso.is_none() {
        e.paso = Some(bmo::Memoria::request(PASO)?);
    }
    Some(f(e.paso.as_ref()?))
}

/// Un fichero entero: se abre y se trae en trozos por el bloque de paso
/// (el monton de la app se queda con los bytes).
pub(crate) fn leer_fichero(ruta: &[u8]) -> Option<alloc::vec::Vec<u8>> {
    let a = bmo::Archivo::leer_de(ruta).ok()?;
    let n = a.size() as usize;
    let mut v = alloc::vec::Vec::with_capacity(n);
    if n == 0 {
        return Some(v);
    }
    con_paso(|paso| {
        while v.len() < n {
            let k = (n - v.len()).min(PASO as usize) as u64;
            let got = a.leer_en(paso, 0, k).min(k);
            // SAFETY: `got` bytes que el kernel acaba de escribir en el bloque de paso.
            v.extend_from_slice(unsafe { core::slice::from_raw_parts(paso.base() as *const u8, got as usize) });
            if got < k {
                break;
            }
        }
    })?;
    (v.len() == n).then_some(v)
}

/// Un fichero entero, de UNA llamada (P4f3): los bytes a un bloque y
/// `Archivo::escribir_de`, en vez de siete bytes por llamada con `write`.
/// Sin bloque libre (son ocho), el camino lento: sale igual.
/// **Escribir un fichero del `.exe`** (relevo 01-10, paso 4b): lo de
/// `proton-x/` (el perfil de un juego) va a ESTRATOS; lo demas, a FAT32 como
/// siempre, y si FAT32 no puede (su carpeta solo esta en ESTRATOS), a
/// ESTRATOS.
pub(crate) fn escribir_fichero(ruta: &[u8], bytes: &[u8]) -> bool {
    if ruta.starts_with(b"proton-x/") {
        return escribir_en_estratos(ruta, bytes);
    }
    escribir_en_fat32(ruta, bytes) || escribir_en_estratos(ruta, bytes)
}

/// ESTRATOS montado y escribible.
fn estratos_escribible() -> bool {
    bmo::info(bmo::INFO_ES_MONTADO) != 0 && bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0
}

/// Guardar en ESTRATOS: lo crea, o publica su version nueva (la de antes se
/// queda en el historial: guardar encima no pierde nada). Un fichero VACIO
/// solo se puede crear (ESTRATOS no publica una version de cero bytes).
fn escribir_en_estratos(ruta: &[u8], bytes: &[u8]) -> bool {
    if !estratos_escribible() {
        return false;
    }
    if bytes.is_empty() {
        return bmo::estratos::crear_fichero(ruta, &[]) != 0;
    }
    // ESTRATOS publica la version de UNA vez: el fichero entero en un
    // bloque. El de paso si cabe (V3); si no, uno para el (raro: > 1 MiB).
    let guardar = |m: &bmo::Memoria| {
        // SAFETY: un bloque nuestro de al menos `bytes.len()` bytes.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), m.base(), bytes.len()) };
        bmo::estratos::guardar_desde(ruta, m.handle(), 0, bytes.len() as u64) != 0
    };
    if bytes.len() as u64 <= PASO {
        if let Some(g) = con_paso(guardar) {
            return g;
        }
    }
    let Some(m) = bmo::Memoria::request(bytes.len() as u64) else {
        return false;
    };
    let g = guardar(&m);
    m.soltar();
    g
}

/// **Las carpetas en ESTRATOS** (relevo 01-10, paso 4b): crear, quitar y
/// renombrar. En FAT32 no se puede desde Ring 3: lo de alli contesta que no.
const CARPETAS: bmo_proton_x_casa::Carpetas = bmo_proton_x_casa::Carpetas {
    crear: |r| estratos_escribible() && bmo::estratos::crear_carpeta(r) != 0,
    quitar: |r| estratos_escribible() && bmo::estratos::quitar(r) != 0,
    renombrar: |r, nuevo| estratos_escribible() && bmo::estratos::renombrar(r, nuevo) != 0,
};

fn escribir_en_fat32(ruta: &[u8], bytes: &[u8]) -> bool {
    let Ok(a) = bmo::Archivo::create(ruta) else {
        return false;
    };
    // Por el bloque de paso, en trozos (V3): sin pedir bloque.
    let por_paso = |paso: &bmo::Memoria| {
        let mut hecho = 0usize;
        while hecho < bytes.len() {
            let k = (bytes.len() - hecho).min(PASO as usize);
            // SAFETY: `k` <= PASO, la medida del bloque de paso, que es nuestro.
            unsafe { core::ptr::copy_nonoverlapping(bytes[hecho..].as_ptr(), paso.base(), k) };
            let w = (a.escribir_de(paso, 0, k as u64) as usize).min(k);
            hecho += w;
            if w < k {
                break;
            }
        }
        hecho
    };
    let n = if bytes.is_empty() { 0 } else { con_paso(por_paso).unwrap_or_else(|| a.write(bytes)) };
    a.close() && n == bytes.len()
}

/// Lo que hay en una carpeta del volumen (P4f3), con el nombre 8.3 como lo
/// muestra BMO-X (`cobol.bex`, en minuscula). `""` es la raiz.
fn listar(ruta: &[u8]) -> Option<alloc::vec::Vec<bmo_proton_x::ficheros::Entrada>> {
    let d = bmo::Directorio::open(ruta).ok()?;
    let mut v = alloc::vec::Vec::new();
    // ** N2 (29-09): una carpeta del disco Personal (`d:`) trae nombres NTFS
    // ENTEROS en UTF-8 y medidas de 64 bits, no 8.3: se piden asi.
    if ruta.len() >= 2 && ruta[0] | 0x20 == b'd' && ruta[1] == b':' {
        let mut n = [0u8; 256];
        while let Some((k, carpeta, bytes)) = d.siguiente_largo(&mut n) {
            let nombre = alloc::string::String::from_utf8_lossy(&n[..k]).into_owned();
            // `.` y los ficheros del propio NTFS (`$MFT`...) no son del juego.
            if nombre == "." || nombre == ".." || nombre.starts_with('$') {
                continue;
            }
            // Y sus fechas y atributos de NTFS (01-10): sin ellas un fichero es
            // de 1601, y Cyberpunk toma su `final.redscripts` por roto.
            let (fechas, atributos) = d.fechas();
            v.push(bmo_proton_x::ficheros::Entrada {
                nombre,
                carpeta,
                bytes,
                fechas,
                atributos,
            });
        }
        return Some(v);
    }
    // El volumen de BMO-X: FAT32 (8.3) y ESTRATOS (nombres enteros, 01-10)
    // en la misma carpeta.
    let mut n = [0u8; 256];
    while let Some((k, carpeta, bytes)) = d.siguiente_todo(&mut n) {
        let nombre = alloc::string::String::from_utf8_lossy(&n[..k]).into_owned();
        v.push(bmo_proton_x::ficheros::Entrada {
            nombre,
            carpeta,
            bytes,
            ..Default::default()
        });
    }
    Some(v)
}

/// **A la carta** (01-10): la medida de un fichero sin traerlo, y un rango.
const TROZOS: bmo_proton_x_casa::Trozos = bmo_proton_x_casa::Trozos {
    medida: trozo_medida,
    leer: trozo_leer,
    umbral: 1 << 20,
};

/// Los ficheros abiertos a la carta que se quedan abiertos: el kernel tiene
/// 16 ranuras para TODO el sistema, asi que pocos, y se cierra el mas viejo.
const ABIERTOS: usize = 3;
/// El bloque de paso: el kernel escribe en un bloque SUYO (`leer_en` va por
/// capability, no por puntero) y de ahi se copia al `.exe`. Uno, y se queda:
/// pedir y soltar uno por `ReadFile` serian dos llamadas mas por lectura.
const PASO: u64 = 1 << 20;

struct Carta {
    ruta: alloc::vec::Vec<u8>,
    a: bmo::Archivo,
    /// Donde esta el cursor del kernel: leer seguido no salta.
    pos: u64,
    uso: u64,
}

struct ALaCarta {
    abiertos: alloc::vec::Vec<Carta>,
    paso: Option<bmo::Memoria>,
    reloj: u64,
}

struct Global(core::cell::UnsafeCell<ALaCarta>);
// SAFETY: una tarea, y los hilos de la casa son cooperativos.
// [hilos] cerrojo -- el bloque de paso y los ficheros a la carta: los lee cualquier hilo
unsafe impl Sync for Global {}
static CARTA: Global = Global(core::cell::UnsafeCell::new(ALaCarta {
    abiertos: alloc::vec::Vec::new(),
    paso: None,
    reloj: 0,
}));

fn trozo_medida(ruta: &[u8]) -> Option<u64> {
    let a = bmo::Archivo::reflejar(ruta).ok()?;
    let m = a.size();
    a.close();
    Some(m)
}

fn trozo_leer(ruta: &[u8], desde: u64, dst: &mut [u8]) -> Option<usize> {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    let e = unsafe { &mut *CARTA.0.get() };
    e.reloj += 1;
    if e.paso.is_none() {
        e.paso = Some(bmo::Memoria::request(PASO)?);
    }
    let k = match e.abiertos.iter().position(|c| c.ruta == ruta) {
        Some(k) => k,
        None => {
            if e.abiertos.len() >= ABIERTOS {
                let viejo = (0..e.abiertos.len())
                    .min_by_key(|&k| e.abiertos[k].uso)
                    .unwrap_or(0);
                e.abiertos.swap_remove(viejo).a.close();
            }
            let a = bmo::Archivo::reflejar(ruta).ok()?;
            e.abiertos.push(Carta {
                ruta: ruta.to_vec(),
                a,
                pos: 0,
                uso: 0,
            });
            e.abiertos.len() - 1
        }
    };
    let (c, paso) = (&mut e.abiertos[k], e.paso.as_ref()?);
    c.uso = e.reloj;
    if c.pos != desde {
        c.pos = c.a.saltar(desde);
        if c.pos != desde {
            return Some(0);
        }
    }
    let mut hecho = 0usize;
    while hecho < dst.len() {
        let n = ((dst.len() - hecho) as u64).min(PASO);
        let got = c.a.leer_en(paso, 0, n);
        // SAFETY: `got` bytes que el kernel acaba de escribir en el bloque de paso.
        unsafe {
            core::ptr::copy_nonoverlapping(paso.base(), dst[hecho..].as_mut_ptr(), got as usize)
        };
        hecho += got as usize;
        c.pos += got;
        if got < n {
            break;
        }
    }
    Some(hecho)
}

/// Una arena del monton de Windows (P4e): un bloque del kernel, que ya viene
/// a ceros y vive lo que el proceso (uno de los ocho).
fn memoria(bytes: usize) -> Option<u64> {
    let m = bmo::Memoria::request(bytes as u64)?;
    let base = m.base() as u64;
    core::mem::forget(m);
    Some(base)
}

/// **El tramo del MONTON de la casa** (07-10): los ultimos 16 GiB de la
/// ventana de reserva son SOLO del monton del cargador (`main::MONTON`, que
/// crece por aqui al llenar sus 64 MiB); la casa de Windows no los ve. Lo
/// pidio el metal: Cyberpunk lleno el monton a los 17,7 s con la RAM casi
/// vacia (`memory allocation of 3670016 bytes failed`).
pub(crate) const TRAMO_MONTON: u64 = 16 << 30;
pub(crate) const TRAMO_MONTON_BASE: u64 = bmo::reserva::VENTANA_BASE + bmo::reserva::VENTANA_BYTES - TRAMO_MONTON;

/// **Hacer las paginas de un trozo del monton** (07-10): corre DENTRO del
/// asignador (con su cerrojo), asi que NO pide memoria: sin textos, sin
/// avisos. El NO lo dice el panico del asignador, con su medida.
pub(crate) fn monton_hacer(va: u64, bytes: u64) -> bool {
    let mut hecho = 0u64;
    while hecho < bytes {
        let k = bmo::reserva::MAX_POR_VEZ.min(bytes - hecho);
        if bmo::reserva::hacer(va + hecho, k).is_err() {
            return false;
        }
        hecho += k;
    }
    true
}

/// **La RESERVA del kernel** (P0.4c): la ventana de `TASK_OP_RESERVA_*`,
/// menos el tramo del monton (07-10).
const RESERVA: bmo_proton_x_casa::Reserva = bmo_proton_x_casa::Reserva {
    base: bmo::reserva::VENTANA_BASE,
    bytes: bmo::reserva::VENTANA_BYTES - TRAMO_MONTON,
    hacer: reserva_hacer,
    deshacer: reserva_deshacer,
    ram,
};

/// Hacer `[va, va + bytes)`, en trozos de lo mas que el kernel hace de una
/// vez. Si dice que no, se dice por que (una vez por NO, con sus numeros) y
/// `false`: lo hecho hasta ahi se queda hecho (y vuelve al deshacer).
fn reserva_hacer(va: u64, bytes: u64) -> bool {
    let mut hecho = 0u64;
    while hecho < bytes {
        let k = bmo::reserva::MAX_POR_VEZ.min(bytes - hecho);
        if let Err(no) = bmo::reserva::hacer(va + hecho, k) {
            let v = no.valor;
            escribir(alloc::format!("PROTON-X: la RESERVA dice NO a {} MiB en {:#x}: {} [valor {:#x}: pide {} MiB, hay {} MiB]\n", bytes >> 20, va, no.frase(), v, v >> 32, v & 0xFFFF_FFFF).as_bytes());
            return false;
        }
        hecho += k;
    }
    true
}

fn reserva_deshacer(va: u64, bytes: u64) {
    let mut hecho = 0u64;
    while hecho < bytes {
        let k = bmo::reserva::MAX_POR_VEZ.min(bytes - hecho);
        let _ = bmo::reserva::deshacer(va + hecho, k);
        hecho += k;
    }
}

/// La RAM de la maquina: total y libre AHORA (para GlobalMemoryStatus).
fn ram() -> (u64, u64) {
    (
        bmo::info(bmo::INFO_RAM_TOTAL),
        bmo::info(bmo::INFO_RAM_LIBRE),
    )
}

/// La fecha de la placa (el RTC, `INFO_FECHA`), en segundos desde 1970
/// (P4f2). La placa no dice su zona: se toma como UTC.
fn fecha() -> Option<u64> {
    let f = bmo_rtc::desempaquetar(bmo::info(bmo::INFO_FECHA))?;
    Some(bmo_proton_x::hora::segundos_unix(
        f.anio, f.mes, f.dia, f.hora, f.minuto, f.segundo,
    ))
}

fn soltar_codigo(base: u64, _bytes: usize) {
    // SAFETY: ver `Codigo`.
    let v = unsafe { &mut *CODIGO.0.get() };
    if let Some(i) = v.iter().position(|m| m.base() as u64 == base) {
        v.swap_remove(i).soltar();
    }
}

/// Un GS que el kernel no acepta seria un hilo sin TEB: no se sigue.
fn poner_gs(teb: u64) {
    if bmo::poner_gs(teb).is_err() {
        bmo::consola("PROTON-X: el kernel no pone el GS de un hilo: no se sigue\n");
        super::fin_del_exe(0xC000_0005);
    }
}

/// La frecuencia del TSC, pedida una vez (0 = todavia no).
static TSC_HZ: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

pub(crate) fn ahora_ns() -> u64 {
    use core::sync::atomic::Ordering;
    let mut hz = TSC_HZ.load(Ordering::Relaxed);
    if hz == 0 {
        hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
        TSC_HZ.store(hz, Ordering::Relaxed);
    }
    (bmo::ciclos() as u128 * 1_000_000_000 / hz as u128) as u64
}

/// La consola es de lineas: el retorno de carro de Windows pintaria un
/// caracter de mas. Un `\r` delante de `\n` no se manda.
fn escribir(bytes: &[u8]) {
    // 05-10: los avisos de la casa, tambien al cartel rojo de la ventana.
    super::cartel::oir(bytes);
    let mut desde = 0;
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            decir(&bytes[desde..i]);
            desde = i + 1;
        }
    }
    decir(&bytes[desde..]);
}

fn decir(b: &[u8]) {
    match core::str::from_utf8(b) {
        Ok(s) => bmo::consola(s),
        Err(_) => {
            for &c in b {
                bmo::consola(if c.is_ascii() {
                    core::str::from_utf8(core::slice::from_ref(&c)).unwrap_or("?")
                } else {
                    "?"
                });
            }
        }
    }
}

fn salir(codigo: u32) -> ! {
    // Con el diario: las ultimas llamadas del anillo, al fichero (01-10).
    bmo_proton_x_casa::diario::al_salir(codigo);
    super::fin_del_exe(codigo)
}

/// La cabecera BSUP, palabra a palabra. `volatile`: la lee otro proceso.
fn pon(base: u64, i: u64, v: u32) {
    // SAFETY: `base` es un bloque nuestro con la cabecera dentro.
    unsafe { core::ptr::write_volatile((base + 4 * i) as *mut u32, v) };
}

fn campo(base: u64, i: u64) -> u32 {
    // SAFETY: como arriba.
    unsafe { core::ptr::read_volatile((base + 4 * i) as *const u32) }
}

/// P3b4c.9 Z1: la cabecera de la ultima superficie (0 = ninguna), para
/// pedirle al escritorio la pantalla directa.
static SUP_BASE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **P3b4c.9 Z1: pedirle al escritorio que la 3060 dibuje DIRECTO en la
/// pantalla** (`si`), o volver a que la componga (`SUP_BGRA32`). El
/// escritorio relee la cabecera cada vuelta; quien decide si se da es el, y
/// quien dibuja alli, el kernel.
pub(crate) fn pedir_pantalla(si: bool) {
    let base = SUP_BASE.load(core::sync::atomic::Ordering::Acquire);
    if base != 0 {
        pon(
            base,
            4,
            if si {
                bmo::SUP_LA_3060_DIRECTA
            } else {
                bmo::SUP_BGRA32
            } as u32,
        );
    }
}

fn base_de(s: &Superficie) -> u64 {
    s.pixeles as u64 - bmo::SUP_CABECERA
}

fn superficie(ancho: u32, alto: u32) -> Option<Superficie> {
    let pixeles = ancho as u64 * alto as u64 * 4;
    let buzon = bmo::SUP_CABECERA + pixeles;
    let bytes = buzon + bmo::SUP_BUZON_CABECERA + RANURAS * bmo::SUP_BUZON_RANURA;
    let bloque = bmo::Memoria::request(bytes)?;
    let base = bloque.base() as u64;
    pon(base, 0, bmo::SUP_MAGIC as u32);
    pon(base, 1, ancho);
    pon(base, 2, alto);
    pon(base, 3, ancho);
    pon(base, 4, bmo::SUP_BGRA32 as u32);
    pon(base, bmo::SUP_CAMPO_SECUENCIA, 0);
    pon(base, 6, buzon as u32);
    pon(base, 7, RANURAS as u32);
    // Cabeza, cola y estado a cero ANTES de ofrecer.
    for i in 0..4 {
        pon(buzon + base, i, 0);
    }
    let s = Superficie {
        pixeles: (base + bmo::SUP_CABECERA) as *mut u32,
        ancho,
        alto,
        stride: ancho,
        dato: bloque.handle(),
    };
    SUP_BASE.store(base, core::sync::atomic::Ordering::Release);
    // Vive hasta que el proceso muera: el escritorio la esta leyendo.
    core::mem::forget(bloque);
    Some(s)
}

fn mostrar(s: &Superficie) -> bool {
    let padre = bmo::mi_padre();
    if padre == 0 {
        return false;
    }
    let bytes =
        campo(base_de(s), 6) as u64 + bmo::SUP_BUZON_CABECERA + RANURAS * bmo::SUP_BUZON_RANURA;
    bmo::offer(s.dato, 0, bytes, padre)
}

fn presentar(s: &Superficie) {
    // 05-10: si la casa dijo que algo fue mal, el cartel rojo encima, antes
    // de que el escritorio vea el fotograma.
    if super::cartel::cuantos() > 0 {
        // SAFETY: los pixeles de la superficie: `stride * alto`, nuestros.
        let px = unsafe { core::slice::from_raw_parts_mut(s.pixeles, s.stride as usize * s.alto as usize) };
        super::cartel::pintar(px, s.ancho, s.alto, s.stride);
    }
    let base = base_de(s);
    pon(
        base,
        bmo::SUP_CAMPO_SECUENCIA,
        campo(base, bmo::SUP_CAMPO_SECUENCIA).wrapping_add(1),
    );
}

/// El siguiente evento del buzon: la cabeza la escribe el escritorio, la cola
/// esta app (ver `superficie/amarilla.inti`).
fn evento(s: &Superficie) -> u64 {
    let base = base_de(s);
    let buzon = base + campo(base, 6) as u64;
    let (cabeza, cola) = (campo(buzon, 0), campo(buzon, 1));
    if cabeza == cola {
        return 0;
    }
    let mascara = RANURAS as u32 - 1;
    let ranura = buzon + bmo::SUP_BUZON_CABECERA + (cola & mascara) as u64 * bmo::SUP_BUZON_RANURA;
    // SAFETY: la ranura cae dentro del buzon de nuestro bloque.
    let e = unsafe { core::ptr::read_volatile(ranura as *const u64) };
    pon(buzon, 1, (cola + 1) & mascara);
    e
}

fn dormir() {
    bmo::wait(0, 0, 4_000_000);
}
