//! **EL PULSO** (02-10): una FOTO EN VIVO del proceso, al final del DIARIO,
//! cada [`CADA_NS`] mientras corre.
//!
//! Por que: en el metal (02-10) Cyberpunk paso el montaje de D3D12 sin un
//! aviso y se quedo GIRANDO (la CPU al 100 %, sin ventana) hasta el `^C`. El
//! diario solo dice lo PRIMERO que se llama, y el anillo de las ultimas
//! llamadas solo salia con ExitProcess: un `^C` mata sin salir, y no quedaba
//! nada que diga en que estaba.
//!
//! ```text
//!    cuando     cada 1024 llamadas del `.exe` (el trampolin del diario) y
//!               en cada vuelta del planificador sin nadie listo; se escribe
//!               si pasaron 2 s desde la ultima
//!    que        el reloj y las llamadas por segundo; cada hilo (su funcion
//!               de arranque, en que esta y a que espera, y su ULTIMA
//!               llamada con quien la hizo); lo que D3D12 lleva hecho (PSO y
//!               lo que tardo compilarlos, recursos, listas, Present); y las
//!               ultimas 32 llamadas
//! ```
//!
//! Como se lee: si la foto sigue al dia con llamadas por segundo, el juego
//! trabaja (o espera algo que no llega: lo dicen los hilos); si se quedo
//! VIEJA (su reloj, muy por detras de cuando se freno), un hilo da vueltas
//! sin llamar a nada -- y su ultima llamada dice desde donde.
//!
//! Solo con el diario encendido; apagado, ni se mira el reloj.

use alloc::string::String;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::plataforma;

/// Cada cuanto se escribe la foto.
const CADA_NS: u64 = 2_000_000_000;

/// **Quien mide el monton de la casa** (07-10): `(en uso, pico, crecido)`
/// en bytes; lo pone el cargador (`medir_monton`). El metal (06-10):
/// Cyberpunk lleno sus 64 MiB a los 17,7 s; con esto cada foto dice si se
/// llena DE GOLPE (cargando) o GOTEA (una fuga que buscar).
static MEDIR_MONTON: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Poner quien mide el monton (una vez, al arrancar).
pub fn medir_monton(f: fn() -> (usize, usize, usize)) {
    MEDIR_MONTON.store(f as usize, Ordering::Relaxed);
}

/// Lo que se cuenta de D3D12.
#[derive(Clone, Copy)]
pub(crate) enum Cosa {
    /// Un PSO grafico o de computo, con lo que tardo en crearse.
    Pso,
    Recurso,
    Lista,
    Present,
}

#[derive(Default)]
struct Cuentas {
    inicio: u64,
    ultima: u64,
    llamadas_ultima: u32,
    fotos: u32,
    psos: u32,
    pso_ns: u64,
    pso_peor_ns: u64,
    recursos: u32,
    listas: u32,
    presents: u32,
}

struct Global(UnsafeCell<Cuentas>);
// SAFETY: una tarea, hilos cooperativos (ver `hilos.rs`).
unsafe impl Sync for Global {}
static CUENTAS: Global = Global(UnsafeCell::new(Cuentas {
    inicio: 0,
    ultima: 0,
    llamadas_ultima: 0,
    fotos: 0,
    psos: 0,
    pso_ns: 0,
    pso_peor_ns: 0,
    recursos: 0,
    listas: 0,
    presents: 0,
}));

fn cuentas() -> &'static mut Cuentas {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *CUENTAS.0.get() }
}

/// La casilla que vigila el pulso (N4.4): la que valia 0 en el salto a 0 de
/// la corrida anterior. `0` = ninguna.
static VIGILADA: AtomicU64 = AtomicU64::new(0);

/// **Vigilar una casilla** de la imagen: cada foto dice cuanto vale y a
/// donde apunta. La pone la app al arrancar, si el informe anterior trae un
/// salto a 0 (`bmo_proton_x::nulo`). Tiene que ser memoria de la imagen,
/// que vive lo que el proceso.
pub fn vigilar(dir: u64) {
    VIGILADA.store(dir, Ordering::Relaxed);
}

/// Al empezar un `.exe`: todo a cero y el reloj en marcha.
pub(crate) fn reiniciar() {
    let ahora = (plataforma().ahora_ns)();
    *cuentas() = Cuentas { inicio: ahora, ultima: ahora, ..Cuentas::default() };
}

