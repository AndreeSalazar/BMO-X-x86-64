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

use std::sync::{Mutex, MutexGuard};

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use bmo_proton_x::*;
use bmo_proton_x_casa::{Plataforma, Superficie};

const HOLA: &[u8] = include_bytes!("../../proton-x/prueba/hola.exe");
const TEB: &[u8] = include_bytes!("../../proton-x/prueba/teb.exe");
const VENTANA: &[u8] = include_bytes!("../../proton-x/prueba/ventana.exe");
const LIMPIA: &[u8] = include_bytes!("../../proton-x/prueba/limpia.exe");
const CUBO: &[u8] = include_bytes!("../../proton-x/prueba/cubo.exe");
const HILOS: &[u8] = include_bytes!("../../proton-x/prueba/hilos.exe");
const FICHEROS: &[u8] = include_bytes!("../../proton-x/prueba/ficheros.exe");
const CRT: &[u8] = include_bytes!("../../proton-x/prueba/crt.exe");
const TEXTO: &[u8] = include_bytes!("../../proton-x/prueba/texto.exe");
const ESPERAS: &[u8] = include_bytes!("../../proton-x/prueba/esperas.exe");
const CARPETAS: &[u8] = include_bytes!("../../proton-x/prueba/carpetas.exe");
const SISTEMA: &[u8] = include_bytes!("../../proton-x/prueba/sistema.exe");
const UCRT: &[u8] = include_bytes!("../../proton-x/prueba/ucrt.exe");
const STDIO: &[u8] = include_bytes!("../../proton-x/prueba/stdio.exe");

/// Como se llama el `.exe` que corre y lo que se escribio detras (P4e: su
/// GetModuleFileNameW y su GetCommandLineW).
static NOMBRE: Mutex<(&str, &str)> = Mutex::new(("window/prueba.exe", ""));
const CUBO_DATOS_H: &str = include_str!("../../proton-x/prueba/cubo_datos.h");

#[path = "../examples/cubo_datos.rs"]
mod cubo_datos;

/// Los `.exe` comparten la vuelta, lo dicho y la pantalla (estaticos): uno a
/// la vez. Cada prueba lo coge ENTERA, no solo mientras corre su `.exe`: lo
/// que mira despues (la pantalla, los dibujos) otra prueba lo borraria.
static UNO_A_LA_VEZ: Mutex<()> = Mutex::new(());

fn uno_a_la_vez() -> MutexGuard<'static, ()> {
    UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

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
/// La huella (`bmo_cubo::referencia::huella`) de cada superficie PRESENTADA,
/// en orden: lo que se vio en la ventana, fotograma a fotograma.
static VISTAS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

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

fn presentar(s: &Superficie) {
    PRESENTADAS.fetch_add(1, Ordering::SeqCst);
    // SAFETY: la superficie es un Vec de PANTALLA (ver `superficie`), vivo.
    let px = unsafe { core::slice::from_raw_parts(s.pixeles, (s.stride * s.alto) as usize) };
    VISTAS.lock().unwrap().push(bmo_cubo::referencia::huella(px));
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
    // De verdad, como en BMO-X (4 ms alli): los plazos de P4 (Sleep, WaitFor*
    // con tiempo) cuentan con que el reloj avance mientras se duerme.
    std::thread::sleep(std::time::Duration::from_micros(500));
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
    Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x_casa::nativo::dibujar, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar }
}

/// Codigo SELLADO, como `MEM_OP_SELLAR`: memoria nueva, los bytes, y de
/// R+W a R+X (sin W). Cuantos se sellaron y cuantos se soltaron, para el banco.
static SELLADOS: AtomicU32 = AtomicU32::new(0);
static SOLTADOS: AtomicU32 = AtomicU32::new(0);

fn sellar_codigo(bytes: &[u8]) -> Option<u64> {
    let n = bytes.len().div_ceil(4096) as u64 * 4096;
    let base = mmap(n);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), base as *mut u8, bytes.len()) };
    mprotect(base, n, PROT_LEE | PROT_EJECUTA);
    SELLADOS.fetch_add(1, Ordering::SeqCst);
    Some(base)
}

