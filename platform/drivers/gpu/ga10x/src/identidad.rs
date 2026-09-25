//! **LA 3060 12G, Y SOLO ELLA** (2026-09-25).
//!
//! El propietario: *"AISLAR por completo ... en la version que VA A SER, `GPU
//! RTX 3060 12G`, eso mismo que mi BMO-X en x86-64: SOLO agarre uno, por
//! motivos, eso es optimizacion tambien"*.
//!
//! Por que es aislar: todo este crate se escribio y se probo contra UNA
//! tarjeta (la del propietario: `10DE:2504`, subsistema `1462:397D`, GA106 rev
//! A1, 12288 MiB). Sus direcciones de VRAM (el tramo, las tablas, FRTS), sus
//! cuentas de GPC y sus firmas son las de esa. En otra -- una 3060 de 8 GiB, un
//! GA104 -- las mismas cuentas caerian en otro sitio, y una escritura en la
//! VRAM de mas es una tarjeta colgada o algo peor. Mejor un NO claro que un
//! "casi".
//!
//! Por que es optimizacion: con una sola tarjeta, lo que en un driver general
//! se pregunta en cada arranque (cuanta VRAM, que chip, que tabla) aqui es una
//! CONSTANTE, y el compilador la pliega.
//!
//! ```text
//!    fabricante   0x10DE (NVIDIA)
//!    dispositivo  0x2503 (RTX 3060) o 0x2504 (RTX 3060, la LHR: la tuya)
//!    chip         GA106 (BOOT_0: chipset 0x176)
//!    VRAM         12288 MiB exactos (0x1183A4, cuando el GFW acabo)
//! ```
//!
//! La misma lista va en el cargador (`s1_cpu/src/gpu_reinicio.rs`, que no
//! enlaza con este crate), y el guardian `la-3060` exige que coincidan.

use crate::Chip;

pub const NVIDIA: u16 = 0x10DE;
/// Las dos RTX 3060 de 12 GiB con GA106. La 0x2504 es la del propietario.
pub const DISPOSITIVOS: [u16; 2] = [0x2503, 0x2504];
/// GA106.
pub const CHIPSET: u16 = 0x176;
/// La VRAM que dice `0x1183A4` (MiB) cuando el firmware de arranque acabo.
pub const VRAM_MIB: u32 = 12 * 1024;

/// **Es la 3060 12G?** Por lo que se sabe al sondear: el dispositivo PCI y
/// BOOT_0. La VRAM se mira despues ([`vram_es_la_suya`]): al sondear el
/// firmware de la tarjeta puede no haber acabado.
pub const fn es_la_3060_12g(dispositivo: u16, chip: Chip) -> bool {
    let mut i = 0;
    let mut suyo = false;
    while i < DISPOSITIVOS.len() {
        suyo |= DISPOSITIVOS[i] == dispositivo;
        i += 1;
    }
    suyo && chip.es_ampere() && chip.chipset() == CHIPSET
}

/// **La VRAM es la suya?** 12288 MiB exactos.
pub const fn vram_es_la_suya(mib: u32) -> bool {
    mib == VRAM_MIB
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El BOOT_0 del metal del 23-09: GA106 rev A1.
    const LA_SUYA: Chip = Chip(0xB760_00A1);

    #[test]
    fn la_suya_si() {
        assert_eq!(LA_SUYA.chipset(), CHIPSET);
        assert!(es_la_3060_12g(0x2504, LA_SUYA));
        assert!(es_la_3060_12g(0x2503, LA_SUYA));
        assert!(vram_es_la_suya(12288));
    }

    #[test]
    fn otras_no() {
        // Una 3060 de 8 GiB (GA106 con otro dispositivo) o una 3060 Ti (GA104).
        assert!(!es_la_3060_12g(0x2487, LA_SUYA));
        assert!(!es_la_3060_12g(0x2504, Chip(0xB740_00A1)));
        // Un error del anillo no es un chip.
        assert!(!es_la_3060_12g(0x2504, Chip(0xBADF_5040)));
        assert!(!vram_es_la_suya(8192));
        assert!(!vram_es_la_suya(12287));
    }
}
