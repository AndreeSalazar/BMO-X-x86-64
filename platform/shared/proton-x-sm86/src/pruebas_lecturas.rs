//! ** E8g (09-10, DL17): `SampleLevel` y `Load` en la 3060 (`lecturas.rs`)
//! -- en 2D, 3D y array de 2D, con mips, capas y rebanadas --: el emisor, el
//! juez (R0..R6 y R7 con las asas del kernel) y el simulador, contra el
//! interprete de la casa, bit a bit.
//!
//! El simulador lee como la 3060 (`LecturaTex`): el muestreador de la casa
//! hace de su unidad de texturas -- con la capa ya ENTERA, sujeta a las de la
//! vista, y un Load fuera, 0 --. Lo que se prueba es lo que el emisor pone:
//! que registro lleva que, el nivel, y la capa de un `SampleLevel`
//! (`suelo(z + 0.5)` sujeta) con FADD, F2I y dos IMNMX.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con_asas, juzgar_cuerpo_de_app, juzgar_drenado, Contexto, Regla, RESERVADOS};
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Lectura, Op, Programa};
use bmo_proton_x::dxil::ranuras::Lugar;
use bmo_proton_x::textura::{Clase, Como, Direccion, Filtro, Lod, Muestreador, Recursos, Textura};

use crate::simula::{correr, LecturaTex, Maquina};
use crate::{emitir_con, Abi, Emitido, NoEmite, Precarga};

const TECHO: u32 = 64;

fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// Los texeles de una textura de `capas` capas, `mips` mips y `hondo`
/// rebanadas (3D) de `ancho` x `alto`: cada uno distinto.
fn texeles(ancho: u32, alto: u32, hondo: u32, capas: u32, mips: u32) -> Vec<u32> {
    let mut v = Vec::new();
    for c in 0..capas {
        for m in 0..mips {
            let (w, h, d) = ((ancho >> m).max(1), (alto >> m).max(1), (hondo >> m).max(1));
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        v.push((x * 37 + c * 11) & 0xFF | ((y * 53 + m * 70) & 0xFF) << 8 | ((z * 29 + c * 90) & 0xFF) << 16 | (0x80 + m * 30 + c) << 24);
                    }
                }
            }
        }
    }
    v
}

fn textura(t: &[u32], clase: Clase, ancho: u32, alto: u32, hondo: u32, capas: u32, mips: u32) -> Textura<'_> {
    Textura { texeles: t, ancho, alto, como: Como::Rgba8, srgb: false, mapeo: Textura::MAPEO, mips, capas, hondo, clase, mip: 0, capa: 0, niveles: u32::MAX, lod_min: 0.0, vista: None }
}

/// `d = tN.<como>(c, nivel)`: las coordenadas y el nivel de la entrada 0 y 1.
fn programa(como: Lectura, especie: u32) -> Programa {
    let mut ops: Vec<Op> = (0..4).map(|k| Op::Entrada { d: k as u16, elemento: 0, componente: k as u8 }).collect();
    ops.push(Op::Entrada { d: 4, elemento: 1, componente: 0 });
    ops.push(Op::Lee { d: 10, t: 0, s: 0, como, c: [0, 1, 2, 3], nivel: 4, desp: [0; 3] });
    ops.extend((0..4).map(|k| Op::Salida { s: 10 + k as u16, elemento: 0, componente: k as u8 }));
    let mut p = ejemplos::programa(ops, 14, &[]);
    p.entradas = 2;
    let l = Lugar { espacio: 0, registro: 0, vista: 0 };
    p.ranuras.texturas = vec![l];
    p.ranuras.muestreadores = vec![l];
    p.ranuras.formas = vec![(0, 0, 0, especie)];
    p
}

/// Emitido y juzgado: R0..R6 y R7 con las asas que pone el kernel.
fn emitido(p: &Programa) -> Emitido {
    let e = emitir_con(p, TECHO, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    let v = juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None });
    assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    let asas = crate::pso::cargas(&e).iter().fold(0u64, |m, c| match *c {
        bmo_gpu_ga10x::pegamento::Carga::Asa { reg, .. } => m | 1 << reg,
        _ => m,
    });
    assert_eq!(juzgar_cuerpo_con_asas(&e.codigo, e.registros, asas), Ok(()));
    assert_eq!(juzgar_cuerpo_de_app(&e.codigo, e.registros).unwrap_err().regla, Regla::R7CuerpoAjeno, "sin asas, no");
    e
}

