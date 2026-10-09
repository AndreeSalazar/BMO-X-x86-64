//! `calc::clase` -- THE FIRST PASS of the calculation: what every value IS.
//!
//! Cut out of `calc.rs` on 05-10 (it had reached 1.761 lines, and the census
//! of modules, L6a, says no new file passes 1.000): the classes go here, the
//! arithmetic to `numero.rs`, and the run stays in `calc.rs`. Nothing changed
//! but the place: every block -- the ones that will never run too -- gets the
//! class of each local and each value, and a value of the wrong KIND is a NO
//! wherever it is (T0063, T0064, T0065, T0071, T0091).

use super::*;

// ===================================================================
//  THE FIRST PASS: classes
// ===================================================================

/// The class of every local and every value, in EVERY block. The blocks are
/// in reading order, and a local that dies (`Drop`) forgets its class: the
/// next one with that name is another value.
///
/// ** 08-10 (found by LB5): a block starts with what the paths that ENTER it
/// bring -- its blocks before it in reading order -- and not with what the
/// block just above it left. A `break` or a `continue` inside an `if` closes
/// the loop's scopes on ITS path (`Drop`, `ir.rs` `leave`); read in a line,
/// that also wiped the loop's name for the path that goes on, and `for i in
/// range(5)` / `if i == 2` / `continue` / `r = r + i` stopped the compiler
/// with a panic instead of compiling. A block no path enters (what follows a
/// jump) keeps the line, as before.
pub(super) fn classes(f: &Function, m: &Module) -> Result<(), Message> {
    let types = m.defs();
    let mut known: Vec<Option<Class>> = vec![None; f.locals.len()];
    for (l, t) in &f.params {
        known[*l] = Some(of_ty(t, types));
    }
    let mut enters: Vec<Vec<usize>> = vec![Vec::new(); f.blocks.len()];
    for (k, b) in f.blocks.iter().enumerate() {
        for t in b.end.targets() {
            if t > k {
                enters[t].push(k);
            }
        }
    }
    let mut left: Vec<Option<Vec<Option<Class>>>> = vec![None; f.blocks.len()];
    for (k, b) in f.blocks.iter().enumerate() {
        let from: Vec<&Vec<Option<Class>>> = enters[k].iter().filter_map(|p| left[*p].as_ref()).collect();
        if let Some((first, rest)) = from.split_first() {
            known = (*first).clone();
            for other in rest {
                for (mine, theirs) in known.iter_mut().zip(other.iter()) {
                    if mine.is_none() {
                        mine.clone_from(theirs);
                    }
                }
            }
        }
        for op in &b.ops {
            // ** D2 (level 11): outside a `gpu fn`, an f32 is kept or
            // passed, never counted, compared or printed.
            if !f.gpu {
                match op {
                    Op::Let { value, .. } | Op::Set { value, .. } => cpu_f32(value, &known, m)?,
                    Op::SetAt { value, .. } => cpu_f32(value, &known, m)?,
                    Op::Call { args, .. } | Op::Director { args, .. } => args.iter().try_for_each(|a| cpu_f32(a, &known, m))?,
                    Op::Write { parts, .. } => {
                        for p in parts {
                            cpu_f32(p, &known, m)?;
                            let c = class(p, &known, m)?;
                            if holds_f32(&c, types) {
                                return Err(f32_here(p.at(), "se imprime", "un f32 es la cuenta de la 3060, con sus redondeos de base 2: lo que se muestra en la CPU es un `dec`, y el redondeo se escribe"));
                            }
                        }
                    }
                    Op::Drop { .. } => {}
                }
            }
            // ** LB6: inside a `gpu fn`, two records are never compared.
            if f.gpu {
                match op {
                    Op::Let { value, .. } | Op::Set { value, .. } | Op::SetAt { value, .. } => gpu_records(value, &known, m)?,
                    _ => {}
                }
            }
            match op {
                Op::Let { local, value, at, ty, .. } => {
                    let mut c = class(value, &known, m)?;
                    // `let x: T = v` (level 7): the value must fit the type
                    // DECLARED, and the local is of that type from then on.
                    if let Some(t) = ty {
                        let want = of_ty(t, types);
                        // D4 (level 11): a number becomes f32 only where a
                        // type DECLARES it: `let xs: [f32; 4] = [1.0, ...]`.
                        if into_f32(&want, &c) {
                            c = want.clone();
                        }
                        if !fits(&want, &c) && f.locals[*local].name.starts_with("#m") {
                            // The hidden local of a `match` (level 8): its arms
                            // are the cases of one enum, so that is what it reads.
                            return Err(wrong(value.at(), &want, &c, types, &format!("los casos de este `match` son los de `enum {}`", want.short(types)), "dale al `match` un valor de ese enum, o escribe los casos del enum que tiene"));
                        }
                        if !fits(&want, &c) {
                            return Err(wrong(value.at(), &want, &c, types, &format!("`{}` se declaro `{}`", f.locals[*local].name, t.name()), "dale un valor de esa clase, o cambia el tipo declarado"));
                        }
                        c = want;
                    }
                    // `let l = []` (level 13): a list with nothing in it does
                    // not say what it will hold -- its type has to.
                    if has_any(&c) {
                        let what = if matches!(c, Class::Map(..)) { "un mapa vacio, `{}`" } else { "una lista vacia, `[]`" };
                        return Err(Message::new(
                            Code::WrongType,
                            value.at().0,
                            value.at().1,
                            &format!("{} no dice de que es", what),
                            "una lista o un mapa guarda valores de UNA clase, y sin nada dentro no se ve cual: hay que decirla",
                            &format!("di su tipo: let {}: [text] = []  o  let {}: {{text: int}} = {{}}", f.locals[*local].name, f.locals[*local].name),
                        ));
                    }
                    // The hidden count of a `for` (`#i`, `#fin`): `range`
                    // counts with whole numbers. Inside a `gpu fn` (LB5)
                    // every number is f32, and so is the count: `gpu.rs`
                    // let only integers WRITTEN up to 2^24 in, which f32
                    // counts exactly.
                    let name = &f.locals[*local].name;
                    if (name.starts_with("#i") || name.starts_with("#fin")) && c != Class::Int && !(f.gpu && c == Class::F32) {
                        return Err(Message::new(
                            Code::Mixed,
                            at.0,
                            at.1,
                            &format!("`range` cuenta con numeros enteros, y aqui hay {}", c.name(types)),
                            "un `for` da vueltas de un entero al siguiente: un texto, un decimal o un si-o-no no tienen siguiente",
                            "for i in range(10)  o  for i in range(1, 11)",
                        ));
                    }
                    if name.starts_with("#t") && !matches!(c, Class::Table(..) | Class::List(_) | Class::Map(..)) {
                        return Err(Message::new(
                            Code::Mixed,
                            at.0,
                            at.1,
                            &format!("`for ... in` recorre una tabla, una lista o un mapa, y aqui hay {}", c.name(types)),
                            "las vueltas de `for x in t` son las celdas de `t` (o, en un mapa, sus claves)",
                            "for x in [1, 2, 3]  o  for i in range(10)",
                        ));
                    }
                    known[*local] = Some(c);
                }
                Op::Set { local, value, at } => {
                    let c = class(value, &known, m)?;
                    // ** A `mut` changes its VALUE, never its kind: a number
                    // stays a number (Python lets `x = 5` become `x = "hola"`;
                    // the checker could not say what x is).
                    if let Some(old) = &known[*local] {
                        if !fits(old, &c) {
                            return Err(Message::new(
                                Code::Retype,
                                at.0,
                                at.1,
                                &format!("aqui `{}` pasaria de {} a {}", f.locals[*local].name, old.name(types), c.name(types)),
                                "un `mut` cambia de valor, no de clase: el que lee tiene que saber siempre que es",
                                "dale un valor de la misma clase, o usa otro nombre para el nuevo",
                            ));
                        }
                    }
                }
                Op::SetAt { local, path, value, at } => {
                    let mut c = known[*local].clone().expect("juez: the local has a value");
                    for st in path {
                        c = step_class(&c, st, &known, m, *at)?;
                    }
                    let got = class(value, &known, m)?;
                    if !fits(&c, &got) {
                        return Err(wrong(value.at(), &c, &got, types, "esa parte de la tabla o del registro es de otra clase", "dale un valor de la clase de esa celda o ese campo"));
                    }
                }
                Op::Write { parts, .. } => {
                    for p in parts {
                        class(p, &known, m)?;
                    }
                }
                Op::Call { func, args, at } if m.functions[*func].gpu => {
                    gpu_call(*func, args, *at, &known, m)?;
                }
                Op::Call { func, args, at } => args_fit(*func, args, *at, &known, m)?,
                Op::Director { what, args, at } => {
                    director(*what, args, *at, &known, m)?;
                }
                Op::Drop { local, .. } => known[*local] = None,
            }
        }
        if !f.gpu {
            match &b.end {
                End::Return(Some(v)) => cpu_f32(v, &known, m)?,
                End::Branch { cond, .. } => cpu_f32(cond, &known, m)?,
                _ => {}
            }
        } else {
            match &b.end {
                End::Return(Some(v)) => gpu_records(v, &known, m)?,
                End::Branch { cond, .. } => gpu_records(cond, &known, m)?,
                _ => {}
            }
        }
        if let End::Return(Some(v)) = &b.end {
            let got = class(v, &known, m)?;
            let want = of_ty(f.ret.as_ref().expect("check: a return with a value is in a fn with ->"), types);
            if !fits(&want, &got) {
                return Err(wrong(v.at(), &want, &got, types, &format!("`fn {}` promete `-> {}` en su linea {}", f.name, want.short(types), f.line), "devuelve un valor de esa clase, o cambia lo que promete la primera linea"));
            }
        }
        if let End::Branch { cond, at, .. } = &b.end {
            let c = class(cond, &known, m)?;
            if c != Class::Bool {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("un `if` pregunta SI o NO, y aqui hay {}", c.name(types)),
                    "TITAN++ no adivina que quiere decir un numero o un texto como condicion (en C, 0 es NO; en Python, \"\" tambien)",
                    &format!("di la pregunta entera: if {} > 0   o   if {} == ...", written(cond, f), written(cond, f)),
                ));
            }
        }
        left[k] = Some(known.clone());
    }
    Ok(())
}

