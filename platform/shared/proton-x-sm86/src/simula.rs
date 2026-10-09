//! **La 3060 de mentira**: ejecuta en el anfitrion las palabras de SASS que
//! emite [`crate::emitir`], leyendo los MISMOS bits que lee la tarjeta (los de
//! `bmo_sm86::codifica`). Es el oraculo de E3: si el simulador y la casa
//! (`Programa::correr`) dan los mismos bits, el emisor tradujo bien.
//!
//! Lo que sabe ejecutar es lo que el emisor pone -- FADD, FMUL, FMNMX, MUFU,
//! MOV, TEX (P3b4c.8: el asa, a [`Maquina::muestrear`]), EXIT y NOP, con registros, inmediatos y constantes, `-`, `|x|` y
//! `.SAT`; y desde E6 (02-10) FSETP, ISETP, SEL, IADD3 y BRA, con los
//! predicados P0..P6 y el GUARDA de cada instruccion; desde E6c y E6d las
//! de enteros: IMAD, IMAD.HI, LOP3, SHF, IMNMX, IABS, I2F y F2I; desde
//! DL10 (09-10) FFMA, con un redondeo y en sus cuatro modos (`fma.rs`); y
//! desde DL12 (09-10) KILL, que acaba el hilo y deja [`Maquina::matado`] --; cualquier otra
//! palabra (u otra forma de esas) es [`NoSimula::Instruccion`], nunca un
//! "seguramente".
//!
//! Corre UN hilo: lo que un warp hace cuando sus hilos se separan en un
//! salto es cosa de la 3060 (y del metal); cada hilo, por su lado, hace
//! esto.
//!
//! # Lo que es MODELO y no hardware, dicho
//!
//! - `MUFU.RSQ`/`RCP`/`SQRT` se hacen como la casa (`1 / raiz(x)`, `1 / x`,
//!   `raiz(x)`): la 3060 da una APROXIMACION de un ULP o dos. Cuanto se
//!   separa, y si mueve un pixel del cubo, lo dice el metal (E5), no esto.
//!   DL10 (09-10): una cuenta que dependa del MUFU.RCP se prueba con el
//!   inverso MOVIDO (`Maquina::inverso_ulp`), como la 3060 se equivoca.
//! - FMNMX con un NaN da el otro (como la casa y como D3D); el orden de -0 y
//!   +0 que haga la 3060 no se modela.
//! - Los subnormales se conservan (FADD y FMUL sin `.FTZ`, como las emite).

use bmo_proton_x::dxil::programa::{raiz, saturar};

/// El registro cero.
const RZ: usize = 255;

/// Por que no se pudo simular, y en que instruccion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSimula {
    /// Una palabra que el simulador no sabe ejecutar.
    Instruccion(usize),
    /// El programa se acabo sin EXIT (o salto fuera de el).
    SinExit,
    /// Mas de [`PASOS_MAXIMOS`] instrucciones: un bucle que no sale.
    SinFin,
}

/// Lo que corre como mucho un programa en el simulador.
pub const PASOS_MAXIMOS: usize = 1 << 22;

/// La maquina: 256 registros (los bits de cada `f32`) y los bancos de
/// constantes (`c[banco][desp]`); lo que no esta se lee como 0.
pub struct Maquina<'a> {
    pub r: [u32; 256],
    /// P0..P6 (PT, el 7, es siempre cierto).
    pub p: [bool; 7],
    pub bancos: [&'a [u8]; 8],
    /// Lo que un TEX lee: `(asa, u, v)` -> los cuatro canales. Es el
    /// muestreo de la casa (`bmo_proton_x::textura`), que iguala a la 3060
    /// bit a bit en las 96 muestras medidas (`tests/metal_textura.rs`).
    pub muestrear: Option<&'a dyn Fn(u32, f32, f32) -> [f32; 4]>,
    /// ** DL10: cuantos ULP se mueve el resultado de MUFU.RCP (con su signo)
    /// cuando es un numero normal: la 3060 aproxima, y lo que viva de su
    /// inverso tiene que aguantarlo. 0, el de la casa.
    pub inverso_ulp: i32,
    /// ** DL12: un KILL acabo el hilo (el `discard`): su pixel no queda.
    pub matado: bool,
}

