//! **Lo que el codigo traducido LLAMA** (X2, la VELOCIDAD, 05-10): las
//! texturas y la matematica de un sombreador traducido no se copian a
//! codigo de maquina: el x86-64 de `nativo_computo` llama, por un puntero,
//! al MISMO Rust que usa el interprete (`textura.rs`, `mates.rs`). Los bits
//! salen iguales por construccion, no por parecido.
//!
//! [carril]  VERDE     datos y funciones de Rust; sin `unsafe` (la casa sigue
//!                     los punteros: es quien los pone)
//! [cuesta]  DATO      una llamada por muestra o por `sin`: el filtro cuesta
//!                     mucho mas que ella
//! [riesgo]  ESPEJO    el juez es el interprete, bit a bit
//!                     (`proton-x-casa/tests/nativo/texturas.rs`)
//! [consumo] NADA      solo cuando un sombreador traducido muestrea o hace
//!                     matematica
//!
//! ```text
//!    el contexto   `Contexto::llamadas` apunta a unas [`Llamadas`]: la
//!                  medida del cbuffer y dos punteros a funcion
//!    mate          `fn(cual, bits) -> bits` (System V: edi, esi -> eax):
//!                  [`mate_sysv`], la de `Mate::aplicar`
//!    textura       `fn(datos, registros, operacion)` (rdi, rsi, edx): la
//!                  pone la casa; sigue `datos` hasta unas [`Muestras`] y
//!                  corre la operacion `k` del programa con [`Muestras::llamar`]
//!    el MXCSR      la llamada corre con el de D3D (0x1F80: lo pone la
//!                  entrada del traducido), el de Rust en el banco; en el
//!                  metal, Rust es soft-float y no lo mira
//!    la pila       el cuerpo la tiene a 16 en cada `call` (cinco push y
//!                  `sub rsp, 16` sobre la vuelta): lo que pide System V
//! ```

use crate::dxil::leer_textura;
use crate::dxil::programa::{Op, Programa};
use crate::mates::Mate;
use crate::textura::Recursos;

/// **Lo que el codigo traducido ve para llamar fuera**: por
/// `Contexto::llamadas`. Los desplazamientos (`L_*`) los lee el codigo.
#[repr(C)]
#[derive(Debug)]
pub struct Llamadas {
    /// Los bytes del cbuffer que se le pasa: un `ConstantesEn` (fila
    /// CALCULADA: luces, huesos) lee hasta aqui y lo de fuera da 0, como el
    /// interprete.
    pub cb_bytes: u64,
    /// `extern "sysv64" fn(u32, u32) -> u32`: [`mate_sysv`].
    pub mate: usize,
    /// `extern "sysv64" fn(*mut u8, *mut f32, u32)`: la de la casa (0 si no
    /// hay: entonces el codigo no la llama, porque no se tradujo con
    /// texturas).
    pub textura: usize,
    /// Lo que la casa le pasa a `textura` (unas [`Muestras`]).
    pub datos: *mut u8,
}

pub(crate) const L_CB_BYTES: i32 = 0;
pub(crate) const L_MATE: i32 = 8;
pub(crate) const L_TEXTURA: i32 = 16;
pub(crate) const L_DATOS: i32 = 24;

impl Llamadas {
    /// Las de un cbuffer de `cb_bytes`, con la matematica y sin texturas.
    pub fn nuevas(cb_bytes: usize) -> Llamadas {
        let mate: extern "sysv64" fn(u32, u32) -> u32 = mate_sysv;
        Llamadas { cb_bytes: cb_bytes as u64, mate: mate as usize, textura: 0, datos: core::ptr::null_mut() }
    }
}

/// **Las funciones de `Op::Mate`, por su numero** (el que el codigo pasa en
/// edi). Una que falte aqui no se traduce (`indice_mate` da `None`) y va por
/// el interprete: nunca otra cuenta.
pub const MATES: [Mate; 27] = {
    use Mate::*;
    [
        Sin, Cos, Tan, Exp2, Log2, Frac, RedondoPar, Suelo, Techo, Trunca, F16aF32, F32aF16, Acos, Asin, Atan, Cosh, Senh, Tanh, EsNan, EsInf, EsFinito, EsNormal, InvierteBits, CuentaBits, PrimerBitBajo,
        PrimerBitAlto, PrimerBitAltoConSigno,
    ]
};

/// El numero de `f` en [`MATES`].
pub fn indice_mate(f: Mate) -> Option<u32> {
    MATES.iter().position(|&m| m == f).map(|i| i as u32)
}

/// **La matematica, para el codigo traducido**: `Mate::aplicar`, la del
/// interprete, sobre los bits. Un numero que no es de [`MATES`] (no lo emite
/// nadie) deja los bits como estan.
pub extern "sysv64" fn mate_sysv(cual: u32, bits: u32) -> u32 {
    match MATES.get(cual as usize) {
        Some(m) => m.aplicar(bits),
        None => bits,
    }
}

/// **Lo que ve la llamada de texturas** de UN sombreador en UN dibujo: su
/// programa (la operacion `k` se busca en el), los recursos del dibujo y
/// la textura que eligio el ultimo `EligeTextura` (N5.4: un array de
/// texturas). Quien llama al traducido la pone a `None` al empezar cada
/// vertice o pixel, como el interprete.
pub struct Muestras<'a> {
    pub programa: &'a Programa,
    pub recursos: &'a Recursos<'a>,
    pub elegida: Option<crate::textura::Elegida>,
}

impl Muestras<'_> {
    /// **Correr la operacion `k`** (`Muestra`, `Lee` o `EligeTextura`) sobre
    /// los registros, como el interprete: con [`leer_textura`], la suya.
    pub fn llamar(&mut self, regs: &mut [f32], k: u32) {
        match self.programa.ops.get(k as usize) {
            Some(&Op::EligeTextura { i, rango }) => {
                let bits = regs.get(i as usize).map_or(0, |x| x.to_bits());
                self.elegida = self.recursos.elegir(rango, bits);
            }
            Some(&op) => leer_textura(op, regs, self.recursos, self.elegida),
            None => {}
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los desplazamientos que lee el codigo son los de la estructura.
    #[test]
    fn las_llamadas_tienen_los_desplazamientos_del_codigo() {
        assert_eq!(core::mem::offset_of!(Llamadas, cb_bytes) as i32, L_CB_BYTES);
        assert_eq!(core::mem::offset_of!(Llamadas, mate) as i32, L_MATE);
        assert_eq!(core::mem::offset_of!(Llamadas, textura) as i32, L_TEXTURA);
        assert_eq!(core::mem::offset_of!(Llamadas, datos) as i32, L_DATOS);
    }

    /// Cada funcion de la lista, una vez y en su sitio; y la llamada da lo
    /// de `aplicar`.
    #[test]
    fn cada_mate_tiene_su_numero_y_da_lo_de_aplicar() {
        for (k, &m) in MATES.iter().enumerate() {
            assert_eq!(indice_mate(m), Some(k as u32), "{m:?}");
            for x in [0u32, 0x8000_0000, 0x3F80_0000, 0x7FC0_0000, 0x0000_0001, 0xC2F6_E979] {
                assert_eq!(mate_sysv(k as u32, x), m.aplicar(x));
            }
        }
        assert_eq!(mate_sysv(MATES.len() as u32, 7), 7);
    }
}