/// One step into a class: a cell of a table, a field of a record.
pub(super) fn step_class(c: &Class, st: &PathStep, known: &[Option<Class>], m: &Module, at: At) -> Result<Class, Message> {
    match st {
        PathStep::Index(i) => {
            let ic = class(i, known, m)?;
            if ic != Class::Int {
                return Err(wrong(i.at(), &Class::Int, &ic, m.defs(), "una celda se pide con su numero: 0, 1, 2...", "a[0]"));
            }
            match c {
                Class::Table(inner, _) | Class::List(inner) => Ok((**inner).clone()),
                Class::Map(k, _) => Err(Message::new(
                    Code::Mixed,
                    at.0,
                    at.1,
                    "un mapa no tiene celdas numeradas: sus valores se piden por su clave",
                    &format!("lo que una clave ({}) lleva puede no estar: `get` lo dice con un caso, Hay(v) o NoHay", k.short(m.defs())),
                    "match get(mapa, clave)   o   put(mut mapa, clave, valor)",
                )),
                other => Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene celdas", other.name(m.defs())), "`[...]` pide una celda, y solo una tabla las tiene", "usa `[i]` sobre una tabla: [1, 2, 3][0]")),
            }
        }
        PathStep::Field(name, fat) => field_class(c, name, *fat, m),
    }
}

