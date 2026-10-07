//! **EL HUD DE MISION, del lado de las APPS** (HM6 de `docs/plan/PLAN_EL_HUD.md`,
//! 07-10). Lo comparten HERMES (F3), la LUDOTECA (F4) y BANK CAT (F5), como
//! `piezas.rs`.
//!
//! [consumo] NADA      son funciones de pintar, y la pregunta del fondo lee
//!                     el fichero UNA vez por proceso (L6h)
//!
//! Con el escritorio de mision detras, cada app lleva su INSTRUMENTO -- uno
//! distinto por tecla, como pide la seccion 2 del plan --, hecho con las
//! piezas de siempre del HUD (las de `services/director/src/scene/hud.rs`):
//!
//! ```text
//!    marco      el panel, su raya y las cuatro esquinas en L del ojo
//!    lectura    el rotulo chico, la cifra grande y su unidad al pie
//!    barra      lo que llena, sobre su carril, con la regla de marcas
//! ```
//!
//! ** Los colores son la paleta de mision del escritorio: el MISMO fichero
//! generado (`tema_gen.rs`, de `tema.maqueta`), no una copia. Y si hay
//! mision lo dice el MISMO `sys/director.cfg` que lee el escritorio, con la
//! misma regla (`bmo_config::es_mision`).

use core::sync::atomic::{AtomicU8, Ordering};

use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_userland as bmo;

use crate::canvas::Canvas;
use crate::piezas::{medir, texto, Estilo};

/// La paleta: la del escritorio, generada.
#[path = "../../../services/director/src/scene/tema_gen.rs"]
#[allow(dead_code)]
pub mod tema;

#[allow(unused_imports)]
pub use tema::{MISION_BORDE as BORDE, MISION_CUIDADO as CUIDADO, MISION_FONDO as FONDO, MISION_GO as GO, MISION_NEON as NEON, MISION_NOGO as NOGO, MISION_OJO as OJO, MISION_TENUE as TENUE, MISION_TINTA as TINTA};

/// 0 = sin preguntar, 1 = no, 2 = si.
static MISION: AtomicU8 = AtomicU8::new(0);

/// **Hay escritorio de mision detras?** Lee `sys/director.cfg` la primera vez.
pub fn es_mision() -> bool {
    match MISION.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let mut b = [0u8; 4096];
            let texto = bmo::Archivo::leer_de(b"sys/director.cfg").ok().map(|a| {
                let n = a.read(&mut b[..(a.size() as usize).min(4096)]);
                n
            });
            let si = bmo_config::es_mision(texto.map(|n| &b[..n]));
            MISION.store(if si { 2 } else { 1 }, Ordering::Relaxed);
            si
        }
    }
}

/// La L de cada esquina.
const L: i32 = 14;

/// **El marco de instrumento**: el panel, su raya, las esquinas en L, el
/// rotulo a la izquierda y su codigo (la tecla) a la derecha.
pub fn marco(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, rotulo: &[u8], codigo: &[u8]) {
    if w < 2 * L || h < 2 * L {
        return;
    }
    cv.rect(x, y, w, h, BORDE);
    cv.rect(x + 1, y + 1, w - 2, h - 2, FONDO);
    for (ex, ey, vx, vy) in [(x, y, x, y), (x + w - L, y, x + w - 2, y), (x, y + h - 2, x, y + h - L), (x + w - L, y + h - 2, x + w - 2, y + h - L)] {
        cv.rect(ex, ey, L, 2, OJO);
        cv.rect(vx, vy, 2, L, OJO);
    }
    if !rotulo.is_empty() {
        texto(cv, x + 12, y + 8, 14, rotulo, OJO, Estilo::media(11).espaciado(180).mayusculas());
    }
    if !codigo.is_empty() {
        let c = Estilo::normal(10);
        texto(cv, x + w - 12 - medir(codigo, c), y + 8, 14, codigo, TENUE, c);
    }
}

/// **La lectura**: el rotulo chico, la cifra grande en `tinta` y su unidad.
/// `y` es lo alto del rotulo; ocupa 44 px.
pub fn lectura(cv: &mut Canvas, x: i32, y: i32, rotulo: &[u8], valor: &[u8], unidad: &[u8], tinta: Color) {
    texto(cv, x, y, 14, rotulo, TENUE, Estilo::normal(10).espaciado(140).mayusculas());
    let ancho = texto(cv, x, y + 14, 30, valor, tinta, Estilo::media(24));
    texto(cv, x + ancho + 6, y + 23, 16, unidad, TENUE, Estilo::normal(11));
}

/// **La barra**: `parte` de `total` llena en `tinta`, sobre su carril, con una
/// marca cada cuarto. `total == 0` deja el carril vacio.
pub fn barra(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, parte: u64, total: u64, tinta: Color) {
    cv.rect(x, y, w, h, mezclar(BORDE, FONDO, 128, 256));
    let lleno = if total == 0 { 0 } else { ((parte.min(total) as u128 * w as u128) / total as u128) as i32 };
    cv.rect(x, y, lleno, h, tinta);
    for k in 1..4 {
        cv.rect(x + w * k / 4, y + h, 1, 4, TENUE);
    }
}

/// `n` en decimal, con el punto de los miles, en `b`; devuelve cuantos bytes.
pub fn miles(n: u64, b: &mut [u8; 24]) -> usize {
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
