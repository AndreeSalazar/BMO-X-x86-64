//! LI2d-HDR (12-10): el render target de float por la 3060. El censo de
//! Cyberpunk: 28 lotes de 34 los paraba "un render target de float". La
//! casa lo guarda en floats de 32 YA redondeados a su formato
//! (`formato_ia::cuantizar`, la CPU, la referencia); la 3060 escribe floats
//! de 32 y el pegamento redondea la salida (`pegamento::Redondeo`).

extern crate std;

use alloc::vec::Vec;

use bmo_gpu_ga10x::destino::Flotante;
use bmo_gpu_ga10x::pegamento::{self, Canal, Datos, Redondeo};
use bmo_proton_x::formato_ia::cuantizar;
use bmo_proton_x::lote::{self, Lote, Topologia};
use bmo_proton_x::dxil;

use crate::puerta::{de_flotante, Blanco, Puerta, Z_CON_FLOAT};
use crate::simula::{correr, Maquina};

/// DXGI: R32G32B32A32, R32G32B32, R16G16B16A16, R32G32, R16G16, R16 (FLOAT).
const FORMATOS: [u32; 6] = [2, 6, 10, 16, 34, 54];

/// El epilogo que EMITE el pegamento de pixel para `r` (las palabras de
/// detras del cuerpo: F2FP, HADD2 y los MOV a R0..R3), y su EXIT.
fn epilogo(r: Redondeo) -> Vec<(u64, u64)> {
    let cuerpo = [bmo_gpu_ga10x::cubo::mov(5, 0), bmo_gpu_ga10x::cubo::EXIT];
    let mut g = pegamento::Pegado::VACIO;
    pegamento::pixel_entero_en(&mut g, &cuerpo, 8, &[], Datos { filas: 0, paso: 0, elementos: &[] }, &[], None, r, None, 1).expect("se pega");
    let mut v: Vec<(u64, u64)> = g
        .codigo()
        .iter()
        .copied()
        .filter(|w| matches!(w.0 & 0xFFFF, 0x723E | 0x7230) || (w.0 & 0xFFFF == 0x7424 && w.0 >> 16 & 0xFF < 4))
        .collect();
    v.push(bmo_gpu_ga10x::cubo::EXIT);
    v
}

/// Floats de prueba: los bordes del half y una lluvia de bits.
fn valores() -> Vec<f32> {
    let mut v = std::vec![0.0, -0.0, 1.0, -2.5, 0.1, 65504.0, 65519.0, 65520.0, 1.0e-5, 5.96e-8, 2.98e-8, 6.1e-5, 1.0e30, f32::INFINITY, f32::NEG_INFINITY, 0.333_333_34];
    let mut x: u32 = 0x1234_5678;
    for _ in 0..20_000 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        v.push(f32::from_bits(x));
    }
    v
}

/// *** LA PRUEBA DE LA PIEZA: lo que deja la 3060 (el epilogo del pegamento en
/// su simulador) es, bit a bit, lo que guarda la CPU, en cada formato y para
/// 20 000 floats (los NaN, NaN en los dos).
#[test]
fn el_redondeo_de_la_3060_es_el_de_la_cpu() {
    let vals = valores();
    for f in FORMATOS {
        let (fl, r) = de_flotante(f, 4).unwrap_or_else(|m| panic!("formato {f}: {m}"));
        assert_eq!(fl, Flotante::Rgba32);
        let codigo = epilogo(r);
        for k in (0..vals.len()).step_by(4) {
            let c = [vals[k], vals[(k + 1) % vals.len()], vals[(k + 2) % vals.len()], vals[(k + 3) % vals.len()]];
            let mut m = Maquina::nueva([&[]; 8]);
            for i in 0..4 {
                m.r[i] = c[i].to_bits();
            }
            correr(&codigo, &mut m).unwrap_or_else(|e| panic!("formato {f}: {e:?}"));
            let cpu = cuantizar(f, c);
            for i in 0..4 {
                let (g, h) = (m.r[i], cpu[i].to_bits());
                assert!(g == h || f32::from_bits(g).is_nan() && f32::from_bits(h).is_nan(), "formato {f}, canal {i}, {:#010x}: la 3060 {g:#010x}, la CPU {h:#010x}", c[i].to_bits());
            }
        }
    }
}

