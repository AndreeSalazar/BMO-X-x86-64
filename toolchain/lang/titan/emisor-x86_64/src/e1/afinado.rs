//! ** LO QUE AFINA E1 SIN QUE TITAN++ SEPA DE CPU (`docs/plan/PLAN_TITAN_AFINADO.md`).
//!
//! La ley del propietario (16-10): TITAN++ dice QUE se hace; el emisor decide
//! COMO. Cada cosa de aqui hace el mismo programa con menos instrucciones, y
//! se mide en el DUELO (`metro --duelo`: el mismo programa en C y en TITAN++).
//!
//! ```text
//!    AF-1  lo que una fn solo LEE le llega como puntero, sin copia
//!    AF1   `if` / `while` sobre una comparacion de enteros (y su `and`):
//!          el salto con el `cmp` mismo, sin el si/no de en medio
//!    AF3   el local entero que mas se usa (pesando los bucles) vive en un
//!          REGISTRO, `r14`, guardado al entrar y devuelto al volver
//!    AF4   las cuentas de enteros DERECHO en `rax` (sin dejar cada paso en
//!          la pila), el `cmp` que lee el otro lado de su sitio, un divisor
//!          escrito que no se mira, y la vuelta de un `for` en el sitio de
//!          su contador (sin copiarlo en cada vuelta)
//!    B1    hasta cuatro locales en registros (`REGISTROS`), y el otro lado
//!          de una cuenta de su registro o su sitio
//!    B2    lo MENOS que vale cada local, demostrado (`minimos`): divisores
//!          que no se miran, `% 2^k` que es un `and`
//!    B3    el bucle girado (`vuelta_girada`): un salto por vuelta
//!    B4    `imul` con el numero dentro, `byte(x)` que se demuestra que cabe,
//!          `a + b` con `b` ya en `rax`
//! ```
//! (AF0, los numeros cortos, vive en `E1::imm`.)

use super::E1;
use bmo_lower::x86::{self, RAX, RCX};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{End, Function, Lib, Module, Op, PathStep, Value};
use bmo_titan_front::tree::Mode;

impl<'a> E1<'a> {
    /// ** AF1: el SALTO de un `if` o un `while` cuya condicion es comparar
    /// dos numeros enteros (`<`, `<=`, `>`, `>=`, `==`, `!=`): el `cmp` y el
    /// salto de cuando NO se cumple, sin el si/no de en medio. Da el sitio
    /// del salto a parchear, o `None` si la condicion es otra cosa (y nada
    /// emitido). Si calcular un lado dejo algo con monton que soltar, se
    /// suelta ANTES del `cmp` (soltar llama y pisa las banderas): sus dos
    /// valores ya estan en su sitio de la pila, que no se suelta.
    /// Si la condicion se puede saltar asi (`salto_directo`): una
    /// comparacion de enteros, o un `and` de dos que se pueden.
    pub(super) fn se_funde(&self, cond: &Value) -> Result<bool, String> {
        let Value::Bin(op, a, b, _) = cond else { return Ok(false) };
        if *op == "and" {
            return Ok(self.se_funde(a)? && self.se_funde(b)?);
        }
        if !matches!(*op, "<" | "<=" | ">" | ">=" | "==" | "!=") {
            return Ok(false);
        }
        let entero = |c: &Class| matches!(c, Class::Int | Class::Byte);
        Ok(entero(&self.class_of(a)?) && entero(&self.class_of(b)?))
    }

