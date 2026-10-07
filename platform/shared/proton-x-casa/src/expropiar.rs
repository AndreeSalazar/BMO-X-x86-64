//! **EXPROPIAR: el reloj le quita el turno a un hilo del juego** (07-10).
//!
//! [carril]  ROJO      salta en medio del codigo del juego, en cualquier
//!                     instruccion, y tiene que dejarlo TODO como estaba
//! [cuesta]  PROCESO   un registro mal devuelto rompe al juego, no a BMO-X
//! [consumo] NADA      una visita cada 4 ms; ceder, solo si hay otro listo
//!
//! El metal (07-10): Cyberpunk a los 24 s, un hilo dando vueltas en SU
//! codigo sin llamar a Windows, y ningun otro hilo de la casa volvio a
//! correr (el cuanto de `hilos.rs` solo actua en las puertas de
//! sincronizar). Windows y Linux no tienen ese problema porque el reloj le
//! QUITA el nucleo a quien sea. Aqui lo hace la ALARMA del kernel
//! (`TASK_OP_ALARMA`, como un signal de reloj de Linux; Go la usa igual
//! desde 2020 para sus gorrutinas):
//!
//! ```text
//!    el kernel   cada 4 ms, si pilla a la tarea fuera de la puerta: el RIP
//!                que llevaba, al BUZON; y el RIP, a la PUERTA
//!    la puerta   (abajo, en ensamblador) salta la zona roja, guarda TODO
//!                (16 registros, banderas, x87/SSE con fxsave), y pregunta
//!                a [`decidir`]
//!    decidir     solo si el RIP era del JUEGO (el `.exe` o una DLL suya,
//!                [`juego`]) y su cuanto paso: cede el turno, como si el
//!                juego hubiera llamado a SwitchToThread justo ahi
//!    al volver   lo devuelve todo y salta al RIP del buzon
//! ```
//!
//! **Por que es seguro ceder ahi:** en cualquier instruccion del juego, el
//! juego PODRIA haber llamado a `Sleep(0)`, y la casa ya aguanta eso. Lo que
//! NO es del juego no se toca: el codigo de la casa (a medias de su estado),
//! los sombreadores nativos y los trampolines (los llama la casa). Ahi la
//! puerta vuelve sin hacer nada.
//!
//! **Por que el RIP no se pierde:** el kernel nunca salta DENTRO de la
//! puerta (`bmo_alarma::salta`), y lo primero que hace la puerta es copiar
//! el buzon a su pila. Una alarma en medio de [`decidir`] (codigo de la
//! casa) vuelve en el acto.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering::{Acquire, Relaxed, Release}};

/// Cuantos tramos del juego caben (el `.exe` y sus DLL).
const TRAMOS: usize = 256;

/// El codigo del juego, `[inicio, fin)`. Solo se AGNADE (desde la casa, con
/// la puerta pudiendo saltar en medio): el tramo se escribe entero y
/// despues sube la cuenta, asi que la puerta nunca lee uno a medias.
static INICIOS: [AtomicU64; TRAMOS] = [const { AtomicU64::new(0) }; TRAMOS];
static FINES: [AtomicU64; TRAMOS] = [const { AtomicU64::new(0) }; TRAMOS];
static CUANTOS: AtomicUsize = AtomicUsize::new(0);

/// **El buzon**: aqui deja el kernel el RIP que llevaba la tarea.
#[no_mangle]
static PROTON_X_BUZON: AtomicU64 = AtomicU64::new(0);

/// Las visitas de la alarma, las que pillaron al juego, y las que cedieron.
static VISITAS: AtomicU64 = AtomicU64::new(0);
static EN_EL_JUEGO: AtomicU64 = AtomicU64::new(0);
static CEDIDAS: AtomicU64 = AtomicU64::new(0);
/// Donde pillo al juego la ultima vez (el pulso lo dice como `modulo+rva`:
/// si el juego se queda dando vueltas, es AHI).
static ULTIMO_RIP: AtomicU64 = AtomicU64::new(0);

/// Un `.exe` nuevo: sin tramos.
pub(crate) fn reiniciar() {
    CUANTOS.store(0, Release);
}

