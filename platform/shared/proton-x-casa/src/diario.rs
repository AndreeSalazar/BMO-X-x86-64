//! **EL DIARIO** (P0.3 de `PLAN_LAS_TRES_GRANDES`, 30-09): cada funcion de
//! Windows que el `.exe` llama POR PRIMERA VEZ, en orden, a un fichero.
//!
//! Es lo que convierte el primer contacto con un juego en un mapa: cuando se
//! para, la cola del diario dice por donde iba.
//!
//! ```text
//!    apagado (lo normal)   tabla() da la funcion de la casa, tal cual
//!    encendido             tabla() da un TRAMPOLIN: 16 bytes que ponen su
//!                          numero en r11 y saltan a `comun`
//!    comun                 si es la primera vez: guarda los registros de
//!                          argumentos (rcx, rdx, r8, r9, xmm0 a xmm5),
//!                          apunta la funcion y los vuelve a poner; y salta
//!                          a la funcion de verdad
//! ```
//!
//! r11 y rax son volatiles en Win64 y no llevan argumentos: el trampolin los
//! puede usar sin que la funcion llamada note nada. Los trampolines estan
//! hechos de antemano (`.rept`), en el codigo de la casa: no se genera codigo
//! en vivo.
//!
//! Solo pasa por aqui lo que el `.exe` resuelve (el cargador y
//! `GetProcAddress`): la casa se busca a si misma con `tabla_casa`, que nunca
//! da un trampolin. Los metodos de COM (D3D12, DXGI) van por sus vtables y no
//! se apuntan.
//!
//! Se escribe el fichero ENTERO en cada funcion nueva: si el `.exe` muere,
//! lo ultimo apuntado ya esta en el disco. Cuesta disco y solo se paga con
//! el diario encendido.
//!
//! **EL ANILLO** (01-10): Cyberpunk acabo saliendo con ExitProcess(0), sin
//! fallo, y el diario (solo la PRIMERA vez de cada funcion) no decia que
//! hizo justo antes. Cada llamada, primera o no, deja en un anillo de
//! [`ANILLO`] su numero de trampolin y su direccion de vuelta (quien
//! llamo): dos `mov` y un `inc`, con rax y r10 (volatiles, sin argumentos).
//! Al salir (`al_salir`) el anillo va al final del diario: las ultimas
//! llamadas, con modulo+RVA de quien las hizo.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{aviso, plataforma};

/// Cuantos trampolines hay (las 1619 de Cyberpunk caben, con sitio).
const PUESTOS: usize = 4096;

/// Cuantas llamadas guarda el anillo (potencia de 2: el asm hace `and`).
const ANILLO: usize = 256;

/// El anillo: cuantas llamadas van, y de cada una su trampolin y su vuelta.
#[no_mangle]
static mut PROTON_X_DIARIO_CUENTA: u32 = 0;
#[no_mangle]
static mut PROTON_X_DIARIO_ANILLO: [u16; ANILLO] = [0; ANILLO];
#[no_mangle]
static mut PROTON_X_DIARIO_VUELTAS: [u64; ANILLO] = [0; ANILLO];
/// El hilo (su id de Windows, `gs:[0x48]`) de cada llamada del anillo
/// (el pulso, 02-10).
#[no_mangle]
static mut PROTON_X_DIARIO_HILOS: [u32; ANILLO] = [0; ANILLO];

/// Lo que el trampolin lee: si ya se vio, y a donde saltar. Estaticos con
/// nombre para que el asm los alcance por `sym`.
#[no_mangle]
static mut PROTON_X_DIARIO_VISTOS: [u8; PUESTOS] = [0; PUESTOS];
#[no_mangle]
static mut PROTON_X_DIARIO_DESTINOS: [u64; PUESTOS] = [0; PUESTOS];