    /// Los saltos de cuando NO se cumple (`se_funde` dijo que si). Un `and`
    /// salta en el primero que no se cumpla, y el de la derecha solo se
    /// calcula si el de la izquierda se cumplio: el mismo atajo de siempre.
    pub(super) fn salto_directo(&mut self, cond: &Value) -> Result<Vec<usize>, String> {
        let Value::Bin(op, a, b, _) = cond else { return Ok(Vec::new()) };
        if *op == "and" {
            let mut v = self.salto_directo(a)?;
            v.extend(self.salto_directo(b)?);
            return Ok(v);
        }
        let no_se_cumple = match *op {
            "<" => 0x8D,
            "<=" => 0x8F,
            ">" => 0x8E,
            ">=" => 0x8C,
            "==" => 0x85,
            _ => 0x84,
        };
        // AF3: un local contra un numero escrito, comparado EN SU SITIO
        // (el registro, o la pila): una instruccion
        if let (Value::Local(l, _), Value::Int(v, _)) = (&**a, &**b) {
            if let Ok(v) = i32::try_from(*v) {
                let p = self.local(*l);
                if self.cmp_en_sitio(p, v) {
                    return Ok(vec![self.jcc(no_se_cumple)]);
                }
            }
        }
        // AF2b y AF4: contra un numero escrito, el numero va DENTRO del
        // `cmp`, y el otro lado se calcula derecho en `rax`
        if let Value::Int(v, _) = &**b {
            if let Ok(v) = i32::try_from(*v) {
                self.a_rax(a)?;
                self.suelta_con_rax();
                x86::cmp_r64_imm32(&mut self.code, RAX, v);
                return Ok(vec![self.jcc(no_se_cumple)]);
            }
        }
        // AF3: el local del registro contra otro local: el registro mismo,
        // y el otro de su sitio (AF4: sin cargarlo antes)
        if let (Value::Local(l, _), Value::Local(k, _)) = (&**a, &**b) {
            if let super::Base::Reg(r) = self.local(*l).base {
                let pb = self.local(*k);
                if !self.cmp_con_sitio(r, pb) {
                    self.load(pb, RCX);
                    x86::cmp_r64_r64(&mut self.code, r, RCX);
                }
                return Ok(vec![self.jcc(no_se_cumple)]);
            }
        }
        // AF4: contra un local, el `cmp` lo lee de su sitio
        if let Value::Local(k, _) = &**b {
            self.a_rax(a)?;
            self.suelta_con_rax();
            let pb = self.local(*k);
            if !self.cmp_con_sitio(RAX, pb) {
                self.load(pb, RCX);
                x86::cmp_r64_r64(&mut self.code, RAX, RCX);
            }
            return Ok(vec![self.jcc(no_se_cumple)]);
        }
        // `a` se guarda aparte solo si calcular `b` puede tocar lo que
        // leyo (una llamada); un numero escrito, no
        self.a_rax(a)?;
        if let Value::Int(n, _) = &**b {
            self.suelta_con_rax();
            self.imm(RCX, *n);
        } else {
            let t = self.temp(8);
            self.store(t, RAX);
            self.a_rax(b)?;
            self.suelta_con_rax();
            x86::mov_r64_r64(&mut self.code, RCX, RAX);
            self.load(t, RAX);
        }
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        Ok(vec![self.jcc(no_se_cumple)])
    }

    /// Lo que calcular dejo con monton se suelta ANTES del `cmp` (soltar
    /// llama y pisa las banderas), y el valor de `rax` pasa por la pila.
    fn suelta_con_rax(&mut self) {
        if !self.owned.is_empty() {
            let t = self.temp(8);
            self.store(t, RAX);
            self.drop_owned_from(0);
            self.load(t, RAX);
        }
    }
}

/// ** PRESTADO PARA LEER (16-10, LE2 de PL18 de PLAN_ECCE). `fn f(n: T)`
/// es una COPIA (nivel 7), y E1 la hacia entera al entrar: una `[byte]` de
/// 20 KB pasada a una fn pequenia costaba 200 mil pasos por llamada (medido:
/// la letra de ECCE tardaba 100 millones en pintar una cabecera). Si la fn
/// NUNCA puede cambiar lo que le llega ni quedarselo, la copia no se
/// distingue del original: va un PUNTERO, como con `mut`, y no se suelta al
/// volver (es del que llama).
///
/// Cuando, y solo cuando: llega copiado; es una lista, una tabla o un mapa
/// (con monton) sin `dec` dentro (su `fit` es la copia misma); y en todo el
/// cuerpo no se le da otro valor, ni se presta (`mut`) ni se entrega
/// (`take`), ni se GUARDA entero en otro sitio (un `let`, un `=`, un
/// `return`, dentro de una tabla, un registro, un caso, un mapa o un
/// `push`). Leerlo -- una celda, `len`, `for`, compararlo, pasarlo COPIADO a
/// otra fn -- no lo cambia. Quien llama no puede tocarlo mientras: la fn es
/// lo unico que corre, y prestar y leer lo mismo en una llamada es T0076.
pub(super) fn solo_se_lee(f: &Function, i: usize, c: &Class) -> bool {
    if f.modes.get(i).copied().unwrap_or(Mode::Copy) != Mode::Copy || f.dispatch.is_some() || f.gpu {
        return false;
    }
    if !matches!(c, Class::List(_) | Class::Map(..) | Class::Table(..)) || format!("{:?}", f.params[i].1).contains("Dec") || format!("{c:?}").contains("Dec") {
        return false;
    }
    let l = f.params[i].0;
    let toca = |v: &Value| se_queda(v, l, true);
    let lee = |v: &Value| se_queda(v, l, false);
    for b in &f.blocks {
        for op in &b.ops {
            let mal = match op {
                Op::Let { local, value, .. } | Op::Set { local, value, .. } => *local == l || toca(value),
                Op::SetAt { local, path, value, .. } => *local == l || toca(value) || path.iter().any(|p| matches!(p, PathStep::Index(v) if lee(v))),
                Op::Write { parts: vs, .. } | Op::Call { args: vs, .. } | Op::Director { args: vs, .. } => vs.iter().any(lee),
                Op::Drop { .. } => false,
            };
            if mal {
                return false;
            }
        }
        let mal = match &b.end {
            End::Return(Some(v)) => toca(v),
            End::Branch { cond, .. } => lee(cond),
            _ => false,
        };
        if mal {
            return false;
        }
    }
    true
}

