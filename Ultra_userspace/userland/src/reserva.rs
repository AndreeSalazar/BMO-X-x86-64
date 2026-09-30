//! **LA RESERVA, desde Ring 3** (P0.4c, 30-09): la memoria que un juego pide
//! EN MARCHA. La usa la casa de PROTON-X para `VirtualAlloc` y sus montones.
//!
//! ```text
//!    reserva::hacer(va, bytes)      las paginas de [va, va+bytes) que faltan,
//!                                   a cero, R+W, sin X; hasta MAX_POR_VEZ;
//!                                   juzgado contra la RAM libre de ahora
//!    reserva::deshacer(va, bytes)   de vuelta al kernel, a cero
//! ```
//!
//! Elegir las direcciones (reservar) es cosa de quien llama, dentro de
//! [`VENTANA_BASE`]..+[`VENTANA_BYTES`]: no le cuesta nada al kernel.

use crate::*;

/// La ventana de reserva (espejo de `bmo_abi::...::RESERVA_VENTANA_*`).
pub const VENTANA_BASE: u64 = 0x0000_0020_0000_0000;
pub const VENTANA_BYTES: u64 = 384 << 30;
/// Lo mas que se hace o deshace en una llamada.
pub const MAX_POR_VEZ: u64 = 64 << 20;

/// Por que no: el motivo (`RESERVA_*` de `bmo-abi`) y su valor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoReserva {
    pub motivo: u32,
    pub valor: u64,
}

impl NoReserva {
    /// La frase del NO (sin alloc: los numeros van en `valor`).
    pub fn frase(&self) -> &'static str {
        match self.motivo {
            1 => "0 bytes, o sin alinear a pagina",
            2 => "fuera de la ventana de reserva",
            3 => "de mas de una vez (valor: lo mas, en bytes)",
            4 => "no hay RAM: pide / hay, en MiB (lo libre menos el margen del kernel)",
            5 => "el kernel se quedo sin marcos a mitad (valor: los bytes que SI se hicieron)",
            6 => "un mapeo fallo a mitad (valor: los bytes que SI se hicieron)",
            _ => "un motivo que este userland no conoce",
        }
    }
}

fn no(st: Status) -> NoReserva {
    NoReserva { motivo: if st.flags == 0 { st.code } else { st.flags }, valor: st.value }
}

/// **HACER** las paginas que faltan de `[va, va + bytes)`: los bytes nuevos.
pub fn hacer(va: u64, bytes: u64) -> Result<u64, NoReserva> {
    let st = invoke(CURRENT_TASK, OP_RESERVA_HACER, va, bytes, 0);
    if st.ok() {
        Ok(st.value)
    } else {
        Err(no(st))
    }
}

/// **DESHACER** las paginas de `[va, va + bytes)`: los bytes devueltos.
pub fn deshacer(va: u64, bytes: u64) -> Result<u64, NoReserva> {
    let st = invoke(CURRENT_TASK, OP_RESERVA_DESHACER, va, bytes, 0);
    if st.ok() {
        Ok(st.value)
    } else {
        Err(no(st))
    }
}
