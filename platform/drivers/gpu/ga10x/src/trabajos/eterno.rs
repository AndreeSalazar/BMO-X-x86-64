//! **E7: EL TRABAJO ETERNO** -- un computo hecho A PROPOSITO para no acabar
//! nunca: lo que el vigilante (`vigilante.rs`) tiene que cortar. Es la prueba
//! en el metal de E7 (`gpu eterno`): la cabina dice el corte, el canal de GR
//! queda fuera, y el resto del sistema sigue.
//!
//! capa: puro -- el programa, el QMD y las ordenes; los registros los toca el
//! kernel (L8)
//!
//! [eje]     CORRECCION -- el programa son palabras que ya corrieron en el
//!           metal: el `BRA` a si mismo que `ptxas` pone detras de cada EXIT
//!           (el ultimo del fractal), puesto DELANTE
//!
//! # El programa
//!
//! ```text
//!    0x00  BRA 0x00     la primera instruccion salta a si misma: el hilo
//!                       no pasa nunca de ahi, y el QMD nunca paga
//!    0x10  EXIT         (no se llega)
//!    0x20  BRA 0x20     el de siempre, detras del EXIT
//! ```
//!
//! Un bloque de 32 hilos (un warp): lo justo para ocupar el motor grafico.
//!
//! # Donde vive
//!
//! ```text
//!    tramo   11 el programa, 13 el QMD, 15 las ordenes (las del fractal: un
//!            trabajo del GR a la vez, con su cerrojo); sus semaforos en
//!            14 +0x220 y +0x230, libres entre la imagen y la escalera
//! ```
//!
//! Sin memoria de salida: no escribe nada, solo gira.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_con, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

/// Los hilos del bloque (un warp) y los bloques (uno).
pub const HILOS: u32 = 32;
pub const BLOQUES: u32 = 1;
/// Los registros del QMD: los del fractal (sobran: no usa ninguno).
pub const REGISTROS: u32 = 32;
/// Lo que pagaria si acabara. No acaba: si se ve, algo va mal.
pub const PAGA_QMD: u32 = 0x3060_E7C0;
pub const PAGA_FIN: u32 = 0x3060_E7F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x220;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x230;

/// **El programa**: el `BRA` a si mismo delante (el ultimo del fractal,
/// `fractal::CODIGO[43]`), el EXIT y el `BRA` de siempre.
pub const CODIGO: [(u64, u64); 3] = [
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x0 (a si misma)
    (0x0000_0000_0000_794D, 0x000F_EA00_0380_0000), // EXIT
    (0xFFFF_FFF0_0000_7947, 0x000F_C000_0383_FFFF), // BRA 0x20
];
pub const PALABRAS_CODIGO: usize = CODIGO.len() * 4;

pub fn codigo() -> [u32; PALABRAS_CODIGO] {
    let mut w = [0u32; PALABRAS_CODIGO];
    for (k, &(lo, hi)) in CODIGO.iter().enumerate() {
        w[4 * k] = lo as u32;
        w[4 * k + 1] = (lo >> 32) as u32;
        w[4 * k + 2] = hi as u32;
        w[4 * k + 3] = (hi >> 32) as u32;
    }
    w
}

