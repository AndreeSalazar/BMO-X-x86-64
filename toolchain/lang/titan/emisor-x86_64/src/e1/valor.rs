//! **LOS VALORES AL CORRER** -- cada valor de la IR, calculado a un sitio de
//! memoria con la forma de su clase (`forma.rs`), y lo que se hace con ellos
//! sin aritmetica: copiar, convertir (`13` en un `dec` es `13`, escala 0),
//! mirar el PIC de un `dec(p, s)`, entrar en `t[i].x`, y comparar por VALOR
//! (12.50 == 12.5, tablas celda a celda, casos con lo que llevan) -- como
//! `calc.rs`, que es la vara.

use super::{Base, Helper, Place, E1, TEXT_CAP};
use bmo_lower::memoria;
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{PathStep, Value};
use bmo_titan_front::tree::Ty;

impl E1<'_> {
    /// Un valor, calculado: donde queda y de que clase es. Un local, un campo
    /// o una celda no se copian: se devuelve SU sitio.
    pub fn eval(&mut self, v: &Value) -> Result<(Place, Class), String> {
        Ok(match v {
            Value::Int(n, _) => {
                let t = self.temp(8);
                self.store_imm(t, *n);
                (t, Class::Int)
            }
            Value::Bool(b, _) => {
                let t = self.temp(8);
                self.store_imm(t, *b as i64);
                (t, Class::Bool)
            }
            Value::Dec(d, s, _) => {
                let t = self.temp(16);
                self.store_imm(t, *d);
                self.store_imm(t.at(8), *s as i64);
                (t, Class::Dec)
            }
            Value::Text(s, at) => {
                let t = self.temp(8 + TEXT_CAP as i32);
                self.put_text(t, s, *at)?;
                (t, Class::Text)
            }
            // ** un f32 (LB4): sus bits, la mitad alta a cero. E1 no cuenta
            // con el (D2): lo guarda, lo copia y se lo pasa a una gpu fn
            Value::F32(bits, _) => {
                let t = self.temp(8);
                self.store_imm(t, *bits as i64);
                (t, Class::F32)
            }
            Value::Local(l, _) | Value::Lend(_, l, _) => (self.local(*l), self.f.known[*l].clone().ok_or("un local sin valor (fallo del compilador)")?),
            Value::Read(at) => {
                let t = self.temp(8 + TEXT_CAP as i32);
                self.read_into(t, *at);
                (t, Class::Text)
            }
            Value::Number(inner, e, _) => self.number(inner, *e)?,
            Value::Bin(op, a, b, at) => self.bin(op, a, b, *at)?,
            Value::Neg(a, at) => self.neg(a, *at)?,
            Value::Not(a, _) => {
                let (p, _) = self.eval(a)?;
                self.load(p, RAX);
                self.imm(RCX, 1);
                x86::xor_r64_r64(&mut self.code, RAX, RCX);
                let t = self.temp(8);
                self.store(t, RAX);
                (t, Class::Bool)
            }
            Value::Call(func, args, at) => self.call(*func, args, *at)?.ok_or_else(|| format!("linea {}: una llamada sin valor usada como valor", at.0))?,
            // LB7b: lo que contesta el director, un si/no (`director.rs`).
            Value::Director(what, args, at) => self.director(*what, args, *at)?.ok_or_else(|| format!("linea {}: `{}` no da un valor (fallo del compilador)", at.0, what.name()))?,
            Value::Round(inner, n, at) => self.round(inner, *n, *at)?,
            // `[]` (nivel 13): una lista sin nada, el asa a cero
            Value::Table(items, _) if items.is_empty() => {
                let t = self.temp(24);
                for k in 0..3 {
                    self.store_imm(t.at(8 * k), 0);
                }
                (t, Class::List(Box::new(Class::Any)))
            }
            Value::Table(items, at) => {
                let c = self.class_of(v)?;
                let Class::Table(inner, n) = &c else { return Err(format!("linea {}: una tabla sin clase de tabla", at.0)) };
                let sz = self.forms.size(inner);
                let t = self.temp(sz * *n as i32);
                for (i, item) in items.iter().enumerate() {
                    let (p, ic) = self.eval(item)?;
                    self.convert(t.at(i as i32 * sz), p, &ic, inner, None, *at)?;
                }
                if **inner == Class::Dec {
                    self.rescale_cells(t, *n, *at);
                }
                self.own(t, &c);
                (t, c)
            }
            Value::Repeat(item, n, _) => {
                let (p, ic) = self.eval(item)?;
                let sz = self.forms.size(&ic);
                let t = self.temp(sz * *n as i32);
                // cada celda, su COPIA (con monton dentro, entera)
                let one = ic.clone();
                self.each_cell(*n, &[(t, sz)], &mut |e, cells| {
                    e.clone_at(cells[0], p, &one);
                    Ok(())
                })?;
                let c = Class::Table(Box::new(ic), *n);
                self.own(t, &c);
                (t, c)
            }
            Value::Index(b, i, at) => {
                let (pb, cb) = self.eval(b)?;
                if let Class::List(inner) = &cb {
                    let p = self.list_index(pb, inner, i, *at)?;
                    return Ok((p, (**inner).clone()));
                }
                let Class::Table(inner, n) = cb else { return Err(format!("linea {}: una celda de algo que no es tabla", at.0)) };
                let p = self.index_place(pb, &inner, n, i, *at)?;
                (p, *inner)
            }
            Value::Field(b, name, at) => {
                let (pb, cb) = self.eval(b)?;
                let Class::Record(t) = cb else { return Err(format!("linea {}: un campo de algo que no es registro", at.0)) };
                let (off, fc, _) = self.forms.field(t, name);
                (pb.at(off), fc)
            }
            Value::Record(ti, items, at) => {
                let c = Class::Record(*ti);
                let t = self.temp(self.forms.size(&c));
                for (k, item) in items.iter().enumerate() {
                    let (off, fc, fty) = self.forms.field_k(*ti, k);
                    let (p, ic) = self.eval(item)?;
                    self.convert(t.at(off), p, &ic, &fc, Some(&fty), *at)?;
                }
                self.own(t, &c);
                (t, c)
            }
            Value::Variant(e, k, items, at) => {
                let c = Class::Enum(*e);
                let t = self.temp(self.forms.size(&c));
                for (j, item) in items.iter().enumerate() {
                    let (off, fc, fty) = self.forms.case_field(*e, *k, j);
                    let (p, ic) = self.eval(item)?;
                    self.convert(t.at(off), p, &ic, &fc, Some(&fty), *at)?;
                }
                self.store_imm(t, *k as i64);
                self.own(t, &c);
                (t, c)
            }
            Value::Is(inner, _, k, _) => {
                let (p, _) = self.eval(inner)?;
                self.load(p, RAX);
                self.imm(RCX, *k as i64);
                x86::cmp_r64_r64(&mut self.code, RAX, RCX);
                x86::setcc_low(&mut self.code, 0x94, RAX);
                x86::movzx_r64_low(&mut self.code, RAX, RAX);
                let t = self.temp(8);
                self.store(t, RAX);
                (t, Class::Bool)
            }
            Value::Payload(inner, e, k, j, _) => {
                let (p, c) = self.eval(inner)?;
                // `Hay(v)` (nivel 13): su valor, de la clase que trae
                if let Class::Opt(t) = c {
                    return Ok((p.at(8), *t));
                }
                let (off, fc, _) = self.forms.case_field(*e, *k, *j);
                (p.at(off), fc)
            }
            Value::Lib(lib, args, at) => self.lib(*lib, args, *at)?,
            Value::Map(items, at) => {
                let c = self.class_of(v)?;
                (self.map_written(items, &c, *at)?, c)
            }
            Value::Len(inner, at) => {
                let (p, c) = self.eval(inner)?;
                if matches!(c, Class::List(_) | Class::Map(..) | Class::Any) {
                    return Ok((self.len_of(p), Class::Int));
                }
                let Class::Table(_, n) = c else { return Err(format!("linea {}: `len` de algo que no es tabla", at.0)) };
                let t = self.temp(8);
                self.store_imm(t, n as i64);
                (t, Class::Int)
            }
        })
    }

    /// Un texto escrito, a su sitio: de ocho en ocho bytes como inmediatos.
    pub fn put_text(&mut self, t: Place, s: &str, at: (usize, usize)) -> Result<(), String> {
        let bytes = s.as_bytes();
        if bytes.len() > TEXT_CAP {
            return Err(format!("linea {}: un texto de {} bytes; al correr un texto guarda {} como mucho", at.0, bytes.len(), TEXT_CAP));
        }
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            self.store_imm(t.at(8 + 8 * i as i32), i64::from_le_bytes(w));
        }
        self.store_imm(t, bytes.len() as i64);
        Ok(())
    }

    /// Las celdas de `n`, una a una: escritas en linea si son pocas, en un
    /// bucle con un puntero por tabla si son muchas. `bases`: el sitio de la
    /// primera celda de cada tabla y lo que mide una celda.
    pub fn each_cell(&mut self, n: usize, bases: &[(Place, i32)], body: &mut dyn FnMut(&mut Self, &[Place]) -> Result<(), String>) -> Result<(), String> {
        if n <= 6 {
            for i in 0..n {
                let cells: Vec<Place> = bases.iter().map(|(p, sz)| p.at(i as i32 * sz)).collect();
                body(self, &cells)?;
            }
            return Ok(());
        }
        let mut ptrs = Vec::with_capacity(bases.len());
        for (p, _) in bases {
            self.addr(*p, RAX);
            let s = self.temp(8);
            self.store(s, RAX);
            ptrs.push(s);
        }
        let count = self.temp(8);
        self.store_imm(count, n as i64);
        let top = self.code.len();
        let cells: Vec<Place> = ptrs.iter().map(|s| Place { base: Base::Ptr(s.off), off: 0 }).collect();
        body(self, &cells)?;
        for ((_, sz), s) in bases.iter().zip(&ptrs) {
            self.load(*s, RAX);
            self.lea(RAX, RAX, *sz);
            self.store(*s, RAX);
        }
        self.load(count, RAX);
        x86::dec_r64(&mut self.code, RAX);
        self.store(count, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let back = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, back, top);
        Ok(())
    }

    /// `t[i]`: el sitio de la celda, mirando que este dentro (T0072).
    pub fn index_place(&mut self, base: Place, inner: &Class, n: usize, idx: &Value, at: (usize, usize)) -> Result<Place, String> {
        let (pi, _) = self.eval(idx)?;
        self.load(pi, RAX);
        x86::cmp_r64_imm8(&mut self.code, RAX, 0);
        self.trap(0x8C, "T0072", &format!("una celda fuera de la tabla (tiene {}: de 0 a {})", n, n.saturating_sub(1)), at);
        self.imm(RCX, n as i64);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        self.trap(0x8D, "T0072", &format!("una celda fuera de la tabla (tiene {}: de 0 a {})", n, n.saturating_sub(1)), at);
        self.imm(RCX, self.forms.size(inner) as i64);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        self.addr(base, RDX);
        x86::add_r64_r64(&mut self.code, RDX, RAX);
        Ok(self.pointer_from(RDX))
    }

    /// El sitio de `local` mas su camino (`t[i].x = v`), su clase y el tipo
    /// que se le DECLARO, si se le declaro.
    pub fn path(&mut self, local: usize, path: &[PathStep], at: (usize, usize)) -> Result<(Place, Class, Option<Ty>), String> {
        let mut p = self.local(local);
        let mut c = self.f.known[local].clone().ok_or("un local sin valor")?;
        let mut ty = self.f.decl[local].clone();
        for st in path {
            match (st, c.clone()) {
                (PathStep::Field(name, _), Class::Record(t)) => {
                    let (off, fc, fty) = self.forms.field(t, name);
                    p = p.at(off);
                    c = fc;
                    ty = Some(fty);
                }
                (PathStep::Index(i), Class::List(inner)) => {
                    p = self.list_index(p, &inner, i, at)?;
                    c = *inner;
                    ty = match ty {
                        Some(Ty::List(it)) => Some(*it),
                        _ => None,
                    };
                }
                (PathStep::Index(i), Class::Table(inner, n)) => {
                    p = self.index_place(p, &inner, n, i, at)?;
                    c = *inner;
                    ty = match ty {
                        Some(Ty::Table(it, _)) => Some(*it),
                        _ => None,
                    };
                }
                _ => return Err(format!("linea {}: un camino que no lleva a ningun sitio", at.0)),
            }
        }
        Ok((p, c, ty))
    }

    /// Un valor de clase `from` a un sitio de clase `to`, y el PIC de su tipo
    /// declarado si lleva un `dec(p, s)`.
    /// Si `src` es un sitio de paso de la operacion, se MUEVE (sus bytes, y
    /// ya no se suelta al acabar); si no, se CLONA (nivel 13).
    pub fn convert(&mut self, dst: Place, src: Place, from: &Class, to: &Class, ty: Option<&Ty>, at: (usize, usize)) -> Result<(), String> {
        let moving = self.take_owned(src);
        self.conv(dst, src, from, to, moving, at)?;
        if let Some(t) = ty {
            if self.forms.has_decp(t) {
                self.fit(dst, t, at)?;
            }
        }
        Ok(())
    }

    /// `convert` sin el PIC: `moving` dice si lo de `src` se lleva (sus
    /// bytes) o se copia entero.
    pub fn conv(&mut self, dst: Place, src: Place, from: &Class, to: &Class, moving: bool, at: (usize, usize)) -> Result<(), String> {
        match (from, to) {
            (a, b) if a == b => {
                if moving {
                    let size = self.forms.size(to);
                    self.copy(dst, src, size);
                } else {
                    self.clone_at(dst, src, to);
                }
            }
            // ** a f32 (LB4): un int o un dec, al mas cercano (`gpu.rs`)
            (Class::Int | Class::Dec, Class::F32) => self.to_f32(dst, src, from),
            // `[]` y `{}`: el asa a cero, a una lista o un mapa de lo que sea
            (Class::List(f), Class::List(_)) | (Class::Map(f, _), Class::Map(..)) if **f == Class::Any => self.copy(dst, src, 24),
            (Class::Table(fi, n), Class::List(ti)) => {
                let (fi, ti) = ((**fi).clone(), (**ti).clone());
                self.list_from_table(dst, src, &fi, *n, &ti, moving, at)?;
            }
            (Class::List(_), Class::List(_)) | (Class::Map(..), Class::Map(..)) => self.retype_cells(dst, src, from, to, moving, at)?,
            (Class::Opt(fi), Class::Opt(ti)) => {
                let (fi, ti) = ((**fi).clone(), (**ti).clone());
                self.load(src, RAX);
                self.store(dst, RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let none = self.jcc(0x85);
                self.conv(dst.at(8), src.at(8), &fi, &ti, moving, at)?;
                self.here(none);
            }
            (Class::Int, Class::Dec) => {
                self.load(src, RAX);
                self.store(dst, RAX);
                self.store_imm(dst.at(8), 0);
            }
            (Class::Table(fi, n), Class::Table(ti, _)) => {
                let (fs, ts) = (self.forms.size(fi), self.forms.size(ti));
                let (fi, ti) = ((**fi).clone(), (**ti).clone());
                self.each_cell(*n, &[(dst, ts), (src, fs)], &mut |e, cells| e.conv(cells[0], cells[1], &fi, &ti, moving, at))?;
            }
            (c, Class::Trait(_)) if !matches!(c, Class::Trait(_)) => {
                let name = self.forms.type_name(c).ok_or_else(|| format!("linea {}: este valor no tiene tipo para un trait", at.0))?;
                let id = self.forms.type_id(&name);
                self.conv(dst.at(8), src, c, c, moving, at)?;
                self.store_imm(dst, id);
            }
            (a, b) => {
                let what = format!("pasar {:?} a {:?}", a, b);
                return Err(self.not_yet(&what, super::E1_WHY, super::E1_HOW, at));
            }
        }
        Ok(())
    }

    /// El PIC de COBOL (`calc::fit_into`): cada `dec(p, s)` dentro del valor,
    /// con sus `s` decimales y sus `p` cifras, o T0074.
    pub fn fit(&mut self, p: Place, ty: &Ty, at: (usize, usize)) -> Result<(), String> {
        match ty {
            Ty::DecP(digits, scale) => {
                self.load(p, RAX);
                self.load(p.at(8), RCX);
                self.imm(RDX, *digits as i64);
                self.imm(R8, *scale as i64);
                self.call_helper(Helper::DecFit);
                self.trap_status(at);
                self.store(p, RAX);
                self.store(p.at(8), RCX);
            }
            Ty::Table(inner, n) if self.forms.has_decp(inner) => {
                let c = self.forms.class(inner);
                let sz = self.forms.size(&c);
                let inner = (**inner).clone();
                self.each_cell(*n, &[(p, sz)], &mut |e, cells| e.fit(cells[0], &inner, at))?;
            }
            Ty::List(_) | Ty::Map(..) | Ty::Opt(_) => self.fit_collection(p, ty, at)?,
            Ty::Named(_) => match self.forms.class(ty) {
                Class::Record(t) => {
                    for k in 0..self.m.types[t].fields.len() {
                        let (off, _, fty) = self.forms.field_k(t, k);
                        if self.forms.has_decp(&fty) {
                            self.fit(p.at(off), &fty, at)?;
                        }
                    }
                }
                Class::Enum(e) => {
                    for v in 0..self.m.enums[e].cases.len() {
                        let tys = self.m.enums[e].cases[v].fields.clone();
                        if !tys.iter().any(|t| self.forms.has_decp(t)) {
                            continue;
                        }
                        self.load(p, RAX);
                        self.imm(RCX, v as i64);
                        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
                        let next = self.jcc(0x85);
                        for (k, t) in tys.iter().enumerate() {
                            if self.forms.has_decp(t) {
                                let (off, _, _) = self.forms.case_field(e, v, k);
                                self.fit(p.at(off), t, at)?;
                            }
                        }
                        self.here(next);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Un `int` a un local `dec` sin tipo declarado: con la escala que ese
    /// local ya tiene (`calc.rs`: 13 en un dec de 2 decimales es 13.00).
    pub fn int_to_dec_like(&mut self, dst: Place, src: Place, at: (usize, usize)) {
        self.load(src, RAX);
        self.load(dst.at(8), RCX);
        self.call_helper(Helper::Pow10);
        self.trap_status(at);
        self.store(dst, RAX);
    }

    /// Las celdas `dec` de una tabla recien escrita, todas a la escala mayor
    /// (`[1, 2.5]` es `[1.0, 2.5]`, como en el calculo).
    fn rescale_cells(&mut self, t: Place, n: usize, at: (usize, usize)) {
        let max = self.temp(8);
        self.store_imm(max, 0);
        for i in 0..n {
            self.load(t.at(16 * i as i32 + 8), RAX);
            self.load(max, RCX);
            x86::cmp_r64_r64(&mut self.code, RAX, RCX);
            let skip = self.jcc(0x8E);
            self.store(max, RAX);
            self.here(skip);
        }
        for i in 0..n {
            let cell = t.at(16 * i as i32);
            self.load(max, RCX);
            self.load(cell.at(8), RAX);
            x86::sub_r64_r64(&mut self.code, RCX, RAX);
            self.load(cell, RAX);
            self.call_helper(Helper::Pow10);
            self.trap_status(at);
            self.store(cell, RAX);
            self.load(max, RAX);
            self.store(cell.at(8), RAX);
        }
    }

    /// Un numero como `dec` en dos registros: sus cifras y su escala.
    pub fn load_dec(&mut self, p: Place, c: &Class, digits: u8, scale: u8) {
        self.load(p, digits);
        if *c == Class::Dec {
            self.load(p.at(8), scale);
        } else {
            self.imm(scale, 0);
        }
    }

    /// `a == b` POR VALOR, a un si/no nuevo.
    pub fn equal(&mut self, pa: Place, ca: &Class, pb: Place, cb: &Class, at: (usize, usize)) -> Result<Place, String> {
        let mut fails = Vec::new();
        self.eq_into(pa, ca, pb, cb, &mut fails, at)?;
        let t = self.temp(8);
        self.store_imm(t, 1);
        let done = self.jmp();
        for f in fails {
            self.here(f);
        }
        self.store_imm(t, 0);
        self.here(done);
        Ok(t)
    }

    pub fn eq_into(&mut self, pa: Place, ca: &Class, pb: Place, cb: &Class, fails: &mut Vec<usize>, at: (usize, usize)) -> Result<(), String> {
        match (ca, cb) {
            (Class::Int, Class::Int) | (Class::Bool, Class::Bool) => {
                self.load(pa, RAX);
                self.load(pb, RDX);
                x86::cmp_r64_r64(&mut self.code, RAX, RDX);
                fails.push(self.jcc(0x85));
            }
            (Class::Int | Class::Dec, Class::Int | Class::Dec) => {
                self.load_dec(pa, ca, RAX, RCX);
                self.load_dec(pb, cb, RDX, R8);
                self.call_helper(Helper::DecCmp);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                fails.push(self.jcc(0x85));
            }
            (Class::Text, Class::Text) => {
                self.load(pa, RDX);
                self.load(pb, RAX);
                x86::cmp_r64_r64(&mut self.code, RDX, RAX);
                fails.push(self.jcc(0x85));
                self.addr(pa.at(8), RDI);
                self.addr(pb.at(8), RSI);
                memoria::comparar_n(&mut self.code, false);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                fails.push(self.jcc(0x85));
            }
            (Class::Table(ia, n), Class::Table(ib, _)) => {
                let (sa, sb) = (self.forms.size(ia), self.forms.size(ib));
                let (ia, ib) = ((**ia).clone(), (**ib).clone());
                let mut inner = Vec::new();
                self.each_cell(*n, &[(pa, sa), (pb, sb)], &mut |e, cells| e.eq_into(cells[0], &ia, cells[1], &ib, &mut inner, at))?;
                fails.extend(inner);
            }
            (Class::Record(t), Class::Record(_)) => {
                for k in 0..self.m.types[*t].fields.len() {
                    let (off, fc, _) = self.forms.field_k(*t, k);
                    self.eq_into(pa.at(off), &fc, pb.at(off), &fc, fails, at)?;
                }
            }
            (Class::Enum(e), Class::Enum(_)) => {
                self.load(pa, RAX);
                self.load(pb, RDX);
                x86::cmp_r64_r64(&mut self.code, RAX, RDX);
                fails.push(self.jcc(0x85));
                for v in 0..self.m.enums[*e].cases.len() {
                    let k = self.m.enums[*e].cases[v].fields.len();
                    if k == 0 {
                        continue;
                    }
                    self.load(pa, RAX);
                    self.imm(RCX, v as i64);
                    x86::cmp_r64_r64(&mut self.code, RAX, RCX);
                    let next = self.jcc(0x85);
                    for j in 0..k {
                        let (off, fc, _) = self.forms.case_field(*e, v, j);
                        self.eq_into(pa.at(off), &fc, pb.at(off), &fc, fails, at)?;
                    }
                    self.here(next);
                }
            }
            (Class::List(_) | Class::Map(..) | Class::Opt(_) | Class::Any, _) | (_, Class::List(_) | Class::Map(..) | Class::Opt(_) | Class::Any) => self.eq_collection(pa, ca, pb, cb, fails, at)?,
            (a, b) => {
                let what = format!("comparar {:?} con {:?}", a, b);
                return Err(self.not_yet(&what, super::EQ_WHY, super::EQ_HOW, at));
            }
        }
        let _ = R9;
        Ok(())
    }
}