/// Si el valor `v` presta o entrega el local `l`, o lo GUARDA entero (`aqui`:
/// `v` mismo va a un sitio que se queda con el).
fn se_queda(v: &Value, l: usize, aqui: bool) -> bool {
    let guarda = |v: &Value| se_queda(v, l, true);
    let lee = |v: &Value| se_queda(v, l, false);
    match v {
        Value::Local(k, _) => aqui && *k == l,
        Value::Lend(_, k, _) => *k == l,
        Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Read(..) => false,
        Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => items.iter().any(guarda),
        Value::Map(items, _) => items.iter().any(|(k, w)| guarda(k) || guarda(w)),
        Value::Repeat(w, _, _) => guarda(w),
        Value::Lib(lib, args, _) => match lib {
            // leer: la celda de un `for`, `get`, `has`, el ultimo
            Lib::Turn | Lib::Get | Lib::Has | Lib::Last => args.iter().any(lee),
            _ => args.iter().enumerate().any(|(k, a)| if k == 0 { lee(a) } else { guarda(a) }),
        },
        Value::Call(_, args, _) | Value::Director(_, args, _) => args.iter().any(lee),
        Value::Index(b, i, _) | Value::Bin(_, b, i, _) => lee(b) || lee(i),
        Value::Field(w, _, _) | Value::Len(w, _) | Value::Round(w, _, _) | Value::Is(w, _, _, _) | Value::Payload(w, _, _, _, _) | Value::Number(w, _, _) | Value::Byte(w, _) | Value::Lamina(w, _) | Value::Neg(w, _) | Value::Not(w, _) => lee(w),
    }
}


impl E1<'_> {
    /// AF0: lo corto que cabe (sin tocar las banderas: nada de `xor`, que
    /// alguien puede tener un `cmp` pendiente). `mov r32` pone a cero lo de
    /// arriba; uno negativo de 32 bits, `mov r64` con signo.
    pub(super) fn imm_corto(&mut self, reg: u8, v: i64) {
        if (0..=u32::MAX as i64).contains(&v) {
            x86::mov_r32_imm32(&mut self.code, reg, v as u32);
        } else if (i32::MIN as i64..0).contains(&v) {
            self.code.push(0x48 | ((reg >> 3) & 1));
            self.code.push(0xC7);
            self.code.push(0xC0 | (reg & 7));
            self.code.extend_from_slice(&(v as i32).to_le_bytes());
        } else {
            x86::mov_r64_imm64(&mut self.code, reg, v as u64);
        }
    }

    /// AF3: si `local = local + n` (o `- n`) de un entero, hecho EN SU SITIO
    /// (`suma_en_sitio`); `false`, y nada emitido, si es otra cosa.
    pub(super) fn suma_propia(&mut self, local: usize, value: &Value) -> bool {
        let Value::Bin(op @ ("+" | "-"), a, b, at) = value else { return false };
        let (Value::Local(l, _), Value::Int(v, _)) = (&**a, &**b) else { return false };
        let entero = self.f.known[local] == Some(Class::Int) && self.f.decl[local].as_ref().is_none_or(|t| matches!(t, bmo_titan_front::tree::Ty::Int));
        if *l != local || !entero {
            return false;
        }
        let v = if *op == "+" { Some(*v) } else { v.checked_neg() };
        let p = self.local(local);
        let hecho = v.is_some_and(|v| self.suma_en_sitio(p, v, *at));
        if hecho {
            self.guardado = None;
        }
        hecho
    }

    /// AF3: `cmp` de un local en su sitio con un numero. `false` si el
    /// sitio no es ni el registro ni la pila (un `mut` prestado).
    pub(super) fn cmp_en_sitio(&mut self, p: super::Place, v: i32) -> bool {
        match p.base {
            super::Base::Reg(r) => {
                x86::cmp_r64_imm32(&mut self.code, r, v);
                true
            }
            super::Base::Frame => {
                // cmp qword [rbp + disp32], imm32
                self.code.extend_from_slice(&[0x48, 0x81, 0xBD]);
                self.code.extend_from_slice(&p.off.to_le_bytes());
                self.code.extend_from_slice(&v.to_le_bytes());
                true
            }
            super::Base::Ptr(_) => false,
        }
    }

    /// AF3: `x = x + v` (o `- v`) EN SU SITIO: un `add` al registro o a la
    /// pila, y el `jo` de siempre. `false` si el sitio es otro.
    pub(super) fn suma_en_sitio(&mut self, p: super::Place, v: i64, at: (usize, usize)) -> bool {
        let Ok(v32) = i32::try_from(v) else { return false };
        match p.base {
            super::Base::Reg(r) => self.suma_inmediata(r, v),
            super::Base::Frame => {
                // add qword [rbp + disp32], imm8 / imm32
                if let Ok(b) = i8::try_from(v) {
                    self.code.extend_from_slice(&[0x48, 0x83, 0x85]);
                    self.code.extend_from_slice(&p.off.to_le_bytes());
                    self.code.push(b as u8);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x81, 0x85]);
                    self.code.extend_from_slice(&p.off.to_le_bytes());
                    self.code.extend_from_slice(&v32.to_le_bytes());
                }
            }
            super::Base::Ptr(_) => return false,
        }
        self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
        true
    }
}

