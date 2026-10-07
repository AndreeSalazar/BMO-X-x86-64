//! **9d: LA LIBRETA DE LA 3060, en el kernel** (06-10, con permiso del
//! propietario para Ring 0) -- leer, tras el dibujo de una receta, si la
//! 3060 apunto algo raro, y decirselo SOLO a quien la mando.
//!
//! [carril]  VERDE     una lectura de 4 bytes por el BAR0, tras un dibujo
//!                     pagado, y solo si la receta trae libreta
//! [consumo] NADA      ~1 us por lote con libreta (una lectura por el PCIe)
//!
//! AISLADA del todo (pedido del propietario: "AISLAR por completo"):
//!
//! ```text
//!    la palabra     `bmo_gpu_ga10x::libreta::LIBRETA`: del KERNEL, en su
//!                   pagina de semaforos; la escribe solo el pegamento que
//!                   pone el kernel, con una constante (la app no elige ni
//!                   donde ni que)
//!    cada dibujo    la pone a 0 su `preparar` (en frio y en caliente) y se
//!                   lee aqui, con el cerrojo del GR aun tomado: nadie mas
//!                   dibuja entre medias
//!    la respuesta   el bit `cubo::LIBRETA_RARO` del `Ok` de ESA receta: a
//!                   la app que la mando, a nadie mas
//! ```
//!
//! Lo que hace la app con el (`Puerta::apunto` de PROTON-X): el vigia
//! revisa ese PSO con los datos del juego en su siguiente lote.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::cubo as cu;

use crate::ring0::dev::gpu_prestamo::Bar0;

/// Los dibujos con libreta y en cuantos apunto la 3060 (para el log).
static LEIDAS: AtomicU64 = AtomicU64::new(0);
static APUNTADAS: AtomicU64 = AtomicU64::new(0);

/// **Tras el dibujo `x` de una receta**: si trae libreta y se pago, se lee
/// la palabra y, si la 3060 apunto, el `Ok` lleva `LIBRETA_RARO`. Se llama
/// con el cerrojo del GR tomado.
pub(super) fn tras_el_dibujo(bar0: u64, con_libreta: bool, x: u64) -> u64 {
    if !con_libreta || !cu::sano(x) || cu::es_en_vuelo(x) {
        return x;
    }
    LEIDAS.fetch_add(1, Ordering::Relaxed);
    if !bmo_gpu_ga10x::libreta::leer(&mut Bar0(bar0)) {
        return x;
    }
    if APUNTADAS.fetch_add(1, Ordering::Relaxed) == 0 {
        crate::ring0::cabina::count("gpu", "9d: LA LIBRETA de la 3060 apunto algo raro (una salida NaN o infinita); dibujos con libreta", LEIDAS.load(Ordering::Relaxed));
    }
    cu::con_raro(x)
}
