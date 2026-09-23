//! **S4b: las TRASCENDENTES en x86-64** -- seno, coseno, exp, log y pow.
//!
//! No existen en SSE. Se emiten como TRES rutinas en doble precision al
//! principio del codigo (solo si el modulo las usa), a las que se llama:
//!
//! ```text
//!   sincos   xmm0 = x (f64)  ->  xmm0 = sin(x), xmm1 = cos(x)
//!   exp      xmm0 = x (f64)  ->  xmm0 = e^x
//!   ln       xmm0 = x (f64)  ->  xmm0 = ln(x)
//! ```
//!
//! ** Son `bmo_spirv_front::math` operacion por operacion: las constantes y
//! los coeficientes se leen de `math::table` (la MISMA tabla que usa el
//! oraculo) y los polinomios se evaluan en el mismo orden -- Horner de dentro
//! a fuera, `acc = acc * x + c`, sin fusionar. Cada `mulsd`/`addsd` aqui es
//! un `*`/`+` de alli. Por eso dan los mismos bits, y la prueba diferencial lo
//! comprueba.
//!
//! Los detalles que hay que repetir TAL CUAL, porque cambian bits:
//!
//! - `nearest(y)`: `(y + 1.5*2^52) - 1.5*2^52` si y >= 0, y al reves si no.
//! - el cuadrante `(k as i64).rem_euclid(4)`: Rust SATURA `k as i64` y
//!   `cvttsd2si` no -- con k >= 2^63 Rust da `i64::MAX` (cuadrante 3) y el
//!   silicio `i64::MIN` (cuadrante 0). Se corrige a mano.
//! - `medio = k / 2` de Rust trunca hacia cero (no es `sar`).
//! - los NaN que devuelve `math` son el CONSTANTE `f64::NAN` salvo `exp`, que
//!   devuelve el suyo: se copia eso.
//! - `ln` recibe siempre un `f32` ampliado, que nunca es subnormal en doble:
//!   la rama de subnormales de `math::ln_f64` no hace falta aqui.
//!
//! Registros: xmm0..xmm7, rax, rcx, rdx. Nadie mas vive en registros entre
//! instrucciones (ver `emit/mod.rs`), asi que no hay nada que guardar.
//!
//! [consumo]  NADA   emite y se va

use super::Emitter;
use crate::asm::{cc, Alu, RAX, RCX, RDX};
use bmo_spirv_front::math::table as t;

const SIGNO: u64 = 0x8000_0000_0000_0000;

impl<'m, 'a, 'b, 't, 'c> Emitter<'m, 'a, 'b, 't, 'c> {
    /// `xmm[x] = v` (por `rax`).
    fn f64c(&mut self, x: u8, v: f64) {
        self.w.mov_imm64(RAX, v.to_bits());
        self.w.movd_to_xmm(true, x, RAX);
    }

    /// `xmm[acc] = Horner(tabla, xmm[x])`, con xmm7 de paso.
    fn horner(&mut self, acc: u8, x: u8, tabla: &[f64]) {
        self.f64c(acc, tabla[0]);
        for &c in &tabla[1..] {
            self.w.sd(0x59, acc, x);
            self.f64c(7, c);
            self.w.sd(0x58, acc, 7);
        }
    }

    /// `xmm[y] = nearest(xmm[y])`. Usa xmm7.
    fn nearest(&mut self, y: u8) {
        self.w.xorpd(7, 7);
        self.w.comisd(y, 7);
        let negativo = self.w.jcc_fwd(cc::B);
        self.f64c(7, t::NEAREST);
        self.w.sd(0x58, y, 7);
        self.w.sd(0x5C, y, 7);
        let fin = self.w.jmp_fwd();
        self.w.here(negativo);
        self.f64c(7, t::NEAREST);
        self.w.sd(0x5C, y, 7);
        self.w.sd(0x58, y, 7);
        self.w.here(fin);
    }

    /// Voltea el signo de `xmm[x]` (el `-s` de Rust). Usa xmm7.
    fn neg64(&mut self, x: u8) {
        self.w.mov_imm64(RAX, SIGNO);
        self.w.movd_to_xmm(true, 7, RAX);
        self.w.xorpd(x, 7);
    }

    /// **Emite las tres rutinas.** Devuelve `(sincos, exp, ln)`.
    pub(super) fn transcendental_routines(&mut self) -> (usize, usize, usize) {
        let sincos = self.w.pos;
        self.routine_sincos();
        let exp = self.w.pos;
        self.routine_exp();
        let ln = self.w.pos;
        self.routine_ln();
        (sincos, exp, ln)
    }

