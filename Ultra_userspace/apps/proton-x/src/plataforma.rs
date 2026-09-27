//! **La plataforma de BMO-X** para las DLL de la casa (`bmo-proton-x-casa`).
//!
//! Lo que `kernel32`, `user32` y `gdi32` de la casa piden al sistema, con la
//! puerta de BMO-X debajo:
//!
//! ```text
//!    escribir     la consola de quien nos lanzo (sin el \r de un \r\n)
//!    salir        dice con que salio el .exe, y el proceso se va
//!    superficie   un bloque con la cabecera BSUP y un buzon de 64 ranuras,
//!                 SIN ofrecer: la ventana existe y todavia no se ve
//!    mostrar      ofrecerla al escritorio (MEM_OP_OFRECER a quien nos lanzo)
//!    presentar    subir la secuencia: el dibujo esta entero (R-APP4)
//!    evento       el siguiente del buzon, o 0
//!    dormir       4 ms: un .exe esperando teclas no gasta CPU (R21)
//!    poner_gs     el TEB del hilo de Windows que va a correr (P4):
//!                 TASK_OP_PON_GS, que no toca el MSR si no cambia
//!    ahora_ns     `rdtsc` y la frecuencia que publica el kernel (INFO_TSC_HZ)
//! ```
//!
//! La superficie es la MISMA que pide una app de INTI o de C
//! (`runtime/superficie/roja.inti`, `superficie/amarilla.h`): el escritorio no
//! sabe que dentro hay un `.exe` de Windows, y no tiene por que saberlo.

use bmo_proton_x_casa::{Plataforma, Superficie};
use bmo_userland as bmo;

/// Ranuras del buzon: potencia de dos, como pide el escritorio.
const RANURAS: u64 = 64;

pub fn de_bmo() -> Plataforma {
    Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x::lote::en_cpu }
}

/// Un GS que el kernel no acepta seria un hilo sin TEB: no se sigue.
fn poner_gs(teb: u64) {
    if bmo::poner_gs(teb).is_err() {
        bmo::consola("PROTON-X: el kernel no pone el GS de un hilo: no se sigue\n");
        super::fin_del_exe(0xC000_0005);
    }
}

/// La frecuencia del TSC, pedida una vez (0 = todavia no).
static TSC_HZ: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

fn ahora_ns() -> u64 {
    use core::sync::atomic::Ordering;
    let mut hz = TSC_HZ.load(Ordering::Relaxed);
    if hz == 0 {
        hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
        TSC_HZ.store(hz, Ordering::Relaxed);
    }
    (bmo::ciclos() as u128 * 1_000_000_000 / hz as u128) as u64
}

/// La consola es de lineas: el retorno de carro de Windows pintaria un
/// caracter de mas. Un `\r` delante de `\n` no se manda.
fn escribir(bytes: &[u8]) {
    let mut desde = 0;
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            decir(&bytes[desde..i]);
            desde = i + 1;
        }
    }
    decir(&bytes[desde..]);
}

fn decir(b: &[u8]) {
    match core::str::from_utf8(b) {
        Ok(s) => bmo::consola(s),
        Err(_) => {
            for &c in b {
                bmo::consola(if c.is_ascii() { core::str::from_utf8(core::slice::from_ref(&c)).unwrap_or("?") } else { "?" });
            }
        }
    }
}

fn salir(codigo: u32) -> ! {
    super::fin_del_exe(codigo)
}

/// La cabecera BSUP, palabra a palabra. `volatile`: la lee otro proceso.
fn pon(base: u64, i: u64, v: u32) {
    // SAFETY: `base` es un bloque nuestro con la cabecera dentro.
    unsafe { core::ptr::write_volatile((base + 4 * i) as *mut u32, v) };
}

fn campo(base: u64, i: u64) -> u32 {
    // SAFETY: como arriba.
    unsafe { core::ptr::read_volatile((base + 4 * i) as *const u32) }
}

fn base_de(s: &Superficie) -> u64 {
    s.pixeles as u64 - bmo::SUP_CABECERA
}

fn superficie(ancho: u32, alto: u32) -> Option<Superficie> {
    let pixeles = ancho as u64 * alto as u64 * 4;
    let buzon = bmo::SUP_CABECERA + pixeles;
    let bytes = buzon + bmo::SUP_BUZON_CABECERA + RANURAS * bmo::SUP_BUZON_RANURA;
    let bloque = bmo::Memoria::request(bytes)?;
    let base = bloque.base() as u64;
    pon(base, 0, bmo::SUP_MAGIC as u32);
    pon(base, 1, ancho);
    pon(base, 2, alto);
    pon(base, 3, ancho);
    pon(base, 4, bmo::SUP_BGRA32 as u32);
    pon(base, bmo::SUP_CAMPO_SECUENCIA, 0);
    pon(base, 6, buzon as u32);
    pon(base, 7, RANURAS as u32);
    // Cabeza, cola y estado a cero ANTES de ofrecer.
    for i in 0..4 {
        pon(buzon + base, i, 0);
    }
    let s = Superficie { pixeles: (base + bmo::SUP_CABECERA) as *mut u32, ancho, alto, stride: ancho, dato: bloque.handle() };
    // Vive hasta que el proceso muera: el escritorio la esta leyendo.
    core::mem::forget(bloque);
    Some(s)
}

fn mostrar(s: &Superficie) -> bool {
    let padre = bmo::mi_padre();
    if padre == 0 {
        return false;
    }
    let bytes = campo(base_de(s), 6) as u64 + bmo::SUP_BUZON_CABECERA + RANURAS * bmo::SUP_BUZON_RANURA;
    bmo::offer(s.dato, 0, bytes, padre)
}

fn presentar(s: &Superficie) {
    let base = base_de(s);
    pon(base, bmo::SUP_CAMPO_SECUENCIA, campo(base, bmo::SUP_CAMPO_SECUENCIA).wrapping_add(1));
}

/// El siguiente evento del buzon: la cabeza la escribe el escritorio, la cola
/// esta app (ver `superficie/amarilla.inti`).
fn evento(s: &Superficie) -> u64 {
    let base = base_de(s);
    let buzon = base + campo(base, 6) as u64;
    let (cabeza, cola) = (campo(buzon, 0), campo(buzon, 1));
    if cabeza == cola {
        return 0;
    }
    let mascara = RANURAS as u32 - 1;
    let ranura = buzon + bmo::SUP_BUZON_CABECERA + (cola & mascara) as u64 * bmo::SUP_BUZON_RANURA;
    // SAFETY: la ranura cae dentro del buzon de nuestro bloque.
    let e = unsafe { core::ptr::read_volatile(ranura as *const u64) };
    pon(buzon, 1, (cola + 1) & mascara);
    e
}

fn dormir() {
    bmo::wait(0, 0, 4_000_000);
}
