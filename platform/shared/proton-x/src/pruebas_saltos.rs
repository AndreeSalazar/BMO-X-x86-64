//! E6 (02-10): los `si` y los bucles del [`Programa`], en el interprete --
//! que es el juez del emisor de la 3060 (`bmo-proton-x-sm86`).

use alloc::vec;
use alloc::vec::Vec;

use crate::dxil::ejemplos::{anidado, bucle_entero, bucle_geometrico, programa, si_sino};
use crate::dxil::programa::{MalaForma, Op, OpEntera, Programa};

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


// -- E6c: enteros, conversiones y switch del DXIL de dxc --------------------

const DXIL_ENTEROS: &[u8] = include_bytes!("../prueba/enteros.dxil");

/// `enteros.hlsl`, como lo dejo `dxc` (su `.ll`): `n` es la fila 1 del
/// cbuffer, como enteros.
fn enteros_ref(x: f32, y: f32, n: [i32; 4]) -> [f32; 4] {
    let a = (x * 16.0) as i32;
    let b = (y * 8.0) as u32;
    let c = n[0].wrapping_add(4).wrapping_mul(a).wrapping_sub((b >> 1) as i32);
    let d = (b ^ 5) | (a as u32 & 15);
    let m = c.max(n[1]).min(n[2]);
    let u = d.min(n[3] as u32);
    let r = match a & 3 {
        0 => c as f32,
        1 => d as f32 * 0.5,
        2 | 3 => m.wrapping_sub(u as i32) as f32,
        _ => -1.0,
    };
    let w = if b & 1 == 0 { 1.0 } else { (c >> 3) as f32 };
    [r, m as f32, u as f32, w]
}

#[test]
fn el_dxil_con_enteros_y_switch_corre_como_su_hlsl() {
    let p = dxil(DXIL_ENTEROS);
    assert!(p.ops.iter().any(|o| matches!(o, Op::Entera { .. })) && p.ops.iter().any(|o| matches!(o, Op::Convierte { .. })));
    let ns: [[i32; 4]; 3] = [[3, -50, 50, 7], [-2, i32::MIN, i32::MAX, -1], [0, 10, 5, 0]];
    let uv: [(f32, f32); 8] = [(0.1, 0.9), (0.3, 0.2), (0.7, 0.7), (-0.4, 0.6), (0.99, -0.3), (1e9, 3.0), (-1e9, 1e12), (0.0, 0.125)];
    for n in ns {
        let mut cb: Vec<u8> = [0.0f32; 4].iter().flat_map(|v| v.to_le_bytes()).collect();
        cb.extend(n.iter().flat_map(|v| v.to_le_bytes()));
        for (x, y) in uv {
            let mut s = [[0.0f32; 4]; 1];
            p.correr(&[[0.0; 4], [x, y, 0.0, 0.0]], &cb, &mut s, &mut Vec::new());
            let r = enteros_ref(x, y, n);
            for k in 0..4 {
                assert_eq!(s[0][k].to_bits(), r[k].to_bits(), "uv ({x}, {y}) n {n:?}: {:?} y el hlsl {r:?}", s[0]);
            }
        }
    }
}

// -- E6c: el SM5 de enteros y el switch --------------------------------------

fn sm5_ref_switch(x: f32, y: f32) -> [f32; 2] {
    let (a, b) = (x as i32, y as u32);
    match a {
        0 => [10.0, 0.0],
        1 | 2 => [a.wrapping_mul(3) as f32, (b >> 1) as f32],
        3 => [0.0, (b >> 1) as f32],
        _ => [a.wrapping_neg() as f32, 0.0],
    }
}

