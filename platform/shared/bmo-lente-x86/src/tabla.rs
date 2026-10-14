//! **LA TABLA** -- cada instruccion que la lente sabe leer: su mapa, su
//! opcode, el prefijo que la elige, la extension de su ModRM y su FORMA (de
//! la que salen su largo y como se escribe).
//!
//! [carril]  VERDE     una tabla constante
//! [consumo] NADA      se busca al leer una instruccion
//!
//! Sale del SDM de Intel (vol. 2) y del APM de AMD (vol. 3), y la recorta lo
//! que los emisores de BMO-X escriben de verdad: el oraculo (`metro`, el
//! programa corriendo) dice que falta. Una fila que nadie emite no se pone:
//! una tabla que adivina es una lente que miente.

use crate::Mapa;

/// El prefijo que ELIGE la instruccion (SSE: `66`, `F2`, `F3` cambian el
/// opcode), o que no cuenta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pref {
    /// Ninguno de los tres: la forma `ps` de SSE, `bsf`...
    Ninguno,
    /// Da igual: `66` es el medida de operando, `F3` el `rep`.
    Cualquiera,
    P66,
    F2,
    F3,
}

/// Que dice el campo `reg` del ModRM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ext {
    /// Un registro (o no hay ModRM).
    No,
    /// La extension del opcode: `/0`..`/7`.
    Es(u8),
    /// Un ModRM FIJO (`lfence` es `0F AE E8`).
    Byte(u8),
}

/// La forma de los operandos. Dice si hay ModRM, cuantos bytes de inmediato
/// y como se escribe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forma {
    /// Sin operandos: `ret`, `cqo`, `syscall`.
    Nada,
    /// Un salto relativo de 8 o de 32 bits.
    Rel8,
    Rel32,
    /// El registro en los 3 bits bajos del opcode, de 64 (`push`, `pop`).
    Reg64,
    /// El registro en los 3 bits bajos, de su medida (`bswap`).
    Reg,
    /// `mov r, imm` (`B8+r`): 32 bits, o 64 con REX.W.
    MovImm,
    /// `mov r8, imm8` (`B0+r`).
    MovImm8,
    /// `al, imm8` / `eax, imm32` (16 con `66`).
    Acc8Imm8,
    AccImm,
    /// Solo un inmediato: `push imm32` / `push imm8`.
    Imm,
    Imm8,
    /// `r/m, r` y `r, r/m`, de la medida del operando; y de un byte.
    MR,
    RM,
    MR8,
    RM8,
    /// Solo `r/m`, de la medida del operando, de 64 (`call`/`jmp`/`push`
    /// indirectos, `pop` a memoria) o de un byte.
    M,
    M64,
    M8,
    /// `r/m, imm32` (16 con `66`), `r/m, imm8` (con signo), `r/m8, imm8`.
    MI,
    MI8,
    M8I8,
    /// Los desplazamientos: por 1 y por `cl`, de su medida o de un byte.
    M1,
    MCl,
    M8One,
    M8Cl,
    /// `imul r, r/m, imm32` / `imm8`.
    RMI,
    RMI8,
    /// `movzx`/`movsx`: de un byte y de dos a la medida del operando.
    RM8a,
    RM16a,
    /// `movsxd r64, r/m32`.
    Movsxd,
    /// Solo memoria (`ldmxcsr`, `prefetch`): un ModRM que no es registro.
    Mem,
    /// SSE: `xmm, xmm/m` y `xmm/m, xmm`; y con un `imm8` detras.
    X,
    XM,
    XI8,
    /// SSE: `xmm, r/m` (`cvtsi2sd`, `movd`) y `r, xmm/m` (`cvttsd2si`).
    XR,
    RX,
    /// SSE: `r/m, xmm` (`movd r/m, xmm`).
    RmX,
    /// VEX de tres operandos: `x, v, x/m`; y de dos: `x, x/m` y `x/m, x`.
    V3,
    V2,
    V2M,
}

impl Forma {
    /// Lleva ModRM?
    pub const fn modrm(self) -> bool {
        !matches!(self, Forma::Nada | Forma::Rel8 | Forma::Rel32 | Forma::Reg64 | Forma::Reg | Forma::MovImm | Forma::MovImm8 | Forma::Acc8Imm8 | Forma::AccImm | Forma::Imm | Forma::Imm8)
    }
}

