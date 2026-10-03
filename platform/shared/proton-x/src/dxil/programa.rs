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
//! salte callada. Un solo cbuffer. Desde E6b (02-10) un cuerpo de VARIOS
//! bloques: `br`, `phi`, `fcmp`/`icmp`, `select` y `add`/`sub` de enteros,
//! y el grafo vuelve a ser `si` y bucles en `estructura.rs`.
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
pub use super::ranuras::{Lugar, Mapa, Ranuras};
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
const TYPE_ARRAY: u64 = 11;
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
// E6b (02-10): los saltos del DXIL (`estructura.rs`).
const FUNC_BR: u64 = 11;
const FUNC_PHI: u64 = 16;
const FUNC_CMP2: u64 = 28;
const FUNC_VSELECT: u64 = 29;
// E6c (02-10): las conversiones y el `switch` (`enteros.rs`, `estructura.rs`).
const FUNC_CAST: u64 = 3;
const FUNC_SWITCH: u64 = 12;
const FUNC_EXTRACTVAL: u64 = 26;
const FUNC_DEBUG_LOC_AGAIN: u64 = 33;
const FUNC_CALL: u64 = 34;
const FUNC_ALLOCA: u64 = 19;
const FUNC_LOAD: u64 = 20;
const FUNC_GEP: u64 = 43;
const FUNC_STORE: u64 = 44;
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
const DX_SAMPLE: i64 = 60;
const DX_SAMPLE_BIAS: i64 = 61;
const DX_SAMPLE_LEVEL: i64 = 62;
const DX_SAMPLE_GRAD: i64 = 63;
const DX_TEXTURE_LOAD: i64 = 66;
const DX_GET_DIMENSIONS: i64 = 72;
const DX_BUFFER_LOAD: i64 = 68;
const DX_DISCARD: i64 = 82;
/// Las filas de 16 bytes que puede tener un cbuffer en D3D (64 KiB).
const FILAS_DE_D3D: u16 = 4096;

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
    /// Una instruccion de SM5 (`SHEX`) que no se sabe correr (su codigo).
    Sm5(u32),
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
    /// Los 4 floats del registro `fila` del cbuffer, en `d..d+4`. `cb` es
    /// su ranura ([`Ranuras::cbuffers`], 03-10, N5.2); tras el enlace,
    /// `fila` ya es la del bloque APLANADO (`lote::Enlace::constantes`), y
    /// quien corre el programa solo mira `fila`.
    Constantes { d: Reg, fila: u16, cb: u8 },
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
    /// N5.6: sin, cos, tan, exp2, log2, frac, los redondeos y los medios
    /// floats (`crate::mates`), sobre los BITS del registro.
    Mate { d: Reg, a: Reg, f: crate::mates::Mate },
    Min { d: Reg, a: Reg, b: Reg },
    Max { d: Reg, a: Reg, b: Reg },
    /// `Sample`: la textura `t` (el registro tN) con el muestreador `s` (sN)
    /// en `(u, v)`; los cuatro canales (R, G, B, A) en `d..d+4`.
    Muestra { d: Reg, t: u8, s: u8, u: Reg, v: Reg },
    /// 02-10: leer una textura con lo que `Muestra` (2D, la mip de la
    /// vista) no dice: `Sample` con mas coordenadas (arrays, cubos, 3D) o
    /// desplazado, `SampleLevel`, `SampleBias` y `SampleGrad` (sin su sesgo
    /// ni sus gradientes: la mip de la vista), `Load` y `GetDimensions` (ver
    /// [`Lectura`]). `c`: las coordenadas (floats, o enteros en `Load`; las
    /// que no trae, un registro a 0); `nivel`: la mip (un float en
    /// SampleLevel, un entero en Load y GetDimensions); `desp`: el
    /// desplazamiento en texeles. Los cuatro canales en `d..d+4`.
    Lee { d: Reg, t: u8, s: u8, como: Lectura, c: [Reg; 4], nivel: Reg, desp: [i8; 3] },

    // -- E6 (02-10): comparar, elegir y saltar. Un registro guarda BITS: un
    // float, un entero (complemento a dos) o un booleano de D3D (0xFFFFFFFF
    // si, 0 no). Las de abajo los copian tal cual; las de arriba los leen
    // como float.
    /// `d = a <como> b`: 0xFFFFFFFF si se cumple, 0 si no (como `lt`, `ge`,
    /// `eq`, `ne` de SM5). Float: con un NaN solo se cumple `Distinto` (D3D:
    /// `ne` es desordenada). `entero`: los bits como `i32`.
    Compara { d: Reg, a: Reg, b: Reg, como: Comparacion, entero: bool },
    /// `d = c != 0 ? a : b`, los bits (`movc`).
    Elige { d: Reg, c: Reg, a: Reg, b: Reg },
    /// `d = a`, los bits: una VARIABLE (escrita mas de una vez) se escribe asi.
    Copia { d: Reg, a: Reg },
    /// `d = a + b` como enteros de 32 bits (modulo 2^32: `iadd`).
    SumaEntera { d: Reg, a: Reg, b: Reg },
    /// E6c (02-10): las demas de enteros de 32 bits (ver [`OpEntera`]).
    Entera { d: Reg, a: Reg, b: Reg, op: OpEntera },
    /// E6c: de entero a float y al reves (ver [`Conversion`]).
    Convierte { d: Reg, a: Reg, como: Conversion },
    /// Lo de hasta su `SiNo` o su `FinSi` corre si los bits de `c` no son 0.
    Si { c: Reg },
    SiNo,
    FinSi,
    /// Lo de hasta su `FinBucle` se repite hasta un `Romper`.
    Bucle,
    /// Sale del bucle mas interno si los bits de `c` no son 0 (`si_cero`:
    /// si SON 0): `breakc_nz` y `breakc_z`.
    RomperSi { c: Reg, si_cero: bool },
    /// Sale del bucle mas interno (`break`).
    Romper,
    /// Vuelve a la cabeza del bucle mas interno (`continue`; E6b, 02-10: lo
    /// pide el DXIL, que salta a la cabeza desde el medio del cuerpo).
    Continuar,
    FinBucle,
    /// N5.7 (03-10): `discard` y `clip()`: si los bits de `c` no son 0, el
    /// pixel se TIRA -- ni color ni profundidad -- y el programa acaba ahi.
    /// Solo tiene sentido en el de pixeles.
    Descarta { c: Reg },
    /// N5.10 (03-10): `d = array[i]`: el array son los `n` registros desde
    /// `base`; el indice, los bits de `i` (fuera, 0).
    LeeIndexado { d: Reg, base: Reg, n: u16, i: Reg },
    /// `array[i] = s` (fuera, nada).
    EscribeIndexado { base: Reg, n: u16, i: Reg, s: Reg },
    /// 03-10: [`Op::Constantes`] con la fila CALCULADA: la `fila + i` (los
    /// bits de `i`), si `i < filas` (las que se reservan para el cbuffer:
    /// sin saber hasta donde llega el indice, las 4096 de D3D); si no, 0.
    /// Los arrays de un cbuffer (luces, huesos) se leen asi.
    ConstantesEn { d: Reg, fila: u16, filas: u16, i: Reg, cb: u8 },
}

