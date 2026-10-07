//! **LOS SUB-DIRECTORES: un trozo de Ring 3 en cada obrero** (H4.3 de
//! `PLAN_LOS_DOCE_DIRECTORES`, 07-10; con el permiso de CPU del propietario:
//! *"te doy permiso con CPU pero que sea brutal para que exprima"*).
//!
//! [carril]  ROJO      un nucleo entra en el espacio de un proceso, en Ring 3
//! [consumo] NADA      corre cuando una app reparte una faena suya
//!
//! [cuesta]  MAQUINA -- una IDT, un TSS o un CR3 mal puestos aqui no dan un
//!           fallo: dan un TRIPLE FALLO en ese nucleo. Por eso todo lo que
//!           pisa el CPU vive en ESTE fichero, es de cada obrero, y el camino
//!           de ida y el de vuelta son dos trozos de ensamblador cortos.
//!
//! [riesgo]  UNICO -- cada obrero tiene SU salto, SU pila de trap y su
//!           `EN_CR3`, indexados por su orden de llegada (`fetch_add`: no se
//!           repite).
//!
//! # QUE HACE
//!
//! Hasta hoy un obrero solo corria faenas DEL KERNEL (`atril`: numeros de
//! catalogo). Esto deja que una app reparta SU codigo, en SU espacio:
//!
//! ```text
//!    la app       llena n bloques (cada uno con su pila al final, y en lo
//!                 alto de la pila la vuelta, bmo_orquesta::ring3::FIN) y
//!                 pide `repartir(funcion, arg, n)`; la parte 0 la corre ella
//!    el kernel    juzga el pedido (`bmo_orquesta::ring3`, con banco) y da a
//!                 cada obrero sano una parte k = 1..n-1
//!    el obrero    entra en Ring 3 con el CR3 de la app, en
//!                 `funcion(k, n, arg)`, IF=0, sin syscalls
//!    la vuelta    `funcion` vuelve a FIN: el #PF de buscar instrucciones
//!                 ALLI es "termine"; cualquier otra excepcion, "esta parte
//!                 fallo". En los dos casos el obrero vuelve a su bucle
//!    la app       `esperar`: que partes salieron mal (las rehace ella)
//! ```
//!
//! # POR QUE ES SEGURO (y en que se diferencia de lo que `atril` prohibe)
//!
//! `atril` dice que un puntero a funcion de Ring 3 no se llama JAMAS: seria
//! codigo de Ring 3 con el privilegio del kernel. Aqui no se llama: el
//! obrero BAJA a Ring 3 (CPL 3, `iretq`) en el CR3 del proceso que lo pidio.
//! El codigo tiene exactamente el privilegio que ya tenia en su nucleo 0.
//!
//! **Aislado del todo:**
//!
//! * su PROPIA IDT ([`IDT3`]), solo mientras esta en Ring 3: cualquier
//!   excepcion (y la NMI) va a un stub de este fichero que apunta el
//!   vector y vuelve al bucle del obrero. `fault_dispatch`, el planificador
//!   y CABINA ni se enteran: lo que un obrero hace mal no toca nada del BSP;
//! * su PROPIA pila de trap (`TSS.rsp0`, de este fichero);
//! * sin syscalls: el trampolin no enciende EFER.SCE en los APs, asi que un
//!   `syscall` en Ring 3 es #UD, o sea "esta parte fallo";
//! * IF=0: nada lo interrumpe y nada lo expropia (salvo la NMI de rescate);
//! * los registros de vector se ponen a cero al entrar (lo de otro proceso
//!   no se ve), y el CR3 se recarga en cada entrada (ni una traduccion vieja);
//! * el espacio del proceso no cambia debajo de un obrero: `vmm` llama a
//!   [`sacar`] antes de quitar o cambiar una pagina de usuario (espera a que
//!   salgan; al tope, la NMI de rescate), `revoke_all` llama a [`muere`]
//!   antes de devolver un solo marco del muerto, y `enterrador` pregunta
//!   [`cr3_ocupado`] antes de desmontar sus tablas;
//! * una parte que no vuelve no se lleva el nucleo: al tope, la NMI de
//!   rescate la saca por el stub 2 (y si la NMI llega tarde, la del kernel en
//!   un obrero es un `iretq`: ver `armar_nmi_kernel`).

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst};

use bmo_orquesta::ring3::{NoRing3, Pedido, FIN, PARTES_MAXIMAS};

use super::ficha::MAX_OBREROS;

/// Lo mas que se espera a una faena antes de dar por perdidas sus partes.
pub const TOPE_NS: u64 = 5_000_000_000;

// == EL SALTO de cada obrero: donde se vuelve de Ring 3 ===================

