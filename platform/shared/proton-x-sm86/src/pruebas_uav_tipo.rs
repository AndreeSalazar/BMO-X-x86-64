//! ** L3 (14-10): LOS UAV CON TIPO Y DE TEXTURA en la 3060 de mentira --
//! `RWTexture2D<T>` y `RWBuffer<T>` en las palabras en que los guarda la
//! casa, por la MISMA cadena sujeta (`uav.rs`) --, contra
//! `Programa::despachar` de la CPU: los mismos bytes en cada UAV, con lo
//! que se sale (x, y, z, el elemento), el relleno de los cuatro floats, los
//! atomicos y `GetDimensions`. Y el juez del kernel (R0..R7).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::bufer::{Atomo, Modo, Rebanadas, Uav, CUATRO_FLOATS};
use bmo_proton_x::dxil::programa::{Op, OpEntera, Programa};
use bmo_proton_x::textura::Recursos;

use crate::pruebas_uav::{palabras, programa, Ventanas, BASE, RANURA};
use crate::simula_grupo::correr_grupo;
use crate::{emitir_con, forma_de, Abi, DelUav, Precarga};

/// Un UAV con tipo: sus bytes y su vista como la de la casa.
struct Vista {
    bytes: Vec<u8>,
    formato: u32,
    paso: u32,
    elementos: u32,
}

/// Lo que dejan la CPU y la 3060 de mentira (un grupo).
fn las_dos(p: &Programa, vistas: &[Vista]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut casa: Vec<Vec<u8>> = vistas.iter().map(|v| v.bytes.clone()).collect();
    {
        let mut uavs: Vec<Option<Uav>> = casa.iter_mut().zip(vistas).map(|(b, v)| Some(Uav { bytes: b, formato: v.formato, paso: v.paso, elementos: v.elementos, contador: None, rebanadas: Rebanadas::PLANA })).collect();
        let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: None };
        p.despachar([1, 1, 1], &[], &rec, &mut uavs);
    }
    let e = emitir_con(p, 248, Abi::Registros).unwrap_or_else(|x| panic!("no se emite: {x:?}"));
    {
        use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_drenado, Contexto, RESERVADOS};
        juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("R0..R6: {b}"));
        juzgar_cuerpo_con(&e.codigo, e.registros, crate::permisos_computo(&e, p).unwrap()).unwrap_or_else(|b| panic!("R7: {b}"));
    }
    // Lo que escribe cada UAV del cuerpo, para su forma.
    let escribe = |u: u8| p.ops.iter().any(|o| matches!(*o, Op::EscribeUav { u: x, .. } | Op::Atomico { u: x, .. } if x == u));
    let mut mem = Ventanas(vistas.iter().map(|v| v.bytes.clone()).collect());
    let cargar = |_: u32, m: &mut crate::simula::Maquina| {
        for q in &e.precargas {
            let Precarga::Uav { u, que, reg } = *q else { panic!("solo UAV") };
            let v = &vistas[u as usize];
            let forma = forma_de(v.formato, escribe(u), true).expect("la 3060 lo ve");
            let x = que.valor_con(BASE + u as u64 * RANURA, v.bytes.len() as u32, v.elementos, v.paso, forma).expect("se presta");
            m.r[reg as usize] = x as u32;
            if que == DelUav::Base {
                m.r[reg as usize + 1] = (x >> 32) as u32;
            }
        }
    };
    correr_grupo(&e.codigo, p.computo.hilos, [0, 0, 0], [&[]; 8], &mut [], &mut mem, cargar).unwrap_or_else(|x| panic!("la 3060 de mentira: {x:?}"));
    (casa, mem.0)
}

/// Cuatro floats por texel (lo de la casa), con NaN, -0, infinitos y
/// subnormales en los canales.
fn texeles(n: usize, semilla: u32) -> Vec<u8> {
    let raros = [0x7FC0_1234u32, 0x8000_0000, 0x7F80_0000, 0x0000_0001, 0x3F80_0000, 0xC2F6_E979];
    palabras(&(0..4 * n as u32).map(|k| if k % 5 == 3 { raros[(k / 5) as usize % raros.len()] } else { k.wrapping_mul(0x9E37_79B9) ^ semilla }).collect::<Vec<u32>>())
}

