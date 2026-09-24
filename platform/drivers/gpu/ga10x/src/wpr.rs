//! **DONDE VA CADA COSA EN LA VRAM, Y LO QUE SE LE DICE AL BOOTER (L0c1)** --
//! el reparto de nova-core (`FbRanges`) y los 256 bytes de `GspFwWprMeta`.
//!
//! capa: puro -- cuentas sobre numeros que el kernel ya leyo; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un rango que se pisa con otro es el GSP-RM
//!           escribiendo encima de su propio codigo
//!
//! # El reparto, de arriba abajo (nova-core, `fb.rs`, Linux 7.3)
//!
//! ```text
//!    fin de la VRAM
//!    vga        la VBIOS la usa; lo que dijo 0x625F04        (ya en L0a)
//!    frts       1 MiB bajo la vga, alineado a 128 KiB        (ya en L0b: es
//!                                                              donde FWSEC puso
//!                                                              la WPR2)
//!    boot       el bootloader RISC-V, alineado a 4 KiB
//!    elf        el GSP-RM (.fwimage), alineado a 64 KiB
//!    heap       el heap del GSP, alineado a 1 MiB
//!    wpr2       desde la WPR meta hasta el fin de frts: lo que el booter
//!               PROTEGE
//!    no_wpr     1 MiB justo debajo, sin proteger
//! ```
//!
//! El heap es la cuenta de LIBOS3 (GA102 en adelante), con las constantes de
//! `r570_144/bindings.rs`: el carveout del OS, el RM al arrancar, UN cliente y
//! 96 KiB por GiB de VRAM, entre 88 y 280 MiB.
//!
//! # [!] Las constantes son de la version 570.144
//!
//! `GspFwWprMeta` y el heap cambian con la version del GSP-RM, y por eso el
//! build baja la 570.144 y no otra: es la que nova-core arranca.

use crate::booter::Riscv;
use crate::vbios::Frts;

const K4: u64 = 4 * 1024;
const K64: u64 = 64 * 1024;
const K128: u64 = 128 * 1024;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

/// `GSP_FW_HEAP_PARAM_OS_SIZE_LIBOS3_BAREMETAL`.
pub const HEAP_OS: u64 = 23_068_672;
/// `GSP_FW_HEAP_PARAM_BASE_RM_SIZE_TU10X` (Turing, Ampere y Ada).
pub const HEAP_RM: u64 = 8_388_608;
/// `GSP_FW_HEAP_PARAM_CLIENT_ALLOC_SIZE`: UN cliente.
pub const HEAP_CLIENTE: u64 = 100_663_296;
/// `GSP_FW_HEAP_PARAM_SIZE_PER_GB_FB`.
pub const HEAP_POR_GIB: u64 = 98_304;
/// `GSP_FW_HEAP_SIZE_OVERRIDE_LIBOS3_BAREMETAL_MIN_MB` / `_MAX_MB`.
pub const HEAP_MIN: u64 = 88 * MIB;
pub const HEAP_MAX: u64 = 280 * MIB;
/// `non_wpr_heap_size_tu102`.
pub const NO_WPR: u64 = MIB;

/// `GSP_FW_WPR_META_MAGIC` y `_REVISION`.
pub const MAGIA_WPR: u64 = 0xDC3A_AE21_371A_60B3;
pub const REVISION_WPR: u64 = 1;
/// Lo que mide `GspFwWprMeta` (r570.144): 256 bytes, contados campo a campo.
pub const WPR_META: usize = 256;

const fn abajo(v: u64, a: u64) -> u64 {
    v & !(a - 1)
}
const fn arriba(v: u64, a: u64) -> u64 {
    (v + a - 1) & !(a - 1)
}

/// **Lo que el GSP necesita de heap** con `vram` bytes de VRAM.
pub const fn heap(vram: u64) -> u64 {
    let gib = (vram + GIB - 1) / GIB;
    let t = HEAP_OS + HEAP_RM + arriba(HEAP_CLIENTE, MIB) + arriba(HEAP_POR_GIB * gib, MIB);
    // `clamp(min, max - 1)`, como nova-core.
    if t < HEAP_MIN {
        HEAP_MIN
    } else if t > HEAP_MAX - 1 {
        HEAP_MAX - 1
    } else {
        t
    }
}

/// Un rango de la VRAM.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rango {
    pub desde: u64,
    pub hasta: u64,
}

