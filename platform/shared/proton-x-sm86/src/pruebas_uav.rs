//! ** LI2f (12-10): EL COMPUTO DE UNA APP, de la casa a la 3060 de mentira --
//! los ids (S2R), la barrera y los UAV crudos y estructurados con la
//! direccion SUJETA (`uav.rs`), contra `Programa::despachar` de la CPU: los
//! MISMOS bytes en los UAV, grupo a grupo. Y lo de fuera: ni una direccion
//! que se salga de lo prestado (el simulador lo diria: `NoSimula::Fallo`).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::bufer::{Modo, Rebanadas, Uav};
use bmo_proton_x::dxil::programa::{Computo, Op, OpEntera, Programa, Ranuras};
use bmo_proton_x::textura::Recursos;

use crate::simula::Memoria;
use crate::simula_grupo::correr_grupo;
use crate::{emitir_con, Abi, DelUav, NoEmite, Precarga};

/// Un UAV de la prueba: sus bytes, su vista, y donde lo ve la 3060.
#[derive(Clone)]
struct Vista {
    bytes: Vec<u8>,
    modo: Modo,
    elementos: u32,
    paso: u32,
}

/// La memoria de la 3060: cada UAV en su ventana (la IOVA de la ranura k,
/// `BASE + k * RANURA`); fuera de ellas, un fallo.
struct Ventanas(Vec<Vec<u8>>);
const BASE: u64 = 0x1_2340_0000;
const RANURA: u64 = 0x10_0000;

impl Ventanas {
    fn donde(&self, dir: u64) -> Option<(usize, usize)> {
        let k = dir.checked_sub(BASE)? / RANURA;
        let o = (dir - BASE - k * RANURA) as usize;
        let b = self.0.get(k as usize)?;
        (dir % 4 == 0 && o + 4 <= b.len()).then_some((k as usize, o))
    }
}

impl Memoria for Ventanas {
    fn leer(&mut self, dir: u64) -> Option<u32> {
        let (k, o) = self.donde(dir)?;
        Some(u32::from_le_bytes(self.0[k][o..o + 4].try_into().unwrap()))
    }
    fn escribir(&mut self, dir: u64, v: u32) -> bool {
        match self.donde(dir) {
            Some((k, o)) => {
                self.0[k][o..o + 4].copy_from_slice(&v.to_le_bytes());
                true
            }
            None => false,
        }
    }
}

fn programa(ops: Vec<Op>, iniciales: Vec<u32>, uavs: usize, hilos: [u32; 3]) -> Programa {
    let l = bmo_proton_x::dxil::programa::Lugar { espacio: 0, registro: 0, vista: 0 };
    Programa {
        ops,
        iniciales: iniciales.into_iter().map(f32::from_bits).collect(),
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 0,
        ranuras: Ranuras { uavs: vec![l; uavs], ..Ranuras::default() },
        computo: Computo { hilos, compartida: 0, temprana: false },
    }
}

/// Lo que dejan en los UAV la CPU y la 3060 de mentira, `grupos` en x. Con
/// `pisar`, lo que el kernel pone se cambia antes de correr (un cuerpo que
/// se lo salte).
fn las_dos(p: &Programa, vistas: &[Vista], grupos: u32) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut casa: Vec<Vec<u8>> = vistas.iter().map(|v| v.bytes.clone()).collect();
    {
        let mut uavs: Vec<Option<Uav>> = casa.iter_mut().zip(vistas).map(|(b, v)| Some(Uav { bytes: b, formato: 0, paso: v.paso, elementos: v.elementos, contador: None, rebanadas: Rebanadas::PLANA })).collect();
        let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: None };
        p.despachar([grupos, 1, 1], &[], &rec, &mut uavs);
    }
    let e = emitir_con(p, 64, Abi::Registros).unwrap_or_else(|x| panic!("no se emite: {x:?}"));
    let mut mem = Ventanas(vistas.iter().map(|v| v.bytes.clone()).collect());
    for gx in 0..grupos {
        correr_grupo(&e.codigo, p.computo.hilos, [gx, 0, 0], [&[]; 8], &mut [], &mut mem, |_, m| {
            for q in &e.precargas {
                if let Precarga::Uav { u, que, reg } = *q {
                    let v = &vistas[u as usize];
                    let x = que.valor(BASE + u as u64 * RANURA, v.bytes.len() as u32, v.elementos, v.paso).expect("se presta");
                    m.r[reg as usize] = x as u32;
                    if que == DelUav::Base {
                        m.r[reg as usize + 1] = (x >> 32) as u32;
                    }
                }
            }
        })
        .unwrap_or_else(|x| panic!("la 3060 de mentira: {x:?}"));
    }
    (casa, mem.0)
}

