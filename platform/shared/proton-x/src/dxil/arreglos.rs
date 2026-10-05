//! **Los arrays del DXIL** (03-10, N5.10): `alloca`, `getelementptr`,
//! `load` y `store`, y los arrays GLOBALES constantes (las tablas de un
//! sombreador: `static const float pesos[4] = {...}`). La quinta, sexta y
//! septima corrida de Cyberpunk lo pedian en vertices y pixeles
//! (`Instruccion(19)` y `(43)`).
//!
//! ```text
//!    alloca [N x T]          N registros seguidos, a cero
//!    @global = constant ..   N registros seguidos, con su inicial
//!    getelementptr A, 0, i   un PUNTERO: el array y el indice (en un
//!                            registro: constante o calculado; los de
//!                            arrays de arrays, aplanados: i * filas + j)
//!    load  / store           `Op::LeeIndexado` / `Op::EscribeIndexado`
//! ```
//!
//! Todo acceso va por las dos operaciones indexadas, tambien el de indice
//! constante: el registro del array cambia (no es SSA), y asi ni el
//! emisor de la 3060 ni el x86-64 lo confunden con una constante -- esos
//! sombreadores van por el interprete (lo dice `NoEmite::Operacion`). Fuera
//! del array se lee 0 y no se escribe (en D3D es indefinido).

use alloc::vec::Vec;

use super::estructura::{bits, literal};
use super::programa::{Compilador, NoPrograma, Op, Operandos, Reg, Tipo, Valor};

/// Lo que mide un array (en elementos, aplanado) y si son enteros.
fn forma(tipos: &[Tipo], floats: &[bool], anchos: &[u32], t: usize) -> Result<(usize, bool), NoPrograma> {
    match tipos.get(t) {
        Some(&Tipo::Arreglo { n, elem }) => {
            let (m, enteros) = forma(tipos, floats, anchos, elem)?;
            Ok((n * m, enteros))
        }
        _ if floats.get(t).copied().unwrap_or(false) => Ok((1, false)),
        _ if anchos.get(t).copied().unwrap_or(0) > 0 => Ok((1, true)),
        _ => Err(NoPrograma::Forma("un array de algo que no es float ni entero (structs, vectores): todavia no")),
    }
}

/// N registros seguidos con sus iniciales.
fn reservar(c: &mut Compilador, iniciales: &[f32]) -> Result<Reg, NoPrograma> {
    if iniciales.len() > 4096 {
        return Err(NoPrograma::Forma("un array de mas de 4096 elementos: todavia no"));
    }
    let base = c.registro(iniciales.first().copied().unwrap_or(0.0))?;
    for &x in iniciales.iter().skip(1) {
        c.registro(x)?;
    }
    Ok(base)
}

/// Un array nuevo, del tipo `t` (aplanado), con estos iniciales o a cero.
fn nuevo(c: &mut Compilador, tipos: &[Tipo], floats: &[bool], anchos: &[u32], t: usize, iniciales: Option<Vec<f32>>) -> Result<Valor, NoPrograma> {
    let (n, enteros) = forma(tipos, floats, anchos, t)?;
    let mut v = iniciales.unwrap_or_default();
    v.resize(n.max(1), 0.0);
    let base = reservar(c, &v)?;
    Ok(Valor::Arreglo { base, n: n as u16, enteros, tipo: t as u32 })
}