/// Lo que guarda la ida para la vuelta, y lo que la vuelta trae. Los
/// desplazamientos estan escritos a mano en el ensamblador de abajo.
#[repr(C)]
struct Salto {
    rsp: u64,     // 0
    rbx: u64,     // 8
    rbp: u64,     // 16
    r12: u64,     // 24
    r13: u64,     // 32
    r14: u64,     // 40
    r15: u64,     // 48
    vector: u64,  // 56
    error: u64,   // 64
    rip: u64,     // 72
    cr2: u64,     // 80
    idtr: [u8; 16], // 88: el IDTR del kernel (10 bytes)
    cr3_kernel: u64, // 104
    cr3_app: u64,    // 112
}

const SALTO_VACIO: Salto = Salto { rsp: 0, rbx: 0, rbp: 0, r12: 0, r13: 0, r14: 0, r15: 0, vector: 0, error: 0, rip: 0, cr2: 0, idtr: [0; 16], cr3_kernel: 0, cr3_app: 0 };

/// La pila con la que el CPU entra en un stub desde Ring 3 (`TSS.rsp0`). En
/// lo alto, el puntero al salto de su obrero: el stub lo lee de ahi.
#[repr(C, align(16))]
struct PilaTrap {
    datos: [u8; 4096 - 16],
    salto: u64,
    _relleno: u64,
}

static mut SALTOS: [Salto; MAX_OBREROS] = [const { SALTO_VACIO }; MAX_OBREROS];
static mut PILAS: [PilaTrap; MAX_OBREROS] = [const { PilaTrap { datos: [0; 4096 - 16], salto: 0, _relleno: 0 } }; MAX_OBREROS];

// == LA IDT DE LOS SUB-DIRECTORES =========================================

#[repr(C)]
#[derive(Clone, Copy)]
struct Puerta {
    bajo: u16,
    selector: u16,
    ist: u8,
    tipo: u8,
    medio: u16,
    alto: u32,
    _r: u32,
}

impl Puerta {
    /// Una puerta de INTERRUPCION (IF a 0), DPL 0 (un `int n` de Ring 3 es
    /// #GP: otra parte fallida), sin IST: entra por `rsp0`.
    const fn de(f: u64) -> Puerta {
        Puerta { bajo: f as u16, selector: super::tss::KERNEL_CS, ist: 0, tipo: 0x8E, medio: (f >> 16) as u16, alto: (f >> 32) as u32, _r: 0 }
    }
}

static mut IDT3: [Puerta; 32] = [Puerta { bajo: 0, selector: 0, ist: 0, tipo: 0, medio: 0, alto: 0, _r: 0 }; 32];
#[repr(C, packed)]
struct Idtr {
    limite: u16,
    base: u64,
}
#[no_mangle]
static mut BMO_RING3_IDTR: Idtr = Idtr { limite: 0, base: 0 };
static IDT_LISTA: AtomicBool = AtomicBool::new(false);

// Los 32 stubs. Con codigo de error (8, 10..14, 17, 21, 29, 30) el marco
// mide 48 bytes; sin el, 40. El puntero al salto esta justo encima (lo alto
// de la pila de trap). `sin`/`con`: sin o con codigo de error.
macro_rules! stub {
    (sin $v:literal) => {
        core::arch::global_asm!(
            concat!(".global bmo_ring3_stub_", $v),
            concat!("bmo_ring3_stub_", $v, ":"),
            "mov rax, [rsp + 40]",
            concat!("mov qword ptr [rax + 56], ", $v),
            "xor ecx, ecx",
            "mov [rax + 64], rcx",
            "mov rcx, [rsp]",
            "mov [rax + 72], rcx",
            "mov rcx, cr2",
            "mov [rax + 80], rcx",
            "jmp bmo_ring3_volver",
        );
    };
    (con $v:literal) => {
        core::arch::global_asm!(
            concat!(".global bmo_ring3_stub_", $v),
            concat!("bmo_ring3_stub_", $v, ":"),
            "mov rax, [rsp + 48]",
            concat!("mov qword ptr [rax + 56], ", $v),
            "mov rcx, [rsp]",
            "mov [rax + 64], rcx",
            "mov rcx, [rsp + 8]",
            "mov [rax + 72], rcx",
            "mov rcx, cr2",
            "mov [rax + 80], rcx",
            "jmp bmo_ring3_volver",
        );
    };
}
stub!(sin 0); stub!(sin 1); stub!(sin 2); stub!(sin 3); stub!(sin 4); stub!(sin 5); stub!(sin 6); stub!(sin 7);
stub!(con 8); stub!(sin 9); stub!(con 10); stub!(con 11); stub!(con 12); stub!(con 13); stub!(con 14); stub!(sin 15);
stub!(sin 16); stub!(con 17); stub!(sin 18); stub!(sin 19); stub!(sin 20); stub!(con 21); stub!(sin 22); stub!(sin 23);
stub!(sin 24); stub!(sin 25); stub!(sin 26); stub!(sin 27); stub!(sin 28); stub!(con 29); stub!(con 30); stub!(sin 31);

