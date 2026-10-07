//! **Lo que la PILA A escribe en Rust** (MAQUETA 3, 06-10, MA2). Aparte de
//! `rust.rs` por medida, y con su misma regla: traduce, no decide. Lo que se
//! decide esta en `capas.rs`.

use bmo_maqueta_layout::{Laid, Rect};
use std::fmt::Write;

/// El recorte que lleva una llamada: el de siempre (`None` o `Some(limite)`)
/// y, si un `overflow` corta el trazo a medias, tambien ese.
pub(crate) fn limite_de(base: &str, recorte: Option<Rect>) -> String {
    let Some(c) = recorte else { return base.to_string() };
    let r = format!("Recorte::nuevo(ox as i32 + {}, oy as i32 + {}, {}, {})", c.x, c.y, c.w, c.h);
    if base == "None" {
        format!("Some({r})")
    } else {
        format!("Some({r}.interseccion(&limite))")
    }
}

/// **`puntero`**: la forma que pide un punto, de las cajas que dijeron
/// `cursor`. Solo si alguna lo dijo: sin zonas, el modulo de siempre.
pub(crate) fn puntero(s: &mut String, l: &Laid) {
    let zonas = crate::capas::zonas(l);
    if zonas.is_empty() {
        return;
    }
    let _ = writeln!(
        s,
        "/// **La forma del puntero en `(px, py)`** (`cursor`, MAQUETA 3): la de la\n\
         /// caja de mas encima que la dijo, o `\"\"` si ninguna.\n\
         pub fn puntero(ox: u32, oy: u32, px: u32, py: u32) -> &'static str {{\n\
         \x20   const ZONAS: [(i32, i32, u32, u32, &str); {}] = [",
        zonas.len()
    );
    for (r, n) in &zonas {
        let _ = writeln!(s, "        ({}, {}, {}, {}, {n:?}),", r.x, r.y, r.w, r.h);
    }
    s.push_str(
        "    ];\n\
         \x20   let (x, y) = (px as i64 - ox as i64, py as i64 - oy as i64);\n\
         \x20   for &(zx, zy, zw, zh, n) in ZONAS.iter().rev() {\n\
         \x20       if x >= zx as i64 && y >= zy as i64 && x < zx as i64 + zw as i64 && y < zy as i64 + zh as i64 {\n\
         \x20           return n;\n\
         \x20       }\n\
         \x20   }\n\
         \x20   \"\"\n\
         }\n\n",
    );
}