/// D4: may a value of class `got` become `want` by a DECLARED type? A number
/// into an f32, cell by cell into a table of f32.
pub(super) fn into_f32(want: &Class, got: &Class) -> bool {
    match (want, got) {
        (Class::F32, Class::Int | Class::Dec) => true,
        (Class::Table(w, n), Class::Table(g, k)) => n == k && into_f32(w, g),
        _ => false,
    }
}

/// Is there an f32 in a value of this class (a cell, a field)?
pub(super) fn holds_f32(c: &Class, d: Defs) -> bool {
    match c {
        Class::F32 => true,
        Class::Table(inner, _) | Class::List(inner) | Class::Opt(inner) => holds_f32(inner, d),
        Class::Map(k, v) => holds_f32(k, d) || holds_f32(v, d),
        Class::Record(t) => d.types[*t].fields.iter().any(|f| holds_f32(&of_ty(&f.ty, d), d)),
        _ => false,
    }
}

/// T0091: an f32 used on the CPU (D2).
pub(super) fn f32_here(at: At, how: &str, why: &str) -> Message {
    Message::new(
        Code::F32Cpu,
        at.0,
        at.1,
        &format!("aqui un f32 {} en la CPU", how),
        why,
        "en la CPU un f32 se guarda o se pasa a otra gpu fn; para usarlo, se vuelve dec a la vista: round(x, 2)",
    )
}

/// ** D2 on the CPU: an f32 is never counted or compared here. `round(x, n)`
/// is the door out, so what it takes is not judged as a count -- only what
/// is inside it.
pub(super) fn cpu_f32(v: &Value, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    match v {
        Value::Bin(op, l, r, at) => {
            if class(l, known, m)? == Class::F32 || class(r, known, m)? == Class::F32 {
                let how = if matches!(*op, "==" | "!=" | "<" | "<=" | ">" | ">=") { "se compara" } else { "se cuenta" };
                return Err(f32_here(*at, how, "la CPU cuenta con dec exacto (la regla de la casa, PLAN_EL_CENTAURO D2): el f32 cuenta dentro de una gpu fn, en la 3060"));
            }
            cpu_f32(l, known, m)?;
            cpu_f32(r, known, m)
        }
        Value::Neg(x, at) | Value::Not(x, at) => {
            if class(x, known, m)? == Class::F32 {
                return Err(f32_here(*at, "se cuenta", "la CPU cuenta con dec exacto (PLAN_EL_CENTAURO D2): el f32 cuenta en la 3060"));
            }
            cpu_f32(x, known, m)
        }
        Value::Round(x, _, _) | Value::Repeat(x, _, _) | Value::Field(x, _, _) | Value::Len(x, _) | Value::Is(x, _, _, _) | Value::Payload(x, _, _, _, _) => cpu_f32(x, known, m),
        Value::Index(b, i, _) => {
            cpu_f32(b, known, m)?;
            cpu_f32(i, known, m)
        }
        Value::Call(_, items, _) | Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) | Value::Lib(_, items, _) => items.iter().try_for_each(|x| cpu_f32(x, known, m)),
        Value::Map(items, _) => items.iter().try_for_each(|(k, v)| cpu_f32(k, known, m).and_then(|_| cpu_f32(v, known, m))),
        _ => Ok(()),
    }
}

/// ** LB6: inside a `gpu fn` that draws, two RECORDS are not compared with
/// `==` or `!=`. An f32 is compared by IEEE -- a NaN equals nothing, -0
/// equals 0 -- and a whole record has no such rule: the calculation compares
/// its cells by their bits (`numero::same`), a card would field by field.
/// Said field by field, it is one rule, and the one of the f32.
pub(super) fn gpu_records(v: &Value, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    match v {
        Value::Bin(op, l, r, at) => {
            if matches!(*op, "==" | "!=") && matches!(class(l, known, m)?, Class::Record(_)) {
                return Err(Message::new(
                    Code::GpuBody,
                    at.0,
                    at.1,
                    &format!("dos registros con `{}` dentro de una gpu fn", op),
                    "un f32 se compara por IEEE (un NaN no es igual a nada, -0 es igual a 0), y un registro entero no tiene esa regla: el calculo los miraria por sus bits y la tarjeta campo a campo, y una gpu fn da los MISMOS bits en los tres (L29)",
                    "compara campo a campo: v.color.x == w.color.x and v.color.y == w.color.y",
                ));
            }
            gpu_records(l, known, m)?;
            gpu_records(r, known, m)
        }
        Value::Neg(x, _) | Value::Not(x, _) | Value::Field(x, _, _) => gpu_records(x, known, m),
        Value::Call(_, items, _) | Value::Record(_, items, _) => items.iter().try_for_each(|x| gpu_records(x, known, m)),
        _ => Ok(()),
    }
}

