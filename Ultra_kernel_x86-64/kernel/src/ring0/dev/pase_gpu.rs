//! **EL PASE DE LA GPU** (P1, 2026-09-25) -- la burocracia se paga UNA vez:
//! el lienzo prestado para quedarse y un buzon mapeado en el escritorio. El
//! latido del VBLANK que lo lee y el radar que lo vigila son P2.
//!
//! [carril]  ROJO      mapea una pagina en un proceso y se la quita sin pedirle
//!                     permiso; presta su lienzo a una GPU (por el MOTOR)
//! [consumo] NADA      corre al abrir y al cerrar; nada por fotograma (el latido
//!                     que llegara en P2 sera LATE, y solo con un pase abierto)
//! [cuesta]  PUERTA    es la unica entrada del atajo: lo que no se pregunte al
//!                     abrir no lo pregunta nadie despues
//! [riesgo]  UNICO     un pase que no se cierra en una estacion del desmontaje
//!                     deja a la GPU viendo marcos que van a ser de otro
//!
//! # *** NEUTRO: este fichero no sabe que GPU hay debajo
//!
//! Lo pidio el propietario (25-09): *"AISLAR BIEN POR COMPLETO el NVIDIA eso
//! por si voy a tener mi GPU alternativos"*. Asi que aqui vive lo que es de
//! TODA GPU -- quien pide, la pagina del buzon, la lapida, las estaciones del
//! desmontaje -- y lo que es de una tarjeta lo contesta su [`Motor`]:
//!
//! ```text
//!    este fichero (neutro)              el motor (uno por tarjeta)
//!    quien pide y su lienzo             que sabe del aparato (apagado, IOMMU,
//!    el buzon: marco, mapa, lapida        pantalla, copiador, VBLANK, ocupado)
//!    las estaciones: soltar, morir      la medida de lo que pinta
//!    el juicio (`bmo_pase_gpu::pase`)   prestar el lienzo y devolverlo
//! ```
//!
//! Hoy hay UN motor, el de la 3060 (`gpu_trabajo/pase_nv.rs`), y lo elige la
//! puerta (`syscall/op_maquina.rs`). Una tarjeta nueva trae el suyo y no toca
//! este fichero. El guardian `la-3060` (regla N) lo vigila: aqui no se nombra
//! ni un modulo de NVIDIA ni un registro.
//!
//! # *** Por que una LAPIDA y no un hueco
//!
//! Lo mismo que el GATE RED (`red/puerta.rs`): el que tiene el pase es el
//! ESCRITORIO, y un fallo de pagina al leer su buzon se llevaria la pantalla.
//! Al cerrar, la VA pasa a un marco a ceros con el motivo en `ESTADO`: el
//! escritorio lee "cerrado, y por que"; lo que escriba cae donde nadie lee.
//!
//! # [!] Lo que esto supone, dicho
//!
//! - **UN nucleo corre Ring 3**, como en `red/puerta.rs`: el `invlpg` de
//!   `map_page` basta. Con dos, cerrar pide un shootdown.
//! - **El turno NO apaga interrupciones** (ver [`Turno`]): abrir y cerrar
//!   hablan con la IOMMU. P2, que lee desde el aviso del VBLANK, mirara
//!   `ABIERTO` y nunca un pase a medio abrir.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bmo_pase_gpu::{buzon, orden, pase, radar};

use crate::ring0::mm;

/// El pase no se puede. El POR QUE exacto (un `pase::NoPase`) lo dice la
/// suborden ESTADO: la puerta solo lleva un numero.
pub const IOMMU_NO_PASE: u32 = 86;

// == EL MOTOR: lo unico que cambia de una GPU a otra ===========================

