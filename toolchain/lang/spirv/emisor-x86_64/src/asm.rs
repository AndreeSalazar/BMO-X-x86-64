//! **El codificador**: los bytes de x86-64 que el emisor escribe, y ni uno mas.
//!
//! Cada forma de aqui la sabe EJECUTAR el emulador de `bmo-lower`, que es el
//! juez de S4: una instruccion que el emulador no conoce le hace parar con su
//! codigo, que es exactamente el aviso que se quiere.
//!
//! == Dos pasadas, sin memoria ==
//!
//! `Writer` escribe en el buffer que da quien llama, o no escribe nada y solo
//! CUENTA (primera pasada). Todos los saltos son de 32 bits, asi que la
//! medida de cada instruccion no depende de a donde salte: la primera pasada
//! fija donde cae cada etiqueta y la segunda escribe los mismos bytes con los
//! destinos buenos. Sin lista de parches, sin `alloc`.
//!
//! La memoria siempre es `[base + disp32]` (mod = 10) y la base nunca es
//! `rsp` ni `r12`, que pedirian SIB.
//!
//! [consumo]  NADA   escribe bytes mientras se emite

pub const RAX: u8 = 0;
pub const RCX: u8 = 1;
pub const RDX: u8 = 2;
pub const RSP: u8 = 4;
pub const RSI: u8 = 6;
pub const RDI: u8 = 7;
pub const R8: u8 = 8;
pub const R9: u8 = 9;
pub const R10: u8 = 10;
pub const R11: u8 = 11;
pub const R14: u8 = 14;
pub const R15: u8 = 15;

/// Codigos de condicion (el nibble de `jcc`/`setcc`).
pub mod cc {
    pub const B: u8 = 0x2; // CF
    pub const AE: u8 = 0x3;
    pub const E: u8 = 0x4;
    pub const NE: u8 = 0x5;
    pub const BE: u8 = 0x6;
    pub const A: u8 = 0x7;
    pub const S: u8 = 0x8;
    pub const P: u8 = 0xA;
    pub const NP: u8 = 0xB;
    pub const L: u8 = 0xC;
    pub const GE: u8 = 0xD;
    pub const LE: u8 = 0xE;
    pub const G: u8 = 0xF;
}

/// Operaciones aritmeticas de la familia `op r/m, reg` y `81 /ext`.
#[derive(Clone, Copy)]
pub enum Alu {
    Add,
    Or,
    And,
    Sub,
    Xor,
    Cmp,
}

impl Alu {
    fn opcode(self) -> u8 {
        match self {
            Alu::Add => 0x01,
            Alu::Or => 0x09,
            Alu::And => 0x21,
            Alu::Sub => 0x29,
            Alu::Xor => 0x31,
            Alu::Cmp => 0x39,
        }
    }
    fn ext(self) -> u8 {
        match self {
            Alu::Add => 0,
            Alu::Or => 1,
            Alu::And => 4,
            Alu::Sub => 5,
            Alu::Xor => 6,
            Alu::Cmp => 7,
        }
    }
}

pub struct Writer<'c> {
    buf: Option<&'c mut [u8]>,
    pub pos: usize,
}

impl<'c> Writer<'c> {
    pub fn counting() -> Self {
        Writer { buf: None, pos: 0 }
    }

    pub fn writing(buf: &'c mut [u8]) -> Self {
        Writer { buf: Some(buf), pos: 0 }
    }

    pub fn byte(&mut self, b: u8) {
        if let Some(buf) = self.buf.as_deref_mut() {
            if let Some(x) = buf.get_mut(self.pos) {
                *x = b;
            }
        }
        self.pos += 1;
    }

    fn bytes(&mut self, bs: &[u8]) {
        for &b in bs {
            self.byte(b);
        }
    }

    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    fn put_u32_at(&mut self, at: usize, v: u32) {
        if let Some(buf) = self.buf.as_deref_mut() {
            if let Some(dst) = buf.get_mut(at..at + 4) {
                dst.copy_from_slice(&v.to_le_bytes());
            }
        }
    }

