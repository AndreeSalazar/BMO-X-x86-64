//! # E1 -- el programa que lee de fuera, CORRIENDO en la maquina
//!
//! `docs/plan/PLAN_LA_ENTRADA.md` (R3, R6, R7); la escalera es la de
//! `docs/maestro/TITAN_MAESTRO.md` 7.3. Un programa con `lee()` no se puede
//! correr al compilar -- lo que se teclee no se sabe todavia --, asi que el
//! calculo lo juzga entero (clases, prestamos) y aqui se EMITE para correr:
//!
//! ```text
//!    cada valor      vive en MEMORIA, con la forma de su clase (`forma.rs`):
//!                    un local en el marco de su fn, lo que se calcula en un
//!                    sitio de paso del mismo marco
//!    cada fn         su marco en `rbp`; recibe en `rdi` los punteros a sus
//!                    valores y en `rsi` donde dejar el suyo. Un `mut` llega
//!                    como puntero y se escribe a traves de el; lo demas se
//!                    copia al entrar
//!    un trait        el valor va con su TIPO delante; la fn del trait lo mira
//!                    y salta a la del tipo (`calc.rs` lo elige igual)
//!    lo dificil      el `dec` (alinear, multiplicar, dividir exacto, round,
//!                    el PIC de COBOL) y mostrar un valor: subrutinas que se
//!                    emiten UNA vez, solo si alguien las llama (`numero.rs`,
//!                    `escribe.rs`)
//! ```
//!
//! ** EL CALCULO ES LA VARA. La semantica es la de `calc.rs`, letra a letra
//! -- la escala del `dec`, el redondeo, como se escribe un registro --, y una
//! prueba lo comprueba: cada programa BIEN del banco que no usa la 3060 se
//! emite TAMBIEN por aqui y tiene que escribir lo mismo que el calculo
//! (`emisor-x86_64/tests/e1.rs`).
//!
//! ** LO QUE SOLO FALLA AL CORRER, ATRAPA (D4 del plan, la regla 1 de INTI):
//! desbordar (T0060), dividir por cero (T0061), una division que no es exacta
//! (T0062), salirse de una tabla (T0072), unas cifras que no caben en su
//! `dec(p, s)` (T0074), llamadas que se anidan mas de lo que cabe en la pila
//! (T0066). El programa escribe el NO con su linea del `.titan` y sale.
//!
//! [!] Lo que E1 no emite lo dice al compilar: la 3060 (G4 de
//! PLAN_EL_CENTAURO: sin el lanzamiento en Ring 0 no hay donde correrla).

mod ancho;
mod escribe;
mod forma;
mod numero;
mod valor;

use crate::Emitted;
use bmo_lower::x86::{self, RAX, RSI, RDI, R11};
use bmo_lower::{console, memoria, task};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{End, Function, Module, Op, Value};
use bmo_titan_front::tree::{Mode, Ty};
use forma::Forms;
use std::collections::BTreeMap;

pub(crate) const RBP: u8 = 5;
/// Lo mas largo que guarda un texto al correr.
pub(crate) const TEXT_CAP: usize = 248;
/// Una linea de consola: lo que `lee()` trae como mucho.
pub(crate) const READ_MAX: u8 = 127;
/// Cuanto de la pila de 64 KiB pueden ocupar los marcos de las llamadas.
const STACK_FOR_CALLS: i32 = 48 * 1024;

/// Donde esta un valor: en el marco (`[rbp + off]`) o detras de un puntero
/// guardado en el marco (`[[rbp + slot] + off]`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Base {
    Frame,
    Ptr(i32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Place {
    pub base: Base,
    pub off: i32,
}

impl Place {
    pub fn frame(off: i32) -> Place {
        Place { base: Base::Frame, off }
    }
    pub fn at(self, extra: i32) -> Place {
        Place { base: self.base, off: self.off + extra }
    }
}

/// Las subrutinas compartidas: se emiten una vez, al final, si se llaman.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Helper {
    Pow10,
    DecCmp,
    DecAdd,
    DecMul,
    DecDiv,
    DecRound,
    DecFit,
    DecShow,
    ParseInt,
    WriteQuoted,
    WriteInt,
}

