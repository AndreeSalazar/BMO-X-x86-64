//! **LA PRUEBA DE LA RAM** (`ram prueba`, 29-09): escribir, vaciar la cache,
//! releer y comparar -- en la RAM LIBRE, tanda a tanda.
//!
//! [carril]  AMARILLO  escribe patrones en marcos que PIDE, y los devuelve
//! [consumo] NADA      corre por orden (`ram prueba`), una tanda por syscall
//!
//! # Por que existe
//!
//! El 0x15 del booter de la 3060 volvio el 29-09 sin causa a la vista, y la
//! pregunta del propietario fue: *"no sera que la RAM altera?"*. Sospechar de la
//! RAM sin medirla es contar una historia. BMO-X es el propietario de toda la
//! maquina: puede probarla el mismo.
//!
//! # Como
//!
//! ```text
//!    una TANDA   pide 16 MiB contiguos (o 2 MiB si no hay) como `Kernel`,
//!                y en ellos, tres patrones:
//!                  1. cada palabra = su propia direccion fisica
//!                     (caza direcciones cruzadas: dos sitios que son uno)
//!                  2. lo mismo, invertido (caza bits pegados a 1 o a 0)
//!                  3. pseudoaleatorio (xorshift, sembrado por el trozo)
//!                tras escribir, `wbinvd`: la relectura viene de la RAM, no
//!                del L3 de 32 MiB, que es justo lo que se quiere mirar
//!    SE QUEDA    el trozo probado no se devuelve hasta acabar: si se
//!                devolviera, la tanda siguiente recibiria EL MISMO
//!    ACABA       cuando quedaria menos de la RESERVA libre (1 GiB, para
//!                que el disco, el USB y el escritorio sigan vivos), o no
//!                hay mas trozos. Entonces lo devuelve TODO
//! ```
//!
//! # Lo que NO prueba, dicho antes de que alguien lo suponga
//!
//! - **La RAM que ya esta en uso** (el kernel, los programas, lo prestado a la
//!   3060): no se escribe encima de lo que es de alguien. Lo prestado a la
//!   3060 lo vigila el booter por su cuenta (`gpu_despertar`, la huella).
//! - **La RESERVA**: el ultimo GiB libre no se toca.
//! - **Fallos que solo salen con calor o con horas**: esto es una pasada de
//!   segundos. Cero errores aqui es un dato, no un certificado.
//!
//! [!] Si quien la pidio muere a mitad, lo tenido NO vuelve solo: vuelve con
//! la siguiente `ram prueba` (la primera tanda suelta lo de antes) o con
//! [`cancelar`]. Y la ultima tanda devuelve miles de marcos de golpe: es la
//! mas lenta (decimas de segundo), una vez.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::ring0::mm::{phys, PAGE};

/// El trozo de cada tanda: 16 MiB, ~10 ms de trabajo por syscall.
const TROZO_PAGINAS: u64 = 4096;
/// Si no hay 16 MiB seguidos: 2 MiB.
const TROZO_CHICO: u64 = 512;
/// Lo que se deja libre siempre: 1 GiB.
const RESERVA_PAGINAS: u64 = (1 << 30) / PAGE;
/// Cuantos trozos se pueden tener a la vez (16 GiB en trozos de 2 MiB).
const MAX_TROZOS: usize = 8192;
/// El bit 0 de una entrada de `TROZOS`: es un trozo CHICO.
const CHICO: u64 = 1;

/// La prueba ACABO en esta tanda (y ya se devolvio todo).
pub const RAM_ACABO: u64 = 1 << 63;
pub const RAM_ERRORES_SHIFT: u64 = 40;

/// Los trozos tenidos: la fisica, con [`CHICO`] en el bit 0.
static mut TROZOS: [u64; MAX_TROZOS] = [0; MAX_TROZOS];
static N: AtomicU64 = AtomicU64::new(0);
/// Una tanda a la vez; si llega otra, se niega (no se espera: un cerrojo que
/// gira 10 ms con las interrupciones cerradas cuesta un latido del bus).
static EN_CURSO: AtomicBool = AtomicBool::new(false);

