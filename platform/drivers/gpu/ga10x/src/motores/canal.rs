//! **EL CANAL (L1d2b)** -- `AMPERE_CHANNEL_GPFIFO_A`: la cola por la que la
//! GPU recibe ordenes SIN pasar por el GSP-RM. Se le pide al RM una vez; despues
//! se le escribe en su GPFIFO y se toca su timbre (L1d2d).
//!
//! capa: puro -- arma los parametros; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- 368 B exactos (`NV_CHANNEL_ALLOC_PARAMS` de la
//!           r570.144): otra medida es `NV_ERR_INVALID_PARAM_STRUCT`
//!
//! # Los parametros (r570.144, `alloc/alloc_channel.h`)
//!
//! ```text
//!    +0   hObjectError, hObjectBuffer          0: sin notificador
//!    +8   gpFifoOffset (u64)                   la VA del GPFIFO en el tramo
//!    +16  gpFifoEntries                        512 (una pagina de 8 B)
//!    +20  flags (NVOS04_FLAGS_*)               USERD pagina/indice FIJOS
//!    +24  hContextShare, hVASpace              0, el espacio de L1c1
//!    +32  hUserdMemory[8], +64 userdOffset[8]  0: el USERD va por direccion
//!    +128 engineType, cid, subDeviceId, hObjectEccError
//!    +144 instanceMem  +168 userdMem  +192 ramfcMem  +216 mthdbufMem
//!         (NV_MEMORY_DESC_PARAMS, 24 B: base, size, addressSpace, cacheAttrib)
//!    +240 hPhysChannelGroup, +244 internalFlags
//!    +248 errorNotifierMem, +272 eccErrorNotifierMem   a cero
//!    +296 ProcessID, SubProcessID, +304 encryptIv[3], decryptIv[3],
//!         hmacNonce[8], +360 tpcConfigID       a cero (sin CC)
//!    368  la medida, alineada a 8
//! ```
//!
//! Como nouveau (`gsp/rm/r570/fifo.c::r570_chan_alloc`): el canal cuelga del
//! DISPOSITIVO, su asa es `NVKM_RM_CHAN(chid)` y el RM crea solo su grupo. La
//! instancia, el RAMFC y el USERD van en VRAM por direccion FISICA (espacio 2);
//! el bufer de metodos, en la RAM del PC por la IOMMU (espacio 1).
//!
//! # Por que COPY2 y no COPY0
//!
//! En GA10x las LCE 0 y 1 son **GRCE**, las copias atadas al motor 3D
//! (`kernel_ce_ga102.c`: `NV_CE_GRCE_ALLOWED_LCE_MASK 0x03`). Un canal propio de
//! copia va a la primera LCE asincrona: COPY2 (`NV2080_ENGINE_TYPE_COPY2`).
//!
//! Todo es FIJO -- la memoria, el motor, el espacio --, y el contrato compara
//! los 368 B uno a uno: ni el kernel puede pedir otro canal.

use crate::objeto::{DISPOSITIVO, ESPACIO};
use crate::vram::{TRAMO, TRAMO_VA};

/// `AMPERE_CHANNEL_GPFIFO_A`.
pub const AMPERE_CHANNEL_GPFIFO_A: u32 = 0xC56F;
/// El numero del canal. El 0 lo reserva el RM (`rsvd_chids = 1` en nouveau).
pub const CHID: u32 = 1;
/// Su asa: `NVKM_RM_CHAN(chid)` de nouveau.
pub const CANAL: u32 = 0xF1F0_0000 | CHID;
/// `sizeof(NV_CHANNEL_ALLOC_PARAMS)` en la r570.144.
pub const MEDIDA: usize = 368;

/// `NV2080_ENGINE_TYPE_COPY2`: la primera LCE que no es GRCE.
pub const MOTOR: u32 = 0x0B;

