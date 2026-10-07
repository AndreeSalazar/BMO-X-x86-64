//! **EXPROPIAR, con hilos de la casa de verdad** (07-10, el metal:
//! Cyberpunk a los 24 s con un hilo dando vueltas en SU codigo, sin llamar
//! a Windows, y los demas sin turno para siempre).
//!
//! El kernel de BMO-X, aqui, es un SIGNAL de Linux: otro hilo del anfitrion
//! le manda `SIGUSR2` a este cada 2 ms, y el manejador hace EXACTAMENTE lo
//! que hace `task/alarma.rs` en el tick: si el RIP no esta en la puerta, lo
//! deja en el buzon y pone la puerta. Lo demas es la casa de verdad.
//!
//! El "juego" es [`vueltas_del_juego`]: espera una bandera dando vueltas con
//! `pause` y `rdtsc`, sin llamar a nadie. La bandera la pone OTRO hilo de la
//! casa, que solo corre si a este le quitan el turno.

use super::*;
use std::sync::atomic::{AtomicU64, AtomicU8};

static LISTO: AtomicU8 = AtomicU8::new(0);
static PARAR: AtomicBool = AtomicBool::new(false);
/// Las alarmas que el "kernel" del banco dejo en la puerta.
static ALARMAS: AtomicU64 = AtomicU64::new(0);

// El codigo del "juego": vueltas hasta que `[rdi]` no sea 0 o pasen `rsi`
// ciclos del TSC. Devuelve 1 si vio la bandera. Entre las dos etiquetas: lo
// que se le dice a la casa que es del juego.
core::arch::global_asm!(
    ".globl banco_vueltas_del_juego",
    "banco_vueltas_del_juego:",
    "rdtsc",
    "shl rdx, 32",
    "or rax, rdx",
    "lea r8, [rax + rsi]",
    "2:",
    "pause",
    "cmp byte ptr [rdi], 0",
    "jne 3f",
    "rdtsc",
    "shl rdx, 32",
    "or rax, rdx",
    "cmp rax, r8",
    "jb 2b",
    "xor eax, eax",
    "ret",
    "3:",
    "mov eax, 1",
    "ret",
    ".globl banco_vueltas_del_juego_fin",
    "banco_vueltas_del_juego_fin:",
    // Lo que el kernel de Linux llama al volver de un manejador.
    ".globl banco_sigreturn",
    "banco_sigreturn:",
    "mov eax, 15",
    "syscall",
    "ud2",
);

extern "C" {
    fn banco_vueltas_del_juego(bandera: *const AtomicU8, ciclos: u64) -> u64;
    fn banco_vueltas_del_juego_fin();
    fn banco_sigreturn();
}

fn vueltas_del_juego(ciclos: u64) -> bool {
    // SAFETY: lee un byte de una estatica y el TSC.
    unsafe { banco_vueltas_del_juego(&LISTO, ciclos) == 1 }
}

/// El "kernel" del banco: lo que hace `task/alarma.rs` en el tick, sobre el
/// `ucontext` de Linux (`uc_mcontext` en +40; su RIP, el registro 16).
extern "C" fn alarma(_sig: i32, _info: *mut u8, uc: *mut u8) {
    let (inicio, fin, buzon) = bmo_proton_x_casa::expropiar::puerta();
    // SAFETY: el `ucontext_t` que da el kernel de Linux al manejador.
    unsafe {
        let rip = uc.add(40 + 16 * 8) as *mut u64;
        if *rip >= inicio && *rip < fin {
            return;
        }
        (buzon as *mut u64).write_volatile(*rip);
        *rip = inicio;
    }
    ALARMAS.fetch_add(1, Ordering::SeqCst);
}

/// `rt_sigaction(SIGUSR2)` con [`alarma`], una vez.
fn poner_alarma() {
    static HECHO: std::sync::Once = std::sync::Once::new();
    HECHO.call_once(|| {
        // struct kernel_sigaction { handler, flags, restorer, mask }.
        let act: [u64; 4] = [alarma as extern "C" fn(i32, *mut u8, *mut u8) as usize as u64, 4 | 0x0400_0000 | 0x1000_0000, banco_sigreturn as *const () as usize as u64, 0];
        let r = unsafe { syscall6(13, 12, act.as_ptr() as u64, 0, 8, 0, 0) };
        assert_eq!(r, 0, "rt_sigaction dijo {r:#x}");
    });
}

