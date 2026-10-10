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
//! **El Programa mismo vive en PROMETEO** desde el 08-10 (LB3b de
//! `docs/plan/PLAN_LAS_LIBRERIAS.md`): `platform/shared/prometeo/src/programa.rs`,
//! con sus numeros (FMad sin fundir, Dot, Rsqrt, Saturate) y su interprete.
//! Aqui queda el TRADUCTOR de DXIL, y la ruta de siempre: todo lo del
//! Programa se re-exporta en `dxil::programa`, para que PROTON-X no cambie
//! una linea de las suyas.

use alloc::string::String;
use alloc::vec::Vec;

use super::bits::{Bloque, Registro};
// El Programa de la casa, de PROMETEO, en la ruta de siempre (LB3b).
pub use bmo_prometeo::programa::*;
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
/// 19 de la pila A (07-10): `<n x T>` y los structs (`{ T, U }`, con
/// nombre o sin el): se aplanan como los arrays.
const TYPE_VECTOR: u64 = 12;
const TYPE_STRUCT_ANON: u64 = 18;
const TYPE_STRUCT_NAMED: u64 = 20;
/// Los campos que cabe en un [`Tipo::Estructura`].
pub(super) const CAMPOS: usize = 16;

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
/// 18 de la pila A (07-10): `atomicrmw` (un Interlocked* de `groupshared`).
const FUNC_ATOMICRMW: u64 = 38;
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
const DX_TEXTURE_STORE: i64 = 67;
const DX_GET_DIMENSIONS: i64 = 72;
/// D4.4: `CalculateLevelOfDetail` (y su `Unclamped`).
const DX_CALCULATE_LOD: i64 = 81;
const DX_BUFFER_LOAD: i64 = 68;
// N5.5 (05-10): el computo.
const DX_BUFFER_STORE: i64 = 69;
// E2.4 (05-10): el contador de un UAV (Append, Consume, Increment/DecrementCounter).
const DX_BUFFER_UPDATE_COUNTER: i64 = 70;
// 13 de la pila A (07-10): `RawBufferLoad`/`RawBufferStore`, lo que dxc da
// desde SM 6.2 para ByteAddressBuffer y StructuredBuffer: los argumentos de
// BufferLoad/BufferStore y dos mas (la mascara de lo leido y la alineacion).
const DX_RAW_BUFFER_LOAD: i64 = 139;
const DX_RAW_BUFFER_STORE: i64 = 140;
// 05-10: los `Interlocked*` de un UAV (en un dibujo y en el computo).
const DX_ATOMIC_BIN_OP: i64 = 78;
const DX_ATOMIC_COMPARE_EXCHANGE: i64 = 79;
const DX_BARRIER: i64 = 80;
const DX_THREAD_ID: i64 = 93;
const DX_GROUP_ID: i64 = 94;
const DX_THREAD_ID_IN_GROUP: i64 = 95;
const DX_FLATTENED_THREAD_ID_IN_GROUP: i64 = 96;
// E2.3b (05-10): el sombreador de geometria.
const DX_EMIT_STREAM: i64 = 97;
const DX_CUT_STREAM: i64 = 98;
const DX_EMIT_THEN_CUT_STREAM: i64 = 99;
// A6 (06-10): el SV_PrimitiveID de un GS.
const DX_PRIMITIVE_ID: i64 = 108;
const DX_DISCARD: i64 = 82;
/// Las filas de 16 bytes que puede tener un cbuffer en D3D (64 KiB).
const FILAS_DE_D3D: u16 = 4096;

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
/// N5.4: la `t` de una lectura del handle `k`: su ranura, o -- si es de un
/// array con el registro calculado -- [`DINAMICA`], tras apuntar el
/// [`Op::EligeTextura`] que la escoge. `None` si `k` no es el handle de una textura.
pub(super) fn textura(c: &mut Compilador, k: usize) -> Option<u8> {
    match c.valores.get(k).copied()? {
        Valor::Textura(t) => Some(t),
        Valor::TexturaEn { rango, i } => {
            c.ops.push(Op::EligeTextura { i, rango });
            Some(DINAMICA)
        }
        _ => None,
    }
}

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

