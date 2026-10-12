//! **LOS FORMATOS DE VERTICE DE LA 3060** (LI2e, 12-10): como lee el
//! pegamento del de vertice un elemento que NO es float de 32 bits -- UNORM,
//! SNORM, half, enteros de 8, 16 y 32, R10G10B10A2 y B8G8R8A8 --, con los
//! MISMOS bits que la CPU (`bmo_prometeo::formato_ia::leer`, la referencia).
//!
//! capa: puro -- una tabla de formatos y las palabras de cada conversion
//!
//! [eje]     CORRECCION -- un vertice mal leido no falla: se dibuja en otro
//!           sitio. Cada clase la prueba el banco con TODOS sus valores (los
//!           256 de 8 bits, los 65 536 de 16) contra la CPU
//!
//! El censo de Cyberpunk (metal del 12-10): 12 de 34 lotes los paraba "un
//! formato de vertice que no es float de 32 bits".
//!
//! ```text
//!    cargar     un LDG de la palabra de 32 bits que lleva el componente
//!               (el elemento empieza en 4 bytes; ninguno cruza palabras)
//!    sacarlo    SHF.L y SHF.R (con signo en SNORM y SINT): sus bits, abajo
//!    UNORM      n | 0x4B000000 es el float 2^23 + n: menos 2^23, n EXACTO
//!               (sin I2F, que es de latencia variable); y n / (2^b - 1)
//!               con el cociente corregido: q = n * r, e = n - q * D (una
//!               FFMA, exacta), q + e * r -- el redondeo de la division de
//!               IEEE, comprobado en todo el dominio
//!    SNORM      lo mismo con 0x4B400000 (1.5 * 2^23 + n, n con signo), y
//!               max(-1, x): el mas bajo es -1, como D3D
//!    half       HADD2.F32 (exacto)
//!    UINT/SINT  los bits del entero (el interprete los guarda asi)
//!    lo que no  0, 0, 0 y 1 (el 1 como float, o como entero en UINT/SINT)
//!    trae
//!    B8G8R8A8   el sombreador ve RGBA: x y z se cambian
//! ```
//!
//! Todas las instrucciones son de latencia FIJA (6 ciclos, `cubo::ALU`): sin
//! barreras nuevas. Las palabras son las de `bmo-sm86` (el codificador con
//! oro de `ptxas`), comprobadas bit a bit en las pruebas.

use crate::cubo::{con_control, ALU};

/// Como se lee cada componente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clase {
    Float,
    Unorm,
    Snorm,
    Uint,
    Sint,
}

/// **La forma de un formato que la 3060 lee**: la clase, los bits de cada
/// componente (0: no lo trae) y si viene al reves (B8G8R8A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forma {
    pub clase: Clase,
    pub bits: [u8; 4],
    pub bgra: bool,
}

impl Forma {
    const fn de(clase: Clase, bits: [u8; 4]) -> Forma {
        Forma { clase, bits, bgra: false }
    }

    /// Cuantos componentes trae.
    pub const fn componentes(&self) -> u8 {
        (self.bits[0] != 0) as u8 + (self.bits[1] != 0) as u8 + (self.bits[2] != 0) as u8 + (self.bits[3] != 0) as u8
    }

    /// Cuantos bytes ocupa.
    pub const fn bytes(&self) -> u32 {
        (self.bits[0] as u32 + self.bits[1] as u32 + self.bits[2] as u32 + self.bits[3] as u32) / 8
    }

    /// Si es float de 32 bits por componente (el LDG, tal cual).
    pub const fn es_float32(&self) -> bool {
        matches!(self.clase, Clase::Float) && self.bits[0] == 32
    }

    /// **Donde esta el componente `c` que ve el sombreador**: `(palabra,
    /// desplazamiento, bits)`, o `None` si el formato no lo trae.
    pub const fn lugar(&self, c: u8) -> Option<(u32, u32, u32)> {
        if c > 3 {
            return None;
        }
        // B8G8R8A8: el sombreador ve RGBA; en memoria, B primero.
        let c = if self.bgra && c != 1 && c != 3 { 2 - c } else { c } as usize;
        if self.bits[c] == 0 {
            return None;
        }
        let mut desde = 0u32;
        let mut k = 0;
        while k < c {
            desde += self.bits[k] as u32;
            k += 1;
        }
        Some((desde / 32, desde % 32, self.bits[c] as u32))
    }

