//! **LO QUE LA CPU LE ESCRIBE AL GSP (L0c4b2a)** -- los dos primeros
//! mensajes de la cola de la CPU, `GSP_SET_SYSTEM_INFO` y `SET_REGISTRY`, en
//! los bytes exactos de r570.144.
//!
//! capa: puro -- arma bytes en el hueco que le dan; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un campo corrido aqui es un GSP-RM que cree que
//!           su BAR0 esta en otro sitio, o que no encuentra su registro
//!
//! # Un mensaje de la CPU (nova-core, `gsp/cmdq.rs::send_single_command`)
//!
//! ```text
//!    la misma forma que los del GSP (`rpc.rs`), con:
//!      seqNum        0, 1, 2... uno por mensaje enviado
//!      elemCount     paginas de 4 KiB: (80 + datos) redondeado arriba
//!      length        32 + datos (CUENTA la cabecera del RPC)
//!      rpc_result    0xFFFFFFFF, y rpc_result_private igual
//!      checkSum      el que hace que TODO el mensaje sume 0
//! ```
//!
//! # Los dos de L0c4b2a
//!
//! `GspSystemInfo` (928 B): donde estan sus BAR, quien es en el PCI, el espejo
//! de su espacio de configuracion en BAR0 (0x88000, Turing a Ada) y hasta
//! donde llegan las direcciones de usuario -- los campos que llena nova-core,
//! el resto a 0. El registro: las tres claves de nova-core, todas DWORD a 1.
//! Van a la cola ANTES de despertar el GSP, como hacen nouveau
//! (`r535_gsp_oneinit`) y OpenRM: el GSP-RM las encuentra al mirar por
//! primera vez.

use crate::rpc::{Suma, CABECERA, CABECERA_RPC, FIRMA_VRPC, VERSION_3_0};

/// `NV_VGPU_MSG_FUNCTION_GSP_SET_SYSTEM_INFO`.
pub const SET_SYSTEM_INFO: u32 = 72;
/// `NV_VGPU_MSG_FUNCTION_SET_REGISTRY`.
pub const SET_REGISTRY: u32 = 73;

/// Lo que mide `GspSystemInfo` en r570.144.
pub const SISTEMA_BYTES: usize = 928;

/// El espejo del espacio de configuracion PCI dentro de BAR0 (Turing, Ampere,
/// Ada: `gpu/hal/tu102.rs`).
pub const ESPEJO_PCI: u32 = 0x0008_8000;
pub const ESPEJO_PCI_BYTES: u32 = 0x1000;

/// Hasta donde llegan las direcciones de usuario: 128 TiB menos una pagina,
/// como nova-core.
pub const MAX_USER_VA: u64 = (1 << 47) - 4096;

/// Las claves del registro de nova-core: la tarjeta se puede reiniciar por su
/// puente, el GSP-RM guarda su configuracion PCI al reiniciarse, y arranca
/// aunque su ID no este en su lista de productos.
pub const REGISTRO: [(&[u8], u32); 3] = [(b"RMSecBusResetEnable", 1), (b"RMForcePcieConfigSave", 1), (b"RMDevidCheckIgnore", 1)];

/// `REGISTRY_TABLE_ENTRY_TYPE_DWORD`.
const TIPO_DWORD: u8 = 1;

/// Quien es la 3060 y donde esta, leido de su espacio de configuracion PCI.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Sistema {
    /// Las direcciones FISICAS de BAR0, BAR1 y BAR3 (las `resource` 0, 1 y 3
    /// de Linux: BAR1 es de 64 bits y se come el hueco 2).
    pub bar0: u64,
    pub bar1: u64,
    pub bar3: u64,
    /// `bus << 8 | dispositivo << 3 | funcion` (`pci_dev_id`).
    pub bdf: u16,
    /// Configuracion +0x00: `dispositivo << 16 | fabricante`.
    pub id: u32,
    /// Configuracion +0x2C: `subsistema << 16 | su fabricante`.
    pub subid: u32,
    /// Configuracion +0x08, el byte bajo.
    pub revision: u8,
}

