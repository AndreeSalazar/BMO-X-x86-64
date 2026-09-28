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

/// `FFMA[.SAT] Rd, a, b, c` (`c` un registro).
pub fn ffma(rd: u8, a: Fuente, b: Fuente, c: Fuente, sat: bool, control: u64) -> (u64, u64) {
    let (rc, mc) = match c {
        Fuente::R { r, neg, abs } => (r, (abs as u64) << 10 | (neg as u64) << 11),
        _ => (RZ, 0),
    };
    let (lo, hi) = dos(0x023, 4, 5, 0, rd, a, b, sat, control);
    (lo, hi | rc as u64 | mc)
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

/// `EXIT` (con su predicado PT en 87..90, como lo pone `ptxas`).
pub fn exit(control: u64) -> (u64, u64) {
    palabra(0x14D | 4 << 9 | SIEMPRE, 7 << 23, control)
}

/// `NOP`.
pub fn nop(control: u64) -> (u64, u64) {
    palabra(0x118 | 4 << 9 | SIEMPRE, 0, control)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Las PALABRAS DE ORO: `ptxas -arch=sm_86 -O3` (CUDA 12.9) sobre un PTX
    /// hecho para esto, leido con `nvdisasm -hex` (13.4), 28-09. El texto es
    /// el de `nvdisasm`, tal cual.
    const ORO: &[(&str, u64, u64)] = &[
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

    /// Y el juez (J1) las conoce todas: ninguna es "R0 no se".
    #[test]
    fn el_juez_las_conoce() {
        use crate::sass::juez;
        for (texto, lo, hi) in ORO {
            assert!(juez::conoce(*lo, *hi), "{texto}");
        }
    }
}
#[cfg(test)]
mod leidas_por_nvdisasm {
    use super::*;

    /// Combinaciones que `ptxas` NO dio (otras formas, otros modificadores,
    /// otros bancos): las fabrica ESTE codificador y `nvdisasm -b SM86`
    /// (13.4) las lee como dice el texto (28-09). Es el criterio de E2: lo
    /// que se fabrica, NVIDIA lo lee de vuelta.
    const LEIDAS: &[(&str, u64, u64)] = &[
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
    ];

    #[test]
    fn nvdisasm_lee_lo_que_fabrica_el_codificador() {
        let k = crate::trabajos::cubo::ALU;
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
        ];
        for ((texto, lo, hi), h) in LEIDAS.iter().zip(hechas) {
            assert_eq!(h, (*lo, *hi), "{texto}");
            assert!(crate::sass::juez::conoce(*lo, *hi), "{texto}");
        }
    }
}
