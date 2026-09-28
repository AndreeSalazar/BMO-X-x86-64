//! E3: el emisor, contra la casa. Cada sombreador del cubo -- los DXIL de dxc
//! y los SM5 que FXC compilo para BMOX-12 -- se emite a SASS, se corre en el
//! simulador y se compara, BIT A BIT, con `Programa::correr`.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil::{self, programa::compilar, programa::Programa};

use crate::simula::{correr, Maquina};
use crate::{emitir, NoEmite};

const DXIL_VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const DXIL_PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");
const SM5_VS: &[u8] = include_bytes!("../../proton-x/prueba/sombras/f3ef42a0.cso");
const SM5_PS: &[u8] = include_bytes!("../../proton-x/prueba/sombras/4d67f5e4.cso");

/// Los registros que se le dan: los del cubo de VERRANO (`REGISTROS = 16`)
/// menos los DOS que Volta y despues se quedan (juez, `RESERVADOS`)... se
/// pide lo que haga falta y se dice cuanto: aqui, 64 de techo.
const TECHO: u32 = 64;

fn programa(d: &[u8]) -> Programa {
    compilar(&dxil::leer(d).unwrap()).unwrap()
}

fn cb_de(f: u32) -> Vec<u8> {
    let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
    c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
}

/// Corre `p` en la casa y su SASS en el simulador con las mismas entradas y
/// el mismo cbuffer; los bits de cada salida tienen que ser los mismos.
fn igual(p: &Programa, codigo: &[(u64, u64)], entradas: &[[f32; 4]], cb: &[u8]) {
    let mut casa = std::vec![[0.0f32; 4]; p.salidas];
    let mut regs = Vec::new();
    p.correr(entradas, cb, &mut casa, &mut regs);
    let banco: Vec<u8> = entradas.iter().flat_map(|e| e.iter().flat_map(|x| x.to_le_bytes())).collect();
    let mut m = Maquina::nueva([&[], &banco, &[], cb, &[], &[], &[], &[]]);
    correr(codigo, &mut m).unwrap();
    for (e, s) in casa.iter().enumerate() {
        for k in 0..4 {
            assert_eq!(m.r[4 * e + k], s[k].to_bits(), "salida {e}.{k}: la 3060 {} y la casa {}", f32::from_bits(m.r[4 * e + k]), s[k]);
        }
    }
}

/// *** Los de VERTICE (DXIL y SM5): 24 vertices en 4 fotogramas, bit a bit.
#[test]
fn el_vertice_emitido_da_los_bits_de_la_casa() {
    for d in [DXIL_VS, SM5_VS] {
        let p = programa(d);
        let e = emitir(&p, TECHO).unwrap();
        assert_eq!(e.mufus, 0, "el de vertice no tiene raices");
        for f in [0u32, 30, 60, 123] {
            let cb = cb_de(f);
            for v in bmo_cubo::vertices() {
                let ent = [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color];
                igual(&p, &e.codigo, &ent, &cb);
            }
        }
    }
}

/// *** Los de PIXEL: la luz, con su `rsq` (MUFU.RSQ, modelado como la casa)
/// y su `_sat` (FADD.SAT), en cada cara y 3 fotogramas.
#[test]
fn el_pixel_emitido_da_los_bits_de_la_casa() {
    for d in [DXIL_PS, SM5_PS] {
        let p = programa(d);
        let e = emitir(&p, TECHO).unwrap();
        assert_eq!(e.mufus, 1, "normalize: una raiz inversa");
        for f in [0u32, 30, 60] {
            let cb = cb_de(f);
            let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
            for v in bmo_cubo::vertices() {
                let n = bmo_cubo::mat::transformar_dir(&c.world, v.normal);
                igual(&p, &e.codigo, &[[0.0; 4], [n[0], n[1], n[2], 0.0], v.color], &cb);
            }
        }
    }
}

/// Lo que se usa, dicho: cuantas instrucciones y registros salen, que
/// ninguna es una FFMA (sin fundir, como la casa), que cabe en la puerta del
/// kernel y que el control por regla tarda menos de la MITAD que el de E3.
/// Hoy: vertice 50 instrucciones (116-128 ciclos, 18-19 registros), pixel 28
/// (72-75 ciclos, 9 registros).
#[test]
fn lo_emitido_es_corto_sin_fundir_y_cabe() {
    for (nombre, d) in [("vs dxil", DXIL_VS), ("ps dxil", DXIL_PS), ("vs sm5", SM5_VS), ("ps sm5", SM5_PS)] {
        let e = emitir(&programa(d), TECHO).unwrap();
        assert!(e.codigo.iter().all(|&(lo, _)| lo & 0x1FF != 0x023), "{nombre}: una FFMA");
        assert!(e.registros <= 32, "{nombre}: {} registros", e.registros);
        // E4: cabe en la puerta del kernel (`juez::MAX_INSTRUCCIONES`, 64) y
        // tarda menos que si cada una esperara 6 ciclos (el control de E3).
        assert!(e.codigo.len() <= 64, "{nombre}: {} instrucciones", e.codigo.len());
        assert!(e.ciclos < (6 * (e.codigo.len() - 1) + 1) as u32 / 2, "{nombre}: {} ciclos", e.ciclos);
        assert_eq!(e.codigo.last().map(|w| w.0 & 0x1FF), Some(0x14D), "{nombre}: acaba en EXIT");
    }
    // Y sin sitio, se dice.
    assert_eq!(emitir(&programa(SM5_VS), 13), Err(NoEmite::Registros));
}

