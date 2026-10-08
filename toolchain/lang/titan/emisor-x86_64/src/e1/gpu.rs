//! **LO DE LA GPU, AL CORRER** (LB4 de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 08-10) -- una gpu fn llamada por un programa que lee, y el f32 como DATO:
//!
//! ```text
//!    la llamada    la corre la CPU, su RESERVA: el x86-64 que escribio la
//!                  tarjeta de la CPU (`toolchain/forge/tarjeta-cpu`), juzgado
//!                  y comparado en el build con la casa y el calculo (L29,
//!                  L31, L32). Un hilo por celda, como en la 3060: con tablas,
//!                  una llamada por celda; con valores, una
//!    a f32         un int o un dec, al MAS CERCANO, como `parse::<f32>` de su
//!                  decimal exacto (`calc::fit_into`, D4): una subrutina de
//!                  enteros, sin coma flotante
//!    round(x, n)   el f32 escrito ENTERO en decimal y redondeado a n, la
//!                  mitad lejos del cero (`calc::round_f32`): otra subrutina
//!                  de enteros. Un infinito o un NaN es T0062; lo que no
//!                  cabe, T0060
//! ```
//!
//! ** E1 no cuenta en f32 (D2, la ley L26): guarda, copia y pasa BITS. Las
//! unicas instrucciones de coma flotante de un .bex son las del cuerpo de la
//! gpu fn, y las escribio su tarjeta.
//!
//! ** La llamada habla el ABI de la tarjeta de la CPU (su cabecera): rdi sus
//! registros, rsi sus entradas, rdx un puntero cualquiera, rcx su salida, r8
//! a cero. Conserva rbx, rbp, r12..r15 y rsp -- r14 (el monton) y r15 (la
//! pila que se gasta) siguen valiendo al volver -- y su juez lo comprobo en
//! cada celda del oraculo. Gasta unos 200 bytes de pila: caben en los 16 KiB
//! que E1 deja libres por encima de `STACK_FOR_CALLS`.

use super::{Helper, Place, E1};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::Value;
use bmo_titan_front::tree::Ty;

/// Lo que dice el NO de `round` con un infinito o un NaN (el calculo: T0062).
const NO_ES_UN_NUMERO: &str = "`round` recibe un infinito o un NaN de la gpu fn, y eso no es un numero: un f32 que dividio entre cero, o desbordo, no tiene decimal que lo diga";

