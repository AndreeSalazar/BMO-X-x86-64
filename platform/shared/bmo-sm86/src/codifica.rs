//! **EL CODIFICADOR DE SM86** (E2 de `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`)
//! -- cada instruccion que el emisor va a poner, bit a bit como `ptxas`.
//!
//! [carril]  VERDE     puro: de campos a dos palabras de 64 bits
//! [consumo] NADA
//!
//! # De donde sale cada bit (nada adivinado)
//!
//! Cada codificador tiene su PALABRA DE ORO: la que escribio `ptxas
//! -arch=sm_86` (CUDA 12.9, de PyPI) para un PTX hecho a proposito, leida con
//! `nvdisasm -hex` (13.4). La prueba de abajo compara los 128 bits, control
//! incluido. Donde va cada campo es lo que ya dice el juez (`juez.rs`, de NAK
//! `sm70_encode.rs`):
//!
//! ```text
//!    0..9     opcode              9..12   forma: 1 reg-reg, 2/4 inmediato,
//!    12..16   predicado (7 = PT,          3/5 constante (c[banco][desp])
//!             +8 negado)          16..24  destino
//!    24..32   fuente 0            32..40  fuente 1 (registro), o 32..64 un
//!                                         inmediato de 32 bits, o 40..54 el
//!                                         desplazamiento/4 y 54..59 el banco
//!    64..72   fuente 2 (registro) 72/73   fuente 0 negada / valor absoluto
//!    62/63    fuente 1 absoluta / negada
//!    77       .SAT                105..128  el control (lo de `juez.rs`), que
//!                                           aqui se da hecho: `control` =
//!                                           palabra alta >> 41
//! ```
//!
//! Lo que es de cada instruccion y no tiene nombre en esta tabla va como
//! `ptxas` lo pone, con su comentario: el bit 86 de FMUL, la mascara 0xF de
//! MOV, el predicado PT de EXIT y de FMNMX.
//!
//! # Lo que NO es
//!
//! - No asigna registros ni pone los bits de control: eso es el emisor (E3) y
//!   sus reglas (E4). Aqui el control se recibe.
//! - No cubre todo SM86: lo que el emisor necesita para el cubo de VERRANO y
//!   para BMOX-12 (dp4, dp3, mad, rsq, saturar). Una instruccion nueva entra
//!   con su palabra de oro, nunca sin ella.
//! - `MUFU.RSQ` es la de la 3060, APROXIMADA: no es la raiz exacta de la
//!   casa (`raiz`). Cuanto se separa se mide en E3, no se supone aqui.

/// Un operando de fuente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fuente {
    /// Un registro, con sus modificadores.
    R { r: u8, neg: bool, abs: bool },
    /// Un inmediato de 32 bits (un `f32` en sus bits).
    Imm(u32),
    /// Una constante: `c[banco][desp]` (desp multiplo de 4).
    C { banco: u8, desp: u16 },
}

/// Un registro sin modificadores.
pub const fn r(r: u8) -> Fuente {
    Fuente::R { r, neg: false, abs: false }
}

/// Un registro negado.
pub const fn neg(r: u8) -> Fuente {
    Fuente::R { r, neg: true, abs: false }
}

/// El valor absoluto de un registro.
pub const fn abs(r: u8) -> Fuente {
    Fuente::R { r, neg: false, abs: true }
}

/// El registro cero.
pub const RZ: u8 = 255;

/// Siempre (el predicado PT, sin negar).
const SIEMPRE: u64 = 7 << 12;

fn palabra(lo: u64, hi: u64, control: u64) -> (u64, u64) {
    (lo, (hi & ((1 << 41) - 1)) | control << 41)
}

/// Los modificadores de la fuente 0 (72 neg, 73 abs), en la palabra alta.
fn mod0(f: Fuente) -> (u8, u64) {
    match f {
        Fuente::R { r, neg, abs } => (r, (neg as u64) << 8 | (abs as u64) << 9),
        _ => (RZ, 0),
    }
}

/// La segunda fuente de una de dos o tres: su forma y sus bits.
/// `forma_imm`/`forma_c` son las del hueco donde la pone esta instruccion
/// (FADD: 2 y 3; FMUL y FFMA: 4 y 5).
fn segunda(f: Fuente, forma_imm: u64, forma_c: u64) -> (u64, u64) {
    match f {
        Fuente::R { r, neg, abs } => (1 << 9 | (r as u64) << 32 | (abs as u64) << 62 | (neg as u64) << 63, 0),
        Fuente::Imm(v) => (forma_imm << 9 | (v as u64) << 32, 0),
        Fuente::C { banco, desp } => (forma_c << 9 | ((desp as u64 >> 2) & 0x3FFF) << 40 | (banco as u64 & 0x1F) << 54, 0),
    }
}

fn dos(op: u64, forma_imm: u64, forma_c: u64, hi_fijo: u64, rd: u8, a: Fuente, b: Fuente, sat: bool, control: u64) -> (u64, u64) {
    let (ra, m0) = mod0(a);
    let (lo_b, _) = segunda(b, forma_imm, forma_c);
    let lo = op | SIEMPRE | (rd as u64) << 16 | (ra as u64) << 24 | lo_b;
    palabra(lo, hi_fijo | m0 | (sat as u64) << 13, control)
}

/// `FADD[.SAT] Rd, a, b`.
pub fn fadd(rd: u8, a: Fuente, b: Fuente, sat: bool, control: u64) -> (u64, u64) {
    dos(0x021, 2, 3, 0, rd, a, b, sat, control)
}

/// `FMUL[.SAT] Rd, a, b`. El bit 86 lo pone `ptxas` en todo FMUL.
pub fn fmul(rd: u8, a: Fuente, b: Fuente, sat: bool, control: u64) -> (u64, u64) {
    dos(0x020, 4, 5, 1 << 22, rd, a, b, sat, control)
}

/// `FFMA[.SAT] Rd, a, b, c` (`c` un registro, o un inmediato).
///
/// ** Con `c` INMEDIATO, la forma 2 (`ORO_FFMA_C`, 09-10): el inmediato va
/// en 32..64 y la `b` -- que entonces tiene que ser un registro -- se muda al
/// hueco de la `c` (64..72, con su `|x|` en 74 y su `-` en 75). Una `b` que
/// no es un registro con una `c` inmediata no se puede escribir: la de la
/// `c` se queda en RZ, como antes, y lo dice el `debug_assert`.
pub fn ffma(rd: u8, a: Fuente, b: Fuente, c: Fuente, sat: bool, control: u64) -> (u64, u64) {
    let hueco_c = |f: Fuente| match f {
        Fuente::R { r, neg, abs } => (r, (abs as u64) << 10 | (neg as u64) << 11),
        _ => (RZ, 0),
    };
    if let (Fuente::Imm(v), Fuente::R { .. }) = (c, b) {
        let (rb, mb) = hueco_c(b);
        let (lo, hi) = dos(0x023, 2, 3, 0, rd, a, Fuente::Imm(v), sat, control);
        return (lo, hi | rb as u64 | mb);
    }
    debug_assert!(!matches!(c, Fuente::Imm(_)), "FFMA con la c inmediata pide la b en un registro");
    let (rc, mc) = hueco_c(c);
    let (lo, hi) = dos(0x023, 4, 5, 0, rd, a, b, sat, control);
    (lo, hi | rc as u64 | mc)
}