extern "C" {
    fn bmo_ring3_stub_0(); fn bmo_ring3_stub_1(); fn bmo_ring3_stub_2(); fn bmo_ring3_stub_3();
    fn bmo_ring3_stub_4(); fn bmo_ring3_stub_5(); fn bmo_ring3_stub_6(); fn bmo_ring3_stub_7();
    fn bmo_ring3_stub_8(); fn bmo_ring3_stub_9(); fn bmo_ring3_stub_10(); fn bmo_ring3_stub_11();
    fn bmo_ring3_stub_12(); fn bmo_ring3_stub_13(); fn bmo_ring3_stub_14(); fn bmo_ring3_stub_15();
    fn bmo_ring3_stub_16(); fn bmo_ring3_stub_17(); fn bmo_ring3_stub_18(); fn bmo_ring3_stub_19();
    fn bmo_ring3_stub_20(); fn bmo_ring3_stub_21(); fn bmo_ring3_stub_22(); fn bmo_ring3_stub_23();
    fn bmo_ring3_stub_24(); fn bmo_ring3_stub_25(); fn bmo_ring3_stub_26(); fn bmo_ring3_stub_27();
    fn bmo_ring3_stub_28(); fn bmo_ring3_stub_29(); fn bmo_ring3_stub_30(); fn bmo_ring3_stub_31();
    /// La ida: guarda el salto, pone la IDT de los sub-directores y el CR3 de
    /// la app, y baja a Ring 3. Vuelve (por un stub) con el vector.
    fn bmo_ring3_entrar(salto: *mut Salto, rip: u64, rsp: u64, a0: u64, a1: u64, a2: u64) -> u64;
}

// La ida y la vuelta. rdi = salto, rsi = rip, rdx = rsp de Ring 3, rcx/r8/r9
// = los tres argumentos de la funcion.
core::arch::global_asm!(
    ".global bmo_ring3_entrar",
    "bmo_ring3_entrar:",
    "mov [rdi + 0], rsp",
    "mov [rdi + 8], rbx",
    "mov [rdi + 16], rbp",
    "mov [rdi + 24], r12",
    "mov [rdi + 32], r13",
    "mov [rdi + 40], r14",
    "mov [rdi + 48], r15",
    "sidt [rdi + 88]",
    // El marco de iretq: SS, RSP, RFLAGS (IF=0), CS, RIP de Ring 3.
    "push 0x1B",
    "push rdx",
    "push 0x2",
    "push 0x23",
    "push rsi",
    // Lo ultimo en Ring 0: la IDT de los sub-directores y el CR3 de la app.
    "lidt [rip + BMO_RING3_IDTR]",
    "mov rax, [rdi + 112]",
    "mov cr3, rax",
    "mov rdi, rcx",
    "mov rsi, r8",
    "mov rdx, r9",
    // Nada del kernel cruza: los demas a cero.
    "xor eax, eax", "xor ebx, ebx", "xor ecx, ecx", "xor ebp, ebp",
    "xor r8d, r8d", "xor r9d, r9d", "xor r10d, r10d", "xor r11d, r11d",
    "xor r12d, r12d", "xor r13d, r13d", "xor r14d, r14d", "xor r15d, r15d",
    "iretq",
    // La vuelta: rax = el salto. Lo primero, el CR3 y la IDT del kernel.
    ".global bmo_ring3_volver",
    "bmo_ring3_volver:",
    "mov rcx, [rax + 104]",
    "mov cr3, rcx",
    "lidt [rax + 88]",
    "mov rsp, [rax + 0]",
    "mov rbx, [rax + 8]",
    "mov rbp, [rax + 16]",
    "mov r12, [rax + 24]",
    "mov r13, [rax + 32]",
    "mov r14, [rax + 40]",
    "mov r15, [rax + 48]",
    // Las banderas de Ring 3 que una interrupcion NO limpia (DF, AC): un
    // `std` o un `popf` con AC de la parte no llegan al kernel.
    "push 0x2",
    "popfq",
    "mov rax, [rax + 56]",
    "ret",
);

// ** LA NMI DE RESCATE QUE LLEGA TARDE. Para sacar a un obrero de una parte
// que no vuelve (o de un espacio que se va a desmapear) se le manda una NMI:
// en Ring 3 la recoge el stub 2 de arriba. Pero puede salir de la parte JUSTO
// antes de que llegue, y entonces la recoge la IDT del kernel, que en el
// vector 2 tiene un `hlt` para siempre: ese nucleo se perderia. Por eso el
// vector 2 de la IDT del kernel pasa por aqui: en un obrero (el bit 8 de
// IA32_APIC_BASE, "soy el BSP", a cero) la NMI solo puede ser un rescate y
// se ignora; en el BSP sigue al manejador de siempre, sin cambiar nada.
#[no_mangle]
static mut BMO_RING3_NMI_ORIGINAL: u64 = 0;
core::arch::global_asm!(
    ".global bmo_ring3_nmi_kernel",
    "bmo_ring3_nmi_kernel:",
    "push rax",
    "push rcx",
    "push rdx",
    "mov ecx, 0x1B",
    "rdmsr",
    "test eax, 0x100",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "jnz 2f",
    "iretq",
    "2:",
    "jmp qword ptr [rip + BMO_RING3_NMI_ORIGINAL]",
);
extern "C" {
    fn bmo_ring3_nmi_kernel();
}