impl E1<'_> {
    /// **Una llamada a una gpu fn, al correr.** Cada valor es una tabla de
    /// f32 o de si/no (un hilo por celda) o uno solo (el mismo en cada hilo);
    /// lo que vuelve es la tabla de resultados, o el resultado.
    pub fn gpu_call(&mut self, func: usize, args: &[Value], at: (usize, usize)) -> Result<Option<(Place, Class)>, String> {
        let f = &self.m.functions[func];
        let Some(cuerpo) = self.cuerpos.get(&func) else {
            return Err(format!("linea {}: `gpu fn {}` llega al correr sin el codigo de la CPU (fallo del compilador: avisa)", at.0, f.name));
        };
        let registros = cuerpo.registros.max(1) as i32;
        let ret = if f.ret == Some(Ty::Bool) { Class::Bool } else { Class::F32 };
        let name = f.name.clone();
        let mut bases = Vec::with_capacity(args.len() + 1);
        let mut celdas: Option<usize> = None;
        for a in args {
            let (p, c) = self.eval(a)?;
            match c {
                Class::Table(inner, n) if matches!(*inner, Class::F32 | Class::Bool) => {
                    celdas = Some(n);
                    bases.push((p, 8));
                }
                Class::F32 | Class::Bool => bases.push((p, 0)),
                other => return Err(format!("linea {}: un {:?} a `gpu fn {}`: el frontend tenia que haberlo dicho", at.0, other, name)),
            }
        }
        let n = celdas.unwrap_or(1);
        let out_class = match celdas {
            Some(n) => Class::Table(Box::new(ret), n),
            None => ret,
        };
        let out = self.temp(8 * n as i32);
        bases.push((out, 8));
        // Su sitio, en el marco de quien llama: sus registros, una entrada por
        // valor ([u32; 4]: la celda en el componente 0) y su salida.
        let regs = self.temp(4 * registros);
        let entradas = args.len().max(1);
        let ent = self.temp(16 * entradas as i32);
        let sal = self.temp(16);
        for k in 0..entradas {
            self.store_imm(ent.at(16 * k as i32 + 8), 0);
        }
        self.store_imm(sal, 0);
        self.store_imm(sal.at(8), 0);
        let valores = args.len();
        self.each_cell(n, &bases, &mut |e, celdas| {
            // la celda (sus bits; la mitad alta, a cero) a los componentes 0 y 1
            for (k, celda) in celdas.iter().take(valores).enumerate() {
                e.load(*celda, RAX);
                e.store(ent.at(16 * k as i32), RAX);
            }
            e.addr(regs, RDI);
            e.addr(ent, RSI);
            e.addr(ent, RDX);
            e.addr(sal, RCX);
            x86::zero_r32(&mut e.code, R8);
            e.code.push(0xE8);
            e.gpu_calls.push((e.code.len(), func));
            e.code.extend_from_slice(&[0; 4]);
            // la salida 0, componente 0: sus 32 bits, y la mitad alta a cero
            e.load(sal, RAX);
            x86::mov_r32_r32(&mut e.code, RAX, RAX);
            e.store(celdas[valores], RAX);
            Ok(())
        })?;
        Ok(Some((out, out_class)))
    }

    /// **Un int o un dec a f32**, al mas cercano (D4): lo de `calc::fit_into`.
    pub fn to_f32(&mut self, dst: Place, src: Place, from: &Class) {
        self.load_dec(src, from, RAX, RCX);
        self.call_helper(Helper::DecToF32);
        self.store(dst, RAX);
    }

    /// **`round(x, n)` de un f32**: la puerta de vuelta a `dec` (D2).
    pub fn round_f32(&mut self, p: Place, n: u32, at: (usize, usize)) -> Result<(Place, Class), String> {
        self.load(p, RAX);
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        x86::and_r64_imm32(&mut self.code, RCX, 0x7F80_0000);
        self.imm(RDX, 0x7F80_0000);
        x86::cmp_r64_r64(&mut self.code, RCX, RDX);
        self.trap(0x84, "T0062", NO_ES_UN_NUMERO, at);
        self.imm(RDX, n as i64);
        self.call_helper(Helper::F32Round);
        self.trap_status(at);
        let t = self.temp(16);
        self.store(t, RAX);
        self.store(t.at(8), RCX);
        Ok((t, Class::Dec))
    }

    // -- las subrutinas: solo enteros --------------------------------------

    /// `bsr <dst>, <src>` (64 bits): el bit mas alto de un numero que no es 0.
    fn bsr(&mut self, dst: u8, src: u8) {
        self.code.push(0x48 | if dst >= 8 { 0x04 } else { 0 } | if src >= 8 { 0x01 } else { 0 });
        self.code.extend_from_slice(&[0x0F, 0xBD, 0xC0 | ((dst & 7) << 3) | (src & 7)]);
    }

    /// `and <dst>, <src>` (64 bits).
    fn and(&mut self, dst: u8, src: u8) {
        self.code.push(0x48 | if src >= 8 { 0x04 } else { 0 } | if dst >= 8 { 0x01 } else { 0 });
        self.code.extend_from_slice(&[0x21, 0xC0 | ((src & 7) << 3) | (dst & 7)]);
    }

    /// Salta a `to` (ya escrito).
    fn back(&mut self, to: usize) {
        let j = self.jmp();
        x86::patch_jump_to(&mut self.code, j, to);
    }

    /// `rax` += 1 si `rax` es impar y `rcx` lo permite: el EMPATE al par.
    /// Deja el salto que sigue si NO hay que subir.
    fn odd_or_skip(&mut self) -> usize {
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        x86::and_r64_imm32(&mut self.code, RCX, 1);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        self.jcc(0x84)
    }

    /// **(rax, rcx) -> f32 en eax** -- las cifras y la escala de un dec (o un
    /// int, escala 0) al f32 MAS CERCANO, el empate al par: el `parse::<f32>`
    /// de su decimal exacto. Con |cifras| < 2^63 y escala <= 18 el valor esta
    /// entre 1e-18 y 2^63: un f32 normal siempre, sin subnormales ni
    /// infinitos. Ensucia rcx, rdx, r8..r11.
    ///
    /// ```text
    ///    a = |cifras|, q = 10^escala; e = el exponente de a/q (por sus bits
    ///    altos, y una comparacion); k = 23 - e
    ///    k >= 0   m = a*2^k / q: la parte entera y k pasos de division larga
    ///             (el resto siempre < q < 2^60: nada desborda)
    ///    k < 0    m = (a/q) >> -k, con lo que se cae y el resto mirados
    ///    m sube si lo que queda pasa de la mitad, o es la mitad y m es impar
    /// ```
    pub(crate) fn h_dec_a_f32(&mut self) {
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let nonzero = self.jcc(0x85);
        self.code.push(0xC3); // 0 -> +0.0
        self.here(nonzero);
        x86::zero_r32(&mut self.code, R11);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let pos = self.jcc(0x89);
        x86::neg_r64(&mut self.code, RAX);
        x86::mov_r32_imm32(&mut self.code, R11, 0x8000_0000);
        self.here(pos);
        x86::mov_r64_r64(&mut self.code, R8, RAX); // a
        // q = 10^escala
        self.imm(R9, 1);
        self.imm(R10, 10);
        let pow = self.code.len();
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let pow_done = self.jcc(0x84);
        x86::imul_r64_r64(&mut self.code, R9, R10);
        x86::dec_r64(&mut self.code, RCX);
        self.back(pow);
        self.here(pow_done);
        // e = bsr(a) - bsr(q), y uno menos si a < q * 2^e
        self.bsr(R10, R8);
        self.bsr(RCX, R9);
        x86::sub_r64_r64(&mut self.code, R10, RCX);
        x86::test_r64_r64(&mut self.code, R10, R10);
        let e_neg = self.jcc(0x88);
        x86::mov_r64_r64(&mut self.code, RCX, R10);
        x86::mov_r64_r64(&mut self.code, RDX, R9);
        x86::shl_r64_cl(&mut self.code, RDX);
        x86::cmp_r64_r64(&mut self.code, R8, RDX);
        let e_ok1 = self.jcc(0x83);
        x86::dec_r64(&mut self.code, R10);
        let e_ok2 = self.jmp();
        self.here(e_neg);
        x86::mov_r64_r64(&mut self.code, RCX, R10);
        x86::neg_r64(&mut self.code, RCX);
        x86::mov_r64_r64(&mut self.code, RDX, R8);
        x86::shl_r64_cl(&mut self.code, RDX);
        x86::cmp_r64_r64(&mut self.code, RDX, R9);
        let e_ok3 = self.jcc(0x83);
        x86::dec_r64(&mut self.code, R10);
        for j in [e_ok1, e_ok2, e_ok3] {
            self.here(j);
        }
        // k = 23 - e
        self.imm(RCX, 23);
        x86::sub_r64_r64(&mut self.code, RCX, R10);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let k_neg = self.jcc(0x88);
        // k >= 0: la parte entera, y k pasos de division larga
        x86::mov_r64_r64(&mut self.code, RAX, R8);
        x86::zero_r32(&mut self.code, RDX);
        x86::div_r64(&mut self.code, R9);
        let step = self.code.len();
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let steps_done = self.jcc(0x84);
        x86::add_r64_r64(&mut self.code, RDX, RDX);
        x86::add_r64_r64(&mut self.code, RAX, RAX);
        x86::cmp_r64_r64(&mut self.code, RDX, R9);
        let no_bit = self.jcc(0x82);
        x86::sub_r64_r64(&mut self.code, RDX, R9);
        x86::inc_r64(&mut self.code, RAX);
        self.here(no_bit);
        x86::dec_r64(&mut self.code, RCX);
        self.back(step);
        self.here(steps_done);
        // el resto: 2r contra q
        x86::add_r64_r64(&mut self.code, RDX, RDX);
        x86::cmp_r64_r64(&mut self.code, RDX, R9);
        let up1 = self.jcc(0x87);
        let keep1 = self.jcc(0x82);
        let keep2 = self.odd_or_skip();
        self.here(up1);
        x86::inc_r64(&mut self.code, RAX);
        self.here(keep1);
        self.here(keep2);
        let rounded1 = self.jmp();
        // k < 0: (a / q) >> j, con lo que se cae y el resto
        self.here(k_neg);
        x86::neg_r64(&mut self.code, RCX);
        x86::mov_r64_r64(&mut self.code, RAX, R8);
        x86::zero_r32(&mut self.code, RDX);
        x86::div_r64(&mut self.code, R9);
        x86::mov_r64_r64(&mut self.code, R8, RAX);
        x86::shr_r64_cl(&mut self.code, RAX);
        self.imm(R9, 1);
        x86::shl_r64_cl(&mut self.code, R9);
        x86::dec_r64(&mut self.code, R9);
        self.and(R8, R9);
        x86::inc_r64(&mut self.code, R9);
        x86::shr_r64_imm8(&mut self.code, R9, 1);
        x86::cmp_r64_r64(&mut self.code, R8, R9);
        let up2 = self.jcc(0x87);
        let keep3 = self.jcc(0x82);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let up3 = self.jcc(0x85);
        let keep4 = self.odd_or_skip();
        self.here(up2);
        self.here(up3);
        x86::inc_r64(&mut self.code, RAX);
        self.here(keep3);
        self.here(keep4);
        self.here(rounded1);
        // 2^24 al subir: 2^23 y un exponente mas
        self.imm(RCX, 1 << 24);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let normal = self.jcc(0x85);
        x86::shr_r64_imm8(&mut self.code, RAX, 1);
        x86::inc_r64(&mut self.code, R10);
        self.here(normal);
        x86::and_r64_imm32(&mut self.code, RAX, 0x7F_FFFF);
        x86::add_r64_imm8(&mut self.code, R10, 127);
        x86::shl_r64_imm8(&mut self.code, R10, 23);
        x86::or_r64_r64(&mut self.code, RAX, R10);
        x86::or_r64_r64(&mut self.code, RAX, R11);
        self.code.push(0xC3);
    }

    /// **round(f32 en eax, rdx decimales) -> (rax, rcx)**: el f32 = m * 2^e,
    /// exacto; m * 10^n en 128 bits (< 2^84) y desplazado e: si e < 0, el
    /// ultimo bit que se cae dice si sube (la mitad, o mas: lejos del cero).
    /// r10 = 1 si no cabe en un dec (T0060: mas de 2^63, o 2^63 positivo), o
    /// si n > 18 (la escala de un dec: `calc::SCALE`). El infinito y el NaN
    /// los mira quien llama. Ensucia rdx, r8, r9, r11.
    pub(crate) fn h_f32_round(&mut self) {
        x86::mov_r64_r64(&mut self.code, R9, RDX); // n, para devolverlo en rcx
        x86::zero_r32(&mut self.code, R10);
        x86::cmp_r64_imm8(&mut self.code, R9, 18);
        let ovf1 = self.jcc(0x8F);
        x86::mov_r32_r32(&mut self.code, RAX, RAX);
        x86::mov_r64_r64(&mut self.code, R11, RAX);
        x86::shr_r64_imm8(&mut self.code, R11, 31); // el signo
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        x86::shr_r64_imm8(&mut self.code, RCX, 23);
        x86::and_r64_imm32(&mut self.code, RCX, 0xFF); // el exponente
        x86::and_r64_imm32(&mut self.code, RAX, 0x7F_FFFF);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let subnormal = self.jcc(0x84);
        self.imm(RDX, 0x80_0000);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        x86::mov_r64_r64(&mut self.code, R8, RCX);
        self.imm(RDX, 150);
        x86::sub_r64_r64(&mut self.code, R8, RDX); // e = exponente - 150
        let have = self.jmp();
        self.here(subnormal);
        self.imm(R8, -149);
        self.here(have);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let zero1 = self.jcc(0x84);
        // N = m * 10^n, en rdx:rax
        self.imm(RCX, 1);
        x86::mov_r64_r64(&mut self.code, RDX, R9);
        self.imm(R10, 10);
        let pow = self.code.len();
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let pow_done = self.jcc(0x84);
        x86::imul_r64_r64(&mut self.code, RCX, R10);
        x86::dec_r64(&mut self.code, RDX);
        self.back(pow);
        self.here(pow_done);
        x86::zero_r32(&mut self.code, R10);
        x86::mul_r64(&mut self.code, RCX);
        x86::test_r64_r64(&mut self.code, R8, R8);
        let e_neg = self.jcc(0x88);
        // e >= 0: N << e, si no pasa de 2^63 (sin perder un bit)
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let ovf2 = self.jcc(0x85);
        x86::cmp_r64_imm8(&mut self.code, R8, 64);
        let ovf3 = self.jcc(0x8D);
        self.imm(RCX, 63);
        x86::sub_r64_r64(&mut self.code, RCX, R8);
        x86::mov_r64_r64(&mut self.code, RDX, RAX);
        x86::shr_r64_cl(&mut self.code, RDX);
        x86::cmp_r64_imm8(&mut self.code, RDX, 1);
        let ovf4 = self.jcc(0x87);
        x86::mov_r64_r64(&mut self.code, RCX, R8);
        x86::shl_r64_cl(&mut self.code, RAX);
        let fits1 = self.jmp();
        // e < 0: t = N >> (k - 1); sube el bit bajo de t; q = t >> 1
        self.here(e_neg);
        x86::mov_r64_r64(&mut self.code, RCX, R8);
        x86::neg_r64(&mut self.code, RCX);
        x86::dec_r64(&mut self.code, RCX);
        x86::cmp_r64_imm8(&mut self.code, RCX, 127);
        let zero2 = self.jcc(0x8F);
        x86::cmp_r64_imm8(&mut self.code, RCX, 64);
        let big = self.jcc(0x8D);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let t_ready1 = self.jcc(0x84);
        x86::mov_r64_r64(&mut self.code, R8, RDX);
        x86::shr_r64_cl(&mut self.code, RAX);
        x86::shr_r64_cl(&mut self.code, RDX);
        x86::neg_r64(&mut self.code, RCX);
        x86::add_r64_imm8(&mut self.code, RCX, 64);
        x86::shl_r64_cl(&mut self.code, R8);
        x86::or_r64_r64(&mut self.code, RAX, R8);
        let t_ready2 = self.jmp();
        self.here(big);
        x86::sub_r64_imm8(&mut self.code, RCX, 64);
        x86::mov_r64_r64(&mut self.code, RAX, RDX);
        x86::shr_r64_cl(&mut self.code, RAX);
        x86::zero_r32(&mut self.code, RDX);
        self.here(t_ready1);
        self.here(t_ready2);
        x86::mov_r64_r64(&mut self.code, R8, RAX);
        x86::and_r64_imm32(&mut self.code, R8, 1); // sube?
        x86::shr_r64_imm8(&mut self.code, RAX, 1);
        x86::mov_r64_r64(&mut self.code, RCX, RDX);
        x86::shl_r64_imm8(&mut self.code, RCX, 63);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        x86::shr_r64_imm8(&mut self.code, RDX, 1);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let ovf5 = self.jcc(0x85);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let ovf6 = self.jcc(0x87);
        x86::add_r64_r64(&mut self.code, RAX, R8);
        // |r| <= 2^63, y 2^63 justo solo si es negativo: i64::MIN cabe en un
        // dec, su opuesto no (`calc::dec_result`)
        self.here(fits1);
        self.imm(RCX, i64::MIN);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let ovf7 = self.jcc(0x87);
        let signed1 = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, R11, R11);
        let ovf8 = self.jcc(0x84);
        self.here(signed1);
        x86::test_r64_r64(&mut self.code, R11, R11);
        let done1 = self.jcc(0x84);
        x86::neg_r64(&mut self.code, RAX);
        let done2 = self.jmp();
        self.here(zero1);
        self.here(zero2);
        x86::zero_r32(&mut self.code, RAX);
        self.here(done1);
        self.here(done2);
        x86::mov_r64_r64(&mut self.code, RCX, R9);
        self.code.push(0xC3);
        for j in [ovf1, ovf2, ovf3, ovf4, ovf5, ovf6, ovf7, ovf8] {
            self.here(j);
        }
        x86::mov_r64_r64(&mut self.code, RCX, R9);
        self.imm(R10, 1);
        self.code.push(0xC3);
    }
}

