//! ** E8 (09-10): la matematica EXACTA de la casa en la 3060 (`mates.rs`) --
//! cada `Mate` exacta, emitida en los dos ABI, juzgada, y corrida en el
//! simulador sobre los bordes, valores al azar y (los medios floats) los
//! 65536: los MISMOS bits que `Mate::aplicar`. Exactos tambien los que dan
//! un entero o un si/no (`mismos` acepta un NaN por otro, y un 0xFFFFFFFF
//! es un NaN: aqui no vale).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::mates::Mate;

use crate::simula::{correr, Maquina};
use crate::{emitir, emitir_con, Abi, Emitido, Precarga};

pub(crate) const TECHO: u32 = 64;

const EXACTAS: [Mate; 16] = [
    Mate::Trunca,
    Mate::Suelo,
    Mate::Techo,
    Mate::RedondoPar,
    Mate::Frac,
    Mate::EsNan,
    Mate::EsInf,
    Mate::EsFinito,
    Mate::EsNormal,
    Mate::CuentaBits,
    Mate::InvierteBits,
    Mate::PrimerBitBajo,
    Mate::PrimerBitAlto,
    Mate::PrimerBitAltoConSigno,
    Mate::F16aF32,
    Mate::F32aF16,
];

/// `y = f(x)`: una entrada, la cuenta, una salida.
pub(crate) fn una(f: Mate) -> Programa {
    ejemplos::programa(
        vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Mate { d: 1, a: 0, f }, Op::Salida { s: 1, elemento: 0, componente: 0 }],
        2,
        &[],
    )
}

