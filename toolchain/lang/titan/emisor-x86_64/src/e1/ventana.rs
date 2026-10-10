//! **LA VENTANA, AL CORRER** (F1 de `docs/plan/EL_FOCO.md` = TA2 de
//! `docs/plan/PLAN_LA_TINTA.md`, 10-10): una SUPERFICIE de BMO-X pedida,
//! descrita y ofrecida por un programa de TITAN++, como la escriben
//! `roja.inti` (INTI) y `roja.h` (C). El contrato vive UNA vez en
//! `bmo_abi::syscalls::surface::superficie`: de alli salen los numeros de
//! aqui, por su nombre.
//!
//! ```text
//!    r12                  la ventana del programa, todo el programa (0:
//!                         ninguna). Solo si habla con el director
//!    ventana(an, al)      el bloque -- la cabecera BSUP, los pixeles a negro,
//!                         el buzon de BUZON ranuras y 16 bytes de cola -- pedido
//!                         al kernel, ENTERO antes de ofrecerlo; quien nos
//!                         lanzo; y la OFERTA. Si: ofrecida. No: medidas fuera
//!                         de 1..=MEDIDA_MAXIMA, sin memoria, sin padre (el
//!                         shell), o con ventana ya
//!    pixel(x, y, c)       un pixel; fuera de la ventana, nada
//!    rect(x, y, an, al, c)  un rectangulo lleno, recortado a la ventana
//!    fila(y, t)           la fila y de una tabla de int: las celdas que
//!                         quepan
//!    presenta()           la SECUENCIA sube: el dibujo esta entero, y es lo
//!                         unico que hace que el DIRECTOR lo componga (R-APP4)
//! ```
//!
//! Un color es `r * 65536 + g * 256 + b`: en memoria, BGRA de 32 bits (el
//! byte alto, el alfa, a 0xFF: opaco).
//!
//! ** El buzon va SIEMPRE: una ventana de TITAN++ es de una app que se usa
//! (F3 lee sus teclas y su raton). Cabeza, cola y estado a cero antes de
//! ofrecer.
//!
//! ** La cola privada, como en `roja.inti` y en la lamina: 16 bytes DETRAS de
//! lo ofrecido, con el asa del bloque y lo ofrecido. El escritorio solo ve
//! `[s, s + bytes)`.

use super::{Helper, Place, E1};
use bmo_abi::syscalls::surface::{CURRENT_TASK, MEM_OP_BASE, MEM_OP_OFRECER, SUP_BGRA32, SUP_BUZON_CABECERA, SUP_BUZON_RANURA, SUP_CABECERA, SUP_CAMPO_SECUENCIA, SUP_MAGIC, TASK_OP_MEMORIA_PEDIR, TASK_OP_MI_PADRE};
use bmo_lower::memoria;
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{At, Value};

/// La ventana del programa, todo el programa.
pub(crate) const R12: u8 = 12;
/// Los bytes de cola detras de lo ofrecido: el asa y lo ofrecido; y desde
/// F3 (`entrada.rs`), en +32, el ultimo evento.
const COLA: i32 = 48;
/// Lo mas ancha (y lo mas alta) que se pide: 4096 pixeles (64 MiB).
pub const MEDIDA_MAXIMA: i32 = 4096;
/// Las ranuras del buzon: potencia de dos (el indice avanza con una mascara).
pub const BUZON: u64 = 64;
/// Opaco: el alfa del BGRA.
const OPACO: u32 = 0xFF00_0000;

const _: () = assert!(SUP_CABECERA == 32 && SUP_BUZON_CABECERA == 16 && SUP_BUZON_RANURA == 8 && SUP_BGRA32 == 0 && SUP_CAMPO_SECUENCIA == 5);
const _: () = assert!(BUZON.is_power_of_two());

/// El byte del campo `i` (u32) de la cabecera.
const fn campo(i: u64) -> i32 {
    4 * i as i32
}

