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
    /// `r/m, r, imm8` y `r/m, r, cl` (`shld`, `shrd`).
    MRI8,
    MRCl,
    /// `r/m16, sreg` y `sreg, r/m16` (`mov [rcx+0x38], cs`).
    MSreg,
    SregM,
    /// x87: solo memoria, de 8 bytes (`fld qword`) o de 2 (`fldcw`).
    M87q,
    M87w,
    /// x87 de registro: `st(i)`, `st(i), st` y `st, st(i)` (solo `mod == 3`).
    StI,
    StISt,
    StStI,
}

impl Forma {
    /// Su ModRM tiene que ser memoria.
    pub const fn solo_memoria(self) -> bool {
        matches!(self, Forma::Mem | Forma::M87q | Forma::M87w)
    }

    /// Su ModRM tiene que ser registro (`mod == 3`).
    pub const fn solo_registro(self) -> bool {
        matches!(self, Forma::StI | Forma::StISt | Forma::StStI)
    }

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
    /// La extension de x86-64 que pide (`"POPCNT"`, `"AVX"`...), con el
    /// nombre de `bmo_carga_juicio::instrucciones`; `""` si es x86-64 base.
    /// Con esto la lente juzga lo que un `.bex` DECLARA (A6 del foco).
    pub pide: &'static str,
}

impl Fila {
    /// **Sin su extension, falla?** Casi todas si: `#UD`. Pero `tzcnt` y
    /// `lzcnt` son `F3` delante de `bsf` y `bsr`, y una CPU sin BMI1 o sin
    /// LZCNT ignora el `F3` y ejecuta la vieja -- por eso LLVM emite `rep bsf`
    /// en x86-64 BASE (el DIRECTOR lo lleva, visto el 14-10). Con una entrada
    /// distinta de cero `tzcnt` y `bsf` dan lo mismo; `lzcnt` y `bsr`, no (son
    /// complementarios): ese es el programa que miente, y lo juzga quien lo
    /// emitio (A6), no esto.
    pub const fn sin_ella_falla(&self) -> bool {
        !matches!(self.op, 0xBC | 0xBD) || !matches!(self.mapa, crate::Mapa::Dos)
    }

    /// La misma fila, pidiendo una extension.
    pub const fn pide(self, ext: &'static str) -> Fila {
        Fila { pide: ext, ..self }
    }
}

const fn f(mapa: Mapa, op: u8, pref: Pref, ext: Ext, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta: op, pref, ext, vex: false, nombre, forma, pide: "" }
}

const fn r(mapa: Mapa, op: u8, hasta: u8, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta, pref: Pref::Cualquiera, ext: Ext::No, vex: false, nombre, forma, pide: "" }
}

const fn v(mapa: Mapa, op: u8, pref: Pref, nombre: &'static str, forma: Forma) -> Fila {
    Fila { mapa, op, hasta: op, pref, ext: Ext::No, vex: true, nombre, forma, pide: "AVX" }
}

