//! **Los desplazamientos de los grupos C1 y D3** (`shl`, `shr`, `sar`),
//! como el silicio.
//!
//! ** Salio de `mod.rs` el 15-10 por L6a, como `grupo_f7.rs`: el despachador
//! llego a 1.001 lineas de codigo al entrar las rafagas del buzon (ECCE). Es
//! la funcion movida como texto.

/// ** UN DESPLAZAMIENTO COMO EL SILICIO (LB4, 08-10): `/4 shl`, `/5 shr`,
/// `/7 sar`. La cuenta se enmascara a 6 bits en 64 y a CINCO en 32, y `sar`
/// arrastra el bit de signo DE SU ANCHO (el 31 en 32 bits).
///
/// ** Hasta el 08-10 la cuenta iba siempre a 6 bits y `sar` miraba el bit 63:
/// en 32 bits, `shl eax, cl` con cl = 32 daba 0 (el silicio: eax, intacto) y
/// `sar` de 0x80000000 daba 0x40000000 (el silicio: 0xC0000000). Ningun
/// emisor de la casa desplazaba en 32 bits hasta que la CPU fue una libreria
/// de la GPU: los enteros del Programa de la casa son de 32.
pub(super) fn desplaza(ext: usize, a: u64, cuenta: u32, wide: bool) -> Option<u64> {
    let c = cuenta & if wide { 0x3F } else { 0x1F };
    Some(match (ext, wide) {
        (4, _) => a << c,
        (5, _) => a >> c,
        (7, true) => ((a as i64) >> c) as u64,
        (7, false) => ((a as u32 as i32) >> c) as u32 as u64,
        _ => return None,
    })
}
