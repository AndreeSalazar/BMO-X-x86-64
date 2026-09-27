//! **`hola.exe`, `teb.exe` y `ventana.exe` CORREN en esta CPU** (P1b, P1d y
//! P2, 27-09): el banco de verdad, con las DLL de la casa DE VERDAD.
//!
//! Las otras pruebas miran los bytes. Esta EJECUTA el `.exe` de Windows en el
//! anfitrion (Linux, x86-64), como `proton-x.bex` en el Ryzen:
//!
//! ```text
//!    partir     dos zonas seguidas: codigo (R+X, sin W) y datos (R+W, sin X)
//!    colocar    en una base que no es la suya, con las relocalizaciones
//!    resolver   contra `bmo_proton_x_casa::tabla`: LAS MISMAS kernel32,
//!               user32 y gdi32 que corren en BMO-X
//!    el GS      un TEB y un PEB (`bmo_proton_x::teb`) y `arch_prctl`, lo que
//!               en BMO-X es `TASK_OP_PON_GS`
//!    saltar     a su entrada con la convencion de Windows
//! ```
//!
//! Lo unico de mentira es la PLATAFORMA de debajo ([`plataforma`]): la consola
//! es un `Vec`, una superficie es memoria de este proceso y el buzon es un
//! guion de teclas y clics. Si el cargador o una DLL de la casa fallan, aqui
//! no sale un "distinto": sale un fallo de pagina.
//!
//! La memoria se pide con `mmap`/`mprotect` por `syscall`, sin `libc`.
//! `ExitProcess` no vuelve (en BMO-X es `salir`); aqui el proceso es el de las
//! pruebas y no puede morir, asi que la plataforma salta de vuelta a donde se
//! llamo a la entrada, con la pila de entonces: [`correr`].

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use bmo_proton_x::*;
use bmo_proton_x_casa::{Plataforma, Superficie};

const HOLA: &[u8] = include_bytes!("../../proton-x/prueba/hola.exe");
const TEB: &[u8] = include_bytes!("../../proton-x/prueba/teb.exe");
const VENTANA: &[u8] = include_bytes!("../../proton-x/prueba/ventana.exe");
const LIMPIA: &[u8] = include_bytes!("../../proton-x/prueba/limpia.exe");

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

// ============================ LA PLATAFORMA DE MENTIRA ============================

static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Las superficies que pidio el `.exe`: (pixeles, ancho, alto), y cuantas
/// veces se mostraron y se presentaron.
static PANTALLA: Mutex<Vec<(Vec<u32>, u32, u32)>> = Mutex::new(Vec::new());
static MOSTRADAS: AtomicU32 = AtomicU32::new(0);
static PRESENTADAS: AtomicU32 = AtomicU32::new(0);
static DORMIDAS: AtomicU32 = AtomicU32::new(0);

/// **El guion del buzon.** Cada `evento` saca el siguiente; un 0 del guion es
/// "ahora no hay nada", y se gasta. Asi se ve lo que hace `GetMessageW` cuando
/// la cola se queda libre: pintar.
static GUION: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());

fn escribir(b: &[u8]) {
    DICHO.lock().unwrap().extend_from_slice(b);
}

fn superficie(ancho: u32, alto: u32) -> Option<Superficie> {
    let mut p = PANTALLA.lock().unwrap();
    p.push((vec![0u32; (ancho * alto) as usize], ancho, alto));
    let i = p.len() - 1;
    // El Vec no se mueve mientras viva la prueba: su puntero vale.
    Some(Superficie { pixeles: p[i].0.as_mut_ptr(), ancho, alto, stride: ancho, dato: i as u64 })
}

fn mostrar(_s: &Superficie) -> bool {
    MOSTRADAS.fetch_add(1, Ordering::SeqCst);
    true
}

fn presentar(_s: &Superficie) {
    PRESENTADAS.fetch_add(1, Ordering::SeqCst);
}

fn evento(_s: &Superficie) -> u64 {
    GUION.lock().unwrap().pop_front().unwrap_or(0)
}

/// Un `.exe` que espera para siempre no puede colgar el banco: a las mil
/// siestas, fuera con 0xDEAD.
fn dormir() {
    if DORMIDAS.fetch_add(1, Ordering::SeqCst) > 1000 {
        salir(0xDEAD);
    }
}

/// Donde volver al salir, y con que pila: los deja [`correr`].
static mut PILA: u64 = 0;
static mut VUELTA: u64 = 0;
static mut MARCO: u64 = 0;
static mut SALIO: u32 = u32::MAX;

