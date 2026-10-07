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
        // 19 (07-10): un struct, campo a campo; de enteros si todos lo son
        // (lo que se lea de un campo lo dice SU tipo: `apuntar`).
        Some(&Tipo::Estructura { n, campos }) => {
            let (mut total, mut enteros) = (0, true);
            for &t in &campos[..n as usize] {
                let (m, e) = forma(tipos, floats, anchos, t as usize)?;
                total += m;
                enteros &= e;
            }
            Ok((total, enteros))
        }
        _ if floats.get(t).copied().unwrap_or(false) => Ok((1, false)),
        _ if anchos.get(t).copied().unwrap_or(0) > 0 => Ok((1, true)),
        _ => Err(NoPrograma::Forma("un array de algo que no es float ni entero (structs, vectores): todavia no")),
    }
}

/// **El campo `k` de un struct** (19, 07-10): donde empieza (lo que miden
/// los de antes, aplanado) y su tipo.
fn campo(tipos: &[Tipo], floats: &[bool], anchos: &[u32], campos: &[u32], k: i64) -> Result<(usize, usize), NoPrograma> {
    let k = usize::try_from(k).ok().filter(|&k| k < campos.len()).ok_or(NoPrograma::Forma("un getelementptr a un campo que el struct no tiene"))?;
    let mut desde = 0;
    for &t in &campos[..k] {
        desde += forma(tipos, floats, anchos, t as usize)?.0;
    }
    Ok((desde, campos[k] as usize))
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
    if !matches!(tipos.get(tipo), Some(Tipo::Arreglo { .. } | Tipo::Estructura { .. })) {
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
pub(super) const CST_CE_GEP: u64 = 12;
pub(super) const CST_CE_INBOUNDS_GEP: u64 = 20;

/// **Un global** (`MODULE_CODE_GLOBALVAR`: [tipo, constante | explicito
/// << 1, inicial + 1, ...]): su array, con el inicial si lo tiene; o (14,
/// 07-10) un array de uno si es un numero suelto.
pub(super) fn global(c: &mut Compilador, ops: &[u64], tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<Valor, NoPrograma> {
    let (t, banderas, inicial) = (ops.first().copied().unwrap_or(0) as usize, ops.get(1).copied().unwrap_or(0), ops.get(2).copied().unwrap_or(0));
    // Sin el bit "explicito", el campo 0 es el tipo PUNTERO.
    let t = match (banderas & 2, tipos.get(t)) {
        (0, Some(&Tipo::Puntero { a })) => a,
        _ => t,
    };
    // 14 de la pila A (07-10): un global que es UN numero (`groupshared uint
    // suma;`, `static float x;`) es un array de uno; los demas (los
    // recursos, que son structs) no son nada aqui.
    let numero = floats.get(t).copied().unwrap_or(false) || anchos.get(t).copied().unwrap_or(0) > 0;
    if !matches!(tipos.get(t), Some(Tipo::Arreglo { .. } | Tipo::Estructura { .. })) && !numero {
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
    let v = apuntar(c, p, &indices, tipos, floats, anchos)?;
    c.valores.push(v);
    Ok(())
}

/// `CST_CODE_CE_GEP` (12) y `CST_CODE_CE_INBOUNDS_GEP` (20): un
/// getelementptr CONSTANTE (05-10, el `sharedPos[counter + 1]` desenrollado
/// de nBodyGravity: 384 asi). El registro es [tipo de lo apuntado si la
/// medida es impar, y pares (tipo, valor)]; los valores, ABSOLUTOS. LLVM
/// pone los enteros antes que los GEP en el bloque (sus indices ya estan).
pub(super) fn gep_constante(c: &mut Compilador, ops: &[u64], tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<Valor, NoPrograma> {
    let pares = &ops[ops.len() % 2..];
    let ids: Vec<usize> = pares.chunks_exact(2).map(|par| par[1] as usize).collect();
    let Some((&p, indices)) = ids.split_first() else {
        return Err(NoPrograma::Forma("un getelementptr constante sin puntero"));
    };
    apuntar(c, p, indices, tipos, floats, anchos)
}

/// El puntero de un getelementptr: `p` (un array de registros o de la
/// memoria compartida) por los `indices` (ids de valores).
fn apuntar(c: &mut Compilador, p: usize, indices: &[usize], tipos: &[Tipo], floats: &[bool], anchos: &[u32]) -> Result<Valor, NoPrograma> {
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
        // 19 (07-10): un campo de un struct: el indice es constante, y lo
        // que salta es lo que miden los campos de antes.
        if let Some(&Tipo::Estructura { n, campos }) = tipos.get(t) {
            let Some(Valor::Entero(k)) = c.valores.get(i).copied() else {
                return Err(NoPrograma::Forma("un getelementptr a un campo CALCULADO de un struct"));
            };
            let (desde, campo) = campo(tipos, floats, anchos, &campos[..n as usize], k)?;
            fijo += desde as i64;
            t = campo;
            continue;
        }
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
    // 19 (07-10): lo que se lee es del tipo de lo APUNTADO (un campo de un
    // struct puede ser entero en un struct de floats).
    let enteros = match tipos.get(t) {
        Some(Tipo::Arreglo { .. } | Tipo::Estructura { .. }) => enteros,
        _ => anchos.get(t).copied().unwrap_or(0) > 0,
    };
    Ok(if compartida { Valor::PunteroCompartido { base, n, i, enteros } } else { Valor::Puntero { base: base as Reg, n: n as u16, i, enteros } })
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

/// **`atomicrmw`** (38: [puntero con su tipo, valor, operacion, volatil,
/// orden, alcance]), 18 de la pila A (07-10): un `Interlocked*` sobre la
/// memoria compartida del grupo; da la palabra de antes. Las operaciones de
/// LLVM: 0 xchg, 1 add, 2 sub, 3 and, 5 or, 6 xor, 7 max, 8 min, 9 umax,
/// 10 umin (`sub` es sumar el negado; `nand` no lo escribe HLSL).
pub(super) fn atomico(c: &mut Compilador, o: &mut Operandos) -> Result<(), NoPrograma> {
    use crate::bufer::Atomo;
    let p = o.con_tipo()?;
    let v = o.solo()?;
    let que = o.crudo()?;
    let (compartida, base, n, i, _) = donde(c, p)?;
    if !compartida {
        return Err(NoPrograma::Forma("un atomicrmw de algo que no es la memoria compartida: todavia no"));
    }
    let mut rv = bits(c, v)?;
    let como = match que {
        0 => Atomo::Cambia,
        1 => Atomo::Suma,
        2 => {
            let (cero, d) = (literal(c, 0)?, c.registro(0.0)?);
            c.ops.push(Op::Entera { d, a: cero, b: rv, op: super::programa::OpEntera::Resta });
            rv = d;
            Atomo::Suma
        }
        3 => Atomo::Y,
        5 => Atomo::O,
        6 => Atomo::Xor,
        7 => Atomo::MaxConSigno,
        8 => Atomo::MinConSigno,
        9 => Atomo::MaxSinSigno,
        10 => Atomo::MinSinSigno,
        _ => return Err(NoPrograma::Forma("un atomicrmw que HLSL no escribe (nand)")),
    };
    let d = c.registro(0.0)?;
    c.ops.push(Op::AtomicoCompartido { d, base, n, i, v: rv, como });
    c.valores.push(Valor::Bits(d));
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dxil::programa::CAMPOS;

    fn estructura(campos: &[u32]) -> Tipo {
        let mut c = [0u32; CAMPOS];
        c[..campos.len()].copy_from_slice(campos);
        Tipo::Estructura { n: campos.len() as u8, campos: c }
    }

    /// 19 de la pila A (07-10): `struct { float3 pos; uint id; } [64]`, y
    /// un `float2 [64]` (un vector se aplana como un array). Los tipos: 0
    /// float, 1 i32, 2 <3 x float>, 3 el struct, 4 [64 x struct], 5 <2 x
    /// float>, 6 [64 x <2 x float>], 7 un handle (`{ i8* }`: un puntero, ni
    /// float ni entero).
    #[test]
    fn un_array_de_structs_y_de_vectores_se_aplana() {
        let tipos = [
            Tipo::Otro,
            Tipo::Otro,
            Tipo::Arreglo { n: 3, elem: 0 },
            estructura(&[2, 1]),
            Tipo::Arreglo { n: 64, elem: 3 },
            Tipo::Arreglo { n: 2, elem: 0 },
            Tipo::Arreglo { n: 64, elem: 5 },
            estructura(&[8]),
            Tipo::Puntero { a: 1 },
        ];
        let floats = [true, false, false, false, false, false, false, false, false];
        let anchos = [0, 32, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(forma(&tipos, &floats, &anchos, 3).ok(), Some((4, false)), "float3 + uint: 4, mezclado");
        assert_eq!(forma(&tipos, &floats, &anchos, 4).ok(), Some((256, false)));
        assert_eq!(forma(&tipos, &floats, &anchos, 6).ok(), Some((128, false)), "un float2 son 2 floats");
        assert_eq!(campo(&tipos, &floats, &anchos, &[2, 1], 1).ok(), Some((3, 1)), "id va detras de los 3 de pos, y es entero");
        assert_eq!(campo(&tipos, &floats, &anchos, &[2, 1], 0).ok(), Some((0, 2)));
        assert!(campo(&tipos, &floats, &anchos, &[2, 1], 2).is_err(), "no tiene tercer campo");
        // La prueba que dice NO: un struct con un puntero dentro no se aplana.
        assert!(forma(&tipos, &floats, &anchos, 7).is_err());
    }
}