/// ** DL10 (09-10): el MODO DE REDONDEO de una FFMA, en 78..80 (el campo
/// `rnd`): lo que pone `ptxas` en `FFMA.RM`, `.RP` y `.RZ` (`ORO_DL10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redondeo {
    /// Al mas cercano, par en el empate (el de siempre, sin sufijo).
    Cercano = 0,
    /// Hacia menos infinito (`.RM`).
    Abajo = 1,
    /// Hacia mas infinito (`.RP`).
    Arriba = 2,
    /// Hacia el cero (`.RZ`).
    Cero = 3,
}

/// `FFMA[.RM|.RP|.RZ] Rd, a, b, c`: [`ffma`] con su modo de redondeo.
pub fn ffma_redondeo(rd: u8, a: Fuente, b: Fuente, c: Fuente, modo: Redondeo, control: u64) -> (u64, u64) {
    let (lo, hi) = ffma(rd, a, b, c, false, control);
    (lo, hi | (modo as u64) << 14)
}

/// `FMNMX Rd, a, b, PT` (el menor) o `!PT` (el mayor).
pub fn fmnmx(rd: u8, a: Fuente, b: Fuente, mayor: bool, control: u64) -> (u64, u64) {
    // El predicado que elige va en 87..91: PT (7), negado +8.
    dos(0x009, 4, 5, (7 | (mayor as u64) << 3) << 23, rd, a, b, false, control)
}

/// Lo que calcula MUFU (su campo, bits 74..78).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mufu {
    Rcp = 4,
    Rsq = 5,
    Sqrt = 8,
}

/// `MUFU.<f> Rd, a` (desacoplada: necesita barrera).
pub fn mufu(rd: u8, f: Mufu, a: u8, control: u64) -> (u64, u64) {
    let lo = 0x108 | 1 << 9 | SIEMPRE | (rd as u64) << 16 | (a as u64) << 32;
    palabra(lo, (f as u64) << 10, control)
}

/// `MOV Rd, fuente` (un registro, un inmediato o una constante). La mascara
/// de carriles 0xF (72..76) la pone `ptxas` en todo MOV.
pub fn mov(rd: u8, f: Fuente, control: u64) -> (u64, u64) {
    let lo_f = match f {
        Fuente::R { r, .. } => 1 << 9 | (r as u64) << 32,
        Fuente::Imm(v) => 4 << 9 | (v as u64) << 32,
        Fuente::C { banco, desp } => 5 << 9 | ((desp as u64 >> 2) & 0x3FFF) << 40 | (banco as u64 & 0x1F) << 54,
    };
    palabra(0x002 | SIEMPRE | (rd as u64) << 16 | lo_f, 0xF << 8, control)
}

/// **`TEX.SCR.B.LZ Rd, Ra, Rb, 2D`** (P3b4c.8): los cuatro canales en
/// `rd..rd+3`, las coordenadas en `ra, ra+1`, el asa en `rb`. Los bits los dio
/// `ptxas -arch=sm_86` y cada campo se releyo con `nvdisasm` (el mismo que
/// `bmo_gpu_ga10x::texturas::tex`, que lo comprueba).
pub fn tex(rd: u8, ra: u8, rb: u8, control: u64) -> (u64, u64) {
    let lo = 0x361 | SIEMPRE | (rd as u64) << 16 | (ra as u64) << 24 | (rb as u64) << 32 | 0x38 << 56;
    palabra(lo, (rd as u64 + 2) | 0xF << 8 | 7 << 17 | 1 << 20 | 1 << 23, control)
}

/// ** E8f (DL18, 09-10): `LDC[.64] Rd, c[banco][Ra + desp]` -- una o dos
/// palabras de un BANCO DE CONSTANTES con el indice en un registro, en BYTES
/// (`RZ`: sin indice). Las palabras de `ptxas` (`ORO_LDC`): el opcode 0x182 en
/// la forma 5, Ra en 24..32, el desplazamiento/4 en 40..54 (hasta 0xFFFC), el
/// banco en 54..59 y el ancho en 73..76 (4: 32 bits; 5: 64). Desacoplada: su
/// barrera la pone quien emite (E4).
pub fn ldc(rd: u8, banco: u8, ra: u8, desp: u16, doble: bool, control: u64) -> (u64, u64) {
    debug_assert!(desp % 4 == 0 && banco < 32, "LDC: el desplazamiento en palabras, el banco de 0 a 31");
    let lo = 0x182 | 5 << 9 | SIEMPRE | (rd as u64) << 16 | (ra as u64) << 24 | ((desp as u64 >> 2) & 0x3FFF) << 40 | (banco as u64 & 0x1F) << 54;
    palabra(lo, (if doble { 5 } else { 4 }) << 9, control)
}

/// `EXIT` (con su predicado PT en 87..90, como lo pone `ptxas`).
pub fn exit(control: u64) -> (u64, u64) {
    palabra(0x14D | 4 << 9 | SIEMPRE, 7 << 23, control)
}

/// ** DL12 (09-10): `[@[!]Pg] KILL` -- el `discard` de un programa de PIXEL:
/// el hilo acaba y su pixel no se escribe (con la SPH diciendo KillsPixels,
/// el bit 15: sin el, NVIDIA dice que el KILL es un NOP y una excepcion).
/// Como el EXIT: su predicado PT en 87..90; el guarda, 0..6 (P0..P6, +8
/// negado) o 7 (siempre). `ptxas` no la da (solo computo): la leyo `nvdisasm
/// -b SM86` (13.4), [`LEIDAS_DL12`].
pub fn kill(guarda: u8, control: u64) -> (u64, u64) {
    palabra(0x15B | 4 << 9 | (guarda as u64 & 0xF) << 12, 7 << 23, control)
}

// == Comparar, elegir y saltar (E6, 02-10) ===================================
//
// Lo que el emisor necesita para `if` y para los bucles. `ptxas` (12.9) lo
// hace asi para la 3060, sin BSSY/BSYNC aunque los hilos se separen (la
// reconvergencia es rendimiento, no correccion; con TEX.LZ no hay
// derivadas que la pidan):
//
//    FSETP.<cmp>.AND P0, PT, Ra, b, PT     el predicado
//    @!P0 BRA destino                      13 ciclos despues (ver `planifica`)
//
// Los campos nuevos, de las palabras de oro de abajo:
//
//    76..80   la comparacion (`Cmp`)      81..84  el predicado que escribe
//    84..87   el segundo (PT: no se usa)  87..90  el que se combina (PT) o el
//                                                 que elige SEL; 90 = negado
//    12..16   el GUARDA de cualquier instruccion: 7 = siempre (PT), 0..6 = P0..P6,
//             +8 = negado
//    32..82   el desplazamiento de BRA, en BYTES y con signo, desde la siguiente

/// La comparacion de FSETP e ISETP (bits 76..80). Las `U` son de float: se
/// cumplen tambien si alguno es NaN (desordenadas).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Lt = 1,
    Eq = 2,
    Le = 3,
    Gt = 4,
    Ne = 5,
    Ge = 6,
    Ltu = 9,
    Equ = 10,
    Leu = 11,
    Gtu = 12,
    Neu = 13,
    Geu = 14,
}

/// Los predicados del guarda y de SEL: P0..P6, y PT (7).
pub const PT: u8 = 7;

/// Lo comun de FSETP e ISETP: escribe `pu`, combina con PT por AND.
fn setp_hi(como: Cmp, pu: u8) -> u64 {
    (como as u64) << 12 | (pu as u64 & 7) << 17 | (PT as u64) << 20 | (PT as u64) << 23
}

