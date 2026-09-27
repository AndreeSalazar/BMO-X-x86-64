//! **Un sombreador DXIL, EJECUTABLE en la CPU** (P3b3, 27-09).
//!
//! `mod.rs` sabe que funciones hay; esto sabe que HACE la de entrada. Lee el
//! cuerpo instruccion a instruccion (el bitcode de LLVM 3.7 que escribe `dxc`)
//! y lo convierte en un programa de registros que la CPU corre sin volver a
//! mirar el bitcode:
//!
//! ```text
//!    los tipos        TYPE_BLOCK: para saber que llamada devuelve algo
//!    los valores      globales, constantes del modulo, constantes de la
//!                     funcion y el resultado de cada instruccion, numerados
//!                     en ese orden; los operandos van RELATIVOS (VERSION 1)
//!    las operaciones  fmul fadd fsub fdiv, extractvalue, y las de D3D por su
//!                     numero: LoadInput StoreOutput CBufferLoadLegacy
//!                     CreateHandle FMad Dot2/3/4 Rsqrt Sqrt Saturate FAbs
//!                     FMin FMax
//! ```
//!
//! Lo que no sabe se dice al leer, con su nombre: nunca una instruccion que se
//! salte callada. Hoy, un cuerpo de UN bloque (sin saltos) y un solo cbuffer.
//!
//! # Los numeros, a mano (y por que asi)
//!
//! - **FMad NO se funde**: `a * b` redondeado y luego `+ c` redondeado. DXIL
//!   deja al driver fundirla o no; asi es como el juez de `bmo-cubo` hace
//!   `wvp * pos`, y el juez da las huellas de D3D12 en la 3060.
//! - **Dot** suma de izquierda a derecha, sin fundir (lo mismo).
//! - **Rsqrt** es `1 / raiz`, con la raiz EXACTA (redondeada al mas cercano,
//!   en enteros: `raiz`), igual en el anfitrion y en Ring 3 con soft-float.
//! - **Saturate** de un NaN es 0, como pide D3D.
//!
//! Cada decision se paga contra las huellas: el banco de la casa dibuja
//! `cubo.exe` con esto y compara con lo que la 3060 dibujo bajo Windows.

use alloc::string::String;
use alloc::vec::Vec;

use super::bits::{Bloque, Registro};
use super::Sombreador;

// Bloques y registros de LLVM 3.7 (LLVMBitCodes.h).
const MODULE: u64 = 8;
const CONSTANTS: u64 = 11;
const FUNCTION_BLOCK: u64 = 12;
const VALUE_SYMTAB: u64 = 14;
const TYPE_BLOCK_NEW: u64 = 17;

const MODULE_CODE_VERSION: u64 = 1;
const MODULE_CODE_GLOBALVAR: u64 = 7;
const MODULE_CODE_FUNCTION: u64 = 8;
const MODULE_CODE_ALIAS: u64 = 9;

const TYPE_NUMENTRY: u64 = 1;
const TYPE_VOID: u64 = 2;
const TYPE_POINTER: u64 = 8;
const TYPE_STRUCT_NAME: u64 = 19;
const TYPE_FUNCTION: u64 = 21;

const CST_SETTYPE: u64 = 1;
const CST_NULL: u64 = 2;
const CST_UNDEF: u64 = 3;
const CST_INTEGER: u64 = 4;
const CST_FLOAT: u64 = 6;

const FUNC_DECLAREBLOCKS: u64 = 1;
const FUNC_BINOP: u64 = 2;
const FUNC_RET: u64 = 10;
const FUNC_EXTRACTVAL: u64 = 26;
const FUNC_DEBUG_LOC_AGAIN: u64 = 33;
const FUNC_CALL: u64 = 34;
const FUNC_DEBUG_LOC: u64 = 35;

const CALL_EXPLICIT_TYPE: u64 = 1 << 15;

