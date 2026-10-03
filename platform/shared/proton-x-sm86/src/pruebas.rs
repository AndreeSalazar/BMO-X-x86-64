//! E3: el emisor, contra la casa. Cada sombreador del cubo -- los DXIL de dxc
//! y los SM5 que FXC compilo para BMOX-12 -- se emite a SASS, se corre en el
//! simulador y se compara, BIT A BIT, con `Programa::correr`.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil::{self, programa::compilar, programa::Programa};

use crate::simula::{correr, Maquina};
use crate::{emitir, emitir_con, Abi, Emitido, NoEmite, Precarga};

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

/// Los mismos bits -- o los dos NaN: la carga util y el signo de un NaN no
/// se modelan (la 3060 da su NaN canonico; `FADD a, -b` no es `a - b` de la
/// CPU con un NaN en `b`), y D3D solo pide que sea NaN. Lo mismo dice el
/// banco de la casa (`corre.rs`) y `nativo`. E6 (02-10), con NaN de entrada.
pub(crate) fn mismos(sass: u32, casa: f32) -> bool {
    sass == casa.to_bits() || (f32::from_bits(sass).is_nan() && casa.is_nan())
}

/// Corre `p` en la casa y su SASS en el simulador con las mismas entradas y
/// el mismo cbuffer; los bits de cada salida tienen que ser los mismos.
pub(crate) fn igual(p: &Programa, codigo: &[(u64, u64)], entradas: &[[f32; 4]], cb: &[u8]) {
    let mut casa = std::vec![[0.0f32; 4]; p.salidas];
    let mut regs = Vec::new();
    p.correr(entradas, cb, &mut casa, &mut regs);
    let banco: Vec<u8> = entradas.iter().flat_map(|e| e.iter().flat_map(|x| x.to_le_bytes())).collect();
    let mut m = Maquina::nueva([&[], &banco, &[], cb, &[], &[], &[], &[]]);
    correr(codigo, &mut m).unwrap();
    for (e, s) in casa.iter().enumerate() {
        for k in 0..4 {
            assert!(mismos(m.r[4 * e + k], s[k]), "salida {e}.{k}: la 3060 {} y la casa {}", f32::from_bits(m.r[4 * e + k]), s[k]);
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
        // E4: el cuerpo cabe en 64 (la puerta del kernel es de 128 desde E5:
        // lo demas es para el pegamento del driver) y
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
    let p = Programa { ops: ops.into_iter().chain(salidas).collect(), iniciales, entradas: 1, salidas: 5, lee: 1, filas_cb: 1, ranuras: Default::default() };
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

/// E5: con [`Abi::Registros`] las entradas y las filas del cbuffer llegan YA
/// en los registros que dice `precargas` (como las dejara el LDG del
/// pegamento); el banco va VACIO, y los bits tienen que ser los de la casa.
pub(crate) fn igual_en_registros(p: &Programa, e: &Emitido, entradas: &[[f32; 4]], cb: &[u8]) {
    let mut casa = std::vec![[0.0f32; 4]; p.salidas];
    let mut regs = Vec::new();
    p.correr(entradas, cb, &mut casa, &mut regs);
    let mut m = Maquina::nueva([&[]; 8]);
    // Basura en todo lo demas: nadie puede leer un registro sin escribirlo.
    for (i, r) in m.r.iter_mut().enumerate() {
        *r = 0x7FC0_0000 | i as u32;
    }
    for &q in &e.precargas {
        match q {
            Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = entradas[elemento as usize][componente as usize & 3].to_bits(),
            Precarga::Fila { fila, reg } => {
                for k in 0..4 {
                    let o = 16 * fila as usize + 4 * k;
                    m.r[reg as usize + k] = cb.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                }
            }
            Precarga::Asa { .. } => panic!("un programa sin texturas no pide asas"),
        }
    }
    correr(&e.codigo, &mut m).unwrap();
    // Solo lo que el programa ESCRIBE (lo demas no se exporta: la SPH dice
    // que componentes salen).
    use bmo_proton_x::dxil::programa::Op;
    let escritas: Vec<(usize, usize)> = p.ops.iter().filter_map(|o| if let Op::Salida { elemento, componente, .. } = *o { Some((elemento as usize, componente as usize & 3)) } else { None }).collect();
    for (el, s) in casa.iter().enumerate() {
        for k in (0..4).filter(|&k| escritas.contains(&(el, k))) {
            assert!(mismos(m.r[4 * el + k], s[k]), "salida {el}.{k}: la 3060 {} y la casa {}", f32::from_bits(m.r[4 * el + k]), s[k]);
        }
    }
}

#[test]
fn con_las_entradas_en_registros_da_los_bits_de_la_casa() {
    for (d, vertice) in [(DXIL_VS, true), (SM5_VS, true), (DXIL_PS, false), (SM5_PS, false)] {
        let p = programa(d);
        let e = emitir_con(&p, TECHO, Abi::Registros).unwrap();
        // Nada lee un banco de constantes: todo llega en registros.
        assert!(e.codigo.iter().all(|&(lo, _)| !matches!(lo >> 9 & 7, 3 | 5)), "lee un banco");
        // Sin repetidos, y las filas alineadas a 4 (un LDG.128 cada una).
        for (i, a) in e.precargas.iter().enumerate() {
            if let Precarga::Fila { reg, .. } = a {
                assert_eq!(reg % 4, 0);
            }
            assert!(e.precargas[..i].iter().all(|b| core::mem::discriminant(a) != core::mem::discriminant(b) || a != b));
        }
        std::eprintln!("{} instrucciones, {} registros, {} precargas", e.codigo.len(), e.registros, e.precargas.len());
        assert!(e.codigo.len() <= 64 && e.registros <= TECHO, "{} instrucciones, {} registros", e.codigo.len(), e.registros);
        for f in [0u32, 30, 60, 123] {
            let cb = cb_de(f);
            let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
            for v in bmo_cubo::vertices() {
                let ent = if vertice {
                    [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color]
                } else {
                    let n = bmo_cubo::mat::transformar_dir(&c.world, v.normal);
                    [[0.0; 4], [n[0], n[1], n[2], 0.0], v.color]
                };
                igual_en_registros(&p, &e, &ent, &cb);
            }
        }
    }
}

/// ** P3b4c.8 T2b: el pixel de HelloTexture (`textura.hlsl`: `imagen.Sample(
/// muestreo, uv)`) sale como UN TEX con el asa que pone el kernel; y
/// simulado -- el TEX muestreando con la casa, que iguala a la 3060 -- da
/// los MISMOS bits que el interprete, con punto y con lineal.
#[test]
fn el_pixel_de_hellotexture_es_un_tex() {
    use bmo_proton_x::textura::{Direccion, Filtro, Muestreador, Recursos, Textura};
    let p = programa(include_bytes!("../../proton-x/prueba/textura_ps.dxil"));
    let e = emitir_con(&p, TECHO, Abi::Registros).unwrap();
    let texs = e.codigo.iter().filter(|w| w.0 & 0xFFF == 0x361).count();
    assert_eq!(texs, 1, "un Sample, un TEX");
    let asas: Vec<Precarga> = e.precargas.iter().copied().filter(|q| matches!(q, Precarga::Asa { .. })).collect();
    assert!(matches!(asas[..], [Precarga::Asa { textura: 0, muestreador: 0, .. }]), "{asas:?}");
    assert_eq!(crate::pso::texturas_de(&e), std::vec![(0u8, 0u8)]);
    // Lo que el juez del kernel dira del cuerpo: R7 con el asa del kernel.
    let asa_reg = match asas[0] { Precarga::Asa { reg, .. } => reg, _ => unreachable!() };
    assert_eq!(bmo_gpu_ga10x::sass::juez::juzgar_cuerpo_con_asas(&e.codigo, e.registros, 1 << asa_reg), Ok(()));
    let t: Vec<u32> = (0..64u32).map(|k| (k * 37 & 0xFF) | (k * 91 & 0xFF) << 8 | (255 - k * 3) << 16 | 0xFF << 24).collect();
    let tx = [Some(Textura::rgba(&t, 8, 8, false))];
    for filtro in [Filtro::Punto, Filtro::Lineal] {
        let ms = [Some(Muestreador { filtro, u: Direccion::Repetir, v: Direccion::Espejo, borde: [0.0; 4] })];
        let rec = Recursos { texturas: &tx, muestreadores: &ms };
        let mu = |asa: u32, u: f32, v: f32| {
            assert_eq!(asa, bmo_gpu_ga10x::texturas::asa(0, 0), "el asa de la textura 0");
            rec.muestrear(0, 0, u, v)
        };
        for &(u, v) in &[(0.1f32, 0.2f32), (0.5, 0.5), (0.93, 0.07), (1.3, -0.4), (0.0625, 0.9375)] {
            let entradas = [[0.0f32; 4], [u, v, 0.0, 0.0]];
            let mut casa = std::vec![[0.0f32; 4]; p.salidas];
            let mut regs = Vec::new();
            p.correr_con(&entradas, &[], &rec, &mut casa, &mut regs);
            let mut m = Maquina::nueva([&[]; 8]);
            m.muestrear = Some(&mu);
            for (i, r) in m.r.iter_mut().enumerate() {
                *r = 0x7FC0_0000 | i as u32;
            }
            for &q in &e.precargas {
                match q {
                    Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = entradas[elemento as usize][componente as usize & 3].to_bits(),
                    Precarga::Asa { reg, .. } => m.r[reg as usize] = bmo_gpu_ga10x::texturas::asa(0, 0),
                    Precarga::Fila { .. } => panic!("no lee cbuffer"),
                }
            }
            correr(&e.codigo, &mut m).unwrap();
            for k in 0..4 {
                assert_eq!(m.r[k], casa[0][k].to_bits(), "{filtro:?} ({u}, {v}) canal {k}");
            }
        }
    }
}

/// ** P3b4c.8 T3: la tabla contra la que `gpu verrano textura` juzga a la
/// 3060 (`texturas::prueba::ESPERADO`, la 3060 bajo CUDA en 8 bits) es
/// EXACTAMENTE lo que da el muestreador de la casa en esos 96 puntos.
#[test]
fn la_tabla_del_metal_es_la_de_la_casa() {
    use bmo_gpu_ga10x::texturas::prueba as pr;
    use bmo_proton_x::textura::{Direccion, Filtro, Muestreador, Textura};
    let t: Vec<u32> = (0..16u32).map(|i| (i % 4) * 60 | (i / 4 * 60) << 8 | ((i % 4 + i / 4) * 20) << 16 | 255 << 24).collect();
    let tx = Textura::rgba(&t, 4, 4, false);
    for (m, &(lineal, modo)) in pr::MUESTREADORES.iter().enumerate() {
        let d = [Direccion::Repetir, Direccion::Espejo, Direccion::Sujetar, Direccion::Borde][modo as usize - 1];
        let mu = Muestreador { filtro: if lineal { Filtro::Lineal } else { Filtro::Punto }, u: d, v: d, borde: pr::BORDE };
        for (k, &(u, v)) in pr::PUNTOS.iter().enumerate() {
            let c = tx.muestrear(&mu, u, v).map(|x| (x * 255.0 + 0.5) as u32);
            assert_eq!(c[0] | c[1] << 8 | c[2] << 16 | c[3] << 24, pr::ESPERADO[m][k], "{} ({u}, {v})", pr::NOMBRES[m]);
        }
    }
}
