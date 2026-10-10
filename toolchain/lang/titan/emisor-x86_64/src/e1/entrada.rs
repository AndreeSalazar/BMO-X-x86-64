//! **LO QUE LA VENTANA LEE Y LO QUE LE LLEGA** (F2 y F3 de
//! `docs/plan/EL_FOCO.md`, 10-10): los BYTES que tiene -- lo que alguien le
//! ofrecio (la pagina del antenista) o un fichero entero -- y las TECLAS y el
//! RATON de su buzon, como `navegar.inti` y `amarilla.inti` de INTI.
//!
//! ```text
//!    toma()          TASK_OP_TOMAR hasta ocho veces, un fotograma entre una y
//!                    otra (lo ofrecido puede llegar un poco despues de
//!                    lanzarnos); su base y sus bytes. Si, si lo tomo
//!    fichero(ruta)   la ruta de 8 en 8 (TASK_OP_RUTA), abrir, su medida (1 ..
//!                    256 KiB), un bloque suyo y ARCH_OP_LEER_EN hasta el
//!                    final; cerrar. Si, si lo leyo ENTERO
//!    medida()        cuantos bytes se tienen (0: ninguno)
//!    byte(i)         el byte i, o -1 fuera
//!    evento()        el siguiente del buzon (cabeza, cola, 64 ranuras) y QUE
//!                    es: 0 nada, 1 tecla pulsada, 2 soltada, 3 letra, 4 raton,
//!                    5 la ventana cambio. Se guarda: codigo(), raton_x(),
//!                    raton_y() y botones() lo leen
//!    se_ve()         la VISTA del estado del buzon: se ve (R-APP8)
//! ```
//!
//! ** LO TENIDO va en un bloque de 16 bytes del programa (base y bytes),
//! apuntado por `rbx` -- que E1 solo toca al escribir un NO y salir --,
//! pedido la primera vez: se lee ANTES de abrir la ventana (NAVEGAR elige su
//! medida segun haya pagina o no). El ULTIMO EVENTO va en la COLA PRIVADA de
//! la ventana (`ventana.rs`): sin ventana no llega nada.

use super::ventana::{BUZON, R12};
use super::{Place, E1};
use bmo_abi::syscalls::surface::{
    ARCH_OP_CERRAR, ARCH_OP_LEER_EN, ARCH_OP_MEDIDA, CURRENT_TASK, MEM_OP_BASE, NR_WAIT, PRESTADO_OP_BASE, PRESTADO_OP_BYTES, SUP_BUZON_CABECERA, SUP_BUZON_RANURA, SUP_EV_CARACTER, SUP_EV_CONFIGURE, SUP_EV_RATON, SUP_VISTA_SE_VE,
    TASK_OP_ARCHIVO_ABRIR, TASK_OP_MEMORIA_PEDIR, TASK_OP_RUTA, TASK_OP_TOMAR,
};
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R8};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{At, Director, Value};

/// El bloque de lo tenido, todo el programa (0: ninguno): su base y sus
/// bytes.
pub(crate) const RBX: u8 = 3;
const TENIDO_BASE: i32 = 0;
const TENIDO_BYTES: i32 = 8;
/// En la cola privada de la ventana: el ultimo evento.
pub const COLA_EVENTO: i32 = 32;
/// Lo mas que se lee de un fichero: 256 KiB, lo de NAVEGAR.
pub const FICHERO_MAXIMO: i32 = 256 * 1024;
/// Las veces que se intenta tomar lo ofrecido, y lo que se espera entre una y
/// otra: un fotograma.
const INTENTOS: i64 = 8;
const UN_FOTOGRAMA_NS: i64 = 16_000_000;
/// Del evento crudo: hay uno (bit 8) y la tecla esta pulsada (bit 9).
const HAY: u32 = 0x100;
const PULSADA: u32 = 0x200;

const _: () = assert!(SUP_EV_RATON == 1 << 63 && SUP_EV_CARACTER == 1 << 62 && SUP_EV_CONFIGURE == 1 << 61 && SUP_VISTA_SE_VE == 0);

/// `and <dst>, <src>` (64 bits).
fn and_r64_r64(code: &mut Vec<u8>, dst: u8, src: u8) {
    code.extend_from_slice(&[0x48 | if src >= 8 { 4 } else { 0 } | if dst >= 8 { 1 } else { 0 }, 0x21, 0xC0 | (src & 7) << 3 | (dst & 7)]);
}

