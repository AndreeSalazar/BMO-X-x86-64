//! **EL ESCRITORIO DE MISION** -- `fondo = mision` (HM3 de
//! `docs/plan/PLAN_EL_HUD.md`, 06-10).
//!
//! [consumo] NADA      pinta UNA vez, cuando `fondo` prepara el fondo (al
//!                     arrancar, o al cambiarlo en `aspecto`); despues el
//!                     escritorio es una lectura de memoria, como la foto.
//!                     En reposo, cero fotogramas (L6h)
//!
//! Es `docs/arte/maqueta_escritorio_mision.html` en la CPU: el cielo, su
//! rejilla, la ESTRELLA GATO --el SOL DE PLASMA, quieto-- con la orbita de su
//! planeta, y las esquinas del HUD. Viva, en la 3060, es HM3c; sin ella, esto.
//!
//! ```text
//!    el cielo      de MISION_CIELO_FONDO arriba a MISION_FONDO abajo
//!    las estrellas tres capas, con la semilla de la maqueta; MISION_TINTA
//!    la galaxia    una mancha azul tenue arriba a la derecha (MISION_AZUL)
//!    la rejilla    cada 120 px, cruzando por el centro del sol (MISION_REJILLA)
//!    la orbita     elipse discontinua (MISION_BORDE)
//!    el sol        `sol_gen.rs`, de `toolchain/tools/maqueta/escritorio/sol.maqueta`
//!    el planeta    `planeta_gen.rs`, con su nombre en la letra de la casa
//!    las esquinas  los cuatro angulos del HUD (MISION_OJO)
//! ```
//!
//! ## *** Por que se pinta en MEMORIA y no en la pantalla
//!
//! El fondo lo preguntan DOS: el que lo pinta y el que lo restaura (el cursor,
//! una ventana que se cierra: `scene::background_at`). Un sol pintado encima
//! del degradado dejaria, al cerrar una ventana, un hueco de degradado en mitad
//! de la estrella. Asi que el escritorio entero se pinta UNA vez en un bufer, y
//! `fondo` lo sirve igual que sirve la foto: pintar y restaurar leen lo mismo.
//!
//! Los colores son los de la paleta de mision (`tema_gen.rs`, HM1); los del sol
//! y el planeta son los de su SVG, que es el dibujo.

use bmo_userland as bmo;

use super::tema_gen::{MISION_AZUL, MISION_BORDE, MISION_CIELO_FONDO, MISION_FONDO, MISION_NEON, MISION_OJO, MISION_REJILLA, MISION_TINTA};
use super::{planeta_gen, sol_gen};

/// El centro del sol, en milesimas de la pantalla: el de la maqueta (1040, 520
/// de 1920 x 1080), a la derecha del centro porque el panel ocupa la izquierda.
const CENTRO: (u32, u32) = (542, 481);
/// El paso de la rejilla, en pixeles: el de la maqueta.
const REJILLA: u32 = 120;
/// La orbita, en milesimas de la pantalla (700 y 210 de 1920 x 1080).
const ORBITA: (u32, u32) = (365, 194);

/// **Donde cae el centro de la estrella** en una pantalla `w x h`: el sol del
/// escritorio y la nebulosa del INICIO (HM3b), en el MISMO sitio.
pub(crate) fn centro(w: u32, h: u32) -> (u32, u32) {
    (w * CENTRO.0 / 1000, h * CENTRO.1 / 1000)
}

/// **El cielo solo**, sin la estrella ni su planeta: el degradado, las
/// estrellas, la galaxia y la rejilla. Lo comparten el escritorio y el INICIO
/// (HM3b), que pone encima la NEBULOSA en vez del sol.
pub(crate) fn cielo_solo(p: &bmo::Pantalla) {
    let (w, h) = (p.ancho, p.alto);
    // Se marca UNA vez: marcar por pixel es el 68 a 1 de `verde.rs`.
    p.marcar(0, 0, w, h);
    cielo(p);
    estrellas(p);
    galaxia(p, w * 854 / 1000, h * 231 / 1000);
    let (cx, cy) = centro(w, h);
    rejilla(p, cx, cy);
}

