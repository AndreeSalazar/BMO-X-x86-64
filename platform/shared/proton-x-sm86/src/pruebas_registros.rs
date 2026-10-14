//! ** LI2b, LOS REGISTROS (12-10): el censo de Cyberpunk paraba lotes por
//! "mas registros de los que da la tuberia" -- el cuerpo y su pegamento no
//! cabian en 64 --. Desde hoy el emisor tiene hasta `puerta::REGISTROS`, el
//! pegamento hasta `tuberia::MAX_REGISTROS` (248), y cada dibujo pide los que
//! usan sus dos programas (`tuberia::registros_para`): el juez juzga con ESE
//! numero y las ordenes lo dan.

extern crate std;

use alloc::vec::Vec;

use bmo_gpu_ga10x::pegamento::{self, Datos};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::tuberia;
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{self, Op, Programa};

use crate::{emitir_con, Abi};

/// *** Lo que ya cabia en 64 sale IGUAL, byte a byte (y con los mismos
/// registros): darle mas al emisor no cambia un programa que no los usa. Los
/// sombreadores de `proton-x/prueba` enteros. (S1, 14-10: lo que cabia SIN
/// ser frugal: un `Sample` con sus mips pide mas, y a 64 sale frugal.)
#[test]
fn lo_que_cabia_en_64_sale_igual() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../proton-x/prueba");
    let mut comparados = 0;
    for f in std::fs::read_dir(&dir).unwrap() {
        let f = f.unwrap().path();
        if f.extension().is_none_or(|e| e != "dxil") {
            continue;
        }
        let Ok(s) = bmo_proton_x::dxil::leer(&std::fs::read(&f).unwrap()) else { continue };
        let Ok(p) = programa::compilar(&s) else { continue };
        for libreta in [false, true] {
            let Ok(a) = crate::emitir_modo(&p, 64, Abi::Registros, libreta, false) else { continue };
            let b = crate::emitir_libreta(&p, crate::puerta::REGISTROS, Abi::Registros, libreta).unwrap_or_else(|x| panic!("{f:?}: con mas registros, {x:?}"));
            assert_eq!((a.codigo, a.registros, a.termometro), (b.codigo, b.registros, b.termometro), "{f:?} (libreta {libreta})");
            comparados += 1;
        }
    }
    assert!(comparados > 40, "solo {comparados}");
}

/// `n` valores vivos a la vez: `v_k = x * c_k + y`, todos, y despues su
/// suma. Ninguno se puede soltar hasta la suma.
fn muchos_vivos(n: u16) -> Programa {
    let mut ops: Vec<Op> = (0..2).map(|k| Op::Entrada { d: k, elemento: 0, componente: k as u8 }).collect();
    let (c0, v0) = (2u16, 2 + n);
    for k in 0..n {
        ops.push(Op::Mad { d: v0 + k, a: 0, b: c0 + k, c: 1 });
    }
    let mut s = v0;
    for k in 1..n {
        ops.push(Op::Mad { d: v0 + n + k, a: v0 + k, b: c0, c: s });
        s = v0 + n + k;
    }
    ops.push(Op::Salida { s, elemento: 0, componente: 0 });
    let constantes: Vec<(u16, f32)> = (0..n).map(|k| (c0 + k, 1.0 + k as f32 / 64.0)).collect();
    ejemplos::programa(ops, (v0 + 2 * n) as usize, &constantes)
}

/// *** Un programa de 100 valores vivos: en 64 no cabe ni frugal; con lo de
/// hoy si, pegado, juzgado con los registros que pide el dibujo (y NO con
/// 64: ahi seria BODRIO), y con los bits de la CPU en el simulador.
#[test]
fn cien_valores_vivos_van_a_la_3060() {
    let p = muchos_vivos(100);
    assert!(emitir_con(&p, 64, Abi::Registros).is_err(), "en 64 no cabe");
    let e = emitir_con(&p, crate::puerta::REGISTROS, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(e.registros > 64, "usa {}", e.registros);
    for x in [[1.0f32, 2.0, 0.0, 0.0], [-3.5, 1e-3, 0.0, 0.0]] {
        crate::pruebas::igual_en_registros(&p, &e, &[x], &[]);
    }
    let pegado = pegamento::pixel(&e.codigo, e.registros, &crate::pso::cargas(&e), Datos { filas: p.filas_cb as u32, paso: 0, elementos: &[] }, &[Some(0)]).unwrap_or_else(|x| panic!("{x:?}"));
    let mut b = std::vec![0u8; tuberia::HUECO];
    let n = pegado.bytes(&mut b);
    let pide = tuberia::registros_para(pegado.registros);
    assert!(pide > tuberia::REGISTROS && pide <= tuberia::MAX_REGISTROS && pide % 8 == 0, "{pide}");
    juez::juzgar_programa(&b[..n], pide).unwrap_or_else(|x| panic!("{x}"));
    assert!(juez::juzgar_programa(&b[..n], tuberia::REGISTROS).is_err(), "con 64, el juez lo para");
    assert!(crate::vivo::comprobar(&p, &e, "de pixel").is_ok());
}

/// *** Lo que pasa de 248 lo sigue diciendo el emisor (o el pegamento): por
/// la CPU, con su motivo.
#[test]
fn lo_que_no_cabe_ni_en_248() {
    assert!(emitir_con(&muchos_vivos(260), crate::puerta::REGISTROS, Abi::Registros).is_err());
}

/// Los registros de un dibujo: lo de siempre (64) hasta que hace falta mas,
/// a multiplo de 8, con los dos del contador encima.
#[test]
fn los_registros_que_pide_un_dibujo() {
    assert_eq!(tuberia::registros_para(0), 64);
    assert_eq!(tuberia::registros_para(62), 64);
    assert_eq!(tuberia::registros_para(63), 72);
    assert_eq!(tuberia::registros_para(100), 104);
    assert_eq!(tuberia::registros_para(246), 248);
    assert_eq!(crate::puerta::REGISTROS + juez::RESERVADOS + 8, tuberia::MAX_REGISTROS);
}