fn soltar_codigo(base: u64, bytes: usize) {
    munmap(base, bytes.div_ceil(4096) as u64 * 4096);
    SOLTADOS.fetch_add(1, Ordering::SeqCst);
}

/// El volumen del banco: un directorio del anfitrion, uno por proceso de
/// pruebas (las rutas de la casa van relativas a el).
fn volumen() -> std::path::PathBuf {
    static RAIZ: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    RAIZ.get_or_init(|| {
        let r = std::env::temp_dir().join(format!("proton-x-volumen-{}", std::process::id()));
        std::fs::create_dir_all(r.join("window")).unwrap();
        r
    })
    .clone()
}

fn leer_fichero(ruta: &[u8]) -> Option<Vec<u8>> {
    std::fs::read(volumen().join(std::str::from_utf8(ruta).ok()?)).ok()
}

fn escribir_fichero(ruta: &[u8], bytes: &[u8]) -> bool {
    let Ok(r) = std::str::from_utf8(ruta) else { return false };
    std::fs::write(volumen().join(r), bytes).is_ok()
}

/// Una arena del monton de Windows (P4e): memoria del anfitrion a ceros, que
/// no se suelta (vive lo que el proceso, como en BMO-X).
fn memoria(bytes: usize) -> Option<u64> {
    let forma = std::alloc::Layout::from_size_align(bytes, 1 << 16).ok()?;
    // SAFETY: una forma de medida no nula.
    let p = unsafe { std::alloc::alloc_zeroed(forma) };
    (!p.is_null()).then_some(p as u64)
}

/// La fecha del banco: la del anfitrion (P4f2).
fn fecha() -> Option<u64> {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Lo que hay en una carpeta del volumen del banco (P4f3).
fn listar(ruta: &[u8]) -> Option<Vec<bmo_proton_x::ficheros::Entrada>> {
    let r = volumen().join(std::str::from_utf8(ruta).ok()?);
    let mut v = Vec::new();
    for e in std::fs::read_dir(r).ok()? {
        let e = e.ok()?;
        let m = e.metadata().ok()?;
        v.push(bmo_proton_x::ficheros::Entrada { nombre: e.file_name().to_string_lossy().into_owned(), carpeta: m.is_dir(), bytes: m.len() });
    }
    Some(v)
}

/// La hora del banco: la del anfitrion, desde que empezo el proceso.
fn ahora_ns() -> u64 {
    static ORIGEN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGEN.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
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
fn correr_exe(_uno: &MutexGuard<'static, ()>, exe: &[u8], con_teb: bool, guion: &[u64]) -> (u32, Vec<u8>, u64) {
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
    VISTAS.lock().unwrap().clear();
    for c in [&MOSTRADAS, &PRESENTADAS, &DORMIDAS, &SELLADOS, &SOLTADOS] {
        c.store(0, Ordering::SeqCst);
    }
    // SAFETY: un `.exe` a la vez (el cerrojo de arriba), antes de saltar.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    // P4: el TLS, como el cargador de Windows (y como `proton-x.bex`).
    let t = tls::leer(&pe, &img, base).unwrap();
    // SAFETY: GS puesto (si hay TEB), `empezar` hecho, la imagen en su sitio.
    unsafe { bmo_proton_x_casa::hilos::preparar_tls(t, base) };
    // P4d: como `run sys/proton-x.bex window/x.exe`, su directorio es `window`.
    let (nombre, resto) = *NOMBRE.lock().unwrap();
    bmo_proton_x_casa::ficheros::poner_directorio(nombre.rsplit_once('/').map_or("", |(d, _)| d));
    bmo_proton_x_casa::proceso::poner_exe(nombre, resto);
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
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, HOLA, false, &[]);
    assert_eq!(salio, 0, "ExitProcess con 0: escribio la frase entera");
    assert_eq!(dicho.as_slice(), b"hola desde un .exe de Windows\r\n");
}

/// **P1d en el anfitrion**: `teb.exe` lee su TEB, su PEB, su base, su pila y su
/// LastError por `gs:`, y dice `bien` seis veces. Sin el GS puesto, el primer
/// `gs:[0x30]` leeria de la direccion 0x30: un fallo de pagina, no un "MAL".
#[test]
fn teb_exe_encuentra_su_teb_y_su_peb_en_gs() {
    let uno = uno_a_la_vez();
    let (salio, dicho, base) = correr_exe(&uno, TEB, true, &[]);
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
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, VENTANA, true, &guion);
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
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, LIMPIA, true, &[letra(b'b'), 0, letra(b'q')]);
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
/// y se salta al hueco 11 de su vtabla, `CreateComputePipelineState` (los
/// sombreadores de calculo no son de P3b).
#[test]
fn un_hueco_que_falta_dice_cual_es_y_sale() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    // SAFETY: nada corre; se pone la plataforma de mentira.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let dir = bmo_proton_x_casa::tabla("d3d12.dll", &Funcion::Nombre("D3D12CreateDevice".into())).unwrap();
    let crear: extern "win64" fn(u64, u32, *const [u8; 16], *mut u64) -> i32 = unsafe { core::mem::transmute(dir as usize) };
    let mut disp = 0u64;
    assert_eq!(crear(0, 0xb000, &bmo_proton_x_casa::com::IID_DEVICE, &mut disp), 0);
    // SAFETY: `disp` es un objeto de la casa: su primer puntero es la vtabla.
    let hueco11 = unsafe { (*(disp as *const *const u64)).add(11).read() };
    let salio = unsafe { correr(hueco11) };
    assert_eq!(salio, 0xC0DE_000B, "0xC0DE0000 | interfaz 0 (el dispositivo) << 8 | hueco 11");
    assert_eq!(
        String::from_utf8_lossy(&DICHO.lock().unwrap()),
        "PROTON-X: ID3D12Device::CreateComputePipelineState (hueco 11) no esta en la casa\n"
    );
}

