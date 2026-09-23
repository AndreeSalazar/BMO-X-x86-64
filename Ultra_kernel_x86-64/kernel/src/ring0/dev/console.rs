//! Serial port (COM1: 0x3F8) ??? debug output.
//!
//! [carril]  VERDE     el puerto serie: escribir bytes y ya
//! [consumo] NADA      el serial: escribe cuando alguien escribe
//!
//! This is the lowest-level output device; every other subsystem
//! (logger, diagnostics, ring3 debug print) routes through here.
//!
//! # ** Y POR ESO NO PUEDE HACER ESPERAR A NADIE (2026-09-23)
//!
//! Hasta hoy `serial_write_byte` GIRABA por cada byte hasta que el UART
//! vaciaba su FIFO: a 115200 baudios son ~87 us por byte, y una linea de log
//! de 50 caracteres, ~4 ms. Lo pagaba QUIEN HABLABA, fuera quien fuera: el
//! hilo del bus USB (prioridad 2), un syscall con las interrupciones cerradas.
//! El `save` del 23-09 (06:57) lo midio sin saberlo: `latido tarde 68 ms`, la
//! vuelta del bus 72 ms, y el CPU la tuvo EL PROPIO hilo del bus 59 de ellos
//! -- girando sobre el puerto serie mientras escribia las ~15 lineas de
//! `[uhid] iface=... clase=...` de instalar el audifono.
//!
//! Ahora el puerto es un CONSUMIDOR, no un embudo: quien escribe apunta el
//! byte en una cola de RAM y se va, y la tarea IDLE --la que corre cuando
//! nadie mas quiere el CPU-- llena la FIFO del UART cuando esta vacia. Si la
//! cola se llena, el byte se CUENTA como perdido: nunca se espera. Y el texto
//! entero sigue yendo antes a la caja negra en RAM (`cabina::caida`), que es
//! donde de verdad se lee despues.
//!
//! Dos momentos en los que sigue siendo SINCRONO, a proposito:
//!   * el arranque, hasta que hay IDLE: si la maquina se cuelga antes, lo que
//!     dijo tiene que haber salido ya (`abrir_cola`);
//!   * una pantalla azul: la maquina se para y nadie va a vaciar nada
//!     (`directo`, que primero vacia lo que quedaba).


const COM1: u16 = 0x3F8;

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v); }
    v
}

pub fn init() {
    outb(COM1 + 1, 0x00);  // Disable IRQs
    outb(COM1 + 3, 0x80);  // DLAB
    outb(COM1 + 0, 0x01);  // 115200 baud
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);  // 8N1
    outb(COM1 + 2, 0xC7);  // FIFO
    outb(COM1 + 4, 0x0B);
}

#[inline(never)]
pub fn serial_write_byte(b: u8) {
    // ** PRIMERO A LA CAJA NEGRA EN RAM, y antes del puerto serie a proposito:
    // el serie puede no existir; la RAM esta siempre. Este es EL embudo --nueve
    // sitios escriben aqui-- asi que un solo gancho captura todo lo que la
    // maquina dice.
    crate::ring0::cabina::caida::anotar(b);
    if COLA_ABIERTA.load(Ordering::Acquire) {
        encolar(b);
    } else {
        byte_directo(b);
    }
}

// -- La cola ---------------------------------------------------------------

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 16 KiB: a 115200 baudios, ~1,4 s de texto. Un arranque que enumera el USB
/// no lo llena; si algo lo llena, lo que sobra se cuenta y la caja negra lo
/// tiene igual.
const COLA: usize = 16 * 1024;
/// La FIFO de transmision de un 16550: lo que se le puede dar de una vez
/// cuando dice que esta vacia (LSR bit 5 con la FIFO encendida en `init`).
const FIFO: usize = 16;
const LSR_VACIA: u8 = 0x20;