/// El vector 2 de la IDT del kernel (la de todos: el BSP y los obreros
/// cargan la misma), a [`bmo_ring3_nmi_kernel`]. Misma puerta y misma IST:
/// solo cambia a donde salta. Una vez, en el BSP.
fn armar_nmi_kernel() {
    #[repr(C, packed)]
    struct Leido {
        limite: u16,
        base: u64,
    }
    let mut r = Leido { limite: 0, base: 0 };
    // SAFETY: `sidt` solo escribe los 10 bytes de `r`.
    unsafe { core::arch::asm!("sidt [{}]", in(reg) &mut r, options(nostack)) };
    let (base, limite) = (r.base, r.limite);
    if base == 0 || limite < 3 * 16 - 1 {
        return;
    }
    let puerta = (base + 2 * 16) as *mut u8;
    // SAFETY: la IDT del kernel esta mapeada (es la que esta cargada) y se
    // escribe en marcha igual que `faults::init`. El desplazamiento original
    // se guarda ANTES de cambiar la puerta: una NMI del BSP entre medias ya
    // encuentra a donde ir.
    unsafe {
        let leer16 = |o: usize| core::ptr::read_unaligned(puerta.add(o) as *const u16) as u64;
        let leer32 = |o: usize| core::ptr::read_unaligned(puerta.add(o) as *const u32) as u64;
        let original = leer16(0) | leer16(6) << 16 | leer32(8) << 32;
        if original == bmo_ring3_nmi_kernel as *const () as usize as u64 {
            return;
        }
        core::ptr::write_volatile(core::ptr::addr_of_mut!(BMO_RING3_NMI_ORIGINAL), original);
        let f = bmo_ring3_nmi_kernel as *const () as usize as u64;
        // Primero la parte alta (el kernel vive por debajo de 4 GiB: no
        // cambia) y la baja al final.
        core::ptr::write_volatile(puerta.add(8) as *mut u32, (f >> 32) as u32);
        core::ptr::write_volatile(puerta.add(6) as *mut u16, (f >> 16) as u16);
        core::ptr::write_volatile(puerta.add(0) as *mut u16, f as u16);
    }
}

/// La IDT de los sub-directores, UNA vez (la llena el BSP antes del primer
/// reparto; despues solo se lee).
fn armar_idt() {
    if IDT_LISTA.load(SeqCst) {
        return;
    }
    let f: [unsafe extern "C" fn(); 32] = [
        bmo_ring3_stub_0, bmo_ring3_stub_1, bmo_ring3_stub_2, bmo_ring3_stub_3, bmo_ring3_stub_4, bmo_ring3_stub_5, bmo_ring3_stub_6, bmo_ring3_stub_7,
        bmo_ring3_stub_8, bmo_ring3_stub_9, bmo_ring3_stub_10, bmo_ring3_stub_11, bmo_ring3_stub_12, bmo_ring3_stub_13, bmo_ring3_stub_14, bmo_ring3_stub_15,
        bmo_ring3_stub_16, bmo_ring3_stub_17, bmo_ring3_stub_18, bmo_ring3_stub_19, bmo_ring3_stub_20, bmo_ring3_stub_21, bmo_ring3_stub_22, bmo_ring3_stub_23,
        bmo_ring3_stub_24, bmo_ring3_stub_25, bmo_ring3_stub_26, bmo_ring3_stub_27, bmo_ring3_stub_28, bmo_ring3_stub_29, bmo_ring3_stub_30, bmo_ring3_stub_31,
    ];
    armar_nmi_kernel();
    // SAFETY: solo el BSP, antes de publicar el primer reparto (IDT_LISTA).
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT3);
        for (k, s) in f.iter().enumerate() {
            idt[k] = Puerta::de(*s as usize as u64);
        }
        let r = &mut *core::ptr::addr_of_mut!(BMO_RING3_IDTR);
        r.limite = (core::mem::size_of::<[Puerta; 32]>() - 1) as u16;
        r.base = idt.as_ptr() as u64;
    }
    IDT_LISTA.store(true, SeqCst);
}

// == LO PUBLICADO: la faena de ahora ======================================