/// **Lo que el motor sabe del aparato**, en el momento de preguntar.
#[derive(Clone, Copy, Default)]
pub struct Aparato {
    /// Se apago en orden en este arranque: no acepta trabajo.
    pub apagado: bool,
    /// Su entrada de la IOMMU es TRADUCIDA (y releida).
    pub iommu_traducida: bool,
    /// Alcanza la pantalla que barre el monitor.
    pub pantalla_alcanzable: bool,
    /// Tiene un motor de copia listo.
    pub copiador: bool,
    /// Su aviso de VBLANK esta armado: el latido del radar.
    pub vblank_armado: bool,
    /// Ya presta un lienzo por otro camino (en la 3060: el volcador ARMADO).
    pub ocupado: bool,
}

/// **Una GPU que sabe hacer el pase.** Todo lo que toca la tarjeta pasa por
/// aqui; el resto del fichero no la ve.
pub trait Motor: Sync {
    /// Para CABINA: quien presto.
    fn nombre(&self) -> &'static str;
    fn aparato(&self) -> Aparato;
    /// La pantalla que pinta, y los bytes que tiene que medir el lienzo.
    fn medidas(&self) -> Option<(buzon::Medidas, u64)>;
    /// Presta `bytes` desde `fisica` (el lienzo) a la GPU, SOLO LECTURA, para
    /// quedarse. `false` si no se pudo, y entonces no queda nada prestado.
    fn prestar(&self, fisica: u64, bytes: u64) -> bool;
    /// Devuelve lo prestado. Tiene que servir siempre, tambien al morir.
    fn devolver(&self, bytes: u64);
}

// == EL TURNO ==================================================================

static EN_MARCHA: AtomicBool = AtomicBool::new(false);

/// **Uno a la vez abre o cierra.** Sin apagar interrupciones: el motor presta
/// y devuelve por la IOMMU, que espera su invalidacion, y eso no se hace con
/// las interrupciones fuera. (P2 leera el pase desde el latido con su propio
/// turno, no con este.)
struct Turno;

impl Turno {
    /// El turno, o `None` si otro lo tiene (la puerta contesta que no).
    fn probar() -> Option<Self> {
        (!EN_MARCHA.swap(true, Ordering::AcqRel)).then_some(Turno)
    }

    /// El turno, esperando cediendo el nucleo: las estaciones del desmontaje
    /// NO pueden saltarse la devolucion porque alguien estuviera abriendo.
    fn esperar() -> Self {
        while EN_MARCHA.swap(true, Ordering::AcqRel) {
            crate::ring0::task::scheduler::yield_current();
        }
        Turno
    }
}

impl Drop for Turno {
    fn drop(&mut self) {
        EN_MARCHA.store(false, Ordering::Release);
    }
}

// == LA MEMORIA =================================================================

/// El marco del buzon y el de la lapida. Se piden una vez y no se sueltan: un
/// marco que estuvo mapeado en un proceso no vuelve al asignador mientras ese
/// proceso pueda seguir teniendolo en su tabla.
static mut BUZON_FIS: u64 = 0;
static mut LAPIDA_FIS: u64 = 0;

fn reservar() -> bool {
    use crate::ring0::mm::phys::{alloc_frames_contig_de, Titular};
    // SAFETY: con el turno tomado; nadie mas los toca.
    unsafe {
        // ** `Bufer`: memoria del kernel que un proceso ve.
        for m in [&mut *core::ptr::addr_of_mut!(BUZON_FIS), &mut *core::ptr::addr_of_mut!(LAPIDA_FIS)] {
            if *m == 0 {
                match alloc_frames_contig_de(1, Titular::Bufer) {
                    Some(p) => *m = p,
                    None => return false,
                }
            }
        }
    }
    true
}

/// Una pagina del kernel vista como palabras, SIEMPRE volatil: el proceso
/// puede estar escribiendo mientras se lee.
fn palabra(fisica: u64, k: usize, v: Option<u32>) -> u32 {
    let p = (mm::phys_to_virt(fisica) as *mut u32).wrapping_add(k);
    // SAFETY: `k` < 1024 (lo acota quien llama con `buzon::PALABRAS`); el
    // marco es nuestro y esta en el physmap.
    unsafe {
        match v {
            Some(v) => {
                core::ptr::write_volatile(p, v);
                v
            }
            None => core::ptr::read_volatile(p),
        }
    }
}