/// The class of a call to a `gpu fn` (level 11, D1): with values, its
/// result; with TABLES of n cells -- all of the same n -- n results, one per
/// thread.
pub(super) fn gpu_call(func: usize, args: &[Value], at: At, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let g = &m.functions[func];
    // ** LB6: una gpu fn que DIBUJA no se llama: es una etapa de la tuberia, y
    // la pone a dibujar VERRANO (LB7).
    let tys: Vec<&Ty> = g.params.iter().map(|(_, t)| t).collect();
    if crate::gpu::forma(&m.types, &tys, g.ret.as_ref()) != crate::gpu::Forma::Celda {
        return Err(Message::new(
            Code::GpuBody,
            at.0,
            at.1,
            &format!("`{}()`: una gpu fn que DIBUJA no se llama", g.name),
            "es una etapa de la tuberia de la 3060: la de vertice corre una vez por vertice y la de pixel una por pixel, y las pone a dibujar VERRANO (LB7), no un programa",
            "llama a una gpu fn de celdas; la que dibuja la usara VERRANO",
        ));
    }
    let ret = of_ty(g.ret.as_ref().expect("gpu: a gpu fn gives a value"), m.defs());
    let got: Vec<Class> = args.iter().map(|a| class(a, known, m)).collect::<Result<_, _>>()?;
    let wants: Vec<Class> = g.params.iter().map(|(_, t)| of_ty(t, m.defs())).collect();
    if got.iter().zip(&wants).all(|(c, w)| c == w) {
        return Ok(ret);
    }
    let n = match got.first() {
        Some(Class::Table(_, n)) => *n,
        _ => 0,
    };
    let cells = got.iter().zip(&wants).all(|(c, w)| matches!(c, Class::Table(inner, k) if *k == n && **inner == *w));
    if n > 0 && cells {
        return Ok(Class::Table(Box::new(ret), n));
    }
    let want: Vec<String> = wants.iter().map(|w| w.short(m.defs())).collect();
    Err(Message::new(
        Code::WrongType,
        at.0,
        at.1,
        &format!("`{}` se aplica a valores ({}) o a tablas de esos valores, todas del mismo largo", g.name, want.join(", ")),
        &format!("aqui llega: {}", got.iter().map(|c| c.short(m.defs())).collect::<Vec<_>>().join(", ")),
        &format!("una celda de cada tabla por hilo: {}(xs, ys) con xs, ys de [f32; n]", g.name),
    ))
}

/// ** LB7b: what the DIRECTOR takes, and what it gives back -- a yes/no, or
/// nothing (`director.espera`). The tables of `publica` are f32 (the
/// vertices of a frame are counted by a `gpu fn`: on the CPU they are only
/// PASSED, D2), four per vertex, both of the same length.
pub(super) fn director(what: Director, args: &[Value], at: At, known: &[Option<Class>], m: &Module) -> Result<Option<Class>, Message> {
    // What goes to the director is READ (a copy): nothing is lent or given.
    if let Some(Value::Lend(mode, _, lat)) = args.iter().find(|a| matches!(a, Value::Lend(..))) {
        return Err(Message::new(Code::Mode, lat.0, lat.1, &format!("`{}` a `{}`", mode.word(), what.name()), "el director lee lo que le das, y no lo cambia ni se lo queda: no hay nada que prestar ni entregar", "dale el valor: director.publica(f, n, posiciones, colores)"));
    }
    let got: Vec<Class> = args.iter().map(|a| class(a, known, m)).collect::<Result<_, _>>()?;
    let int = |k: usize, what: &str, how: &str| -> Result<(), Message> {
        if got[k] == Class::Int {
            Ok(())
        } else {
            Err(wrong(args[k].at(), &Class::Int, &got[k], m.defs(), what, how))
        }
    };
    match what {
        Director::Lamina => {
            int(0, "`director.lamina` pide cuantos vertices caben en cada una de sus dos ranuras", "director.lamina(18)")?;
            Ok(Some(Class::Bool))
        }
        Director::Espera => {
            int(0, "`director.espera` pide los milisegundos que duerme el programa", "director.espera(16)")?;
            Ok(None)
        }
        Director::Publica => {
            int(0, "lo primero que publica es el NUMERO del fotograma", "director.publica(f, n, posiciones, colores)")?;
            int(1, "lo segundo, cuantos vertices del fotograma van en las tablas", "director.publica(f, n, posiciones, colores)")?;
            match (&got[2], &got[3]) {
                (Class::Table(a, n), Class::Table(b, k)) if **a == Class::F32 && **b == Class::F32 && n == k && *n > 0 && n % 4 == 0 => Ok(Some(Class::Bool)),
                (a, b) => Err(Message::new(
                    Code::WrongType,
                    at.0,
                    at.1,
                    "`director.publica` lleva los vertices en dos tablas de f32 del mismo largo: sus posiciones y sus colores, cuatro f32 por vertice",
                    &format!("aqui llega: {} y {}", a.short(m.defs()), b.short(m.defs())),
                    "let mut posiciones: [f32; 72] = [0.0; 72]  (18 vertices: x, y, z, w de cada uno; los colores, r, g, b, a)",
                )),
            }
        }
    }
}