impl Rango {
    pub const fn bytes(&self) -> u64 {
        self.hasta - self.desde
    }
}

/// **El reparto entero** (`FbRanges`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mapa {
    pub vram: u64,
    pub vga: Rango,
    pub frts: Rango,
    pub boot: Rango,
    pub elf: Rango,
    pub heap: Rango,
    pub wpr2: Rango,
    pub no_wpr: Rango,
}

/// **Repartir**, con lo que ya dijo L0a (`frts`) y lo que miden el bootloader
/// y el `.fwimage`. `None` si no cabe: una VRAM que no llega no se reparte.
pub const fn mapa(f: &Frts, bootloader: u64, imagen: u64) -> Option<Mapa> {
    let Some(b) = f.desde.checked_sub(bootloader) else { return None };
    let boot = Rango { desde: abajo(b, K4), hasta: abajo(b, K4) + bootloader };
    let Some(e) = boot.desde.checked_sub(imagen) else { return None };
    let elf = Rango { desde: abajo(e, K64), hasta: abajo(e, K64) + imagen };
    let Some(h) = elf.desde.checked_sub(heap(f.vram)) else { return None };
    let heap = Rango { desde: abajo(h, MIB), hasta: abajo(elf.desde, MIB) };
    let Some(w) = heap.desde.checked_sub(WPR_META as u64) else { return None };
    let wpr2 = Rango { desde: abajo(w, MIB), hasta: f.hasta };
    let Some(n) = wpr2.desde.checked_sub(NO_WPR) else { return None };
    Some(Mapa {
        vram: f.vram,
        vga: Rango { desde: f.vga, hasta: f.vram },
        frts: Rango { desde: f.desde, hasta: f.hasta },
        boot,
        elf,
        heap,
        wpr2,
        no_wpr: Rango { desde: n, hasta: wpr2.desde },
    })
}

// -- Lo que se presta: la tabla radix3 -------------------------------------------

/// **Las paginas de la tabla radix3** (`GspFirmware`): el `.fwimage` a paginas
/// de 4 KiB, una entrada de 8 bytes por pagina en el nivel 2, una por pagina
/// del nivel 2 en el nivel 1, y el nivel 0 de una pagina.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Radix3 {
    pub imagen: u64,
    pub nivel2: u64,
    pub nivel1: u64,
}

impl Radix3 {
    pub const fn de(imagen_bytes: u64) -> Self {
        let imagen = (imagen_bytes + K4 - 1) / K4;
        let nivel2 = (imagen * 8 + K4 - 1) / K4;
        let nivel1 = (nivel2 * 8 + K4 - 1) / K4;
        Radix3 { imagen, nivel2, nivel1 }
    }

    /// Todas, con el nivel 0.
    pub const fn paginas(&self) -> u64 {
        self.imagen + self.nivel2 + self.nivel1 + 1
    }
}

// -- GspFwWprMeta ----------------------------------------------------------------

/// Donde vera el GSP lo que vive en la RAM del PC (IOVAs, por la IOMMU).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Prestado {
    pub radix3: u64,
    pub bootloader: u64,
    pub firma: u64,
}

/// **Los 256 bytes que el booter lee por el buzon del SEC2**
/// (`GspFwWprMeta::from_ranges`). `imagen` y `firma` en bytes.
pub fn wpr_meta(m: &Mapa, bl: &Riscv, imagen: u64, firma: u64, p: &Prestado) -> [u8; WPR_META] {
    let mut b = [0u8; WPR_META];
    let mut pon = |o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
    pon(0, MAGIA_WPR);
    pon(8, REVISION_WPR);
    pon(16, p.radix3);
    pon(24, imagen);
    pon(32, p.bootloader);
    pon(40, bl.bin.bytes as u64);
    pon(48, bl.codigo as u64);
    pon(56, bl.datos as u64);
    pon(64, bl.manifiesto as u64);
    pon(72, p.firma);
    pon(80, firma);
    pon(88, m.no_wpr.desde); // gspFwRsvdStart
    pon(96, m.no_wpr.desde);
    pon(104, m.no_wpr.bytes());
    pon(112, m.wpr2.desde);
    pon(120, m.heap.desde);
    pon(128, m.heap.bytes());
    pon(136, m.elf.desde);
    pon(144, m.boot.desde);
    pon(152, m.frts.desde);
    pon(160, m.frts.bytes());
    pon(168, abajo(m.vga.desde, K128)); // gspFwWprEnd
    pon(176, m.vram);
    pon(184, m.vga.desde);
    pon(192, m.vga.bytes());
    // 200 bootCount, 208..240 la union del RPC, 240 particiones VF, 241
    // flags, 244 lo reservado de la PMU (0 en Ampere) y 248 verified: a 0.
    b
}

