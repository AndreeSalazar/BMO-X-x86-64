//! **LAS LISTAS Y LOS MAPAS AL CORRER** (nivel 13,
//! `docs/plan/PLAN_LISTAS_Y_MAPAS.md`, L4 y L5).
//!
//! ```text
//!    el asa        [puntero, cuantas, cuantas caben]: 24 bytes donde viva
//!                  el valor (un local, un campo, la celda de otra lista)
//!    las celdas    en el monton, seguidas, con la forma de su clase; las de
//!                  un mapa son sus ENTRADAS, clave y valor, en el orden en
//!                  que entraron (D4) -- buscar una clave es recorrerlas
//!    push / put    EN SU SITIO: `l = push(l, x)` de la IR no copia la
//!                  lista; si no cabe, crece al doble (un bloque nuevo, se
//!                  pasan las celdas, se suelta el viejo)
//!    pop / get     el caso de `Opcion`: Hay (0) con una COPIA de lo que hay,
//!                  o NoHay (1)
//! ```
//!
//! La vara es `calc/coleccion.rs`: lo mismo, con los mismos casos.

use super::{Place, E1};
use bmo_lower::x86::{self, RAX, RCX, RDI, RSI};
use bmo_lower::{console, memoria};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{Lib, Value};
use bmo_titan_front::tree::Ty;

