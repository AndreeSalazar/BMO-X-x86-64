//! **LA ESCALERA DE PROTON-X en el banco** (05-10,
//! `docs/plan/PLAN_LA_ESCALERA_PROTON_X.md`): las muestras de Microsoft
//! (DirectX-Graphics-Samples, MIT), el `.exe` DE VERDAD compilado de su
//! fuente sin cambiarla (como, en `prueba/muestras/HACER.txt`). Una capa
//! nueva por escalon, y la imagen esperada sale de las reglas de D3D12, no de
//! la casa (R3 del plan).

use super::*;

/// La huella (`bmo_cubo::referencia::huella`) de una superficie de `w` x `h`
/// pixeles, todos de `color` (`0x00RRGGBB`).
fn huella_lisa(color: u32, w: u32, h: u32) -> u64 {
    bmo_cubo::referencia::huella(&vec![color; (w * h) as usize])
}

/// **E1.1 -- HelloWindow** (`D3D12HelloWindow`, 1280 x 720): la cadena de
/// intercambio de un `.exe` de Microsoft, un Clear, una valla y Present, y
/// nada mas. Lo que pide de la casa: CreateDXGIFactory2, EnumAdapterByGpu
/// Preference, D3D12CreateDevice, la cola, CreateSwapChainForHwnd con
/// FLIP_DISCARD, MakeWindowAssociation, el monton de RTV, ClearRenderTarget
/// View con barreras PRESENT <-> RENDER_TARGET, Signal y SetEventOnCompletion.
///
/// **El juez:** el color de limpiar es `{0.0, 0.2, 0.4, 1.0}` en R8G8B8A8_UNORM.
/// D3D12 pasa de float a UNORM con `f * 255` redondeado al mas cercano: 0,
/// 51 (51.000001) y 102 (102.000002), sin empate posible. Asi que CADA pixel
/// de CADA fotograma es R 0x00, G 0x33, B 0x66, exacto: bit a bit.
///
/// La muestra solo sale cerrando su ventana (WM_CLOSE); aqui, a los 60
/// Present el banco la saca con 0xF00D.
#[test]
fn e1_1_hellowindow_de_microsoft_limpia_cada_fotograma_al_azul_exacto_de_d3d12() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/hwindow.exe", "");
    TOPE_PRESENTES.store(60, Ordering::SeqCst);
    let (salio, dicho, _) = correr_exe(&uno, HWINDOW, true, &[]);
    TOPE_PRESENTES.store(1000, Ordering::SeqCst);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = String::from_utf8_lossy(&dicho);
    assert_eq!(salio, 0xF00D, "presento hasta el tope del banco, sin salir antes: {texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    let (w, h) = {
        let p = PANTALLA.lock().unwrap();
        assert_eq!(p.len(), 1, "una ventana, una superficie");
        (p[0].1, p[0].2)
    };
    assert_eq!((w, h), (1280, 720), "la ventana es el area de cliente que pidio");
    let vistas = VISTAS.lock().unwrap().clone();
    assert_eq!(vistas.len(), 60);
    let azul = huella_lisa(0x00_00_33_66, w, h);
    assert!(vistas.iter().all(|&v| v == azul), "cada Present, R 0x00 G 0x33 B 0x66 en cada pixel");
}

/// Una funcion de la casa por su DLL y su nombre, como la resuelve el cargador.
fn de_la_casa(dll: &str, nombre: &str) -> u64 {
    bmo_proton_x_casa::tabla(dll, &Funcion::Nombre(nombre.into())).unwrap_or_else(|| panic!("la casa no tiene {dll}!{nombre}"))
}