/// `FSETP.<como>.AND Ppu, PT, a, b, PT` (`a` un registro, con `-` y `|x|`).
pub fn fsetp(pu: u8, como: Cmp, a: Fuente, b: Fuente, control: u64) -> (u64, u64) {
    dos(0x00B, 4, 5, setp_hi(como, pu), 0, a, b, false, control)
}

/// `ISETP.<como>[.U32].AND Ppu, PT, Ra, b, PT` -- con signo salvo
/// `sin_signo` (bit 73); el 68..71 es el predicado de .EX, PT.
pub fn isetp(pu: u8, como: Cmp, a: u8, b: Fuente, sin_signo: bool, control: u64) -> (u64, u64) {
    let hi = setp_hi(como, pu) | (PT as u64) << 4 | (!sin_signo as u64) << 9;
    dos(0x00C, 4, 5, hi, 0, r(a), b, false, control)
}

/// `SEL Rd, Ra, b, [!]Pp`: `Rd = Pp ? Ra : b`, los bits tal cual.
pub fn sel(rd: u8, a: u8, b: Fuente, p: u8, negado: bool, control: u64) -> (u64, u64) {
    dos(0x007, 4, 5, (p as u64 & 7) << 23 | (negado as u64) << 26, rd, r(a), b, false, control)
}

/// `IADD3 Rd, Ra, b, RZ`: la suma entera (modulo 2^32). Lo de 77..90 son
/// los predicados de acarreo, `!PT`/PT como los pone `ptxas`.
pub fn iadd3(rd: u8, a: u8, b: Fuente, control: u64) -> (u64, u64) {
    dos(0x010, 4, 5, 0x07ff_e000 | RZ as u64, rd, r(a), b, false, control)
}

/// `[@[!]Pg] BRA destino`: `guarda` 0..6 (P0..P6, +8 negado) o 7 (siempre);
/// `desplazamiento` en BYTES desde la instruccion SIGUIENTE (multiplo de 16).
pub fn bra(guarda: u8, desplazamiento: i64, control: u64) -> (u64, u64) {
    let d = desplazamiento as u64;
    let lo = 0x147 | 4 << 9 | (guarda as u64 & 0xF) << 12 | d << 32;
    palabra(lo, (PT as u64) << 23 | (d >> 32) & 0x3_FFFF, control)
}

// == Los enteros y las conversiones (E6c, 02-10) ==============================
//
// De `ptxas` 12.9 (`oro_enteros.ptx`): `mul.lo` es IMAD (el bit 73, con
// signo), `and/or/xor` son LOP3 con su tabla (0xC0, 0xFC, 0x3C), `shl` es
// SHF.L.U32 y `shr` es SHF.R.{U32,S32}.HI (el valor en el 64..72, la cuenta
// en la segunda fuente), `min/max` son IMNMX (PT el menor, !PT el mayor; el
// bit 73, con signo). Las conversiones: F2I.TRUNC.NTZ (desacoplada; con
// signo, el bit 72) y, de entero a float, I2F -- desacoplada, con barrera --
// aunque `ptxas` ponga en la 3060 I2FP (acoplada, opcode 0x45): de I2FP no
// se tiene su latencia de NAK, y una espera mal puesta es un pixel al azar.
// I2F con RN es la de `ptxas` con su redondeo (78..80) a 0, y `nvdisasm` la
// lee (`LEIDAS_E6C`).

/// `IMAD Rd, Ra, b, Rc`: `Ra * b + Rc` (modulo 2^32), con signo (73).
pub fn imad(rd: u8, a: u8, b: Fuente, c: u8, control: u64) -> (u64, u64) {
    dos(0x024, 4, 5, 0x078e_0200 | c as u64, rd, r(a), b, false, control)
}

/// La tabla de LOP3 para `a & b`, `a | b` y `a ^ b` (con `c` = RZ).
pub const Y: u8 = 0xC0;
pub const O: u8 = 0xFC;
pub const OX: u8 = 0x3C;

/// `LOP3.LUT Rd, Ra, b, RZ, lut, !PT`.
pub fn lop3(rd: u8, a: u8, b: Fuente, lut: u8, control: u64) -> (u64, u64) {
    dos(0x012, 4, 5, 0x078e_0000 | (lut as u64) << 8 | RZ as u64, rd, r(a), b, false, control)
}

/// `SHF.L.U32 Rd, Ra, n, RZ`: `Ra << n`.
pub fn shl(rd: u8, a: u8, n: Fuente, control: u64) -> (u64, u64) {
    dos(0x019, 4, 5, 0x0600 | RZ as u64, rd, r(a), n, false, control)
}

/// `SHF.R.{U32,S32}.HI Rd, RZ, n, Ra`: `Ra >> n`, logico o aritmetico.
pub fn shr(rd: u8, a: u8, n: Fuente, con_signo: bool, control: u64) -> (u64, u64) {
    let tipo = if con_signo { 0x0400 } else { 0x0600 };
    dos(0x019, 4, 5, 0x0001_1000 | tipo | a as u64, rd, r(RZ), n, false, control)
}

/// `IMNMX[.U32] Rd, Ra, b, PT|!PT`: el menor (PT) o el mayor (!PT).
pub fn imnmx(rd: u8, a: u8, b: Fuente, mayor: bool, con_signo: bool, control: u64) -> (u64, u64) {
    dos(0x017, 4, 5, (7 | (mayor as u64) << 3) << 23 | (con_signo as u64) << 9, rd, r(a), b, false, control)
}

/// `I2F[.U32] Rd, Ra`: de entero a float, al mas cercano (desacoplada).
pub fn i2f(rd: u8, a: u8, con_signo: bool, control: u64) -> (u64, u64) {
    palabra(0x106 | 1 << 9 | SIEMPRE | (rd as u64) << 16 | (a as u64) << 32, 0x20_1000 | (con_signo as u64) << 10, control)
}

/// `F2I[.U32].TRUNC.NTZ Rd, Ra`: de float a entero, hacia cero (desacoplada).
pub fn f2i(rd: u8, a: u8, con_signo: bool, control: u64) -> (u64, u64) {
    palabra(0x105 | 1 << 9 | SIEMPRE | (rd as u64) << 16 | (a as u64) << 32, 0x20_f000 | (con_signo as u64) << 8, control)
}

/// Las PALABRAS DE ORO de E6c (02-10): `ptxas -arch=sm_86 -O3` (CUDA 12.9)
/// sobre `ga10x/sombreadores/oro_enteros.ptx`, leido con `nvdisasm -hex`.
pub const ORO_E6C: &[(&str, u64, u64)] = &[
    ("IMAD R9, R0.reuse, R7.reuse, RZ", 0x0000000700097224, 0x0c0fe200078e02ff),
    ("IMAD R11, R0.reuse, 0x7, RZ", 0x00000007000b7824, 0x048fe200078e02ff),
    ("LOP3.LUT R13, R0.reuse, R7.reuse, RZ, 0xc0, !PT", 0x00000007000d7212, 0x0d0fe200078ec0ff),
    ("LOP3.LUT R15, R0.reuse, R7.reuse, RZ, 0xfc, !PT", 0x00000007000f7212, 0x0c0fe200078efcff),
    ("LOP3.LUT R5, R0, R7.reuse, RZ, 0x3c, !PT", 0x0000000700057212, 0x080fe400078e3cff),
    ("LOP3.LUT R17, R0, 0x1f, RZ, 0xc0, !PT", 0x0000001f00117812, 0x000fcc00078ec0ff),
    ("SHF.L.U32 R19, R0.reuse, R7.reuse, RZ", 0x0000000700137219, 0x0c1fe200000006ff),
    ("SHF.R.U32.HI R21, RZ, R7.reuse, R0.reuse", 0x00000007ff157219, 0x180fe20000011600),
    ("SHF.R.S32.HI R23, RZ, R7, R0", 0x00000007ff177219, 0x000fc60000011400),
    ("SHF.R.U32.HI R21, RZ, 0x5, R0", 0x00000005ff157819, 0x004fc60000011600),
    ("SHF.R.S32.HI R7, RZ, 0x6, R7", 0x00000006ff077819, 0x000fe20000011407),
    ("IMNMX R11, R0.reuse, R7.reuse, PT", 0x00000007000b7217, 0x0c4fe40003800200),
    ("IMNMX R13, R0, R7, !PT", 0x00000007000d7217, 0x008fc60007800200),
    ("IMNMX.U32 R15, R0, R7, PT", 0x00000007000f7217, 0x001fc60003800000),
    ("IMNMX.U32 R25, R0, R7, !PT", 0x0000000700197217, 0x002fc60007800000),
    ("F2I.TRUNC.NTZ R19, R6", 0x0000000600137305, 0x004e22000020f100),
    ("F2I.U32.TRUNC.NTZ R25, R6", 0x0000000600197305, 0x000e62000020f000),
];