use Ext::{Byte as B, Es as E, No as N};
use Forma::*;
use Mapa::{Dos as D, Tres38 as D38, Tres3A as D3A, Uno as U};
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
    f(D, 0xAE, P0, E(0), "fxsave", Mem),
    f(D, 0xAE, P0, E(1), "fxrstor", Mem),
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
    f(D, 0xB8, P3, N, "popcnt", RM).pide("POPCNT"),
    f(D, 0xBA, C, E(4), "bt", MI8),
    f(D, 0xBC, P3, N, "tzcnt", RM).pide("BMI1"),
    f(D, 0xBC, C, N, "bsf", RM),
    f(D, 0xBD, P3, N, "lzcnt", RM).pide("LZCNT"),
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
    // ** 0F 12..17 y 0F 50 (15-10): las mezclas y medias cargas que LLVM
    // emite para `estudio` (x86_64-bmo-ring3, con SSE). Sin ellas bex-link
    // paraba el build. Los nombres son los de la forma con memoria; con dos
    // registros 0F 12 es `movhlps` y 0F 16 es `movlhps`, mismo largo.
    f(D, 0x12, P0, N, "movlps", X),
    f(D, 0x12, P6, N, "movlpd", X),
    f(D, 0x13, P0, N, "movlps", XM),
    f(D, 0x13, P6, N, "movlpd", XM),
    f(D, 0x14, P0, N, "unpcklps", X),
    f(D, 0x14, P6, N, "unpcklpd", X),
    f(D, 0x15, P0, N, "unpckhps", X),
    f(D, 0x15, P6, N, "unpckhpd", X),
    f(D, 0x16, P0, N, "movhps", X),
    f(D, 0x16, P6, N, "movhpd", X),
    f(D, 0x17, P0, N, "movhps", XM),
    f(D, 0x17, P6, N, "movhpd", XM),
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
    f(D, 0x50, P0, N, "movmskps", RX),
    f(D, 0x50, P6, N, "movmskpd", RX),
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
    // ** Los desplazamientos por un inmediato (15-10, `estudio`): el
    // registro va en r/m y el campo reg es la extension. Se leen con XI8, que
    // tiene el mismo largo (ModRM + imm8); el texto nombra un xmm de mas.
    f(D, 0x72, P6, E(2), "psrld", XI8),
    f(D, 0x72, P6, E(4), "psrad", XI8),
    f(D, 0x72, P6, E(6), "pslld", XI8),
    f(D, 0x7E, P6, N, "movd", RmX),
    f(D, 0x7E, P3, N, "movq", X),
    f(D, 0x7F, P6, N, "movdqa", XM),
    f(D, 0x7F, P3, N, "movdqu", XM),
    f(D, 0xC2, P0, N, "cmpps", XI8),
    f(D, 0xC2, P3, N, "cmpss", XI8),
    f(D, 0xC2, P2, N, "cmpsd", XI8),
    // `pinsrw xmm, r32/m16, imm8`: mismo largo que XI8 (15-10, `estudio`).
    f(D, 0xC4, P6, N, "pinsrw", XI8),
    f(D, 0xC6, P0, N, "shufps", XI8),
    f(D, 0xD6, P6, N, "movq", XM),
    // ** Lo entero de SSE2 que LLVM saca de las cuentas de `estudio` y de
    // VERRANO en la CPU (15-10): todas `xmm, xmm/m`.
    f(D, 0x62, P6, N, "punpckldq", X),
    f(D, 0x66, P6, N, "pcmpgtd", X),
    f(D, 0x6C, P6, N, "punpcklqdq", X),
    f(D, 0x6D, P6, N, "punpckhqdq", X),
    f(D, 0x76, P6, N, "pcmpeqd", X),
    f(D, 0xD4, P6, N, "paddq", X),
    f(D, 0xDB, P6, N, "pand", X),
    f(D, 0xDF, P6, N, "pandn", X),
    f(D, 0xEB, P6, N, "por", X),
    f(D, 0xF4, P6, N, "pmuludq", X),
    f(D, 0xFA, P6, N, "psubd", X),
    f(D, 0xFB, P6, N, "psubq", X),
    f(D, 0xEF, P6, N, "pxor", X),
    f(D, 0xFE, P6, N, "paddd", X),
    // ** El SSE2 de ENTEROS entero (LX2, 15-10): lo que MSVC escribe en el
    // CRT y en los `.exe` de Windows (`pcmpeqw` en el BMOX-12 paraba la
    // lente). Todas `xmm, xmm/m`, y las de inmediato como las de arriba.
    f(D, 0x60, P6, N, "punpcklbw", X),
    f(D, 0x61, P6, N, "punpcklwd", X),
    f(D, 0x63, P6, N, "packsswb", X),
    f(D, 0x64, P6, N, "pcmpgtb", X),
    f(D, 0x65, P6, N, "pcmpgtw", X),
    f(D, 0x67, P6, N, "packuswb", X),
    f(D, 0x68, P6, N, "punpckhbw", X),
    f(D, 0x69, P6, N, "punpckhwd", X),
    f(D, 0x6A, P6, N, "punpckhdq", X),
    f(D, 0x6B, P6, N, "packssdw", X),
    f(D, 0x74, P6, N, "pcmpeqb", X),
    f(D, 0x75, P6, N, "pcmpeqw", X),
    f(D, 0xD1, P6, N, "psrlw", X),
    f(D, 0xD2, P6, N, "psrld", X),
    f(D, 0xD3, P6, N, "psrlq", X),
    f(D, 0xD5, P6, N, "pmullw", X),
    f(D, 0xD8, P6, N, "psubusb", X),
    f(D, 0xD9, P6, N, "psubusw", X),
    f(D, 0xDA, P6, N, "pminub", X),
    f(D, 0xDC, P6, N, "paddusb", X),
    f(D, 0xDD, P6, N, "paddusw", X),
    f(D, 0xDE, P6, N, "pmaxub", X),
    f(D, 0xE0, P6, N, "pavgb", X),
    f(D, 0xE1, P6, N, "psraw", X),
    f(D, 0xE2, P6, N, "psrad", X),
    f(D, 0xE3, P6, N, "pavgw", X),
    f(D, 0xE4, P6, N, "pmulhuw", X),
    f(D, 0xE5, P6, N, "pmulhw", X),
    f(D, 0xE8, P6, N, "psubsb", X),
    f(D, 0xE9, P6, N, "psubsw", X),
    f(D, 0xEA, P6, N, "pminsw", X),
    f(D, 0xEC, P6, N, "paddsb", X),
    f(D, 0xED, P6, N, "paddsw", X),
    f(D, 0xEE, P6, N, "pmaxsw", X),
    f(D, 0xF1, P6, N, "psllw", X),
    f(D, 0xF2, P6, N, "pslld", X),
    f(D, 0xF3, P6, N, "psllq", X),
    f(D, 0xF5, P6, N, "pmaddwd", X),
    f(D, 0xF6, P6, N, "psadbw", X),
    f(D, 0xF8, P6, N, "psubb", X),
    f(D, 0xF9, P6, N, "psubw", X),
    f(D, 0xFC, P6, N, "paddb", X),
    f(D, 0xFD, P6, N, "paddw", X),
    f(D, 0xD7, P6, N, "pmovmskb", RX),
    f(D, 0x71, P6, E(2), "psrlw", XI8),
    f(D, 0x71, P6, E(4), "psraw", XI8),
    f(D, 0x71, P6, E(6), "psllw", XI8),
    f(D, 0x73, P6, E(2), "psrlq", XI8),
    f(D, 0x73, P6, E(3), "psrldq", XI8),
    f(D, 0x73, P6, E(6), "psllq", XI8),
    f(D, 0x73, P6, E(7), "pslldq", XI8),
    f(D, 0x70, P2, N, "pshuflw", XI8),
    f(D, 0x70, P3, N, "pshufhw", XI8),
    // -- VEX: lo que emite la tabla de intrinsecos (AVX2) ---------------
    v(D, 0x77, P0, "vzeroupper", Nada),
    v(D, 0x10, P0, "vmovups", V2),
    v(D, 0x11, P0, "vmovups", V2M),
    v(D, 0x58, P0, "vaddps", V3),
    v(D, 0x59, P0, "vmulps", V3),
    v(D, 0x5C, P0, "vsubps", V3),
    v(D38, 0xB8, P6, "vfmadd231ps", V3).pide("FMA"),
    // Las mismas en `pd` (`66`): cuatro flotante64, las de `avx_suma4` y
    // sus hermanas. `vfmadd231` es `pd` con VEX.W: lo dice el texto.
    v(D, 0x10, P6, "vmovupd", V2),
    v(D, 0x11, P6, "vmovupd", V2M),
    v(D, 0x58, P6, "vaddpd", V3),
    v(D, 0x59, P6, "vmulpd", V3),
    v(D, 0x5C, P6, "vsubpd", V3),
    // -- Lo que LLVM emite en los programas de Rust (B4, 14-10) ---------
    //    `bex-link --censo` sobre los diez `.bex` de Rust dijo que faltaba;
    //    sin esto la lente se paraba en el 3..70 % de su codigo.
    f(U, 0x8C, C, N, "mov", MSreg),
    f(U, 0x8E, C, N, "mov", SregM),
    f(U, 0xA8, C, N, "test", Acc8Imm8),
    f(U, 0xA9, C, N, "test", AccImm),
    f(D, 0xA4, C, N, "shld", MRI8),
    f(D, 0xA5, C, N, "shld", MRCl),
    f(D, 0xAC, C, N, "shrd", MRI8),
    f(D, 0xAD, C, N, "shrd", MRCl),
    f(D3A, 0x0A, P6, N, "roundss", XI8).pide("SSE4.1"),
    f(D3A, 0x0B, P6, N, "roundsd", XI8).pide("SSE4.1"),
    //    x87: el `crt_mates` de PROTON-X (`sin`, `atan`, `log`...) y la
    //    palabra de control. De memoria, de registro y fijas.
    f(U, 0xDD, C, E(0), "fld", M87q),
    f(U, 0xDD, C, E(3), "fstp", M87q),
    f(U, 0xDD, C, E(3), "fstp", StI),
    f(U, 0xD9, C, E(5), "fldcw", M87w),
    f(U, 0xD9, C, E(7), "fnstcw", M87w),
    f(U, 0xD9, C, E(0), "fld", StI),
    f(U, 0xD9, C, E(1), "fxch", StI),
    f(U, 0xD8, C, E(1), "fmul", StStI),
    f(U, 0xDC, C, E(5), "fsub", StISt),
    f(U, 0xDE, C, E(0), "faddp", StISt),
    f(U, 0xDE, C, E(4), "fsubrp", StISt),
    f(U, 0xDF, C, B(0xE0), "fnstsw ax", M),
    f(U, 0xD9, C, B(0xE8), "fld1", M),
    f(U, 0xD9, C, B(0xED), "fldln2", M),
    f(U, 0xD9, C, B(0xF0), "f2xm1", M),
    f(U, 0xD9, C, B(0xF1), "fyl2x", M),
    f(U, 0xD9, C, B(0xF2), "fptan", M),
    f(U, 0xD9, C, B(0xF3), "fpatan", M),
    f(U, 0xD9, C, B(0xF8), "fprem", M),
    f(U, 0xD9, C, B(0xFA), "fsqrt", M),
    f(U, 0xD9, C, B(0xFC), "frndint", M),
    f(U, 0xD9, C, B(0xFD), "fscale", M),
    f(U, 0xD9, C, B(0xFE), "fsin", M),
    f(U, 0xD9, C, B(0xFF), "fcos", M),
    // -- Las de la tabla de intrinsecos que piden una extension (A6) -----
    f(D, 0x01, P0, B(0xF9), "rdtscp", M).pide("RDTSCP"),
    f(D, 0x01, P0, B(0xD0), "xgetbv", M).pide("CR4.OSXSAVE"),
    f(D, 0x01, P0, B(0xD1), "xsetbv", M).pide("CR4.OSXSAVE"),
    f(D, 0x01, P0, B(0xC8), "monitor", M).pide("MONITOR/MWAIT"),
    f(D, 0x01, P0, B(0xC9), "mwait", M).pide("MONITOR/MWAIT"),
    f(D, 0xC7, P0, E(6), "rdrand", M).pide("RDRAND"),
    f(D, 0xC7, P0, E(7), "rdseed", M).pide("RDSEED"),
];

/// Todas las filas, en un orden: las de un prefijo EXACTO antes que las de
/// "da igual" (`F3 90` es `pause` antes de ser `nop`).
pub const GRUPOS: [&[Fila]; 18] = [SUELTAS, &A0, &A1, &A2, &A3, &A4, &A5, &A6, &A7, &G80, &G81, &G83, &GC0, &GC1, &GD0, &GD1, &GD2, &GD3];

/// Las filas candidatas a `(mapa, op)` con o sin VEX.
pub fn candidatas(mapa: Mapa, op: u8, vex: bool) -> impl Iterator<Item = &'static Fila> {
    GRUPOS.iter().flat_map(|g| g.iter()).filter(move |f| f.mapa == mapa && f.vex == vex && (f.op..=f.hasta).contains(&op))
}
