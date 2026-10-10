//! ** E8f (09-10, DL18): la fila del cbuffer CALCULADA en la 3060
//! (`constantes.rs`) -- `luces.hlsl` de `dxc` de punta a punta, y a mano con
//! indices de todas las clases (dentro, el borde, fuera, los bits de un
//! negativo, de un float, de un NaN, y los enormes que el `<< 4` da la
//! vuelta), en los dos ABI, juzgados: R0..R6 y R7 con el banco de la app.
//! Los bits de la casa, y fuera de sus filas 0 aunque el banco tenga detras
//! las filas de otro.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_cuerpo_de_app, juzgar_drenado, BancoDeApp, Contexto, Permisos, Regla, RESERVADOS};
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Op, Programa};

use crate::pruebas::{igual, igual_en_registros};
use crate::{emitir, emitir_con, Abi, Emitido, BANCO_APP};

const TECHO: u32 = 64;

fn bits(x: u32) -> f32 {
    f32::from_bits(x)
}

/// El banco de la app: el cbuffer aplanado del programa, entero.
fn banco(p: &Programa) -> Permisos {
    Permisos { asas: 0, banco: Some(BancoDeApp { numero: BANCO_APP, bytes: 16 * p.filas_cb as u32 }) }
}

/// Los dos ABI, juzgados: R0..R6 los dos; R7 el de registros, con su banco.
fn emitidos(p: &Programa) -> (Emitido, Emitido) {
    let e = emitir(p, TECHO).unwrap_or_else(|x| panic!("{:?}", x));
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap_or_else(|x| panic!("{:?}", x));
    for x in [&e, &r] {
        let v = juzgar_drenado(&x.codigo, &Contexto { registros: x.registros + RESERVADOS, sph: None });
        assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(juzgar_cuerpo_con(&r.codigo, r.registros, banco(p)), Ok(()));
    (e, r)
}

/// Un cbuffer de `n` filas: la fila k, `(k, -k, k/2, 1000 + k)`.
fn cbuffer(n: u32) -> Vec<u8> {
    (0..n).flat_map(|k| [k as f32, -(k as f32), k as f32 * 0.5, 1000.0 + k as f32]).flat_map(|x| x.to_le_bytes()).collect()
}

/// Indices de todas las clases.
fn indices() -> Vec<u32> {
    let mut v: Vec<u32> = (0..10).collect();
    v.extend([0xFFFF_FFFF, 0x8000_0000, 0x7FFF_FFFF, 0x3F80_0000, 0x7FC0_0000, 1 << 16, 256, 1 << 28, (1 << 28) + 1, 0x1000_0002, 0xF000_0001]);
    v
}

/// `color = cbuffer[fila + i]` (si `i < filas`), las cuatro a la salida.
fn leer(fila: u16, filas: u16) -> Programa {
    let mut ops = vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::ConstantesEn { d: 1, fila, filas, i: 0, cb: 0 }];
    ops.extend((0..4).map(|c| Op::Salida { s: 1 + c, elemento: 0, componente: c as u8 }));
    let mut p = ejemplos::programa(ops, 5, &[]);
    p.filas_cb = fila + filas;
    p
}

/// ** A mano: un array de 3 filas desde la 2 de un cbuffer de 8 (detras y
/// delante, las filas de OTROS): cada indice, los bits de la casa.
#[test]
fn la_fila_calculada_da_los_bits_de_la_casa() {
    for (fila, filas) in [(2u16, 3u16), (0, 1), (0, 8), (5, 3), (4093, 2)] {
        let mut p = leer(fila, filas);
        // Un cbuffer MAS LARGO que el array: lo de detras es de otro.
        p.filas_cb = fila + filas + 3;
        let cb = cbuffer(p.filas_cb as u32);
        let (e, r) = emitidos(&p);
        assert!(r.usa_banco() && e.usa_banco());
        for i in indices() {
            let ent = [[bits(i), 0.0, 0.0, 0.0]];
            igual(&p, &e.codigo, &ent, &cb);
            igual_en_registros(&p, &r, &ent, &cb);
        }
        std::eprintln!("fila {fila}, {filas} filas: {} instrucciones ({} con el ABI de registros)", e.codigo.len(), r.codigo.len());
    }
}

/// ** El indice ESCRITO (`color[1]`, `color[5]` de un array de 3): el LDC
/// sin indice, o cuatro ceros; y sin filas, ceros.
#[test]
fn con_el_indice_escrito() {
    for (j, filas) in [(1u32, 3u16), (2, 3), (3, 3), (5, 3), (0, 0)] {
        let mut p = ejemplos::programa(
            {
                let mut ops = vec![Op::ConstantesEn { d: 1, fila: 2, filas, i: 0, cb: 0 }];
                ops.extend((0..4).map(|c| Op::Salida { s: 1 + c, elemento: 0, componente: c as u8 }));
                ops
            },
            5,
            &[(0, bits(j))],
        );
        p.filas_cb = 2 + filas.max(1);
        let cb = cbuffer(p.filas_cb as u32);
        let (e, r) = emitidos(&p);
        let ent = [[0.0; 4]];
        igual(&p, &e.codigo, &ent, &cb);
        igual_en_registros(&p, &r, &ent, &cb);
        // Dentro, el LDC sin indice; fuera (o sin filas), ni uno.
        assert_eq!(r.usa_banco(), j < filas as u32, "j {j}, {filas} filas");
    }
}