/// Ciclos del TSC en `ms` milisegundos, medidos.
fn ciclos_en(ms: u64) -> u64 {
    let leer = || unsafe { core::arch::x86_64::_rdtsc() };
    let t0 = std::time::Instant::now();
    let c0 = leer();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let c = leer() - c0;
    let ns = t0.elapsed().as_nanos() as u64;
    c * ms * 1_000_000 / ns.max(1)
}

/// Un TEB y un PEB en el GS de este hilo (CreateThread copia el suyo),
/// quitados al acabar.
struct Teb(u64, u64);

impl Teb {
    fn poner() -> Self {
        let bytes = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
        let mem = mmap(bytes);
        let rsp: u64;
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let h = teb::Hilo { teb: mem, peb: mem + teb::TEB_BYTES as u64, pila_tope: rsp + (64 << 10), pila_fondo: rsp - (256 << 10), proceso: 7, hilo: 42, base_imagen: 0 };
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, bytes as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
        poner_gs(mem);
        Self(mem, bytes)
    }
}

impl Drop for Teb {
    fn drop(&mut self) {
        poner_gs(0);
        munmap(self.0, self.1);
    }
}

type CrearHilo = extern "win64" fn(u64, usize, u64, u64, u32, *mut u32) -> u64;
type Esperar = extern "win64" fn(u64, u32) -> u32;

fn k32(n: &str) -> usize {
    bmo_proton_x_casa::tabla("kernel32.dll", &bmo_proton_x::Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n}")) as usize
}

extern "win64" fn el_que_avisa(_: u64) -> u32 {
    LISTO.store(1, Ordering::SeqCst);
    0
}

#[test]
fn el_que_da_vueltas_sin_llamar_a_nada_lo_expropia_la_alarma() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let _teb = Teb::poner();
    let ms = ciclos_en(1);
    poner_alarma();

    let crear_hilo: CrearHilo = unsafe { core::mem::transmute(k32("CreateThread")) };
    let esperar: Esperar = unsafe { core::mem::transmute(k32("WaitForSingleObject")) };
    LISTO.store(0, Ordering::SeqCst);
    let mut tid = 0u32;
    let h_otro = crear_hilo(0, 0, el_que_avisa as extern "win64" fn(u64) -> u32 as usize as u64, 0, 0, &mut tid);
    assert!(h_otro != 0);

    // El "kernel": una alarma cada 2 ms a ESTE hilo (tgkill).
    let pid = unsafe { syscall6(39, 0, 0, 0, 0, 0, 0) };
    let yo = unsafe { syscall6(186, 0, 0, 0, 0, 0, 0) };
    PARAR.store(false, Ordering::SeqCst);
    let reloj = std::thread::spawn(move || {
        while !PARAR.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(2));
            unsafe { syscall6(234, pid, yo, 12, 0, 0, 0) };
        }
    });
    struct Parar(Option<std::thread::JoinHandle<()>>);
    impl Drop for Parar {
        fn drop(&mut self) {
            PARAR.store(true, Ordering::SeqCst);
            if let Some(h) = self.0.take() {
                let _ = h.join();
            }
        }
    }
    let mut reloj = Parar(Some(reloj));
    let (visitas0, _, cedidas0, _) = bmo_proton_x_casa::expropiar::cuentas();

    // LA PRUEBA QUE DICE NO: con la alarma visitando, pero sin decirle a la
    // casa que ese codigo es del juego, la puerta vuelve sin ceder (asi
    // trata el codigo de la casa): tandas de 20 ms de vueltas hasta que la
    // alarma salte 10 veces (por alarmas y no por tiempo: con el banco
    // entero corriendo, el hilo del reloj llega cuando llega), y el otro NO
    // corre en ninguna.
    let alarmas0 = ALARMAS.load(Ordering::SeqCst);
    let t0 = std::time::Instant::now();
    while ALARMAS.load(Ordering::SeqCst) < alarmas0 + 10 && t0.elapsed().as_secs() < 10 {
        assert!(!vueltas_del_juego(20 * ms), "sin ser del juego, nadie le quita el turno");
    }
    assert_eq!(LISTO.load(Ordering::SeqCst), 0);
    assert!(ALARMAS.load(Ordering::SeqCst) >= alarmas0 + 10, "la alarma SI salto: {} veces", ALARMAS.load(Ordering::SeqCst) - alarmas0);
    let (visitas, _, cedidas, _) = bmo_proton_x_casa::expropiar::cuentas();
    assert!(visitas > visitas0, "la puerta las atendio");
    assert_eq!(cedidas, cedidas0, "y no cedio ninguna");

    // Ahora ES del juego: la alarma le quita el turno, el otro pone la
    // bandera, y las vueltas acaban mucho antes de su tope (10 s).
    let inicio = banco_vueltas_del_juego as *const () as usize as u64;
    let fin = banco_vueltas_del_juego_fin as *const () as usize as u64;
    assert!(bmo_proton_x_casa::expropiar::juego(inicio, fin - inicio));
    let t0 = std::time::Instant::now();
    assert!(vueltas_del_juego(10_000 * ms), "la alarma le quito el turno y el otro hilo puso la bandera");
    assert!(t0.elapsed().as_millis() < 2000, "en unos ms, no al tope: {:?}", t0.elapsed());
    let (_, en_juego, cedidas, rip) = bmo_proton_x_casa::expropiar::cuentas();
    assert!(cedidas > cedidas0, "lo cedio la alarma");
    assert!(en_juego > 0 && rip >= inicio && rip < fin, "la pillo DENTRO de las vueltas: {rip:#x}");
    drop(reloj.0.take().map(|h| {
        PARAR.store(true, Ordering::SeqCst);
        h.join()
    }));
    assert_eq!(esperar(h_otro, u32::MAX), 0);
}

