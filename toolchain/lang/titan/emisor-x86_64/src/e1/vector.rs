//! ** AF9: EL BUCLE DE DATOS EN VECTORES (`docs/plan/PLAN_TITAN_AFINADO.md`).
//!
//! El propietario, 16-10: *"sobre simd eso usa aprende de clang y superarlo"*.
//! Clang suma una lista de bytes con `psadbw`: la suma de las diferencias,
//! byte a byte, contra CERO es la suma de 8 bytes, y un registro de 128 bits
//! hace dos de esas a la vez. TITAN++ no escribe vectores (la ley del plan):
//! el emisor reconoce el bucle y lo hace el.
//!
//! ```text
//!    for b in a          a: [byte], y el cuerpo es SOLO `s = s + b`
//!        s = s + b
//!
//!    antes del bucle, de 32 en 32:      pxor  xmm1, xmm1    (la cuenta)
//!                                       pxor  xmm2, xmm2    (el cero)
//!       mientras i + 32 <= fin:         movdqu xmm0, [a + i]
//!                                       psadbw xmm0, xmm2
//!                                       paddq  xmm1, xmm0   (y lo mismo + 16)
//!                                       i = i + 32
//!       s = s + las dos mitades de xmm1, con su `jo`
//!    y el bucle de siempre hace lo que queda (menos de 32)
//! ```
//!
//! ** El mismo programa: cada byte es 0 o mas, asi que la suma solo sube; si
//! se pasa de 64 bits, se pasa tambien sumada de golpe, y el NO es el mismo
//! en la misma linea (el cuerpo no hace nada mas que se vea). Las dos mitades
//! de `xmm1` no desbordan: 8 bytes son 2040 como mucho.
//!
//! SSE2: lo tiene toda CPU de x86-64 (y el PERFIL lo lleva en `mano`). AVX2
//! (32 bytes por instruccion) espera la decision del propietario.

use super::E1;
use bmo_lower::x86::{self, RAX, RCX, RDX};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{End, Function, Lib, Op, Value};

impl E1<'_> {
    /// Si el bloque `i` entra (`End::Jump(t)`) en un bucle `for b in a` de
    /// una `[byte]` cuyo cuerpo es solo `s = s + b`, emite aqui la parte en
    /// vectores. Nada emitido si no lo es.
    pub(super) fn suma_en_vectores(&mut self, f: &Function, t: usize) -> Result<(), String> {
        let Some((lista, cuenta, fin, s, at)) = self.es_suma_de_bytes(f, t) else { return Ok(()) };
        let (h, ci, pf, ps) = (self.local(lista), self.local(cuenta), self.local(fin), self.local(s));
        if matches!(h.base, super::Base::Reg(_)) {
            return Ok(());
        }
        // pxor xmm1, xmm1 ; pxor xmm2, xmm2
        self.code.extend_from_slice(&[0x66, 0x0F, 0xEF, 0xC9, 0x66, 0x0F, 0xEF, 0xD2]);
        let vuelta = self.etiqueta();
        self.load(ci, RAX);
        self.lea(RCX, RAX, 32);
        self.load(pf, RDX);
        x86::cmp_r64_r64(&mut self.code, RCX, RDX);
        let fuera = self.jcc(0x8F);
        self.load(h, RDX);
        x86::add_r64_r64(&mut self.code, RAX, RDX);
        for off in [0u8, 16] {
            // movdqu xmm0, [rax + off] ; psadbw xmm0, xmm2 ; paddq xmm1, xmm0
            self.code.extend_from_slice(&[0xF3, 0x0F, 0x6F, 0x40, off]);
            self.code.extend_from_slice(&[0x66, 0x0F, 0xF6, 0xC2, 0x66, 0x0F, 0xD4, 0xC8]);
        }
        self.store(ci, RCX);
        let j = self.jmp();
        x86::patch_jump_to(&mut self.code, j, vuelta);
        self.here(fuera);
        // las dos mitades: movq rax, xmm1 ; pshufd xmm0, xmm1, 0xEE ; movq rcx, xmm0
        self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC8, 0x66, 0x0F, 0x70, 0xC1, 0xEE, 0x66, 0x48, 0x0F, 0x7E, 0xC1]);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.load(ps, RCX);
        x86::add_r64_r64(&mut self.code, RCX, RAX);
        self.trap(0x80, "T0060", "el resultado no cabe en 64 bits", at);
        self.store(ps, RCX);
        self.guardado = None;
        Ok(())
    }

    /// `(la lista, el contador, el fin, s, la linea de s = s + b)` si el
    /// bucle de la cabeza `t` es una suma de bytes.
    fn es_suma_de_bytes(&self, f: &Function, t: usize) -> Option<(usize, usize, usize, usize, (usize, usize))> {
        let cabeza = f.blocks.get(t)?;
        let End::Branch { cond: Value::Bin("<", ci, fin, _), then, .. } = &cabeza.end else { return None };
        let (Value::Local(ci, _), Value::Local(fin, _)) = (&**ci, &**fin) else { return None };
        if !cabeza.ops.is_empty() || !f.locals[*ci].name.starts_with("#i") {
            return None;
        }
        let cuerpo = &f.blocks[*then];
        let mut ops = cuerpo.ops.iter().filter(|o| !matches!(o, Op::Drop { .. }));
        let Some(Op::Let { local: b, value: Value::Lib(Lib::Turn, args, _), .. }) = ops.next() else { return None };
        let [Value::Local(lista, _), Value::Local(c2, _)] = &args[..] else { return None };
        let Some(Op::Set { local: s, value: Value::Bin("+", x, y, _), at }) = ops.next() else { return None };
        if ops.next().is_some() || c2 != ci || s == b || s == ci || s == lista {
            return None;
        }
        let es = |v: &Value, l: usize| matches!(v, Value::Local(k, _) if *k == l);
        if !((es(x, *s) && es(y, *b)) || (es(x, *b) && es(y, *s))) {
            return None;
        }
        // el paso: `#i = #i + 1`, y vuelve a la cabeza
        let End::Jump(paso) = cuerpo.end else { return None };
        let paso_b = &f.blocks[paso];
        let [Op::Set { local: c3, value: Value::Bin("+", _, uno, _), .. }] = &paso_b.ops[..] else { return None };
        if c3 != ci || !matches!(**uno, Value::Int(1, _)) || !matches!(paso_b.end, End::Jump(h) if h == t) {
            return None;
        }
        let byte = matches!(&self.f.known[*lista], Some(Class::List(c)) if **c == Class::Byte);
        let entero = self.f.known[*s] == Some(Class::Int);
        (byte && entero).then_some((*lista, *ci, *fin, *s, *at))
    }
}
