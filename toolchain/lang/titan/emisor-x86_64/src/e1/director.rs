//! **EL DIRECTOR, AL CORRER** (LB7b de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 09-10): la LAMINA de VERRANO escrita por un programa de TITAN++, como la
//! escribe el runtime de INTI (`verrano.inti`). La forma y el trato estan UNA
//! vez, en `platform/shared/verrano/src/lamina.rs`: de alli salen los numeros
//! de aqui, y la prueba (`tests/cubo_lamina.rs`) abre lo publicado con SU
//! lector.
//!
//! ```text
//!    r13             la lamina del programa, todo el programa (0: ninguna).
//!                    Solo si el programa habla con el director: los demas no
//!                    pagan ni una instruccion
//!    lamina(c)       el bloque -- la cabecera, dos ranuras de c vertices y 16
//!                    bytes de cola -- pedido al kernel; su cabecera ENTERA
//!                    antes de ofrecerlo; quien nos lanzo; y la OFERTA. Si la
//!                    toma, la lamina es esa: si. Sin padre, sin memoria, o
//!                    con lamina ya: no
//!    publica(f, n,   en la ranura que nadie lee (la de la secuencia + 1): su
//!            p, c)   sello a impar, los n vertices -- cuatro f32 de `p` y
//!                    cuatro de `c` cada uno --, su numero, su fotograma, el
//!                    sello a par y, LO ULTIMO, la secuencia: eso publica. No,
//!                    sin lamina, si n no son triangulos enteros, o si no
//!                    caben en la ranura o en las tablas
//!    espera(ms)      WAIT sin asa: dormir hasta el plazo (en el emulador
//!                    vuelve en el acto; WAIT es ADVISORY tambien en el metal)
//! ```
//!
//! ** Sin barreras: en x86-64 las escrituras se ven en el orden en que se
//! hacen (TSO), y aqui se hacen en el del trato. Y sin cerrojo: una app que
//! se para deja publicado su ultimo fotograma, y ya.
//!
//! ** La cola privada, como en INTI y en la superficie: 16 bytes DETRAS de lo
//! ofrecido, con el asa del bloque y lo ofrecido. El escritorio solo ve
//! `[l, l + bytes)`: un asa a la vista de otro proceso seria una capacidad
//! regalada.

use super::{Place, E1};
use super::entrada::{HZ, PLAZO, RBX};
use super::ventana::R12;
use bmo_abi::syscalls::surface::SUP_TOMADA;
use bmo_abi::syscalls::surface::{CURRENT_TASK, INFO_TSC_HZ, MEM_OP_BASE, MEM_OP_OFRECER, NR_INVOKE, NR_WAIT, TASK_OP_INFO, TASK_OP_MEMORIA_PEDIR, TASK_OP_MI_PADRE};
use bmo_lower::memoria;
use bmo_lower::x86::{self, RAX, RCX, RDI, RDX, RSI, R10, R11, R8, R9};
use bmo_titan_front::calc::Class;
use bmo_titan_front::ir::{At, Director, Value};
use bmo_verrano::lamina::{CABECERA, CAMPO_CAPACIDAD, CAMPO_FOTOGRAMA, CAMPO_MAGIA, CAMPO_SECUENCIA, CAMPO_SELLO, CAMPO_VERSION, CAMPO_VERTICES, MAGIA, VERSION};
use bmo_verrano::{VERTEX_BYTES, VERTEX_COLOR, VERTEX_POSITION};

