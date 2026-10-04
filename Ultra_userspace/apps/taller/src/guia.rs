//! **ELEMENTOS and GUIA** -- the two pages of the SPACE tabs that are not the
//! sky: every element, animated, with WHAT it represents and WHY it looks
//! like that; and the guide, in order, with lines (the owner, 04-10: "que
//! tengan solapas para tener ideas de las guias y porque ... con lineas y
//! que sean ordenadas").
//!
//! Each card is drawn by the SAME painter the sky uses (`astros.rs`): what the
//! catalogue shows is what the package will look like, never a second drawing
//! that could drift away from the first.

use crate::aspecto::{self as look, GOLD};
use crate::astros::{self, cable, centaur, double_star, emitter, protoplanet, pulsar, station, supernova, Cable, AMBER, CYAN};
use crate::canvas::Canvas;
use crate::view::{BG, BLUE, DIM, EDGE, GOOD, INK, LEFT, PANEL, TITLE, TOP, VIOLET};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_titan_lector::Traits;

/// One card: its figure, its name, what it is, why it looks like that.
struct Card {
    name: &'static [u8],
    is: &'static [u8],
    why: &'static [u8],
    color: Color,
    draw: fn(&mut Canvas, i32, i32, i32),
}

const fn traits(fns: u8, lets: u8, muts: u8, changes: u8, writes: u8, calls: u8) -> Traits {
    Traits { fns, lets, muts, changes, writes, calls, ifs: 0, lines: 9 }
}

const CARDS: [Card; 13] = [
    Card {
        name: b"EL CENTAURO",
        is: b"el paquete: su Titan.toml",
        why: b"patas BMO-X, torso y arco TITAN++: el que pisa y el que apunta",
        color: VIOLET,
        draw: |c, x, y, t| centaur(c, x, y, 84, t),
    },
    Card {
        name: b"PLANETA",
        is: b"un modulo: un .titan",
        why: b"su color sale de su NOMBRE y su medida de sus fn: dos nunca iguales",
        color: BLUE,
        draw: |c, x, y, t| astros::planet(c, x, y, 20, b"ship", traits(2, 0, 0, 0, 0, 0), t),
    },
    Card {
        name: b"LUNAS",
        is: b"sus let: valores con nombre",
        why: b"quietas en su orbita, como el valor: sin mut no cambia nunca",
        color: INK,
        draw: |c, x, y, t| astros::planet(c, x, y, 16, b"rock", traits(1, 4, 0, 0, 0, 0), t),
    },
    Card {
        name: b"ANILLOS MUT",
        is: b"sus let mut: lo que cambia",
        why: b"ambar y girando: cuanto mas x = ... tiene, mas rapido gira la chispa",
        color: AMBER,
        draw: |c, x, y, t| astros::planet(c, x, y, 16, b"nave", traits(1, 0, 2, 6, 0, 0), t),
    },
    Card {
        name: b"EMISOR",
        is: b"sus print: envia paquetes",
        why: b"suben hacia la consola, la unica puerta de hoy: el certificado la nombra",
        color: GOOD,
        draw: |c, x, y, t| {
            astros::planet(c, x, y + 12, 14, b"main", traits(1, 0, 0, 0, 0, 0), t);
            emitter(c, x, y - 2, 3, t);
        },
    },
    Card {
        name: b"COMETAS",
        is: b"sus llamadas a otras fn",
        why: b"dan la vuelta y vuelven: una llamada sale y regresa a su sitio",
        color: TITLE,
        draw: |c, x, y, t| astros::planet(c, x, y, 14, b"physics", traits(2, 0, 0, 0, 0, 2), t),
    },
    Card {
        name: b"ESTRELLA DOBLE",
        is: b"sus if: decide",
        why: b"dos caminos, uno brilla: el compilador decide cual al compilar y el otro no deja bytes",
        color: GOLD,
        draw: |c, x, y, t| {
            astros::planet(c, x - 14, y, 14, b"semaforo", traits(1, 1, 0, 0, 1, 0), t);
            double_star(c, x + 18, y + 4, 2, t);
        },
    },
    Card {
        name: b"LAZO FUERTE",
        is: b"mod: el padre declara",
        why: b"trenzado y grueso: es el arbol, quien es de quien",
        color: VIOLET,
        draw: |c, x, y, t| cable(c, (x - 44, y - 22), (x + 44, y + 22), Cable::Mod, t, true, 4),
    },
    Card {
        name: b"CONECTOR",
        is: b"use: depende de otro modulo",
        why: b"luz cian: una dependencia SOLO baja; hacia arriba seria un ciclo y no compila",
        color: CYAN,
        draw: |c, x, y, t| cable(c, (x - 44, y - 22), (x + 44, y + 22), Cable::Use, t + 300, true, 4),
    },
    Card {
        name: b"PULSAR",
        is: b"la 3060",
        why: b"lo que se le presta no se toca hasta el wait: el borrow checker lo sabe (U1)",
        color: GOOD,
        draw: |c, x, y, t| pulsar(c, x, y, 20, t),
    },
    Card {
        name: b"ESTACION",
        is: b"el DIRECTOR: el kernel",
        why: b"el segundo juez: cada puerta pide su capability, siempre",
        color: BLUE,
        draw: |c, x, y, t| station(c, x, y, 20, t),
    },
    Card {
        name: b"SUPERNOVA",
        is: b"un modulo en fallo",
        why: b"roja y respirando: el NO de un juez cae en su nodo",
        color: 0x00FF_4D6A,
        draw: |c, x, y, t| {
            supernova(c, x, y, 14, t);
            astros::planet(c, x, y, 12, b"rota", traits(1, 0, 0, 0, 0, 0), t);
        },
    },
    Card {
        name: b"PROTOPLANETA",
        is: b"un modulo sin cuerpo aun",
        why: b"una nube: tiene su cabecera mod, y nada que hacer aun",
        color: DIM,
        draw: |c, x, y, t| protoplanet(c, x, y, 22, BLUE, t),
    },
];

