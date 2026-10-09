//! ** DL13 (09-10): las Mate de SERIES en la 3060 (`series.rs`) -- cada una,
//! su receta grabada y traducida, emitida en los dos ABI, juzgada (R0..R6 y
//! R7, el cuerpo de una app), y corrida en el simulador sobre los bordes, los
//! de su funcion y valores al azar: los MISMOS bits que la casa
//! (`Mate::aplicar`, la receta corrida). Y la FMA de la casa contra la FFMA
//! del simulador: dos cuentas que no comparten nada.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Op};
use bmo_proton_x::fma_casa;
use bmo_proton_x::mates::Mate;

use crate::fma::{ffma, Redondeo};
use crate::pruebas_mates::{azar, bordes, emitidas, en_banco, en_registros, igual, TECHO};
use crate::{emitir, series};

const SERIES: [Mate; 11] = [Mate::Sin, Mate::Cos, Mate::Tan, Mate::Exp2, Mate::Log2, Mate::Atan, Mate::Asin, Mate::Acos, Mate::Senh, Mate::Cosh, Mate::Tanh];

/// Los de cada funcion: donde cambia de camino, sus ceros y sus polos, lo
/// grande, y una rejilla de su dominio.
fn de_la_funcion(f: Mate) -> Vec<u32> {
    let mut v: Vec<f32> = Vec::new();
    let rejilla = |v: &mut Vec<f32>, desde: f32, hasta: f32, n: usize| {
        for k in 0..=n {
            let x = desde + (hasta - desde) * k as f32 / n as f32;
            v.extend([x, f32::from_bits(x.to_bits() + 1), f32::from_bits(x.to_bits().wrapping_sub(1))]);
        }
    };
    match f {
        Mate::Sin | Mate::Cos | Mate::Tan => {
            rejilla(&mut v, -20.0, 20.0, 400);
            for k in -40..=40 {
                let x = k as f32 * core::f32::consts::FRAC_PI_4;
                v.extend([x, f32::from_bits(x.to_bits() + 1), f32::from_bits(x.to_bits().wrapping_sub(1))]);
            }
            v.extend([105_615.0, 6_291_455.5, 6_291_456.0, 6_291_457.0, 8_388_607.0, 8_388_608.0, 1e7, 1.6e7, 1e9, 1e20, 3e38, 22.0, 355.0, 1e5, 6.2831855]);
        }
        Mate::Exp2 => {
            rejilla(&mut v, -160.0, 140.0, 600);
            v.extend([-149.0, -149.5, -150.0, -150.5, -126.0, -126.5, -125.9, 127.0, 127.99999, 128.0, 128.5, 129.0, -152.0, -153.0, 1e-10, -1e-10]);
        }
        Mate::Log2 => {
            rejilla(&mut v, 0.0, 4.0, 400);
            v.extend([0.70710677, 0.7071068, 1.4142135, 1.4142137, 1.0000001, 0.99999994, 1e-45, 1e-40, 1.1754942e-38, 1.1754944e-38, 3.4e38]);
        }
        Mate::Atan => {
            rejilla(&mut v, -10.0, 10.0, 400);
            v.extend([0.41421354, 0.41421357, 0.4142136, 2.4142134, 2.4142137, 1e10, 1e30, 1.2676506e30, 1.3e30, 3e38, 1e-30]);
        }
        Mate::Asin | Mate::Acos => {
            rejilla(&mut v, -1.0, 1.0, 400);
            v.extend([0.5, 0.50000006, 0.49999997, 1.0, 0.99999994, 1.0000001, 2.0, 1e-30]);
        }
        _ => {
            rejilla(&mut v, -12.0, 12.0, 400);
            v.extend([0.625, 0.62499994, 1.0, 0.99999994, 9.5, 9.49, 20.0, 88.0, 88.7, 89.4, 89.5, 90.0, 1e3, 1e-30, 1e-6]);
        }
    }
    let mut b: Vec<u32> = v.iter().map(|x| x.to_bits()).collect();
    b.extend(v.iter().map(|x| (-x).to_bits()));
    b
}

