//! **LOS DIBUJOS QUE ANIMAN** (MAQUETA 3, S7): sus pasos, escritos.
//!
//! El lector de SVG (`bmo_maqueta_dibujo::pasos`) ya convirtio cada
//! animacion en PASOS: el dibujo entero, aplanado, en los instantes que
//! hacen falta, todos con la misma forma. Aqui se escriben:
//!
//! ```text
//!    ANIMA_MS          el ciclo mas largo de los dibujos de la maqueta
//!    pintar_anima      a los `ms`: debajo de cada dibujo, `pintar_en_fijo`
//!                      (todo menos los dibujos que animan); encima, los
//!                      dos pasos vecinos MEZCLADOS punto a punto
//!                      (`Pantalla::pieza_entre`, sin monton)
//! ```
//!
//! En reposo no corre nada: quien llama decide el ritmo (L6h). Y la foto
//! del anfitrion a los `ms` (`foto::foto_anima`) sale de estas MISMAS
//! listas: es el oraculo de lo que hara el aparato.

use std::fmt::Write;

use bmo_maqueta_dibujo::anima::Pasos;
use bmo_maqueta_layout::{Frame, Laid};

use crate::literal::pieza_literal;
use crate::orden::{figuras_como_trazos, nombre_de, Trazo};

/// **Los pasos de un `<svg>` que anima**, o `None`.
pub fn de(f: &Frame) -> Option<Pasos> {
    let d = f.dibujo.as_ref()?;
    let h = bmo_maqueta_cascade::herencia(&f.style);
    let c = f.content;
    bmo_maqueta_dibujo::pasos(d, &h, (c.x as f64, c.y as f64, c.w as f64, c.h as f64))?.ok()
}

/// Si algun dibujo de la maqueta anima.
pub fn hay(l: &Laid) -> bool {
    l.all().iter().any(|f| de(f).is_some())
}

/// Un dibujo que anima, con sus pasos ya como trazos y la caja que mancha.
pub struct Animado {
    pub de: String,
    pub ciclo_ms: u32,
    pub repite: bool,
    pub tiempos: Vec<u32>,
    /// `trazos[paso][figura]`.
    pub trazos: Vec<Vec<Trazo>>,
    pub caja: (i32, i32, i32, i32),
}

/// Los dibujos que animan, en orden de pintado.
pub fn animados(l: &Laid) -> Vec<Animado> {
    l.all()
        .iter()
        .filter_map(|f| {
            let p = de(f)?;
            let trazos: Vec<Vec<Trazo>> = p.figuras.iter().map(|fs| figuras_como_trazos(fs, true)).collect();
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for t in trazos.iter().flatten() {
                if let Some((x, y, w, h)) = t.con_pieza(bmo_pinta::caja_de) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + w);
                    y1 = y1.max(y + h);
                }
            }
            let caja = if x0 > x1 { (0, 0, 0, 0) } else { (x0, y0, x1 - x0, y1 - y0) };
            Some(Animado { de: nombre_de(f), ciclo_ms: p.ciclo_ms, repite: p.repite, tiempos: p.tiempos, trazos, caja })
        })
        .collect()
}

impl Animado {
    /// El tramo (`k` y `k + 1`) y su avance en milesimas a los `ms`.
    pub fn tramo(&self, ms: u32) -> (usize, i32) {
        let n = self.tiempos.len();
        if n < 2 {
            return (0, 0);
        }
        let t = if self.repite { ms % self.ciclo_ms.max(1) } else { ms.min(self.ciclo_ms) };
        let k = self.tiempos.partition_point(|&x| x <= t).saturating_sub(1).min(n - 2);
        let (t0, t1) = (self.tiempos[k], self.tiempos[k + 1]);
        let mil = if t1 > t0 { ((t.saturating_sub(t0)) as u64 * 1000 / (t1 - t0) as u64).min(1000) as i32 } else { 1000 };
        (k, mil)
    }
}

/// **Escribe `ANIMA_MS` y `pintar_anima`.**
pub fn emitir(s: &mut String, l: &Laid) {
    let an = animados(l);
    if an.is_empty() {
        return;
    }
    let ciclo = an.iter().map(|a| a.ciclo_ms).max().unwrap_or(0);
    let _ = writeln!(
        s,
        "// ------------------------------------------------------------------------\n\
         //  Los dibujos que animan (MAQUETA 3, S7)\n\
         // ------------------------------------------------------------------------\n\n\
         /// El ciclo mas largo de los dibujos que animan, en ms.\n\
         pub const ANIMA_MS: u32 = {ciclo};\n\n\
         /// **Pinta los dibujos que animan a los `ms` de empezar.** Debajo de\n\
         /// cada uno se repinta lo que hay (`pintar_en_fijo`) y encima va el\n\
         /// dibujo en ese instante: los dos pasos vecinos mezclados punto a\n\
         /// punto. En reposo no corre nada: quien llama decide el ritmo (L6h).\n\
         pub fn pintar_anima(p: &bmo::Pantalla, ox: u32, oy: u32, ms: u32) {{"
    );
    for k in 0..an.len() {
        let _ = writeln!(s, "    anima_{k}(p, ox, oy, ms);");
    }
    s.push_str("}\n\n");
    for (k, a) in an.iter().enumerate() {
        let (np, nf) = (a.trazos.len(), a.trazos.first().map_or(0, Vec::len));
        let (x, y, w, h) = a.caja;
        let _ = writeln!(
            s,
            "/// {}: {np} pasos, ciclo de {} ms{}.\n\
             fn anima_{k}(p: &bmo::Pantalla, ox: u32, oy: u32, ms: u32) {{\n\
             \x20   const TIEMPOS: [u32; {np}] = {:?};\n\
             \x20   static PASOS: [[bmo::Pieza<'static>; {nf}]; {np}] = [",
            a.de,
            a.ciclo_ms,
            if a.repite { ", se repite" } else { ", una vez" },
            a.tiempos
        );
        for paso in &a.trazos {
            let v: Vec<String> = paso.iter().filter_map(|t| t.con_pieza(pieza_literal)).collect();
            let _ = writeln!(s, "        [{}],", v.join(", "));
        }
        let _ = writeln!(
            s,
            "    ];\n\
             \x20   let t = {};\n\
             \x20   let k = TIEMPOS.partition_point(|&x| x <= t).saturating_sub(1).min({});\n\
             \x20   let (t0, t1) = (TIEMPOS[k], TIEMPOS[k + 1]);\n\
             \x20   let mil = if t1 > t0 {{ ((t - t0) as u64 * 1000 / (t1 - t0) as u64).min(1000) as i32 }} else {{ 1000 }};\n\
             \x20   pintar_en_fijo(p, ox, oy, (ox as i32 + {x}).max(0) as u32, (oy as i32 + {y}).max(0) as u32, {w}, {h});\n\
             \x20   for f in 0..{nf} {{\n\
             \x20       p.pieza_entre(&PASOS[k][f], &PASOS[k + 1][f], mil, ox as i32, oy as i32);\n\
             \x20   }}\n\
             }}\n",
            if a.repite { format!("ms % {}", a.ciclo_ms.max(1)) } else { format!("ms.min({})", a.ciclo_ms) },
            np.saturating_sub(2)
        );
    }
}