/// **Una constante de array** del CONSTANTS_BLOCK: `CST_CODE_DATA` (22:
/// los bits de cada elemento) o `CST_CODE_AGGREGATE` (7: ids de otras
/// constantes, que ya deben estar).
pub(super) fn constante(c: &mut Compilador, codigo: u64, ops: &[u64], tipo: usize, tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<Valor, NoPrograma> {
    if !matches!(tipos.get(tipo), Some(Tipo::Arreglo { .. })) {
        return Ok(Valor::Nada);
    }
    let iniciales: Vec<f32> = if codigo == CST_DATA {
        ops.iter().map(|&b| f32::from_bits(b as u32)).collect()
    } else {
        let mut v = Vec::with_capacity(ops.len());
        for &id in ops {
            v.push(match c.valores.get(id as usize).copied() {
                Some(Valor::Float(r)) => c.iniciales[r as usize],
                Some(Valor::Entero(e)) => f32::from_bits(e as i32 as u32),
                Some(Valor::Arreglo { base, n, .. }) => {
                    // Un array de arrays: lo de dentro, seguido.
                    v.extend_from_slice(&c.iniciales[base as usize..base as usize + n as usize]);
                    continue;
                }
                Some(Valor::Nada | Valor::Indefinido) | None => 0.0,
                _ => return Err(NoPrograma::Forma("un array constante con algo que no es un numero")),
            });
        }
        v
    };
    nuevo(c, tipos, floats, anchos, tipo, Some(iniciales))
}

pub(super) const CST_AGGREGATE: u64 = 7;
/// El `addrspace` de `groupshared` en DXIL.
const ESPACIO_COMPARTIDO: u64 = 3;
pub(super) const CST_DATA: u64 = 22;

/// **Un global** (`MODULE_CODE_GLOBALVAR`: [tipo, constante | explicito
/// << 1, inicial + 1, ...]): su array, con el inicial si lo tiene.
pub(super) fn global(c: &mut Compilador, ops: &[u64], tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<Valor, NoPrograma> {
    let (t, banderas, inicial) = (ops.first().copied().unwrap_or(0) as usize, ops.get(1).copied().unwrap_or(0), ops.get(2).copied().unwrap_or(0));
    // Sin el bit "explicito", el campo 0 es el tipo PUNTERO.
    let t = match (banderas & 2, tipos.get(t)) {
        (0, Some(&Tipo::Puntero { a })) => a,
        _ => t,
    };
    if !matches!(tipos.get(t), Some(Tipo::Arreglo { .. })) {
        return Ok(Valor::Nada);
    }
    // N5.5 (05-10): `addrspace(3)` (los bits de arriba de las banderas) es la
    // memoria COMPARTIDA del grupo (`groupshared`): no son registros del
    // hilo, sino palabras que ven todos los hilos del grupo.
    if banderas >> 2 == ESPACIO_COMPARTIDO {
        let (n, enteros) = forma(tipos, floats, anchos, t)?;
        let base = c.compartida;
        // D3D12: 32 KiB de memoria compartida por grupo.
        if base as usize + n > 8192 {
            return Err(NoPrograma::Forma("mas de 32 KiB de memoria compartida (groupshared): D3D12 no lo deja"));
        }
        c.compartida += n as u32;
        return Ok(Valor::Compartida { base, n: n as u32, enteros, tipo: t as u32 });
    }
    let v = if inicial == 0 { None } else { c.valores.get(inicial as usize - 1).copied() };
    match v {
        Some(a @ Valor::Arreglo { .. }) => Ok(a),
        // Sin inicial o con `zeroinitializer`: a cero.
        _ => nuevo(c, tipos, floats, anchos, t, None),
    }
}

/// `alloca [N x T]` (19: [tipo, tipo de la medida, medida, alineacion]).
pub(super) fn alloca(c: &mut Compilador, ops: &[u64], tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<(), NoPrograma> {
    let t = ops.first().copied().unwrap_or(0) as usize;
    let v = nuevo(c, tipos, floats, anchos, t, None)?;
    c.valores.push(v);
    Ok(())
}

/// `getelementptr inbounds T, T* p, 0, i (, j...)` (43: [inbounds, tipo,
/// operandos con su tipo...]): el indice aplanado, en un registro.
pub(super) fn gep(c: &mut Compilador, o: &mut Operandos, tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<(), NoPrograma> {
    let _inbounds = o.crudo()?;
    let _tipo = o.crudo()?;
    let p = o.con_tipo()?;
    let mut indices = Vec::new();
    while o.i < o.ops.len() {
        indices.push(o.con_tipo()?);
    }
    // El array de registros, o (N5.5) el de la memoria compartida.
    let (compartida, base, n, enteros, tipo) = match c.valores.get(p).copied() {
        Some(Valor::Arreglo { base, n, enteros, tipo }) => (false, base as u32, n as u32, enteros, tipo),
        Some(Valor::Compartida { base, n, enteros, tipo }) => (true, base, n, enteros, tipo),
        _ => return Err(NoPrograma::Forma("un getelementptr de algo que no es un array (un puntero de un puntero): todavia no")),
    };
    // El primero salta arrays enteros: tiene que ser 0.
    if !matches!(indices.first().and_then(|&i| c.valores.get(i)), Some(Valor::Entero(0))) {
        return Err(NoPrograma::Forma("un getelementptr que no empieza por 0"));
    }
    // Los demas bajan por los arrays: cada uno por lo que mide lo de dentro.
    let mut t = tipo as usize;
    let mut total: Option<Reg> = None;
    let mut fijo = 0i64;
    for &i in &indices[1..] {
        let Some(&Tipo::Arreglo { elem, .. }) = tipos.get(t) else {
            return Err(NoPrograma::Forma("un getelementptr que baja mas hondo que el array"));
        };
        let paso = forma(tipos, floats, anchos, elem)?.0 as i64;
        match c.valores.get(i).copied() {
            Some(Valor::Entero(k)) => fijo += k * paso,
            _ => {
                let r = bits(c, i)?;
                let r = if paso == 1 {
                    r
                } else {
                    let (lp, d) = (literal(c, paso as u32)?, c.registro(0.0)?);
                    c.ops.push(Op::Entera { d, a: r, b: lp, op: super::programa::OpEntera::Mul });
                    d
                };
                total = Some(match total {
                    None => r,
                    Some(a) => {
                        let d = c.registro(0.0)?;
                        c.ops.push(Op::SumaEntera { d, a, b: r });
                        d
                    }
                });
            }
        }
        t = elem;
    }
    // Lo fijo, sumado a lo calculado (o solo, como un literal).
    let lf = literal(c, fijo as i32 as u32)?;
    let i = match total {
        None => lf,
        Some(r) if fijo == 0 => r,
        Some(r) => {
            let d = c.registro(0.0)?;
            c.ops.push(Op::SumaEntera { d, a: r, b: lf });
            d
        }
    };
    c.valores.push(if compartida { Valor::PunteroCompartido { base, n, i, enteros } } else { Valor::Puntero { base: base as Reg, n: n as u16, i, enteros } });
    Ok(())
}

/// El array y el indice de un puntero (un array solo es su elemento 0), y
/// si es de la memoria compartida (N5.5).
fn donde(c: &mut Compilador, p: usize) -> Result<(bool, u32, u32, Reg, bool), NoPrograma> {
    match c.valores.get(p).copied() {
        Some(Valor::Puntero { base, n, i, enteros }) => Ok((false, base as u32, n as u32, i, enteros)),
        Some(Valor::Arreglo { base, n, enteros, .. }) => Ok((false, base as u32, n as u32, literal(c, 0)?, enteros)),
        Some(Valor::PunteroCompartido { base, n, i, enteros }) => Ok((true, base, n, i, enteros)),
        Some(Valor::Compartida { base, n, enteros, .. }) => Ok((true, base, n, literal(c, 0)?, enteros)),
        _ => Err(NoPrograma::Forma("un load o un store de algo que no es un array")),
    }
}

/// `load T, T* p` (20: [puntero con su tipo, tipo, alineacion, volatil]).
pub(super) fn load(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let p = o.con_tipo()?;
    let (compartida, base, n, i, enteros) = donde(c, p)?;
    let d = c.registro(0.0)?;
    c.ops.push(if compartida { Op::LeeCompartida { d, base, n, i } } else { Op::LeeIndexado { d, base: base as Reg, n: n as u16, i } });
    c.valores.push(if enteros { Valor::Bits(d) } else { Valor::Float(d) });
    Ok(())
}

/// `store T v, T* p` (44: [puntero con su tipo, valor con su tipo, ...]).
pub(super) fn store(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    let p = o.con_tipo()?;
    let v = o.con_tipo()?;
    let (compartida, base, n, i, _) = donde(c, p)?;
    let s = bits(c, v)?;
    c.ops.push(if compartida { Op::EscribeCompartida { base, n, i, s } } else { Op::EscribeIndexado { base: base as Reg, n: n as u16, i, s } });
    Ok(())
}