/// Lo que se va sabiendo (ver [`detalle`]).
static PROBADAS: AtomicU64 = AtomicU64::new(0);
static ERRORES: AtomicU64 = AtomicU64::new(0);
static PRIMERO: AtomicU64 = AtomicU64::new(0);
static ESPERADO: AtomicU64 = AtomicU64::new(0);
static LEIDO: AtomicU64 = AtomicU64::new(0);
static BITS: AtomicU64 = AtomicU64::new(0);
static ULTIMO: AtomicU64 = AtomicU64::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);
static GRANDES: AtomicU64 = AtomicU64::new(0);
static CHICOS: AtomicU64 = AtomicU64::new(0);

/// Otra tanda de la prueba de antes, o de una nueva.
pub const RAM_OCUPADA: u32 = 1;

fn wbinvd() {
    // SAFETY: Ring 0; escribe y vacia caches, no toca memoria ajena.
    unsafe { core::arch::asm!("wbinvd", options(nostack, preserves_flags)) };
}

fn xorshift(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Un fallo: la fisica, lo que se escribio y lo que se leyo.
fn apuntar_fallo(fisica: u64, esperado: u64, leido: u64) {
    if ERRORES.fetch_add(1, Ordering::AcqRel) == 0 {
        PRIMERO.store(fisica, Ordering::Release);
        ESPERADO.store(esperado, Ordering::Release);
        LEIDO.store(leido, Ordering::Release);
        crate::ring0::cabina::fault("ram", "PRUEBA DE LA RAM: la primera palabra que NO se leyo como se escribio", fisica);
    }
    BITS.fetch_or(esperado ^ leido, Ordering::AcqRel);
    ULTIMO.store(fisica, Ordering::Release);
}

/// **Los tres patrones sobre un trozo** (`fisica`, `paginas`).
fn probar(fisica: u64, paginas: u64) {
    let v = crate::ring0::mm::phys_to_virt(fisica) as *mut u64;
    let n = (paginas * PAGE / 8) as usize;
    for patron in 0..3u64 {
        let semilla = (fisica ^ 0x9E37_79B9_7F4A_7C15) | 1;
        let valor = |i: usize, r: u64| -> u64 {
            let a = fisica + i as u64 * 8;
            match patron {
                0 => a,
                1 => !a,
                _ => r,
            }
        };
        let mut r = semilla;
        for i in 0..n {
            r = xorshift(r);
            // SAFETY: `v..v+n` es el trozo que esta prueba pidio y tiene.
            unsafe { v.add(i).write_volatile(valor(i, r)) };
        }
        wbinvd();
        let mut r = semilla;
        for i in 0..n {
            r = xorshift(r);
            let esperado = valor(i, r);
            // SAFETY: igual que arriba.
            let leido = unsafe { v.add(i).read_volatile() };
            if leido != esperado {
                apuntar_fallo(fisica + i as u64 * 8, esperado, leido);
            }
        }
    }
}

/// Devuelve todo lo tenido.
fn soltar_todo() {
    let n = N.swap(0, Ordering::AcqRel) as usize;
    for k in 0..n {
        // SAFETY: `TROZOS[..n]` solo lo toca quien tiene `EN_CURSO`.
        let e = unsafe { (*core::ptr::addr_of!(TROZOS))[k] };
        let (base, paginas) = (e & !CHICO, if e & CHICO != 0 { TROZO_CHICO } else { TROZO_PAGINAS });
        for p in 0..paginas {
            phys::free_frame_de(base + p * PAGE, phys::Titular::Kernel);
        }
    }
}

fn resumen(acabo: bool) -> u64 {
    PROBADAS.load(Ordering::Acquire).min((1 << RAM_ERRORES_SHIFT) - 1)
        | ERRORES.load(Ordering::Acquire).min(0x7F_FFFF) << RAM_ERRORES_SHIFT
        | if acabo { RAM_ACABO } else { 0 }
}

/// **UNA TANDA.** `primera` empieza una prueba nueva (y suelta lo de una a
/// medias). `Ok(paginas probadas | errores << 40 | acabo << 63)`.
pub fn tanda(primera: bool) -> Result<u64, u32> {
    if EN_CURSO.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
        return Err(RAM_OCUPADA);
    }
    if primera {
        soltar_todo();
        for c in [&PROBADAS, &ERRORES, &PRIMERO, &ESPERADO, &LEIDO, &BITS, &ULTIMO, &TICKS, &GRANDES, &CHICOS] {
            c.store(0, Ordering::Release);
        }
        let (_, libres) = phys::stats();
        crate::ring0::cabina::info("ram", "PRUEBA DE LA RAM: empieza; marcos libres", libres);
    }
    let t0 = crate::ring0::task::scheduler::rdtsc();
    let libres = phys::stats().1;
    let n = N.load(Ordering::Acquire) as usize;
    let trozo = if n >= MAX_TROZOS || libres < RESERVA_PAGINAS + TROZO_CHICO {
        None
    } else if libres >= RESERVA_PAGINAS + TROZO_PAGINAS {
        phys::alloc_frames_contig_de(TROZO_PAGINAS, phys::Titular::Kernel)
            .map(|f| (f, TROZO_PAGINAS))
            .or_else(|| phys::alloc_frames_contig_de(TROZO_CHICO, phys::Titular::Kernel).map(|f| (f, TROZO_CHICO)))
    } else {
        phys::alloc_frames_contig_de(TROZO_CHICO, phys::Titular::Kernel).map(|f| (f, TROZO_CHICO))
    };
    let acabo = match trozo {
        Some((f, paginas)) => {
            // SAFETY: `EN_CURSO` es nuestro; `n < MAX_TROZOS`.
            unsafe { (*core::ptr::addr_of_mut!(TROZOS))[n] = f | if paginas == TROZO_CHICO { CHICO } else { 0 } };
            N.store(n as u64 + 1, Ordering::Release);
            probar(f, paginas);
            PROBADAS.fetch_add(paginas, Ordering::AcqRel);
            if paginas == TROZO_CHICO { &CHICOS } else { &GRANDES }.fetch_add(1, Ordering::AcqRel);
            false
        }
        None => {
            soltar_todo();
            crate::ring0::cabina::count("ram", "PRUEBA DE LA RAM: acabo; MiB probados", PROBADAS.load(Ordering::Acquire) * PAGE >> 20);
            crate::ring0::cabina::count("ram", "PRUEBA DE LA RAM: errores", ERRORES.load(Ordering::Acquire));
            true
        }
    };
    TICKS.fetch_add(crate::ring0::task::scheduler::rdtsc().saturating_sub(t0), Ordering::AcqRel);
    EN_CURSO.store(false, Ordering::Release);
    Ok(resumen(acabo))
}