static ACTIVA: AtomicBool = AtomicBool::new(false);
/// Sube con cada faena: una parte que vuelve tarde (de una faena ya cerrada)
/// no apunta en la de ahora.
static FAENA: AtomicU64 = AtomicU64::new(0);
static OWNER: AtomicU32 = AtomicU32::new(0);
static CR3_APP: AtomicU64 = AtomicU64::new(0);
static GS_APP: AtomicU64 = AtomicU64::new(0);
static FUNCION: AtomicU64 = AtomicU64::new(0);
static ARG: AtomicU64 = AtomicU64::new(0);
static BLOQUES: AtomicU64 = AtomicU64::new(0);
static BLOQUE_BYTES: AtomicU64 = AtomicU64::new(0);
static PARTES: AtomicU32 = AtomicU32::new(0);
static EMPEZO: AtomicU64 = AtomicU64::new(0);
static HECHAS: AtomicU32 = AtomicU32::new(0);
/// Cuantas partes se dieron a obreros en esta faena (fijo hasta `esperar`:
/// `ASIGNADA` baja segun acaban y no sirve para contar).
static DADAS: AtomicU32 = AtomicU32::new(0);
/// El obrero esta dentro de `atender` (aunque ya haya salido del CR3): no se
/// le da otra parte hasta que apunte la suya.
static ATENDIENDO: [AtomicBool; MAX_OBREROS] = [const { AtomicBool::new(false) }; MAX_OBREROS];
/// La parte de cada obrero en esta faena (0: ninguna).
static ASIGNADA: [AtomicU32; MAX_OBREROS] = [const { AtomicU32::new(0) }; MAX_OBREROS];
/// El CR3 en el que esta cada obrero AHORA (0: el del kernel).
static EN_CR3: [AtomicU64; MAX_OBREROS] = [const { AtomicU64::new(0) }; MAX_OBREROS];
/// Cuantos obreros estan dentro de ALGUN espacio de app (el camino rapido
/// de [`sacar`]: casi siempre 0, y entonces no mira nada mas).
static DENTRO: AtomicU32 = AtomicU32::new(0);
/// Sube con cada entrada de cada obrero, y el rescate apunta cual rescato:
/// una sola NMI por entrada (con la NMI dentro del stub, otra quedaria
/// pendiente y tumbaria la parte siguiente).
static ENTRADA: [AtomicU64; MAX_OBREROS] = [const { AtomicU64::new(0) }; MAX_OBREROS];
static RESCATADA: [AtomicU64; MAX_OBREROS] = [const { AtomicU64::new(0) }; MAX_OBREROS];
static RESCATES: AtomicU64 = AtomicU64::new(0);
/// Un obrero que no volvio a tiempo: no se le da mas.
static PERDIDO: [AtomicBool; MAX_OBREROS] = [const { AtomicBool::new(false) }; MAX_OBREROS];
/// Lo de cada parte: 0 pendiente, [`BIEN`], o `MAL | vector`.
static RESULTADO: [AtomicU64; PARTES_MAXIMAS as usize] = [const { AtomicU64::new(0) }; PARTES_MAXIMAS as usize];
const BIEN: u64 = 1;
const MAL: u64 = 1 << 8;

// Lo preparado (los bloques), de quien lo preparo.
static PREP_PID: AtomicU32 = AtomicU32::new(0);
static PREP_BLOQUES: AtomicU64 = AtomicU64::new(0);
static PREP_BYTES: AtomicU64 = AtomicU64::new(0);
static PREP_PARTES: AtomicU32 = AtomicU32::new(0);

// Lo que se ve en `smp`: cuantas partes de Ring 3 hechas y falladas.
static PARTES_BIEN: AtomicU64 = AtomicU64::new(0);
static PARTES_MAL: AtomicU64 = AtomicU64::new(0);

/// Si hay una faena de Ring 3 en marcha (`crew::repartir` no reparte encima).
pub fn activa() -> bool {
    ACTIVA.load(SeqCst)
}

/// `(hechas bien, falladas)` desde el arranque.
pub fn cuentas() -> (u64, u64) {
    (PARTES_BIEN.load(SeqCst), PARTES_MAL.load(SeqCst))
}

/// Un obrero que puede recibir una parte: dado de alta, sin fallar, sin
/// perderse, y en el CR3 del kernel.
fn sano(i: usize) -> bool {
    let r = super::ficha::retrato(i as u32);
    r.apic != u32::MAX && !super::ficha::fallado(i as u32) && r.estado != super::ficha::PARADO && !PERDIDO[i].load(SeqCst) && EN_CR3[i].load(SeqCst) == 0 && !ATENDIENDO[i].load(SeqCst)
}

/// **Cuantos obreros sanos hay**: las partes que se pueden pedir son estos
/// mas la de la app.
pub fn sanos() -> u32 {
    if super::crew::parados() {
        return 0;
    }
    (0..MAX_OBREROS).filter(|&i| sano(i)).count() as u32
}

