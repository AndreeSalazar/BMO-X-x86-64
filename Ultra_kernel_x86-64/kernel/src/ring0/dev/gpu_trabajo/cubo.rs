//! **X5: EL CUBO POR LA 3060, SIN WINDOWS** -- un fotograma del cubo del
//! estudio D3D dibujado por el pipeline 3D de la 3060 en una ventana de
//! 1280x720 de la pantalla (el framebuffer del GOP), y leido de vuelta para
//! que el escritorio saque su huella. Que se dibuja: `bmo_cubo::tanda` (las
//! cuentas del juez); como: `bmo_gpu_ga10x::cubo` (programas y ordenes).
//!
//! [carril]  ROJO      la 3060 escribe en la memoria que el monitor ESCANEA:
//!                     solo la ventana, dentro del mapa de `pantalla`
//! [consumo] NADA      corre por orden (`gpu cubo 3060`): un dibujo de unos
//!                     cientos de us, y la lectura de vuelta a pedazos
//!
//! Dos subordenes en `IOMMU_OP_GPU_CUBO`:
//!
//! ```text
//!    DIBUJAR  arg = la ficha de S3 (0..31) y el fotograma (32..40)
//!             Ok(cubo::empaquetar(..)): us de la 3060, triangulos, escalera
//!    LEER     arg = CUBO_LEER | k: los pixeles 2k y 2k+1 de la ventana, fila
//!             a fila, como 0x00RRGGBB (Ok(p0 | p1 << 32)); solo tras DIBUJAR
//!    VERRANO  arg = CUBO_VERRANO | la VA de un paquete del escritorio
//!             (`tuberia::Paquete`: sus dos programas, tomados del BSF, y sus
//!             vertices). La tuberia FIJA de VERRANO V0: el kernel sube el
//!             codigo tal cual, no traduce nada. Ok como DIBUJAR, con lo que
//!             costo preparar (`cubo::con_preparar`)
//!    LIGERO   con VERRANO: las ordenes SIN la escalera de T1c (V1)
//!    ANILLO   con VERRANO: EL ANILLO (V1b, `bmo_gpu_ga10x::anillo`): los
//!             vertices en RAM del PC y el fotograma EN VUELO -- se envia y
//!             se vuelve, sin esperar a la 3060 (`cubo::es_en_vuelo`)
//!    VACIAR   arg = CUBO_VACIAR: esperar lo que el anillo dejo en vuelo
//!    COOPERA  con ANILLO (V1c): la limpieza RECORTADA a lo que el
//!             escritorio dice en el paquete (`Paquete::limpiar`): la CPU
//!             sabe donde estaba y donde va a estar el cubo, y la 3060 solo
//!             limpia eso. El primero, al armar, limpia la ventana entera
//! ```
//!
//! ** EL ANILLO (V1b, 26-09): la CPU ORQUESTA, no espera. El primer
//! fotograma lo ARMA en frio (todo releido, y se espera a que se pague);
//! los siguientes, mientras sean los mismos programas y la misma ventana y
//! nadie mas pase por el GR, solo copian sus vertices a su ranura de RAM,
//! escriben la cola de sus ordenes y tocan el timbre. Cuatro ranuras: se
//! espera solo si la 3060 va CUATRO fotogramas por detras. Cualquier otro
//! trabajo del GR (`gr_ocupado`) y LEER esperan antes a que se vacie: el
//! anillo comparte con ellos el tramo (programas, tabla, ordenes).
//!
//! ** EN CALIENTE (V1, 26-09): un dibujo de VERRANO con los MISMOS
//! programas, triangulos, ventana y modo que el anterior, sin que nadie haya
//! lanzado nada por el GR entre medias y a menos de 100 ms, solo escribe lo
//! que cambia (`tuberia::preparar_caliente`). Lo decide ESTE fichero, no el
//! escritorio: el escritorio no puede pedir "en caliente".
//!
//! La lectura va de dos en dos y no la ventana entera en una llamada por lo
//! mismo que D2 (`pantalla::FILA`): un syscall corre con las interrupciones
//! cerradas, y 921.600 pixeles leidos de la VRAM son demasiado tiempo sin
//! reloj.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bmo_gpu_ga10x::cubo as cu;

use super::pantalla::{asegurar_mapa, la_pantalla, IOMMU_NO_PANTALLA};
use super::{esperando, gr_ocupado, BLUR_ENTRADA, BLUR_EN_MARCHA, DIAG_3D, FRACTAL_ESPERA_US, IOMMU_NO_BLUR, IOMMU_NO_BLUR_PREPARAR, LIENZO_HECHO};
use super::porque::{apuntar, no};
use crate::ring0::dev::gpu_prestamo::Bar0;
use bmo_gpu_ga10x::porque::Porque;

/// El bit de `arg` que pide LEER en vez de DIBUJAR.
pub const CUBO_LEER: u64 = 1 << 63;

/// El bit de `arg` que pide dibujar un paquete de VERRANO V0.
pub const CUBO_VERRANO: u64 = 1 << 62;

/// Con VERRANO: las ordenes sin la escalera (`tuberia::ordenes_con`).
pub const CUBO_LIGERO: u64 = 1 << 61;

/// Con VERRANO: el anillo (V1b).
pub const CUBO_ANILLO: u64 = 1 << 60;

/// Solo: vaciar el anillo.
pub const CUBO_VACIAR: u64 = 1 << 59;

/// Con el anillo: la limpieza recortada (V1c).
pub const CUBO_COOPERA: u64 = 1 << 58;

/// **TOMA TU BODRIO** en la puerta: un programa del paquete de VERRANO que
/// el juez del SASS (`bmo_gpu_ga10x::sass::juez`) rechaza no se sube.
pub const IOMMU_NO_BODRIO: u32 = 87;

/// Lo que dejo el ultimo dibujo de VERRANO pagado entero: la huella de lo
/// fijo (0 = nada que reusar), la entrada del GR tras el, y cuando.
static CALIENTE_HUELLA: AtomicU64 = AtomicU64::new(0);
static CALIENTE_ENTRADA: AtomicU32 = AtomicU32::new(u32::MAX);
static CALIENTE_TSC: AtomicU64 = AtomicU64::new(0);
/// La huella de los programas del ultimo dibujo pagado (ya juzgados).
static CALIENTE_PROGRAMAS: AtomicU64 = AtomicU64::new(0);

/// EL ANILLO: su pagina de RAM (los vertices de las cuatro ranuras) y si ya
/// se presto y mapeo (una vez por arranque).
static ANILLO_F: AtomicU64 = AtomicU64::new(0);
static ANILLO_PRESTADO: AtomicBool = AtomicBool::new(false);
/// La huella de lo fijo del anillo armado (0 = sin armar), la entrada del GR
/// tras su ultimo envio, y cuando.
static ANILLO_HUELLA: AtomicU64 = AtomicU64::new(0);
static ANILLO_ENTRADA: AtomicU32 = AtomicU32::new(u32::MAX);
static ANILLO_TSC: AtomicU64 = AtomicU64::new(0);
/// El numero del ultimo fotograma ENVIADO y el del ultimo visto PAGADO.
static ANILLO_NUMERO: AtomicU32 = AtomicU32::new(0);
static ANILLO_PAGADO: AtomicU32 = AtomicU32::new(0);

/// Ya se dibujo en este arranque (LEER antes no tiene que leer).
static DIBUJADO: AtomicBool = AtomicBool::new(false);
/// Cuantos fotogramas del cubo dibujo la 3060 (para el log).
static VECES: AtomicU32 = AtomicU32::new(0);