/// **`prueba/cubo_datos.h` es la salida de su fabrica**: nadie lo toco a mano,
/// y lleva los bits de `bmo-cubo` y los `.dxil` de hoy.
#[test]
fn cubo_datos_h_es_lo_que_fabrica_su_ejemplo() {
    assert!(cubo_datos::texto() == CUBO_DATOS_H, "rehacer: cargo run -p bmo-proton-x-casa --example cubo_datos > platform/shared/proton-x/prueba/cubo_datos.h");
}

/// **P3b2 y P3b3 en el anfitrion**: `cubo.exe`, un programa D3D12 ENTERO
/// (root signature serializada, PSO con los DXIL de dxc, buferes UPLOAD
/// mapeados, y cada fotograma Reset con el PSO, raiz, CBV, viewport, destino,
/// limpiar, vertices, indices y DrawIndexedInstanced). Dos cosas:
///
/// - lo que cada dibujo VE (la captura de P3b2): los vertices, los indices y
///   las constantes de los fotogramas 0, 30 y 60 son, bit a bit, los de
///   `bmo-cubo` (lo que X4 subio a la 3060);
/// - lo que se VIO (P3b3): la ventana de cada Present tiene la HUELLA de lo
///   que D3D12 dibujo en la 3060 bajo Windows. Los sombreadores de dxc
///   corridos en la CPU y la trama de la casa, sin el juez de por medio.
#[test]
fn cubo_exe_monta_la_tuberia_entera_y_cada_dibujo_ve_lo_de_x1() {
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, CUBO, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    assert_eq!(
        String::from_utf8_lossy(&dicho),
        "",
        "ni un aviso ni un hueco que falte: la casa supo hacer todo el dibujo"
    );
    let vistas = VISTAS.lock().unwrap().clone();
    let huellas: Vec<u64> = bmo_cubo::referencia::HUELLAS.iter().map(|&(_, h)| h).collect();
    assert_eq!(vistas, huellas, "lo que se vio en cada Present es lo que dibujo la 3060 (fotogramas 0, 30, 60)");
    // PostQuitMessage(presentados): el de arrancar y el de cada letra.
    assert_eq!(salio, 3);
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 3);

    // P3b3b: un PSO, un bloque de codigo sellado; y lo que se VIO arriba lo
    // dibujaron sus sombreadores NATIVOS (el ejecutor de la casa).
    assert_eq!(SELLADOS.load(Ordering::SeqCst), 1, "un PSO: un bloque sellado");
    let dibujos = bmo_proton_x_casa::tuberia::dibujos();
    assert_eq!(dibujos.len(), 3, "un DrawIndexedInstanced por fotograma");
    let v = bmo_cubo::vertices();
    let i: Vec<u32> = bmo_cubo::indices().iter().map(|&x| x as u32).collect();
    for (d, f) in dibujos.iter().zip([0u32, 30, 60]) {
        assert_eq!((d.vs.as_str(), d.ps.as_str()), ("vertice", "pixel"), "los puntos de entrada de cubo.hlsl");
        assert_eq!((d.topologia, d.cuantos, d.instancias), (4, 36, 1), "TRIANGLELIST, 36 indices, una instancia");
        assert_eq!((d.descarte, d.antihorario), (3, false), "descarte de las caras de detras, horario delante");
        assert_eq!(d.viewport, [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0]);
        assert_ne!(d.destino, 0);
        // Los vertices, leidos A TRAVES del input layout: pos, normal, color.
        assert_eq!(d.vertices.len(), v.len());
        for (leido, x) in d.vertices.iter().zip(&v) {
            let bits = |s: &[f32]| s.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
            assert_eq!(leido.len(), 3);
            assert_eq!(bits(&leido[0]), bits(&x.pos));
            assert_eq!(bits(&leido[1]), bits(&x.normal));
            assert_eq!(bits(&leido[2]), bits(&x.color));
        }
        assert_eq!(d.indices, i);
        // b0: wvp, world y luz del fotograma `f`, y el resto del bufer a cero.
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        let mut esperado: Vec<u8> = c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect();
        esperado.resize(256, 0);
        assert!(d.constantes == esperado, "las constantes del fotograma {f}");
    }

    // La ventana: 1280x720; la esquina, el fondo de X1 (16, 16, 24) en BGRA.
    let p = PANTALLA.lock().unwrap();
    assert_eq!((p[0].1, p[0].2), (1280, 720));
    assert_eq!(p[0].0[0], 0xFF10_1018);
}

