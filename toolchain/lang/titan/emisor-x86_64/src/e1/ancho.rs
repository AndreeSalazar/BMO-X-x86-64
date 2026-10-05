//! **LOS 128 BITS DEL `dec`** -- donde el calculo (`calc/numero.rs`) hace un
//! paso intermedio en 128 bits, E1 tambien: alinear dos escalas muy
//! distintas, el producto antes de quitar sus ceros, el numerador y el
//! divisor de una division. Sin esto, E1 decia T0060 donde el calculo llegaba
//! al numero (lo encontro la prueba al azar contra el calculo).
//!
//! ```text
//!    un numero ancho   su MAGNITUD en 128 bits (lo, hi) en el marco de la
//!                      subrutina, y su signo aparte: sumar y restar
//!                      magnitudes es mas facil de ver que con complemento
//!    las piezas        poner, copiar, por 10, entre 10 (por trozos de 32
//!                      bits: cada `div` cabe), sumar, restar, comparar,
//!                      dividir (larga, bit a bit), y si cabe en un `dec`
//! ```
//!
//! Las subrutinas de aqui: comparar, sumar/restar, multiplicar y dividir dos
//! `dec` (cifras en rax/rdx, escalas en rcx/r8), con el estado en `r10`.

use super::E1;
use bmo_lower::x86::{self, RAX, RCX, RDX, R10, R11, R8, R9};

const RBP: u8 = super::RBP;

// el marco de cada subrutina: cuatro numeros anchos y unos cuantos sueltos
const A: i32 = -16;
const B: i32 = -32;
const C: i32 = -48;
const D: i32 = -64;
const SIGN_A: i32 = -72;
const SIGN_B: i32 = -80;
const SA: i32 = -88;
const SB: i32 = -96;
const S: i32 = -104;
const K: i32 = -112;
const S0: i32 = -120;
const FLAG: i32 = -128;
const FRAME: u8 = 144;