/// **`IOMMU_OP_GPU_CUBO`**.
pub fn cubo(arg: u64) -> Result<u64, u32> {
    if arg & CUBO_LEER != 0 {
        return leer(arg & 0xF_FFFF);
    }
    if arg == CUBO_VACIAR {
        return vaciar_con_cerrojo();
    }
    if arg & CUBO_VERRANO != 0 {
        let anillo = arg & CUBO_ANILLO != 0;
        return verrano(arg & 0xFFFF_FFFF_FFFF, arg & CUBO_LIGERO != 0, anillo, anillo && arg & CUBO_COOPERA != 0);
    }
    let (ficha, f) = (arg & 0xFFFF_FFFF, (arg >> 32) as u32 & 0x1FF);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) || f >= 360 {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    // La tanda: las cuentas del juez, en bits para el driver.
    let Some(t) = bmo_cubo::tanda::de_fotograma(f, cu::ANCHO, cu::ALTO) else { return Err(IOMMU_NO_BLUR_PREPARAR) };
    let mut tris = [cu::Triangulo { clip: [[0; 4]; 3], color: [0; 4] }; cu::CABEN];
    for (d, s) in tris.iter_mut().zip(t.tris()) {
        *d = cu::Triangulo { clip: s.clip.map(|v| v.map(f32::to_bits)), color: s.color.map(f32::to_bits) };
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || gr_ocupado() {
        return Err(IOMMU_NO_BLUR);
    }
    // Lo que el volcado del escritorio tenga en vuelo, antes: si no, su copia
    // podria caer ENCIMA del cubo entre el dibujo y la lectura.
    let tris = &tris[..t.n];
    let r = super::volcado::quieto().and_then(|()| dibujar(bar0, ficha as u32, e, &p, t.n as u32, true, |r| cu::preparar(r, e, &v, tris).then_some(()).ok_or(0)));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **VERRANO V0**: el paquete del escritorio (sus dos programas y sus
/// vertices) por la tuberia fija. Con `anillo`, por el anillo (V1b).
fn verrano(va: u64, ligero: bool, anillo: bool, coopera: bool) -> Result<u64, u32> {
    use bmo_gpu_ga10x::tuberia as tu;
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    // El paquete: un bloque de quien lo pide, entero, dentro del physmap.
    let pid = crate::ring0::task::scheduler::current_pid();
    let dentro = |fisica: u64, bytes: u64| fisica.checked_add(bytes).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE);
    // La cabecera MAS GRANDE (VRN1, 64 B): un paquete V0 mide mucho mas
    // (sus dos programas), asi que leerla nunca se sale de un paquete bueno.
    let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, tu::CABECERA_MAX as u64).filter(|&f| dentro(f, tu::CABECERA_MAX as u64)) else {
        crate::ring0::cabina::warn("gpu", "VERRANO: el paquete no es un bloque de quien lo pide", va);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    // SAFETY: `fisica_de` dio CABECERA bytes de un bloque del proceso, dentro
    // del physmap (comprobado); solo se leen.
    let cabecera = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, tu::CABECERA_MAX) };
    let Some(total) = tu::medida(cabecera) else { return Err(IOMMU_NO_BLUR_PREPARAR) };
    let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, total as u64).filter(|&f| dentro(f, total as u64)) else {
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    // SAFETY: como arriba, con `total` bytes; el escritorio esta dentro de
    // esta llamada y no los toca mientras.
    let bytes = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, total) };
    let Some(paquete) = tu::leer(bytes) else {
        crate::ring0::cabina::warn("gpu", "VERRANO: el paquete no se sostiene (programas o vertices); bytes", total as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    if !bmo_gpu_ga10x::computo::ficha_valida(paquete.ficha as u64) {
        return Err(IOMMU_NO_BLUR);
    }
    // ** EL JUEZ EN LA PUERTA (J2, 26-09): lo que el escritorio diga haber
    // juzgado no cuenta; aqui se juzga otra vez, con los registros que las
    // ordenes le dan (`tuberia::REGISTROS`), y un BODRIO no llega a la 3060.
    // En caliente no hace falta: la huella de lo fijo lleva los programas,
    // y los de un dibujo pagado ya pasaron por aqui. En el anillo, igual:
    // los de un anillo ARMADO ya pasaron por aqui al armarlo.
    let huella_anillo = if anillo { bmo_gpu_ga10x::anillo::huella(&v, &paquete, coopera) } else { 0 };
    let ya_juzgados = if anillo { ANILLO_HUELLA.load(Ordering::Acquire) == huella_anillo } else { caliente_posible(&paquete) };
    if !ya_juzgados {
        let r = bmo_gpu_ga10x::tuberia::REGISTROS;
        for (cual, prog) in [("vertice", paquete.vs), ("pixel", paquete.ps)] {
            if let Err(b) = bmo_gpu_ga10x::sass::juez::juzgar_programa(prog, r) {
                crate::ring0::cabina::warn("gpu", cual, b.instruccion as u64);
                crate::ring0::cabina::warn("gpu", b.regla.nombre(), b.que as u64);
                crate::ring0::cabina::warn("gpu", "[BMO-X Juez V3b]: TOMA TU BODRIO! no sube a la 3060; instruccion", b.instruccion as u64);
                crate::ring0::cabina::warn("gpu", bmo_gpu_ga10x::sass::juez::REMATE, 87);
                return Err(IOMMU_NO_BODRIO);
            }
        }
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if anillo {
        // El anillo toma el cerrojo SIN vaciarse a si mismo: es el unico que
        // puede seguir con fotogramas suyos en vuelo.
        if !bmo_gpu_ga10x::blur::entrada_valida(e) || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
            return Err(IOMMU_NO_BLUR);
        }
        let r = en_anillo(bar0, e, &p, &v, &paquete, huella_anillo, coopera);
        BLUR_EN_MARCHA.store(false, Ordering::Release);
        return r;
    }
    en_frio(bar0, pid, e, &p, &v, &paquete, ligero, false)
}

// == P3b4c: LA PUERTA DE LAS APPS =============================================

/// El taller donde el kernel pega los programas de una receta: ~6 KiB,
/// ESTATICO (fuera de la pila del syscall). Lo guarda [`TALLER_EN_USO`].
struct TallerFijo(core::cell::UnsafeCell<bmo_gpu_ga10x::receta::Taller>);
// SAFETY: solo se toca con `TALLER_EN_USO` tomado (ver `receta`).
unsafe impl Sync for TallerFijo {}
static TALLER: TallerFijo = TallerFijo(core::cell::UnsafeCell::new(bmo_gpu_ga10x::receta::Taller::nuevo()));
static TALLER_EN_USO: AtomicBool = AtomicBool::new(false);

/// **`IOMMU_OP_GPU_DIBUJAR`: la PUERTA ESTRECHA** (P3b4c, 28-09) -- la unica
/// orden de la 3060 que no pide la autoridad `MAQUINA` (la desvia
/// `syscall::op_maquina::iommu` antes de sus dos llaves). Lo que la hace
/// estrecha, todo aqui y todo comprobado:
///
/// ```text
///    lo que manda   una RECETA (VRN2) en un bloque suyo: los CUERPOS, nunca
///                   un programa entero (`receta`: juez R7, lista blanca)
///    las lecturas   las pone el KERNEL (el pegamento): cada una dentro de
///                   los DATOS de la receta (`receta::leer`)
///    donde pinta    en SU RAM (el destino es obligatorio y tiene que ser un
///                   bloque ESCRIBIBLE suyo), nunca en la pantalla
///    la ficha       la del GR que preparo el escritorio (`ficha_del_gr`):
///                   sin escritorio que la haya preparado, NO
///    el cerrojo     el del GR, como `gpu verrano`: si esta ocupado, NO (la
///                   app dibuja ese lote en la CPU y lo dice)
/// ```
/// **P3b4c.9 Z6: el RESTO, partido** (29-09). El metal de las 08:18 dio
/// `resto 105` us de 247 por lote sin saber de quien. Cada pieza de
/// [`receta`] suma aqui sus ciclos (y `[0]` los lotes); `INFO_RECETA` los
/// da en ns y PROTON-X los parte por segundo en su linea `[resto]`:
///
/// ```text
///    0 lotes       1 leer (validar, las dos `fisica_de`, `rc::leer`)
///    2 pegar (`ya_pegada` o `pegar`)   3 prestar las texturas
///    4 el paquete y la pantalla        5 `en_frio` entero (dentro van la
///    3060, preparar y la sombra: los resta quien lee)
///    6 devolver las texturas           7 la receta ENTERA
/// ```
pub static RECETA_PARTES: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

/// Suma a la pieza `k` lo que va de `desde` a ahora, y devuelve ahora.
fn pieza(k: usize, desde: u64) -> u64 {
    let ahora = crate::ring0::task::scheduler::rdtsc();
    RECETA_PARTES[k].fetch_add(ahora.wrapping_sub(desde), Ordering::Relaxed);
    ahora
}

/// `INFO_RECETA | k << 8`: la pieza `k` en ns acumulados (`k = 0`: lotes).
pub fn info_receta(c: u64) -> u64 {
    let k = ((c >> 8) & 7) as usize;
    let v = RECETA_PARTES[k].load(Ordering::Relaxed);
    if k == 0 {
        return v;
    }
    let mhz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    v.saturating_mul(1000) / mhz
}

/// La pieza 7 (la receta entera), sumada al SALIR por donde sea: las
/// salidas tempranas tambien cuestan y tambien cuentan.
struct Entera(u64);

impl Drop for Entera {
    fn drop(&mut self) {
        pieza(7, self.0);
    }
}

pub fn receta(va: u64) -> Result<u64, u32> {
    use bmo_gpu_ga10x::receta as rc;
    let inicio = crate::ring0::task::scheduler::rdtsc();
    RECETA_PARTES[0].fetch_add(1, Ordering::Relaxed);
    let _entera = Entera(inicio);
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some(ficha) = super::ficha_del_gr() else {
        crate::ring0::cabina::warn("gpu", "P3b4c: una app pide la 3060 y el escritorio no la preparo (`gpu verrano`, hasta `lienzo`)", va);
        apuntar(Porque::SinEscritorio, 0, 0);
        return Err(IOMMU_NO_BLUR);
    };
    if bar0 == 0 || crate::ring0::dev::gpu_apagar::despedido() {
        apuntar(Porque::SinEscritorio, 1, 0);
        return Err(IOMMU_NO_BLUR);
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    if !crate::ring0::dev::gpu_despertar::bar1_fisica() {
        return Err(IOMMU_NO_PANTALLA);
    }
    // El taller (y la copia de la receta) se toman YA: soltados al salir
    // por donde sea (`TallerTomado`).
    if TALLER_EN_USO.swap(true, Ordering::AcqRel) {
        apuntar(Porque::TallerOcupado, 0, 0);
        return Err(IOMMU_NO_BLUR);
    }
    let _tomado = TallerTomado;
    let pid = crate::ring0::task::scheduler::current_pid();
    let bytes = leer_receta(pid, va)?;
    let total = bytes.len();
    let Some(r) = rc::leer(bytes) else {
        crate::ring0::cabina::warn("gpu", "P3b4c: la receta no se sostiene (medidas, destino, cargas, datos o indices); bytes", total as u64);
        return Err(no(Porque::RecetaNoSeSostiene, 0, total as u64));
    };
    let t0 = pieza(1, inicio);
    // SAFETY: `TALLER_EN_USO` tomado arriba: nadie mas lo toca hasta soltarlo.
    let t = unsafe { &mut *TALLER.0.get() };
    // ** P3b4c.9 Z3: si lo que `pegar` leeria es IGUAL, byte a byte, a lo de
    // la ultima receta que el juez aprobo, el taller ya tiene esos programas
    // pegados y juzgados: no se repite (el metal de las 07:50: "resto" 124 us
    // por lote, casi todo esto). Cualquier diferencia, o un pegado que fallo
    // antes, y se pega y se juzga entero.
    let pegada = if rc::ya_pegada(&r, t) { Ok(()) } else { rc::pegar(&r, t) };
    let t0 = pieza(2, t0);
    let salida = match pegada {
        // ** P3b4c.8 T2: las texturas de la app, prestadas a la 3060 SOLO
        // LECTURA y solo mientras dibuja, con sus TIC y TSC en las piscinas;
        // devueltas SIEMPRE, salga como salga el dibujo.
        Ok(()) => match prestar_texturas(bar0, pid, r.texturas()) {
            Ok(prestadas) => {
                let t0 = pieza(3, t0);
                let mut paquete = rc::paquete(&r, t, ficha);
                // ** P3b4c.9 Z1: a la PANTALLA, si el escritorio se la dio a
                // ESTA app y el destino es un back buffer de la cadena que
                // mide la ventana. Lo decide el kernel; la receta solo dice
                // "es de la cadena".
                let pantalla = a_la_pantalla(pid, &paquete.dibujo);
                paquete.dibujo.pantalla = pantalla;
                let t0 = pieza(4, t0);
                // 9d: con la libreta si la receta trae termometros.
                let libreta = r.termometro_vs.is_some() || r.termometro_ps.is_some();
                // 13-10: con la ESCALERA si la app la pide (`--3060-escalera`):
                // un semaforo por escalon, y un lote que no paga dice DONDE.
                let ligero = !paquete.dibujo.escalera;
                let x = en_frio(bar0, pid, BLUR_ENTRADA.load(Ordering::Acquire), &p, &v, &paquete, ligero, libreta)
                    .map(|x| if pantalla && cu::sano(x) { cu::en_pantalla(x) } else { x });
                let t0 = pieza(5, t0);
                devolver_texturas(&prestadas);
                pieza(6, t0);
                x
            }
            Err(m) => Err(m),
        },
        Err(e) => {
            decir_no_receta(e);
            Err(IOMMU_NO_BODRIO)
        }
    };
    salida
}

/// Suelta el taller (y la copia de la receta) al salir de [`receta`].
struct TallerTomado;

impl Drop for TallerTomado {
    fn drop(&mut self) {
        TALLER_EN_USO.store(false, Ordering::Release);
    }
}

/// **La COPIA de una receta de la reserva** (12-10): ~200 KiB ESTATICOS, como
/// el taller; los guarda el mismo [`TALLER_EN_USO`].
struct CopiaFija(core::cell::UnsafeCell<[u8; bmo_gpu_ga10x::receta::MAX_RECETA]>);
// SAFETY: solo se toca con `TALLER_EN_USO` tomado (ver `leer_receta`).
unsafe impl Sync for CopiaFija {}
static COPIA: CopiaFija = CopiaFija(core::cell::UnsafeCell::new([0; bmo_gpu_ga10x::receta::MAX_RECETA]));

/// **Los bytes de la receta de `pid` en `va`** (con el taller TOMADO):
///
/// ```text
///    en un bloque suyo   leidos donde estan, por el physmap (como siempre)
///    en su RESERVA       COPIADOS aqui pagina a pagina (12-10): la reserva
///                        son marcos sueltos (`obj::reserva`), y el monton de
///                        PROTON-X crece por ella -- la receta de Cyberpunk
///                        vive alli en cuanto el monton pasa de 64 MiB
/// ```
fn leer_receta(pid: u32, va: u64) -> Result<&'static [u8], u32> {
    use bmo_gpu_ga10x::receta as rc;
    use crate::ring0::obj::reserva;
    let dentro = |fisica: u64, bytes: u64| fisica.checked_add(bytes).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE);
    if let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, rc::CABECERA_2 as u64).filter(|&f| dentro(f, rc::CABECERA_2 as u64)) {
        // SAFETY: `fisica_de` dio la cabecera en un bloque del proceso, dentro
        // del physmap (comprobado); solo se lee.
        let cabecera = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, rc::CABECERA_2) };
        let Some(total) = rc::medida(cabecera) else {
            crate::ring0::cabina::warn("gpu", "P3b4c: la cabecera de la receta no se sostiene (VRN2)", va);
            return Err(no(Porque::RecetaCabecera, 0, va));
        };
        let Some(f) = crate::ring0::obj::memory::fisica_de(pid, va, total as u64).filter(|&f| dentro(f, total as u64)) else {
            crate::ring0::cabina::warn("gpu", "P3b4c: la receta entera no cabe en UN bloque de quien la manda; bytes", total as u64);
            return Err(no(Porque::RecetaEnteraNoEsSuya, 0, total as u64));
        };
        // SAFETY: como arriba, con `total` bytes; vive lo que el proceso, y
        // `receta` la usa solo dentro de esta llamada.
        return Ok(unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, total) });
    }
    if !reserva::en_la_ventana(va, rc::CABECERA_2 as u64) {
        crate::ring0::cabina::warn("gpu", "P3b4c: la receta no es un bloque de quien la manda ni de su reserva", va);
        return Err(no(Porque::RecetaNoEsSuya, 0, va));
    }
    // SAFETY: `TALLER_EN_USO` lo tiene quien llama (`receta`): nadie mas.
    let copia = unsafe { &mut *COPIA.0.get() };
    let aspace = crate::ring0::mm::vmm::read_cr3();
    if !copiar_de_la_reserva(aspace, va, &mut copia[..rc::CABECERA_2]) {
        crate::ring0::cabina::warn("gpu", "P3b4c: la cabecera de la receta no esta en paginas HECHAS de la reserva", va);
        return Err(no(Porque::RecetaNoEsSuya, 1, va));
    }
    let Some(total) = rc::medida(&copia[..rc::CABECERA_2]).filter(|&t| t <= copia.len()) else {
        crate::ring0::cabina::warn("gpu", "P3b4c: la cabecera de la receta no se sostiene (VRN2)", va);
        return Err(no(Porque::RecetaCabecera, 0, va));
    };
    if !copiar_de_la_reserva(aspace, va, &mut copia[..total]) {
        crate::ring0::cabina::warn("gpu", "P3b4c: la receta entera no esta en paginas HECHAS de la reserva; bytes", total as u64);
        return Err(no(Porque::RecetaEnteraNoEsSuya, 1, total as u64));
    }
    Ok(&copia[..total])
}

