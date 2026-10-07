//! **EL SIMULADOR** -- el instrumento de la LUDOTECA con el escritorio de
//! mision (HM6c de `docs/plan/PLAN_EL_HUD.md`, 07-10): *"cada juego, un
//! parche de mision"*.
//!
//! [consumo] NADA      se pinta con la ventana (L6h)
//!
//! ```text
//!    SIMULADOR                    F4
//!              .----.
//!             ( * C  )     el parche del juego fijado: su inicial, el
//!              '----'      aro del color de su camino
//!           Cyberpunk 2077
//!         PROTON-X  .  Steam
//!    JUEGOS  12      PROTON-X  listo
//! ```
//!
//! ** El parche lo pone lo que el catalogo YA dice del juego (el titulo, su
//! camino, su tienda): nada de arte inventado por juego. El aro es del color
//! del camino, el mismo de la cabecera del canal.

use crate::canvas::Canvas;
use crate::mision::{marco, miles, CUIDADO, GO, OJO, TENUE, TINTA};
use crate::mates::{fase, seno};
use crate::piezas::{arco, medir, texto, texto_cabe, Estilo};
use bmo_dibujo::{mezclar, Color};

/// Lo que mide el instrumento.
pub const ALTO: i32 = 232;

/// Lo que el parche sabe del juego fijado.
pub struct Fijado<'a> {
    pub titulo: &'a [u8],
    /// El camino como lo escribe la cabecera, con su color.
    pub camino: (&'a [u8], Color),
    pub tienda: &'a [u8],
}

/// **Pinta el simulador** en `(x, y)`, `w` de ancho.
pub fn pintar(cv: &mut Canvas, x: i32, y: i32, w: i32, fijado: Option<Fijado>, juegos: usize, proton: bool, ms: u32) {
    marco(cv, x, y, w, ALTO, b"SIMULADOR", b"F4");
    let (cx, cy) = (x + w / 2, y + 84);
    let aro = fijado.as_ref().map_or(TENUE, |f| f.camino.1);
    // El parche: el aro del camino, el fondo, el aro fino del ojo y tres
    // estrellas que respiran.
    cv.disc(cx, cy, 42, aro);
    cv.disc(cx, cy, 37, mezclar(aro, 0x0009_080F, 40, 256));
    arco(cv, cx, cy, 32, 0, 256, mezclar(OJO, 0x0009_080F, 150, 256));
    for k in 0..3 {
        let brillo = 140 + seno(fase(ms + k as u32 * 500, 2400)) * 100 / 256;
        cv.disc(cx - 14 + k * 14, cy - 22 + if k == 1 { -3 } else { 0 }, 2, mezclar(TINTA, 0x0009_080F, brillo as u32, 256));
    }
    let Some(f) = fijado else {
        let e = Estilo::media(26);
        texto(cv, cx - medir(b"?", e) / 2, cy - 14, 32, b"?", TENUE, e);
        let n = Estilo::normal(12);
        texto(cv, cx - medir(b"sin juego fijado", n) / 2, y + 136, 18, b"sin juego fijado", TENUE, n);
        lecturas(cv, x, y, w, juegos, proton);
        return;
    };
    // La inicial del juego, grande, en el centro del parche.
    let ini = [f.titulo.first().map_or(b'?', |c| c.to_ascii_uppercase())];
    let e = Estilo::negrita(30);
    texto(cv, cx - medir(&ini, e) / 2, cy - 14, 36, &ini, TINTA, e);
    // El nombre debajo, centrado y recortado si no cabe.
    let t = Estilo::media(14);
    let ancho = medir(f.titulo, t).min(w - 32);
    texto_cabe(cv, cx - ancho / 2, y + 134, 20, f.titulo, TINTA, t, w - 32);
    let n = Estilo::normal(11);
    let a = medir(f.camino.0, n);
    let b = medir(b"  .  ", n);
    let c = medir(f.tienda, n);
    let x0 = cx - (a + b + c) / 2;
    texto(cv, x0, y + 156, 16, f.camino.0, f.camino.1, n);
    texto(cv, x0 + a, y + 156, 16, b"  .  ", TENUE, n);
    texto_cabe(cv, x0 + a + b, y + 156, 16, f.tienda, TENUE, n, (x + w - 12) - (x0 + a + b));
    lecturas(cv, x, y, w, juegos, proton);
}

/// Las dos lecturas de abajo: cuantos juegos y si hay simulador de Windows.
fn lecturas(cv: &mut Canvas, x: i32, y: i32, w: i32, juegos: usize, proton: bool) {
    let rotulo = Estilo::normal(10).espaciado(140).mayusculas();
    let cifra = Estilo::media(15);
    let mut b = [0u8; 24];
    let k = miles(juegos as u64, &mut b);
    texto(cv, x + 16, y + 184, 14, b"JUEGOS", TENUE, rotulo);
    texto(cv, x + 16, y + 199, 20, &b[..k], TINTA, cifra);
    let xd = x + w / 2 + 4;
    texto(cv, xd, y + 184, 14, b"PROTON-X", TENUE, rotulo);
    let (dice, c): (&[u8], Color) = if proton { (b"listo", GO) } else { (b"falta", CUIDADO) };
    texto(cv, xd, y + 199, 20, dice, c, cifra);
}