/// Un NO al correr, por escribir: el salto que lleva a el, su linea, y lo
/// que dice -- `head` hasta "linea " y `why` despues del numero. Los NO con
/// el mismo `head` y `why` comparten UNA escritura (`emit`): cada sitio solo
/// deja su linea en `rbx` y salta.
struct Trap {
    field: usize,
    head: String,
    line: usize,
    why: String,
}

/// El NO de una subrutina de numeros: cual es lo dice `r10` al correr.
struct StatusTrap {
    field: usize,
    file: String,
    line: usize,
}

/// Los NO que dice una subrutina de numeros por su estado en `r10`.
const STATUS: [(u8, &str, &str); 4] = [
    (1, "T0060", "el resultado no cabe en 64 bits"),
    (2, "T0062", "una division que no da un numero exacto: TITAN++ no redondea a escondidas (round(x, 2) lo escribe)"),
    (3, "T0061", "una division por cero"),
    (4, "T0074", "el valor no cabe en las cifras de su dec(p, s)"),
];

/// Donde vive la linea de un NO mientras se escribe: `rbx` no lo toca
/// ninguna puerta, y despues del NO el programa sale.
const RBX: u8 = 3;

/// Lo que se sabe de la fn que se esta emitiendo.
pub(crate) struct Fun {
    pub known: Vec<Option<Class>>,
    pub decl: Vec<Option<Ty>>,
    pub slot: Vec<i32>,
    /// Un parametro `mut`: su sitio guarda un PUNTERO al valor del que llama.
    pub indirect: Vec<bool>,
    pub locals_end: i32,
    pub temp: i32,
    pub temp_max: i32,
}

pub(crate) struct E1<'m> {
    pub m: &'m Module,
    pub forms: Forms<'m>,
    pub code: Vec<u8>,
    /// (campo rel32, funcion destino), de `call` y de `jmp`.
    calls: Vec<(usize, usize)>,
    traps: Vec<Trap>,
    status_traps: Vec<StatusTrap>,
    helper_calls: Vec<(usize, Helper)>,
    /// (campo imm32 de un `cmp r15`, la fn que se llama): la pila que queda.
    depth_checks: Vec<(usize, usize)>,
    pub f: Fun,
    /// Dentro de un `round(...)`: una division que no acaba se lleva a 18
    /// decimales, y `round` corta donde DICE (`calc.rs`, `lenient`).
    pub lenient: u32,
}

/// Lo que E1 no emite: un NO al compilar.
pub(crate) fn later(what: &str, why: &str, at: (usize, usize)) -> String {
    format!("linea {}: {} no se emite al correr todavia ({})", at.0, what, why)
}

impl<'m> E1<'m> {
    // -- los bytes mas chicos -------------------------------------------

