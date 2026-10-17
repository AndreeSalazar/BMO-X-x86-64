//! **PTIMER** -- el reloj de la 3060 puesto en hora antes del arranque del GSP.
//!
//! capa: puro -- decide que se escribe y en que orden; quien escribe es el kernel, por [`Registros`]
//!
//! [estado]  VOLATIL  el contador de la tarjeta; se pierde al cortar la corriente
//! [consumo] NADA -- una lectura y dos escrituras, una vez por arranque y solo si `gpu g0 ptimer`
//!
//! OpenRM 570.144 lo hace en `kgspInitRm_IMPL`, antes de FWSEC y del booter
//! (`tmrSetCurrentTime_GV100`): solo si el PLM deja escribir al nivel 0, los
//! nanosegundos desde 1970 en `TIME_1` y luego `TIME_0`, que es la escritura
//! que los carga. BMO-X no lo hacia: es la prueba E6 contra el 0x15.

use crate::Registros;

pub const TIME_0: u32 = 0x0000_9400;
pub const TIME_1: u32 = 0x0000_9410;
pub const TIME_PLM: u32 = 0x0000_9430;
/// `WRITE_PROTECTION_LEVEL0`, `_ENABLE`: el nivel 0 puede escribir la hora.
pub const PLM_ESCRIBE_NIVEL0: u32 = 1 << 4;

/// Pone la hora `ns` (desde 1970). `false` si el PLM no deja: entonces no se
/// escribe nada, como OpenRM.
pub fn poner_hora(r: &mut impl Registros, ns: u64) -> bool {
    if r.leer(TIME_PLM) & PLM_ESCRIBE_NIVEL0 == 0 {
        return false;
    }
    r.escribir(TIME_1, (ns >> 32) as u32);
    r.escribir(TIME_0, ns as u32);
    true
}

/// Los nanosegundos desde el 1-1-1970 de una fecha del calendario (UTC o la
/// hora local del CMOS: a la tarjeta le basta un contador que avance).
pub fn ns_desde_1970(anio: u16, mes: u8, dia: u8, hora: u8, minuto: u8, segundo: u8) -> u64 {
    // Dias desde la epoca por el algoritmo civil de Howard Hinnant.
    let (y, m) = if mes <= 2 { (anio as i64 - 1, mes as i64 + 9) } else { (anio as i64, mes as i64 - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    let s = dias * 86_400 + hora as i64 * 3600 + minuto as i64 * 60 + segundo as i64;
    s.max(0) as u64 * 1_000_000_000
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    struct Timer {
        plm: u32,
        escritos: Vec<(u32, u32)>,
    }

    impl Registros for Timer {
        fn leer(&mut self, reg: u32) -> u32 {
            if reg == TIME_PLM { self.plm } else { 0 }
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            self.escritos.push((reg, v));
        }
    }

    #[test]
    fn la_hora_va_alta_primero_y_baja_despues() {
        let mut t = Timer { plm: PLM_ESCRIBE_NIVEL0, escritos: Vec::new() };
        assert!(poner_hora(&mut t, 0x1234_5678_9ABC_DEF0));
        assert_eq!(t.escritos, [(TIME_1, 0x1234_5678), (TIME_0, 0x9ABC_DEF0)], "TIME_0 la ultima: es la que carga");
    }

    #[test]
    fn sin_permiso_no_se_toca() {
        let mut t = Timer { plm: 0, escritos: Vec::new() };
        assert!(!poner_hora(&mut t, 1));
        assert!(t.escritos.is_empty());
    }

    #[test]
    fn la_epoca_y_una_fecha_conocida() {
        assert_eq!(ns_desde_1970(1970, 1, 1, 0, 0, 0), 0);
        // 2026-10-17 09:21:00 = 1792228860 s
        assert_eq!(ns_desde_1970(2026, 10, 17, 9, 21, 0), 1_792_228_860 * 1_000_000_000);
        assert_eq!(ns_desde_1970(2000, 3, 1, 0, 0, 0), 951_868_800 * 1_000_000_000, "tras un 29 de febrero");
    }
}