/// La puerta lo deja TODO como estaba: un "juego" que tiene sus 16
/// registros y sus xmm con valores conocidos, dando vueltas mientras la
/// alarma salta cientos de veces (cediendo el turno a otro hilo que los
/// pisa todos), y al final cada uno vale lo mismo.
#[test]
fn la_puerta_devuelve_todos_los_registros() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let _teb = Teb::poner();
    poner_alarma();
    let ms = ciclos_en(1);
    let crear_hilo: CrearHilo = unsafe { core::mem::transmute(k32("CreateThread")) };
    let esperar: Esperar = unsafe { core::mem::transmute(k32("WaitForSingleObject")) };
    PISAR.store(false, Ordering::SeqCst);
    PISADAS.store(0, Ordering::SeqCst);
    let mut tid = 0u32;
    let h_pisa = crear_hilo(0, 0, el_que_pisa as extern "win64" fn(u64) -> u32 as usize as u64, 0, 0, &mut tid);
    assert!(h_pisa != 0);
    let pid = unsafe { syscall6(39, 0, 0, 0, 0, 0, 0) };
    let yo = unsafe { syscall6(186, 0, 0, 0, 0, 0, 0) };
    let inicio = banco_registros as *const () as usize as u64;
    let fin = banco_registros_fin as *const () as usize as u64;
    assert!(bmo_proton_x_casa::expropiar::juego(inicio, fin - inicio));
    PARAR.store(false, Ordering::SeqCst);
    let reloj = std::thread::spawn(move || {
        while !PARAR.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_micros(500));
            unsafe { syscall6(234, pid, yo, 12, 0, 0, 0) };
        }
    });
    let alarmas0 = ALARMAS.load(Ordering::SeqCst);
    let mut malos = [0u64; 1];
    // Las vueltas, en tandas de 30 ms hasta que la alarma salte 50 veces y
    // el otro hilo haya pisado 10 (por cuentas, no por tiempo: ver la de
    // arriba; ceder pide ademas que pase el cuanto). Cada tanda mira.
    let t0 = std::time::Instant::now();
    while (ALARMAS.load(Ordering::SeqCst) < alarmas0 + 50 || PISADAS.load(Ordering::SeqCst) < 10) && t0.elapsed().as_secs() < 20 {
        let mut m = 0u64;
        // SAFETY: el "juego" de abajo; escribe en `m`.
        unsafe { banco_registros(&mut m, 30 * ms) };
        malos[0] |= m;
    }
    PARAR.store(true, Ordering::SeqCst);
    reloj.join().unwrap();
    PISAR.store(true, Ordering::SeqCst);
    assert_eq!(esperar(h_pisa, u32::MAX), 0);
    assert!(PISADAS.load(Ordering::SeqCst) >= 10, "el otro hilo corrio en medio y lo piso todo: {} veces", PISADAS.load(Ordering::SeqCst));
    assert!(ALARMAS.load(Ordering::SeqCst) >= alarmas0 + 50, "la alarma salto: {}", ALARMAS.load(Ordering::SeqCst) - alarmas0);
    assert_eq!(malos[0], 0, "registros que cambiaron por debajo (bit n = el n-esimo de la lista)");
}