/// `buzon::formar` o `buzon::revocar` sobre un marco, con una copia de por medio.
fn sobre(fisica: u64, f: impl FnOnce(&mut [u32; buzon::PALABRAS])) {
    let mut c = [0u32; buzon::PALABRAS];
    for (k, w) in c.iter_mut().enumerate() {
        *w = palabra(fisica, k, None);
    }
    f(&mut c);
    for (k, w) in c.iter().enumerate() {
        palabra(fisica, k, Some(*w));
    }
}

// == EL PASE ====================================================================

#[derive(Clone, Copy)]
struct Pase {
    pid: u32,
    aspace: u64,
    bytes: u64,
    motor: &'static dyn Motor,
}

static mut PASE: Option<Pase> = None;
static ABIERTO: AtomicBool = AtomicBool::new(false);
static ULTIMO_MOTIVO: AtomicU32 = AtomicU32::new(0);
static ULTIMO_NO: AtomicU32 = AtomicU32::new(0);

/// Hay un pase abierto? Lo pregunta tambien el motor, para no prestar lo mismo
/// por dos caminos.
pub fn abierto() -> bool {
    ABIERTO.load(Ordering::Acquire)
}

/// **La puerta**, desde `syscall/op_maquina.rs` (que ya exigio `MAQUINA`).
/// `motor` = el de la GPU que hay, o `None` si ninguna sabe hacer el pase.
pub fn orden(pid: u32, arg: u64, motor: Option<&'static dyn Motor>) -> Result<u64, u32> {
    match orden::suborden(arg) {
        orden::ABRIR => {
            let r = abrir(pid, orden::lienzo_de(arg), motor);
            ULTIMO_NO.store(r.err().map_or(0, pase::NoPase::codigo), Ordering::Release);
            r.map_err(|_| IOMMU_NO_PASE)
        }
        orden::CERRAR => cerrar_por(pid, radar::Motivo::CerradoPorElPropietario).then_some(0).ok_or(IOMMU_NO_PASE),
        orden::ESTADO => Ok(orden::estado(abierto(), ULTIMO_MOTIVO.load(Ordering::Acquire), ULTIMO_NO.load(Ordering::Acquire))),
        _ => Err(IOMMU_NO_PASE),
    }
}

fn abrir(pid: u32, lienzo: u64, motor: Option<&'static dyn Motor>) -> Result<u64, pase::NoPase> {
    let Some(motor) = motor else { return Err(pase::NoPase::SinMotor) };
    let aspace = crate::ring0::mm::vmm::read_cr3();
    let Some(_t) = Turno::probar() else { return Err(pase::NoPase::Ocupado) };
    let a = motor.aparato();
    let medidas = motor.medidas();
    let bytes = medidas.map_or(0, |(_, b)| b);
    let suyo = crate::ring0::obj::memory::fisica_de(pid, lienzo, 1).is_some();
    let fisica = crate::ring0::obj::memory::fisica_de(pid, lienzo, bytes.max(1));
    let hechos = pase::Hechos {
        propietario_pantalla: crate::ring0::obj::fb::owner() == Some(pid),
        aparato_apagado: a.apagado,
        iommu_traducida: a.iommu_traducida,
        pantalla_alcanzable: a.pantalla_alcanzable && medidas.is_some(),
        copiador: a.copiador,
        vblank_armado: a.vblank_armado,
        lienzo_suyo: suyo,
        lienzo_mide: fisica.is_some_and(|f| f % mm::PAGE == 0) && bytes > 0,
        ocupado: a.ocupado || abierto(),
    };
    pase::juzgar(&hechos)?;
    let (Some((m, bytes)), Some(fisica)) = (medidas, fisica) else { return Err(pase::NoPase::SinPantalla) };
    if !reservar() {
        return Err(pase::NoPase::NoSeMapea);
    }
    // ** A CERO ANTES DE MAPEAR: el proceso no ve nunca lo del pase anterior.
    // SAFETY: escritos una vez en `reservar`, con este mismo turno.
    let (buzon_fis, lapida_fis) = unsafe { (BUZON_FIS, LAPIDA_FIS) };
    sobre(buzon_fis, |b| buzon::formar(b, m));
    if !motor.prestar(fisica, bytes) {
        return Err(pase::NoPase::NoSePresta);
    }
    if crate::ring0::mm::vmm::map_page(aspace, orden::BUZON_VA, buzon_fis, true, true).is_err() {
        motor.devolver(bytes);
        let _ = crate::ring0::mm::vmm::map_page(aspace, orden::BUZON_VA, lapida_fis, true, true);
        return Err(pase::NoPase::NoSeMapea);
    }
    // SAFETY: con el turno tomado.
    unsafe { PASE = Some(Pase { pid, aspace, bytes, motor }) };
    ULTIMO_MOTIVO.store(0, Ordering::Release);
    ABIERTO.store(true, Ordering::Release);
    crate::ring0::cabina::info("gpu", "PASE ABIERTO: el lienzo prestado para quedarse, el buzon mapeado; pid", pid as u64);
    crate::ring0::cabina::info("gpu", motor.nombre(), bytes / 1024);
    Ok(orden::BUZON_VA)
}