    fn routine_sincos(&mut self) {
        // !x.is_finite() -> (NAN, NAN)
        self.w.movd_from_xmm(true, RAX, 0);
        self.w.mov(true, RDX, RAX);
        self.w.shift_imm(true, 5, RDX, 52);
        self.w.alu_imm(false, Alu::And, RDX, 0x7FF);
        self.w.alu_imm(false, Alu::Cmp, RDX, 0x7FF);
        let finito = self.w.jcc_fwd(cc::NE);
        self.f64c(0, f64::NAN);
        self.f64c(1, f64::NAN);
        self.w.ret();
        self.w.here(finito);
        // k = nearest(x * FRAC_2_PI)
        self.w.sd(0x10, 2, 0);
        self.f64c(7, t::FRAC_2_PI);
        self.w.sd(0x59, 2, 7);
        self.nearest(2);
        // r = (x - k*HI) - k*LO
        self.w.sd(0x10, 3, 2);
        self.f64c(7, t::FRAC_PI_2_HI);
        self.w.sd(0x59, 3, 7);
        self.w.sd(0x10, 4, 0);
        self.w.sd(0x5C, 4, 3);
        self.w.sd(0x10, 3, 2);
        self.f64c(7, t::FRAC_PI_2_LO);
        self.w.sd(0x59, 3, 7);
        self.w.sd(0x5C, 4, 3);
        // El cuadrante, con la saturacion de Rust.
        self.w.cvttsd2si(RCX, 2);
        self.f64c(7, 9_223_372_036_854_775_808.0);
        self.w.comisd(2, 7);
        let cabe = self.w.jcc_fwd(cc::B);
        self.w.mov_imm(RCX, 3);
        self.w.here(cabe);
        self.w.alu_imm(false, Alu::And, RCX, 3);
        // r2, s = r * P_sin(r2), c = P_cos(r2)
        self.w.sd(0x10, 5, 4);
        self.w.sd(0x59, 5, 4);
        self.horner(6, 5, &t::SIN);
        self.w.sd(0x10, 0, 4);
        self.w.sd(0x59, 0, 6);
        self.horner(1, 5, &t::COS);
        // 0: (s, c)   1: (c, -s)   2: (-s, -c)   3: (-c, s)
        self.w.alu_imm(false, Alu::Cmp, RCX, 0);
        let q0 = self.w.jcc_fwd(cc::E);
        self.w.alu_imm(false, Alu::Cmp, RCX, 1);
        let q1 = self.w.jcc_fwd(cc::E);
        self.w.alu_imm(false, Alu::Cmp, RCX, 2);
        let q2 = self.w.jcc_fwd(cc::E);
        // 3
        self.w.sd(0x10, 2, 0);
        self.w.sd(0x10, 0, 1);
        self.neg64(0);
        self.w.sd(0x10, 1, 2);
        self.w.ret();
        self.w.here(q1);
        self.w.sd(0x10, 2, 0);
        self.w.sd(0x10, 0, 1);
        self.w.sd(0x10, 1, 2);
        self.neg64(1);
        self.w.ret();
        self.w.here(q2);
        self.neg64(0);
        self.neg64(1);
        self.w.here(q0);
        self.w.ret();
    }

    fn routine_exp(&mut self) {
        // NaN -> el suyo; > EXP_MAX -> inf; < EXP_MIN -> 0.
        self.w.comisd(0, 0);
        let nan = self.w.jcc_fwd(cc::P);
        self.f64c(7, t::EXP_MAX);
        self.w.comisd(0, 7);
        let inf = self.w.jcc_fwd(cc::A);
        self.f64c(7, t::EXP_MIN);
        self.w.comisd(0, 7);
        let cero = self.w.jcc_fwd(cc::B);
        // k = nearest(x * INV_LN2); r = (x - k*LN2_HI) - k*LN2_LO
        self.w.sd(0x10, 2, 0);
        self.f64c(7, t::INV_LN2);
        self.w.sd(0x59, 2, 7);
        self.nearest(2);
        self.w.sd(0x10, 3, 2);
        self.f64c(7, t::LN2_HI);
        self.w.sd(0x59, 3, 7);
        self.w.sd(0x10, 4, 0);
        self.w.sd(0x5C, 4, 3);
        self.w.sd(0x10, 3, 2);
        self.f64c(7, t::LN2_LO);
        self.w.sd(0x59, 3, 7);
        self.w.sd(0x5C, 4, 3);
        // p = P_exp(r)
        self.horner(6, 4, &t::EXP);
        // k entero; medio = k / 2 truncando hacia cero; p * 2^medio * 2^(k - medio)
        self.w.cvttsd2si(RCX, 2);
        self.w.mov(true, RDX, RCX);
        self.w.shift_imm(true, 5, RDX, 63);
        self.w.alu(true, Alu::Add, RDX, RCX);
        self.w.shift_imm(true, 7, RDX, 1);
        self.pow2_mul(RDX);
        self.w.alu(true, Alu::Sub, RCX, RDX);
        self.pow2_mul(RCX);
        self.w.sd(0x10, 0, 6);
        self.w.here(nan);
        self.w.ret();
        self.w.here(inf);
        self.f64c(0, f64::INFINITY);
        self.w.ret();
        self.w.here(cero);
        self.w.xorpd(0, 0);
        self.w.ret();
    }

