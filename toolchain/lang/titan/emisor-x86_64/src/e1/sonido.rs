//! **EL SONIDO DE TITAN++** (Q2 de `docs/plan/EL_FOCO.md`, 15-10; el
//! propietario: *"titan que aprenda del audio y no inti"*).
//!
//! [carril]  VERDE     puertas sobre el asa del sonido, que vive en el bloque
//!                     de lo tenido (`entrada.rs`, `SONIDO` y `ARMADO`)
//! [consumo] NADA      una puerta por orden; las muestras no cruzan nunca
//!
//! Son las VOCES del orquestador (`AUDIO_OP_VOZ`, `<bmo/sonido.h>`): la app
//! presta una LAMINA con sus muestras -- 16 bits con signo, mono -- y DECLARA
//! que suena; el kernel las mezcla cada milisegundo, asi que una app que se
//! atasca ya no corta el sonido (lo que el reproductor de INTI tenia que
//! empujar a tiempo por el tubo, aqui lo hace el orquestador).
//!
//! ```text
//!    sonido.toma()                  reclamar + armar el tubo si nadie lo armo
//!    sonido.banco(l)                VOZ_BANCO con la base de la lamina
//!    sonido.toca(c, desde, n, hz)   VOZ_TOCAR, 16 bits, los dos lados a pleno
//!    sonido.suena(c)                VOZ_SUENA
//!    sonido.calla(c)                VOZ_CALLAR (255: todos)
//!    sonido.pita(hz, ms)            el altavoz del PC (BEEP)
//!    sonido.suelta()                callar todo, desarmar lo que se armo,
//!                                   soltar el sonido
//! ```
//!
//! Sin asa (no se tomo, o el kernel dijo que no) cada una es un NO callado:
//! `false`, 0, o nada -- nunca una puerta con un asa que no es suya --.

use super::entrada::{ARMADO, RBX, SONIDO};
use super::{Place, E1};
use bmo_abi::syscalls::surface::{AUDIO_OP_BEEP, AUDIO_OP_TUBO, AUDIO_OP_VOZ, CURRENT_TASK, TASK_OP_AUDIO_CLAIM, TASK_OP_AUDIO_RELEASE};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R8};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::Value;

/// Las subordenes del tubo y de las voces (las de `<bmo/sonido.h>`).
const TUBO_ABIERTO: i64 = 0;
const TUBO_ARMAR: i64 = 1;
const TUBO_CALLAR: i64 = 2;
const TUBO_ARMADO: i64 = 7;
const VOZ_BANCO: i64 = 1;
const VOZ_TOCAR: i64 = 2;
const VOZ_CALLAR: i64 = 4;
const VOZ_SUENA: i64 = 5;
const VOZ_SOLTAR: i64 = 6;
const VOZ_TODOS: i64 = 0xFF;
/// `como` de VOZ_TOCAR sin el canal ni los hercios: 16 bits con signo
/// (formato 1), los dos lados a pleno (256), sin bucle.
const COMO_FIJO: u64 = 1 << 8 | 256 << 18 | 256 << 27;

