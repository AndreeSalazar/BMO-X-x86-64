//! **LAS PIEZAS DEL HUD, en Rust** (HM4 de `docs/plan/PLAN_EL_HUD.md`, 07-10):
//! el marco de instrumento y la lectura de `toolchain/tools/maqueta/pruebas/hud/`
//! (HM2), para lo que se pinta con datos de cada cuarto de segundo.
//!
//! [consumo] NADA      son funciones de pintar: corren cuando alguien pinta
//!
//! Las mismas medidas y los mismos colores que las `.maqueta` (la paleta de
//! mision de `tema_gen.rs`): lo que alli es la muestra, aqui es el dato.

use bmo_userland as bmo;

use super::tema_gen::{MISION_BORDE, MISION_FONDO, MISION_OJO, MISION_TENUE};

/// La L de cada esquina del marco.
const L: u32 = 14;

/// **El marco de instrumento**: el panel, su raya, las cuatro esquinas en L del
/// ojo del gato, el rotulo arriba a la izquierda y su codigo a la derecha.
pub(crate) fn marco(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), rotulo: &[u8], codigo: &[u8]) {
    if w < 2 * L || h < 2 * L {
        return;
    }
    p.rect(x, y, w, h, MISION_BORDE);
    p.rect(x + 1, y + 1, w - 2, h - 2, MISION_FONDO);
    for (ex, ey, vx, vy) in [(x, y, x, y), (x + w - L, y, x + w - 2, y), (x, y + h - 2, x, y + h - L), (x + w - L, y + h - 2, x + w - 2, y + h - L)] {
        p.rect(ex, ey, L, 2, MISION_OJO);
        p.rect(vx, vy, 2, L, MISION_OJO);
    }
    let e = bmo::Estilo::media(11).espaciado(180).mayusculas();
    if !rotulo.is_empty() {
        p.letra((x + 12) as i32, (y + 18) as i32, rotulo, MISION_OJO, e);
    }
    if !codigo.is_empty() {
        let c = bmo::Estilo::normal(10);
        let largo = p.medir(codigo, c).max(0) as u32;
        p.letra((x + w).saturating_sub(12 + largo) as i32, (y + 18) as i32, codigo, MISION_TENUE, c);
    }
}

/// **La lectura**: el rotulo chico, la cifra grande en `tinta` y su unidad al
/// pie. `y` es la de arriba del rotulo.
pub(crate) fn lectura(p: &bmo::Pantalla, x: u32, y: u32, rotulo: &[u8], valor: &[u8], unidad: &[u8], tinta: u32) {
    let chica = bmo::Estilo::normal(10).espaciado(140).mayusculas();
    p.letra(x as i32, (y + 10) as i32, rotulo, MISION_TENUE, chica);
    let grande = bmo::Estilo::media(26);
    // `letra` pinta con la BASE de la linea en `y`, y devuelve el ancho.
    let ancho = p.letra(x as i32, (y + 40) as i32, valor, tinta, grande).max(0);
    // La unidad como se escribe (dB, kHz, MiB): el rotulo va en mayusculas, ella no.
    p.letra(x as i32 + ancho + 6, (y + 40) as i32, unidad, MISION_TENUE, bmo::Estilo::normal(11));
}

/// `n` en decimal, con el punto de los miles, en `b`.
pub(crate) fn miles(n: u64, b: &mut [u8; 24]) -> usize {
    let mut d = [0u8; 20];
    let (mut k, mut v) = (0usize, n);
    loop {
        d[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let mut o = 0;
    for i in (0..k).rev() {
        b[o] = d[i];
        o += 1;
        if i > 0 && i % 3 == 0 {
            b[o] = b'.';
            o += 1;
        }
    }
    o
}