static mut BYTES: [u8; COLA] = [0; COLA];
/// Bytes apuntados y bytes ya dados al UART, desde el arranque. Su resta es lo
/// que espera; los dos solo suben.
static APUNTADOS: AtomicU64 = AtomicU64::new(0);
static SALIDOS: AtomicU64 = AtomicU64::new(0);
/// Bytes que no cupieron (cola llena, o el guardia ocupado por un NMI).
static PERDIDOS: AtomicU64 = AtomicU64::new(0);
/// El mayor numero de bytes que llego a haber esperando.
static PICO: AtomicU64 = AtomicU64::new(0);
static COLA_ABIERTA: AtomicBool = AtomicBool::new(false);
/// El guardia: apuntar y vaciar tocan los mismos indices, y un obrero de AXION
/// puede escribir a la vez que el BSP.
static GUARDIA: AtomicBool = AtomicBool::new(false);

/// Toma el guardia con las interrupciones cerradas, o devuelve `None` si no
/// pudo en unas pocas vueltas. No espera de verdad NUNCA: quien no lo consigue
/// pierde el byte (y se cuenta) en vez de girar -- que es lo que este fichero
/// dejo de hacer.
fn guardia() -> Option<u64> {
    let flags: u64;
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) flags, options(nomem)) };
    for _ in 0..64 {
        if GUARDIA
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Some(flags);
        }
        core::hint::spin_loop();
    }
    soltar_flags(flags);
    None
}

fn soltar(flags: u64) {
    GUARDIA.store(false, Ordering::Release);
    soltar_flags(flags);
}

fn soltar_flags(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
    }
}

