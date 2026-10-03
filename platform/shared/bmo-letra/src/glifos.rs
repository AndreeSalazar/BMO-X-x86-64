//! **LOS GLIFOS**: cada letra, escrita a mano como TRAZOS de pluma redonda.
//!
//! Nada viene de una fuente de otro: es arte de la casa, como el de `fontgen`,
//! solo que en vez de pixeles son lineas y arcos, y por eso sirve en
//! cualquier talla. La inspiracion es la letra de las maquetas (una sans
//! geometrica, de las de pantalla), no sus contornos.
//!
//! ## Las medidas, en centesimas de eme
//!
//! ```text
//!    y = -74   ascendente (b d f h k l)
//!    y = -70   mayuscula y cifra
//!    y = -52   altura de la x
//!    y =   0   la linea base
//!    y = +20   descendente (g j p q y)
//! ```
//!
//! `y` crece hacia ABAJO. Las cotas son de la LINEA CENTRAL del trazo: la
//! tinta sobresale media pluma por cada lado, y el rasterizador encaja esas
//! cinco alturas en la rejilla de pixeles (`raster::Alturas`).
//!
//! ## El idioma de los trazos
//!
//! ```text
//!    L x y x y ...          una polilinea
//!    A cx cy rx ry a0 a1    un arco de elipse, de a0 a a1 grados
//!                           (0 = derecha, 90 = ARRIBA, como en el colegio)
//!    O cx cy rx ry          una elipse entera
//!    P x y                  un punto (el de la i, el final de la frase)
//!    ;                      separa, solo para leerlo
//! ```

/// Un glifo: lo que avanza la pluma (en centesimas de eme) y sus trazos.
#[derive(Clone, Copy)]
pub struct Glifo {
    pub avance: i32,
    pub trazos: &'static str,
}

const fn g(avance: i32, trazos: &'static str) -> Option<Glifo> {
    Some(Glifo { avance, trazos })
}