    /// REX si hace falta (W, o algun registro de 8 a 15).
    fn rex(&mut self, w: bool, reg: u8, rm: u8) {
        let r = (w as u8) << 3 | ((reg >> 3) & 1) << 2 | ((rm >> 3) & 1);
        if r != 0 {
            self.byte(0x40 | r);
        }
    }

    fn modrm_reg(&mut self, reg: u8, rm: u8) {
        self.byte(0xC0 | (reg & 7) << 3 | (rm & 7));
    }

    fn modrm_mem(&mut self, reg: u8, base: u8, disp: i32) {
        debug_assert!(base & 7 != 4, "rsp/r12 como base pedirian SIB");
        self.byte(0x80 | (reg & 7) << 3 | (base & 7));
        self.u32(disp as u32);
    }

    // ---- movimientos enteros -------------------------------------------------

    /// `mov r32, [base+disp]` (o r64 con `w`).
    pub fn load(&mut self, w: bool, dst: u8, base: u8, disp: i32) {
        self.rex(w, dst, base);
        self.byte(0x8B);
        self.modrm_mem(dst, base, disp);
    }

    /// `mov [base+disp], r32` (o r64 con `w`).
    pub fn store(&mut self, w: bool, base: u8, disp: i32, src: u8) {
        self.rex(w, src, base);
        self.byte(0x89);
        self.modrm_mem(src, base, disp);
    }

    /// `mov dword [base+disp], imm32`.
    pub fn store_imm(&mut self, base: u8, disp: i32, imm: u32) {
        self.rex(false, 0, base);
        self.byte(0xC7);
        self.modrm_mem(0, base, disp);
        self.u32(imm);
    }

    /// `mov r32, imm32` (pone a cero la mitad alta).
    pub fn mov_imm(&mut self, dst: u8, imm: u32) {
        self.rex(false, 0, dst);
        self.byte(0xB8 + (dst & 7));
        self.u32(imm);
    }

    /// `mov r64, imm64`.
    pub fn mov_imm64(&mut self, dst: u8, imm: u64) {
        self.rex(true, 0, dst);
        self.byte(0xB8 + (dst & 7));
        self.bytes(&imm.to_le_bytes());
    }

    /// `mov dst, src` (r32, o r64 con `w`).
    pub fn mov(&mut self, w: bool, dst: u8, src: u8) {
        self.rex(w, src, dst);
        self.byte(0x89);
        self.modrm_reg(src, dst);
    }

    /// `op dst, src`.
    pub fn alu(&mut self, w: bool, op: Alu, dst: u8, src: u8) {
        self.rex(w, src, dst);
        self.byte(op.opcode());
        self.modrm_reg(src, dst);
    }

    /// `op dst, imm32`.
    pub fn alu_imm(&mut self, w: bool, op: Alu, dst: u8, imm: u32) {
        self.rex(w, 0, dst);
        self.byte(0x81);
        self.modrm_reg(op.ext(), dst);
        self.u32(imm);
    }

    /// `cmp dword [base+disp], imm32`.
    pub fn cmp_mem_imm(&mut self, base: u8, disp: i32, imm: u32) {
        self.rex(false, 0, base);
        self.byte(0x81);
        self.modrm_mem(7, base, disp);
        self.u32(imm);
    }

    /// `test dst, src` (r32).
    pub fn test(&mut self, a: u8, b: u8) {
        self.rex(false, b, a);
        self.byte(0x85);
        self.modrm_reg(b, a);
    }

    /// `imul dst, src` (r32, o r64 con `w`).
    pub fn imul(&mut self, w: bool, dst: u8, src: u8) {
        self.rex(w, dst, src);
        self.bytes(&[0x0F, 0xAF]);
        self.modrm_reg(dst, src);
    }

