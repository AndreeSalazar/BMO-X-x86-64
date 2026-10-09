//! ** DL12 (09-10): el `discard` en la 3060 (`Op::Descarta`, el KILL con su
//! guarda) -- en los dos ABI, juzgado (R0..R6 y R7), con el pixel y los bits
//! de la casa: el que la casa tira, la 3060 lo tira; el que deja, con su
//! color. Y por la PUERTA: el pegamento dice KillsPixels y el juez de
//! programas no deja un KILL sin el.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, Regla, RESERVADOS};
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Op, OpEntera, Programa};

use crate::pruebas::{igual, igual_en_registros};
use crate::{emitir, emitir_con, Abi, Emitido};

const TECHO: u32 = 64;

fn bits(x: u32) -> f32 {
    f32::from_bits(x)
}

/// Los dos ABI, juzgados (R0..R6, y R7 el de registros).
fn emitidos(p: &Programa) -> (Emitido, Emitido) {
    let e = emitir(p, TECHO).unwrap_or_else(|x| panic!("{:?}", x));
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap_or_else(|x| panic!("{:?}", x));
    for x in [&e, &r] {
        let v = juzgar_drenado(&x.codigo, &Contexto { registros: x.registros + RESERVADOS, sph: None });
        assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(juzgar_cuerpo_de_app(&r.codigo, r.registros), Ok(()));
    (e, r)
}

fn los_dos(p: &Programa, entradas: &[Vec<[f32; 4]>]) -> (Emitido, Emitido) {
    let (e, r) = emitidos(p);
    for ent in entradas {
        igual(p, &e.codigo, ent, &[]);
        igual_en_registros(p, &r, ent, &[]);
    }
    (e, r)
}

/// Cuantas instrucciones de `codigo` son `op` (el opcode con su forma, 12 bits).
fn cuantas(codigo: &[(u64, u64)], op: u64) -> usize {
    codigo.iter().filter(|w| w.0 & 0xFFF == op).count()
}

const KILL: u64 = 0x95B;

/// Floats de todas las clases: ceros de cada signo, NaN, infinitos,
/// subnormales, y los de cada lado de 0.5.
fn floats() -> Vec<f32> {
    vec![-1.0, -0.0, 0.0, 1.0, 0.5, 0.49999997, 0.50000006, f32::NAN, -f32::NAN, f32::INFINITY, f32::NEG_INFINITY, bits(1), bits(0x8000_0001), 1e30, -1e-30]
}

/// ** `if (x < 0) discard;` -- la Compara se FUNDE con el KILL (sin SEL ni
/// ISETP de mas): un FSETP y `@P0 KILL`.
#[test]
fn tira_el_negativo() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Compara { d: 1, a: 0, b: 5, como: Comparacion::Menor, entero: false },
            Op::Descarta { c: 1 },
            Op::Add { d: 2, a: 0, b: 0 },
            Op::Salida { s: 2, elemento: 0, componente: 0 },
        ],
        6,
        &[(5, 0.0)],
    );
    let ent: Vec<Vec<[f32; 4]>> = floats().into_iter().map(|x| vec![[x, 0.0, 0.0, 0.0]]).collect();
    let (e, r) = los_dos(&p, &ent);
    for x in [&e, &r] {
        assert_eq!(cuantas(&x.codigo, KILL), 1);
        assert_eq!(cuantas(&x.codigo, 0x207) + cuantas(&x.codigo, 0x807), 0, "la Compara fundida no pide SEL");
    }
}

/// ** Con los BITS de un entero (`discard_nz` de SM5): sin Compara que
/// fundir, `ISETP.NE P0, c, RZ` y el KILL.
#[test]
fn tira_con_los_bits_de_un_entero() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 0, componente: 1 },
            Op::Entera { d: 2, a: 1, b: 6, op: OpEntera::Y },
            Op::Descarta { c: 2 },
            Op::Salida { s: 0, elemento: 0, componente: 0 },
        ],
        7,
        &[(6, bits(1))],
    );
    let mut ent = Vec::new();
    for x in [1.5f32, -0.0, f32::NAN] {
        for b in [0u32, 1, 2, 3, 0x8000_0000, 0xFFFF_FFFF, 0xFFFF_FFFE] {
            ent.push(vec![[x, bits(b), 0.0, 0.0]]);
        }
    }
    los_dos(&p, &ent);
}

