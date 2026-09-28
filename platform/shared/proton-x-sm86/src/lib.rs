//! # bmo-proton-x-sm86 -- el emisor de PROTON-X para la RTX 3060
//!
//! generacion: nieto -- traduce un Programa de la casa a SASS; no toca la tarjeta
//! capa: puro -- ni un `unsafe`, ni un aparato: se prueba entero en el anfitrion
//!
//! [carril]  VERDE     una pasada por programa
//!
//! E3 de `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`, con una decision del
//! propietario (28-09): la entrada NO es SPIR-V sino el [`Programa`] de la
//! casa -- el que ya sale de los DXIL de dxc y de los SM5 de FXC, y el que la
//! CPU ya corre (el interprete, y `nativo` a x86-64) --. Un emisor sirve a
//! BMOX-12 y a lo que venga, y el interprete de la casa es su juez.
//!
//! ```text
//!    Programa (escalar, sin saltos)  -->  emitir  -->  SASS de SM86 en linea
//!                                                      recta, con su EXIT
//!    el simulador (`simula`) corre ese SASS en el anfitrion; tiene que dar
//!    los MISMOS bits que `Programa::correr`
//! ```
//!
//! # El ABI de E3 (`V0`), el del banco
//!
//! ```text
//!    entrada e, componente k    c[1][16 e + 4 k]
//!    el cbuffer b0, fila f      c[3][16 f + 4 k]
//!    salida e, componente k     el registro R(4 e + k) al EXIT
//! ```
//!
//! Es el de la prueba, no el de la tuberia: en el metal (E5) las entradas y
//! salidas son atributos (ALD/AST/IPA) y el cbuffer el banco que diga el
//! QMD. Cambia la PUERTA, no las cuentas.
//!
//! # Las cuentas, como la casa
//!
//! - `Mad` y `Dot` SIN fundir: FMUL y despues FADD (como `Programa::correr`
//!   y como el juez de las huellas); ni una FFMA.
//! - `Saturate` es `FADD.SAT d, a, -0`; `Abs`, `FADD d, |a|, -0`: sumar -0
//!   no cambia ningun valor (tampoco +0 ni -0).
//! - `Rsqrt` y `Sqrt` son `MUFU`: la 3060 APROXIMA (ver `simula`).
//! - `Div` se RECHAZA hoy: la de la 3060 (MUFU.RCP y FMUL) no es la division
//!   exacta de la casa, y ningun sombreador de los que corren la usa.
//!
//! # Los bits de control, CONSERVADORES (E3; las reglas son E4)
//!
//! Toda instruccion con el control de ALU del driver (6 ciclos); un MUFU
//! enciende la barrera 0 y la SIGUIENTE instruccion la espera.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod simula;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, Programa, Reg};
use bmo_sm86::codifica::{self as c, Fuente, Mufu, ALU, RZ};

/// Los bancos de constantes del ABI de E3.
pub const BANCO_ENTRADAS: u8 = 1;
pub const BANCO_CB: u8 = 3;

/// Control de un MUFU: 1 ciclo, el bit 4, ENCIENDE la barrera 0 al escribir.
const MUFU_CONTROL: u64 = 1 | 1 << 4 | 7 << 8;
/// El de la instruccion que va detras: espera la barrera 0.
const ESPERA_0: u64 = ALU | 1 << 11;

/// Por que un programa no se emite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEmite {
    /// Una operacion que el emisor no sabe poner todavia (su indice).
    Operacion(usize),
    /// No caben los registros vivos en `registros`.
    Registros,
    /// Un registro del Programa escrito dos veces (el emisor pide SSA).
    NoSsa(usize),
}

/// Lo emitido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emitido {
    pub codigo: Vec<(u64, u64)>,
    /// Cuantos registros usa (R0..R(n-1)): lo que va en la SPH o el QMD.
    pub registros: u32,
    pub mufus: usize,
}

/// Donde vive un valor del Programa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Valor {
    Reg(u8),
    Imm(u32),
    C(u8, u16),
}

struct Emisor<'a> {
    p: &'a Programa,
    valor: Vec<Option<Valor>>,
    ultimo: Vec<usize>,
    libres: Vec<bool>,
    maximo: u32,
    codigo: Vec<(u64, u64)>,
    control: u64,
    mufus: usize,
}