// == La division de enteros (E6d, 02-10) =====================================
//
// La 3060 no divide enteros: `ptxas` (`oro_division.ptx`) saca un inverso
// aproximado de `b` en float (I2F.U32.RP, redondeo hacia arriba: 2 en el
// 78..80; y MUFU.RCP), lo pasa a entero (F2I.FTZ.U32.TRUNC: FTZ, el bit 80),
// lo afina con IMAD.HI.U32 (la mitad ALTA de un producto de 32 x 32) y
// corrige el cociente dos veces. Con signo, sobre IABS.

/// `I2F[.U32].RP Rd, Ra`: de entero a float, redondeando hacia arriba.
pub fn i2f_arriba(rd: u8, a: u8, con_signo: bool, control: u64) -> (u64, u64) {
    let (lo, hi) = i2f(rd, a, con_signo, control);
    (lo, hi | 2 << 14)
}

/// `F2I.FTZ[.U32].TRUNC.NTZ Rd, Ra`.
pub fn f2i_ftz(rd: u8, a: u8, con_signo: bool, control: u64) -> (u64, u64) {
    let (lo, hi) = f2i(rd, a, con_signo, control);
    (lo, hi | 1 << 16)
}

/// `IMAD.HI.U32 Rd, Ra, b, Rc`: la mitad alta de `Ra * b` (sin signo), con
/// `Rc` = RZ (con otro, `ptxas` suma el PAR `Rc+1:Rc`: aqui no se usa).
pub fn imad_hi(rd: u8, a: u8, b: Fuente, control: u64) -> (u64, u64) {
    dos(0x027, 4, 5, 0x078e_0000 | RZ as u64, rd, r(a), b, false, control)
}

/// `IABS Rd, Ra`: el valor absoluto de un entero (de i32::MIN, el mismo).
pub fn iabs(rd: u8, a: u8, control: u64) -> (u64, u64) {
    palabra(0x013 | 1 << 9 | SIEMPRE | (rd as u64) << 16 | (a as u64) << 32, 0, control)
}

/// `IADD3 Rd, Pu, Ra, b, RZ`: la suma, y su ACARREO (el bit 32) a `Pu` (el
/// 81..84). Con `b` negado es `Ra - b`, y el acarreo dice `Ra >= b` sin
/// signo (lo que `ptxas` saca de `sub.cc`).
pub fn iadd3_acarreo(rd: u8, pu: u8, a: u8, b: Fuente, control: u64) -> (u64, u64) {
    dos(0x010, 4, 5, (0x07ff_e000 & !(7 << 17)) | (pu as u64 & 7) << 17 | RZ as u64, rd, r(a), b, false, control)
}

/// `IADD3.X Rd, Ra, b, RZ, Pp, !PT`: la suma MAS el acarreo de `Pp` (el
/// 87..91; .X es el bit 74).
pub fn iadd3_x(rd: u8, a: u8, b: Fuente, pp: u8, control: u64) -> (u64, u64) {
    dos(0x010, 4, 5, (0x07ff_e000 & !(0xF << 23)) | (pp as u64 & 7) << 23 | 1 << 10 | RZ as u64, rd, r(a), b, false, control)
}

/// Las PALABRAS DE ORO de E6d (02-10): `ptxas -arch=sm_86 -O3` sobre
/// `ga10x/sombreadores/oro_division.ptx` (div y rem, u32 y s32; `mul.hi`
/// con un inmediato, el de dividir por una constante; y el acarreo).
pub const ORO_E6D: &[(&str, u64, u64)] = &[
    ("I2F.U32.RP R6, R7", 0x0000000700067306, 0x004e220000209000),
    ("I2F.RP R6, R9", 0x0000000900067306, 0x000e220000209400),
    ("IADD3 R4, R6, 0xffffffe, RZ", 0x0ffffffe06047810, 0x001fcc0007ffe0ff),
    ("F2I.FTZ.U32.TRUNC.NTZ R5, R4", 0x0000000400057305, 0x000064000021f000),
    ("IMAD R9, R9, R5, RZ", 0x0000000509097224, 0x002fc800078e02ff),
    ("IMAD.HI.U32 R5, R5, R0, RZ", 0x0000000005057227, 0x008fc800078e00ff),
    ("IMAD R0, R7, R2, R0", 0x0000000207007224, 0x000fe400078e0200),
    ("IABS R9, R7.reuse", 0x0000000700097213, 0x084fe40000000000),
    ("IABS R10, R0", 0x00000000000a7213, 0x008fc40000000000),
    // `mulhi`: la de la division por una constante, con su inmediato.
    ("IMAD.HI.U32 R0, R2, 0x24924925, RZ", 0x2492492502007827, 0x004fc800078e00ff),
    ("IMAD.HI.U32 R9, R2, -0x33333333, RZ", 0xcccccccd02097827, 0x000fe200078e00ff),
    // `acarreo`: el de las correcciones.
    ("IADD3 R13, P1, R0, R6, RZ", 0x00000006000d7210, 0x004fc40007f3e0ff),
    ("IADD3 R9, P0, R0, -R7, RZ", 0x8000000700097210, 0x008fc60007f1e0ff),
    ("IADD3.X R11, RZ, R6, RZ, P0, !PT", 0x00000006ff0b7210, 0x000fe200007fe4ff),
];

/// Las PALABRAS DE ORO de DL10 (09-10): `ptxas -arch=sm_86 -O3` (CUDA 12.9)
/// sobre `oro_reales.ptx` (al lado de este crate), leido con `cuobjdump
/// -sass` (13.4): las FFMA de `div.rn.f32` -- su cuenta y su redondeo hacia
/// el cero, abajo y arriba, el de un cociente subnormal -- y su MUFU.RCP.
pub const ORO_DL10: &[(&str, u64, u64)] = &[
    ("MUFU.RCP R4, R5", 0x0000000500047308, 0x004e300000001000),
    ("FFMA R7, R4, R7, R4", 0x0000000704077223, 0x000fc80000000004),
    ("FFMA R4, R0, R7, RZ", 0x0000000700047223, 0x000fc800000000ff),
    ("FFMA R6, -R5, R4, R0", 0x0000000405067223, 0x000fc80000000100),
    ("FFMA.RZ R0, R12, R10.reuse, R11.reuse", 0x0000000a0c007223, 0x180fe2000000c00b),
    ("FFMA.RM R3, R12, R10.reuse, R11.reuse", 0x0000000a0c037223, 0x180fe2000000400b),
    ("FFMA.RP R0, R12, R10, R11", 0x0000000a0c007223, 0x000fe2000000800b),
];