impl E1<'_> {
    /// `rdi` = la ventana (r12), y salta (a rellenar) si no hay.
    fn sin_ventana(&mut self, saltos: &mut Vec<usize>) {
        x86::mov_r64_r64(&mut self.code, RDI, R12);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        saltos.push(self.jcc(0x84));
    }

    /// `director.ventana(ancho, alto)`.
    pub(super) fn ventana(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [an, al] = args else { return Err(format!("linea {}: `director.ventana` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (an, _) = self.eval(an)?;
        let (al, _) = self.eval(al)?;
        let out = self.temp(8);
        let bytes = self.temp(8);
        let buzon = self.temp(8);
        let bloque = self.temp(8);
        let base = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        // Una ventana por programa (UNA APP, UNA VENTANA: la regla del DIRECTOR).
        x86::test_r64_r64(&mut self.code, R12, R12);
        no.push(self.jcc(0x85));
        // Las medidas, de 1 a MEDIDA_MAXIMA.
        for m in [an, al] {
            self.load(m, RAX);
            x86::test_r64_r64(&mut self.code, RAX, RAX);
            no.push(self.jcc(0x8E));
            x86::cmp_r64_imm32(&mut self.code, RAX, MEDIDA_MAXIMA);
            no.push(self.jcc(0x8F));
        }
        // Lo ofrecido: la cabecera, los pixeles, y detras el buzon.
        self.load(an, RAX);
        self.load(al, RCX);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        x86::shl_r64_imm8(&mut self.code, RAX, 2);
        self.lea(RAX, RAX, SUP_CABECERA as i32);
        self.store(buzon, RAX);
        self.lea(RAX, RAX, (SUP_BUZON_CABECERA + BUZON * SUP_BUZON_RANURA) as i32);
        self.store(bytes, RAX);
        // El bloque, con su cola: su asa, y donde esta.
        self.lea(RDX, RAX, COLA);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MEMORIA_PEDIR as i64);
        self.invoke();
        self.si_no_vale(&mut no);
        self.store(bloque, RDX);
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RSI, MEM_OP_BASE as i64);
        self.invoke();
        self.si_no_vale(&mut no);
        self.store(base, RDX);
        // TODO a cero antes de ofrecer -- la cabecera, los pixeles (negro) y
        // el buzon --: si el escritorio la tomara antes, leeria basura.
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.load(bytes, RCX);
        self.lea(RCX, RCX, COLA);
        x86::zero_r32(&mut self.code, RAX);
        memoria::rellenar(&mut self.code);
        // La cabecera, dos campos por palabra: magia y ancho; alto y stride
        // (= ancho, sin relleno); formato y secuencia (0 y 0, ya); buzon y
        // sus ranuras.
        self.load(base, RDI);
        self.load(an, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 32);
        self.imm(RCX, SUP_MAGIC as i64);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, campo(0), RAX);
        self.load(an, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 32);
        self.load(al, RCX);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, campo(2), RAX);
        self.imm(RAX, (BUZON << 32) as i64);
        self.load(buzon, RCX);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, campo(6), RAX);
        // La cola: el asa del bloque y lo ofrecido.
        self.load(bytes, RCX);
        x86::add_r64_r64(&mut self.code, RDI, RCX);
        self.load(bloque, RAX);
        x86::mov_at_reg_from_r64(&mut self.code, RDI, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, 8, RCX);
        // Quien nos lanzo: el escritorio, o nadie (el shell).
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MI_PADRE as i64);
        self.invoke();
        self.si_no_vale(&mut no);
        // La OFERTA: (desde 0, lo ofrecido, a quien).
        x86::mov_r64_r64(&mut self.code, R8, RDX);
        self.load(bloque, RDI);
        self.imm(RSI, MEM_OP_OFRECER as i64);
        x86::zero_r32(&mut self.code, RDX);
        self.load(bytes, R10);
        self.invoke();
        self.si_no_vale(&mut no);
        // Ofrecida: la ventana es esta.
        self.load(base, R12);
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `rax` = el ancho y `rcx` = el alto de la ventana de `rdi`.
    fn medidas(&mut self) {
        self.lea(RSI, RDI, campo(1));
        x86::mov_r32_at_reg(&mut self.code, RAX, RSI);
        self.lea(RSI, RDI, campo(2));
        x86::mov_r32_at_reg(&mut self.code, RCX, RSI);
    }

    /// `rsi` = el pixel (x en `r8`, y en `r9`) de la ventana de `rdi`, con
    /// el ancho en `rax`: `rdi + 32 + (y * ancho + x) * 4`. Toca `rsi`.
    fn donde(&mut self) {
        x86::mov_r64_r64(&mut self.code, RSI, R9);
        x86::imul_r64_r64(&mut self.code, RSI, RAX);
        x86::add_r64_r64(&mut self.code, RSI, R8);
        x86::shl_r64_imm8(&mut self.code, RSI, 2);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        self.lea(RSI, RSI, SUP_CABECERA as i32);
    }

    /// `rdx` = el color de `p`, opaco.
    fn color(&mut self, p: Place) {
        self.load(p, RDX);
        x86::and_r64_imm32(&mut self.code, RDX, 0x00FF_FFFF);
        self.imm(R10, OPACO as i64);
        x86::or_r64_r64(&mut self.code, RDX, R10);
    }

    /// `director.pixel(x, y, color)`.
    pub(super) fn pixel(&mut self, args: &[Value], at: At) -> Result<(), String> {
        let [x, y, c] = args else { return Err(format!("linea {}: `director.pixel` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (x, _) = self.eval(x)?;
        let (y, _) = self.eval(y)?;
        let (c, _) = self.eval(c)?;
        let mut fuera = Vec::new();
        self.sin_ventana(&mut fuera);
        self.medidas();
        // Sin signo: un negativo es enorme, y tambien se queda fuera.
        self.load(x, R8);
        x86::cmp_r64_r64(&mut self.code, R8, RAX);
        fuera.push(self.jcc(0x83));
        self.load(y, R9);
        x86::cmp_r64_r64(&mut self.code, R9, RCX);
        fuera.push(self.jcc(0x83));
        self.donde();
        self.color(c);
        x86::mov_at_reg_from_r32(&mut self.code, RSI, RDX);
        for j in fuera {
            self.here(j);
        }
        Ok(())
    }

    /// `director.rect(x, y, ancho, alto, color)`: recortado a la ventana, por
    /// la subrutina de siempre (`Helper::Rect`, `letra.rs`).
    pub(super) fn rect(&mut self, args: &[Value], at: At) -> Result<(), String> {
        let [x, y, w, h, c] = args else { return Err(format!("linea {}: `director.rect` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (x, _) = self.eval(x)?;
        let (y, _) = self.eval(y)?;
        let (w, _) = self.eval(w)?;
        let (h, _) = self.eval(h)?;
        let (c, _) = self.eval(c)?;
        self.color(c);
        self.load(x, R8);
        self.load(y, R9);
        self.load(w, R10);
        self.load(h, R11);
        self.call_helper(Helper::Rect);
        Ok(())
    }

    /// `director.fila(y, pixeles)`: las celdas que quepan.
    pub(super) fn fila(&mut self, args: &[Value], at: At) -> Result<(), String> {
        let [y, t] = args else { return Err(format!("linea {}: `director.fila` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (y, _) = self.eval(y)?;
        let (t, ct) = self.eval(t)?;
        let Class::Table(_, celdas) = ct else { return Err(format!("linea {}: `director.fila` sin tabla (fallo del compilador)", at.0)) };
        let mut nada = Vec::new();
        self.sin_ventana(&mut nada);
        self.medidas();
        self.load(y, R9);
        x86::cmp_r64_r64(&mut self.code, R9, RCX);
        nada.push(self.jcc(0x83));
        // n = min(celdas, ancho), en r10.
        self.imm(R10, celdas as i64);
        x86::cmp_r64_r64(&mut self.code, R10, RAX);
        let cabe = self.jcc(0x8E);
        x86::mov_r64_r64(&mut self.code, R10, RAX);
        self.here(cabe);
        x86::zero_r32(&mut self.code, R8);
        self.donde();
        // Una celda son 8 bytes: el color, en su mitad baja.
        self.addr(t, RCX);
        let celda = self.code.len();
        x86::mov_r64_at_reg(&mut self.code, RDX, RCX);
        x86::and_r64_imm32(&mut self.code, RDX, 0x00FF_FFFF);
        self.imm(RAX, OPACO as i64);
        x86::or_r64_r64(&mut self.code, RDX, RAX);
        x86::mov_at_reg_from_r32(&mut self.code, RSI, RDX);
        x86::add_r64_imm8(&mut self.code, RSI, 4);
        x86::add_r64_imm8(&mut self.code, RCX, 8);
        x86::dec_r64(&mut self.code, R10);
        let mas = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, mas, celda);
        for j in nada {
            self.here(j);
        }
        Ok(())
    }

    /// `director.presenta()`: la secuencia, uno mas. LO ULTIMO del dibujo.
    pub(super) fn presenta(&mut self) -> Result<(), String> {
        let mut nada = Vec::new();
        self.sin_ventana(&mut nada);
        self.lea(RDI, RDI, campo(SUP_CAMPO_SECUENCIA));
        x86::mov_r32_at_reg(&mut self.code, RAX, RDI);
        x86::inc_r64(&mut self.code, RAX);
        x86::mov_at_reg_from_r32(&mut self.code, RDI, RAX);
        for j in nada {
            self.here(j);
        }
        Ok(())
    }
}