    /// `lea <reg>, [<base> + disp32]`.
    pub fn lea(&mut self, reg: u8, base: u8, disp: i32) {
        self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 } | if base >= 8 { 0x01 } else { 0 });
        self.code.push(0x8D);
        self.code.push(0x80 | ((reg & 7) << 3) | (base & 7));
        if base & 7 == 4 {
            self.code.push(0x24);
        }
        self.code.extend_from_slice(&disp.to_le_bytes());
    }

    /// `jcc rel32` (el segundo byte: 0x84 je, 0x85 jne, 0x80 jo, 0x8C jl...).
    pub fn jcc(&mut self, cc: u8) -> usize {
        self.code.extend_from_slice(&[0x0F, cc, 0, 0, 0, 0]);
        self.code.len() - 4
    }

    pub fn jmp(&mut self) -> usize {
        self.code.extend_from_slice(&[0xE9, 0, 0, 0, 0]);
        self.code.len() - 4
    }

    /// Aterriza aqui el salto de `field`.
    pub fn here(&mut self, field: usize) {
        x86::patch_jump(&mut self.code, field);
    }

    pub fn imm(&mut self, reg: u8, v: i64) {
        x86::mov_r64_imm64(&mut self.code, reg, v as u64);
    }

    /// La direccion de un sitio, a `reg`.
    pub fn addr(&mut self, p: Place, reg: u8) {
        match p.base {
            Base::Frame => self.lea(reg, RBP, p.off),
            Base::Ptr(slot) => {
                x86::mov_r64_at_reg_disp32(&mut self.code, reg, RBP, slot);
                if p.off != 0 {
                    self.lea(reg, reg, p.off);
                }
            }
        }
    }

    /// Los 8 bytes de un sitio, a `reg`. No toca otro registro.
    pub fn load(&mut self, p: Place, reg: u8) {
        match p.base {
            Base::Frame => x86::mov_r64_at_reg_disp32(&mut self.code, reg, RBP, p.off),
            Base::Ptr(slot) => {
                x86::mov_r64_at_reg_disp32(&mut self.code, reg, RBP, slot);
                x86::mov_r64_at_reg_disp32(&mut self.code, reg, reg, p.off);
            }
        }
    }

    /// `reg` a los 8 bytes de un sitio. Usa `r11` si el sitio es un puntero.
    pub fn store(&mut self, p: Place, reg: u8) {
        match p.base {
            Base::Frame => x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, p.off, reg),
            Base::Ptr(slot) => {
                debug_assert_ne!(reg, R11);
                x86::mov_r64_at_reg_disp32(&mut self.code, R11, RBP, slot);
                x86::mov_at_reg_disp32_from_r64(&mut self.code, R11, p.off, reg);
            }
        }
    }

    pub fn store_imm(&mut self, p: Place, v: i64) {
        self.imm(RAX, v);
        self.store(p, RAX);
    }

    /// `size` bytes de `src` a `dst`.
    pub fn copy(&mut self, dst: Place, src: Place, size: i32) {
        if dst == src || size == 0 {
            return;
        }
        if size <= 32 {
            let mut k = 0;
            while k < size {
                self.load(src.at(k), RAX);
                self.store(dst.at(k), RAX);
                k += 8;
            }
            return;
        }
        self.addr(dst, RDI);
        self.addr(src, RSI);
        x86::mov_r64_imm64(&mut self.code, bmo_lower::x86::RCX, size as u64);
        memoria::copiar(&mut self.code);
    }

    /// Un sitio de paso del marco, de `size` bytes.
    pub fn temp(&mut self, size: i32) -> Place {
        self.f.temp += (size + 7) / 8 * 8;
        self.f.temp_max = self.f.temp_max.max(self.f.temp);
        Place::frame(-(self.f.locals_end + self.f.temp))
    }

    /// Un puntero calculado al correr, guardado en el marco: el sitio al que
    /// apunta.
    pub fn pointer_from(&mut self, reg: u8) -> Place {
        let slot = self.temp(8);
        self.store(slot, reg);
        Place { base: Base::Ptr(slot.off), off: 0 }
    }

    pub fn local(&self, l: usize) -> Place {
        if self.f.indirect[l] {
            Place { base: Base::Ptr(self.f.slot[l]), off: 0 }
        } else {
            Place::frame(self.f.slot[l])
        }
    }

    // -- los NO al correr y las subrutinas -------------------------------

    /// "src/main.titan, linea 4" o "linea 4": el sitio en el `.titan`.
    /// El fichero (si el paquete tiene varios: "ruta, ") y la linea de `at`.
    fn place(&self, at: (usize, usize)) -> (String, usize) {
        match self.m.sources.place(at.0) {
            Some((k, line)) if self.m.sources.0.len() > 1 => (format!("{}, ", self.m.sources.path(k)), line),
            Some((_, line)) => (String::new(), line),
            None => (String::new(), at.0),
        }
    }

    /// Si se da `cc`, el NO `code_id` con su linea.
    pub fn trap(&mut self, cc: u8, code_id: &str, why: &str, at: (usize, usize)) {
        let field = self.jcc(cc);
        let (file, line) = self.place(at);
        self.traps.push(Trap { field, head: format!("NO {} al correr, {}linea ", code_id, file), line, why: why.to_string() });
    }

    /// Los NO de una subrutina de numeros: deja su estado en `r10` (0 bien,
    /// 1 no cabe, 2 no es exacta, 3 entre cero, 4 no cabe en su dec(p, s)).
    /// Aqui solo se mira si NO es 0: cual es, lo mira el NO (`emit`).
    pub fn trap_status(&mut self, at: (usize, usize)) {
        use bmo_lower::x86::R10;
        x86::test_r64_r64(&mut self.code, R10, R10);
        let field = self.jcc(0x85);
        let (file, line) = self.place(at);
        self.status_traps.push(StatusTrap { field, file, line });
    }

    /// Los NO del final. Un NO (su `head` y su `why`) que sale en UNA sola
    /// linea se escribe entero, con su linea dentro. Uno que sale en varias
    /// se escribe UNA vez, con la linea sacada de `rbx` al correr, y cada
    /// sitio es `mov ebx, linea` + `jmp`. Los de una subrutina de numeros
    /// pasan antes por quien mira `r10`.
    fn write_traps(&mut self) {
        use bmo_lower::x86::R10;
        type Key = (String, String);
        let traps = std::mem::take(&mut self.traps);
        let status = std::mem::take(&mut self.status_traps);
        let status_key = |file: &str, id: &str, why: &str| -> Key { (format!("NO {} al correr, {}linea ", id, file), why.to_string()) };
        // en cuantas lineas sale cada NO
        let mut lines: BTreeMap<Key, std::collections::BTreeSet<usize>> = BTreeMap::new();
        for t in &traps {
            lines.entry((t.head.clone(), t.why.clone())).or_default().insert(t.line);
        }
        for t in &status {
            for (_, id, why) in STATUS {
                lines.entry(status_key(&t.file, id, why)).or_default().insert(t.line);
            }
        }
        // los que se comparten: los que asi ocupan MENOS, contando los bytes
        // de verdad -- y el que escribe la linea, solo si alguno lo usa
        let bytes = |f: &dyn Fn(&mut Vec<u8>)| {
            let mut v = Vec::new();
            f(&mut v);
            v.len()
        };
        let exit = bytes(&|v| task::exit(v));
        let printer = bytes(&|v| bmo_lower::fmt::write_i64(v)) + 1;
        let gain = |key: &Key, at: &std::collections::BTreeSet<usize>| -> i64 {
            let whole: usize = at.iter().map(|l| bytes(&|v| console::write_const(v, format!("{}{}: {}\n", key.0, l, key.1).as_bytes())) + exit).sum();
            let routine = bytes(&|v| console::write_const(v, key.0.as_bytes())) + 3 + 5 + bytes(&|v| console::write_const(v, format!(": {}\n", key.1).as_bytes())) + exit;
            whole as i64 - (routine + 10 * at.len()) as i64
        };
        let worth: i64 = lines.iter().map(|(k, at)| gain(k, at).max(0)).sum();
        let mut shared: BTreeMap<Key, usize> = BTreeMap::new();
        for (key, at) in &lines {
            if worth > printer as i64 && gain(key, at) > 0 {
                shared.insert(key.clone(), self.code.len());
                console::write_const(&mut self.code, key.0.as_bytes());
                x86::mov_r64_r64(&mut self.code, RAX, RBX);
                self.call_helper(Helper::WriteInt);
                console::write_const(&mut self.code, format!(": {}\n", key.1).as_bytes());
                task::exit(&mut self.code);
            }
        }
        // a donde salta el NO `key` de la linea `line`, escrito una vez
        let mut targets: BTreeMap<(Key, usize), usize> = BTreeMap::new();
        let mut target = |e: &mut Self, key: Key, line: usize| -> usize {
            if let Some(&t) = targets.get(&(key.clone(), line)) {
                return t;
            }
            let here = e.code.len();
            match shared.get(&key) {
                Some(&routine) => {
                    x86::mov_r32_imm32(&mut e.code, RBX, line as u32);
                    let j = e.jmp();
                    x86::patch_jump_to(&mut e.code, j, routine);
                }
                None => {
                    console::write_const(&mut e.code, format!("{}{}: {}\n", key.0, line, key.1).as_bytes());
                    task::exit(&mut e.code);
                }
            }
            targets.insert((key, line), here);
            here
        };
        for t in traps {
            let to = target(self, (t.head, t.why), t.line);
            x86::patch_jump_to(&mut self.code, t.field, to);
        }
        let mut dispatch: BTreeMap<(String, usize), usize> = BTreeMap::new();
        for t in status {
            let to = match dispatch.get(&(t.file.clone(), t.line)) {
                Some(&d) => d,
                None => {
                    let ends: Vec<usize> = STATUS.iter().map(|(_, id, why)| target(self, status_key(&t.file, id, why), t.line)).collect();
                    let d = self.code.len();
                    for ((k, _, _), end) in STATUS.iter().zip(ends) {
                        x86::cmp_r64_imm8(&mut self.code, R10, *k as i8);
                        let j = self.jcc(0x84);
                        x86::patch_jump_to(&mut self.code, j, end);
                    }
                    dispatch.insert((t.file.clone(), t.line), d);
                    d
                }
            };
            x86::patch_jump_to(&mut self.code, t.field, to);
        }
    }

    pub fn call_helper(&mut self, h: Helper) {
        self.code.push(0xE8);
        self.helper_calls.push((self.code.len(), h));
        self.code.extend_from_slice(&[0; 4]);
    }

    // -- las fn --------------------------------------------------------

    /// La clase de un valor en este punto de la fn (el frontend la juzgo).
    pub fn class_of(&self, v: &Value) -> Result<Class, String> {
        bmo_titan_front::calc::class(v, &self.f.known, self.m).map_err(|e| format!("linea {}: {}", e.line, e.what))
    }

    /// Cuanto ocupa cada local: el mayor de las clases que toma (un nombre
    /// que muere y renace puede ser otra cosa).
    fn plan(&mut self, f: &Function) -> Result<Vec<i32>, String> {
        let mut sizes = vec![8; f.locals.len()];
        self.f.known = vec![None; f.locals.len()];
        for (l, t) in &f.params {
            let c = self.forms.class(t);
            sizes[*l] = sizes[*l].max(self.forms.size(&c));
            self.f.known[*l] = Some(c);
        }
        for b in &f.blocks {
            for op in &b.ops {
                match op {
                    Op::Let { local, value, ty, .. } => {
                        let c = match ty {
                            Some(t) => self.forms.class(t),
                            None => self.class_of(value)?,
                        };
                        sizes[*local] = sizes[*local].max(self.forms.size(&c));
                        self.f.known[*local] = Some(c);
                    }
                    Op::Drop { local, .. } => self.f.known[*local] = None,
                    _ => {}
                }
            }
        }
        Ok(sizes)
    }

    /// Emite la fn `f`, y da lo que gasta de pila cada vez que se llama (su
    /// marco, el `rbp` guardado y la vuelta). Un trait no gasta: salta.
    /// `root`: es `main` y nadie mas la llama.
    fn function(&mut self, f: &Function, root: bool) -> Result<i32, String> {
        if let Some(table) = &f.dispatch {
            self.dispatcher(table, f.line)?;
            return Ok(0);
        }
        if f.gpu {
            return Err(later(&format!("`gpu fn {}`", f.name), "la 3060 corre cuando su lanzamiento exista en Ring 0: G4 de PLAN_EL_CENTAURO", (f.line, 1)));
        }
        let sizes = self.plan(f)?;
        let n = f.locals.len();
        self.f = Fun { known: vec![None; n], decl: vec![None; n], slot: vec![0; n], indirect: vec![false; n], locals_end: 16, temp: 0, temp_max: 0 };
        for (l, s) in sizes.iter().enumerate() {
            self.f.locals_end += (s + 7) / 8 * 8;
            self.f.slot[l] = -self.f.locals_end;
        }
        // el marco: su medida se sabe al final
        self.code.push(0x55); // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC, 0, 0, 0, 0]); // sub rsp, imm32
        let frame_patch = self.code.len() - 4;
        // el sitio del resultado y los valores: solo si los hay
        if f.ret.is_some() {
            x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, -8, RSI);
        }
        if !f.params.is_empty() {
            x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, -16, RDI);
        }
        // la pila: cada llamada abierta suma lo que gasta (quien llama ya
        // miro que cabe, `call`). La RAIZ no: la cuenta empieza con ella
        // (`emit`)
        let mut depth_patches = Vec::new();
        if !root {
            self.code.extend_from_slice(&[0x49, 0x81, 0xC7, 0, 0, 0, 0]); // add r15, imm32
            depth_patches.push(self.code.len() - 4);
        }
        // los valores que llegan
        for (i, (l, t)) in f.params.iter().enumerate() {
            let c = self.forms.class(t);
            x86::mov_r64_at_reg_disp32(&mut self.code, R11, RBP, -16);
            x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R11, 8 * i as i32);
            if f.modes.get(i) == Some(&Mode::Mut) {
                x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, self.f.slot[*l], RAX);
                self.f.indirect[*l] = true;
            } else {
                let from = self.pointer_from(RAX);
                let to = self.local(*l);
                let size = self.forms.size(&c);
                self.copy(to, from, size);
                self.fit(to, t, (f.line, 1))?;
            }
            self.f.known[*l] = Some(c);
            self.f.decl[*l] = Some(t.clone());
            self.f.temp = 0;
        }
        let ret_class = f.ret.as_ref().map(|t| self.forms.class(t));
        let mut starts = vec![usize::MAX; f.blocks.len()];
        let mut jumps: Vec<(usize, usize)> = Vec::new();
        for (i, b) in f.blocks.iter().enumerate() {
            starts[i] = self.code.len();
            for op in &b.ops {
                self.f.temp = 0;
                self.op(op)?;
            }
            self.f.temp = 0;
            match &b.end {
                End::Return(v) => {
                    if let (Some(v), Some(rc)) = (v, &ret_class) {
                        let (p, c) = self.eval(v)?;
                        let dst = Place { base: Base::Ptr(-8), off: 0 };
                        self.convert(dst, p, &c, rc, f.ret.as_ref(), v.at())?;
                    }
                    if !root {
                        self.code.extend_from_slice(&[0x49, 0x81, 0xEF, 0, 0, 0, 0]); // sub r15, imm32
                        depth_patches.push(self.code.len() - 4);
                    }
                    self.code.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
                    self.code.push(0x5D); // pop rbp
                    self.code.push(0xC3); // ret
                }
                End::Jump(t) => {
                    if *t != i + 1 {
                        let j = self.jmp();
                        jumps.push((j, *t));
                    }
                }
                End::Branch { cond, then, other, .. } => {
                    let (p, _) = self.eval(cond)?;
                    self.load(p, RAX);
                    x86::test_r64_r64(&mut self.code, RAX, RAX);
                    let j = self.jcc(0x84);
                    jumps.push((j, *other));
                    if *then != i + 1 {
                        let j = self.jmp();
                        jumps.push((j, *then));
                    }
                }
            }
        }
        for (field, t) in jumps {
            x86::patch_jump_to(&mut self.code, field, starts[t]);
        }
        let frame = (self.f.locals_end + self.f.temp_max + 15) / 16 * 16;
        self.code[frame_patch..frame_patch + 4].copy_from_slice(&frame.to_le_bytes());
        let cost = frame + 16;
        for p in depth_patches {
            self.code[p..p + 4].copy_from_slice(&cost.to_le_bytes());
        }
        Ok(cost)
    }

    /// La fn de un trait: mira el TIPO del primer valor y salta a la fn de
    /// ese tipo, con el puntero ya en el valor (sin su tipo delante).
    fn dispatcher(&mut self, table: &[(String, usize)], line: usize) -> Result<(), String> {
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
            return Err(later(&format!("una llamada a `gpu fn {}`", f.name), "la 3060 corre cuando su lanzamiento exista en Ring 0: G4 de PLAN_EL_CENTAURO", at));
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
                        let t = self.temp(self.forms.size(pc));
                        self.convert(t, lp, &lc, pc, None, a.at())?;
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
        // T0066 AQUI, en la linea de la llamada (donde lo dice el calculo):
        // r15 + lo que gasta la fn > la pila. Lo que gasta se sabe al final.
        self.code.extend_from_slice(&[0x49, 0x81, 0xFF, 0, 0, 0, 0]); // cmp r15, imm32
        self.depth_checks.push((self.code.len() - 4, func));
        let name = self.m.functions[func].name.clone();
        self.trap(0x8F, "T0066", &format!("las llamadas se anidan mas de lo que cabe en la pila (`{}` sin llegar a su caso de parada?)", forma::short(&name)), at);
        self.code.push(0xE8);
        self.calls.push((self.code.len(), func));
        self.code.extend_from_slice(&[0; 4]);
        for (to, from, size) in back {
            self.copy(to, from, size);
        }
        Ok(out)
    }

    fn op(&mut self, op: &Op) -> Result<(), String> {
        match op {
            // lo tecleado va derecho a su local: sin pasar por un temporal
            Op::Let { local, value: Value::Read(at), .. } | Op::Set { local, value: Value::Read(at), .. } => {
                let dst = self.local(*local);
                self.read_into(dst, *at);
                self.f.known[*local] = Some(Class::Text);
            }
            Op::Let { local, value, ty, at, .. } => {
                let (p, c) = self.eval(value)?;
                let want = match ty {
                    Some(t) => self.forms.class(t),
                    None => c.clone(),
                };
                let dst = self.local(*local);
                self.convert(dst, p, &c, &want, ty.as_ref(), *at)?;
                self.f.known[*local] = Some(want);
                self.f.decl[*local] = ty.clone();
            }
            Op::Set { local, value, at } => {
                let (p, c) = self.eval(value)?;
                let want = self.f.known[*local].clone().ok_or("un local sin valor")?;
                let dst = self.local(*local);
                let decl = self.f.decl[*local].clone();
                if want == Class::Dec && c == Class::Int && decl.is_none() {
                    // 13 en un `dec` de 2 decimales es 13.00 (`calc.rs`)
                    self.int_to_dec_like(dst, p, *at);
                } else {
                    self.convert(dst, p, &c, &want, decl.as_ref(), *at)?;
                }
            }
            Op::SetAt { local, path, value, at } => {
                let (dst, cell, ty) = self.path(*local, path, *at)?;
                let (p, c) = self.eval(value)?;
                self.convert(dst, p, &c, &cell, ty.as_ref(), *at)?;
            }
            Op::Write { parts, .. } => {
                // primero se CALCULA todo: un NO nunca deja media linea. Lo
                // ESCRITO (un texto, un int, un si/no) ya se sabe: no se
                // calcula, y va con lo escrito de al lado en UNA escritura
                // de constantes, el salto incluido
                let mut vals = Vec::with_capacity(parts.len());
                for p in parts {
                    vals.push(match written(p) {
                        Some(text) => Err(text),
                        None => Ok(self.eval(p)?),
                    });
                }
                let mut pending: Vec<u8> = Vec::new();
                for v in vals {
                    match v {
                        Err(text) => pending.extend_from_slice(text.as_bytes()),
                        Ok((p, c)) => {
                            console::write_const(&mut self.code, &std::mem::take(&mut pending));
                            self.show(p, &c, false)?;
                        }
                    }
                }
                pending.push(b'\n');
                console::write_const(&mut self.code, &pending);
            }
            Op::Call { func, args, at } => {
                self.call(*func, args, *at)?;
            }
            Op::Drop { local, .. } => self.f.known[*local] = None,
        }
        Ok(())
    }
}

