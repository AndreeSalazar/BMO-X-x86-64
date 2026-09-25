//! **LA CAJA ORGANIZADA COMO UN EXPLORADOR** (2026-09-25). Pedido del
//! propietario, con una foto del Explorador de Windows 11 abierto en `datos/`:
//! *"puedes organizar en escritorio asi? ... mi BMO-X no se ve asi el estilo
//! pero puedes mejorar MAS la caja"*.
//!
//! [consumo] NADA      pinta cuando la caja se pinta (abrir, mover, destapar);
//!                     no hay animacion ni reloj aqui (L6h)
//!
//! La ORGANIZACION es la del Explorador; el ESTILO sigue siendo el de BMO-X
//! (el morado, el acento, el cromo hacker de `chrome.rs`):
//!
//! ```text
//!    [# Ejecutar  x] +                                     ==-   _  []
//!    ============================ (el rail de neon del cromo) ===========
//!    <  >  ^  @  | >  la orden o la ruta de un .bex        | (o) Buscar |
//!    [>] Ejecutar  (i) info  [=] disco  ::: ls  [#] gpu ...     (?) ayuda
//!    Tab  sugerencias, o la pista del consejero (como las columnas)
//!    ------------------------------------------------------------------
//!    la salida
//!    ------------------------------------------------------------------
//!    F11 kernel  F12 datos  ESC cierra  |  Ctrl+flechas ...       [=][#]
//! ```
//!
//! Aqui no se sabe QUE hace cada boton: se pinta y se dice cual hay bajo el
//! puntero ([`boton_en`]). Lo que pasa al pulsarlo es de `desktop::caja`.

use bmo_userland as bmo;

use super::globo::mezcla;
use super::{acento, RunBox, BOX_BG, BOX_EDGE, BOX_TITLE, FIELD_BG, INK, INK_DIM, TITLE_H};

/// La banda de navegacion y de ordenes: un poco mas clara que el cuerpo, como
/// la del Explorador sobre su lista.
pub(crate) const BANDA: u32 = 0x0022_1D3E;
/// La barra de navegacion (flechas, direccion, buscador).
pub(crate) const NAV_H: u32 = 40;
/// La barra de ordenes (los botones).
pub(crate) const ORDENES_H: u32 = 36;
/// Todo lo de arriba del cuerpo, desde el borde de la caja.
pub(crate) const ARRIBA: u32 = TITLE_H + NAV_H + ORDENES_H;
/// La barra de estado del pie.
pub(crate) const PIE_H: u32 = 28;
/// El boton de una flecha, y los de la barra de ordenes.
const BOTON: u32 = 30;
const BOTON_H: u32 = 28;
/// La pestana del titulo.
const PESTANA_W: u32 = 150;
/// Por debajo de este ancho, el buscador es solo su lupa.
const ANCHO_BUSCADOR: u32 = 1000;
const BUSCADOR_W: u32 = 200;

/// Un icono de 12 x 12: una fila por `u16`, el bit 11 a la izquierda.
type Icono = [u16; 12];

