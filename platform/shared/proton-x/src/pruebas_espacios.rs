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
    let m = Muestreador { filtro: Filtro::Punto, u: Direccion::Borde, v: Direccion::Borde, borde: [0.0; 4], comparacion: 0 };
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

const MATES_PS: &[u8] = include_bytes!("../prueba/mates.dxil");

/// *** N5.6: `mates.hlsl` (de `dxc`) usa las doce operaciones que pedian
/// los sombreadores de Cyberpunk en el metal (sin, cos, tan, exp2, log2,
/// frac, los cuatro redondeos y los medios floats). Corrido en la casa, da lo
/// que da la `libm` del anfitrion (dentro de los ULP que D3D permite).
#[test]
fn un_pixel_de_dxc_hace_la_matematica_que_faltaba() {
    extern crate std;
    use crate::dxil::programa::compilar;
    let ps = compilar(&dxil::leer(MATES_PS).unwrap()).unwrap();
    assert_eq!(ps.ops.iter().filter(|o| matches!(o, Op::Mate { .. })).count(), 12, "{:?}", ps.ops);
    let x = [1.25f32, -2.75, 0.5, 3.3];
    let u = 0x3C00u32; // el half de 1.0
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    ps.correr(&[[0.0; 4], x, [f32::from_bits(u), 0.0, 0.0, 0.0]], &[], &mut sal, &mut regs);
    let cerca = |a: f32, b: f32, que: &str| assert!((a - b).abs() <= 1e-6 * b.abs().max(1.0), "{que}: {a} vs {b}");
    cerca(sal[0][0], x[0].sin(), "sin");
    cerca(sal[0][1], x[1].cos(), "cos");
    cerca(sal[0][2], x[2].tan(), "tan");
    cerca(sal[0][3], x[3].exp2(), "exp2");
    cerca(sal[1][0], x[0].log2(), "log2");
    assert_eq!(sal[1][1], x[1] - x[1].floor(), "frac de HLSL");
    assert_eq!(sal[1][2], x[2].round_ties_even(), "round: 0.5 al par, 0");
    assert_eq!(sal[1][3], x[3].floor());
    assert_eq!(sal[2][0], x[0].ceil());
    assert_eq!(sal[2][1], x[1].trunc());
    assert_eq!(sal[2][2], 1.0, "f16tof32(0x3C00)");
    assert_eq!(sal[2][3].to_bits(), 0x3800, "f32tof16(0.5)");
}

const DESCARTE_PS: &[u8] = include_bytes!("../prueba/descarte.dxil");

/// *** N5.7: `descarte.hlsl` (de `dxc`): `clip(x - 0.5)` llega como
/// `discard(x < 0)` y el `discard` de un `if`, como `discard(true)` en su
/// rama. Corrido: el pixel queda solo si pasa los dos, y entonces sale el
/// color.
#[test]
fn un_pixel_de_dxc_se_tira_con_clip_y_discard() {
    use crate::dxil::programa::compilar;
    let ps = compilar(&dxil::leer(DESCARTE_PS).unwrap()).unwrap();
    assert_eq!(ps.ops.iter().filter(|o| matches!(o, Op::Descarta { .. })).count(), 2, "{:?}", ps.ops);
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    let mut corre = |x: f32, y: f32| ps.correr(&[[0.0; 4], [x, y, 0.0, 0.0]], &[], &mut sal, &mut regs);
    assert!(!corre(0.25, 0.5), "clip: x - 0.5 < 0");
    assert!(!corre(0.75, 0.9), "discard: y > 0.75");
    assert!(corre(0.5, 0.75), "en el borde de los dos, queda (clip tira solo si < 0)");
    assert_eq!(sal[0], [0.5, 0.75, 0.0, 1.0]);
}

/// El mismo `discard` en SM5 (`discard_nz`, `discard_z`), de `fxc`.
#[test]
fn el_sm5_tira_con_discard_nz_y_discard_z() {
    let (t, e, s) = crate::dxil::ejemplos::sm5_descarte();
    let p = crate::sm5::compilar(&t, &e, &s).unwrap();
    let mut sal = [[0f32; 4]; 1];
    let mut corre = |x: f32, y: f32| p.correr(&[[x, y, 0.0, 0.0]], &[], &mut sal, &mut Vec::new());
    assert!(!corre(0.25, 1.0), "discard_nz (x < 0.5)");
    assert!(!corre(0.75, 0.0), "discard_z (los bits de y, 0)");
    assert!(corre(0.75, -0.0), "-0.0 no son bits 0: queda");
    assert!(corre(0.75, 2.0));
    assert_eq!(sal[0][..2], [0.75, 2.0]);
}