    /// `imul dst, src, imm32` (r64).
    pub fn imul_imm64(&mut self, dst: u8, src: u8, imm: u32) {
        self.rex(true, dst, src);
        self.byte(0x69);
        self.modrm_reg(dst, src);
        self.u32(imm);
    }

    /// `shl/shr/sar r32, cl`: `ext` 4, 5 o 7.
    pub fn shift_cl(&mut self, ext: u8, dst: u8) {
        self.rex(false, 0, dst);
        self.byte(0xD3);
        self.modrm_reg(ext, dst);
    }

    /// `shl/shr/sar dst, imm8` (r32, o r64 con `w`).
    pub fn shift_imm(&mut self, w: bool, ext: u8, dst: u8, imm: u8) {
        self.rex(w, 0, dst);
        self.byte(0xC1);
        self.modrm_reg(ext, dst);
        self.byte(imm);
    }

    /// `neg`/`not`/`div`/`idiv` (grupo F7): `ext` 3, 2, 6, 7.
    pub fn f7(&mut self, w: bool, ext: u8, dst: u8) {
        self.rex(w, 0, dst);
        self.byte(0xF7);
        self.modrm_reg(ext, dst);
    }

    /// `cqo` (con `w`) o `cdq`.
    pub fn cqo(&mut self) {
        self.bytes(&[0x48, 0x99]);
    }

    /// `movsxd dst64, src32`.
    pub fn movsxd(&mut self, dst: u8, src: u8) {
        self.rex(true, dst, src);
        self.byte(0x63);
        self.modrm_reg(dst, src);
    }

    /// `setcc al` + `movzx eax, al`: la condicion en `eax`, 0 o 1.
    pub fn setcc_eax(&mut self, c: u8) {
        self.bytes(&[0x0F, 0x90 | c, 0xC0, 0x0F, 0xB6, 0xC0]);
    }

    /// `setcc cl` + `movzx ecx, cl`.
    pub fn setcc_ecx(&mut self, c: u8) {
        self.bytes(&[0x0F, 0x90 | c, 0xC1, 0x0F, 0xB6, 0xC9]);
    }

    pub fn push(&mut self, r: u8) {
        self.rex(false, 0, r);
        self.byte(0x50 + (r & 7));
    }

    pub fn pop(&mut self, r: u8) {
        self.rex(false, 0, r);
        self.byte(0x58 + (r & 7));
    }

    pub fn ret(&mut self) {
        self.byte(0xC3);
    }

    // ---- saltos ------------------------------------------------------------

    /// `jmp` a un destino YA conocido (o conocido en la segunda pasada).
    pub fn jmp_to(&mut self, target: usize) {
        self.byte(0xE9);
        let rel = target as i64 - (self.pos as i64 + 4);
        self.u32(rel as i32 as u32);
    }

    pub fn jcc_to(&mut self, c: u8, target: usize) {
        self.bytes(&[0x0F, 0x80 | c]);
        let rel = target as i64 - (self.pos as i64 + 4);
        self.u32(rel as i32 as u32);
    }

    pub fn call_to(&mut self, target: usize) {
        self.byte(0xE8);
        let rel = target as i64 - (self.pos as i64 + 4);
        self.u32(rel as i32 as u32);
    }

    /// `jcc` hacia DELANTE: devuelve donde parchear con [`Writer::here`].
    pub fn jcc_fwd(&mut self, c: u8) -> usize {
        self.bytes(&[0x0F, 0x80 | c]);
        let at = self.pos;
        self.u32(0);
        at
    }

    pub fn jmp_fwd(&mut self) -> usize {
        self.byte(0xE9);
        let at = self.pos;
        self.u32(0);
        at
    }

    /// El salto que se apunto en `at` aterriza AQUI.
    pub fn here(&mut self, at: usize) {
        let rel = self.pos as i64 - (at as i64 + 4);
        self.put_u32_at(at, rel as i32 as u32);
    }