/// Lo que `print` escribe de un valor ESCRITO en el fuente, como lo escribe
/// el calculo (`Const::show`): nada que calcular al correr.
fn written(v: &Value) -> Option<String> {
    match v {
        Value::Text(t, _) => Some(t.clone()),
        Value::Int(n, _) => Some(n.to_string()),
        Value::Bool(b, _) => Some(b.to_string()),
        _ => None,
    }
}

/// ** E1 entero: arrancar en `main`, cada fn con su marco, las subrutinas
/// que alguien llamo, y los NO del final.
pub fn emit(m: &Module) -> Result<Emitted, String> {
    let mut e = E1 {
        m,
        forms: Forms::new(m),
        code: Vec::new(),
        calls: Vec::new(),
        traps: Vec::new(),
        status_traps: Vec::new(),
        helper_calls: Vec::new(),
        depth_checks: Vec::new(),
        f: Fun { known: Vec::new(), decl: Vec::new(), slot: Vec::new(), indirect: Vec::new(), locals_end: 16, temp: 0, temp_max: 0 },
        lenient: 0,
    };
    // r15: la pila que gastan las llamadas abiertas, empezando por la raiz
    e.code.extend_from_slice(&[0x41, 0xBF, 0, 0, 0, 0]); // mov r15d, imm32
    let root_cost = e.code.len() - 4;
    x86::zero_r32(&mut e.code, RDI);
    x86::zero_r32(&mut e.code, RSI);
    e.code.push(0xE8);
    e.calls.push((e.code.len(), m.entry));
    e.code.extend_from_slice(&[0; 4]);
    task::exit(&mut e.code);
    let mut starts = vec![usize::MAX; m.functions.len()];
    // solo las fn que alguien puede llamar: una `gpu fn` que nadie llama al
    // correr no estorba
    let reach = reachable(m);
    let root = !m.functions.iter().any(|f| callees(f).contains(&m.entry));
    let mut cost = vec![0; m.functions.len()];
    for (k, f) in m.functions.iter().enumerate() {
        if !reach[k] {
            continue;
        }
        starts[k] = e.code.len();
        cost[k] = e.function(f, root && k == m.entry)?;
    }
    let start = if root { cost[m.entry] } else { 0 };
    e.code[root_cost..root_cost + 4].copy_from_slice(&start.to_le_bytes());
    // la fn de un trait gasta lo que la mas cara de sus tipos
    for (k, f) in m.functions.iter().enumerate() {
        if let Some(table) = &f.dispatch {
            cost[k] = table.iter().map(|(_, t)| cost[*t]).max().unwrap_or(0);
        }
    }
    for (field, k) in std::mem::take(&mut e.depth_checks) {
        e.code[field..field + 4].copy_from_slice(&(STACK_FOR_CALLS - cost[k]).to_le_bytes());
    }
    e.write_traps();
    // las subrutinas, y las que ellas llaman, hasta que no falte ninguna
    let mut at: BTreeMap<Helper, usize> = BTreeMap::new();
    loop {
        let missing: Vec<Helper> = e.helper_calls.iter().map(|(_, h)| *h).filter(|h| !at.contains_key(h)).collect();
        let Some(&h) = missing.first() else { break };
        at.insert(h, e.code.len());
        e.helper(h);
    }
    for (field, h) in std::mem::take(&mut e.helper_calls) {
        x86::patch_jump_to(&mut e.code, field, at[&h]);
    }
    for (field, k) in std::mem::take(&mut e.calls) {
        if starts[k] == usize::MAX {
            return Err(format!("una llamada a `{}`, que no se emitio", m.functions[k].name));
        }
        x86::patch_jump_to(&mut e.code, field, starts[k]);
    }
    Ok(Emitted { code: e.code, starts: starts.into_iter().map(|s| if s == usize::MAX { 0 } else { s }).collect() })
}