/// **Preparar los bloques** (la primera mitad del pedido: la puerta solo
/// lleva tres numeros). Se guardan con el pid: `repartir` solo los acepta del
/// mismo.
pub fn preparar(pid: u32, bloques: u64, bloque_bytes: u64, partes: u32) {
    PREP_PID.store(0, SeqCst);
    PREP_BLOQUES.store(bloques, SeqCst);
    PREP_BYTES.store(bloque_bytes, SeqCst);
    PREP_PARTES.store(partes, SeqCst);
    PREP_PID.store(pid, SeqCst);
}

/// **Repartir** (desde el syscall del propietario, en el BSP): juzga, publica y
/// despierta a los obreros. La parte 0 la corre la app al volver.
pub fn repartir(pid: u32, cr3: u64, gs: u64, funcion: u64, arg: u64) -> Result<u64, NoRing3> {
    if ACTIVA.load(SeqCst) {
        return Err(NoRing3::Ocupado);
    }
    if PREP_PID.load(SeqCst) != pid {
        return Err(NoRing3::Bloque);
    }
    let partes = PREP_PARTES.load(SeqCst);
    let p = Pedido { funcion, arg, partes, bloques: PREP_BLOQUES.load(SeqCst), bloque_bytes: PREP_BYTES.load(SeqCst) }.juzgar(sanos())?;
    armar_idt();
    let id = FAENA.fetch_add(1, SeqCst) + 1;
    for r in RESULTADO.iter() {
        r.store(0, SeqCst);
    }
    // Una parte (1..n-1) por obrero sano, en el orden de la CPU (H4.0): un
    // nucleo fisico distinto para cada una primero, despues los hermanos SMT
    // (`topologia::orden`).
    for a in ASIGNADA.iter() {
        a.store(0, SeqCst);
    }
    let mut orden = [0u32; MAX_OBREROS];
    let n = super::topologia::orden(&mut orden);
    let mut k = 1u32;
    for &i in &orden[..n] {
        let i = i as usize;
        if k < p.partes && i < MAX_OBREROS && sano(i) {
            ASIGNADA[i].store(k, SeqCst);
            k += 1;
        }
    }
    DADAS.store(k - 1, SeqCst);
    OWNER.store(pid, SeqCst);
    CR3_APP.store(cr3, SeqCst);
    GS_APP.store(gs, SeqCst);
    FUNCION.store(p.funcion, SeqCst);
    ARG.store(p.arg, SeqCst);
    BLOQUES.store(p.bloques, SeqCst);
    BLOQUE_BYTES.store(p.bloque_bytes, SeqCst);
    PARTES.store(p.partes, SeqCst);
    HECHAS.store(0, SeqCst);
    EMPEZO.store(crate::ring0::task::scheduler::rdtsc(), SeqCst);
    ACTIVA.store(true, SeqCst);
    // La faena del kernel, a ninguna (un obrero que vea esta ronda y no
    // tenga parte de Ring 3 no repite la de antes), y la ronda: la signal.
    super::crew::TAREA.store(0, SeqCst);
    super::crew::RONDA.0.fetch_add(1, SeqCst);
    Ok(id)
}

/// **Esperar** (desde el syscall del propietario): `Ok(None)` si sigue en marcha;
/// `Ok(Some(mascara))` al acabar, con un bit por parte que NO salio bien
/// (fallo, o no la hizo nadie: la app la rehace). Al tope, las que faltan
/// cuentan como no hechas y sus obreros se dan por perdidos.
pub fn esperar(pid: u32) -> Result<Option<u64>, NoRing3> {
    if !ACTIVA.load(SeqCst) || OWNER.load(SeqCst) != pid {
        return Err(NoRing3::Partes);
    }
    let partes = PARTES.load(SeqCst);
    let asignadas = DADAS.load(SeqCst);
    let hechas = HECHAS.load(SeqCst);
    let hz = crate::ring0::task::scheduler::tsc_freq().max(1);
    let pasado = (crate::ring0::task::scheduler::rdtsc().saturating_sub(EMPEZO.load(SeqCst)) as u128 * 1_000_000_000 / hz as u128) as u64;
    if hechas < asignadas && pasado < TOPE_NS {
        return Ok(None);
    }
    if hechas < asignadas {
        // Al tope: a los que siguen dentro, la NMI de rescate (vuelven por el
        // stub 2: su parte cuenta como fallada). El que ni asi vuelve, o el
        // que ni empezo su parte, se da por perdido: no se le da mas.
        let cr3 = CR3_APP.load(SeqCst);
        let fuera = sacar_ya(cr3);
        for (i, a) in ASIGNADA.iter().enumerate() {
            if a.load(SeqCst) != 0 && (EN_CR3[i].load(SeqCst) != 0 || !fuera) {
                PERDIDO[i].store(true, SeqCst);
            }
        }
        crate::ring0::cabina::warn("smp", "sub-director: una parte no volvio a tiempo (rescatada con NMI)", pasado);
    }
    let mut mal = 0u64;
    for k in 1..partes {
        if RESULTADO[k as usize].load(SeqCst) != BIEN {
            mal |= 1 << k;
        }
    }
    for a in ASIGNADA.iter() {
        a.store(0, SeqCst);
    }
    ACTIVA.store(false, SeqCst);
    Ok(Some(mal))
}

