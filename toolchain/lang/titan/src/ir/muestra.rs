//! `ir::muestra` -- the IR as TEXT, for `titan ir`: what the emitter will
//! receive, read by a person. Cut out of `ir.rs` on 05-10 (L6a: no file over
//! 1.000 lines) when level 13 brought the lists and maps.

use super::*;

impl Module {
    /// The IR as text, for `titan ir`: what the emitter will receive.
    pub fn show(&self) -> String {
        let mut s = format!("mod {}  \"{}\"   entra por f{}\n", self.name, self.purpose, self.entry);
        let tail = match &self.flat {
            Some(flat) => {
                let mut t = format!("\nlo que escribe, CORRIDO al compilar ({} escrituras): es lo que hace el .bex\n", flat.len());
                for op in flat {
                    if let Op::Write { parts, at } = op {
                        let p: Vec<String> = parts.iter().map(show).collect();
                        t += &format!("    linea {:<4} escribe {}\n", at.0, p.join(", "));
                    }
                }
                t
            }
            None => String::new(),
        };
        for (i, f) in self.functions.iter().enumerate() {
            let locals: Vec<String> = f.locals.iter().enumerate().map(|(k, l)| format!("%{}={}", k, l.name)).collect();
            s += &format!("f{} {}   {}\n", i, f.name, locals.join(" "));
            for (j, b) in f.blocks.iter().enumerate() {
                s += &format!("  b{}{}\n", j, if b.dead { "   (muerto: ninguna ejecucion llega aqui, y no deja bytes)" } else { "" });
                for op in &b.ops {
                    s += &match op {
                        Op::Let { local, value, mutable: true, .. } => format!("    %{} = {}   (mut)\n", local, show(value)),
                        Op::Let { local, value, .. } => format!("    %{} = {}\n", local, show(value)),
                        Op::Set { local, value, .. } => format!("    %{} := {}\n", local, show(value)),
                        Op::Write { parts, .. } => {
                            let p: Vec<String> = parts.iter().map(show).collect();
                            format!("    escribe {}\n", p.join(", "))
                        }
                        Op::Call { func, args, .. } => {
                            let a: Vec<String> = args.iter().map(show).collect();
                            format!("    llama   f{} ({})({})\n", func, self.functions[*func].name, a.join(", "))
                        }
                        Op::Drop { local, at } => format!("    muere   %{} ({}, al cerrarse el bloque de la linea {})\n", local, f.locals[*local].name, at.0),
                        Op::SetAt { local, path, value, .. } => {
                            let p: String = path
                                .iter()
                                .map(|st| match st {
                                    PathStep::Index(i) => format!("[{}]", show(i)),
                                    PathStep::Field(n, _) => format!(".{}", n),
                                })
                                .collect();
                            format!("    %{}{} := {}\n", local, p, show(value))
                        }
                    };
                }
                s += &match &b.end {
                    End::Return(None) => "    vuelve\n".to_string(),
                    End::Return(Some(v)) => format!("    vuelve con {}\n", show(v)),
                    End::Jump(t) => format!("    salta   b{}\n", t),
                    End::Branch { cond, then, other, .. } => format!("    si {} -> b{}, sino -> b{}\n", show(cond), then, other),
                };
            }
        }
        s + &tail
    }
}

fn show(v: &Value) -> String {
    match v {
        Value::Int(n, _) => n.to_string(),
        Value::Text(t, _) => format!("{:?}", t),
        Value::Bool(b, _) => b.to_string(),
        Value::Local(l, _) => format!("%{}", l),
        Value::Bin(op, l, r, _) => format!("({} {} {})", show(l), op, show(r)),
        Value::Neg(v, _) => format!("-{}", show(v)),
        Value::Not(v, _) => format!("not {}", show(v)),
        Value::Call(f, args, _) => {
            let a: Vec<String> = args.iter().map(show).collect();
            format!("f{}({})", f, a.join(", "))
        }
        Value::Dec(d, sc, _) => crate::tree::show_dec(*d, *sc),
        Value::Table(items, _) => format!("[{}]", items.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Repeat(v, n, _) => format!("[{}; {}]", show(v), n),
        Value::Index(b, i, _) => format!("{}[{}]", show(b), show(i)),
        Value::Field(b, n, _) => format!("{}.{}", show(b), n),
        Value::Record(t, items, _) => format!("T{} {{ {} }}", t, items.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Len(v, _) => format!("len({})", show(v)),
        Value::Read(_) => "lee()".to_string(),
        Value::Number(v, _, _) => format!("numero({})", show(v)),
        Value::Lend(m, l, _) => format!("{} %{}", m.word(), l),
        Value::Round(v, n, _) => format!("round({}, {})", show(v), n),
        Value::F32(b, _) => format!("{}f32", f32::from_bits(*b)),
        Value::Variant(e, v, args, _) if args.is_empty() => format!("E{}.{}", e, v),
        Value::Variant(e, v, args, _) => format!("E{}.{}({})", e, v, args.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Is(x, e, v, _) => format!("{} es E{}.{}", show(x), e, v),
        Value::Payload(x, e, v, k, _) => format!("dato {} de {} (E{}.{})", k, show(x), e, v),
        Value::Lib(lib, args, _) => format!("{}({})", lib.name(), args.iter().map(show).collect::<Vec<_>>().join(", ")),
        Value::Map(items, _) => format!("{{{}}}", items.iter().map(|(k, v)| format!("{}: {}", show(k), show(v))).collect::<Vec<_>>().join(", ")),
    }
}
