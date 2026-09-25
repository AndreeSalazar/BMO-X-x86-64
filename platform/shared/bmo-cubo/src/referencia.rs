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