fn encolar(b: u8) {
    let Some(flags) = guardia() else {
        PERDIDOS.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let a = APUNTADOS.load(Ordering::Relaxed);
    let esperando = a - SALIDOS.load(Ordering::Relaxed);
    if esperando as usize >= COLA {
        PERDIDOS.fetch_add(1, Ordering::Relaxed);
    } else {
        unsafe { (*core::ptr::addr_of_mut!(BYTES))[a as usize % COLA] = b };
        APUNTADOS.store(a + 1, Ordering::Release);
        if esperando + 1 > PICO.load(Ordering::Relaxed) {
            PICO.store(esperando + 1, Ordering::Relaxed);
        }
    }
    soltar(flags);
}

/// **Da al UART lo que quepa en su FIFO, si esta vacia.** Lo llama la tarea
/// IDLE en cada vuelta. Sin nada esperando no toca el puerto (una lectura de
/// memoria); con algo, una lectura de `LSR` y como mucho 16 `out`.
///
/// Sin UART en la placa, `LSR` no dice nunca "vacia": la cola se llena, lo que
/// sobra se cuenta, y NADIE gira -- el viejo guardia de 100.000 vueltas por
/// byte era en ese caso lo que mas costaba.
pub fn vaciar() {
    if APUNTADOS.load(Ordering::Acquire) == SALIDOS.load(Ordering::Relaxed) {
        return;
    }
    let Some(flags) = guardia() else { return };
    if inb(COM1 + 5) & LSR_VACIA != 0 {
        let mut s = SALIDOS.load(Ordering::Relaxed);
        let a = APUNTADOS.load(Ordering::Relaxed);
        let mut n = 0;
        while s < a && n < FIFO {
            outb(COM1, unsafe { (*core::ptr::addr_of!(BYTES))[s as usize % COLA] });
            s += 1;
            n += 1;
        }
        SALIDOS.store(s, Ordering::Release);
    }
    soltar(flags);
}

/// **Desde aqui se escribe por la cola.** Lo llama el arranque en cuanto hay
/// tarea IDLE, que es quien la vacia. Antes, cada byte sale en el acto.
pub fn abrir_cola() {
    COLA_ABIERTA.store(true, Ordering::Release);
}

/// **Vuelta al puerto directo, vaciando antes lo que habia.** Para una
/// pantalla azul: la maquina se va a parar y nadie vaciara la cola despues.
pub fn directo() {
    COLA_ABIERTA.store(false, Ordering::Release);
    // El guardia puede tenerlo quien fallo: se vacia sin el. Con la maquina
    // cayendo, un byte repetido o partido vale mas que el silencio.
    let a = APUNTADOS.load(Ordering::Acquire);
    let mut s = SALIDOS.load(Ordering::Relaxed);
    let desde = a.saturating_sub(COLA as u64).max(s);
    s = desde;
    while s < a {
        byte_directo(unsafe { (*core::ptr::addr_of!(BYTES))[s as usize % COLA] });
        s += 1;
    }
    SALIDOS.store(s, Ordering::Release);
}

// -- El contrato, espejo de `bmo_abi::...::informe::SERIE_*` ---------------

pub const SERIE_APUNTADOS_MASK: u64 = 0xFFFF_FFFF;
pub const SERIE_PERDIDOS_SHIFT: u64 = 32;
pub const SERIE_PERDIDOS_MASK: u64 = 0xFFFF;
pub const SERIE_PICO_SHIFT: u64 = 48;
pub const SERIE_PICO_MASK: u64 = 0x7FFF;
pub const SERIE_COLA: u64 = 1 << 63;

/// `INFO_SERIE`, empaquetado y saturado. Ver el ABI.
pub fn cuentas() -> u64 {
    APUNTADOS.load(Ordering::Relaxed).min(SERIE_APUNTADOS_MASK)
        | PERDIDOS.load(Ordering::Relaxed).min(SERIE_PERDIDOS_MASK) << SERIE_PERDIDOS_SHIFT
        | PICO.load(Ordering::Relaxed).min(SERIE_PICO_MASK) << SERIE_PICO_SHIFT
        | if COLA_ABIERTA.load(Ordering::Relaxed) { SERIE_COLA } else { 0 }
}

/// Un byte al puerto, esperando a que el UART lo acepte. Solo el arranque
/// antes de la cola y la pantalla azul.
///
/// [!] El tope de vueltas no es "~50 us": cada `inb` a un puerto de E/S del
/// LPC cuesta del orden de un microsegundo, asi que 100.000 son ~0,1 s por
/// byte si NO hay UART. Por eso solo se usa donde no hay otra salida.
fn byte_directo(b: u8) {
    let mut timeout = 100_000u32;
    while inb(COM1 + 5) & LSR_VACIA == 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
    outb(COM1, b);
}

/// ** `inline(never)`, y es medido (2026-09-23): al hacerse corta con la cola,
/// el compilador la COPIO en cada llamada -- cientos en el shell y el arranque --
/// y el `.text` crecio 107 KB (`run_shell` +59 KB, `phase::main` +17 KB). Es
/// el embudo de una salida lenta: copiarlo no la acelera, solo engorda.
#[inline(never)]
pub fn serial_write(s: &str) {
    for b in s.bytes() {
        if b == b'\n' { serial_write_byte(b'\r'); }
        serial_write_byte(b);
    }
}

pub fn serial_read_byte() -> Option<u8> {
    if inb(COM1 + 5) & 1 != 0 {
        Some(inb(COM1))
    } else {
        None
    }
}

/// Write a u64 to serial as hex (without 0x prefix).
#[inline(never)]
pub fn serial_write_u64(value: u64, min_width: usize) {
    if value == 0 {
        for _ in 0..min_width.min(1) {
            serial_write_byte(b'0');
        }
        if min_width == 0 {
            serial_write_byte(b'0');
        }
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut v = value;
    while v > 0 {
        let digit = (v & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        v >>= 4;
        i += 1;
    }
    while i < min_width && i < buf.len() {
        buf[i] = b'0';
        i += 1;
    }
    while i > 0 {
        i -= 1;
        serial_write_byte(buf[i]);
    }
}

/// Write a u64 to serial as decimal.
#[inline(never)]
pub fn serial_write_u64_dec(value: u64) {
    if value == 0 {
        serial_write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = value;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    while i < buf.len() {
        serial_write_byte(buf[i]);
        i += 1;
    }
}

/// Write a u32 to serial as hex padded to 8 chars (no prefix).
pub fn serial_write_u32_hex(value: u32) {
    serial_write_u64(value as u64, 8);
}

/// Convenience: write `name = 0xVAL (DEC)` on one line.
pub fn serial_kv_u64(name: &str, val: u64) {
    serial_write(name);
    serial_write(" = 0x");
    serial_write_u64(val, 16);
    serial_write(" (");
    serial_write_u64_dec(val);
    serial_write(")\n");
}