/// Un grupo de 4x4 sobre texturas de 3x3 (la fila y la columna 3 se salen):
/// cada hilo lee su texel de u0 (un RGBA16F en cuatro floats: se lee tal
/// cual), escribe tres canales en u1 (un R32G32_FLOAT: dos canales, y la
/// casa reescribe el relleno: 0 y 1.0), suma 1 en el texel (x % 2, y % 2) de
/// u2 (un R32_UINT en cuatro floats: atomico y relleno de enteros), escribe
/// lo de antes en u3 (un RWBuffer de R32G32_UINT: dos palabras por elemento;
/// uno con su ultimo elemento a medias va por la CPU, `cabe_entero`)
/// y las medidas de u0 en u4 (un R32G32B32A32_UINT). Uno con z = 1 no
/// escribe nada.
#[test]
fn texturas_y_buferes_con_tipo_como_la_casa() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::IdHilo { d: 1, que: 0, c: 1 },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Textura, i: 0, desp: 1, z: 2 },
        Op::SumaEntera { d: 8, a: 3, b: 7 },
        Op::EscribeUav { u: 1, modo: Modo::Textura, i: 0, desp: 1, z: 2, v: [8, 4, 5, 6], mascara: 0b1011 },
        Op::EscribeUav { u: 1, modo: Modo::Textura, i: 0, desp: 1, z: 9, v: [9, 9, 9, 9], mascara: 0xF },
        Op::Entera { d: 10, a: 0, b: 13, op: OpEntera::RemU },
        Op::Entera { d: 11, a: 1, b: 13, op: OpEntera::RemU },
        Op::Atomico { d: 12, u: 2, modo: Modo::Textura, i: 10, desp: 11, z: 2, como: Atomo::Suma, v: 9, igual: 2 },
        Op::Entera { d: 14, a: 1, b: 15, op: OpEntera::Shl },
        Op::SumaEntera { d: 14, a: 14, b: 0 },
        Op::EscribeUav { u: 3, modo: Modo::Tipado, i: 14, desp: 2, z: 2, v: [12, 3, 3, 3], mascara: 0b0111 },
        Op::MedidasUav { d: 16, u: 0, modo: Modo::Textura },
        Op::MedidasUav { d: 20, u: 3, modo: Modo::Tipado },
        Op::EscribeUav { u: 4, modo: Modo::Tipado, i: 14, desp: 2, z: 2, v: [16, 17, 20, 6], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 24];
    ini[7] = 0x5A5A_1234;
    ini[9] = 1;
    ini[13] = 2;
    ini[15] = 2;
    let p = programa(ops, ini, 5, [4, 4, 1]);
    let vistas = [
        Vista { bytes: texeles(9, 0x1111), formato: 10 | CUATRO_FLOATS, paso: 3, elementos: 9 },
        // El texel 8 (2, 2) no cabe entero: ni se lee ni se escribe.
        Vista { bytes: vec![0x33; 16 * 8 + 8], formato: 16 | CUATRO_FLOATS, paso: 3, elementos: 9 },
        Vista { bytes: palabras(&[5, 7, 7, 7, 6, 7, 7, 7, 7, 7, 7, 7, 8, 7, 7, 7]), formato: 42 | CUATRO_FLOATS, paso: 2, elementos: 4 },
        Vista { bytes: vec![0x44; 128], formato: 17, paso: 0, elementos: 16 },
        Vista { bytes: vec![0x55; 16 * 16], formato: 3, paso: 0, elementos: 16 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas);
    for (u, (c, g)) in casa.iter().zip(&gpu).enumerate() {
        assert_eq!(g, c, "el UAV {u}");
    }
    let w = |u: usize, k: usize| u32::from_le_bytes(gpu[u][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!([w(1, 2), w(1, 3)], [0, 0x3F80_0000], "el relleno de un R32G32 en cuatro floats");
    assert_eq!([w(2, 0), w(2, 1), w(2, 3)], [5 + 4, 0, 1], "cuatro hilos en (0, 0); el relleno de enteros");
    assert_eq!([w(4, 0), w(4, 1), w(4, 2)], [3, 3, 16], "GetDimensions de la textura y del bufer");
    assert_eq!(&gpu[1][128..136], &[0x33; 8], "el texel que no cabe entero no se toca");
}

/// La forma de cada vista: lo que va y lo que no.
#[test]
fn que_formatos_van() {
    use crate::Forma;
    assert_eq!(forma_de(0, true, true), Some(Forma::SIN_TIPO));
    assert_eq!(forma_de(42 | CUATRO_FLOATS, true, true), Some(Forma::nueva(1, true, true)));
    assert_eq!(forma_de(10 | CUATRO_FLOATS, false, true), Some(Forma::nueva(4, true, false)), "un RGBA16F que solo se lee");
    assert_eq!(forma_de(10 | CUATRO_FLOATS, true, true), None, "escrito, se cuantiza a half: por la CPU");
    assert_eq!(forma_de(28, false, true), None, "un RGBA8 empaquetado");
    assert_eq!(forma_de(41, true, true), Some(Forma::nueva(1, false, false)));
    assert_eq!(forma_de(42 | CUATRO_FLOATS, true, false), None, "un 3D o un array");
    assert_eq!(forma_de(bmo_proton_x::bufer::con_vista(26, 42), false, true), None, "otra vista");
}
