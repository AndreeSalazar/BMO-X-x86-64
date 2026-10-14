//! **LAS INSTRUCCIONES QUE USA UN PROGRAMA** (A6 de `docs/plan/EL_FOCO.md`,
//! LB6 de `docs/plan/PLAN_LA_LENTE_BINARIA.md`, 14-10).
//!
//! [carril]  VERDE     una tabla constante y dos mascaras
//! [consumo] NADA      al admitir un `.bex`, una vez
//!
//! Un `.bex` declara en su anexo de REQUISITOS ([`CLASE_INSTRUCCIONES`]) las
//! extensiones de x86-64 que su codigo usa: una por bit. La escribe QUIEN
//! EMITE el codigo -- cada fila de `intrinsics.toml` dice la suya, y el emisor
//! la apunta al escribirla --, el enlazador junta las de cada objeto, y el
//! kernel la compara al admitir con lo que dice el CPUID de la maquina: lo
//! que no tenga, NO, con su nombre. Antes, un `popcnt` en una CPU sin el era
//! un `#UD` a mitad de camino.
//!
//! ** Lo que NO va aqui: x86-64 base (x87, SSE, SSE2, CMOV, CMPXCHG8B...),
//! que toda CPU de 64 bits tiene; y el ESTADO que el kernel guarda en un
//! cambio de contexto, que es otra pregunta y tiene su clase (`CLASE_CPU`,
//! la mascara `xcr0`).
//!
//! ** Las tablas salen del SDM de Intel (vol. 2, CPUID) y del APM de AMD (vol.
//! 3, apendice E): la hoja, el registro y el bit de cada una. La lente es de
//! AMD (Zen 3) y estas son de las dos: un bit de CPUID es el mismo en las dos.

use crate::CLASE_MONTON;

/// **Las instrucciones que usa el codigo**: `cantidad` es una mascara de los
/// bits de [`EXTENSIONES`]. Obligatoria: sin la extension, no arranca.
pub const CLASE_INSTRUCCIONES: u16 = 0x0009;
const _: () = assert!(CLASE_INSTRUCCIONES == CLASE_MONTON + 1, "las clases van seguidas y no se reusan");

/// En que registro de CPUID esta el bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    Ebx,
    Ecx,
    Edx,
}

/// **Una extension**: su bit en la mascara, su nombre, y donde la dice el CPUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extension {
    pub bit: u64,
    pub nombre: &'static str,
    pub hoja: u32,
    pub subhoja: u32,
    pub reg: Reg,
    pub bit_cpuid: u8,
}

const fn e(k: u8, nombre: &'static str, hoja: u32, subhoja: u32, reg: Reg, bit_cpuid: u8) -> Extension {
    Extension { bit: 1 << k, nombre, hoja, subhoja, reg, bit_cpuid }
}

/// **Todas**, en el orden de su bit. Un bit que se retira no se reusa.
pub const EXTENSIONES: [Extension; 16] = [
    e(0, "SSE3", 1, 0, Reg::Ecx, 0),
    e(1, "SSSE3", 1, 0, Reg::Ecx, 9),
    e(2, "SSE4.1", 1, 0, Reg::Ecx, 19),
    e(3, "SSE4.2", 1, 0, Reg::Ecx, 20),
    e(4, "POPCNT", 1, 0, Reg::Ecx, 23),
    e(5, "LZCNT", 0x8000_0001, 0, Reg::Ecx, 5),
    e(6, "BMI1", 7, 0, Reg::Ebx, 3),
    e(7, "BMI2", 7, 0, Reg::Ebx, 8),
    e(8, "AVX", 1, 0, Reg::Ecx, 28),
    e(9, "AVX2", 7, 0, Reg::Ebx, 5),
    e(10, "FMA", 1, 0, Reg::Ecx, 12),
    e(11, "RDRAND", 1, 0, Reg::Ecx, 30),
    e(12, "RDSEED", 7, 0, Reg::Ebx, 18),
    e(13, "RDTSCP", 0x8000_0001, 0, Reg::Edx, 27),
    // `xgetbv`/`xsetbv` piden que el sistema haya encendido XSAVE: OSXSAVE.
    e(14, "OSXSAVE", 1, 0, Reg::Ecx, 27),
    e(15, "MONITOR", 1, 0, Reg::Ecx, 3),
];

/// La extension de este nombre (el que lleva `intrinsics.toml`).
pub fn por_nombre(nombre: &str) -> Option<&'static Extension> {
    EXTENSIONES.iter().find(|x| x.nombre == nombre)
}

/// Las que conoce este sistema: un bit fuera de aqui es desconocido.
pub const CONOCIDAS: u64 = (1 << EXTENSIONES.len()) - 1;