/// **Pinta el escritorio de mision entero** en `p` (el bufer del fondo).
pub(crate) fn pintar(p: &bmo::Pantalla) {
    let (w, h) = (p.ancho, p.alto);
    if w == 0 || h == 0 {
        return;
    }
    cielo_solo(p);
    let (cx, cy) = centro(w, h);
    let (rx, ry) = (w * ORBITA.0 / 1000, h * ORBITA.1 / 1000);
    orbita(p, cx, cy, rx, ry);
    sol_gen::pintar(p, cx.saturating_sub(sol_gen::ANCHO / 2), cy.saturating_sub(sol_gen::ALTO / 2));
    // El planeta, en su orbita a la derecha del sol: donde lo deja la maqueta
    // quieta.
    let (px, py) = (cx + rx, cy);
    planeta_gen::pintar(p, px.saturating_sub(planeta_gen::ANCHO / 2), py.saturating_sub(planeta_gen::ALTO / 2));
    // Su nombre, arriba a la derecha; si ahi no cabe (una pantalla chica),
    // a la izquierda.
    const NOMBRE: &[u8] = b"KEPLER-5600 b";
    let e = bmo::Estilo::normal(11).espaciado(200);
    let largo = p.medir(NOMBRE, e).max(0) as u32;
    let x = if px + 30 + largo + 8 <= w { px + 30 } else { px.saturating_sub(30 + largo) };
    p.letra(x as i32, py as i32 - 40, NOMBRE, MISION_NEON, e);
    esquinas(p);
}

/// Los cuatro angulos del HUD, para quien pinta el cielo sin el escritorio.
pub(crate) fn angulos(p: &bmo::Pantalla) {
    esquinas(p);
}

/// `c` sobre lo que haya en `(x, y)`, con `a` de 0 a 255.
fn mezclar(p: &bmo::Pantalla, x: u32, y: u32, c: u32, a: u32) {
    if x >= p.ancho || y >= p.alto || a == 0 {
        return;
    }
    let d = p.read(x, y);
    let canal = |s: u32| {
        let (cs, ds) = ((c >> s) & 0xFF, (d >> s) & 0xFF);
        ((cs * a + ds * (255 - a)) / 255) << s
    };
    p.punto_ya_marcado(x, y, canal(16) | canal(8) | canal(0));
}

/// El degradado del cielo, fila a fila: dos tonos de casi negro.
fn cielo(p: &bmo::Pantalla) {
    let h = p.alto;
    let tono = |s: u32, y: u32| {
        let (a, b) = ((MISION_CIELO_FONDO >> s) & 0xFF, (MISION_FONDO >> s) & 0xFF);
        // `b >= a` en los tres canales: el cielo se aclara hacia abajo.
        (a + (b.saturating_sub(a)) * y / h) << s
    };
    for y in 0..h {
        p.rect(0, y, p.ancho, 1, tono(16, y) | tono(8, y) | tono(0, y));
    }
}

/// El generador de la maqueta (`semilla` de `docs/arte/estrella_gato.js`):
/// la misma cuenta, en enteros.
struct Semilla(u64);

impl Semilla {
    /// De 0 a 1000.
    fn mil(&mut self) -> u32 {
        self.0 = (self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345)) & 0x7FFF_FFFF;
        (self.0 * 1000 / 0x7FFF_FFFF) as u32
    }
}

/// Tres capas de estrellas: muchas chicas y tenues, pocas grandes. Cuantas, por
/// area: las de la maqueta en 1920 x 1080.
fn estrellas(p: &bmo::Pantalla) {
    let (w, h) = (p.ancho, p.alto);
    let area = w as u64 * h as u64;
    let mut r = Semilla(5);
    // (cuantas en 1920 x 1080, radio en centesimas, brillo maximo en milesimas)
    for (n, radio, brillo) in [(340u64, 50u32, 300u32), (160, 100, 600), (50, 170, 1000)] {
        for _ in 0..(n * area / (1920 * 1080)) {
            let x = r.mil() * w / 1000;
            let y = r.mil() * h / 1000;
            let rad = radio * (500 + r.mil()) / 1000;
            let a = (150 + r.mil() * brillo * 7 / 10_000) * 255 / 1000;
            if rad < 90 {
                // Menos de un pixel: un pixel, tanto mas tenue cuanto mas chica.
                mezclar(p, x, y, MISION_TINTA, a * rad / 90);
            } else {
                mezclar(p, x, y, MISION_TINTA, a);
                let lado = a * (rad - 90).min(100) / 200;
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    mezclar(p, x.wrapping_add_signed(dx), y.wrapping_add_signed(dy), MISION_TINTA, lado);
                }
            }
        }
    }
}