fn palabras(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// `salida[id] = entrada[id] * 2` en CRUDO (`ByteAddressBuffer`, el byte
/// `4 * id`), con 3 grupos de 8 y una entrada de 20 palabras: las 4 ultimas
/// leen FUERA (0 en las dos), y la salida de 22 palabras pierde las 2
/// ultimas escrituras (en las dos).
#[test]
fn crudo_el_doble_con_lo_de_fuera() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::Entera { d: 1, a: 0, b: 2, op: OpEntera::Shl },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9 },
        Op::Mul { d: 7, a: 3, b: 8 },
        Op::EscribeUav { u: 1, modo: Modo::Crudo, i: 1, desp: 9, z: 9, v: [7, 7, 7, 7], mascara: 1 },
    ];
    let mut ini = vec![0u32; 10];
    ini[2] = 2;
    ini[8] = 2.0f32.to_bits();
    let p = programa(ops, ini, 2, [8, 1, 1]);
    let entrada: Vec<u32> = (0..20).map(|k| (k as f32 + 0.5).to_bits()).collect();
    let vistas = [
        Vista { bytes: palabras(&entrada), modo: Modo::Crudo, elementos: 20, paso: 0 },
        Vista { bytes: palabras(&[0xDEAD_BEEF; 22]), modo: Modo::Crudo, elementos: 22, paso: 0 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 3);
    assert_eq!(gpu, casa, "los mismos bytes en los dos UAV");
    let w = |k: usize| u32::from_le_bytes(gpu[1][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!(f32::from_bits(w(3)), 7.0);
    assert_eq!(w(21), 0, "la palabra 21 se leyo FUERA de la entrada (0) y se escribio");
    let _ = vistas[0].modo;
}

/// ESTRUCTURADO (`StructuredBuffer<S>` de 12 bytes): cada hilo lee las
/// cuatro palabras desde `desp` = 4 de SU elemento -- la cuarta se sale del
/// elemento (0) -- y las escribe en el suyo de 16 bytes. Con 7 elementos de
/// entrada y 2 grupos de 4, el elemento 7 entero se lee fuera.
#[test]
fn estructurado_con_la_palabra_que_se_sale() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Estructurado, i: 0, desp: 1, z: 2 },
        Op::EscribeUav { u: 1, modo: Modo::Estructurado, i: 0, desp: 2, z: 2, v: [3, 4, 5, 6], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 7];
    ini[1] = 4;
    let p = programa(ops, ini, 2, [4, 1, 1]);
    let entrada: Vec<u32> = (0..21).map(|k| 0x100 + k).collect();
    let vistas = [
        Vista { bytes: palabras(&entrada), modo: Modo::Estructurado, elementos: 7, paso: 12 },
        Vista { bytes: palabras(&[0x5555_5555; 32]), modo: Modo::Estructurado, elementos: 8, paso: 16 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 2);
    assert_eq!(gpu, casa);
    let w = |k: usize| u32::from_le_bytes(gpu[1][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!([w(4), w(5), w(6), w(7)], [0x104, 0x105, 0, 0], "el elemento 1: dos palabras dentro, la tercera sale del elemento");
    assert_eq!([w(28), w(29), w(30), w(31)], [0, 0, 0, 0], "el elemento 7 no esta en la entrada");
}

/// Los ids: SV_GroupIndex en un grupo de 4 x 2 x 2 y SV_GroupID.x, cada
/// hilo en su palabra; y una barrera (fuera de todo `si`) en medio.
#[test]
fn los_ids_y_la_barrera() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 3, c: 0 },
        Op::IdHilo { d: 1, que: 1, c: 0 },
        Op::Barrera,
        Op::Entera { d: 2, a: 1, b: 9, op: OpEntera::Mul },
        Op::SumaEntera { d: 3, a: 2, b: 0 },
        Op::Entera { d: 4, a: 3, b: 8, op: OpEntera::Shl },
        Op::IdHilo { d: 5, que: 2, c: 2 },
        Op::IdHilo { d: 6, que: 0, c: 1 },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 4, desp: 10, z: 10, v: [3, 5, 6, 0], mascara: 7 },
    ];
    let mut ini = vec![0u32; 11];
    ini[8] = 4; // * 16 bytes
    ini[9] = 16; // hilos del grupo
    let p = programa(ops, ini, 1, [4, 2, 2]);
    let vistas = [Vista { bytes: vec![0xAA; 16 * 48], modo: Modo::Crudo, elementos: 4 * 48, paso: 0 }];
    let (casa, gpu) = las_dos(&p, &vistas, 3);
    assert_eq!(gpu, casa);
    let w = |k: usize| u32::from_le_bytes(gpu[0][4 * k..4 * k + 4].try_into().unwrap());
    // El hilo 13 del grupo 2: (1, 1, 1); el 33 del despacho.
    assert_eq!([w(4 * 45), w(4 * 45 + 1), w(4 * 45 + 2)], [45, 1, 1]);
}

