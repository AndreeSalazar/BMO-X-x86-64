//! **EL GLOBO DEL PUNTERO** -- un consejo o un dato que sigue al raton unos
//! segundos: el texto se escribe letra a letra, una barrita se vacia con el
//! tiempo que le queda, y al final se apaga.
//!
//! [consumo] NADA      pinta solo en los fotogramas que ya pintan (el cuarto
//!                     de segundo o el raton); no despierta al bucle
//!
//! Aqui no se sabe QUE decir ni CUANDO: llega como texto y como fracciones
//! (`desktop::globo` decide, esto solo pinta), por la misma regla que
//! `sugerir`.
//!
//! Se pone como el cursor (`cursor::SaveUnder`): guarda lo que va a tapar al
//! FINAL del fotograma, justo antes de la capa del recorte y del puntero, y lo
//! devuelve al PRINCIPIO del siguiente, despues de quitarlos.
//!
//! ```text
//!     \
//!      +---------------------------------------------+
//!      | LA 3060   49 grados, P0; `gpu` lo muestra   |
//!      | ============-------------------------------- |
//!      +---------------------------------------------+
//! ```

use bmo_userland as bmo;

use super::{acento, BOX_BG, INK_DIM};

/// Letras que caben en la linea.
pub(crate) const LETRAS: usize = 64;
const PAD: u32 = 8;
const ANCHO_MAX: u32 = LETRAS as u32 * bmo::GLIFO_ANCHO + 2 * PAD;
const BARRA: u32 = 3;
const ALTO: u32 = PAD + bmo::GLIFO_ALTO + 6 + BARRA + PAD;
/// Lo que guarda: el globo entero, en su medida mas grande.
const GUARDADO: usize = (ANCHO_MAX * ALTO) as usize;

const TINTA: u32 = 0x00E6_EDF6;

struct Globo {
    px: [u32; GUARDADO],
    caja: (u32, u32, u32, u32),
    puesto: bool,
}

static mut GLOBO: Globo = Globo { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn globo() -> &'static mut Globo {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(GLOBO) }
}

/// **Quita el globo**: devuelve lo que tapaba. Al PRINCIPIO del fotograma,
/// despues del cursor y de la capa del recorte. Si no estaba, no hace nada.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    let g = globo();
    if !g.puesto {
        return;
    }
    let (x, y, w, h) = g.caja;
    p.marcar(x, y, w, h);
    for dy in 0..h {
        for dx in 0..w {
            p.punto_ya_marcado(x + dx, y + dy, g.px[(dy * w + dx) as usize]);
        }
    }
    g.puesto = false;
}

/// Lo que se ve, de un fotograma.
pub(crate) struct Cara<'a> {
    /// En el acento, delante: de QUE habla (`LA 3060`, `sabias`, `atajo`).
    pub titulo: &'a [u8],
    pub texto: &'a [u8],
    /// Letras ya escritas (la animacion de la maquina de escribir).
    pub escritas: usize,
    /// Lo que le queda, en milesimas (1000 al nacer, 0 al morir).
    pub queda: u32,
    /// En los ultimos segundos se apaga: el texto en tenue.
    pub apagandose: bool,
}

/// **Pone el globo** junto al puntero `(ax, ay)`: abajo a la derecha, o donde
/// quepa. Al FINAL del fotograma, antes de la capa del recorte y del cursor.
pub(crate) fn poner(p: &bmo::Pantalla, ax: u32, ay: u32, c: &Cara) {
    let g = globo();
    if g.puesto || p.ancho < 64 || p.alto < 2 * ALTO {
        return;
    }
    let n = (c.titulo.len() + 2 + c.texto.len()).min(LETRAS) as u32;
    let w = (n * bmo::GLIFO_ANCHO + 2 * PAD).min(p.ancho - 2);
    let h = ALTO;
    // Abajo a la derecha del puntero; si no cabe, al otro lado.
    let x = if ax + 14 + w < p.ancho { ax + 14 } else { ax.saturating_sub(w + 6) };
    let y = if ay + 20 + h < p.alto { ay + 20 } else { ay.saturating_sub(h + 6) };
    g.caja = (x, y, w, h);
    p.sincronizar_lectura();
    for dy in 0..h {
        for dx in 0..w {
            g.px[(dy * w + dx) as usize] = p.read(x + dx, y + dy);
        }
    }
    g.puesto = true;

    let ac = acento();
    p.rect(x, y, w, h, BOX_BG);
    // El borde: arriba en el acento, el resto en tenue.
    p.rect(x, y, w, 1, ac);
    p.rect(x, y + h - 1, w, 1, INK_DIM);
    p.rect(x, y, 1, h, ac);
    p.rect(x + w - 1, y, 1, h, INK_DIM);

    // El titulo, entero desde el principio; el texto, letra a letra.
    let ty = y + PAD;
    let mut cx = p.texto_bytes(x + PAD, ty, c.titulo, ac);
    cx += 2 * bmo::GLIFO_ANCHO;
    let cabe = (n as usize).saturating_sub(c.titulo.len() + 2);
    let hasta = c.escritas.min(c.texto.len()).min(cabe);
    let tinta = if c.apagandose { INK_DIM } else { TINTA };
    cx = p.texto_bytes(cx, ty, &c.texto[..hasta], tinta);
    if hasta < c.texto.len().min(cabe) {
        // El cursor de la maquina de escribir.
        p.rect(cx, ty + bmo::GLIFO_ALTO - 3, bmo::GLIFO_ANCHO - 1, 2, ac);
    }

    // La barrita: lo que le queda.
    let by = ty + bmo::GLIFO_ALTO + 6;
    let largo = w - 2 * PAD;
    let lleno = largo * c.queda.min(1000) / 1000;
    p.rect(x + PAD, by, largo, BARRA, INK_DIM);
    p.rect(x + PAD, by, lleno, BARRA, ac);
}