/// Splits a text at spaces into lines of `max` characters, at most `n`.
fn lines(s: &[u8], max: usize, n: usize, mut f: impl FnMut(usize, &[u8])) {
    let mut rest = s;
    let mut k = 0;
    while !rest.is_empty() && k < n {
        let cut = if rest.len() <= max {
            rest.len()
        } else {
            rest[..max].iter().rposition(|&b| b == b' ').filter(|&i| i > 0).unwrap_or(max)
        };
        f(k, &rest[..cut]);
        rest = rest[cut..].strip_prefix(b" ").unwrap_or(&rest[cut..]);
        k += 1;
    }
}

/// ELEMENTOS: the catalogue, in reading order, three columns: each card has
/// its figure ALIVE on the left and what it is and why on the right, so the
/// why has room to be said whole.
pub fn elements(c: &mut Canvas, t: i32) {
    let (x0, y0) = (LEFT + 16, TOP + 40);
    let (cols, gap) = (3, 10);
    let rows = (CARDS.len() as i32 + cols - 1) / cols;
    let w = (c.w - x0 - 16 - gap * (cols - 1)) / cols;
    let h = (c.h - y0 - 12 - gap * (rows - 1)) / rows;
    let fig_w = 112;
    for (k, card) in CARDS.iter().enumerate() {
        let (col, row) = (k as i32 % cols, k as i32 / cols);
        let (x, y) = (x0 + col * (w + gap), y0 + row * (h + gap));
        // A soft card (MAQUETA 2's pieces): night, a thread of its colour.
        look::card(c, x, y, w, h, look::R_CARD, mezclar(BG, look::SEL, 1, 2));
        look::edge(c, x, y, w, h, look::R_CARD, 1, mezclar(card.color, EDGE, 1, 3));
        look::card(c, x + 1, y + look::R_CARD, 3, h - 2 * look::R_CARD, 1, card.color);
        // The figure, alive, on the left; a thin line, and the words.
        (card.draw)(c, x + fig_w / 2 + 4, y + h / 2, t);
        c.rect(x + fig_w, y + 10, 1, h - 20, EDGE);
        let tx = x + fig_w + 10;
        let chars = ((x + w - 10 - tx) / 8) as usize;
        let mut ty = y + 8;
        c.text(tx, ty, card.name, card.color, 1);
        ty += 18;
        lines(card.is, chars, 2, |i, l| {
            c.text(tx, ty + i as i32 * 16, l, INK, 1);
        });
        ty += if card.is.len() > chars { 34 } else { 18 };
        lines(card.why, chars, ((y + h - ty - 2) / 16).max(1) as usize, |i, l| {
            c.text(tx, ty + i as i32 * 16, l, DIM, 1);
        });
    }
}