/// Copiar `[va, va + a.len())` de la reserva del espacio `aspace` a `a`, pagina
/// a pagina; `false` si alguna no esta hecha (o no es suya).
pub(super) fn copiar_de_la_reserva(aspace: u64, va: u64, a: &mut [u8]) -> bool {
    use crate::ring0::mm::PAGE;
    let mut hecho = 0usize;
    while hecho < a.len() {
        let dir = va + hecho as u64;
        let Some(marco) = crate::ring0::obj::reserva::marco(aspace, dir, false) else { return false };
        let desde = (dir % PAGE) as usize;
        let n = (PAGE as usize - desde).min(a.len() - hecho);
        // SAFETY: `reserva::marco` dio un marco PROPIO del proceso, dentro del
        // physmap; se leen `n` bytes sin pasar de su pagina.
        let src = unsafe { core::slice::from_raw_parts((crate::ring0::mm::phys_to_virt(marco) + desde as u64) as *const u8, n) };
        a[hecho..hecho + n].copy_from_slice(src);
        hecho += n;
    }
    true
}

/// Las tablas de las texturas y su pagina de piscinas, escritas (una vez
/// por arranque).
static TEXTURAS_MAPEADAS: AtomicBool = AtomicBool::new(false);

/// Las paginas prestadas de cada ranura de textura (0 = nada).
type Prestadas = [u64; bmo_gpu_ga10x::texturas::MAX_TEXTURAS];