impl E1<'_> {
    /// El asa del sonido en `rdi`; salta (a rellenar) si no hay.
    fn con_asa(&mut self, no: &mut Vec<usize>) {
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        no.push(self.jcc(0x84));
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, SONIDO);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        no.push(self.jcc(0x84));
    }

    /// Una puerta sobre el asa: `op` y sus tres (`rdx`, `r10`, `r8` ya
    /// puestos si hacen falta: aqui solo `rdi` y `rsi`).
    fn sobre_asa(&mut self, op: u64) {
        self.imm(RSI, op as i64);
        self.invoke();
    }

    /// `sonido.toma()`.
    pub(super) fn sonido_toma(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_tenido(&mut no);
        // Ya es nuestro: si.
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, SONIDO);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let ya = self.jcc(0x85);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_AUDIO_CLAIM as i64);
        x86::zero_r32(&mut self.code, RDX);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
        self.si_no_vale(&mut no);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, SONIDO, RDX);
        // El tubo: si esta abierto y NADIE lo armo, se arma, y se apunta que
        // fue este programa (solo se desarma lo que se armo).
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RDX, TUBO_ABIERTO);
        self.sobre_asa(AUDIO_OP_TUBO);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let sin_tubo = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let sin_tubo2 = self.jcc(0x84);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, SONIDO);
        self.imm(RDX, TUBO_ARMADO);
        self.sobre_asa(AUDIO_OP_TUBO);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let armado = self.jcc(0x85);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, SONIDO);
        self.imm(RDX, TUBO_ARMAR);
        self.sobre_asa(AUDIO_OP_TUBO);
        self.imm(RAX, 1);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, ARMADO, RAX);
        for j in [sin_tubo, sin_tubo2, armado, ya] {
            self.here(j);
        }
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `sonido.banco(l)`: sus bytes, o 0.
    pub(super) fn sonido_banco(&mut self, args: &[Value]) -> Result<(Place, Class), String> {
        let [l] = args else { return Err("`sonido.banco` sin su lamina (fallo del compilador)".into()) };
        let (l, _) = self.eval(l)?;
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_asa(&mut no);
        self.imm(RDX, VOZ_BANCO);
        // La base de la lamina: la primera palabra de su asa.
        self.load(l, R10);
        x86::zero_r32(&mut self.code, R8);
        self.sobre_asa(AUDIO_OP_VOZ);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x85));
        self.store(out, RDX);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Int))
    }

    /// `sonido.toca(canal, desde, muestras, hz)`.
    pub(super) fn sonido_toca(&mut self, args: &[Value]) -> Result<(Place, Class), String> {
        let [c, desde, n, hz] = args else { return Err("`sonido.toca` con otros valores (fallo del compilador)".into()) };
        let (c, _) = self.eval(c)?;
        let (desde, _) = self.eval(desde)?;
        let (n, _) = self.eval(n)?;
        let (hz, _) = self.eval(hz)?;
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_asa(&mut no);
        // donde = desde (32 bits) | muestras << 32
        self.load(desde, R10);
        x86::mov_r32_r32(&mut self.code, R10, R10);
        self.load(n, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 32);
        x86::or_r64_r64(&mut self.code, R10, RAX);
        // como = canal (8 bits) | lo fijo | hz (20 bits) << 36
        self.load(c, R8);
        x86::and_r64_imm32(&mut self.code, R8, 0xFF);
        self.load(hz, RAX);
        x86::and_r64_imm32(&mut self.code, RAX, 0xF_FFFF);
        x86::shl_r64_imm8(&mut self.code, RAX, 36);
        x86::or_r64_r64(&mut self.code, R8, RAX);
        self.imm(RCX, COMO_FIJO as i64);
        x86::or_r64_r64(&mut self.code, R8, RCX);
        self.imm(RDX, VOZ_TOCAR);
        self.sobre_asa(AUDIO_OP_VOZ);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x85));
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        no.push(self.jcc(0x84));
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `sonido.suena(canal)`.
    pub(super) fn sonido_suena(&mut self, args: &[Value]) -> Result<(Place, Class), String> {
        let [c] = args else { return Err("`sonido.suena` sin su canal (fallo del compilador)".into()) };
        let (c, _) = self.eval(c)?;
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_asa(&mut no);
        self.imm(RDX, VOZ_SUENA);
        self.load(c, R10);
        x86::zero_r32(&mut self.code, R8);
        self.sobre_asa(AUDIO_OP_VOZ);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x85));
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        no.push(self.jcc(0x84));
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `sonido.calla(canal)`.
    pub(super) fn sonido_calla(&mut self, args: &[Value]) -> Result<(), String> {
        let [c] = args else { return Err("`sonido.calla` sin su canal (fallo del compilador)".into()) };
        let (c, _) = self.eval(c)?;
        let mut no = Vec::new();
        self.con_asa(&mut no);
        self.imm(RDX, VOZ_CALLAR);
        self.load(c, R10);
        x86::zero_r32(&mut self.code, R8);
        self.sobre_asa(AUDIO_OP_VOZ);
        for j in no {
            self.here(j);
        }
        Ok(())
    }

    /// `sonido.pita(hz, ms)`.
    pub(super) fn sonido_pita(&mut self, args: &[Value]) -> Result<(), String> {
        let [hz, ms] = args else { return Err("`sonido.pita` con otros valores (fallo del compilador)".into()) };
        let (hz, _) = self.eval(hz)?;
        let (ms, _) = self.eval(ms)?;
        let mut no = Vec::new();
        self.con_asa(&mut no);
        self.load(hz, RDX);
        self.load(ms, R10);
        x86::zero_r32(&mut self.code, R8);
        self.sobre_asa(AUDIO_OP_BEEP);
        for j in no {
            self.here(j);
        }
        Ok(())
    }

    /// `sonido.suelta()`.
    pub(super) fn sonido_suelta(&mut self) -> Result<(), String> {
        let mut no = Vec::new();
        self.con_asa(&mut no);
        self.imm(RDX, VOZ_CALLAR);
        self.imm(R10, VOZ_TODOS);
        x86::zero_r32(&mut self.code, R8);
        self.sobre_asa(AUDIO_OP_VOZ);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, SONIDO);
        self.imm(RDX, VOZ_SOLTAR);
        x86::zero_r32(&mut self.code, R10);
        self.sobre_asa(AUDIO_OP_VOZ);
        // Solo se desarma lo que este programa armo.
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, ARMADO);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let no_armo = self.jcc(0x84);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, SONIDO);
        self.imm(RDX, TUBO_CALLAR);
        self.sobre_asa(AUDIO_OP_TUBO);
        self.here(no_armo);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_AUDIO_RELEASE as i64);
        x86::zero_r32(&mut self.code, RDX);
        x86::zero_r32(&mut self.code, R10);
        self.invoke();
        x86::zero_r32(&mut self.code, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, SONIDO, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, ARMADO, RAX);
        for j in no {
            self.here(j);
        }
        Ok(())
    }
}