// Las operaciones de D3D que se saben (DxilConstants.h, OpCode).
const DX_FABS: i64 = 6;
const DX_SATURATE: i64 = 7;
const DX_SQRT: i64 = 24;
const DX_RSQRT: i64 = 25;
const DX_FMAX: i64 = 35;
const DX_FMIN: i64 = 36;
const DX_FMAD: i64 = 46;
const DX_DOT2: i64 = 54;
const DX_DOT3: i64 = 55;
const DX_DOT4: i64 = 56;
const DX_LOAD_INPUT: i64 = 4;
const DX_STORE_OUTPUT: i64 = 5;
const DX_CREATE_HANDLE: i64 = 57;
const DX_CBUFFER_LOAD_LEGACY: i64 = 59;

/// Por que un sombreador no se deja correr. El texto dice CUAL cosa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoPrograma {
    /// La forma del modulo no es la esperada.
    Forma(&'static str),
    /// Una instruccion de LLVM que no se sabe correr (su codigo de registro).
    Instruccion(u64),
    /// Una operacion de D3D que no se sabe correr (su numero de `dx.op`).
    OperacionD3d(i64),
    /// Una llamada a algo que no es `dx.op.*`.
    Llamada(String),
}

/// Un registro del programa: un `f32`.
pub type Reg = u16;

/// Una operacion del programa ya compilado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// `d = entrada[elemento][componente]`.
    Entrada { d: Reg, elemento: u8, componente: u8 },
    /// `salida[elemento][componente] = s`.
    Salida { s: Reg, elemento: u8, componente: u8 },
    /// Los 4 floats del registro `fila` del cbuffer, en `d..d+4`.
    Constantes { d: Reg, fila: u16 },
    Mul { d: Reg, a: Reg, b: Reg },
    Add { d: Reg, a: Reg, b: Reg },
    Sub { d: Reg, a: Reg, b: Reg },
    Div { d: Reg, a: Reg, b: Reg },
    /// `a * b + c`, sin fundir.
    Mad { d: Reg, a: Reg, b: Reg, c: Reg },
    /// El producto escalar de `n` componentes.
    Dot { d: Reg, n: u8, a: [Reg; 4], b: [Reg; 4] },
    Rsqrt { d: Reg, a: Reg },
    Sqrt { d: Reg, a: Reg },
    Saturate { d: Reg, a: Reg },
    Abs { d: Reg, a: Reg },
    Min { d: Reg, a: Reg, b: Reg },
    Max { d: Reg, a: Reg, b: Reg },
}

/// **Un sombreador listo para correr.**
#[derive(Debug, Clone, PartialEq)]
pub struct Programa {
    pub ops: Vec<Op>,
    /// Los registros al empezar: las constantes float del modulo ya puestas.
    pub iniciales: Vec<f32>,
    /// Cuantos elementos de entrada y de salida toca (por su id en la firma).
    pub entradas: usize,
    pub salidas: usize,
    /// Que elementos de entrada LEE de verdad (bit = id en la firma).
    pub lee: u32,
    /// El mayor registro del cbuffer que lee, mas uno (en filas de 16 bytes).
    pub filas_cb: u16,
}