/// Lo que pregunta [`Op::Compara`]. Las `SinSigno` (E6c), solo con
/// `entero`: los bits como `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparacion {
    Menor,
    MenorIgual,
    Mayor,
    MayorIgual,
    Igual,
    Distinto,
    MenorSinSigno,
    MenorIgualSinSigno,
    MayorSinSigno,
    MayorIgualSinSigno,
}

/// Lo que hace [`Op::Entera`], sobre los bits (modulo 2^32). Los
/// desplazamientos usan los 5 bits de abajo de la cuenta, como D3D (`ishl`,
/// `ushr`, `ishr`; `dxc` pone ese `& 31` el mismo).
///
/// La division y el resto (E6d): por 0 dan 0xFFFFFFFF, cociente y resto, con
/// signo o sin el (lo de D3D en `udiv`; la 3060 hace eso mismo, `~b`). Con
/// signo, hacia cero y el resto con el signo de `a`; `i32::MIN / -1` es
/// `i32::MIN` y su resto 0 (lo que no cabe da la vuelta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpEntera {
    Resta,
    Mul,
    Shl,
    /// `>>` logico (`ushr`, `lshr`).
    ShrL,
    /// `>>` aritmetico (`ishr`, `ashr`).
    ShrA,
    Y,
    O,
    OX,
    MinS,
    MaxS,
    MinU,
    MaxU,
    DivU,
    RemU,
    DivS,
    RemS,
}

impl OpEntera {
    pub fn hacer(self, a: u32, b: u32) -> u32 {
        match self {
            OpEntera::Resta => a.wrapping_sub(b),
            OpEntera::Mul => a.wrapping_mul(b),
            OpEntera::Shl => a << (b & 31),
            OpEntera::ShrL => a >> (b & 31),
            OpEntera::ShrA => ((a as i32) >> (b & 31)) as u32,
            OpEntera::Y => a & b,
            OpEntera::O => a | b,
            OpEntera::OX => a ^ b,
            OpEntera::MinS => (a as i32).min(b as i32) as u32,
            OpEntera::MaxS => (a as i32).max(b as i32) as u32,
            OpEntera::MinU => a.min(b),
            OpEntera::MaxU => a.max(b),
            _ if b == 0 => u32::MAX,
            OpEntera::DivU => a / b,
            OpEntera::RemU => a % b,
            OpEntera::DivS => (a as i32).wrapping_div(b as i32) as u32,
            OpEntera::RemS => (a as i32).wrapping_rem(b as i32) as u32,
        }
    }
}

