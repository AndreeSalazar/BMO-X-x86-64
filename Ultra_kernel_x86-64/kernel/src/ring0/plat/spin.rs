//! IRQ-saving spinlock for short Ring 0 critical sections.
//!
//! [carril]  ROJO      el cerrojo. Tomarlo dos veces es un abrazo mortal
//! [consumo] NADA      el cerrojo: gira solo mientras alguien lo espera
//!
//! The guard saves RFLAGS and clears IF while held, then restores IF only
//! if it was set on entry. This makes the lock safe to take from both
//! interrupt and non-interrupt context on the BSP, and SMP-ready once
//! application processors come online (contention is real then, so critical
//! sections must stay short -- no waits, no logging inside the lock).
//!
//! === Why these locks count their own collisions ===
//!
//! `docs/maestro/SMP_MAESTRO.md` puts one line in the SMP dashboard above the rest:
//! *"spinlock contention -- if it goes up, two cores are fighting over the same
//! thing; it is the early warning of the race"*. Step 4 of that plan is
//! **measure before handing out real work**, and this is that instrument.
//!
//! * The number to watch is `hits`, and **today it has to be zero.** Not
//! "low" -- zero, by construction, and that is what makes it worth reading:
//!
//! * On one core a taken lock cannot be found taken by anyone else. The holder
//!   runs with IF clear, so nothing can interleave and reach the same lock.
//! * The workers in `plat/smp/obra.rs` compute and nothing else. A worker that
//!   never enters the kernel cannot touch any of the kernel's 209 `static mut`
//!   -- which is precisely the golden rule that makes SMP safe today.
//!
//! So a non-zero count is not a performance figure. **It is the report that
//! something broke one of those two statements**, and it says so before
//! anything corrupts. It is the same shape as `FatVolume::fallos_mudos()`: a
//! counter whose correct value is zero is a claim the machine can check.
//!
//! === What it costs when nothing is contended ===
//!
//! Nothing. The fast path is the same `swap` it always was and returns before
//! touching a single counter; every atomic here lives on the branch that only
//! runs when the lock was **already held**. Instrumentation that taxes the
//! uncontended path would be paid on every allocation, forever, to measure an
//! event that is not supposed to happen.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// Contended acquisitions across **every** lock in the kernel.
static HITS: AtomicU32 = AtomicU32::new(0);
/// The longest single wait ever seen, in spin rounds.
static PEAK: AtomicU32 = AtomicU32::new(0);
/// The name of the lock that set that peak, as `(ptr, len)`.
///
/// Two halves of a `&'static str`, which is why this is sound: the string is
/// baked into the image and outlives everything. Publishing it is deliberately
/// **not** synchronised with `PEAK` -- two cores can interleave and leave the
/// name of the previous record holder. The number is the alarm; the name is a
/// hint about where to look first, and paying for a lock to protect a hint
/// inside the lock code would be its own joke.
static WORST_PTR: AtomicUsize = AtomicUsize::new(0);
static WORST_LEN: AtomicUsize = AtomicUsize::new(0);

pub struct SpinLock {
    locked: AtomicBool,
    /// Who this is, so the kernel can name it instead of handing out an index
    /// that Ring 3 would have to translate back.
    name: &'static str,
    /// Times somebody found this lock already taken.
    hits: AtomicU32,
    /// Spin rounds waited here in total.
    spins: AtomicU64,

    /// The longest single wait on this lock.
    peak: AtomicU32,
    /// Lo mas que se retuvo este cerrojo, en ciclos de TSC.
    retenido: AtomicU64,
}

pub struct Guard<'a> {
    lock: &'a SpinLock,
    rflags: u64,
    /// El `rdtsc` de cuando se tomo. Ver `RETENIDO_PEAK`.
    desde: u64,
    /// Quien lo tomo: fichero y linea, gratis con `#[track_caller]`. Es lo que
    /// convierte "`phys` retuvo 4.658 us" en "`phys` en `roja.rs:335`": el
    /// nombre dice que cerrojo, el sitio dice que FUNCION.
    sitio: &'static core::panic::Location<'static>,
}