/// Cada operacion, a mano, con los valores que muerden (NaN, infinitos, -0,
/// subnormales, negativos en una raiz): lo que el cubo no usa (Min, Max, Abs,
/// Sqrt, Sub con inmediato y con constante, Dot2/3/4, Mad con constante)
/// tambien tiene que dar los bits de la casa.
#[test]
fn cada_operacion_emitida_da_los_bits_de_la_casa() {
    use bmo_proton_x::dxil::programa::Op;
    // r0, r1: las entradas 0.x y 0.y; r2..r5: la fila 0 del cbuffer; r40, r41
    // constantes del modulo (inmediatos); r10..: los resultados.
    let ops = std::vec![
        Op::Entrada { d: 0, elemento: 0, componente: 0 },
        Op::Entrada { d: 1, elemento: 0, componente: 1 },
        Op::Constantes { d: 2, fila: 0 },
        Op::Mul { d: 10, a: 0, b: 1 },
        Op::Add { d: 11, a: 0, b: 40 },
        Op::Sub { d: 12, a: 0, b: 41 },
        Op::Sub { d: 13, a: 1, b: 3 },
        Op::Sub { d: 14, a: 40, b: 0 },
        Op::Mad { d: 15, a: 0, b: 2, c: 1 },
        Op::Dot { d: 16, n: 2, a: [0, 1, 0, 0], b: [2, 3, 0, 0] },
        Op::Dot { d: 17, n: 3, a: [0, 1, 41, 0], b: [2, 3, 4, 0] },
        Op::Dot { d: 18, n: 4, a: [0, 1, 0, 1], b: [2, 3, 4, 5] },
        Op::Rsqrt { d: 19, a: 0 },
        Op::Sqrt { d: 20, a: 1 },
        Op::Saturate { d: 21, a: 0 },
        Op::Abs { d: 22, a: 1 },
        Op::Min { d: 23, a: 0, b: 1 },
        Op::Max { d: 24, a: 0, b: 1 },
        Op::Min { d: 25, a: 1, b: 5 },
        Op::Max { d: 26, a: 40, b: 0 },
    ];
    let mut salidas = Vec::new();
    for (i, r) in (10u16..=26).enumerate() {
        salidas.push(Op::Salida { s: r, elemento: (i / 4) as u8, componente: (i % 4) as u8 });
    }
    let mut iniciales = std::vec![0.0f32; 42];
    iniciales[40] = 0.75;
    iniciales[41] = -3.5;
    let p = Programa { ops: ops.into_iter().chain(salidas).collect(), iniciales, entradas: 1, salidas: 5, lee: 1, filas_cb: 1 };
    let e = emitir(&p, TECHO).unwrap();
    assert_eq!(e.mufus, 2);
    let raros = [0.0f32, -0.0, 1.0, -1.0, 2.5, -7.25, 1.0e-40, -1.0e-40, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 3.0e38, 0.3];
    let mut n = 0;
    for &x in &raros {
        for &y in &raros {
            let cb: Vec<u8> = [y, x, 0.5, -2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
            let ent = [[x, y, 0.0, 0.0]];
            // Un NaN cuenta como igual a otro NaN (su carga util puede ser otra).
            let mut casa = std::vec![[0.0f32; 4]; p.salidas];
            let mut regs = Vec::new();
            p.correr(&ent, &cb, &mut casa, &mut regs);
            let banco: Vec<u8> = ent.iter().flat_map(|e| e.iter().flat_map(|v| v.to_le_bytes())).collect();
            let mut m = Maquina::nueva([&[], &banco, &[], &cb, &[], &[], &[], &[]]);
            correr(&e.codigo, &mut m).unwrap();
            for (i, s) in casa.iter().enumerate().flat_map(|(el, s)| s.iter().enumerate().map(move |(k, v)| (4 * el + k, *v))).take(17) {
                let g = f32::from_bits(m.r[i]);
                assert!(g.to_bits() == s.to_bits() || (g.is_nan() && s.is_nan()), "resultado {} con x={x:e} y={y:e}: la 3060 {g:e}, la casa {s:e}", i + 10);
                n += 1;
            }
        }
    }
    assert_eq!(n, 13 * 13 * 17);
}
