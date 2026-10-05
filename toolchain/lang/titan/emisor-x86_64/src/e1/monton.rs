//! **EL MONTON DE E1** -- donde viven las celdas de las listas y los mapas
//! (nivel 13, `docs/plan/PLAN_LISTAS_Y_MAPAS.md`, L3).
//!
//! ```text
//!    el estado     un bloque de 512 bytes al principio del primer trozo,
//!                  apuntado por `r14` todo el programa: el cursor, el final
//!                  del trozo, cuantos bloques estan vivos, y la cabeza de la
//!                  lista de libres de cada medida
//!    pedir         la medida (con 16 bytes de cabecera) se sube a potencia
//!                  de 2, de 32 en adelante; si su lista de libres tiene uno,
//!                  ese; si no, del cursor; si el trozo no da, otro trozo al
//!                  kernel (TASK_OP_MEMORIA_PEDIR, 16 MiB o mas)
//!    soltar        el bloque vuelve a la lista de libres de su medida: lo
//!                  soltado se REUSA, como en `bmo-monton`
//! ```
//!
//! ** Y DE QUIEN ES CADA VALOR. Un valor con monton dentro tiene UN
//! propietario: un local, la celda de otra lista, o -- mientras dura una
//! operacion -- un sitio de paso. Copiarlo es CLONARLO (D2: `let b = a` es
//! otra lista); pasarlo de un sitio de paso a su destino es MOVERLO (los
//! bytes, y el sitio de paso se olvida). Lo que muere se SUELTA: un local en
//! su `Drop` o al volver su fn, lo viejo de un `=`, y lo que una operacion
//! calculo y nadie se quedo. Al acabar el programa no queda NADA pedido: si
//! quedara, el programa lo dice (`emit`).

use super::{Base, Fun, Helper, Place, E1};
use bmo_abi::syscalls::surface::{CURRENT_TASK, MEM_OP_BASE, NR_INVOKE, TASK_OP_MEMORIA_PEDIR};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R8, R9};
use bmo_lower::{console, memoria, task};
use bmo_titan_front::calc::Class;

/// El estado del monton, todo el programa.
pub(crate) const R14: u8 = 14;
const STATE: i32 = 512;
/// Las cabezas de las listas de libres: `[r14 + HEADS + 8 * medida]`.
const HEADS: i32 = 24;
const CURSOR: i32 = 0;
const END: i32 = 8;
/// Cuantos bloques estan vivos: al acabar, cero.
pub(crate) const LIVE: i32 = 16;
const CHUNK: i64 = 16 << 20;