    /// Lo que vale un componente que el formato NO trae: 0, y 1 en w (como
    /// float, o como entero en UINT y SINT).
    pub const fn falta(&self, c: u8) -> u32 {
        match (c, self.clase) {
            (3, Clase::Uint | Clase::Sint) => 1,
            (3, _) => 0x3F80_0000,
            _ => 0,
        }
    }
}

/// **La forma de un `DXGI_FORMAT` de vertice que la 3060 lee**, o `None`:
/// R11G11B10_FLOAT (26) todavia no, ni lo que no es de vertice.
pub const fn forma(formato: u32) -> Option<Forma> {
    use Clase::*;
    Some(match formato {
        2 => Forma::de(Float, [32, 32, 32, 32]),
        3 => Forma::de(Uint, [32, 32, 32, 32]),
        4 => Forma::de(Sint, [32, 32, 32, 32]),
        6 => Forma::de(Float, [32, 32, 32, 0]),
        7 => Forma::de(Uint, [32, 32, 32, 0]),
        8 => Forma::de(Sint, [32, 32, 32, 0]),
        10 => Forma::de(Float, [16, 16, 16, 16]),
        11 => Forma::de(Unorm, [16, 16, 16, 16]),
        12 => Forma::de(Uint, [16, 16, 16, 16]),
        13 => Forma::de(Snorm, [16, 16, 16, 16]),
        14 => Forma::de(Sint, [16, 16, 16, 16]),
        16 => Forma::de(Float, [32, 32, 0, 0]),
        17 => Forma::de(Uint, [32, 32, 0, 0]),
        18 => Forma::de(Sint, [32, 32, 0, 0]),
        24 => Forma::de(Unorm, [10, 10, 10, 2]),
        25 => Forma::de(Uint, [10, 10, 10, 2]),
        28 => Forma::de(Unorm, [8, 8, 8, 8]),
        30 => Forma::de(Uint, [8, 8, 8, 8]),
        31 => Forma::de(Snorm, [8, 8, 8, 8]),
        32 => Forma::de(Sint, [8, 8, 8, 8]),
        34 => Forma::de(Float, [16, 16, 0, 0]),
        35 => Forma::de(Unorm, [16, 16, 0, 0]),
        36 => Forma::de(Uint, [16, 16, 0, 0]),
        37 => Forma::de(Snorm, [16, 16, 0, 0]),
        38 => Forma::de(Sint, [16, 16, 0, 0]),
        41 => Forma::de(Float, [32, 0, 0, 0]),
        42 => Forma::de(Uint, [32, 0, 0, 0]),
        43 => Forma::de(Sint, [32, 0, 0, 0]),
        49 => Forma::de(Unorm, [8, 8, 0, 0]),
        50 => Forma::de(Uint, [8, 8, 0, 0]),
        51 => Forma::de(Snorm, [8, 8, 0, 0]),
        52 => Forma::de(Sint, [8, 8, 0, 0]),
        54 => Forma::de(Float, [16, 0, 0, 0]),
        56 => Forma::de(Unorm, [16, 0, 0, 0]),
        57 => Forma::de(Uint, [16, 0, 0, 0]),
        58 => Forma::de(Snorm, [16, 0, 0, 0]),
        59 => Forma::de(Sint, [16, 0, 0, 0]),
        61 => Forma::de(Unorm, [8, 0, 0, 0]),
        62 => Forma::de(Uint, [8, 0, 0, 0]),
        63 => Forma::de(Snorm, [8, 0, 0, 0]),
        64 => Forma::de(Sint, [8, 0, 0, 0]),
        87 => Forma { clase: Unorm, bits: [8, 8, 8, 8], bgra: true },
        _ => return None,
    })
}

// == Las palabras (las de `bmo-sm86`, ver las pruebas) =======================

const SIEMPRE: u64 = 7 << 12;
const RZ: u64 = 0xFF;

const fn alu(lo: u64, hi: u64) -> (u64, u64) {
    (lo | SIEMPRE, con_control(hi, ALU))
}

/// `SHF.L.U32 Rd, Ra, n, RZ`.
pub const fn shl(rd: u64, ra: u64, n: u32) -> (u64, u64) {
    alu(0x019 | 4 << 9 | rd << 16 | ra << 24 | (n as u64) << 32, 0x0600 | RZ)
}

/// `SHF.R.{U32,S32}.HI Rd, RZ, n, Ra`.
pub const fn shr(rd: u64, ra: u64, n: u32, con_signo: bool) -> (u64, u64) {
    let tipo = if con_signo { 0x0400 } else { 0x0600 };
    alu(0x019 | 4 << 9 | rd << 16 | RZ << 24 | (n as u64) << 32, 0x0001_1000 | tipo | ra)
}