impl Emisor<'_> {
    fn poner(&mut self, w: (u64, u64)) {
        self.codigo.push(w);
    }

    /// El control de la siguiente instruccion (y lo gasta).
    fn ctl(&mut self) -> u64 {
        core::mem::replace(&mut self.control, ALU)
    }

    fn pedir(&mut self) -> Result<u8, NoEmite> {
        let i = self.libres.iter().position(|&l| l).ok_or(NoEmite::Registros)?;
        self.libres[i] = false;
        self.maximo = self.maximo.max(i as u32 + 1);
        Ok(i as u8)
    }

    fn soltar(&mut self, r: u8) {
        if let Some(l) = self.libres.get_mut(r as usize) {
            *l = true;
        }
    }

    /// El valor de un registro del Programa: si nadie lo escribio, es una
    /// constante del modulo (`iniciales`), como inmediato.
    fn valor(&mut self, r: Reg) -> Valor {
        *self.valor[r as usize].get_or_insert(Valor::Imm(self.p.iniciales.get(r as usize).copied().unwrap_or(0.0).to_bits()))
    }

    /// Una fuente cualquiera (registro, inmediato o constante).
    fn fuente(&mut self, r: Reg) -> Fuente {
        match self.valor(r) {
            Valor::Reg(x) => c::r(x),
            Valor::Imm(v) => Fuente::Imm(v),
            Valor::C(b, d) => Fuente::C { banco: b, desp: d },
        }
    }

    /// Una fuente que TIENE que ser registro: si no lo es, un MOV a uno de
    /// paso (que se devuelve para soltarlo despues).
    fn registro(&mut self, r: Reg, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        match self.valor(r) {
            Valor::Reg(x) => Ok(x),
            _ => {
                let f = self.fuente(r);
                let t = self.pedir()?;
                let k = self.ctl();
                self.poner(c::mov(t, f, k));
                paso.push(t);
                Ok(t)
            }
        }
    }

    /// Una fuente negada: registro con su bit, inmediato con su signo, y una
    /// constante pasa antes por un registro.
    fn negada(&mut self, r: Reg, paso: &mut Vec<u8>) -> Result<Fuente, NoEmite> {
        Ok(match self.valor(r) {
            Valor::Imm(v) => Fuente::Imm(v ^ 0x8000_0000),
            _ => c::neg(self.registro(r, paso)?),
        })
    }

    fn destino(&mut self, d: Reg, i: usize) -> Result<u8, NoEmite> {
        if self.valor[d as usize].is_some() {
            return Err(NoEmite::NoSsa(i));
        }
        let x = self.pedir()?;
        self.valor[d as usize] = Some(Valor::Reg(x));
        Ok(x)
    }
}