impl Programa {
    /// **Correr el sombreador una vez.** `entradas` y `salidas` por el id del
    /// elemento en su firma; `cb`, los bytes del cbuffer (lo que falte se lee
    /// como 0). `regs` es memoria de trabajo (se reusa entre llamadas).
    pub fn correr(&self, entradas: &[[f32; 4]], cb: &[u8], salidas: &mut [[f32; 4]], regs: &mut Vec<f32>) {
        regs.clear();
        regs.extend_from_slice(&self.iniciales);
        for op in &self.ops {
            match *op {
                Op::Entrada { d, elemento, componente } => {
                    regs[d as usize] = entradas.get(elemento as usize).map(|e| e[componente as usize & 3]).unwrap_or(0.0);
                }
                Op::Salida { s, elemento, componente } => {
                    if let Some(e) = salidas.get_mut(elemento as usize) {
                        e[componente as usize & 3] = regs[s as usize];
                    }
                }
                Op::Constantes { d, fila } => {
                    for k in 0..4 {
                        let o = fila as usize * 16 + 4 * k;
                        regs[d as usize + k] = cb.get(o..o + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0.0);
                    }
                }
                Op::Mul { d, a, b } => regs[d as usize] = regs[a as usize] * regs[b as usize],
                Op::Add { d, a, b } => regs[d as usize] = regs[a as usize] + regs[b as usize],
                Op::Sub { d, a, b } => regs[d as usize] = regs[a as usize] - regs[b as usize],
                Op::Div { d, a, b } => regs[d as usize] = regs[a as usize] / regs[b as usize],
                Op::Mad { d, a, b, c } => {
                    let p = regs[a as usize] * regs[b as usize];
                    regs[d as usize] = p + regs[c as usize];
                }
                Op::Dot { d, n, a, b } => {
                    let mut s = regs[a[0] as usize] * regs[b[0] as usize];
                    for k in 1..n as usize {
                        s = s + regs[a[k] as usize] * regs[b[k] as usize];
                    }
                    regs[d as usize] = s;
                }
                Op::Rsqrt { d, a } => regs[d as usize] = 1.0 / raiz(regs[a as usize]),
                Op::Sqrt { d, a } => regs[d as usize] = raiz(regs[a as usize]),
                Op::Saturate { d, a } => regs[d as usize] = saturar(regs[a as usize]),
                Op::Abs { d, a } => regs[d as usize] = f32::from_bits(regs[a as usize].to_bits() & 0x7FFF_FFFF),
                // FMin/FMax de D3D: si uno es NaN, el otro.
                Op::Min { d, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[d as usize] = if x.is_nan() || y < x { y } else { x };
                }
                Op::Max { d, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[d as usize] = if x.is_nan() || y > x { y } else { x };
                }
            }
        }
    }
}

/// `saturate` de D3D: a [0, 1], y un NaN es 0.
pub fn saturar(x: f32) -> f32 {
    if x > 0.0 {
        if x < 1.0 {
            x
        } else {
            1.0
        }
    } else {
        0.0
    }
}

/// **La raiz cuadrada EXACTA de un `f32`**, redondeada al mas cercano con
/// empates al par (lo que pide IEEE-754), con enteros: sale igual en cualquier
/// CPU y con soft-float, sin `libm`.
pub fn raiz(x: f32) -> f32 {
    let b = x.to_bits();
    if x.is_nan() || x == 0.0 || x == f32::INFINITY {
        return x; // raiz(-0) = -0
    }
    if b >> 31 != 0 {
        return f32::NAN;
    }
    let (mut e, mut m) = (((b >> 23) & 0xFF) as i32, (b & 0x7F_FFFF) as u64);
    if e == 0 {
        // Subnormal: normalizar la mantisa.
        e = 1;
        while m & 0x80_0000 == 0 {
            m <<= 1;
            e -= 1;
        }
    } else {
        m |= 0x80_0000;
    }
    // x = m * 2^(e - 150); con el exponente par, raiz(x) = raiz(m') * 2^(k/2).
    let mut k = e - 150;
    if k & 1 != 0 {
        m <<= 1;
        k -= 1;
    }
    // raiz(m << 48) tiene 36..37 bits: de sobra para 24 y el redondeo.
    let n = (m as u128) << 48;
    let mut r: u128 = 0;
    let mut bit: u128 = 1 << 72;
    let mut resto = n;
    while bit > n {
        bit >>= 2;
    }
    while bit != 0 {
        if resto >= r + bit {
            resto -= r + bit;
            r = (r >> 1) + bit;
        } else {
            r >>= 1;
        }
        bit >>= 2;
    }
    // raiz(x) = (r + fraccion) * 2^(k/2 - 24), con `resto != 0` si no es exacta.
    let largo = 128 - r.leading_zeros() as i32;
    let sobra = largo - 24;
    let mut mant = (r >> sobra) as u64;
    let caidos = r & ((1u128 << sobra) - 1);
    let mitad = 1u128 << (sobra - 1);
    if caidos > mitad || (caidos == mitad && (resto != 0 || mant & 1 == 1)) {
        mant += 1;
    }
    let mut exp = k / 2 - 24 + sobra + 23; // del bit alto de `mant`
    if mant == 1 << 24 {
        mant >>= 1;
        exp += 1;
    }
    f32::from_bits((((exp + 127) as u32) << 23) | (mant as u32 & 0x7F_FFFF))
}

