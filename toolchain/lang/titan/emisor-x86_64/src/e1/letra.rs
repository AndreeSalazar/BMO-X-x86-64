//! **LA LETRA DE BMO-X EN LA VENTANA** (F2 de `docs/plan/EL_FOCO.md`, 10-10):
//! los glifos de 8x16 de `fontgen` -- los MISMOS que dibujan el kernel
//! (`core/font16_data.rs`) e INTI (`runtime/fuente/datos.inti`) -- escritos
//! en la ventana de un programa de TITAN++, con su escala.
//!
//! ```text
//!    letra(x, y, b, e, c)   el glifo del byte Latin-1 `b`: el ASCII directo,
//!                           uno de los 25 extras por su byte, o el HUECO (`?`)
//!                           como `glifo_de` de INTI. Cada pixel encendido, un
//!                           cuadrado de e x e (`lamina_glifo` de INTI), y todo
//!                           RECORTADO a la ventana. Da la x de la siguiente:
//!                           x + 8 e (con una escala fuera de 1..16, no pinta y
//!                           da x)
//!    texto(x, y, t, e, c)   cada byte de `t`, uno tras otro. Da la x de detras
//! ```
//!
//! ** Dos SUBRUTINAS, una vez por programa (`Helper::Rect`, `Helper::Glifo`):
//! el rectangulo recortado -- tambien el de `director.rect` -- y el glifo, con
//! la tabla de la fuente (1920 bytes) y sus extras (25) DETRAS de su `ret`,
//! leidas por `lea [rip + ...]`. La region de codigo se lee: es la del
//! programa.

use super::ventana::R12;
use super::{Helper, Place, E1};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{At, Value};

/// Los glifos: 95 ASCII (32..=126) y 25 extras Latin-1, 16 filas cada uno.
static FUENTE: [[u8; 16]; 120] = include!("../../../../../../Ultra_userspace/userland/src/font16_data.rs");
/// El byte Latin-1 de cada extra, en su orden desde el glifo 95.
static EXTRAS: [u8; 25] = include!("../../../../../../Ultra_userspace/userland/src/font16_extra.rs");
/// Los glifos ASCII, y el que se pinta si la fuente no tiene el byte: `?`.
const ASCII: i32 = 95;
const HUECO: u32 = b'?' as u32 - 32;
/// La escala mas grande: 16 (un glifo de 128 x 256).
const ESCALA_MAXIMA: i32 = 16;
const RBX: u8 = 3;
const RBP: u8 = 5;
const R13: u8 = 13;
const R14: u8 = 14;
const R15: u8 = 15;

const _: () = assert!(HUECO == 31);

/// `mov <reg>, [rsp + d]`.
fn de_la_pila(code: &mut Vec<u8>, reg: u8, d: u8) {
    code.extend_from_slice(&[0x48 | if reg >= 8 { 4 } else { 0 }, 0x8B, 0x44 | (reg & 7) << 3, 0x24, d]);
}

/// `lea <reg>, [rip + ?]`: el campo del desplazamiento, para rellenar.
fn lea_rip(code: &mut Vec<u8>, reg: u8) -> usize {
    code.extend_from_slice(&[0x48 | if reg >= 8 { 4 } else { 0 }, 0x8D, 0x05 | (reg & 7) << 3, 0, 0, 0, 0]);
    code.len() - 4
}

/// El desplazamiento de un `lea [rip + ...]` hasta `a`.
fn apunta(code: &mut [u8], campo: usize, a: usize) {
    let d = a as i64 - (campo as i64 + 4);
    code[campo..campo + 4].copy_from_slice(&(d as i32).to_le_bytes());
}