/// **Cancelar**: devolver todo lo tenido ya. `Ok(paginas probadas)`.
pub fn cancelar() -> Result<u64, u32> {
    if EN_CURSO.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
        return Err(RAM_OCUPADA);
    }
    let tenia = N.load(Ordering::Acquire);
    soltar_todo();
    if tenia != 0 {
        crate::ring0::cabina::info("ram", "PRUEBA DE LA RAM: cancelada; trozos devueltos", tenia);
    }
    EN_CURSO.store(false, Ordering::Release);
    Ok(PROBADAS.load(Ordering::Acquire))
}

/// **El detalle**, campo a campo: 0 errores, 1 la PRIMERA fisica que fallo, 2
/// lo que se escribio ahi, 3 lo que se leyo, 4 los bits que fallaron alguna
/// vez (OR de los XOR), 5 la ULTIMA fisica que fallo, 6 los us de trabajo, 7
/// trozos de 16 MiB | de 2 MiB << 32, 8 paginas probadas.
pub fn detalle(campo: u64) -> u64 {
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    match campo {
        0 => ERRORES.load(Ordering::Acquire),
        1 => PRIMERO.load(Ordering::Acquire),
        2 => ESPERADO.load(Ordering::Acquire),
        3 => LEIDO.load(Ordering::Acquire),
        4 => BITS.load(Ordering::Acquire),
        5 => ULTIMO.load(Ordering::Acquire),
        6 => TICKS.load(Ordering::Acquire) / hz,
        7 => GRANDES.load(Ordering::Acquire) | CHICOS.load(Ordering::Acquire) << 32,
        8 => PROBADAS.load(Ordering::Acquire),
        _ => 0,
    }
}