// -- Leer el modulo ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tipo {
    Vacio,
    Funcion { devuelve: usize },
    Puntero { a: usize },
    Otro,
}

/// Un valor de LLVM, lo que se sabe de el al compilar.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Valor {
    Nada,
    Entero(i64),
    /// Un float: en este registro.
    Float(Reg),
    /// Lo que devuelve CBufferLoadLegacy: 4 floats seguidos.
    Cuatro(Reg),
    /// El handle de un cbuffer.
    Cbuffer,
    /// Una funcion del modulo (su indice en `funciones`).
    Funcion(usize),
    Indefinido,
}

struct Funcion {
    nombre: String,
    tipo: usize,
    declarada: bool,
}

fn tipos(m: &Bloque) -> Vec<Tipo> {
    let mut v = Vec::new();
    if let Some(t) = m.hijo(TYPE_BLOCK_NEW) {
        for r in &t.registros {
            match r.codigo {
                TYPE_NUMENTRY | TYPE_STRUCT_NAME => {}
                TYPE_VOID => v.push(Tipo::Vacio),
                TYPE_POINTER => v.push(Tipo::Puntero { a: r.ops.first().copied().unwrap_or(0) as usize }),
                // [vararg, devuelve, parametros...]
                TYPE_FUNCTION => v.push(Tipo::Funcion { devuelve: r.ops.get(1).copied().unwrap_or(0) as usize }),
                _ => v.push(Tipo::Otro),
            }
        }
    }
    v
}

fn texto(ops: &[u64]) -> String {
    ops.iter().map(|&c| c as u8 as char).collect()
}

/// El valor de un entero de constantes: con el signo en el bit 0.
fn con_signo(v: u64) -> i64 {
    if v & 1 == 0 {
        (v >> 1) as i64
    } else if v != 1 {
        -((v >> 1) as i64)
    } else {
        i64::MIN
    }
}

struct Compilador {
    valores: Vec<Valor>,
    iniciales: Vec<f32>,
    ops: Vec<Op>,
    entradas: usize,
    salidas: usize,
    lee: u32,
    filas_cb: u16,
}

impl Compilador {
    fn registro(&mut self, inicial: f32) -> Result<Reg, NoPrograma> {
        let r = self.iniciales.len();
        if r >= Reg::MAX as usize - 4 {
            return Err(NoPrograma::Forma("un sombreador con mas de 65000 valores"));
        }
        self.iniciales.push(inicial);
        Ok(r as Reg)
    }

    /// Las constantes de un CONSTANTS_BLOCK, en orden, como valores nuevos.
    fn constantes(&mut self, b: &Bloque, tipos_float: &[bool]) -> Result<(), NoPrograma> {
        let mut es_float = false;
        for r in &b.registros {
            let v = match r.codigo {
                CST_SETTYPE => {
                    es_float = tipos_float.get(r.ops.first().copied().unwrap_or(0) as usize).copied().unwrap_or(false);
                    continue;
                }
                CST_INTEGER => Valor::Entero(con_signo(r.ops.first().copied().unwrap_or(0))),
                CST_FLOAT if es_float => {
                    let bits = r.ops.first().copied().unwrap_or(0) as u32;
                    Valor::Float(self.registro(f32::from_bits(bits))?)
                }
                // El cero de su tipo (`i32 0`, `float 0.0`, `i1 false`).
                CST_NULL if es_float => Valor::Float(self.registro(0.0)?),
                CST_NULL => Valor::Entero(0),
                CST_UNDEF => Valor::Indefinido,
                _ => Valor::Nada,
            };
            self.valores.push(v);
        }
        Ok(())
    }

    fn float(&self, id: usize) -> Result<Reg, NoPrograma> {
        match self.valores.get(id) {
            Some(Valor::Float(r)) => Ok(*r),
            _ => Err(NoPrograma::Forma("un operando que deberia ser un float y no lo es")),
        }
    }