/// La lamina del programa, todo el programa.
pub(crate) const R13: u8 = 13;
/// Los bytes de cola detras de lo ofrecido.
const COLA: i32 = 16;
/// Lo mas que cabe en una ranura: 65536 vertices (4 MiB las dos).
const CAPACIDAD_MAXIMA: i32 = 1 << 16;
/// Lo mas que duerme `espera`: una hora (los nanosegundos caben de sobra).
const ESPERA_MAXIMA: i32 = 3_600_000;
/// Hasta cuanto se pace por plazo (ms): un fotograma; mas, la siesta (y asi
/// `(plazo - ahora) * 1e9` cabe en 64 bits con cualquier reloj de hasta 9 GHz).
const PLAZO_MAXIMO_MS: i32 = 1000;
/// Q0a4: las miradas a que el escritorio tome la ventana antes de ofrecer la
/// lamina, un fotograma entre una y otra (medio segundo).
const ESPERAS_TOMA: i64 = 30;
const UN_FOTOGRAMA_NS: i64 = 16_000_000;
/// El campo de la cabecera BSUP con donde empieza el buzon (`superficie.rs`).
const SUP_CAMPO_BUZON: u64 = 6;

// El vertice de VERRANO: su posicion y su color, cuatro f32 cada uno, en ese
// orden. La copia de `publica` lo escribe asi; si `bmo_verrano` cambiara, esto
// no compila.
const _: () = assert!(VERTEX_BYTES == 32 && VERTEX_POSITION == 0 && VERTEX_COLOR == 16);
// La cabecera: magia y version en la primera palabra de 64 bits, la medida de
// la cabecera y la capacidad en la segunda.
const _: () = assert!(CAMPO_MAGIA == 0 && CAMPO_VERSION == 1 && CAMPO_CAPACIDAD == 3 && CABECERA == 64);

/// El byte de la palabra `campo` de la cabecera.
const fn byte(campo: usize) -> i32 {
    4 * campo as i32
}