/// Does a value of class `c` keep the trait `k`? Its type has a
/// `trait ... for` of it (level 10).
pub(super) fn keeps(c: &Class, k: usize, d: Defs) -> bool {
    let ty = match c {
        Class::Int => "int".to_string(),
        Class::Dec => "dec".to_string(),
        Class::Text => "text".to_string(),
        Class::Bool => "bool".to_string(),
        Class::Record(t) => d.types[*t].name.clone(),
        Class::Enum(e) => d.enums[*e].name.clone(),
        Class::F32 => "f32".to_string(),
        Class::Table(..) | Class::Trait(_) | Class::List(_) | Class::Map(..) | Class::Opt(_) | Class::Any => return false,
    };
    d.impls.iter().any(|(t, i)| t == &d.traits[k].name && i == &ty)
}

/// The type a value IS, by name: what picks the fn of a trait it runs.
pub(super) fn type_of(c: &Const, d: Defs) -> String {
    match c {
        Const::Int(_) => "int".into(),
        Const::Dec(..) => "dec".into(),
        Const::Text(_) => "text".into(),
        Const::Bool(_) => "bool".into(),
        Const::Record(t, _) => d.types[*t].name.clone(),
        Const::Variant(e, ..) => d.enums[*e].name.clone(),
        Const::F32(_) => "f32".into(),
        Const::Table(_) => "tabla".into(),
        Const::List(..) => "lista".into(),
        Const::Map(..) => "mapa".into(),
    }
}

