//! Las pruebas de D4.4 (05-10): las DERIVADAS de un cuadro de 2x2 y la MIP
//! que eligen con ellas los muestreos, sin la casa ni un `.exe` (el juez de
//! verdad es `prueba/derivadas.exe`, en `proton-x-casa/tests/corre`).

use alloc::vec::Vec;

use crate::dxil::olas::{self, Ola};
use crate::textura::{Muestreador, Textura};

/// 8 x 8 con sus 4 mips (8, 4, 2, 1), cada una de un color solido: la k, R =
/// 50k y A = 255.
fn mips() -> Vec<u32> {
    (0..4u32).flat_map(|k| core::iter::repeat_n(50 * k | 0xFF << 24, (64 >> (2 * k)) as usize)).collect()
}

/// Un muestreador estatico (`D3D12_STATIC_SAMPLER_DESC`, 13 palabras):
/// `filtro`, CLAMP, y su sesgo, MinLOD y MaxLOD.
fn estatico(filtro: u32, sesgo: f32, minimo: f32, maximo: f32) -> Muestreador {
    Muestreador::de_estatico(&[filtro, 3, 3, 3, sesgo.to_bits(), 1, 0, 0, minimo.to_bits(), maximo.to_bits(), 0, 0, 0]).unwrap()
}

/// El rojo de la mip `k` (o de la mezcla), en float.
fn rojo(c: [f32; 4]) -> f32 {
    c[0] * 255.0
}

/// Unos gradientes isotropicos de `paso` en (u, v) por pixel.
fn iso(paso: f32) -> [f32; 4] {
    [paso, 0.0, 0.0, paso]
}

#[test]
fn lambda_es_log2_del_lado_mayor_del_pixel_en_texeles() {
    let t = mips();
    let tx = Textura { mips: 4, ..Textura::rgba(&t, 8, 8, false) };
    // Un texel por pixel: 0; dos, 1; cuatro, 2. Exactos (potencias de dos).
    assert_eq!(tx.lambda(iso(1.0 / 8.0)), 0.0);
    assert_eq!(tx.lambda(iso(0.25)), 1.0);
    assert_eq!(tx.lambda(iso(0.5)), 2.0);
    // El lado MAYOR (isotropico): el de y, de cuatro texeles.
    assert_eq!(tx.lambda([1.0 / 8.0, 0.0, 0.0, 0.5]), 2.0);
    // Sin gradientes (un carril solo): -infinito, que se sujeta a la mip 0.
    assert_eq!(tx.lambda([0.0; 4]), f32::NEG_INFINITY);
    // Medio texel por pixel: -1 sin sujetar, 0 sujeto (CalculateLevelOfDetail).
    let m = estatico(0x01, 0.0, 0.0, f32::MAX);
    assert_eq!((tx.lod(&m, iso(1.0 / 16.0), false), tx.lod(&m, iso(1.0 / 16.0), true)), (-1.0, 0.0));
}

#[test]
fn la_mip_de_un_muestreo_es_la_de_sus_gradientes_y_sus_limites() {
    let t = mips();
    let tx = Textura { mips: 4, ..Textura::rgba(&t, 8, 8, false) };
    let punto = estatico(0x00, 0.0, 0.0, f32::MAX);
    let c = [0.5, 0.5, 0.0, 0.0];
    let en = |tx: &Textura, m: &Muestreador, g: [f32; 4], sesgo: f32| rojo(tx.muestrear_grad(m, c, g, [sesgo, 0.0], [0; 3]));
    // PUNTO: la mip de lambda; de mas, la ultima.
    for (k, paso) in [(0, 0.125), (1, 0.25), (2, 0.5), (3, 1.0), (3, 4.0)] {
        assert_eq!(en(&tx, &punto, iso(paso), 0.0), 50.0 * k as f32, "paso {paso}");
    }
    // MIP_LINEAR a lambda 1,5 (el sesgo del sombreador): la media de la 1 y la 2.
    let lineal = estatico(0x01, 0.0, 0.0, f32::MAX);
    assert!((en(&tx, &lineal, iso(0.25), 0.5) - 75.0).abs() < 1e-4);
    // El MipLODBias del muestreador suma igual: lambda 1 + 1 = la mip 2.
    assert_eq!(en(&tx, &estatico(0x00, 1.0, 0.0, f32::MAX), iso(0.25), 0.0), 100.0);
    // MaxLOD 1: lambda 3 se queda en la 1. MinLOD 2: lambda 0 sube a la 2.
    assert_eq!(en(&tx, &estatico(0x00, 0.0, 0.0, 1.0), iso(1.0), 0.0), 50.0);
    assert_eq!(en(&tx, &estatico(0x00, 0.0, 2.0, f32::MAX), iso(0.125), 0.0), 100.0);
    // Una vista desde la mip 1 (MostDetailedMip): lambda se mide en ELLA (4
    // texeles), y un texel por pixel es la mip 1 del recurso.
    assert_eq!(en(&Textura { mip: 1, ..tx }, &punto, iso(0.25), 0.0), 50.0);
    // ResourceMinLODClamp 3 (en mips del RECURSO): nada baja de la 3.
    assert_eq!(en(&Textura { lod_min: 3.0, ..tx }, &punto, iso(0.125), 0.0), 150.0);
    // Una vista de UNA mip (MipLevels = 1): lambda 3 se queda en ella.
    assert_eq!(en(&Textura { niveles: 1, ..tx }, &punto, iso(1.0), 0.0), 0.0);
}

#[test]
fn las_derivadas_restan_carriles_de_su_cuadro() {
    // Los carriles 4..8 son el segundo cuadro: 4 5 arriba, 6 7 abajo. Cada
    // uno vale j * j (en float), para que cada resta se distinga.
    let a = |j: usize| ((j * j) as f32).to_bits();
    let d = |y: bool, fina: bool, k: usize| f32::from_bits(olas::hacer(Ola::Derivada { y, fina, muestra: false }, k, 0xFF, a, 0)[0]);
    // Gruesas: la fila (columna) de arriba a la izquierda, para los cuatro.
    for k in 4..8 {
        assert_eq!(d(false, false, k), 25.0 - 16.0, "ddx gruesa, carril {k}");
        assert_eq!(d(true, false, k), 36.0 - 16.0, "ddy gruesa, carril {k}");
    }
    // Finas: la de SU fila (x) y la de SU columna (y).
    assert_eq!([d(false, true, 4), d(false, true, 5), d(false, true, 6), d(false, true, 7)], [9.0, 9.0, 13.0, 13.0]);
    assert_eq!([d(true, true, 4), d(true, true, 5), d(true, true, 6), d(true, true, 7)], [20.0, 24.0, 20.0, 24.0]);
}
