//! **EL RESPIRO** (10-10, el cuello de botella 5 de `docs/plan/EL_FOCO.md`):
//! mientras una puerta de la 3060 la ESPERA, las interrupciones se abren un
//! instante en cada vuelta. El kernel se aparta: el reloj, el radar de 4 ms
//! (el hilo del bus: teclado y raton) y las demas tareas corren mientras la
//! 3060 trabaja, en vez de esperar a que la puerta acabe.
//!
//! [carril]  ROJO      abre las interrupciones DENTRO de un syscall: la tarea
//!                     puede quedar apartada a mitad de la puerta y volver
//! [consumo] NADA      una instruccion de frontera por vuelta de espera
//!
//! ** Por que hacia falta: un syscall corre entero con IF=0 (`MSR_SFMASK`).
//! Un dibujo bueno tarda < 1 ms, pero una espera larga llega al plazo del
//! vigilante (1 s) y a sus cortes, y todo ese tiempo el reloj no sonaba. El
//! `yield_current` de antes no cedia nada (ver `esperando`).
//!
//! Las CUATRO condiciones para abrir, y cada una tapa un agujero:
//!
//! ```text
//!    la tarea TIENE la 3060   solo dentro de una puerta de la IOMMU
//!                             (`entrar`): nadie mas entra a la 3060 mientras
//!                             esta apartada -- la puerta es UNA --
//!    IF esta cerrado          si ya estaba abierto (un hilo de kernel), no
//!                             se toca: el `cli` de despues lo cerraria
//!    ningun cerrojo tomado    abrir con un cerrojo en la mano dejaria a otra
//!                             tarea girando en el con IF=0 para siempre
//!                             (`spin::tomados`)
//!    `RESPIRO`                la linea que se apaga si el metal dice que no
//! ```
//!
//! Y al volver, la pila del trap se RE-PUBLICA: si el reloj entro, publico la
//! suya (`gs:[0x10]`), y la puerta tiene que volver por la del syscall. Una
//! tarea apartada aqui es INTOCABLE (`scheduler::intocable`): cerrarla la
//! apunta, y muere al salir de la puerta, con la 3060 ya fuera de su memoria.
//!
//! **Falta el metal** (6 de EL_FOCO): `gpu doom` y el cubo en su ventana
//! con el raton moviendose; en `save`, la fila `respiro` subiendo (instantes
//! y apartadas), el latido del bus SIN llegar tarde, y ni una pantalla azul.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::ring0::task::{percpu, scheduler};

/// ** EL INTERRUPTOR. `false` = la espera vuelve a girar con IF=0 entera,
/// como antes del 10-10.
pub const RESPIRO: bool = true;

/// Quien tiene la 3060 (tid; 0 = nadie): la puerta es UNA.
static TITULAR: AtomicU32 = AtomicU32::new(0);
/// Para la cabina: instantes abiertos, y los que trajeron una interrupcion
/// (la pila del trap habia cambiado al volver).
static RESPIROS: AtomicU64 = AtomicU64::new(0);
static APARTADAS: AtomicU64 = AtomicU64::new(0);
/// Los ciclos que la puerta del TITULAR paso con las interrupciones ABIERTAS
/// (de cada `sti` a su `cli`, con lo que corrio en medio). `syscall::larga`
/// los resta: una puerta que respira no tuvo el reloj callado ese rato, y
/// apuntarla entera seria culpar a la 3060 de un silencio que no hubo.
static AFUERA: AtomicU64 = AtomicU64::new(0);
static AFUERA_DE: AtomicU32 = AtomicU32::new(0);
/// Lo mas que se espera a que OTRA suelte la 3060: 2 s (mas que el vigilante
/// y sus cortes). Pasado, la puerta dice "uno en marcha".
const PLAZO_TOMAR_US: u64 = 2_000_000;

/// `INFO_RESPIRO`: `[0..32)` los instantes abiertos, `[32..64)` los que
/// trajeron una interrupcion, desde el arranque (saturan en `u32::MAX`).
pub fn cuenta() -> u64 {
    let r = RESPIROS.load(Ordering::Relaxed).min(u32::MAX as u64);
    let a = APARTADAS.load(Ordering::Relaxed).min(u32::MAX as u64);
    r | (a << 32)
}

/// Para `syscall::larga`, al salir de la puerta: los ciclos que `tid` paso
/// con las interrupciones abiertas DENTRO de ella. Se gastan al leerse.
pub fn afuera_de(tid: u32) -> u64 {
    if AFUERA_DE.load(Ordering::Acquire) != tid {
        return 0;
    }
    AFUERA_DE.store(0, Ordering::Release);
    AFUERA.swap(0, Ordering::AcqRel)
}

fn if_abierto() -> bool {
    let rflags: u64;
    // SAFETY: leer RFLAGS no toca nada.
    unsafe { core::arch::asm!("pushfq", "pop {}", out(reg) rflags) };
    rflags & (1 << 9) != 0
}