/// El registro de AF3. E1 tiene los preservados repartidos (`rbx` lo
/// tenido, `r12` la ventana, `r13` la lamina, `r15` el bloque del programa:
/// su pila y su monton); `r14` era el monton y desde AF3 el monton vive en
/// `[r15 + 8]` y solo lo carga en sus subrutinas. Es un PRESERVADO: quien lo
/// usa lo guarda y lo devuelve (el kernel tambien: INTI vive de eso).
pub(super) const R14: u8 = 14;

/// Lo que un registro tiene que pagar: guardarlo al entrar y devolverlo al
/// volver son dos instrucciones por llamada a la fn.
const PESO_MINIMO: u64 = 30;

/// ** AF3: EL LOCAL QUE VA A `r14`, si alguno lo merece. Un local que solo
/// es un ENTERO (`solo_int`, de `plan`), que nunca se presta ni se entrega
/// (un puntero a un registro no existe), que no es un parametro `mut`, y el
/// que mas se usa: cada uso pesa 10 por cada bucle que lo rodea. Una fn que
/// llama a una `gpu fn` no: su x86-64 nativo no se ha mirado entero.
pub(super) fn en_registro(f: &Function, solo_int: &[bool], vuelta: &[Option<usize>], m: &Module, pool: &[u8], raiz: bool) -> Vec<(usize, u8)> {
    if f.gpu || f.dispatch.is_some() {
        return Vec::new();
    }
    let n = f.blocks.len();
    let mut hondo = vec![0u32; n];
    for (k, b) in f.blocks.iter().enumerate() {
        for t in b.end.targets() {
            if t <= k {
                for h in hondo.iter_mut().take(k + 1).skip(t) {
                    *h += 1;
                }
            }
        }
    }
    // sin un bucle no hay nada que ganar: el registro solo cobra en vueltas
    if hondo.iter().all(|&h| h == 0) {
        return Vec::new();
    }
    let mut cuenta = Cuenta::recorre(f, m, &hondo);
    // AF4: la vuelta de un `for` vive en el sitio de su contador
    for (c, v) in vuelta.iter().enumerate() {
        if let Some(v) = v {
            cuenta.peso[*v] += cuenta.peso[c];
            cuenta.fuera[*v] |= cuenta.fuera[c];
            cuenta.fuera[c] = true;
        }
    }
    if cuenta.gpu {
        return Vec::new();
    }
    let mut elegidos: Vec<usize> = (0..f.locals.len()).filter(|&l| solo_int.get(l).copied().unwrap_or(false) && !cuenta.fuera[l] && cuenta.peso[l] >= PESO_MINIMO).collect();
    elegidos.sort_by_key(|&l| (std::cmp::Reverse(cuenta.peso[l]), l));
    // B1: el primero (`r14`) basta con PESO_MINIMO; los demas, en una fn que
    // se llama muchas veces, piden diez veces mas (medido en el metro: un
    // bucle de pocas vueltas pagaba mas guardando el registro que usandolo).
    // En `main`, que corre una vez, guardarlos cuesta dos instrucciones: le
    // basta el doble (un uso de un bucle mas que uno suelto no la paga).
    elegidos.into_iter().enumerate().filter(|(k, l)| *k == 0 || (!cuenta.pasa[*l] && cuenta.peso[*l] >= if raiz { 2 * PESO_MINIMO } else { 10 * PESO_MINIMO })).map(|(_, l)| l).zip(pool.iter().copied()).collect()
}

/// ** B1: los registros de los locales, en orden: `r14` siempre; `rbx`,
/// `r12` y `r13` solo si el programa no habla con el director (alli son lo
/// tenido, la ventana y la lamina). Los cuatro son PRESERVADOS: quien los
/// usa los guarda al entrar y los devuelve al volver, y las subrutinas de la
/// casa (el monton, la letra) los respetan.
pub(super) const REGISTROS: [u8; 4] = [R14, 3, 12, 13];