impl SpinLock {
    pub const fn new(name: &'static str) -> Self {
        Self {
            locked: AtomicBool::new(false),
            name,
            hits: AtomicU32::new(0),
            spins: AtomicU64::new(0),
            peak: AtomicU32::new(0),
            retenido: AtomicU64::new(0),
        }
    }

    #[track_caller]
    pub fn lock(&self) -> Guard<'_> {
        let sitio = core::panic::Location::caller();
        let rflags: u64;
        unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) rflags); }

        // The fast path, untouched: one `swap` and out.
        if !self.locked.swap(true, Ordering::Acquire) {
            return Guard { lock: self, rflags, desde: unsafe { core::arch::x86_64::_rdtsc() }, sitio };
        }

        // From here on the lock was already held, which is the event worth
        // counting. The wait itself is the same two-level spin as before --
        // read relaxed until it looks free, then try the exchange.
        let mut rounds: u32 = 0;
        loop {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
                rounds = rounds.saturating_add(1);
            }
            if !self.locked.swap(true, Ordering::Acquire) {
                break;
            }
        }
        self.record(rounds);
        Guard { lock: self, rflags, desde: unsafe { core::arch::x86_64::_rdtsc() }, sitio }
    }

    /// Lo mas que se retuvo este cerrojo, en ciclos de TSC.
    pub fn retenido(&self) -> u64 {
        self.retenido.load(Ordering::Relaxed)
    }

    /// `(hits, spin rounds, longest wait)` for this lock alone.
    pub fn stats(&self) -> (u32, u64, u32) {
        (
            self.hits.load(Ordering::Relaxed),
            self.spins.load(Ordering::Relaxed),
            self.peak.load(Ordering::Relaxed),
        )
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Off the fast path on purpose: never inlined into `lock`.
    #[inline(never)]
    fn record(&self, rounds: u32) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.spins.fetch_add(rounds as u64, Ordering::Relaxed);
        raise(&self.peak, rounds);

        HITS.fetch_add(1, Ordering::Relaxed);
        if raise(&PEAK, rounds) {
            WORST_PTR.store(self.name.as_ptr() as usize, Ordering::Relaxed);
            WORST_LEN.store(self.name.len(), Ordering::Relaxed);
        }
    }
}

/// Lift `cell` to `v` if `v` is bigger. Returns whether this call did it.
fn raise(cell: &AtomicU32, v: u32) -> bool {
    let mut seen = cell.load(Ordering::Relaxed);
    while v > seen {
        match cell.compare_exchange_weak(seen, v, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => seen = actual,
        }
    }
    false
}

// == *** CUANTO SE RETIENE, no solo cuanto se pelea (2026-09-21) ==========
//
// `hits`/`spins`/`peak` cuentan la PELEA: cuantas veces alguien encontro el
// cerrojo tomado y cuanto giro. En una maquina de un nucleo la pelea es cero
// siempre --el `save` dice `0 choques (lo correcto: nadie pelea)`-- y eso no
// dice NADA de lo que importa aqui: `lock()` hace `cli`, asi que **mientras
// alguien tiene un cerrojo, el reloj no suena y el orquestador esta ciego**.
// Un cerrojo retenido 1.266 ms es un latido del bus 1.266 ms tarde sin que
// ninguna prioridad pueda hacer nada. Y no lo contaba nadie.
//
// Ahora cada `Guard` apunta el `rdtsc` al tomar y resta al soltar. Se guarda
// lo PEOR por cerrojo y lo peor de todos con su nombre: un maximo, no una
// media, porque una media de retenciones esconde justo el pico que se nota
// con la mano. Dos `rdtsc` por cerrojo: unos 50 ciclos, en el sitio donde el
// cerrojo ya costo mas que eso.
//
// [!] `_rdtsc` de `core::arch` y no `scheduler::rdtsc()`: este fichero esta
// DEBAJO del planificador (el planificador lo usa), y L8 no deja subir.
static RETENIDO_PEAK: AtomicU64 = AtomicU64::new(0);
static RETENIDO_PTR: AtomicUsize = AtomicUsize::new(0);
static RETENIDO_LEN: AtomicUsize = AtomicUsize::new(0);
/// El `Location` de quien tomo el cerrojo en la retencion mas larga.
static RETENIDO_SITIO: AtomicUsize = AtomicUsize::new(0);

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        // Se mide ANTES de soltar: lo que cuenta es el tiempo con el cerrojo
        // en la mano (y las interrupciones cerradas), no el de despues.
        let retenido = unsafe { core::arch::x86_64::_rdtsc() }.wrapping_sub(self.desde);
        if raise64(&self.lock.retenido, retenido) && raise64(&RETENIDO_PEAK, retenido) {
            RETENIDO_PTR.store(self.lock.name.as_ptr() as usize, Ordering::Relaxed);
            RETENIDO_LEN.store(self.lock.name.len(), Ordering::Relaxed);
            RETENIDO_SITIO.store(self.sitio as *const _ as usize, Ordering::Relaxed);
        }
        self.lock.locked.store(false, Ordering::Release);
        if self.rflags & (1 << 9) != 0 {
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
        }
    }
}

