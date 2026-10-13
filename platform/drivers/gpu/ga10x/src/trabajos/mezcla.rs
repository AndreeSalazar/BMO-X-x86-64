//! **LA MEZCLA EN LA 3060** (12-10): lo que el pixel nuevo hace con el que
//! ya esta en el render target (`D3D12_RENDER_TARGET_BLEND_DESC`), con los
//! MISMOS bits que la CPU (`bmo_proton_x::mezcla::Mezcla::aplicar` y, a 8
//! bits, `trama::desempaquetar` / `empaquetar`).
//!
//! capa: puro -- la mezcla que la 3060 sabe, su forma en la receta y las
//! palabras del pegamento
//!
//! [eje]     CORRECCION -- un factor cambiado oscurece o quema la imagen
//!           entera. Las cuentas las prueba el banco contra la CPU
//!
//! # Por que en el SOMBREADOR y no en el ROP
//!
//! El ROP de la 3060 mezcla con su aritmetica, y a 8 bits nadie dice con
//! cuanta precision: la regla de la casa es "los mismos bits que la CPU"
//! (LI2g, DL10). En el sombreador las cuentas son las de IEEE, una a una:
//!
//! ```text
//!    donde   la posicion del pixel (IPA; x - 0.5 + 2^23: su entero EXACTO en
//!            los bits) -> su byte en el destino (fila * y + bytes * x)
//!    leer    LDG.E.STRONG.SYS de lo que ya esta (1 palabra a 8 bits o en un
//!            R32; 4 en RGBA de float)
//!    d       a 8 bits, byte / 255 (`formatos`, la conversion de UNORM8:
//!            los bits de `desempaquetar`); de float, tal cual
//!    mezcla  o * F(origen) <op> d * F(destino): FMUL, FMUL y FADD, cada uno
//!            con su redondeo -- lo que hace la CPU, sin FMA --; el alfa, el
//!            ultimo (los factores de color lo leen); la mascara, d
//!    y       el pixel sale como cualquier otro: a 8 bits lo escribe la 3060
//!            (como sin mezcla), de float va al redondeo de su formato
//! ```
//!
//! **Lo que pide** (la puerta lo comprueba): que ningun pixel se pinte DOS
//! VECES en el mismo lote -- el sombreador lee lo de ANTES del lote, no lo
//! del triangulo de al lado --, sin la Z (con Z el color va a la sombra en
//! bloque, no al destino) y sin limpieza en el lote. Y de las operaciones,
//! sumar y las dos restas: MIN, MAX y `SRC_ALPHA_SAT` usan un minimo, y el
//! signo del cero de un minimo no se promete igual (por la CPU).

use crate::cubo::{con_control, mov, ALU};
use crate::destino::{Destino, Flotante};
use crate::formatos;

/// Los factores (`D3D12_BLEND`) que la 3060 sabe.
pub const CERO: u8 = 1;
pub const UNO: u8 = 2;
pub const ORIGEN_COLOR: u8 = 3;
pub const INV_ORIGEN_COLOR: u8 = 4;
pub const ORIGEN_ALFA: u8 = 5;
pub const INV_ORIGEN_ALFA: u8 = 6;
pub const DESTINO_ALFA: u8 = 7;
pub const INV_DESTINO_ALFA: u8 = 8;
pub const DESTINO_COLOR: u8 = 9;
pub const INV_DESTINO_COLOR: u8 = 10;
pub const FACTOR: u8 = 14;
pub const INV_FACTOR: u8 = 15;
pub const FACTOR_ALFA: u8 = 20;
pub const INV_FACTOR_ALFA: u8 = 21;

/// Las operaciones (`D3D12_BLEND_OP`) que la 3060 sabe.
pub const SUMAR: u8 = 1;
pub const RESTAR: u8 = 2;
pub const RESTAR_AL_REVES: u8 = 3;

/// **La mezcla del render target 0**, y el factor de `OMSetBlendFactor` (sus
/// bits de f32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mezcla {
    pub encendida: bool,
    pub origen: u8,
    pub destino: u8,
    pub op: u8,
    pub origen_a: u8,
    pub destino_a: u8,
    pub op_a: u8,
    /// R 1, G 2, B 4, A 8.
    pub mascara: u8,
    pub factor: [u32; 4],
}