/// ** Dentro de un BUCLE y de un `si`: la vuelta `i` tira el pixel (si `i`
/// es de las cuatro); el que sigue suma su `x` cada vuelta.
#[test]
fn tira_dentro_de_un_bucle_y_de_un_si() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 0, componente: 1 },
            Op::Copia { d: 2, a: 40 },
            Op::Copia { d: 3, a: 43 },
            Op::Bucle,
            Op::Compara { d: 4, a: 2, b: 41, como: Comparacion::MayorIgual, entero: true },
            Op::RomperSi { c: 4, si_cero: false },
            Op::Compara { d: 5, a: 2, b: 1, como: Comparacion::Igual, entero: true },
            Op::Si { c: 5 },
            Op::Descarta { c: 5 },
            Op::FinSi,
            Op::Add { d: 6, a: 3, b: 0 },
            Op::Copia { d: 3, a: 6 },
            Op::SumaEntera { d: 7, a: 2, b: 42 },
            Op::Copia { d: 2, a: 7 },
            Op::FinBucle,
            Op::Salida { s: 3, elemento: 0, componente: 0 },
        ],
        44,
        &[(40, bits(0)), (41, bits(4)), (42, bits(1)), (43, 0.0)],
    );
    let mut ent = Vec::new();
    for x in [0.25f32, -3.0, f32::NAN] {
        for i in [0u32, 1, 3, 4, 7, 0xFFFF_FFFF] {
            ent.push(vec![[x, bits(i), 0.0, 0.0]]);
        }
    }
    los_dos(&p, &ent);
}

const DESCARTE: &[u8] = include_bytes!("../../proton-x/prueba/descarte.dxil");

/// *** `descarte.hlsl` de `dxc` (N5.7: el `clip` y el `discard` de un `if`,
/// lo recortado por alfa de Cyberpunk) -- de punta a punta en la 3060: el
/// lector, el emisor, el juez y el simulador. Dos KILL.
#[test]
fn el_pixel_de_dxc_con_clip_y_discard() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(DESCARTE).unwrap()).unwrap();
    let mut ent = Vec::new();
    for x in [0.25f32, 0.49999997, 0.5, 0.75, 1.0, f32::NAN, f32::NEG_INFINITY] {
        for y in [0.5f32, 0.75, 0.75000006, 0.9, f32::NAN, -0.0] {
            ent.push(vec![[0.0; 4], [x, y, 0.0, 0.0]]);
        }
    }
    let (e, r) = los_dos(&p, &ent);
    for x in [&e, &r] {
        assert_eq!(cuantas(&x.codigo, KILL), 2);
    }
    std::eprintln!("descarte.hlsl: {} instrucciones ({} con el ABI de registros)", e.codigo.len(), r.codigo.len());
}

/// ** El mismo `discard` de SM5 (`discard_nz`, `discard_z`, de `fxc`).
#[test]
fn el_sm5_con_discard_nz_y_discard_z() {
    let (t, en, sa) = ejemplos::sm5_descarte();
    let p = bmo_proton_x::sm5::compilar(&t, &en, &sa).unwrap();
    let mut ent = Vec::new();
    for x in [0.25f32, 0.5, 0.75, f32::NAN] {
        for y in [1.0f32, 0.0, -0.0, 2.0, f32::NAN] {
            ent.push(vec![[x, y, 0.0, 0.0]]);
        }
    }
    let (e, _) = los_dos(&p, &ent);
    assert_eq!(cuantas(&e.codigo, KILL), 2);
}

/// *** LA PUERTA: `descarte.hlsl` pegado con el pegamento de pixel del
/// driver lleva KillsPixels en su SPH (el bit 15) y el juez de programas lo
/// da por bueno; la misma SPH SIN el bit es R5 (NVIDIA: el KILL seria un NOP
/// y una excepcion). Uno sin KILL no lo lleva.
#[test]
fn la_puerta_dice_kills_pixels() {
    use bmo_gpu_ga10x::pegamento::{self, Datos};
    use bmo_gpu_ga10x::raster::MATA_PIXELES;
    use bmo_gpu_ga10x::sass::juez;
    use bmo_gpu_ga10x::tuberia;
    let pegar = |p: &Programa, genericos: &[Option<u8>]| {
        let r = emitir_con(p, TECHO, Abi::Registros).unwrap();
        pegamento::pixel(&r.codigo, r.registros, &crate::pso::cargas(&r), Datos { filas: p.filas_cb as u32, paso: 0, elementos: &[] }, genericos).unwrap()
    };
    let bytes = |g: &pegamento::Pegado| {
        let mut b = vec![0u8; tuberia::HUECO];
        let n = g.bytes(&mut b);
        b.truncate(n);
        b
    };
    let dxc = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(DESCARTE).unwrap()).unwrap();
    let g = pegar(&dxc, &[None, Some(0)]);
    assert_ne!(g.sph[0] & MATA_PIXELES, 0, "el cuerpo tira pixeles: la SPH lo dice");
    let mut b = bytes(&g);
    let v = juez::juzgar_programa(&b, tuberia::REGISTROS).unwrap_or_else(|x| panic!("{x}"));
    std::eprintln!("descarte.hlsl pegado: {} instrucciones", v.instrucciones);
    // La SPH sin KillsPixels: el bit 15 de la palabra 0 (byte 1, bit 7).
    b[1] &= !0x80;
    let x = juez::juzgar_programa(&b, tuberia::REGISTROS).unwrap_err();
    assert_eq!((x.regla, x.que), (Regla::R5CabeceraMiente, 15));
    // Sin KILL, sin el bit.
    let sin = ejemplos::programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Salida { s: 0, elemento: 0, componente: 0 }], 1, &[]);
    assert_eq!(pegar(&sin, &[Some(0)]).sph[0] & MATA_PIXELES, 0);
}