/// Las fn a las que se llega desde `main` (y las de los tipos de un trait).
fn reachable(m: &Module) -> Vec<bool> {
    let mut seen = vec![false; m.functions.len()];
    let mut stack = vec![m.entry];
    while let Some(k) = stack.pop() {
        if std::mem::replace(&mut seen[k], true) {
            continue;
        }
        let f = &m.functions[k];
        if let Some(table) = &f.dispatch {
            stack.extend(table.iter().map(|(_, t)| *t));
        }
        stack.extend(callees(f));
    }
    seen
}

/// Las fn a las que `f` llama.
fn callees(f: &Function) -> Vec<usize> {
    let mut found = Vec::new();
    for b in &f.blocks {
        for op in &b.ops {
            match op {
                Op::Call { func, args, .. } => {
                    found.push(*func);
                    args.iter().for_each(|a| calls_in(a, &mut found));
                }
                Op::Let { value, .. } | Op::Set { value, .. } | Op::SetAt { value, .. } => calls_in(value, &mut found),
                Op::Write { parts, .. } => parts.iter().for_each(|a| calls_in(a, &mut found)),
                Op::Drop { .. } => {}
            }
            if let Op::SetAt { path, .. } = op {
                for st in path {
                    if let bmo_titan_front::ir::PathStep::Index(i) = st {
                        calls_in(i, &mut found);
                    }
                }
            }
        }
        match &b.end {
            End::Return(Some(v)) | End::Branch { cond: v, .. } => calls_in(v, &mut found),
            _ => {}
        }
    }
    found
}

fn calls_in(v: &Value, out: &mut Vec<usize>) {
    match v {
        Value::Call(f, args, _) => {
            out.push(*f);
            args.iter().for_each(|a| calls_in(a, out));
        }
        Value::Bin(_, a, b, _) | Value::Index(a, b, _) => {
            calls_in(a, out);
            calls_in(b, out);
        }
        Value::Neg(a, _) | Value::Not(a, _) | Value::Repeat(a, _, _) | Value::Field(a, _, _) | Value::Len(a, _) | Value::Round(a, _, _) | Value::Is(a, _, _, _) | Value::Payload(a, _, _, _, _) | Value::Number(a, _, _) => calls_in(a, out),
        Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => items.iter().for_each(|a| calls_in(a, out)),
        Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Local(..) | Value::Lend(..) | Value::Read(..) => {}
    }
}