impl Default for Mezcla {
    fn default() -> Self {
        Mezcla::NINGUNA
    }
}

/// Lo que mide en la receta.
pub const BYTES: usize = 24;

impl Mezcla {
    /// Sin mezcla y los cuatro canales.
    pub const NINGUNA: Mezcla = Mezcla { encendida: false, origen: UNO, destino: CERO, op: SUMAR, origen_a: UNO, destino_a: CERO, op_a: SUMAR, mascara: 0xF, factor: [0x3F80_0000; 4] };

    /// El pixel nuevo, tal cual: no hace falta leer el destino.
    pub const fn trivial(&self) -> bool {
        !self.encendida && self.mascara == 0xF
    }

    /// Si la 3060 la sabe (ver el principio), o por que no.
    pub const fn valida(&self) -> Result<(), &'static str> {
        if self.mascara > 0xF {
            return Err("una mascara de escritura de mas de cuatro canales");
        }
        if !self.encendida {
            return Ok(());
        }
        let factores = [self.origen, self.destino, self.origen_a, self.destino_a];
        let mut k = 0;
        while k < 4 {
            if !matches!(factores[k], CERO..=INV_DESTINO_COLOR | FACTOR | INV_FACTOR | FACTOR_ALFA | INV_FACTOR_ALFA) {
                return Err("una mezcla con SRC_ALPHA_SAT, dos fuentes (SRC1) o un factor que no existe: el signo del cero de su minimo no se promete igual, por la CPU");
            }
            k += 1;
        }
        if !matches!(self.op, SUMAR..=RESTAR_AL_REVES) || !matches!(self.op_a, SUMAR..=RESTAR_AL_REVES) {
            return Err("una mezcla con MIN o MAX: el signo del cero de un minimo no se promete igual, por la CPU");
        }
        Ok(())
    }

    /// Sus bytes en la receta.
    pub fn a_bytes(&self) -> [u8; BYTES] {
        let mut b = [0u8; BYTES];
        b[..8].copy_from_slice(&[self.encendida as u8, self.origen, self.destino, self.op, self.origen_a, self.destino_a, self.op_a, self.mascara]);
        for (k, f) in self.factor.iter().enumerate() {
            b[8 + 4 * k..12 + 4 * k].copy_from_slice(&f.to_le_bytes());
        }
        b
    }

    /// De sus bytes, si es una que la 3060 sabe.
    pub fn de_bytes(b: &[u8]) -> Option<Mezcla> {
        let b = b.get(..BYTES)?;
        if b[0] > 1 {
            return None;
        }
        let f = |k: usize| u32::from_le_bytes([b[8 + 4 * k], b[9 + 4 * k], b[10 + 4 * k], b[11 + 4 * k]]);
        let m = Mezcla { encendida: b[0] == 1, origen: b[1], destino: b[2], op: b[3], origen_a: b[4], destino_a: b[5], op_a: b[6], mascara: b[7], factor: [f(0), f(1), f(2), f(3)] };
        m.valida().ok().map(|_| m)
    }
}

// == Las palabras (las de `bmo-sm86`, ver las pruebas) =======================

const SIEMPRE: u64 = 7 << 12;
const RZ: u64 = 0xFF;

const fn alu(lo: u64, hi: u64) -> (u64, u64) {
    (lo | SIEMPRE, con_control(hi, ALU))
}

/// `MOV Rd, Rb`.
pub const fn mover(rd: u64, rb: u64) -> (u64, u64) {
    alu(0x002 | 1 << 9 | rd << 16 | rb << 32, 0xF << 8)
}

/// `IMAD Rd, Ra, imm, Rc`.
pub const fn imad(rd: u64, ra: u64, imm: u32, rc: u64) -> (u64, u64) {
    alu(0x024 | 4 << 9 | rd << 16 | ra << 24 | (imm as u64) << 32, 0x078e_0200 | rc)
}

/// `FADD Rd, [-]Ra, [-]Rb`.
pub const fn fadd(rd: u64, ra: u64, menos_a: bool, rb: u64, menos_b: bool) -> (u64, u64) {
    alu(0x021 | 1 << 9 | rd << 16 | ra << 24 | rb << 32 | (menos_b as u64) << 63, (menos_a as u64) << 8)
}

/// `FADD Rd, -Ra, imm`: `imm - Ra`.
pub const fn menos(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    alu(0x021 | 2 << 9 | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, 1 << 8)
}