/// Una fila: de `op` a `hasta` (una sola si son iguales).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fila {
    pub mapa: Mapa,
    pub op: u8,
    pub hasta: u8,
    pub pref: Pref,
    pub ext: Ext,
    pub vex: bool,
    pub nombre: &'static str,
    pub forma: Forma,
}

const fn f(mapa: Mapa, op: u8, pref: Pref, ext: Ext, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta: op, pref, ext, vex: false, nombre, forma }
}

const fn r(mapa: Mapa, op: u8, hasta: u8, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta, pref: Pref::Cualquiera, ext: Ext::No, vex: false, nombre, forma }
}

const fn v(mapa: Mapa, op: u8, pref: Pref, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta: op, pref, ext: Ext::No, vex: true, nombre, forma }
}

use Ext::{Byte as B, Es as E, No as N};
use Forma::*;
use Mapa::{Dos as D, Tres38 as D38, Uno as U};
use Pref::{Cualquiera as C, F2 as P2, F3 as P3, Ninguno as P0, P66 as P6};

/// Las del grupo 1 (`80`, `81`, `83`), en el orden de su `/n`.
const GRUPO1: [&str; 8] = ["add", "or", "adc", "sbb", "and", "sub", "xor", "cmp"];
/// Las del grupo 2 (desplazamientos), en el orden de su `/n`.
pub const GRUPO2: [&str; 8] = ["rol", "ror", "rcl", "rcr", "shl", "shr", "sal", "sar"];
/// Los sufijos de condicion de `jcc`, `setcc` y `cmovcc`, por los 4 bits bajos.
pub const CC: [&str; 16] = ["o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g"];

/// Una fila por cada (grupo, forma) de la ALU clasica (`00`..`3D`): r/m8,r8
/// r/m,r r8,r/m8 r,r/m al,imm8 eax,imm32.
const fn alu(base: u8, nombre: &'static str) -> [Fila; 6] {
    [
        f(U, base, C, N, nombre, MR8),
        f(U, base + 1, C, N, nombre, MR),
        f(U, base + 2, C, N, nombre, RM8),
        f(U, base + 3, C, N, nombre, RM),
        f(U, base + 4, C, N, nombre, Acc8Imm8),
        f(U, base + 5, C, N, nombre, AccImm),
    ]
}

