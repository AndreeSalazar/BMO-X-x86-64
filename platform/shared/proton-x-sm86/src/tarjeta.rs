//! **LA LIBRERIA DE LA 3060 para PROMETEO** (LB3 de
//! `docs/plan/PLAN_LAS_LIBRERIAS.md`, 08-10): lo que ya existia -- el emisor
//! de este crate en sus dos ABI, el juez del SASS de `bmo-gpu-ga10x` y el
//! simulador (`simula`) --, junto detras del contrato de una tarjeta
//! (`bmo_prometeo::Tarjeta`). Ni una regla nueva ni un bit distinto: el SASS
//! que sale por aqui es, byte a byte, el que salia antes por `bmo-titan-sm86`.
//!
//! [carril]  VERDE     junta piezas que ya estaban: no cuenta nada nuevo
//!
//! ```text
//!    Para::Oraculo   Abi::Banco       las entradas en c[1]; lo corre `simula`
//!    Para::Viaje     Abi::Registros   las entradas ya en registros (las carga
//!                                     el pegamento del driver, E5)
//!    juzgar          juzgar_drenado sobre cualquiera; y sobre el que viaja,
//!                    ademas, la regla de un cuerpo de app (R7)
//! ```
//!
//! AISLADA, como pidio el propietario (*"TODAS LAS GPU en emisor SON AISLADAS
//! por completo"*): esto es de la 3060 y de nadie mas. PROMETEO no lo nombra;
//! quien arma una herramienta (`titan`) dice que la 3060 esta.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_prometeo::programa::Op;
use bmo_prometeo::{Codigo, Ficha, NoEmite as No, Para, Programa, Tarjeta};

use crate::simula::{correr, Maquina};
use crate::{emitir_con, Abi, NoEmite};

/// Los registros que se le dan a un cuerpo: los de un hueco de la tuberia de
/// la 3060, de donde los cuenta el driver.
pub const REGISTROS: u32 = bmo_gpu_ga10x::trabajos::tuberia::REGISTROS;

/// ** LA DIVISION, el limite de la 3060 hoy: su MUFU.RCP y su FMUL no dan los
/// bits exactos de la casa (E3 de PLAN_LA_LENGUA_DE_LA_3060). No es un fallo
/// del emisor: es lo que esta tarjeta todavia no sabe, dicho a proposito.
const DIVISION_QUE: &str = "una division que la 3060 todavia no hace exacta";
const DIVISION_POR_QUE: &str = "la division de la 3060 (MUFU.RCP y FMUL) no da los bits exactos de la casa";

/// **La 3060, como tarjeta de PROMETEO.** Sin estado: lo que sabe esta en su
/// emisor, su juez y su simulador.
pub struct Sm86;

/// La que usan las herramientas.
pub static SM86: Sm86 = Sm86;

fn abi(para: Para) -> Abi {
    match para {
        Para::Oraculo => Abi::Banco,
        Para::Viaje => Abi::Registros,
    }
}

/// Las instrucciones de 128 bits en bytes, como viajan: la mitad baja y la
/// alta, cada una en little endian.
fn a_bytes(codigo: &[(u64, u64)]) -> Vec<u8> {
    codigo.iter().flat_map(|(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect()
}

/// Y de vuelta: un codigo de ESTA tarjeta, o por que no lo es.
fn de_bytes(bytes: &[u8]) -> Result<Vec<(u64, u64)>, String> {
    if bytes.len() % 16 != 0 {
        return Err(format!("{} bytes no son instrucciones de 128 bits: no es un codigo de la 3060", bytes.len()));
    }
    let palabra = |b: &[u8]| b.iter().rev().fold(0u64, |acc, x| (acc << 8) | *x as u64);
    Ok(bytes.chunks_exact(16).map(|i| (palabra(&i[..8]), palabra(&i[8..]))).collect())
}

impl Tarjeta for Sm86 {
    fn ficha(&self) -> Ficha {
        Ficha { nombre: "la 3060", lengua: "SM86", registros: REGISTROS }
    }

    fn emitir(&self, p: &Programa, para: Para) -> Result<Codigo, No> {
        let e = emitir_con(p, REGISTROS, abi(para)).map_err(|e| match e {
            NoEmite::Operacion(i) if matches!(p.ops.get(i), Some(Op::Div { .. })) => {
                No::Limite { op: i, que: DIVISION_QUE.to_string(), por_que: DIVISION_POR_QUE.to_string() }
            }
            NoEmite::Operacion(i) => No::Fallo { op: Some(i), por_que: format!("el emisor de la 3060 dijo que no: {:?}", e) },
            NoEmite::Registros => No::Fallo { op: None, por_que: format!("no cabe en los {} registros de un hueco de la 3060", REGISTROS) },
            e => No::Fallo { op: None, por_que: format!("el emisor de la 3060 dijo que no: {:?}", e) },
        })?;
        Ok(Codigo { bytes: a_bytes(&e.codigo), instrucciones: e.codigo.len(), registros: e.registros })
    }

    fn juzgar(&self, c: &Codigo, para: Para) -> Result<(), String> {
        let codigo = de_bytes(&c.bytes)?;
        juzgar_drenado(&codigo, &Contexto { registros: c.registros + RESERVADOS, sph: None }).map_err(|b| format!("{}", b))?;
        if para == Para::Viaje {
            juzgar_cuerpo_de_app(&codigo, c.registros).map_err(|b| format!("{}", b))?;
        }
        Ok(())
    }

    fn simular(&self, c: &Codigo, entradas: &[[u32; 4]], salidas: usize) -> Result<Vec<[u32; 4]>, String> {
        let codigo = de_bytes(&c.bytes)?;
        // El ABI del banco: la entrada e, componente k, en c[1][16 e + 4 k].
        let banco: Vec<u8> = entradas.iter().flat_map(|e| e.iter().flat_map(|x| x.to_le_bytes())).collect();
        let mut m = Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
        correr(&codigo, &mut m).map_err(|e| format!("el simulador de la 3060: {:?}", e))?;
        // La salida e, componente k, en el registro R(4 e + k) al EXIT.
        Ok((0..salidas).map(|e| [m.r[4 * e], m.r[4 * e + 1], m.r[4 * e + 2], m.r[4 * e + 3]]).collect())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los bytes van y vuelven sin perder un bit, y lo que no es de 128 bits
    /// no se toma por un codigo de la 3060.
    #[test]
    fn the_code_goes_to_bytes_and_back_bit_for_bit() {
        let codigo = [(0x0123_4567_89AB_CDEFu64, 0xFEDC_BA98_7654_3210u64), (u64::MAX, 0)];
        let bytes = a_bytes(&codigo);
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[..8], &0x0123_4567_89AB_CDEFu64.to_le_bytes());
        assert_eq!(de_bytes(&bytes).unwrap(), codigo.to_vec());
        assert!(de_bytes(&bytes[..15]).is_err());
    }
}