/// Lo que hace [`Op::Convierte`]. De float a entero: hacia cero, y lo que no
/// cabe se queda en el limite (un NaN, 0) -- lo de D3D (`ftoi`, `ftou`) y de
/// PTX (`cvt.rzi`); `as` de Rust hace eso mismo. De entero a float: al mas
/// cercano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conversion {
    EnteroAFloat,
    SinSignoAFloat,
    FloatAEntero,
    FloatASinSigno,
}

impl Conversion {
    pub fn hacer(self, x: u32) -> u32 {
        match self {
            Conversion::EnteroAFloat => (x as i32 as f32).to_bits(),
            Conversion::SinSignoAFloat => (x as f32).to_bits(),
            Conversion::FloatAEntero => f32::from_bits(x) as i32 as u32,
            Conversion::FloatASinSigno => f32::from_bits(x) as u32,
        }
    }
}

impl Comparacion {
    /// Con floats (`Distinto` se cumple con un NaN; las demas, no).
    pub fn floats(self, x: f32, y: f32) -> bool {
        match self {
            Comparacion::Menor => x < y,
            Comparacion::MenorIgual => x <= y,
            Comparacion::Mayor => x > y,
            Comparacion::MayorIgual => x >= y,
            Comparacion::Igual => x == y,
            Comparacion::Distinto => x != y,
            // Sin sentido con floats (los lectores no las dan): las ordenadas.
            Comparacion::MenorSinSigno => x < y,
            Comparacion::MenorIgualSinSigno => x <= y,
            Comparacion::MayorSinSigno => x > y,
            Comparacion::MayorIgualSinSigno => x >= y,
        }
    }

    /// Con enteros: con signo, o sin el las `SinSigno`.
    pub fn enteros(self, x: i32, y: i32) -> bool {
        let (u, v) = (x as u32, y as u32);
        match self {
            Comparacion::Menor => x < y,
            Comparacion::MenorIgual => x <= y,
            Comparacion::Mayor => x > y,
            Comparacion::MayorIgual => x >= y,
            Comparacion::Igual => x == y,
            Comparacion::Distinto => x != y,
            Comparacion::MenorSinSigno => u < v,
            Comparacion::MenorIgualSinSigno => u <= v,
            Comparacion::MayorSinSigno => u > v,
            Comparacion::MayorIgualSinSigno => u >= v,
        }
    }
}

/// Por que la forma de un programa no vale (su indice): un `SiNo` o un
/// `FinSi` sin su `Si`, un `Romper` fuera de un bucle, algo sin cerrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalaForma(pub usize);

/// Lo que se puede anidar (bucles dentro de `si` dentro de bucles...).
pub const ANIDADO_MAXIMO: usize = 32;

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
    /// Las texturas y los muestreadores que lee, con su espacio y su registro
    /// (03-10, N5.1): el `t` y el `s` de [`Op::Lee`] y [`Op::Muestra`] son
    /// su POSICION aqui, no un registro.
    pub ranuras: Ranuras,
}

/// **Como lee una textura** [`Op::Lee`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lectura {
    /// `Sample` (y Bias y Grad): la mip mas detallada de la vista.
    Muestra,
    /// `SampleLevel`: la mip de `nivel` (float).
    Nivel,
    /// `Load`: el texel de coordenadas enteras; `enteros`, del formato
    /// entero (`.i32`), los canales como enteros.
    Carga { enteros: bool },
    /// `GetDimensions`: ancho, alto, profundidad o capas, mips (enteros).
    Medidas,
    /// N5.3: `Load` de un BUFER: `c[0]` el elemento (o el byte, crudo) y
    /// `c[1]` el desplazamiento dentro de el (estructurado). Como bits.
    Bufer(crate::bufer::Modo),
    /// N5.3: `GetDimensions` de un bufer: sus elementos (o bytes).
    MedidasBufer(crate::bufer::Modo),
}

impl Programa {
    /// N5.12: el que no hace nada (el de pixeles de un PSO sin el: solo
    /// profundidad, las sombras).
    pub fn vacio() -> Programa {
        Programa { ops: Vec::new(), iniciales: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0, ranuras: Ranuras::default() }
    }

    /// **Correr el sombreador una vez.** `entradas` y `salidas` por el id del
    /// elemento en su firma; `cb`, los bytes del cbuffer (lo que falte se lee
    /// como 0). `regs` es memoria de trabajo (se reusa entre llamadas).
    pub fn correr(&self, entradas: &[[f32; 4]], cb: &[u8], salidas: &mut [[f32; 4]], regs: &mut Vec<f32>) -> bool {
        self.correr_con(entradas, cb, &crate::textura::Recursos::NINGUNO, salidas, regs)
    }