core::arch::global_asm!(
    ".balign 16",
    ".globl proton_x_diario_puestos",
    "proton_x_diario_puestos:",
    ".set proton_x_diario_i, 0",
    ".rept 4096",
    ".balign 16",
    "movl $proton_x_diario_i, %r11d",
    "jmp proton_x_diario_comun",
    ".set proton_x_diario_i, proton_x_diario_i + 1",
    ".endr",
    ".globl proton_x_diario_comun",
    "proton_x_diario_comun:",
    // El anillo: [cuenta & 255] = (trampolin, vuelta). rax y r10 son
    // volatiles y no llevan argumentos.
    "movl {cuenta}(%rip), %eax",
    "incl %eax",
    "movl %eax, {cuenta}(%rip)",
    // El pulso (02-10): cada 1024 llamadas, el latido mira el reloj. Se
    // guardan los argumentos como para `primera` (abajo).
    "testl $1023, %eax",
    "jnz 3f",
    "pushq %rcx",
    "pushq %rdx",
    "pushq %r8",
    "pushq %r9",
    "pushq %r11",
    "subq $0x80, %rsp",
    "movdqu %xmm0, 0x20(%rsp)",
    "movdqu %xmm1, 0x30(%rsp)",
    "movdqu %xmm2, 0x40(%rsp)",
    "movdqu %xmm3, 0x50(%rsp)",
    "movdqu %xmm4, 0x60(%rsp)",
    "movdqu %xmm5, 0x70(%rsp)",
    "callq {latido}",
    "movdqu 0x20(%rsp), %xmm0",
    "movdqu 0x30(%rsp), %xmm1",
    "movdqu 0x40(%rsp), %xmm2",
    "movdqu 0x50(%rsp), %xmm3",
    "movdqu 0x60(%rsp), %xmm4",
    "movdqu 0x70(%rsp), %xmm5",
    "addq $0x80, %rsp",
    "popq %r11",
    "popq %r9",
    "popq %r8",
    "popq %rdx",
    "popq %rcx",
    "movl {cuenta}(%rip), %eax",
    "3:",
    "andl $255, %eax",
    "leaq {anillo}(%rip), %r10",
    "movw %r11w, (%r10,%rax,2)",
    "leaq {hilos}(%rip), %r10",
    "leaq (%r10,%rax,4), %r10",
    "pushq %rax",
    "movl %gs:0x48, %eax",
    "movl %eax, (%r10)",
    "popq %rax",
    "leaq {vueltas}(%rip), %r10",
    "leaq (%r10,%rax,8), %r10",
    "movq (%rsp), %rax",
    "movq %rax, (%r10)",
    "leaq {vistos}(%rip), %rax",
    "cmpb $0, (%rax,%r11)",
    "jne 2f",
    // Al entrar, rsp = 8 mod 16 (lo dejo el `call` del `.exe`). Cinco push
    // (48) y 0x80 lo dejan en 0: alineada para llamar.
    "pushq %rcx",
    "pushq %rdx",
    "pushq %r8",
    "pushq %r9",
    "pushq %r11",
    "subq $0x80, %rsp",
    "movdqu %xmm0, 0x20(%rsp)",
    "movdqu %xmm1, 0x30(%rsp)",
    "movdqu %xmm2, 0x40(%rsp)",
    "movdqu %xmm3, 0x50(%rsp)",
    "movdqu %xmm4, 0x60(%rsp)",
    "movdqu %xmm5, 0x70(%rsp)",
    "movl %r11d, %ecx",
    "callq {primera}",
    "movdqu 0x20(%rsp), %xmm0",
    "movdqu 0x30(%rsp), %xmm1",
    "movdqu 0x40(%rsp), %xmm2",
    "movdqu 0x50(%rsp), %xmm3",
    "movdqu 0x60(%rsp), %xmm4",
    "movdqu 0x70(%rsp), %xmm5",
    "addq $0x80, %rsp",
    "popq %r11",
    "popq %r9",
    "popq %r8",
    "popq %rdx",
    "popq %rcx",
    "2:",
    "leaq {destinos}(%rip), %rax",
    "jmpq *(%rax,%r11,8)",
    cuenta = sym PROTON_X_DIARIO_CUENTA,
    anillo = sym PROTON_X_DIARIO_ANILLO,
    vueltas = sym PROTON_X_DIARIO_VUELTAS,
    hilos = sym PROTON_X_DIARIO_HILOS,
    latido = sym crate::pulso::latido,
    vistos = sym PROTON_X_DIARIO_VISTOS,
    destinos = sym PROTON_X_DIARIO_DESTINOS,
    primera = sym primera,
    options(att_syntax)
);

extern "C" {
    static proton_x_diario_puestos: u8;
}

