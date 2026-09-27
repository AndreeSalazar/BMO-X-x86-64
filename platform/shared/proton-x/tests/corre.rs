//! **`hola.exe` y `teb.exe` CORREN en esta CPU** (P1b y P1d, 27-09): el banco
//! de verdad.
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
//! **P1d, el GS.** `teb.exe` lee su TEB en `gs:[0x30]` como el CRT de
//! Microsoft. Aqui se lo pone `arch_prctl(ARCH_SET_GS)` --el GS de este hilo
//! en Linux-- y en BMO-X `TASK_OP_PON_GS`: la misma pieza, dos kernels. El
//! TEB y el PEB los escribe `bmo_proton_x::teb`, el mismo que usa la app.
//!
//! `ExitProcess` no vuelve (en BMO-X es `salir`). Aqui el proceso es el de las
//! pruebas y no puede morir, asi que `ExitProcess` salta de vuelta a donde se
//! llamo a la entrada, con la pila de entonces: [`correr`].

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::*;

const HOLA: &[u8] = include_bytes!("../prueba/hola.exe");
const TEB: &[u8] = include_bytes!("../prueba/teb.exe");

/// Los `.exe` comparten la vuelta y lo dicho (estaticos): uno a la vez.
static UNO_A_LA_VEZ: Mutex<()> = Mutex::new(());

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

fn gs_teb() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, gs:[0x30]", out(reg) v, options(nostack, readonly)) };
    v
}

/// Como en Windows: el ultimo error VIVE en el TEB, no en una variable nuestra.
extern "win64" fn set_last_error(e: u32) {
    unsafe { ((gs_teb() + teb::TEB_LAST_ERROR as u64) as *mut u32).write(e) };
}

extern "win64" fn get_last_error() -> u32 {
    unsafe { ((gs_teb() + teb::TEB_LAST_ERROR as u64) as *const u32).read() }
}

extern "win64" fn get_current_process_id() -> u32 {
    unsafe { ((gs_teb() + teb::TEB_PROCESS_ID as u64) as *const u64).read() as u32 }
}

extern "win64" fn get_current_thread_id() -> u32 {
    unsafe { ((gs_teb() + teb::TEB_THREAD_ID as u64) as *const u64).read() as u32 }
}

fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    if !dll.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    match f {
        Funcion::Nombre(n) if n == "GetStdHandle" => Some(get_std_handle as *const () as usize as u64),
        Funcion::Nombre(n) if n == "WriteFile" => Some(write_file as *const () as usize as u64),
        Funcion::Nombre(n) if n == "ExitProcess" => Some(exit_process as *const () as usize as u64),
        Funcion::Nombre(n) if n == "SetLastError" => Some(set_last_error as *const () as usize as u64),
        Funcion::Nombre(n) if n == "GetLastError" => Some(get_last_error as *const () as usize as u64),
        Funcion::Nombre(n) if n == "GetCurrentProcessId" => Some(get_current_process_id as *const () as usize as u64),
        Funcion::Nombre(n) if n == "GetCurrentThreadId" => Some(get_current_thread_id as *const () as usize as u64),
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

/// `arch_prctl(ARCH_SET_GS)`: el GS de ESTE hilo de Linux.
fn poner_gs(v: u64) {
    let r = unsafe { syscall6(158, 0x1001, v, 0, 0, 0, 0) };
    assert_eq!(r, 0, "arch_prctl(ARCH_SET_GS) dijo {r:#x}");
}

/// **Cargar y correr un `.exe`** como `proton-x.bex`: partir, colocar en una
/// base que no es la suya, resolver, codigo R+X, datos sin X; y si `con_teb`,
/// un TEB y un PEB en el GS. Devuelve (con que salio, lo que dijo, la base).
fn correr_exe(exe: &[u8], con_teb: bool) -> (u32, Vec<u8>, u64) {
    let _uno = UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let pe = leer(exe).unwrap();
    let partes = partir(&pe).unwrap();
    let total = (partes.codigo + partes.datos) as u64;
    let base = mmap(total);
    assert_ne!(base, pe.base, "en una base que NO es la suya");
    let mut img = colocar(&pe, exe, base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    resolver(&mut img, &imps, tabla).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(img.as_ptr(), base as *mut u8, img.len()) };
    mprotect(base, partes.codigo as u64, PROT_LEE | PROT_EJECUTA);
    let hilo_mem = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(hilo_mem);
    if con_teb {
        // La pila: la de este hilo de prueba, alrededor de donde estamos.
        let rsp: u64;
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let h = teb::Hilo {
            teb: mem,
            peb: mem + teb::TEB_BYTES as u64,
            pila_tope: rsp + (64 << 10),
            pila_fondo: rsp - (256 << 10),
            proceso: 7,
            hilo: 42,
            base_imagen: base,
        };
        // SAFETY: `mem` son TEB_BYTES + PEB_BYTES recien pedidos, R+W.
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, hilo_mem as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
        poner_gs(mem);
    }
    DICHO.lock().unwrap().clear();
    let salio = unsafe { correr(base + pe.entrada as u64) };
    if con_teb {
        poner_gs(0);
    }
    munmap(mem, hilo_mem);
    munmap(base, total);
    let dicho = DICHO.lock().unwrap().clone();
    (salio, dicho, base)
}

#[test]
fn hola_exe_corre_en_esta_cpu_y_dice_su_frase() {
    let pe = leer(HOLA).unwrap();
    // hola.exe: cabeceras y .text en dos paginas; .rdata y .reloc en otras dos.
    assert_eq!(partir(&pe).unwrap(), Partes { codigo: 2 * PAGINA, datos: 2 * PAGINA });
    let (salio, dicho, _) = correr_exe(HOLA, false);
    assert_eq!(salio, 0, "ExitProcess con 0: escribio la frase entera");
    assert_eq!(dicho.as_slice(), b"hola desde un .exe de Windows\r\n");
}

/// **P1d en el anfitrion**: `teb.exe` lee su TEB, su PEB, su base, su pila y su
/// LastError por `gs:`, y dice `bien` seis veces. Sin el GS puesto, el primer
/// `gs:[0x30]` leeria de la direccion 0x30: un fallo de pagina, no un "MAL".
#[test]
fn teb_exe_encuentra_su_teb_y_su_peb_en_gs() {
    let (salio, dicho, base) = correr_exe(TEB, true);
    let texto = String::from_utf8(dicho).unwrap();
    assert_eq!(texto.matches("  bien  ").count(), 6, "{texto}");
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(texto.contains(&format!("PEB+0x10 es la base de esta imagen 0x{base:016x}")), "{texto}");
    assert!(texto.contains("SetLastError deja su valor en gs:[0x68] 0x0000000000001234"), "{texto}");
    assert!(texto.contains("GetCurrent*Id son los del TEB 0x000000070000002a"), "{texto}");
    assert!(texto.ends_with("teb.exe: el TEB y el PEB son los de Windows\r\n"), "{texto}");
    assert_eq!(salio, 0, "ExitProcess con el numero de fallos: 0");
}

#[test]
fn una_seccion_de_datos_en_las_paginas_del_codigo_no_se_parte() {
    let mut pe = leer(HOLA).unwrap();
    // .rdata movida a la pagina de .text, a proposito.
    pe.secciones[1].rva = pe.secciones[0].rva + 0x100;
    assert_eq!(partir(&pe), Err(Fallo::NoSeParte(".rdata".into())));
}