/// Lo que da el SASS con el ABI del banco: la entrada en c[1].
pub(crate) fn en_banco(codigo: &[(u64, u64)], x: u32) -> u32 {
    let banco: Vec<u8> = [x, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut m = Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
    correr(codigo, &mut m).unwrap();
    m.r[0]
}

/// Lo mismo con el ABI de registros: la entrada precargada, basura en todo
/// lo demas (nadie lee un registro sin escribirlo).
pub(crate) fn en_registros(e: &Emitido, x: u32) -> u32 {
    let mut m = Maquina::nueva([&[]; 8]);
    for (i, r) in m.r.iter_mut().enumerate() {
        *r = 0x7FC0_0000 | i as u32;
    }
    for &q in &e.precargas {
        match q {
            Precarga::Entrada { reg, .. } => m.r[reg as usize] = x,
            otra => panic!("una sola entrada, y nada mas: {:?}", otra),
        }
    }
    correr(&e.codigo, &mut m).unwrap();
    m.r[0]
}

/// La salida de la casa y la de la 3060 son la misma: exacta si es un entero
/// o un si/no; si es un f32, un NaN vale por otro (la casa los cuenta en Rust
/// y la 3060 da el suyo).
pub(crate) fn igual(f: Mate, x: u32, sass: u32, abi: &str) {
    let casa = f.aplicar(x);
    let real = !(f.da_entero() || f.da_booleano());
    let bien = sass == casa || (real && f32::from_bits(sass).is_nan() && f32::from_bits(casa).is_nan());
    assert!(bien, "{:?}({:#010x} = {:e}) en {}: la 3060 {:#010x} y la casa {:#010x}", f, x, f32::from_bits(x), abi, sass, casa);
}

/// Los bordes: ceros, unos, mitades, potencias de dos y sus vecinas, los de
/// 2^23 y 2^24, los subnormales, los enormes, los infinitos, NaN de cada
/// signo y carga; y los bordes de los medios floats.
pub(crate) fn bordes() -> Vec<u32> {
    let mut v: Vec<u32> = vec![0, 0x8000_0000, 1, 0x8000_0001, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0000, 0xFFC0_0000, 0x7F80_0001, 0x7FFF_FFFF, 0xFFFF_FFFF, 0x7FBF_FFFF];
    for x in [0.5f32, 1.0, 1.5, 2.5, 3.5, 0.49999997, 0.50000006, 1e-7, 123.456, 8388607.5, 8388608.0, 8388609.0, 16777215.0, 16777216.0, 1e30, 65504.0, 65519.0, 65520.0, 65535.0, 6.1035156e-5, 6.097555e-5, 5.9604645e-8, 2.9802322e-8, 2.9802326e-8, 8.940697e-8] {
        v.push(x.to_bits());
        v.push((-x).to_bits());
    }
    for e in 0..255u32 {
        let p = e << 23;
        v.extend([p, p | 1, p | 0x40_0000, p | 0x7F_FFFF, p | 0x1000, p | 0x2000, p | 0x3000, p | 0x8000_0000, p | 0x8000_1000]);
    }
    v.extend((0..32).map(|k| 1u32 << k));
    v.extend((0..32).map(|k| u32::MAX >> k));
    v
}

pub(crate) fn azar(n: usize, semilla: u32) -> Vec<u32> {
    let mut z = semilla;
    (0..n)
        .map(|_| {
            z = z.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            z ^ z >> 15
        })
        .collect()
}

/// Los dos ABI, juzgados: el del banco por R0..R6 y el de registros, ademas,
/// por R7 (el cuerpo de una app).
pub(crate) fn emitidas(f: Mate) -> (Emitido, Emitido) {
    let p = una(f);
    let e = emitir(&p, TECHO).unwrap_or_else(|x| panic!("{:?}: {:?}", f, x));
    let r = emitir_con(&p, TECHO, Abi::Registros).unwrap_or_else(|x| panic!("{:?}: {:?}", f, x));
    for x in [&e, &r] {
        let v = juzgar_drenado(&x.codigo, &Contexto { registros: x.registros + RESERVADOS, sph: None });
        assert!(v.is_ok(), "{:?}: {}", f, v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(juzgar_cuerpo_de_app(&r.codigo, r.registros), Ok(()), "{:?}: R7", f);
    (e, r)
}

/// *** CADA MATE EXACTA, con los bits de la casa en los dos ABI.
#[test]
fn cada_mate_exacta_da_los_bits_de_la_casa() {
    let mut entradas = bordes();
    entradas.extend(azar(20_000, 0x5eed));
    for f in EXACTAS {
        let (e, r) = emitidas(f);
        let mut todas = entradas.clone();
        if f == Mate::F16aF32 {
            // los 65536 medios floats, y con basura en los 16 bits de arriba
            todas.extend(0..=0xFFFFu32);
            todas.extend((0..=0xFFFFu32).map(|h| h | 0xABCD_0000));
        }
        for &x in &todas {
            igual(f, x, en_banco(&e.codigo, x), "el banco");
            igual(f, x, en_registros(&r, x), "registros");
        }
    }
}

/// ** 09-10, DL13: lo que la casa cuenta por SERIES ya se emite, por su
/// receta: sus pruebas, en `pruebas_series.rs`.

/// ** `x = f(x)` -- la misma variable de entrada y de salida, en un bucle
/// -- no pisa lo que aun lee: cada cuenta escribe su destino en la ultima.
#[test]
fn en_un_bucle_sobre_si_misma() {
    for f in [Mate::Suelo, Mate::Frac, Mate::F32aF16, Mate::PrimerBitAltoConSigno, Mate::InvierteBits] {
        // v = entrada; tres vueltas de v = f(v); salida v
        let p = ejemplos::programa(
            vec![
                Op::Entrada { d: 0, elemento: 0, componente: 0 },
                Op::Copia { d: 1, a: 0 },
                Op::Copia { d: 2, a: 5 },
                Op::Bucle,
                Op::Compara { d: 3, a: 2, b: 6, como: bmo_proton_x::dxil::programa::Comparacion::MayorIgual, entero: true },
                Op::RomperSi { c: 3, si_cero: false },
                Op::SumaEntera { d: 4, a: 2, b: 7 },
                Op::Copia { d: 2, a: 4 },
                Op::Mate { d: 1, a: 1, f },
                Op::FinBucle,
                Op::Salida { s: 1, elemento: 0, componente: 0 },
            ],
            8,
            &[(5, f32::from_bits(0)), (6, f32::from_bits(3)), (7, f32::from_bits(1))],
        );
        let e = emitir(&p, TECHO).unwrap_or_else(|x| panic!("{:?}: {:?}", f, x));
        for &x in bordes().iter().chain(&azar(2_000, 7)) {
            let mut v = x;
            for _ in 0..3 {
                v = f.aplicar(v);
            }
            let sass = en_banco(&e.codigo, x);
            let real = !(f.da_entero() || f.da_booleano());
            assert!(sass == v || (real && f32::from_bits(sass).is_nan() && f32::from_bits(v).is_nan()), "{:?}^3({:#010x}): la 3060 {:#010x} y la casa {:#010x}", f, x, sass, v);
        }
    }
}

/// ** LO QUE CUESTA cada una, con su entrada y su EXIT (el ABI del banco): lo
/// dice, y ninguna pasa de 48 instrucciones -- caben de sobra en un hueco de
/// la tuberia de VERRANO (128) --.
#[test]
fn lo_que_cuesta_cada_mate() {
    for f in EXACTAS {
        let (e, r) = emitidas(f);
        std::eprintln!("{:?}: {} instrucciones ({} con el ABI de registros), {} registros", f, e.codigo.len(), r.codigo.len(), e.registros);
        assert!(e.codigo.len() <= 48 && r.codigo.len() <= 48, "{:?}: {} y {}", f, e.codigo.len(), r.codigo.len());
    }
}