/// **P3b4c.9 Z1: la app a la que el ESCRITORIO le dio la pantalla** (0 =
/// a ninguna). Lo pone `pantalla_para`, detras de las dos llaves de la
/// IOMMU: solo el escritorio. Los pid no se reusan (`proc::next_pid`), y
/// el escritorio la quita al irse la app.
static PANTALLA_PID: AtomicU32 = AtomicU32::new(0);

/// **`IOMMU_OP_GPU_PANTALLA_PARA`**: el escritorio le da la pantalla a la
/// app de la tarea `tid` (0 = a nadie; Ring 3 solo conoce tids, y aqui se
/// guarda su pid, que no cambia). Desde ahora sus recetas con un back buffer de la
/// cadena que mida la ventana (`cu::ANCHO` x `cu::ALTO`) dibujan en la
/// ventana de la pantalla y NO en su RAM: el fotograma no sale de la VRAM
/// (el metal del 29-09 07:21: por lote, la sombra por el PCIe ~1,16 ms y el
/// prestamo del back buffer dentro del resto ~2,2 ms).
pub fn pantalla_para(tid: u64) -> Result<u64, u32> {
    let pid = if tid == 0 {
        0
    } else {
        match crate::ring0::task::scheduler::pid_de(tid as u32) {
            Some(pid) => pid,
            None => return Err(IOMMU_NO_PANTALLA),
        }
    };
    let antes = PANTALLA_PID.swap(pid, Ordering::AcqRel);
    if antes != pid {
        crate::ring0::cabina::count("gpu", "P3b4c.9 Z1: la pantalla, a la app (0 = a ninguna); pid", pid as u64);
    }
    Ok(antes as u64)
}

/// Si ESTE dibujo va a la pantalla: la app es la que el escritorio dijo, el
/// destino es de la cadena y mide la ventana.
fn a_la_pantalla(pid: u32, d: &bmo_gpu_ga10x::tuberia::Dibujo) -> bool {
    pid != 0
        && d.cadena
        // 12-10: la mezcla lee la RAM de la app: el dibujo va alli.
        && d.mezcla.trivial()
        // LI2d: con varios render targets, todos a la RAM de la app.
        && d.n_mas == 0
        && PANTALLA_PID.load(Ordering::Acquire) == pid
        && d.destino.is_some_and(|(va, dst)| va % 4096 == 0 && dst.ancho == cu::ANCHO && dst.alto == cu::ALTO)
}

/// **P3b4c.8 T2: prestar las texturas de una receta a la 3060**: cada una
/// es un bloque de QUIEN la manda (lo dice `fisica_de`), prestado SOLO
/// LECTURA en su ranura de `texturas::IOVA`; y sus TIC y TSC, escritos y
/// releidos en las piscinas. Lo que salga mal devuelve lo ya prestado.
fn prestar_texturas(bar0: u64, pid: u32, texturas: &[bmo_gpu_ga10x::texturas::DeApp]) -> Result<Prestadas, u32> {
    use bmo_gpu_ga10x::texturas as tx;
    let mut prestadas: Prestadas = [0; tx::MAX_TEXTURAS];
    if texturas.is_empty() {
        return Ok(prestadas);
    }
    texturas_mapeadas(bar0)?;
    let aspace = crate::ring0::mm::vmm::read_cr3();
    for (k, t) in texturas.iter().enumerate().take(tx::MAX_TEXTURAS) {
        let fisica = crate::ring0::obj::memory::fisica_de(pid, t.va, t.bytes()).filter(|&f| f.checked_add(t.bytes()).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE));
        let paginas = t.paginas();
        let iova = tx::IOVA + k as u64 * tx::RANURA;
        // ** 12-10: o un bloque suyo (seguido), o su RESERVA: las texturas de
        // 64 KiB o mas de un juego viven alli, en marcos sueltos. Solo
        // LECTURA, como siempre; cada pagina HECHA y suya.
        let base = t.va & !0xFFF;
        let marco = |j: u64| crate::ring0::obj::reserva::marco(aspace, base + j * crate::ring0::mm::PAGE, false);
        let prestado = match fisica {
            Some(f) if f % 4096 == t.va % 4096 => crate::ring0::plat::iommu::prestar_gpu(iova, f & !0xFFF, paginas, false),
            None if crate::ring0::obj::reserva::en_la_ventana(t.va, t.bytes()) && (0..paginas).all(|j| marco(j).is_some()) => {
                crate::ring0::plat::iommu::prestar_gpu_marcos(iova, paginas, false, marco)
            }
            _ => {
                crate::ring0::cabina::warn("gpu", "P3b4c.8: una textura no es un bloque de quien manda la receta ni paginas hechas de su reserva", t.va);
                devolver_texturas(&prestadas);
                return Err(no(Porque::TexturaNoEsSuya, k as u8, t.va));
            }
        };
        if let Err(m) = prestado {
            crate::ring0::cabina::warn("gpu", "P3b4c.8: una textura no se pudo prestar a la 3060; motivo de la IOMMU", m as u64);
            devolver_texturas(&prestadas);
            return Err(no(Porque::TexturaNoSePresta, k as u8, t.va));
        }
        prestadas[k] = paginas;
    }
    if !tx::escribir_piscinas(&mut Bar0(bar0), texturas) {
        crate::ring0::cabina::warn("gpu", "P3b4c.8: los TIC y TSC no quedaron en las piscinas; texturas", texturas.len() as u64);
        devolver_texturas(&prestadas);
        return Err(no(Porque::TexturaPiscinas, 0, texturas.len() as u64));
    }
    Ok(prestadas)
}

/// Las ranuras de las texturas y sus piscinas, MAPEADAS para la 3060 (una
/// vez por arranque). LI2f: tambien las de los SRV de un computo.
pub(super) fn texturas_mapeadas(bar0: u64) -> Result<(), u32> {
    use bmo_gpu_ga10x::texturas as tx;
    if !TEXTURAS_MAPEADAS.load(Ordering::Acquire) {
        match tx::mapear(&mut Bar0(bar0)) {
            Some((n, bien)) if n == bien => {
                TEXTURAS_MAPEADAS.store(true, Ordering::Release);
                crate::ring0::cabina::count("gpu", "P3b4c.8: las ranuras de las texturas y sus piscinas MAPEADAS para la 3060; entradas", n as u64);
            }
            _ => {
                crate::ring0::cabina::warn("gpu", "P3b4c.8: las tablas de las texturas no se escribieron o no se releyeron", 0);
                return Err(no(Porque::TexturasTablas, 0, 0));
            }
        }
    }
    Ok(())
}

/// Las ranuras del destino, MAPEADAS para la 3060 (una vez por arranque).
/// LI2f: tambien las de los UAV de un computo.
pub(super) fn destino_mapeado(bar0: u64) -> Result<(), u32> {
    use bmo_gpu_ga10x::destino as ds;
    if !DESTINO_MAPEADO.load(Ordering::Acquire) {
        match ds::mapear(&mut Bar0(bar0)) {
            Some((n, bien)) if n == bien => {
                DESTINO_MAPEADO.store(true, Ordering::Release);
                crate::ring0::cabina::count("gpu", "P3b4b: el destino de la app MAPEADO para la 3060 (PTE de sistema); entradas", n as u64);
            }
            _ => {
                crate::ring0::cabina::warn("gpu", "P3b4b: las tablas del destino no se escribieron o no se releyeron", 0);
                return Err(no(Porque::DestinoTablas, 0, 0));
            }
        }
    }
    Ok(())
}

/// Devolver lo que [`prestar_texturas`] presto, ranura a ranura.
fn devolver_texturas(prestadas: &Prestadas) {
    use bmo_gpu_ga10x::texturas as tx;
    for (k, &paginas) in prestadas.iter().enumerate() {
        if paginas != 0 && crate::ring0::plat::iommu::devolver_gpu(tx::IOVA + k as u64 * tx::RANURA, paginas).is_err() {
            crate::ring0::cabina::warn("gpu", "P3b4c.8: una textura NO se pudo devolver (la invalidacion no contesto); paginas", paginas);
        }
    }
}

/// POR QUE una receta no se pego, en CABINA: la regla, la instruccion y que.
fn decir_no_receta(e: bmo_gpu_ga10x::receta::NoReceta) {
    use bmo_gpu_ga10x::receta::NoReceta;
    match e {
        NoReceta::Cuerpo(cual, b) | NoReceta::Juez(cual, b) => {
            apuntar(Porque::Juez, 1 + (cual == "pixel") as u8, b.instruccion as u64);
            crate::ring0::cabina::warn("gpu", cual, b.instruccion as u64);
            crate::ring0::cabina::warn("gpu", b.regla.nombre(), b.que as u64);
            crate::ring0::cabina::warn("gpu", "[BMO-X Juez V3b]: TOMA TU BODRIO! la receta de una app no sube a la 3060; instruccion", b.instruccion as u64);
        }
        NoReceta::Pegamento(cual, _) => {
            apuntar(Porque::Pegamento, 1 + (cual == "pixel") as u8, 0);
            crate::ring0::cabina::warn("gpu", "P3b4c: el pegamento del kernel no puede con esta receta (cargas, registros o salidas)", (cual == "pixel") as u64)
        }
        NoReceta::Forma => {
            apuntar(Porque::Forma, 0, 0);
            crate::ring0::cabina::warn("gpu", "P3b4c: la receta no se sostiene", 0)
        }
    }
}

