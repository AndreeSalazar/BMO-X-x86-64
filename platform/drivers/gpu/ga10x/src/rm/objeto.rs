//! **NUESTROS OBJETOS EN EL RM (L1b)** -- `GSP_RM_ALLOC`: pedirle al GSP-RM un
//! cliente, un dispositivo y un subdispositivo PROPIOS. Todo lo que sigue
//! (espacio de direcciones, memoria, canales, el copiador, el sombreador) se
//! le pide a esos tres; las asas internas de L1a son del RM, no nuestras.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un parametro de otra medida es un
//!           `NV_ERR_INVALID_PARAM_STRUCT` y nada despues abre
//!
//! # La pregunta (r570.144, `g_rpc-structures.h` y `class/cl*.h`)
//!
//! ```text
//!    rpc_gsp_rm_alloc_v03_00   32 B: hClient, hParent, hObject, hClass,
//!                              status, paramsSize, flags, reserved[4]
//!    y sus parametros:
//!    NV01_ROOT        0x0000   NV0000_ALLOC_PARAMETERS  120 B: hClient,
//!                              processID = ~0, processName[100], pOsPidInfo
//!    NV01_DEVICE_0    0x0080   NV0080_ALLOC_PARAMETERS   56 B: deviceId 0,
//!                              hClientShare = el cliente, lo demas a cero
//!    NV20_SUBDEVICE_0 0x2080   NV2080_ALLOC_PARAMETERS    4 B: subDeviceId 0
//!    FERMI_VASPACE_A  0x90F1   NV_VASPACE_ALLOCATION_PARAMETERS 48 B (L1c1):
//!                              index 0 (GPU_NEW), flags IS_EXTERNALLY_OWNED;
//!                              vaSize +8, vaStart/LimitInternal +16/+24,
//!                              bigPageSize +32, vaBase +40, a cero
//! ```
//!
//! # El espacio de direcciones, y por que es "de fuera" (L1c1)
//!
//! Con el GSP, el RM del GSP NO lleva las tablas de paginas de un cliente:
//! las lleva quien hace de RM de la CPU -- aqui, nosotros. nouveau lo hace
//! igual (`r535/vmm.c`): pide el VASPACE con `IS_EXTERNALLY_OWNED` y luego le
//! dice al RM donde esta su directorio (`NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY`),
//! que construye EL en la VRAM. Esto es el primer paso: el objeto. El
//! directorio va detras, cuando la CPU sepa escribir en la VRAM (L1c2).
//!
//! Como nouveau (`r535/client.c`, `r535/device.c`); las asas son las suyas
//! (`rm/handles.h`), que el RM ya acepta.

use crate::orden;

/// `NV_VGPU_MSG_FUNCTION_GSP_RM_ALLOC`.
pub const GSP_RM_ALLOC: u32 = 103;
/// Lo que mide `rpc_gsp_rm_alloc_v03_00` antes de sus parametros.
pub const CABECERA_ALLOC: usize = 32;

/// Nuestro cliente: `NVKM_RM_CLIENT(0x0B)`.
pub const CLIENTE: u32 = 0xC1D0_000B;
pub const DISPOSITIVO: u32 = 0xDE1D_0000;
pub const SUBDISPOSITIVO: u32 = 0x5D1D_0000;
/// `NVKM_RM_VASPACE`.
pub const ESPACIO: u32 = 0x90F1_0000;

pub const NV01_ROOT: u32 = 0x0000;
pub const NV01_DEVICE_0: u32 = 0x0080;
pub const NV20_SUBDEVICE_0: u32 = 0x2080;
pub const FERMI_VASPACE_A: u32 = 0x90F1;
/// `NV_VASPACE_ALLOCATION_FLAGS_IS_EXTERNALLY_OWNED`.
pub const VASPACE_DE_FUERA: u32 = 1 << 3;

/// `NV_ERR_INSERT_DUPLICATE_NAME`: el asa ya existe (se pidio antes).
pub const YA_EXISTE: u32 = 0x19;

/// Nuestros objetos, en el orden en que se piden: cada uno cuelga de uno de
/// antes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Objeto {
    Cliente,
    Dispositivo,
    Subdispositivo,
    /// L1c1: el espacio de direcciones de la GPU, hijo del dispositivo.
    Espacio,
}

impl Objeto {
    pub const TODOS: [Objeto; 4] = [Objeto::Cliente, Objeto::Dispositivo, Objeto::Subdispositivo, Objeto::Espacio];

    pub fn de(n: u64) -> Option<Objeto> {
        Self::TODOS.get(n as usize).copied()
    }

    /// `(hClient, hParent, hObject, hClass, medida de los parametros)`.
    pub const fn forma(self) -> (u32, u32, u32, u32, usize) {
        match self {
            Objeto::Cliente => (CLIENTE, 0, CLIENTE, NV01_ROOT, 120),
            Objeto::Dispositivo => (CLIENTE, CLIENTE, DISPOSITIVO, NV01_DEVICE_0, 56),
            Objeto::Subdispositivo => (CLIENTE, DISPOSITIVO, SUBDISPOSITIVO, NV20_SUBDEVICE_0, 4),
            Objeto::Espacio => (CLIENTE, DISPOSITIVO, ESPACIO, FERMI_VASPACE_A, 48),
        }
    }

    pub const fn asa(self) -> u32 {
        self.forma().2
    }

