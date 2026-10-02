//! E6 (02-10): los `si` y los bucles del [`Programa`], en el interprete --
//! que es el juez del emisor de la 3060 (`bmo-proton-x-sm86`).

use alloc::vec;
use alloc::vec::Vec;

use crate::dxil::ejemplos::{anidado, bucle_entero, bucle_geometrico, programa, si_sino};
use crate::dxil::programa::{MalaForma, Op, Programa};

fn correr(p: &Programa, x: f32, y: f32) -> [f32; 4] {
    assert_eq!(p.forma(), Ok(()));
    let mut s = [[0.0f32; 4]; 1];
    p.correr(&[[x, y, 0.0, 0.0]], &[], &mut s, &mut Vec::new());
    s[0]
}

#[test]
fn un_si_corre_una_rama_o_la_otra() {
    let p = si_sino();
    assert_eq!(correr(&p, 2.0, 3.0)[0], 6.0);
    assert_eq!(correr(&p, 3.0, 2.0)[0], 5.0);
    // Con un NaN `Menor` no se cumple: la rama del SiNo.
    assert!(correr(&p, f32::NAN, 2.0)[0].is_nan());
    assert!(p.salta());
}

#[test]
fn un_bucle_sale_por_su_romper() {
    let p = bucle_geometrico();
    let (mut acc, mut v) = (0.0f32, 1.0f32);
    loop {
        acc += v;
        v *= 0.5;
        if acc >= 1.9 {
            break;
        }
    }
    let s = correr(&p, 1.9, 1.0);
    assert_eq!((s[0], s[1]), (acc, v));
    // Una sola vuelta si la primera ya llega.
    assert_eq!(correr(&p, 0.5, 1.0)[0], 1.0);
}

#[test]
fn un_contador_entero_y_elige() {
    let p = bucle_entero();
    let s = correr(&p, 1.5, 0.0);
    assert_eq!(s[0], 7.5);
    // 7.5 > 6: elige acc.
    assert_eq!(s[1], 7.5);
    // El contador son BITS de entero: 5.
    assert_eq!(s[2].to_bits(), 5);
    // Con x negativo, 5x < 4x: elige -1.
    assert_eq!(correr(&p, -1.0, 0.0)[1], -1.0);
}

#[test]
fn bucles_anidados_y_romper_dentro_de_un_si() {
    let p = anidado();
    // Cada vuelta de fuera cuenta los j con j*y <= x: j = 0, 1, 2 (con
    // x = 2, y = 1), tres veces: 9.
    assert_eq!(correr(&p, 2.0, 1.0)[0], 9.0);
    assert_eq!(correr(&p, 0.5, 1.0)[0], 3.0);
}

#[test]
fn la_forma_se_comprueba() {
    use Op::*;
    let mal = |ops: Vec<Op>| programa(ops, 2, &[]).forma();
    assert_eq!(mal(vec![SiNo]), Err(MalaForma(0)));
    assert_eq!(mal(vec![Si { c: 0 }, SiNo, SiNo, FinSi]), Err(MalaForma(2)));
    assert_eq!(mal(vec![Romper]), Err(MalaForma(0)));
    assert_eq!(mal(vec![Si { c: 0 }, Romper, FinSi]), Err(MalaForma(1)));
    assert_eq!(mal(vec![Bucle, FinSi]), Err(MalaForma(1)));
    assert_eq!(mal(vec![Bucle]), Err(MalaForma(1)));
    assert_eq!(mal(vec![Bucle, Si { c: 0 }, Romper, FinSi, FinBucle]), Ok(()));
    assert_eq!(mal(vec![Bucle; 33]), Err(MalaForma(32)));
}

// -- El SM5 de FXC con saltos ------------------------------------------------

fn sm5(f: fn() -> (Vec<u32>, Vec<crate::dxil::Elemento>, Vec<crate::dxil::Elemento>)) -> Programa {
    let (t, e, s) = f();
    crate::sm5::compilar(&t, &e, &s).unwrap()
}

fn correr2(p: &Programa, x: f32, y: f32) -> [f32; 4] {
    let mut s = [[0.0f32; 4]; 1];
    p.correr(&[[x, y, 0.0, 0.0]], &[], &mut s, &mut Vec::new());
    s[0]
}

#[test]
fn el_sm5_con_loop_breakc_if_else_iadd_y_movc() {
    let p = sm5(crate::dxil::ejemplos::sm5_bucle);
    assert!(p.salta());
    assert_eq!(correr2(&p, 1.5, 2.0)[..2], [6.0, 1.0]);
    assert_eq!(correr2(&p, 2.0, 1.5)[..2], [6.0, 1.0]);
    assert_eq!(correr2(&p, 3.0, 0.25)[..2], [1.0, 1.0]);
    // Las lecturas de v0 fueron AL PRINCIPIO (las pide un camino del if).
    assert!(matches!(p.ops[0], Op::Entrada { .. }) && matches!(p.ops[1], Op::Entrada { .. }));
}

#[test]
fn el_sm5_con_if_z_eq_y_break() {
    let p = sm5(crate::dxil::ejemplos::sm5_si_cero);
    assert_eq!(correr2(&p, 2.0, 2.0)[..2], [7.0, 3.0]);
    assert_eq!(correr2(&p, 2.0, 3.0)[..2], [5.0, 3.0]);
    // Con NaN, eq es falsa: 5.
    assert_eq!(correr2(&p, f32::NAN, f32::NAN)[0], 5.0);
}