/// **Un dibujo EN FRIO (o en caliente) por la tuberia fija**, con el cerrojo
/// del GR tomado aqui y soltado en TODA salida: el de `gpu verrano` y el de
/// la puerta de las apps (`receta`). `paquete` ya esta leido y juzgado.
#[allow(clippy::too_many_arguments)]
fn en_frio(bar0: u64, pid: u32, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, ligero: bool, libreta: bool) -> Result<u64, u32> {
    use bmo_gpu_ga10x::tuberia as tu;
    let (p, v, paquete) = (*p, *v, *paquete);
    // P3b4c (29-09): un canal MUERTO dice NO al instante, con SU motivo.
    // 13-10: con SU Xid en el detalle: el juego lo dice en su consola.
    if let Some(xid) = super::gr_muerto() {
        apuntar(Porque::CanalMuerto, 0, xid as u64);
        return Err(super::IOMMU_NO_CANAL_MUERTO);
    }
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || gr_ocupado() {
        apuntar(Porque::GrOcupado, 0, e as u64);
        return Err(IOMMU_NO_BLUR);
    }
    // ** P3b4b (3): con DESTINO, la 3060 dibuja en la RAM de la app -- un
    // bloque ESCRIBIBLE de quien lo pide (no sellado), entero, prestado a la
    // 3060 SOLO mientras dibuja y devuelto SIEMPRE, salga como salga.
    //
    // *** Y DESDE AQUI EL CERROJO DEL GR ESTA TOMADO (`gr_ocupado`): toda
    // salida lo suelta. Hasta el 28-09 un destino malo salia por un `?` con
    // el cerrojo puesto, y la 3060 contestaba "uno en marcha" a todo hasta
    // reiniciar.
    // ** P3b4c: la superficie de la Z, mapeada la primera vez que se pide.
    if paquete.dibujo.z.is_some() {
        if let Err(m) = mapear_z(bar0) {
            BLUR_EN_MARCHA.store(false, Ordering::Release);
            return Err(m);
        }
    }
    // ** P3b4c.6b: con Z, el color va a la SOMBRA en bloque (la 3060 no
    // dibuja con Z sobre un color pitch: Xid 69 en el metal, 28-09) y el
    // motor de copia la lleva al destino. `sombra` = la ventana pitch donde
    // acaba y si hay que cargar la sombra antes.
    let sombra = bmo_gpu_ga10x::sombra::plan(&v, &paquete.dibujo);
    if sombra.is_some() {
        if let Err(m) = mapear_sombra(bar0) {
            BLUR_EN_MARCHA.store(false, Ordering::Release);
            return Err(m);
        }
    }
    // ** LI2d (12-10): el 0 y los render targets 1.., cada uno en SU ranura
    // (`destino::iova_de`); las paginas de cada uno, para devolverlas TODAS
    // salga como salga (0 = esa ranura no se presto).
    let mut prestado = [0u64; bmo_gpu_ga10x::destino::OBJETIVOS];
    match paquete.dibujo.destino {
        None => {}
        // Z1: a la pantalla, la RAM de la app no se presta ni se toca.
        Some(_) if paquete.dibujo.pantalla => {}
        Some(d0) => {
            let todos = core::iter::once(d0).chain(paquete.dibujo.mas().iter().copied());
            for (k, (va_app, dst)) in todos.enumerate() {
                match prestar_destino(bar0, pid, va_app, &dst, k) {
                    Ok(p) => prestado[k] = p,
                    Err(m) => {
                        devolver_destinos(&prestado);
                        BLUR_EN_MARCHA.store(false, Ordering::Release);
                        return Err(m);
                    }
                }
            }
        }
    }
    let n = (paquete.n / 3) as u32;
    // En caliente, o no: lo fijo es lo mismo, nadie lanzo nada por el GR
    // desde nuestro ultimo dibujo (la entrada sigue donde la dejamos) y fue
    // hace menos de 100 ms (entre dos fotogramas de un banco, no entre dos
    // ordenes tecleadas: un preparar de otro trabajo que fallo a medias no
    // mueve la entrada, pero tampoco cabe en ese hueco). Y se olvida ANTES
    // de dibujar: si este sale mal, el siguiente va en frio.
    let huella = tu::huella_fija(&v, &paquete, ligero);
    let tsc = crate::ring0::task::scheduler::tsc_freq().max(1);
    let ahora = crate::ring0::task::scheduler::rdtsc();
    // ** Con destino TAMBIEN en caliente (29-09). Hasta hoy, "con destino,
    // siempre en frio": y el metal de las 07:02 (BMOX-12) dio `preparar` 5,4
    // ms de los 10,3 del lote, releyendo tres paginas por el PCIe en cada
    // fotograma. El prestamo de la RAM de la app NO toca la VRAM: las ordenes
    // apuntan a `destino::VA`, fija, y lo que cambia es la IOMMU detras, que
    // se presta arriba en cada dibujo. Lo que el caliente da por escrito (los
    // programas juzgados, la tabla, las ordenes) lo dice la huella, que lleva
    // las MEDIDAS del destino y no su direccion (`tuberia::huella_fija`).
    let caliente = CALIENTE_HUELLA.swap(0, Ordering::AcqRel) == huella
        && CALIENTE_ENTRADA.load(Ordering::Acquire) == e
        && ahora.wrapping_sub(CALIENTE_TSC.load(Ordering::Acquire)) < tsc / 10;
    let mut preparar_ciclos = 0u64;
    let preparar = |r: &mut Bar0| {
        let desde = crate::ring0::task::scheduler::rdtsc();
        let bien = if caliente { tu::preparar_caliente(r, e, &v, &paquete, ligero).then_some(()).ok_or(0) } else { tu::preparar_paso(r, e, &v, &paquete, ligero) };
        preparar_ciclos = crate::ring0::task::scheduler::rdtsc() - desde;
        bien
    };
    // Los us de las copias de la sombra (P3b4c.6c: van en el `Ok`, para
    // que la app sepa cuanto de su fotograma es copiar).
    let mut copia_us = 0u64;
    let r = super::volcado::quieto()
        .and_then(|()| match sombra {
            // Lo que la app ya pinto en su back buffer, DEBAJO del dibujo.
            Some((w, true)) => super::volcado::copia_de_sombra(&mut Bar0(bar0), &w, true).map(|us| copia_us += us),
            // Z4a: la copia del fotograma anterior aun leyendo la sombra:
            // que acabe antes de dibujar encima.
            _ => super::volcado::esperar_copia_en_vuelo(&mut Bar0(bar0)).map(|us| copia_us += us),
        })
        .and_then(|()| dibujar(bar0, paquete.ficha, e, &p, n, !ligero, preparar))
        .map(|x| match sombra {
            // Pagado el dibujo, la sombra al destino; si la copia no se paga,
            // el destino NO tiene el dibujo: se dice como un dibujo no pagado.
            // ** Z4a: a la PANTALLA, la copia se lanza y no se espera (nadie lee
            // la pantalla; la espera el siguiente que toque la sombra). A la
            // RAM de la app, se espera: la app la lee al volver.
            Some((w, _)) if cu::sano(x) && paquete.dibujo.pantalla => match super::volcado::lanzar_copia_de_sombra(&mut Bar0(bar0), &w) {
                Ok(()) => cu::con_copia(x, copia_us),
                Err(_) => cu::sin_dibujo(x),
            },
            Some((w, _)) if cu::sano(x) => match super::volcado::copia_de_sombra(&mut Bar0(bar0), &w, false) {
                Ok(us) => cu::con_copia(x, copia_us + us),
                Err(_) => cu::sin_dibujo(x),
            },
            _ => x,
        })
        // 9d: la libreta, leida con el cerrojo aun tomado.
        .map(|x| super::libreta::tras_el_dibujo(bar0, libreta, x));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    devolver_destinos(&prestado);
    let r = r.map(|x| cu::con_preparar(x, caliente, preparar_ciclos * 1_000_000 / tsc));
    if let Ok(x) = r {
        if cu::sano(x) {
            CALIENTE_ENTRADA.store(BLUR_ENTRADA.load(Ordering::Acquire), Ordering::Release);
            CALIENTE_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
            CALIENTE_PROGRAMAS.store(huella_programas(&paquete), Ordering::Release);
            CALIENTE_HUELLA.store(huella, Ordering::Release);
        }
    }
    r
}

