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
//!    Programa (escalar)  -->  emitir  -->  SASS de SM86, con su EXIT; desde
//!                                          E6 (02-10) con `si`, bucles y
//!                                          sus saltos (`saltos.rs`)
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
//! - La division de ENTEROS (E6d) si es exacta: la cuenta de `ptxas` con el
//!   inverso, sin guardas; una sola para `a / b` y `a % b`, y entre una
//!   constante, una multiplicacion (`division.rs`).
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

mod division;
/// E6 (02-10): lo que se mira antes de emitir un programa que salta.
mod saltos;
/// P3b4a: un PSO de la casa, listo y juzgado para la 3060.
pub mod pso;
/// P3b4c: el lote a la PUERTA de la 3060 (la receta VRN2 que manda la app).
pub mod puerta;
/// P3b4c.8 T0: el muestreador y la textura de la casa en el TSC y el TIC de
/// la 3060 (29-09).
pub mod muestreo;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Comparacion, Conversion, Op, OpEntera, Programa, Reg};
use bmo_sm86::codifica::{self as c, Cmp, Fuente, Mufu, PT, RZ};

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
    /// P3b4c.8 T2b: el ASA de la textura `textura` (tN) con el muestreador
    /// `muestreador` (sM), en `reg`. La pone el pegamento del KERNEL: la
    /// pareja (tN, sM) sera la textura k de la receta.
    Asa { textura: u8, muestreador: u8, reg: u8 },
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
    /// El TEX: desacoplado, escribe CUATRO registros con una barrera.
    Tex,
    /// E6d: IMAD.HI (la mitad alta de un producto: la tabla "ancha" de NAK).
    Ancha,
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
    /// Un registro del Programa escrito dos veces donde no puede serlo (una
    /// Entrada o una fila del cbuffer que ademas se reescribe).
    NoSsa(usize),
    /// E6: un `si` o un bucle mal cerrado (el indice de `Programa::forma`).
    Forma(usize),
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
    /// E6: las variables (un registro de la 3060 todo el programa).
    variable: Vec<bool>,
    /// E6: cuantos `si` y bucles hay abiertos donde se emite.
    hondo: usize,
    /// E6: la Compara fundida cuyo resultado esta en P0 (y no en un registro).
    p0: Option<Reg>,
    /// E6: los `si` y bucles abiertos, con sus saltos por parchear.
    abiertos: Vec<Abierto>,
}

/// Un `si` o un bucle abierto mientras se emite.
enum Abierto {
    /// El salto condicional del Si y, tras su SiNo, el salto al final.
    Si { salto: usize, sino: Option<usize> },
    /// Donde empieza el cuerpo, y los saltos que salen (Romper).
    Bucle { cabeza: usize, salidas: Vec<usize> },
}