/// **Contar** una cosa de D3D12 (`ns`: lo que tardo, si se mide).
pub(crate) fn contar(c: Cosa, ns: u64) {
    let k = cuentas();
    match c {
        Cosa::Pso => {
            k.psos += 1;
            k.pso_ns += ns;
            k.pso_peor_ns = k.pso_peor_ns.max(ns);
        }
        Cosa::Recurso => k.recursos += 1,
        Cosa::Lista => k.listas += 1,
        Cosa::Present => k.presents += 1,
    }
}

/// **El latido**: si toca, la foto al diario. Lo llaman el trampolin del
/// diario (cada 1024 llamadas) y el planificador.
pub(crate) extern "win64" fn latido() {
    if !crate::diario::encendido() {
        return;
    }
    let ahora = (plataforma().ahora_ns)();
    if ahora.saturating_sub(cuentas().ultima) >= CADA_NS {
        foto(ahora);
    }
}

/// La foto, al fichero, con sus ultimas 32 llamadas.
fn foto(ahora: u64) {
    let mut t = texto(ahora);
    t.push_str(&crate::diario::ultimas(32));
    crate::diario::escribir_con(&t);
}

/// **La foto como texto** (sin las ultimas llamadas): la de cada latido, y
/// la de la salida (`diario::al_salir`, que pone el anillo entero detras).
/// Con el GS del `.exe` puesto (los hilos se miran por el TEB).
pub(crate) fn texto(ahora: u64) -> String {
    let k = cuentas();
    let llamadas = crate::diario::llamadas();
    let dt = ahora.saturating_sub(k.ultima).max(1);
    let por_s = (llamadas.wrapping_sub(k.llamadas_ultima) as u64).saturating_mul(1_000_000_000) / dt;
    k.ultima = ahora;
    k.llamadas_ultima = llamadas;
    k.fotos += 1;
    let ms = |ns: u64| ns / 1_000_000;
    let mut t = alloc::format!(
        "# EN VIVO (el pulso, foto {}): {} ms desde la entrada del .exe; {} llamadas ({} por segundo en las ultimas)\n",
        k.fotos,
        ms(ahora.saturating_sub(k.inicio)),
        llamadas,
        por_s
    );
    t.push_str(&alloc::format!(
        "# d3d12: {} PSO ({} ms creandolos, el peor {} ms; {} enlaces distintos, {} se corren; {} compilados y {} del recuerdo), {} recursos, {} ExecuteCommandLists, {} Present\n",
        k.psos,
        ms(k.pso_ns),
        ms(k.pso_peor_ns),
        crate::enlaces::cuantos().0,
        crate::enlaces::cuantos().1,
        crate::enlaces::cuentas().0,
        crate::enlaces::cuentas().1,
        k.recursos,
        k.listas,
        k.presents
    ));
    let m = MEDIR_MONTON.load(Ordering::Relaxed);
    if m != 0 {
        // SAFETY: solo `medir_monton` lo pone, con un `fn` de esa firma.
        let f = unsafe { core::mem::transmute::<usize, fn() -> (usize, usize, usize)>(m) };
        let (uso, pico, crecido) = f();
        t.push_str(&alloc::format!("# el monton de la casa: {} MiB en uso (pico {} MiB), crecio {} MiB por la reserva\n", uso >> 20, pico >> 20, crecido >> 20));
    }
    t.push_str(&crate::nativo::foto());
    let v = VIGILADA.load(Ordering::Relaxed);
    if v != 0 {
        // SAFETY: `vigilar` solo recibe casillas de la imagen, que no se
        // suelta; se lee sin alinear por si acaso.
        let x = unsafe { core::ptr::read_unaligned(v as *const u64) };
        t.push_str(&alloc::format!(
            "# la casilla del salto a 0 ({}): vale {}\n",
            donde(v),
            if x == 0 { String::from("0 (todavia nula)") } else { donde(x) }
        ));
    }
    t.push_str("# hilos (id, su funcion de arranque, en que esta, y su ultima llamada <- quien la hizo):\n");
    for l in crate::hilos::describir() {
        t.push_str("#   ");
        t.push_str(&l);
        t.push('\n');
    }
    t
}

/// `modulo+rva` de una direccion, o la direccion.
pub(crate) fn donde(dir: u64) -> String {
    match crate::kernel32_procesos::imagen_con(dir).and_then(|_| crate::modulos::nombre_de(dir)) {
        Some((m, rva)) => alloc::format!("{m}+{rva:#x}"),
        None => alloc::format!("{dir:#x}"),
    }
}