impl E1<'_> {
    /// `len(l)` de una lista o un mapa: lo que dice su asa.
    pub fn len_of(&mut self, h: Place) -> Place {
        let t = self.temp(8);
        self.load(h.at(8), RAX);
        self.store(t, RAX);
        t
    }

    /// La celda `i` (un sitio con un int) de lo que apunta el asa `h`.
    fn cell_at(&mut self, h: Place, cell: i32, i: Place) -> Place {
        self.load(i, RAX);
        self.imm(RCX, cell as i64);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        self.load(h, RCX);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.pointer_from(RAX)
    }

    /// `l[i]` de una lista: dentro, o T0072.
    pub fn list_index(&mut self, h: Place, t: &Class, idx: &Value, at: (usize, usize)) -> Result<Place, String> {
        let (pi, _) = self.eval(idx)?;
        self.load(pi, RAX);
        x86::cmp_r64_imm8(&mut self.code, RAX, 0);
        self.trap(0x8C, "T0072", "una celda fuera de la lista", at);
        self.load(h.at(8), RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        self.trap(0x8D, "T0072", "una celda fuera de la lista", at);
        let cell = self.forms.size(t);
        Ok(self.cell_at(h, cell, pi))
    }

    /// Lo que dan `get`, `has`, `pop` (su ultimo) y una vuelta de `for`.
    pub fn lib(&mut self, lib: Lib, args: &[Value], at: (usize, usize)) -> Result<(Place, Class), String> {
        let (h, c) = self.eval(&args[0])?;
        Ok(match (lib, &c) {
            (Lib::Last, Class::List(t)) => {
                let t = (**t).clone();
                let out = self.temp(8 + self.forms.size(&t));
                let n = self.temp(8);
                self.load(h.at(8), RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let none = self.jcc(0x84);
                x86::dec_r64(&mut self.code, RAX);
                self.store(n, RAX);
                let cell = self.forms.size(&t);
                let src = self.cell_at(h, cell, n);
                self.clone_at(out.at(8), src, &t);
                self.store_imm(out, 0);
                let done = self.jmp();
                self.here(none);
                self.store_imm(out, 1);
                self.here(done);
                let oc = Class::Opt(Box::new(t));
                self.own(out, &oc);
                (out, oc)
            }
            (Lib::Get | Lib::Has, Class::Map(k, v)) => {
                let (k, v) = ((**k).clone(), (**v).clone());
                let (pk, ck) = self.eval(&args[1])?;
                let i = self.map_find(h, &k, &v, pk, &ck, at)?;
                if lib == Lib::Has {
                    let out = self.temp(8);
                    self.load(i, RAX);
                    x86::test_r64_r64(&mut self.code, RAX, RAX);
                    x86::setcc_low(&mut self.code, 0x99, RAX); // setns
                    x86::movzx_r64_low(&mut self.code, RAX, RAX);
                    self.store(out, RAX);
                    (out, Class::Bool)
                } else {
                    let out = self.temp(8 + self.forms.size(&v));
                    self.load(i, RAX);
                    x86::test_r64_r64(&mut self.code, RAX, RAX);
                    let none = self.jcc(0x88);
                    let (cell, _) = self.cells_of(&c);
                    let entry = self.cell_at(h, cell, i);
                    let ks = self.forms.size(&k);
                    self.clone_at(out.at(8), entry.at(ks), &v);
                    self.store_imm(out, 0);
                    let done = self.jmp();
                    self.here(none);
                    self.store_imm(out, 1);
                    self.here(done);
                    let oc = Class::Opt(Box::new(v));
                    self.own(out, &oc);
                    (out, oc)
                }
            }
            (Lib::Turn, Class::Table(t, n)) => {
                let p = self.index_place(h, t, *n, &args[1], at)?;
                (p, (**t).clone())
            }
            (Lib::Turn, Class::List(t)) => {
                let (pi, _) = self.eval(&args[1])?;
                let cell = self.forms.size(t);
                (self.cell_at(h, cell, pi), (**t).clone())
            }
            (Lib::Turn, Class::Map(k, _)) => {
                let (pi, _) = self.eval(&args[1])?;
                let (cell, _) = self.cells_of(&c);
                (self.cell_at(h, cell, pi), (**k).clone())
            }
            (lib, c) => {
                let what = format!("`{}` sobre {:?}", lib.name(), c);
                return Err(self.not_yet(&what, super::E1_WHY, super::E1_HOW, at));
            }
        })
    }

    /// Donde esta la clave `key` en el mapa del asa `h`: un sitio con su
    /// numero de entrada, o -1.
    pub fn map_find(&mut self, h: Place, k: &Class, v: &Class, key: Place, kc: &Class, at: (usize, usize)) -> Result<Place, String> {
        let cell = self.forms.size(k) + self.forms.size(v);
        let i = self.temp(8);
        let found = self.temp(8);
        self.store_imm(i, 0);
        self.store_imm(found, -1);
        let top = self.code.len();
        self.load(i, RAX);
        self.load(h.at(8), RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let out = self.jcc(0x8D);
        let entry = self.cell_at(h, cell, i);
        let mut fails = Vec::new();
        self.eq_into(entry, k, key, kc, &mut fails, at)?;
        self.load(i, RAX);
        self.store(found, RAX);
        let done = self.jmp();
        for f in fails {
            self.here(f);
        }
        self.load(i, RAX);
        x86::inc_r64(&mut self.code, RAX);
        self.store(i, RAX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(out);
        self.here(done);
        Ok(found)
    }

    /// Si el asa `h` esta llena, el doble de sitio (4 la primera vez).
    fn grow(&mut self, h: Place, cell: i32) {
        self.load(h.at(8), RAX);
        self.load(h.at(16), RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let room = self.jcc(0x8C);
        let cap = self.temp(8);
        x86::shl_r64_imm8(&mut self.code, RCX, 1);
        x86::cmp_r64_imm8(&mut self.code, RCX, 4);
        let ok = self.jcc(0x8D);
        self.imm(RCX, 4);
        self.here(ok);
        self.store(cap, RCX);
        self.imm(RAX, cell as i64);
        x86::imul_r64_r64(&mut self.code, RCX, RAX);
        x86::mov_r64_r64(&mut self.code, RDI, RCX);
        self.alloc();
        let fresh = self.temp(8);
        self.store(fresh, RAX);
        x86::mov_r64_r64(&mut self.code, RDI, RAX);
        self.load(h, RSI);
        self.load(h.at(8), RCX);
        self.imm(RAX, cell as i64);
        x86::imul_r64_r64(&mut self.code, RCX, RAX);
        memoria::copiar(&mut self.code);
        self.load(h, RDI);
        self.free();
        self.load(fresh, RAX);
        self.store(h, RAX);
        self.load(cap, RAX);
        self.store(h.at(16), RAX);
        self.here(room);
    }

    /// `l = push(l, x)`, `put`, `remove` y lo que `pop` deja: EN SU SITIO.
    pub fn mutate(&mut self, local: usize, lib: Lib, args: &[Value], at: (usize, usize)) -> Result<(), String> {
        let c = self.f.known[local].clone().ok_or("un local sin valor")?;
        let decl = self.f.decl[local].clone();
        let h = self.local(local);
        self.mutate_at(h, &c, decl, lib, args, at)
    }

    /// Lo mismo sobre el asa que esta en `h` (un local, o una PARTE de uno:
    /// `push(mut nave.carga, x)`), de clase `c` y tipo declarado `decl`.
    pub fn mutate_at(&mut self, h: Place, c: &Class, decl: Option<Ty>, lib: Lib, args: &[Value], at: (usize, usize)) -> Result<(), String> {
        let c = c.clone();
        let mut vals = Vec::with_capacity(args.len());
        for a in &args[1..] {
            vals.push(self.eval(a)?);
        }
        // lo que entra es NUESTRO antes de tocar la lista: si apuntaba a una
        // celda de ella (`push(mut l, l[0])`), crecer la movera
        let vals: Vec<(Place, Class)> = vals.into_iter().map(|(p, c)| (self.mine(p, &c), c)).collect();
        let (cell, _) = self.cells_of(&c);
        match (lib, &c) {
            (Lib::Push, Class::List(t)) => {
                let t = (**t).clone();
                self.grow(h, cell);
                let n = h.at(8);
                let dst = self.cell_at(h, cell, n);
                let (px, cx) = vals[0].clone();
                self.conv(dst, px, &cx, &t, true, at)?;
                if let Some(Ty::List(it)) = &decl {
                    if self.forms.has_decp(it) {
                        self.fit(dst, it, at)?;
                    }
                }
                self.load(h.at(8), RAX);
                x86::inc_r64(&mut self.code, RAX);
                self.store(h.at(8), RAX);
            }
            (Lib::DropLast, Class::List(t)) => {
                let t = (**t).clone();
                self.load(h.at(8), RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let empty = self.jcc(0x84);
                x86::dec_r64(&mut self.code, RAX);
                self.store(h.at(8), RAX);
                let last = self.cell_at(h, cell, h.at(8));
                self.drop_at(last, &t);
                self.here(empty);
            }
            (Lib::Put, Class::Map(k, v)) => {
                let (k, v) = ((**k).clone(), (**v).clone());
                let (pk, ck) = vals[0].clone();
                let (pv, cv) = vals[1].clone();
                let (kt, vt) = match &decl {
                    Some(Ty::Map(a, b)) => (Some((**a).clone()), Some((**b).clone())),
                    _ => (None, None),
                };
                self.map_put(h, &k, &v, (pk, ck), (pv, cv), kt.as_ref(), vt.as_ref(), at)?;
            }
            (Lib::Remove, Class::Map(k, v)) => {
                let (k, v) = ((**k).clone(), (**v).clone());
                let (pk, ck) = vals[0].clone();
                let i = self.map_find(h, &k, &v, pk, &ck, at)?;
                self.drop_at(pk, &ck);
                self.load(i, RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let none = self.jcc(0x88);
                let entry = self.cell_at(h, cell, i);
                self.drop_at(entry, &k);
                let ks = self.forms.size(&k);
                self.drop_at(entry.at(ks), &v);
                // las de detras, una entrada hacia delante
                self.addr(entry, RDI);
                self.lea(RSI, RDI, cell);
                self.load(h.at(8), RCX);
                self.load(i, RAX);
                x86::sub_r64_r64(&mut self.code, RCX, RAX);
                x86::dec_r64(&mut self.code, RCX);
                self.imm(RAX, cell as i64);
                x86::imul_r64_r64(&mut self.code, RCX, RAX);
                memoria::copiar(&mut self.code);
                self.load(h.at(8), RAX);
                x86::dec_r64(&mut self.code, RAX);
                self.store(h.at(8), RAX);
                self.here(none);
            }
            (lib, c) => {
                let what = format!("`{}` sobre {:?}", lib.name(), c);
                return Err(self.not_yet(&what, super::E1_WHY, super::E1_HOW, at));
            }
        }
        Ok(())
    }

    /// `put` en el mapa del asa `h`: su valor nuevo si la clave esta; si no,
    /// una entrada mas al final (D4). `key` y `val` son NUESTROS (`mine`): en
    /// cada camino, o entran en el mapa (movidos), o se sueltan.
    #[allow(clippy::too_many_arguments)]
    pub fn map_put(&mut self, h: Place, k: &Class, v: &Class, key: (Place, Class), val: (Place, Class), kt: Option<&Ty>, vt: Option<&Ty>, at: (usize, usize)) -> Result<(), String> {
        let cell = self.forms.size(k) + self.forms.size(v);
        let ks = self.forms.size(k);
        let i = self.map_find(h, k, v, key.0, &key.1, at)?;
        self.load(i, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let new = self.jcc(0x88);
        // la clave ya esta: su valor viejo se suelta, el nuevo entra; la
        // clave que llego sobra
        let entry = self.cell_at(h, cell, i);
        self.drop_at(entry.at(ks), v);
        self.conv(entry.at(ks), val.0, &val.1, v, true, at)?;
        self.fit_ty(entry.at(ks), vt, at)?;
        self.drop_at(key.0, &key.1);
        let done = self.jmp();
        // una entrada mas
        self.here(new);
        self.grow(h, cell);
        let entry = self.cell_at(h, cell, h.at(8));
        self.conv(entry, key.0, &key.1, k, true, at)?;
        self.fit_ty(entry, kt, at)?;
        self.conv(entry.at(ks), val.0, &val.1, v, true, at)?;
        self.fit_ty(entry.at(ks), vt, at)?;
        self.load(h.at(8), RAX);
        x86::inc_r64(&mut self.code, RAX);
        self.store(h.at(8), RAX);
        self.here(done);
        Ok(())
    }

    fn fit_ty(&mut self, p: Place, ty: Option<&Ty>, at: (usize, usize)) -> Result<(), String> {
        match ty {
            Some(t) if self.forms.has_decp(t) => self.fit(p, t, at),
            _ => Ok(()),
        }
    }

    /// Un valor que es NUESTRO y de nadie mas -- fuera de la cuenta de la
    /// operacion: el sitio de paso si lo era (se saca de la cuenta), o una
    /// copia en uno nuevo. Quien lo pide lo mueve o lo suelta.
    pub fn mine(&mut self, p: Place, c: &Class) -> Place {
        if self.take_owned(p) {
            return p;
        }
        let t = self.temp(self.forms.size(c));
        self.clone_at(t, p, c);
        t
    }

    /// Un valor que es SOLO de esta operacion: el mismo sitio si ya lo es; si
    /// no (un local, una celda), una copia en un sitio de paso.
    pub fn ensure_owned(&mut self, p: Place, c: &Class) -> Place {
        if !self.forms.has_heap(c) || self.is_owned(p) {
            return p;
        }
        let t = self.temp(self.forms.size(c));
        self.clone_at(t, p, c);
        self.own(t, c);
        t
    }

    /// `{"a": 1, "b": 2}` escrito: un mapa vacio y un `put` por entrada (una
    /// clave dos veces se queda con su ultimo valor, como en el calculo).
    pub fn map_written(&mut self, items: &[(Value, Value)], c: &Class, at: (usize, usize)) -> Result<Place, String> {
        let t = self.temp(24);
        for k in 0..3 {
            self.store_imm(t.at(8 * k), 0);
        }
        if let Class::Map(k, v) = c {
            let (k, v) = ((**k).clone(), (**v).clone());
            for (kv, vv) in items {
                let (pk, ck) = self.eval(kv)?;
                let (pv, cv) = self.eval(vv)?;
                let (pk, pv) = (self.mine(pk, &ck), self.mine(pv, &cv));
                self.map_put(t, &k, &v, (pk, ck), (pv, cv), None, None, at)?;
            }
        }
        self.own(t, c);
        Ok(t)
    }

    /// `[1, 2]` (una tabla) a una lista: un bloque con sus celdas.
    #[allow(clippy::too_many_arguments)]
    pub fn list_from_table(&mut self, dst: Place, src: Place, from: &Class, n: usize, to: &Class, moving: bool, at: (usize, usize)) -> Result<(), String> {
        let (fs, ts) = (self.forms.size(from), self.forms.size(to));
        self.store_imm(dst.at(8), n as i64);
        self.store_imm(dst.at(16), n as i64);
        if n == 0 {
            self.store_imm(dst, 0);
            return Ok(());
        }
        self.imm(RDI, (ts * n as i32) as i64);
        self.alloc();
        self.store(dst, RAX);
        // el bloque, por un puntero del marco: `each_cell` lo camina igual
        // que camina la tabla
        let first = self.pointer_from(RAX);
        let (from, to) = (from.clone(), to.clone());
        self.each_cell(n, &[(first, ts), (src, fs)], &mut |e, cells| e.conv(cells[0], cells[1], &from, &to, moving, at))
    }

    /// Una lista o un mapa de una clase a otra de celdas mas anchas (int a
    /// dec): un bloque nuevo, celda a celda; si se MUEVE, el viejo se suelta.
    pub fn retype_cells(&mut self, dst: Place, src: Place, from: &Class, to: &Class, moving: bool, at: (usize, usize)) -> Result<(), String> {
        let (fcell, fparts) = self.cells_of(from);
        let (tcell, tparts) = self.cells_of(to);
        self.load(src.at(8), RAX);
        self.store(dst.at(8), RAX);
        self.store(dst.at(16), RAX);
        self.imm(RCX, tcell as i64);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        x86::mov_r64_r64(&mut self.code, RDI, RAX);
        self.alloc();
        let fresh = self.temp(8);
        self.store(fresh, RAX);
        let parts: Vec<(i32, Class, i32, Class)> = fparts.into_iter().zip(tparts).map(|((fo, fc), (to, tc))| (fo, fc, to, tc)).collect();
        let tmp = self.temp(24);
        self.store(tmp, RAX);
        self.load(src.at(8), RAX);
        self.store(tmp.at(8), RAX);
        self.each_heap(src.at(8), &[(tmp, tcell), (src, fcell)], &mut |e, cells| {
            for (fo, fc, to, tc) in &parts {
                e.conv(cells[0].at(*to), cells[1].at(*fo), fc, tc, moving, at)?;
            }
            Ok(())
        })?;
        if moving {
            self.load(src, RDI);
            self.free();
        }
        self.load(fresh, RAX);
        self.store(dst, RAX);
        Ok(())
    }

    /// Escribe una lista, un mapa o un `Hay(v)` / `NoHay`, como `calc.rs`.
    pub fn show_collection(&mut self, p: Place, c: &Class) -> Result<(), String> {
        match c {
            Class::List(_) | Class::Map(..) | Class::Any => {
                let (open, close) = if matches!(c, Class::Map(..)) { (b"{", b"}") } else { (b"[", b"]") };
                console::write_const(&mut self.code, open);
                let (cell, parts) = self.cells_of(c);
                if !parts.is_empty() {
                    let first = self.temp(8);
                    self.store_imm(first, 1);
                    self.each_heap(p.at(8), &[(p, cell)], &mut |e, cells| {
                        e.load(first, RAX);
                        x86::test_r64_r64(&mut e.code, RAX, RAX);
                        let skip = e.jcc(0x85);
                        console::write_const(&mut e.code, b", ");
                        e.here(skip);
                        e.store_imm(first, 0);
                        e.show(cells[0], &parts[0].1, true)?;
                        if let Some((off, vc)) = parts.get(1) {
                            console::write_const(&mut e.code, b": ");
                            e.show(cells[0].at(*off), vc, true)?;
                        }
                        Ok(())
                    })?;
                }
                console::write_const(&mut self.code, close);
            }
            Class::Opt(t) => {
                self.load(p, RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let none = self.jcc(0x85);
                console::write_const(&mut self.code, b"Hay(");
                self.show(p.at(8), t, true)?;
                console::write_const(&mut self.code, b")");
                let done = self.jmp();
                self.here(none);
                console::write_const(&mut self.code, b"NoHay");
                self.here(done);
            }
            _ => unreachable!("show_collection: only lists, maps and Opcion"),
        }
        Ok(())
    }

    /// `a == b` de listas (o una lista y una tabla), mapas y `Opcion`, POR
    /// VALOR: como `calc::same` -- un mapa, con las mismas claves llevando a
    /// lo mismo, en cualquier orden. `[]` y `{}` (Any) son vacios.
    pub fn eq_collection(&mut self, pa: Place, ca: &Class, pb: Place, cb: &Class, fails: &mut Vec<usize>, at: (usize, usize)) -> Result<(), String> {
        if let Class::Table(..) = ca {
            return self.eq_collection(pb, cb, pa, ca, fails, at);
        }
        if let (Class::Opt(ta), Class::Opt(tb)) = (ca, cb) {
            self.load(pa, RAX);
            self.load(pb, RCX);
            x86::cmp_r64_r64(&mut self.code, RAX, RCX);
            fails.push(self.jcc(0x85));
            x86::test_r64_r64(&mut self.code, RAX, RAX);
            let none = self.jcc(0x85);
            let (ta, tb) = ((**ta).clone(), (**tb).clone());
            self.eq_into(pa.at(8), &ta, pb.at(8), &tb, fails, at)?;
            self.here(none);
            return Ok(());
        }
        // cuantas: las de un asa, o las de una tabla (fijas)
        let table_n = match cb {
            Class::Table(_, n) => Some(*n),
            _ => None,
        };
        self.load(pa.at(8), RAX);
        match table_n {
            Some(n) => self.imm(RCX, n as i64),
            None => self.load(pb.at(8), RCX),
        }
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        fails.push(self.jcc(0x85));
        match (ca, cb) {
            // una vacia escrita (`[]`, `{}`): con el mismo largo, ya esta
            (Class::Any, _) | (_, Class::Any) => {}
            (Class::List(ta), Class::List(tb) | Class::Table(tb, _)) => {
                let (ta, tb) = ((**ta).clone(), (**tb).clone());
                let (sa, sb) = (self.forms.size(&ta), self.forms.size(&tb));
                let hb = match table_n {
                    Some(_) => {
                        let t = self.temp(8);
                        self.addr(pb, RAX);
                        self.store(t, RAX);
                        t
                    }
                    None => pb,
                };
                let mut inner = Vec::new();
                self.each_heap(pa.at(8), &[(pa, sa), (hb, sb)], &mut |e, cells| e.eq_into(cells[0], &ta, cells[1], &tb, &mut inner, at))?;
                fails.extend(inner);
            }
            (Class::Map(ka, va), Class::Map(kb, vb)) => {
                let (ka, va, kb, vb) = ((**ka).clone(), (**va).clone(), (**kb).clone(), (**vb).clone());
                let (cell, _) = self.cells_of(ca);
                let (bcell, _) = self.cells_of(cb);
                let (ks, kbs) = (self.forms.size(&ka), self.forms.size(&kb));
                let mut inner = Vec::new();
                self.each_heap(pa.at(8), &[(pa, cell)], &mut |e, cells| {
                    let i = e.map_find(pb, &kb, &vb, cells[0], &ka, at)?;
                    e.load(i, RAX);
                    x86::test_r64_r64(&mut e.code, RAX, RAX);
                    inner.push(e.jcc(0x88));
                    let entry = e.cell_at(pb, bcell, i);
                    e.eq_into(cells[0].at(ks), &va, entry.at(kbs), &vb, &mut inner, at)
                })?;
                fails.extend(inner);
            }
            (a, b) => {
                let what = format!("comparar {:?} con {:?}", a, b);
                return Err(self.not_yet(&what, super::EQ_WHY, super::EQ_HOW, at));
            }
        }
        Ok(())
    }

    /// El PIC (`dec(p, s)`) dentro de una lista, un mapa o un `Hay`.
    pub fn fit_collection(&mut self, p: Place, ty: &Ty, at: (usize, usize)) -> Result<(), String> {
        let c = self.forms.class(ty);
        match ty {
            Ty::List(inner) => {
                let (cell, _) = self.cells_of(&c);
                let inner = (**inner).clone();
                self.each_heap(p.at(8), &[(p, cell)], &mut |e, cells| e.fit(cells[0], &inner, at))
            }
            Ty::Map(_, v) => {
                let (cell, parts) = self.cells_of(&c);
                let (off, _) = parts[1].clone();
                let v = (**v).clone();
                self.each_heap(p.at(8), &[(p, cell)], &mut |e, cells| e.fit(cells[0].at(off), &v, at))
            }
            Ty::Opt(inner) => {
                self.load(p, RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let none = self.jcc(0x85);
                self.fit(p.at(8), inner, at)?;
                self.here(none);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