/// **Este tramo es codigo del juego** (lo dice quien carga: el `.exe` y cada
/// DLL suya, al colocarla). `false` si ya no caben.
pub fn juego(inicio: u64, bytes: u64) -> bool {
    let n = CUANTOS.load(Acquire);
    if n >= TRAMOS || bytes == 0 {
        return false;
    }
    INICIOS[n].store(inicio, Relaxed);
    FINES[n].store(inicio.saturating_add(bytes), Relaxed);
    CUANTOS.store(n + 1, Release);
    true
}

/// `rip` cae en el codigo del juego. Sin memoria ni cerrojos: lo pregunta
/// la puerta en cualquier momento.
pub fn es_del_juego(rip: u64) -> bool {
    let n = CUANTOS.load(Acquire);
    (0..n).any(|k| rip >= INICIOS[k].load(Relaxed) && rip < FINES[k].load(Relaxed))
}

/// **La puerta y el buzon**, para armar la alarma: `(inicio, fin, buzon)`.
pub fn puerta() -> (u64, u64, u64) {
    // Tocado una vez: la pagina del buzon, hecha y escribible antes de que
    // el kernel la mire.
    PROTON_X_BUZON.store(0, Relaxed);
    (
        proton_x_expropiar as *const () as usize as u64,
        proton_x_expropiar_fin as *const () as usize as u64,
        &PROTON_X_BUZON as *const AtomicU64 as u64,
    )
}

/// `(visitas, en el juego, cedidas, ultimo RIP del juego)` desde el
/// arranque (lo dice el pulso).
pub fn cuentas() -> (u64, u64, u64, u64) {
    (VISITAS.load(Relaxed), EN_EL_JUEGO.load(Relaxed), CEDIDAS.load(Relaxed), ULTIMO_RIP.load(Relaxed))
}

/// **Lo que decide la puerta**, con todo guardado: ceder o volver.
extern "C" fn decidir(rip: u64) {
    VISITAS.fetch_add(1, Relaxed);
    if !es_del_juego(rip) {
        return;
    }
    EN_EL_JUEGO.fetch_add(1, Relaxed);
    ULTIMO_RIP.store(rip, Relaxed);
    // La foto del diario sigue saliendo aunque nadie llame a Windows: aqui
    // la casa esta como en un Sleep(0) del juego.
    crate::pulso::latido();
    if crate::hilos::expropiar() {
        CEDIDAS.fetch_add(1, Relaxed);
    }
}

// -- La puerta, en ensamblador ---------------------------------------------------
//
//    R = el rsp del juego al saltar la alarma
//    [R-128, R)   su zona roja (la de System V; Windows no tiene, y no cuesta)
//    [R-136]      el RIP del buzon, copiado (el `jmp` final lo lee de aqui:
//                 debajo de rsp nada lo pisa, una interrupcion de Ring 3 va a
//                 la pila del kernel)
//    debajo       rax, banderas, los otros 14, y 512 de fxsave alineados a 16
core::arch::global_asm!(
    ".globl proton_x_expropiar",
    "proton_x_expropiar:",
    "lea rsp, [rsp - 256]",
    "push rax",
    "mov rax, qword ptr [rip + {buzon}]",
    "mov qword ptr [rsp + 128], rax",
    "pushfq",
    "push rcx",
    "push rdx",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rbp, rsp",
    "and rsp, -16",
    "sub rsp, 512",
    "fxsave64 [rsp]",
    "cld",
    "mov rdi, rax",
    "call {decidir}",
    "fxrstor64 [rsp]",
    "mov rsp, rbp",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "pop rdx",
    "pop rcx",
    "popfq",
    "pop rax",
    "lea rsp, [rsp + 256]",
    "jmp qword ptr [rsp - 136]",
    ".globl proton_x_expropiar_fin",
    "proton_x_expropiar_fin:",
    buzon = sym PROTON_X_BUZON,
    decidir = sym decidir,
);

extern "C" {
    fn proton_x_expropiar();
    fn proton_x_expropiar_fin();
}