/// *** CADA MATE DE SERIES, con los bits de la casa en los dos ABI.
#[test]
fn cada_mate_de_series_da_los_bits_de_la_casa() {
    let mut comunes = bordes();
    comunes.extend(azar(4_000, 0xD113));
    for f in SERIES {
        let (e, r) = emitidas(f);
        for &x in comunes.iter().chain(&de_la_funcion(f)) {
            igual(f, x, en_banco(&e.codigo, x), "el banco");
            igual(f, x, en_registros(&r, x), "registros");
        }
    }
}

/// ** `x = f(x)` en un bucle: la receta escribe su destino en la ULTIMA, y
/// no pisa lo que aun lee.
#[test]
fn en_un_bucle_sobre_si_misma() {
    for f in SERIES {
        let p = ejemplos::programa(
            alloc::vec![
                Op::Entrada { d: 0, elemento: 0, componente: 0 },
                Op::Copia { d: 1, a: 0 },
                Op::Copia { d: 2, a: 5 },
                Op::Bucle,
                Op::Compara { d: 3, a: 2, b: 6, como: Comparacion::MayorIgual, entero: true },
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
        for &x in bordes().iter().step_by(7).chain(&azar(300, 11)) {
            let mut v = x;
            for _ in 0..3 {
                v = f.aplicar(v);
            }
            let sass = en_banco(&e.codigo, x);
            assert!(sass == v || (f32::from_bits(sass).is_nan() && f32::from_bits(v).is_nan()), "{:?}^3({:#010x}): la 3060 {:#010x} y la casa {:#010x}", f, x, sass, v);
        }
    }
}

/// ** LO QUE CUESTA cada una (con su entrada y su EXIT, el ABI del banco):
/// lo dice, y lo que dice `series::instrucciones` es lo que sale.
#[test]
fn lo_que_cuesta_cada_serie() {
    for f in SERIES {
        let (e, r) = emitidas(f);
        let receta = series::instrucciones(f).unwrap();
        std::eprintln!("{:?}: la receta {} instrucciones; con el ABI de registros {} en total, {} registros", f, receta, r.codigo.len(), r.registros);
        assert!(r.codigo.len() <= receta + 2, "{:?}: {} y la receta {}", f, r.codigo.len(), receta);
        assert!(e.codigo.len() <= 128, "{:?}: no cabe en un hueco: {}", f, e.codigo.len());
    }
}

/// *** LA FMA DE LA CASA (f64 a lo impar) y la FFMA del simulador (enteros):
/// los mismos bits, sobre bordes cruzados y tres millones al azar -- y las
/// dos, la `fmaf` de la `std` --.
#[test]
fn la_fma_de_la_casa_es_la_del_simulador() {
    let b = bordes();
    let mut todos: Vec<(u32, u32, u32)> = Vec::new();
    for (i, &x) in b.iter().enumerate().step_by(5) {
        for &y in b.iter().skip(i % 13).step_by(23) {
            todos.push((x, y, b[(i * 7 + y as usize) % b.len()]));
        }
    }
    let r = azar(9_000_000, 0xF3A);
    for k in r.chunks_exact(3) {
        // Los exponentes juntos, para que se cancelen y redondeen de verdad.
        let c = (k[2] & 0x807F_FFFF) | ((k[0] >> 23 & 0xFF).wrapping_add(k[1] >> 23 & 0xFF).wrapping_sub(127).min(254) << 23);
        todos.push((k[0], k[1], c));
        todos.push((k[0], k[1], k[2]));
    }
    for (x, y, z) in todos {
        let (a, bb, c) = (f32::from_bits(x), f32::from_bits(y), f32::from_bits(z));
        let casa = fma_casa::fma(a, bb, c);
        let sim = ffma(x, y, z, Redondeo::Cercano);
        let std_ = a.mul_add(bb, c);
        let mismo = |p: f32, q: u32| p.to_bits() == q || (p.is_nan() && f32::from_bits(q).is_nan());
        assert!(mismo(casa, sim), "fma({:e}, {:e}, {:e}): la casa {:#010x}, el simulador {:#010x}", a, bb, c, casa.to_bits(), sim);
        assert!(mismo(std_, casa.to_bits()), "fma({:e}, {:e}, {:e}): la std {:#010x}, la casa {:#010x}", a, bb, c, std_.to_bits(), casa.to_bits());
    }
}

const MATES: &[u8] = include_bytes!("../../proton-x/prueba/mates.dxil");

/// *** `mates.hlsl` de `dxc` (N5.6: las doce que pedian los sombreadores de
/// Cyberpunk en el metal -- sin, cos, tan, exp2, log2, frac, los redondeos y
/// los medios floats --) de punta a punta en la 3060: el lector, el emisor,
/// el juez y el simulador, en los dos ABI, con los bits de la casa.
#[test]
fn el_pixel_de_dxc_con_la_matematica_de_cyberpunk() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(MATES).unwrap()).unwrap();
    let mut ent = Vec::new();
    let mut xs: Vec<f32> = alloc::vec![0.0, -0.0, 0.5, 1.25, -2.75, 3.3, 100.0, -1e-30, 1e30, f32::NAN, f32::INFINITY, -f32::INFINITY, 127.5, -149.5, 1e-40];
    xs.extend(azar(200, 0xC2077).into_iter().map(f32::from_bits));
    for (k, &x) in xs.iter().enumerate() {
        let y = xs[(k * 7 + 3) % xs.len()];
        let u = (x.to_bits() >> 7) & 0xFFFF;
        ent.push(alloc::vec![[0.0; 4], [x, y, -x, y * 0.5], [f32::from_bits(u), 0.0, 0.0, 0.0]]);
    }
    let e = emitir(&p, TECHO).unwrap_or_else(|x| panic!("{:?}", x));
    let r = crate::emitir_con(&p, TECHO, crate::Abi::Registros).unwrap_or_else(|x| panic!("{:?}", x));
    for x in [&e, &r] {
        let v = bmo_gpu_ga10x::sass::juez::juzgar_drenado(&x.codigo, &bmo_gpu_ga10x::sass::juez::Contexto { registros: x.registros + bmo_gpu_ga10x::sass::juez::RESERVADOS, sph: None });
        assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(bmo_gpu_ga10x::sass::juez::juzgar_cuerpo_de_app(&r.codigo, r.registros), Ok(()));
    for x in &ent {
        crate::pruebas::igual(&p, &e.codigo, x, &[]);
        crate::pruebas::igual_en_registros(&p, &r, x, &[]);
    }
    std::eprintln!("mates.hlsl: {} instrucciones ({} con el ABI de registros), {} registros", e.codigo.len(), r.codigo.len(), r.registros);
}

/// *** LA PUERTA: cada una sola, en un programa de pixel pegado por el
/// pegamento del driver, CABE en el hueco de 128 y el juez de programas la
/// da por buena. `mates.hlsl` entero -- cinco de series y siete exactas, 290
/// instrucciones -- no cabe: el pegamento lo dice (`NoPega::Instrucciones`)
/// y su PSO va por la CPU, como `arreglos.hlsl`.
#[test]
fn cada_serie_por_la_puerta() {
    use crate::pruebas_indexado::por_la_puerta;
    use bmo_gpu_ga10x::pegamento::NoPega;
    for f in SERIES {
        let n = por_la_puerta(&crate::pruebas_mates::una(f), &[Some(0)]).unwrap_or_else(|x| panic!("{:?}: {:?}", f, x));
        std::eprintln!("{:?}: {} instrucciones pegado", f, n);
    }
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(MATES).unwrap()).unwrap();
    assert_eq!(por_la_puerta(&p, &[None, Some(0), Some(1)]), Err(NoPega::Instrucciones));
}