/// La comparacion de la 3060 para una del Programa: con floats, las
/// ordenadas, y `Distinto` desordenada (NEU: cierta con un NaN, como `ne`).
fn cmp_de(como: Comparacion, entero: bool) -> Cmp {
    match como {
        Comparacion::MenorSinSigno => Cmp::Lt,
        Comparacion::MenorIgualSinSigno => Cmp::Le,
        Comparacion::MayorSinSigno => Cmp::Gt,
        Comparacion::MayorIgualSinSigno => Cmp::Ge,
        Comparacion::Menor => Cmp::Lt,
        Comparacion::MenorIgual => Cmp::Le,
        Comparacion::Mayor => Cmp::Gt,
        Comparacion::MayorIgual => Cmp::Ge,
        Comparacion::Igual => Cmp::Eq,
        Comparacion::Distinto if entero => Cmp::Ne,
        Comparacion::Distinto => Cmp::Neu,
    }
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
        self.poner_meta(w, Meta::de(clase, escribe, lee));
    }

    fn poner_meta(&mut self, w: (u64, u64), m: Meta) {
        self.codigo.push(w);
        self.metas.push(m);
    }

    /// Un BRA por parchear (el destino, cuando se sepa): su indice.
    fn saltar(&mut self, guarda: u8) -> usize {
        let k = self.codigo.len();
        let lee_p = (guarda & 7 != PT).then_some((guarda & 7, true));
        self.poner_meta(c::bra(guarda, 0, 0), Meta { lee_p, salto: true, ..Meta::de(Clase::Nada, None, [None; 3]) });
        k
    }

    /// El BRA `k` salta a la instruccion `destino`.
    fn parchear(&mut self, k: usize, destino: usize) {
        let guarda = (self.codigo[k].0 >> 12 & 0xF) as u8;
        self.codigo[k] = c::bra(guarda, (destino as i64 - k as i64 - 1) * 16, 0);
    }

    /// El salto `k` sale del bucle mas interno (se parchea en su FinBucle).
    fn salida_de_bucle(&mut self, k: usize) {
        if let Some(Abierto::Bucle { salidas, .. }) = self.abiertos.iter_mut().rev().find(|x| matches!(x, Abierto::Bucle { .. })) {
            salidas.push(k);
        }
    }

    /// P0 = los bits de `c` no son 0 -- o ya lo es, si `c` es la Compara
    /// fundida de justo antes.
    fn condicion(&mut self, cond: Reg, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        if self.p0.take() == Some(cond) {
            return Ok(());
        }
        let rc = self.registro(cond, paso)?;
        let w = c::isetp(0, Cmp::Ne, rc, c::r(RZ), false, 0);
        self.poner_meta(w, Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, None, [Some(rc), None, None]) });
        Ok(())
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
    ///
    /// E6: DENTRO de un `si` o de un bucle no se queda: es de paso. El MOV
    /// corre solo por ese camino, y quien lea despues por otro no lo tendria.
    fn registro(&mut self, r: Reg, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        match self.valor(r) {
            Valor::Reg(x) => Ok(x),
            _ => {
                let f = self.fuente(r);
                let t = self.pedir()?;
                self.poner(c::mov(t, f, 0), Clase::Alu, Some(t), [None; 3]);
                if self.hondo == 0 {
                    self.valor[r as usize] = Some(Valor::Reg(t));
                } else {
                    paso.push(t);
                }
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
        // Una variable: su registro de siempre (puesto al empezar).
        if self.variable[d as usize] {
            if let Some(Valor::Reg(x)) = self.valor[d as usize] {
                return Ok(x);
            }
        }
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
                (Precarga::Asa { textura, muestreador, reg }, Precarga::Asa { textura: t2, muestreador: s2, .. }) if (textura, muestreador) == (t2, s2) => return Ok(reg),
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
            Precarga::Asa { textura, muestreador, .. } => Precarga::Asa { textura, muestreador, reg },
        });
        Ok(reg)
    }

    /// `op(x, a, b)` de dos fuentes, con `a` en registro. Si es conmutativa y
    /// `a` no es registro pero `b` si, se ponen al reves (un MOV menos).
    /// `n` registros seguidos y alineados a `n` (las coordenadas de un TEX,
    /// su resultado). Con `para_siempre` no se devuelven nunca: las fuentes
    /// de una desacoplada no se pisan mientras la 3060 aun pueda leerlas.
    fn bloque(&mut self, n: usize, para_siempre: bool) -> Result<u8, NoEmite> {
        let mut i = self.reservados.div_ceil(n) * n;
        while i + n <= self.libres.len() && !(i..i + n).all(|k| self.libres[k]) {
            i += n;
        }
        if i + n > self.libres.len() {
            return Err(NoEmite::Registros);
        }
        for k in i..i + n {
            self.libres[k] = false;
            self.fijos[k] = para_siempre;
        }
        self.maximo = self.maximo.max((i + n) as u32);
        Ok(i as u8)
    }

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
    p.forma().map_err(|f| NoEmite::Forma(f.0))?;
    // El ultimo uso de cada registro (con los bucles), sus variables...
    let an = saltos::analizar(p);
    // ...y la salida de cada valor: la calcula YA en su registro de salida
    // si es la UNICA Salida de ese componente, fuera de todo `si` y bucle,
    // y el valor no es una variable (E6: si no, otro camino lo pisaria).
    let mut salida_de: Vec<Option<u8>> = vec![None; n];
    let mut veces = vec![0u32; reservados];
    for op in &p.ops {
        if let Op::Salida { elemento, componente, .. } = *op {
            if let Some(v) = veces.get_mut(4 * elemento as usize + (componente as usize & 3)) {
                *v += 1;
            }
        }
    }
    for (i, op) in p.ops.iter().enumerate() {
        if let Op::Salida { s, elemento, componente } = *op {
            let o = 4 * elemento + (componente & 3);
            if an.hondo[i] == 0 && veces.get(o as usize) == Some(&1) && !an.variable.get(s as usize).copied().unwrap_or(true) {
                if let Some(x) = salida_de.get_mut(s as usize) {
                    x.get_or_insert(o);
                }
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
        ultimo: an.ultimo.clone(),
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
        variable: an.variable.clone(),
        hondo: 0,
        p0: None,
        abiertos: Vec::new(),
    };
    // Con `Abi::Registros` TODO lo precargado se pide ANTES del cuerpo: el
    // pegamento lo carga al empezar, asi que su registro no puede servir de
    // temporal antes. Cada uno se suelta tras su ultimo uso (`fin`).
    let mut fin = vec![None::<usize>; registros as usize];
    if abi == Abi::Registros {
        for op in &p.ops {
            let (base, d, k) = match *op {
                Op::Entrada { d, elemento, componente } => (e.precarga(Precarga::Entrada { elemento, componente, reg: 0 })?, d, 1),
                Op::Constantes { d, fila, .. } => (e.precarga(Precarga::Fila { fila, reg: 0 })?, d, 4),
                // El asa: fija TODO el programa (la lee un TEX desacoplado).
                Op::Muestra { t, s, .. } => {
                    e.precarga(Precarga::Asa { textura: t, muestreador: s, reg: 0 })?;
                    continue;
                }
                _ => continue,
            };
            for j in 0..k {
                let u = e.ultimo.get(d as usize + j).copied().unwrap_or(0);
                let f = &mut fin[base as usize + j];
                *f = Some(f.map_or(u, |x| x.max(u)));
            }
        }
    }
    // E6: cada variable, su registro, con el valor que tiene en el
    // interprete antes de que nada la escriba.
    for r in 0..n {
        if an.variable[r] {
            let x = e.pedir()?;
            let bits = p.iniciales[r].to_bits();
            e.poner(c::mov(x, Fuente::Imm(bits), 0), Clase::Alu, Some(x), [None; 3]);
            e.valor[r] = Some(Valor::Reg(x));
        }
    }
    // E6d: las divisiones que ya hizo su pareja.
    let mut hecha = vec![false; p.ops.len()];
    for (i, op) in p.ops.iter().enumerate() {
        let mut paso: Vec<u8> = Vec::new();
        e.hondo = an.hondo[i];
        match *op {
            // ** E6: comparar, elegir, copiar, sumar enteros y saltar.
            Op::Compara { d, a, b, como, entero } => {
                let ra = e.registro(a, &mut paso)?;
                let fb = e.fuente(b);
                let cmp = cmp_de(como, entero);
                let sin_signo = matches!(como, Comparacion::MenorSinSigno | Comparacion::MenorIgualSinSigno | Comparacion::MayorSinSigno | Comparacion::MayorIgualSinSigno);
                let w = if entero { c::isetp(0, cmp, ra, fb, sin_signo, 0) } else { c::fsetp(0, cmp, c::r(ra), fb, 0) };
                e.poner_meta(w, Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, None, [Some(ra), reg_de(fb), None]) });
                if an.fundible[i] {
                    e.p0 = Some(d);
                } else {
                    // 0xFFFFFFFF si P0, 0 si no: SEL d, RZ, -1, !P0.
                    let x = e.destino(d, i)?;
                    e.poner_meta(c::sel(x, RZ, Fuente::Imm(u32::MAX), 0, true, 0), Meta { lee_p: Some((0, false)), ..Meta::de(Clase::Alu, Some(x), [None; 3]) });
                }
            }
            Op::Elige { d, c: cond, a, b } => {
                e.condicion(cond, &mut paso)?;
                let ra = e.registro(a, &mut paso)?;
                let fb = e.fuente(b);
                let x = e.destino(d, i)?;
                e.poner_meta(c::sel(x, ra, fb, 0, false, 0), Meta { lee_p: Some((0, false)), ..Meta::de(Clase::Alu, Some(x), [Some(ra), reg_de(fb), None]) });
            }
            Op::Copia { d, a } => {
                let f = e.fuente(a);
                let x = e.destino(d, i)?;
                if f != c::r(x) {
                    e.poner(c::mov(x, f, 0), Clase::Alu, Some(x), [reg_de(f), None, None]);
                }
            }
            Op::SumaEntera { d, a, b } => {
                let (ra, fb) = e.dos(a, b, true, &mut paso)?;
                let x = e.destino(d, i)?;
                e.poner(c::iadd3(x, ra, fb, 0), Clase::Alu, Some(x), [Some(ra), reg_de(fb), None]);
            }
            // ** E6c: las de enteros y las conversiones.
            // ** E6d: la division; con su pareja (el resto del mismo par, o
            // el cociente), una cuenta: la pareja ya no hace nada.
            Op::Entera { d, a, b, op } if division::es_division(op) => {
                if !hecha[i] {
                    let j = division::pareja(&p.ops, i);
                    let otro = j.and_then(|j| if let Op::Entera { d, .. } = p.ops[j] { Some(d) } else { None });
                    let (cociente, resto) = if matches!(op, OpEntera::RemU | OpEntera::RemS) { (otro, Some(d)) } else { (Some(d), otro) };
                    e.dividir(a, b, matches!(op, OpEntera::DivS | OpEntera::RemS), cociente, resto, i, &mut paso)?;
                    if let Some(j) = j {
                        hecha[j] = true;
                    }
                }
            }
            Op::Entera { d, a, b, op } => {
                let conmuta = matches!(op, OpEntera::Mul | OpEntera::Y | OpEntera::O | OpEntera::OX | OpEntera::MinS | OpEntera::MaxS | OpEntera::MinU | OpEntera::MaxU);
                let (ra, fb) = e.dos(a, b, conmuta, &mut paso)?;
                // Restar y desplazar: la segunda, negada o con sus 5 bits.
                let fb = match (op, fb) {
                    (OpEntera::Resta, Fuente::Imm(v)) => Fuente::Imm(v.wrapping_neg()),
                    (OpEntera::Resta, Fuente::R { r, .. }) => c::neg(r),
                    (OpEntera::Resta, f) => {
                        let t = e.pedir()?;
                        paso.push(t);
                        e.poner(c::mov(t, f, 0), Clase::Alu, Some(t), [None; 3]);
                        c::neg(t)
                    }
                    (OpEntera::Shl | OpEntera::ShrL | OpEntera::ShrA, Fuente::Imm(v)) => Fuente::Imm(v & 31),
                    // Una cuenta en registro (o en c[][]): `& 31` antes (SHF
                    // no lo hace: con 32 o mas da 0).
                    (OpEntera::Shl | OpEntera::ShrL | OpEntera::ShrA, f) => {
                        let t = e.pedir()?;
                        paso.push(t);
                        let r = match f {
                            Fuente::R { r, .. } => r,
                            _ => {
                                e.poner(c::mov(t, f, 0), Clase::Alu, Some(t), [None; 3]);
                                t
                            }
                        };
                        e.poner(c::lop3(t, r, Fuente::Imm(31), c::Y, 0), Clase::Alu, Some(t), [Some(r), None, None]);
                        c::r(t)
                    }
                    (_, f) => f,
                };
                let x = e.destino(d, i)?;
                let lee = [Some(ra), reg_de(fb), None];
                let (w, clase) = match op {
                    OpEntera::Resta => (c::iadd3(x, ra, fb, 0), Clase::Alu),
                    OpEntera::Mul => (c::imad(x, ra, fb, RZ, 0), Clase::Fma),
                    OpEntera::Shl => (c::shl(x, ra, fb, 0), Clase::Alu),
                    OpEntera::ShrL => (c::shr(x, ra, fb, false, 0), Clase::Alu),
                    OpEntera::ShrA => (c::shr(x, ra, fb, true, 0), Clase::Alu),
                    OpEntera::Y => (c::lop3(x, ra, fb, c::Y, 0), Clase::Alu),
                    OpEntera::O => (c::lop3(x, ra, fb, c::O, 0), Clase::Alu),
                    OpEntera::OX => (c::lop3(x, ra, fb, c::OX, 0), Clase::Alu),
                    OpEntera::MinS => (c::imnmx(x, ra, fb, false, true, 0), Clase::Alu),
                    OpEntera::MaxS => (c::imnmx(x, ra, fb, true, true, 0), Clase::Alu),
                    OpEntera::MinU => (c::imnmx(x, ra, fb, false, false, 0), Clase::Alu),
                    OpEntera::MaxU => (c::imnmx(x, ra, fb, true, false, 0), Clase::Alu),
                    // La division va por `dividir`, arriba.
                    OpEntera::DivU | OpEntera::RemU | OpEntera::DivS | OpEntera::RemS => return Err(NoEmite::Operacion(i)),
                };
                e.poner(w, clase, Some(x), lee);
            }
            Op::Convierte { d, a, como } => {
                let ra = e.registro(a, &mut paso)?;
                let x = e.destino(d, i)?;
                let w = match como {
                    Conversion::EnteroAFloat => c::i2f(x, ra, true, 0),
                    Conversion::SinSignoAFloat => c::i2f(x, ra, false, 0),
                    Conversion::FloatAEntero => c::f2i(x, ra, true, 0),
                    Conversion::FloatASinSigno => c::f2i(x, ra, false, 0),
                };
                // Desacopladas, como MUFU: con barrera.
                e.poner(w, Clase::Mufu, Some(x), [Some(ra), None, None]);
                e.mufus += 1;
            }
            Op::Si { c: cond } => {
                e.condicion(cond, &mut paso)?;
                let salto = e.saltar(8);
                e.abiertos.push(Abierto::Si { salto, sino: None });
            }
            Op::SiNo => {
                let k = e.saltar(PT);
                let destino = e.codigo.len();
                if let Some(Abierto::Si { salto, sino }) = e.abiertos.last_mut() {
                    *sino = Some(k);
                    let s = *salto;
                    e.parchear(s, destino);
                }
            }
            Op::FinSi => {
                let destino = e.codigo.len();
                if let Some(Abierto::Si { salto, sino }) = e.abiertos.pop() {
                    e.parchear(sino.unwrap_or(salto), destino);
                }
            }
            Op::Bucle => {
                let cabeza = e.codigo.len();
                e.abiertos.push(Abierto::Bucle { cabeza, salidas: Vec::new() });
            }
            Op::RomperSi { c: cond, si_cero } => {
                e.condicion(cond, &mut paso)?;
                let k = e.saltar(if si_cero { 8 } else { 0 });
                e.salida_de_bucle(k);
            }
            Op::Romper => {
                let k = e.saltar(PT);
                e.salida_de_bucle(k);
            }
            // A la cabeza del bucle mas interno: ya se sabe donde esta.
            Op::Continuar => {
                let k = e.saltar(PT);
                if let Some(cabeza) = e.abiertos.iter().rev().find_map(|x| if let Abierto::Bucle { cabeza, .. } = x { Some(*cabeza) } else { None }) {
                    e.parchear(k, cabeza);
                }
            }
            Op::FinBucle => {
                let k = e.saltar(PT);
                if let Some(Abierto::Bucle { cabeza, salidas }) = e.abiertos.pop() {
                    e.parchear(k, cabeza);
                    let destino = e.codigo.len();
                    for s in salidas {
                        e.parchear(s, destino);
                    }
                }
            }
            // ** P3b4c.8 T2b: el muestreo, un TEX (2D, nivel 0) con el asa
            // que pone el KERNEL. Solo con el ABI de registros (el del
            // pegamento); con el del banco, no hay asa: se dice.
            // D4.4: sin mirar `g`: el TEX de la 3060 saca la mip de sus
            // derivadas el solo (la puerta mira que de lo mismo, `preparar`).
            Op::Muestra { d, t, s, u, v, .. } => {
                if e.abi != Abi::Registros {
                    return Err(NoEmite::Operacion(i));
                }
                if (0..4).any(|k| e.valor.get(d as usize + k).is_none_or(|x| x.is_some())) {
                    return Err(NoEmite::NoSsa(i));
                }
                let asa = e.precarga(Precarga::Asa { textura: t, muestreador: s, reg: 0 })?;
                // Las coordenadas, en un par alineado que no se devuelve.
                let par = e.bloque(2, true)?;
                for (k, r) in [u, v].into_iter().enumerate() {
                    let f = e.fuente(r);
                    e.poner(c::mov(par + k as u8, f, 0), Clase::Alu, Some(par + k as u8), [reg_de(f), None, None]);
                }
                // Los cuatro canales, seguidos y alineados; cada uno se
                // devuelve tras su ultimo uso, como cualquier valor.
                let q = e.bloque(4, false)?;
                e.poner(c::tex(q, par, asa, 0), Clase::Tex, Some(q), [Some(par), Some(par + 1), Some(asa)]);
                if let Some(m) = e.metas.last_mut() {
                    m.escribe_n = 4;
                }
                for k in 0..4u8 {
                    e.valor[d as usize + k as usize] = Some(Valor::Reg(q + k));
                }
            }
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
            Op::Constantes { d, fila, .. } => {
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
                // ** Min y Max NO se ponen al reves (E6, 02-10, lo encontro la
                // prueba al azar): con un cero de cada signo dan el PRIMERO
                // (`max(-0, +0)` es -0 en la casa); al reves, el otro.
                let (ra, fb) = e.dos(a, b, matches!(op, Op::Mul { .. } | Op::Add { .. }), &mut paso)?;
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
                // E6: una variable que es tambien fuente se acumula aparte (el
                // primer FMUL la pisaria antes de leerla).
                let alias = e.variable[d as usize] && a[..n as usize].iter().chain(&b[..n as usize]).any(|&r| r == d);
                let destino = e.destino(d, i)?;
                let x = if alias {
                    let t = e.pedir()?;
                    paso.push(t);
                    t
                } else {
                    destino
                };
                let (r0, f0) = e.dos(a[0], b[0], true, &mut paso)?;
                e.poner(c::fmul(x, c::r(r0), f0, false, 0), Clase::Fma, Some(x), [Some(r0), reg_de(f0), None]);
                let u = e.pedir()?;
                paso.push(u);
                for j in 1..n as usize {
                    let (rj, fj) = e.dos(a[j], b[j], true, &mut paso)?;
                    e.poner(c::fmul(u, c::r(rj), fj, false, 0), Clase::Fma, Some(u), [Some(rj), reg_de(fj), None]);
                    e.poner(c::fadd(x, c::r(x), c::r(u), false, 0), Clase::Fma, Some(x), [Some(x), Some(u), None]);
                }
                if alias {
                    e.poner(c::mov(destino, c::r(x), 0), Clase::Alu, Some(destino), [Some(x), None, None]);
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
            // 02-10: arrays, cubos, 3D, mips, Load y GetDimensions: en la
            // CPU todavia (la 3060 lee aqui un TEX 2D de nivel 0).
            // N5.4: elegir la textura al correr, la 3060 todavia no.
            Op::Lee { .. } | Op::EligeTextura { .. } => return Err(NoEmite::Operacion(i)),
            // N5.5: el computo, todavia no en la 3060 (va por la CPU).
            Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } | Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } => return Err(NoEmite::Operacion(i)),
            // E2.3b: el sombreador de geometria, igual (va por la CPU).
            Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. } => return Err(NoEmite::Operacion(i)),
            // E2.4: el contador de un UAV, igual; y (05-10) sus Interlocked.
            Op::Contador { .. } | Op::Atomico { .. } => return Err(NoEmite::Operacion(i)),
            // N5.6: sin, cos, exp2, log2... (MUFU) todavia no: va por la CPU.
            Op::Mate { .. } => return Err(NoEmite::Operacion(i)),
            // N5.7: `discard` (el KILL de la 3060) todavia no: va por la CPU.
            Op::Descarta { .. } => return Err(NoEmite::Operacion(i)),
            // N5.10: los arrays (registros indexables) todavia no: por la CPU.
            Op::LeeIndexado { .. } | Op::EscribeIndexado { .. } | Op::ConstantesEn { .. } => return Err(NoEmite::Operacion(i)),
            // E2.5: las olas (`vote`, `shfl` de la 3060) todavia no: por la CPU,
            // donde van de 32 en 32 carriles como en un warp.
            // D4.4: las derivadas de la mip de un muestreo no se emiten: solo
            // las lee el `Op::Muestra` de la CPU, y el TEX las hace solo.
            Op::Ola { que: bmo_proton_x::dxil::olas::Ola::Derivada { muestra: true, .. }, .. } => {}
            Op::Ola { .. } => return Err(NoEmite::Operacion(i)),
        }
        // Lo de paso, y lo que ya nadie lee, se devuelve.
        for t in paso {
            e.soltar(t);
        }
        for &r in &an.muere[i] {
            if let Some(Some(Valor::Reg(x))) = e.valor.get(r as usize).copied() {
                e.soltar(x);
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

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_saltos;