impl<'a> Cuenta<'a> {
    /// Cada uso de cada local, pesado por los bucles (`hondo`: cuantos
    /// rodean cada bloque), y los que se prestan o se cambian por partes.
    fn recorre(f: &Function, m: &'a Module, hondo: &[u32]) -> Cuenta<'a> {
        let mut cuenta = Cuenta { peso: vec![0; f.locals.len()], fuera: vec![false; f.locals.len()], pasa: vec![false; f.locals.len()], gpu: false, w: 1, m };
        for (i, (l, _)) in f.params.iter().enumerate() {
            if f.modes.get(i).copied().unwrap_or(Mode::Copy) != Mode::Copy {
                cuenta.fuera[*l] = true;
            }
        }
        for (k, b) in f.blocks.iter().enumerate() {
            if b.dead {
                continue;
            }
            cuenta.w = 10u64.pow(hondo[k].min(4));
            for op in &b.ops {
                match op {
                    Op::Let { local, value, .. } | Op::Set { local, value, .. } => {
                        cuenta.peso[*local] += cuenta.w;
                        cuenta.valor(value);
                    }
                    Op::SetAt { local, path, value, .. } => {
                        cuenta.fuera[*local] = true;
                        cuenta.valor(value);
                        for p in path {
                            if let PathStep::Index(v) = p {
                                cuenta.valor(v);
                            }
                        }
                    }
                    Op::Call { func, args, .. } => {
                        cuenta.gpu |= m.functions[*func].gpu;
                        cuenta.pasan(args);
                    }
                    Op::Director { args: vs, .. } => cuenta.pasan(vs),
                    Op::Write { parts: vs, .. } => {
                        for v in vs {
                            cuenta.valor(v);
                        }
                    }
                    Op::Drop { .. } => {}
                }
            }
            if let End::Return(Some(v)) | End::Branch { cond: v, .. } = &b.end {
                cuenta.valor(v);
            }
        }
        cuenta
    }
}

/// ** AF4: LA VUELTA DE UN `for` ES SU CONTADOR. `for i in range(a, b)`
/// baja (en `ir.rs`) a un contador escondido (`#i`) que se suma al final de
/// cada vuelta, y una `i` que lo COPIA al empezarla. `i` no se puede cambiar
/// (no es `mut`), y el contador solo cambia entre dos vueltas, cuando `i` ya
/// murio: los dos pueden vivir en el MISMO sitio, y la copia sobra. Da, por
/// cada contador, la vuelta en cuyo sitio vive -- si ningun camino presta la
/// vuelta ni la cambia --. Dos `for` seguidos con la misma `i` comparten su
/// sitio: no viven a la vez (el nombre esta tomado mientras uno vive).
pub(super) fn vueltas(f: &Function, m: &Module) -> Vec<Option<usize>> {
    let n = f.locals.len();
    let mut v = vec![None; n];
    let mut tocada = vec![false; n];
    for b in &f.blocks {
        for op in &b.ops {
            match op {
                Op::Let { local, value: Value::Local(c, _), mutable: false, ty: None, .. } if f.locals[*c].name.starts_with("#i") && !f.locals[*local].name.starts_with('#') => v[*c] = Some(*local),
                Op::Set { local, .. } | Op::SetAt { local, .. } => tocada[*local] = true,
                _ => {}
            }
        }
    }
    let prestada = Cuenta::recorre(f, m, &vec![0; f.blocks.len()]).fuera;
    for (c, w) in v.iter_mut().enumerate() {
        if w.is_some_and(|w| tocada[w] || prestada[w] || prestada[c]) {
            *w = None;
        }
    }
    v
}

struct Cuenta<'a> {
    peso: Vec<u64>,
    fuera: Vec<bool>,
    /// B1: va DERECHO de valor a una llamada o al director: se le pide su
    /// direccion, y en un registro seria una copia.
    pasa: Vec<bool>,
    gpu: bool,
    w: u64,
    m: &'a Module,
}

impl Cuenta<'_> {
    /// Los valores de una llamada: un local que va tal cual, `pasa`.
    fn pasan(&mut self, items: &[Value]) {
        for i in items {
            if let Value::Local(l, _) = i {
                self.pasa[*l] = true;
            }
            self.valor(i);
        }
    }

    fn valor(&mut self, v: &Value) {
        match v {
            Value::Local(l, _) => self.peso[*l] += self.w,
            Value::Lend(_, l, _) => self.fuera[*l] = true,
            Value::Int(..) | Value::Text(..) | Value::Bool(..) | Value::Dec(..) | Value::F32(..) | Value::Read(..) => {}
            Value::Table(items, _) | Value::Record(_, items, _) | Value::Variant(_, _, items, _) => {
                for i in items {
                    self.valor(i);
                }
            }
            Value::Lib(_, items, _) => {
                for i in items {
                    self.valor(i);
                }
            }
            Value::Director(_, items, _) => self.pasan(items),
            Value::Call(func, items, _) => {
                self.gpu |= self.m.functions[*func].gpu;
                self.pasan(items);
            }
            Value::Map(items, _) => {
                for (k, w) in items {
                    self.valor(k);
                    self.valor(w);
                }
            }
            Value::Index(a, b, _) | Value::Bin(_, a, b, _) => {
                self.valor(a);
                self.valor(b);
            }
            Value::Repeat(w, _, _) | Value::Field(w, _, _) | Value::Len(w, _) | Value::Round(w, _, _) | Value::Is(w, _, _, _) | Value::Payload(w, _, _, _, _) | Value::Number(w, _, _) | Value::Byte(w, _) | Value::Lamina(w, _) | Value::Neg(w, _) | Value::Not(w, _) => self.valor(w),
        }
    }
}