/// Las PALABRAS DE ORO del LDC con indice (E8f, DL18): `ptxas -arch=sm_86 -O3`
/// (CUDA 12.9) sobre `oro_ldc.ptx` (al lado de este crate), leido con
/// `nvdisasm -hex` (13.4): un `.const` leido con la fila en un registro.
pub const ORO_LDC: &[(&str, u64, u64)] = &[
    ("LDC.64 R4, c[0x3][R0+0x20]", 0x00c0080000047b82, 0x000e300000000a00),
    ("LDC.64 R6, c[0x3][R0+0x28]", 0x00c00a0000067b82, 0x000e300000000a00),
    ("LDC R9, c[0x3][R0+0x4]", 0x00c0010000097b82, 0x000e620000000800),
];

/// Las PALABRAS DE ORO de la FFMA con el INMEDIATO EN LA c (la forma 2; R7 la
/// deja desde el 09-10, por decision del propietario): `ptxas -arch=sm_86
/// -O3` (CUDA 12.9) sobre `oro_ffma_c.ptx` (al lado de este crate), leido con
/// `nvdisasm -hex` (13.4). La `b`, con sus `-` y `|x|`, en el hueco de la c.
pub const ORO_FFMA_C: &[(&str, u64, u64)] = &[
    ("FFMA R7, R0, -R5, 0.5", 0x3f00000000077423, 0x004fc80000000805),
    ("FFMA R7, |R4|, R7, -0.3333333432674407959", 0xbeaaaaab04077423, 0x008fc80000000207),
    ("FFMA R7, R0, -R7, 3.1415927410125732422", 0x40490fdb00077423, 0x000fc80000000807),
    ("FFMA.SAT R4, R7, R7, 0.10000000149011611938", 0x3dcccccd07047423, 0x000fc80000002007),
    ("FFMA R5, R5, -|R4|, -100", 0xc2c8000005057423, 0x000fc80000000c04),
    ("FFMA R5, R0, R5, 0.5", 0x3f00000000057423, 0x000fca0000000005),
];

/// El control de una de ALU (el de `ptxas` y del driver: 6 ciclos, el bit 4,
/// sin barreras): el de las combinaciones leidas por `nvdisasm`.
pub const ALU: u64 = 6 | 1 << 4 | 7 << 5 | 7 << 8;

/// `NOP`.
pub fn nop(control: u64) -> (u64, u64) {
    palabra(0x118 | 4 << 9 | SIEMPRE, 0, control)
}

/// Las PALABRAS DE ORO: `ptxas -arch=sm_86 -O3` (CUDA 12.9) sobre
/// `ga10x/sombreadores/oro_codifica.ptx`, leido con `nvdisasm -hex` (13.4),
/// 28-09. El texto es el de `nvdisasm`, tal cual. Publicas: el juez del
/// driver (J1) las lee en sus pruebas.
pub const ORO: &[(&str, u64, u64)] = &[
    ("MOV R1, c[0x0][0x28]", 0x00000a0000017a02, 0x000fe40000000f00),
    ("MOV R6, 0x4b800000", 0x4b80000000067802, 0x000fe40000000f00),
    ("FMUL R15, R0.reuse, 16777216", 0x4b800000000f7820, 0x048fe20000400000),
    ("FMUL R8, R9, R6", 0x0000000609087220, 0x000fe20000400000),
    ("MUFU.RSQ R27, R15", 0x0000000f001b7308, 0x0000700000001400),
    ("MUFU.SQRT R29, R4", 0x00000004001d7308, 0x000eb00000002000),
    ("MUFU.RCP R31, R8", 0x00000008001f7308, 0x000ee20000001000),
    ("FADD R11, R0.reuse, R7.reuse", 0x00000007000b7221, 0x140fe20000000000),
    ("FADD R13, R0.reuse, -R7", 0x80000007000d7221, 0x040fe20000000000),
    ("FMUL R5, R0.reuse, R7.reuse", 0x0000000700057220, 0x0c0fe20000400000),
    ("FMUL.SAT R15, R0.reuse, R7.reuse", 0x00000007000f7220, 0x0c1fe20000402000),
    ("FMNMX R23, R0.reuse, R7.reuse, PT", 0x0000000700177209, 0x0c0fe20003800000),
    ("FMNMX R25, R0.reuse, R7, !PT", 0x0000000700197209, 0x040fe20007800000),
    ("FADD R17, R0, 1", 0x3f80000000117421, 0x000fc40000000000),
    ("FMUL R19, R0.reuse, 3.1415927410125732422", 0x40490fdb00137820, 0x040fe20000400000),
    ("FADD R21, R0, c[0x0][0x170]", 0x00005c0000157621, 0x000fe40000000000),
    ("FMUL R31, R6, R31", 0x0000001f061f7220, 0x008fe20000400000),
    ("FFMA R11, R0.reuse, R7, R9.reuse", 0x00000007000b7223, 0x141fe20000000009),
    ("FFMA R9, R0.reuse, c[0x0][0x170], R9", 0x00005c0000097a23, 0x040fe20000000009),
    ("FADD.SAT R13, R0.reuse, R7.reuse", 0x00000007000d7221, 0x150fe20000002000),
    ("FADD R5, |R0|, -R7", 0x8000000700057221, 0x020fe20000000200),
    ("FMUL R7, R7, c[0x0][0x170]", 0x00005c0007077a20, 0x000fe20000400000),
    ("EXIT", 0x000000000000794d, 0x000fea0003800000),
    ("NOP", 0x0000000000007918, 0x000fc00000000000),
];

/// Combinaciones que `ptxas` NO dio (otras formas, otros modificadores, otros
/// bancos): las fabrica ESTE codificador y `nvdisasm -b SM86` (13.4) las lee
/// como dice el texto (28-09). Es el criterio de E2: lo que se fabrica,
/// NVIDIA lo lee de vuelta.
pub const LEIDAS: &[(&str, u64, u64)] = &[
        ("FFMA R2, R3, 0.5, R4", 0x3f00000003027823, 0x000fec0000000004),
        ("MOV R10, R11", 0x0000000b000a7202, 0x000fec0000000f00),
        ("FFMA.SAT R5, -R6, R7, -R8", 0x0000000706057223, 0x000fec0000002908),
        ("MUFU.RSQ R12, R13", 0x0000000d000c7308, 0x000fec0000001400),
        ("FMNMX R1, |R2|, c[0x3][0x40], !PT", 0x00c0100002017a09, 0x000fec0007800200),
        ("FADD.SAT R9, -R1, |R2|", 0x4000000201097221, 0x000fec0000002100),
        ("FMUL R3, |R4|, -R5", 0x8000000504037220, 0x000fec0000400200),
        ("FFMA R6, R7, c[0x0][0x1fc], |R8|", 0x00007f0007067a23, 0x000fec0000000408),
        ("MOV R0, c[0x1][0x10]", 0x0040040000007a02, 0x000fec0000000f00),
        ("EXIT", 0x000000000000794d, 0x000fec0003800000),
        // 09-10, la forma 2: la b solo con `|x|`, y las dos negadas.
        ("FFMA R1, R2, |R3|, 2", 0x4000000002017423, 0x000fec0000000403),
        ("FFMA R1, -R2, -R3, -1", 0xbf80000002017423, 0x000fec0000000903),
        // E8f: el LDC sin indice (RZ), y con otro banco y el desplazamiento
        // mas alto que cabe.
        ("LDC R1, c[0x3][0x10]", 0x00c00400ff017b82, 0x000fec0000000800),
        ("LDC.64 R2, c[0x1][R5+0x7ff8]", 0x005ffe0005027b82, 0x000fec0000000a00),
];