/// `FMUL Rd, Ra, Rb`.
pub const fn fmul(rd: u64, ra: u64, rb: u64) -> (u64, u64) {
    alu(0x020 | 1 << 9 | rd << 16 | ra << 24 | rb << 32, 1 << 22)
}

/// Lo mas que mide lo que el pegamento pone por la mezcla.
pub const MAX_INSTRUCCIONES: usize = 96;

/// La barrera de las lecturas del destino (la de las cargas del pegamento:
/// tras el cuerpo, libre).
pub const B_LEER: u64 = 3;

/// Donde esta un registro de trabajo: `s .. s + 8` (`s` par), del pegamento.
struct Trabajo(u64);

impl Trabajo {
    /// La posicion x e y; los temporales de cada canal.
    const fn t(&self, k: u64) -> u64 {
        self.0 + k
    }
    /// Lo que ya estaba, canal `c`.
    const fn d(&self, c: u64) -> u64 {
        self.0 + 4 + c
    }
}

/// **Leer lo que ya esta** (la parte que no se simula: IPA ya hecho en `s`
/// y `s + 1`, la direccion y los LDG). La primera espera la barrera 0 (los
/// IPA); los LDG escriben [`B_LEER`].
/// `desplazamiento`: donde empieza el destino en su pagina (Q0a1b, a 128 B):
/// va en el inmediato del LDG.
pub fn leer_destino(dst: &Destino, desplazamiento: u32, s: u64, o: &mut impl FnMut((u64, u64))) {
    let w = Trabajo(s);
    // x e y: el centro del pixel (n + 0.5); n - 0.5 + 2^23 es un entero
    // EXACTO (< 2^24), y sus bits menos los de 2^23 son n.
    o(crate::formatos::fadd(w.t(0), w.t(0), 8_388_607.5));
    o(crate::formatos::fadd(w.t(1), w.t(1), 8_388_607.5));
    o(crate::formatos::iadd3(w.t(0), w.t(0), 0xB500_0000));
    o(crate::formatos::iadd3(w.t(1), w.t(1), 0xB500_0000));
    // El byte: fila * y + bytes * x, bajo `destino::VA` (sus 32 bits bajos
    // son 0 y el mapa mide menos de 4 GiB: sin acarreo).
    o(imad(w.t(2), w.t(1), dst.fila, RZ));
    o(imad(w.t(2), w.t(0), dst.flotante.bytes(), w.t(2)));
    o(mov(w.t(3), (crate::destino::VA >> 32) as u32));
    let palabras = if dst.flotante == Flotante::Rgba32 { 4 } else { 1 };
    for c in 0..4u64 {
        // A 8 bits, la misma palabra en los cuatro (cada uno saca su byte).
        let desde = desplazamiento + if palabras == 4 { 4 * c as u32 } else { 0 };
        if palabras == 4 || dst.flotante == Flotante::No || c == 0 {
            o(crate::tuberia::ldg(w.d(c), w.t(2), desde, crate::tuberia::carga(B_LEER)));
        }
    }
}