#[test]
fn cada_formato_su_redondeo() {
    use Canal::*;
    assert_eq!(de_flotante(10, 4), Ok((Flotante::Rgba32, Redondeo([Half; 4]))));
    assert_eq!(de_flotante(2, 4), Ok((Flotante::Rgba32, Redondeo::NADA)));
    assert_eq!(de_flotante(34, 4), Ok((Flotante::Rgba32, Redondeo([Half, Half, Cero, Uno]))));
    assert_eq!(de_flotante(41, 1), Ok((Flotante::R32, Redondeo::NADA)));
    // La prueba del NO: lo que la 3060 no sabe redondear todavia.
    assert!(de_flotante(26, 4).unwrap_err().contains("R11G11B10"));
    assert!(de_flotante(24, 4).unwrap_err().contains("UNORM"));
    assert!(de_flotante(10, 1).is_err(), "un RGBA16F no se guarda en una palabra");
}

const CUADRO_VS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_vs.dxil");
const COLOR_PS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_color.dxil");

fn reglas() -> bmo_proton_x::trama::Reglas {
    bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], tijera: [0, 0, 1280, 720], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None }
}

/// *** Un cuadro en un RGBA16F de 1280x720 va a la 3060: la receta lleva el
/// destino de float (16 B por pixel) y el redondeo al half; el pegado de
/// prueba (el del kernel) y su juez lo aprueban. Con Z, sin limpieza y en
/// la cadena, NO (la prueba del NO).
#[test]
fn un_cuadro_en_un_rgba16f_va_a_la_3060() {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let ids = [0, 1, 2, 3, 4, 5];
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &ids, topologia: Topologia::Lista, cb: &cb, reglas: reglas(), limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let b = Blanco { va: 0x4000_0000, ancho: 1280, alto: 720, bgra: false, cadena: false, flotante: Some((10, 4)), mas: crate::puerta::SIN_MAS };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, b).unwrap_or_else(|m| panic!("el RGBA16F va a la 3060: {m}"));
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel lee la receta");
    let (_, dst) = r.dibujo.destino.unwrap();
    assert_eq!((dst.flotante, dst.fila), (Flotante::Rgba32, 1280 * 16));
    assert_eq!(r.dibujo.redondeo, Redondeo([Canal::Half; 4]));
    assert_eq!(p.revisados, 1, "y el vigia lo reviso contra la CPU");
    // NO: con Z, limpiandose, o en la cadena.
    let con_z = bmo_proton_x::trama::Reglas { profundidad: Some(bmo_proton_x::trama::Profundidad { funcion: 2, escribir: true }), ..reglas() };
    assert_eq!(Puerta::nueva().preparar(&Lote { reglas: con_z, ..l }, b).unwrap_err(), Z_CON_FLOAT);
    assert!(Puerta::nueva().preparar(&Lote { limpiar_rt: Some(0), ..l }, b).is_err());
    assert!(Puerta::nueva().preparar(&l, Blanco { cadena: true, ..b }).is_err());
}

/// ** Metal 12-10: el censo decia "un formato que la casa no conoce (DXGI
/// 32571392)": era la MARCA de la casa (`bufer::CUATRO_FLOATS`) con la vista
/// arriba. Desempaquetada: un RGBA16F guardado en cuatro floats VA; uno visto
/// con otro formato lo dice por su nombre.
#[test]
fn la_marca_de_cuatro_floats_se_desempaqueta() {
    use bmo_proton_x::bufer::{con_vista, CUATRO_FLOATS};
    let (f, _) = crate::puerta::de_flotante(10 | CUATRO_FLOATS, 4).expect("RGBA16F en cuatro floats");
    assert_eq!(f, bmo_gpu_ga10x::destino::Flotante::Rgba32);
    assert_eq!(32571392 & CUATRO_FLOATS, CUATRO_FLOATS, "la del metal");
    assert_eq!(crate::puerta::de_flotante(32571392, 4).unwrap_err(), crate::puerta::FLOTANTE_OTRA_VISTA);
    assert_eq!(crate::puerta::de_flotante(con_vista(10, 34), 4).unwrap_err(), crate::puerta::FLOTANTE_OTRA_VISTA);
}
