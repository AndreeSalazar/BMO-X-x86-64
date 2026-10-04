//! **El idioma de los trazos**, leido: de `"L 10 0 10 -70 ; O 38 -35 28 35"`
//! a segmentos en 1/16 de centesima de eme (ver `glifos.rs`).

use crate::pila::Pila;

/// Los trozos que caben en una letra: la `@` (tres arcos y un palo) usa ~130.
pub const MAX_TROZOS: usize = 320;

pub type Trozos = Pila<Trozo, MAX_TROZOS>;

/// Un segmento de pluma, o un punto (`a == b`, con la pluma algo mas gorda).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Trozo {
    pub a: (i32, i32),
    pub b: (i32, i32),
    pub punto: bool,
}

/// El seno de 0 a 90 grados, en Q15.
const SENO: [i32; 91] = [
    0, 572, 1144, 1715, 2286, 2856, 3425, 3993, 4560, 5126, 5690, 6252, 6813, 7371, 7927, 8481, 9032, 9580, 10126, 10668, 11207, 11743, 12275,
    12803, 13328, 13848, 14365, 14876, 15384, 15886, 16384, 16877, 17364, 17847, 18324, 18795, 19261, 19720, 20174, 20622, 21063, 21498, 21926,
    22348, 22763, 23170, 23571, 23965, 24351, 24730, 25102, 25466, 25822, 26170, 26510, 26842, 27166, 27482, 27789, 28088, 28378, 28660, 28932,
    29197, 29452, 29698, 29935, 30163, 30382, 30592, 30792, 30983, 31164, 31336, 31499, 31651, 31795, 31928, 32052, 32166, 32270, 32365, 32449,
    32524, 32588, 32643, 32688, 32723, 32748, 32763, 32768,
];

/// El seno de `a` grados (cualquiera, tambien negativos), en Q15.
pub fn seno(a: i32) -> i32 {
    let a = a.rem_euclid(360);
    match a {
        0..=90 => SENO[a as usize],
        91..=180 => SENO[(180 - a) as usize],
        181..=270 => -SENO[(a - 180) as usize],
        _ => -SENO[(360 - a) as usize],
    }
}

pub fn coseno(a: i32) -> i32 {
    seno(a + 90)
}

/// Los numeros de una orden, en 1/16.
fn numeros<'a>(it: &mut core::iter::Peekable<impl Iterator<Item = &'a str>>) -> Pila<i32, 64> {
    let mut v = Pila::nueva(0);
    while let Some(t) = it.peek() {
        match t.parse::<i32>() {
            Ok(n) => {
                v.push(n * 16);
                it.next();
            }
            Err(_) => break,
        }
    }
    v
}

/// Los puntos de un arco (todo en 1/16): un segmento cada ~3 centesimas.
fn arco(cx: i32, cy: i32, rx: i32, ry: i32, a0: i32, a1: i32, out: &mut Trozos) {
    let largo = (a1 - a0).abs() * rx.max(ry) / 16;
    let pasos = (largo / 170).clamp(4, 120);
    let punto = |a: i32| (cx + rx * coseno(a) / 32768, cy - ry * seno(a) / 32768);
    let mut antes = punto(a0);
    for k in 1..=pasos {
        let p = punto(a0 + (a1 - a0) * k / pasos);
        out.push(Trozo { a: antes, b: p, punto: false });
        antes = p;
    }
}

/// **Lee** unos trazos y los deja en `out`.
pub fn leer(out: &mut Trozos, s: &str) {
    leer_en(out, s, 0, 0)
}

/// Lee unos trazos corridos `(dx, dy)` centesimas: los acentos se escriben
/// alrededor de (0, 0) y se ponen donde caen.
pub fn leer_en(out: &mut Trozos, s: &str, dx: i32, dy: i32) {
    let (dx, dy) = (dx * 16, dy * 16);
    let mut it = s.split_ascii_whitespace().peekable();
    while let Some(orden) = it.next() {
        let n = numeros(&mut it);
        let n = n.as_slice();
        match orden {
            "L" => {
                for k in (2..n.len().saturating_sub(1)).step_by(2) {
                    out.push(Trozo { a: (n[k - 2] + dx, n[k - 1] + dy), b: (n[k] + dx, n[k + 1] + dy), punto: false });
                }
            }
            "A" if n.len() == 6 => arco(n[0] + dx, n[1] + dy, n[2], n[3], n[4] / 16, n[5] / 16, out),
            "O" if n.len() == 4 => arco(n[0] + dx, n[1] + dy, n[2], n[3], 0, 360, out),
            "P" if n.len() == 2 => out.push(Trozo { a: (n[0] + dx, n[1] + dy), b: (n[0] + dx, n[1] + dy), punto: true }),
            _ => {}
        }
    }
}

/// Si unos trazos estan bien escritos: cada orden con los numeros que pide.
#[cfg(test)]
pub fn valido(s: &str) -> Result<(), alloc::string::String> {
    let mut it = s.split_ascii_whitespace().peekable();
    while let Some(orden) = it.next() {
        let n = numeros(&mut it).as_slice().len();
        let bien = match orden {
            "L" => n >= 4 && n % 2 == 0,
            "A" => n == 6,
            "O" => n == 4,
            "P" => n == 2,
            ";" => n == 0,
            _ => false,
        };
        if !bien {
            return Err(alloc::format!("`{orden}` con {n} numeros en \"{s}\""));
        }
    }
    Ok(())
}
