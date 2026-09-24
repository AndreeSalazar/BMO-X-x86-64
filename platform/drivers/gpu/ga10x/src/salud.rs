//! **LA SALUD DE LA 3060, EN SOLO LECTURA** -- lo que se puede saber sin
//! pedirle nada al GSP-RM: la temperatura del chip por su sensor, y el enlace
//! PCIe por el espacio de configuracion.
//!
//! capa: puro -- el kernel lee los dos registros; esto los entiende (L8)
//!
//! [eje]     CORRECCION -- una temperatura con el bit de validez caido no es
//!           0 grados: es "sin dato"
//!
//! # La temperatura (`0x020460`)
//!
//! La lee nouveau desde Pascal (`therm/gp100.c`, `gp100_temp_get`) y la usa
//! en Turing: bits 3..16 son la temperatura en 1/256 grados (se toman los
//! enteros, `>> 8`), el bit 29 dice que es valida y el 30 que viene de la
//! sombra. Para Ampere nouveau ya no la declara (todo va por el GSP); el
//! registro sigue ahi, y por eso sale CRUDO al lado: si un dia no cuadra, se ve.
//!
//! # Los vatios, dichos con su nombre
//!
//! La potencia de la placa la miden sensores que lee la PMU por I2C, y NVIDIA
//! no publica los controles del RM que la dan: `ctrl2080pmgr.h`, `thermal`,
//! `clk` y `fan` de OpenRM 570.144 salen sin ordenes. Aqui no se inventa un
//! numero: los vatios de la CPU ya estan en el panel, los de la 3060 no.

/// `NV_THERM_I2CS_SENSOR_00` segun nouveau: el sensor interno.
pub const TERMICO: u32 = 0x0002_0460;
const VALIDA: u32 = 1 << 29;

/// **La temperatura en grados**, si el sensor dice que es valida.
pub fn grados(crudo: u32) -> Option<u32> {
    (crudo & VALIDA != 0).then_some((crudo & 0x0001_FFF8) >> 8)
}

/// El enlace PCIe: lo que va AHORA y lo mas que puede.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enlace {
    /// 1 = 2.5 GT/s (Gen1) ... 4 = 16 GT/s (Gen4).
    pub gen: u8,
    pub ancho: u8,
    pub gen_max: u8,
    pub ancho_max: u8,
}

/// **El enlace**, de Link Status (16 bits) y Link Capabilities (32 bits) de la
/// capacidad PCI Express. `None` si no hay capacidad (ambos a 0).
pub fn enlace(estado: u16, capacidad: u32) -> Option<Enlace> {
    if estado == 0 && capacidad == 0 {
        return None;
    }
    Some(Enlace {
        gen: (estado & 0xF) as u8,
        ancho: ((estado >> 4) & 0x3F) as u8,
        gen_max: (capacidad & 0xF) as u8,
        ancho_max: ((capacidad >> 4) & 0x3F) as u8,
    })
}

/// Los GT/s de una generacion, en decimas (`25` = 2.5).
pub fn gts(gen: u8) -> u32 {
    match gen {
        1 => 25,
        2 => 50,
        3 => 80,
        4 => 160,
        5 => 320,
        _ => 0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_temperatura_con_y_sin_validez() {
        // 45 grados y medio: 45.5 * 256 = 11648 = 0x2D80, en los bits 3..16.
        assert_eq!(grados(VALIDA | 0x2D80), Some(45));
        assert_eq!(grados(0x2D80), None, "sin el bit 29 no hay dato");
        assert_eq!(grados(VALIDA | 0x4000_0000 | 0x5000), Some(0x50));
    }

    #[test]
    fn el_enlace_de_una_3060_en_un_zen3() {
        // Link Status: Gen4 x16 = 0x0104; Link Capabilities: Gen4 x16 = 0x104.
        let e = enlace(0x0104, 0x0000_0104).unwrap();
        assert_eq!((e.gen, e.ancho, e.gen_max, e.ancho_max), (4, 16, 4, 16));
        // En reposo baja a Gen1: eso es ahorro, no una averia.
        assert_eq!(enlace(0x0101, 0x104).unwrap().gen, 1);
        assert_eq!(enlace(0, 0), None);
        assert_eq!((gts(1), gts(4)), (25, 160));
    }
}