impl<'a> Maquina<'a> {
    pub fn nueva(bancos: [&'a [u8]; 8]) -> Self {
        Maquina { r: [0; 256], p: [false; 7], bancos, muestrear: None, inverso_ulp: 0, matado: false }
    }

    fn reg(&self, i: usize) -> u32 {
        if i == RZ {
            0
        } else {
            self.r[i]
        }
    }

    fn constante(&self, lo: u64) -> u32 {
        let banco = (lo >> 54 & 0x1F) as usize;
        let desp = ((lo >> 40 & 0x3FFF) * 4) as usize;
        self.bancos.get(banco).and_then(|b| b.get(desp..desp + 4)).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn modificar(bits: u32, neg: bool, abs: bool) -> u32 {
    let b = if abs { bits & 0x7FFF_FFFF } else { bits };
    if neg {
        b ^ 0x8000_0000
    } else {
        b
    }
}

fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// El valor de un predicado: 0..6, o 7 (PT); con `+8`, negado.
fn predicado(m: &Maquina, g: u64) -> bool {
    let v = if g & 7 == 7 { true } else { m.p[(g & 7) as usize] };
    v != (g & 8 != 0)
}

/// Una comparacion de FSETP (bits 76..80): las ordenadas no se cumplen con
/// un NaN, las `U` si; 0 nunca, 15 siempre.
fn compara_floats(como: u64, x: f32, y: f32) -> Option<bool> {
    let nan = x.is_nan() || y.is_nan();
    let base = match como & 7 {
        0 => false,
        1 => x < y,
        2 => x == y,
        3 => x <= y,
        4 => x > y,
        5 => x != y && !nan,
        6 => x >= y,
        _ => return None,
    };
    Some(if como >= 8 { base || nan } else { base })
}

/// La segunda fuente de una de ENTEROS: registro (con `-` de complemento a
/// dos, bit 63), inmediato (forma 4) o constante (forma 5).
fn entera(m: &Maquina, lo: u64, forma: u64) -> Option<u32> {
    match forma {
        1 => {
            let x = m.reg((lo >> 32 & 0xFF) as usize);
            Some(if lo >> 63 != 0 { x.wrapping_neg() } else { x })
        }
        4 => Some((lo >> 32) as u32),
        5 => Some(m.constante(lo)),
        _ => None,
    }
}

/// **Correr** `codigo` sobre `m` hasta su EXIT. Devuelve cuantas instrucciones
/// se ejecutaron.
pub fn correr(codigo: &[(u64, u64)], m: &mut Maquina) -> Result<usize, NoSimula> {
    let mut pc = 0usize;
    for pasos in 1..=PASOS_MAXIMOS {
        let n = pc;
        let Some(&(lo, hi)) = codigo.get(pc) else { return Err(NoSimula::SinExit) };
        pc += 1;
        // El guarda: si no se cumple, la instruccion no hace nada.
        if !predicado(m, lo >> 12 & 0xF) {
            continue;
        }
        let op = lo & 0x1FF;
        let forma = lo >> 9 & 7;
        let rd = (lo >> 16 & 0xFF) as usize;
        let ra = (lo >> 24 & 0xFF) as usize;
        let a = || modificar(m.reg(ra), hi >> 8 & 1 != 0, hi >> 9 & 1 != 0);
        // La segunda fuente: registro (con 62/63), inmediato o constante.
        let segunda = |imm: u64, cte: u64| -> Option<u32> {
            match forma {
                1 => Some(modificar(m.reg((lo >> 32 & 0xFF) as usize), lo >> 63 != 0, lo >> 62 & 1 != 0)),
                x if x == imm => Some((lo >> 32) as u32),
                x if x == cte => Some(m.constante(lo)),
                _ => None,
            }
        };
        let sat = |x: f32| if hi >> 13 & 1 != 0 { saturar(x) } else { x };
        // FSETP/ISETP: escriben Pu (81..84) combinando con PT por AND: el
        // segundo (84..87) y el que se combina (87..91) tienen que ser PT.
        let setp_sabido = hi >> 20 & 7 == 7 && hi >> 23 & 0xF == 7 && hi >> 10 & 3 == 0;
        let v: u32 = match op {
            0x00B if setp_sabido => {
                let y = f(segunda(4, 5).ok_or(NoSimula::Instruccion(n))?);
                let si = compara_floats(hi >> 12 & 0xF, f(a()), y).ok_or(NoSimula::Instruccion(n))?;
                if let Some(p) = m.p.get_mut((hi >> 17 & 7) as usize) {
                    *p = si;
                }
                continue;
            }
            0x00C if setp_sabido && hi >> 4 & 0xF == 7 && hi >> 8 & 1 == 0 => {
                let (x, y) = (m.reg(ra), segunda(4, 5).ok_or(NoSimula::Instruccion(n))?);
                let (x, y) = if hi >> 9 & 1 != 0 { (x as i32 as i64, y as i32 as i64) } else { (x as i64, y as i64) };
                let si = match hi >> 12 & 0xF {
                    1 => x < y,
                    2 => x == y,
                    3 => x <= y,
                    4 => x > y,
                    5 => x != y,
                    6 => x >= y,
                    _ => return Err(NoSimula::Instruccion(n)),
                };
                if let Some(p) = m.p.get_mut((hi >> 17 & 7) as usize) {
                    *p = si;
                }
                continue;
            }
            // SEL Rd, Ra, b, [!]Pp (87..91).
            0x007 if hi >> 8 & 3 == 0 => {
                let b = segunda(4, 5).ok_or(NoSimula::Instruccion(n))?;
                if predicado(m, hi >> 23 & 0xF) {
                    m.reg(ra)
                } else {
                    b
                }
            }
            // IADD3 Rd, Ra, b, Rc, sin acarreos (los de 77..91 como los pone
            // `ptxas`: `!PT`).
            // E6c: con `-` en una fuente, el de los ENTEROS (complemento a
            // dos), no el del bit de signo de un float.
            // E6d: y su ACARREO (el bit 32 de la suma) a Pu (81..84), y con
            // .X (74) sumando el de Pp (87..91). El acarreo de `a - b` es el
            // de `a + ~b + 1`: con b = 0, 1.
            0x010 if hi & ((1 << 41) - 1) & !(0x1FF | 1 << 10 | 7 << 17 | 0xF << 23) == 0x0071_e000 => {
                let x = hi >> 10 & 1 != 0;
                let pp = hi >> 23 & 0xF;
                if x == (pp == 0xF) {
                    return Err(NoSimula::Instruccion(n));
                }
                let termino = |v: u32, negado: bool| if negado { (!v) as u64 + 1 } else { v as u64 };
                let b = match forma {
                    1 => termino(m.reg((lo >> 32 & 0xFF) as usize), lo >> 63 != 0),
                    4 => (lo >> 32) as u64,
                    5 => m.constante(lo) as u64,
                    _ => return Err(NoSimula::Instruccion(n)),
                };
                let suma = termino(m.reg(ra), hi >> 8 & 1 != 0) + b + m.reg((hi & 0xFF) as usize) as u64 + (x && predicado(m, pp)) as u64;
                if let Some(p) = m.p.get_mut((hi >> 17 & 7) as usize) {
                    *p = suma >> 32 & 1 != 0;
                }
                suma as u32
            }
            // ** E6c: IMAD (sin negar la tercera), LOP3, SHF, IMNMX, I2F, F2I.
            0x024 if hi & ((1 << 41) - 1) & !0x3FF == 0x078e_0000 => {
                let b = entera(m, lo, forma).ok_or(NoSimula::Instruccion(n))?;
                m.reg(ra).wrapping_mul(b).wrapping_add(m.reg((hi & 0xFF) as usize))
            }
            // ** E6d: IMAD.HI.U32 Rd, Ra, b, RZ (la mitad alta, sin signo) e
            // IABS Rd, Rb.
            0x027 if hi & ((1 << 41) - 1) == 0x078e_00ff => {
                let b = entera(m, lo, forma).ok_or(NoSimula::Instruccion(n))?;
                ((m.reg(ra) as u64 * b as u64) >> 32) as u32
            }
            0x013 if forma == 1 && hi & ((1 << 41) - 1) == 0 => (m.reg((lo >> 32 & 0xFF) as usize) as i32).unsigned_abs(),
            0x012 if hi & ((1 << 41) - 1) & !0xFFFF == 0x078e_0000 => {
                let (x, y, z) = (m.reg(ra), entera(m, lo, forma).ok_or(NoSimula::Instruccion(n))?, m.reg((hi & 0xFF) as usize));
                let lut = hi >> 8 & 0xFF;
                (0..8).filter(|k| lut >> k & 1 != 0).fold(0u32, |r, k| r | (if k & 4 != 0 { x } else { !x }) & (if k & 2 != 0 { y } else { !y }) & (if k & 1 != 0 { z } else { !z }))
            }
            0x019 => {
                let cuenta = entera(m, lo, forma).ok_or(NoSimula::Instruccion(n))?.min(32);
                let v = m.reg((hi & 0xFF) as usize);
                match hi & ((1 << 41) - 1) & !0xFF {
                    // SHF.L.U32 Rd, Ra, n, RZ (la tercera, RZ).
                    0x0600 if hi & 0xFF == 0xFF => (m.reg(ra) as u64).checked_shl(cuenta).map_or(0, |x| x as u32),
                    // SHF.R.U32.HI / S32.HI Rd, RZ, n, Rc (el valor, en la tercera).
                    0x0001_1600 if ra == RZ => (v as u64 >> cuenta) as u32,
                    0x0001_1400 if ra == RZ => (v as i32 as i64 >> cuenta) as u32,
                    _ => return Err(NoSimula::Instruccion(n)),
                }
            }
            0x017 if hi & ((1 << 41) - 1) & !(1 << 26 | 1 << 9) == 7 << 23 => {
                let (x, y) = (m.reg(ra), entera(m, lo, forma).ok_or(NoSimula::Instruccion(n))?);
                let (mayor, signo) = (hi >> 26 & 1 != 0, hi >> 9 & 1 != 0);
                match (mayor, signo) {
                    (false, true) => (x as i32).min(y as i32) as u32,
                    (true, true) => (x as i32).max(y as i32) as u32,
                    (false, false) => x.min(y),
                    (true, false) => x.max(y),
                }
            }
            // I2F: al mas cercano o (E6d, `.RP`: 2 en 78..80) hacia arriba.
            0x106 if forma == 1 && hi & ((1 << 41) - 1) & !(1 << 10 | 2 << 14) == 0x20_1000 => {
                let x = m.reg((lo >> 32 & 0xFF) as usize);
                let exacto = if hi >> 10 & 1 != 0 { x as i32 as f64 } else { x as f64 };
                let y = exacto as f32;
                let y = if hi >> 15 & 1 != 0 && (y as f64) < exacto { f32::from_bits(if y > 0.0 { y.to_bits() + 1 } else { y.to_bits() - 1 }) } else { y };
                y.to_bits()
            }
            // F2I hacia cero; `.FTZ` (80) no cambia nada: un subnormal da 0
            // igual.
            0x105 if forma == 1 && hi & ((1 << 41) - 1) & !(1 << 8 | 1 << 16) == 0x20_f000 => {
                let x = f(m.reg((lo >> 32 & 0xFF) as usize));
                if hi >> 8 & 1 != 0 {
                    x as i32 as u32
                } else {
                    x as u32
                }
            }
            // BRA: el desplazamiento en BYTES (32..82, con signo) desde la
            // siguiente.
            0x147 if forma == 4 && hi >> 23 & 0xF == 7 => {
                let d = (((hi & 0x3_FFFF) << 32 | lo >> 32) << 14) as i64 >> 14;
                let destino = 16 * pc as i64 + d;
                if destino < 0 || destino % 16 != 0 {
                    return Err(NoSimula::Instruccion(n));
                }
                pc = (destino / 16) as usize;
                continue;
            }
            0x021 => sat(f(a()) + f(segunda(2, 3).ok_or(NoSimula::Instruccion(n))?)).to_bits(),
            0x020 => sat(f(a()) * f(segunda(4, 5).ok_or(NoSimula::Instruccion(n))?)).to_bits(),
            0x009 => {
                let (x, y) = (f(a()), f(segunda(4, 5).ok_or(NoSimula::Instruccion(n))?));
                let mayor = match hi >> 23 & 0xF {
                    7 => false,
                    15 => true,
                    _ => return Err(NoSimula::Instruccion(n)),
                };
                let otro = if mayor { y > x } else { y < x };
                (if x.is_nan() || otro { y } else { x }).to_bits()
            }
            // ** DL10: FFMA Rd, a, b, Rc -- UN redondeo, en su modo (78..80),
            // con la FFMA exacta (`fma.rs`). La tercera, con su `-` (75) y su
            // `|x|` (74); sin `.SAT` ni nada mas (no la emite el emisor).
            0x023 if matches!(forma, 1 | 4) && hi & ((1 << 41) - 1) & !(0xFF | 0xF << 8 | 3 << 14) == 0 => {
                let b = segunda(4, 5).ok_or(NoSimula::Instruccion(n))?;
                let c = modificar(m.reg((hi & 0xFF) as usize), hi >> 11 & 1 != 0, hi >> 10 & 1 != 0);
                let modo = match hi >> 14 & 3 {
                    0 => crate::fma::Redondeo::Cercano,
                    1 => crate::fma::Redondeo::Abajo,
                    2 => crate::fma::Redondeo::Arriba,
                    _ => crate::fma::Redondeo::Cero,
                };
                crate::fma::ffma(a(), b, c, modo)
            }
            0x108 if forma == 1 => {
                let x = f(m.reg((lo >> 32 & 0xFF) as usize));
                match hi >> 10 & 0xF {
                    4 => {
                        let r = 1.0 / x;
                        if r.is_normal() { r.to_bits().wrapping_add_signed(m.inverso_ulp) } else { r.to_bits() }
                    }
                    5 => (1.0 / raiz(x)).to_bits(),
                    8 => raiz(x).to_bits(),
                    _ => return Err(NoSimula::Instruccion(n)),
                }
            }
            0x002 => match forma {
                1 => m.reg((lo >> 32 & 0xFF) as usize),
                4 => (lo >> 32) as u32,
                5 => m.constante(lo),
                _ => return Err(NoSimula::Instruccion(n)),
            },
            // TEX.SCR.B.LZ 2D (0x361): R y G en rd, rd+1; B y A en rd2, rd2+1.
            0x161 if forma == 1 => {
                let Some(mu) = m.muestrear else { return Err(NoSimula::Instruccion(n)) };
                let (u, v) = (f(m.reg(ra)), f(m.reg(ra + 1)));
                let c = mu(m.reg((lo >> 32 & 0xFF) as usize), u, v);
                let rd2 = (hi & 0xFF) as usize;
                if rd == RZ || rd2 == RZ || hi >> 8 & 0xF != 0xF {
                    return Err(NoSimula::Instruccion(n));
                }
                m.r[rd] = c[0].to_bits();
                m.r[rd + 1] = c[1].to_bits();
                m.r[rd2] = c[2].to_bits();
                m.r[rd2 + 1] = c[3].to_bits();
                continue;
            }
            0x14D => return Ok(pasos),
            // ** DL12: KILL (su guarda ya se miro arriba) -- solo la forma de
            // `codifica::kill` --: el hilo acaba y su pixel no queda.
            0x15B if lo & !0xF000 == 0x95B && hi & ((1 << 41) - 1) == 7 << 23 => {
                m.matado = true;
                return Ok(pasos);
            }
            0x118 => continue,
            _ => return Err(NoSimula::Instruccion(n)),
        };
        if rd != RZ {
            m.r[rd] = v;
        }
    }
    Err(NoSimula::SinFin)
}