/// **El glifo de un byte Latin-1**, si es de los que se dibujan enteros.
/// Las letras con acento salen de [`compuesta`].
pub fn glifo(c: u8) -> Option<Glifo> {
    match c {
        b' ' => g(26, ""),
        b'!' => g(26, "L 13 -70 13 -20 ; P 13 -3"),
        b'"' => g(34, "L 11 -74 11 -56 ; L 23 -74 23 -56"),
        b'#' => g(62, "L 22 -6 28 -64 ; L 38 -6 44 -64 ; L 10 -46 54 -46 ; L 8 -24 52 -24"),
        b'$' => g(60, "A 30 -50 17 15 30 270 ; A 30 -20 19 15 90 -150 ; L 30 -78 30 8"),
        b'%' => g(72, "O 17 -55 10 14 ; O 55 -15 10 14 ; L 56 -70 16 0"),
        b'&' => g(64, "A 27 -55 11 14 -40 220 ; L 19 -46 58 0 ; L 35 -46 13 -26 ; A 27 -17 16 17 150 330"),
        b'\'' => g(22, "L 11 -74 11 -56"),
        b'(' => g(34, "A 38 -30 24 46 116 244"),
        b')' => g(34, "A -4 -30 24 46 64 -64"),
        b'*' => g(46, "L 23 -72 23 -48 ; L 12 -66 34 -54 ; L 12 -54 34 -66"),
        b'+' => g(58, "L 10 -30 48 -30 ; L 29 -49 29 -11"),
        b',' => g(24, "L 13 -6 8 12"),
        b'-' => g(40, "L 9 -27 31 -27"),
        b'.' => g(24, "P 12 -3"),
        b'/' => g(44, "L 8 12 36 -74"),
        b'0' => g(58, "O 29 -35 20 35"),
        b'1' => g(58, "L 14 -56 32 -70 32 0"),
        b'2' => g(58, "A 29 -51 19 19 160 -38 ; L 44 -39 10 0 49 0"),
        b'3' => g(58, "A 28 -53 17 17 150 -90 ; A 29 -18 19 18 90 -150"),
        b'4' => g(58, "L 40 0 40 -70 8 -20 52 -20"),
        b'5' => g(58, "L 46 -70 16 -70 13 -37 ; A 29 -22 19 22 128 -150"),
        b'6' => g(58, "O 29 -22 19 22 ; L 42 -70 13 -30"),
        b'7' => g(58, "L 9 -70 49 -70 22 0"),
        b'8' => g(58, "O 29 -53 16 17 ; O 29 -18 19 18"),
        b'9' => g(58, "O 29 -48 19 22 ; L 16 0 45 -40"),
        b':' => g(24, "P 12 -3 ; P 12 -45"),
        b';' => g(24, "P 12 -45 ; L 13 -6 8 12"),
        b'<' => g(56, "L 46 -50 10 -30 46 -10"),
        b'=' => g(56, "L 10 -38 46 -38 ; L 10 -22 46 -22"),
        b'>' => g(56, "L 10 -50 46 -30 10 -10"),
        b'?' => g(52, "A 26 -52 17 18 160 -50 ; L 37 -38 27 -28 27 -20 ; P 27 -3"),
        b'@' => g(88, "O 41 -31 12 14 ; L 53 -46 53 -24 ; A 62 -24 9 10 180 340 ; A 43 -31 33 38 -15 300"),
        b'A' => g(64, "L 6 0 32 -70 58 0 ; L 15 -23 49 -23"),
        b'B' => g(62, "L 10 0 10 -70 32 -70 ; A 32 -53 16 17 90 -90 ; L 10 -36 34 -36 ; A 34 -18 18 18 90 -90 ; L 34 0 10 0"),
        b'C' => g(66, "A 38 -35 28 35 48 312"),
        b'D' => g(68, "L 10 0 10 -70 30 -70 ; A 30 -35 28 35 90 -90 ; L 30 0 10 0"),
        b'E' => g(56, "L 46 -70 10 -70 10 0 46 0 ; L 10 -36 41 -36"),
        b'F' => g(54, "L 46 -70 10 -70 10 0 ; L 10 -36 41 -36"),
        b'G' => g(70, "A 38 -35 28 35 48 360 ; L 42 -35 66 -35"),
        b'H' => g(64, "L 10 0 10 -70 ; L 54 0 54 -70 ; L 10 -36 54 -36"),
        b'I' => g(24, "L 12 0 12 -70"),
        b'J' => g(50, "L 40 -70 40 -20 ; A 24 -20 16 20 0 -170"),
        b'K' => g(60, "L 10 0 10 -70 ; L 52 -70 10 -26 ; L 24 -40 54 0"),
        b'L' => g(50, "L 10 -70 10 0 46 0"),
        b'M' => g(78, "L 10 0 10 -70 39 -16 68 -70 68 0"),
        b'N' => g(66, "L 10 0 10 -70 56 0 56 -70"),
        b'O' => g(76, "O 38 -35 28 35"),
        b'P' => g(60, "L 10 0 10 -70 32 -70 ; A 32 -51 19 19 90 -90 ; L 32 -32 10 -32"),
        b'Q' => g(76, "O 38 -35 28 35 ; L 44 -16 66 6"),
        b'R' => g(62, "L 10 0 10 -70 32 -70 ; A 32 -51 19 19 90 -90 ; L 32 -32 10 -32 ; L 32 -32 54 0"),
        b'S' => g(62, "A 31 -52 19 18 25 270 ; A 31 -17 21 17 90 -155"),
        b'T' => g(58, "L 4 -70 54 -70 ; L 29 -70 29 0"),
        b'U' => g(66, "L 10 -70 10 -24 ; A 33 -24 23 24 180 360 ; L 56 -24 56 -70"),
        b'V' => g(62, "L 4 -70 31 0 58 -70"),
        b'W' => g(86, "L 4 -70 22 0 43 -62 64 0 82 -70"),
        b'X' => g(60, "L 6 -70 54 0 ; L 54 -70 6 0"),
        b'Y' => g(60, "L 4 -70 30 -33 56 -70 ; L 30 -33 30 0"),
        b'Z' => g(60, "L 8 -70 52 -70 8 0 53 0"),
        b'[' => g(32, "L 26 -74 13 -74 13 14 26 14"),
        b'\\' => g(44, "L 8 -74 36 12"),
        b']' => g(32, "L 6 -74 19 -74 19 14 6 14"),
        b'^' => g(52, "L 10 -48 26 -70 42 -48"),
        b'_' => g(50, "L 2 14 48 14"),
        b'`' => g(30, "L 10 -74 20 -62"),
        b'a' => g(56, "O 26 -26 19 26 ; L 45 -52 45 0"),
        b'b' => g(58, "L 10 -74 10 0 ; O 30 -26 20 26"),
        b'c' => g(52, "A 29 -26 20 26 50 310"),
        b'd' => g(58, "O 28 -26 20 26 ; L 48 -74 48 0"),
        b'e' => g(56, "L 9 -26 48 -26 ; A 28 -26 20 26 0 315"),
        b'f' => g(36, "L 16 0 16 -60 ; A 28 -60 12 12 180 50 ; L 5 -51 31 -51"),
        b'g' => g(58, "O 28 -26 20 26 ; L 48 -52 48 4 ; A 28 4 20 16 0 -150"),
        b'h' => g(60, "L 10 -74 10 0 ; A 30 -32 20 20 180 0 ; L 50 -32 50 0"),
        b'i' => g(22, "L 11 -52 11 0 ; P 11 -68"),
        b'j' => g(24, "L 13 -52 13 6 ; A 1 6 12 13 0 -100 ; P 13 -68"),
        b'k' => g(52, "L 10 -74 10 0 ; L 45 -52 10 -18 ; L 22 -30 47 0"),
        b'l' => g(22, "L 11 -74 11 0"),
        b'm' => g(86, "L 10 -52 10 0 ; A 26 -34 16 18 180 0 ; L 42 -34 42 0 ; A 58 -34 16 18 180 0 ; L 74 -34 74 0"),
        b'n' => g(60, "L 10 -52 10 0 ; A 30 -32 20 20 180 0 ; L 50 -32 50 0"),
        b'o' => g(56, "O 28 -26 20 26"),
        b'p' => g(58, "L 10 -52 10 20 ; O 30 -26 20 26"),
        b'q' => g(58, "O 28 -26 20 26 ; L 48 -52 48 20"),
        b'r' => g(38, "L 10 -52 10 0 ; A 27 -34 17 18 180 65"),
        b's' => g(50, "A 25 -39 15 13 25 270 ; A 25 -13 16 13 90 -155"),
        b't' => g(38, "L 17 -66 17 -10 ; A 29 -10 12 10 180 290 ; L 5 -52 33 -52"),
        b'u' => g(60, "L 10 -52 10 -20 ; A 30 -20 20 20 180 360 ; L 50 -52 50 0"),
        b'v' => g(52, "L 5 -52 26 0 47 -52"),
        b'w' => g(76, "L 4 -52 19 0 38 -46 57 0 72 -52"),
        b'x' => g(52, "L 7 -52 45 0 ; L 45 -52 7 0"),
        b'y' => g(52, "L 5 -52 27 0 ; L 47 -52 19 20"),
        b'z' => g(50, "L 8 -52 42 -52 8 0 43 0"),
        b'{' => g(36, "L 28 -74 23 -74 18 -69 18 -36 12 -30 18 -24 18 9 23 14 28 14"),
        b'|' => g(24, "L 12 -76 12 16"),
        b'}' => g(36, "L 8 -74 13 -74 18 -69 18 -36 24 -30 18 -24 18 9 13 14 8 14"),
        b'~' => g(54, "A 17 -30 9 7 180 0 ; A 35 -30 9 7 180 360"),
        // -- Latin-1: lo que el castellano pide ----------------------------
        0xA1 => g(26, "P 13 -49 ; L 13 -32 13 18"),
        0xBF => g(52, "A 26 -2 17 18 340 130 ; L 15 -16 25 -26 25 -34 ; P 25 -51"),
        0xAA => g(36, "O 17 -55 9 11 ; L 27 -66 27 -44 ; L 8 -32 28 -32"),
        0xBA => g(36, "O 18 -55 10 11 ; L 8 -32 28 -32"),
        0xB0 => g(32, "O 16 -60 8 9"),
        0xB7 => g(22, "P 11 -28"),
        0xAC => g(56, "L 10 -38 46 -38 46 -24"),
        0xB4 => g(30, "L 11 -62 21 -76"),
        0xA8 => g(34, "P 10 -68 ; P 24 -68"),
        0xD7 => g(56, "L 13 -45 43 -15 ; L 43 -45 13 -15"),
        // La i SIN punto: la base de la i con acento.
        0x01 => g(22, "L 11 -52 11 0"),
        _ => None,
    }
}