    /// `xmm6 *= 2^r` (r entero en un registro que no es rax). Usa rax, xmm7.
    fn pow2_mul(&mut self, r: u8) {
        self.w.mov(true, RAX, r);
        self.w.alu_imm(true, Alu::Add, RAX, 1023);
        self.w.shift_imm(true, 4, RAX, 52);
        self.w.movd_to_xmm(true, 7, RAX);
        self.w.sd(0x59, 6, 7);
    }

    fn routine_ln(&mut self) {
        // NaN o < 0 -> NAN; 0 -> -inf; inf -> inf
        self.w.comisd(0, 0);
        let nan1 = self.w.jcc_fwd(cc::P);
        self.w.xorpd(7, 7);
        self.w.comisd(0, 7);
        let nan2 = self.w.jcc_fwd(cc::B);
        let cero = self.w.jcc_fwd(cc::E);
        self.w.movd_from_xmm(true, RAX, 0);
        self.w.mov(true, RDX, RAX);
        self.w.shift_imm(true, 5, RDX, 52);
        self.w.alu_imm(false, Alu::Cmp, RDX, 0x7FF);
        let inf = self.w.jcc_fwd(cc::E);
        // e = exponente - 1023; m = mantisa con exponente 0 (en [1, 2))
        self.w.alu_imm(true, Alu::Sub, RDX, 1023);
        self.w.mov_imm64(RCX, 0x000F_FFFF_FFFF_FFFF);
        self.w.alu(true, Alu::And, RAX, RCX);
        self.w.mov_imm64(RCX, 0x3FF0_0000_0000_0000);
        self.w.alu(true, Alu::Or, RAX, RCX);
        self.w.movd_to_xmm(true, 1, RAX);
        // m > SQRT_2 -> m *= 0.5, e += 1
        self.f64c(7, core::f64::consts::SQRT_2);
        self.w.comisd(1, 7);
        let no = self.w.jcc_fwd(cc::BE);
        self.f64c(7, 0.5);
        self.w.sd(0x59, 1, 7);
        self.w.alu_imm(true, Alu::Add, RDX, 1);
        self.w.here(no);
        // s = (m - 1) / (m + 1); p = P_ln(s^2)
        self.f64c(7, 1.0);
        self.w.sd(0x10, 2, 1);
        self.w.sd(0x5C, 2, 7);
        self.w.sd(0x10, 3, 1);
        self.w.sd(0x58, 3, 7);
        self.w.sd(0x5E, 2, 3);
        self.w.sd(0x10, 3, 2);
        self.w.sd(0x59, 3, 2);
        self.horner(4, 3, &t::LN);
        // (2*s*p + e*LN2_LO) + e*LN2_HI
        self.w.cvtsi2sd(5, RDX);
        self.f64c(0, 2.0);
        self.w.sd(0x59, 0, 2);
        self.w.sd(0x59, 0, 4);
        self.w.sd(0x10, 6, 5);
        self.f64c(7, t::LN2_LO);
        self.w.sd(0x59, 6, 7);
        self.w.sd(0x58, 0, 6);
        self.w.sd(0x10, 6, 5);
        self.f64c(7, t::LN2_HI);
        self.w.sd(0x59, 6, 7);
        self.w.sd(0x58, 0, 6);
        self.w.ret();
        self.w.here(nan1);
        self.w.here(nan2);
        self.f64c(0, f64::NAN);
        self.w.ret();
        self.w.here(cero);
        self.f64c(0, f64::NEG_INFINITY);
        self.w.here(inf);
        self.w.ret();
    }
}