/// Un grupo de `/n` sobre el mismo opcode, con la misma forma.
const fn grupo(mapa: Mapa, op: u8, nombres: [&'static str; 8], forma: Forma) -> [Fila; 8] {
    let mut out = [f(mapa, op, C, E(0), nombres[0], forma); 8];
    let mut k = 1;
    while k < 8 {
        out[k] = f(mapa, op, C, E(k as u8), nombres[k], forma);
        k += 1;
    }
    out
}

const A0: [Fila; 6] = alu(0x00, "add");
const A1: [Fila; 6] = alu(0x08, "or");
const A2: [Fila; 6] = alu(0x10, "adc");
const A3: [Fila; 6] = alu(0x18, "sbb");
const A4: [Fila; 6] = alu(0x20, "and");
const A5: [Fila; 6] = alu(0x28, "sub");
const A6: [Fila; 6] = alu(0x30, "xor");
const A7: [Fila; 6] = alu(0x38, "cmp");
const G80: [Fila; 8] = grupo(U, 0x80, GRUPO1, M8I8);
const G81: [Fila; 8] = grupo(U, 0x81, GRUPO1, MI);
const G83: [Fila; 8] = grupo(U, 0x83, GRUPO1, MI8);
const GC0: [Fila; 8] = grupo(U, 0xC0, GRUPO2, M8I8);
const GC1: [Fila; 8] = grupo(U, 0xC1, GRUPO2, MI8);
const GD0: [Fila; 8] = grupo(U, 0xD0, GRUPO2, M8One);
const GD1: [Fila; 8] = grupo(U, 0xD1, GRUPO2, M1);
const GD2: [Fila; 8] = grupo(U, 0xD2, GRUPO2, M8Cl);
const GD3: [Fila; 8] = grupo(U, 0xD3, GRUPO2, MCl);

/// El resto, a mano: cada una con su forma.
const SUELTAS: &[Fila] = &[
    r(U, 0x50, 0x57, "push", Reg64),
    r(U, 0x58, 0x5F, "pop", Reg64),
    f(U, 0x63, C, N, "movsxd", Movsxd),
    f(U, 0x68, C, N, "push", Imm),
    f(U, 0x69, C, N, "imul", RMI),
    f(U, 0x6A, C, N, "push", Imm8),
    f(U, 0x6B, C, N, "imul", RMI8),
    r(U, 0x70, 0x7F, "j", Rel8),
    f(U, 0x84, C, N, "test", MR8),
    f(U, 0x85, C, N, "test", MR),
    f(U, 0x86, C, N, "xchg", MR8),
    f(U, 0x87, C, N, "xchg", MR),
    f(U, 0x88, C, N, "mov", MR8),
    f(U, 0x89, C, N, "mov", MR),
    f(U, 0x8A, C, N, "mov", RM8),
    f(U, 0x8B, C, N, "mov", RM),
    f(U, 0x8D, C, N, "lea", RM),
    f(U, 0x8F, C, E(0), "pop", M64),
    f(U, 0x90, P3, N, "pause", Nada),
    f(U, 0x90, C, N, "nop", Nada),
    f(U, 0x98, C, N, "cdqe", Nada),
    f(U, 0x99, C, N, "cqo", Nada),
    f(U, 0x9C, C, N, "pushfq", Nada),
    f(U, 0x9D, C, N, "popfq", Nada),
    f(U, 0xA4, C, N, "movsb", Nada),
    f(U, 0xA5, C, N, "movs", Nada),
    f(U, 0xAA, C, N, "stosb", Nada),
    f(U, 0xAB, C, N, "stos", Nada),
    r(U, 0xB0, 0xB7, "mov", MovImm8),
    r(U, 0xB8, 0xBF, "mov", MovImm),
    f(U, 0xC3, C, N, "ret", Nada),
    f(U, 0xC6, C, E(0), "mov", M8I8),
    f(U, 0xC7, C, E(0), "mov", MI),
    f(U, 0xCC, C, N, "int3", Nada),
    f(U, 0xE8, C, N, "call", Rel32),
    f(U, 0xE9, C, N, "jmp", Rel32),
    f(U, 0xEB, C, N, "jmp", Rel8),
    f(U, 0xF4, C, N, "hlt", Nada),
    f(U, 0xF6, C, E(0), "test", M8I8),
    f(U, 0xF6, C, E(2), "not", M8),
    f(U, 0xF6, C, E(3), "neg", M8),
    f(U, 0xF6, C, E(4), "mul", M8),
    f(U, 0xF6, C, E(5), "imul", M8),
    f(U, 0xF6, C, E(6), "div", M8),
    f(U, 0xF6, C, E(7), "idiv", M8),
    f(U, 0xF7, C, E(0), "test", MI),
    f(U, 0xF7, C, E(2), "not", M),
    f(U, 0xF7, C, E(3), "neg", M),
    f(U, 0xF7, C, E(4), "mul", M),
    f(U, 0xF7, C, E(5), "imul", M),
    f(U, 0xF7, C, E(6), "div", M),
    f(U, 0xF7, C, E(7), "idiv", M),
    f(U, 0xFA, C, N, "cli", Nada),
    f(U, 0xFB, C, N, "sti", Nada),
    f(U, 0xFC, C, N, "cld", Nada),
    f(U, 0xFD, C, N, "std", Nada),
    f(U, 0xFE, C, E(0), "inc", M8),
    f(U, 0xFE, C, E(1), "dec", M8),
    f(U, 0xFF, C, E(0), "inc", M),
    f(U, 0xFF, C, E(1), "dec", M),
    f(U, 0xFF, C, E(2), "call", M64),
    f(U, 0xFF, C, E(4), "jmp", M64),
    f(U, 0xFF, C, E(6), "push", M64),
    // -- el mapa 0F: lo entero ------------------------------------------
    f(D, 0x05, C, N, "syscall", Nada),
    f(D, 0x0B, C, N, "ud2", Nada),
    f(D, 0x1F, C, E(0), "nop", M),
    f(D, 0x31, C, N, "rdtsc", Nada),
    r(D, 0x40, 0x4F, "cmov", RM),
    r(D, 0x80, 0x8F, "j", Rel32),
    r(D, 0x90, 0x9F, "set", M8),
    f(D, 0xA2, C, N, "cpuid", Nada),
    f(D, 0xA3, C, N, "bt", MR),
    f(D, 0xAB, C, N, "bts", MR),
    f(D, 0xAE, P0, E(2), "ldmxcsr", Mem),
    f(D, 0xAE, P0, E(3), "stmxcsr", Mem),
    f(D, 0xAE, P0, B(0xE8), "lfence", M),
    f(D, 0xAE, P0, B(0xF0), "mfence", M),
    f(D, 0xAE, P0, B(0xF8), "sfence", M),
    f(D, 0xAF, C, N, "imul", RM),
    f(D, 0xB0, C, N, "cmpxchg", MR8),
    f(D, 0xB1, C, N, "cmpxchg", MR),
    f(D, 0xB6, C, N, "movzx", RM8a),
    f(D, 0xB7, C, N, "movzx", RM16a),
    f(D, 0xB8, P3, N, "popcnt", RM),
    f(D, 0xBA, C, E(4), "bt", MI8),
    f(D, 0xBC, P3, N, "tzcnt", RM),
    f(D, 0xBC, C, N, "bsf", RM),
    f(D, 0xBD, P3, N, "lzcnt", RM),
    f(D, 0xBD, C, N, "bsr", RM),
    f(D, 0xBE, C, N, "movsx", RM8a),
    f(D, 0xBF, C, N, "movsx", RM16a),
    f(D, 0xC1, C, N, "xadd", MR),
    r(D, 0xC8, 0xCF, "bswap", Reg),
    // -- el mapa 0F: SSE ------------------------------------------------
    f(D, 0x10, P0, N, "movups", X),
    f(D, 0x10, P6, N, "movupd", X),
    f(D, 0x10, P3, N, "movss", X),
    f(D, 0x10, P2, N, "movsd", X),
    f(D, 0x11, P0, N, "movups", XM),
    f(D, 0x11, P6, N, "movupd", XM),
    f(D, 0x11, P3, N, "movss", XM),
    f(D, 0x11, P2, N, "movsd", XM),
    f(D, 0x28, P0, N, "movaps", X),
    f(D, 0x28, P6, N, "movapd", X),
    f(D, 0x29, P0, N, "movaps", XM),
    f(D, 0x29, P6, N, "movapd", XM),
    f(D, 0x2A, P3, N, "cvtsi2ss", XR),
    f(D, 0x2A, P2, N, "cvtsi2sd", XR),
    f(D, 0x2C, P3, N, "cvttss2si", RX),
    f(D, 0x2C, P2, N, "cvttsd2si", RX),
    f(D, 0x2D, P3, N, "cvtss2si", RX),
    f(D, 0x2D, P2, N, "cvtsd2si", RX),
    f(D, 0x2E, P0, N, "ucomiss", X),
    f(D, 0x2E, P6, N, "ucomisd", X),
    f(D, 0x2F, P0, N, "comiss", X),
    f(D, 0x2F, P6, N, "comisd", X),
    f(D, 0x51, P0, N, "sqrtps", X),
    f(D, 0x51, P3, N, "sqrtss", X),
    f(D, 0x51, P2, N, "sqrtsd", X),
    f(D, 0x54, P0, N, "andps", X),
    f(D, 0x54, P6, N, "andpd", X),
    f(D, 0x55, P0, N, "andnps", X),
    f(D, 0x55, P6, N, "andnpd", X),
    f(D, 0x56, P0, N, "orps", X),
    f(D, 0x56, P6, N, "orpd", X),
    f(D, 0x57, P0, N, "xorps", X),
    f(D, 0x57, P6, N, "xorpd", X),
    f(D, 0x58, P0, N, "addps", X),
    f(D, 0x58, P6, N, "addpd", X),
    f(D, 0x58, P3, N, "addss", X),
    f(D, 0x58, P2, N, "addsd", X),
    f(D, 0x59, P0, N, "mulps", X),
    f(D, 0x59, P6, N, "mulpd", X),
    f(D, 0x59, P3, N, "mulss", X),
    f(D, 0x59, P2, N, "mulsd", X),
    f(D, 0x5A, P0, N, "cvtps2pd", X),
    f(D, 0x5A, P6, N, "cvtpd2ps", X),
    f(D, 0x5A, P3, N, "cvtss2sd", X),
    f(D, 0x5A, P2, N, "cvtsd2ss", X),
    f(D, 0x5B, P0, N, "cvtdq2ps", X),
    f(D, 0x5B, P6, N, "cvtps2dq", X),
    f(D, 0x5B, P3, N, "cvttps2dq", X),
    f(D, 0x5C, P0, N, "subps", X),
    f(D, 0x5C, P6, N, "subpd", X),
    f(D, 0x5C, P3, N, "subss", X),
    f(D, 0x5C, P2, N, "subsd", X),
    f(D, 0x5D, P0, N, "minps", X),
    f(D, 0x5D, P6, N, "minpd", X),
    f(D, 0x5D, P3, N, "minss", X),
    f(D, 0x5D, P2, N, "minsd", X),
    f(D, 0x5E, P0, N, "divps", X),
    f(D, 0x5E, P6, N, "divpd", X),
    f(D, 0x5E, P3, N, "divss", X),
    f(D, 0x5E, P2, N, "divsd", X),
    f(D, 0x5F, P0, N, "maxps", X),
    f(D, 0x5F, P6, N, "maxpd", X),
    f(D, 0x5F, P3, N, "maxss", X),
    f(D, 0x5F, P2, N, "maxsd", X),
    f(D, 0x6E, P6, N, "movd", XR),
    f(D, 0x6F, P6, N, "movdqa", X),
    f(D, 0x6F, P3, N, "movdqu", X),
    f(D, 0x70, P6, N, "pshufd", XI8),
    f(D, 0x7E, P6, N, "movd", RmX),
    f(D, 0x7E, P3, N, "movq", X),
    f(D, 0x7F, P6, N, "movdqa", XM),
    f(D, 0x7F, P3, N, "movdqu", XM),
    f(D, 0xC2, P0, N, "cmpps", XI8),
    f(D, 0xC2, P3, N, "cmpss", XI8),
    f(D, 0xC2, P2, N, "cmpsd", XI8),
    f(D, 0xC6, P0, N, "shufps", XI8),
    f(D, 0xD6, P6, N, "movq", XM),
    f(D, 0xEF, P6, N, "pxor", X),
    f(D, 0xFE, P6, N, "paddd", X),
    // -- VEX: lo que emite la tabla de intrinsecos (AVX2) ---------------
    v(D, 0x77, P0, "vzeroupper", Nada),
    v(D, 0x10, P0, "vmovups", V2),
    v(D, 0x11, P0, "vmovups", V2M),
    v(D, 0x58, P0, "vaddps", V3),
    v(D, 0x59, P0, "vmulps", V3),
    v(D, 0x5C, P0, "vsubps", V3),
    v(D38, 0xB8, P6, "vfmadd231ps", V3),
];

/// Todas las filas, en un orden: las de un prefijo EXACTO antes que las de
/// "da igual" (`F3 90` es `pause` antes de ser `nop`).
pub const GRUPOS: [&[Fila]; 18] = [SUELTAS, &A0, &A1, &A2, &A3, &A4, &A5, &A6, &A7, &G80, &G81, &G83, &GC0, &GC1, &GD0, &GD1, &GD2, &GD3];

/// Las filas candidatas a `(mapa, op)` con o sin VEX.
pub fn candidatas(mapa: Mapa, op: u8, vex: bool) -> impl Iterator<Item = &'static Fila> {
    GRUPOS.iter().flat_map(|g| g.iter()).filter(move |f| f.mapa == mapa && f.vex == vex && (f.op..=f.hasta).contains(&op))
}