/// Abrir UN instante, si se puede: `sti` deja pasar las interrupciones en la
/// frontera tras la instruccion siguiente, y `cli` cierra en la otra.
/// `titular`: la tarea tiene la 3060, y su rato abierto se apunta en
/// `AFUERA`. Quien solo espera a entrar (`entrar`) no lo apunta: dos a la
/// vez se pisarian la cuenta, y contar de MENOS abierto solo hace la puerta
/// mas larga en el informe -- el lado seguro --.
fn abrir_un_instante(titular: bool) -> bool {
    if !RESPIRO || if_abierto() || crate::ring0::plat::spin::tomados() != 0 {
        return false;
    }
    let mio = percpu::trap_rsp();
    let antes = scheduler::rdtsc();
    // SAFETY: IF estaba cerrado y ningun cerrojo esta tomado (arriba); la pila
    // es la propia de la tarea, y un trap encima cabe (`pila.py`). Sin
    // `nomem`: el reloj puede cambiarlo TODO por debajo, y el compilador no
    // debe suponer nada de la memoria a traves de esto.
    unsafe { core::arch::asm!("sti", "pause", "cli", options(nostack)) };
    RESPIROS.fetch_add(1, Ordering::Relaxed);
    if titular {
        AFUERA.fetch_add(scheduler::rdtsc().wrapping_sub(antes), Ordering::Relaxed);
    }
    if percpu::trap_rsp() != mio {
        APARTADAS.fetch_add(1, Ordering::Relaxed);
        percpu::set_trap_rsp(mio);
    }
    true
}

/// **Una vuelta de espera a la 3060**, por quien la tiene: respira, o gira.
pub fn respirar() {
    // SAFETY: con IF=0 (si estuviera abierto, `abrir_un_instante` no abre) y
    // en un solo nucleo, `current` no cambia mientras se lee.
    let yo = unsafe { scheduler::current_tid_en_trap() };
    if TITULAR.load(Ordering::Acquire) != yo || !abrir_un_instante(true) {
        core::hint::spin_loop();
    }
}

/// Como se entro en la puerta.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Entrada {
    /// Era de nadie y ahora es de esta tarea: al acabar, [`salir`].
    Nueva,
    /// Esta tarea YA la tenia (una estacion de la muerte dentro de una
    /// puerta): se sigue dentro, y quien entro primero es quien sale.
    YaDentro,
    /// Otra no la solto en `PLAZO_TOMAR_US`.
    NoSePudo,
}

/// **Entrar en la puerta de la 3060**. Si otra la tiene (apartada en su
/// respiro), se espera respirando, hasta `PLAZO_TOMAR_US`.
pub fn entrar() -> Entrada {
    let yo = scheduler::current_tid();
    let hz = (scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = scheduler::rdtsc();
    loop {
        match TITULAR.compare_exchange(0, yo, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(d) if d == yo => return Entrada::YaDentro,
            Err(_) => {
                if (scheduler::rdtsc() - desde) / hz > PLAZO_TOMAR_US {
                    crate::ring0::cabina::warn("gpu", "respiro: otra tarea no solto la 3060 en 2 s; tid", yo as u64);
                    return Entrada::NoSePudo;
                }
                una_vuelta();
            }
        }
    }
    AFUERA.store(0, Ordering::Relaxed);
    AFUERA_DE.store(yo, Ordering::Release);
    scheduler::intocable(yo);
    Entrada::Nueva
}

/// **Una vuelta esperando a OTRA** (que tiene la 3060, o un turno detras de
/// la puerta): con IF=0 se abre un instante para que la otra pueda acabar;
/// con IF=1 (un hilo de kernel) el reloj ya entra solo, y se gira.
///
/// ** Lo que habia antes en esos sitios (`pase_gpu::Turno::esperar`) era
/// `yield_current` en bucle: dentro de un syscall eso no cede nada (ver
/// `esperando`), y si quien tenia el turno estaba apartado, la espera no
/// acababa nunca.
pub fn una_vuelta() {
    if !abrir_un_instante(false) {
        core::hint::spin_loop();
    }
}

/// **Las estaciones de la muerte que tocan la 3060** (devolver el lienzo,
/// cerrar el pase) pasan por la MISMA puerta: con el respiro, la titular
/// puede estar apartada a mitad de una orden, y devolverle la memoria por
/// debajo seria lo que la puerta unica impide. Si en 2 s no se pudo entrar,
/// se devuelve igual (R-DMA-3: los marcos no se liberan con la 3060
/// viendolos), con el aviso ya dado.
///
/// Si a quien llama la cerraron mientras estaba dentro, se la cierra al
/// salir (ya tocable): la muerte apuntada no se pierde.
pub fn con_la_gpu<R>(f: impl FnOnce() -> R) -> R {
    let e = entrar();
    let r = f();
    if e == Entrada::Nueva && salir() {
        scheduler::terminar(scheduler::current_tid());
    }
    r
}

/// **Salir de la puerta**. `true` si a la tarea la cerraron mientras estaba
/// dentro: quien llama la termina YA (revocar y `exit_current`, como EXIT).
pub fn salir() -> bool {
    TITULAR.store(0, Ordering::Release);
    let muere = scheduler::tocable();
    if muere {
        // No volvera a `syscall::larga`: que su cuenta no la herede otro tid.
        AFUERA_DE.store(0, Ordering::Release);
    }
    muere
}
