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
        let rec = Recursos { texturas: &tex, muestreadores: &mue, buferes: &[] };
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
    let mut todas = Ranuras { texturas: vec![l(0, 0, 1), l(1, 5, 0)], muestreadores: vec![l(0, 0, 1)], cbuffers: vec![l(0, 0, 1)] };
    let ps = Ranuras { texturas: vec![l(0, 0, 5), l(1, 5, 0)], muestreadores: vec![l(0, 0, 5)], cbuffers: vec![l(0, 3, 5)] };
    let m = todas.unir(&ps).unwrap();
    assert_eq!(m.texturas, [2, 1], "el t0 del pixel es otro; el t5 sin etapa, el mismo");
    assert_eq!((m.muestreadores.as_slice(), m.cbuffers.as_slice()), (&[1u8][..], &[1u8][..]));
    let mut p = Programa {
        ops: vec![Op::Muestra { d: 0, t: 0, s: 0, u: 4, v: 5 }, Op::Muestra { d: 0, t: 1, s: 0, u: 4, v: 5 }, Op::Constantes { d: 6, fila: 2, cb: 0 }],
        iniciales: vec![0.0; 10],
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 3,
        ranuras: ps,
    };
    p.renumerar(&m);
    assert!(matches!(p.ops[..], [Op::Muestra { t: 2, s: 1, .. }, Op::Muestra { t: 1, s: 1, .. }, Op::Constantes { cb: 1, .. }]), "{:?}", p.ops);
    // Aplanado: el cbuffer 1 empieza en la fila 7.
    p.aplanar(&[0, 7]);
    assert!(matches!(p.ops[2], Op::Constantes { fila: 9, cb: 1, .. }));
    assert_eq!(p.filas_cb, 10);
    // Mas de 256 lugares distintos: no cabe en un u8, se dice.
    let mut llena = Ranuras { texturas: (0..256).map(|r| l(0, r, 0)).collect(), ..Default::default() };
    assert!(llena.unir(&Ranuras { texturas: vec![l(9, 9, 0)], ..Default::default() }).is_err());
}

const CBUFFERS_PS: &[u8] = include_bytes!("../prueba/cbuffers.dxil");

/// *** N5.2: `cbuffers.hlsl` (de `dxc`) lee b1 (su segunda fila), b2 de
/// space3 y b0. Cosido, cada uno es un bloque con las filas que se leen de
/// el; con las constantes juntadas en su sitio, da `a2 * 2 + b + k`, y un
/// cbuffer que falta se lee como 0.
#[test]
fn el_enlace_aplana_los_cbuffers_que_no_son_b0() {
    use crate::donde::VISTA_PIXELES;
    use crate::dxil::programa::Lugar;
    use crate::lote::{self, Bloque, ElementoIa};
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(CBUFFERS_PS).unwrap());
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde };
    let en = lote::enlazar(&vs, &ps, &[e("POSITION", 0), e("TEXCOORD", 16)]).unwrap();
    let l = |espacio, registro| Lugar { espacio, registro, vista: VISTA_PIXELES };
    let filas: Vec<(Lugar, u16)> = en.ranuras.cbuffers.iter().zip(&en.constantes).map(|(&x, b)| (x, b.filas)).collect();
    let mut ordenadas = filas.clone();
    ordenadas.sort_by_key(|x| (x.0.espacio, x.0.registro));
    assert_eq!(ordenadas, [(l(0, 0), 1), (l(0, 1), 2), (l(3, 2), 1)], "b1 lee dos filas (a y a2)");
    // Uno detras de otro, sin huecos.
    let mut fila = 0;
    for b in &en.constantes {
        assert_eq!(b.fila, fila);
        fila += b.filas;
    }
    assert_eq!(en.ps.filas_cb, fila);
    let f4 = |v: [f32; 4]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let a: Vec<u8> = [f4([9.0; 4]), f4([1.0, 2.0, 3.0, 4.0])].concat();
    let (b, k) = (f4([10.0, 20.0, 30.0, 40.0]), f4([0.5; 4]));
    let datos = |x: Lugar| match (x.espacio, x.registro) {
        (0, 1) => &a[..],
        (3, 2) => &b[..],
        _ => &k[..],
    };
    let cb = lote::juntar_constantes(&en.constantes, |i| Some(datos(en.ranuras.cbuffers[i])));
    let (mut sal, mut regs) = (vec![[0f32; 4]; en.ps.salidas], Vec::new());
    en.ps.correr(&[[0.0; 4], [0.0; 4]], &cb, &mut sal, &mut regs);
    assert_eq!(sal[0], [12.5, 24.5, 36.5, 48.5]);
    // Sin el b2 de space3: lo suyo es 0.
    let sin_b = lote::juntar_constantes(&en.constantes, |i| (en.ranuras.cbuffers[i].espacio != 3).then(|| datos(en.ranuras.cbuffers[i])));
    en.ps.correr(&[[0.0; 4], [0.0; 4]], &sin_b, &mut sal, &mut regs);
    assert_eq!(sal[0], [2.5, 4.5, 6.5, 8.5]);
    // Y un cbuffer mas corto de lo que se lee: lo que falta, 0.
    let corto = lote::juntar_constantes(&[Bloque { fila: 1, filas: 2 }], |_| Some(&k[..]));
    assert_eq!(corto.len(), 48);
    assert_eq!(&corto[16..32], &k[..]);
    assert!(corto[..16].iter().chain(&corto[32..]).all(|&x| x == 0));
}

