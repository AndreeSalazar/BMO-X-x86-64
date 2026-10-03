//! **ESCUCHAR: la misma musica, a la fuerza de escucharla** (S4k de
//! `PLAN_EL_SONIDO.md`, 2026-10-03).
//!
//! El propietario: *"que se escuchen fuerte todo"*. La musica de FONDO se
//! compone a -26 dBFS de fuerza a proposito -- es para no tener que
//! escucharla --, y con la voz al 70 % queda en -32: por mucho que suba el
//! fader, el aparato no da mas que su tope. Cuando la pieza se ELIGE (la ONDA
//! de HERMES), no es fondo: se compone con la mezcla de CANCION y se NORMALIZA
//! su sonoridad, como hace un servicio de musica: todas a la misma fuerza
//! ([`OBJETIVO`], -14 dBFS), con un limite que sujeta las puntas.
//!
//! ```text
//!    medido (CANCION)   ambiente ~-17 dBFS, picos a 0; neko ~-23, picos a -7
//!    despues            todas a -14 dBFS: +12 dB sobre la de fondo
//! ```
//!
//! *** **El bucle sigue sin costura.** El limite se CALIENTA con el ultimo
//! medio segundo antes de empezar: la primera muestra sale con la reduccion
//! que traia la ultima, que es la que suena justo antes en el bucle. La prueba
//! `escuchar_normaliza_sin_costura` lo mide.

use bmo_amplificador::{Ganancia, Limite, Medidor, MilesimasDb, DB};

use crate::HZ;

/// La fuerza (RMS) a la que se escucha: -14 dBFS, la de los servicios de
/// musica. Mas arriba solo se gana aplastando.
pub const OBJETIVO: MilesimasDb = -14 * DB;
/// Lo mas que se sube: una pieza muy floja no se convierte en ruido.
pub const SUBIDA_MAX: MilesimasDb = 16 * DB;
/// El relajo del limite de ESCUCHAR: 50 ms, el de un limitador de masterizar.
/// El del maestro (250 ms) es de SEGURIDAD y sujetaria la pieza entera tras
/// cada golpe de bateria: con +13 dB se oiria bombear.
const RELAJO_MS: u32 = 50;
/// Cuantas pasadas de prueba (solo leen) para dar con la ganancia.
const PASADAS: usize = 4;

/// Una pasada por el limite, con la cola de calentamiento; si `escribir`,
/// deja el resultado en `m`. Devuelve la fuerza (RMS) de lo que sale.
fn pasar(m: &mut [i16], g: MilesimasDb, escribir: bool) -> MilesimasDb {
    let gan = Ganancia::db(g);
    let mut lim = Limite::inmediato(HZ).con_relajo_ms(HZ, RELAJO_MS);
    // El calentamiento: la cola, que en el bucle suena justo antes del
    // principio. Se lee ANTES de que el bucle de abajo la toque.
    let cola = (HZ as usize / 2).min(m.len());
    for &x in &m[m.len() - cola..] {
        lim.muestra(gan.aplicar(x as i32));
    }
    let mut med = Medidor::nuevo();
    for x in m.iter_mut() {
        let y = lim.muestra(gan.aplicar(*x as i32));
        med.mirar_uno(y as i32);
        if escribir {
            *x = y;
        }
    }
    med.rms_dbfs()
}

/// **Normaliza** `m` (un bucle mono ya compuesto) desde su `fuerza` medida
/// hasta [`OBJETIVO`]. Devuelve lo que subio, en 1/256 dB (0 = nada).
///
/// El limite se come parte de lo que se sube (sujeta los golpes de la
/// bateria), asi que la ganancia se BUSCA: unas pasadas que solo leen y miden
/// lo que de verdad saldria, y una ultima que escribe. Sin memoria de mas: el
/// banco del escritorio no tiene sitio para una copia.
pub fn normalizar(m: &mut [i16], fuerza: MilesimasDb) -> MilesimasDb {
    let mut g = (OBJETIVO - fuerza).clamp(0, SUBIDA_MAX);
    if g == 0 || m.is_empty() {
        return 0;
    }
    for _ in 0..PASADAS {
        let sale = pasar(m, g, false);
        let falta = OBJETIVO - sale;
        if falta.abs() < DB / 4 {
            break;
        }
        g = (g + falta).clamp(0, SUBIDA_MAX);
    }
    pasar(m, g, true);
    g
}