    fn entero(&self, id: usize) -> Result<i64, NoPrograma> {
        match self.valores.get(id) {
            Some(Valor::Entero(v)) => Ok(*v),
            _ => Err(NoPrograma::Forma("un operando que deberia ser un entero constante y no lo es")),
        }
    }
}

/// Los `float` (y solo esos) por id de tipo: el registro TYPE de 32 bits es
/// `FLOAT` (codigo 3).
fn tipos_float(m: &Bloque) -> Vec<bool> {
    let mut v = Vec::new();
    if let Some(t) = m.hijo(TYPE_BLOCK_NEW) {
        for r in &t.registros {
            match r.codigo {
                TYPE_NUMENTRY | TYPE_STRUCT_NAME => {}
                c => v.push(c == 3),
            }
        }
    }
    v
}

/// **Compilar el punto de entrada** de un sombreador leido.
pub fn compilar(s: &Sombreador) -> Result<Programa, NoPrograma> {
    let m = s.modulo.bloques.iter().find(|b| b.id == MODULE).ok_or(NoPrograma::Forma("sin MODULE_BLOCK"))?;
    let relativos = m.registros.iter().find(|r| r.codigo == MODULE_CODE_VERSION).and_then(|r| r.ops.first()).copied().unwrap_or(0) >= 1;
    let tipos = tipos(m);
    let floats = tipos_float(m);
    let mut c = Compilador { valores: Vec::new(), iniciales: Vec::new(), ops: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0 };

    // 1. Los globales, en el orden de sus registros.
    let mut funciones: Vec<Funcion> = Vec::new();
    for r in &m.registros {
        match r.codigo {
            MODULE_CODE_GLOBALVAR | MODULE_CODE_ALIAS => c.valores.push(Valor::Nada),
            MODULE_CODE_FUNCTION => {
                c.valores.push(Valor::Funcion(funciones.len()));
                funciones.push(Funcion {
                    nombre: String::new(),
                    tipo: r.ops.first().copied().unwrap_or(0) as usize,
                    declarada: r.ops.get(2).copied().unwrap_or(1) != 0,
                });
            }
            _ => {}
        }
    }
    if let Some(vst) = m.hijo(VALUE_SYMTAB) {
        for r in &vst.registros {
            let (id, nombre) = match r.codigo {
                1 => (r.ops.first().copied(), r.ops.get(1..).map(texto)),
                3 => (r.ops.first().copied(), r.ops.get(2..).map(texto)),
                _ => (None, None),
            };
            if let (Some(id), Some(n)) = (id, nombre) {
                if let Some(Valor::Funcion(f)) = c.valores.get(id as usize) {
                    funciones[*f].nombre = n;
                }
            }
        }
    }
    // 2. Las constantes del modulo.
    for b in m.bloques.iter().filter(|b| b.id == CONSTANTS) {
        c.constantes(b, &floats)?;
    }
    // 3. El cuerpo: el primer FUNCTION_BLOCK es el de la primera funcion
    //    definida (la entrada: sin argumentos).
    if funciones.iter().filter(|f| !f.declarada).count() != 1 {
        return Err(NoPrograma::Forma("un modulo con mas de una funcion definida"));
    }
    let cuerpo = m.hijo(FUNCTION_BLOCK).ok_or(NoPrograma::Forma("la entrada no tiene cuerpo"))?;
    for b in cuerpo.bloques.iter().filter(|b| b.id == CONSTANTS) {
        c.constantes(b, &floats)?;
    }
    for r in &cuerpo.registros {
        instruccion(&mut c, r, relativos, &tipos, &funciones)?;
    }
    Ok(Programa { ops: c.ops, iniciales: c.iniciales, entradas: c.entradas, salidas: c.salidas, lee: c.lee, filas_cb: c.filas_cb })
}

/// Lee operandos de un registro de instruccion: relativos o absolutos, y si
/// es una referencia HACIA DELANTE, su tipo va detras (y se salta).
struct Operandos<'a> {
    ops: &'a [u64],
    i: usize,
    siguiente: usize,
    relativos: bool,
}

