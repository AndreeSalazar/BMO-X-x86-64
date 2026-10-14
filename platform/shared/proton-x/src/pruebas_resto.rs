//! **14-10: el `%` de floats y los BINOP solo de enteros** (`resto.hlsl`, de
//! `dxc`, cs_6_0), el CS de Cyberpunk que decia "una operacion binaria que
//! no es + - * /". `frem` es `a - b * trunc(a / b)`, cada paso redondeado
//! (`OpFRem` en la especificacion de Vulkan; dxil-spirv lo traduce asi), y
//! los enteros, los de HLSL.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use crate::bufer::{Bufer, Uav};
use crate::dxil::{self, programa};
use crate::textura::Recursos;

const RESTO: &[u8] = include_bytes!("../prueba/resto.dxil");

/// Los pares `(a, b)` de los 32 hilos: chicos, negativos, cocientes grandes,
/// b = 0, infinitos y NaN.
fn pares() -> [(f32, f32); 32] {
    let mut p = [(0.0f32, 1.0f32); 32];
    let fijos = [
        (7.5, 2.0), (-7.5, 2.0), (7.5, -2.0), (-7.5, -2.0), (5.25, 0.5), (1e10, 3.0), (3.0, 1e10), (0.0, 3.0),
        (-0.0, 3.0), (1.0, 0.0), (f32::INFINITY, 2.0), (2.0, f32::INFINITY), (f32::NAN, 1.0), (16777217.0, 0.1), (123.456, 7.89), (-1e-30, 1e-31),
    ];
    for (k, x) in fijos.iter().enumerate() {
        p[k] = *x;
    }
    for k in 16..32 {
        p[k] = ((k as f32 - 20.0) * 1.37, 0.3 + k as f32 * 0.11);
    }
    p
}

#[test]
fn el_resto_de_floats_y_los_enteros_dan_lo_de_hlsl() {
    let p = programa::compilar(&dxil::leer(RESTO).unwrap()).unwrap_or_else(|e| panic!("{e:?}"));
    let pares = pares();
    let entrada: Vec<u8> = pares.iter().flat_map(|(a, b)| [a.to_bits(), b.to_bits()]).flat_map(u32::to_le_bytes).collect();
    let srv = [Some(Bufer { bytes: &entrada, formato: 0, paso: 4, elementos: 64 })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let mut salida = vec![0xFFu8; 32 * 8];
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 4, elementos: 64, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
        assert_eq!(p.despachar([1, 1, 1], &[0u8; 16], &rec, &mut uavs), 32);
    }
    let w = |k: usize| u32::from_le_bytes(salida[4 * k..4 * k + 4].try_into().unwrap());
    for (k, &(a, b)) in pares.iter().enumerate() {
        let resto = a - b * (a / b).trunc();
        let visto = f32::from_bits(w(2 * k));
        assert!(visto.to_bits() == resto.to_bits() || (visto.is_nan() && resto.is_nan()), "{a} % {b}: {visto} y no {resto}");
        let u = a.to_bits();
        assert_eq!(w(2 * k + 1), (((u >> 3) ^ (u << 5)) | (u & 0xF0)).wrapping_add(u % 7).wrapping_add(u / 3), "los enteros del hilo {k}");
    }
}