/// **Cierra el pase de `pid`**, si es suyo. `false` si no tenia.
fn cerrar_por(pid: u32, motivo: radar::Motivo) -> bool {
    let _t = Turno::esperar();
    // SAFETY: con el turno tomado.
    match unsafe { PASE } {
        Some(p) if p.pid == pid => {
            revocar(p, motivo);
            true
        }
        _ => false,
    }
}

/// **REVOCA.** Con el turno tomado. El lienzo vuelve SIEMPRE, antes que nada.
fn revocar(p: Pase, motivo: radar::Motivo) {
    p.motor.devolver(p.bytes);
    let codigo = motivo.codigo();
    // SAFETY: escritos una vez en `reservar`, con este mismo turno.
    let (buzon_fis, lapida_fis) = unsafe { (BUZON_FIS, LAPIDA_FIS) };
    sobre(buzon_fis, |b| buzon::revocar(b, codigo));
    sobre(lapida_fis, |b| {
        b.iter_mut().for_each(|w| *w = 0);
        buzon::revocar(b, codigo);
    });
    // Al morir, su espacio se destruye despues: su tabla no se toca.
    if motivo != radar::Motivo::PropietarioMurio
        && crate::ring0::mm::vmm::map_page(p.aspace, orden::BUZON_VA, lapida_fis, true, true).is_err()
    {
        crate::ring0::mm::vmm::unmap_page(p.aspace, orden::BUZON_VA);
    }
    // SAFETY: con el turno tomado.
    unsafe { PASE = None };
    ABIERTO.store(false, Ordering::Release);
    ULTIMO_MOTIVO.store(codigo, Ordering::Release);
    if motivo.es_normal() {
        crate::ring0::cabina::info("gpu", motivo.texto(), p.pid as u64);
    } else {
        crate::ring0::cabina::fault("gpu", motivo.texto(), p.pid as u64);
    }
}

// == LAS ESTACIONES ============================================================

/// **El propietario de la pantalla la suelta (`murio = false`) o muere.** Va
/// en `obj::fb`, ANTES de `memory`: los marcos no se liberan con la GPU
/// viendolos (R-DMA-3).
pub fn suelta_si_es_de(pid: u32, murio: bool) {
    let motivo = if murio { radar::Motivo::PropietarioMurio } else { radar::Motivo::PantallaSoltada };
    cerrar_por(pid, motivo);
}

/// **La GPU se apaga en orden**: el pase se cierra antes, sea de quien sea.
pub fn cerrar_por_apagado() {
    let _t = Turno::esperar();
    // SAFETY: con el turno tomado.
    if let Some(p) = unsafe { PASE } {
        revocar(p, radar::Motivo::AparatoApagado);
    }
}