/// En la trama, el pixel tirado no escribe NADA: ni color ni profundidad
/// (la Z se escribe despues del sombreador), y se cuenta en `tirados`.
#[test]
fn la_trama_no_escribe_el_pixel_tirado_ni_su_profundidad() {
    use crate::trama;
    let v = crate::pruebas::triangulo([1.0; 3], [0.0, 1.0, 1.0], true);
    let mut px = vec![0u32; 64];
    let mut z = vec![1.0f32.to_bits(); 64];
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: Some(trama::Profundidad { funcion: 2, escribir: true }), mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: true, z: Some(&mut z), cadena: false, otros: &mut [] };
    // Tira lo de atributo < 0.5 (cerca del vertice de (0,0)).
    let c = trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, |e, c| {
        c[0] = [1.0; 4];
        e[0][0] >= 0.5
    });
    assert_eq!(c.pixeles, 28);
    assert!(c.tirados > 0 && c.tirados < 28, "{c:?}");
    let pintados = px.iter().filter(|&&p| p != 0).count() as u64;
    let escritos = z.iter().filter(|&&b| b != 1.0f32.to_bits()).count() as u64;
    assert_eq!((pintados, escritos), (28 - c.tirados, 28 - c.tirados), "{c:?}");
    // Los mismos: donde hay color hay Z, y al reves.
    assert!(px.iter().zip(&z).all(|(&p, &b)| (p != 0) == (b != 1.0f32.to_bits())));
}

const OLAS_PS: &[u8] = include_bytes!("../prueba/olas.dxil");

/// *** `olas.hlsl` (de `dxc`): las olas y las derivadas que pedian los
/// sombreadores de Cyberpunk (83, 84, 118 en el metal) y sus hermanas, con
/// un pixel por ola: el valor mismo, el neutro del prefijo, un carril, y las
/// derivadas a 0 (ver `dxil/olas.rs`).
#[test]
fn un_pixel_de_dxc_con_olas_y_derivadas_corre_con_un_carril() {
    use crate::dxil::programa::compilar;
    let ps = compilar(&dxil::leer(OLAS_PS).unwrap()).unwrap();
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    let x = [0.25f32, 0.5, 3.0, 4.0];
    assert!(ps.correr(&[[0.0; 4], x, [f32::from_bits(7), 0.0, 0.0, 0.0]], &[], &mut sal, &mut regs));
    // a = x.x; b = x.y; n = 1 carril + prefijo 0 + indice 0, k = 1 (x.y > 0);
    // las derivadas, 0; y los cuatro booleanos, ciertos.
    assert_eq!(sal[0], [0.25, 0.5, 11.0, 1.0]);
    // Con x.y = 0: la cuenta de bits es 0; con x.z <= 1, AnyTrue es falso.
    ps.correr(&[[0.0; 4], [0.25, 0.0, 0.5, 4.0], [f32::from_bits(7), 0.0, 0.0, 0.0]], &[], &mut sal, &mut regs);
    assert_eq!(sal[0], [0.25, 0.0, 10.0, 0.0]);
}

const ARREGLOS_PS: &[u8] = include_bytes!("../prueba/arreglos.dxil");

/// *** N5.10: `arreglos.hlsl` (de `dxc`): alloca y getelementptr (lo que
/// pedian vertices y pixeles de Cyberpunk, `Instruccion(19)` y `(43)`), con
/// indices calculados, un bucle que escribe, y dos tablas globales (float e
/// int). Antes no compilaba.
#[test]
fn un_pixel_de_dxc_con_arrays_y_tablas() {
    use crate::dxil::programa::compilar;
    let ps = compilar(&dxil::leer(ARREGLOS_PS).unwrap()).unwrap();
    assert!(ps.ops.iter().any(|o| matches!(o, Op::LeeIndexado { .. })) && ps.ops.iter().any(|o| matches!(o, Op::EscribeIndexado { .. })));
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    let x = [1.0f32, 2.0, 3.0, 4.0];
    for i in 0u32..4 {
        ps.correr(&[[0.0; 4], [f32::from_bits(i), 0.0, 0.0, 0.0], x], &[], &mut sal, &mut regs);
        let mut a = x;
        a[(i & 3) as usize] += 1.0;
        let pesos = [0.1f32, 0.2, 0.3, 0.4];
        let mut s = 0.0f32;
        for j in 0..4 {
            s += a[j] * pesos[j];
        }
        let m = |f: u32, c: u32| ((f * 3 + c) * 10) as f32;
        let saltos = [5i32, -2, 7];
        let esperado = [a[((i + 1) & 3) as usize], s, m(i & 1, 2) + saltos[(i % 3) as usize] as f32, a[0]];
        assert_eq!(sal[0], esperado, "i = {i}");
    }
}

const LUCES_PS: &[u8] = include_bytes!("../prueba/luces.dxil");

