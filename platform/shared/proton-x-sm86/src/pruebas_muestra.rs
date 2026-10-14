//! ** S1 (14-10): `Sample` con la mip de sus gradientes en la 3060 de
//! mentira -- los cuatro hilos de un cuadro (`simula_cuadro`): las derivadas
//! con SHFL, el lambda con la receta de PROMETEO y las mips de G5 --, contra
//! los carriles de la casa (`Programa::correr_pixeles`), bit a bit; y el juez
//! (R0..R6, y R7 con las bases).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::carriles::Carril;
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::dxil::ranuras::Lugar;
use bmo_proton_x::textura::{Clase, Recursos, Textura};

use crate::pruebas_lecturas::{emitido, muestreadores_2d, precargar, texeles, textura, tres_mil};
use crate::simula::LecturaTex;
use crate::simula_cuadro::correr_cuadro;
use crate::Precarga;

/// `d = t0.Sample(s0, (u, v))`: u y v de la entrada 0, con sus gradientes
/// del cuadro (como los pone el DXIL de un `Sample`).
fn programa() -> Programa {
    let mut ops: Vec<Op> = (0..2).map(|k| Op::Entrada { d: k as u16, elemento: 0, componente: k as u8 }).collect();
    bmo_proton_x::dxil::olas::gradientes(&mut ops, 4, 0, 1);
    ops.push(Op::Muestra { d: 8, t: 0, s: 0, u: 0, v: 1, g: Some(4) });
    ops.extend((0..4).map(|k| Op::Salida { s: 8 + k as u16, elemento: 0, componente: k as u8 }));
    let mut p = ejemplos::programa(ops, 12, &[]);
    p.entradas = 1;
    let l = Lugar { espacio: 0, registro: 0, vista: 0 };
    p.ranuras.texturas = vec![l];
    p.ranuras.muestreadores = vec![l];
    p.ranuras.formas = vec![(0, 0, 0, 2)];
    p
}

/// Un cuadro: el (u, v) de su hilo 0 y lo que suma un paso en x y en y.
type Cuadro = ((f32, f32), (f32, f32), (f32, f32));

/// Los cuadros: de cerca (lambda < 0) a lejos (mas alla de la ultima mip),
/// anisotropos, en las medias que redondea `fijo`, girados, sin gradientes, y
/// con NaN e infinitos.
fn cuadros() -> Vec<Cuadro> {
    let mut v = Vec::new();
    for &(u, w) in &[(0.1f32, 0.2f32), (0.53, 0.47), (1.3, -0.4)] {
        for &e in &[0.01f32, 0.0625, 0.07, 0.1, 0.125, 0.17, 0.2, 0.25, 0.3, 0.5, 0.9, 2.0, 40.0] {
            v.push(((u, w), (e, 0.0), (0.0, e)));
            v.push(((u, w), (e, 0.0), (0.0, e * 0.37)));
            v.push(((u, w), (0.6 * e, 0.8 * e), (-0.8 * e, 0.6 * e)));
        }
    }
    v.push(((0.4, 0.4), (0.0, 0.0), (0.0, 0.0)));
    v.push(((0.4, 0.4), (f32::NAN, 0.0), (0.0, 0.1)));
    v.push(((0.4, 0.4), (f32::INFINITY, 0.0), (0.0, 0.1)));
    v.push(((0.4, 0.4), (1e-30, 0.0), (0.0, -1e-30)));
    v
}