/// **P4 en el anfitrion**: `hilos.exe`, hilos, TLS y sincronizacion de
/// Windows con los hilos COOPERATIVOS de la casa. Lo que dice no depende del
/// orden en que corran los hilos: es lo mismo que dice en Windows.
#[test]
fn hilos_exe_tiene_hilos_tls_y_sincronizacion_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, HILOS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 19, "{texto}");
    assert!(texto.ends_with("hilos.exe: los hilos son los de Windows\r\n[salio 0x0]"), "{texto}");
    assert_eq!(salio, 0, "{texto}");
}

/// **P4d en el anfitrion**: `ficheros.exe` crea, escribe, lee, se mueve y
/// vuelve a crear un fichero junto al `.exe` (`window/pxtest.txt` del volumen
/// del banco), con los errores de Windows donde tocan.
#[test]
fn ficheros_exe_lee_y_escribe_ficheros_como_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, FICHEROS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 16, "{texto}");
    assert!(texto.ends_with("ficheros.exe: los ficheros son los de Windows\r\n[salio 0x0]"), "{texto}");
    assert_eq!(std::fs::read(volumen().join("window/pxtest.txt")).unwrap(), b"corto", "y en el volumen queda lo ultimo que escribio");
}

/// **P4e en el anfitrion**: `crt.exe` pide y suelta del monton de Windows
/// (dos mil bloques, 8 MiB, un HeapCreate), reserva y hace paginas con
/// VirtualAlloc, y lee su nombre, su linea y su entorno.
#[test]
fn crt_exe_tiene_la_memoria_y_el_proceso_de_windows() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/crt.exe", "-nivel 3");
    let (salio, dicho, _) = correr_exe(&uno, CRT, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 35, "{texto}");
    assert!(texto.ends_with("crt.exe: la memoria y el proceso son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f en el anfitrion**: `texto.exe` pasa texto entre UTF-8 y UTF-16,
/// escribe por WriteConsoleW, y carga d3d12.dll y sus funciones por
/// LoadLibraryW + GetProcAddress, sobre la tabla de la casa.
#[test]
fn texto_exe_tiene_el_texto_la_consola_y_los_modulos_de_windows() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/texto.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, TEXTO, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(texto.contains("  bien  WriteConsoleW escribe UTF-16 en la consola\r\n"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 21, "{texto}");
    assert!(texto.ends_with("texto.exe: el texto y los modulos son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f2 en el anfitrion**: `esperas.exe` -- mutex (y abandonado),
/// temporizadores, WaitOnAddress importado de un API set, FLS con sus
/// callbacks, DuplicateHandle y la hora del dia.
#[test]
fn esperas_exe_tiene_las_esperas_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, ESPERAS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 24, "{texto}");
    assert!(texto.ends_with("esperas.exe: las esperas son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f3 en el anfitrion**: `carpetas.exe` busca con comodines, abre su
/// carpeta, pregunta a un handle, cambia una medida, copia, y oye los NO de
/// Windows. Dos veces seguidas: lo que deja no le estorba a la segunda.
#[test]
fn carpetas_exe_tiene_las_carpetas_de_windows() {
    let uno = uno_a_la_vez();
    for vez in 0..2 {
        *NOMBRE.lock().unwrap() = ("window/carpetas.exe", "");
        let (salio, dicho, _) = correr_exe(&uno, CARPETAS, true, &[]);
        *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
        let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
        assert!(!texto.contains("  MAL   "), "vuelta {vez}: {texto}");
        assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
        assert_eq!(texto.matches("  bien  ").count(), 34, "{texto}");
        assert!(texto.ends_with("carpetas.exe: las carpetas son las de Windows\r\n[salio 0x0]"), "{texto}");
    }
    assert_eq!(std::fs::read(volumen().join("window/pzc.txt")).unwrap(), b"abc");
}

/// **P4f4 en el anfitrion**: `sistema.exe` -- NtReadFile/NtWriteFile,
/// ProcessPrng, OVERLAPPED, FormatMessageW, SetStdHandle, los nombres del
/// sistema, la red que no esta y los procesos; sale con TerminateProcess.
#[test]
fn sistema_exe_tiene_lo_demas_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, SISTEMA, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 22, "{texto}");
    assert!(texto.ends_with("sistema.exe: lo demas es lo de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f5 en el anfitrion**: `ucrt.exe` -- el arranque del CRT de MSVC
/// (argv, entorno, _initterm), sus tablas de salida, su monton, memoria y
/// cadenas, importado de api-ms-win-crt-* y vcruntime140.dll; y su ultima
/// linea la dice una funcion de _crt_atexit que corre exit().
#[test]
fn ucrt_exe_tiene_el_crt_de_msvc() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/ucrt.exe", "-nivel 3");
    let (salio, dicho, _) = correr_exe(&uno, UCRT, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(texto.contains("__p___argc y __p___argv: argv[argc] es NULL 0x0000000000000003"), "tres argumentos: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 19, "{texto}");
    assert!(texto.ends_with("ucrt.exe: el CRT es el de Windows (dicho desde _crt_atexit)\r\n[salio 0x0]"), "{texto}");
}

/// **P4f5 en el anfitrion**: `stdio.exe` -- el printf del CRT de MSVC
/// (__stdio_common_* con las banderas de las cabeceras del UCRT), y stdout y
/// stderr en modo texto.
#[test]
fn stdio_exe_tiene_el_printf_de_msvc() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, STDIO, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(!texto.replace("\r\n", "").contains('\n'), "modo texto: cada \\n sale \\r\\n: {texto:?}");
    assert_eq!(texto.matches("  bien  ").count(), 15, "{texto}");
    assert!(texto.ends_with("stdio.exe: el printf es el de Windows\r\n[salio 0x0]"), "{texto}");
}