pub fn qmd() -> [u32; QMD_PALABRAS] {
    qmd_con(sombreador_va(PROGRAMA), HILOS, BLOQUES, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar** con la entrada `e` del GPFIFO de GR (la cuenta del blur).
pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    if !crate::blur::entrada_valida(e) {
        return false;
    }
    let c = codigo();
    let q = qmd();
    let o = ordenes();
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let pag = crate::vram::PALABRAS;
    escribir(r, SEMAFORO_QMD, &[0; 8]) == 8
        && a_cero(r, PROGRAMA) as usize == pag
        && a_cero(r, QMD) as usize == pag
        && escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO
        && escribir(r, QMD, &q) == QMD_PALABRAS
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, semaforo del QMD, semaforo del final)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

/// Pago (no deberia).
pub const fn pagado(qmd: u32, fin: u32) -> bool {
    qmd == PAGA_QMD && fin == PAGA_FIN
}

/// Lo que devuelve `gpu eterno`: `lanzado | pagado << 1 | cortado << 2 |
/// us esperados (21) << 8 | el final del corte (`vigilante::empaquetar`,
/// 32) << 32`.
pub const fn empaquetar(lanzado: bool, pagado: bool, cortado: bool, us: u64, final_: u64) -> u64 {
    let us = if us > 0x1F_FFFF { 0x1F_FFFF } else { us };
    lanzado as u64 | (pagado as u64) << 1 | (cortado as u64) << 2 | us << 8 | (final_ & 0xFFFF_FFFF) << 32
}

/// `(lanzado, pagado, cortado, us, final)`.
pub const fn desempaquetar(v: u64) -> (bool, bool, bool, u64, u64) {
    (v & 1 != 0, v >> 1 & 1 != 0, v >> 2 & 1 != 0, v >> 8 & 0x1F_FFFF, v >> 32)
}

/// **Salio bien la PRUEBA**: se lanzo, NO pago (es eterno), y el vigilante
/// lo corto con un final que no es "sigue girando".
pub fn sano(v: u64) -> bool {
    let (lanzado, pagado, cortado, _, f) = desempaquetar(v);
    lanzado && !pagado && cortado && matches!(crate::vigilante::desempaquetar(f), Some(crate::vigilante::Final::Rc(_) | crate::vigilante::Final::Quieto(_)))
}

const _: () = assert!(SEMAFORO_QMD >= crate::imagen::SEMAFORO_FIN + 16 && SEMAFORO_FIN + 16 <= crate::raster::ESCALONES);
const _: () = assert!(SEMAFORO_FIN == SEMAFORO_QMD + 16);
const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El desplazamiento de un BRA (en bytes, desde la siguiente), como lo
    /// leen el simulador y el juez.
    fn salto(lo: u64, hi: u64) -> i64 {
        (((hi & 0x3_FFFF) << 32 | lo >> 32) << 14) as i64 >> 14
    }

    #[test]
    fn la_primera_salta_a_si_misma() {
        let (lo, hi) = CODIGO[0];
        assert_eq!(lo & 0xFFF, 0x947);
        // Desde la siguiente (+16), -16: ella misma.
        assert_eq!(16 + salto(lo, hi), 0);
        // Es la palabra del fractal (la de detras de su EXIT: metal).
        assert_eq!(CODIGO[0], crate::fractal::CODIGO[43]);
        assert_eq!(CODIGO[1], crate::fractal::CODIGO[42]);
    }

    #[test]
    fn sus_semaforos_no_pisan_a_nadie() {
        let otros = [
            crate::fractal::SEMAFORO_QMD,
            crate::fractal::SEMAFORO_FIN,
            crate::blur::SEMAFORO_QMD,
            crate::blur::SEMAFORO_FIN,
            crate::imagen::SEMAFORO_QMD,
            crate::imagen::SEMAFORO_FIN,
            crate::cubo::SEMAFORO_FIN,
            crate::anillo::SEMAFORO,
        ];
        for s in [SEMAFORO_QMD, SEMAFORO_FIN] {
            assert!(otros.iter().all(|&o| o.abs_diff(s) >= 16), "{s:#x}");
        }
    }

    #[test]
    fn el_paquete_y_la_prueba() {
        use crate::vigilante::{empaquetar as fin, Final, Paso};
        let v = empaquetar(true, false, true, 1_000_003, fin(Final::Rc(8)));
        assert_eq!(desempaquetar(v), (true, false, true, 1_000_003, 1 | 8 << 8));
        assert!(sano(v));
        assert!(sano(empaquetar(true, false, true, 5, fin(Final::Quieto(Paso::Parar)))));
        // Sigue girando tras los dos pasos, sin cortar, o pago: NO.
        assert!(!sano(empaquetar(true, false, true, 5, fin(Final::Sigue))));
        assert!(!sano(empaquetar(true, false, false, 5, 0)));
        assert!(!sano(empaquetar(true, true, true, 5, fin(Final::Rc(0)))));
        assert_eq!(desempaquetar(empaquetar(true, false, false, u64::MAX, 0)).3, 0x1F_FFFF);
    }
}