/// The class of the field `name` of a value of class `c`, or T0073.
pub(super) fn field_class(c: &Class, name: &str, at: At, m: &Module) -> Result<Class, Message> {
    if let Class::Trait(k) = c {
        let t = &m.traits[*k];
        return Err(Message::new(
            Code::Field,
            at.0,
            at.1,
            &format!("de algo que cumple `trait {}` no se conoce el campo `{}`", t.name, name),
            &format!("puede ser cualquier tipo que cumpla {}: de el solo se sabe lo que el trait promete (linea {}), sus fn", t.name, t.line),
            &format!("pidelo con una fn del trait: agrega `fn {}(x: {}) -> ...` a `trait {}`", name, t.name, t.name),
        ));
    }
    let Class::Record(t) = c else {
        return Err(Message::new(Code::Field, at.0, at.1, &format!("{} no tiene campos", c.name(m.defs())), "`.campo` pide un campo, y solo un registro (un `type`) los tiene", "usa `.x` sobre un registro: nave.x"));
    };
    let def = &m.defs()[*t];
    match def.fields.iter().find(|f| f.name == name) {
        Some(f) => Ok(of_ty(&f.ty, m.defs())),
        None => Err(Message::new(
            Code::Field,
            at.0,
            at.1,
            &format!("`{}` no tiene un campo `{}`", def.name, name),
            &format!("los campos de `type {}` (linea {}) son: {}", def.name, def.line, def.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")),
            "usa uno de esos, o agregalo al `type`",
        )),
    }
}

/// A value as the author wrote it, with the NAMES (the IR numbers them):
/// for the COMO of a message.
pub(super) fn written(v: &Value, f: &Function) -> String {
    match v {
        Value::Int(n, _) => n.to_string(),
        Value::Text(t, _) => format!("{:?}", t),
        Value::Bool(b, _) => b.to_string(),
        Value::Dec(d, s, _) => show_dec(*d, *s),
        Value::Local(l, _) => f.locals[*l].name.clone(),
        Value::Bin(op, l, r, _) => format!("{} {} {}", written(l, f), op, written(r, f)),
        Value::Neg(v, _) => format!("-{}", written(v, f)),
        Value::Not(v, _) => format!("not {}", written(v, f)),
        Value::Field(b, n, _) => format!("{}.{}", written(b, f), n),
        Value::Index(b, i, _) => format!("{}[{}]", written(b, f), written(i, f)),
        Value::Len(v, _) => format!("len({})", written(v, f)),
        _ => "...".into(),
    }
}

/// The values given to a call, each against the class its parameter says.
pub(super) fn args_fit(func: usize, args: &[Value], _at: At, known: &[Option<Class>], m: &Module) -> Result<(), Message> {
    let g = &m.functions[func];
    for ((a, (l, t)), mode) in args.iter().zip(&g.params).zip(&g.modes) {
        let got = class(a, known, m)?;
        let want = of_ty(t, m.defs());
        // ** A parameter of a TRAIT (level 10): any value whose type keeps
        // it -- said HERE, at the call, in one sentence (T0085), never from
        // inside the generic fn.
        if let Class::Trait(k) = want {
            if got == want || keeps(&got, k, m.defs()) {
                continue;
            }
            let t = &m.traits[k].name;
            let ty = got.short(m.defs());
            return Err(Message::new(
                Code::NotImpl,
                a.at().0,
                a.at().1,
                &format!("{} no cumple `trait {}`", ty, t),
                &format!("`{}` pide `{}: {}` (su linea {}): cualquier valor que sepa hacer lo que {} promete, y {} no dice como", g.name, g.locals[*l].name, t, g.line, t, ty),
                &format!("dile como: trait {} for {}, con sus fn ({})", t, ty, m.traits[k].methods.iter().map(|s| s.name.rsplit('.').next().unwrap_or(&s.name)).collect::<Vec<_>>().join(", ")),
            ));
        }
        // Lent to be changed: the SAME class, since what comes back goes in
        // the caller's own local (an int lent as a dec would come back a dec).
        let ok = if *mode == crate::tree::Mode::Mut { want == got } else { fits(&want, &got) };
        if !ok {
            return Err(wrong(
                a.at(),
                &want,
                &got,
                m.defs(),
                &format!("`{}` pide `{}: {}` (su linea {})", g.name, g.locals[*l].name, t.name(), g.line),
                "TITAN++ no convierte solo: dale un valor de esa clase",
            ));
        }
    }
    Ok(())
}

/// The class of a value, or the NO that says why it has none.
pub fn class(v: &Value, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let types = m.defs();
    Ok(match v {
        Value::Int(..) => Class::Int,
        Value::Text(..) => Class::Text,
        // `lee()`: a text, whatever is typed (E1).
        Value::Read(..) => Class::Text,
        Value::Bool(..) => Class::Bool,
        Value::Dec(..) => Class::Dec,
        Value::F32(..) => Class::F32,
        Value::Local(l, _) | Value::Lend(_, l, _) => known[*l].clone().expect("juez: every local read has a value"),
        Value::Round(inner, _, at) => match class(inner, known, m)? {
            c if c.number() || c == Class::F32 => Class::Dec,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("`round` redondea un numero, y aqui hay {}", c.name(types)), "solo un numero tiene decimales que redondear", "round(total / 3, 2)")),
        },
        Value::Call(func, args, at) if m.functions[*func].gpu => gpu_call(*func, args, *at, known, m)?,
        // LB7b: what the director answers, a yes/no (the checker keeps
        // `espera`, which answers nothing, out of a value).
        Value::Director(what, args, at) => match director(*what, args, *at, known, m)? {
            Some(c) => c,
            None => return Err(Message::new(Code::Result, at.0, at.1, &format!("`{}` no devuelve nada, y aqui se usa como un valor", what.name()), "espera y ya: no hay un valor que guardar", &format!("llamala en su propia linea: {}(16)", what.name()))),
        },
        Value::Call(func, args, at) => {
            args_fit(*func, args, *at, known, m)?;
            of_ty(m.functions[*func].ret.as_ref().expect("check: a call used as a value gives one back"), types)
        }
        // `[]` (level 13): nothing yet -- where it goes says what it holds.
        Value::Table(items, _) if items.is_empty() => Class::List(Box::new(Class::Any)),
        Value::Table(items, at) => {
            let mut c = class(&items[0], known, m)?;
            for i in &items[1..] {
                let ci = class(i, known, m)?;
                if fits(&c, &ci) {
                    continue;
                }
                if fits(&ci, &c) {
                    // [1, 2.5]: the whole table is `dec`.
                    c = ci;
                    continue;
                }
                // [[1], [2, 3]] (13): tables of one class and two lengths
                // can only be LISTS of it -- what a `[[int]]` says.
                if let (Class::Table(a, _) | Class::List(a), Class::Table(b, _) | Class::List(b)) = (&c, &ci) {
                    if a == b {
                        c = Class::List(a.clone());
                        continue;
                    }
                }
                return Err(wrong(i.at(), &c, &ci, types, "las celdas de una tabla son todas de UNA clase", &format!("o todas {}, o una tabla para cada clase", c.short(types))));
            }
            let _ = at;
            Class::Table(Box::new(c), items.len())
        }
        Value::Repeat(item, n, _) => Class::Table(Box::new(class(item, known, m)?), *n),
        Value::Index(b, i, at) => {
            let bc = class(b, known, m)?;
            step_class(&bc, &PathStep::Index((**i).clone()), known, m, *at)?
        }
        Value::Field(b, name, at) => field_class(&class(b, known, m)?, name, *at, m)?,
        Value::Record(t, items, _) => {
            let def = &types[*t];
            for (f, item) in def.fields.iter().zip(items) {
                let got = class(item, known, m)?;
                let want = of_ty(&f.ty, types);
                if !fits(&want, &got) {
                    return Err(wrong(item.at(), &want, &got, types, &format!("el campo `{}` de `type {}` es `{}` (linea {})", f.name, def.name, f.ty.name(), f.line), "dale un valor de esa clase"));
                }
            }
            Class::Record(*t)
        }
        Value::Variant(e, v, items, _) => {
            let case = &types.enums[*e].cases[*v];
            for (k, (want_ty, item)) in case.fields.iter().zip(items).enumerate() {
                let got = class(item, known, m)?;
                let want = of_ty(want_ty, types);
                if !fits(&want, &got) {
                    return Err(wrong(item.at(), &want, &got, types, &format!("el dato {} de `{}` es `{}` (linea {})", k + 1, case.name, want_ty.name(), case.line), "dale un valor de esa clase"));
                }
            }
            Class::Enum(*e)
        }
        Value::Is(inner, e, _, at) => match class(inner, known, m)? {
            Class::Enum(k) if k == *e => Class::Bool,
            // `match get(m, k)` (13): Hay and NoHay are of every `Opcion[T]`.
            Class::Opt(_) if crate::prelude::is_opcion(&types.enums[*e].name) => Class::Bool,
            other => return Err(wrong(*at, &Class::Enum(*e), &other, types, &format!("los casos de este `match` son los de `enum {}`", types.enums[*e].name), "dale al `match` un valor de ese enum")),
        },
        // The value a `Hay(v)` carries is of the list or map it came from (13).
        Value::Payload(inner, e, _, _, _) if crate::prelude::is_opcion(&types.enums[*e].name) => match class(inner, known, m)? {
            Class::Opt(c) => *c,
            other => unreachable!("classes: an arm of Opcion reads an Opcion, not {:?}", other),
        },
        Value::Payload(_, e, v, k, _) => of_ty(&types.enums[*e].cases[*v].fields[*k], types),
        Value::Lib(lib, args, at) => lib_class(*lib, args, *at, known, m)?,
        Value::Map(items, at) => map_class(items, *at, known, m)?,
        // `numero(t)` (the prelude): it reads a TEXT, and gives its case.
        Value::Number(inner, e, at) => match class(inner, known, m)? {
            Class::Text => Class::Enum(*e),
            other => return Err(wrong(*at, &Class::Text, &other, types, "`numero` mira si UN TEXTO es un numero entero", "numero(linea), con la linea que trae lee()")),
        },
        Value::Len(inner, at) => match class(inner, known, m)? {
            Class::Table(..) | Class::List(_) | Class::Map(..) => Class::Int,
            other => {
                return Err(Message::new(Code::Mixed, at.0, at.1, &format!("`len` cuenta las celdas de una tabla, una lista o un mapa, y aqui hay {}", other.name(types)), "solo una tabla, una lista o un mapa tienen celdas que contar", "len([1, 2, 3])"))
            }
        },
        Value::Neg(inner, at) => match class(inner, known, m)? {
            c if c.number() || c == Class::F32 => c,
            c => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("{} no tiene signo", c.name(types)), &format!("aqui hay {} con un `-` delante", c.name(types)), "el `-` va delante de un numero")),
        },
        Value::Not(inner, at) => match class(inner, known, m)? {
            Class::Bool => Class::Bool,
            c => {
                return Err(Message::new(
                    Code::NotBool,
                    at.0,
                    at.1,
                    &format!("`not` da la vuelta a un si-o-no, y aqui hay {}", c.name(types)),
                    "`not` cambia true por false y false por true; un numero o un texto no tienen vuelta",
                    "pregunta primero: not (vidas > 0)",
                ))
            }
        },
        Value::Bin(op, l, r, at) if class(l, known, m)? == Class::F32 || class(r, known, m)? == Class::F32 => {
            // ** f32 with f32 only (level 11): in the 3060 every number is
            // one; a dec or an int never becomes f32 without a declaration.
            let (a, b) = (class(l, known, m)?, class(r, known, m)?);
            if a != b {
                let other = if a == Class::F32 { b } else { a };
                return Err(Message::new(
                    Code::Mixed,
                    at.0,
                    at.1,
                    &format!("un f32 y {} no se mezclan", other.name(types)),
                    "un f32 redondea en base 2 y un dec es exacto: juntarlos callados perderia lo exacto sin decirlo",
                    "dentro de una gpu fn todo es f32; en la CPU, el dec entra a f32 por un tipo declarado: let xs: [f32; 4] = ...",
                ));
            }
            match *op {
                "+" | "-" | "*" | "/" => Class::F32,
                "==" | "!=" | "<" | "<=" | ">" | ">=" => Class::Bool,
                _ => return Err(Message::new(Code::Mixed, at.0, at.1, &format!("dos f32 no se pueden `{}`", op), "un f32 tiene + - * / y se compara; el resto `%` es de enteros", "a - b * floor(a / b) cuando llegue; hoy, ints para el resto")),
            }
        }
        Value::Bin(op, l, r, at) => {
            let (a, b) = (class(l, known, m)?, class(r, known, m)?);
            let numbers = a.number() && b.number();
            let dec = numbers && (a == Class::Dec || b == Class::Dec);
            match *op {
                "+" | "-" | "*" | "/" if numbers => {
                    if dec {
                        Class::Dec
                    } else {
                        Class::Int
                    }
                }
                "%" if a == Class::Int && b == Class::Int => Class::Int,
                "%" if numbers => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        "el resto `%` es de numeros enteros",
                        "un decimal no tiene resto: la division de un `dec` da su resultado EXACTO",
                        "divide con `/`, o usa ints para el resto",
                    ))
                }
                "+" if a == Class::Text && b == Class::Text => Class::Text,
                "==" | "!=" if a == b || numbers || fits(&a, &b) || fits(&b, &a) => Class::Bool,
                "<" | "<=" | ">" | ">=" if numbers => Class::Bool,
                "and" | "or" if a == Class::Bool && b == Class::Bool => Class::Bool,
                "and" | "or" => {
                    let odd = if a != Class::Bool { a } else { b };
                    return Err(Message::new(
                        Code::NotBool,
                        at.0,
                        at.1,
                        &format!("`{}` une dos si-o-no, y aqui hay {}", op, odd.name(types)),
                        "cada lado de `and` / `or` es una pregunta entera: `vidas and escudo` no dice que se pregunta",
                        &format!("compara cada lado: vidas > 0 {} escudo > 0", op),
                    ));
                }
                "<" | "<=" | ">" | ">=" if a == Class::Text && b == Class::Text => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("dos textos no se ordenan con `{}`", op),
                        "el orden de los textos depende del idioma (la n y la enie, las mayusculas); hoy un texto solo se compara con `==` y `!=`",
                        "compara si son iguales: a == b",
                    ))
                }
                "-" | "*" | "/" | "%" if a == Class::Text && b == Class::Text => {
                    return Err(Message::new(Code::Mixed, at.0, at.1, &format!("dos textos no se pueden `{}`", op), "con textos solo hay `+`: ponerlos uno detras del otro", "\"a\" + \"b\""))
                }
                "==" | "!=" => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se comparan", a.name(types), b.name(types)),
                        "nunca son iguales, y TITAN++ no convierte solo: 1 == \"1\" seria una pregunta con trampa",
                        "compara cosas de la misma clase",
                    ))
                }
                // `numero(t)` usado como si fuera el int: su caso esta sin mirar
                _ if [&a, &b].iter().any(|c| matches!(c, Class::Enum(e) if types.enums[*e].name.rsplit('.').next() == Some(crate::prelude::NUMERO))) => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se pueden `{}`", a.name(types), b.name(types), op),
                        "`numero(t)` no da un int: da un CASO, Es(n) o NoEs, porque lo tecleado puede no ser un numero",
                        "mira el caso con match: en `Es(n)` n ya es el int; en `NoEs` di que no lo era",
                    ))
                }
                _ => {
                    return Err(Message::new(
                        Code::Mixed,
                        at.0,
                        at.1,
                        &format!("{} y {} no se pueden `{}`", a.name(types), b.name(types), op),
                        "TITAN++ no convierte solo: \"a\" + 1 no es \"a1\" (ni 1 + \"1\" es 2, ni true + 1 es 2)",
                        "para mostrarlos juntos, separalos con comas: print(\"total: \", n)",
                    ))
                }
            }
        }
    })
}

