//! **Las cuentas de las animaciones**, en enteros: `no_std` no trae `sin`,
//! y una tabla de un cuarto de vuelta basta para mover iconos.

/// Un cuarto de seno en 64 pasos, por 256.
const CUARTO: [i32; 65] = [
    0, 6, 13, 19, 25, 31, 38, 44, 50, 56, 62, 68, 74, 80, 86, 92, 98, 104, 109, 115, 121, 126, 132, 137, 142, 147, 152, 157, 162,
    167, 172, 177, 181, 185, 190, 194, 198, 202, 206, 209, 213, 216, 220, 223, 226, 229, 231, 234, 237, 239, 241, 243, 245, 247,
    248, 250, 251, 252, 253, 254, 255, 255, 256, 256, 256,
];

/// El seno de `a` (una vuelta son 256), por 256.
pub fn seno(a: i32) -> i32 {
    let a = a.rem_euclid(256);
    match a / 64 {
        0 => CUARTO[(a % 64) as usize],
        1 => CUARTO[(64 - a % 64) as usize],
        2 => -CUARTO[(a % 64) as usize],
        _ => -CUARTO[(64 - a % 64) as usize],
    }
}

pub fn coseno(a: i32) -> i32 {
    seno(a + 64)
}

/// Donde va `ms` dentro de un ciclo de `periodo` ms, de 0 a 256.
pub fn fase(ms: u32, periodo: u32) -> i32 {
    ((ms % periodo.max(1)) as u64 * 256 / periodo.max(1) as u64) as i32
}

/// De 0 a 256 y vuelta, suave, en `periodo` ms.
pub fn onda(ms: u32, periodo: u32) -> i32 {
    (seno(fase(ms, periodo) - 64) + 256) / 2
}

/// Un numero que salta, el mismo para la misma semilla.
pub fn azar(semilla: u32) -> u32 {
    let mut z = semilla.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15);
    z ^= z >> 15;
    z = z.wrapping_mul(0x2C1B_3C6D);
    z ^= z >> 12;
    z
}

/// De `a` a `b` en `t` (0..=256).
pub fn entre(a: i32, b: i32, t: i32) -> i32 {
    a + (b - a) * t.clamp(0, 256) / 256
}