/// `IADD3 Rd, Ra, imm, RZ`.
pub const fn iadd3(rd: u64, ra: u64, imm: u32) -> (u64, u64) {
    alu(0x010 | 4 << 9 | rd << 16 | ra << 24 | (imm as u64) << 32, 0x07ff_e000 | RZ)
}

/// `FADD Rd, Ra, imm`.
pub const fn fadd(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    alu(0x021 | 2 << 9 | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, 0)
}

/// `FMUL Rd, Ra, imm`.
pub const fn fmul(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    alu(0x020 | 4 << 9 | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, 1 << 22)
}

/// `FFMA Rd, [-]Ra, imm, Rc`.
pub const fn ffma(rd: u64, ra: u64, negada: bool, imm: f32, rc: u64) -> (u64, u64) {
    alu(0x023 | 4 << 9 | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, rc | (negada as u64) << 8)
}

/// `FMNMX Rd, Ra, imm, !PT`: el mayor.
pub const fn fmax(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    alu(0x009 | 4 << 9 | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, 15 << 23)
}

/// Lo mas que mide la conversion de un componente.
pub const MAX_CONVERSION: usize = 8;

/// **La conversion de un componente** que ya esta, crudo, en `r` (su
/// palabra de 32 bits): sus bits abajo y, segun la clase, el float que lee
/// la CPU. `t` y `t + 1`: dos registros de trabajo. Devuelve cuantas puso
/// en `sale` (0: el LDG ya es el valor, un float o un entero de 32).
pub fn conversion(f: &Forma, r: u64, t: u64, desplazamiento: u32, bits: u32, sale: &mut [(u64, u64); MAX_CONVERSION]) -> usize {
    let mut n = 0;
    let mut p = |w: (u64, u64)| {
        sale[n] = w;
        n += 1;
    };
    if bits < 32 {
        let con_signo = matches!(f.clase, Clase::Snorm | Clase::Sint);
        let arriba = 32 - desplazamiento - bits;
        if arriba > 0 {
            p(shl(r, r, arriba));
        }
        p(shr(r, r, 32 - bits, con_signo));
    }
    match f.clase {
        Clase::Float if bits == 16 => p(crate::pegamento::hadd2_f32(r, r, ALU)),
        Clase::Unorm | Clase::Snorm => {
            let (magia, menos, d) = match f.clase {
                Clase::Unorm => (0x4B00_0000, -8_388_608.0f32, ((1u64 << bits) - 1) as f32),
                _ => (0x4B40_0000, -12_582_912.0f32, ((1u64 << (bits - 1)) - 1) as f32),
            };
            let inverso = 1.0 / d;
            // n EXACTO, como float.
            p(iadd3(r, r, magia));
            p(fadd(r, r, menos));
            // q = n * (1/D); e = n - q * D; q + e * (1/D).
            p(fmul(t, r, inverso));
            p(ffma(t + 1, t, true, d, r));
            p(ffma(r, t + 1, false, inverso, t));
            if f.clase == Clase::Snorm {
                p(fmax(r, r, -1.0));
            }
        }
        _ => {}
    }
    n
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_sm86::codifica as c;

    /// Cada palabra, la de `bmo-sm86` (control incluido): el codificador
    /// con oro de `ptxas`.
    #[test]
    fn las_palabras_son_las_del_codificador() {
        let k = ALU;
        assert_eq!(shl(5, 6, 8), c::shl(5, 6, c::Fuente::Imm(8), k));
        assert_eq!(shr(5, 6, 24, false), c::shr(5, 6, c::Fuente::Imm(24), false, k));
        assert_eq!(shr(5, 6, 24, true), c::shr(5, 6, c::Fuente::Imm(24), true, k));
        assert_eq!(iadd3(5, 6, 0x4B00_0000), c::iadd3(5, 6, c::Fuente::Imm(0x4B00_0000), k));
        assert_eq!(fadd(5, 6, -8_388_608.0), c::fadd(5, c::r(6), c::Fuente::Imm((-8_388_608.0f32).to_bits()), false, k));
        assert_eq!(fmul(5, 6, 0.5), c::fmul(5, c::r(6), c::Fuente::Imm(0.5f32.to_bits()), false, k));
        assert_eq!(ffma(5, 6, true, 255.0, 7), c::ffma(5, c::neg(6), c::Fuente::Imm(255.0f32.to_bits()), c::r(7), false, k));
        assert_eq!(ffma(5, 6, false, 0.25, 7), c::ffma(5, c::r(6), c::Fuente::Imm(0.25f32.to_bits()), c::r(7), false, k));
        assert_eq!(fmax(5, 6, -1.0), c::fmnmx(5, c::r(6), c::Fuente::Imm((-1.0f32).to_bits()), true, k));
    }

    #[test]
    fn donde_esta_cada_componente() {
        let f = forma(24).unwrap();
        assert_eq!((f.lugar(0), f.lugar(1), f.lugar(2), f.lugar(3)), (Some((0, 0, 10)), Some((0, 10, 10)), Some((0, 20, 10)), Some((0, 30, 2))));
        let b = forma(87).unwrap();
        assert_eq!((b.lugar(0), b.lugar(2)), (Some((0, 16, 8)), Some((0, 0, 8))), "B8G8R8A8: x es el tercer byte");
        let h = forma(10).unwrap();
        assert_eq!((h.lugar(2), h.lugar(3)), (Some((1, 0, 16)), Some((1, 16, 16))));
        assert_eq!(forma(49).unwrap().lugar(2), None);
        assert_eq!((forma(49).unwrap().falta(3), forma(50).unwrap().falta(3), forma(50).unwrap().falta(2)), (0x3F80_0000, 1, 0));
        assert_eq!(forma(26), None, "R11G11B10_FLOAT, todavia no");
        assert_eq!((forma(10).unwrap().bytes(), forma(24).unwrap().componentes()), (8, 4));
    }
}