const BUFERES_PS: &[u8] = include_bytes!("../prueba/buferes.dxil");

/// *** N5.3: `buferes.hlsl` (de `dxc`) lee un `Buffer<float4>` (t0), un
/// `StructuredBuffer` de 20 bytes (t1, su `b`, a 4 bytes) y un
/// `ByteAddressBuffer` (t2 de space1, por bytes), y suma el GetDimensions del
/// estructurado. Cada uno es un bufer en SU ranura, y sin el, ceros.
#[test]
fn un_pixel_de_dxc_lee_los_tres_buferes() {
    use crate::bufer::Bufer;
    use crate::dxil::programa::{compilar, Lectura};
    use crate::textura::Recursos;
    let ps = compilar(&dxil::leer(BUFERES_PS).unwrap()).unwrap();
    assert_eq!(ps.ops.iter().filter(|o| matches!(o, Op::Lee { como: Lectura::Bufer(_), .. })).count(), 3, "{:?}", ps.ops);
    let f = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let tipado = f(&[0.0; 8].iter().copied().chain([1.0, 2.0, 3.0, 4.0]).collect::<Vec<f32>>());
    // Tres elementos de 20 bytes (a, b.xyzw); el 1 con b = (10, 20, 30, 40).
    let estructurado = f(&[0.0, 0.0, 0.0, 0.0, 0.0, 7.0, 10.0, 20.0, 30.0, 40.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let crudo = f(&[0.0, 0.0, 100.0, 200.0, 300.0, 400.0]);
    let bufer = |l: crate::dxil::programa::Lugar| match (l.espacio, l.registro) {
        (0, 0) => Bufer { bytes: &tipado, formato: 2, paso: 0, elementos: 3 },
        (0, 1) => Bufer { bytes: &estructurado, formato: 0, paso: 20, elementos: 3 },
        _ => Bufer { bytes: &crudo, formato: 0, paso: 0, elementos: 6 },
    };
    let buf: Vec<Option<Bufer>> = ps.ranuras.texturas.iter().map(|&l| Some(bufer(l))).collect();
    let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &buf };
    let i = [2u32, 1, 8, 0].map(f32::from_bits);
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    ps.correr_con(&[[0.0; 4], i], &[], &rec, &mut sal, &mut regs);
    assert_eq!(sal[0], [114.0, 225.0, 336.0, 447.0], "1 + 10 + 100 + 3 elementos...");
    // Sin buferes (SRV nulos): todo 0, y 0 elementos.
    ps.correr_con(&[[0.0; 4], i], &[], &Recursos::NINGUNO, &mut sal, &mut regs);
    assert_eq!(sal[0], [0.0; 4]);
}