const FLECHA_IZQ: Icono = [0x000, 0x040, 0x0C0, 0x1C0, 0x3C0, 0x7FE, 0x7FE, 0x3C0, 0x1C0, 0x0C0, 0x040, 0x000];
const FLECHA_DER: Icono = [0x000, 0x020, 0x030, 0x038, 0x03C, 0x7FE, 0x7FE, 0x03C, 0x038, 0x030, 0x020, 0x000];
const FLECHA_ARR: Icono = [0x060, 0x0F0, 0x1F8, 0x3FC, 0x7FE, 0x060, 0x060, 0x060, 0x060, 0x060, 0x060, 0x000];
const REPETIR: Icono = [0x0F8, 0x306, 0x201, 0x400, 0x400, 0x400, 0x40F, 0x406, 0x204, 0x308, 0x0F0, 0x000];
const LUPA: Icono = [0x1E0, 0x210, 0x408, 0x408, 0x408, 0x408, 0x210, 0x1F0, 0x018, 0x00C, 0x006, 0x002];
const CHEVRON: Icono = [0x000, 0x080, 0x0C0, 0x060, 0x030, 0x018, 0x018, 0x030, 0x060, 0x0C0, 0x080, 0x000];
const PLAY: Icono = [0x100, 0x180, 0x1C0, 0x1E0, 0x1F0, 0x1F8, 0x1F8, 0x1F0, 0x1E0, 0x1C0, 0x180, 0x100];
const INFO: Icono = [0x1F8, 0x204, 0x462, 0x402, 0x862, 0x862, 0x862, 0x862, 0x462, 0x402, 0x204, 0x1F8];
const DISCO: Icono = [0x1F8, 0x606, 0x801, 0x606, 0x9F9, 0x801, 0x801, 0x801, 0x80D, 0x801, 0x606, 0x1F8];
const LISTA: Icono = [0x000, 0xDFE, 0xDFE, 0x000, 0x000, 0xDFE, 0xDFE, 0x000, 0x000, 0xDFE, 0xDFE, 0x000];
const CHIP: Icono = [0x248, 0x248, 0xFFF, 0x801, 0xB7D, 0x945, 0x945, 0xB7D, 0x801, 0xFFF, 0x248, 0x248];
const BARRAS: Icono = [0x000, 0x003, 0x003, 0x01B, 0x01B, 0x0DB, 0x0DB, 0x6DB, 0x6DB, 0x6DB, 0xFFF, 0x000];
const DISQUETE: Icono = [0xFFE, 0x8E3, 0x8E1, 0x8E1, 0x801, 0x801, 0xBFD, 0xA05, 0xA05, 0xA05, 0xFFF, 0x000];
const AYUDA: Icono = [0x1F8, 0x204, 0x4F2, 0x49A, 0x81A, 0x839, 0x861, 0x861, 0x401, 0x461, 0x204, 0x1F8];
const VISTA_LISTA: Icono = [0xFFF, 0x801, 0xBFD, 0x801, 0xBFD, 0x801, 0xBFD, 0x801, 0xBFD, 0x801, 0xFFF, 0x000];
const VISTA_CUADROS: Icono = [0xFFF, 0x861, 0x861, 0x861, 0xFFF, 0x861, 0x861, 0x861, 0xFFF, 0x000, 0x000, 0x000];

/// Lo que hay bajo el puntero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Boton {
    Atras,
    Adelante,
    Subir,
    Repetir,
    Buscar,
    /// La orden `k` de [`ORDENES`].
    Orden(usize),
}

/// **Los botones de la barra de ordenes**: `(icono, lo que dice, la orden)`.
/// La primera ejecuta lo que haya en el campo (orden vacia). La ultima va a
/// la derecha, como el "Detalles" del Explorador.
pub(crate) const ORDENES: [(&Icono, &str, &[u8]); 8] = [
    (&PLAY, "Ejecutar", b""),
    (&INFO, "info", b"info"),
    (&DISCO, "disco", b"disco"),
    (&LISTA, "ls", b"ls"),
    (&CHIP, "gpu", b"gpu"),
    (&BARRAS, "consumo", b"consumo"),
    (&DISQUETE, "save", b"save"),
    (&AYUDA, "ayuda", b"ayuda"),
];

fn icono(p: &bmo::Pantalla, x: u32, y: u32, ic: &Icono, color: u32) {
    for (fila, &bits) in ic.iter().enumerate() {
        let mut col = 0u32;
        while col < 12 {
            if bits & (0x800 >> col) != 0 {
                // Los tramos seguidos, de un rect: menos llamadas que puntos.
                let desde = col;
                while col < 12 && bits & (0x800 >> col) != 0 {
                    col += 1;
                }
                p.rect(x + desde, y + fila as u32, col - desde, 1, color);
            } else {
                col += 1;
            }
        }
    }
}

/// Un rectangulo con las cuatro esquinas biseladas un pixel.
fn redondo(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), color: u32, fuera: u32) {
    p.rect(x, y, w, h, color);
    for &(cx, cy) in &[(x, y), (x + w - 1, y), (x, y + h - 1), (x + w - 1, y + h - 1)] {
        p.rect(cx, cy, 1, 1, fuera);
    }
}

// -- La geometria: UNA cuenta, la usan el pintor y el puntero -----------------

/// La barra de direccion: el campo de siempre (`RunBox::field_*`).
fn buscador(c: &RunBox) -> (u32, u32, u32, u32) {
    let w = if c.w() >= ANCHO_BUSCADOR { BUSCADOR_W } else { BOTON };
    (c.x + c.w() - 12 - w, c.y + TITLE_H + 6, w, BOTON_H)
}

