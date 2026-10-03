//! Las pruebas de N5.1 (03-10): los ESPACIOS de registros y los registros
//! altos (`t40, space1`), del sombreador de `dxc` a la ranura del enlace.
//! Donde cae cada ranura en la root signature lo prueba `donde.rs`.

use alloc::vec;
use alloc::vec::Vec;

use crate::dxil::{self, programa::Op};
use crate::pruebas::TEXTURA_VS;

const ESPACIOS_PS: &[u8] = include_bytes!("../prueba/espacios.dxil");

/// *** `espacios.hlsl` (de `dxc`): t3, t40 de space1 y el array t7..t10 de
/// space2 (lee c[2], el t9), con s2 y s20 de space1. Cosido al de vertices
/// de `textura.hlsl`, cada ranura del enlace dice su lugar (y la etapa que
/// lo lee), y corrido con UNA sola textura puesta cada vez, sale esa: la
/// ranura lleva a SU textura, no a la de al lado.
#[test]
fn el_enlace_da_a_cada_espacio_su_ranura() {
    use crate::donde::VISTA_PIXELES;
    use crate::dxil::programa::Lugar;
    use crate::lote::{self, ElementoIa};
    use crate::textura::{Direccion, Filtro, Muestreador, Recursos, Textura};
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(ESPACIOS_PS).unwrap());
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde };
    let en = lote::enlazar(&vs, &ps, &[e("POSITION", 0), e("TEXCOORD", 16)]).unwrap();
    let l = |espacio, registro| Lugar { espacio, registro, vista: VISTA_PIXELES };
    let mut t = en.ranuras.texturas.clone();
    t.sort_by_key(|x| (x.espacio, x.registro));
    assert_eq!(t, [l(0, 3), l(1, 40), l(2, 9)], "c[2] del array t7 es el t9");
    let mut s = en.ranuras.muestreadores.clone();
    s.sort_by_key(|x| (x.espacio, x.registro));
    assert_eq!(s, [l(0, 2), l(1, 20)]);
    assert_eq!(en.ps.ranuras, en.ranuras, "el de pixeles cuenta con la tabla del enlace");

    // Tres colores, uno por textura (RGBA, sin alfa: se suman).
    let color = |x: Lugar| match (x.espacio, x.registro) {
        (0, 3) => 0x0000_00FFu32,
        (1, 40) => 0x0000_FF00,
        _ => 0x00FF_0000,
    };
    let pix: Vec<[u32; 1]> = en.ranuras.texturas.iter().map(|&x| [color(x)]).collect();
    let m = Muestreador { filtro: Filtro::Punto, u: Direccion::Borde, v: Direccion::Borde, borde: [0.0; 4] };
    let mue: Vec<Option<Muestreador>> = en.ranuras.muestreadores.iter().map(|_| Some(m)).collect();
    let cb = [0u8; 16];
    let (mut sal, mut regs) = (vec![[0f32; 4]; en.ps.salidas], Vec::new());
    for (k, &lugar) in en.ranuras.texturas.iter().enumerate() {
        let c = color(lugar);
        let esperado = [(c & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, ((c >> 16) & 0xFF) as f32 / 255.0, 0.0];
        let tex: Vec<Option<Textura>> = (0..pix.len()).map(|i| (i == k).then(|| Textura::rgba(&pix[i], 1, 1, false))).collect();
        let rec = Recursos { texturas: &tex, muestreadores: &mue };
        en.ps.correr_con(&[[0.0; 4], [0.5, 0.5, 0.0, 0.0]], &cb, &rec, &mut sal, &mut regs);
        assert_eq!(sal[0], esperado, "solo la textura de {lugar:?}");
    }
}

/// *** Unir las ranuras de dos etapas: las del de vertices se quedan, las
/// del de pixeles van detras (o a la misma, si es el mismo lugar de la misma
/// etapa), y sus operaciones se renumeran.
#[test]
fn unir_ranuras_renumera_las_del_de_pixeles() {
    use crate::dxil::programa::{Lugar, Programa, Ranuras};
    let l = |espacio, registro, vista| Lugar { espacio, registro, vista };
    let mut todas = Ranuras { texturas: vec![l(0, 0, 1), l(1, 5, 0)], muestreadores: vec![l(0, 0, 1)] };
    let ps = Ranuras { texturas: vec![l(0, 0, 5), l(1, 5, 0)], muestreadores: vec![l(0, 0, 5)] };
    let (t, s) = todas.unir(&ps).unwrap();
    assert_eq!((t.as_slice(), s.as_slice()), (&[2u8, 1][..], &[1u8][..]), "el t0 del pixel es otro; el t5 sin etapa, el mismo");
    let mut p = Programa { ops: vec![Op::Muestra { d: 0, t: 0, s: 0, u: 4, v: 5 }, Op::Muestra { d: 0, t: 1, s: 0, u: 4, v: 5 }], iniciales: vec![0.0; 6], entradas: 0, salidas: 0, lee: 0, filas_cb: 0, ranuras: ps };
    p.renumerar(&t, &s);
    assert!(matches!(p.ops[..], [Op::Muestra { t: 2, s: 1, .. }, Op::Muestra { t: 1, s: 1, .. }]), "{:?}", p.ops);
    // Mas de 256 lugares distintos: no cabe en un u8, se dice.
    let mut llena = Ranuras { texturas: (0..256).map(|r| l(0, r, 0)).collect(), muestreadores: vec![] };
    assert!(llena.unir(&Ranuras { texturas: vec![l(9, 9, 0)], muestreadores: vec![] }).is_err());
}