/// Las PALABRAS DE ORO de E6 (02-10): `ptxas -arch=sm_86 -O3` (CUDA 12.9)
/// sobre `ga10x/sombreadores/oro_saltos.ptx`, leido con `nvdisasm -hex`
/// (13.4). Comparar, elegir, sumar enteros y saltar, con su control.
pub const ORO_E6: &[(&str, u64, u64)] = &[
    ("FSETP.GEU.AND P0, PT, R0, R5, PT", 0x000000050000720b, 0x004fda0003f0e000),
    ("FSETP.GE.AND P5, PT, R7, R0, PT", 0x000000000700720b, 0x000fc40003fa6000),
    ("FSETP.NEU.AND P6, PT, R7, R0, PT", 0x000000000700720b, 0x000fe20003fcd000),
    ("FSETP.GT.AND P0, PT, R3, 100, PT", 0x42c800000300780b, 0x000fda0003f04000),
    ("FSETP.GT.AND P1, PT, R7.reuse, c[0x0][0x170], PT", 0x00005c0007007a0b, 0x040fe40003f24000),
    ("FSETP.GEU.AND P0, PT, |R0|, 1.175494350822287508e-38, PT", 0x008000000000780b, 0x000fc80003f0e200),
    ("FSETP.GTU.AND P3, PT, R7.reuse, R0.reuse, PT", 0x000000000700720b, 0x0c0fe40003f6c000),
    ("ISETP.GT.AND P0, PT, R4, RZ, PT", 0x000000ff0400720c, 0x020fe20003f04270),
    ("ISETP.GE.AND P2, PT, R6, 0x7, PT", 0x000000070600780c, 0x000fc60003f46270),
    ("ISETP.NE.AND P1, PT, R6.reuse, R9, PT", 0x000000090600720c, 0x040fe40003f25270),
    ("SEL R11, R9, R6, P3", 0x00000006090b7207, 0x000fe20001800000),
    ("SEL R3, RZ, 0xffffffff, P3", 0xffffffffff037807, 0x000fe20001800000),
    ("IADD3 R9, R9, 0x5, RZ", 0x0000000509097810, 0x000fe20007ffe0ff),
    ("@!P0 BRA `(.L_x_0)", 0x0000007000008947, 0x000fea0003800000),
    ("@!P0 BRA `(.L_x_3)", 0xffffffa000008947, 0x000fea000383ffff),
    ("@P0 BRA `(.L_x_2)", 0x0000003000000947, 0x000fea0003800000),
    ("BRA `(.L_x_1)", 0x0000005000007947, 0x000fea0003800000),
    ("BRA `(.L_x_0)", 0xfffffff000007947, 0x000fc0000383ffff),
];

/// Lo que `ptxas` NO dio de E6 y fabrica ESTE codificador, leido por
/// `nvdisasm -b SM86` (13.4) como dice el texto (02-10): las otras
/// comparaciones, `.U32`, los modificadores, c[][], el predicado negado de
/// SEL, IADD3 de registro y de constante, y saltos a ambos lados con otros
/// guardas (el texto de BRA dice donde estaba y a donde lo leyo `nvdisasm`).
pub const LEIDAS_E6: &[(&str, u64, u64)] = &[
    ("FSETP.LT.AND P0, PT, R1, R2, PT", 0x000000020100720b, 0x000fec0003f01000),
    ("FSETP.LE.AND P1, PT, R3, 1, PT", 0x3f8000000300780b, 0x000fec0003f23000),
    ("FSETP.EQ.AND P2, PT, -R4, c[0x3][0x10], PT", 0x00c0040004007a0b, 0x000fec0003f42100),
    ("FSETP.NE.AND P3, PT, R5, |R6|, PT", 0x400000060500720b, 0x000fec0003f65000),
    ("FSETP.GE.AND P4, PT, R7, -R8, PT", 0x800000080700720b, 0x000fec0003f86000),
    ("FSETP.LTU.AND P6, PT, R9, R10, PT", 0x0000000a0900720b, 0x000fec0003fc9000),
    ("ISETP.LT.AND P0, PT, R1, R2, PT", 0x000000020100720c, 0x000fec0003f01270),
    ("ISETP.LE.AND P1, PT, R3, -0x1, PT", 0xffffffff0300780c, 0x000fec0003f23270),
    ("ISETP.EQ.AND P2, PT, R4, c[0x3][0x20], PT", 0x00c0080004007a0c, 0x000fec0003f42270),
    ("ISETP.LT.U32.AND P5, PT, R6, R7, PT", 0x000000070600720c, 0x000fec0003fa1070),
    ("ISETP.NE.AND P0, PT, R12, RZ, PT", 0x000000ff0c00720c, 0x000fec0003f05270),
    ("SEL R1, R2, R3, !P0", 0x0000000302017207, 0x000fec0004000000),
    ("SEL R4, RZ, 0xffffffff, !P2", 0xffffffffff047807, 0x000fec0005000000),
    ("SEL R5, R6, c[0x3][0x30], P6", 0x00c00c0006057a07, 0x000fec0003000000),
    ("IADD3 R7, R8, R9, RZ", 0x0000000908077210, 0x000fec0007ffe0ff),
    ("IADD3 R10, R11, c[0x3][0x40], RZ", 0x00c010000b0a7a10, 0x000fec0007ffe0ff),
    ("IADD3 R12, R13, -0x1, RZ", 0xffffffff0d0c7810, 0x000fec0007ffe0ff),
    ("@P0 BRA +0x100 (en 0x110: a 0x220)", 0x0000010000000947, 0x000fec0003800000),
    ("@!P0 BRA -0x200 (en 0x120: a -0xd0)", 0xfffffe0000008947, 0x000fec000383ffff),
    ("@P3 BRA +0x10 (en 0x130: a 0x150)", 0x0000001000003947, 0x000fec0003800000),
    ("BRA -0x70000 (en 0x140: a -0x6feb0)", 0xfff9000000007947, 0x000fec000383ffff),
];

/// ** DL12 (09-10): el KILL, fabricado aqui con el control de [`ALU`] y
/// leido por `nvdisasm -b SM86` (13.4) como dice el texto: sin guarda y con
/// los de P0, !P0, P3 y !P5.
pub const LEIDAS_DL12: &[(&str, u64, u64)] = &[
    ("KILL", 0x000000000000795b, 0x000fec0003800000),
    ("@P0 KILL", 0x000000000000095b, 0x000fec0003800000),
    ("@!P0 KILL", 0x000000000000895b, 0x000fec0003800000),
    ("@P3 KILL", 0x000000000000395b, 0x000fec0003800000),
    ("@!P5 KILL", 0x000000000000d95b, 0x000fec0003800000),
];