fn poner(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}

/// **Un mensaje de la CPU** en `hueco`, con `datos` bytes de datos que
/// escribe `llenar`. `Some(bytes)` que ocupa todo; `None` si no cabe en el
/// hueco o en una pagina (los dos de L0c4b2a caben de sobra).
pub fn componer(hueco: &mut [u8], numero: u32, funcion: u32, datos: usize, llenar: impl FnOnce(&mut [u8])) -> Option<usize> {
    let total = CABECERA + datos;
    if total > hueco.len() || total > 4096 {
        return None;
    }
    let m = &mut hueco[..total];
    m.fill(0);
    poner(m, 36, &numero.to_le_bytes());
    poner(m, 40, &1u32.to_le_bytes());
    poner(m, 48, &VERSION_3_0.to_le_bytes());
    poner(m, 52, &FIRMA_VRPC.to_le_bytes());
    poner(m, 56, &((CABECERA_RPC + datos) as u32).to_le_bytes());
    poner(m, 60, &funcion.to_le_bytes());
    poner(m, 64, &u32::MAX.to_le_bytes());
    poner(m, 68, &u32::MAX.to_le_bytes());
    llenar(&mut m[CABECERA..]);
    let mut s = Suma::default();
    s.mas(m);
    poner(m, 32, &s.valor().to_le_bytes());
    Some(total)
}

/// **`GSP_SET_SYSTEM_INFO`** en `hueco`.
pub fn sistema(hueco: &mut [u8], numero: u32, s: &Sistema) -> Option<usize> {
    componer(hueco, numero, SET_SYSTEM_INFO, SISTEMA_BYTES, |d| {
        poner(d, 0x00, &s.bar0.to_le_bytes());
        poner(d, 0x08, &s.bar1.to_le_bytes());
        poner(d, 0x10, &s.bar3.to_le_bytes());
        poner(d, 0x20, &(s.bdf as u64).to_le_bytes());
        poner(d, 0x48, &MAX_USER_VA.to_le_bytes());
        poner(d, 0x50, &ESPEJO_PCI.to_le_bytes());
        poner(d, 0x54, &ESPEJO_PCI_BYTES.to_le_bytes());
        poner(d, 0x58, &s.id.to_le_bytes());
        poner(d, 0x5C, &s.subid.to_le_bytes());
        poner(d, 0x60, &(s.revision as u32).to_le_bytes());
        // bIsPrimary (0x380) y bPreserveVideoMemoryAllocations (0x38C) a 0,
        // como nova-core: ya lo estan.
    })
}

/// Lo que miden los datos de `SET_REGISTRY`: la tabla (8), una entrada de 16
/// por clave, y los nombres con su 0.
pub const fn registro_bytes() -> usize {
    let mut n = 8 + 16 * REGISTRO.len();
    let mut k = 0;
    while k < REGISTRO.len() {
        n += REGISTRO[k].0.len() + 1;
        k += 1;
    }
    n
}

