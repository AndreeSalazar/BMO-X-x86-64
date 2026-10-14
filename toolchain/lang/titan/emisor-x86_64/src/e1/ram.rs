//! **LA PUERTA DE LA RAM** (A4 de `docs/plan/EL_FOCO.md`, 14-10) -- el UNICO
//! sitio de un programa de TITAN++ que le pide RAM al kernel.
//!
//! [carril]  VERDE     dos llamadas al kernel y nada mas
//! [consumo] NADA      corre cuando el programa pide una lamina
//!
//! El propietario, 14-10: *"es TITAN++ tomando llamadas a la RAM"*. Hasta hoy
//! CINCO sitios del emisor pedian su bloque cada uno a su manera (el monton,
//! la ventana, la lamina de VERRANO, lo tenido y el fichero), con sus dos
//! llamadas escritas a mano en cada uno. Ahora todos llaman a esta subrutina,
//! que se emite UNA vez por programa (como `Helper::Rect`), y la prueba
//! `tests/puerta_ram.rs` lo juzga: ningun otro fichero del emisor nombra la
//! operacion de pedir memoria.
//!
//! ```text
//!    entra    rdx   los bytes
//!    sale     rax   0 si el kernel la dio; otro, que no
//!             rdx   donde empieza (0 si no)
//!             rcx   su asa (la que se OFRECE o se lee por ARCH_OP_LEER_EN)
//!    pisa     rdi, rsi, r11 (y rcx, que `syscall` ya pisaba)
//! ```
//!
//! El asa sale en `rcx` y no en otro a proposito: `syscall` ya lo pisa, asi
//! que ningun sitio que llamaba al kernel podia estar contando con el.
//!
//! ** Lo que NO hace, todavia: llevar la cuenta de cada lamina por su clase.
//! El kernel ya la lleva (`INFO_MEM_ENTREGADA`) y recoge todo al morir el
//! programa; esta puerta es el sitio donde ira el dia que el programa tenga
//! que decirla el mismo.

use super::{Helper, E1};
use bmo_abi::syscalls::surface::{CURRENT_TASK, MEM_OP_BASE, NR_INVOKE, TASK_OP_MEMORIA_PEDIR};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI};

impl E1<'_> {
    /// Pide una lamina de `rdx` bytes por la puerta: deja `rax`, `rdx` y
    /// `rcx` como dice la cabecera de este fichero.
    pub(super) fn pide_lamina(&mut self) {
        self.call_helper(Helper::PideLamina);
    }

    /// **`Helper::PideLamina`**: el bloque y donde esta.
    pub(super) fn h_pide_lamina(&mut self) {
        // El bloque: su asa en rdx.
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MEMORIA_PEDIR as i64);
        self.imm(RAX, NR_INVOKE as i64);
        x86::syscall(&mut self.code);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let no_dio = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let sin_asa = self.jcc(0x84);
        // Donde esta: la base en rdx; el asa, guardada y devuelta en rcx.
        x86::push_r64(&mut self.code, RDX);
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RSI, MEM_OP_BASE as i64);
        self.imm(RAX, NR_INVOKE as i64);
        x86::syscall(&mut self.code);
        x86::pop_r64(&mut self.code, RCX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let no_base = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let bien = self.jcc(0x85);
        // Sin asa, o con base cero: que no, con rax distinto de cero.
        self.here(sin_asa);
        self.imm(RAX, 1);
        x86::zero_r32(&mut self.code, RDX);
        self.here(no_dio);
        self.here(no_base);
        self.here(bien);
        self.code.push(0xC3);
    }
}