impl Operandos<'_> {
    fn crudo(&mut self) -> Result<u64, NoPrograma> {
        let v = *self.ops.get(self.i).ok_or(NoPrograma::Forma("una instruccion cortada"))?;
        self.i += 1;
        Ok(v)
    }

    fn id(&self, v: u64) -> usize {
        if self.relativos {
            (self.siguiente as u64).wrapping_sub(v) as u32 as usize
        } else {
            v as usize
        }
    }

    /// Un valor con su tipo si hace falta (`getValueTypePair`).
    fn con_tipo(&mut self) -> Result<usize, NoPrograma> {
        let v = self.crudo()?;
        let id = self.id(v);
        if id >= self.siguiente {
            self.crudo()?;
        }
        Ok(id)
    }

    /// Un valor sin tipo (`getValue`).
    fn solo(&mut self) -> Result<usize, NoPrograma> {
        let v = self.crudo()?;
        Ok(self.id(v))
    }
}

fn instruccion(c: &mut Compilador, r: &Registro, relativos: bool, tipos: &[Tipo], funciones: &[Funcion]) -> Result<(), NoPrograma> {
    let mut o = Operandos { ops: &r.ops, i: 0, siguiente: c.valores.len(), relativos };
    match r.codigo {
        FUNC_DECLAREBLOCKS => {
            if r.ops.first().copied() != Some(1) {
                return Err(NoPrograma::Forma("un cuerpo con saltos (mas de un bloque): todavia no"));
            }
        }
        FUNC_DEBUG_LOC | FUNC_DEBUG_LOC_AGAIN => {}
        FUNC_RET => {}
        FUNC_BINOP => {
            let a = o.con_tipo()?;
            let b = o.solo()?;
            let opcode = o.crudo()?;
            let (ra, rb) = (c.float(a)?, c.float(b)?);
            let d = c.registro(0.0)?;
            c.ops.push(match opcode {
                0 => Op::Add { d, a: ra, b: rb },
                1 => Op::Sub { d, a: ra, b: rb },
                2 => Op::Mul { d, a: ra, b: rb },
                4 => Op::Div { d, a: ra, b: rb },
                _ => return Err(NoPrograma::Forma("una operacion binaria que no es + - * /")),
            });
            c.valores.push(Valor::Float(d));
        }
        FUNC_EXTRACTVAL => {
            let a = o.con_tipo()?;
            let k = o.crudo()?;
            let v = match (c.valores.get(a), k) {
                (Some(Valor::Cuatro(base)), 0..=3) => Valor::Float(base + k as Reg),
                _ => return Err(NoPrograma::Forma("extractvalue de algo que no es un CBufRet")),
            };
            c.valores.push(v);
        }
        FUNC_CALL => {
            let _atributos = o.crudo()?;
            let cc = o.crudo()?;
            if cc & CALL_EXPLICIT_TYPE != 0 {
                o.crudo()?;
            }
            let f = o.con_tipo()?;
            let Some(Valor::Funcion(fi)) = c.valores.get(f).copied() else {
                return Err(NoPrograma::Forma("una llamada indirecta"));
            };
            let fun = &funciones[fi];
            if !fun.nombre.starts_with("dx.op.") {
                return Err(NoPrograma::Llamada(fun.nombre.clone()));
            }
            let mut t = tipos.get(fun.tipo).copied().unwrap_or(Tipo::Otro);
            if let Tipo::Puntero { a } = t {
                t = tipos.get(a).copied().unwrap_or(Tipo::Otro);
            }
            let vacia = match t {
                Tipo::Funcion { devuelve } => tipos.get(devuelve) == Some(&Tipo::Vacio),
                _ => return Err(NoPrograma::Forma("una llamada a algo sin tipo de funcion")),
            };
            let mut args = Vec::new();
            while o.i < o.ops.len() {
                args.push(o.solo()?);
            }
            let v = llamada(c, &args)?;
            if !vacia {
                c.valores.push(v);
            }
        }
        codigo => return Err(NoPrograma::Instruccion(codigo)),
    }
    Ok(())
}