fn salir(codigo: u32) -> ! {
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

fn plataforma() -> Plataforma {
    Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir }
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
fn correr_exe(exe: &[u8], con_teb: bool, guion: &[u64]) -> (u32, Vec<u8>, u64) {
    let _uno = UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    *GUION.lock().unwrap() = guion.iter().copied().collect();
    let pe = leer(exe).unwrap();
    let partes = partir(&pe).unwrap();
    let total = (partes.codigo + partes.datos) as u64;
    let base = mmap(total);
    assert_ne!(base, pe.base, "en una base que NO es la suya");
    let mut img = colocar(&pe, exe, base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    resolver(&mut img, &imps, bmo_proton_x_casa::tabla).unwrap();
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
    PANTALLA.lock().unwrap().clear();
    for c in [&MOSTRADAS, &PRESENTADAS, &DORMIDAS] {
        c.store(0, Ordering::SeqCst);
    }
    // SAFETY: un `.exe` a la vez (el cerrojo de arriba), antes de saltar.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
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
    let (salio, dicho, _) = correr_exe(HOLA, false, &[]);
    assert_eq!(salio, 0, "ExitProcess con 0: escribio la frase entera");
    assert_eq!(dicho.as_slice(), b"hola desde un .exe de Windows\r\n");
}

/// **P1d en el anfitrion**: `teb.exe` lee su TEB, su PEB, su base, su pila y su
/// LastError por `gs:`, y dice `bien` seis veces. Sin el GS puesto, el primer
/// `gs:[0x30]` leeria de la direccion 0x30: un fallo de pagina, no un "MAL".
#[test]
fn teb_exe_encuentra_su_teb_y_su_peb_en_gs() {
    let (salio, dicho, base) = correr_exe(TEB, true, &[]);
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

/// **P2 en el anfitrion**: `ventana.exe`, una ventana Win32 de manual, con el
/// `user32` y el `gdi32` de la casa. El guion: una letra, un clic en (10, 20)
/// y `q`, con un momento libre entre cada uno (ahi Windows pinta).
#[test]
fn ventana_exe_abre_su_ventana_pinta_y_obedece_al_teclado_y_al_raton() {
    const HAY: u64 = 1 << 8;
    const PULSADA: u64 = 1 << 9;
    let letra = |c: u8| 1 << 62 | HAY | PULSADA | c as u64;
    let clic = 1 << 63 | HAY | PULSADA | 1 | 10 << 16 | 20 << 32;
    let guion = [letra(b'b'), 0, 0, clic, 0, 0, letra(b'q')];
    let (salio, dicho, _) = correr_exe(VENTANA, true, &guion);
    assert_eq!(dicho, b"", "ni un aviso: la casa supo hacer todo lo que pidio");
    // PostQuitMessage((letras << 8) | clics): una letra (la q cierra) y un clic.
    assert_eq!(salio, 0x101);
    assert_eq!(MOSTRADAS.load(Ordering::SeqCst), 1, "ShowWindow la ofrecio UNA vez");
    // Tres dibujos: UpdateWindow, tras la letra y tras el clic. La q destruye
    // la ventana antes de que su ultimo WM_PAINT salga, como en Windows.
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 3);
    let p = PANTALLA.lock().unwrap();
    assert_eq!(p.len(), 1);
    let (px, ancho, alto) = (&p[0].0, p[0].1, p[0].2);
    assert_eq!((ancho, alto), (320, 200));
    let en = |x: u32, y: u32| px[(y * ancho + x) as usize];
    assert_eq!(en(0, 0), 0xFFFF_D700, "el marco dorado");
    assert_eq!(en(10, 20), 0xFFFF_FFFF, "el cuadrado blanco donde cayo el clic");
    // El degradado con el tinte 1 (una letra): en (100, 100), r=128 g=0x80 b=79.
    assert_eq!(en(100, 100), 0xFF80_804F);
}

/// **P3a en el anfitrion**: `limpia.exe`, el esqueleto de todo programa D3D12
/// (dispositivo, cola, cadena de intercambio, RTV, lista, valla y evento), con
/// el `d3d12` y el `dxgi` de la casa. Limpia y presenta al arrancar, y otra
/// vez con el color siguiente en cada letra.
#[test]
fn limpia_exe_limpia_su_ventana_con_d3d12_y_presenta_por_dxgi() {
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let (salio, dicho, _) = correr_exe(LIMPIA, true, &[letra(b'b'), 0, letra(b'q')]);
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso ni un hueco que falte");
    // PostQuitMessage(presentados): el de arrancar y el de la letra.
    assert_eq!(salio, 2);
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 2);
    let p = PANTALLA.lock().unwrap();
    assert_eq!(p.len(), 1, "una ventana");
    let (px, ancho, alto) = (&p[0].0, p[0].1, p[0].2);
    assert_eq!((ancho, alto), (320, 200));
    // El color 2 (0.75, 0.25, 0.0) en R8G8B8A8, presentado en la superficie
    // BGRA: R = 191, G = 64, B = 0. Todos los pixeles.
    assert!(px.iter().all(|&c| c == 0xFFBF_4000), "primero {:#x}", px[0]);
}

/// **Un hueco que la casa no tiene dice su NOMBRE y sale**: nunca un S_OK
/// callado ni un salto a cero. Se crea un dispositivo por la tabla de la casa
/// y se salta al hueco 16 de su vtabla, `CreateRootSignature` (de P3b).
#[test]
fn un_hueco_que_falta_dice_cual_es_y_sale() {
    let _uno = UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    DICHO.lock().unwrap().clear();
    // SAFETY: nada corre; se pone la plataforma de mentira.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let dir = bmo_proton_x_casa::tabla("d3d12.dll", &Funcion::Nombre("D3D12CreateDevice".into())).unwrap();
    let crear: extern "win64" fn(u64, u32, *const [u8; 16], *mut u64) -> i32 = unsafe { core::mem::transmute(dir as usize) };
    let mut disp = 0u64;
    assert_eq!(crear(0, 0xb000, &bmo_proton_x_casa::com::IID_DEVICE, &mut disp), 0);
    // SAFETY: `disp` es un objeto de la casa: su primer puntero es la vtabla.
    let hueco16 = unsafe { (*(disp as *const *const u64)).add(16).read() };
    let salio = unsafe { correr(hueco16) };
    assert_eq!(salio, 0xC0DE_0010, "0xC0DE0000 | interfaz 0 (el dispositivo) << 8 | hueco 16");
    assert_eq!(
        String::from_utf8_lossy(&DICHO.lock().unwrap()),
        "PROTON-X: ID3D12Device::CreateRootSignature (hueco 16) no esta en la casa\n"
    );
}
