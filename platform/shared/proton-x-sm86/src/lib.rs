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
//! # Los bits de control, POR REGLA (E4)
//!
//! Se emiten las instrucciones con lo que leen y escriben ([`planifica`]), y
//! despues se calcula el control de cada una con la tabla de Ampere del juez:
//! la espera justa y, para un MUFU, una de las 6 barreras que espera el
//! primero que lee su resultado. Y para que el cuerpo quepa en 64
//! instrucciones (la puerta del kernel, `juez::MAX_INSTRUCCIONES`, es de 128
//! desde E5, con el pegamento del driver alrededor): los operandos de lo
//! conmutativo se ponen al reves si eso ahorra un MOV, un resultado que es
//! una salida se calcula YA en su registro de salida, y el producto escalar
//! se acumula en su destino.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod planifica;
pub mod simula;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, Programa, Reg};
use bmo_sm86::codifica::{self as c, Fuente, Mufu, RZ};

use planifica::Meta;

/// Los bancos de constantes del ABI de E3.
pub const BANCO_ENTRADAS: u8 = 1;
pub const BANCO_CB: u8 = 3;

/// De donde leen las entradas y el cbuffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abi {
    /// El del banco de E3: entradas en c[1], el cbuffer en c[3].
    Banco,
    /// El de la 3060 (E5): las entradas y las filas del cbuffer que el
    /// programa usa llegan YA en registros -- las carga el pegamento del
    /// driver con LDG, lo que ya corrio en el metal --, y el emisor dice
    /// cuales y donde ([`Emitido::precargas`]).
    Registros,
}

/// Lo que el pegamento tiene que cargar antes del cuerpo (modo
/// [`Abi::Registros`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precarga {
    /// El componente `componente` de la entrada `elemento`, en `reg`.
    Entrada { elemento: u8, componente: u8, reg: u8 },
    /// La fila `fila` del cbuffer (16 bytes), en `reg`..`reg + 3`.
    Fila { fila: u16, reg: u8 },
}

/// Como se cronometra una instruccion (las clases del juez que emite esto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clase {
    /// Acoplada de ALU: MOV, FMNMX.
    Alu,
    /// Acoplada del FMA: FADD, FMUL.
    Fma,
    /// Desacoplada: MUFU (necesita barrera).
    Mufu,
    /// EXIT.
    Nada,
}

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
    /// Los ciclos hasta emitir la ultima (con las esperas calculadas).
    pub ciclos: u32,
    /// Lo que va cargado antes (vacio con [`Abi::Banco`]).
    pub precargas: Vec<Precarga>,
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
    reservados: usize,
    /// Los precargados: no se devuelven (dos registros del Programa pueden
    /// ser la misma entrada o la misma fila).
    fijos: Vec<bool>,
    abi: Abi,
    precargas: Vec<Precarga>,
    maximo: u32,
    /// El registro de salida que le toca a cada valor (su primera Salida).
    salida_de: Vec<Option<u8>>,
    salida_usada: Vec<bool>,
    codigo: Vec<(u64, u64)>,
    metas: Vec<Meta>,
    mufus: usize,
}

/// El registro de una fuente, si lo es (para el planificador).
fn reg_de(f: Fuente) -> Option<u8> {
    match f {
        Fuente::R { r, .. } if r != RZ => Some(r),
        _ => None,
    }
}

