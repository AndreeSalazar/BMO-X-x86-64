//! **Las olas, leidas del DXIL** (E2.5, 05-10): las operaciones `Wave*` y
//! `Quad*` del modelo 6.0 y las derivadas (`dx.op` 83-86, 110-123, 135-136)
//! vueltas una [`Op::Ola`] del Programa de la casa.
//!
//! Lo que HACE cada una -- los carriles, las derivadas de su cuadro, los
//! gradientes de un muestreo -- vive en PROMETEO desde el 08-10 (LB3b de
//! `docs/plan/PLAN_LAS_LIBRERIAS.md`): `platform/shared/prometeo/src/olas.rs`,
//! re-exportado aqui en la ruta de siempre.

use super::estructura::{bits, literal};
use super::programa::{Compilador, NoPrograma, Op, Reg, Valor};
// Lo que hacen las olas, de PROMETEO, en la ruta de siempre (LB3b).
pub use bmo_prometeo::olas::*;

const DERIV_PRIMERA: i64 = 83;
const DERIV_ULTIMA: i64 = 86;
const ES_PRIMER_CARRIL: i64 = 110;
const INDICE_DE_CARRIL: i64 = 111;
const CARRILES_DE_OLA: i64 = 112;
const ALGUNO_CIERTO: i64 = 113;
const TODOS_CIERTOS: i64 = 114;
const TODOS_IGUALES: i64 = 115;
const PAPELETA: i64 = 116;
const LEER_CARRIL: i64 = 117;
const LEER_PRIMERO: i64 = 118;
const OP_DE_OLA: i64 = 119;
const BITS_DE_OLA: i64 = 120;
const PREFIJO: i64 = 121;
const LEER_DEL_CUADRO: i64 = 122;
const CRUZA_EL_CUADRO: i64 = 123;
const CUENTA_DE_BITS: i64 = 135;
const CUENTA_DE_PREFIJO: i64 = 136;

/// **La operacion de D3D `op` si es de olas o una derivada** (`None` si
/// no): una [`Op::Ola`] (las derivadas tambien, D4.4), o -- `WaveGetLaneCount`
/// -- una constante. `nombre` es el de la funcion (`dx.op.waveActiveOp.f32`): su
/// sobrecarga dice si son floats.
pub(super) fn de(c: &mut Compilador, op: i64, args: &[usize], nombre: &str) -> Option<Result<Valor, NoPrograma>> {
    if !(DERIV_PRIMERA..=DERIV_ULTIMA).contains(&op) && !(ES_PRIMER_CARRIL..=CRUZA_EL_CUADRO).contains(&op) && !(CUENTA_DE_BITS..=CUENTA_DE_PREFIJO).contains(&op) {
        return None;
    }
    Some(compilar(c, op, args, nombre))
}