/// The guide: what F1 is for, in numbered sections, two columns, lines
/// between them.
const GUIDE: [(&[u8], [&[u8]; 4]); 6] = [
    (
        b"1  LA VERDAD ES EL TEXTO",
        [
            b"Lo que se compila es el .titan y el Titan.toml.",
            b"GRAFO, CIELO y ELEMENTOS son maneras de VERLO.",
            b"La LOGICA (lector, juez) no nombra un color; el",
            b"ASPECTO sale de titan.maqueta, con MAQUETA.",
        ],
    ),
    (
        b"2  DEL TEXTO AL ASTRO, EN VIVO",
        [
            b"Guardas un .titan: ESTRATOS sube su generacion,",
            b"F1 la mira en cada latido y relee el paquete.",
            b"Un let mut nuevo: su anillo. Un print: paquetes.",
            b"Un if: su estrella doble, dos caminos, uno brilla.",
        ],
    ),
    (
        b"3  LOS DOS JUECES",
        [
            b"El COMPILADOR juzga dentro, antes, una vez:",
            b"su NO es final. El KERNEL juzga en cada puerta.",
            b"Entre los dos viaja el CERTIFICADO del .bex:",
            b"que puerta usa y desde que linea (titan juez).",
        ],
    ),
    (
        b"4  LOS CABLES Y SUS COLORES",
        [
            b"El color lo da la CLASE, no el gusto: violeta mod,",
            b"cian use, verde la 3060, azul el sistema.",
            b"Asi no hay que pelear por colores: un color",
            b"dice lo mismo en todas las solapas.",
        ],
    ),
    (
        b"5  POR QUE 3D",
        [
            b"El texto es plano; el paquete no. La hondura es",
            b"lo hondo que esta un modulo en el arbol: lo de",
            b"arriba queda cerca, el metal (3060, DIRECTOR)",
            b"queda al fondo. Arrastra el cielo para girarlo.",
        ],
    ),
    (
        b"6  LAS TECLAS",
        [
            b"[t] la solapa siguiente   [e] el siguiente error",
            b"[+] [-] [0] zoom y encuadre   [Esc] sale de F1",
            b"EXPLORER: doble clic renombra, Supr quita,",
            b"clic derecho el menu, arrastrar ordena o mueve.",
        ],
    ),
];

pub fn guide(c: &mut Canvas) {
    let (x0, y0) = (LEFT + 24, TOP + 44);
    let colw = (c.w - x0 - 24) / 2;
    let rowh = (c.h - y0 - 20) / 3;
    for (k, (title, body)) in GUIDE.iter().enumerate() {
        let (col, row) = (k as i32 % 2, k as i32 / 2);
        let (x, y) = (x0 + col * colw, y0 + row * rowh);
        // The number's dot and the section's line: ordered, as asked.
        c.disc(x + 4, y + 7, 4, if k % 2 == 0 { BLUE } else { VIOLET });
        c.text(x + 16, y, title, TITLE, 1);
        c.gradient(x + 16, y + 20, colw - 48, 1, BLUE, VIOLET);
        for (i, l) in body.iter().enumerate() {
            c.text_fit(x + 16, y + 30 + i as i32 * 18, l, if i == 0 { INK } else { DIM }, colw - 40);
        }
        if row < 2 {
            c.rect(x + 16, y + rowh - 14, colw - 48, 1, EDGE);
        }
    }
    c.rect(x0 + colw - 12, y0, 1, c.h - y0 - PANEL / 4, EDGE);
}