impl Emisor<'_> {
    /// Una instruccion (sin control: lo pone el planificador) y su meta.
    fn poner(&mut self, w: (u64, u64), clase: Clase, escribe: Option<u8>, lee: [Option<u8>; 3]) {
        self.codigo.push(w);
        self.metas.push(Meta { clase, escribe, lee, lee_salidas: 0 });
    }

    fn pedir(&mut self) -> Result<u8, NoEmite> {
        let i = self.libres.iter().position(|&l| l).ok_or(NoEmite::Registros)?;
        self.libres[i] = false;
        self.maximo = self.maximo.max(i as u32 + 1);
        Ok(i as u8)
    }

    /// Devolver un registro. Los de salida NO se devuelven nunca.
    fn soltar(&mut self, r: u8) {
        if (r as usize) < self.reservados || self.fijos.get(r as usize) == Some(&true) {
            return;
        }
        if let Some(l) = self.libres.get_mut(r as usize) {
            *l = true;
        }
    }

    /// El valor de un registro del Programa: si nadie lo escribio, es una
    /// constante del modulo (`iniciales`), como inmediato.
    fn valor(&mut self, r: Reg) -> Valor {
        *self.valor[r as usize].get_or_insert(Valor::Imm(self.p.iniciales.get(r as usize).copied().unwrap_or(0.0).to_bits()))
    }

    fn es_reg(&mut self, r: Reg) -> bool {
        matches!(self.valor(r), Valor::Reg(_))
    }

    /// Una fuente cualquiera (registro, inmediato o constante).
    fn fuente(&mut self, r: Reg) -> Fuente {
        match self.valor(r) {
            Valor::Reg(x) => c::r(x),
            Valor::Imm(v) => Fuente::Imm(v),
            Valor::C(b, d) => Fuente::C { banco: b, desp: d },
        }
    }

    /// Una fuente que TIENE que ser registro: si no lo es, un MOV a uno, y
    /// el valor SE QUEDA en el hasta su ultimo uso (una entrada o una fila
    /// del cbuffer que se lee cuatro veces se carga UNA).
    fn registro(&mut self, r: Reg, _paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        match self.valor(r) {
            Valor::Reg(x) => Ok(x),
            _ => {
                let f = self.fuente(r);
                let t = self.pedir()?;
                self.poner(c::mov(t, f, 0), Clase::Alu, Some(t), [None; 3]);
                self.valor[r as usize] = Some(Valor::Reg(t));
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

    /// El registro del resultado: el de su salida si es una y esta libre, o
    /// uno nuevo.
    fn destino(&mut self, d: Reg, i: usize) -> Result<u8, NoEmite> {
        if self.valor[d as usize].is_some() {
            return Err(NoEmite::NoSsa(i));
        }
        let x = match self.salida_de[d as usize] {
            Some(o) if !self.salida_usada[o as usize] => {
                self.salida_usada[o as usize] = true;
                o
            }
            _ => self.pedir()?,
        };
        self.valor[d as usize] = Some(Valor::Reg(x));
        Ok(x)
    }

    /// El registro de una precarga (la misma entrada o fila, el mismo): la
    /// primera vez se pide y se apunta.
    fn precarga(&mut self, pedida: Precarga) -> Result<u8, NoEmite> {
        for &p in &self.precargas {
            match (p, pedida) {
                (Precarga::Entrada { elemento, componente, reg }, Precarga::Entrada { elemento: e2, componente: c2, .. }) if (elemento, componente) == (e2, c2) => return Ok(reg),
                (Precarga::Fila { fila, reg }, Precarga::Fila { fila: f2, .. }) if fila == f2 => return Ok(reg),
                _ => {}
            }
        }
        let n = if matches!(pedida, Precarga::Fila { .. }) { 4 } else { 1 };
        // n seguidos (una fila va con un LDG.128: alineada a 4 registros).
        let paso = if n == 4 { 4 } else { 1 };
        let mut i = self.reservados.div_ceil(paso) * paso;
        while i + n <= self.libres.len() && !(i..i + n).all(|k| self.libres[k]) {
            i += paso;
        }
        if i + n > self.libres.len() {
            return Err(NoEmite::Registros);
        }
        for k in i..i + n {
            self.libres[k] = false;
            self.fijos[k] = true;
        }
        self.maximo = self.maximo.max((i + n) as u32);
        let reg = i as u8;
        self.precargas.push(match pedida {
            Precarga::Entrada { elemento, componente, .. } => Precarga::Entrada { elemento, componente, reg },
            Precarga::Fila { fila, .. } => Precarga::Fila { fila, reg },
        });
        Ok(reg)
    }

    /// `op(x, a, b)` de dos fuentes, con `a` en registro. Si es conmutativa y
    /// `a` no es registro pero `b` si, se ponen al reves (un MOV menos).
    fn dos(&mut self, a: Reg, b: Reg, conmuta: bool, paso: &mut Vec<u8>) -> Result<(u8, Fuente), NoEmite> {
        // Al reves si `b` ya esta en un registro y `a` no; y si ninguno lo
        // esta, va al registro el que vive MAS (se reusara).
        let vive = |e: &Self, r: Reg| e.ultimo.get(r as usize).copied().unwrap_or(0);
        let al_reves = conmuta && !self.es_reg(a) && (self.es_reg(b) || vive(self, b) > vive(self, a));
        let (a, b) = if al_reves { (b, a) } else { (a, b) };
        let ra = self.registro(a, paso)?;
        Ok((ra, self.fuente(b)))
    }
}

/// **Emitir** un [`Programa`] a SASS de SM86, con `registros` para usar
/// (R0..R(registros-1)). Las salidas van a R0..R(4 * salidas - 1).
pub fn emitir(p: &Programa, registros: u32) -> Result<Emitido, NoEmite> {
    emitir_con(p, registros, Abi::Banco)
}

/// **Emitir** con el [`Abi`] que se pida.
pub fn emitir_con(p: &Programa, registros: u32, abi: Abi) -> Result<Emitido, NoEmite> {
    let n = p.iniciales.len();
    let reservados = 4 * p.salidas;
    if reservados as u32 > registros || registros > 255 {
        return Err(NoEmite::Registros);
    }
    // El ultimo uso de cada registro del Programa, y su salida (la primera).
    let mut ultimo = vec![0usize; n];
    let mut salida_de: Vec<Option<u8>> = vec![None; n];
    for (i, op) in p.ops.iter().enumerate() {
        for &r in leidos(op).iter().flatten() {
            if let Some(u) = ultimo.get_mut(r as usize) {
                *u = i;
            }
        }
        if let Op::Salida { s, elemento, componente } = *op {
            if let Some(x) = salida_de.get_mut(s as usize) {
                x.get_or_insert(4 * elemento + (componente & 3));
            }
        }
    }
    let mut libres = vec![true; registros as usize];
    for l in libres.iter_mut().take(reservados) {
        *l = false;
    }
    let mut e = Emisor {
        p,
        valor: vec![None; n],
        ultimo,
        libres,
        reservados,
        fijos: vec![false; registros as usize],
        abi,
        precargas: Vec::new(),
        maximo: reservados as u32,
        salida_de,
        salida_usada: vec![false; reservados],
        codigo: Vec::new(),
        metas: Vec::new(),
        mufus: 0,
    };
    // Con `Abi::Registros` TODO lo precargado se pide ANTES del cuerpo: el
    // pegamento lo carga al empezar, asi que su registro no puede servir de
    // temporal antes. Cada uno se suelta tras su ultimo uso (`fin`).
    let mut fin = vec![None::<usize>; registros as usize];
    if abi == Abi::Registros {
        for op in &p.ops {
            let (base, d, k) = match *op {
                Op::Entrada { d, elemento, componente } => (e.precarga(Precarga::Entrada { elemento, componente, reg: 0 })?, d, 1),
                Op::Constantes { d, fila } => (e.precarga(Precarga::Fila { fila, reg: 0 })?, d, 4),
                _ => continue,
            };
            for j in 0..k {
                let u = e.ultimo.get(d as usize + j).copied().unwrap_or(0);
                let f = &mut fin[base as usize + j];
                *f = Some(f.map_or(u, |x| x.max(u)));
            }
        }
    }
    for (i, op) in p.ops.iter().enumerate() {
        let mut paso: Vec<u8> = Vec::new();
        match *op {
            Op::Entrada { d, elemento, componente } => {
                if e.valor[d as usize].is_some() {
                    return Err(NoEmite::NoSsa(i));
                }
                let v = match e.abi {
                    Abi::Banco => Valor::C(BANCO_ENTRADAS, 16 * elemento as u16 + 4 * (componente as u16 & 3)),
                    Abi::Registros => Valor::Reg(e.precarga(Precarga::Entrada { elemento, componente, reg: 0 })?),
                };
                e.valor[d as usize] = Some(v);
            }
            Op::Constantes { d, fila } => {
                for k in 0..4u16 {
                    let r = d as usize + k as usize;
                    if e.valor.get(r).is_some_and(|v| v.is_some()) {
                        return Err(NoEmite::NoSsa(i));
                    }
                    let v = match e.abi {
                        Abi::Banco => Valor::C(BANCO_CB, 16 * fila + 4 * k),
                        Abi::Registros => Valor::Reg(e.precarga(Precarga::Fila { fila, reg: 0 })? + k as u8),
                    };
                    if let Some(x) = e.valor.get_mut(r) {
                        *x = Some(v);
                    }
                }
            }
            Op::Salida { s, elemento, componente } => {
                let o = 4 * elemento + (componente & 3);
                // Ya calculado en su registro de salida: nada que mover.
                if e.valor(s) != Valor::Reg(o) {
                    let f = e.fuente(s);
                    e.poner(c::mov(o, f, 0), Clase::Alu, Some(o), [reg_de(f), None, None]);
                }
            }
            Op::Mul { d, a, b } | Op::Add { d, a, b } | Op::Min { d, a, b } | Op::Max { d, a, b } => {
                let (ra, fb) = e.dos(a, b, true, &mut paso)?;
                let x = e.destino(d, i)?;
                let lee = [Some(ra), reg_de(fb), None];
                match op {
                    Op::Mul { .. } => e.poner(c::fmul(x, c::r(ra), fb, false, 0), Clase::Fma, Some(x), lee),
                    Op::Add { .. } => e.poner(c::fadd(x, c::r(ra), fb, false, 0), Clase::Fma, Some(x), lee),
                    Op::Min { .. } => e.poner(c::fmnmx(x, c::r(ra), fb, false, 0), Clase::Alu, Some(x), lee),
                    _ => e.poner(c::fmnmx(x, c::r(ra), fb, true, 0), Clase::Alu, Some(x), lee),
                }
            }
            Op::Sub { d, a, b } => {
                let ra = e.registro(a, &mut paso)?;
                let fb = e.negada(b, &mut paso)?;
                let x = e.destino(d, i)?;
                e.poner(c::fadd(x, c::r(ra), fb, false, 0), Clase::Fma, Some(x), [Some(ra), reg_de(fb), None]);
            }
            Op::Mad { d, a, b, c: cc } => {
                // a * b redondeado, y despues + c redondeado: sin fundir.
                let (ra, fb) = e.dos(a, b, true, &mut paso)?;
                let t = e.pedir()?;
                paso.push(t);
                e.poner(c::fmul(t, c::r(ra), fb, false, 0), Clase::Fma, Some(t), [Some(ra), reg_de(fb), None]);
                let fc = e.fuente(cc);
                let x = e.destino(d, i)?;
                e.poner(c::fadd(x, c::r(t), fc, false, 0), Clase::Fma, Some(x), [Some(t), reg_de(fc), None]);
            }
            Op::Dot { d, n, a, b } => {
                // De izquierda a derecha, sin fundir, acumulando en el destino.
                let x = e.destino(d, i)?;
                let (r0, f0) = e.dos(a[0], b[0], true, &mut paso)?;
                e.poner(c::fmul(x, c::r(r0), f0, false, 0), Clase::Fma, Some(x), [Some(r0), reg_de(f0), None]);
                let u = e.pedir()?;
                paso.push(u);
                for j in 1..n as usize {
                    let (rj, fj) = e.dos(a[j], b[j], true, &mut paso)?;
                    e.poner(c::fmul(u, c::r(rj), fj, false, 0), Clase::Fma, Some(u), [Some(rj), reg_de(fj), None]);
                    e.poner(c::fadd(x, c::r(x), c::r(u), false, 0), Clase::Fma, Some(x), [Some(x), Some(u), None]);
                }
            }
            Op::Rsqrt { d, a } | Op::Sqrt { d, a } => {
                let ra = e.registro(a, &mut paso)?;
                let x = e.destino(d, i)?;
                let f = if matches!(op, Op::Rsqrt { .. }) { Mufu::Rsq } else { Mufu::Sqrt };
                e.poner(c::mufu(x, f, ra, 0), Clase::Mufu, Some(x), [Some(ra), None, None]);
                e.mufus += 1;
            }
            Op::Saturate { d, a } | Op::Abs { d, a } => {
                let ra = e.registro(a, &mut paso)?;
                let x = e.destino(d, i)?;
                let (fa, sat) = if matches!(op, Op::Abs { .. }) { (c::abs(ra), false) } else { (c::r(ra), true) };
                e.poner(c::fadd(x, fa, c::neg(RZ), sat, 0), Clase::Fma, Some(x), [Some(ra), None, None]);
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
        for (r, f) in fin.iter().enumerate() {
            if *f == Some(i) {
                e.libres[r] = true;
                e.fijos[r] = false;
            }
        }
    }
    // El EXIT LEE las salidas (R0..): lo que las escribe tiene que haber
    // llegado (metal 28-09: el color de pixel salia a medio escribir).
    e.poner(c::exit(0), Clase::Nada, None, [None; 3]);
    if let Some(m) = e.metas.last_mut() {
        m.lee_salidas = reservados as u8;
    }
    // El control, por regla.
    let (controles, ciclos) = planifica::planificar(&e.metas);
    let codigo = e.codigo.iter().zip(&controles).map(|(&(lo, hi), &k)| (lo, (hi & ((1 << 41) - 1)) | k << 41)).collect();
    Ok(Emitido { codigo, registros: e.maximo, mufus: e.mufus, ciclos, precargas: e.precargas })
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