/// Las tablas de la superficie de la Z, escritas (una vez por arranque).
static Z_MAPEADA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// **P3b4c: la superficie de la Z** en `profundidad::VA`, una vez: su VRAM
/// no se mueve ni se presta (es de la 3060, no de una app).
fn mapear_z(bar0: u64) -> Result<(), u32> {
    if Z_MAPEADA.load(Ordering::Acquire) {
        return Ok(());
    }
    match bmo_gpu_ga10x::profundidad::mapear(&mut Bar0(bar0)) {
        Some((n, bien)) if n == bien => {
            Z_MAPEADA.store(true, Ordering::Release);
            crate::ring0::cabina::count("gpu", "P3b4c: la Z de la 3060 MAPEADA (ZF32 bloque-lineal, kind generico); entradas", n as u64);
            Ok(())
        }
        _ => {
            crate::ring0::cabina::warn("gpu", "P3b4c: las tablas de la Z no se escribieron o no se releyeron", 0);
            Err(no(Porque::ZTablas, 0, 0))
        }
    }
}

/// Las tablas de la sombra del color, escritas (una vez por arranque).
static SOMBRA_MAPEADA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// **P3b4c.6b: la sombra del color** en `sombra::VA`, una vez: como la Z,
/// VRAM de la 3060 que no se mueve ni se presta.
fn mapear_sombra(bar0: u64) -> Result<(), u32> {
    if SOMBRA_MAPEADA.load(Ordering::Acquire) {
        return Ok(());
    }
    match bmo_gpu_ga10x::sombra::mapear(&mut Bar0(bar0)) {
        Some((n, bien)) if n == bien => {
            SOMBRA_MAPEADA.store(true, Ordering::Release);
            crate::ring0::cabina::count("gpu", "P3b4c.6b: la SOMBRA del color MAPEADA (A8R8G8B8 bloque-lineal junto a la Z); entradas", n as u64);
            Ok(())
        }
        _ => {
            crate::ring0::cabina::warn("gpu", "P3b4c.6b: las tablas de la sombra no se escribieron o no se releyeron", 0);
            Err(no(Porque::SombraTablas, 0, 0))
        }
    }
}

/// Las tablas del destino de la app, escritas (una vez por arranque).
static DESTINO_MAPEADO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// **Prestar la RAM de la app a la 3060** (P3b4b 3): un bloque escribible
/// del proceso, entero y alineado, en `destino::IOVA`, con las tablas del
/// destino puestas. `Ok(paginas prestadas)`.
/// **Devolver los destinos prestados** (LI2d: cada ranura con lo suyo).
fn devolver_destinos(prestado: &[u64; bmo_gpu_ga10x::destino::OBJETIVOS]) {
    for (k, &paginas) in prestado.iter().enumerate() {
        if paginas != 0 && crate::ring0::plat::iommu::devolver_gpu(bmo_gpu_ga10x::destino::iova_de(k), paginas).is_err() {
            crate::ring0::cabina::warn("gpu", "VERRANO: la RAM de la app NO se pudo devolver (la invalidacion no contesto); paginas", paginas);
        }
    }
}

/// `k`: el render target (LI2d), y con el su ranura (`destino::iova_de`).
fn prestar_destino(bar0: u64, pid: u32, va_app: u64, dst: &bmo_gpu_ga10x::destino::Destino, k: usize) -> Result<u64, u32> {
    use bmo_gpu_ga10x::destino as ds;
    use bmo_gpu_ga10x::porque::DESTINO_RESERVA;
    use crate::ring0::obj::reserva;
    let bytes = dst.bytes();
    // ** Q0a1b (12-10): el destino puede empezar a mitad de pagina (a 128 B,
    // `destino::ALINEA`; lo valido `receta::leer`). Se presta desde la pagina
    // de su BASE, y las ordenes dibujan en `destino::VA` + lo que se mete.
    let base = va_app & !0xFFF;
    let paginas = dst.paginas_en(va_app);
    // Donde esta: un bloque suyo (seguido), o su RESERVA (marcos sueltos).
    let fisica = crate::ring0::obj::memory::fisica_para_escribir(pid, va_app, bytes).map(|f| f - va_app % 4096);
    let aspace = crate::ring0::mm::vmm::read_cr3();
    let marco = |k: u64| reserva::marco(aspace, base + k * crate::ring0::mm::PAGE, true);
    match fisica {
        Some(f) if f % 4096 != 0 || f + paginas * 4096 > crate::ring0::mm::PHYSMAP_SIZE => {
            crate::ring0::cabina::warn("gpu", "VERRANO: el destino no empieza en pagina o se sale del physmap; fisica", f);
            return Err(no(Porque::DestinoNoAlineado, 0, f));
        }
        Some(_) => {}
        None if reserva::en_la_ventana(va_app, bytes) => {
            if va_app % bmo_gpu_ga10x::destino::ALINEA != 0 {
                return Err(no(Porque::DestinoNoAlineado, 1, va_app));
            }
            // ** 12-10: el render target de un juego vive en su reserva (lo
            // de 64 KiB o mas: `memoria::pedir_paginas` de la casa). Cada
            // pagina tiene que estar HECHA, ser suya y escribible.
            if let Some(k) = (0..paginas).find(|&k| marco(k).is_none()) {
                crate::ring0::cabina::warn("gpu", "VERRANO: una pagina del destino en la reserva no esta hecha o no es escribible", base + k * 4096);
                return Err(no(Porque::DestinoNoEscribible, DESTINO_RESERVA, base + k * 4096));
            }
        }
        None => {
            crate::ring0::cabina::warn("gpu", "VERRANO: el destino no es un bloque ESCRIBIBLE de quien lo pide", va_app);
            let sub = crate::ring0::obj::memory::por_que_no_escribir(pid, va_app, bytes);
            return Err(no(Porque::DestinoNoEscribible, sub, va_app));
        }
    }
    destino_mapeado(bar0)?;
    let prestado = match fisica {
        Some(f) => crate::ring0::plat::iommu::prestar_gpu(ds::iova_de(k), f, paginas, true),
        None => crate::ring0::plat::iommu::prestar_gpu_marcos(ds::iova_de(k), paginas, true, marco),
    };
    if let Err(m) = prestado {
        crate::ring0::cabina::warn("gpu", "VERRANO: la RAM de la app no se pudo prestar a la 3060; motivo de la IOMMU", m as u64);
        return Err(no(Porque::DestinoNoSePresta, fisica.is_none() as u8, va_app));
    }
    Ok(paginas)
}

/// Si este paquete PODRIA ir en caliente (sus programas son los del ultimo
/// dibujo pagado): solo mira la huella, sin gastarla -- la decision de verdad
/// la toma `verrano` mas abajo, con la entrada y el reloj.
fn caliente_posible(p: &bmo_gpu_ga10x::tuberia::Paquete) -> bool {
    let h = CALIENTE_HUELLA.load(Ordering::Acquire);
    h != 0 && CALIENTE_PROGRAMAS.load(Ordering::Acquire) == huella_programas(p)
}