/// `(contended acquisitions, longest wait in spin rounds)` for the whole
/// kernel. **Both are supposed to be zero.**
pub fn contention() -> (u32, u32) {
    (HITS.load(Ordering::Relaxed), PEAK.load(Ordering::Relaxed))
}

fn raise64(cell: &AtomicU64, v: u64) -> bool {
    let mut seen = cell.load(Ordering::Relaxed);
    while v > seen {
        match cell.compare_exchange_weak(seen, v, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => seen = actual,
        }
    }
    false
}

/// **La retencion mas larga de cualquier cerrojo**, en ciclos de TSC. Con
/// `retenido_peor_quien` dice cual. Cero = ningun cerrojo se ha soltado aun.
pub fn retenido_peor() -> u64 {
    RETENIDO_PEAK.load(Ordering::Relaxed)
}

/// **Olvidar las retenciones vistas hasta ahora.** Lo llama `start_bus_thread`:
/// la medida es "cuanto puede un cerrojo robarle al latido", y antes de que
/// exista un latido no hay a quien robar. El `save` de las 12:48 enseno
/// `phys` 4.651 us en `roja.rs:108`, que es `init()` del asignador en el
/// arranque: verdad, y sin consecuencia. Con esto, lo que quede en el peor es
/// lo que paso con el bus vivo.
pub fn reiniciar_retenciones() {
    RETENIDO_PEAK.store(0, Ordering::Relaxed);
    RETENIDO_PTR.store(0, Ordering::Relaxed);
    RETENIDO_LEN.store(0, Ordering::Relaxed);
    RETENIDO_SITIO.store(0, Ordering::Relaxed);
}

/// **Donde se tomo** el cerrojo de la retencion mas larga: `(fichero, linea)`.
/// `None` si ninguno se ha soltado aun.
pub fn retenido_peor_sitio() -> Option<(&'static str, u32)> {
    let p = RETENIDO_SITIO.load(Ordering::Relaxed);
    if p == 0 {
        return None;
    }
    let l = unsafe { &*(p as *const core::panic::Location<'static>) };
    Some((l.file(), l.line()))
}

pub fn retenido_peor_quien() -> &'static str {
    let ptr = RETENIDO_PTR.load(Ordering::Relaxed);
    let len = RETENIDO_LEN.load(Ordering::Relaxed);
    if ptr == 0 || len == 0 {
        return "-";
    }
    unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, len))
    }
}

/// The lock that set the current peak, or `"-"` if nothing ever waited.
pub fn worst() -> &'static str {
    let ptr = WORST_PTR.load(Ordering::Relaxed);
    let len = WORST_LEN.load(Ordering::Relaxed);
    if ptr == 0 || len == 0 {
        return "-";
    }
    // Sound because the only writer is `record`, and what it publishes is
    // always the two halves of a `&'static str` from the image.
    unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, len))
    }
}
