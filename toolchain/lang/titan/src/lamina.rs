//! `lamina` -- LA LAMINA de TITAN++ (A4b de `docs/plan/EL_FOCO.md`, 14-10):
//! RAM que el programa le pide al kernel por su UNA puerta (`e1/ram.rs`), con
//! UN propietario.
//!
//! El propietario, 14-10: *"RAM = convertir como LAMINA ABSOLUTA, LAMINA ese
//! mismo para administrar y llamar, no quiero tener que lidiar como hizo C
//! con C++"*. Lo que en C y C++ se escribe a mano y se rompe, aqui no se
//! puede escribir:
//!
//! ```text
//!    let mut l = lamina(4096)   pide 4096 bytes; si el kernel no los da, el
//!                               programa lo dice (T0060 al correr)
//!    l[i] = byte(7)   l[i]      sus celdas son bytes, como las de una [byte]
//!    len(l)                     cuantos; fuera de ellos, T0072
//!    f(mut l)   f(take l)       se PRESTA o se ENTREGA (nivel 7)
//!    al morir su propietario    se suelta sola (MEM_OP_SOLTAR): sin free,
//!                               y nadie la suelta dos veces
//! ```
//!
//! Y lo que NO, todo T0077 (los modos de prestar y entregar): copiarla --
//! `let b = l`, pasarla sin `mut` ni `take`, imprimirla, meterla en una
//! tabla, una lista, un mapa, un registro o un caso --, y devolverla de una
//! fn (aun: va con el dia que una lamina se OFREZCA, `director.ofrece`).
//!
//! ** Por que T0077 y no un codigo nuevo: es la misma pregunta que ya
//! contesta -- como llega un valor a quien lo usa --. Una lamina solo tiene
//! dos respuestas, prestada o entregada; la copia no existe.
//!
//! ** Este fichero lo llaman tres: `check.rs` (las firmas y los tipos),
//! `calc/clase.rs` (cada valor, sabiendo la clase de cada local) y `gpu.rs`
//! (una gpu fn no pide RAM). La regla vive aqui una vez.

use crate::calc::Class;
use crate::ir::Value;
use crate::message::{Code, Message};
use crate::tree::{Mode, Param, Ty};

/// Hay una lamina en este tipo, a cualquier profundidad?
pub fn en(t: &Ty) -> bool {
    match t {
        Ty::Lamina => true,
        Ty::Table(i, _) | Ty::List(i) | Ty::Opt(i) => en(i),
        Ty::Map(k, v) => en(k) || en(v),
        _ => false,
    }
}

/// Hay una lamina DENTRO de esta clase (no ella misma)?
pub fn dentro(c: &Class) -> bool {
    match c {
        Class::Table(i, _) | Class::List(i) | Class::Opt(i) => matches!(**i, Class::Lamina) || dentro(i),
        Class::Map(k, v) => matches!(**k, Class::Lamina) || matches!(**v, Class::Lamina) || dentro(k) || dentro(v),
        _ => false,
    }
}

fn no(line: usize, col: usize, what: &str, how: &str) -> Message {
    Message::new(Code::Mode, line, col, what, "una lamina es RAM del kernel con UN propietario: se presta (`mut`) o se entrega (`take`), y nunca se copia", how)
}

/// T0077: una lamina dentro de un tipo -- el campo de un registro, lo que
/// lleva un caso, la celda de una tabla, una lista o un mapa --.
pub fn no_en_un_tipo(t: &Ty, line: usize, col: usize) -> Result<(), Message> {
    if matches!(t, Ty::Lamina) {
        return Err(no(line, col, "una lamina no va dentro de otro valor", "la lamina vive en un `let` o en un parametro con `mut` o `take`"));
    }
    if en(t) {
        return Err(no(line, col, &format!("`{}` lleva una lamina dentro", t.name()), "la lamina vive en un `let` o en un parametro con `mut` o `take`"));
    }
    Ok(())
}

/// La FIRMA de una fn: devolver una lamina, o recibirla como copia, no.
pub fn firma(ret: Option<&Ty>, params: &[Param], line: usize, col: usize) -> Result<(), Message> {
    if let Some(t) = ret {
        if en(t) {
            return Err(no(line, col, "esta fn devolveria una lamina", "pidela con `mut` y llenala ahi: fn llena(mut l: lamina)"));
        }
    }
    for p in params {
        if matches!(p.ty, Ty::Lamina) {
            if p.mode == Mode::Copy {
                return Err(no(p.line, p.col, &format!("`{}: lamina` la recibiria como COPIA", p.name), &format!("prestada: mut {}: lamina  --  o entregada: take {}: lamina", p.name, p.name)));
            }
        } else {
            no_en_un_tipo(&p.ty, p.line, p.col)?;
        }
    }
    Ok(())
}

/// **Ningun valor COPIA una lamina.** Un local de clase `lamina` solo puede
/// aparecer como la base de `l[i]`, dentro de `len(l)`, o prestado o
/// entregado a una llamada (`Value::Lend`); en cualquier otro sitio --un
/// `let`, un `print`, una tabla, un argumento sin modo, un `return`-- seria
/// una copia.
pub fn no_se_copia(v: &Value, known: &[Option<Class>], names: &dyn Fn(usize) -> String) -> Result<(), Message> {
    let es = |l: usize| matches!(known.get(l), Some(Some(Class::Lamina)));
    match v {
        Value::Local(l, at) if es(*l) => {
            let n = names(*l);
            Err(no(at.0, at.1, &format!("aqui se copiaria la lamina `{}`", n), &format!("prestala: f(mut {})  --  entregala: f(take {})  --  o lee una celda: {}[0]", n, n, n)))
        }
        Value::Index(b, i, _) => {
            if !matches!(**b, Value::Local(l, _) if es(l)) {
                no_se_copia(b, known, names)?;
            }
            no_se_copia(i, known, names)
        }
        Value::Len(b, _) => match **b {
            Value::Local(l, _) if es(l) => Ok(()),
            _ => no_se_copia(b, known, names),
        },
        Value::Bin(_, a, b, _) => {
            no_se_copia(a, known, names)?;
            no_se_copia(b, known, names)
        }
        Value::Neg(a, _) | Value::Not(a, _) | Value::Repeat(a, _, _) | Value::Field(a, _, _) | Value::Round(a, _, _) | Value::Is(a, _, _, _) | Value::Payload(a, _, _, _, _) | Value::Number(a, _, _) | Value::Byte(a, _) | Value::Lamina(a, _) => no_se_copia(a, known, names),
        // `push(mut l, x)` y los suyos son de las listas y los mapas: una
        // lamina tiene la medida que se pidio, y no crece ni encoge.
        Value::Lib(_, items, at) if matches!(items.first(), Some(Value::Local(l, _) | Value::Lend(_, l, _)) if es(*l)) => {
            Err(Message::new(Code::Mode, at.0, at.1, "una lamina no crece ni encoge", "tiene los bytes que se pidieron: `push`, `pop`, `put` y `remove` son de las listas y los mapas", "pide otra de la medida nueva: l = lamina(8192)"))
        }
        Value::Call(_, items, _) | Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) | Value::Lib(_, items, _) | Value::Director(_, items, _) => items.iter().try_for_each(|x| no_se_copia(x, known, names)),
        Value::Map(items, _) => items.iter().try_for_each(|(k, x)| no_se_copia(k, known, names).and_then(|_| no_se_copia(x, known, names))),
        Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Local(..) | Value::Lend(..) | Value::Read(..) => Ok(()),
    }
}