/// **El obrero `indice` atiende su parte**, si tiene. `false` si no hay
/// faena de Ring 3 para el (entonces mira la del kernel).
pub fn atender(indice: u32) -> bool {
    let i = indice as usize;
    if i >= MAX_OBREROS || !ACTIVA.load(SeqCst) {
        return false;
    }
    let k = ASIGNADA[i].load(SeqCst);
    if k == 0 {
        return false;
    }
    let id = FAENA.load(SeqCst);
    ATENDIENDO[i].store(true, SeqCst);
    super::ficha::marcar(indice, super::ficha::TRABAJANDO);
    let t0 = super::ficha::ciclos();
    let r = correr(i, k);
    super::ficha::apuntar(indice, super::ficha::ciclos().wrapping_sub(t0));
    // Solo si sigue siendo la misma faena: una que volvio tarde, calla.
    if FAENA.load(SeqCst) == id && ACTIVA.load(SeqCst) {
        RESULTADO[k as usize].store(r, SeqCst);
        if r == BIEN {
            PARTES_BIEN.fetch_add(1, SeqCst);
        } else {
            PARTES_MAL.fetch_add(1, SeqCst);
        }
        ASIGNADA[i].store(0, SeqCst);
        HECHAS.fetch_add(1, SeqCst);
    }
    ATENDIENDO[i].store(false, SeqCst);
    true
}

/// La parte `k` en el obrero `i`: la ida a Ring 3 y su vuelta.
fn correr(i: usize, k: u32) -> u64 {
    let p = Pedido {
        funcion: FUNCION.load(SeqCst),
        arg: ARG.load(SeqCst),
        partes: PARTES.load(SeqCst),
        bloques: BLOQUES.load(SeqCst),
        bloque_bytes: BLOQUE_BYTES.load(SeqCst),
    };
    let cr3 = CR3_APP.load(SeqCst);
    // SAFETY: el salto y la pila de trap del obrero `i` solo los toca el; el
    // TSS es el suyo (`tss.rs`); los MSR, los selectores y los registros de
    // vector son de este nucleo.
    unsafe {
        let salto = &mut *core::ptr::addr_of_mut!(SALTOS[i]);
        let pila = &mut *core::ptr::addr_of_mut!(PILAS[i]);
        pila.salto = salto as *mut Salto as u64;
        super::tss::poner_rsp0(i as u32, &pila.salto as *const u64 as u64);
        let kcr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) kcr3, options(nomem, nostack));
        salto.cr3_kernel = kcr3;
        salto.cr3_app = cr3;
        salto.vector = u64::MAX;
        // Los selectores de datos a nulo y DESPUES las bases: el GS de Ring 3
        // es el TEB del hilo que repartio (lo que vea `gs:` es lo suyo).
        core::arch::asm!("mov ds, {0:x}", "mov es, {0:x}", "mov fs, {0:x}", "mov gs, {0:x}", in(reg) 0u64, options(nomem, nostack));
        escribir_msr(0xC000_0100, 0);
        escribir_msr(0xC000_0101, GS_APP.load(SeqCst));
        escribir_msr(0xC000_0102, 0);
        limpiar_vectores();
        DENTRO.fetch_add(1, SeqCst);
        ENTRADA[i].fetch_add(1, SeqCst);
        EN_CR3[i].store(cr3, SeqCst);
        bmo_ring3_entrar(salto, p.funcion, p.pila(k), k as u64, p.partes as u64, p.arg);
        EN_CR3[i].store(0, SeqCst);
        DENTRO.fetch_sub(1, SeqCst);
        // Los de datos del kernel otra vez (el iretq los dejo nulos y la
        // excepcion dejo SS a nulo), las bases de la app fuera, y `rsp0` a
        // cero: fuera de una parte, un trap desde Ring 3 falla DICIENDOLO.
        core::arch::asm!("mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}", in(reg) super::tss::KERNEL_DS as u64, options(nomem, nostack));
        escribir_msr(0xC000_0101, 0);
        super::tss::poner_rsp0(i as u32, 0);
        // El MXCSR y la x87 son del que llama en el ABI (la parte pudo
        // cambiar el redondeo): de vuelta a los de arranque, y los registros
        // de vector de la app fuera.
        limpiar_vectores();
        if salto.vector == 14 && salto.rip == FIN && salto.cr2 == FIN {
            BIEN
        } else {
            MAL | (salto.vector & 0xFF)
        }
    }
}

unsafe fn escribir_msr(msr: u32, v: u64) {
    // SAFETY: lo de quien llama.
    unsafe { core::arch::asm!("wrmsr", in("ecx") msr, in("eax") v as u32, in("edx") (v >> 32) as u32, options(nomem, nostack)) };
}