static PISAR: AtomicBool = AtomicBool::new(false);
static PISADAS: AtomicU64 = AtomicU64::new(0);

/// El otro hilo: cada vez que le toca, pisa TODOS los registros generales y
/// los xmm, y devuelve el turno.
extern "win64" fn el_que_pisa(_: u64) -> u32 {
    let ceder: extern "win64" fn() -> i32 = unsafe { core::mem::transmute(k32("SwitchToThread")) };
    while !PISAR.load(Ordering::SeqCst) {
        // SAFETY: solo pisa registros que se declaran pisados.
        unsafe {
            core::arch::asm!(
                "mov rax, -1", "mov rcx, -1", "mov rdx, -1", "mov rsi, -1", "mov rdi, -1",
                "mov r8, -1", "mov r9, -1", "mov r10, -1", "mov r11, -1",
                "pcmpeqd xmm0, xmm0", "pcmpeqd xmm1, xmm1", "pcmpeqd xmm2, xmm2", "pcmpeqd xmm3, xmm3",
                "pcmpeqd xmm4, xmm4", "pcmpeqd xmm5, xmm5", "pcmpeqd xmm6, xmm6", "pcmpeqd xmm13, xmm13",
                "pcmpeqd xmm15, xmm15",
                "clc",
                out("rax") _, out("rcx") _, out("rdx") _, out("rsi") _, out("rdi") _,
                out("r8") _, out("r9") _, out("r10") _, out("r11") _,
                out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _, out("xmm4") _,
                out("xmm5") _, out("xmm6") _, out("xmm13") _, out("xmm15") _,
            );
        }
        PISADAS.fetch_add(1, Ordering::SeqCst);
        ceder();
    }
    0
}