impl E1<'_> {
    /// **Una llamada al director**: lo que da (un si/no), o nada (`espera`).
    pub fn director(&mut self, what: Director, args: &[Value], at: At) -> Result<Option<(Place, Class)>, String> {
        match what {
            Director::Lamina => self.lamina(args, at).map(Some),
            Director::Publica => self.publica(args, at).map(Some),
            Director::Espera => self.espera(args).map(|_| None),
            // F1 (EL_FOCO): la ventana (`ventana.rs`).
            Director::Ventana => self.ventana(args, at).map(Some),
            Director::Pixel => self.pixel(args, at).map(|_| None),
            Director::Rect => self.rect(args, at).map(|_| None),
            Director::Fila => self.fila(args, at).map(|_| None),
            Director::Presenta => self.presenta().map(|_| None),
            // F2 y F3 (EL_FOCO): lo que se lee y lo que llega (`entrada.rs`),
            // y la letra (`letra.rs`).
            Director::Letra => self.letra(args, at).map(Some),
            Director::Texto => self.texto(args, at).map(Some),
            // TA4 (10-10): lo que se escribe en el disco (`disco.rs`).
            Director::Guarda | Director::Crea | Director::Cierra => self.disco(what, args, at).map(Some),
            Director::Escribe => self.escribe_byte(args, at).map(|_| None),
            // TA5 (10-10): el reloj, para medir.
            Director::Ms => self.ms().map(Some),
            Director::Toma | Director::Fichero | Director::Medida | Director::Byte | Director::Evento | Director::Codigo | Director::RatonX | Director::RatonY | Director::Botones | Director::SeVe => self.entrada(what, args, at).map(Some),
        }
    }

    /// `director.ms()` (TA5): `rdtsc` y los ciclos por segundo que dice el
    /// kernel (`INFO_TSC_HZ`), en milisegundos. Si el kernel no lo sabe, 0:
    /// una resta de ceros dice "no se mide", no un tiempo inventado.
    fn ms(&mut self) -> Result<(Place, Class), String> {
        let out = self.temp(8);
        let tsc = self.temp(8);
        self.code.extend_from_slice(&[0x0F, 0x31]);
        x86::shl_r64_imm8(&mut self.code, RDX, 32);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        self.store(tsc, RAX);
        self.store_imm(out, 0);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_INFO as i64);
        self.imm(RDX, INFO_TSC_HZ as i64);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
        let mut nada = Vec::new();
        self.si_no_vale(&mut nada);
        // ms = tsc / (hz / 1000): hz / 1000 > 0 con cualquier reloj de verdad
        x86::mov_r64_r64(&mut self.code, RAX, RDX);
        x86::zero_r32(&mut self.code, RDX);
        self.imm(RCX, 1000);
        x86::div_r64(&mut self.code, RCX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        nada.push(self.jcc(0x84));
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        self.load(tsc, RAX);
        x86::zero_r32(&mut self.code, RDX);
        x86::div_r64(&mut self.code, RCX);
        self.store(out, RAX);
        for s in nada {
            self.here(s);
        }
        Ok((out, Class::Int))
    }

    /// `INVOKE(rdi, rsi, rdx, r10, r8)`: el codigo en `rax`, el valor en `rdx`.
    pub(super) fn invoke(&mut self) {
        self.imm(RAX, NR_INVOKE as i64);
        x86::syscall(&mut self.code);
    }

    /// Salta (a rellenar) si `rax` no es 0 -- el kernel dijo que no -- o si el
    /// valor de `rdx` es 0.
    pub(super) fn si_no_vale(&mut self, saltos: &mut Vec<usize>) {
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        saltos.push(self.jcc(0x85));
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        saltos.push(self.jcc(0x84));
    }

    /// `director.lamina(c)`.
    fn lamina(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [c] = args else { return Err(format!("linea {}: `director.lamina` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (c, _) = self.eval(c)?;
        let out = self.temp(8);
        let bytes = self.temp(8);
        let bloque = self.temp(8);
        let base = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        // Una lamina por programa.
        x86::test_r64_r64(&mut self.code, R13, R13);
        no.push(self.jcc(0x85));
        // La capacidad, de 1 a CAPACIDAD_MAXIMA.
        self.load(c, RAX);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        no.push(self.jcc(0x8E));
        x86::cmp_r64_imm32(&mut self.code, RAX, CAPACIDAD_MAXIMA);
        no.push(self.jcc(0x8F));
        // Lo ofrecido: la cabecera y las dos ranuras (2 * 32 = 2^6 por vertice).
        x86::shl_r64_imm8(&mut self.code, RAX, 6);
        self.lea(RAX, RAX, CABECERA as i32);
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
        // La cabecera ENTERA antes de ofrecer (si el escritorio la tomara
        // antes, leeria basura): sus 64 bytes a cero, y lo que no es cero.
        x86::mov_r64_r64(&mut self.code, RDI, RDX);
        self.imm(RCX, CABECERA as i64);
        x86::zero_r32(&mut self.code, RAX);
        memoria::rellenar(&mut self.code);
        self.load(base, RDI);
        self.imm(RAX, (MAGIA as u64 | (VERSION as u64) << 32) as i64);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, byte(CAMPO_MAGIA), RAX);
        self.load(c, RAX);
        x86::shl_r64_imm8(&mut self.code, RAX, 32);
        self.imm(RCX, CABECERA as i64);
        x86::or_r64_r64(&mut self.code, RAX, RCX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, byte(CAMPO_CABECERA_Y_CAPACIDAD), RAX);
        // La cola: el asa del bloque y lo ofrecido.
        self.load(bytes, RCX);
        x86::add_r64_r64(&mut self.code, RDI, RCX);
        self.load(bloque, RAX);
        x86::mov_at_reg_from_r64(&mut self.code, RDI, RAX);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RDI, 8, RCX);
        // ** Q0a4 (EL_FOCO, 10-10): con VENTANA, primero que el escritorio la
        // TOME (`SUP_TOMADA` en el estado de su buzon): ofrecer otra cosa al
        // MISMO destino sustituye una oferta aun no tomada (`loan::offer`), y
        // la ventana se perderia. Un fotograma entre mirada y mirada, hasta
        // `ESPERAS_TOMA`; despues se ofrece igual (sin escritorio no hay quien).
        x86::mov_r64_r64(&mut self.code, RDI, R12);
        x86::test_r64_r64(&mut self.code, RDI, RDI);
        let sin_ventana = self.jcc(0x84);
        let intentos = self.temp(8);
        self.store_imm(intentos, ESPERAS_TOMA);
        let mira = self.code.len();
        x86::mov_r64_r64(&mut self.code, RDI, R12);
        self.lea(RSI, RDI, 4 * SUP_CAMPO_BUZON as i32);
        x86::mov_r32_at_reg(&mut self.code, RSI, RSI);
        x86::add_r64_r64(&mut self.code, RSI, RDI);
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RSI, 8);
        x86::and_r64_imm32(&mut self.code, RAX, SUP_TOMADA as u32);
        let tomada = self.jcc(0x85);
        self.imm(RDX, UN_FOTOGRAMA_NS);
        x86::zero_r32(&mut self.code, RDI);
        x86::zero_r32(&mut self.code, RSI);
        self.imm(RAX, NR_WAIT as i64);
        x86::syscall(&mut self.code);
        self.load(intentos, RAX);
        x86::dec_r64(&mut self.code, RAX);
        self.store(intentos, RAX);
        let otra = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, otra, mira);
        self.here(tomada);
        self.here(sin_ventana);
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
        // La tomo: la lamina es esta.
        self.load(base, R13);
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `director.publica(f, n, posiciones, colores)`.
    fn publica(&mut self, args: &[Value], at: At) -> Result<(Place, Class), String> {
        let [f, n, p, c] = args else { return Err(format!("linea {}: `director.publica` con {} valores (fallo del compilador)", at.0, args.len())) };
        let (f, _) = self.eval(f)?;
        let (n, _) = self.eval(n)?;
        let (p, cp) = self.eval(p)?;
        let (c, _) = self.eval(c)?;
        let Class::Table(_, celdas) = cp else { return Err(format!("linea {}: `director.publica` sin tablas (fallo del compilador)", at.0)) };
        let out = self.temp(8);
        self.store_imm(out, 0);
        let mut no = Vec::new();
        // Sin lamina, nada.
        x86::test_r64_r64(&mut self.code, R13, R13);
        no.push(self.jcc(0x84));
        // n: de 0 a lo que traen las tablas y a lo que cabe en la ranura, y
        // triangulos enteros.
        self.load(n, RCX);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        no.push(self.jcc(0x8C));
        x86::cmp_r64_imm32(&mut self.code, RCX, (celdas / 4) as i32);
        no.push(self.jcc(0x8F));
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, R13, byte(CAMPO_CAPACIDAD - 1));
        x86::shr_r64_imm8(&mut self.code, RAX, 32);
        x86::cmp_r64_r64(&mut self.code, RCX, RAX);
        no.push(self.jcc(0x8F));
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        x86::zero_r32(&mut self.code, RDX);
        self.imm(R8, 3);
        x86::div_r64(&mut self.code, R8);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        no.push(self.jcc(0x85));
        // La ranura que nadie lee: k = (secuencia + 1) % 2, en r8.
        self.lea(RDI, R13, byte(CAMPO_SECUENCIA));
        x86::mov_r32_at_reg(&mut self.code, R8, RDI);
        x86::inc_r64(&mut self.code, R8);
        x86::and_r64_imm32(&mut self.code, R8, 1);
        // Su sello, a IMPAR: "estoy escribiendo aqui". En rdi su sitio y en
        // r11 lo que vale ahora (sello + 1): al acabar, uno mas.
        x86::mov_r64_r64(&mut self.code, RDI, R8);
        x86::shl_r64_imm8(&mut self.code, RDI, 2);
        x86::add_r64_r64(&mut self.code, RDI, R13);
        self.lea(RDI, RDI, byte(CAMPO_SELLO));
        x86::mov_r32_at_reg(&mut self.code, R11, RDI);
        x86::inc_r64(&mut self.code, R11);
        x86::mov_at_reg_from_r32(&mut self.code, RDI, R11);
        // Donde empieza la ranura: r9 = lamina + CABECERA + k * capacidad * 32.
        x86::mov_r64_at_reg_disp32(&mut self.code, R9, R13, byte(CAMPO_CAPACIDAD - 1));
        x86::shr_r64_imm8(&mut self.code, R9, 32);
        x86::shl_r64_imm8(&mut self.code, R9, 5);
        x86::imul_r64_r64(&mut self.code, R9, R8);
        x86::add_r64_r64(&mut self.code, R9, R13);
        self.lea(R9, R9, CABECERA as i32);
        // Los n vertices: cuatro f32 de cada tabla (una celda son 8 bytes, el
        // f32 en la mitad baja), su posicion y su color.
        self.addr(p, RSI);
        self.addr(c, RDX);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let vacio = self.jcc(0x84);
        let vuelta = self.code.len();
        for tabla in [RSI, RDX] {
            for k in 0..4 {
                x86::mov_r64_at_reg_disp32(&mut self.code, RAX, tabla, 8 * k);
                x86::mov_at_reg_from_r32(&mut self.code, R9, RAX);
                x86::add_r64_imm8(&mut self.code, R9, 4);
            }
            x86::add_r64_imm8(&mut self.code, tabla, 32);
        }
        x86::dec_r64(&mut self.code, RCX);
        let mas = self.jcc(0x85);
        x86::patch_jump_to(&mut self.code, mas, vuelta);
        self.here(vacio);
        // Su numero de vertices y su fotograma, en la ranura k.
        x86::mov_r64_r64(&mut self.code, RSI, R8);
        x86::shl_r64_imm8(&mut self.code, RSI, 2);
        x86::add_r64_r64(&mut self.code, RSI, R13);
        self.load(n, RAX);
        self.lea(RCX, RSI, byte(CAMPO_VERTICES));
        x86::mov_at_reg_from_r32(&mut self.code, RCX, RAX);
        self.load(f, RAX);
        self.lea(RCX, RSI, byte(CAMPO_FOTOGRAMA));
        x86::mov_at_reg_from_r32(&mut self.code, RCX, RAX);
        // El sello, a PAR: la ranura esta entera.
        x86::inc_r64(&mut self.code, R11);
        x86::mov_at_reg_from_r32(&mut self.code, RDI, R11);
        // Y LO ULTIMO, la secuencia: la publicacion.
        self.lea(RDI, R13, byte(CAMPO_SECUENCIA));
        x86::mov_r32_at_reg(&mut self.code, RAX, RDI);
        x86::inc_r64(&mut self.code, RAX);
        x86::mov_at_reg_from_r32(&mut self.code, RDI, RAX);
        self.store_imm(out, 1);
        for j in no {
            self.here(j);
        }
        Ok((out, Class::Bool))
    }

    /// `director.espera(ms)`: dormir hasta el SIGUIENTE PLAZO, no `ms` mas.
    ///
    /// ** EL CUELLO DE BOTELLA MEDIDO (S0 de `PLAN_VERRANO`, 09-10): el
    /// fotograma del cubo media 18,7 ms y el 92 % era esperar -- la app hacia
    /// su trabajo y DESPUES dormia 16 ms, y el kernel la despertaba tarde --.
    /// Ahora (10-10) el programa lleva su plazo (el TSC del fotograma
    /// siguiente, en su bloque de 64 bytes) y duerme solo lo que FALTA hasta
    /// el: el fotograma mide `ms`, no trabajo + `ms` + retraso. Si ya va
    /// tarde, cede el turno (WAIT de 0) y se pone en hora, sin amontonar.
    /// Mas de un segundo, o sin bloque, la siesta de siempre.
    fn espera(&mut self, args: &[Value]) -> Result<(), String> {
        let [ms] = args else { return Err("`director.espera` sin su valor (fallo del compilador)".to_string()) };
        let (ms, _) = self.eval(ms)?;
        let pedido = self.temp(8);
        let por = self.temp(8);
        let ahora = self.temp(8);
        self.load(ms, RDX);
        x86::test_r64_r64(&mut self.code, RDX, RDX);
        let nada = self.jcc(0x8E);
        x86::cmp_r64_imm32(&mut self.code, RDX, ESPERA_MAXIMA);
        let cabe = self.jcc(0x8E);
        self.imm(RDX, ESPERA_MAXIMA as i64);
        self.here(cabe);
        self.store(pedido, RDX);
        let mut siesta = Vec::new();
        x86::cmp_r64_imm32(&mut self.code, RDX, PLAZO_MAXIMO_MS);
        siesta.push(self.jcc(0x8F));
        self.con_tenido(&mut siesta);
        // Los ciclos por segundo del reloj, una vez.
        x86::mov_r64_at_reg_disp32(&mut self.code, RAX, RBX, HZ);
        x86::test_r64_r64(&mut self.code, RAX, RAX);
        let tiene = self.jcc(0x85);
        self.imm(RDI, CURRENT_TASK as i64);
        self.imm(RSI, TASK_OP_INFO as i64);
        self.imm(RDX, INFO_TSC_HZ as i64);
        x86::zero_r32(&mut self.code, R10);
        x86::zero_r32(&mut self.code, R8);
        self.invoke();
        self.si_no_vale(&mut siesta);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, HZ, RDX);
        self.here(tiene);
        // Lo que mide un fotograma: ms * hz / 1000 ciclos.
        self.load(pedido, RAX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, HZ);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        x86::zero_r32(&mut self.code, RDX);
        self.imm(RCX, 1000);
        x86::div_r64(&mut self.code, RCX);
        self.store(por, RAX);
        // Ahora: rdtsc.
        self.code.extend_from_slice(&[0x0F, 0x31]);
        x86::shl_r64_imm8(&mut self.code, RDX, 32);
        x86::or_r64_r64(&mut self.code, RAX, RDX);
        self.store(ahora, RAX);
        // El plazo: el anterior (o ahora, la primera vez) mas un fotograma.
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, PLAZO);
        x86::test_r64_r64(&mut self.code, RCX, RCX);
        let hay = self.jcc(0x85);
        x86::mov_r64_r64(&mut self.code, RCX, RAX);
        self.here(hay);
        self.load(por, RDX);
        x86::add_r64_r64(&mut self.code, RCX, RDX);
        x86::cmp_r64_r64(&mut self.code, RCX, RAX);
        let a_tiempo = self.jcc(0x87);
        // Tarde: en hora, y se cede el turno.
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, PLAZO, RAX);
        x86::zero_r32(&mut self.code, RDX);
        let cede = self.jmp();
        self.here(a_tiempo);
        x86::mov_at_reg_disp32_from_r64(&mut self.code, RBX, PLAZO, RCX);
        // Lo que falta, en ns: (plazo - ahora) * 1e9 / hz.
        x86::sub_r64_r64(&mut self.code, RCX, RAX);
        x86::mov_r64_r64(&mut self.code, RAX, RCX);
        self.imm(RCX, 1_000_000_000);
        x86::imul_r64_r64(&mut self.code, RAX, RCX);
        x86::zero_r32(&mut self.code, RDX);
        x86::mov_r64_at_reg_disp32(&mut self.code, RCX, RBX, HZ);
        x86::div_r64(&mut self.code, RCX);
        x86::mov_r64_r64(&mut self.code, RDX, RAX);
        self.here(cede);
        let dormir = self.jmp();
        // La siesta de siempre: `ms` desde ahora.
        for j in siesta {
            self.here(j);
        }
        self.load(pedido, RDX);
        self.imm(RAX, 1_000_000);
        x86::imul_r64_r64(&mut self.code, RDX, RAX);
        self.here(dormir);
        x86::zero_r32(&mut self.code, RDI);
        x86::zero_r32(&mut self.code, RSI);
        self.imm(RAX, NR_WAIT as i64);
        x86::syscall(&mut self.code);
        self.here(nada);
        Ok(())
    }
}

/// La palabra de 64 bits que lleva la medida de la cabecera (abajo) y la
/// capacidad (arriba): la de las palabras 2 y 3.
const CAMPO_CABECERA_Y_CAPACIDAD: usize = CAMPO_CAPACIDAD - 1;