struct Estado {
    /// Donde se escribe; `None`: apagado.
    ruta: Option<Vec<u8>>,
    /// (dll, funcion, direccion de la casa) de cada trampolin dado.
    puestos: Vec<(String, String, u64)>,
    /// El diario, tal como va al fichero.
    texto: Vec<u8>,
    vistas: u32,
    lleno: bool,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea, hilos cooperativos; se lee y escribe en el acto.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { ruta: None, puestos: Vec::new(), texto: Vec::new(), vistas: 0, lleno: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

const CABECERA: &[u8] = b"# DIARIO de PROTON-X: cada funcion de Windows, la PRIMERA vez que el .exe la llama\n# orden hilo dll funcion\n";

/// **Encender el diario** en `ruta` (`None`: apagarlo). Antes de resolver
/// las importaciones del `.exe`: lo resuelto antes no pasa por aqui. Todo lo
/// de un diario anterior se olvida.
pub fn diario(ruta: Option<&[u8]>) {
    let e = estado();
    e.ruta = ruta.map(|r| r.to_vec());
    e.puestos.clear();
    e.texto = CABECERA.to_vec();
    e.vistas = 0;
    e.lleno = false;
    // SAFETY: ver `Notas`.
    unsafe { *NOTAS.0.get() = 0 };
    // SAFETY: una tarea (ver `Global`); nadie salta por un trampolin ahora.
    unsafe {
        *core::ptr::addr_of_mut!(PROTON_X_DIARIO_CUENTA) = 0;
        (*core::ptr::addr_of_mut!(PROTON_X_DIARIO_VISTOS)).fill(0);
        (*core::ptr::addr_of_mut!(PROTON_X_DIARIO_DESTINOS)).fill(0);
    }
}

/// Si el diario esta encendido (`personal diario ...`): lo que solo se
/// cuenta al investigar (los NO de la memoria) sale solo entonces.
pub(crate) fn encendido() -> bool {
    estado().ruta.is_some()
}

/// **Lo que `tabla` da al `.exe`**: la funcion `d` de la casa, o su
/// trampolin si el diario esta encendido. La misma funcion, el mismo
/// trampolin (GetProcAddress dos veces da lo mismo, como en Windows).
pub(crate) fn envolver(dll: &str, funcion: &str, d: u64) -> u64 {
    let e = estado();
    if e.ruta.is_none() {
        return d;
    }
    if let Some(i) = e.puestos.iter().position(|p| p.2 == d) {
        return puesto(i);
    }
    if e.puestos.len() >= PUESTOS {
        if !e.lleno {
            e.lleno = true;
            aviso("diario: mas de 4096 funciones distintas; las demas no se apuntan");
        }
        return d;
    }
    let i = e.puestos.len();
    e.puestos.push((String::from(dll), String::from(funcion), d));
    // SAFETY: una tarea; el indice esta dentro.
    unsafe { (*core::ptr::addr_of_mut!(PROTON_X_DIARIO_DESTINOS))[i] = d };
    puesto(i)
}

/// La direccion del trampolin `i` (16 bytes cada uno).
fn puesto(i: usize) -> u64 {
    // Solo se toma la direccion del simbolo del asm (no se lee).
    let base = core::ptr::addr_of!(proton_x_diario_puestos) as u64;
    base + 16 * i as u64
}

/// **La primera llamada** a la funcion del trampolin `i`: se apunta y el
/// diario entero va al disco.
extern "win64" fn primera(i: u32) {
    let e = estado();
    let i = i as usize;
    // SAFETY: una tarea; el indice lo puso `envolver`.
    unsafe { (*core::ptr::addr_of_mut!(PROTON_X_DIARIO_VISTOS))[i] = 1 };
    let Some((dll, f, _)) = e.puestos.get(i) else { return };
    e.vistas += 1;
    let hilo = crate::kernel32::get_current_thread_id();
    let linea = alloc::format!("{:>5} {:>5} {dll} {f}\n", e.vistas, hilo);
    e.texto.extend_from_slice(linea.as_bytes());
    if let Some(r) = &e.ruta {
        if !(plataforma().escribir_fichero)(r, &e.texto) {
            aviso("diario: no se pudo escribir el fichero");
        }
    }
}


/// **Al salir el proceso** (ExitProcess, o cualquier salida de la casa): las
/// ultimas llamadas del anillo, de la mas vieja a la mas nueva, al final del
/// diario, con quien las hizo. Con el diario apagado, nada.
pub fn al_salir(codigo: u32) {
    let e = estado();
    let Some(r) = e.ruta.clone() else { return };
    let mut t = alloc::format!("# saliendo con {codigo:#x}\n");
    t.push_str(&crate::pulso::texto((plataforma().ahora_ns)()));
    t.push_str(&ultimas(ANILLO));
    e.texto.extend_from_slice(t.as_bytes());
    let _ = (plataforma().escribir_fichero)(&r, &e.texto);
}

/// Cuantas llamadas del `.exe` van (las que pasaron por un trampolin).
pub(crate) fn llamadas() -> u32 {
    // SAFETY: una tarea; un u32 que solo escribe el trampolin.
    unsafe { *core::ptr::addr_of!(PROTON_X_DIARIO_CUENTA) }
}

/// `dll funcion`, de un trampolin.
fn nombre(i: u16) -> String {
    estado().puestos.get(i as usize).map_or(String::from("? ?"), |p| alloc::format!("{} {}", p.0, p.1))
}

/// **Las ultimas `n` llamadas del anillo** (como mucho [`ANILLO`]), de la
/// mas vieja a la mas nueva: hilo, funcion y quien la llamo.
pub(crate) fn ultimas(n: usize) -> String {
    // SAFETY: una tarea; el `.exe` no llama a nada mientras se lee.
    let (cuenta, anillo, vueltas, hilos) = unsafe {
        (*core::ptr::addr_of!(PROTON_X_DIARIO_CUENTA), *core::ptr::addr_of!(PROTON_X_DIARIO_ANILLO), *core::ptr::addr_of!(PROTON_X_DIARIO_VUELTAS), *core::ptr::addr_of!(PROTON_X_DIARIO_HILOS))
    };
    let n = n.min(ANILLO).min(cuenta as usize);
    let mut t = alloc::format!("# las ultimas {n} llamadas (de {cuenta}); la ultima, abajo\n# hilo dll funcion <- quien la llamo\n");
    for k in 0..n {
        let j = (cuenta as usize + 1 + ANILLO - n + k) % ANILLO;
        t.push_str(&alloc::format!("  {:>5} {} <- {}\n", hilos[j], nombre(anillo[j]), crate::pulso::donde(vueltas[j])));
    }
    t
}

/// La ultima llamada del hilo `id` que queda en el anillo, y quien la hizo.
pub(crate) fn ultima_de(id: u32) -> Option<String> {
    // SAFETY: como en `ultimas`.
    let (cuenta, anillo, vueltas, hilos) = unsafe {
        (*core::ptr::addr_of!(PROTON_X_DIARIO_CUENTA), *core::ptr::addr_of!(PROTON_X_DIARIO_ANILLO), *core::ptr::addr_of!(PROTON_X_DIARIO_VUELTAS), *core::ptr::addr_of!(PROTON_X_DIARIO_HILOS))
    };
    let n = (cuenta as usize).min(ANILLO);
    (0..n).map(|k| (cuenta as usize + ANILLO - k) % ANILLO).find(|&j| hilos[j] == id).map(|j| alloc::format!("{} <- {}", nombre(anillo[j]), crate::pulso::donde(vueltas[j])))
}

/// **El diario y, detras, `extra`** (la foto del pulso), al fichero: la foto
/// no se queda en el texto, la siguiente la reemplaza.
pub(crate) fn escribir_con(extra: &str) {
    let e = estado();
    let Some(r) = &e.ruta else { return };
    let mut v = e.texto.clone();
    v.extend_from_slice(extra.as_bytes());
    if !(plataforma().escribir_fichero)(r, &v) {
        aviso("diario: no se pudo escribir la foto del pulso");
    }
}

/// Cuantas notas de investigacion (rutas que no estan, carpetas que se dan)
/// se dicen por la consola: las primeras.
const MAX_NOTAS: u32 = 64;

struct Notas(UnsafeCell<u32>);
// SAFETY: una tarea, hilos cooperativos.
unsafe impl Sync for Notas {}
static NOTAS: Notas = Notas(UnsafeCell::new(0));

/// **Una nota de investigacion** (01-10), solo con el diario encendido y
/// solo las [`MAX_NOTAS`] primeras: Cyberpunk se rinde en silencio leyendo
/// su configuracion, y lo que no encuentra es justo lo que hay que ver.
pub(crate) fn nota(texto: &str) {
    if !encendido() {
        return;
    }
    // SAFETY: ver `Notas`.
    let n = unsafe { &mut *NOTAS.0.get() };
    *n += 1;
    if *n <= MAX_NOTAS {
        crate::decir(texto);
    }
}

/// La nota de una ruta del `.exe` que no se pudo abrir o mirar.
pub(crate) fn no_esta(que: &str, nombre: *const u16) {
    if !encendido() || nombre.is_null() {
        return;
    }
    // SAFETY: una cadena del `.exe` acabada en 0 (la funcion de Windows la pide asi).
    let w = unsafe { crate::user32::utf16(nombre) };
    nota(&alloc::format!("{que}(\"{}\"): no (error {})", String::from_utf16_lossy(&w), crate::kernel32::ultimo_error()));
}