/// **Emitir** un [`Programa`] a SASS de SM86, con `registros` para usar
/// (R0..R(registros-1)). Las salidas van a R0..R(4 * salidas - 1).
pub fn emitir(p: &Programa, registros: u32) -> Result<Emitido, NoEmite> {
    let n = p.iniciales.len();
    let reservados = 4 * p.salidas;
    if reservados as u32 > registros || registros > 255 {
        return Err(NoEmite::Registros);
    }
    // El ultimo uso de cada registro del Programa (para devolverlo despues).
    let mut ultimo = vec![0usize; n];
    for (i, op) in p.ops.iter().enumerate() {
        for &r in leidos(op).iter().flatten() {
            if let Some(u) = ultimo.get_mut(r as usize) {
                *u = i;
            }
        }
    }
    let mut libres = vec![true; registros as usize];
    for l in libres.iter_mut().take(reservados) {
        *l = false;
    }
    let mut e = Emisor { p, valor: vec![None; n], ultimo, libres, maximo: reservados as u32, codigo: Vec::new(), control: ALU, mufus: 0 };
    for (i, op) in p.ops.iter().enumerate() {
        let mut paso: Vec<u8> = Vec::new();
        match *op {
            Op::Entrada { d, elemento, componente } => {
                if e.valor[d as usize].is_some() {
                    return Err(NoEmite::NoSsa(i));
                }
                e.valor[d as usize] = Some(Valor::C(BANCO_ENTRADAS, 16 * elemento as u16 + 4 * (componente as u16 & 3)));
            }
            Op::Constantes { d, fila } => {
                for k in 0..4u16 {
                    let r = d as usize + k as usize;
                    if e.valor.get(r).is_some_and(|v| v.is_some()) {
                        return Err(NoEmite::NoSsa(i));
                    }
                    if let Some(v) = e.valor.get_mut(r) {
                        *v = Some(Valor::C(BANCO_CB, 16 * fila + 4 * k));
                    }
                }
            }
            Op::Salida { s, elemento, componente } => {
                let f = e.fuente(s);
                let k = e.ctl();
                e.poner(c::mov(4 * elemento + (componente & 3), f, k));
            }
            Op::Mul { d, a, b } | Op::Add { d, a, b } | Op::Sub { d, a, b } | Op::Min { d, a, b } | Op::Max { d, a, b } => {
                let ra = e.registro(a, &mut paso)?;
                let fb = if matches!(op, Op::Sub { .. }) { e.negada(b, &mut paso)? } else { e.fuente(b) };
                let x = e.destino(d, i)?;
                let k = e.ctl();
                let w = match op {
                    Op::Mul { .. } => c::fmul(x, c::r(ra), fb, false, k),
                    Op::Min { .. } => c::fmnmx(x, c::r(ra), fb, false, k),
                    Op::Max { .. } => c::fmnmx(x, c::r(ra), fb, true, k),
                    _ => c::fadd(x, c::r(ra), fb, false, k),
                };
                e.poner(w);
            }
            Op::Mad { d, a, b, c: cc } => {
                // a * b redondeado, y despues + c redondeado: sin fundir.
                let ra = e.registro(a, &mut paso)?;
                let fb = e.fuente(b);
                let t = e.pedir()?;
                paso.push(t);
                let k = e.ctl();
                e.poner(c::fmul(t, c::r(ra), fb, false, k));
                let fc = e.fuente(cc);
                let x = e.destino(d, i)?;
                let k = e.ctl();
                e.poner(c::fadd(x, c::r(t), fc, false, k));
            }
            Op::Dot { d, n, a, b } => {
                // De izquierda a derecha, sin fundir: t = a0 b0; t = t + ak bk.
                let t = e.pedir()?;
                paso.push(t);
                let r0 = e.registro(a[0], &mut paso)?;
                let f0 = e.fuente(b[0]);
                let k = e.ctl();
                e.poner(c::fmul(t, c::r(r0), f0, false, k));
                let u = e.pedir()?;
                paso.push(u);
                for j in 1..n as usize {
                    let rj = e.registro(a[j], &mut paso)?;
                    let fj = e.fuente(b[j]);
                    let k = e.ctl();
                    e.poner(c::fmul(u, c::r(rj), fj, false, k));
                    let k = e.ctl();
                    e.poner(c::fadd(t, c::r(t), c::r(u), false, k));
                }
                let x = e.destino(d, i)?;
                let k = e.ctl();
                e.poner(c::mov(x, c::r(t), k));
            }
            Op::Rsqrt { d, a } | Op::Sqrt { d, a } => {
                let ra = e.registro(a, &mut paso)?;
                let x = e.destino(d, i)?;
                let f = if matches!(op, Op::Rsqrt { .. }) { Mufu::Rsq } else { Mufu::Sqrt };
                e.ctl();
                e.poner(c::mufu(x, f, ra, MUFU_CONTROL));
                e.control = ESPERA_0;
                e.mufus += 1;
            }
            Op::Saturate { d, a } | Op::Abs { d, a } => {
                let ra = e.registro(a, &mut paso)?;
                let x = e.destino(d, i)?;
                let k = e.ctl();
                let (fa, sat) = if matches!(op, Op::Abs { .. }) { (c::abs(ra), false) } else { (c::r(ra), true) };
                e.poner(c::fadd(x, fa, c::neg(RZ), sat, k));
            }
            Op::Div { .. } => return Err(NoEmite::Operacion(i)),
        }
        // Lo de paso, y lo que ya nadie lee, se devuelve.
        for t in paso {
            e.soltar(t);
        }
        for &r in leidos(op).iter().flatten() {
            if e.ultimo.get(r as usize) == Some(&i) {
                if let Some(Some(Valor::Reg(x))) = e.valor.get(r as usize).copied() {
                    e.soltar(x);
                }
            }
        }
    }
    let k = e.ctl();
    e.poner(c::exit(k));
    Ok(Emitido { codigo: e.codigo, registros: e.maximo, mufus: e.mufus })
}

/// Los registros del Programa que lee una operacion.
fn leidos(op: &Op) -> [Option<Reg>; 8] {
    let mut v = [None; 8];
    match *op {
        Op::Salida { s, .. } => v[0] = Some(s),
        Op::Mul { a, b, .. } | Op::Add { a, b, .. } | Op::Sub { a, b, .. } | Op::Div { a, b, .. } | Op::Min { a, b, .. } | Op::Max { a, b, .. } => {
            v[0] = Some(a);
            v[1] = Some(b);
        }
        Op::Mad { a, b, c, .. } => {
            v[0] = Some(a);
            v[1] = Some(b);
            v[2] = Some(c);
        }
        Op::Dot { n, a, b, .. } => {
            for j in 0..n as usize {
                v[2 * j] = Some(a[j]);
                v[2 * j + 1] = Some(b[j]);
            }
        }
        Op::Rsqrt { a, .. } | Op::Sqrt { a, .. } | Op::Saturate { a, .. } | Op::Abs { a, .. } => v[0] = Some(a),
        Op::Entrada { .. } | Op::Constantes { .. } => {}
    }
    v
}

#[cfg(test)]
mod pruebas;