/// La raiz entera.
fn raiz(v: u64) -> u64 {
    let mut x = v;
    let mut y = x.div_ceil(2);
    if v < 2 {
        return v;
    }
    while y < x {
        x = y;
        y = (x + v / x) / 2;
    }
    x
}

/// La galaxia: una elipse de luz azul, girada 24 grados, que se apaga del
/// centro al borde.
fn galaxia(p: &bmo::Pantalla, gx: u32, gy: u32) {
    const RX: i64 = 120;
    const RY: i64 = 34;
    // -24 grados, en milesimas.
    const COS: i64 = 914;
    const SEN: i64 = -407;
    for dy in -RX..=RX {
        for dx in -RX..=RX {
            let u = (dx * COS + dy * SEN) * 1000 / (RX * 1000);
            let v = (-dx * SEN + dy * COS) * 1000 / (RY * 1000);
            let d2 = (u * u + v * v) as u64;
            if d2 >= 1_000_000 {
                continue;
            }
            let a = (1000 - raiz(d2)) * 35 * 255 / 100_000;
            mezclar(p, (gx as i64 + dx) as u32, (gy as i64 + dy) as u32, MISION_AZUL, a as u32);
        }
    }
}

/// La rejilla: cada [`REJILLA`] pixeles, con una linea por el centro del sol.
fn rejilla(p: &bmo::Pantalla, cx: u32, cy: u32) {
    let (w, h) = (p.ancho, p.alto);
    let a = 7 * 255 / 10;
    let mut x = cx % REJILLA;
    while x < w {
        for y in 0..h {
            mezclar(p, x, y, MISION_REJILLA, a);
        }
        x += REJILLA;
    }
    let mut y = cy % REJILLA;
    while y < h {
        for x in 0..w {
            // El cruce ya tiene su linea: no se mezcla dos veces.
            if x % REJILLA != cx % REJILLA {
                mezclar(p, x, y, MISION_REJILLA, a);
            }
        }
        y += REJILLA;
    }
}

/// La orbita: una elipse discontinua (2 pixeles si, 7 no, como la maqueta),
/// recorrida con un giro fijo en coma fija -- sin seno ni coseno.
fn orbita(p: &bmo::Pantalla, cx: u32, cy: u32, rx: u32, ry: u32) {
    // 2 pi / 4096, en Q30.
    const COS_D: i64 = 1_073_740_561;
    const SEN_D: i64 = 1_647_099;
    const UNO: i64 = 1 << 30;
    let (mut c, mut s) = (UNO, 0i64);
    let punto = |c: i64, s: i64| ((cx as i64 * 256 + rx as i64 * 256 * c / UNO), (cy as i64 * 256 + ry as i64 * 256 * s / UNO));
    let mut antes = punto(c, s);
    // El largo recorrido, en 1/256 de pixel.
    let mut largo = 0i64;
    for _ in 0..4096 {
        (c, s) = ((c * COS_D - s * SEN_D) >> 30, (s * COS_D + c * SEN_D) >> 30);
        let ahora = punto(c, s);
        let (dx, dy) = (ahora.0 - antes.0, ahora.1 - antes.1);
        largo += raiz((dx * dx + dy * dy) as u64) as i64;
        antes = ahora;
        if largo % (9 * 256) < 2 * 256 {
            let (x, y) = (ahora.0 / 256, ahora.1 / 256);
            if x >= 0 && y >= 0 {
                mezclar(p, x as u32, y as u32, MISION_BORDE, 255);
            }
        }
    }
}

/// Los cuatro angulos del HUD: dos trazos de 2 pixeles en cada esquina.
fn esquinas(p: &bmo::Pantalla) {
    const DENTRO: u32 = 16;
    const BRAZO: u32 = 28;
    let (w, h) = (p.ancho, p.alto);
    if w < 2 * (DENTRO + BRAZO) || h < 2 * (DENTRO + BRAZO) {
        return;
    }
    let (x1, y1) = (w - DENTRO - BRAZO, h - DENTRO - BRAZO);
    for (x, y) in [(DENTRO, DENTRO), (x1, DENTRO), (DENTRO, y1), (x1, y1)] {
        // El brazo horizontal y el vertical, pegados a la esquina de fuera.
        let hx = x;
        let hy = if y == DENTRO { y } else { y + BRAZO - 2 };
        let vx = if x == DENTRO { x } else { x + BRAZO - 2 };
        p.rect(hx, hy, BRAZO, 2, MISION_OJO);
        p.rect(vx, y, 2, BRAZO, MISION_OJO);
    }
}
