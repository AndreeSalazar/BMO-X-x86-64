//! **EL GATO VIVO** -- el logo animado de BMO-X, fotograma a fotograma (01-10).
//!
//! [consumo] NADA      no corre solo: pinta UN fotograma cuando quien lo llama
//!                     ya esta pintando (la intro, el despertar, el panel) (L6h)
//!
//! El propietario, con `docs/arte/bmo-x-gato-hd.svg` delante: *"estan perfectos
//! asi, ya aplicar en BMO-X"*. Aquel SVG es el gato ORIGINAL calcado con las
//! animaciones encima; este es el mismo gato --las mascaras de `scene::gato`
//! salen de la misma `bmo-x-gato.jpg`-- con las MISMAS animaciones, en los
//! mismos tiempos:
//!
//! ```text
//!    halo        el contorno en cian, apagado, alrededor del trazo
//!    ojos        laten (2,6 s) y parpadean (cada 6 s, 120 ms cerrados)
//!    glitch      cada 3,7 s los ojos se parten en rosa y azul y saltan
//!    franjas     cada 5,3 s dos franjas del gato saltan en rosa
//!    escaneo     una linea cian baja por el gato cada 3,2 s
//! ```
//!
//! Todo sale de `ms`: el mismo `ms` da el mismo fotograma, asi que quien llama
//! decide el ritmo y nada aqui guarda estado.

use bmo_userland as bmo;

use super::gato::{EYES, HEIGHT, STROKE, WIDTH};
use super::globo::{mezcla, onda};

const BLANCO: u32 = 0x00F2_F7F9;
const CIAN: u32 = 0x005E_F2E6;
const ROSA: u32 = 0x00FF_2E88;
const AZUL: u32 = 0x003D_A5FF;

/// El margen que el fotograma limpia alrededor del gato, en pixeles de la
/// mascara: el halo y los saltos del glitch caben dentro, y fuera no se pinta.
pub(crate) const MARGEN: u32 = 5;

fn bit(m: &[u8], fx: u32, fy: u32) -> bool {
    let i = (fy * WIDTH + fx) as usize;
    m[i / 8] >> (i % 8) & 1 == 1
}

/// El salto del glitch en el instante `k` de su ventana, en pixeles de la
/// mascara. Los mismos cuatro tiempos que el SVG.
fn salto(k: u64, largo: u64) -> i32 {
    match k * 4 / largo.max(1) {
        0 => -3,
        1 => 2,
        2 => -1,
        _ => 4,
    }
}

/// **Un fotograma del gato** en `(x, y)` a `escala`, en el instante `ms`.
/// Limpia su caja (`MARGEN` incluido) con `fondo` y pinta encima; `ojos` es el
/// color de los ojos (el acento de quien llama).
pub(crate) fn fotograma(p: &bmo::Pantalla, x: u32, y: u32, escala: u32, ms: u64, fondo: u32, ojos: u32) {
    let e = escala.max(1);
    let m = MARGEN * e;
    p.rect(x.saturating_sub(m), y.saturating_sub(m), WIDTH * e + 2 * m, HEIGHT * e + 2 * m, fondo);
    let px = |fx: u32| x + fx * e;
    let py = |fy: u32| y + fy * e;

    // -- el halo: el contorno en cian apagado --
    let halo = mezcla(fondo, CIAN, 70);
    for fy in 0..HEIGHT {
        for fx in 0..WIDTH {
            if bit(&STROKE, fx, fy) || bit(&EYES, fx, fy) {
                p.rect(px(fx).saturating_sub(e), py(fy).saturating_sub(e), 3 * e, 3 * e, halo);
            }
        }
    }
    // -- el trazo --
    for fy in 0..HEIGHT {
        for fx in 0..WIDTH {
            if bit(&STROKE, fx, fy) {
                p.rect(px(fx), py(fy), e, e, BLANCO);
            }
        }
    }

    // -- las franjas que saltan (5,3 s, del 89 % al 95 %) --
    let c = ms % 5300;
    if (4717..5035).contains(&c) {
        let dx = salto(c - 4717, 318) * 3;
        for (a, b) in [(HEIGHT * 55 / 100, HEIGHT * 60 / 100), (HEIGHT * 80 / 100, HEIGHT * 83 / 100)] {
            for fy in a..b {
                for fx in 0..WIDTH {
                    if bit(&STROKE, fx, fy) {
                        let qx = (px(fx) as i32 + dx * e as i32).max(0) as u32;
                        p.rect(qx, py(fy), e, e, ROSA);
                    }
                }
            }
        }
    }

    // -- los ojos: laten, parpadean y dan glitch --
    let cerrados = (2760..2880).contains(&(ms % 6000));
    let g = ms % 3700;
    let glitch = (3108..3478).contains(&g);
    let pinta_ojos = |color: u32, dx: i32| {
        for fy in 0..HEIGHT {
            for fx in 0..WIDTH {
                if bit(&EYES, fx, fy) {
                    let qx = (px(fx) as i32 + dx * e as i32).max(0) as u32;
                    p.rect(qx, py(fy), e, e, color);
                }
            }
        }
    };
    if glitch {
        let dx = salto(g - 3108, 370);
        pinta_ojos(ROSA, dx);
        pinta_ojos(AZUL, -dx);
    }
    if cerrados {
        // Cerrados: solo el halo, como un parpado que tapa la luz.
        pinta_ojos(halo, 0);
    } else {
        pinta_ojos(mezcla(ojos, BLANCO, onda(ms, 2600) * 50 / 256), 0);
    }

    // -- el escaneo --
    let sy = ((ms % 3200) * (HEIGHT * e) as u64 / 3200) as u32;
    p.rect(x, y + sy, WIDTH * e, e.max(2), mezcla(fondo, CIAN, 120));
}

/// **Los ojos `=` del panel** en el instante `ms`: el parpadeo y el glitch,
/// para quien pinta los ojos solos (el logo de la barra). Devuelve
/// `(cerrados, salto)`: cerrados no se pintan; con salto, rosa a la derecha y
/// azul a la izquierda antes de los ojos.
pub(crate) fn estado_ojos(ms: u64) -> (bool, i32) {
    let cerrados = (2760..2880).contains(&(ms % 6000));
    let g = ms % 3700;
    let salto = if (3108..3478).contains(&g) { salto(g - 3108, 370) } else { 0 };
    (cerrados, salto)
}