impl E1<'_> {
    /// ** AF4: una cuenta de enteros (`+ - * / %`) hecha DERECHO en `rax`,
    /// sin dejar cada paso en la pila para leerlo otra vez. `false`, y nada
    /// emitido, si `v` no es una cuenta de dos enteros.
    pub(super) fn entero_a_rax(&mut self, v: &Value) -> Result<bool, String> {
        let Value::Bin(op, a, b, at) = v else { return Ok(false) };
        self.cuenta_a_rax(op, a, b, *at)
    }

    pub(super) fn cuenta_a_rax(&mut self, op: &str, a: &Value, b: &Value, at: (usize, usize)) -> Result<bool, String> {
        if !matches!(op, "+" | "-" | "*" | "/" | "%") || (op == "/" && self.lenient > 0) {
            return Ok(false);
        }
        let entero = |c: &Class| matches!(c, Class::Int | Class::Byte);
        if !entero(&self.class_of(a)?) || !entero(&self.class_of(b)?) {
            return Ok(false);
        }
        let escrito = if let Value::Int(n, _) = b { Some(*n) } else { None };
        // AF2b: `x + 1`, `x - 1` con el numero DENTRO de la instruccion
        if let (Some(n), "+" | "-") = (escrito, op) {
            let n = if op == "+" { Some(n) } else { n.checked_neg() };
            if let Some(n) = n.filter(|n| i32::try_from(*n).is_ok()) {
                self.a_rax(a)?;
                self.suma_inmediata(RAX, n);
                self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
                return Ok(true);
            }
        }
        // B2: `x % 2^k` de un `x` que se DEMUESTRA no negativo: un `and`
        if let (Some(n), "%") = (escrito, op) {
            if n > 0 && (n as u64).is_power_of_two() && n <= i32::MAX as i64 && self.cota(a).is_some_and(|m| m >= 0) {
                self.a_rax(a)?;
                x86::and_r64_imm32(&mut self.code, RAX, (n - 1) as u32);
                return Ok(true);
            }
        }
        // B4: por un numero escrito, el numero va DENTRO del `imul`
        if let (Some(n), "*") = (escrito, op) {
            if let Ok(n) = i32::try_from(n) {
                self.a_rax(a)?;
                // imul rax, rax, imm8 / imm32
                match i8::try_from(n) {
                    Ok(b) => self.code.extend_from_slice(&[0x48, 0x6B, 0xC0, b as u8]),
                    Err(_) => {
                        self.code.extend_from_slice(&[0x48, 0x69, 0xC0]);
                        self.code.extend_from_slice(&n.to_le_bytes());
                    }
                }
                self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
                return Ok(true);
            }
        }
        let mut seguro = escrito.is_some_and(|d| d != 0 && d != -1);
        match (a, b) {
            // `b` se lee sin tocar `rax`: `a` primero, y `b` a `rcx`
            (_, Value::Int(n, _)) => {
                self.a_rax(a)?;
                self.imm(RCX, *n);
            }
            (_, Value::Local(l, _)) => {
                self.a_rax(a)?;
                let p = self.local(*l);
                // B2: un divisor que se demuestra 1 o mas no se mira
                seguro = self.cota(b).is_some_and(|m| m >= 1);
                // B1: el otro lado, de su registro o de su sitio
                if self.opera_con_sitio(op, p, seguro, at)? {
                    return Ok(true);
                }
                self.load(p, RCX);
            }
            // un numero escrito no cambia: `b` primero y el numero detras
            (Value::Int(n, _), _) => {
                self.a_rax(b)?;
                x86::mov_r64_r64(&mut self.code, RCX, RAX);
                self.imm(RAX, *n);
            }
            // dos cosas con su sitio (llamadas, celdas): cada una al suyo,
            // y las dos a los registros al final
            _ => {
                let (pa, _) = self.eval(a)?;
                let (pb, _) = self.eval(b)?;
                // B4: `+` y `*` dan lo mismo al reves: si `b` acaba de quedar
                // en `rax`, `a` se le suma (o multiplica) de su sitio
                if matches!(op, "+" | "*") && self.en_rax(pb) && self.opera_con_sitio(op, pa, false, at)? {
                    return Ok(true);
                }
                self.load(pa, RAX);
                self.load(pb, RCX);
            }
        }
        self.opera(op, seguro, at)?;
        Ok(true)
    }

    /// Un entero a `rax`: una cuenta, derecha (`entero_a_rax`); un numero
    /// escrito, con su `mov`; lo demas, de su sitio.
    pub(super) fn a_rax(&mut self, v: &Value) -> Result<(), String> {
        if let Value::Int(n, _) = v {
            self.imm(RAX, *n);
            return Ok(());
        }
        if !self.entero_a_rax(v)? {
            let (p, _) = self.eval(v)?;
            self.load(p, RAX);
        }
        Ok(())
    }

    /// AF4: `cmp reg, [sitio]` -- el otro lado sin cargarlo antes. `false`
    /// si el sitio no es la pila.
    pub(super) fn cmp_con_sitio(&mut self, reg: u8, p: super::Place) -> bool {
        match p.base {
            super::Base::Reg(r) => {
                x86::cmp_r64_r64(&mut self.code, reg, r);
                true
            }
            super::Base::Frame => {
                // cmp r64, qword [rbp + disp32]
                self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 });
                self.code.push(0x3B);
                self.code.push(0x85 | ((reg & 7) << 3));
                self.code.extend_from_slice(&p.off.to_le_bytes());
                true
            }
            super::Base::Ptr(_) => false,
        }
    }
}

