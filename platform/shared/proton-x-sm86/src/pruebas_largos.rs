//! ** LI2b (12-10): los PROGRAMAS LARGOS. El censo de Cyberpunk en el metal:
//! el de vertice de 129 a 256 instrucciones y el de pixel hasta 4096, y el
//! hueco era de 128 (una pagina). Desde LI2b el codigo vive en su region
//! (`vram::CODIGO`, 17 paginas por programa) y el juez lo lee ENTERO. La
//! prueba que pedia el plan: un programa de ~2000 instrucciones da los
//! mismos bits que la CPU, pegado y juzgado entero.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Op, Programa};

use crate::{emitir, emitir_con, Abi};

/// `x = x * 0.999 + 0.001`, `n` veces, por canal, y las cuatro a la salida.
fn largo(n: usize) -> Programa {
    let mut ops: Vec<Op> = (0..4).map(|k| Op::Entrada { d: k as u16, elemento: 0, componente: k as u8 }).collect();
    let mut ultimos = [0u16, 1, 2, 3];
    let mut d = 6u16;
    for _ in 0..n {
        for u in ultimos.iter_mut() {
            ops.push(Op::Mad { d, a: *u, b: 4, c: 5 });
            *u = d;
            d += 1;
        }
    }
    ops.extend(ultimos.iter().enumerate().map(|(k, &s)| Op::Salida { s, elemento: 0, componente: k as u8 }));
    ejemplos::programa(ops, d as usize, &[(4, 0.999), (5, 0.001)])
}

/// *** ~2000 instrucciones: los bits de la CPU en el simulador (los dos
/// ABI), el juez de la puerta del kernel con el pegamento (entero, mas de
/// 128), y el juez en vivo (`vivo::comprobar`).
#[test]
fn un_programa_de_dos_mil_da_los_bits_de_la_cpu() {
    let p = largo(500);
    let e = emitir(&p, 64).unwrap_or_else(|x| panic!("{x:?}"));
    let r = emitir_con(&p, 64, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(r.codigo.len() > 1900, "{} instrucciones", r.codigo.len());
    for x in [[1.0f32, 2.0, -3.0, 0.5], [1e30, -0.0, f32::NAN, 7.25]] {
        crate::pruebas::igual(&p, &e.codigo, &[x], &[]);
        crate::pruebas::igual_en_registros(&p, &r, &[x], &[]);
    }
    let n = crate::pruebas_indexado::por_la_puerta(&p, &[Some(0)]).unwrap_or_else(|x| panic!("{x:?}"));
    assert!(n > 1900 && n <= bmo_gpu_ga10x::sass::juez::MAX_INSTRUCCIONES, "{n} pegado y juzgado entero");
    assert!(crate::vivo::comprobar(&p, &r, "de pixel").is_ok());
    std::eprintln!("largo: {} instrucciones ({} con el ABI de registros), {n} pegado", e.codigo.len(), r.codigo.len());
}

/// *** Lo que pasa del hueco (4096) lo sigue diciendo el pegamento: va por
/// la CPU, y el censo dice cuanto mide.
#[test]
fn lo_que_no_cabe_ni_en_4096() {
    use bmo_gpu_ga10x::pegamento::NoPega;
    let p = largo(1100);
    assert_eq!(crate::pruebas_indexado::por_la_puerta(&p, &[Some(0)]), Err(NoPega::Instrucciones));
}