/// **Donde empieza el campo y cuanto mide** (lo usa `RunBox::relayout`):
/// detras de las cuatro flechas y antes del buscador.
pub(crate) fn campo(x: u32, y: u32, w: u32) -> (u32, u32, u32, u32) {
    let bw = if w >= ANCHO_BUSCADOR { BUSCADOR_W } else { BOTON };
    let fx = x + 12 + 4 * BOTON + 10;
    let fin = x + w - 12 - bw - 10;
    (fx, y + TITLE_H + 6, fin.saturating_sub(fx), BOTON_H)
}

fn flecha(c: &RunBox, k: u32) -> (u32, u32, u32, u32) {
    (c.x + 12 + k * BOTON, c.y + TITLE_H + 6, BOTON - 2, BOTON_H)
}

/// Donde cae el boton de orden `k`, si cabe: `(x, y, w, h)`.
fn orden_caja(c: &RunBox, k: usize) -> Option<(u32, u32, u32, u32)> {
    let y = c.y + TITLE_H + NAV_H + 4;
    let ancho = |i: usize| 12 + 16 + ORDENES[i].1.len() as u32 * bmo::GLIFO_ANCHO + 10;
    let ultima = ORDENES.len() - 1;
    let fin = c.x + c.w() - 12;
    if k == ultima {
        // A la derecha, como el "Detalles" del Explorador.
        let w = ancho(k);
        return Some((fin - w, y, w, BOTON_H));
    }
    let mut x = c.x + 12;
    for i in 0..k {
        x += ancho(i) + if i == 0 { 13 } else { 2 };
    }
    let w = ancho(k);
    // Lo que no cabe antes del de la derecha, no se pinta (ni se pulsa).
    (x + w + 12 <= fin - ancho(ultima)).then_some((x, y, w, BOTON_H))
}

fn dentro(x: u32, y: u32, (bx, by, bw, bh): (u32, u32, u32, u32)) -> bool {
    x >= bx && x < bx + bw && y >= by && y < by + bh
}

/// **El boton bajo el puntero**, si lo hay.
pub(crate) fn boton_en(c: &RunBox, x: u32, y: u32) -> Option<Boton> {
    const FLECHAS: [Boton; 4] = [Boton::Atras, Boton::Adelante, Boton::Subir, Boton::Repetir];
    for (k, &b) in FLECHAS.iter().enumerate() {
        if dentro(x, y, flecha(c, k as u32)) {
            return Some(b);
        }
    }
    if dentro(x, y, buscador(c)) {
        return Some(Boton::Buscar);
    }
    (0..ORDENES.len()).find(|&k| orden_caja(c, k).map_or(false, |b| dentro(x, y, b))).map(Boton::Orden)
}

/// **El modelo** (para `scene_color`): el color de fondo de un pixel de la
/// caja que cae en lo de este fichero, o `None` si no es suyo.
pub(crate) fn color_en(c: &RunBox, x: u32, y: u32) -> Option<u32> {
    if y < c.y + TITLE_H {
        let pestana = x >= c.x + 10 && x < c.x + 10 + PESTANA_W && y >= c.y + 5 && y < c.y + TITLE_H - 1;
        return pestana.then_some(BANDA);
    }
    if y < c.y + ARRIBA {
        if dentro(x, y, buscador(c)) {
            return Some(FIELD_BG);
        }
        return Some(BANDA);
    }
    None
}

// -- El pintor ----------------------------------------------------------------