// El "juego" de los registros: carga valores conocidos en rax..r15 (menos
// rsp) y en xmm0..xmm15, da vueltas `rsi` ciclos SIN tocarlos (solo rdtsc
// en una copia), y al final mira cada uno. `[rdi]` = bit por registro malo.
core::arch::global_asm!(
    ".globl banco_registros",
    "banco_registros:",
    "push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15",
    "push rdi",
    // La hora del final, en la pila: [rsp-8] tras el ultimo push.
    "rdtsc", "shl rdx, 32", "or rax, rdx", "add rax, rsi", "push rax",
    // Los valores: registro n = 0x1111_1111_1111_1111 * (n + 1), salvo rsp.
    "mov rax, 0x1111111111111111",
    "mov rbx, 0x2222222222222222",
    "mov rcx, 0x3333333333333333",
    "mov rdx, 0x4444444444444444",
    "mov rsi, 0x5555555555555555",
    "mov rdi, 0x6666666666666666",
    "mov rbp, 0x7777777777777777",
    "mov r8, 0x8888888888888888",
    "mov r9, 0x9999999999999999",
    "mov r10, 0xAAAAAAAAAAAAAAAA",
    "mov r11, 0xBBBBBBBBBBBBBBBB",
    "mov r12, 0xCCCCCCCCCCCCCCCC",
    "mov r13, 0xDDDDDDDDDDDDDDDD",
    "mov r14, 0xEEEEEEEEEEEEEEEE",
    "mov r15, 0x0F0F0F0F0F0F0F0F",
    "movq xmm0, rax", "movq xmm1, rbx", "movq xmm2, rcx", "movq xmm3, rdx",
    "movq xmm4, rsi", "movq xmm5, rdi", "movq xmm6, rbp", "movq xmm7, r8",
    "movq xmm8, r9", "movq xmm9, r10", "movq xmm10, r11", "movq xmm11, r12",
    "movq xmm12, r13", "movq xmm13, r14", "movq xmm14, r15", "movq xmm15, rax",
    // Una bandera puesta (CF=1, con stc) que tiene que seguir puesta.
    "stc",
    // Las vueltas: solo se toca la pila (que la puerta respeta) y las
    // banderas se guardan con pushfq/popfq alrededor de la comparacion.
    "2:",
    "pause",
    "pushfq",
    "push rax", "push rdx",
    "rdtsc", "shl rdx, 32", "or rax, rdx",
    "cmp rax, [rsp + 24]",
    "pop rdx", "pop rax",
    "jae 3f",
    "popfq",
    "jmp 2b",
    "3:",
    "popfq",
    // CF tiene que seguir en 1: si no, bit 16.
    "push 0",
    "jc 4f",
    "or qword ptr [rsp], 0x10000",
    "4:",
    // Cada uno, contra su valor (la mascara en [rsp]).
    "push r15",
    "mov r15, 0x1111111111111111", "cmp rax, r15", "je 5f", "or qword ptr [rsp + 8], 1", "5:",
    "mov r15, 0x2222222222222222", "cmp rbx, r15", "je 5f", "or qword ptr [rsp + 8], 2", "5:",
    "mov r15, 0x3333333333333333", "cmp rcx, r15", "je 5f", "or qword ptr [rsp + 8], 4", "5:",
    "mov r15, 0x4444444444444444", "cmp rdx, r15", "je 5f", "or qword ptr [rsp + 8], 8", "5:",
    "mov r15, 0x5555555555555555", "cmp rsi, r15", "je 5f", "or qword ptr [rsp + 8], 16", "5:",
    "mov r15, 0x6666666666666666", "cmp rdi, r15", "je 5f", "or qword ptr [rsp + 8], 32", "5:",
    "mov r15, 0x7777777777777777", "cmp rbp, r15", "je 5f", "or qword ptr [rsp + 8], 64", "5:",
    "mov r15, 0x8888888888888888", "cmp r8, r15", "je 5f", "or qword ptr [rsp + 8], 128", "5:",
    "mov r15, 0x9999999999999999", "cmp r9, r15", "je 5f", "or qword ptr [rsp + 8], 256", "5:",
    "mov r15, 0xAAAAAAAAAAAAAAAA", "cmp r10, r15", "je 5f", "or qword ptr [rsp + 8], 512", "5:",
    "mov r15, 0xBBBBBBBBBBBBBBBB", "cmp r11, r15", "je 5f", "or qword ptr [rsp + 8], 1024", "5:",
    "mov r15, 0xCCCCCCCCCCCCCCCC", "cmp r12, r15", "je 5f", "or qword ptr [rsp + 8], 2048", "5:",
    "mov r15, 0xDDDDDDDDDDDDDDDD", "cmp r13, r15", "je 5f", "or qword ptr [rsp + 8], 4096", "5:",
    "mov r15, 0xEEEEEEEEEEEEEEEE", "cmp r14, r15", "je 5f", "or qword ptr [rsp + 8], 8192", "5:",
    "pop r15",
    "mov rax, 0x0F0F0F0F0F0F0F0F", "cmp r15, rax", "je 5f", "or qword ptr [rsp], 16384", "5:",
    // Los xmm: cada uno contra el general que se le copio (bit 32 + n).
    "mov rbx, 0x1111111111111111",
    "movq rax, xmm0", "cmp rax, rbx", "je 5f", "bts qword ptr [rsp], 32", "5:",
    "mov rbx, 0x7777777777777777",
    "movq rax, xmm6", "cmp rax, rbx", "je 5f", "bts qword ptr [rsp], 38", "5:",
    "mov rbx, 0xEEEEEEEEEEEEEEEE",
    "movq rax, xmm13", "cmp rax, rbx", "je 5f", "bts qword ptr [rsp], 45", "5:",
    "mov rbx, 0x1111111111111111",
    "movq rax, xmm15", "cmp rax, rbx", "je 5f", "bts qword ptr [rsp], 47", "5:",
    "mov rbx, 0x5555555555555555",
    "movq rax, xmm4", "cmp rax, rbx", "je 5f", "bts qword ptr [rsp], 36", "5:",
    // La mascara a `[rdi]` (el rdi del principio, en la pila).
    "pop rax",
    "add rsp, 8",
    "pop rdi",
    "mov [rdi], rax",
    "pop r15", "pop r14", "pop r13", "pop r12", "pop rbp", "pop rbx",
    "ret",
    ".globl banco_registros_fin",
    "banco_registros_fin:",
);

extern "C" {
    fn banco_registros(malos: *mut u64, ciclos: u64);
    fn banco_registros_fin();
}