impl E1<'_> {
    /// **`Helper::Rect`**: el rectangulo `r8` (x), `r9` (y), `r10` (ancho),
    /// `r11` (alto) del color `rdx` (ya opaco), recortado a la ventana (r12).
    /// Toca rax, rcx, rsi, rdi y r8..r11; deja rdx.
    pub(super) fn h_rect(&mut self) {
        let mut fin = Vec::new();
        x86::mov_r64_r64(&mut self.code, RDI, R12);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        fin.push(self.jcc(0x84));
        self.lea(RSI, RDI, 4);
        x86::mov_r32_at_reg(&mut self.code, RAX, RSI);
        self.lea(RSI, RDI, 8);
        x86::mov_r32_at_reg(&mut self.code, RCX, RSI);
        // [x0, x1) y [y0, y1), con signo: hi = min(desde + medida, limite),
        // lo = max(desde, 0); vacio si lo >= hi.
        for (desde, medida, limite) in [(R8, R10, RAX), (R9, R11, RCX)] {
            x86::add_r64_r64(&mut self.code, medida, desde);
            x86::cmp_r64_r64(&mut self.code, medida, limite);
            let cabe = self.jcc(0x8E);
            x86::mov_r64_r64(&mut self.code, medida, limite);
            self.here(cabe);
            x86::test_r64_r64(&mut self.code, desde, desde);
            let positivo = self.jcc(0x8D);
            x86::zero_r32(&mut self.code, desde);
            self.here(positivo);
            x86::cmp_r64_r64(&mut self.code, desde, medida);
            fin.push(self.jcc(0x8D));
        }
        let fila = self.code.len();
        x86::mov_r64_r64(&mut self.code, RSI, R9);
        x86::imul_r64_r64(&mut self.code, RSI, RAX);
        x86::add_r64_r64(&mut self.code, RSI, R8);
        x86::shl_r64_imm8(&mut self.code, RSI, 2);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        self.lea(RSI, RSI, 32);
        x86::mov_r64_r64(&mut self.code, RCX, R10);
        x86::sub_r64_r64(&mut self.code, RCX, R8);
        let celda = self.code.len();
        x86::mov_at_reg_from_r32(&mut self.code, RSI, RDX);
        x86::add_r64_imm8(&mut self.code, RSI, 4);
        x86::dec_r64(&mut self.code, RCX);
        let mas = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, mas, celda);
        x86::inc_r64(&mut self.code, R9);
        x86::cmp_r64_r64(&mut self.code, R9, R11);
        let otra = self.jcc(0x8C);
        x86::patch_jump_to(&mut self.code, otra, fila);
        for j in fin {
            self.here(j);
        }
        self.code.push(0xC3);
    }

    /// **`Helper::Glifo`**: el glifo del byte `rax` en (`r8`, `r9`), escala
    /// `r10` (1..16), color `rdx` (opaco). Toca rax, rcx, rdx, rsi, rdi y
    /// r8..r11; deja rbx, rbp y r12..r15.
    pub(super) fn h_glifo(&mut self) {
        // El indice del glifo, en rax.
        x86::cmp_r64_imm32(&mut self.code, RAX, 32);
        let bajo = self.jcc(0x8C);
        x86::cmp_r64_imm32(&mut self.code, RAX, 127);
        let alto = self.jcc(0x8D);
        x86::sub_r64_imm8(&mut self.code, RAX, 32);
        let tengo = self.jmp();
        self.here(bajo);
        self.here(alto);
        let extras = lea_rip(&mut self.code, RSI);
        x86::zero_r32(&mut self.code, RCX);
        let busca = self.code.len();
        x86::cmp_r64_imm32(&mut self.code, RCX, EXTRAS.len() as i32);
        let hueco = self.jcc(0x8D);
        x86::movzx_r32_byte_base_index(&mut self.code, RDI, RSI, RCX);
        x86::cmp_r64_r64(&mut self.code, RDI, RAX);
        let hallado = self.jcc(0x84);
        x86::inc_r64(&mut self.code, RCX);
        let sigue = self.jmp();
        x86::patch_jump_to(&mut self.code, sigue, busca);
        self.here(hallado);
        self.lea(RAX, RCX, ASCII);
        let tengo2 = self.jmp();
        self.here(hueco);
        x86::mov_r32_imm32(&mut self.code, RAX, HUECO);
        self.here(tengo);
        self.here(tengo2);
        // Lo que se guarda, y la pila: [rsp] color, [+8] escala, [+16] y,
        // [+24] x.
        for r in [RBX, RBP, R13, R14, R15, R8, R9, R10, RDX] {
            x86::push_r64(&mut self.code, r);
        }
        let fuente = lea_rip(&mut self.code, RBX);
        x86::shl_r64_imm8(&mut self.code, RAX, 4);
        x86::add_r64_r64(&mut self.code, RBX, RAX);
        x86::zero_r32(&mut self.code, R13);
        // Fila a fila (r13), columna a columna (r15); los bits, en r14.
        let fila = self.code.len();
        x86::movzx_r32_byte_base_index(&mut self.code, R14, RBX, R13);
        x86::test_r64_r64(&mut self.code, R14, R14);
        let vacia = self.jcc(0x84);
        x86::zero_r32(&mut self.code, R15);
        let col = self.code.len();
        x86::mov_r64_r64(&mut self.code, RCX, R15);
        self.imm(RAX, 128);
        x86::shr_r64_cl(&mut self.code, RAX);
        x86::test_r64_r64(&mut self.code, RAX, R14);
        let apagado = self.jcc(0x84);
        de_la_pila(&mut self.code, R10, 8);
        x86::mov_r64_r64(&mut self.code, R8, R15);
        x86::imul_r64_r64(&mut self.code, R8, R10);
        de_la_pila(&mut self.code, RAX, 24);
        x86::add_r64_r64(&mut self.code, R8, RAX);
        x86::mov_r64_r64(&mut self.code, R9, R13);
        x86::imul_r64_r64(&mut self.code, R9, R10);
        de_la_pila(&mut self.code, RAX, 16);
        x86::add_r64_r64(&mut self.code, R9, RAX);
        x86::mov_r64_r64(&mut self.code, R11, R10);
        de_la_pila(&mut self.code, RDX, 0);
        self.call_helper(Helper::Rect);
        self.here(apagado);
        x86::inc_r64(&mut self.code, R15);
        x86::cmp_r64_imm32(&mut self.code, R15, 8);
        let otra_col = self.jcc(0x8C);
        x86::patch_jump_to(&mut self.code, otra_col, col);
        self.here(vacia);
        x86::inc_r64(&mut self.code, R13);
        x86::cmp_r64_imm32(&mut self.code, R13, 16);
        let otra_fila = self.jcc(0x8C);
        x86::patch_jump_to(&mut self.code, otra_fila, fila);
        for r in [RDX, R10, R9, R8, R15, R14, R13, RBP, RBX] {
            x86::pop_r64(&mut self.code, r);
        }
        self.code.push(0xC3);
        // Los datos, detras: la fuente y sus extras.
        let a = self.code.len();
        self.code.extend(FUENTE.iter().flatten());
        apunta(&mut self.code, fuente, a);
        let b = self.code.len();
        self.code.extend_from_slice(&EXTRAS);
        apunta(&mut self.code, extras, b);
    }

    /// `rdx` = el color de `p`, opaco.
    fn opaco(&mut self, p: Place) {
        self.load(p, RDX);
        x86::and_r64_imm32(&mut self.code, RDX, 0x00FF_FFFF);
        self.imm(R10, 0xFF00_0000);
        x86::or_r64_r64(&mut self.code, RDX, R10);
    }

    /// `director.letra(x, y, byte, escala, color)`.
    pub(super) fn letra(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [x, y, b, e, c] = args else { return Err(format!("linea {}: `director.letra` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (x, _) = self.eval(x)?;
        let (y, _) = self.eval(y)?;
        let (b, _) = self.eval(b)?;
        let (e, _) = self.eval(e)?;
        let (c, _) = self.eval(c)?;
        let out = self.temp(8);
        self.load(x, RAX);
        self.store(out, RAX);
        let mut nada = Vec::new();
        self.escala_vale(e, &mut nada);
        x86::test_r64_r64(&mut self.code, R12, R12);
        let sin = self.jcc(0x84);
        self.opaco(c);
        self.load(x, R8);
        self.load(y, R9);
        self.load(e, R10);
        self.load(b, RAX);
        self.call_helper(Helper::Glifo);
        self.here(sin);
        self.avanza(out, e);
        for j in nada {
            self.here(j);
        }
        Ok((out, Class::Int))
    }

    /// Salta (a rellenar) si la escala de `e` no esta en 1..=16.
    fn escala_vale(&mut self, e: Place, nada: &mut Vec<usize>) {
        self.load(e, RAX);
        x86::cmp_r64_imm32(&mut self.code, RAX, 1);
        nada.push(self.jcc(0x8C));
        x86::cmp_r64_imm32(&mut self.code, RAX, ESCALA_MAXIMA);
        nada.push(self.jcc(0x8F));
    }

    /// `out += 8 * e`.
    fn avanza(&mut self, out: Place, e: Place) {
        self.load(e, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 3);
        self.load(out, RCX);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.store(out, RAX);
    }

    /// `director.texto(x, y, t, escala, color)`.
    pub(super) fn texto(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [x, y, t, e, c] = args else { return Err(format!("linea {}: `director.texto` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (x, _) = self.eval(x)?;
        let (y, _) = self.eval(y)?;
        let (t, _) = self.eval(t)?;
        let (e, _) = self.eval(e)?;
        let (c, _) = self.eval(c)?;
        let out = self.temp(8);
        let i = self.temp(8);
        self.load(x, RAX);
        self.store(out, RAX);
        let mut nada = Vec::new();
        self.escala_vale(e, &mut nada);
        self.store_imm(i, 0);
        let vuelta = self.code.len();
        self.load(i, RCX);
        self.load(t, RAX);
        x86::cmp_r64_r64(&mut self.code, RCX, RAX);
        nada.push(self.jcc(0x8D));
        x86::test_r64_r64(&mut self.code, R12, R12);
        let sin = self.jcc(0x84);
        // El byte i del texto: sus bytes, detras de su largo.
        self.addr(t, RSI);
        self.lea(RSI, RSI, 8);
        x86::movzx_r32_byte_base_index(&mut self.code, RAX, RSI, RCX);
        x86::mov_r64_r64(&mut self.code, R11, RAX);
        self.opaco(c);
        x86::mov_r64_r64(&mut self.code, RAX, R11);
        self.load(out, R8);
        self.load(y, R9);
        self.load(e, R10);
        self.call_helper(Helper::Glifo);
        self.here(sin);
        self.avanza(out, e);
        self.load(i, RCX);
        x86::inc_r64(&mut self.code, RCX);
        self.store(i, RCX);
        let otra = self.jmp();
        x86::patch_jump_to(&mut self.code, otra, vuelta);
        for j in nada {
            self.here(j);
        }
        Ok((out, Class::Int))
    }
}
