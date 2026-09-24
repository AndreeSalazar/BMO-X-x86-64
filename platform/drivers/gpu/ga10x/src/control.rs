//! **PREGUNTARLE AL RM POR UN OBJETO NUESTRO** -- `GSP_RM_CONTROL`: una orden
//! de control (`NV2080_CTRL_CMD_*`) sobre nuestro subdispositivo de L1b. Hoy
//! una sola: el P-state, lo publico que dice cuanto corre la 3060.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- los parametros van en la medida exacta del struct:
//!           otra es `NV_ERR_INVALID_PARAM_STRUCT`
//!
//! # La pregunta (r570.144, `g_rpc-structures.h`)
//!
//! ```text
//!    rpc_gsp_rm_control_v03_00   24 B: hClient, hObject, cmd, status,
//!                                paramsSize, flags; y los parametros
//!    PERF_GET_CURRENT_PSTATE     0x20802068, 4 B: currPstate, una mascara
//!                                NV2080_CTRL_PERF_PSTATES_P0 = 1 .. P15 = 0x8000
//! ```

use crate::objeto::{CLIENTE, SUBDISPOSITIVO};
use crate::orden;

/// `NV_VGPU_MSG_FUNCTION_GSP_RM_CONTROL`.
pub const GSP_RM_CONTROL: u32 = 76;
pub const CABECERA_CONTROL: usize = 24;

/// Las ordenes de control que se piden. Solo las de la lista: el kernel no
/// arma otra (ver `contrato`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Pstate,
}

impl Control {
    pub const TODOS: [Control; 1] = [Control::Pstate];

    pub fn de(n: u64) -> Option<Control> {
        Self::TODOS.get(n as usize).copied()
    }

    /// `(cmd, medida de los parametros)`.
    pub const fn forma(self) -> (u32, usize) {
        match self {
            Control::Pstate => (0x2080_2068, 4),
        }
    }
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **La pregunta** en `hueco`, sobre nuestro subdispositivo.
pub fn pedir(hueco: &mut [u8], numero: u32, c: Control) -> Option<usize> {
    let (cmd, medida) = c.forma();
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL + medida, |d| {
        poner(d, 0, CLIENTE);
        poner(d, 4, SUBDISPOSITIVO);
        poner(d, 8, cmd);
        poner(d, 16, medida as u32);
    })
}

/// Lo que dice la respuesta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Respuesta {
    pub cmd: u32,
    pub estado: u32,
    /// El primer `u32` de los parametros (el P-state, en `Pstate`).
    pub valor: u32,
}

/// **Leer la respuesta**: los datos del mensaje (tras sus 80 B de cabecera).
pub fn leer(d: &[u8]) -> Option<Respuesta> {
    if d.len() < CABECERA_CONTROL + 4 {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    Some(Respuesta { cmd: u(8), estado: u(12), valor: u(CABECERA_CONTROL) })
}

/// **El P-state** de la mascara: `Some(0)` es P0 (lo mas rapido), `Some(8)` P8
/// (reposo). `None` si no dice ninguno o dice mas de uno.
pub fn pstate(mascara: u32) -> Option<u8> {
    (mascara.count_ones() == 1).then(|| mascara.trailing_zeros() as u8)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn la_pregunta_del_pstate() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 7, Control::Pstate).unwrap();
        assert_eq!(n, CABECERA + 24 + 4);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.datos()), (GSP_RM_CONTROL, 28));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        let d = &h[CABECERA..];
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        assert_eq!((u(0), u(4), u(8), u(12), u(16), u(20), u(24)), (CLIENTE, SUBDISPOSITIVO, 0x2080_2068, 0, 4, 0, 0));
    }

    #[test]
    fn la_respuesta_y_el_pstate() {
        let mut d = [0u8; 28];
        d[8..12].copy_from_slice(&0x2080_2068u32.to_le_bytes());
        d[24..28].copy_from_slice(&0x100u32.to_le_bytes());
        let r = leer(&d).unwrap();
        assert_eq!((r.cmd, r.estado, pstate(r.valor)), (0x2080_2068, 0, Some(8)));
        assert_eq!(pstate(1), Some(0));
        assert_eq!(pstate(0), None);
        assert_eq!(pstate(0x101), None);
        assert_eq!(Control::de(1), None);
    }
}
