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