impl E1<'_> {
    /// `rsi` = la cola privada de la ventana `rdi`: detras del buzon.
    fn cola(&mut self) {
        self.lea(RSI, RDI, 24);
        x86::mov_r32_at_reg(&mut self.code, RSI, RSI);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        self.lea(RSI, RSI, (SUP_BUZON_CABECERA + BUZON * SUP_BUZON_RANURA) as i32);
    }

    /// `rdi` = la ventana, y salta (a rellenar) si no hay.
    fn con_ventana(&mut self, saltos: &mut Vec<usize>) {
        x86::mov_r64_r64(&mut self.code, RDI, R12);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        saltos.push(self.jcc(0x84));
    }

    /// Lo que se pide al director de F2 y F3.
    pub(super) fn entrada(&mut self, what: Director, args: &[Value], at: At) -> Result<(Place, Class), String> {
        match what {
            Director::Toma => self.toma(),
            Director::Fichero => self.fichero(args, at),
            Director::Medida => self.medida(),
            Director::Byte => self.byte(args, at),
            Director::Evento => self.evento(),
            Director::Codigo => self.de_la_cola(COLA_EVENTO, 0, 0xFF, 0),
            Director::RatonX => self.de_la_cola(COLA_EVENTO, 16, 0xFFFF, 0),
            Director::RatonY => self.de_la_cola(COLA_EVENTO, 32, 0xFFFF, 0),
            Director::Botones => self.de_la_cola(COLA_EVENTO, 0, 0xFF, 0),
            Director::SeVe => self.se_ve(),
            _ => Err(format!("linea {}: `{}` no es de la entrada (fallo del compilador)", at.0, what.name())),
        }
    }

    /// Un campo de la cola privada: `(palabra >> desplaza) & mascara` (0: entera);
    /// sin ventana, `sin`.
    fn de_la_cola(&mut self, campo: i32, desplaza: u8, mascara: u32, sin: i64) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, sin);
        let mut no = Vec::new();
        self.con_ventana(&mut no);
        self.cola();
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RSI, campo);
        if desplaza > 0 {
            x86::shr_r64_imm8(&mut self.code, RAX, desplaza);
        }
        if mascara != 0 {
            x86::and_r64_imm32(&mut self.code, RAX, mascara);
        }
        self.store(out, RAX);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Int))
    }

    /// El bloque de lo tenido en `rbx`: si no lo hay, se pide (16 bytes, a
    /// cero). Salta (a rellenar) si el kernel no lo da.
    fn con_tenido(&mut self, no: &mut Vec<usize>) {
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        let ya = self.jcc(0x85);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MEMORIA_PEDIR as i64);
        self.imm(RDX, 16);
        self.invoke();
        self.si_no_vale(no);
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RSI, MEM_OP_BASE as i64);
        self.invoke();
        self.si_no_vale(no);
        x86::zero_r32(&mut self.code, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDX, TENIDO_BASE, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDX, TENIDO_BYTES, RAX);
        x86::mov_r64_r64(&mut self.code, RBX, RDX);
        self.here(ya);
    }

    /// Apunta lo tenido -- base en `base`, bytes en `bytes` -- en su bloque.
    fn tener(&mut self, base: Place, bytes: Place) {
        self.load(base, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, TENIDO_BASE, RAX);
        self.load(bytes, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, TENIDO_BYTES, RAX);
    }

    /// `director.medida()`.
    fn medida(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, 0);
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        let no = self.jcc(0x84);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, TENIDO_BYTES);
        self.store(out, RAX);
        self.here(no);
        Ok((out, Class::Int))
    }

    /// `INVOKE(rdi, rsi, rdx, r10, r8)` con `rdx`, `r10` y `r8` a cero.
    fn invoke_solo(&mut self, cap: Option<Place>, cap_imm: u64, op: u64) {
        match cap {
            Some(p) => self.load(p, RDI),
            None => self.imm(RDI, cap_imm as i64),
        }
        self.imm(RSI, op as i64);
        x86::zero_r32(&mut self.code, RDX);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
    }

    /// `director.toma()`.
    fn toma(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        let intentos = self.temp(8);
        let h = self.temp(8);
        let base = self.temp(8);
        let bytes = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.store_imm(intentos, INTENTOS);
        let vuelta = self.code.len();
        self.invoke_solo(None, CURRENT_TASK, TASK_OP_TOMAR);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let fallo = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let tomado = self.jcc(0x85);
        self.here(fallo);
        // Un fotograma, y otra vez.
        self.imm(RDX, UN_FOTOGRAMA_NS);
        x86::zero_r32(&mut self.code, RDI);
        x86::zero_r32(&mut self.code, RSI);
        self.imm(RAX, NR_WAIT as i64);
        x86::syscall(&mut self.code);
        self.load(intentos, RAX);
        x86::dec_r64(&mut self.code, RAX);
        self.store(intentos, RAX);
        let otra = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, otra, vuelta);
        no.push(self.jmp());
        self.here(tomado);
        self.store(h, RDX);
        self.invoke_solo(Some(h), 0, PRESTADO_OP_BASE);
        self.si_no_vale(&mut no);
        self.store(base, RDX);
        self.invoke_solo(Some(h), 0, PRESTADO_OP_BYTES);
        self.si_no_vale(&mut no);
        self.store(bytes, RDX);
        self.con_tenido(&mut no);
        self.tener(base, bytes);
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `director.fichero(ruta)`.
    fn fichero(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [ruta] = args else { return Err(format!("linea {}: `director.fichero` sin su ruta (fallo del compilador)", at.0)) };
        let (t, _) = self.eval(ruta)?;
        let out = self.temp(8);
        let k = self.temp(8);
        let f = self.temp(8);
        let medida = self.temp(8);
        let bloque = self.temp(8);
        let base = self.temp(8);
        let hechos = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_tenido(&mut no);
        self.load(t, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x84));
        // La ruta, de 8 en 8: lo de detras del largo, a cero.
        self.store_imm(k, 0);
        let ruta_vuelta = self.code.len();
        self.load(k, RAX);
        self.load(t, RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let abrir = self.jcc(0x8D);
        self.addr(t, RDI);
        x86::add_r64_r64(&mut self.code, RDI, RAX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, RDI, 8);
        x86::sub_r64_r64(&mut self.code, RCX, RAX);
        x86::cmp_r64_imm32(&mut self.code, RCX, 8);
        let entero = self.jcc(0x8D);
        x86::shl_r64_imm8(&mut self.code, RCX, 3);
        self.imm(R8, 1);
        x86::shl_r64_cl(&mut self.code, R8);
        x86::dec_r64(&mut self.code, R8);
        and_r64_r64(&mut self.code, RDX, R8);
        self.here(entero);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_RUTA as i64);
        self.invoke();
        self.load(k, RAX);
        x86::add_r64_imm8(&mut self.code, RAX, 8);
        self.store(k, RAX);
        let otra = self.jmp();
        x86::patch_jump_to(&mut self.code, otra, ruta_vuelta);
        self.here(abrir);
        self.invoke_solo(None, CURRENT_TASK, TASK_OP_ARCHIVO_ABRIR);
        self.si_no_vale(&mut no);
        self.store(f, RDX);
        let mut cerrar = Vec::new();
        self.invoke_solo(Some(f), 0, ARCH_OP_MEDIDA);
        self.si_no_vale(&mut cerrar);
        x86::cmp_r64_imm32(&mut self.code, RDX, FICHERO_MAXIMO);
        cerrar.push(self.jcc(0x87));
        self.store(medida, RDX);
        // Su bloque.
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_MEMORIA_PEDIR as i64);
        self.load(medida, RDX);
        self.invoke();
        self.si_no_vale(&mut cerrar);
        self.store(bloque, RDX);
        self.invoke_solo(Some(bloque), 0, MEM_OP_BASE);
        self.si_no_vale(&mut cerrar);
        self.store(base, RDX);
        // Leer hasta el final: (bloque, desde, cuantos) -> leidos.
        self.store_imm(hechos, 0);
        let leer = self.code.len();
        self.load(hechos, RAX);
        self.load(medida, RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let leido = self.jcc(0x83);
        self.load(f, RDI);
        self.imm(RSI, ARCH_OP_LEER_EN as i64);
        self.load(bloque, RDX);
        self.load(hechos, R10);
        self.load(medida, R8);
        x86::sub_r64_r64(&mut self.code, R8, R10);
        self.invoke();
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let corto = self.jcc(0x85);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let corto2 = self.jcc(0x84);
        self.load(hechos, RAX);
        x86::add_r64_r64(&mut self.code, RAX, RDX);
        self.store(hechos, RAX);
        let mas = self.jmp();
        x86::patch_jump_to(&mut self.code, mas, leer);
        self.here(leido);
        self.here(corto);
        self.here(corto2);
        self.invoke_solo(Some(f), 0, ARCH_OP_CERRAR);
        // Entero, o nada.
        self.load(hechos, RAX);
        self.load(medida, RCX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        no.push(self.jcc(0x85));
        self.tener(base, medida);
        self.store_imm(out, 1);
        let fin = self.jmp();
        for j in cerrar {
            self.here(j);
        }
        self.invoke_solo(Some(f), 0, ARCH_OP_CERRAR);
        for j in no {
            self.here(j);
        }
        self.here(fin);
        Ok((out, Class::Bool))
    }

    /// `director.byte(i)`.
    fn byte(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [i] = args else { return Err(format!("linea {}: `director.byte` sin su indice (fallo del compilador)", at.0)) };
        let (i, _) = self.eval(i)?;
        let out = self.temp(8);
        self.store_imm(out, -1);
        let mut no = Vec::new();
        x86::test_r64_r64(&mut self.code, RBX, RBX);
        no.push(self.jcc(0x84));
        self.load(i, RCX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, TENIDO_BYTES);
        x86::cmp_r64_r64(&mut self.code, RCX, RAX);
        no.push(self.jcc(0x83));
        x86::mov_r64_at_reg_disp32(&mut self.code, RDI, RBX, TENIDO_BASE);
        x86::movzx_r32_byte_base_index(&mut self.code, RAX, RDI, RCX);
        self.store(out, RAX);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Int))
    }

    /// `director.evento()`.
    fn evento(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_ventana(&mut no);
        // El buzon: cabeza en +0, cola en +4; igual, vacio (y el ultimo, 0).
        self.lea(RSI, RDI, 24);
        x86::mov_r32_at_reg(&mut self.code, RSI, RSI);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        x86::mov_r32_at_reg(&mut self.code, RAX, RSI);
        self.lea(R8, RSI, 4);
        x86::mov_r32_at_reg(&mut self.code, RCX, R8);
        x86::zero_r32(&mut self.code, RDX);
        x86::cmp_r64_r64(&mut self.code, RAX, RCX);
        let vacio = self.jcc(0x84);
        // El evento de la ranura `cola`, y la cola uno mas.
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        x86::and_r64_imm32(&mut self.code, RAX, (BUZON - 1) as u32);
        x86::shl_r64_imm8(&mut self.code, RAX, 3);
        x86::add_r64_r64(&mut self.code, RAX, RSI);
        x86::mov_r64_at_reg_disp32(&mut self.code, RDX, RAX, SUP_BUZON_CABECERA as i32);
        x86::inc_r64(&mut self.code, RCX);
        x86::and_r64_imm32(&mut self.code, RCX, (BUZON - 1) as u32);
        x86::mov_at_reg_from_r32(&mut self.code, R8, RCX);
        self.here(vacio);
        // Guardado en la cola privada.
        self.cola();
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RSI, COLA_EVENTO, RDX);
        // Que es: sin el bit de "hay", nada.
        x86::mov_r64_r64(&mut self.code, RAX, RDX);
        x86::and_r64_imm32(&mut self.code, RAX, HAY);
        no.push(self.jcc(0x84));
        let mut fin = Vec::new();
        for (bit, que) in [(63u8, 4i64), (62, 3), (61, 5)] {
            x86::mov_r64_r64(&mut self.code, RAX, RDX);
            x86::shl_r64_imm8(&mut self.code, RAX, 63 - bit);
            x86::shr_r64_imm8(&mut self.code, RAX, 63);
            let no_es = self.jcc(0x84);
            self.store_imm(out, que);
            fin.push(self.jmp());
            self.here(no_es);
        }
        // Una tecla: pulsada o soltada.
        self.store_imm(out, 2);
        x86::mov_r64_r64(&mut self.code, RAX, RDX);
        x86::and_r64_imm32(&mut self.code, RAX, PULSADA);
        fin.push(self.jcc(0x84));
        self.store_imm(out, 1);
        for j in no.into_iter().chain(fin) {
            self.here(j);
        }
        Ok((out, Class::Int))
    }

    /// `director.se_ve()`: la VISTA (byte 2 del estado alto del buzon) es "se
    /// ve". Sin ventana, no.
    fn se_ve(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        self.con_ventana(&mut no);
        self.lea(RSI, RDI, 24);
        x86::mov_r32_at_reg(&mut self.code, RSI, RSI);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        self.lea(RSI, RSI, 12);
        x86::mov_r32_at_reg(&mut self.code, RAX, RSI);
        x86::shr_r64_imm8(&mut self.code, RAX, 16);
        x86::and_r64_imm32(&mut self.code, RAX, 0xFF);
        no.push(self.jcc(0x85));
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }
}
