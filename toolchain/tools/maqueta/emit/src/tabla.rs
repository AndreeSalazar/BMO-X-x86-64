//! **LA TABLA DE FIGURAS** (06-10, HM3 de `docs/plan/PLAN_EL_HUD.md`).
//!
//! Un dibujo de verdad son cientos de figuras. El SOL del escritorio de mision
//! son 225, y emitidas una a una salian DOS veces --en `pintar` y en
//! `pintar_en`, cada una con su literal entero--: 1005 lineas y 297 KB de
//! fuente para un solo dibujo. Aqui las figuras seguidas van a UNA tabla,
//!
//! ```text
//!    static FIGURAS: [(bmo::Pieza<'static>, [i32; 4]); N]   la pieza y su caja
//! ```
//!
//! y cada pintado la recorre con un bucle: `pintar` las pinta todas,
//! `pintar_en` solo las que tocan el recorte. Lo que se pinta es lo MISMO, en
//! el mismo orden: solo cambia como se escribe.
//!
//! Solo desde [`RACHA`] figuras seguidas, y nunca con datos (sus colores se
//! cambian por el dato en el texto de cada llamada): un dibujo chico se sigue
//! escribiendo como siempre, byte a byte.

use std::cell::RefCell;
use std::fmt::Write;

use crate::literal::pieza_literal;
use crate::orden::{Orden, Trazo};

/// Desde cuantas figuras seguidas compensa la tabla.
pub(crate) const RACHA: usize = 8;

/// Las filas de `FIGURAS`: el literal de la pieza y su caja `[x, y, w, h]`.
#[derive(Default)]
pub(crate) struct Tabla {
    filas: RefCell<Vec<(String, [i64; 4])>>,
}

impl Tabla {
    fn indice(&self, literal: String, caja: [i64; 4]) -> usize {
        let mut f = self.filas.borrow_mut();
        if let Some(k) = f.iter().position(|(l, c)| *l == literal && *c == caja) {
            return k;
        }
        f.push((literal, caja));
        f.len() - 1
    }

    /// Si en `ordenes[i]` empieza una racha de figuras que la tabla guarda
    /// SEGUIDAS: `(desde, hasta)` en la tabla. Cuantas ordenes se come es
    /// `hasta - desde`.
    pub(crate) fn racha(&self, ordenes: &[&Orden], i: usize) -> Option<(usize, usize)> {
        let n = ordenes[i..].iter().take_while(|o| matches!(o.trazo, Trazo::Figura { .. })).count();
        if n < RACHA {
            return None;
        }
        let mut k = Vec::with_capacity(n);
        for o in &ordenes[i..i + n] {
            let r = o.trazo.area();
            k.push(self.indice(o.trazo.con_pieza(pieza_literal)?, [r.x as i64, r.y as i64, r.w as i64, r.h as i64]));
        }
        k.windows(2).all(|w| w[1] == w[0] + 1).then(|| (k[0], k[0] + n))
    }

    /// El `static FIGURAS`, si alguna racha lo uso.
    pub(crate) fn emitir(&self, s: &mut String) {
        let f = self.filas.borrow();
        if f.is_empty() {
            return;
        }
        let _ = writeln!(
            s,
            "/// Las figuras de los dibujos, con su caja `[x, y, w, h]`: las recorren\n\
             /// `pintar` y `pintar_en` (la tabla de figuras de MAQUETA 3).\n\
             static FIGURAS: [(bmo::Pieza<'static>, [i32; 4]); {}] = [",
            f.len()
        );
        for (l, c) in f.iter() {
            let _ = writeln!(s, "    ({l}, [{}, {}, {}, {}]),", c[0], c[1], c[2], c[3]);
        }
        s.push_str("];\n\n");
    }
}

/// El bucle de `pintar`: todas, sin recorte.
pub(crate) fn bucle(a: usize, b: usize) -> String {
    format!("    for f in &FIGURAS[{a}..{b}] {{\n        p.pieza(&f.0, ox as i32, oy as i32, None);\n    }}")
}

/// El bucle de `pintar_en`: solo las que tocan `limite`, recortadas.
pub(crate) fn bucle_en(a: usize, b: usize) -> String {
    format!(
        "    for f in &FIGURAS[{a}..{b}] {{\n\
         \x20       if !Recorte::nuevo(ox as i32 + f.1[0], oy as i32 + f.1[1], f.1[2], f.1[3]).interseccion(&limite).vacio() {{\n\
         \x20           p.pieza(&f.0, ox as i32, oy as i32, Some(limite));\n\
         \x20       }}\n\
         \x20   }}"
    )
}
