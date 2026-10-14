//! **EL TEXTO** -- como se le muestra una instruccion leida a una persona:
//! Intel, en minusculas, con los destinos de los saltos ya sumados.
//!
//! [carril]  VERDE     escribe en un bufer fijo; sin memoria dinamica
//! [consumo] NADA      solo cuando alguien lo pide (una autopsia, una lente)
//!
//! Es una VISTA de lo que se leyo, no lo que se leyo: la lente decide con los
//! campos de [`crate::Instr`], y esto solo los dice.

use core::fmt::Write;

use crate::tabla::CC;
use crate::{Ext, Forma, Instr, Mapa, PF3};

/// El texto de una instruccion, en un bufer fijo (cabe la mas larga).
pub struct Texto {
    buf: [u8; 112],
    n: usize,
}

impl Texto {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.n]).unwrap_or("?")
    }
}

impl Write for Texto {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let cabe = b.len().min(self.buf.len() - self.n);
        self.buf[self.n..self.n + cabe].copy_from_slice(&b[..cabe]);
        self.n += cabe;
        Ok(())
    }
}

const R64: [&str; 16] = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"];
const R32: [&str; 16] = ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "r8d", "r9d", "r10d", "r11d", "r12d", "r13d", "r14d", "r15d"];
const R16: [&str; 16] = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w", "r13w", "r14w", "r15w"];
const R8: [&str; 16] = ["al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b", "r12b", "r13b", "r14b", "r15b"];
const R8_SIN_REX: [&str; 8] = ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"];

impl Instr {
    fn r_rex(&self) -> u8 {
        match self.vex {
            Some(v) => v.r as u8,
            None => (self.rex >> 2) & 1,
        }
    }
    fn x_rex(&self) -> u8 {
        match self.vex {
            Some(v) => v.x as u8,
            None => (self.rex >> 1) & 1,
        }
    }
    fn b_rex(&self) -> u8 {
        match self.vex {
            Some(v) => v.b as u8,
            None => self.rex & 1,
        }
    }
    /// El registro del campo `reg` del ModRM.
    fn reg(&self) -> u8 {
        ((self.modrm.unwrap_or(0) >> 3) & 7) | (self.r_rex() << 3)
    }
    fn mod_(&self) -> u8 {
        self.modrm.unwrap_or(0) >> 6
    }
    /// El registro del campo `rm` (solo si `mod == 3`).
    fn rm_reg(&self) -> u8 {
        (self.modrm.unwrap_or(0) & 7) | (self.b_rex() << 3)
    }
}

fn gpr(n: u8, ancho: u8, hay_rex: bool) -> &'static str {
    let n = (n & 15) as usize;
    match ancho {
        8 => R64[n],
        2 => R16[n],
        1 if !hay_rex && n < 8 => R8_SIN_REX[n],
        1 => R8[n],
        _ => R32[n],
    }
}

fn medida(ancho: u8) -> &'static str {
    match ancho {
        1 => "byte ",
        2 => "word ",
        4 => "dword ",
        8 => "qword ",
        _ => "",
    }
}

fn hex(t: &mut Texto, v: i64) {
    let _ = if v < 0 { write!(t, "-0x{:x}", v.unsigned_abs()) } else { write!(t, "0x{:x}", v) };
}

/// La memoria `[base + indice*escala + disp]`, o el registro si `mod == 3`.
fn rm(t: &mut Texto, i: &Instr, ancho: u8, xmm: Option<bool>) {
    if i.mod_() == 3 {
        let n = i.rm_reg();
        let _ = match xmm {
            Some(y) => write!(t, "{}mm{}", if y { "y" } else { "x" }, n),
            None => t.write_str(gpr(n, ancho, i.rex != 0)),
        };
        return;
    }
    let _ = t.write_str(if xmm.is_some() { "" } else { medida(ancho) });
    let _ = t.write_str("[");
    let mut algo = false;
    if i.rip_relativa {
        let _ = t.write_str("rip");
        algo = true;
    } else if let Some(s) = i.sib {
        let base = (s & 7) | (i.b_rex() << 3);
        let indice = ((s >> 3) & 7) | (i.x_rex() << 3);
        if !(i.mod_() == 0 && s & 7 == 5) {
            let _ = t.write_str(R64[base as usize]);
            algo = true;
        }
        if indice != 4 {
            let _ = write!(t, "{}{}*{}", if algo { "+" } else { "" }, R64[indice as usize], 1 << (s >> 6));
            algo = true;
        }
    } else {
        let _ = t.write_str(R64[i.rm_reg() as usize]);
        algo = true;
    }
    if i.disp != 0 || !algo {
        if algo && i.disp >= 0 {
            let _ = t.write_str("+");
        }
        hex(t, i.disp);
    }
    let _ = t.write_str("]");
}