/// El bloque de instancia (4 KiB, `gf100_chan_inst`), y el RAMFC en su
/// principio (0x200): la primera pagina del tramo.
pub const INSTANCIA: u64 = TRAMO;
pub const INSTANCIA_MEDIDA: u64 = 0x1000;
pub const RAMFC_MEDIDA: u64 = 0x200;
/// El USERD (0x200 por canal, `gv100_chan_userd`): la pagina 1 del tramo es
/// la pagina de USERD 0, y el canal `CHID` ocupa su hueco `CHID % 8`.
pub const USERD_PAGINA: u64 = TRAMO + 0x1000;
pub const USERD_MEDIDA: u64 = 0x200;
pub const USERD: u64 = USERD_PAGINA + USERD_MEDIDA * (CHID % 8) as u64;
/// El GPFIFO: la pagina 2 del tramo, vista por la GPU en su VA.
pub const GPFIFO: u64 = TRAMO + 0x2000;
pub const GPFIFO_VA: u64 = TRAMO_VA + 0x2000;
pub const GPFIFO_ENTRADAS: u32 = 512;
/// Las paginas del tramo que el canal usa y se ponen a cero antes de pedirlo.
pub const PAGINAS_A_CERO: [u64; 3] = [INSTANCIA, USERD_PAGINA, GPFIFO];

/// Donde ve la 3060 el bufer de metodos (libre: 0x3B00_0000 es el booter).
pub const IOVA_METODOS: u64 = 0x3A00_0000;
/// Lo que mide, segun `CE_GET_FAULT_METHOD_BUFFER_SIZE` en el Ryzen (24-09
/// 13:00: 20480 B). Fijo: si el RM dice otra cosa, el canal NO se pide.
pub const METODOS: u64 = 0x5000;

/// `ADDR_SYSMEM` y `ADDR_FBMEM`.
const SISTEMA: u32 = 1;
const VIDEO: u32 = 2;

