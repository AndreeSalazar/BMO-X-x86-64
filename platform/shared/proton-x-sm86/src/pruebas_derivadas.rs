//! ** L2 (14-10): `ddx`, `ddy` y la fina en x en la 3060 de mentira (los
//! cuatro hilos de un cuadro, `simula_cuadro`), contra los carriles de la
//! casa (`Programa::correr_pixeles`), bit a bit; el juez (R0..R6 y R7 con
//! SHFL) y un hilo solo (el vigia) como la casa.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_proton_x::dxil::carriles::Carril;
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::olas::Ola;
use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::textura::Recursos;

use crate::simula::{correr, Maquina};
use crate::simula_cuadro::correr_cuadro;
use crate::{emitir_con, Abi, NoEmite, Precarga};

/// `ddx`, `ddy` y la fina en x de la entrada 0.x, y `ddy` de 0.y.
fn programa(fina_y: bool) -> Programa {
    let der = |d, a, y, fina| Op::Ola { d, a, b: a, que: Ola::Derivada { y, fina, muestra: false } };
    let mut ops: Vec<Op> = (0..2).map(|k| Op::Entrada { d: k as u16, elemento: 0, componente: k as u8 }).collect();
    ops.extend([der(4, 0, false, false), der(5, 0, true, false), der(6, 0, false, true), der(7, 1, true, fina_y)]);
    ops.extend((0..4).map(|k| Op::Salida { s: 4 + k as u16, elemento: 0, componente: k as u8 }));
    let mut p = ejemplos::programa(ops, 8, &[]);
    p.entradas = 1;
    p
}

#[test]
fn las_derivadas_del_cuadro_como_la_casa() {
    let p = programa(false);
    let e = emitir_con(&p, 64, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    let v = juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None });
    assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    assert_eq!(juzgar_cuerpo_de_app(&e.codigo, e.registros), Ok(()), "R7: un SHFL no toca memoria");
    assert_eq!(e.codigo.iter().filter(|w| w.0 & 0xFFF == 0xF89).count(), 8, "dos SHFL por derivada");
    let vacios = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: None };
    let casos: [[[f32; 2]; 4]; 5] = [
        [[0.5, 0.5], [1.5, 0.5], [0.5, 1.5], [1.5, 1.5]],
        [[3.25, -7.0], [2.0, 1e30], [-0.0, 0.1], [9.5, -1e-38]],
        [[f32::NAN, 1.0], [2.0, f32::INFINITY], [f32::INFINITY, 3.0], [-1.0, f32::NEG_INFINITY]],
        [[1e-45, 0.0], [-1e-45, -0.0], [0.0, 1e-45], [5.0, 5.0]],
        [[16777216.0, 0.3], [16777217.0, 0.30000001], [-3.4e38, 7.0], [3.4e38, -7.0]],
    ];
    for caso in casos {
        // La casa: los cuatro carriles juntos.
        let entradas: Vec<[[f32; 4]; 1]> = caso.iter().map(|xy| [[xy[0], xy[1], 0.0, 0.0]]).collect();
        let refs: Vec<&[[f32; 4]]> = entradas.iter().map(|x| &x[..]).collect();
        let mut carriles: Vec<Carril> = (0..4).map(|_| Carril::nuevo(&p, false)).collect();
        let mut salidas = vec![vec![[0.0f32; 4]; 1]; 4];
        p.correr_pixeles(&mut carriles, &refs, &[], &vacios, &mut salidas, &mut []);
        // La 3060 de mentira.
        let hilos = correr_cuadro(&e.codigo, [&[]; 8], |k, m| {
            for &q in &e.precargas {
                if let Precarga::Entrada { componente, reg, .. } = q {
                    m.r[reg as usize] = entradas[k][0][componente as usize].to_bits();
                }
            }
        })
        .unwrap_or_else(|x| panic!("{x:?}"));
        for k in 0..4 {
            for j in 0..4 {
                let (g, c) = (hilos[k].r[j], salidas[k][0][j].to_bits());
                assert!(g == c || (f32::from_bits(g).is_nan() && f32::from_bits(c).is_nan()), "hilo {k} salida {j} con {caso:?}: la 3060 {g:#x}, la casa {c:#x}");
            }
        }
    }
    // Un hilo solo (el vigia): lo propio menos lo propio, como la casa.
    let mut m = Maquina::nueva([&[]; 8]);
    for &q in &e.precargas {
        if let Precarga::Entrada { componente, reg, .. } = q {
            m.r[reg as usize] = [2.5f32, -1.0][componente as usize].to_bits();
        }
    }
    correr(&e.codigo, &mut m).unwrap();
    assert_eq!(&m.r[..4], &[0, 0, 0, 0], "fuera de un cuadro, 0");
}

#[test]
fn la_fina_en_y_todavia_no() {
    assert_eq!(emitir_con(&programa(true), 64, Abi::Registros).err(), Some(NoEmite::Operacion(5)));
}