    // ---- SSE ---------------------------------------------------------------

    fn sse(&mut self, pre: u8, w: bool, op: u8, reg: u8, rm: u8) {
        self.byte(pre);
        self.rex(w, reg, rm);
        self.bytes(&[0x0F, op]);
        self.modrm_reg(reg, rm);
    }

    /// `movss xmm, [base+disp]`.
    pub fn movss_load(&mut self, x: u8, base: u8, disp: i32) {
        self.byte(0xF3);
        self.rex(false, x, base);
        self.bytes(&[0x0F, 0x10]);
        self.modrm_mem(x, base, disp);
    }

    /// `movss [base+disp], xmm`.
    pub fn movss_store(&mut self, base: u8, disp: i32, x: u8) {
        self.byte(0xF3);
        self.rex(false, x, base);
        self.bytes(&[0x0F, 0x11]);
        self.modrm_mem(x, base, disp);
    }

    /// `addss`(58) `mulss`(59) `subss`(5C) `minss`(5D) `divss`(5E) `maxss`(5F) `sqrtss`(51).
    pub fn ss(&mut self, op: u8, dst: u8, src: u8) {
        self.sse(0xF3, false, op, dst, src);
    }

    /// `addsd`(58) `mulsd`(59) `subsd`(5C), y `movsd xmm, xmm` (10).
    pub fn sd(&mut self, op: u8, dst: u8, src: u8) {
        self.sse(0xF2, false, op, dst, src);
    }

    /// `ucomiss a, b`.
    pub fn ucomiss(&mut self, a: u8, b: u8) {
        self.rex(false, a, b);
        self.bytes(&[0x0F, 0x2E]);
        self.modrm_reg(a, b);
    }

    /// `comisd a, b`.
    pub fn comisd(&mut self, a: u8, b: u8) {
        self.sse(0x66, false, 0x2F, a, b);
    }

    /// `xorpd x, x`: el cero.
    pub fn xorpd(&mut self, a: u8, b: u8) {
        self.sse(0x66, false, 0x57, a, b);
    }

    /// `cvtss2sd dst, src`.
    pub fn cvtss2sd(&mut self, dst: u8, src: u8) {
        self.sse(0xF3, false, 0x5A, dst, src);
    }

    /// `cvtsd2ss dst, src`.
    pub fn cvtsd2ss(&mut self, dst: u8, src: u8) {
        self.sse(0xF2, false, 0x5A, dst, src);
    }

    /// `cvtsi2ss xmm, r32` (o r64 con `w`).
    pub fn cvtsi2ss(&mut self, w: bool, x: u8, r: u8) {
        self.sse(0xF3, w, 0x2A, x, r);
    }

    /// `cvttss2si r32, xmm` (o r64 con `w`).
    pub fn cvttss2si(&mut self, w: bool, r: u8, x: u8) {
        self.sse(0xF3, w, 0x2C, r, x);
    }

    /// `cvttsd2si r64, xmm`: trunca; si no cabe, el entero mas negativo.
    pub fn cvttsd2si(&mut self, r: u8, x: u8) {
        self.sse(0xF2, true, 0x2C, r, x);
    }

    /// `cvtsi2sd xmm, r64`.
    pub fn cvtsi2sd(&mut self, x: u8, r: u8) {
        self.sse(0xF2, true, 0x2A, x, r);
    }

    /// `movd xmm, r32` (o `movq xmm, r64` con `w`).
    pub fn movd_to_xmm(&mut self, w: bool, x: u8, r: u8) {
        self.sse(0x66, w, 0x6E, x, r);
    }

    /// `movd r32, xmm` (o `movq r64, xmm` con `w`).
    pub fn movd_from_xmm(&mut self, w: bool, r: u8, x: u8) {
        self.sse(0x66, w, 0x7E, x, r);
    }
}