/// **Todo lo que no es el marco, el campo ni la salida**: la pestana, las dos
/// bandas, los botones, la cabecera y el pie. Lo llama `paint_run_box`.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &RunBox) {
    let a = acento();
    // La pestana: se funde con la banda de abajo, sobre el rail de neon.
    let t = (c.x + 10, c.y + 5, PESTANA_W, TITLE_H - 6);
    redondo(p, t, BANDA, BOX_TITLE);
    p.rect(t.0 + 10, c.y + 12, 8, 8, a);
    p.texto(t.0 + 26, c.y + 9, "Ejecutar", INK);
    p.texto(t.0 + PESTANA_W - 18, c.y + 9, "x", INK_DIM);
    p.texto(t.0 + PESTANA_W + 10, c.y + 9, "+", INK_DIM);

    // Las dos bandas, y la raya que las separa del cuerpo.
    p.rect(c.x + 1, c.y + TITLE_H, c.w() - 2, NAV_H + ORDENES_H, BANDA);
    let raya = mezcla(BANDA, BOX_EDGE, 200);
    p.rect(c.x + 12, c.y + TITLE_H + NAV_H, c.w() - 24, 1, mezcla(BANDA, BOX_EDGE, 110));
    p.rect(c.x + 1, c.y + ARRIBA, c.w() - 2, 1, raya);

    // Las flechas: atras, adelante (el historial), subir (la salida) y repetir.
    let flechas: [&Icono; 4] = [&FLECHA_IZQ, &FLECHA_DER, &FLECHA_ARR, &REPETIR];
    for (k, ic) in flechas.iter().enumerate() {
        let (bx, by, bw, bh) = flecha(c, k as u32);
        icono(p, bx + (bw - 12) / 2, by + (bh - 12) / 2, ic, if k == 3 { a } else { INK });
    }

    // El buscador: la lupa, y su texto si hay sitio (Ctrl+F busca en la salida).
    let b = buscador(c);
    redondo(p, b, FIELD_BG, BANDA);
    if b.2 > BOTON {
        p.texto(b.0 + 10, b.1 + 6, "Buscar  (Ctrl+F)", INK_DIM);
        icono(p, b.0 + b.2 - 20, b.1 + 8, &LUPA, INK_DIM);
    } else {
        icono(p, b.0 + (b.2 - 12) / 2, b.1 + 8, &LUPA, INK_DIM);
    }

    // Los botones de orden, y los separadores del Explorador.
    for k in 0..ORDENES.len() {
        let Some((bx, by, bw, _)) = orden_caja(c, k) else { continue };
        let (ic, texto, _) = ORDENES[k];
        let color = if k == 0 { a } else { mezcla(INK, a, 90) };
        icono(p, bx + 10, by + 8, ic, color);
        p.texto(bx + 10 + 18, by + 6, texto, if k == 0 { INK } else { INK });
        if k == 0 {
            // El separador tras "Ejecutar", como el del Explorador tras "Nuevo".
            p.rect(bx + bw + 6, by + 5, 1, BOTON_H - 10, mezcla(BANDA, BOX_EDGE, 200));
        }
    }

    // La cabecera: una raya fina bajo la linea de sugerencias, como la de las
    // columnas de la lista.
    p.rect(c.x + 12, c.status_y + bmo::GLIFO_ALTO + 6, c.w() - 24, 1, mezcla(BOX_BG, BOX_EDGE, 160));

    // El pie: la barra de estado.
    let py = c.y + c.h() - PIE_H;
    p.rect(c.x + 1, py, c.w() - 2, 1, mezcla(BOX_BG, BOX_EDGE, 160));
    p.texto(c.x + 16, py + 7, "F11 kernel   F12 datos   ESC cierra", INK_DIM);
    let medio = c.x + 16 + 38 * bmo::GLIFO_ANCHO;
    let derecha = c.x + c.w() - 16 - 2 * 18;
    let atajos = "|   Ctrl+flechas encaja   Ctrl+Q cierra   Ctrl+Alt esconde";
    if medio + atajos.len() as u32 * bmo::GLIFO_ANCHO + 12 <= derecha {
        p.texto(medio, py + 7, atajos, INK_DIM);
    }
    icono(p, derecha, py + 8, &VISTA_LISTA, a);
    icono(p, derecha + 18, py + 8, &VISTA_CUADROS, INK_DIM);
}

/// **El campo vacio**: la direccion, como el Explorador cuando no se escribe
/// nada -- donde se esta, y que se puede teclear. Lo pinta `paint_field`.
pub(crate) fn direccion(p: &bmo::Pantalla, c: &RunBox) {
    // Detras del cursor (que va en `texto_x`), no debajo.
    icono(p, c.texto_x + 6, c.field_y + 8, &CHEVRON, INK_DIM);
    let x = p.texto(c.texto_x + 22, c.texto_y, "BMO-X", INK_DIM);
    let x = p.texto(x, c.texto_y, "  >  ", mezcla(INK_DIM, BOX_BG, 90));
    let x = p.texto(x, c.texto_y, "Ejecutar", INK_DIM);
    let pista = "     una orden, o la ruta de un .bex y Enter";
    if x + pista.len() as u32 * bmo::GLIFO_ANCHO <= c.field_x + c.field_w - 8 {
        p.texto(x, c.texto_y, pista, mezcla(INK_DIM, FIELD_BG, 110));
    }
}