#[test]
fn un_sm5_con_el_if_sin_cerrar_se_dice() {
    let (mut t, e, s) = crate::dxil::ejemplos::sm5_si_cero();
    // Sin el endif (21, una palabra): el if queda abierto, y el ret del final
    // cae DENTRO de el -- lo primero que se dice.
    let k = t.iter().position(|&w| w == 21 | 1 << 24).unwrap();
    t.remove(k);
    t[1] -= 1;
    assert_eq!(crate::sm5::compilar(&t, &e, &s), Err(crate::dxil::programa::NoPrograma::Forma("un ret dentro de un if o un loop: todavia no")));
}

// -- E6b: el DXIL de dxc con saltos ------------------------------------------

const DXIL_SALTOS: &[u8] = include_bytes!("../prueba/saltos.dxil");
const DXIL_ANIDADO: &[u8] = include_bytes!("../prueba/anidado.dxil");
const DXIL_MIENTRAS: &[u8] = include_bytes!("../prueba/mientras.dxil");

fn dxil(d: &[u8]) -> Programa {
    let p = crate::dxil::programa::compilar(&crate::dxil::leer(d).unwrap()).unwrap();
    assert_eq!(p.forma(), Ok(()));
    p
}

/// El pixel con `uv = (x, y)` (el elemento 1) y el cbuffer `k`.
fn pixel(p: &Programa, x: f32, y: f32, k: [f32; 4]) -> [f32; 4] {
    let cb: Vec<u8> = k.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut s = [[0.0f32; 4]; 1];
    p.correr(&[[0.0; 4], [x, y, 0.0, 0.0]], &cb, &mut s, &mut Vec::new());
    s[0]
}

/// [!] Con `k.y = 0.5` y `x = -0.5`, el `while` de `mientras.hlsl` hace
/// `x = -0.5 * 2 + 0.5 = -0.5` para siempre: el sombreador NO acaba (ni en
/// la casa ni en una GPU). Con `k.y = 1` si.
const K: [[f32; 4]; 3] = [[2.0, 1.0, 0.75, 3.0], [0.5, 1.0, 1.5, 100.0], [5.0, 1.0, -0.25, 1.0]];
const UV: [(f32, f32); 6] = [(0.1, 0.9), (0.9, 0.1), (0.7, 0.7), (3.0, 0.25), (-0.5, 0.6), (0.0, -0.0)];

/// `saltos.hlsl`: un if/else con su `phi`, y un bucle rotado con `break`.
fn saltos_ref(x: f32, y: f32, k: [f32; 4]) -> [f32; 4] {
    let mut acc = if x < y { k[0] * x } else { k[1] + y };
    let mut i = 0;
    loop {
        acc = k[2] + acc;
        if acc > k[3] {
            break;
        }
        i += 1;
        if i >= 4 {
            break;
        }
    }
    [acc, x, y, 1.0]
}

/// `anidado.hlsl`: dos bucles, un `continue` y un `break` dentro, y un `?:`.
fn anidado_ref(x: f32, y: f32, k: [f32; 4]) -> [f32; 4] {
    let mut s = 0.0f32;
    for _ in 0..3 {
        let mut s7 = s;
        let mut j = 0;
        let s22 = loop {
            let s18 = if j == 1 {
                s7
            } else {
                let s14 = k[0] * x + s7;
                if s14 > k[1] {
                    break s14;
                }
                s14
            };
            j += 1;
            if j >= 4 {
                break s18;
            }
            s7 = s18;
        };
        s = s22 * 0.5;
    }
    let t = if y > 0.5 { s } else { -0.0 - s };
    [s, t, 0.0, 1.0]
}

/// `mientras.hlsl`: un `while` (el `if` de fuera y su bucle) con un if/else.
fn mientras_ref(mut x: f32, mut y: f32, k: [f32; 4]) -> [f32; 4] {
    if x < k[0] {
        loop {
            x = k[1] + x * 2.0;
            y = if x != y { y + 1.0 } else { y + -1.0 };
            if !(x < k[0]) {
                break;
            }
        }
    }
    [x, y, 0.0, 1.0]
}

#[test]
fn el_dxil_de_dxc_con_saltos_corre_como_su_hlsl() {
    for (d, f) in [(DXIL_SALTOS, saltos_ref as fn(f32, f32, [f32; 4]) -> [f32; 4]), (DXIL_ANIDADO, anidado_ref), (DXIL_MIENTRAS, mientras_ref)] {
        let p = dxil(d);
        assert!(p.salta());
        for k in K {
            for (x, y) in UV {
                let (casa, juez) = (pixel(&p, x, y, k), f(x, y, k));
                for c in 0..4 {
                    assert_eq!(casa[c].to_bits(), juez[c].to_bits(), "uv ({x}, {y}) k {k:?}: {casa:?} y el hlsl {juez:?}");
                }
            }
        }
    }
}

/// Lo que sale del grafo de `anidado`: dos bucles, el `continue` y el
/// `break` del de dentro como `Si`, y ni un bloque repetido (los dos
/// `add` de los contadores, una vez cada uno).
#[test]
fn el_grafo_de_anidado_sale_estructurado_sin_repetir() {
    let p = dxil(DXIL_ANIDADO);
    let n = |f: fn(&Op) -> bool| p.ops.iter().filter(|o| f(o)).count();
    assert_eq!(n(|o| matches!(o, Op::Bucle)), 2);
    assert_eq!(n(|o| matches!(o, Op::SumaEntera { .. })), 2);
    assert!(n(|o| matches!(o, Op::Romper | Op::RomperSi { .. })) >= 2);
}