/// Una llamada a `dx.op.*`: el primer argumento es el numero de operacion.
fn llamada(c: &mut Compilador, args: &[usize]) -> Result<Valor, NoPrograma> {
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de D3D con menos argumentos"));
    let op = c.entero(arg(0)?)?;
    let uno = |c: &mut Compilador, f: fn(Reg, Reg) -> Op| -> Result<Valor, NoPrograma> {
        let a = c.float(arg(1)?)?;
        let d = c.registro(0.0)?;
        c.ops.push(f(d, a));
        Ok(Valor::Float(d))
    };
    Ok(match op {
        DX_LOAD_INPUT | DX_STORE_OUTPUT => {
            let (elemento, fila, col) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?, c.entero(arg(3)?)?);
            if fila != 0 || !(0..32).contains(&elemento) || !(0..4).contains(&col) {
                return Err(NoPrograma::Forma("una entrada o salida en array o fuera de rango: todavia no"));
            }
            let (elemento, componente) = (elemento as u8, col as u8);
            if op == DX_LOAD_INPUT {
                c.entradas = c.entradas.max(elemento as usize + 1);
                c.lee |= 1 << elemento;
                let d = c.registro(0.0)?;
                c.ops.push(Op::Entrada { d, elemento, componente });
                Valor::Float(d)
            } else {
                c.salidas = c.salidas.max(elemento as usize + 1);
                let s = c.float(arg(4)?)?;
                c.ops.push(Op::Salida { s, elemento, componente });
                Valor::Nada
            }
        }
        DX_CREATE_HANDLE => {
            // (clase, rango, indice, no uniforme): clase 2 es CBuffer.
            let (clase, rango, indice) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?, c.entero(arg(3)?)?);
            if clase != 2 || rango != 0 || indice != 0 {
                return Err(NoPrograma::Forma("un recurso que no es el cbuffer b0 (texturas, UAV...): todavia no"));
            }
            Valor::Cbuffer
        }
        DX_CBUFFER_LOAD_LEGACY => {
            if c.valores.get(arg(1)?) != Some(&Valor::Cbuffer) {
                return Err(NoPrograma::Forma("CBufferLoadLegacy sin el handle del cbuffer"));
            }
            let fila = c.entero(arg(2)?)?;
            if !(0..4096).contains(&fila) {
                return Err(NoPrograma::Forma("CBufferLoadLegacy con una fila fuera del cbuffer"));
            }
            let d = c.registro(0.0)?;
            for _ in 0..3 {
                c.registro(0.0)?;
            }
            c.filas_cb = c.filas_cb.max(fila as u16 + 1);
            c.ops.push(Op::Constantes { d, fila: fila as u16 });
            Valor::Cuatro(d)
        }
        DX_FMAD => {
            let (a, b, cc) = (c.float(arg(1)?)?, c.float(arg(2)?)?, c.float(arg(3)?)?);
            let d = c.registro(0.0)?;
            c.ops.push(Op::Mad { d, a, b, c: cc });
            Valor::Float(d)
        }
        DX_DOT2 | DX_DOT3 | DX_DOT4 => {
            let n = (op - DX_DOT2 + 2) as usize;
            let (mut a, mut b) = ([0 as Reg; 4], [0 as Reg; 4]);
            for k in 0..n {
                a[k] = c.float(arg(1 + k)?)?;
                b[k] = c.float(arg(1 + n + k)?)?;
            }
            let d = c.registro(0.0)?;
            c.ops.push(Op::Dot { d, n: n as u8, a, b });
            Valor::Float(d)
        }
        DX_RSQRT => uno(c, |d, a| Op::Rsqrt { d, a })?,
        DX_SQRT => uno(c, |d, a| Op::Sqrt { d, a })?,
        DX_SATURATE => uno(c, |d, a| Op::Saturate { d, a })?,
        DX_FABS => uno(c, |d, a| Op::Abs { d, a })?,
        DX_FMIN | DX_FMAX => {
            let (a, b) = (c.float(arg(1)?)?, c.float(arg(2)?)?);
            let d = c.registro(0.0)?;
            c.ops.push(if op == DX_FMIN { Op::Min { d, a, b } } else { Op::Max { d, a, b } });
            Valor::Float(d)
        }
        otra => return Err(NoPrograma::OperacionD3d(otra)),
    })
}