/// *** EL DOMINIO ENTERO, contra la CPU: cada formato que la 3060 lee, cada
/// componente y CADA valor que cabe en sus bits (los 65 536 de 16), con lo
/// demas de la palabra lleno de basura -- la conversion en el simulador de
/// la 3060 da los bits de `formato_ia::leer` (un NaN de half, NaN en los
/// dos: la carga del NaN no la promete nadie).
#[cfg(test)]
mod dominio {
    use super::*;
    use bmo_proton_x_sm86::simula::{correr, Maquina};

    fn en_la_3060(codigo: &[(u64, u64)], palabra: u32) -> u32 {
        let mut todo = std::vec::Vec::from(codigo);
        todo.push(bmo_sm86::codifica::exit(ALU));
        let mut m = Maquina::nueva([&[]; 8]);
        m.r[0] = palabra;
        correr(&todo, &mut m).unwrap_or_else(|x| panic!("{x:?}"));
        m.r[0]
    }

    extern crate std;

    #[test]
    fn cada_valor_de_cada_formato_es_el_de_la_cpu() {
        let mut probados = 0u64;
        for formato in 0..100u32 {
            let Some(f) = forma(formato) else { continue };
            for c in 0..4u8 {
                let Some((palabra, desplazamiento, bits)) = f.lugar(c) else {
                    // Lo que no trae: lo de la CPU.
                    let cpu = bmo_proton_x::formato_ia::leer(formato, &[0u8; 16])[c as usize].to_bits();
                    assert_eq!(f.falta(c), cpu, "formato {formato}, componente {c} que no trae");
                    continue;
                };
                let mut codigo = [(0, 0); MAX_CONVERSION];
                let n = conversion(&f, 0, 10, desplazamiento, bits, &mut codigo);
                let valores: u64 = if bits >= 32 { 64 } else { 1 << bits };
                for k in 0..valores {
                    let crudo = if bits >= 32 { (k as u32).wrapping_mul(0x9E37_79B9) } else { k as u32 };
                    let mascara = if bits >= 32 { u32::MAX } else { ((1u64 << bits) - 1) as u32 } << desplazamiento;
                    // La basura alrededor: otra en cada valor.
                    let basura = (k as u32).wrapping_mul(0x2545_F491) ^ 0xA5A5_5A5A;
                    let w = (basura & !mascara) | (crudo << desplazamiento & mascara);
                    let mut bytes = [0u8; 16];
                    bytes[4 * palabra as usize..4 * palabra as usize + 4].copy_from_slice(&w.to_le_bytes());
                    let cpu = bmo_proton_x::formato_ia::leer(formato, &bytes)[c as usize];
                    let g = en_la_3060(&codigo[..n], w);
                    assert!(g == cpu.to_bits() || (f32::from_bits(g).is_nan() && cpu.is_nan()), "formato {formato}, componente {c}, crudo {crudo:#x}: la 3060 {g:#010x} y la CPU {:#010x}", cpu.to_bits());
                    probados += 1;
                }
            }
        }
        std::eprintln!("{probados} valores, todos los de la CPU");
        assert!(probados > 1_000_000);
    }
}
