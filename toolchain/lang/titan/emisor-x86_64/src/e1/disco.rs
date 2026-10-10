//! **LO QUE SE ESCRIBE EN EL DISCO** (TA4 de `docs/plan/PLAN_LA_TINTA.md`,
//! 10-10): un fichero ENTERO de una vez, o uno que se va escribiendo byte a
//! byte -- lo de las herramientas de imagen que fueron de INTI (`bico`,
//! `png`: 1d de `docs/plan/EL_FOCO.md`) --.
//!
//! ```text
//!    guarda(r, t)    la ruta (TASK_OP_RUTA), CREARLO de cero, el texto de 7
//!                    en 7 (ARCH_OP_ESCRIBIR) y cerrar: ahi llega al disco.
//!                    Si, si entraron todos y el disco dijo que si
//!    crea(r)         la ruta y CREARLO: su asa queda en el bloque de lo
//!                    tenido (`rbx`). UNO a la vez: con otro abierto, no
//!    escribe(b)      un byte (0..255) a la PALABRA que espera; con siete, se
//!                    mandan (`(7 << 56) | bytes`). Uno fuera de 0..255 no se
//!                    recorta: se apunta como MAL y no se escribe
//!    cierra()        lo que quede de la palabra, y cerrar. Si, si entro todo
//!                    y el disco lo guardo
//! ```
//!
//! [!] El kernel guarda lo que tenga al cerrar: un `escribe` que fallo deja
//! un fichero a medias aunque `cierra` diga que no. Quien no quiera eso hace
//! lo de `bico`: lo comprueba TODO antes de crear el fichero. Y uno que no se
//! cierra no llega al disco.

use super::entrada::RBX;
use super::{Place, E1};
use bmo_abi::syscalls::surface::{ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, CURRENT_TASK, TASK_OP_ARCHIVO_CREAR};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R8};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{At, Director, Value};

/// Los bytes que mete `ARCH_OP_ESCRIBIR` en una llamada: siete, y el octavo
/// dice cuantos.
const POR_LLAMADA: i32 = 7;
/// En el bloque de lo tenido (`entrada.rs`, 48 bytes): el asa del fichero
/// que se escribe (0: ninguno), la palabra que espera, cuantos bytes lleva, y
/// si algo no entro.
const ASA: i32 = 16;
const PALABRA: i32 = 24;
const CUANTOS: i32 = 32;
const MAL: i32 = 40;

/// `and <dst>, <src>` (64 bits).
fn and_r64_r64(code: &mut Vec<u8>, dst: u8, src: u8) {
    code.extend_from_slice(&[0x48 | if src >= 8 { 4 } else { 0 } | if dst >= 8 { 1 } else { 0 }, 0x21, 0xC0 | (src & 7) << 3 | (dst & 7)]);
}

