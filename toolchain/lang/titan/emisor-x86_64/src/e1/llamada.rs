//! ** LAS LLAMADAS de E1: la de una fn de TITAN++ (sus valores y el sitio de
//! su resultado) y la de un trait (salta a la fn del tipo).
//!
//! ```text
//!    la de siempre   rdi = un bloque con un PUNTERO por valor, rsi = el
//!                    sitio del resultado (o 0)
//!    AF7, ligera     una fn de enteros (1 a 4 `int` copiados y un `int` o
//!                    nada de vuelta): los valores en rdx, rcx, r8 y r9, y
//!                    el resultado en rax (`afinado::ligera`)
//! ```
//!
//! [!] La pila de las llamadas abiertas (`[r15]`) se mira en las dos: T0066
//! en la linea de la llamada.

use super::{forma, Class, Place, Trap, E1};
use bmo_lower::x86::{self, RAX, RCX, RDX, RDI, RSI, R11, R8, R9};
use bmo_titan_front::ir::Value;
use bmo_titan_front::tree::{Mode, Ty};

impl E1<'_> {
    /// La fn de un trait: mira el TIPO del primer valor y salta a la fn de
    /// ese tipo, con el puntero ya en el valor (sin su tipo delante).
    pub(super) fn dispatcher(&mut self, table: &[(String, usize)], line: usize) -> Result<(), String> {
        x86::mov_r64_at_reg_disp32(&mut self.code, R11, RDI, 0);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R11, 0);
        for (ty, target) in table {
            let id = self.forms.type_id(ty);
            self.imm(bmo_lower::x86::RCX, id);
            x86::cmp_r64_r64(&mut self.code, RAX, bmo_lower::x86::RCX);
            let next = self.jcc(0x85);
            self.lea(R11, R11, 8);
            x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, 0, R11);
            let j = self.jmp();
            self.calls.push((j, *target));
            self.here(next);
        }
        let j = self.jmp();
        self.traps.push(Trap { field: j, head: "NO al correr, linea ".into(), line, why: "un trait sin la fn de este tipo (fallo del compilador: avisa)".into() });
        Ok(())
    }

    /// Una llamada: los punteros a sus valores, el sitio de su resultado.
    pub fn call(&mut self, func: usize, args: &[Value], at: (usize, usize)) -> Result<Option<(Place, Class)>, String> {
        let f = &self.m.functions[func];
        if f.gpu {
            return self.gpu_call(func, args, at);
        }
        if ligera(self.m, func) {
            return self.llamada_ligera(func, args, at);
        }
        let params: Vec<(Class, Ty, Mode)> = f.params.iter().enumerate().map(|(i, (_, t))| (self.forms.class(t), t.clone(), f.modes.get(i).copied().unwrap_or(Mode::Copy))).collect();
        let ret = f.ret.as_ref().map(|t| self.forms.class(t));
        let block = self.temp(8 * args.len().max(1) as i32);
        let mut back: Vec<(Place, Place, i32)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let (pc, pt, _) = &params[i];
            let ptr = match a {
                Value::Lend(Mode::Mut, l, _) => {
                    let lc = self.f.known[*l].clone().ok_or("un local sin valor")?;
                    let lp = self.local(*l);
                    if matches!(pc, Class::Trait(_)) && !matches!(lc, Class::Trait(_)) {
                        // un `mut` hacia un trait: va con su tipo, y vuelve
                        // se MUEVE al sitio del trait y vuelve movido
                        let t = self.temp(self.forms.size(pc));
                        self.conv(t, lp, &lc, pc, true, a.at())?;
                        back.push((lp, t.at(8), self.forms.size(&lc)));
                        t
                    } else {
                        lp
                    }
                }
                _ => {
                    let (p, c) = self.eval(a)?;
                    if &c == pc {
                        p
                    } else {
                        let t = self.temp(self.forms.size(pc));
                        self.convert(t, p, &c, pc, Some(pt), a.at())?;
                        self.own(t, pc);
                        t
                    }
                }
            };
            self.addr(ptr, RAX);
            self.store(block.at(8 * i as i32), RAX);
        }
        let out = ret.map(|c| (self.temp(self.forms.size(&c)), c));
        self.addr(block, RDI);
        match &out {
            Some((p, _)) => self.addr(*p, RSI),
            None => x86::zero_r32(&mut self.code, RSI),
        }
        self.salta_a(func, at);
        for (to, from, size) in back {
            self.copy(to, from, size);
        }
        // A4b: la lamina ENTREGADA ya es de la fn: aqui no queda nada que soltar.
        for a in args {
            if let Value::Lend(Mode::Take, l, _) = a {
                if self.f.known[*l] == Some(Class::Lamina) {
                    let p = self.local(*l);
                    self.olvida_lamina(p);
                }
            }
        }
        if let Some((p, c)) = &out {
            self.own(*p, c);
        }
        Ok(out)
    }

    /// La pila que queda y el `call`. T0066 AQUI, en la linea de la llamada
    /// (donde lo dice el calculo): si `rsp` menos lo que gasta la fn baja del
    /// fondo de la pila de las llamadas (`r15` menos lo que pueden gastar). Lo que gasta se sabe al
    /// final. (M1, 16-10: antes cada fn sumaba y restaba su gasto en `[r15]`
    /// al entrar y al volver; `rsp` ya lo sabe.)
    pub(super) fn salta_a(&mut self, func: usize, at: (usize, usize)) {
        // M1: `rsp - lo que gasta la fn` no puede bajar del fondo, que es
        // `r15 - la pila de las llamadas`. Sin memoria:
        // lea r11, [rsp + pila - coste] ; cmp r11, r15 ; jb NO
        self.code.extend_from_slice(&[0x4C, 0x8D, 0x9C, 0x24, 0, 0, 0, 0]);
        self.depth_checks.push((self.code.len() - 4, func));
        self.code.extend_from_slice(&[0x4D, 0x39, 0xFB]);
        let name = self.m.functions[func].name.clone();
        self.trap(0x82, "T0066", &format!("las llamadas se anidan mas de lo que cabe en la pila (`{}` sin llegar a su caso de parada?)", forma::short(&name)), at);
        self.code.push(0xE8);
        self.calls.push((self.code.len(), func));
        self.code.extend_from_slice(&[0; 4]);
    }

    /// ** AF7: la llamada a una fn LIGERA: cada valor a su sitio de paso
    /// (calcular el segundo puede llamar y pisar los registros), y al final
    /// los cuatro registros de una vez. El resultado vuelve en `rax`.
    fn llamada_ligera(&mut self, func: usize, args: &[Value], at: (usize, usize)) -> Result<Option<(Place, Class)>, String> {
        // B4: un valor detras del cual solo quedan locales y numeros escritos
        // (que no emiten nada hasta el final) va DERECHO a su registro
        let simple = |v: &Value| matches!(v, Value::Int(..) | Value::Local(..));
        let mut sitios = Vec::with_capacity(args.len());
        for (k, a) in args.iter().enumerate() {
            sitios.push(match a {
                Value::Int(n, _) => Some(Err(*n)),
                _ if !simple(a) && args[k + 1..].iter().all(simple) => {
                    self.a_rax(a)?;
                    x86::mov_r64_r64(&mut self.code, ARGS[k], RAX);
                    None
                }
                _ => Some(Ok(self.eval(a)?.0)),
            });
        }
        for (k, s) in sitios.into_iter().enumerate() {
            match s {
                Some(Ok(p)) => self.load(p, ARGS[k]),
                Some(Err(n)) => self.imm(ARGS[k], n),
                None => {}
            }
        }
        self.salta_a(func, at);
        if self.m.functions[func].ret.is_none() {
            return Ok(None);
        }
        let t = self.temp(8);
        self.store(t, RAX);
        Ok(Some((t, Class::Int)))
    }

    /// AF7: lo que llega a una fn ligera: cada registro, derecho a su local.
    pub(super) fn llegan_ligeros(&mut self, f: &bmo_titan_front::ir::Function) {
        for (i, (l, t)) in f.params.iter().enumerate() {
            let p = self.local(*l);
            self.store(p, ARGS[i]);
            self.f.known[*l] = Some(Class::Int);
            self.f.decl[*l] = Some(t.clone());
        }
    }

    /// AF7: lo que vuelve de una fn ligera, en `rax`. Lo que muere al volver
    /// (`muere`) se suelta con el valor guardado aparte; si soltar no emitio
    /// nada, ni se guarda.
    pub(super) fn vuelve_ligero(&mut self, v: &Value, muere: &mut dyn FnMut(&mut Self)) -> Result<(), String> {
        self.a_rax(v)?;
        let t = self.temp(8);
        let antes = self.code.len();
        self.store(t, RAX);
        let tras = self.code.len();
        muere(self);
        if self.code.len() == tras {
            self.code.truncate(antes);
            self.guardado = None;
        } else {
            self.load(t, RAX);
        }
        Ok(())
    }
}