/// **`SET_REGISTRY`** en `hueco` (`PACKED_REGISTRY_TABLE`: `size`,
/// `numEntries`, las entradas, y detras los nombres; cada `nameOffset` cuenta
/// desde el principio de la tabla).
pub fn registro(hueco: &mut [u8], numero: u32) -> Option<usize> {
    let bytes = registro_bytes();
    componer(hueco, numero, SET_REGISTRY, bytes, |d| {
        poner(d, 0, &(bytes as u32).to_le_bytes());
        poner(d, 4, &(REGISTRO.len() as u32).to_le_bytes());
        let mut nombre = 8 + 16 * REGISTRO.len();
        for (k, &(clave, valor)) in REGISTRO.iter().enumerate() {
            let e = 8 + 16 * k;
            poner(d, e, &(nombre as u32).to_le_bytes());
            d[e + 4] = TIPO_DWORD;
            poner(d, e + 8, &valor.to_le_bytes());
            poner(d, nombre, clave);
            nombre += clave.len() + 1;
        }
    })
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::Mensaje;

    fn u32_de(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }

    fn u64_de(b: &[u8], o: usize) -> u64 {
        u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
    }

    /// Lo que se lee con el lector de L0c4a: si lo entiende y suma 0.
    fn leido(b: &[u8]) -> Mensaje {
        let m = Mensaje::de(b[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado(), "el lector de la cola lo entiende");
        let mut s = Suma::default();
        s.mas(&b[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0, "con su checkSum, el mensaje entero suma 0");
        m
    }

    const LA_3060: Sistema = Sistema {
        bar0: 0xFB00_0000,
        bar1: 0x7C_0000_0000,
        bar3: 0x7E_0000_0000,
        bdf: 0x2900,
        id: 0x2504_10DE,
        subid: 0x1234_1462,
        revision: 0xA1,
    };

    #[test]
    fn set_system_info_como_nova_core() {
        let mut h = [0xEEu8; 4096];
        let n = sistema(&mut h, 0, &LA_3060).unwrap();
        assert_eq!(n, 80 + 928);
        let m = leido(&h);
        assert_eq!((m.funcion, m.numero, m.paginas, m.largo, m.datos()), (72, 0, 1, 32 + 928, 928));
        assert_eq!((m.resultado, m.resultado_privado, m.secuencia), (u32::MAX, u32::MAX, 0));
        let d = &h[CABECERA..n];
        assert_eq!((u64_de(d, 0), u64_de(d, 8), u64_de(d, 0x10)), (0xFB00_0000, 0x7C_0000_0000, 0x7E_0000_0000));
        assert_eq!(u64_de(d, 0x18), 0, "gpuPhysIoAddr, como nova-core");
        assert_eq!(u64_de(d, 0x20), 0x2900);
        assert_eq!(u64_de(d, 0x48), 0x7FFF_FFFF_F000);
        assert_eq!((u32_de(d, 0x50), u32_de(d, 0x54)), (0x88000, 0x1000));
        assert_eq!((u32_de(d, 0x58), u32_de(d, 0x5C), u32_de(d, 0x60)), (0x2504_10DE, 0x1234_1462, 0xA1));
        assert!(d[0x64..].iter().all(|&b| b == 0), "todo lo demas a 0");
        assert_eq!(h[n], 0xEE, "no escribe mas alla de lo suyo");
    }

    #[test]
    fn set_registry_como_nova_core() {
        let mut h = [0u8; 4096];
        let n = registro(&mut h, 1).unwrap();
        let m = leido(&h);
        assert_eq!((m.funcion, m.numero, m.datos()), (73, 1, registro_bytes()));
        let d = &h[CABECERA..n];
        assert_eq!((u32_de(d, 0) as usize, u32_de(d, 4)), (registro_bytes(), 3));
        for (k, &(clave, valor)) in REGISTRO.iter().enumerate() {
            let e = 8 + 16 * k;
            let o = u32_de(d, e) as usize;
            assert_eq!(&d[o..o + clave.len()], clave);
            assert_eq!(d[o + clave.len()], 0, "cada nombre acaba en 0");
            assert_eq!((d[e + 4], u32_de(d, e + 8), u32_de(d, e + 12)), (1, valor, 0));
        }
        assert_eq!(u32_de(d, 8), 56, "el primer nombre va tras la tabla y sus 3 entradas");
        assert_eq!(registro_bytes(), 56 + 20 + 22 + 19);
    }

    #[test]
    fn lo_que_no_cabe_no_se_escribe() {
        let mut chico = [7u8; 100];
        assert_eq!(sistema(&mut chico, 0, &LA_3060), None);
        assert!(chico.iter().all(|&b| b == 7), "ni un byte");
        let mut grande = [0u8; 9000];
        assert_eq!(componer(&mut grande, 0, 1, 4096, |_| {}), None, "mas de una pagina");
    }
}