/// UN CUERPO QUE NO SE PORTA BIEN: la 3060 de mentira corre lo emitido con
/// indices enormes (`i` = 0xFFFF_FFF0 y desp que acarrean), y no sale de lo
/// prestado nunca -- la direccion esta SUJETA al tope --; y con el TOPE
/// mal puesto (mas alla de lo prestado), el simulador SI lo ve: la prueba
/// sabe decir que no.
#[test]
fn la_direccion_nunca_sale_de_lo_prestado() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::SumaEntera { d: 1, a: 0, b: 2 },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9 },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9, v: [3, 4, 5, 6], mascara: 0xF },
        Op::LeeUav { d: 10, u: 1, modo: Modo::Estructurado, i: 1, desp: 9, z: 9 },
        Op::EscribeUav { u: 1, modo: Modo::Estructurado, i: 0, desp: 2, z: 9, v: [10, 11, 12, 13], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 14];
    ini[2] = 0xFFFF_FFF0;
    ini[9] = 0xFFFF_FFF8;
    let p = programa(ops, ini, 2, [32, 1, 1]);
    let vistas = [
        Vista { bytes: palabras(&[7; 8]), modo: Modo::Crudo, elementos: 8, paso: 0 },
        Vista { bytes: palabras(&[9; 12]), modo: Modo::Estructurado, elementos: 3, paso: 16 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 1);
    assert_eq!(gpu, casa, "todo fuera: nada se lee ni se escribe, y nada se sale");
    // El tope MAL puesto: 64 bytes mas alla. Sin la sujecion bien cargada,
    // se sale (y el simulador lo dice).
    let e = emitir_con(&p, 64, Abi::Registros).unwrap();
    let mut mem = Ventanas(vistas.iter().map(|v| v.bytes.clone()).collect());
    let r = correr_grupo(&e.codigo, [32, 1, 1], [0; 3], [&[]; 8], &mut [], &mut mem, |_, m| {
        for q in &e.precargas {
            if let Precarga::Uav { u, que, reg } = *q {
                let v = &vistas[u as usize];
                let x = que.valor(BASE + u as u64 * RANURA, v.bytes.len() as u32 + 64, v.elementos + 16, v.paso).unwrap();
                m.r[reg as usize] = x as u32;
                if que == DelUav::Base {
                    m.r[reg as usize + 1] = (x >> 32) as u32;
                }
            }
        }
    });
    assert!(matches!(r, Err(crate::simula::NoSimula::Fallo(_))), "{r:?}");
}

/// Lo que no va todavia, con su indice (por la CPU): un UAV con tipo, la
/// barrera dentro de un `si`, y lo de computo en un programa que no lo es.
#[test]
fn lo_que_no_va_todavia() {
    let tipado = programa(vec![Op::LeeUav { d: 0, u: 0, modo: Modo::Tipado, i: 4, desp: 4, z: 4 }], vec![0; 5], 1, [1, 1, 1]);
    assert_eq!(emitir_con(&tipado, 64, Abi::Registros).err(), Some(NoEmite::Operacion(0)));
    let en_si = programa(vec![Op::Si { c: 0 }, Op::Barrera, Op::FinSi], vec![1; 1], 0, [2, 1, 1]);
    assert_eq!(emitir_con(&en_si, 64, Abi::Registros).err(), Some(NoEmite::Operacion(1)));
    let dibujo = programa(vec![Op::IdHilo { d: 0, que: 0, c: 0 }], vec![0; 1], 0, [0; 3]);
    assert_eq!(emitir_con(&dibujo, 64, Abi::Registros).err(), Some(NoEmite::Operacion(0)));
}