impl E1<'_> {
    /// Lo que se pide al director para escribir (`escribe` no da nada: va
    /// aparte, `escribe_byte`).
    pub(super) fn disco(&mut self, what: Director, args: &[Value], at: At) -> Result<(Place, Class), String> {
        match what {
            Director::Guarda => self.guarda(args, at),
            Director::Crea => self.crea(args, at),
            Director::Cierra => self.cierra(),
            _ => Err(format!("linea {}: `{}` no es del disco (fallo del compilador)", at.0, what.name())),
        }
    }

    /// `ARCH_OP_ESCRIBIR` con los `rcx` bytes de `rdx` (ya recortados) al
    /// fichero de `rbx`; si no entraron todos, MAL. Pisa la palabra y la
    /// cuenta a cero.
    fn manda_palabra(&mut self) {
        x86::mov_r64_r64(&mut self.code, R8, RCX);
        x86::shl_r64_imm8(&mut self.code, R8, 56);
        x86::or_r64_r64(&mut self.code, RDX, R8);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, ASA);
        self.imm(RSI, ARCH_OP_ESCRIBIR as i64);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        // rcx sobrevive en la pila de E1: el `invoke` lo pisa.
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, CUANTOS, RCX);
        self.invoke();
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, CUANTOS);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let fallo = self.jcc(0x85);
        x86::cmp_r64_r64(&mut self.code, RDX, RCX);
        let bien = self.jcc(0x84);
        self.here(fallo);
        self.imm(RAX, 1);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, MAL, RAX);
        self.here(bien);
        x86::zero_r32(&mut self.code, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, PALABRA, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, CUANTOS, RAX);
    }

    /// `director.crea(ruta)`.
    fn crea(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [ruta] = args else { return Err(format!("linea {}: `director.crea` sin su ruta (fallo del compilador)", at.0)) };
        let (t, _) = self.eval(ruta)?;
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_tenido(&mut no);
        // UNO a la vez.
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, ASA);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x85));
        self.load(t, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x84));
        self.ruta(t);
        self.invoke_solo(None, CURRENT_TASK, TASK_OP_ARCHIVO_CREAR);
        self.si_no_vale(&mut no);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, ASA, RDX);
        x86::zero_r32(&mut self.code, RAX);
        for campo in [PALABRA, CUANTOS, MAL] {
            x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, campo, RAX);
        }
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `director.escribe(b)`: no da nada.
    pub(super) fn escribe_byte(&mut self, args: &[Value], at: At) -> Result<(), String> {
        let [b] = args else { return Err(format!("linea {}: `director.escribe` sin su byte (fallo del compilador)", at.0)) };
        let (b, _) = self.eval(b)?;
        let mut nada = Vec::new();
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        nada.push(self.jcc(0x84));
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, ASA);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        nada.push(self.jcc(0x84));
        // Fuera de 0..255 (sin signo: un negativo es enorme): MAL, y no se escribe.
        self.load(b, RAX);
        x86::cmp_r64_imm32(&mut self.code, RAX, 255);
        let cabe = self.jcc(0x86);
        self.imm(RAX, 1);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, MAL, RAX);
        nada.push(self.jmp());
        self.here(cabe);
        // A la palabra: b << (8 * cuantos).
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, CUANTOS);
        x86::shl_r64_imm8(&mut self.code, RCX, 3);
        x86::shl_r64_cl(&mut self.code, RAX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, RBX, PALABRA);
        x86::or_r64_r64(&mut self.code, RDX, RAX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, CUANTOS);
        x86::add_r64_imm8(&mut self.code, RCX, 1);
        x86::cmp_r64_imm32(&mut self.code, RCX, POR_LLAMADA);
        let espera = self.jcc(0x82);
        self.manda_palabra();
        nada.push(self.jmp());
        self.here(espera);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, PALABRA, RDX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, CUANTOS, RCX);
        for j in nada {
            self.here(j);
        }
        Ok(())
    }

    /// `director.cierra()`.
    fn cierra(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        no.push(self.jcc(0x84));
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, ASA);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x84));
        // Lo que quede de la palabra.
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, CUANTOS);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let vacia = self.jcc(0x84);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, RBX, PALABRA);
        self.manda_palabra();
        self.here(vacia);
        // Cerrar: ahi llega al disco. El asa se olvida salga como salga.
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, ASA);
        self.imm(RSI, ARCH_OP_CERRAR as i64);
        x86::zero_r32(&mut self.code, RDX);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
        x86::zero_r32(&mut self.code, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, ASA, RCX);
        self.si_no_vale(&mut no);
        x86::cmp_r64_imm32(&mut self.code, RDX, 1);
        no.push(self.jcc(0x85));
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, MAL);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x85));
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `director.guarda(ruta, texto)`: crearlo (`TASK_OP_ARCHIVO_CREAR`, de
    /// cero), el texto de 7 en 7 (`ARCH_OP_ESCRIBIR`: `(n << 56) | bytes`) y
    /// cerrar, que es donde llega al disco. Si, si entraron todos y el disco
    /// lo guardo.
    fn guarda(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [ruta, texto] = args else { return Err(format!("linea {}: `director.guarda` sin su ruta y su texto (fallo del compilador)", at.0)) };
        let (t, _) = self.eval(ruta)?;
        let (s, _) = self.eval(texto)?;
        let out = self.temp(8);
        let f = self.temp(8);
        let k = self.temp(8);
        let n = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.load(t, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x84));
        self.ruta(t);
        self.invoke_solo(None, CURRENT_TASK, TASK_OP_ARCHIVO_CREAR);
        self.si_no_vale(&mut no);
        self.store(f, RDX);
        // De 7 en 7: los de detras del largo, a cero.
        let mut mal = Vec::new();
        self.store_imm(k, 0);
        let vuelta = self.code.len();
        self.load(k, RAX);
        self.load(s, RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let escrito = self.jcc(0x83);
        x86::sub_r64_r64(&mut self.code, RCX, RAX);
        x86::cmp_r64_imm32(&mut self.code, RCX, POR_LLAMADA);
        let pocos = self.jcc(0x8C);
        self.imm(RCX, POR_LLAMADA as i64);
        self.here(pocos);
        self.store(n, RCX);
        self.addr(s, RDI);
        x86::add_r64_r64(&mut self.code, RDI, RAX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, RDI, 8);
        x86::shl_r64_imm8(&mut self.code, RCX, 3);
        self.imm(R8, 1);
        x86::shl_r64_cl(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R8);
        and_r64_r64(&mut self.code, RDX, R8);
        self.load(n, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 56);
        x86::or_r64_r64(&mut self.code, RDX, RAX);
        self.load(f, RDI);
        self.imm(RSI, ARCH_OP_ESCRIBIR as i64);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
        self.si_no_vale(&mut mal);
        self.load(n, RCX);
        x86::cmp_r64_r64(&mut self.code, RDX, RCX);
        mal.push(self.jcc(0x85));
        self.load(k, RAX);
        x86::add_r64_r64(&mut self.code, RAX, RCX);
        self.store(k, RAX);
        let otra = self.jmp();
        x86::patch_jump_to(&mut self.code, otra, vuelta);
        self.here(escrito);
        // Cerrar: el disco dice si (1) o no.
        self.invoke_solo(Some(f), 0, ARCH_OP_CERRAR);
        self.si_no_vale(&mut no);
        x86::cmp_r64_imm32(&mut self.code, RDX, 1);
        no.push(self.jcc(0x85));
        self.store_imm(out, 1);
        let fin = self.jmp();
        // Uno que no entro: se cierra igual -- el asa no se queda abierta -- y
        // se dice que no.
        for j in mal {
            self.here(j);
        }
        self.invoke_solo(Some(f), 0, ARCH_OP_CERRAR);
        for j in no {
            self.here(j);
        }
        self.here(fin);
        Ok((out, Class::Bool))
    }

}