/// The class of what a fn of lists and maps gives (level 13), and the NO
/// when its values are not what it works with.
fn lib_class(lib: Lib, args: &[Value], at: At, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let types = m.defs();
    let first = class(&args[0], known, m)?;
    let arg = |k: usize| class(&args[k], known, m);
    let not_one = |what: &str, example: &str| Message::new(Code::WrongType, at.0, at.1, &format!("`{}` es de {}, y aqui le llega {}", lib.name(), what, first.name(types)), &format!("`{}` trabaja con {}", lib.name(), what), example);
    let cell = |want: &Class, got: Class, k: usize, what: &str| -> Result<(), Message> {
        if fits(want, &got) {
            return Ok(());
        }
        Err(wrong(args[k].at(), want, &got, types, what, "dale un valor de esa clase"))
    };
    Ok(match lib {
        Lib::Push | Lib::Last | Lib::DropLast => {
            let Class::List(t) = &first else { return Err(not_one("una lista", "push(mut nombres, \"ana\")")) };
            match lib {
                Lib::Push => {
                    cell(t, arg(1)?, 1, &format!("la lista es de {}", t.short(types)))?;
                    first.clone()
                }
                Lib::Last => Class::Opt(t.clone()),
                _ => first.clone(),
            }
        }
        Lib::Put | Lib::Remove | Lib::Get | Lib::Has => {
            let Class::Map(k, v) = &first else { return Err(not_one("un mapa", "put(mut stock, \"pan\", 3)")) };
            cell(k, arg(1)?, 1, &format!("las claves del mapa son de {}", k.short(types)))?;
            match lib {
                Lib::Put => {
                    cell(v, arg(2)?, 2, &format!("los valores del mapa son de {}", v.short(types)))?;
                    first.clone()
                }
                Lib::Remove => first.clone(),
                Lib::Get => Class::Opt(v.clone()),
                _ => Class::Bool,
            }
        }
        Lib::Turn => match &first {
            Class::Table(t, _) | Class::List(t) => (**t).clone(),
            Class::Map(k, _) => (**k).clone(),
            other => unreachable!("classes: `#t` is a table, a list or a map, not {:?}", other),
        },
    })
}