/// La huella de los DOS programas solos (FNV-1a), para saber sin la
/// ventana si ya se juzgaron.
fn huella_programas(p: &bmo_gpu_ga10x::tuberia::Paquete) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in p.vs.iter().chain([0xA5u8].iter()).chain(p.ps.iter()) {
        h = (h ^ x as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// == V1b: EL ANILLO ===========================================================

/// Si el anillo tiene fotogramas enviados que aun no se vieron pagados.
fn en_vuelo() -> bool {
    !bmo_gpu_ga10x::anillo::ya(ANILLO_PAGADO.load(Ordering::Acquire), ANILLO_NUMERO.load(Ordering::Acquire))
}

/// **Vaciar el anillo** (con el cerrojo del GR ya tomado): esperar a que la
/// 3060 pague el ultimo fotograma enviado. `Ok(us esperados)`. Si no llega
/// en el plazo se da por perdido -- lo dice la cabina -- y el anillo se
/// olvida: nadie se queda esperando a un fotograma que no va a llegar.
fn vaciar(r: &mut Bar0) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    let numero = ANILLO_NUMERO.load(Ordering::Acquire);
    if an::ya(ANILLO_PAGADO.load(Ordering::Acquire), numero) {
        return Ok(0);
    }
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    loop {
        let pagado = an::pagado(r);
        let us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if an::ya(pagado, numero) {
            ANILLO_PAGADO.store(pagado, Ordering::Release);
            return Ok(us);
        }
        if us >= FRACTAL_ESPERA_US {
            crate::ring0::cabina::warn("gpu", "V1b: el anillo no se vacio: la 3060 no pago el fotograma", numero as u64);
            super::vigilante::vigilar("E7: un fotograma del ANILLO no volvio en su plazo; us", true, false, us, FRACTAL_ESPERA_US);
            ANILLO_HUELLA.store(0, Ordering::Release);
            ANILLO_PAGADO.store(numero, Ordering::Release);
            return Err(IOMMU_NO_BLUR);
        }
        esperando(us);
    }
}

/// **VACIAR** (`CUBO_VACIAR`, y LEER): con su cerrojo.
fn vaciar_con_cerrojo() -> Result<u64, u32> {
    if !en_vuelo() {
        return Ok(0);
    }
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || BLUR_EN_MARCHA.swap(true, Ordering::AcqRel) {
        return Err(IOMMU_NO_BLUR);
    }
    let r = vaciar(&mut Bar0(bar0));
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

/// **Soltar el anillo** para otro trabajo del GR (`gr_ocupado`, con el
/// cerrojo ya tomado): lo que quedo en vuelo, pagado, y el anillo olvidado
/// -- el otro va a escribir en el mismo tramo (programas, tabla, ordenes).
/// **E7b (13-10): olvidar lo caliente y el anillo** de un canal de GR que se
/// libero: sus entradas del GPFIFO ya no existen (el canal nuevo empieza en
/// la primera), y nada de lo "ya escrito" vale. Lo mapeado (la pagina del
/// anillo, la Z, las texturas, el destino) sigue: es del espacio de
/// direcciones, que no se libero.
pub(super) fn olvidar() {
    CALIENTE_HUELLA.store(0, Ordering::Release);
    CALIENTE_ENTRADA.store(u32::MAX, Ordering::Release);
    CALIENTE_PROGRAMAS.store(0, Ordering::Release);
    ANILLO_HUELLA.store(0, Ordering::Release);
    ANILLO_ENTRADA.store(u32::MAX, Ordering::Release);
    ANILLO_NUMERO.store(0, Ordering::Release);
    ANILLO_PAGADO.store(0, Ordering::Release);
}

pub(super) fn soltar_anillo() -> Result<(), u32> {
    if ANILLO_HUELLA.load(Ordering::Acquire) == 0 && !en_vuelo() {
        return Ok(());
    }
    ANILLO_HUELLA.store(0, Ordering::Release);
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 {
        return Err(IOMMU_NO_BLUR);
    }
    vaciar(&mut Bar0(bar0)).map(|_| ())
}

/// **La pagina del anillo**: pedida, PRESTADA a la 3060 SOLO LECTURA (lee
/// los vertices; no escribe nada en la RAM del PC) y mapeada, una vez por
/// arranque.
fn asegurar_pagina(r: &mut Bar0) -> Result<(), u32> {
    use crate::ring0::plat::iommu as io;
    use bmo_gpu_ga10x::anillo as an;
    if ANILLO_PRESTADO.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(f) = super::grupo(&ANILLO_F, an::PAGINAS) else {
        crate::ring0::cabina::warn("gpu", "V1b: no hubo una pagina de RAM para el anillo", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    };
    super::memoria(f, an::PAGINAS * super::PAGINA).fill(0);
    if io::prestar_gpu(an::IOVA, f, an::PAGINAS, false).is_err() || !matches!(io::ve_la_gpu(an::IOVA), Some((g, false)) if g == f) {
        crate::ring0::cabina::warn("gpu", "V1b: el anillo no se ve por la IOMMU donde se presto; iova", an::IOVA);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    match an::mapear(r) {
        Some((n, bien)) if n == bien => {}
        _ => {
            crate::ring0::cabina::warn("gpu", "V1b: la PTE del anillo no estaba vacia o no se releyo", 0);
            return Err(IOMMU_NO_BLUR_PREPARAR);
        }
    }
    ANILLO_PRESTADO.store(true, Ordering::Release);
    crate::ring0::cabina::count("gpu", "V1b: EL ANILLO -- una pagina de RAM PRESTADA a la 3060 (solo lectura) y mapeada; iova", an::IOVA);
    Ok(())
}

/// Los vertices del paquete en la ranura `k` de la pagina del anillo.
fn a_la_ranura(k: u32, vertices: &[u8]) {
    let f = ANILLO_F.load(Ordering::Acquire) + bmo_gpu_ga10x::anillo::PASO * k as u64;
    super::memoria(f, bmo_gpu_ga10x::anillo::PASO)[..vertices.len()].copy_from_slice(vertices);
}

/// **VERRANO por el anillo** (con el cerrojo tomado, SIN vaciar): si el
/// anillo esta armado con lo mismo, el fotograma se ENVIA y se vuelve; si
/// no, se ARMA en frio con este fotograma y se espera.
fn en_anillo(bar0: u64, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, huella: u64, coopera: bool) -> Result<u64, u32> {
    // P3b4b: VERRANO en frio acepta 64 KiB de DATOS; una ranura del anillo,
    // 1 KiB. Lo que no cabe se dice aqui, ANTES de copiar nada a la ranura.
    if !bmo_gpu_ga10x::anillo::cabe(paquete) {
        crate::ring0::cabina::warn("gpu", "VERRANO: el paquete no cabe en una ranura del anillo; bytes", paquete.vertices.len() as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    let mut r = Bar0(bar0);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let n = (paquete.n / 3) as u32;
    let mapa_nuevo = asegurar_mapa(&mut r, p)?;
    // Armado con lo mismo, nadie por el GR desde el ultimo envio (la
    // entrada sigue donde la dejo; `gr_ocupado` ademas lo olvida) y hace
    // menos de 100 ms (entre dos fotogramas de un banco). Un mapa de la
    // pantalla recien hecho pide invalidar la MMU: eso lo hace armar.
    let armado = !mapa_nuevo
        && ANILLO_HUELLA.load(Ordering::Acquire) == huella
        && ANILLO_ENTRADA.load(Ordering::Acquire) == e
        && desde.wrapping_sub(ANILLO_TSC.load(Ordering::Acquire)) < crate::ring0::task::scheduler::tsc_freq() / 10;
    if armado {
        // V1c: el recorte que dijo el escritorio; sin el, la ventana entera.
        let recorte = coopera.then(|| paquete.limpiar.unwrap_or(bmo_gpu_ga10x::anillo::TODA));
        enviar_en_anillo(&mut r, e, v, paquete, n, desde, hz, recorte)
    } else {
        armar_anillo(&mut r, e, v, paquete, n, huella, desde, hz, coopera)
    }
}

/// **Enviar** el fotograma siguiente: su ranura libre (si la 3060 va
/// CUATRO por detras, se la espera), sus vertices a la RAM, la cola, la
/// entrada, GP_PUT y el timbre. Sin releer, sin invalidar, sin esperar.
#[allow(clippy::too_many_arguments)]
// ** `inline(never)` (2026-09-28): enviar y armar son HERMANOS -- un
// fotograma hace uno u otro --, y cada uno lleva sus ordenes (4 KiB) en la
// pila. Metidos los dos dentro de `en_anillo`, el compilador los apilaba:
// 28.183 B de syscall, mas el tick encima, contra 32 KiB (`pila.py`).
#[inline(never)]
fn enviar_en_anillo(r: &mut Bar0, e: u32, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, n: u32, desde: u64, hz: u64, recorte: Option<(u32, u32)>) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    let numero = ANILLO_NUMERO.load(Ordering::Acquire).wrapping_add(1);
    // La ranura la usaba el fotograma de hace RANURAS: tiene que estar pagado.
    let viejo = numero.wrapping_sub(an::RANURAS);
    let mut espera = 0u64;
    if !an::ya(ANILLO_PAGADO.load(Ordering::Acquire), viejo) {
        let t0 = crate::ring0::task::scheduler::rdtsc();
        loop {
            let pagado = an::pagado(r);
            espera = (crate::ring0::task::scheduler::rdtsc() - t0) / hz;
            if an::ya(pagado, viejo) {
                ANILLO_PAGADO.store(pagado, Ordering::Release);
                break;
            }
            if espera >= FRACTAL_ESPERA_US {
                crate::ring0::cabina::warn("gpu", "V1b: la 3060 no pago el fotograma que ocupaba la ranura; numero", viejo as u64);
                super::vigilante::vigilar("E7: el fotograma de una ranura del ANILLO no volvio en su plazo; us", true, false, espera, FRACTAL_ESPERA_US);
                ANILLO_HUELLA.store(0, Ordering::Release);
                ANILLO_PAGADO.store(ANILLO_NUMERO.load(Ordering::Acquire), Ordering::Release);
                DIAG_3D[0].store(0, Ordering::Release);
                diagnostico(r);
                return Ok(cu::empaquetar(espera as u32, n, 0, true));
            }
            esperando(espera);
        }
    }
    // Lo que tardo la 3060 en el fotograma que deja la ranura (pagado, y
    // su ranura aun sin tocar): sus dos marcas de reloj. Una ranura de cada
    // cuatro -- son 4 lecturas por la ventana (~5 us) y la muestra basta.
    let k = an::ranura(numero);
    let tarjeta = if k == 0 { an::tardo(r, k).unwrap_or(0) } else { 0 };
    a_la_ranura(k, paquete.vertices);
    core::sync::atomic::fence(Ordering::SeqCst);
    if !an::enviar(r, e, v, n as usize, numero, paquete.ficha, recorte) {
        ANILLO_HUELLA.store(0, Ordering::Release);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    let siguiente = bmo_gpu_ga10x::blur::siguiente(e);
    BLUR_ENTRADA.store(siguiente, Ordering::Release);
    ANILLO_ENTRADA.store(siguiente, Ordering::Release);
    ANILLO_NUMERO.store(numero, Ordering::Release);
    let fin = crate::ring0::task::scheduler::rdtsc();
    ANILLO_TSC.store(fin, Ordering::Release);
    let preparar = ((fin - desde) / hz).saturating_sub(espera);
    Ok(cu::con_preparar(cu::en_vuelo(espera as u32, tarjeta, n), true, preparar))
}

/// **Armar** el anillo con este fotograma (el numero 1), en frio: lo que
/// quedara de otro anillo, pagado; la pagina; los programas, la tabla y las
/// cuatro ranuras, releidos; el timbre CON invalidacion de la MMU, y se
/// espera a que se pague. Solo entonces queda armado.
#[allow(clippy::too_many_arguments)]
// ** `inline(never)` (2026-09-28): enviar y armar son HERMANOS -- un
// fotograma hace uno u otro --, y cada uno lleva sus ordenes (4 KiB) en la
// pila. Metidos los dos dentro de `en_anillo`, el compilador los apilaba:
// 28.183 B de syscall, mas el tick encima, contra 32 KiB (`pila.py`).
#[inline(never)]
fn armar_anillo(r: &mut Bar0, e: u32, v: &cu::Ventana, paquete: &bmo_gpu_ga10x::tuberia::Paquete, n: u32, huella: u64, desde: u64, hz: u64, coopera: bool) -> Result<u64, u32> {
    use bmo_gpu_ga10x::anillo as an;
    vaciar(r)?;
    ANILLO_HUELLA.store(0, Ordering::Release);
    // El anillo pisa EMPUJE y TABLA: el caliente de V1 (`ligero`) ya no
    // puede reusar lo suyo. Su entrada lo diria casi siempre, pero tras 512
    // envios la entrada da la vuelta y podria casar.
    CALIENTE_HUELLA.store(0, Ordering::Release);
    super::volcado::quieto()?;
    asegurar_pagina(r)?;
    super::memoria(ANILLO_F.load(Ordering::Acquire), an::PAGINAS * super::PAGINA).fill(0);
    a_la_ranura(an::ranura(1), paquete.vertices);
    if !an::armar(r, e, v, paquete, 1, coopera) {
        crate::ring0::cabina::warn("gpu", "V1b: el anillo no quedo armado; no se toca el timbre", n as u64);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    let preparar = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
    core::sync::atomic::fence(Ordering::SeqCst);
    let t0 = crate::ring0::task::scheduler::rdtsc();
    let lanzado = cu::lanzar(r, paquete.ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let (mut us, mut pagado) = (0, false);
    while lanzado && us < FRACTAL_ESPERA_US {
        pagado = an::ya(an::pagado(r), 1);
        us = (crate::ring0::task::scheduler::rdtsc() - t0) / hz;
        if pagado {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    super::vigilante::vigilar("E7: el primer fotograma del ANILLO no volvio en su plazo; us", lanzado, pagado, us, FRACTAL_ESPERA_US);
    let etapas = if pagado { 0b100 } else { 0 };
    DIAG_3D[0].store(etapas, Ordering::Release);
    if pagado {
        ANILLO_NUMERO.store(1, Ordering::Release);
        ANILLO_PAGADO.store(1, Ordering::Release);
        ANILLO_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
        ANILLO_TSC.store(crate::ring0::task::scheduler::rdtsc(), Ordering::Release);
        ANILLO_HUELLA.store(huella, Ordering::Release);
        DIBUJADO.store(true, Ordering::Release);
    } else {
        // Nada que esperar despues: el fotograma se da por perdido.
        ANILLO_NUMERO.store(0, Ordering::Release);
        ANILLO_PAGADO.store(0, Ordering::Release);
        diagnostico(r);
        crate::ring0::cabina::warn("gpu", "V1b: el primer fotograma del anillo no se pago; us", us);
    }
    Ok(cu::con_preparar(cu::empaquetar(us as u32, n, etapas, lanzado), false, preparar))
}

/// Los escalones y los registros del GR, para `DIAG_3D` (`gpu cubo 3060`
/// y el banco los muestran cuando algo no se pago).
fn diagnostico(r: &mut Bar0) {
    let pagados = bmo_gpu_ga10x::raster::escalones(r);
    DIAG_3D[4].store(pagados as u32, Ordering::Release);
    DIAG_3D[5].store((pagados >> 32) as u32, Ordering::Release);
    for (k, reg) in super::GR_MIRADOS.iter().enumerate() {
        DIAG_3D[1 + k].store(bmo_gpu_ga10x::Registros::leer(r, *reg), Ordering::Release);
    }
}

/// El dibujo, de X5 o de VERRANO: preparar, el timbre, esperar el semaforo,
/// y la escalera. `n` = los triangulos. `escalera` = leer los escalones y
/// los registros del GR al acabar (sin ella, VERRANO `ligero`, no hay
/// escalones que leer: son ~40 lecturas por PCIe menos por fotograma).
fn dibujar(bar0: u64, ficha: u32, e: u32, p: &bmo_gpu_ga10x::pantalla::Pantalla, n: u32, escalera: bool, preparar: impl FnOnce(&mut Bar0) -> Result<(), u8>) -> Result<u64, u32> {
    let mut r = Bar0(bar0);
    asegurar_mapa(&mut r, p).inspect_err(|_| apuntar(Porque::PantallaMapa, 0, 0))?;
    if let Err(paso) = preparar(&mut r) {
        crate::ring0::cabina::warn("gpu", "X5: el tramo del cubo no quedo preparado; no se toca el timbre; paso", paso as u64);
        return Err(no(Porque::Preparar, paso, n as u64));
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = cu::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let mut us = 0;
    while lanzado && us < FRACTAL_ESPERA_US {
        let fin = cu::mirar(&mut r).1;
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if fin == cu::PAGA_FIN {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    // E7: si no volvio, el vigilante lo corta antes de leer la escalera.
    let pagado = cu::mirar(&mut r).1 == cu::PAGA_FIN;
    super::vigilante::vigilar("E7: el dibujo del CUBO (X5 o VERRANO) no volvio en su plazo; us", lanzado, pagado, us, FRACTAL_ESPERA_US);
    // La escalera y el motor grafico, como T1c: los lee `DIAG_3D` (`gpu cubo
    // 3060` los muestra si no se pago).
    let etapas = bmo_gpu_ga10x::raster::etapas(&mut r, cu::SEMAFORO_FIN, cu::PAGA_FIN);
    DIAG_3D[0].store(etapas, Ordering::Release);
    if escalera || etapas & 0b100 == 0 {
        diagnostico(&mut r);
    }
    let v = cu::empaquetar(us as u32, n, etapas, lanzado);
    if cu::sano(v) {
        DIBUJADO.store(true, Ordering::Release);
        let n = VECES.fetch_add(1, Ordering::AcqRel) + 1;
        if n == 1 {
            crate::ring0::cabina::count("gpu", "X5: LA 3060 DIBUJO EL CUBO del estudio D3D, sin Windows; us", us);
        }
    } else {
        crate::ring0::cabina::warn("gpu", "X5: el cubo no se pago entero; escalera", etapas as u64);
    }
    Ok(v)
}

/// **LEER**: los pixeles `2k` y `2k + 1` de la ventana (fila a fila), como
/// `0x00RRGGBB`.
fn leer(k: u64) -> Result<u64, u32> {
    if !DIBUJADO.load(Ordering::Acquire) {
        return Err(IOMMU_NO_BLUR);
    }
    // Lo que el anillo dejo en vuelo, pagado ANTES de leer (y lo del volcado:
    // lo que se lee es lo ultimo que dibujo la 3060, no lo que va a dibujar).
    if en_vuelo() {
        vaciar_con_cerrojo()?;
        super::volcado::quieto()?;
    }
    let Some(p) = la_pantalla() else { return Err(IOMMU_NO_PANTALLA) };
    let Some(v) = cu::ventana(&p) else { return Err(IOMMU_NO_PANTALLA) };
    let i = 2 * k as u32;
    if i + 1 >= cu::ANCHO * cu::ALTO {
        return Err(IOMMU_NO_BLUR);
    }
    let (x, y) = (i % cu::ANCHO, i / cu::ANCHO);
    // SAFETY: escrito una vez al arrancar (`info::init_from`), solo se lee.
    let fb = crate::ring0::mm::phys_to_virt(unsafe { crate::info::FB_ADDR }) as *const u32;
    // SAFETY: (x0 + x, y0 + y) y el de al lado dentro de la pantalla (la
    // ventana cabe: `ventana` lo exige, y el ancho de la ventana es par), por
    // el physmap como `pantalla` (el framebuffer entero dentro, `la_pantalla`);
    // 32 bits alineados. `clflush` antes: el espejo es WB y la 3060 escribe la
    // VRAM sin pasar por esta cache.
    let (a, b) = unsafe {
        let q = fb.add((v.y0 + y) as usize * p.pitch as usize + (v.x0 + x) as usize);
        core::arch::x86_64::_mm_clflush(q as *const u8);
        (q.read_volatile(), q.add(1).read_volatile())
    };
    Ok(cu::a_rrggbb(a, v.rgb) as u64 | (cu::a_rrggbb(b, v.rgb) as u64) << 32)
}