impl E1<'_> {
    // -- de quien es cada valor -------------------------------------------

    /// Un sitio de paso recien calculado con monton dentro: es de la
    /// operacion, y lo suelta al acabar si nadie se lo lleva.
    pub fn own(&mut self, p: Place, c: &Class) {
        if self.forms.has_heap(c) {
            self.owned.push((p, c.clone()));
        }
    }

    pub fn is_owned(&self, p: Place) -> bool {
        self.owned.iter().any(|(q, _)| *q == p)
    }

    /// Si `p` es un sitio de paso de la operacion, se lo lleva quien pregunta
    /// (y ya no se suelta al acabar): MOVER en vez de clonar.
    pub fn take_owned(&mut self, p: Place) -> bool {
        match self.owned.iter().position(|(q, _)| *q == p) {
            Some(i) => {
                self.owned.remove(i);
                true
            }
            None => false,
        }
    }

    /// Suelta lo que la operacion calculo desde `mark` y nadie se llevo.
    pub fn drop_owned_from(&mut self, mark: usize) {
        let rest = self.owned.split_off(mark);
        for (p, c) in rest.iter().rev() {
            self.drop_at(*p, c);
        }
    }

    /// Un valor de clase `c` que aqui deja de ser de alguien: lo suyo del
    /// monton se suelta, y sus asas quedan a cero.
    pub fn drop_at(&mut self, p: Place, c: &Class) {
        if !self.forms.has_heap(c) {
            return;
        }
        let k = self.kind(c);
        self.addr(p, RDI);
        self.call_helper(Helper::DropOf(k));
    }

    /// Una copia ENTERA de `src` en `dst`: lo del monton, nuevo.
    pub fn clone_at(&mut self, dst: Place, src: Place, c: &Class) {
        if !self.forms.has_heap(c) {
            let size = self.forms.size(c);
            self.copy(dst, src, size);
            return;
        }
        let k = self.kind(c);
        self.addr(dst, RDI);
        self.addr(src, RSI);
        self.call_helper(Helper::CloneOf(k));
    }

    /// El numero de la clase `c` entre las que tienen subrutina de clonar y
    /// soltar.
    fn kind(&mut self, c: &Class) -> u16 {
        match self.kinds.iter().position(|k| k == c) {
            Some(i) => i as u16,
            None => {
                self.kinds.push(c.clone());
                (self.kinds.len() - 1) as u16
            }
        }
    }

    /// `rdi` bytes del monton, a `rax`.
    pub fn alloc(&mut self) {
        self.call_helper(Helper::Alloc);
    }

    /// Suelta el bloque de `rdi` (cero: nada).
    pub fn free(&mut self) {
        self.call_helper(Helper::Free);
    }

    // -- las subrutinas -----------------------------------------------------

    /// El marco de una subrutina que usa sitios de paso: `rdi` en `[rbp-8]`,
    /// `rsi` en `[rbp-16]`. Devuelve donde se parchea su medida.
    fn helper_enter(&mut self) -> usize {
        self.code.push(0x55);
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC, 0, 0, 0, 0]);
        let patch = self.code.len() - 4;
        x86::mov_at_reg_disp32_from_r64(&mut self.code, super::RBP, -8, RDI);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, super::RBP, -16, RSI);
        self.f = Fun { known: Vec::new(), decl: Vec::new(), slot: Vec::new(), indirect: Vec::new(), locals_end: 16, temp: 0, temp_max: 0 };
        patch
    }

    fn helper_leave(&mut self, patch: usize) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC]);
        self.code.push(0x5D);
        self.code.push(0xC3);
        let frame = (self.f.locals_end + self.f.temp_max + 15) / 16 * 16;
        self.code[patch..patch + 4].copy_from_slice(&frame.to_le_bytes());
    }

    /// CloneOf(k): `[rdi]` <- una copia entera de `[rsi]`.
    pub fn h_clone(&mut self, k: u16) {
        let c = self.kinds[k as usize].clone();
        let patch = self.helper_enter();
        let (dst, src) = (Place { base: Base::Ptr(-8), off: 0 }, Place { base: Base::Ptr(-16), off: 0 });
        self.clone_body(dst, src, &c);
        self.helper_leave(patch);
    }

    /// DropOf(k): lo de `[rdi]` del monton, soltado; sus asas a cero.
    pub fn h_drop(&mut self, k: u16) {
        let c = self.kinds[k as usize].clone();
        let patch = self.helper_enter();
        let p = Place { base: Base::Ptr(-8), off: 0 };
        self.drop_body(p, &c);
        self.helper_leave(patch);
    }

    fn clone_body(&mut self, dst: Place, src: Place, c: &Class) {
        let size = self.forms.size(c);
        match c {
            Class::List(_) | Class::Map(..) => {
                let (cell, parts) = self.cells_of(c);
                let any_heap = parts.iter().any(|(_, pc)| self.forms.has_heap(pc));
                self.load(src.at(8), RAX);
                self.store(dst.at(8), RAX);
                self.store(dst.at(16), RAX);
                x86::test_r64_r64(&mut self.code, RAX, RAX);
                let empty = self.jcc(0x84);
                self.imm(RCX, cell as i64);
                x86::imul_r64_r64(&mut self.code, RAX, RCX);
                x86::mov_r64_r64(&mut self.code, RDI, RAX);
                self.alloc();
                self.store(dst, RAX);
                if any_heap {
                    let count = dst.at(8);
                    self.each_heap(count, &[(dst, cell), (src, cell)], &mut |e, cells| {
                        for (off, pc) in &parts {
                            e.clone_at(cells[0].at(*off), cells[1].at(*off), pc);
                        }
                        Ok(())
                    })
                    .expect("clone: nothing here fails");
                } else {
                    self.load(dst, RDI);
                    self.load(src, RSI);
                    self.load(src.at(8), RCX);
                    self.imm(RAX, cell as i64);
                    x86::imul_r64_r64(&mut self.code, RCX, RAX);
                    memoria::copiar(&mut self.code);
                }
                let done = self.jmp();
                self.here(empty);
                self.store_imm(dst, 0);
                self.here(done);
            }
            Class::Table(inner, n) => {
                let sz = self.forms.size(inner);
                let inner = (**inner).clone();
                self.each_cell(*n, &[(dst, sz), (src, sz)], &mut |e, cells| {
                    e.clone_at(cells[0], cells[1], &inner);
                    Ok(())
                })
                .expect("clone: nothing here fails");
            }
            Class::Record(t) => {
                for k in 0..self.m.types[*t].fields.len() {
                    let (off, fc, _) = self.forms.field_k(*t, k);
                    self.clone_at(dst.at(off), src.at(off), &fc);
                }
            }
            Class::Opt(inner) => {
                self.copy(dst, src, size);
                self.payload_if(src, 0, &mut |e| e.clone_at(dst.at(8), src.at(8), inner));
            }
            Class::Enum(e) => {
                self.copy(dst, src, size);
                for v in 0..self.m.enums[*e].cases.len() {
                    let fields: Vec<(i32, Class)> = (0..self.m.enums[*e].cases[v].fields.len()).map(|k| self.forms.case_field(*e, v, k)).map(|(o, c, _)| (o, c)).filter(|(_, c)| self.forms.has_heap(c)).collect();
                    if !fields.is_empty() {
                        self.payload_if(src, v as i64, &mut |e| {
                            for (off, fc) in &fields {
                                e.clone_at(dst.at(*off), src.at(*off), fc);
                            }
                        });
                    }
                }
            }
            Class::Trait(k) => {
                self.copy(dst, src, size);
                for (name, tc) in self.forms.trait_types(*k) {
                    if self.forms.has_heap(&tc) {
                        let id = self.forms.type_id(&name);
                        self.payload_if(src, id, &mut |e| e.clone_at(dst.at(8), src.at(8), &tc));
                    }
                }
            }
            _ => self.copy(dst, src, size),
        }
    }

    fn drop_body(&mut self, p: Place, c: &Class) {
        match c {
            Class::List(_) | Class::Map(..) => {
                let (cell, parts) = self.cells_of(c);
                if parts.iter().any(|(_, pc)| self.forms.has_heap(pc)) {
                    self.each_heap(p.at(8), &[(p, cell)], &mut |e, cells| {
                        for (off, pc) in &parts {
                            e.drop_at(cells[0].at(*off), pc);
                        }
                        Ok(())
                    })
                    .expect("drop: nothing here fails");
                }
                self.load(p, RDI);
                self.free();
                for k in 0..3 {
                    self.store_imm(p.at(8 * k), 0);
                }
            }
            Class::Table(inner, n) => {
                let sz = self.forms.size(inner);
                let inner = (**inner).clone();
                self.each_cell(*n, &[(p, sz)], &mut |e, cells| {
                    e.drop_at(cells[0], &inner);
                    Ok(())
                })
                .expect("drop: nothing here fails");
            }
            Class::Record(t) => {
                for k in 0..self.m.types[*t].fields.len() {
                    let (off, fc, _) = self.forms.field_k(*t, k);
                    self.drop_at(p.at(off), &fc);
                }
            }
            Class::Opt(inner) => self.payload_if(p, 0, &mut |e| e.drop_at(p.at(8), inner)),
            Class::Enum(e) => {
                for v in 0..self.m.enums[*e].cases.len() {
                    let fields: Vec<(i32, Class)> = (0..self.m.enums[*e].cases[v].fields.len()).map(|k| self.forms.case_field(*e, v, k)).map(|(o, c, _)| (o, c)).filter(|(_, c)| self.forms.has_heap(c)).collect();
                    if !fields.is_empty() {
                        self.payload_if(p, v as i64, &mut |e| {
                            for (off, fc) in &fields {
                                e.drop_at(p.at(*off), fc);
                            }
                        });
                    }
                }
            }
            Class::Trait(k) => {
                for (name, tc) in self.forms.trait_types(*k) {
                    if self.forms.has_heap(&tc) {
                        let id = self.forms.type_id(&name);
                        self.payload_if(p, id, &mut |e| e.drop_at(p.at(8), &tc));
                    }
                }
            }
            _ => {}
        }
    }

    /// Lo que hace `body`, solo si los 8 primeros bytes de `p` son `tag`.
    fn payload_if(&mut self, p: Place, tag: i64, body: &mut dyn FnMut(&mut Self)) {
        self.load(p, RAX);
        self.imm(RCX, tag);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let skip = self.jcc(0x85);
        body(self);
        self.here(skip);
    }

    /// Las celdas de una lista o un mapa: lo que mide una, y sus partes
    /// (donde empieza cada una y su clase) -- un mapa, su clave y su valor.
    pub fn cells_of(&self, c: &Class) -> (i32, Vec<(i32, Class)>) {
        match c {
            Class::List(t) => (self.forms.size(t), vec![(0, (**t).clone())]),
            Class::Map(k, v) => {
                let ks = self.forms.size(k);
                (ks + self.forms.size(v), vec![(0, (**k).clone()), (ks, (**v).clone())])
            }
            _ => (0, Vec::new()),
        }
    }

    /// Para cada celda de las listas de `bases` (el ASA de cada una y lo que
    /// mide su celda), tantas como dice `count` AL CORRER: `body` con el
    /// sitio de la celda de cada una.
    pub fn each_heap(&mut self, count: Place, bases: &[(Place, i32)], body: &mut dyn FnMut(&mut Self, &[Place]) -> Result<(), String>) -> Result<(), String> {
        let left = self.temp(8);
        self.load(count, RAX);
        self.store(left, RAX);
        let mut ptrs = Vec::with_capacity(bases.len());
        for (h, _) in bases {
            self.load(*h, RAX);
            let s = self.temp(8);
            self.store(s, RAX);
            ptrs.push(s);
        }
        let top = self.code.len();
        self.load(left, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let out = self.jcc(0x84);
        let cells: Vec<Place> = ptrs.iter().map(|s| Place { base: Base::Ptr(s.off), off: 0 }).collect();
        body(self, &cells)?;
        for ((_, sz), s) in bases.iter().zip(&ptrs) {
            self.load(*s, RAX);
            self.lea(RAX, RAX, *sz);
            self.store(*s, RAX);
        }
        self.load(left, RAX);
        x86::dec_r64(&mut self.code, RAX);
        self.store(left, RAX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(out);
        Ok(())
    }

    /// Alloc: `rdi` bytes, a `rax`. Toca rax, rcx, rdx, rsi, rdi, r8-r11.
    pub fn h_alloc(&mut self) {
        // la medida: 16 de cabecera, a potencia de 2 desde 32
        self.lea(RDX, RDI, 16);
        self.imm(RCX, 5);
        self.imm(RAX, 32);
        let class_top = self.code.len();
        x86::cmp_r64_r64(&mut self.code, RAX, RDX);
        let have = self.jcc(0x83);
        x86::shl_r64_imm8(&mut self.code, RAX, 1);
        x86::inc_r64(&mut self.code, RCX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, class_top);
        self.here(have);
        x86::mov_r64_r64(&mut self.code, R9, RAX);
        x86::test_r64_r64(&mut self.code, R14, R14);
        let first = self.jcc(0x84);
        // la lista de libres de esa medida
        let head = self.code.len();
        x86::mov_r64_r64(&mut self.code, R10, RCX);
        x86::shl_r64_imm8(&mut self.code, R10, 3);
        x86::add_r64_r64(&mut self.code, R10, R14);
        x86::mov_r64_at_reg_disp32(&mut self.code, R8, R10, HEADS);
        x86::test_r64_r64(&mut self.code, R8, R8);
        let bump = self.jcc(0x84);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R8, 0);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R10, HEADS, RAX);
        let got1 = self.jmp();
        // del cursor
        self.here(bump);
        x86::mov_r64_at_reg_disp32(&mut self.code, R8, R14, CURSOR);
        x86::mov_r64_r64(&mut self.code, RAX, R8);
        x86::add_r64_r64(&mut self.code, RAX, R9);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, R14, END);
        x86::cmp_r64_r64(&mut self.code, RAX, RDX);
        let more = self.jcc(0x87);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, CURSOR, RAX);
        let got2 = self.jmp();
        // otro trozo al kernel: max(medida + estado, CHUNK)
        self.here(first);
        self.here(more);
        x86::push_r64(&mut self.code, RCX);
        x86::push_r64(&mut self.code, R9);
        x86::mov_r64_r64(&mut self.code, RDX, R9);
        self.lea(RDX, RDX, STATE);
        self.imm(RAX, CHUNK);
        x86::cmp_r64_r64(&mut self.code, RDX, RAX);
        let big = self.jcc(0x83);
        x86::mov_r64_r64(&mut self.code, RDX, RAX);
        self.here(big);
        x86::push_r64(&mut self.code, RDX);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MEMORIA_PEDIR as i64);
        self.imm(RAX, NR_INVOKE as i64);
        x86::syscall(&mut self.code);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let fail1 = self.jcc(0x85);
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RSI, MEM_OP_BASE as i64);
        self.imm(RAX, NR_INVOKE as i64);
        x86::syscall(&mut self.code);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let fail2 = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let fail3 = self.jcc(0x84);
        x86::pop_r64(&mut self.code, RCX); // los bytes del trozo
        x86::test_r64_r64(&mut self.code, R14, R14);
        let not_first = self.jcc(0x85);
        // el primero: el estado delante, a cero
        x86::mov_r64_r64(&mut self.code, R14, RDX);
        x86::push_r64(&mut self.code, RCX);
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RCX, STATE as i64);
        x86::zero_r32(&mut self.code, RAX);
        memoria::rellenar(&mut self.code);
        x86::pop_r64(&mut self.code, RCX);
        self.lea(RAX, R14, STATE);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, CURSOR, RAX);
        let set_end = self.jmp();
        self.here(not_first);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, CURSOR, RDX);
        self.here(set_end);
        x86::add_r64_r64(&mut self.code, RDX, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, END, RDX);
        x86::pop_r64(&mut self.code, R9);
        x86::pop_r64(&mut self.code, RCX);
        let again = self.jmp();
        x86::patch_jump_to(&mut self.code, again, head);
        for f in [fail1, fail2, fail3] {
            self.here(f);
        }
        console::write_const(&mut self.code, b"NO T0060 al correr: el monton (la memoria de las listas y los mapas) se acabo\n");
        task::exit(&mut self.code);
        // el bloque: su medida en la cabecera, uno vivo mas
        self.here(got1);
        self.here(got2);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R8, 0, RCX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R14, LIVE);
        x86::inc_r64(&mut self.code, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, LIVE, RAX);
        self.lea(RAX, R8, 16);
        self.code.push(0xC3);
    }

    /// Free: el bloque de `rdi` (cero: nada) a la lista de libres de su medida.
    pub fn h_free(&mut self) {
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        let none = self.jcc(0x84);
        self.lea(R8, RDI, -16);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, R8, 0);
        x86::mov_r64_r64(&mut self.code, R10, RCX);
        x86::shl_r64_imm8(&mut self.code, R10, 3);
        x86::add_r64_r64(&mut self.code, R10, R14);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R10, HEADS);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R8, 0, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R10, HEADS, R8);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R14, LIVE);
        x86::dec_r64(&mut self.code, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, R14, LIVE, RAX);
        self.here(none);
        self.code.push(0xC3);
    }
}