impl E1<'_> {
    /// AF6: `reg += [sitio]` -- el otro lado sin cargarlo antes. `false` si
    /// el sitio no es la pila.
    pub(super) fn suma_de_sitio(&mut self, reg: u8, p: super::Place) -> bool {
        if p.base != super::Base::Frame {
            return false;
        }
        // add r64, qword [rbp + disp32]
        self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 });
        self.code.push(0x03);
        self.code.push(0x85 | ((reg & 7) << 3));
        self.code.extend_from_slice(&p.off.to_le_bytes());
        true
    }

    /// AF6: una cuenta que sube de uno (lo que mide una lista): EN su sitio
    /// si esta en la pila. Nunca desborda: antes se acaba la memoria.
    pub(super) fn uno_mas(&mut self, p: super::Place) {
        if p.base == super::Base::Frame {
            // add qword [rbp + disp32], 1
            self.code.extend_from_slice(&[0x48, 0x83, 0x85]);
            self.code.extend_from_slice(&p.off.to_le_bytes());
            self.code.push(1);
            self.guardado = None;
            return;
        }
        self.load(p, RAX);
        x86::inc_r64(&mut self.code, RAX);
        self.store(p, RAX);
    }
}

impl E1<'_> {
    /// ** AF5: la celda de `rax` esta DENTRO de lo que mide `n` (un sitio),
    /// o T0072. UN `cmp` sin signo: un negativo, visto sin signo, es enorme y
    /// tambien se pasa -- las dos miradas (`>= 0` y `< n`) en una.
    pub(super) fn dentro_de(&mut self, n: super::Place, why: &str, at: (usize, usize)) {
        if !self.cmp_con_sitio(RAX, n) {
            self.load(n, RCX);
            x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        }
        self.trap(0x83, "T0072", why, at);
    }
}

impl E1<'_> {
    /// ** B1: `rax op= <sitio>` sin pasar el otro lado por `rcx`: de su
    /// registro (`add rax, r14`, `idiv r14`) o de la pila (`add rax, [x]`).
    /// `false`, y nada emitido, si no se puede (un puntero, o una division
    /// cuyo divisor hay que mirar: eso va por `opera`).
    fn opera_con_sitio(&mut self, op: &str, p: super::Place, seguro: bool, at: (usize, usize)) -> Result<bool, String> {
        match (op, p.base) {
            ("+" | "-" | "*", super::Base::Reg(r)) => match op {
                "+" => x86::add_r64_r64(&mut self.code, RAX, r),
                "-" => x86::sub_r64_r64(&mut self.code, RAX, r),
                _ => x86::imul_r64_r64(&mut self.code, RAX, r),
            },
            ("+" | "-" | "*", super::Base::Frame) => {
                // add / sub / imul rax, qword [rbp + disp32]
                match op {
                    "+" => self.code.extend_from_slice(&[0x48, 0x03, 0x85]),
                    "-" => self.code.extend_from_slice(&[0x48, 0x2B, 0x85]),
                    _ => self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0x85]),
                }
                self.code.extend_from_slice(&p.off.to_le_bytes());
            }
            ("/" | "%", super::Base::Reg(r)) if seguro => {
                x86::cqo(&mut self.code);
                x86::idiv_r64(&mut self.code, r);
                if op == "/" {
                    x86::test_r64_r64(&mut self.code, bmo_lower::x86::RDX, bmo_lower::x86::RDX);
                    self.trap(0x85, "T0062", "una division que no es exacta (entre enteros, 7 / 2 no es 3)", at);
                } else {
                    x86::mov_r64_r64(&mut self.code, RAX, bmo_lower::x86::RDX);
                }
                return Ok(true);
            }
            _ => return Ok(false),
        }
        self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
        Ok(true)
    }

    /// B4: `byte(v)` cabe sin mirarlo: `x % k` (k de 1 a 256) de un `x` que
    /// se demuestra no negativo va de 0 a k - 1.
    pub(super) fn cabe_en_byte(&self, v: &Value) -> bool {
        match v {
            Value::Bin("%", a, b, _) => matches!(&**b, Value::Int(k, _) if (1..=256).contains(k)) && self.cota(a).is_some_and(|m| m >= 0),
            Value::Int(k, _) => (0..=255).contains(k),
            _ => false,
        }
    }

    /// B4: `p` se acaba de guardar desde `rax`, y `rax` lo sigue teniendo.
    fn en_rax(&self, p: super::Place) -> bool {
        matches!(self.guardado, Some((fin, q, RAX)) if fin == self.code.len() && q == p && p.base == super::Base::Frame)
    }

    /// B2: lo MENOS que puede valer `v` al correr, si se demuestra
    /// (`minimos`).
    pub(super) fn cota(&self, v: &Value) -> Option<i64> {
        cota(v, &self.f.minimo)
    }
}