    /// Si el programa lee alguna textura (`Sample`).
    pub fn muestrea(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Muestra { .. } | Op::Lee { .. }))
    }

    /// Si el programa salta (E6): `si`, bucles, o lo que lee bits como
    /// enteros o booleanos. Lo que no sabe de esto (el traductor a x86-64,
    /// `nativo`) lo mira aqui y se aparta.
    pub fn salta(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Compara { .. } | Op::Elige { .. } | Op::Copia { .. } | Op::SumaEntera { .. } | Op::Entera { .. } | Op::Convierte { .. } | Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle))
    }

    /// **La forma**: cada `Si` con su `FinSi` (y a lo sumo un `SiNo`), cada
    /// `Bucle` con su `FinBucle`, cada `Romper` dentro de un bucle, y no mas
    /// de [`ANIDADO_MAXIMO`] por dentro. Quien lee un sombreador la comprueba
    /// antes de darlo; el interprete y el emisor cuentan con ella.
    pub fn forma(&self) -> Result<(), MalaForma> {
        // Lo abierto: `true` un bucle, `false` un si (y si ya vio su SiNo).
        let mut abierto: Vec<(bool, bool)> = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            match op {
                Op::Si { .. } => abierto.push((false, false)),
                Op::Bucle => abierto.push((true, false)),
                Op::SiNo => match abierto.last_mut() {
                    Some((false, visto)) if !*visto => *visto = true,
                    _ => return Err(MalaForma(i)),
                },
                Op::FinSi => {
                    if !matches!(abierto.pop(), Some((false, _))) {
                        return Err(MalaForma(i));
                    }
                }
                Op::FinBucle => {
                    if !matches!(abierto.pop(), Some((true, _))) {
                        return Err(MalaForma(i));
                    }
                }
                Op::RomperSi { .. } | Op::Romper | Op::Continuar => {
                    if !abierto.iter().any(|x| x.0) {
                        return Err(MalaForma(i));
                    }
                }
                _ => {}
            }
            if abierto.len() > ANIDADO_MAXIMO {
                return Err(MalaForma(i));
            }
        }
        if abierto.is_empty() {
            Ok(())
        } else {
            Err(MalaForma(self.ops.len()))
        }
    }
}

/// Cuatro registros seguidos (lo que devuelve un ResRet), el primero.
fn cuatro(c: &mut Compilador) -> Result<Reg, NoPrograma> {
    let d = c.registro(0.0)?;
    for _ in 0..3 {
        c.registro(0.0)?;
    }
    Ok(d)
}