fn xr(t: &mut Texto, n: u8, y: bool) {
    let _ = write!(t, "{}mm{}", if y { "y" } else { "x" }, n);
}

pub(crate) fn escribe(i: &Instr, en: u64) -> Texto {
    let mut t = Texto { buf: [0; 112], n: 0 };
    let ancho = i.ancho();
    let hay_rex = i.rex != 0;
    let f = i.fila;
    let y = i.vex.is_some_and(|v| v.l);
    // El nombre: los de condicion llevan la suya; algunos cambian con la medida.
    let cc = CC[(i.op & 15) as usize];
    let _ = match (f.nombre, f.forma, i.mapa, i.op) {
        ("j", _, _, _) => write!(t, "j{}", cc),
        ("set", _, _, _) => write!(t, "set{}", cc),
        ("cmov", _, _, _) => write!(t, "cmov{}", cc),
        // `movd` con REX.W mueve 64 bits: el manual lo llama `movq`.
        ("movd", _, Mapa::Dos, _) if ancho == 8 => t.write_str("movq"),
        ("vfmadd231ps", _, _, _) if ancho == 8 => t.write_str("vfmadd231pd"),
        (_, _, Mapa::Uno, 0x98) => t.write_str(if ancho == 8 { "cdqe" } else { "cwde" }),
        (_, _, Mapa::Uno, 0x99) => t.write_str(if ancho == 8 { "cqo" } else { "cdq" }),
        (_, _, Mapa::Uno, 0xA4 | 0xA5 | 0xAA | 0xAB) => {
            let rep = if i.prefijos & PF3 != 0 { "rep " } else { "" };
            let base = if i.op <= 0xA5 { "movs" } else { "stos" };
            let suf = if i.op & 1 == 0 { "b" } else { match ancho { 8 => "q", 2 => "w", _ => "d" } };
            write!(t, "{rep}{base}{suf}")
        }
        _ => t.write_str(f.nombre),
    };
    if matches!(f.ext, Ext::Byte(_)) {
        return t;
    }
    let destino = en.wrapping_add(i.largo as u64).wrapping_add(i.imm as u64);
    let sep = |t: &mut Texto| {
        let _ = t.write_str(", ");
    };
    let sp = |t: &mut Texto| {
        let _ = t.write_str(" ");
    };
    match f.forma {
        Forma::Nada => {}
        Forma::Rel8 | Forma::Rel32 => {
            let _ = write!(t, " 0x{:x}", destino);
        }
        Forma::Reg64 => {
            let _ = write!(t, " {}", R64[(((i.op & 7) | (i.b_rex() << 3)) & 15) as usize]);
        }
        Forma::Reg => {
            let _ = write!(t, " {}", gpr((i.op & 7) | (i.b_rex() << 3), ancho, hay_rex));
        }
        Forma::MovImm | Forma::MovImm8 => {
            let a = if f.forma == Forma::MovImm8 { 1 } else { ancho };
            let _ = write!(t, " {}, ", gpr((i.op & 7) | (i.b_rex() << 3), a, hay_rex));
            hex(&mut t, i.imm);
        }
        Forma::Acc8Imm8 | Forma::AccImm => {
            let a = if f.forma == Forma::Acc8Imm8 { 1 } else { ancho };
            let _ = write!(t, " {}, ", gpr(0, a, hay_rex));
            hex(&mut t, i.imm);
        }
        Forma::Imm | Forma::Imm8 => {
            sp(&mut t);
            hex(&mut t, i.imm);
        }
        Forma::MR | Forma::MR8 => {
            let a = if f.forma == Forma::MR8 { 1 } else { ancho };
            sp(&mut t);
            rm(&mut t, i, a, None);
            sep(&mut t);
            let _ = t.write_str(gpr(i.reg(), a, hay_rex));
        }
        Forma::RM | Forma::RM8 | Forma::RM8a | Forma::RM16a | Forma::Movsxd => {
            let (dst, src) = match f.forma {
                Forma::RM8 => (1, 1),
                Forma::RM8a => (ancho, 1),
                Forma::RM16a => (ancho, 2),
                Forma::Movsxd => (ancho, 4),
                _ => (ancho, ancho),
            };
            let _ = write!(t, " {}, ", gpr(i.reg(), dst, hay_rex));
            rm(&mut t, i, src, None);
        }
        Forma::M | Forma::M64 | Forma::M8 | Forma::Mem => {
            let a = match f.forma {
                Forma::M64 => 8,
                Forma::M8 => 1,
                Forma::Mem => 4,
                _ => ancho,
            };
            sp(&mut t);
            rm(&mut t, i, a, None);
        }
        Forma::MI | Forma::MI8 | Forma::M8I8 => {
            sp(&mut t);
            rm(&mut t, i, if f.forma == Forma::M8I8 { 1 } else { ancho }, None);
            sep(&mut t);
            hex(&mut t, i.imm);
        }
        Forma::M1 | Forma::M8One => {
            sp(&mut t);
            rm(&mut t, i, if f.forma == Forma::M8One { 1 } else { ancho }, None);
            let _ = t.write_str(", 1");
        }
        Forma::MCl | Forma::M8Cl => {
            sp(&mut t);
            rm(&mut t, i, if f.forma == Forma::M8Cl { 1 } else { ancho }, None);
            let _ = t.write_str(", cl");
        }
        Forma::RMI | Forma::RMI8 => {
            let _ = write!(t, " {}, ", gpr(i.reg(), ancho, hay_rex));
            rm(&mut t, i, ancho, None);
            sep(&mut t);
            hex(&mut t, i.imm);
        }
        Forma::X | Forma::XI8 => {
            sp(&mut t);
            xr(&mut t, i.reg(), false);
            sep(&mut t);
            rm(&mut t, i, 16, Some(false));
            if f.forma == Forma::XI8 {
                sep(&mut t);
                hex(&mut t, i.imm);
            }
        }
        Forma::XM => {
            sp(&mut t);
            rm(&mut t, i, 16, Some(false));
            sep(&mut t);
            xr(&mut t, i.reg(), false);
        }
        Forma::XR => {
            sp(&mut t);
            xr(&mut t, i.reg(), false);
            sep(&mut t);
            rm(&mut t, i, if ancho == 8 { 8 } else { 4 }, None);
        }
        Forma::RX => {
            let _ = write!(t, " {}, ", gpr(i.reg(), if ancho == 8 { 8 } else { 4 }, hay_rex));
            rm(&mut t, i, 16, Some(false));
        }
        Forma::RmX => {
            sp(&mut t);
            rm(&mut t, i, if ancho == 8 { 8 } else { 4 }, None);
            sep(&mut t);
            xr(&mut t, i.reg(), false);
        }
        Forma::V3 => {
            sp(&mut t);
            xr(&mut t, i.reg(), y);
            sep(&mut t);
            xr(&mut t, i.vex.map_or(0, |v| v.vvvv), y);
            sep(&mut t);
            rm(&mut t, i, 32, Some(y));
        }
        Forma::V2 => {
            sp(&mut t);
            xr(&mut t, i.reg(), y);
            sep(&mut t);
            rm(&mut t, i, 32, Some(y));
        }
        Forma::V2M => {
            sp(&mut t);
            rm(&mut t, i, 32, Some(y));
            sep(&mut t);
            xr(&mut t, i.reg(), y);
        }
    }
    t
}
