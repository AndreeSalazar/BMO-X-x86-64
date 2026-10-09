//! # bmo-tarjeta-rtx3060-12g -- la RTX 3060 12G: una tarjeta de PROMETEO por su APARATO exacto
//!
//! generacion: nieto -- la hermana de la tarjeta de la CPU (`tarjeta-cpu`)
//!
//! 08-10, el propietario: *"que la GPU no sea por ISA sea por especificamente
//! muy precisos no es ISA sino propio ISA modular eso y ya con eso es mas
//! facil de detectar porque falla ... (solo: 3060 12G)"*. Una tarjeta de
//! PROMETEO es UNA grafica concreta, no una familia de ISA: sm_86 es la lengua
//! de toda Ampere GA10x, pero esto es la RTX 3060 12G y ninguna otra -- como su
//! driver (`bmo-gpu-ga10x`, "LA 3060 12G, Y SOLO ELLA", 25-09) y su PERFIL
//! (`PERFIL/GPU_3060.txt`, 28-09). Asi cada NO dice el aparato exacto, y sus
//! techos son los de ESA grafica.
//!
//! ```text
//!    EL APARATO   NVIDIA GeForce RTX 3060 12G, GA106, 10DE:2503 o 10DE:2504,
//!                 12288 MiB: las constantes del driver (`lectura::identidad`)
//!    SU ISA       SASS sm_86, un modulo que lleva dentro (`isa`): el emisor de
//!                 PROTON-X (`bmo-proton-x-sm86`) y su simulador (`simula`)
//!    SU JUEZ      el juez del SASS del driver (esperas, barreras, registros),
//!                 y sobre lo que viaja, la regla de un cuerpo de app (R7)
//!
//!    Para::Oraculo   Abi::Banco       las entradas en c[1]; lo corre `simula`
//!    Para::Viaje     Abi::Registros   las entradas ya en registros (las carga
//!                                     el pegamento del driver, E5)
//! ```
//!
//! Nacio el 08-10 dentro de su emisor, como `proton-x-sm86/src/tarjeta.rs`
//! (LB3), con el nombre de su ISA (`SM86`); el mismo dia salio a su crate con
//! el de su aparato, al lado de la tarjeta de la CPU. Ni un bit del SASS
//! cambio.
//!
//! AISLADA, como pidio el propietario (*"TODAS LAS GPU en emisor SON AISLADAS
//! por completo"*): esto es de la RTX 3060 12G y de nadie mas. PROMETEO no la
//! nombra; quien arma una herramienta (`titan`) dice que esta.

use bmo_gpu_ga10x::lectura::identidad::{DISPOSITIVOS, NVIDIA, VRAM_MIB};
use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_prometeo::programa::Op;
use bmo_prometeo::{Aparato, Codigo, Ficha, NoEmite as No, Para, Pci, Programa, Tarjeta};

use isa::simula::{correr, Maquina};
use isa::{emitir_con, Abi, NoEmite};

/// ** SU ISA, un modulo SUYO: SASS sm_86 -- el emisor de PROTON-X y su
/// simulador. La lengua es de toda Ampere GA10x; la tarjeta, de esta grafica.
pub use bmo_proton_x_sm86 as isa;

/// ** LB6 (08-10): lo que DIBUJA -- una gpu fn de vertice o de pixel pegada a
/// la tuberia de VERRANO, juzgada, y las de un paquete en su sobre.
pub mod dibujo;

/// Como la nombra la casa: el aparato exacto.
pub const NOMBRE: &str = "la RTX 3060 12G";
/// Su codigo maquina.
pub const LENGUA: &str = "SASS sm_86";

/// ** EL APARATO, del driver (`lectura::identidad`): las mismas constantes con
/// las que el kernel la reconoce, y las que mide su PERFIL
/// (`PERFIL/GPU_3060.txt`; el guardian `perfil-campos` las compara).
pub const APARATO: Aparato = Aparato { modelo: "NVIDIA GeForce RTX 3060 12G", chip: "GA106", pci: Some(Pci { fabricante: NVIDIA, dispositivos: &DISPOSITIVOS }), memoria_mib: VRAM_MIB };

/// Los registros que se le dan a un cuerpo: los de un hueco de la tuberia de
/// la RTX 3060 12G, de donde los cuenta el driver.
pub const REGISTROS: u32 = bmo_gpu_ga10x::trabajos::tuberia::REGISTROS;