fn sm5_ref_enteros(x: f32, y: f32) -> [f32; 4] {
    let (a, b) = (x as i32, y as i32);
    let r1 = !((a & 6) ^ (a | b));
    let r2 = a.min(b).wrapping_add(a.max(b)).wrapping_add((a as u32).min(b as u32) as i32).wrapping_add((a as u32).max(b as u32) as i32);
    let r3 = a.wrapping_mul(b).wrapping_add(7).wrapping_add(a >> 1);
    let ult = if (a as u32) < (b as u32) { -1i32 } else { 0 };
    let uge = if (a as u32) >= (b as u32) { -1i32 } else { 0 };
    let r4 = (ult & 1) | (uge & 2);
    [r1 as f32, r2 as f32, r3 as f32, r4 as f32]
}

#[test]
fn el_sm5_con_switch_y_enteros() {
    let ps = sm5(crate::dxil::ejemplos::sm5_switch);
    let pe = sm5(crate::dxil::ejemplos::sm5_enteros);
    let entradas = [(0.5, 7.9), (1.9, 3.0), (2.2, 9.5), (3.7, 1e10), (4.0, 2.0), (-1.5, -3.0), (-7.0, 0.0), (1e10, 5.0), (-1e10, 1.0), (f32::NAN, f32::NAN)];
    for (x, y) in entradas {
        let s = correr2(&ps, x, y);
        let r = sm5_ref_switch(x, y);
        assert_eq!([s[0].to_bits(), s[1].to_bits()], [r[0].to_bits(), r[1].to_bits()], "switch ({x}, {y}): {s:?} y {r:?}");
        let s = correr2(&pe, x, y);
        let r = sm5_ref_enteros(x, y);
        for k in 0..4 {
            assert_eq!(s[k].to_bits(), r[k].to_bits(), "enteros ({x}, {y}): {s:?} y {r:?}");
        }
    }
}

// -- E6d: la division y el resto ----------------------------------------------

const DXIL_DIVISION: &[u8] = include_bytes!("../prueba/division.dxil");

/// `division.hlsl`, en Rust: `n` es la fila 1 del cbuffer, como enteros.
/// Entre 0 da todo unos (lo de la casa); `i32::MIN / -1`, `i32::MIN`.
fn division_ref(x: f32, y: f32, n: [i32; 4]) -> [f32; 4] {
    let div = |a: u32, b: u32| a.checked_div(b).unwrap_or(u32::MAX);
    let rem = |a: u32, b: u32| a.checked_rem(b).unwrap_or(u32::MAX);
    let a = (x * 1000.0) as i32;
    let ua = (y * 100000.0) as u32;
    let (q, r) = if n[0] == 0 { (-1, -1) } else { (a.wrapping_div(n[0]), a.wrapping_rem(n[0])) };
    let (uq, ur) = (div(ua, n[1] as u32), rem(ua, n[1] as u32));
    let z = n[2] as u32;
    let (mut s, mut v, mut i) = (0u32, ua, 0);
    while i < 8 && v != 0 {
        s = s.wrapping_add(rem(v, z));
        v = div(v, z);
        i += 1;
    }
    [q as f32, r as f32, uq.wrapping_add(ur.wrapping_mul(3)) as f32, s as f32 + (a / 4) as f32]
}

#[test]
fn el_dxil_con_division_corre_como_su_hlsl() {
    let p = dxil(DXIL_DIVISION);
    let divisiones = p.ops.iter().filter(|o| matches!(o, Op::Entera { op: OpEntera::DivU | OpEntera::RemU | OpEntera::DivS | OpEntera::RemS, .. })).count();
    assert!(divisiones >= 7, "{divisiones}");
    let ns: [[i32; 4]; 4] = [[7, 13, 10, 0], [-3, 1, 2, 0], [1000, -1, 16, 0], [i32::MIN, 0x7FFF_FFFF, 3, 0]];
    let uv: [(f32, f32); 8] = [(0.1, 0.9), (0.3, 0.2), (-0.7, 0.7), (-0.004, 0.6), (0.99, 0.0), (1e6, 3.0), (-1e6, 42.95), (0.0, 0.125)];
    for n in ns {
        let mut cb: Vec<u8> = [0.0f32; 4].iter().flat_map(|v| v.to_le_bytes()).collect();
        cb.extend(n.iter().flat_map(|v| v.to_le_bytes()));
        for (x, y) in uv {
            let mut s = [[0.0f32; 4]; 1];
            p.correr(&[[0.0; 4], [x, y, 0.0, 0.0]], &cb, &mut s, &mut Vec::new());
            let r = division_ref(x, y, n);
            for k in 0..4 {
                assert_eq!(s[0][k].to_bits(), r[k].to_bits(), "uv ({x}, {y}) n {n:?}: {:?} y el hlsl {r:?}", s[0]);
            }
        }
    }
}