// ===================================================================
//  PRUEBAS -- la 3060 del Ryzen, con los numeros de L0a y L0b
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::booter::{Bin, Riscv};
    use crate::vbios::frts;

    /// Lo que dijo el metal (24-09 04:58): 12288 MiB, FRTS en 0x2FFE00000.
    fn la_3060() -> Frts {
        let f = frts(12288, 0, false);
        assert_eq!((f.desde, f.hasta), (0x2_FFE0_0000, 0x2_FFF0_0000));
        f
    }

    #[test]
    fn el_heap_de_12_gib_son_128_mib() {
        // 22 + 8 + 96 + 2 (12 x 96 KiB = 1,1 MiB, a MiB) = 128 MiB.
        assert_eq!(heap(12 * GIB), 128 * MIB);
        assert_eq!(heap(0), 126 * MIB);
        assert_eq!(heap(4096 * GIB), HEAP_MAX - 1, "se recorta por arriba");
    }

    #[test]
    fn el_reparto_de_la_3060() {
        let m = mapa(&la_3060(), 0x6000, 0x3C9_9000).unwrap();
        assert_eq!(m.boot, Rango { desde: 0x2_FFDF_A000, hasta: 0x2_FFE0_0000 });
        assert_eq!(m.elf, Rango { desde: 0x2_FC16_0000, hasta: 0x2_FFDF_9000 });
        assert_eq!(m.heap, Rango { desde: 0x2_F410_0000, hasta: 0x2_FC10_0000 });
        assert_eq!(m.wpr2, Rango { desde: 0x2_F400_0000, hasta: 0x2_FFF0_0000 });
        assert_eq!(m.no_wpr, Rango { desde: 0x2_F3F0_0000, hasta: 0x2_F400_0000 });
        // Ninguno pisa al de arriba.
        for (abajo, arriba) in [(m.no_wpr, m.heap), (m.heap, m.elf), (m.elf, m.boot), (m.boot, m.frts), (m.frts, m.vga)] {
            assert!(abajo.hasta <= arriba.desde, "{:?} pisa {:?}", abajo, arriba);
        }
        assert!(m.wpr2.desde + WPR_META as u64 <= m.heap.desde, "la WPR meta cabe delante del heap");
    }

    #[test]
    fn una_vram_que_no_llega_no_se_reparte() {
        let f = frts(64, 0, false);
        assert_eq!(mapa(&f, 0x6000, 0x3C9_9000), None);
    }

    #[test]
    fn la_radix3_del_gsp_570() {
        let r = Radix3::de(0x3C9_9000);
        assert_eq!((r.imagen, r.nivel2, r.nivel1), (15513, 31, 1));
        assert_eq!(r.paginas(), 15546);
        assert_eq!(Radix3::de(1).paginas(), 4);
    }

    #[test]
    fn la_wpr_meta_pone_cada_campo_en_su_sitio() {
        let m = mapa(&la_3060(), 0x6000, 0x3C9_9000).unwrap();
        let bl = Riscv { bin: Bin { bytes: 0x6000, ..Bin::default() }, codigo: 0x1800, datos: 0x800, ..Riscv::default() };
        let p = Prestado { radix3: 0x1200_0000, bootloader: 0x1300_0000, firma: 0x1301_0000 };
        let b = wpr_meta(&m, &bl, 0x3C9_9000, 0x1000, &p);
        let q = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        assert_eq!((q(0), q(8)), (MAGIA_WPR, 1));
        assert_eq!((q(16), q(24), q(32), q(40)), (0x1200_0000, 0x3C9_9000, 0x1300_0000, 0x6000));
        assert_eq!((q(48), q(56), q(64), q(72), q(80)), (0x1800, 0x800, 0, 0x1301_0000, 0x1000));
        assert_eq!((q(112), q(136), q(144), q(152), q(160)), (m.wpr2.desde, m.elf.desde, m.boot.desde, 0x2_FFE0_0000, MIB));
        assert_eq!((q(168), q(176)), (0x2_FFF0_0000, 12 * GIB));
        assert!(b[200..].iter().all(|&x| x == 0));
    }
}
