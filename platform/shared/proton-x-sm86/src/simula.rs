//! **La 3060 de mentira**: ejecuta en el anfitrion las palabras de SASS que
//! emite [`crate::emitir`], leyendo los MISMOS bits que lee la tarjeta (los de
//! `bmo_sm86::codifica`). Es el oraculo de E3: si el simulador y la casa
//! (`Programa::correr`) dan los mismos bits, el emisor tradujo bien.
//!
//! Lo que sabe ejecutar es lo que el emisor pone -- FADD, FMUL, FMNMX, MUFU,
//! MOV, EXIT y NOP, con registros, inmediatos y constantes, `-`, `|x|` y
//! `.SAT` --; cualquier otra palabra es [`NoSimula::Instruccion`], nunca un
//! "seguramente".
//!
//! # Lo que es MODELO y no hardware, dicho
//!
//! - `MUFU.RSQ`/`RCP`/`SQRT` se hacen como la casa (`1 / raiz(x)`, `1 / x`,
//!   `raiz(x)`): la 3060 da una APROXIMACION de un ULP o dos. Cuanto se
//!   separa, y si mueve un pixel del cubo, lo dice el metal (E5), no esto.
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
    /// Una instruccion con predicado (el emisor no los pone).
    Predicado(usize),
    /// El programa se acabo sin EXIT.
    SinExit,
}

/// La maquina: 256 registros (los bits de cada `f32`) y los bancos de
/// constantes (`c[banco][desp]`); lo que no esta se lee como 0.
pub struct Maquina<'a> {
    pub r: [u32; 256],
    pub bancos: [&'a [u8]; 8],
}

impl<'a> Maquina<'a> {
    pub fn nueva(bancos: [&'a [u8]; 8]) -> Self {
        Maquina { r: [0; 256], bancos }
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

/// **Correr** `codigo` sobre `m` hasta su EXIT. Devuelve cuantas instrucciones
/// se ejecutaron.
pub fn correr(codigo: &[(u64, u64)], m: &mut Maquina) -> Result<usize, NoSimula> {
    for (n, &(lo, hi)) in codigo.iter().enumerate() {
        if lo >> 12 & 0xF != 7 {
            return Err(NoSimula::Predicado(n));
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
        let v: u32 = match op {
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
            0x108 if forma == 1 => {
                let x = f(m.reg((lo >> 32 & 0xFF) as usize));
                match hi >> 10 & 0xF {
                    4 => (1.0 / x).to_bits(),
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
            0x14D => return Ok(n + 1),
            0x118 => continue,
            _ => return Err(NoSimula::Instruccion(n)),
        };
        if rd != RZ {
            m.r[rd] = v;
        }
    }
    Err(NoSimula::SinExit)
}