/// **La mezcla** con lo de antes ya en `s + 4 .. s + 8` (crudo: la palabra a
/// 8 bits, o sus floats) y el pixel nuevo en R0..R3: el resultado, en R0..R3.
/// La primera instruccion espera [`B_LEER`] (la pone quien llama).
pub fn mezclar(m: &Mezcla, dst: &Destino, s: u64, o: &mut impl FnMut((u64, u64))) {
    let w = Trabajo(s);
    // d: a 8 bits, cada byte / 255 (en el orden del destino); en un R32, el
    // r y (0, 0, 1); en RGBA de float, tal cual.
    match dst.flotante {
        Flotante::No => {
            let f = formatos::forma(if dst.rgb { 28 } else { 87 }).expect("RGBA8 y BGRA8 los lee la 3060");
            for c in 0..4u8 {
                let (_, desplazamiento, bits) = f.lugar(c).expect("los cuatro");
                let mut v = [(0, 0); formatos::MAX_CONVERSION];
                let n = formatos::conversion(&f, w.d(c as u64), w.t(0), desplazamiento, bits, &mut v);
                for &x in &v[..n] {
                    o(x);
                }
            }
        }
        Flotante::R32 => {
            o(mov(w.d(1), 0));
            o(mov(w.d(2), 0));
            o(mov(w.d(3), 0x3F80_0000));
        }
        Flotante::Rgba32 => {}
        // Un render target de ENTEROS no mezcla en D3D12; `tuberia::cabe` no
        // deja llegar aqui una mezcla con el.
        Flotante::Uint8 => {}
    }
    for c in 0..4u64 {
        if m.mascara & (1 << c) == 0 {
            o(mover(c, w.d(c)));
            continue;
        }
        if !m.encendida {
            continue;
        }
        let (fo, fd, op) = if c < 3 { (m.origen, m.destino, m.op) } else { (m.origen_a, m.destino_a, m.op_a) };
        // a = o * F(origen) en t2; b = d * F(destino) en t3.
        for (factor, valor, sale, tmp) in [(fo, c, w.t(2), w.t(0)), (fd, w.d(c), w.t(3), w.t(1))] {
            let k = |i: usize| f32::from_bits(m.factor[i]);
            let inmediato = |x: f32, o: &mut dyn FnMut((u64, u64))| o(crate::formatos::fmul(sale, valor, x));
            let registro = |r: u64, o: &mut dyn FnMut((u64, u64))| o(fmul(sale, valor, r));
            let uno_menos = |r: u64, o: &mut dyn FnMut((u64, u64))| {
                o(menos(tmp, r, 1.0));
                o(fmul(sale, valor, tmp));
            };
            match factor {
                CERO => inmediato(0.0, o),
                UNO => inmediato(1.0, o),
                ORIGEN_COLOR => registro(c, o),
                INV_ORIGEN_COLOR => uno_menos(c, o),
                ORIGEN_ALFA => registro(3, o),
                INV_ORIGEN_ALFA => uno_menos(3, o),
                DESTINO_ALFA => registro(w.d(3), o),
                INV_DESTINO_ALFA => uno_menos(w.d(3), o),
                DESTINO_COLOR => registro(w.d(c), o),
                INV_DESTINO_COLOR => uno_menos(w.d(c), o),
                FACTOR => inmediato(k(c as usize), o),
                // `1.0 - k`, la cuenta de la CPU, en la CPU.
                INV_FACTOR => inmediato(1.0 - k(c as usize), o),
                FACTOR_ALFA => inmediato(k(3), o),
                _ => inmediato(1.0 - k(3), o),
            }
        }
        o(match op {
            RESTAR => fadd(c, w.t(2), false, w.t(3), true),
            RESTAR_AL_REVES => fadd(c, w.t(3), false, w.t(2), true),
            _ => fadd(c, w.t(2), false, w.t(3), false),
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_sm86::codifica as c;

    /// ** Q0a1b (12-10): el destino que empieza a mitad de pagina se lee en
    /// SU sitio: lo que se mete en la pagina va en el inmediato de cada LDG.
    #[test]
    fn leer_destino_a_mitad_de_pagina() {
        extern crate std;
        // (formato, palabras distintas, LDG): a 8 bits, la MISMA palabra en
        // los cuatro canales.
        for (flotante, palabras, cargas) in [(Flotante::Rgba32, 4u32, 4usize), (Flotante::No, 1, 4), (Flotante::R32, 1, 1)] {
            let dst = Destino { fila: 256 * flotante.bytes(), ancho: 256, alto: 4, flotante, rgb: false };
            for desp in [0u32, 0x400, 0xf80] {
                let mut w = std::vec::Vec::new();
                leer_destino(&dst, desp, 20, &mut |x| w.push(x));
                let t = Trabajo(20);
                for c in 0..palabras {
                    let ldg = crate::tuberia::ldg(t.d(c as u64), t.t(2), desp + 4 * c * (palabras == 4) as u32, crate::tuberia::carga(B_LEER));
                    assert!(w.contains(&ldg), "{flotante:?} {desp:#x}: el LDG {c}");
                }
                assert_eq!(w.iter().filter(|x| x.0 & 0xFFF == ldg_op()).count(), cargas);
            }
        }
    }

    fn ldg_op() -> u64 {
        crate::tuberia::ldg(0, 0, 0, 0).0 & 0xFFF
    }

    #[test]
    fn las_palabras_son_las_del_codificador() {
        let k = ALU;
        assert_eq!(mover(5, 6), c::mov(5, c::r(6), k));
        assert_eq!(imad(5, 6, 5120, 7), c::imad(5, 6, c::Fuente::Imm(5120), 7, k));
        assert_eq!(imad(5, 6, 4, 0xFF), c::imad(5, 6, c::Fuente::Imm(4), 0xFF, k));
        assert_eq!(fadd(5, 6, false, 7, true), c::fadd(5, c::r(6), c::neg(7), false, k));
        assert_eq!(fadd(5, 6, false, 7, false), c::fadd(5, c::r(6), c::r(7), false, k));
        assert_eq!(menos(5, 6, 1.0), c::fadd(5, c::neg(6), c::Fuente::Imm(1.0f32.to_bits()), false, k));
        assert_eq!(fmul(5, 6, 7), c::fmul(5, c::r(6), c::r(7), false, k));
    }

    #[test]
    fn la_receta_la_lleva_y_lo_que_no_sabe_no() {
        let m = Mezcla { encendida: true, origen: ORIGEN_ALFA, destino: INV_ORIGEN_ALFA, op: SUMAR, origen_a: UNO, destino_a: INV_ORIGEN_ALFA, op_a: SUMAR, mascara: 0x7, factor: [0.25f32.to_bits(); 4] };
        assert_eq!(Mezcla::de_bytes(&m.a_bytes()), Some(m));
        assert_eq!(Mezcla::de_bytes(&Mezcla::NINGUNA.a_bytes()), Some(Mezcla::NINGUNA));
        assert!(Mezcla { op: 4, ..m }.valida().is_err(), "MIN");
        assert!(Mezcla { op_a: 5, ..m }.valida().is_err(), "MAX");
        assert!(Mezcla { origen: 11, ..m }.valida().is_err(), "SRC_ALPHA_SAT");
        assert!(Mezcla { destino: 16, ..m }.valida().is_err(), "SRC1");
        assert_eq!(Mezcla::de_bytes(&Mezcla { origen: 11, ..m }.a_bytes()), None);
        assert!(Mezcla::NINGUNA.trivial() && !Mezcla { mascara: 7, ..Mezcla::NINGUNA }.trivial());
    }
}

/// *** LAS CUENTAS, contra la CPU: cada factor, cada operacion y mascaras,
/// con pixeles nuevos de todo tipo (negativos, de mas de 1, ceros, -0) y lo
/// de antes a 8 bits (BGRA y RGBA) o de float -- la 3060 de mentira da los
/// bits de `Mezcla::aplicar` (y, a 8 bits, el byte de `empaquetar`).
#[cfg(test)]
mod contra_la_cpu {
    extern crate std;
    use super::*;
    use bmo_proton_x::mezcla as cpu;
    use bmo_proton_x_sm86::simula::{correr, Maquina};
    use std::vec::Vec;

    const S: u64 = 20;

    fn en_la_3060(m: &Mezcla, dst: &Destino, o: [f32; 4], antes: [u32; 4]) -> [u32; 4] {
        let mut codigo = Vec::new();
        mezclar(m, dst, S, &mut |w| codigo.push(w));
        codigo.push(bmo_sm86::codifica::exit(ALU));
        let mut q = Maquina::nueva([&[]; 8]);
        for c in 0..4 {
            q.r[c] = o[c].to_bits();
            // A 8 bits, la misma palabra en los cuatro (como `leer_destino`).
            q.r[(S + 4) as usize + c] = if dst.flotante == Flotante::Rgba32 { antes[c] } else { antes[0] };
        }
        correr(&codigo, &mut q).unwrap_or_else(|x| panic!("{x:?}"));
        [q.r[0], q.r[1], q.r[2], q.r[3]]
    }

    fn a_la_cpu(m: &Mezcla) -> cpu::Mezcla {
        cpu::Mezcla { encendida: m.encendida, origen: m.origen, destino: m.destino, op: m.op, origen_a: m.origen_a, destino_a: m.destino_a, op_a: m.op_a, mascara: m.mascara }
    }

    #[test]
    fn cada_factor_y_cada_operacion_dan_los_bits_de_la_cpu() {
        let factores = [CERO, UNO, ORIGEN_COLOR, INV_ORIGEN_COLOR, ORIGEN_ALFA, INV_ORIGEN_ALFA, DESTINO_ALFA, INV_DESTINO_ALFA, DESTINO_COLOR, INV_DESTINO_COLOR, FACTOR, INV_FACTOR, FACTOR_ALFA, INV_FACTOR_ALFA];
        let nuevos = [[0.8f32, 0.1, -0.25, 0.4], [1.5, 0.0, -0.0, 1.0], [0.3333, 0.6667, 0.123_456_7, 0.0], [2.0, -1.0, 0.5, 0.999]];
        let antes8 = [0xFF80_4000u32, 0x0000_0000, 0x7F12_C3A9, 0x01FE_8001];
        let antesf = [[0.5f32.to_bits(), 0.25f32.to_bits(), (-3.0f32).to_bits(), 1.0f32.to_bits()], [0x3EAA_AAAB, 0, 0x8000_0000, 0x3F00_0000]];
        let k = [0.2f32, 0.6, 1.0, 0.35];
        let mut probadas = 0;
        let mut semilla = 0x1234_5678u32;
        let mut azar = |n: usize| {
            semilla ^= semilla << 13;
            semilla ^= semilla >> 17;
            semilla ^= semilla << 5;
            semilla as usize % n
        };
        for &fo in &factores {
            for &fd in &factores {
                for op in [SUMAR, RESTAR, RESTAR_AL_REVES] {
                    let m = Mezcla { encendida: true, origen: fo, destino: fd, op, origen_a: factores[azar(14)], destino_a: factores[azar(14)], op_a: 1 + azar(3) as u8, mascara: [0xF, 0x7, 0xB, 0x1][azar(4)], factor: k.map(f32::to_bits) };
                    for (rgb, flotante) in [(true, Flotante::No), (false, Flotante::No), (true, Flotante::Rgba32), (true, Flotante::R32)] {
                        let dst = Destino { fila: 5120, ancho: 1280, alto: 720, rgb, flotante };
                        for o in nuevos {
                            let antes: Vec<[u32; 4]> = match flotante {
                                Flotante::No => antes8.iter().map(|&p| [p; 4]).collect(),
                                _ => antesf.to_vec(),
                            };
                            for a in antes {
                                let g = en_la_3060(&m, &dst, o, a);
                                let d: [f32; 4] = match flotante {
                                    Flotante::No => bmo_proton_x::trama::desempaquetar(a[0], !rgb),
                                    Flotante::R32 => [f32::from_bits(a[0]), 0.0, 0.0, 1.0],
                                    Flotante::Rgba32 => a.map(f32::from_bits),
                                    Flotante::Uint8 => unreachable!("un destino de enteros no mezcla (`tuberia::cabe`)"),
                                };
                                let c = a_la_cpu(&m).aplicar(o, d, k);
                                for x in 0..4 {
                                    assert!(g[x] == c[x].to_bits() || (f32::from_bits(g[x]).is_nan() && c[x].is_nan()), "{m:?} {flotante:?} rgb {rgb}: o {o:?} d {d:?}, canal {x}: la 3060 {:#x} y la CPU {:#x}", g[x], c[x].to_bits());
                                }
                                if flotante == Flotante::No {
                                    assert_eq!(bmo_proton_x::trama::empaquetar(g.map(f32::from_bits), !rgb), bmo_proton_x::trama::empaquetar(c, !rgb));
                                }
                                probadas += 1;
                            }
                        }
                    }
                }
            }
        }
        std::eprintln!("{probadas} mezclas, todas las de la CPU");
        assert!(probadas > 20_000);
    }

    /// Sin mezcla pero con mascara: los canales fuera, lo de antes (a 8
    /// bits, el mismo byte otra vez).
    #[test]
    fn la_mascara_deja_lo_de_antes() {
        let m = Mezcla { mascara: 0x5, ..Mezcla::NINGUNA };
        let dst = Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false, flotante: Flotante::No };
        let g = en_la_3060(&m, &dst, [0.9, 0.9, 0.9, 0.9], [0x8040_2010; 4]);
        let c = a_la_cpu(&m).aplicar([0.9; 4], bmo_proton_x::trama::desempaquetar(0x8040_2010, true), [1.0; 4]);
        assert_eq!(g, c.map(f32::to_bits));
        assert_eq!(bmo_proton_x::trama::empaquetar(c, true) & 0xFF00_FF00, 0x8000_2000, "G y A, los de antes");
    }
}
