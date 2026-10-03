//! **Muestrear para las sombras y juntar** (03-10): `SampleCmp` (64),
//! `SampleCmpLevelZero` (65), `TextureGather` (73) y `TextureGatherCmp`
//! (74) -- la octava corrida de Cyberpunk pidio la 65 y la 73 (el PCF de
//! los mapas de sombras y los filtros que leen 2x2 de una vez).
//!
//! ```text
//!    SampleCmp(LevelZero)  (srv, sampler, c0..c3, o0..o2, referencia[, clamp])
//!    TextureGather         (srv, sampler, c0..c3, o0, o1, canal)
//!    TextureGatherCmp      (srv, sampler, c0..c3, o0, o1, canal, referencia)
//! ```
//!
//! Las cuentas, en `textura.rs` (`juntar`, `comparar`); aqui solo se leen
//! los argumentos y sale un [`Op::Lee`]. La comparacion, sin mips: la de
//! la vista (como `SampleCmpLevelZero`, que es la que usan las sombras).

use super::estructura::{bits, literal};
use super::programa::{Compilador, Lectura, NoPrograma, Op, Valor};

const SAMPLE_CMP: i64 = 64;
const SAMPLE_CMP_NIVEL_CERO: i64 = 65;
const GATHER: i64 = 73;
const GATHER_CMP: i64 = 74;

/// La operacion `op` si es una de estas; `None` si no.
pub(super) fn de(c: &mut Compilador, op: i64, args: &[usize]) -> Option<Result<Valor, NoPrograma>> {
    if !matches!(op, SAMPLE_CMP | SAMPLE_CMP_NIVEL_CERO | GATHER | GATHER_CMP) {
        return None;
    }
    Some((|| {
        let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("un Gather o un SampleCmp con menos argumentos"));
        let (Some(Valor::Textura(t)), Some(Valor::Muestreador(s))) = (c.valores.get(arg(1)?).copied(), c.valores.get(arg(2)?).copied()) else {
            return Err(NoPrograma::Forma("un Gather o un SampleCmp sin el handle de una textura y el de un muestreador"));
        };
        let co = [bits(c, arg(3)?)?, bits(c, arg(4)?)?, bits(c, arg(5)?)?, bits(c, arg(6)?)?];
        let gather = matches!(op, GATHER | GATHER_CMP);
        let ids_desp = if gather { [arg(7)?, arg(8)?, usize::MAX] } else { [arg(7)?, arg(8)?, arg(9)?] };
        let mut desp = [0i8; 3];
        for (k, id) in ids_desp.into_iter().enumerate() {
            desp[k] = match c.valores.get(id) {
                Some(Valor::Entero(o)) if (-8..8).contains(o) => *o as i8,
                Some(Valor::Indefinido) | None => 0,
                _ => return Err(NoPrograma::Forma("un desplazamiento de Gather o SampleCmp que no es una constante")),
            };
        }
        let canal = |c: &Compilador| match c.valores.get(arg(9).unwrap_or(usize::MAX)) {
            Some(Valor::Entero(k)) if (0..4).contains(k) => Ok(*k as u8),
            _ => Err(NoPrograma::Forma("un Gather con un canal que no es una constante de 0 a 3")),
        };
        let (como, nivel) = match op {
            GATHER => (Lectura::Junta { canal: canal(c)? }, literal(c, 0)?),
            GATHER_CMP => (Lectura::JuntaCompara { canal: canal(c)? }, bits(c, arg(10)?)?),
            _ => (Lectura::Compara, bits(c, arg(10)?)?),
        };
        let d = c.registro(0.0)?;
        for _ in 0..3 {
            c.registro(0.0)?;
        }
        c.ops.push(Op::Lee { d, t, s, como, c: co, nivel, desp });
        Ok(Valor::Cuatro(d))
    })())
}