/// The class of a map written: its keys of ONE class that can be a key, its
/// values of ONE class (a decimal among ints makes them all decimals).
fn map_class(items: &[(Value, Value)], at: At, known: &[Option<Class>], m: &Module) -> Result<Class, Message> {
    let types = m.defs();
    let Some((k0, v0)) = items.first() else { return Ok(Class::Map(Box::new(Class::Any), Box::new(Class::Any))) };
    let (kc, mut vc) = (class(k0, known, m)?, class(v0, known, m)?);
    if !key_class(&kc, types) {
        return Err(Message::new(
            Code::WrongType,
            k0.at().0,
            k0.at().1,
            &format!("{} no puede ser la clave de un mapa", kc.name(types)),
            "una clave es un int, un text, un bool o un registro de esos: algo con UNA igualdad que nadie discute (1.0 y 1.00 son el mismo decimal, y una lista cambia)",
            "{\"ana\": 3}  o  {7: \"siete\"}",
        ));
    }
    for (k, v) in &items[1..] {
        let c = class(k, known, m)?;
        if c != kc {
            return Err(wrong(k.at(), &kc, &c, types, "las claves de un mapa son todas de UNA clase", "o todas de esa clase, o otro mapa"));
        }
        let c = class(v, known, m)?;
        if fits(&vc, &c) {
            continue;
        }
        if fits(&c, &vc) {
            vc = c;
            continue;
        }
        return Err(wrong(v.at(), &vc, &c, types, "los valores de un mapa son todos de UNA clase", "o todos de esa clase, o otro mapa"));
    }
    let _ = at;
    Ok(Class::Map(Box::new(kc), Box::new(vc)))
}