/// Los desplazamientos de un Sample o un Load: enteros constantes (o
/// `undef`, 0), de -8 a 7 como en D3D.
fn desplazamientos(c: &Compilador, ids: [usize; 3]) -> Result<[i8; 3], NoPrograma> {
    let mut v = [0i8; 3];
    for (k, id) in ids.into_iter().enumerate() {
        v[k] = match c.valores.get(id) {
            Some(Valor::Entero(o)) if (-8..8).contains(o) => *o as i8,
            Some(Valor::Indefinido) | None => 0,
            _ => return Err(NoPrograma::Forma("un desplazamiento de textura que no es una constante de -8 a 7")),
        };
    }
    Ok(v)
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
pub(super) enum Tipo {
    Vacio,
    Funcion { devuelve: usize },
    Puntero { a: usize },
    /// N5.10: `[n x elem]`.
    Arreglo { n: usize, elem: usize },
    Otro,
}

/// Un valor de LLVM, lo que se sabe de el al compilar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Valor {
    Nada,
    Entero(i64),
    /// Un float: en este registro.
    Float(Reg),
    /// E6b: un entero CALCULADO (sus bits, en este registro).
    Bits(Reg),
    /// E6c: un `i1` (0xFFFFFFFF cierto, 0 falso: el `true` de LLVM es -1).
    Bool(Reg),
    /// E6c: lo que devuelve CBufferLoadLegacy.i32: 4 enteros seguidos.
    CuatroEnteros(Reg),
    /// Lo que devuelve CBufferLoadLegacy: 4 floats seguidos.
    Cuatro(Reg),
    /// El handle de un cbuffer: su ranura.
    Cbuffer(u8),
    /// El handle de una textura (su registro tN) o de un muestreador (sN).
    Textura(u8),
    /// N5.3: el handle de un SRV de bufer: su ranura (la de las texturas) y
    /// como se direcciona.
    Bufer(u8, crate::bufer::Modo),
    Muestreador(u8),
    /// Una funcion del modulo (su indice en `funciones`).
    Funcion(usize),
    /// N5.10: un array (`alloca` o global): sus `n` registros desde `base`
    /// (aplanado), si son enteros, y su tipo (para bajar por el con GEP).
    Arreglo { base: Reg, n: u16, enteros: bool, tipo: u32 },
    /// N5.10: un puntero dentro de un array: el indice, en un registro.
    Puntero { base: Reg, n: u16, i: Reg, enteros: bool },
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
                TYPE_ARRAY => v.push(Tipo::Arreglo { n: r.ops.first().copied().unwrap_or(0) as usize, elem: r.ops.get(1).copied().unwrap_or(0) as usize }),
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
pub(super) fn con_signo(v: u64) -> i64 {
    if v & 1 == 0 {
        (v >> 1) as i64
    } else if v != 1 {
        -((v >> 1) as i64)
    } else {
        i64::MIN
    }
}

pub(super) struct Compilador {
    pub(super) valores: Vec<Valor>,
    pub(super) iniciales: Vec<f32>,
    pub(super) ops: Vec<Op>,
    entradas: usize,
    salidas: usize,
    lee: u32,
    filas_cb: u16,
    /// Los recursos con su espacio (de la PSV0) y las ranuras que se piden.
    recursos: Vec<super::recursos::Recurso>,
    ranuras: Ranuras,
    /// E6b: los bloques ya cerrados, el que se esta leyendo, y las
    /// constantes enteras que hicieron falta como registro.
    pub(super) bloques: super::estructura::Bloques,
    pub(super) literales: Vec<(u32, Reg)>,
}

impl Compilador {
    pub(super) fn registro(&mut self, inicial: f32) -> Result<Reg, NoPrograma> {
        let r = self.iniciales.len();
        if r >= Reg::MAX as usize - 4 {
            return Err(NoPrograma::Forma("un sombreador con mas de 65000 valores"));
        }
        self.iniciales.push(inicial);
        Ok(r as Reg)
    }

    /// Las constantes de un CONSTANTS_BLOCK, en orden, como valores nuevos.
    fn constantes(&mut self, b: &Bloque, tipos_float: &[bool], tipos: &[Tipo], anchos: &[u32]) -> Result<(), NoPrograma> {
        let (mut es_float, mut tipo) = (false, 0usize);
        for r in &b.registros {
            let v = match r.codigo {
                CST_SETTYPE => {
                    tipo = r.ops.first().copied().unwrap_or(0) as usize;
                    es_float = tipos_float.get(tipo).copied().unwrap_or(false);
                    continue;
                }
                // N5.10: las tablas (`static const float x[4] = {...}`).
                super::arreglos::CST_AGGREGATE | super::arreglos::CST_DATA => super::arreglos::constante(self, r.codigo, &r.ops, tipo, tipos, tipos_float, anchos)?,
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

    pub(super) fn float(&self, id: usize) -> Result<Reg, NoPrograma> {
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
    if let Some(t) = &s.sm5 {
        return crate::sm5::compilar(t, &s.entradas, &s.salidas);
    }
    let m = s.modulo.bloques.iter().find(|b| b.id == MODULE).ok_or(NoPrograma::Forma("sin MODULE_BLOCK"))?;
    let relativos = m.registros.iter().find(|r| r.codigo == MODULE_CODE_VERSION).and_then(|r| r.ops.first()).copied().unwrap_or(0) >= 1;
    let tipos = tipos(m);
    let floats = tipos_float(m);
    let anchos = super::enteros::anchos(m);
    let mut c = Compilador { valores: Vec::new(), iniciales: Vec::new(), ops: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0, recursos: s.recursos.clone(), ranuras: Ranuras::default(), bloques: Default::default(), literales: Vec::new() };

    // 1. Los globales, en el orden de sus registros (N5.10: las variables,
    //    apuntadas para cuando esten sus iniciales).
    let mut funciones: Vec<Funcion> = Vec::new();
    let mut globales: Vec<(usize, &[u64])> = Vec::new();
    for r in &m.registros {
        match r.codigo {
            MODULE_CODE_GLOBALVAR => {
                globales.push((c.valores.len(), &r.ops));
                c.valores.push(Valor::Nada);
            }
            MODULE_CODE_ALIAS => c.valores.push(Valor::Nada),
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
        c.constantes(b, &floats, &tipos, &anchos)?;
    }
    for (k, ops) in globales {
        c.valores[k] = super::arreglos::global(&mut c, ops, &tipos, &floats, &anchos)?;
    }
    // 3. El cuerpo: el primer FUNCTION_BLOCK es el de la primera funcion
    //    definida (la entrada: sin argumentos).
    if funciones.iter().filter(|f| !f.declarada).count() != 1 {
        return Err(NoPrograma::Forma("un modulo con mas de una funcion definida"));
    }
    let cuerpo = m.hijo(FUNCTION_BLOCK).ok_or(NoPrograma::Forma("la entrada no tiene cuerpo"))?;
    for b in cuerpo.bloques.iter().filter(|b| b.id == CONSTANTS) {
        c.constantes(b, &floats, &tipos, &anchos)?;
    }
    for r in &cuerpo.registros {
        instruccion(&mut c, r, relativos, &tipos, &funciones, &floats, &anchos)?;
    }
    // E6b: con saltos, el grafo de bloques vuelve a ser `si` y bucles.
    super::estructura::armar(&mut c)?;
    Ok(Programa { ops: c.ops, iniciales: c.iniciales, entradas: c.entradas, salidas: c.salidas, lee: c.lee, filas_cb: c.filas_cb, ranuras: c.ranuras })
}

/// Lee operandos de un registro de instruccion: relativos o absolutos, y si
/// es una referencia HACIA DELANTE, su tipo va detras (y se salta).
pub(super) struct Operandos<'a> {
    pub(super) ops: &'a [u64],
    pub(super) i: usize,
    pub(super) siguiente: usize,
    relativos: bool,
}

impl Operandos<'_> {
    pub(super) fn crudo(&mut self) -> Result<u64, NoPrograma> {
        let v = *self.ops.get(self.i).ok_or(NoPrograma::Forma("una instruccion cortada"))?;
        self.i += 1;
        Ok(v)
    }

    pub(super) fn id(&self, v: u64) -> usize {
        if self.relativos {
            (self.siguiente as u64).wrapping_sub(v) as u32 as usize
        } else {
            v as usize
        }
    }

    /// Un valor con su tipo si hace falta (`getValueTypePair`).
    pub(super) fn con_tipo(&mut self) -> Result<usize, NoPrograma> {
        let v = self.crudo()?;
        let id = self.id(v);
        if id >= self.siguiente {
            self.crudo()?;
        }
        Ok(id)
    }

    /// Otro lector en el mismo sitio (para mirar sin avanzar).
    pub(super) fn copia(&self) -> Self {
        Operandos { ops: self.ops, i: self.i, siguiente: self.siguiente, relativos: self.relativos }
    }

    /// Un valor sin tipo (`getValue`).
    pub(super) fn solo(&mut self) -> Result<usize, NoPrograma> {
        let v = self.crudo()?;
        Ok(self.id(v))
    }
}

#[allow(clippy::too_many_arguments)]
fn instruccion(c: &mut Compilador, r: &Registro, relativos: bool, tipos: &[Tipo], funciones: &[Funcion], floats: &[bool], anchos: &[u32]) -> Result<(), NoPrograma> {
    let mut o = Operandos { ops: &r.ops, i: 0, siguiente: c.valores.len(), relativos };
    match r.codigo {
        FUNC_DECLAREBLOCKS => c.bloques.declarar(r.ops.first().copied().unwrap_or(1) as usize, c.ops.len()),
        FUNC_DEBUG_LOC | FUNC_DEBUG_LOC_AGAIN => {}
        FUNC_RET => c.bloques.cerrar(super::estructura::Fin::Ret, c.ops.len())?,
        FUNC_BR => super::estructura::br(c, &mut o)?,
        FUNC_PHI => super::estructura::phi(c, &mut o, floats, anchos)?,
        FUNC_CAST => super::enteros::cast(c, &mut o, floats, anchos)?,
        FUNC_SWITCH => super::estructura::switch(c, &mut o)?,
        FUNC_CMP2 => super::estructura::cmp(c, &mut o)?,
        FUNC_VSELECT => super::estructura::select(c, &mut o)?,
        // N5.10: los arrays (`arreglos.rs`).
        FUNC_ALLOCA => super::arreglos::alloca(c, &r.ops, tipos, floats, anchos)?,
        FUNC_GEP => super::arreglos::gep(c, &mut o, tipos, floats, anchos)?,
        FUNC_LOAD => super::arreglos::load(c, &mut o)?,
        FUNC_STORE => super::arreglos::store(c, &mut o)?,
        // E6b: + y - de ENTEROS (un contador de bucle).
        FUNC_BINOP if super::enteros::es_entero(c, &o)? => super::enteros::binop_entero(c, &mut o)?,
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
                (Some(Valor::CuatroEnteros(base)), 0..=3) => Valor::Bits(base + k as Reg),
                _ => return Err(NoPrograma::Forma("extractvalue de algo que no es un CBufRet ni un ResRet")),
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
            let v = llamada(c, &args, &fun.nombre)?;
            if !vacia {
                c.valores.push(v);
            }
        }
        codigo => return Err(NoPrograma::Instruccion(codigo)),
    }
    Ok(())
}

/// Una llamada a `dx.op.*`: el primer argumento es el numero de operacion.
fn llamada(c: &mut Compilador, args: &[usize], nombre: &str) -> Result<Valor, NoPrograma> {
    // E6c: lo que trae ENTEROS lo dice su sobrecarga (`dx.op.X.i32`).
    let enteros = nombre.ends_with(".i32");
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de D3D con menos argumentos"));
    let op = c.entero(arg(0)?)?;
    let uno = |c: &mut Compilador, f: fn(Reg, Reg) -> Op| -> Result<Valor, NoPrograma> {
        let a = c.float(arg(1)?)?;
        let d = c.registro(0.0)?;
        c.ops.push(f(d, a));
        Ok(Valor::Float(d))
    };
    // Las olas (con un carril) y las derivadas: `olas.rs`.
    if let Some(v) = super::olas::de_un_carril(c, op, args) {
        return v;
    }
    Ok(match op {
        DX_LOAD_INPUT | DX_STORE_OUTPUT => {
            if !matches!(c.valores.get(arg(2)?), Some(Valor::Entero(_))) {
                return Err(NoPrograma::Forma("loadInput/storeOutput con una fila CALCULADA (una entrada en array): todavia no"));
            }
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
                if enteros {
                    Valor::Bits(d)
                } else {
                    Valor::Float(d)
                }
            } else {
                c.salidas = c.salidas.max(elemento as usize + 1);
                let s = super::estructura::bits(c, arg(4)?)?;
                c.ops.push(Op::Salida { s, elemento, componente });
                Valor::Nada
            }
        }
        // N5.7: `discard(i1 c)`; `clip(x)` llega como `discard(x < 0)`, y un
        // `discard` a secas, con un `i1 true` (un literal).
        DX_DISCARD => {
            let c_ = super::estructura::bits(c, arg(1)?)?;
            c.ops.push(Op::Descarta { c: c_ });
            Valor::Nada
        }
        DX_CREATE_HANDLE => {
            // (clase, rango, indice, no uniforme): 0 SRV, 1 UAV, 2 CBuffer, 3
            // Sampler. El indice es el REGISTRO (la base del rango incluida);
            // el ESPACIO, el del rango `rango` de su clase en la PSV0 (03-10).
            if !matches!(c.valores.get(arg(3)?), Some(Valor::Entero(_))) {
                return Err(NoPrograma::Forma("createHandle con un registro CALCULADO (un array de texturas o bindless): todavia no (N5.4)"));
            }
            let (clase, rango, indice) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?, c.entero(arg(3)?)?);
            if !(0..=3).contains(&clase) || rango < 0 || indice < 0 {
                return Err(NoPrograma::Forma("un createHandle con una clase, un rango o un registro imposibles"));
            }
            let espacio = super::recursos::rango(&c.recursos, clase as u8, rango as u32).map_or(0, |r| r.espacio);
            let registro = indice as u32;
            match clase {
                2 => Valor::Cbuffer(c.ranuras.cbuffer(espacio, registro)?),
                0 => {
                    let t = c.ranuras.textura(espacio, registro)?;
                    match super::recursos::rango(&c.recursos, 0, rango as u32).and_then(|r| r.modo_de_bufer()) {
                        Some(modo) => Valor::Bufer(t, modo),
                        None => Valor::Textura(t),
                    }
                }
                3 => Valor::Muestreador(c.ranuras.muestreador(espacio, registro)?),
                _ => return Err(NoPrograma::Forma("un UAV (RWTexture, RWBuffer...): todavia no")),
            }
        }
        DX_CBUFFER_LOAD_LEGACY => {
            let Some(&Valor::Cbuffer(cb)) = c.valores.get(arg(1)?) else {
                return Err(NoPrograma::Forma("CBufferLoadLegacy sin el handle del cbuffer"));
            };
            // 03-10: la fila CALCULADA (un array del cbuffer).
            if !matches!(c.valores.get(arg(2)?), Some(Valor::Entero(_))) {
                let i = super::estructura::bits(c, arg(2)?)?;
                let d = c.registro(0.0)?;
                for _ in 0..3 {
                    c.registro(0.0)?;
                }
                c.filas_cb = c.filas_cb.max(FILAS_DE_D3D);
                c.ops.push(Op::ConstantesEn { d, fila: 0, filas: FILAS_DE_D3D, i, cb });
                return Ok(if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) });
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
            c.ops.push(Op::Constantes { d, fila: fila as u16, cb });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        DX_SAMPLE | DX_SAMPLE_BIAS | DX_SAMPLE_LEVEL | DX_SAMPLE_GRAD => {
            // (srv, sampler, coord0..3, offset0..2, y lo de cada una: el
            // sesgo, la mip o los gradientes, y el clamp).
            let (Some(Valor::Textura(t)), Some(Valor::Muestreador(sm))) = (c.valores.get(arg(1)?).copied(), c.valores.get(arg(2)?).copied()) else {
                return Err(NoPrograma::Forma("Sample sin el handle de una textura y el de un muestreador"));
            };
            let desp = desplazamientos(c, [arg(7)?, arg(8)?, arg(9)?])?;
            let indefinido = |k: usize| matches!(c.valores.get(k), Some(Valor::Indefinido) | None);
            let plana = indefinido(arg(5)?) && indefinido(arg(6)?) && desp == [0; 3];
            if op == DX_SAMPLE && plana {
                // Lo de siempre (2D, sin desplazar): lo que sabe la 3060.
                let (u, v) = (c.float(arg(3)?)?, c.float(arg(4)?)?);
                let d = cuatro(c)?;
                c.ops.push(Op::Muestra { d, t, s: sm, u, v });
                Valor::Cuatro(d)
            } else {
                let co = [super::estructura::bits(c, arg(3)?)?, super::estructura::bits(c, arg(4)?)?, super::estructura::bits(c, arg(5)?)?, super::estructura::bits(c, arg(6)?)?];
                let (como, nivel) = match op {
                    DX_SAMPLE_LEVEL => (Lectura::Nivel, super::estructura::bits(c, arg(10)?)?),
                    _ => (Lectura::Muestra, super::estructura::literal(c, 0)?),
                };
                let d = cuatro(c)?;
                c.ops.push(Op::Lee { d, t, s: sm, como, c: co, nivel, desp });
                Valor::Cuatro(d)
            }
        }
        DX_TEXTURE_LOAD => {
            // (srv, mip o muestra, coord0..2, offset0..2).
            let Some(Valor::Textura(t)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("TextureLoad sin el handle de una textura (un UAV o un bufer: todavia no)"));
            };
            let nivel = super::estructura::bits(c, arg(2)?)?;
            let co = [super::estructura::bits(c, arg(3)?)?, super::estructura::bits(c, arg(4)?)?, super::estructura::bits(c, arg(5)?)?, super::estructura::literal(c, 0)?];
            let desp = desplazamientos(c, [arg(6)?, arg(7)?, arg(8)?])?;
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Carga { enteros }, c: co, nivel, desp });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        DX_BUFFER_LOAD => {
            // (srv, indice, desplazamiento): el desplazamiento solo lo trae
            // uno estructurado; en los demas es `undef`.
            let Some(Valor::Bufer(t, modo)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("BufferLoad sin el handle de un bufer (un UAV: todavia no)"));
            };
            let cero = super::estructura::literal(c, 0)?;
            let indice = super::estructura::bits(c, arg(2)?)?;
            let desp = if matches!(c.valores.get(arg(3)?), Some(Valor::Indefinido) | None) { cero } else { super::estructura::bits(c, arg(3)?)? };
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Bufer(modo), c: [indice, desp, cero, cero], nivel: cero, desp: [0; 3] });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        DX_GET_DIMENSIONS => {
            // (handle, mip): %dx.types.Dimensions, cuatro i32. De un bufer
            // (N5.3), sus elementos; el mip es `undef`.
            let t = match c.valores.get(arg(1)?).copied() {
                Some(Valor::Textura(t)) => t,
                Some(Valor::Bufer(t, modo)) => {
                    let cero = super::estructura::literal(c, 0)?;
                    let d = cuatro(c)?;
                    c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::MedidasBufer(modo), c: [cero; 4], nivel: cero, desp: [0; 3] });
                    return Ok(Valor::CuatroEnteros(d));
                }
                _ => return Err(NoPrograma::Forma("GetDimensions de algo que no es una textura ni un bufer (un UAV: todavia no)")),
            };
            let nivel = super::estructura::bits(c, arg(2)?)?;
            let cero = super::estructura::literal(c, 0)?;
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Medidas, c: [cero; 4], nivel, desp: [0; 3] });
            Valor::CuatroEnteros(d)
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
        // N5.6: la matematica (`crate::mates`). f16tof32 lee un entero; f32tof16
        // da uno (sus bits); las demas, float a float.
        _ if crate::mates::Mate::de_dxil(op).is_some() => {
            let f = crate::mates::Mate::de_dxil(op).unwrap_or(crate::mates::Mate::Frac);
            let a = if f == crate::mates::Mate::F16aF32 { super::estructura::bits(c, arg(1)?)? } else { c.float(arg(1)?)? };
            let d = c.registro(0.0)?;
            c.ops.push(Op::Mate { d, a, f });
            if f == crate::mates::Mate::F32aF16 {
                Valor::Bits(d)
            } else if f.da_booleano() {
                Valor::Bool(d)
            } else {
                Valor::Float(d)
            }
        }
        DX_RSQRT => uno(c, |d, a| Op::Rsqrt { d, a })?,
        DX_SQRT => uno(c, |d, a| Op::Sqrt { d, a })?,
        DX_SATURATE => uno(c, |d, a| Op::Saturate { d, a })?,
        DX_FABS => uno(c, |d, a| Op::Abs { d, a })?,
        // E6c: IMax, IMin, UMax, UMin (`dx.op.binary.i32`).
        37..=40 => super::enteros::min_max(c, op, arg(1)?, arg(2)?)?,
        DX_FMIN | DX_FMAX => {
            let (a, b) = (c.float(arg(1)?)?, c.float(arg(2)?)?);
            let d = c.registro(0.0)?;
            c.ops.push(if op == DX_FMIN { Op::Min { d, a, b } } else { Op::Max { d, a, b } });
            Valor::Float(d)
        }
        otra => return Err(NoPrograma::OperacionD3d(otra)),
    })
}
