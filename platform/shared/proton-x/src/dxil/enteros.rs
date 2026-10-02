//! **Los enteros y las conversiones del DXIL** (E6c, 02-10).
//!
//! Lo que `dxc` escribe para `int`, `uint` y `bool`:
//!
//! ```text
//!    BINOP de enteros   add sub mul udiv sdiv urem srem shl lshr ashr and
//!                       or xor
//!    CAST               trunc a i1, zext/sext de i1, fptosi fptoui sitofp
//!                       uitofp, bitcast entre i32 y float
//!    dx.op.binary.i32   IMax IMin UMax UMin (37..40)
//! ```
//!
//! Un `i1` es un [`Valor::Bool`]: 0xFFFFFFFF cierto, 0 falso -- el `true`
//! de LLVM es -1 (lo escribe con signo), y es lo que deja `Compara` --; asi
//! `and`/`or`/`xor` de bits son los de `i1`, y `sext` no hace nada. `zext`
//! deja 1 (`& 1`), y `trunc` a `i1` mira solo el bit de abajo.

use super::estructura::{bits, literal};
use super::programa::{Comparacion, Compilador, Conversion, NoPrograma, Op, OpEntera, Operandos, Valor};

const TYPE_INTEGER: u64 = 7;
const TYPE_NUMENTRY: u64 = 1;
const TYPE_STRUCT_NAME: u64 = 19;

/// El ancho de cada tipo entero por su id (0 si no es entero).
pub(super) fn anchos(m: &super::bits::Bloque) -> alloc::vec::Vec<u32> {
    let mut v = alloc::vec::Vec::new();
    if let Some(t) = m.hijo(17) {
        for r in &t.registros {
            match r.codigo {
                TYPE_NUMENTRY | TYPE_STRUCT_NAME => {}
                TYPE_INTEGER => v.push(r.ops.first().copied().unwrap_or(0) as u32),
                _ => v.push(0),
            }
        }
    }
    v
}

/// Si el BINOP de `o` es de enteros (su primer operando lo es).
pub(super) fn es_entero(c: &Compilador, o: &Operandos) -> Result<bool, NoPrograma> {
    let mut p = o.copia();
    let a = p.con_tipo()?;
    Ok(matches!(c.valores.get(a), Some(Valor::Bits(_) | Valor::Entero(_) | Valor::Bool(_))))
}

/// Un BINOP de enteros (`[a, b, opcode, banderas?]`).
pub(super) fn binop_entero(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let a = o.con_tipo()?;
    let b = o.solo()?;
    let opcode = o.crudo()?;
    let es_bool = |v: Option<&Valor>| matches!(v, Some(Valor::Bool(_)));
    let booleano = es_bool(c.valores.get(a)) || es_bool(c.valores.get(b));
    let (ra, rb) = (bits(c, a)?, bits(c, b)?);
    let d = c.registro(0.0)?;
    let op = match opcode {
        0 => {
            c.ops.push(Op::SumaEntera { d, a: ra, b: rb });
            c.valores.push(Valor::Bits(d));
            return Ok(());
        }
        1 => OpEntera::Resta,
        2 => OpEntera::Mul,
        3 => OpEntera::DivU,
        4 => OpEntera::DivS,
        5 => OpEntera::RemU,
        6 => OpEntera::RemS,
        7 => OpEntera::Shl,
        8 => OpEntera::ShrL,
        9 => OpEntera::ShrA,
        10 => OpEntera::Y,
        11 => OpEntera::O,
        12 => OpEntera::OX,
        _ => return Err(NoPrograma::Forma("un BINOP de enteros que no existe")),
    };
    c.ops.push(Op::Entera { d, a: ra, b: rb, op });
    let logico = matches!(op, OpEntera::Y | OpEntera::O | OpEntera::OX);
    c.valores.push(if booleano && logico { Valor::Bool(d) } else { Valor::Bits(d) });
    Ok(())
}

/// Un CAST (`[valor, tipo destino, opcode]`).
pub(super) fn cast(c: &mut Compilador, o: &mut Operandos, floats: &[bool], anchos: &[u32]) -> Result<(), NoPrograma> {
    let a = o.con_tipo()?;
    let destino = o.crudo()? as usize;
    let opcode = o.crudo()?;
    let origen = c.valores.get(a).copied();
    let ancho = anchos.get(destino).copied().unwrap_or(0);
    let a_float = floats.get(destino).copied().unwrap_or(false);
    let ra = bits(c, a)?;
    let es_bool = matches!(origen, Some(Valor::Bool(_)));
    let convierte = |c: &mut Compilador, x, como| -> Result<Valor, NoPrograma> {
        let d = c.registro(0.0)?;
        c.ops.push(Op::Convierte { d, a: x, como });
        Ok(if matches!(como, Conversion::EnteroAFloat | Conversion::SinSignoAFloat) { Valor::Float(d) } else { Valor::Bits(d) })
    };
    let v = match opcode {
        // trunc a i1: el bit de abajo, cierto o falso.
        0 if ancho == 1 => {
            let uno = literal(c, 1)?;
            let cero = literal(c, 0)?;
            let t = c.registro(0.0)?;
            c.ops.push(Op::Entera { d: t, a: ra, b: uno, op: OpEntera::Y });
            let d = c.registro(0.0)?;
            c.ops.push(Op::Compara { d, a: t, b: cero, como: Comparacion::Distinto, entero: true });
            Valor::Bool(d)
        }
        // zext de i1: 1 o 0. sext de i1: -1 o 0, como ya esta.
        1 if es_bool && ancho == 32 => {
            let uno = literal(c, 1)?;
            let d = c.registro(0.0)?;
            c.ops.push(Op::Entera { d, a: ra, b: uno, op: OpEntera::Y });
            Valor::Bits(d)
        }
        2 if es_bool && ancho == 32 => Valor::Bits(ra),
        3 => convierte(c, ra, Conversion::FloatASinSigno)?,
        4 => convierte(c, ra, Conversion::FloatAEntero)?,
        // uitofp de un i1: de 1, no de -1.
        5 if es_bool => {
            let uno = literal(c, 1)?;
            let t = c.registro(0.0)?;
            c.ops.push(Op::Entera { d: t, a: ra, b: uno, op: OpEntera::Y });
            convierte(c, t, Conversion::SinSignoAFloat)?
        }
        5 => convierte(c, ra, Conversion::SinSignoAFloat)?,
        6 => convierte(c, ra, Conversion::EnteroAFloat)?,
        // bitcast entre i32 y float: los mismos bits.
        11 if a_float => Valor::Float(ra),
        11 if ancho == 32 => Valor::Bits(ra),
        _ => return Err(NoPrograma::Forma("una conversion que no es de i1, de 32 bits ni entre float y entero: todavia no")),
    };
    c.valores.push(v);
    Ok(())
}

/// `dx.op.binary.i32`: IMax (37), IMin (38), UMax (39), UMin (40).
pub(super) fn min_max(c: &mut Compilador, op: i64, a: usize, b: usize) -> Result<Valor, NoPrograma> {
    let (ra, rb) = (bits(c, a)?, bits(c, b)?);
    let op = match op {
        37 => OpEntera::MaxS,
        38 => OpEntera::MinS,
        39 => OpEntera::MaxU,
        _ => OpEntera::MinU,
    };
    let d = c.registro(0.0)?;
    c.ops.push(Op::Entera { d, a: ra, b: rb, op });
    Ok(Valor::Bits(d))
}