fn ancha(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// Una cadena UTF-16 acabada en 0, de vuelta a `String`.
fn de_ancha(p: u64) -> String {
    let mut v = Vec::new();
    // SAFETY: una cadena de la casa acabada en 0.
    unsafe {
        while *((p as *const u16).add(v.len())) != 0 {
            v.push(*((p as *const u16).add(v.len())));
        }
    }
    String::from_utf16(&v).unwrap()
}

/// **Lo que E1.1 le pidio a la casa, una a una** (05-10). El `.exe` de
/// HelloWindow las importa y no todas las llama: aqui cada una se llama con
/// los casos que muerden.
#[test]
fn e1_1_las_doce_que_pidio_hellowindow_hacen_lo_de_windows() {
    let _uno = uno_a_la_vez();
    // SAFETY: un `.exe` a la vez (el cerrojo); la casa, desde cero.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    bmo_proton_x_casa::proceso::poner_exe("window/hwindow.exe", "");
    // Un TEB y un PEB en GS, como `correr_exe`: GetCurrentThreadId lo lee ahi.
    let hilo_mem = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(hilo_mem);
    {
        let rsp: u64;
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let h = teb::Hilo { teb: mem, peb: mem + teb::TEB_BYTES as u64, pila_tope: rsp + (64 << 10), pila_fondo: rsp - (256 << 10), proceso: 7, hilo: 42, base_imagen: 0 };
        // SAFETY: `mem` son TEB_BYTES + PEB_BYTES recien pedidos, R+W.
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, hilo_mem as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
        poner_gs(mem);
    }

    // shell32!CommandLineToArgvW: las reglas del CRT, en UN bloque de LocalAlloc.
    type Partir = extern "win64" fn(*const u16, *mut i32) -> u64;
    let partir: Partir = unsafe { core::mem::transmute(de_la_casa("shell32.dll", "CommandLineToArgvW")) };
    let soltar: extern "win64" fn(u64) -> u64 = unsafe { core::mem::transmute(de_la_casa("kernel32.dll", "LocalFree")) };
    let argv = |linea: &str| -> Vec<String> {
        let mut n = -1;
        let l = ancha(linea);
        let b = partir(l.as_ptr(), &mut n);
        assert_ne!(b, 0);
        // SAFETY: `n` punteros y un NULL detras, en el bloque que dio la casa.
        let v: Vec<String> = (0..n as u64).map(|k| de_ancha(unsafe { *((b + k * 8) as *const u64) })).collect();
        assert_eq!(unsafe { *((b + n as u64 * 8) as *const u64) }, 0, "argv acaba en NULL");
        assert_eq!(soltar(b), 0, "UN LocalFree lo suelta entero");
        v
    };
    assert_eq!(
        argv(r#""C:\a b\x.exe" uno "dos tres" c\"d e\\\"f g\\h"#),
        [r"C:\a b\x.exe", "uno", "dos tres", "c\"d", r#"e\"f"#, r"g\\h"],
        "el programa hasta su comilla; 2n+1 barras y comilla: n barras y una comilla; barras sin comilla, tal cual"
    );
    let mut ruta = vec![0u16; 260];
    let gmfn: extern "win64" fn(u64, *mut u16, u32) -> u32 = unsafe { core::mem::transmute(de_la_casa("kernel32.dll", "GetModuleFileNameW")) };
    let n = gmfn(0, ruta.as_mut_ptr(), 260) as usize;
    assert_eq!(argv(""), [String::from_utf16(&ruta[..n]).unwrap()], "linea vacia: la ruta del .exe");
    assert_eq!(partir(ancha("x").as_ptr(), core::ptr::null_mut()), 0, "sin argc: NULL");

    // kernel32!GetThreadId: el pseudo-handle de GetCurrentThread es el hilo de ahora.
    let gti: extern "win64" fn(u64) -> u32 = unsafe { core::mem::transmute(de_la_casa("kernel32.dll", "GetThreadId")) };
    let gcti: extern "win64" fn() -> u32 = unsafe { core::mem::transmute(de_la_casa("kernel32.dll", "GetCurrentThreadId")) };
    assert_eq!(gti(u64::MAX - 1), gcti());
    assert_eq!(gcti(), 42, "el hilo del TEB");
    assert_eq!(gti(0x1234_5678), 0, "un handle que no es de un hilo: 0");

    // El CRT, en la locale "C": un byte, un caracter.
    const CONV: &str = "api-ms-win-crt-convert-l1-1-0.dll";
    let mbrtowc: extern "win64" fn(*mut u16, *const u8, usize, u64) -> usize = unsafe { core::mem::transmute(de_la_casa(CONV, "mbrtowc")) };
    let wcrtomb: extern "win64" fn(*mut u8, u16, u64) -> usize = unsafe { core::mem::transmute(de_la_casa(CONV, "wcrtomb")) };
    let mut wc = 0u16;
    assert_eq!(mbrtowc(&mut wc, b"\xE9".as_ptr(), 1, 0), 1);
    assert_eq!(wc, 0xE9, "el byte es el caracter");
    assert_eq!(mbrtowc(&mut wc, b"\0".as_ptr(), 1, 0), 0, "el 0 da 0");
    assert_eq!(mbrtowc(&mut wc, b"a".as_ptr(), 0, 0), usize::MAX - 1, "sin bytes: incompleto");
    let mut b = 0u8;
    assert_eq!(wcrtomb(&mut b, 0x41, 0), 1);
    assert_eq!(b, b'A');
    assert_eq!(wcrtomb(&mut b, 0x3B1, 0), usize::MAX, "una alfa no cabe en un byte");
    let ismbblead: extern "win64" fn(u32) -> i32 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-multibyte-l1-1-0.dll", "_ismbblead")) };
    assert_eq!(ismbblead(0x81), 0, "ningun byte empieza uno de dos");

    // signal: guarda y devuelve el de antes; un numero que no es de C, SIG_ERR.
    const RUN: &str = "api-ms-win-crt-runtime-l1-1-0.dll";
    let signal: extern "win64" fn(i32, u64) -> u64 = unsafe { core::mem::transmute(de_la_casa(RUN, "signal")) };
    assert_eq!(signal(2, 1), 0, "SIGINT: antes, SIG_DFL");
    assert_eq!(signal(2, 0), 1, "y ahora devuelve el SIG_IGN que se puso");
    assert_eq!(signal(3, 1), u64::MAX, "3 no es de C en Windows: SIG_ERR");

    // Las variables cuya DIRECCION da el CRT.
    let acmdln: extern "win64" fn() -> *const u64 = unsafe { core::mem::transmute(de_la_casa(RUN, "__p__acmdln")) };
    let linea = unsafe { std::ffi::CStr::from_ptr(*acmdln() as *const core::ffi::c_char) }.to_str().unwrap().to_owned();
    assert!(linea.contains("hwindow.exe"), "la linea de ordenes entera: {linea}");
    let environ: extern "win64" fn() -> *const u64 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-environment-l1-1-0.dll", "__p__environ")) };
    let inicial: extern "win64" fn() -> u64 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-environment-l1-1-0.dll", "_get_initial_narrow_environment")) };
    assert_eq!(unsafe { *environ() }, inicial(), "_environ es la copia del entorno del CRT");
    let fmode: extern "win64" fn() -> *const i32 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-stdio-l1-1-0.dll", "__p__fmode")) };
    assert_eq!(unsafe { *fmode() }, 0, "_fmode empieza en texto");
    let daylight: extern "win64" fn() -> *const i32 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-time-l1-1-0.dll", "__daylight")) };
    assert_eq!(unsafe { *daylight() }, 0, "UTC: sin horario de verano");

    // rand_s: del generador de la casa; NULL, EINVAL.
    let rand_s: extern "win64" fn(*mut u32) -> i32 = unsafe { core::mem::transmute(de_la_casa("api-ms-win-crt-utility-l1-1-0.dll", "rand_s")) };
    let (mut a, mut z) = (0u32, 0u32);
    assert_eq!(rand_s(&mut a), 0);
    assert_eq!(rand_s(&mut z), 0);
    assert_ne!((a, z), (0, 0), "dos de 32 bits al azar no son los dos 0");
    assert_eq!(rand_s(core::ptr::null_mut()), 22);
    poner_gs(0);
    munmap(mem, hilo_mem);
    *DICHO.lock().unwrap() = Vec::new();
}