/// Lo menos que vale `v`, con lo menos que vale cada local (`m`).
fn cota(v: &Value, m: &[Option<i64>]) -> Option<i64> {
    match v {
        Value::Int(k, _) => Some(*k),
        Value::Local(l, _) => m.get(*l).copied().flatten(),
        Value::Len(..) | Value::Byte(..) => Some(0),
        Value::Bin(op, a, b, _) => {
            let (ca, cb) = (cota(a, m), cota(b, m));
            match *op {
                "+" => ca?.checked_add(cb?),
                // lo menos de un producto de dos no negativos: el de sus menos
                "*" if ca? >= 0 && cb? >= 0 => ca?.checked_mul(cb?),
                "-" => match &**b {
                    Value::Int(k, _) => ca?.checked_sub(*k),
                    _ => None,
                },
                "%" if ca? >= 0 && matches!(&**b, Value::Int(k, _) if *k > 0) => Some(0),
                "/" => match &**b {
                    Value::Int(k, _) if *k > 0 && ca? >= 0 => Some(ca? / *k),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

/// ** B2: LO MENOS QUE VALE CADA LOCAL ENTERO, demostrado al compilar. Un
/// local que solo es un entero, que no llega de fuera (un parametro) y que
/// nunca se presta: lo menos de todo lo que se le da (`let` y `=`). Se
/// empieza creyendo que nada lo baja y se repite hasta que nada cambia: si
/// `d = 2` y `d = d + 1` son todo lo que se le da, `d` nunca baja de 2. Lo
/// que no se asienta en 64 vueltas (`x = x - 1` baja siempre) no se sabe.
///
/// Es verdad en TODO momento del programa, no solo en un sitio: cada valor
/// que el local llega a tener es uno de esos (un desborde ATRAPA antes de
/// guardarse), y nada se lee antes de darle valor (lo mira `check`).
pub(super) fn minimos(f: &Function, solo_int: &[bool], m: &Module) -> Vec<Option<i64>> {
    let n = f.locals.len();
    let fuera = Cuenta::recorre(f, m, &vec![0; f.blocks.len()]).fuera;
    let mut es = vec![false; n];
    for l in 0..n {
        es[l] = solo_int.get(l).copied().unwrap_or(false) && !fuera[l] && !f.params.iter().any(|(p, _)| *p == l);
    }
    let mut cur: Vec<Option<i64>> = (0..n).map(|l| es[l].then_some(i64::MAX)).collect();
    for _ in 0..64 {
        let mut cambio = false;
        for b in f.blocks.iter().filter(|b| !b.dead) {
            for op in &b.ops {
                if let Op::Let { local, value, .. } | Op::Set { local, value, .. } = op {
                    if !es[*local] {
                        continue;
                    }
                    let nuevo = match (cur[*local], cota(value, &cur)) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        _ => None,
                    };
                    if nuevo != cur[*local] {
                        cur[*local] = nuevo;
                        cambio = true;
                    }
                }
            }
        }
        if !cambio {
            // lo que nadie le dio valor no se sabe
            return cur.into_iter().map(|c| c.filter(|&c| c != i64::MAX)).collect();
        }
    }
    vec![None; n]
}

impl E1<'_> {
    /// ** B3: EL BUCLE GIRADO. La vuelta de un bucle (`i` salta ARRIBA, a `t`)
    /// saltaba a la cabeza, que miraba la condicion y saltaba otra vez: dos
    /// saltos por vuelta. Si la cabeza no hace nada mas que mirar una
    /// comparacion de enteros (`se_funde`, sin `and`), la vuelta la mira
    /// aqui mismo y salta al cuerpo SOLO si se cumple: un salto por vuelta.
    /// Da los saltos a parchear (al cuerpo, y a la salida si no va detras), o
    /// `None` si no se gira (y nada emitido).
    pub(super) fn vuelta_girada(&mut self, f: &Function, i: usize, t: usize) -> Result<Option<Vec<(usize, usize)>>, String> {
        if t > i {
            return Ok(None);
        }
        let cabeza = &f.blocks[t];
        let End::Branch { cond, then, other, .. } = &cabeza.end else { return Ok(None) };
        if cabeza.dead || !cabeza.ops.is_empty() || matches!(cond, Value::Bin("and", ..)) || !self.se_funde(cond)? {
            return Ok(None);
        }
        let saltos = self.salto_directo(cond)?;
        let [j] = saltos[..] else { return Err("B3: una comparacion dio mas de un salto (fallo de E1)".into()) };
        // el salto de cuando NO se cumple, al reves: el de cuando SI
        self.code[j - 1] ^= 1;
        let mut out = vec![(j, *then)];
        if *other != i + 1 {
            out.push((self.jmp(), *other));
        }
        Ok(Some(out))
    }
}