impl E1<'_> {
    fn enter(&mut self) {
        self.code.push(0x55);
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC, FRAME, 0, 0, 0]);
    }

    fn leave(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC]);
        self.code.push(0x5D);
        self.code.push(0xC3);
    }

    fn get(&mut self, reg: u8, off: i32) {
        x86::mov_r64_at_reg_disp32(&mut self.code, reg, RBP, off);
    }

    fn put(&mut self, off: i32, reg: u8) {
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBP, off, reg);
    }

    /// La magnitud de `reg` (con signo) al ancho `w`, y su signo a `sign`.
    fn w_from_signed(&mut self, w: i32, sign: i32, reg: u8) {
        x86::mov_r64_r64(&mut self.code, R11, reg);
        x86::shr_r64_imm8(&mut self.code, R11, 63);
        self.put(sign, R11);
        x86::mov_r64_r64(&mut self.code, RAX, reg);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let pos = self.jcc(0x89);
        x86::neg_r64(&mut self.code, RAX);
        self.here(pos);
        self.put(w, RAX);
        x86::zero_r32(&mut self.code, R11);
        self.put(w + 8, R11);
    }

    fn w_copy(&mut self, dst: i32, src: i32) {
        self.get(RAX, src);
        self.put(dst, RAX);
        self.get(RAX, src + 8);
        self.put(dst + 8, RAX);
    }

    /// w *= 10 (las magnitudes de aqui nunca pasan de 2^127).
    fn w_mul10(&mut self, w: i32) {
        self.imm(RCX, 10);
        self.get(RAX, w);
        x86::mul_r64(&mut self.code, RCX);
        self.put(w, RAX);
        x86::mov_r64_r64(&mut self.code, R11, RDX);
        self.get(RAX, w + 8);
        x86::mul_r64(&mut self.code, RCX);
        x86::add_r64_r64(&mut self.code, RAX, R11);
        self.put(w + 8, RAX);
    }

    /// w *= 10, `k` veces (k en el sitio K: se gasta).
    fn w_mul10_times(&mut self, w: i32) {
        let top = self.code.len();
        self.get(RAX, K);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let done = self.jcc(0x8E);
        self.w_mul10(w);
        self.get(RAX, K);
        x86::dec_r64(&mut self.code, RAX);
        self.put(K, RAX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(done);
    }

    /// w /= 10; el resto a rdx. Por trozos de 32 bits, de arriba abajo:
    /// cada `div` divide resto * 2^32 + trozo, que cabe en 64 bits, y su
    /// cociente en 32.
    fn w_div10(&mut self, w: i32) {
        self.imm(RCX, 10);
        x86::zero_r32(&mut self.code, RDX);
        for half in [w + 8, w] {
            // el trozo alto de esta mitad
            self.get(RAX, half);
            x86::shr_r64_imm8(&mut self.code, RAX, 32);
            self.div_chunk();
            x86::mov_r64_r64(&mut self.code, R8, RAX);
            // el bajo
            self.get(RAX, half);
            x86::shl_r64_imm8(&mut self.code, RAX, 32);
            x86::shr_r64_imm8(&mut self.code, RAX, 32);
            self.div_chunk();
            x86::shl_r64_imm8(&mut self.code, R8, 32);
            x86::or_r64_r64(&mut self.code, R8, RAX);
            self.put(half, R8);
        }
    }

    /// rax = (rdx * 2^32 + rax) / rcx, rdx = el resto (rax < 2^32, rdx < rcx).
    fn div_chunk(&mut self) {
        x86::shl_r64_imm8(&mut self.code, RDX, 32);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        x86::zero_r32(&mut self.code, RDX);
        x86::div_r64(&mut self.code, RCX);
    }

    /// dst += src.
    fn w_add(&mut self, dst: i32, src: i32) {
        self.get(RAX, dst);
        self.get(RCX, src);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.put(dst, RAX);
        x86::setcc_low(&mut self.code, 0x92, RDX);
        x86::movzx_r64_low(&mut self.code, RDX, RDX);
        self.get(RAX, dst + 8);
        self.get(RCX, src + 8);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        x86::add_r64_r64(&mut self.code, RAX, RDX);
        self.put(dst + 8, RAX);
    }

    /// dst -= src (dst >= src).
    fn w_sub(&mut self, dst: i32, src: i32) {
        self.get(RAX, dst);
        self.get(RCX, src);
        x86::sub_r64_r64(&mut self.code, RAX, RCX);
        self.put(dst, RAX);
        x86::setcc_low(&mut self.code, 0x92, RDX);
        x86::movzx_r64_low(&mut self.code, RDX, RDX);
        self.get(RAX, dst + 8);
        self.get(RCX, src + 8);
        x86::sub_r64_r64(&mut self.code, RAX, RCX);
        x86::sub_r64_r64(&mut self.code, RAX, RDX);
        self.put(dst + 8, RAX);
    }

    /// rax = -1, 0 o 1 segun a < b, a == b, a > b (magnitudes).
    fn w_cmp(&mut self, a: i32, b: i32) {
        let mut less = Vec::new();
        let mut more = Vec::new();
        for half in [8, 0] {
            self.get(RAX, a + half);
            self.get(RCX, b + half);
            x86::cmp_r64_r64(&mut self.code, RAX, RCX);
            less.push(self.jcc(0x82));
            more.push(self.jcc(0x87));
        }
        x86::zero_r32(&mut self.code, RAX);
        let end1 = self.jmp();
        for f in less {
            self.here(f);
        }
        self.imm(RAX, -1);
        let end2 = self.jmp();
        for f in more {
            self.here(f);
        }
        self.imm(RAX, 1);
        self.here(end1);
        self.here(end2);
    }

    fn w_is_zero(&mut self, w: i32) {
        self.get(RAX, w);
        self.get(RCX, w + 8);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
    }

    /// q, r = n / d, n % d: la division larga, bit a bit (d no es cero).
    fn w_divmod(&mut self, n: i32, d: i32, q: i32, r: i32) {
        x86::zero_r32(&mut self.code, RAX);
        for w in [q, q + 8, r, r + 8] {
            self.put(w, RAX);
        }
        self.imm(RAX, 127);
        self.put(K, RAX);
        let top = self.code.len();
        // r = r << 1 | el bit k de n
        self.get(RAX, r + 8);
        x86::shl_r64_imm8(&mut self.code, RAX, 1);
        self.get(RCX, r);
        x86::shr_r64_imm8(&mut self.code, RCX, 63);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        self.put(r + 8, RAX);
        self.get(RAX, r);
        x86::shl_r64_imm8(&mut self.code, RAX, 1);
        self.put(r, RAX);
        self.get(RCX, K);
        x86::cmp_r64_imm8(&mut self.code, RCX, 64);
        let low = self.jcc(0x8C);
        x86::sub_r64_imm8(&mut self.code, RCX, 64);
        self.get(RDX, n + 8);
        let got = self.jmp();
        self.here(low);
        self.get(RDX, n);
        self.here(got);
        x86::shr_r64_cl(&mut self.code, RDX);
        x86::and_r64_imm32(&mut self.code, RDX, 1);
        self.get(RAX, r);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        self.put(r, RAX);
        // si r >= d: r -= d y el bit k de q
        self.w_cmp(r, d);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let skip = self.jcc(0x88);
        self.w_sub(r, d);
        self.get(RCX, K);
        self.imm(RDX, 1);
        x86::cmp_r64_imm8(&mut self.code, RCX, 64);
        let qlow = self.jcc(0x8C);
        x86::sub_r64_imm8(&mut self.code, RCX, 64);
        x86::shl_r64_cl(&mut self.code, RDX);
        self.get(RAX, q + 8);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        self.put(q + 8, RAX);
        let set = self.jmp();
        self.here(qlow);
        x86::shl_r64_cl(&mut self.code, RDX);
        self.get(RAX, q);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        self.put(q, RAX);
        self.here(set);
        self.here(skip);
        self.get(RAX, K);
        x86::dec_r64(&mut self.code, RAX);
        self.put(K, RAX);
        let back = self.jcc(0x89);
        x86::patch_jump_to(&mut self.code, back, top);
    }

    /// El ancho `w` con el signo de `sign`, a rax -- o salta a `ovf` si no
    /// cabe en 64 bits con signo.
    fn w_to_signed(&mut self, w: i32, sign: i32, ovf: &mut Vec<usize>) {
        self.get(RAX, w + 8);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        ovf.push(self.jcc(0x85));
        self.get(RAX, w);
        self.get(RCX, sign);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let negative = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        ovf.push(self.jcc(0x88));
        let done = self.jmp();
        self.here(negative);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        ovf.push(self.jcc(0x87));
        x86::neg_r64(&mut self.code, RAX);
        self.here(done);
    }

    /// Los dos `dec` de los registros (rax, rcx) y (rdx, r8), anchos y a la
    /// misma escala: A y B, sus signos, y la escala en S.
    fn w_align(&mut self) {
        self.put(SA, RCX);
        self.put(SB, R8);
        x86::mov_r64_r64(&mut self.code, R9, RDX);
        self.w_from_signed(A, SIGN_A, RAX);
        self.w_from_signed(B, SIGN_B, R9);
        self.get(RAX, SA);
        self.get(RCX, SB);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let keep = self.jcc(0x8D);
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        self.here(keep);
        self.put(S, RAX);
        for (w, s) in [(A, SA), (B, SB)] {
            self.get(RAX, S);
            self.get(RCX, s);
            x86::sub_r64_r64(&mut self.code, RAX, RCX);
            self.put(K, RAX);
            self.w_mul10_times(w);
        }
    }

    /// Compara dos `dec`: rax = -1, 0 o 1. Nunca falla.
    pub fn h_cmp(&mut self) {
        self.enter();
        self.w_align();
        // si las dos son cero, iguales; si los signos difieren, gana el
        // positivo; si no, las magnitudes (al reves si son negativos)
        self.w_is_zero(A);
        let a_nonzero = self.jcc(0x85);
        self.w_is_zero(B);
        let b_nonzero = self.jcc(0x85);
        x86::zero_r32(&mut self.code, RAX);
        let out1 = self.jmp();
        self.here(a_nonzero);
        self.here(b_nonzero);
        self.get(RAX, SIGN_A);
        self.get(RCX, SIGN_B);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let same = self.jcc(0x84);
        // signos distintos: a negativo -> -1
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let a_neg = self.jcc(0x85);
        self.imm(RAX, 1);
        let out2 = self.jmp();
        self.here(a_neg);
        self.imm(RAX, -1);
        let out3 = self.jmp();
        self.here(same);
        self.w_cmp(A, B);
        self.get(RCX, SIGN_A);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let out4 = self.jcc(0x84);
        x86::neg_r64(&mut self.code, RAX);
        for f in [out1, out2, out3, out4] {
            self.here(f);
        }
        self.leave();
    }

    /// (rax, rcx) + (rdx, r8), o menos si r9 = 1: cifras y escala, r10.
    pub fn h_add(&mut self) {
        self.enter();
        self.put(FLAG, R9);
        self.w_align();
        // restar es sumar con el signo de b cambiado
        self.get(RAX, SIGN_B);
        self.get(RCX, FLAG);
        x86::xor_r64_r64(&mut self.code, RAX, RCX);
        self.put(SIGN_B, RAX);
        x86::zero_r32(&mut self.code, R10);
        self.get(RAX, SIGN_A);
        self.get(RCX, SIGN_B);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let differ = self.jcc(0x85);
        self.w_add(A, B);
        let result = self.jmp();
        self.here(differ);
        self.w_cmp(A, B);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let b_bigger = self.jcc(0x88);
        self.w_sub(A, B);
        let result2 = self.jmp();
        self.here(b_bigger);
        self.w_sub(B, A);
        self.w_copy(A, B);
        self.get(RAX, SIGN_B);
        self.put(SIGN_A, RAX);
        self.here(result);
        self.here(result2);
        let mut ovf = Vec::new();
        self.w_to_signed(A, SIGN_A, &mut ovf);
        self.get(RCX, S);
        self.leave();
        for f in ovf {
            self.here(f);
        }
        self.imm(R10, 1);
        self.leave();
    }

    /// (rax, rcx) * (rdx, r8): el producto ancho, las escalas sumadas y los
    /// ceros de mas fuera hasta la mayor de las dos.
    pub fn h_mul(&mut self) {
        self.enter();
        x86::zero_r32(&mut self.code, R10);
        self.put(SA, RCX);
        self.put(SB, R8);
        x86::mov_r64_r64(&mut self.code, R9, RDX);
        self.w_from_signed(A, SIGN_A, RAX);
        self.w_from_signed(B, SIGN_B, R9);
        // el signo y el producto de las magnitudes (cada una cabe en 64)
        self.get(RAX, SIGN_A);
        self.get(RCX, SIGN_B);
        x86::xor_r64_r64(&mut self.code, RAX, RCX);
        self.put(SIGN_A, RAX);
        self.get(RAX, A);
        self.get(RCX, B);
        x86::mul_r64(&mut self.code, RCX);
        self.put(A, RAX);
        self.put(A + 8, RDX);
        // escala = sa + sb; se guarda la mayor
        self.get(RAX, SA);
        self.get(RCX, SB);
        x86::mov_r64_r64(&mut self.code, RDX, RAX);
        x86::add_r64_r64(&mut self.code, RDX, RCX);
        self.put(S, RDX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let keep = self.jcc(0x8D);
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        self.here(keep);
        self.put(S0, RAX);
        let top = self.code.len();
        self.get(RAX, S);
        self.get(RCX, S0);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let trimmed = self.jcc(0x8E);
        self.w_copy(C, A);
        self.w_div10(C);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let trimmed2 = self.jcc(0x85);
        self.w_copy(A, C);
        self.get(RAX, S);
        x86::dec_r64(&mut self.code, RAX);
        self.put(S, RAX);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        self.here(trimmed);
        self.here(trimmed2);
        let mut ovf = Vec::new();
        self.get(RAX, S);
        x86::cmp_r64_imm8(&mut self.code, RAX, 18);
        ovf.push(self.jcc(0x8F));
        self.w_to_signed(A, SIGN_A, &mut ovf);
        self.get(RCX, S);
        self.leave();
        for f in ovf {
            self.here(f);
        }
        self.imm(R10, 1);
        self.leave();
    }

    /// (rax, rcx) / (rdx, r8), r9 = dentro de `round`: el decimal EXACTO
    /// con menos cifras (al menos las de los dos), por division larga -- la
    /// misma que el calculo (`calc/numero.rs`).
    pub fn h_div(&mut self) {
        self.enter();
        x86::zero_r32(&mut self.code, R10);
        self.put(FLAG, R9);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let divzero = self.jcc(0x84);
        self.put(SA, RCX);
        self.put(SB, R8);
        x86::mov_r64_r64(&mut self.code, R9, RDX);
        self.w_from_signed(A, SIGN_A, RAX);
        self.w_from_signed(B, SIGN_B, R9);
        self.get(RAX, SIGN_A);
        self.get(RCX, SIGN_B);
        x86::xor_r64_r64(&mut self.code, RAX, RCX);
        self.put(SIGN_A, RAX);
        // num = |da| * 10^sb, den = |db| * 10^sa
        self.get(RAX, SB);
        self.put(K, RAX);
        self.w_mul10_times(A);
        self.get(RAX, SA);
        self.put(K, RAX);
        self.w_mul10_times(B);
        // s0 = la mayor escala
        self.get(RAX, SA);
        self.get(RCX, SB);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let keep = self.jcc(0x8D);
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        self.here(keep);
        self.put(S0, RAX);
        // q (C), r (D)
        self.w_divmod(A, B, C, D);
        x86::zero_r32(&mut self.code, RAX);
        self.put(S, RAX);
        let mut ovf = Vec::new();
        let top = self.code.len();
        self.get(RAX, S);
        self.get(RCX, S0);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let step1 = self.jcc(0x8C);
        self.w_is_zero(D);
        let exact = self.jcc(0x84);
        self.get(RAX, S);
        x86::cmp_r64_imm8(&mut self.code, RAX, 18);
        let step2 = self.jcc(0x8C);
        // 18 decimales y no acaba
        self.get(RAX, FLAG);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let cut = self.jcc(0x85);
        self.imm(R10, 2);
        self.leave();
        self.here(step1);
        self.here(step2);
        // q ya no cabe en un dec: tampoco cabra despues
        self.get(RAX, C + 8);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        ovf.push(self.jcc(0x85));
        self.get(RAX, C);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        ovf.push(self.jcc(0x87));
        // s += 1; q = q*10 + (r*10) / den; r = (r*10) % den
        self.get(RAX, S);
        x86::inc_r64(&mut self.code, RAX);
        self.put(S, RAX);
        self.w_mul10(C);
        self.w_mul10(D);
        let sub_top = self.code.len();
        self.w_cmp(D, B);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let digit_done = self.jcc(0x88);
        self.w_sub(D, B);
        // q += 1
        self.get(RAX, C);
        self.imm(RCX, 1);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.put(C, RAX);
        x86::setcc_low(&mut self.code, 0x92, RDX);
        x86::movzx_r64_low(&mut self.code, RDX, RDX);
        self.get(RAX, C + 8);
        x86::add_r64_r64(&mut self.code, RAX, RDX);
        self.put(C + 8, RAX);
        let again = self.jmp();
        x86::patch_jump_to(&mut self.code, again, sub_top);
        self.here(digit_done);
        let back = self.jmp();
        x86::patch_jump_to(&mut self.code, back, top);
        // el resultado, con su signo
        self.here(exact);
        self.here(cut);
        self.w_to_signed(C, SIGN_A, &mut ovf);
        self.get(RCX, S);
        self.leave();
        self.here(divzero);
        self.imm(R10, 3);
        self.leave();
        for f in ovf {
            self.here(f);
        }
        self.imm(R10, 1);
        self.leave();
    }
}