/// **Lo que tiene ESTA CPU**, preguntandoselo: `cpuid(hoja, subhoja)` da
/// `[eax, ebx, ecx, edx]`. Una hoja por encima de la maxima no se pregunta
/// (contestaria la de mas arriba, no la pedida).
pub fn de_cpuid(cpuid: impl Fn(u32, u32) -> [u32; 4]) -> u64 {
    let max = cpuid(0, 0)[0];
    let max_ext = cpuid(0x8000_0000, 0)[0];
    let mut hay = 0;
    for x in EXTENSIONES.iter() {
        let llega = if x.hoja >= 0x8000_0000 { x.hoja <= max_ext } else { x.hoja <= max };
        if !llega {
            continue;
        }
        let r = cpuid(x.hoja, x.subhoja);
        let v = match x.reg {
            Reg::Ebx => r[1],
            Reg::Ecx => r[2],
            Reg::Edx => r[3],
        };
        if v >> x.bit_cpuid & 1 != 0 {
            hay |= x.bit;
        }
    }
    hay
}

/// **Lo que el programa pide y la CPU no tiene.** 0: se admite. Un bit que
/// este sistema no conoce tambien falta: no se sabe darlo.
pub const fn faltan(pide: u64, hay: u64) -> u64 {
    pide & !(hay & CONOCIDAS)
}

/// Los nombres de una mascara, en orden; los bits desconocidos, como `?`.
pub fn nombres(mascara: u64) -> impl Iterator<Item = &'static str> {
    let desconocido = mascara & !CONOCIDAS != 0;
    EXTENSIONES.iter().filter(move |x| mascara & x.bit != 0).map(|x| x.nombre).chain(desconocido.then_some("?"))
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn nombres_de(m: u64) -> Vec<&'static str> {
        nombres(m).collect()
    }

    #[test]
    fn cada_bit_una_vez_y_en_orden() {
        for (k, x) in EXTENSIONES.iter().enumerate() {
            assert_eq!(x.bit, 1 << k, "{}", x.nombre);
            assert_eq!(por_nombre(x.nombre), Some(x));
        }
        assert_eq!(por_nombre("SSE2"), None, "SSE2 es x86-64 base: no se declara");
    }

    /// ** Un CPUID de mentira: el del Ryzen 5 5600X (Zen 3) en lo que importa
    /// aqui, y uno de un x86-64 de los primeros (sin nada de lo de arriba).
    fn zen3(hoja: u32, _sub: u32) -> [u32; 4] {
        match hoja {
            0 => [0x10, 0, 0, 0],
            1 => [0, 0, (1 << 0) | (1 << 9) | (1 << 19) | (1 << 20) | (1 << 23) | (1 << 28) | (1 << 12) | (1 << 30) | (1 << 27) | (1 << 3), 0],
            7 => [0, (1 << 3) | (1 << 8) | (1 << 5) | (1 << 18), 0, 0],
            0x8000_0000 => [0x8000_0020, 0, 0, 0],
            0x8000_0001 => [0, 0, 1 << 5, 1 << 27],
            _ => [0; 4],
        }
    }
    fn k8(hoja: u32, _sub: u32) -> [u32; 4] {
        match hoja {
            0 => [1, 0, 0, 0],
            0x8000_0000 => [0x8000_0001, 0, 0, 0],
            _ => [0; 4],
        }
    }

    #[test]
    fn el_ryzen_lo_tiene_todo() {
        assert_eq!(de_cpuid(zen3), CONOCIDAS);
    }

    /// ** El caso del plan: una CPU sin AVX2 y un `.bex` que lo usa: NO, con
    /// su nombre.
    #[test]
    fn sin_avx2_un_bex_que_lo_usa_no_entra_y_se_dice_cual() {
        let sin_avx2 = |h: u32, s: u32| {
            let mut r = zen3(h, s);
            if h == 7 {
                r[1] &= !(1 << 5);
            }
            r
        };
        let hay = de_cpuid(sin_avx2);
        let pide = por_nombre("AVX2").unwrap().bit | por_nombre("POPCNT").unwrap().bit;
        let f = faltan(pide, hay);
        assert_eq!(nombres_de(f), ["AVX2"]);
        assert_eq!(faltan(por_nombre("POPCNT").unwrap().bit, hay), 0);
    }

    /// Una hoja por encima de la maxima no se pregunta: el K8 no tiene la 7.
    #[test]
    fn una_cpu_vieja_no_tiene_nada_de_esto() {
        assert_eq!(de_cpuid(k8), 0);
        assert_eq!(nombres_de(faltan(CONOCIDAS, 0)).len(), 16);
    }

    #[test]
    fn un_bit_desconocido_falta_y_se_dice() {
        let raro = 1 << 40;
        assert_eq!(faltan(raro, u64::MAX), raro);
        assert_eq!(nombres_de(raro), ["?"]);
    }
}