/// Los registros de vector y la x87, a cero, y el MXCSR de arranque: lo de
/// la faena de antes (quiza de otro proceso) no se ve, y todos los obreros
/// empiezan igual (los floats del codigo traducido dan los bits del
/// interprete con el MXCSR de serie).
unsafe fn limpiar_vectores() {
    let avx = crate::ring0::cpu_vendor::xsave::informe().xcr0 & 0b100 != 0;
    // SAFETY: CR4.OSFXSR (el trampolin) y, si `avx`, OSXSAVE con YMM en XCR0
    // (`smp_ap_entrada`).
    unsafe {
        if avx {
            core::arch::asm!("vzeroall", options(nomem, nostack));
        } else {
            core::arch::asm!(
                "xorps xmm0, xmm0", "xorps xmm1, xmm1", "xorps xmm2, xmm2", "xorps xmm3, xmm3",
                "xorps xmm4, xmm4", "xorps xmm5, xmm5", "xorps xmm6, xmm6", "xorps xmm7, xmm7",
                "xorps xmm8, xmm8", "xorps xmm9, xmm9", "xorps xmm10, xmm10", "xorps xmm11, xmm11",
                "xorps xmm12, xmm12", "xorps xmm13, xmm13", "xorps xmm14, xmm14", "xorps xmm15, xmm15",
                options(nomem, nostack)
            );
        }
        let mxcsr: u32 = 0x1F80;
        core::arch::asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
    }
}

// == LO QUE PREGUNTA EL RESTO DEL KERNEL ==================================

/// **Hay un obrero dentro de este CR3** (`enterrador` no lo desmonta).
pub fn cr3_ocupado(cr3: u64) -> bool {
    cr3 != 0 && EN_CR3.iter().any(|c| c.load(SeqCst) == cr3)
}

/// Lo que se espera a que un obrero salga por su pie antes de rescatarlo.
const ESPERA_SACAR_US: u64 = 20_000;

/// **Que no quede ningun obrero dentro de `cr3`** -- lo llama `vmm` antes de
/// quitar o cambiar una pagina de un espacio de usuario (un obrero dentro
/// podria tener la traduccion vieja en su TLB y escribir en un marco que ya
/// es de otro). Primero espera a que salgan solos ([`ESPERA_SACAR_US`]); al
/// tope, la NMI de rescate (su parte cuenta como fallada y la app la rehace:
/// las partes son repetibles). `true` si ya no queda ninguno.
pub fn sacar(cr3: u64) -> bool {
    if DENTRO.load(SeqCst) == 0 || !cr3_ocupado(cr3) {
        return true;
    }
    for _ in 0..ESPERA_SACAR_US / 50 {
        if !cr3_ocupado(cr3) {
            return true;
        }
        super::lapic::esperar_us(50);
    }
    sacar_ya(cr3)
}

/// La NMI de rescate a cada obrero dentro de `cr3` (una por entrada), y un
/// milisegundo para que vuelvan.
fn sacar_ya(cr3: u64) -> bool {
    for i in 0..MAX_OBREROS {
        if cr3 == 0 || EN_CR3[i].load(SeqCst) != cr3 {
            continue;
        }
        let e = ENTRADA[i].load(SeqCst);
        if RESCATADA[i].swap(e, SeqCst) == e {
            continue;
        }
        let apic = super::ficha::retrato(i as u32).apic;
        if apic == u32::MAX {
            continue;
        }
        RESCATES.fetch_add(1, SeqCst);
        // SAFETY: una IPI de NMI (modo 100, nivel afirmado) a un obrero que
        // esta en Ring 3 con la IDT de este fichero; si sale antes, la NMI
        // del kernel en un obrero es un `iretq` (`armar_nmi_kernel`).
        unsafe { super::lapic::ipi(apic, NMI_IPI) };
    }
    for _ in 0..20 {
        if !cr3_ocupado(cr3) {
            return true;
        }
        super::lapic::esperar_us(50);
    }
    !cr3_ocupado(cr3)
}

/// ICR: modo de entrega NMI (100b) y nivel afirmado. El vector se ignora.
const NMI_IPI: u32 = 0b100 << 8 | 1 << 14;

/// Las NMI de rescate mandadas desde el arranque (para `smp`).
pub fn rescates() -> u64 {
    RESCATES.load(SeqCst)
}

/// **Muere `pid`** (`revoke_all`, antes de devolver un solo marco suyo): su
/// faena se cierra (lo que vuelva tarde, calla) y los obreros salen de su
/// CR3 ([`sacar`]). `true` si ya no queda ninguno dentro.
pub fn muere(pid: u32, cr3: u64) -> bool {
    if OWNER.load(SeqCst) == pid && ACTIVA.load(SeqCst) {
        FAENA.fetch_add(1, SeqCst);
        for a in ASIGNADA.iter() {
            a.store(0, SeqCst);
        }
        ACTIVA.store(false, SeqCst);
    }
    sacar(cr3)
}