/// ** EN UN BUCLE, como las luces: `acc += cbuffer[k].x` para k de 0 a n,
/// con n de la entrada (y n mas alla de las filas: lo de fuera suma 0).
#[test]
fn las_luces_en_un_bucle() {
    let mut p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Copia { d: 1, a: 20 },
            Op::Copia { d: 2, a: 21 },
            Op::Bucle,
            Op::Compara { d: 3, a: 1, b: 0, como: Comparacion::MayorIgualSinSigno, entero: true },
            Op::RomperSi { c: 3, si_cero: false },
            Op::ConstantesEn { d: 4, fila: 1, filas: 5, i: 1, cb: 0 },
            Op::Add { d: 8, a: 2, b: 4 },
            Op::Copia { d: 2, a: 8 },
            Op::SumaEntera { d: 9, a: 1, b: 22 },
            Op::Copia { d: 1, a: 9 },
            Op::FinBucle,
            Op::Salida { s: 2, elemento: 0, componente: 0 },
        ],
        23,
        &[(20, bits(0)), (21, 0.0), (22, bits(1))],
    );
    p.filas_cb = 7;
    let cb = cbuffer(7);
    let (e, r) = emitidos(&p);
    for n in [0u32, 1, 3, 5, 6, 9] {
        let ent = [[bits(n), 0.0, 0.0, 0.0]];
        igual(&p, &e.codigo, &ent, &cb);
        igual_en_registros(&p, &r, &ent, &cb);
    }
}

const LUCES: &[u8] = include_bytes!("../../proton-x/prueba/luces.dxil");

/// *** `luces.hlsl` de `dxc` (`color[i & 7] + extra`: lo que pedian los
/// pixeles de Cyberpunk, un cbuffer leido con indice calculado) de punta a
/// punta en la 3060: el lector, el emisor, el juez -- R7 con el banco de la
/// app -- y el simulador, en los dos ABI, con los bits de la casa.
#[test]
fn luces_de_dxc_en_la_3060() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(LUCES).unwrap()).unwrap();
    assert!(p.ops.iter().any(|o| matches!(o, Op::ConstantesEn { .. })));
    let (e, r) = emitidos(&p);
    // El cbuffer: 8 colores y `extra`.
    let cb = cbuffer(p.filas_cb as u32);
    for i in indices() {
        // SV_Position (no se lee) y el indice, `nointerpolation uint`.
        let ent = [[0.0; 4], [bits(i), 0.0, 0.0, 0.0]];
        igual(&p, &e.codigo, &ent, &cb);
        igual_en_registros(&p, &r, &ent, &cb);
    }
    std::eprintln!("luces.hlsl: {} instrucciones ({} con el ABI de registros), {} registros", e.codigo.len(), r.codigo.len(), r.registros);
}

/// ** Los NO: el mismo cuerpo, sin el banco atado (la puerta de siempre), o
/// con un banco MAS CORTO que lo que puede leer, es R7.
#[test]
fn sin_su_banco_el_juez_dice_que_no() {
    let p = leer(2, 3);
    let r = emitir_con(&p, TECHO, Abi::Registros).unwrap();
    assert_eq!(juzgar_cuerpo_de_app(&r.codigo, r.registros).unwrap_err().regla, Regla::R7CuerpoAjeno);
    let corto = Permisos { asas: 0, banco: Some(BancoDeApp { numero: BANCO_APP, bytes: 16 * 5 - 4 }) };
    assert_eq!(juzgar_cuerpo_con(&r.codigo, r.registros, corto).unwrap_err().regla, Regla::R7CuerpoAjeno);
    let justo = Permisos { asas: 0, banco: Some(BancoDeApp { numero: BANCO_APP, bytes: 16 * 5 }) };
    assert_eq!(juzgar_cuerpo_con(&r.codigo, r.registros, justo), Ok(()));
}

/// *** POR LA PUERTA del kernel: `luces.hlsl` con el pegamento de pixel del
/// driver, el juez de programas (R0..R6 sobre el programa ENTERO: las
/// barreras del LDC contra las del pegamento) y CABE en el hueco de 128.
#[test]
fn luces_por_la_puerta_de_128() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(LUCES).unwrap()).unwrap();
    let n = crate::pruebas_indexado::por_la_puerta(&p, &[None, Some(0)]).unwrap_or_else(|x| panic!("{x:?}"));
    std::eprintln!("luces.hlsl: {n} instrucciones pegado");
}
