//! **LA ORDEN** -- como viaja el pase por la puerta, igual en los dos anillos.
//!
//! [carril]  VERDE     aritmetica de bits; la usan el kernel y Ring 3
//!
//! ```text
//!    arg1   bits 63..60   la suborden (ABRIR, CERRAR, ESTADO)
//!           bits 46..0    ABRIR: la VA del lienzo del que pide
//!
//!    ABRIR   Ok(la VA del buzon)            el lienzo, prestado para quedarse
//!    CERRAR  Ok(0)                          el lienzo devuelto; el buzon, lapida
//!    ESTADO  Ok(ver `estado`)               se puede pedir con o sin pase
//! ```

pub const ABRIR: u64 = 1;
pub const CERRAR: u64 = 2;
pub const ESTADO: u64 = 3;

/// **Donde queda el buzon en el proceso.** Una pagina, 1 MiB por encima del
/// buzon de la red (`bmo-puerta-red`, 7 paginas desde `0x2_0000_0000`), lejos
/// de lo prestado y de todo lo de debajo de 4 GiB.
pub const BUZON_VA: u64 = 0x0000_0002_0010_0000;

const VA: u64 = (1 << 47) - 1;

pub const fn abrir(lienzo: u64) -> u64 {
    ABRIR << 60 | (lienzo & VA)
}

pub const fn suborden(arg: u64) -> u64 {
    arg >> 60
}

pub const fn lienzo_de(arg: u64) -> u64 {
    arg & VA
}

/// `ESTADO`: `abierto << 63 | motivo del ultimo cierre << 8 | ultimo no`.
/// El motivo es un `radar::Motivo`; el no, un `pase::NoPase` (0: ninguno).
pub const fn estado(abierto: bool, motivo: u32, no: u32) -> u64 {
    (abierto as u64) << 63 | ((motivo & 0xFF) as u64) << 8 | (no & 0xFF) as u64
}

pub const fn abierto(e: u64) -> bool {
    e >> 63 != 0
}

pub const fn motivo(e: u64) -> u32 {
    (e >> 8) as u32 & 0xFF
}

pub const fn no(e: u64) -> u32 {
    e as u32 & 0xFF
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_orden_va_y_vuelve() {
        let a = abrir(0x7FFF_1234_5000);
        assert_eq!((suborden(a), lienzo_de(a)), (ABRIR, 0x7FFF_1234_5000));
        assert_eq!(lienzo_de(abrir(u64::MAX)), VA, "la suborden no se cuela en la VA");
    }

    #[test]
    fn el_estado_va_y_vuelve() {
        let e = estado(true, 9, 12);
        assert!(abierto(e));
        assert_eq!((motivo(e), no(e)), (9, 12));
        assert!(!abierto(estado(false, 0, 0)));
    }

    #[test]
    fn el_buzon_no_pisa_el_de_la_red() {
        let red_fin = 0x2_0000_0000u64 + 7 * 4096;
        assert!(BUZON_VA >= red_fin && BUZON_VA % 4096 == 0);
    }
}