/// Los acentos que se ponen encima (o debajo) de una letra.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Acento {
    Agudo,
    Grave,
    Tilde,
    Dieresis,
    Cedilla,
}

/// **Una letra con acento**: su base y su acento. la i con acento (0xED) lleva la i sin punto.
pub fn compuesta(c: u8) -> Option<(u8, Acento)> {
    use Acento::*;
    Some(match c {
        0xE1 => (b'a', Agudo),
        0xE9 => (b'e', Agudo),
        0xED => (0x01, Agudo),
        0xF3 => (b'o', Agudo),
        0xFA => (b'u', Agudo),
        0xC1 => (b'A', Agudo),
        0xC9 => (b'E', Agudo),
        0xCD => (b'I', Agudo),
        0xD3 => (b'O', Agudo),
        0xDA => (b'U', Agudo),
        0xE0 => (b'a', Grave),
        0xE8 => (b'e', Grave),
        0xEC => (0x01, Grave),
        0xF2 => (b'o', Grave),
        0xF9 => (b'u', Grave),
        0xF1 => (b'n', Tilde),
        0xD1 => (b'N', Tilde),
        0xFC => (b'u', Dieresis),
        0xDC => (b'U', Dieresis),
        0xEF => (0x01, Dieresis),
        0xE7 => (b'c', Cedilla),
        0xC7 => (b'C', Cedilla),
        _ => return None,
    })
}

/// El centro de la letra donde cae el acento (la `a` lo lleva sobre su
/// panza, no sobre el palo).
pub fn centro(base: u8, avance: i32) -> i32 {
    match base {
        b'a' => 26,
        _ => avance / 2,
    }
}
