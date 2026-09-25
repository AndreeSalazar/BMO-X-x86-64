//! **LAS HUELLAS DE LA 3060** -- lo que D3D12 dibujo en la RTX 3060 del
//! propietario, reducido a un numero por fotograma. Esto SI es de BMO-X.
//!
//! [carril]  VERDE     un FNV-1a sobre un bufer
//!
//! # De donde salen, y por que NO del juez
//!
//! De las PNG `estudio-d3d/referencia/dx12/cubo_f0000/0030/0060.png` de la
//! rama `estudio-d3d` (412542e) de EPICX-FRAMEWORK-DirectX12, decodificadas
//! en el anfitrion (25-09). Son capturas de la GPU bajo Windows: si salieran
//! del propio juez, compararlo con ellas seria compararlo consigo mismo. Que
//! el juez de ESTAS huellas es la prueba de que dibuja lo que dibujo la 3060.
//!
//! # La cuenta
//!
//! FNV-1a de 64 bits sobre cada pixel como `0x00RRGGBB` en little-endian
//! (4 bytes: B, G, R, 0), fila a fila. El alfa no cuenta: la PNG no lo trae.

/// La medida de las capturas.
pub const ANCHO: u32 = 1280;
pub const ALTO: u32 = 720;

/// `(fotograma, huella)` de las capturas de D3D12 en la 3060.
pub const HUELLAS: [(u32, u64); 3] = [(0, 0xab7a_fc66_3a34_5885), (30, 0x2b39_85e9_3e1a_6574), (60, 0x8dc7_ef10_f691_548e)];

// == LA 3060 BAJO BMO-X (X5, 25-09 17:18) ====================================
//
// `gpu cubo 3060 32` dio la huella 0x768333a1b90633e2 y 26.236 pixeles
// distintos del juez: 26.235 son la cara amarilla con el azul en 0x21 y no en
// 0x22 (la REGLA 4: el ROP trunca a 12 bits antes de redondear a 8), y 1 es el
// pixel (523, 199), verde en la 3060 y fondo en el juez -- el mismo que el
// estudio de Windows no explico. Con la regla 4 y ese pixel, el juez da
// EXACTAMENTE esa huella. Las dos cosas son del SILICIO: BMO-X configura el
// ROP y el rasterizador por su cuenta, sin el driver de NVIDIA, y salen igual.

/// **El modelo de la 3060**: el juez con la regla 4 del silicio. En 0, 30 y
/// 60 da lo mismo que el redondeo exacto (las huellas de D3D12).
pub const REGLAS_DE_LA_3060: crate::Reglas = crate::Reglas { unorm8: crate::a_unorm8_truncar_12, ..crate::Reglas::D3D10 };

/// Lo que el juez no explica y la 3060 hace igual con y sin Windows:
/// `(fotograma, x, y, 0x00RRGGBB que pinta la 3060)`, a 1280x720.
pub const SIN_EXPLICAR: [(u32, u32, u32, u32); 1] = [(32, 523, 199, 0x1E_8A1E)];

/// Huellas MEDIDAS de la 3060 bajo BMO-X (no son capturas de D3D12).
pub const HUELLAS_BMO_X: [(u32, u64); 1] = [(32, 0x7683_33a1_b906_33e2)];

/// **Lo que dibuja la 3060** en el fotograma `f` a 1280x720: el juez con
/// [`REGLAS_DE_LA_3060`] y los pixeles [`SIN_EXPLICAR`].
pub fn como_la_3060(f: u32, destino: &mut [u32]) {
    crate::dibujar_por_cpu_con(crate::angulo_de_fotograma(f), ANCHO, ALTO, destino, &REGLAS_DE_LA_3060);
    for &(g, x, y, c) in SIN_EXPLICAR.iter() {
        if g == f {
            destino[(y * ANCHO + x) as usize] = 0xFF00_0000 | c;
        }
    }
}

/// La huella de la 3060 para el fotograma `f`, si se capturo.
pub fn de_la_3060(f: u32) -> Option<u64> {
    HUELLAS.iter().find(|(n, _)| *n == f).map(|(_, h)| *h)
}

/// **La huella de un bufer** (`0x??RRGGBB` por pixel).
pub fn huella(pixeles: &[u32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in pixeles {
        for b in (p & 0x00FF_FFFF).to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::vec;

    /// *** EL JUEZ, EN ESTA MAQUINA, DIBUJA LO QUE DIBUJO LA 3060 EN WINDOWS:
    /// los tres fotogramas capturados, bit a bit (via su huella).
    #[test]
    fn el_juez_da_las_huellas_de_la_3060() {
        let mut px = vec![0u32; (ANCHO * ALTO) as usize];
        for (f, esperada) in HUELLAS {
            crate::dibujar_por_cpu(crate::angulo_de_fotograma(f), ANCHO, ALTO, &mut px);
            assert_eq!(huella(&px), esperada, "fotograma {f}");
        }
    }

    /// *** EL MODELO DA LO QUE LA 3060 DIBUJO BAJO BMO-X (y bajo Windows en 0,
    /// 30 y 60): la regla 4 y el pixel sin explicar son del silicio.
    #[test]
    fn el_modelo_da_lo_que_dibujo_la_3060() {
        let mut px = vec![0u32; (ANCHO * ALTO) as usize];
        for (f, esperada) in HUELLAS.iter().chain(HUELLAS_BMO_X.iter()) {
            como_la_3060(*f, &mut px);
            assert_eq!(huella(&px), *esperada, "fotograma {f}");
        }
    }

    #[test]
    fn la_huella_ve_un_solo_bit() {
        let mut a = vec![0x0010_1018u32; 64];
        let h = huella(&a);
        a[33] ^= 1;
        assert_ne!(huella(&a), h);
        a[33] ^= 1;
        a[7] ^= 0xFF00_0000;
        assert_eq!(huella(&a), h, "el alfa no cuenta");
    }

    #[test]
    fn solo_hay_huella_de_lo_capturado() {
        assert!(de_la_3060(30).is_some());
        assert_eq!(de_la_3060(31), None);
    }
}