/// *** `luces.hlsl` (de `dxc`): un array de un cbuffer con indice calculado
/// (`color[i & 7]`): la fila de `CBufferLoadLegacy` no es una constante.
/// Antes: "un operando que deberia ser un entero constante".
#[test]
fn un_cbuffer_se_lee_con_fila_calculada() {
    use crate::dxil::programa::compilar;
    let ps = compilar(&dxil::leer(LUCES_PS).unwrap()).unwrap();
    assert!(ps.ops.iter().any(|o| matches!(o, Op::ConstantesEn { .. })), "{:?}", ps.ops);
    // b0: 8 colores (el k, (k, 2k, 3k, 4k)) y `extra` (0.5).
    let mut cb = Vec::new();
    for k in 0..8u32 {
        for c in 1..=4u32 {
            cb.extend_from_slice(&((k * c) as f32).to_le_bytes());
        }
    }
    for _ in 0..4 {
        cb.extend_from_slice(&0.5f32.to_le_bytes());
    }
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    for i in [0u32, 3, 7, 12] {
        ps.correr(&[[0.0; 4], [f32::from_bits(i), 0.0, 0.0, 0.0]], &cb, &mut sal, &mut regs);
        let k = (i & 7) as f32;
        assert_eq!(sal[0], [k + 0.5, 2.0 * k + 0.5, 3.0 * k + 0.5, 4.0 * k + 0.5], "i = {i}");
    }
}

const SOMBRAS_PS: &[u8] = include_bytes!("../prueba/sombras.dxil");

/// *** `sombras.hlsl` (de `dxc`): SampleCmpLevelZero (el PCF de 2x2, con
/// LESS y referencia 0.5 sobre 0.2 0.8 / 0.4 0.6: pasan dos de cuatro, a
/// pesos iguales en el centro: 0.5), GatherRed (los cuatro rojos, en el
/// orden de D3D) y firstbitlow/high y countbits de 88 (0b1011000).
#[test]
fn las_sombras_comparan_y_gather_junta() {
    use crate::dxil::programa::compilar;
    use crate::textura::{Direccion, Filtro, Muestreador, Recursos, Textura};
    let ps = compilar(&dxil::leer(SOMBRAS_PS).unwrap()).unwrap();
    // 2x2 RGBA8: el rojo de cada texel (de arriba a la izquierda, por filas).
    let rojos = [51u32, 204, 102, 153];
    let pix: Vec<u32> = rojos.iter().map(|&r| 0xFF00_0000 | r).collect();
    let tex = Textura::rgba(&pix, 2, 2, false);
    let reg = |v: &[crate::dxil::programa::Lugar], r: u32| v.iter().position(|l| l.registro == r).unwrap();
    let mut texturas = vec![None, None];
    texturas[reg(&ps.ranuras.texturas, 0)] = Some(tex);
    texturas[reg(&ps.ranuras.texturas, 1)] = Some(tex);
    let mut mues = vec![None, None];
    mues[reg(&ps.ranuras.muestreadores, 0)] = Some(Muestreador { filtro: Filtro::Lineal, u: Direccion::Sujetar, v: Direccion::Sujetar, borde: [0.0; 4], comparacion: 2 });
    mues[reg(&ps.ranuras.muestreadores, 1)] = Some(Muestreador { filtro: Filtro::Punto, u: Direccion::Sujetar, v: Direccion::Sujetar, borde: [0.0; 4], comparacion: 0 });
    let rec = Recursos { texturas: &texturas, muestreadores: &mues, buferes: &[] };
    let (mut sal, mut regs) = (vec![[0f32; 4]; ps.salidas], Vec::new());
    ps.correr_con(&[[0.0; 4], [0.5, 0.5, 0.0, 0.0], [f32::from_bits(88), 0.0, 0.0, 0.0]], &[], &rec, &mut sal, &mut regs);
    let r = |k: usize| rojos[k] as f32 / 255.0;
    // Gather: x (0,1), y (1,1), z (1,0), w (0,0).
    let g = r(2) + r(3) * 10.0 + r(1) * 100.0 + r(0) * 1000.0;
    assert_eq!(sal[0][0], 0.5, "el PCF: dos de cuatro");
    assert!((sal[0][1] - g).abs() < 1e-3, "{} vs {g}", sal[0][1]);
    assert_eq!(sal[0][2..], [3.0 + 6.0 * 100.0, 3.0], "firstbitlow 3, firstbithigh 6, countbits 3");
    // En la esquina de arriba a la izquierda, sujeto: los cuatro son el
    // (0,0) = 0.2, y 0.5 < 0.2 no pasa.
    ps.correr_con(&[[0.0; 4], [0.0, 0.0, 0.0, 0.0], [f32::from_bits(0), 0.0, 0.0, 0.0]], &[], &rec, &mut sal, &mut regs);
    assert_eq!(sal[0][0], 0.0);
}
