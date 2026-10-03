//! **P3b4c.8 T0: el muestreador de la casa, para la 3060** -- lo que la casa
//! D3D12 ya lee (`bmo_proton_x::textura::Muestreador`, de un sampler
//! estatico o de `CreateSampler`) en el TSC de la tarjeta
//! (`bmo_gpu_ga10x::texturas`), y una textura de la casa en su TIC.
//!
//! capa: puro -- traduce numeros; nada llega aun a la 3060 (falta el `TEX`)

use bmo_gpu_ga10x::texturas::{self as tx, Imagen, Muestreo};
use bmo_proton_x::textura::{Direccion, Filtro, Muestreador};

/// El modo de la casa con el numero de D3D12 (`D3D12_TEXTURE_ADDRESS_MODE`).
const fn d3d(d: Direccion) -> u32 {
    match d {
        Direccion::Repetir => 1,
        Direccion::Espejo => 2,
        Direccion::Sujetar => 3,
        Direccion::Borde => 4,
        Direccion::EspejoUnaVez => 5,
    }
}

/// **El TSC de un muestreador de la casa.**
pub fn tsc(m: &Muestreador) -> Option<[u32; 8]> {
    tx::tsc(&Muestreo { lineal: m.filtro == Filtro::Lineal, u: d3d(m.u), v: d3d(m.v), borde: m.borde })
}

/// **El TIC de una textura de la casa** que la 3060 vera en `va`, con sus
/// filas juntas (`4 x ancho` bytes, como las guarda la casa). `None` si esa
/// fila no va alineada a 32 B: hara falta copiarla con otro paso (T2).
pub fn tic(va: u64, ancho: u32, alto: u32, bgra: bool) -> Option<[u32; 8]> {
    tx::tic(&Imagen { va, ancho, alto, fila: ancho.checked_mul(4)?, bgra })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los cinco modos y los dos filtros de la casa llegan al TSC con el
    /// numero de la 3060 que les toca, y el borde tal cual.
    #[test]
    fn la_casa_en_el_tsc() {
        let modos = [Direccion::Repetir, Direccion::Espejo, Direccion::Sujetar, Direccion::Borde, Direccion::EspejoUnaVez];
        let tarjeta = [tx::REPETIR, tx::ESPEJO, tx::SUJETAR, tx::BORDE, tx::ESPEJO_UNA_VEZ];
        for (m, t) in modos.iter().zip(tarjeta) {
            let s = tsc(&Muestreador { filtro: Filtro::Punto, u: *m, v: Direccion::Sujetar, borde: [1.0, 0.0, 0.0, 1.0], comparacion: 0 }).unwrap();
            assert_eq!((s[0] & 7, s[0] >> 3 & 7), (t, tx::SUJETAR));
            assert_eq!(s[1] & 3, tx::PUNTO);
            assert_eq!(s[4], 1.0f32.to_bits());
        }
        let l = tsc(&Muestreador { filtro: Filtro::Lineal, u: Direccion::Repetir, v: Direccion::Repetir, borde: [0.0; 4], comparacion: 0 }).unwrap();
        assert_eq!(l[1] & 3, tx::LINEAL);
    }

    #[test]
    fn la_textura_de_hellotexture() {
        // HelloTexture: 256x256 RGBA, filas de 1024 B.
        let t = tic(0x9_0000_0000, 256, 256, false).unwrap();
        assert_eq!(t[3], 1024 >> 5);
        assert!(tic(0x9_0000_0000, 3, 3, false).is_none(), "filas de 12 B: sin alinear a 32");
    }
}