/// Lo que `ptxas` NO dio de E6c, fabricado aqui y leido por `nvdisasm -b
/// SM86` (13.4) como dice el texto (02-10): I2F al mas cercano (con y sin
/// signo), IADD3 restando un registro, y otras formas de las de arriba.
pub const LEIDAS_E6C: &[(&str, u64, u64)] = &[
    ("I2F R1, R2", 0x0000000200017306, 0x000fec0000201400),
    ("I2F.U32 R3, R4", 0x0000000400037306, 0x000fec0000201000),
    ("IADD3 R5, R6, -R7, RZ", 0x8000000706057210, 0x000fec0007ffe0ff),
    ("IMAD R8, R9, -0x1, RZ", 0xffffffff09087824, 0x000fec00078e02ff),
    ("LOP3.LUT R10, R11, 0xffffffff, RZ, 0x3c, !PT", 0xffffffff0b0a7812, 0x000fec00078e3cff),
    ("SHF.L.U32 R12, R13, 0x3, RZ", 0x000000030d0c7819, 0x000fec00000006ff),
    ("IMNMX R14, R15, 0x5, !PT", 0x000000050f0e7817, 0x000fec0007800200),
    ("F2I.TRUNC.NTZ R16, R17", 0x0000001100107305, 0x000fec000020f100),
    ("LOP3.LUT R18, R19, R20, RZ, 0xc0, !PT", 0x0000001413127212, 0x000fec00078ec0ff),
];

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(banco: u8, desp: u16) -> Fuente {
        Fuente::C { banco, desp }
    }

    /// Cada codificador, con el control de su palabra de oro, da la palabra
    /// de oro ENTERA: los 128 bits.
    #[test]
    fn cada_codificador_da_la_palabra_de_ptxas() {
        let pi = 3.141_592_7f32.to_bits();
        let hechas: [(u64, u64); 24] = {
            let k = |i: usize| ORO[i].2 >> 41;
            [
                mov(1, c(0, 0x28), k(0)),
                mov(6, Fuente::Imm(0x4b80_0000), k(1)),
                fmul(15, r(0), Fuente::Imm(0x4b80_0000), false, k(2)),
                fmul(8, r(9), r(6), false, k(3)),
                mufu(27, Mufu::Rsq, 15, k(4)),
                mufu(29, Mufu::Sqrt, 4, k(5)),
                mufu(31, Mufu::Rcp, 8, k(6)),
                fadd(11, r(0), r(7), false, k(7)),
                fadd(13, r(0), neg(7), false, k(8)),
                fmul(5, r(0), r(7), false, k(9)),
                fmul(15, r(0), r(7), true, k(10)),
                fmnmx(23, r(0), r(7), false, k(11)),
                fmnmx(25, r(0), r(7), true, k(12)),
                fadd(17, r(0), Fuente::Imm(1.0f32.to_bits()), false, k(13)),
                fmul(19, r(0), Fuente::Imm(pi), false, k(14)),
                fadd(21, r(0), c(0, 0x170), false, k(15)),
                fmul(31, r(6), r(31), false, k(16)),
                ffma(11, r(0), r(7), r(9), false, k(17)),
                ffma(9, r(0), c(0, 0x170), r(9), false, k(18)),
                fadd(13, r(0), r(7), true, k(19)),
                fadd(5, abs(0), neg(7), false, k(20)),
                fmul(7, r(7), c(0, 0x170), false, k(21)),
                exit(k(22)),
                nop(k(23)),
            ]
        };
        for ((texto, lo, hi), h) in ORO.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }

    #[test]
    fn nvdisasm_lee_lo_que_fabrica_el_codificador() {
        let k = ALU;
        let c = |banco, desp| Fuente::C { banco, desp };
        let hechas = [
            ffma(2, r(3), Fuente::Imm(0.5f32.to_bits()), r(4), false, k),
            mov(10, r(11), k),
            ffma(5, neg(6), r(7), neg(8), true, k),
            mufu(12, Mufu::Rsq, 13, k),
            fmnmx(1, abs(2), c(3, 0x40), true, k),
            fadd(9, neg(1), abs(2), true, k),
            fmul(3, abs(4), neg(5), false, k),
            ffma(6, r(7), c(0, 0x1fc), abs(8), false, k),
            mov(0, c(1, 0x10), k),
            exit(k),
            ffma(1, r(2), abs(3), Fuente::Imm(2.0f32.to_bits()), false, k),
            ffma(1, neg(2), neg(3), Fuente::Imm((-1.0f32).to_bits()), false, k),
            ldc(1, 3, RZ, 0x10, false, k),
            ldc(2, 1, 5, 0x7ff8, true, k),
        ];
        for ((texto, lo, hi), h) in LEIDAS.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
        }
    }
    /// E6: cada codificador nuevo da la palabra de oro ENTERA de `ptxas`.
    #[test]
    fn comparar_elegir_sumar_y_saltar_como_ptxas() {
        let k = |i: usize| ORO_E6[i].2 >> 41;
        let hechas: [(u64, u64); 18] = [
            fsetp(0, Cmp::Geu, r(0), r(5), k(0)),
            fsetp(5, Cmp::Ge, r(7), r(0), k(1)),
            fsetp(6, Cmp::Neu, r(7), r(0), k(2)),
            fsetp(0, Cmp::Gt, r(3), Fuente::Imm(100.0f32.to_bits()), k(3)),
            fsetp(1, Cmp::Gt, r(7), c(0, 0x170), k(4)),
            fsetp(0, Cmp::Geu, abs(0), Fuente::Imm(0x0080_0000), k(5)),
            fsetp(3, Cmp::Gtu, r(7), r(0), k(6)),
            isetp(0, Cmp::Gt, 4, r(RZ), false, k(7)),
            isetp(2, Cmp::Ge, 6, Fuente::Imm(7), false, k(8)),
            isetp(1, Cmp::Ne, 6, r(9), false, k(9)),
            sel(11, 9, r(6), 3, false, k(10)),
            sel(3, RZ, Fuente::Imm(0xFFFF_FFFF), 3, false, k(11)),
            iadd3(9, 9, Fuente::Imm(5), k(12)),
            bra(8, 0x70, k(13)),
            bra(8, -0x60, k(14)),
            bra(0, 0x30, k(15)),
            bra(PT, 0x50, k(16)),
            bra(PT, -0x10, k(17)),
        ];
        for ((texto, lo, hi), h) in ORO_E6.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }
    /// E6: lo fabricado, con el control de `ALU`, es lo que leyo `nvdisasm`.
    #[test]
    fn nvdisasm_lee_lo_que_fabrica_e6() {
        let k = ALU;
        let hechas = [
            fsetp(0, Cmp::Lt, r(1), r(2), k),
            fsetp(1, Cmp::Le, r(3), Fuente::Imm(0x3f80_0000), k),
            fsetp(2, Cmp::Eq, neg(4), c(3, 0x10), k),
            fsetp(3, Cmp::Ne, r(5), abs(6), k),
            fsetp(4, Cmp::Ge, r(7), neg(8), k),
            fsetp(6, Cmp::Ltu, r(9), r(10), k),
            isetp(0, Cmp::Lt, 1, r(2), false, k),
            isetp(1, Cmp::Le, 3, Fuente::Imm(0xFFFF_FFFF), false, k),
            isetp(2, Cmp::Eq, 4, c(3, 0x20), false, k),
            isetp(5, Cmp::Lt, 6, r(7), true, k),
            isetp(0, Cmp::Ne, 12, r(RZ), false, k),
            sel(1, 2, r(3), 0, true, k),
            sel(4, RZ, Fuente::Imm(0xFFFF_FFFF), 2, true, k),
            sel(5, 6, c(3, 0x30), 6, false, k),
            iadd3(7, 8, r(9), k),
            iadd3(10, 11, c(3, 0x40), k),
            iadd3(12, 13, Fuente::Imm(0xFFFF_FFFF), k),
            bra(0, 0x100, k),
            bra(8, -0x200, k),
            bra(3, 0x10, k),
            bra(PT, -0x7_0000, k),
        ];
        assert_eq!(hechas.len(), LEIDAS_E6.len());
        for ((texto, lo, hi), h) in LEIDAS_E6.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
        }
    }
    /// E6c: los enteros y las conversiones, como `ptxas`.
    #[test]
    fn los_enteros_como_ptxas() {
        let k = |i: usize| ORO_E6C[i].2 >> 41;
        let hechas: [(u64, u64); 17] = [
            imad(9, 0, r(7), RZ, k(0)),
            imad(11, 0, Fuente::Imm(7), RZ, k(1)),
            lop3(13, 0, r(7), Y, k(2)),
            lop3(15, 0, r(7), O, k(3)),
            lop3(5, 0, r(7), OX, k(4)),
            lop3(17, 0, Fuente::Imm(0x1f), Y, k(5)),
            shl(19, 0, r(7), k(6)),
            shr(21, 0, r(7), false, k(7)),
            shr(23, 0, r(7), true, k(8)),
            shr(21, 0, Fuente::Imm(5), false, k(9)),
            shr(7, 7, Fuente::Imm(6), true, k(10)),
            imnmx(11, 0, r(7), false, true, k(11)),
            imnmx(13, 0, r(7), true, true, k(12)),
            imnmx(15, 0, r(7), false, false, k(13)),
            imnmx(25, 0, r(7), true, false, k(14)),
            f2i(19, 6, true, k(15)),
            f2i(25, 6, false, k(16)),
        ];
        for ((texto, lo, hi), h) in ORO_E6C.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }
    /// E6c: lo fabricado es lo que leyo `nvdisasm`.
    #[test]
    fn nvdisasm_lee_lo_que_fabrica_e6c() {
        let k = ALU;
        let hechas = [
            i2f(1, 2, true, k),
            i2f(3, 4, false, k),
            iadd3(5, 6, neg(7), k),
            imad(8, 9, Fuente::Imm(0xFFFF_FFFF), RZ, k),
            lop3(10, 11, Fuente::Imm(0xFFFF_FFFF), OX, k),
            shl(12, 13, Fuente::Imm(3), k),
            imnmx(14, 15, Fuente::Imm(5), true, true, k),
            f2i(16, 17, true, k),
            lop3(18, 19, r(20), Y, k),
        ];
        assert_eq!(hechas.len(), LEIDAS_E6C.len());
        for ((texto, lo, hi), h) in LEIDAS_E6C.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
        }
    }
    /// ** DL12: el KILL fabricado es lo que leyo `nvdisasm`.
    #[test]
    fn nvdisasm_lee_el_kill() {
        let hechas = [kill(PT, ALU), kill(0, ALU), kill(8, ALU), kill(3, ALU), kill(8 + 5, ALU)];
        assert_eq!(hechas.len(), LEIDAS_DL12.len());
        for ((texto, lo, hi), h) in LEIDAS_DL12.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
        }
    }
    /// E6d: lo de la division, como `ptxas`.
    #[test]
    fn la_division_como_ptxas() {
        let k = |i: usize| ORO_E6D[i].2 >> 41;
        let hechas: [(u64, u64); 14] = [
            i2f_arriba(6, 7, false, k(0)),
            i2f_arriba(6, 9, true, k(1)),
            iadd3(4, 6, Fuente::Imm(0x0fff_fffe), k(2)),
            f2i_ftz(5, 4, false, k(3)),
            imad(9, 9, r(5), RZ, k(4)),
            imad_hi(5, 5, r(0), k(5)),
            imad(0, 7, r(2), 0, k(6)),
            iabs(9, 7, k(7)),
            iabs(10, 0, k(8)),
            imad_hi(0, 2, Fuente::Imm(0x2492_4925), k(9)),
            imad_hi(9, 2, Fuente::Imm(0xcccc_cccd), k(10)),
            iadd3_acarreo(13, 1, 0, r(6), k(11)),
            iadd3_acarreo(9, 0, 0, neg(7), k(12)),
            iadd3_x(11, RZ, r(6), 0, k(13)),
        ];
        for ((texto, lo, hi), h) in ORO_E6D.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }

    /// ** DL10: la FFMA de la division exacta, con sus cuatro redondeos, y su
    /// inverso: las palabras de `ptxas` para `div.rn.f32`, los 128 bits.
    #[test]
    fn la_division_real_como_ptxas() {
        let k = |i: usize| ORO_DL10[i].2 >> 41;
        let hechas: [(u64, u64); 7] = [
            mufu(4, Mufu::Rcp, 5, k(0)),
            ffma(7, r(4), r(7), r(4), false, k(1)),
            ffma(4, r(0), r(7), r(RZ), false, k(2)),
            ffma(6, neg(5), r(4), r(0), false, k(3)),
            ffma_redondeo(0, r(12), r(10), r(11), Redondeo::Cero, k(4)),
            ffma_redondeo(3, r(12), r(10), r(11), Redondeo::Abajo, k(5)),
            ffma_redondeo(0, r(12), r(10), r(11), Redondeo::Arriba, k(6)),
        ];
        for ((texto, lo, hi), h) in ORO_DL10.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
        assert_eq!(ffma_redondeo(7, r(4), r(7), r(4), Redondeo::Cercano, k(1)), ffma(7, r(4), r(7), r(4), false, k(1)));
    }

    /// ** E8f: el LDC con indice, las palabras de `ptxas`, los 128 bits.
    #[test]
    fn el_ldc_con_indice_como_ptxas() {
        let k = |i: usize| ORO_LDC[i].2 >> 41;
        let hechas = [ldc(4, 3, 0, 0x20, true, k(0)), ldc(6, 3, 0, 0x28, true, k(1)), ldc(9, 3, 0, 0x4, false, k(2))];
        for ((texto, lo, hi), h) in ORO_LDC.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
    }

    /// ** La FFMA con el inmediato en la c (forma 2, 09-10): las palabras de
    /// `ptxas`, los 128 bits, y la de siempre (la c en un registro) igual.
    #[test]
    fn la_ffma_con_la_c_inmediata_como_ptxas() {
        let k = |i: usize| ORO_FFMA_C[i].2 >> 41;
        let imm = |x: f32| Fuente::Imm(x.to_bits());
        let hechas: [(u64, u64); 6] = [
            ffma(7, r(0), neg(5), imm(0.5), false, k(0)),
            ffma(7, abs(4), r(7), Fuente::Imm(0xbeaa_aaab), false, k(1)),
            ffma(7, r(0), neg(7), Fuente::Imm(0x4049_0fdb), false, k(2)),
            ffma(4, r(7), r(7), Fuente::Imm(0x3dcc_cccd), true, k(3)),
            ffma(5, r(5), Fuente::R { r: 4, neg: true, abs: true }, imm(-100.0), false, k(4)),
            ffma(5, r(0), r(5), imm(0.5), false, k(5)),
        ];
        for ((texto, lo, hi), h) in ORO_FFMA_C.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}: {:#018x} {:#018x}", h.0, h.1);
        }
        // La de siempre no cambia: la c en un registro, el inmediato en la b.
        assert_eq!(ffma(11, r(0), r(7), r(9), false, ORO[17].2 >> 41), (ORO[17].1, ORO[17].2));
        assert_eq!(ffma(2, r(3), imm(0.5), r(4), false, ALU), (LEIDAS[0].1, LEIDAS[0].2));
    }
}