fn compilar(c: &mut Compilador, op: i64, args: &[usize], nombre: &str) -> Result<Valor, NoPrograma> {
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de olas con menos argumentos"));
    let float = nombre.ends_with(".f32");
    if [".f16", ".f64", ".i16", ".i64"].iter().any(|s| nombre.ends_with(s)) {
        return Err(NoPrograma::Forma("una operacion de olas de 16 o 64 bits: todavia no"));
    }
    // Una constante i8 (la operacion, el signo); `undef`, 0.
    let chico = |c: &Compilador, k: usize| -> Result<u8, NoPrograma> {
        match c.valores.get(arg(k)?) {
            Some(Valor::Entero(v)) if (0..4).contains(v) => Ok(*v as u8),
            Some(Valor::Indefinido) => Ok(0),
            _ => Err(NoPrograma::Forma("una operacion de olas con su clase calculada")),
        }
    };
    let num = |c: &Compilador, k: usize| -> Result<Numero, NoPrograma> {
        Ok(if float {
            Numero::Float
        } else if chico(c, k)? == 1 {
            Numero::SinSigno
        } else {
            Numero::Entero
        })
    };
    let que = match op {
        // D4.4: las derivadas, de verdad (83 y 84 gruesas, 85 y 86 finas; las
        // pares, las de y). El numero de carriles, constante.
        DERIV_PRIMERA..=DERIV_ULTIMA => Ola::Derivada { y: (op - DERIV_PRIMERA) % 2 == 1, fina: op >= DERIV_PRIMERA + 2, muestra: false },
        CARRILES_DE_OLA => return literal(c, CARRILES).map(Valor::Bits),
        ES_PRIMER_CARRIL => Ola::EsPrimero,
        INDICE_DE_CARRIL => Ola::Indice,
        ALGUNO_CIERTO => Ola::AlgunoCierto,
        TODOS_CIERTOS => Ola::TodosCiertos,
        TODOS_IGUALES => Ola::TodosIguales { float },
        PAPELETA => Ola::Papeleta,
        LEER_CARRIL => Ola::LeerCarril,
        LEER_PRIMERO => Ola::LeerPrimero,
        OP_DE_OLA => Ola::Activa { op: chico(c, 2)?, num: num(c, 3)? },
        BITS_DE_OLA => Ola::Bits { op: chico(c, 2)? },
        PREFIJO => Ola::Prefijo { op: chico(c, 2)? & 1, num: num(c, 3)? },
        LEER_DEL_CUADRO => Ola::Cuadro,
        CRUZA_EL_CUADRO => match chico(c, 2)? {
            k @ 0..=2 => Ola::Cruza(k + 1),
            _ => return Err(NoPrograma::Forma("un QuadOp que no es X, Y ni la diagonal")),
        },
        CUENTA_DE_BITS => Ola::CuentaBits,
        CUENTA_DE_PREFIJO => Ola::PrefijoBits,
        _ => return Err(NoPrograma::OperacionD3d(op)),
    };
    // El valor (las que no lo llevan, cualquiera: 0) y el segundo operando.
    let a = match args.get(1) {
        Some(&k) => bits(c, k)?,
        None => literal(c, 0)?,
    };
    let b = match que {
        Ola::LeerCarril | Ola::Cuadro => bits(c, arg(2)?)?,
        _ => a,
    };
    let d = c.registro(0.0)?;
    for _ in 1..anchura(que) {
        c.registro(0.0)?;
    }
    c.ops.push(Op::Ola { d, a, b, que });
    // Lo que devuelve: un booleano, un uint4, un entero, o del tipo del valor.
    Ok(match que {
        Ola::EsPrimero | Ola::AlgunoCierto | Ola::TodosCiertos | Ola::TodosIguales { .. } => Valor::Bool(d),
        Ola::Papeleta => Valor::CuatroEnteros(d),
        Ola::Indice | Ola::CuentaBits | Ola::PrefijoBits => Valor::Bits(d),
        _ if float => Valor::Float(d),
        _ if nombre.ends_with(".i1") => Valor::Bool(d),
        _ => Valor::Bits(d),
    })
}

/// **D4.4: el bloque de un muestreo con la mip por gradientes** (ver
/// `Lectura::Gradientes`), y su primer registro: los cuatro gradientes de
/// (u, v) = (`co[0]`, `co[1]`) -- los que da el sombreador (`grad`: ddx u,
/// ddx v, ddy u, ddy v) o los de su cuadro (`olas::gradientes`) -- y detras,
/// copiado, cada uno de `resto` (el sesgo, el clamp, la referencia; uno que
/// no viene, 0).
pub(super) fn bloque(c: &mut Compilador, co: [Reg; 4], grad: Option<[usize; 4]>, resto: &[Option<usize>]) -> Result<Reg, NoPrograma> {
    let g = c.registro(0.0)?;
    for _ in 1..4 + resto.len() {
        c.registro(0.0)?;
    }
    match grad {
        Some(ids) => {
            for (k, id) in ids.into_iter().enumerate() {
                let a = bits(c, id)?;
                c.ops.push(Op::Copia { d: g + k as Reg, a });
            }
        }
        None => gradientes(&mut c.ops, g, co[0], co[1]),
    }
    for (k, id) in resto.iter().enumerate() {
        let a = match id {
            Some(id) => bits(c, *id)?,
            None => literal(c, 0)?,
        };
        c.ops.push(Op::Copia { d: g + 4 + k as Reg, a });
    }
    Ok(g)
}