// -- Leer el modulo ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Tipo {
    Vacio,
    Funcion { devuelve: usize },
    Puntero { a: usize },
    /// N5.10: `[n x elem]`. 19 (07-10): y un vector `<n x elem>`, que se
    /// aplana igual.
    Arreglo { n: usize, elem: usize },
    /// 19 de la pila A (07-10): un struct: sus `n` campos (ids de tipo), en
    /// orden; se aplana campo a campo. Mas de [`CAMPOS`], `Otro`.
    Estructura { n: u8, campos: [u32; CAMPOS] },
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
    /// N5.4: el handle de una textura de un array con el registro
    /// CALCULADO: su rango dinamico, y el registro (absoluto) en `i`.
    TexturaEn { rango: u8, i: Reg },
    /// N5.3: el handle de un SRV de bufer: su ranura (la de las texturas) y
    /// como se direcciona.
    Bufer(u8, crate::bufer::Modo),
    /// 15 de la pila A (07-10): el de un bufer de un array con el registro
    /// CALCULADO (bindless): su rango dinamico, el registro en `i`, y como
    /// se direcciona. Se lee como una textura de las de N5.4.
    BuferEn { rango: u8, i: Reg, modo: crate::bufer::Modo },
    Muestreador(u8),
    /// N5.5: el handle de un UAV de bufer: su ranura y como se direcciona.
    Uav(u8, crate::bufer::Modo),
    /// N5.5: un array `groupshared`: `n` palabras desde la `base` de la
    /// memoria compartida del grupo, si son enteros, y su tipo.
    Compartida { base: u32, n: u32, enteros: bool, tipo: u32 },
    /// N5.5: un puntero dentro de el: el indice, en un registro.
    PunteroCompartido { base: u32, n: u32, i: Reg, enteros: bool },
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
                TYPE_ARRAY | TYPE_VECTOR => v.push(Tipo::Arreglo { n: r.ops.first().copied().unwrap_or(0) as usize, elem: r.ops.get(1).copied().unwrap_or(0) as usize }),
                // [empaquetado, campos...]
                TYPE_STRUCT_ANON | TYPE_STRUCT_NAMED if r.ops.len() > 1 && r.ops.len() - 1 <= CAMPOS => {
                    let mut campos = [0u32; CAMPOS];
                    for (k, &c) in r.ops[1..].iter().enumerate() {
                        campos[k] = c as u32;
                    }
                    v.push(Tipo::Estructura { n: (r.ops.len() - 1) as u8, campos });
                }
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
    /// N5.5: las palabras de memoria compartida ya repartidas.
    pub(super) compartida: u32,
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
                // 05-10: un getelementptr constante (a la memoria compartida o a
                // un array global), con su indice ya sabido.
                super::arreglos::CST_CE_GEP | super::arreglos::CST_CE_INBOUNDS_GEP => super::arreglos::gep_constante(self, &r.ops, tipos, tipos_float, anchos)?,
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

    /// **Un operando float**: su registro. 17 de la pila A (07-10): el IR
    /// de LLVM tiene tipos, asi que si aqui va un float, lo que llegue SON
    /// los bits de un float aunque se apuntaran como entero (un `phi` o un
    /// `select` que se tipo por su primer valor); y `float undef` es 0.0
    /// (lo que la casa lee de un `undef`). Lo demas, con su nombre.
    pub(super) fn float(&mut self, id: usize) -> Result<Reg, NoPrograma> {
        match self.valores.get(id).copied() {
            Some(Valor::Float(r) | Valor::Bits(r)) => Ok(r),
            Some(Valor::Indefinido) => super::estructura::literal(self, 0),
            Some(Valor::Entero(v)) => super::estructura::literal(self, v as i32 as u32),
            otro => Err(NoPrograma::Forma(que_es("un operando que deberia ser un float", otro))),
        }
    }

    fn entero(&self, id: usize) -> Result<i64, NoPrograma> {
        match self.valores.get(id) {
            Some(Valor::Entero(v)) => Ok(*v),
            _ => Err(NoPrograma::Forma("un operando que deberia ser un entero constante y no lo es")),
        }
    }
}

/// **Que era el operando que no se pudo usar** (17 y 20 de la pila A,
/// 07-10): el metal solo decia "no es un numero"; con esto, la siguiente
/// corrida dice QUE era, y se sabe que falta.
pub(super) fn que_es(antes: &'static str, v: Option<Valor>) -> &'static str {
    let float = antes.contains("float");
    match (v, float) {
        (None, _) => "un operando que todavia no existe (una referencia hacia delante fuera de un phi)",
        (Some(Valor::Nada), true) => "un operando float que es una constante que la casa no lee (half, double, o un cast constante)",
        (Some(Valor::Nada), false) => "un operando que es una constante que la casa no lee (half, double, i64, un vector constante o un cast constante)",
        (Some(Valor::Cuatro(_) | Valor::CuatroEnteros(_)), _) => "un operando que es un ResRet o un CBufRet entero (sin extractvalue)",
        (Some(Valor::Bool(_)), true) => "un operando float que es un i1",
        (Some(Valor::Cbuffer(_) | Valor::Textura(_) | Valor::TexturaEn { .. } | Valor::Bufer(..) | Valor::Muestreador(_) | Valor::Uav(..)), _) => "un operando que es el handle de un recurso",
        (Some(Valor::Arreglo { .. } | Valor::Puntero { .. } | Valor::Compartida { .. } | Valor::PunteroCompartido { .. }), _) => "un operando que es un puntero (un array o la memoria compartida) sin load",
        (Some(Valor::Funcion(_)), _) => "un operando que es una funcion",
        _ => antes,
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
    let mut c = Compilador { valores: Vec::new(), iniciales: Vec::new(), ops: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0, recursos: s.recursos.clone(), ranuras: Ranuras::default(), bloques: Default::default(), literales: Vec::new(), compartida: 0 };

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
    // N5.3b (05-10): el paso de cada bufer estructurado, de `dx.resources`.
    let entero = |i: usize| match c.valores.get(i) {
        Some(Valor::Entero(v)) => Some(*v),
        _ => None,
    };
    c.ranuras.pasos = super::recursos::pasos_estructurados(m, entero);
    // ** E8g (09-10): la forma de cada rango de SRV de textura, de la PSV0.
    c.ranuras.formas = c.recursos.iter().filter(|r| r.clase == super::recursos::SRV && r.modo_de_bufer().is_none() && r.especie != 0).map(|r| (r.espacio, r.desde, r.hasta, r.especie)).collect();
    // 05-10: `[earlydepthstencil]`, de las banderas de `dx.entryPoints`.
    let temprana = super::recursos::banderas(m, entero) & super::recursos::TEMPRANA != 0;
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
    Ok(Programa { ops: c.ops, iniciales: c.iniciales, entradas: c.entradas, salidas: c.salidas, lee: c.lee, filas_cb: c.filas_cb, ranuras: c.ranuras, computo: Computo { hilos: s.hilos, compartida: c.compartida, temprana } })
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
        FUNC_ATOMICRMW => super::arreglos::atomico(c, &mut o)?,
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

// Las llamadas a `dx.op.*`: en `programa/llamadas.rs` (L6a, 05-10).
mod llamadas;
use llamadas::llamada;

#[cfg(test)]
mod pruebas_operandos;
