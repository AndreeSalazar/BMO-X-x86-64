//! **LA RED DE ESPACIO PROFUNDO** -- el instrumento de HERMES con el escritorio
//! de mision (HM6b de `docs/plan/PLAN_EL_HUD.md`, 07-10): *"un amigo, una
//! antena"*.
//!
//! [consumo] NADA      se pinta con la ventana; las tres preguntas al kernel
//!                     son de las que no cambian nada (L6h)
//!
//! ```text
//!    ESPACIO PROFUNDO                               F3         ( ( (
//!    LA ANTENA        1.000 Mbit/s                             \__/
//!    TRAMAS OIDAS     12.408                                    ||
//!    AMIGOS EN ESCUCHA   0    un amigo, una antena: llega con H6
//! ```
//!
//! ** La ANTENA es la tarjeta de red de ESTA maquina, como el kernel la ve
//! (`INFO_NET_PRESENTE`, `INFO_NET_MEGABITS`, `INFO_NET_RX_TRAMAS`). Los
//! amigos son claves aceptadas, y hoy no hay ninguna (la huella llega con
//! H6): el instrumento dice cero, no una red de mentira.

use bmo_userland as bmo;

use crate::canvas::Canvas;
use crate::mision::{lectura, marco, miles, CUIDADO, GO, NOGO, OJO, TENUE, TINTA};
use crate::piezas::{arco, texto, trazo, Estilo};

/// Lo que mide el instrumento.
pub const ALTO: i32 = 196;

/// **Pinta la red** en `(x, y)`, `w` de ancho. `ms` mueve las ondas, y solo
/// cuando la antena tiene cable.
pub fn pintar(cv: &mut Canvas, x: i32, y: i32, w: i32, ms: u32) {
    let hay = bmo::info(bmo::INFO_NET_PRESENTE) != 0;
    let mbit = if hay { bmo::info(bmo::INFO_NET_MEGABITS) } else { 0 };
    let tramas = if hay { bmo::info(bmo::INFO_NET_RX_TRAMAS) } else { 0 };
    pintar_con(cv, x, y, w, ms, hay, mbit, tramas);
}

/// La pintura con lo medido (el banco del anfitrion la llama asi).
#[allow(clippy::too_many_arguments)]
pub fn pintar_con(cv: &mut Canvas, x: i32, y: i32, w: i32, ms: u32, hay: bool, mbit: u64, tramas: u64) {
    marco(cv, x, y, w, ALTO, b"ESPACIO PROFUNDO", b"F3");
    let tinta = if !hay {
        NOGO
    } else if mbit == 0 {
        CUIDADO
    } else {
        GO
    };
    // El plato, arriba a la derecha: el cuenco, el mastil y, con cable, las
    // ondas que salen (una se enciende cada medio segundo).
    let (cx, cy) = (x + w - 44, y + 70);
    arco(cv, cx, cy, 20, 32, 160, tinta);
    trazo(cv, (cx, cy), (cx + 10, cy - 10), tinta);
    trazo(cv, (cx, cy + 18), (cx, cy + 34), TENUE);
    trazo(cv, (cx - 10, cy + 34), (cx + 10, cy + 34), TENUE);
    for k in 0..3 {
        let viva = mbit > 0 && (ms / 500) % 3 == k as u32;
        let c = if mbit == 0 { TENUE } else if viva { OJO } else { tinta };
        arco(cv, cx + 10, cy - 10, 8 + k * 6, -56, -8, c);
    }
    let mut b = [0u8; 24];
    if !hay {
        lectura(cv, x + 16, y + 34, b"LA ANTENA", b"--", b"sin tarjeta", tinta);
    } else if mbit == 0 {
        lectura(cv, x + 16, y + 34, b"LA ANTENA", b"--", b"sin cable", tinta);
    } else {
        let n = miles(mbit, &mut b);
        lectura(cv, x + 16, y + 34, b"LA ANTENA", &b[..n], b"Mbit/s", tinta);
    }
    let n = miles(tramas, &mut b);
    lectura(cv, x + 16, y + 86, b"TRAMAS OIDAS", &b[..n], b"", if tramas > 0 { TINTA } else { TENUE });
    let rotulo = Estilo::normal(10).espaciado(140).mayusculas();
    texto(cv, x + 16, y + 140, 14, b"AMIGOS EN ESCUCHA", TENUE, rotulo);
    let ancho = texto(cv, x + 16, y + 156, 22, b"0", TINTA, Estilo::media(18));
    texto(cv, x + 16 + ancho + 10, y + 160, 16, b"un amigo, una antena: H6", CUIDADO, Estilo::normal(11));
}