/// Los registros de los valores de una fn ligera, en orden.
const ARGS: [u8; 4] = [RDX, RCX, R8, R9];

/// ** AF7: UNA FN LIGERA -- de 1 a 4 valores `int` COPIADOS y un `int` (o
/// nada) de vuelta. Sus valores van en registros y su resultado en `rax`: sin
/// el bloque de punteros, sin el sitio del resultado, y sin copiar cada valor
/// de un puntero al entrar. Ni la de un trait (su salto lleva el bloque), ni
/// una fn a la que salta un trait, ni `main`.
pub(super) fn ligera(m: &bmo_titan_front::ir::Module, func: usize) -> bool {
    let f = &m.functions[func];
    if f.gpu || f.dispatch.is_some() || func == m.entry || f.params.is_empty() || f.params.len() > ARGS.len() {
        return false;
    }
    if m.functions.iter().any(|g| g.dispatch.as_ref().is_some_and(|t| t.iter().any(|(_, k)| *k == func))) {
        return false;
    }
    let entero = |t: &Ty| matches!(t, Ty::Int);
    f.params.iter().enumerate().all(|(i, (_, t))| entero(t) && f.modes.get(i).copied().unwrap_or(Mode::Copy) == Mode::Copy) && f.ret.as_ref().is_none_or(entero)
}