/// Los `flags` (`NVOS04_FLAGS_*`): canal fisico, de USUARIO, sin retrasar el
/// planificador; el USERD en la pagina y el hueco FIJOS del chid (como
/// nouveau: indice `chid % 8` en 10:8, pagina `chid / 8` en 20:12, y el bit 21).
pub const FLAGS: u32 = (CHID % 8) << 8 | (CHID / 8) << 12 | 1 << 21;
/// `internalFlags`: PRIVILEGE USER (1:0 = 0) y los dos notificadores de error
/// NONE (`ERROR_NOTIFIER_TYPE_NONE` = 1, en 3:2 y 5:4).
pub const INTERNOS: u32 = 1 << 2 | 1 << 4;

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn poner64(d: &mut [u8], o: usize, v: u64) {
    d[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

fn memoria(d: &mut [u8], o: usize, base: u64, medida: u64, espacio: u32, cache: u32) {
    poner64(d, o, base);
    poner64(d, o + 8, medida);
    poner(d, o + 16, espacio);
    poner(d, o + 20, cache);
}

/// **Un canal nuestro**, todo fijo: su chid, su asa, su motor y donde vive.
/// El de COPIA (L1d2b, VISTO en el metal) y el de GR0 (M5 G1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canal {
    pub chid: u32,
    pub asa: u32,
    /// `NV2080_ENGINE_TYPE_*`.
    pub motor: u32,
    pub instancia: u64,
    pub userd: u64,
    pub gpfifo: u64,
    pub gpfifo_va: u64,
    pub iova_metodos: u64,
}

/// El de copia (L1d2b): lo que dicen las constantes de arriba.
pub const COPIA: Canal = Canal {
    chid: CHID,
    asa: CANAL,
    motor: MOTOR,
    instancia: INSTANCIA,
    userd: USERD,
    gpfifo: GPFIFO,
    gpfifo_va: GPFIFO_VA,
    iova_metodos: IOVA_METODOS,
};

/// **El de GR0 (M5 G1)**: chid 2, en la lista 0 (la de GR0 en la tabla del
/// metal, 24-09 14:47). La instancia en la pagina 5 del tramo, el USERD en el
/// hueco 2 de la pagina de USERD, el GPFIFO en la 6, y su bufer de metodos en
/// otros 5 marcos de la RAM del PC, en 0x3A01_0000. Canal de USUARIO, como el
/// de copia (nouveau hace el contexto de oro en uno privilegiado: si el RM lo
/// pide, el metal lo dira en G3/G4).
pub const GR: Canal = Canal {
    chid: 2,
    asa: 0xF1F0_0000 | 2,
    motor: 0x01,
    instancia: TRAMO + 5 * 0x1000,
    userd: USERD_PAGINA + USERD_MEDIDA * 2,
    gpfifo: TRAMO + 6 * 0x1000,
    gpfifo_va: TRAMO_VA + 6 * 0x1000,
    iova_metodos: 0x3A01_0000,
};

impl Canal {
    /// Los `flags` (`NVOS04_FLAGS_*`) de ESTE canal: como [`FLAGS`] con su chid.
    pub const fn flags(&self) -> u32 {
        (self.chid % 8) << 8 | (self.chid / 8) << 12 | 1 << 21
    }

    /// `(hClient, hParent, hObject, hClass, medida)`.
    pub const fn forma(&self) -> (u32, u32, u32, u32, usize) {
        (crate::objeto::CLIENTE, DISPOSITIVO, self.asa, AMPERE_CHANNEL_GPFIFO_A, MEDIDA)
    }

    /// **Los 368 B**, exactos.
    pub fn parametros(&self, p: &mut [u8]) -> usize {
        p[..MEDIDA].fill(0);
        poner64(p, 8, self.gpfifo_va);
        poner(p, 16, GPFIFO_ENTRADAS);
        poner(p, 20, self.flags());
        poner(p, 28, ESPACIO);
        poner(p, 128, self.motor);
        // Instancia, USERD y RAMFC en VRAM, sin cache (1, como nouveau); el
        // bufer de metodos en la RAM del PC (0).
        memoria(p, 144, self.instancia, INSTANCIA_MEDIDA, VIDEO, 1);
        memoria(p, 168, self.userd, USERD_MEDIDA, VIDEO, 1);
        memoria(p, 192, self.instancia, RAMFC_MEDIDA, VIDEO, 1);
        memoria(p, 216, self.iova_metodos, METODOS, SISTEMA, 0);
        poner(p, 244, INTERNOS);
        MEDIDA
    }

    /// **La pregunta** (`GSP_RM_ALLOC`) en `hueco`.
    pub fn pedir(&self, hueco: &mut [u8], numero: u32) -> Option<usize> {
        let (cliente, padre, asa, clase, medida) = self.forma();
        crate::orden::componer(hueco, numero, crate::objeto::GSP_RM_ALLOC, crate::objeto::CABECERA_ALLOC + medida, |d| {
            poner(d, 0, cliente);
            poner(d, 4, padre);
            poner(d, 8, asa);
            poner(d, 12, clase);
            poner(d, 20, medida as u32);
            self.parametros(&mut d[crate::objeto::CABECERA_ALLOC..]);
        })
    }

    /// Las paginas del tramo que se ponen a cero antes de pedirlo (el USERD
    /// va en la pagina compartida: se pone a cero con el primer canal).
    pub const fn a_cero(&self) -> [u64; 2] {
        [self.instancia, self.gpfifo]
    }
}

/// Los canales que el contrato deja pedir.
pub const TODOS: [Canal; 2] = [COPIA, GR];

/// `(hClient, hParent, hObject, hClass, medida)`, como `objeto::Objeto::forma`.
pub const fn forma() -> (u32, u32, u32, u32, usize) {
    COPIA.forma()
}

/// **Los 368 B** del canal de copia.
pub fn parametros(p: &mut [u8]) -> usize {
    COPIA.parametros(p)
}

/// **La pregunta** del canal de copia (`GSP_RM_ALLOC`) en `hueco`.
pub fn pedir(hueco: &mut [u8], numero: u32) -> Option<usize> {
    COPIA.pedir(hueco, numero)
}


#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::objeto::{CABECERA_ALLOC, CLIENTE, GSP_RM_ALLOC};
    use crate::rpc::{Mensaje, Suma, CABECERA};

    fn u(d: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
    }
    fn u64_(d: &[u8], o: usize) -> u64 {
        u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
    }

    #[test]
    fn la_pregunta_suma_cero_y_mide_368() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 11).unwrap();
        assert_eq!(n, CABECERA + CABECERA_ALLOC + 368);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero), (GSP_RM_ALLOC, 11));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        let d = &h[CABECERA..];
        assert_eq!((u(d, 0), u(d, 4), u(d, 8), u(d, 12), u(d, 20)), (CLIENTE, DISPOSITIVO, 0xF1F0_0001, 0xC56F, 368));
    }

    #[test]
    fn cada_campo_en_su_sitio() {
        let mut p = [0xAAu8; MEDIDA];
        parametros(&mut p);
        assert_eq!((u(&p, 0), u(&p, 4)), (0, 0), "sin notificador de error");
        assert_eq!(u64_(&p, 8), 0x2_0000_2000, "el GPFIFO en la pagina 2 del tramo");
        assert_eq!(u(&p, 16), 512);
        assert_eq!(u(&p, 20), 0x0020_0100, "chid 1: indice 1, pagina 0, PAGE_FIXED");
        assert_eq!((u(&p, 24), u(&p, 28)), (0, ESPACIO));
        assert!(p[32..128].iter().all(|&b| b == 0), "hUserdMemory y userdOffset");
        assert_eq!((u(&p, 128), u(&p, 132), u(&p, 136), u(&p, 140)), (0x0B, 0, 0, 0), "COPY2");
        // instanceMem, userdMem, ramfcMem, mthdbufMem.
        assert_eq!((u64_(&p, 144), u64_(&p, 152), u(&p, 160), u(&p, 164)), (0x420_0000, 0x1000, 2, 1));
        assert_eq!((u64_(&p, 168), u64_(&p, 176), u(&p, 184), u(&p, 188)), (0x420_1200, 0x200, 2, 1));
        assert_eq!((u64_(&p, 192), u64_(&p, 200), u(&p, 208), u(&p, 212)), (0x420_0000, 0x200, 2, 1));
        assert_eq!((u64_(&p, 216), u64_(&p, 224), u(&p, 232), u(&p, 236)), (0x3A00_0000, 0x5000, 1, 0));
        assert_eq!((u(&p, 240), u(&p, 244)), (0, 0x14), "sin grupo propio; USER, notificadores NONE");
        assert!(p[248..368].iter().all(|&b| b == 0), "notificadores, CC y tpcConfigID");
    }

    #[test]
    fn el_canal_de_copia_no_cambio() {
        // Los 368 B del de copia: los mismos que VIO el metal (24-09 14:55).
        let (mut a, mut b) = ([0u8; MEDIDA], [0u8; MEDIDA]);
        parametros(&mut a);
        COPIA.parametros(&mut b);
        assert_eq!(a, b);
        assert_eq!(COPIA.flags(), FLAGS);
    }

    #[test]
    fn el_canal_de_gr0() {
        let mut p = [0xAAu8; MEDIDA];
        GR.parametros(&mut p);
        assert_eq!(u(&p, 128), 1, "GR0");
        assert_eq!(u(&p, 20), 0x0020_0200, "chid 2: indice 2, pagina 0, PAGE_FIXED");
        assert_eq!(u64_(&p, 8), 0x2_0000_6000, "el GPFIFO en la pagina 6");
        assert_eq!((u64_(&p, 144), u64_(&p, 168)), (0x420_5000, 0x420_1400));
        assert_eq!(u64_(&p, 216), 0x3A01_0000);
        assert_eq!(GR.forma().2, 0xF1F0_0002);
        // No pisa nada del de copia ni de la copia (paginas 0..4, 8, 12).
        let fin = TRAMO + crate::vram::TRAMO_PAGINAS as u64 * 0x1000;
        for d in [GR.instancia, GR.gpfifo] {
            assert!(d >= TRAMO + 5 * 0x1000 && d < fin && d != TRAMO + 8 * 0x1000 && d != TRAMO + 12 * 0x1000);
        }
        assert!(GR.userd != COPIA.userd && GR.iova_metodos >= COPIA.iova_metodos + METODOS);
    }

    #[test]
    fn todo_cabe_en_el_tramo_y_no_se_pisa() {
        let fin = TRAMO + crate::vram::TRAMO_PAGINAS as u64 * 0x1000;
        for (base, medida) in [(INSTANCIA, INSTANCIA_MEDIDA), (USERD, USERD_MEDIDA), (GPFIFO, GPFIFO_ENTRADAS as u64 * 8)] {
            assert!(base >= TRAMO && base + medida <= fin);
            assert_eq!(base % 0x200, 0);
        }
        assert!(INSTANCIA + INSTANCIA_MEDIDA <= USERD_PAGINA && USERD + USERD_MEDIDA <= GPFIFO);
        assert_eq!(GPFIFO_VA - TRAMO_VA, GPFIFO - TRAMO, "la VA y la VRAM del GPFIFO son la misma pagina");
        assert!(GPFIFO_ENTRADAS.is_power_of_two());
        assert_eq!(METODOS % 0x1000, 0);
        assert_eq!(crate::control::motor(MOTOR), (b"COPY" as &[u8], 2));
    }
}