/// ** LA DIVISION, el limite de la RTX 3060 12G hoy: su MUFU.RCP y su FMUL no
/// dan los bits exactos de la casa (E3 de PLAN_LA_LENGUA_DE_LA_3060). No es un
/// fallo del emisor: es lo que esta tarjeta todavia no sabe, dicho a proposito.
const DIVISION_QUE: &str = "una division que la RTX 3060 12G todavia no hace exacta";
const DIVISION_POR_QUE: &str = "la division de la RTX 3060 12G (MUFU.RCP y FMUL) no da los bits exactos de la casa";

/// **La RTX 3060 12G, como tarjeta de PROMETEO.** Sin estado: lo que sabe esta
/// en su ISA (su emisor y su simulador) y en su juez.
pub struct Rtx3060_12g;

/// La que usan las herramientas.
pub static RTX_3060_12G: Rtx3060_12g = Rtx3060_12g;

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
        return Err(format!("{} bytes no son instrucciones de 128 bits: no es un codigo de la RTX 3060 12G", bytes.len()));
    }
    let palabra = |b: &[u8]| b.iter().rev().fold(0u64, |acc, x| (acc << 8) | *x as u64);
    Ok(bytes.chunks_exact(16).map(|i| (palabra(&i[..8]), palabra(&i[8..]))).collect())
}

impl Tarjeta for Rtx3060_12g {
    fn ficha(&self) -> Ficha {
        Ficha { nombre: NOMBRE, lengua: LENGUA, registros: REGISTROS, aparato: APARATO }
    }

    fn emitir(&self, p: &Programa, para: Para) -> Result<Codigo, No> {
        let e = emitir_con(p, REGISTROS, abi(para)).map_err(|e| match e {
            NoEmite::Operacion(i) if matches!(p.ops.get(i), Some(Op::Div { .. })) => {
                No::Limite { op: i, que: DIVISION_QUE.to_string(), por_que: DIVISION_POR_QUE.to_string() }
            }
            NoEmite::Operacion(i) => No::Fallo { op: Some(i), por_que: format!("el emisor de {} ({}) dijo que no: {:?}", NOMBRE, LENGUA, e) },
            NoEmite::Registros => No::Fallo { op: None, por_que: format!("no cabe en los {} registros de un hueco de {}", REGISTROS, NOMBRE) },
            e => No::Fallo { op: None, por_que: format!("el emisor de {} ({}) dijo que no: {:?}", NOMBRE, LENGUA, e) },
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
        correr(&codigo, &mut m).map_err(|e| format!("el simulador de {}: {:?}", NOMBRE, e))?;
        // La salida e, componente k, en el registro R(4 e + k) al EXIT.
        Ok((0..salidas).map(|e| [m.r[4 * e], m.r[4 * e + 1], m.r[4 * e + 2], m.r[4 * e + 3]]).collect())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los bytes van y vuelven sin perder un bit, y lo que no es de 128 bits
    /// no se toma por un codigo de la RTX 3060 12G.
    #[test]
    fn the_code_goes_to_bytes_and_back_bit_for_bit() {
        let codigo = [(0x0123_4567_89AB_CDEFu64, 0xFEDC_BA98_7654_3210u64), (u64::MAX, 0)];
        let bytes = a_bytes(&codigo);
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[..8], &0x0123_4567_89AB_CDEFu64.to_le_bytes());
        assert_eq!(de_bytes(&bytes).unwrap(), codigo.to_vec());
        assert!(de_bytes(&bytes[..15]).is_err());
    }

    /// ** EL APARATO EXACTO: la ficha dice la grafica y no su ISA, y la
    /// reconoce por los ids del driver -- la 2504 del propietario y la 2503 --
    /// y a ninguna otra: ni una 3060 de 8 GiB, ni una 3060 Ti, ni otra marca.
    #[test]
    fn the_card_is_the_exact_device_and_knows_it_by_its_ids() {
        let f = RTX_3060_12G.ficha();
        assert_eq!((f.nombre, f.lengua), ("la RTX 3060 12G", "SASS sm_86"));
        assert_eq!((f.aparato.chip, f.aparato.memoria_mib), ("GA106", 12288));
        assert!(f.aparato.es(0x10DE, 0x2504) && f.aparato.es(0x10DE, 0x2503));
        // una 3060 de 8 GiB (2487), una 3060 Ti (2489), otra marca con el mismo numero
        assert!(!f.aparato.es(0x10DE, 0x2487) && !f.aparato.es(0x10DE, 0x2489) && !f.aparato.es(0x1002, 0x2504));
    }
}