/// La casa y la 3060 de mentira, con las mismas entradas: los mismos bits.
fn igual(p: &Programa, e: &Emitido, rec: &Recursos, entradas: &[[f32; 4]; 2], enteros: bool) {
    let mut casa = vec![[0.0f32; 4]; p.salidas];
    let mut regs = Vec::new();
    p.correr_con(entradas, &[], rec, &mut casa, &mut regs);
    let pares = crate::pso::texturas_de(e);
    let leer = |l: &LecturaTex| -> [u32; 4] {
        let k = (l.asa & 0xF_FFFF) as usize;
        assert_eq!(l.asa, bmo_gpu_ga10x::texturas::asa(k as u32, k as u32), "el asa de la textura {k}");
        let (t, s) = pares[k];
        let n = l.nivel.expect("con .LL");
        match (l.dim, l.carga) {
            (1, false) => rec.muestrear_en(t, s, [f(l.c[0]), f(l.c[1]), 0.0, 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            (2, false) => rec.muestrear_en(t, s, [f(l.c[0]), f(l.c[1]), f(l.c[2]), 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            // La capa, ya entera: la 3060 la sujeta a las de la vista.
            (5, false) => rec.muestrear_en(t, s, [f(l.c[1]), f(l.c[2]), l.c[0] as f32, 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            (1, true) => rec.cargar(t, [l.c[0] as i32, l.c[1] as i32, 0], n as i32, [0; 3], enteros),
            (5, true) => rec.cargar(t, [l.c[1] as i32, l.c[2] as i32, l.c[0] as i32], n as i32, [0; 3], enteros),
            otra => panic!("una forma que no se emite: {otra:?}"),
        }
    };
    let mut m = Maquina::nueva([&[]; 8]);
    m.leer_textura = Some(&leer);
    for (i, r) in m.r.iter_mut().enumerate() {
        *r = 0x7FC0_0000 | i as u32;
    }
    for &q in &e.precargas {
        match q {
            Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = entradas[elemento as usize][componente as usize & 3].to_bits(),
            Precarga::Asa { textura, muestreador, reg } | Precarga::AsaPar { textura, muestreador, reg } => {
                let k = pares.iter().position(|&x| x == (textura, muestreador)).unwrap() as u32;
                m.r[reg as usize] = bmo_gpu_ga10x::texturas::asa(k, k);
            }
            Precarga::Fila { .. } => panic!("sin cbuffer"),
        }
    }
    correr(&e.codigo, &mut m).unwrap_or_else(|x| panic!("{x:?}"));
    for k in 0..4 {
        assert_eq!(m.r[k], casa[0][k].to_bits(), "canal {k} con {entradas:?}: la 3060 {:#x} y la casa {:#x}", m.r[k], casa[0][k].to_bits());
    }
}

fn muestreadores() -> [Muestreador; 3] {
    let lod = |mip| Lod { min: None, mip, sesgo: 0.0, minimo: 0.0, maximo: f32::MAX };
    [
        Muestreador { filtro: Filtro::Punto, u: Direccion::Repetir, v: Direccion::Espejo, borde: [0.0; 4], comparacion: 0, lod: lod(Filtro::Punto) },
        Muestreador { filtro: Filtro::Lineal, u: Direccion::Sujetar, v: Direccion::Repetir, borde: [0.0; 4], comparacion: 0, lod: lod(Filtro::Lineal) },
        Muestreador { filtro: Filtro::Lineal, u: Direccion::Borde, v: Direccion::Borde, borde: [0.25, 0.5, 0.75, 1.0], comparacion: 0, lod: Lod { min: Some(Filtro::Punto), mip: Filtro::Punto, sesgo: 0.5, minimo: 0.5, maximo: 2.0 } },
    ]
}

const NIVELES: [f32; 9] = [-1.0, 0.0, 0.3, 0.5, 1.0, 1.7, 2.0, 3.5, 10.0];
const UV: [(f32, f32); 6] = [(0.1, 0.2), (0.5, 0.5), (0.93, 0.07), (1.3, -0.4), (0.0625, 0.9375), (-2.2, 7.01)];

/// ** `SampleLevel` en 2D, con cuatro mips: cada nivel, cada muestreador.
#[test]
fn sample_level_en_2d() {
    let p = programa(Lectura::Nivel, 2);
    let e = emitido(&p);
    assert_eq!(e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x361).count(), 1, "un TEX");
    let t = texeles(16, 8, 1, 1, 4);
    let tx = [Some(textura(&t, Clase::Plana, 16, 8, 1, 1, 4))];
    for m in muestreadores() {
        let ms = [Some(m)];
        let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
        for &(u, v) in &UV {
            for &n in &NIVELES {
                igual(&p, &e, &rec, &[[u, v, 0.0, 0.0], [n, 0.0, 0.0, 0.0]], false);
            }
        }
    }
}

/// ** `SampleLevel` en 3D: u, v, w y el nivel.
#[test]
fn sample_level_en_3d() {
    let p = programa(Lectura::Nivel, 4);
    let e = emitido(&p);
    let t = texeles(8, 8, 8, 1, 3);
    let tx = [Some(textura(&t, Clase::Volumen, 8, 8, 8, 1, 3))];
    for m in muestreadores() {
        let ms = [Some(m)];
        let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
        for &(u, v) in &UV {
            for w in [0.0f32, 0.3, 0.51, 0.99, 1.4, -0.2] {
                for &n in &[0.0f32, 1.0, 1.6, 5.0] {
                    igual(&p, &e, &rec, &[[u, v, w, 0.0], [n, 0.0, 0.0, 0.0]], false);
                }
            }
        }
    }
}

/// ** `SampleLevel` en un ARRAY de 2D: la capa de la casa, `suelo(z + 0.5)`
/// sujeta, en todos sus bordes -- los medios, los negativos, el NaN, los
/// infinitos y los que se pasan de 16 bits --.
#[test]
fn sample_level_en_un_array() {
    let p = programa(Lectura::Nivel, 7);
    let e = emitido(&p);
    let t = texeles(8, 8, 1, 5, 3);
    let tx = [Some(textura(&t, Clase::Array, 8, 8, 1, 5, 3))];
    let capas = [-3.7f32, -0.5, -0.49999997, -0.0, 0.0, 0.49999997, 0.5, 1.5, 2.5, 3.49, 4.0, 4.5, 7.9, 100.0, 65535.6, 70000.0, 1e10, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    for m in muestreadores() {
        let ms = [Some(m)];
        let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
        for &z in &capas {
            for &(u, v) in &UV[..3] {
                for &n in &[0.0f32, 1.3] {
                    igual(&p, &e, &rec, &[[u, v, z, 0.0], [n, 0.0, 0.0, 0.0]], false);
                }
            }
        }
    }
}

/// ** `Load` en 2D y en un array: dentro, en los bordes y fuera -- en x, en
/// y, en la mip y en la capa, tambien negativos --: fuera, 0.
#[test]
fn load_en_2d_y_en_un_array() {
    let i = |x: i32| f32::from_bits(x as u32);
    for (especie, clase, capas) in [(2u32, Clase::Plana, 1u32), (7, Clase::Array, 4)] {
        let p = programa(Lectura::Carga { enteros: false }, especie);
        let e = emitido(&p);
        assert_eq!(e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x367).count(), 1, "un TLD");
        let t = texeles(8, 4, 1, capas, 3);
        let tx = [Some(textura(&t, clase, 8, 4, 1, capas, 3))];
        let rec = Recursos { texturas: &tx, muestreadores: &[], buferes: &[], dinamicas: None };
        for x in [-1, 0, 3, 7, 8, 100] {
            for y in [-2, 0, 3, 4] {
                for mip in [-1, 0, 1, 2, 3] {
                    for capa in [-1, 0, 2, 3, 4, 70000] {
                        igual(&p, &e, &rec, &[[i(x), i(y), i(capa), 0.0], [i(mip), 0.0, 0.0, 0.0]], false);
                    }
                }
            }
        }
    }
}

/// ** Lo que NO va todavia, con su indice: el cubo, una lectura de una
/// textura sin forma declarada, un desplazamiento, y el Load de una 3D.
#[test]
fn lo_que_no_va_todavia() {
    for (como, especie) in [(Lectura::Nivel, 5u32), (Lectura::Nivel, 9), (Lectura::Carga { enteros: false }, 4), (Lectura::Nivel, 0)] {
        let p = programa(como, especie);
        assert_eq!(emitir_con(&p, TECHO, Abi::Registros).err(), Some(NoEmite::Operacion(5)), "{como:?} de la especie {especie}");
    }
    let mut p = programa(Lectura::Nivel, 2);
    if let Op::Lee { desp, .. } = &mut p.ops[5] {
        *desp = [1, 0, 0];
    }
    assert_eq!(emitir_con(&p, TECHO, Abi::Registros).err(), Some(NoEmite::Operacion(5)), "con desplazamiento");
}

/// ** Dos lecturas de la MISMA textura: UN par (asa, nivel) -- el nivel de
/// la segunda espera la barrera de lectura del primer TEX --, UNA textura en
/// la receta, y los bits de la casa.
#[test]
fn dos_lecturas_una_textura() {
    let mut p = programa(Lectura::Nivel, 2);
    p.ops.insert(6, Op::Entrada { d: 5, elemento: 1, componente: 1 });
    p.ops.insert(7, Op::Lee { d: 14, t: 0, s: 0, como: Lectura::Nivel, c: [1, 0, 2, 3], nivel: 5, desp: [0; 3] });
    p.ops.push(Op::Salida { s: 14, elemento: 0, componente: 0 });
    p.iniciales.resize(18, 0.0);
    let e = emitido(&p);
    let pares: Vec<u8> = e.precargas.iter().filter_map(|q| if let Precarga::AsaPar { reg, .. } = *q { Some(reg) } else { None }).collect();
    assert_eq!(pares.len(), 1, "un par por (textura, muestreador)");
    assert!(pares[0] % 2 == 0);
    assert_eq!(crate::pso::texturas_de(&e), vec![(0u8, 0u8)], "una textura");
    // El nivel de la segunda, escrito en el MISMO Rb+1: espera la barrera
    // de LECTURA del primer TEX (el juez no lo veria: sin barrera de lectura
    // da la lectura por hecha al salir).
    let tex: Vec<usize> = (0..e.codigo.len()).filter(|&k| e.codigo[k].0 & 0xFFF == 0x361).collect();
    assert_eq!(tex.len(), 2);
    let leida = e.codigo[tex[0]].1 >> 49 & 7;
    assert!(leida < 6, "el primer TEX enciende su barrera de lectura");
    let nivel = (tex[0] + 1..tex[1]).find(|&k| e.codigo[k].0 >> 16 & 0xFF == pares[0] as u64 + 1).expect("el MOV del nivel");
    assert!((tex[0] + 1..=nivel).any(|k| e.codigo[k].1 >> 52 & 1 << leida != 0), "lo espera antes de pisar el nivel");
    let t = texeles(16, 8, 1, 1, 4);
    let tx = [Some(textura(&t, Clase::Plana, 16, 8, 1, 1, 4))];
    for m in muestreadores() {
        let ms = [Some(m)];
        let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
        for &(u, v) in &UV {
            for (k, &n) in NIVELES.iter().enumerate() {
                igual(&p, &e, &rec, &[[u, v, 0.0, 0.0], [n, NIVELES[(k + 4) % 9], 0.0, 0.0]], false);
            }
        }
    }
}

/// *** POR LA PUERTA del kernel: cada forma con el pegamento de pixel del
/// driver (el MOV de cada asa) y el juez de programas ENTEROS (R0..R6: las
/// barreras del TEX, del F2I de la capa y del pegamento juntas); y cabe.
#[test]
fn cada_forma_por_la_puerta() {
    for (como, especie) in [(Lectura::Nivel, 2u32), (Lectura::Nivel, 4), (Lectura::Nivel, 7), (Lectura::Carga { enteros: false }, 2), (Lectura::Carga { enteros: false }, 7)] {
        let p = programa(como, especie);
        let n = crate::pruebas_indexado::por_la_puerta(&p, &[Some(0), Some(1)]).unwrap_or_else(|x| panic!("{como:?} {especie}: {x:?}"));
        std::eprintln!("{como:?} de la especie {especie}: {n} instrucciones pegado");
    }
}

const NIVELES_DXIL: &[u8] = include_bytes!("../../proton-x/prueba/niveles.dxil");

/// *** `niveles.hlsl` de `dxc` de punta a punta: el lector (y la FORMA de
/// cada textura, de su PSV0), el emisor -- tres TEX y dos TLD --, el juez y el
/// simulador, con los bits de la casa; y por la puerta del kernel.
#[test]
fn niveles_de_dxc_en_la_3060() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(NIVELES_DXIL).unwrap()).unwrap();
    let especies: Vec<Option<u32>> = p.ranuras.texturas.iter().map(|l| p.ranuras.forma(*l)).collect();
    let por_registro = |r: u32| p.ranuras.texturas.iter().position(|l| l.registro == r).unwrap();
    assert_eq!([especies[por_registro(0)], especies[por_registro(1)], especies[por_registro(2)]], [Some(2), Some(4), Some(7)], "t0 2D, t1 3D, t2 array");
    let e = emitido(&p);
    assert_eq!((e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x361).count(), e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x367).count()), (3, 2), "tres TEX y dos TLD");
    let (t0, t1, t2) = (texeles(16, 8, 1, 1, 4), texeles(8, 8, 8, 1, 3), texeles(8, 8, 1, 4, 3));
    let mut tx = vec![None; 3];
    tx[por_registro(0)] = Some(textura(&t0, Clase::Plana, 16, 8, 1, 1, 4));
    tx[por_registro(1)] = Some(textura(&t1, Clase::Volumen, 8, 8, 8, 1, 3));
    tx[por_registro(2)] = Some(textura(&t2, Clase::Array, 8, 8, 1, 4, 3));
    let i = |x: i32| f32::from_bits(x as u32);
    for m in muestreadores() {
        let ms = [Some(m)];
        let rec = Recursos { texturas: &tx, muestreadores: &ms, buferes: &[], dinamicas: None };
        for &(u, v) in &UV {
            for (k, &n) in NIVELES.iter().enumerate() {
                let z = [-0.7f32, 0.0, 0.49999997, 0.5, 1.5, 2.6, 3.4, 9.0, f32::NAN][k];
                let ent = [[0.0; 4], [u, v, z, n], [i(k as i32 % 9 - 1), i(k as i32 % 5 - 1), i(k as i32 % 6 - 1), i(k as i32 % 4 - 1)]];
                igual_tres(&p, &e, &rec, &ent);
            }
        }
    }
    let n = crate::pruebas_indexado::por_la_puerta(&p, &[None, Some(0), Some(1)]).unwrap_or_else(|x| panic!("{x:?}"));
    std::eprintln!("niveles.hlsl: {} instrucciones de cuerpo, {n} pegado, {} registros", e.codigo.len(), e.registros);
}

/// [`igual`] con tres entradas (SV_Position, sin leer, delante).
fn igual_tres(p: &Programa, e: &Emitido, rec: &Recursos, entradas: &[[f32; 4]; 3]) {
    let mut casa = vec![[0.0f32; 4]; p.salidas];
    let mut regs = Vec::new();
    p.correr_con(entradas, &[], rec, &mut casa, &mut regs);
    let pares = crate::pso::texturas_de(e);
    let leer = |l: &LecturaTex| -> [u32; 4] {
        let (t, s) = pares[(l.asa & 0xF_FFFF) as usize];
        let n = l.nivel.expect("con .LL");
        match (l.dim, l.carga) {
            (1, false) => rec.muestrear_en(t, s, [f(l.c[0]), f(l.c[1]), 0.0, 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            (2, false) => rec.muestrear_en(t, s, [f(l.c[0]), f(l.c[1]), f(l.c[2]), 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            (5, false) => rec.muestrear_en(t, s, [f(l.c[1]), f(l.c[2]), l.c[0] as f32, 0.0], Some(f(n)), [0; 3]).map(f32::to_bits),
            (1, true) => rec.cargar(t, [l.c[0] as i32, l.c[1] as i32, 0], n as i32, [0; 3], false),
            (5, true) => rec.cargar(t, [l.c[1] as i32, l.c[2] as i32, l.c[0] as i32], n as i32, [0; 3], false),
            otra => panic!("{otra:?}"),
        }
    };
    let mut m = Maquina::nueva([&[]; 8]);
    m.leer_textura = Some(&leer);
    for (i, r) in m.r.iter_mut().enumerate() {
        *r = 0x7FC0_0000 | i as u32;
    }
    for &q in &e.precargas {
        match q {
            Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = entradas[elemento as usize][componente as usize & 3].to_bits(),
            Precarga::Asa { textura, muestreador, reg } | Precarga::AsaPar { textura, muestreador, reg } => {
                let k = pares.iter().position(|&x| x == (textura, muestreador)).unwrap() as u32;
                m.r[reg as usize] = bmo_gpu_ga10x::texturas::asa(k, k);
            }
            Precarga::Fila { .. } => panic!("sin cbuffer"),
        }
    }
    correr(&e.codigo, &mut m).unwrap_or_else(|x| panic!("{x:?}"));
    for k in 0..4 {
        assert!(crate::pruebas::mismos(m.r[k], casa[0][k]), "canal {k} con {entradas:?}: la 3060 {:#x} y la casa {:#x}", m.r[k], casa[0][k].to_bits());
    }
}