    pub const fn nombre(self) -> &'static [u8] {
        match self {
            Objeto::Cliente => b"cliente",
            Objeto::Dispositivo => b"dispositivo",
            Objeto::Subdispositivo => b"subdispositivo",
            Objeto::Espacio => b"espacio",
        }
    }
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **La pregunta** en `hueco`.
pub fn pedir(hueco: &mut [u8], numero: u32, que: Objeto) -> Option<usize> {
    let (cliente, padre, asa, clase, medida) = que.forma();
    orden::componer(hueco, numero, GSP_RM_ALLOC, CABECERA_ALLOC + medida, |d| {
        poner(d, 0, cliente);
        poner(d, 4, padre);
        poner(d, 8, asa);
        poner(d, 12, clase);
        poner(d, 20, medida as u32);
        let p = &mut d[CABECERA_ALLOC..];
        match que {
            Objeto::Cliente => {
                poner(p, 0, CLIENTE);
                poner(p, 4, u32::MAX);
            }
            Objeto::Dispositivo => poner(p, 4, CLIENTE),
            Objeto::Subdispositivo => {}
            // index 0 (GPU_NEW) y el resto a cero: medida y base, los del RM.
            Objeto::Espacio => poner(p, 4, VASPACE_DE_FUERA),
        }
    })
}

/// Lo que dice la respuesta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Respuesta {
    pub asa: u32,
    pub clase: u32,
    /// El `NV_STATUS` del RM: 0 es que el objeto existe.
    pub estado: u32,
}

/// **Leer la respuesta**: los datos del mensaje (tras sus 80 B de cabecera).
pub fn leer(d: &[u8]) -> Option<Respuesta> {
    if d.len() < CABECERA_ALLOC {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    Some(Respuesta { asa: u(8), clase: u(12), estado: u(16) })
}

/// El nombre de un `NV_STATUS` de los que pueden salir aqui.
pub fn estado(e: u32) -> &'static [u8] {
    match e {
        0 => b"NV_OK",
        YA_EXISTE => b"ya existia",
        0x1B => b"sin permiso",
        0x1F => b"argumento invalido",
        0x22 => b"clase invalida",
        0x23 => b"cliente invalido",
        0x33 => b"asa invalida",
        0x36 => b"padre invalido",
        0x3A => b"parametros de otra medida",
        0x56 => b"no soportado",
        _ => b"otro NV_STATUS",
    }
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    fn u(h: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(h[o..o + 4].try_into().unwrap())
    }

    #[test]
    fn las_tres_preguntas_suman_cero_y_miden_lo_suyo() {
        for (que, medida) in Objeto::TODOS.into_iter().zip([120usize, 56, 4, 48]) {
            let mut h = [0xAAu8; 4096];
            let n = pedir(&mut h, 3, que).unwrap();
            assert_eq!(n, CABECERA + CABECERA_ALLOC + medida);
            let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
            assert!(m.bien_formado());
            assert_eq!((m.funcion, m.numero, m.datos()), (GSP_RM_ALLOC, 3, CABECERA_ALLOC + medida));
            let mut s = Suma::default();
            s.mas(&h[..m.bytes_sumados()]);
            assert_eq!(s.valor(), 0);
            assert_eq!(u(&h, CABECERA + 20) as usize, medida, "paramsSize");
            assert_eq!(u(&h, CABECERA + 16), 0, "status a cero");
        }
    }

    #[test]
    fn el_arbol_cliente_dispositivo_subdispositivo() {
        let mut h = [0u8; 4096];
        pedir(&mut h, 2, Objeto::Cliente).unwrap();
        let d = &h[CABECERA..];
        assert_eq!((u(d, 0), u(d, 4), u(d, 8), u(d, 12)), (CLIENTE, 0, CLIENTE, NV01_ROOT));
        assert_eq!((u(d, 32), u(d, 36)), (CLIENTE, u32::MAX), "hClient y processID = ~0");

        pedir(&mut h, 3, Objeto::Dispositivo).unwrap();
        let d = &h[CABECERA..];
        assert_eq!((u(d, 0), u(d, 4), u(d, 8), u(d, 12)), (CLIENTE, CLIENTE, DISPOSITIVO, NV01_DEVICE_0));
        assert_eq!((u(d, 32), u(d, 36)), (0, CLIENTE), "deviceId 0, hClientShare");

        pedir(&mut h, 4, Objeto::Subdispositivo).unwrap();
        let d = &h[CABECERA..];
        assert_eq!((u(d, 0), u(d, 4), u(d, 8), u(d, 12)), (CLIENTE, DISPOSITIVO, SUBDISPOSITIVO, NV20_SUBDEVICE_0));
        assert_eq!(u(d, 32), 0, "subDeviceId 0");

        pedir(&mut h, 5, Objeto::Espacio).unwrap();
        let d = &h[CABECERA..];
        assert_eq!((u(d, 0), u(d, 4), u(d, 8), u(d, 12)), (CLIENTE, DISPOSITIVO, ESPACIO, FERMI_VASPACE_A));
        assert_eq!((u(d, 32), u(d, 36)), (0, VASPACE_DE_FUERA), "GPU_NEW y de fuera");
        assert!(d[40..80].iter().all(|&b| b == 0), "medida y base: los del RM");
    }

    #[test]
    fn la_respuesta_y_sus_nombres() {
        let mut d = [0u8; 36];
        d[8..12].copy_from_slice(&DISPOSITIVO.to_le_bytes());
        d[12..16].copy_from_slice(&NV01_DEVICE_0.to_le_bytes());
        d[16..20].copy_from_slice(&YA_EXISTE.to_le_bytes());
        assert_eq!(leer(&d), Some(Respuesta { asa: DISPOSITIVO, clase: 0x80, estado: YA_EXISTE }));
        assert_eq!(estado(YA_EXISTE), b"ya existia");
        assert_eq!(leer(&d[..8]), None);
        assert_eq!(Objeto::de(2), Some(Objeto::Subdispositivo));
        assert_eq!(Objeto::de(3), Some(Objeto::Espacio));
        assert_eq!(Objeto::de(4), None);
    }
}