#[test]
fn sample_con_mips_como_la_casa() {
    let p = programa();
    let e = emitido(&p);
    // Ocho SHFL (cuatro gradientes), dos TEX .LZ, y sin MUFU en el lambda.
    assert_eq!(e.codigo.iter().filter(|w| w.0 & 0xFFF == 0xF89).count(), 8, "dos SHFL por gradiente");
    assert_eq!(e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x361).count(), 12, "la mip y la siguiente, y diez del camino de float (F1)");
    assert!(e.precargas.iter().all(|q| !matches!(q, Precarga::Asa { .. })), "sin el asa de la vista");
    let t = texeles(16, 8, 1, 1, 5);
    // F1: y una de cuatro floats (filtrada en el sombreador).
    let tf = crate::pruebas_lecturas::floats(16, 8, 5, true);
    let flot = Textura { como: bmo_proton_x::textura::Como::Flotantes4, ..textura(&tf, Clase::Plana, 16, 8, 1, 1, 5) };
    let vistas = [textura(&t, Clase::Plana, 16, 8, 1, 1, 5), Textura { mip: 1, ..textura(&t, Clase::Plana, 16, 8, 1, 1, 5) }, Textura { niveles: 2, lod_min: 1.5, ..textura(&t, Clase::Plana, 16, 8, 1, 1, 5) }, flot, Textura { mip: 1, ..flot }];
    let mut probados = 0;
    for vista in vistas {
        let tx = [Some(vista)];
        for m in muestreadores_2d() {
            if vista.como != bmo_proton_x::textura::Como::Rgba8 && (m.u == bmo_proton_x::textura::Direccion::Borde || m.v == bmo_proton_x::textura::Direccion::Borde) {
                continue;
            }
            let ms = [Some(m)];
            let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
            let pares = crate::pso::texturas_de(&e);
            let leer = |l: &LecturaTex| tres_mil(&rec, &pares, l, false);
            for ((u, v), (xu, xv), (yu, yv)) in cuadros() {
                // F1: una de float, exacta con |u * ancho| < 2^22 (ver
                // `flotantes.rs`): un gradiente infinito lleva la u alli.
                if vista.como != bmo_proton_x::textura::Como::Rgba8 && !(xu.is_finite() && xv.is_finite() && yu.is_finite() && yv.is_finite()) {
                    continue;
                }
                let uv: [[f32; 4]; 4] = core::array::from_fn(|k| {
                    let (i, j) = ((k & 1) as f32, (k >> 1) as f32);
                    [u + i * xu + j * yu, v + i * xv + j * yv, 0.0, 0.0]
                });
                // La casa: los cuatro carriles juntos.
                let entradas: Vec<[[f32; 4]; 1]> = uv.iter().map(|x| [*x]).collect();
                let refs: Vec<&[[f32; 4]]> = entradas.iter().map(|x| &x[..]).collect();
                let mut carriles: Vec<Carril> = (0..4).map(|_| Carril::nuevo(&p, false)).collect();
                let mut salidas = vec![vec![[0.0f32; 4]; 1]; 4];
                p.correr_pixeles(&mut carriles, &refs, &[], &rec, &mut salidas, &mut []);
                // La 3060 de mentira.
                let hilos = correr_cuadro(&e.codigo, [&[]; 8], |k, mq| {
                    for (i, r) in mq.r.iter_mut().enumerate() {
                        *r = 0x7FC0_0000 | i as u32;
                    }
                    precargar(mq, &e, &rec, &[uv[k]]);
                    mq.leer_textura = Some(&leer);
                })
                .unwrap_or_else(|x| panic!("{x:?}"));
                for k in 0..4 {
                    for j in 0..4 {
                        let (g, c) = (hilos[k].r[j], salidas[k][0][j].to_bits());
                        assert!(g == c || (f32::from_bits(g).is_nan() && f32::from_bits(c).is_nan()), "hilo {k} canal {j}, {uv:?}, {m:?}, la vista desde la mip {}: la 3060 {g:#x}, la casa {c:#x}", vista.mip);
                    }
                }
                probados += 1;
            }
        }
    }
    assert!(probados > 500, "{probados}");
}

/// Los lambdas de la casa son los de la receta: los de siempre en las
/// potencias de dos (lo de `pruebas_niveles` de PROTON-X), sin gradientes
/// -infinito, y un NaN no gana al otro lado.
#[test]
fn el_lambda_de_la_receta() {
    let t = texeles(16, 8, 1, 1, 5);
    let tx = textura(&t, Clase::Plana, 16, 8, 1, 1, 5);
    assert_eq!(tx.lambda([1.0 / 16.0, 0.0, 0.0, 1.0 / 8.0]), 0.0);
    assert_eq!(tx.lambda([0.25, 0.0, 0.0, 0.125]), 2.0);
    assert_eq!(tx.lambda([0.0; 4]), f32::NEG_INFINITY);
    assert_eq!(tx.lambda([f32::NAN, 0.0, 0.25, 0.0]), 2.0);
    assert!(tx.lambda([f32::NAN, 0.0, f32::NAN, 0.0]).is_nan());
    assert_eq!(tx.lambda([f32::INFINITY, 0.0, 0.0, 0.0]), f32::INFINITY);
}