/// ** Las dos subrutinas, CORRIDAS en el emulador contra la regla del calculo
/// (copiada aqui: `calc::fit_into` y `calc::round_f32` son del frontend y no
/// se exportan): el decimal exacto parseado a f32, y el f32 escrito con 150
/// decimales y cortado. `E1_F32=semilla,casos` para buscar mas lejos.
#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_lower::emu::{run, Machine};

    fn show_dec(d: i64, s: u32) -> String {
        if s == 0 {
            return d.to_string();
        }
        let a = (d as i128).abs().to_string();
        let a = if a.len() <= s as usize { format!("{}{}", "0".repeat(s as usize + 1 - a.len()), a) } else { a };
        let (w, f) = a.split_at(a.len() - s as usize);
        format!("{}{}.{}", if d < 0 { "-" } else { "" }, w, f)
    }

    fn calc_round(v: f32, n: u32) -> Option<i64> {
        let s = format!("{:.150}", (v as f64).abs());
        let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
        let keep = &frac[..(n as usize).min(frac.len())];
        let up = frac.as_bytes().get(n as usize).is_some_and(|d| *d >= b'5');
        let digits: i128 = format!("{}{}", whole, keep).parse::<i128>().ok()?;
        let r = (digits + up as i128) * if v < 0.0 { -1 } else { 1 };
        (n <= 18).then_some(())?;
        i64::try_from(r).ok()
    }

    /// Las dos, escritas por E1, detras de una llamada: `call` y un salto al
    /// final (el emulador para al caerse del codigo).
    fn subrutinas() -> (Vec<u8>, usize, usize) {
        let m = bmo_titan_front::lower("mod main \"x\"\nfn main()\n    print(1)\n").unwrap();
        let cuerpos = crate::Cuerpos::new();
        let mut e = E1 {
            m: &m,
            forms: super::super::forma::Forms::new(&m),
            code: Vec::new(),
            calls: Vec::new(),
            traps: Vec::new(),
            status_traps: Vec::new(),
            helper_calls: Vec::new(),
            depth_checks: Vec::new(),
            f: super::super::Fun { known: Vec::new(), decl: Vec::new(), slot: Vec::new(), indirect: Vec::new(), locals_end: 16, temp: 0, temp_max: 0 },
            lenient: 0,
            owned: Vec::new(),
            kinds: Vec::new(),
            not_yet_said: None,
            cuerpos: &cuerpos,
            gpu_calls: Vec::new(),
        };
        let a = e.code.len();
        e.h_dec_a_f32();
        let b = e.code.len();
        e.h_f32_round();
        (e.code, a, b)
    }

    /// `call` a la subrutina en `at` con estos registros; vuelve rax, rcx, r10.
    fn llama(code: &[u8], at: usize, rax: u64, rcx: u64, rdx: u64) -> (u64, u64, u64) {
        let mut c = vec![0xE8];
        c.extend_from_slice(&((10 + at) as i32 - 5).to_le_bytes());
        c.push(0xE9);
        c.extend_from_slice(&(code.len() as u32).to_le_bytes());
        c.extend_from_slice(code);
        let mut m = Machine::new(c);
        m.regs[0] = rax;
        m.regs[1] = rcx;
        m.regs[2] = rdx;
        let m = run(m, 100_000);
        (m.regs[0], m.regs[1], m.regs[10])
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn azar() -> (u64, u64) {
        let env = std::env::var("E1_F32").unwrap_or_default();
        let mut it = env.split(',').map(|x| x.trim().parse::<u64>().ok());
        let seed = it.next().flatten().filter(|s| *s != 0).unwrap_or(0xD1B5_4A32_D192_ED03);
        (seed, it.next().flatten().unwrap_or(4000))
    }

    #[test]
    fn un_dec_a_f32_es_el_mas_cercano_como_el_calculo() {
        let (code, a, _) = subrutinas();
        let (seed, casos) = azar();
        let mut r = Rng(seed);
        let mut bordes: Vec<i64> = vec![0, 1, -1, 5, 15, 25, 16777215, 16777216, 16777217, 16777219, 33554433, i64::MAX, i64::MIN, i64::MIN + 1, 1 << 53, 999999999999999999];
        bordes.extend((0..casos).map(|_| {
            let bits = r.next() % 64;
            (r.next() >> (63 - bits)) as i64 * if r.next() & 1 == 0 { 1 } else { -1 }
        }));
        let mut vistos = 0;
        for (k, &d) in bordes.iter().enumerate() {
            for s in [0u32, 1, 2, 3, 6, 9, 12, 17, 18].into_iter().filter(|s| k < 16 || *s as u64 == r.next() % 19 || *s == 2) {
                let calc = show_dec(d, s).parse::<f32>().unwrap().to_bits();
                let (bits, _, _) = llama(&code, a, d as u64, s as u64, 0);
                assert_eq!(bits as u32, calc, "dec ({d}, {s}): E1 {bits:#x}, el calculo {calc:#x}");
                assert_eq!(bits >> 32, 0, "la mitad alta, a cero");
                vistos += 1;
            }
        }
        assert!(vistos > casos as usize);
    }

    #[test]
    fn round_de_un_f32_es_el_del_calculo() {
        let (code, _, b) = subrutinas();
        let (seed, casos) = azar();
        let mut r = Rng(seed ^ 0xFF);
        let mut bordes: Vec<u32> = vec![0, 0x8000_0000, 1, 0x8000_0001, 0x007F_FFFF, 0x0080_0000, 0x3F00_0000, 0xBF00_0000, 0x3F80_0000, 0x3E99_999A, 0x7F7F_FFFF, 0xFF7F_FFFF, 0x5F00_0000, 0xDF00_0000, 0x5EFF_FFFF, 0xDEFF_FFFF, 0x4B00_0000];
        bordes.extend((0..casos).map(|_| r.next() as u32 & !0x4000_0000 | (r.next() as u32 & 0x4000_0000)).filter(|b| b & 0x7F80_0000 != 0x7F80_0000));
        for (k, &f) in bordes.iter().enumerate() {
            for n in (0u32..=20).filter(|n| k < 17 || r.next() % 7 == 0 || *n == 2) {
                let calc = calc_round(f32::from_bits(f), n);
                let (rax, rcx, r10) = llama(&code, b, f as u64, 0, n as u64);
                let e1 = (r10 == 0).then_some(rax as i64);
                assert_eq!(e1, calc, "round({:e} [{f:#x}], {n})", f32::from_bits(f));
                assert_eq!(rcx, n as u64, "la escala vuelve en rcx");
            }
        }
    }
}
