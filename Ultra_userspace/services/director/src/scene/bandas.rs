//! **LAS BANDAS DE F10 Y F11** (HM5 de `docs/plan/PLAN_EL_HUD.md`, 07-10): con el
//! escritorio de mision, SONIDO y CABINA llevan arriba su instrumento, como
//! VITALES (`vitales_hud`):
//!
//! ```text
//!    F11 CABINA   REGISTRO DE VUELO   la hora de mision (T+, desde que la
//!                                     CPU cuenta), los vivos del anillo y
//!                                     los que se cayeron de el
//!    F10 SONIDO   LAZO DE AUDIO       el maestro (dB), la frecuencia, los
//!                                     canales y como esta el tubo
//! ```
//!
//! [consumo] NADA      se pintan con su ventana (L6h)
//!
//! Cada numero es uno que el kernel ya contesta (la cabina, el TSC) o que la
//! ventana ya leyo (`sound::Lectura`). Lo de debajo baja con la banda: las
//! filas de CABINA (`cabina::visible_rows`) y el fader de SONIDO
//! (`sound::sitio`, que leen el pintor y el raton) cuentan con ella.

use bmo_userland as bmo;

use super::hud::{lectura, marco, miles};
use super::tema_gen::{MISION_CUIDADO, MISION_GO, MISION_NEON, MISION_NOGO, MISION_OJO, MISION_TENUE};

/// Lo que ocupa una banda, con su aire de abajo.
pub(crate) const BANDA: u32 = 112;

/// **Lleva banda una ventana de este alto?** Con el escritorio de mision y
/// sitio para ella; si no, la ventana es la de siempre.
pub(crate) fn alto(ventana: u32, minimo: u32) -> u32 {
    if super::fondo::es_mision() && ventana >= minimo {
        BANDA
    } else {
        0
    }
}

/// `T+hh:mm:ss` de `s` segundos.
fn hora(s: u64, b: &mut [u8; 24]) -> usize {
    let (h, m, sg) = (s / 3600, (s / 60) % 60, s % 60);
    let mut n = miles(h, b);
    for v in [m, sg] {
        b[n] = b':';
        b[n + 1] = b'0' + (v / 10) as u8;
        b[n + 2] = b'0' + (v % 10) as u8;
        n += 3;
    }
    n
}

/// **F11: el registro de vuelo**, en `caja`.
pub(crate) fn registro(p: &bmo::Pantalla, caja: (u32, u32, u32, u32)) {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let s = if hz > 0 { bmo::ciclos() / hz } else { 0 };
    registro_con(p, caja, hz > 0, s, bmo::cabina_disponibles(), bmo::cabina_perdidos());
}

/// La pintura de F11 con lo medido (el banco del anfitrion la llama asi).
pub(crate) fn registro_con(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), con_reloj: bool, segundos: u64, vivos: u64, perdidos: u64) {
    let (x, y, _, _) = caja;
    marco(p, caja, b"REGISTRO DE VUELO", b"F11  RING 0");
    let mut b = [0u8; 24];
    if con_reloj {
        let n = hora(segundos, &mut b);
        lectura(p, x + 16, y + 30, b"HORA DE MISION", &b[..n], b"T+", MISION_OJO);
    } else {
        lectura(p, x + 16, y + 30, b"HORA DE MISION", b"--", b"sin reloj medido", MISION_TENUE);
    }
    let n = miles(vivos, &mut b);
    lectura(p, x + 260, y + 30, b"EN EL ANILLO", &b[..n], b"vivos", MISION_GO);
    let n = miles(perdidos, &mut b);
    lectura(p, x + 440, y + 30, b"SE CAYERON", &b[..n], b"del anillo", if perdidos > 0 { MISION_CUIDADO } else { MISION_TENUE });
}

/// **F10: el lazo de audio**, con lo que la ventana ya leyo.
pub(crate) fn lazo(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), l: &super::sound::Lectura) {
    lazo_con(p, caja, l.hay_aparato, l.fader, l.frecuencia, l.canales, l.mudo, l.estado);
}

/// La pintura de F10: el maestro en 1/256 dB, la frecuencia en Hz.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lazo_con(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), hay: bool, fader: i32, frecuencia: u32, canales: u8, mudo: bool, estado: u8) {
    let (x, y, _, _) = caja;
    marco(p, caja, b"LAZO DE AUDIO", b"F10");
    if !hay {
        lectura(p, x + 16, y + 30, b"APARATO", b"--", b"no hay", MISION_NOGO);
        return;
    }
    let mut b = [0u8; 24];
    // El maestro, redondeado al dB, con su signo.
    let db = (fader + if fader < 0 { -128 } else { 128 }) / 256;
    let mut n = 0;
    if db < 0 {
        b[0] = b'-';
        n = 1;
    }
    let mut d = [0u8; 24];
    let k = miles(db.unsigned_abs() as u64, &mut d);
    b[n..n + k].copy_from_slice(&d[..k]);
    n += k;
    lectura(p, x + 16, y + 30, b"MAESTRO", &b[..n], b"dB", if mudo { MISION_CUIDADO } else { MISION_OJO });
    let k = miles(frecuencia as u64 / 1000, &mut d);
    lectura(p, x + 200, y + 30, b"FRECUENCIA", &d[..k], b"kHz", MISION_TENUE);
    let k = miles(canales as u64, &mut d);
    lectura(p, x + 360, y + 30, b"CANALES", &d[..k], b"", MISION_TENUE);
    let (dice, tinta): (&[u8], u32) = match (mudo, estado) {
        (true, _) => (b"MUDO", MISION_CUIDADO),
        (false, 1) => (b"EN MARCHA", MISION_GO),
        (false, 0) => (b"SIN TUBO", MISION_TENUE),
        _ => (b"EL MAESTRO NO ACTUA", MISION_NEON),
    };
    lectura(p, x + 480, y + 30, b"EL TUBO", dice, b"", tinta);
}
