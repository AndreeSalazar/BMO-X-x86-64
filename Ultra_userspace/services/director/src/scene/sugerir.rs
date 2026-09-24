//! **LA LINEA DE SUGERENCIAS** -- bajo el campo de Ejecutar, donde vive el
//! estado: mientras se teclea, las ordenes que empiezan asi; al invocar la
//! caja, la pista del consejero en UNA linea.
//!
//! [consumo] NADA      pinta una linea cuando cambia lo tecleado, no en el
//!                     parpadeo del cursor
//!
//! Aqui no se sabe que ordenes hay: llegan como texto (`commands::sugerencias`
//! decide, esto solo pinta). La ELEGIDA --la que TAB escribe; cada TAB, la
//! siguiente-- va en el acento con un punto delante, las demas apagadas;
//! detras, que hace la elegida, si cabe.
//!
//! ```text
//!    Tab  gpu init   gpu estatica   gpu objetos  +2    corre el secuenciador y ...
//! ```

use bmo_userland as bmo;

use super::{acento, RunBox, BOX_BG, INK_DIM};

/// Limpia la linea de estado entera: media frase vieja detras de una nueva
/// es peor que ninguna (lo mismo que `paint_status`). Devuelve donde empieza
/// y donde acaba.
fn limpiar(p: &bmo::Pantalla, c: &RunBox) -> (u32, u32) {
    p.rect(c.x + 18, c.status_y, c.w() - 36, bmo::GLIFO_ALTO, BOX_BG);
    (c.x + 18, c.x + c.w() - 18)
}

/// Escribe `s` en `x`, recortado a lo que cabe antes de `fin`. Devuelve donde
/// acaba.
fn trozo(p: &bmo::Pantalla, c: &RunBox, x: u32, fin: u32, s: &[u8], color: u32) -> u32 {
    let caben = (fin.saturating_sub(x) / bmo::GLIFO_ANCHO) as usize;
    p.texto_bytes(x, c.status_y, &s[..s.len().min(caben)], color)
}

/// **Las sugerencias.** `lineas` son las ordenes, `elegida` la que TAB
/// escribe (en el acento, con su marca), `mas` las que no caben en la fila, y
/// `que` lo que hace la elegida.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &RunBox, lineas: &[&[u8]], elegida: usize, mas: usize, que: &[u8]) {
    let (mut x, fin) = limpiar(p, c);
    x = trozo(p, c, x, fin, b"Tab  ", INK_DIM);
    for (k, l) in lineas.iter().enumerate() {
        if k == elegida {
            // La marca: un punto del acento delante, como la luz del panel.
            p.rect(x, c.status_y + bmo::GLIFO_ALTO / 2 - 2, 4, 4, acento());
            x += 8;
        }
        x = trozo(p, c, x, fin, l, if k == elegida { acento() } else { INK_DIM });
        x = trozo(p, c, x, fin, b"   ", INK_DIM);
    }
    if mas > 0 {
        x = trozo(p, c, x, fin, b"+", INK_DIM);
        let mut d = [0u8; 10];
        let n = crate::text::decimal(mas as u64, &mut d);
        x = trozo(p, c, x, fin, &d[..n], INK_DIM);
        x = trozo(p, c, x, fin, b"   ", INK_DIM);
    }
    // Lo que hace la primera, solo si cabe entero con un margen: una
    // explicacion cortada a media palabra se lee peor que ninguna.
    if !que.is_empty() && x + (que.len() as u32 + 2) * bmo::GLIFO_ANCHO <= fin {
        trozo(p, c, x, fin, que, INK_DIM);
    }
}

/// **La pista del consejero** al invocar la caja: `etiqueta` en el acento y
/// el texto apagado, en la misma linea.
pub(crate) fn pista(p: &bmo::Pantalla, c: &RunBox, etiqueta: &[u8], texto: &[u8]) {
    let (mut x, fin) = limpiar(p, c);
    x = trozo(p, c, x, fin, etiqueta, acento());
    x = trozo(p, c, x, fin, b"  ", INK_DIM);
    trozo(p, c, x, fin, texto, INK_DIM);
}
