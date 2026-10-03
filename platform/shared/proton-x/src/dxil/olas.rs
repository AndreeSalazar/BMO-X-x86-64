//! **Las olas y las derivadas, con un pixel por ola** (03-10): lo que los
//! sombreadores de Cyberpunk pedian en el metal (OperacionD3d 83, 84 y 118
//! en la quinta corrida) y sus hermanas.
//!
//! La casa corre cada pixel (y cada vertice) SOLO: una ola de un carril. Lo
//! que una GPU reparte entre 32 carriles aqui es lo que da un carril:
//!
//! ```text
//!    ReadLaneFirst, ReadLaneAt, ActiveOp     el valor mismo (la suma, el
//!    (suma, producto, min, max), ActiveBit   producto, el min de uno: el)
//!    AnyTrue, AllTrue                        el booleano mismo
//!    IsFirstLane, ActiveAllEqual             cierto
//!    GetLaneIndex 0, GetLaneCount 1          un carril
//!    PrefixOp                                el neutro: 0 la suma, 1 el
//!                                            producto (no hay nadie antes)
//!    AllBitCount 1 o 0, PrefixBitCount 0
//!    DerivCoarse/Fine X e Y (ddx, ddy)       0: sin el cuadro de 2x2, el
//!                                            pixel no ve a su vecino
//! ```
//!
//! Los juegos usan las olas para ESCALARIZAR lo que ya es igual en toda la
//! ola (un indice de material, un booleano de "toda la ola entra"): con un
//! carril es exacto. Las derivadas a 0 son la aproximacion: el muestreo ya
//! lee la mip de la vista sin mirarlas (`Lectura::Muestra`), y `fwidth` da 0
//! (un borde suavizado sale duro). Con el cuadro de 2x2 en la trama se
//! calcularan de verdad.

use super::estructura::{bits, literal};
use super::programa::{Compilador, NoPrograma, Op, Valor};

const DERIV_PRIMERA: i64 = 83;
const DERIV_ULTIMA: i64 = 86;
const ES_PRIMER_CARRIL: i64 = 110;
const INDICE_DE_CARRIL: i64 = 111;
const CARRILES: i64 = 112;
const ALGUNO_CIERTO: i64 = 113;
const TODOS_CIERTOS: i64 = 114;
const TODOS_IGUALES: i64 = 115;
const LEER_CARRIL: i64 = 117;
const LEER_PRIMERO: i64 = 118;
const OP_DE_OLA: i64 = 119;
const BITS_DE_OLA: i64 = 120;
const PREFIJO: i64 = 121;
const CUENTA_DE_BITS: i64 = 135;
const CUENTA_DE_PREFIJO: i64 = 136;

/// La operacion de D3D `op` si es de olas o una derivada; `None` si no.
pub(super) fn de_un_carril(c: &mut Compilador, op: i64, args: &[usize]) -> Option<Result<Valor, NoPrograma>> {
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de olas con menos argumentos"));
    let el_mismo = |c: &Compilador| -> Result<Valor, NoPrograma> {
        match c.valores.get(arg(1)?).copied() {
            Some(v @ (Valor::Float(_) | Valor::Bits(_) | Valor::Bool(_) | Valor::Entero(_))) => Ok(v),
            Some(Valor::Indefinido) => Ok(Valor::Entero(0)),
            _ => Err(NoPrograma::Forma("una operacion de olas sobre algo que no es un numero")),
        }
    };
    Some(match op {
        DERIV_PRIMERA..=DERIV_ULTIMA => c.registro(0.0).map(Valor::Float),
        ES_PRIMER_CARRIL | TODOS_IGUALES => literal(c, u32::MAX).map(Valor::Bool),
        INDICE_DE_CARRIL | CUENTA_DE_PREFIJO => Ok(Valor::Entero(0)),
        CARRILES => Ok(Valor::Entero(1)),
        ALGUNO_CIERTO | TODOS_CIERTOS | LEER_CARRIL | LEER_PRIMERO | OP_DE_OLA | BITS_DE_OLA => el_mismo(c),
        // El neutro de la suma (0) o del producto (1), del tipo del valor.
        PREFIJO => (|| {
            let v = el_mismo(c)?;
            let producto = matches!(c.valores.get(arg(2)?), Some(Valor::Entero(1)));
            Ok(match (v, producto) {
                (Valor::Float(_), false) => Valor::Float(c.registro(0.0)?),
                (Valor::Float(_), true) => Valor::Float(c.registro(1.0)?),
                (_, p) => Valor::Entero(p as i64),
            })
        })(),
        // 1 si el booleano es cierto, 0 si no.
        CUENTA_DE_BITS => (|| {
            let b = bits(c, arg(1)?)?;
            let (uno, cero) = (literal(c, 1)?, literal(c, 0)?);
            let d = c.registro(0.0)?;
            c.ops.push(Op::Elige { d, c: b, a: uno, b: cero });
            Ok(Valor::Bits(d))
        })(),
        _ => return None,
    })
}
