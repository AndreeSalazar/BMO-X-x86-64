//! **`hola.exe` CORRE en esta CPU** (P1b, 27-09): el banco de verdad.
//!
//! Las otras pruebas miran los bytes. Esta EJECUTA el `.exe` de Windows en el
//! anfitrion (Linux, x86-64), como lo hara `proton-x.bex` en el Ryzen:
//!
//! ```text
//!    partir     dos zonas seguidas: codigo (R+X, sin W) y datos (R+W, sin X)
//!    colocar    en la direccion de verdad, con las relocalizaciones
//!    resolver   contra una tabla de la casa de PRUEBA: tres `extern "win64"`
//!    saltar     a su entrada, con la convencion de Windows (rcx, rdx, r8, r9
//!               y 32 bytes de sombra): la pone el compilador, no un parche
//! ```
//!
//! Y se mira lo que dijo y con que salio. Si el cargador colocara mal una
//! seccion, relocalizara mal el puntero o dejara una ranura de la IAT vacia,
//! aqui no sale un "distinto": sale un fallo de pagina.
//!
//! La memoria se pide con `mmap`/`mprotect` por `syscall`, sin `libc`: el
//! crate sigue sin dependencias.
//!
//! `ExitProcess` no vuelve (en BMO-X es `salir`). Aqui el proceso es el de las
//! pruebas y no puede morir, asi que `ExitProcess` salta de vuelta a donde se
//! llamo a la entrada, con la pila de entonces: [`correr`].

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::*;

const HOLA: &[u8] = include_bytes!("../prueba/hola.exe");

const PROT_LEE: u64 = 1;
const PROT_ESCRIBE: u64 = 2;
const PROT_EJECUTA: u64 = 4;

unsafe fn syscall6(n: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let r: u64;
    core::arch::asm!("syscall", inlateout("rax") n => r, in("rdi") a, in("rsi") b, in("rdx") c,
        in("r10") d, in("r8") e, in("r9") f, lateout("rcx") _, lateout("r11") _, options(nostack));
    r
}

fn mmap(bytes: u64) -> u64 {
    // MAP_PRIVATE | MAP_ANONYMOUS
    let r = unsafe { syscall6(9, 0, bytes, PROT_LEE | PROT_ESCRIBE, 0x22, u64::MAX, 0) };
    assert!(r < (-4096i64) as u64, "mmap dijo {r:#x}");
    r
}

fn mprotect(dir: u64, bytes: u64, prot: u64) {
    let r = unsafe { syscall6(10, dir, bytes, prot, 0, 0, 0) };
    assert_eq!(r, 0, "mprotect dijo {r:#x}");
}

fn munmap(dir: u64, bytes: u64) {
    unsafe { syscall6(11, dir, bytes, 0, 0, 0, 0) };
}

// ======================= LA TABLA DE LA CASA DE PRUEBA =======================

const SALIDA_ESTANDAR: u64 = 0x5A1D_A000;
static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());

extern "win64" fn get_std_handle(n: u32) -> u64 {
    // STD_OUTPUT_HANDLE = (DWORD)-11
    if n == (-11i32) as u32 { SALIDA_ESTANDAR } else { u64::MAX }
}

extern "win64" fn write_file(h: u64, b: *const u8, n: u32, escritos: *mut u32, _ov: u64) -> i32 {
    if h != SALIDA_ESTANDAR {
        return 0;
    }
    let bytes = unsafe { core::slice::from_raw_parts(b, n as usize) };
    DICHO.lock().unwrap().extend_from_slice(bytes);
    if !escritos.is_null() {
        unsafe { *escritos = n };
    }
    1
}

/// Donde volver al salir, y con que pila: los deja [`correr`].
static mut PILA: u64 = 0;
static mut VUELTA: u64 = 0;
static mut MARCO: u64 = 0;
static mut SALIO: u32 = u32::MAX;

extern "win64" fn exit_process(codigo: u32) -> ! {
    unsafe {
        SALIO = codigo;
        core::arch::asm!(
            "mov rsp, [rip + {pila}]",
            "jmp qword ptr [rip + {vuelta}]",
            pila = sym PILA,
            vuelta = sym VUELTA,
            options(noreturn)
        );
    }
}

fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    if !dll.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    match f {
        Funcion::Nombre(n) if n == "GetStdHandle" => Some(get_std_handle as *const () as usize as u64),
        Funcion::Nombre(n) if n == "WriteFile" => Some(write_file as *const () as usize as u64),
        Funcion::Nombre(n) if n == "ExitProcess" => Some(exit_process as *const () as usize as u64),
        _ => None,
    }
}

/// **Saltar a la entrada** con la pila alineada y su sombra, y volver aqui
/// tanto si la entrada vuelve como si llama a `ExitProcess`. Todo registro que
/// el `.exe` pueda tocar se da por perdido; `rbx` y `rbp` se guardan a mano
/// (el compilador no deja nombrarlos).
unsafe fn correr(entrada: u64) -> u32 {
    SALIO = u32::MAX;
    core::arch::asm!(
        "push rbx",
        "push rbp",
        "mov [rip + {marco}], rsp",
        "and rsp, -16",
        "sub rsp, 32",
        "mov [rip + {pila}], rsp",
        "lea rax, [rip + 3f]",
        "mov [rip + {vuelta}], rax",
        "call r11",
        "3:",
        "mov rsp, [rip + {marco}]",
        "pop rbp",
        "pop rbx",
        in("r11") entrada,
        marco = sym MARCO,
        pila = sym PILA,
        vuelta = sym VUELTA,
        clobber_abi("sysv64"),
        out("r12") _, out("r13") _, out("r14") _, out("r15") _,
    );
    SALIO
}

#[test]
fn hola_exe_corre_en_esta_cpu_y_dice_su_frase() {
    let pe = leer(HOLA).unwrap();
    let partes = partir(&pe).unwrap();
    // hola.exe: cabeceras y .text en dos paginas; .rdata y .reloc en otras dos.
    assert_eq!(partes, Partes { codigo: 2 * PAGINA, datos: 2 * PAGINA });
    let total = (partes.codigo + partes.datos) as u64;
    let base = mmap(total);
    // En una base que NO es la suya: las relocalizaciones tienen que trabajar.
    assert_ne!(base, pe.base);
    let mut img = colocar(&pe, HOLA, base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    resolver(&mut img, &imps, tabla).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(img.as_ptr(), base as *mut u8, img.len()) };
    mprotect(base, partes.codigo as u64, PROT_LEE | PROT_EJECUTA);
    // Los datos siguen R+W y SIN X: el W^X de la casa, tambien aqui.
    DICHO.lock().unwrap().clear();
    let salio = unsafe { correr(base + pe.entrada as u64) };
    munmap(base, total);
    assert_eq!(salio, 0, "ExitProcess con 0: escribio la frase entera");
    assert_eq!(DICHO.lock().unwrap().as_slice(), b"hola desde un .exe de Windows\r\n");
}

#[test]
fn una_seccion_de_datos_en_las_paginas_del_codigo_no_se_parte() {
    let mut pe = leer(HOLA).unwrap();
    // .rdata movida a la pagina de .text, a proposito.
    pe.secciones[1].rva = pe.secciones[0].rva + 0x100;
    assert_eq!(partir(&pe), Err(Fallo::NoSeParte(".rdata".into())));
}