fn sm5_ref_division(x: f32, y: f32) -> [f32; 4] {
    let div = |a: u32, b: u32| a.checked_div(b).unwrap_or(u32::MAX);
    let rem = |a: u32, b: u32| a.checked_rem(b).unwrap_or(u32::MAX);
    let (a, b) = (x as u32, y as u32);
    let w = rem(b, 10).wrapping_add(div(b, a)).wrapping_add(rem(b, a));
    [div(a, b) as f32, rem(a, b) as f32, (a / 7) as f32, w as f32]
}

#[test]
fn el_sm5_con_udiv() {
    let p = sm5(crate::dxil::ejemplos::sm5_division);
    let entradas = [(100.0, 7.0), (7.0, 100.0), (0.0, 3.0), (5.0, 0.0), (0.0, 0.0), (4e9, 3.0), (4294967040.0, 65536.0), (-3.0, 2.0), (1e10, 1.0), (f32::NAN, 9.0)];
    for (x, y) in entradas {
        let s = correr2(&p, x, y);
        let r = sm5_ref_division(x, y);
        for k in 0..4 {
            assert_eq!(s[k].to_bits(), r[k].to_bits(), "udiv ({x}, {y}): {s:?} y {r:?}");
        }
    }
}

/// La VELOCIDAD (05-10): el traductor a x86-64 de los DIBUJOS ya sabe los
/// saltos y los enteros (por el cuerpo del computo), y `por_que_no` lo dice
/// igual que `compilar`: lo que traduce no tiene motivo, y lo que no, si.
/// (Sus bits, contra el interprete: `proton-x-casa/tests/nativo/saltos.rs`.)
#[test]
fn los_dibujos_con_saltos_se_traducen_y_dicen_por_que_no_cuando_no() {
    use crate::nativo::{compilar, por_que_no};
    for p in [si_sino(), bucle_geometrico(), bucle_entero(), anidado()] {
        assert!(p.salta());
        assert_eq!(por_que_no(&p), None);
        assert!(compilar(&p).is_some());
    }
    let con = |op: Op| {
        let mut p = si_sino();
        p.ops.insert(0, op);
        (por_que_no(&p), compilar(&p).is_some())
    };
    assert_eq!(con(Op::Descarta { c: 0 }), (None, true), "discard, tambien");
    assert_eq!(con(Op::LeeIndexado { d: 0, base: 0, n: 1, i: 0 }), (None, true), "y los arrays de registros");
    // X2 (05-10): la matematica, el cbuffer con fila calculada y las
    // texturas (por la llamada de la casa) ya no son motivo.
    assert_eq!(con(Op::Mate { d: 0, a: 0, f: crate::mates::Mate::Exp2 }), (None, true));
    assert_eq!(con(Op::ConstantesEn { d: 0, fila: 0, filas: 4, i: 0, cb: 0 }), (None, true));
    assert_eq!(con(Op::Muestra { d: 0, t: 0, s: 0, u: 0, v: 0, g: None }), (None, true));
    assert_eq!(con(Op::EligeTextura { i: 0, rango: 0 }), (None, true));
    assert_eq!(con(Op::IdHilo { d: 0, que: 0, c: 0 }), (Some("es de computo"), false), "lo del Contexto que un dibujo no pone");
    assert_eq!(con(Op::Barrera), (Some("es de computo"), false));
    assert_eq!(con(Op::Corta { flujo: 0 }), (Some("es de geometria"), false));
}
