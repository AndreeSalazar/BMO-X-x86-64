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

// -- E1.2 a E1.6: los Hello con sombreadores --------------------------------------
//
// Las cinco leen `shaders_VSMain.cso` y `shaders_PSMain.cso` de su carpeta (los
// compilo DXC al construir, `prueba/muestras/construir.sh`), asi que cada una
// vive en SU carpeta del volumen: `window/<nombre>/`. Las cinco dibujan el
// MISMO triangulo -- (0, 0.25 a), (0.25, -0.25 a), (-0.25, -0.25 a), con a =
// 1280/720 -- sobre el mismo azul de E1.1.

const ANCHO: u32 = 1280;
const ALTO: u32 = 720;
const AZUL: u32 = 0x00_00_33_66;

/// Correr la muestra `nombre` (`window/<nombre>/<nombre>.exe`, con lo de
/// `prueba/muestras/<nombre>/` copiado al lado: sus `.cso` y sus datos) hasta `presentes` Present,
/// guardando los pixeles de los de `fotos`. Devuelve (salio, lo dicho, las
/// huellas de cada Present, las fotos).
fn correr_muestra(exe: &[u8], nombre: &'static str, presentes: u32, fotos: &[u32]) -> (u32, String, Vec<u64>, Vec<(u32, Vec<u32>, u32, u32)>) {
    correr_muestra_con(exe, nombre, presentes, fotos, &[])
}

/// [`correr_muestra`] con un GUION de entrada (E2.4: una tecla).
fn correr_muestra_con(exe: &[u8], nombre: &'static str, presentes: u32, fotos: &[u32], guion: &[u64]) -> (u32, String, Vec<u64>, Vec<(u32, Vec<u32>, u32, u32)>) {
    let uno = uno_a_la_vez();
    let dir = volumen().join("window").join(nombre);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Todo lo de su carpeta: sus `.cso` y sus datos (la malla de E2.2).
    for f in std::fs::read_dir(format!("../proton-x/prueba/muestras/{nombre}")).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), dir.join(f.file_name())).unwrap();
    }
    let ruta: &'static str = Box::leak(format!("window/{nombre}/{nombre}.exe").into_boxed_str());
    *NOMBRE.lock().unwrap() = (ruta, "");
    TOPE_PRESENTES.store(presentes, Ordering::SeqCst);
    *GUARDAR_FOTOS.lock().unwrap() = fotos.to_vec();
    let (salio, dicho, _) = correr_exe(&uno, exe, true, guion);
    TOPE_PRESENTES.store(1000, Ordering::SeqCst);
    GUARDAR_FOTOS.lock().unwrap().clear();
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let vistas = VISTAS.lock().unwrap().clone();
    let f = core::mem::take(&mut *FOTOS.lock().unwrap());
    (salio, String::from_utf8_lossy(&dicho).into_owned(), vistas, f)
}

/// Los tres vertices del triangulo en pixeles (x a la derecha, y hacia abajo),
/// corrido `dx` en coordenadas de recorte, con las cuentas en f32 como el `.exe`.
fn triangulo(dx: f32) -> [(f64, f64); 3] {
    let a = ANCHO as f32 / ALTO as f32;
    let v = [(0.0f32, 0.25 * a), (0.25, -0.25 * a), (-0.25, -0.25 * a)];
    v.map(|(x, y)| (((x + dx + 1.0) * 0.5 * ANCHO as f32) as f64, ((1.0 - y) * 0.5 * ALTO as f32) as f64))
}

/// Las coordenadas baricentricas del centro del pixel `(i, j)` en `t`.
fn baricentricas(t: &[(f64, f64); 3], i: u32, j: u32) -> [f64; 3] {
    let (px, py) = (i as f64 + 0.5, j as f64 + 0.5);
    let [(x0, y0), (x1, y1), (x2, y2)] = *t;
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    let l1 = ((px - x0) * (y2 - y0) - (x2 - x0) * (py - y0)) / area;
    let l2 = ((x1 - x0) * (py - y0) - (px - x0) * (y1 - y0)) / area;
    [1.0 - l1 - l2, l1, l2]
}

/// Lo minimo de las tres, en pixeles: > 0 dentro, < 0 fuera. La distancia del
/// centro del pixel al lado mas cercano.
fn distancia_al_borde(t: &[(f64, f64); 3], i: u32, j: u32) -> f64 {
    let (px, py) = (i as f64 + 0.5, j as f64 + 0.5);
    let mut d = f64::MAX;
    for k in 0..3 {
        let (ax, ay) = t[k];
        let (bx, by) = t[(k + 1) % 3];
        let (cx, cy) = t[(k + 2) % 3];
        let largo = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        let lado = |x: f64, y: f64| ((bx - ax) * (y - ay) - (by - ay) * (x - ax)) / largo;
        // El signo de dentro es el del tercer vertice.
        let s = lado(cx, cy).signum();
        d = d.min(s * lado(px, py));
    }
    d
}

/// **El juez del triangulo de colores** (R3 del plan): lo que D3D12 FIJA, y su
/// margen donde lo deja.
///
/// ```text
///    fuera (a mas de 1 pixel del borde)   EXACTAMENTE el azul de E1.1
///    dentro (a mas de 1 pixel del borde)  rojo, verde y azul interpolados con
///                                         las baricentricas (w = 1: lineal),
///                                         cada canal a 2 o menos de lo exacto
///                                         (el margen: la interpolacion y el
///                                         paso a UNORM de 8 bits)
///    cuantos dentro                       el area, 51200 pixeles, +-1 %
/// ```
fn juzgar_triangulo(px: &[u32], dx: f32) {
    let t = triangulo(dx);
    let (mut dentro, mut mal) = (0u32, Vec::new());
    for j in 0..ALTO {
        for i in 0..ANCHO {
            let p = px[(j * ANCHO + i) as usize] & 0x00FF_FFFF;
            if p != AZUL {
                dentro += 1;
            }
            let d = distancia_al_borde(&t, i, j);
            if d < -1.0 && p != AZUL {
                mal.push(format!("({i},{j}) fuera y no es el azul: {p:06x}"));
            } else if d > 1.0 {
                let l = baricentricas(&t, i, j);
                let esperado = l.map(|x| (x * 255.0).round() as i32);
                let visto = [(p >> 16 & 0xFF) as i32, (p >> 8 & 0xFF) as i32, (p & 0xFF) as i32];
                if (0..3).any(|c| (visto[c] - esperado[c]).abs() > 2) {
                    mal.push(format!("({i},{j}) dentro: {visto:?}, se esperaba {esperado:?}"));
                }
            }
            if mal.len() > 8 {
                panic!("el triangulo no es el de D3D12: {mal:#?}");
            }
        }
    }
    assert!(mal.is_empty(), "el triangulo no es el de D3D12: {mal:#?}");
    assert!((dentro as i64 - 51200).abs() <= 512, "{dentro} pixeles dentro; el area son 51200");
}

/// **E1.2 -- HelloTriangle**: un vertex buffer, una root signature vacia, un
/// PSO con sus dos `.cso` de DXC (SM 6.0), y DrawInstanced(3, 1). Lo que pide
/// de mas sobre E1.1: CreateFile2 (lee los `.cso`) y el dibujo de verdad.
#[test]
fn e1_2_hellotriangle_dibuja_el_triangulo_de_colores_de_d3d12() {
    let (salio, texto, vistas, fotos) = correr_muestra(HTRIANG, "htriang", 30, &[0, 29]);
    assert_eq!(salio, 0xF00D, "presento hasta el tope del banco: {texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert_eq!(vistas.len(), 30);
    assert!(vistas.iter().all(|&v| v == vistas[0]), "la misma imagen en cada Present");
    assert_ne!(vistas[0], huella_lisa(AZUL, ANCHO, ALTO), "no es solo el azul: hay triangulo");
    for (_, px, w, h) in &fotos {
        assert_eq!((*w, *h), (ANCHO, ALTO));
        juzgar_triangulo(px, 0.0);
    }
}

/// **E1.5 -- HelloFrameBuffering**: el MISMO triangulo con dos fotogramas en
/// vuelo (un allocator y una valla por fotograma). El juez: cada Present es,
/// bit a bit, el de HelloTriangle.
#[test]
fn e1_5_helloframebuffering_da_bit_a_bit_el_triangulo_de_hellotriangle() {
    let (_, _, triangulo, _) = correr_muestra(HTRIANG, "htriang", 2, &[]);
    let (salio, texto, vistas, _) = correr_muestra(HFRAMES, "hframes", 30, &[]);
    assert_eq!(salio, 0xF00D, "{texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert_eq!(vistas.len(), 30);
    assert!(vistas.iter().all(|&v| v == triangulo[0]), "cada Present, el de HelloTriangle");
}

/// **E1.6 -- HelloBundles**: el MISMO triangulo, pero grabado una vez en un
/// BUNDLE y ejecutado con ExecuteBundle en cada fotograma (la mitad de N5.17).
/// El juez: cada Present es, bit a bit, el de HelloTriangle.
#[test]
fn e1_6_hellobundles_da_bit_a_bit_el_triangulo_de_hellotriangle() {
    let (_, _, triangulo, _) = correr_muestra(HTRIANG, "htriang", 2, &[]);
    let (salio, texto, vistas, _) = correr_muestra(HBUNDLES, "hbundles", 30, &[]);
    assert_eq!(salio, 0xF00D, "{texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert_eq!(vistas.len(), 30);
    assert!(vistas.iter().all(|&v| v == triangulo[0]), "cada Present, el de HelloTriangle");
}

/// **E1.4 -- HelloConstBuffers**: el triangulo corrido por un cbuffer en un
/// monton UPLOAD con Map PERSISTENTE, que el `.exe` escribe ANTES de cada
/// fotograma: `offset.x += 0.005` (en f32). El juez: el fotograma n es el
/// triangulo de E1.2 corrido (n + 1) * 0.005, con el mismo margen.
#[test]
fn e1_4_helloconstbuffers_corre_el_triangulo_lo_que_dice_su_cbuffer() {
    let (salio, texto, vistas, fotos) = correr_muestra(HCBUFFER, "hcbuffer", 30, &[0, 15, 29]);
    assert_eq!(salio, 0xF00D, "{texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert_eq!(vistas.len(), 30);
    assert!(vistas.windows(2).all(|v| v[0] != v[1]), "cada fotograma, otro sitio");
    for (n, px, _, _) in &fotos {
        let mut dx = 0.0f32;
        for _ in 0..=*n {
            dx += 0.005;
        }
        juzgar_triangulo(px, dx);
    }
}

/// **E1.3 -- HelloTexture**: el triangulo con UV (0.5, 0), (1, 1), (0, 1) y una
/// textura de 256x256 que el `.exe` hace en la CPU: un tablero de 8x8 cuadros
/// de 32 texeles, NEGRO donde la columna y la fila del cuadro tienen la misma
/// paridad, BLANCO donde no. Subida con UpdateSubresources (CopyTextureRegion
/// de un monton UPLOAD), un SRV en un monton SHADER_VISIBLE y un muestreador
/// estatico de PUNTO.
///
/// **El juez, bit a bit:** con PUNTO no se mezcla nada, asi que cada pixel de
/// dentro (a mas de 1 pixel del borde) es EXACTAMENTE 0x000000 o 0xFFFFFF, el
/// del cuadro de su UV -- salvo a menos de 0.05 texeles de una raya del
/// tablero, donde el UV interpolado puede caer de un lado o del otro. Fuera,
/// exactamente el azul.
#[test]
fn e1_3_hellotexture_muestrea_el_tablero_por_punto_bit_a_bit() {
    let (salio, texto, vistas, fotos) = correr_muestra(HTEXTURE, "htexture", 10, &[9]);
    assert_eq!(salio, 0xF00D, "{texto}");
    // Ni un aviso: desde X2 (05-10) el PSO que muestrea va por el codigo
    // nativo (llama al muestreo del interprete); antes decia "un PSO con
    // texturas: sus sombreadores se interpretan".
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert!(vistas.iter().all(|&v| v == vistas[0]), "la misma imagen en cada Present");
    let t = triangulo(0.0);
    let uv = [(0.5f64, 0.0f64), (1.0, 1.0), (0.0, 1.0)];
    let (px, _, _) = (&fotos[0].1, fotos[0].2, fotos[0].3);
    let (mut negros, mut blancos, mut mal) = (0u32, 0u32, Vec::new());
    for j in 0..ALTO {
        for i in 0..ANCHO {
            let p = px[(j * ANCHO + i) as usize] & 0x00FF_FFFF;
            let d = distancia_al_borde(&t, i, j);
            if d < -1.0 && p != AZUL {
                mal.push(format!("({i},{j}) fuera y no es el azul: {p:06x}"));
            } else if d > 1.0 {
                let l = baricentricas(&t, i, j);
                let u = (l[0] * uv[0].0 + l[1] * uv[1].0 + l[2] * uv[2].0) * 256.0;
                let v = (l[0] * uv[0].1 + l[1] * uv[1].1 + l[2] * uv[2].1) * 256.0;
                let raya = |x: f64| ((x / 32.0).round() * 32.0 - x).abs() < 0.05;
                if raya(u) || raya(v) {
                    continue;
                }
                let (ci, cj) = ((u as u32).min(255) / 32, (v as u32).min(255) / 32);
                let esperado = if ci % 2 == cj % 2 { 0x000000 } else { 0xFFFFFF };
                match p {
                    0x000000 => negros += 1,
                    0xFFFFFF => blancos += 1,
                    _ => {}
                }
                if p != esperado {
                    mal.push(format!("({i},{j}) uv ({:.3},{:.3}): {p:06x}, se esperaba {esperado:06x}", u / 256.0, v / 256.0));
                }
            }
            if mal.len() > 8 {
                panic!("el tablero no es el de D3D12: {mal:#?}");
            }
        }
    }
    assert!(mal.is_empty(), "el tablero no es el de D3D12: {mal:#?}");
    assert!(negros > 10_000 && blancos > 10_000, "los dos colores del tablero: {negros} negros, {blancos} blancos");
}

/// El tono (HSL, de 0 a 1) de un pixel `0x00RRGGBB`, si tiene color: ni
/// gris (saturacion < 0.25) ni casi negro (luz < 0.15).
fn tono(p: u32) -> Option<f32> {
    let [r, g, b] = [(p >> 16 & 0xFF) as f32 / 255.0, (p >> 8 & 0xFF) as f32 / 255.0, (p & 0xFF) as f32 / 255.0];
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let luz = (mx + mn) / 2.0;
    let d = mx - mn;
    if d == 0.0 || luz < 0.15 {
        return None;
    }
    let sat = if luz > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    if sat < 0.25 {
        return None;
    }
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    Some(h / 6.0)
}

/// **E2.2 -- DynamicIndexing** (N5.4, el bindless): una ciudad de 15 x 8
/// copias de `occcity.bin`, y cada copia lee SU material con el registro
/// CALCULADO -- `g_txMats[materialConstants.matIndex]`, con `matIndex` en
/// una constante de 32 bits de la raiz, distinto por dibujo -- de 120
/// texturas de 64x64 que recorren el arcoiris (el material `k`, del tono
/// k/120 al (k+1)/120). Con profundidad, tres fotogramas en vuelo y bundles.
///
/// **El juez** (sin la imagen de Windows, que va con muestreo LINEAL y no es
/// bit a bit): lo que solo sale si CADA dibujo lee la textura de SU indice.
///
/// ```text
///    el tono medio de cada franja de 20 filas    SUBE de abajo (las ciudades
///    con color (mas de 1000 pixeles)             de cerca, rojo: k chico)
///                                                arriba (las del fondo,
///                                                violeta), sin bajar
///    el de abajo, menos de 0.1; el de arriba,     el arcoiris entero, de
///    mas de 0.8                                   punta a punta
///    los seis tramos del tono, 1000 pixeles cada  rojo, amarillo, verde, cian,
///    uno o mas                                    azul y magenta
/// ```
///
/// Un indice atascado (siempre el 0, o el de la primera ciudad) da un solo
/// tono, y una franja plana.
#[test]
fn e2_2_dynamicindexing_cada_ciudad_lee_su_material_por_indice_dinamico() {
    let (salio, texto, vistas, fotos) = correr_muestra(DYNINDEX, "dynindex", 2, &[1]);
    assert_eq!(salio, 0xF00D, "{texto}");
    // Ni un aviso: desde X3 (06-10) su `Sample` (la mip de las derivadas
    // del cuadro, de texturas del indice dinamico) va TRADUCIDO, en cuadros
    // de 2x2 (antes: el de texturas, y en X2 "va por el interprete").
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte (antes: `createHandle con un registro CALCULADO`)");
    assert_eq!(vistas[0], vistas[1], "la camara quieta: la misma imagen");
    let px = &fotos[0].1;
    let (w, h) = (fotos[0].2, fotos[0].3);
    let mut franjas = Vec::new();
    let mut tramos = [0u32; 6];
    for y0 in (0..h).step_by(20) {
        let (mut n, mut suma) = (0u32, 0.0f32);
        for y in y0..(y0 + 20).min(h) {
            for x in 0..w {
                let p = px[(y * w + x) as usize] & 0x00FF_FFFF;
                // El cielo es el azul de limpiar: no es de ninguna ciudad.
                if let Some(t) = (p != AZUL).then(|| tono(p)).flatten() {
                    n += 1;
                    suma += t;
                    tramos[((t * 6.0) as usize).min(5)] += 1;
                }
            }
        }
        if n > 1000 {
            franjas.push((y0, suma / n as f32));
        }
    }
    assert!(franjas.len() >= 10, "la ciudad ocupa la mitad de abajo: {franjas:?}");
    for par in franjas.windows(2) {
        assert!(par[0].1 >= par[1].1 - 0.01, "el tono SUBE hacia el fondo (arriba): {franjas:?}");
    }
    assert!(franjas.last().unwrap().1 < 0.1 && franjas[0].1 > 0.8, "de rojo (cerca) a violeta (fondo): {franjas:?}");
    assert!(tramos.iter().all(|&n| n >= 1000), "los seis tramos del arcoiris: {tramos:?}");
}

/// **E2.3a, el COMPUTO** (N5.5, 05-10): `computo.exe` (`prueba/computo.cpp`,
/// de consola) corre su CS de 64 hilos con memoria compartida y una barrera
/// dos veces, en la cola DIRECTA (constantes en la raiz) y en una de COMPUTO
/// que la espera con una valla (CBV en la raiz), y compara los 2 x 1024
/// floats con su cuenta, bit a bit. El juez es el `.exe`: en Windows dice lo
/// mismo. Probado que dice NO: con la barrera quitada del interprete, la
/// mitad de cada grupo lee la compartida antes de que la escriban (128
/// elementos distintos en cada Dispatch, y sale con 2).
#[test]
fn e2_3a_el_computo_con_memoria_compartida_y_barrera_da_los_bits_de_la_cuenta() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, COMPUTO, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.ends_with("computo.exe: el computo de D3D12 es el de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **N5.13, las INSTANCIAS** (05-10): `instancias.exe` (`prueba/instancias.cpp`,
/// de consola) dibuja seis instancias con TRES buferes de vertices (la
/// esquina por vertice; el sitio, el color y la fila por instancia, con
/// StepRate 1 y 2, desde la instancia 2) y dos sin bufer de vertices
/// (SV_VertexID, desde la 5), y compara el destino de 64 x 64 con su
/// cuenta, pixel a pixel. El juez es el `.exe`: en Windows dice lo mismo.
#[test]
fn n5_13_las_instancias_y_las_ranuras_de_vertices_dan_los_pixeles_de_la_cuenta() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, INSTANCIAS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Ni un aviso (la VELOCIDAD, 05-10): SV_InstanceID a float y los
    // desplazamientos de SV_VertexID son cuentas ENTERAS, y el traductor a
    // x86-64 de los de dibujo ya las sabe (antes: "un PSO cuyo sombreador
    // salta o hace cuentas ENTERAS ... se interpretan").
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 3, "{texto}");
    assert!(texto.ends_with("instancias.exe: las instancias de D3D12 son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **N5.3b y N5.3c, las VISTAS** (05-10): `vistas.exe` (`prueba/vistas.cpp`,
/// de consola) corre tres CS: con todo en la RAIZ (un SRV estructurado de
/// 32 bytes, un UAV estructurado y uno crudo), con un RWTexture2D y su
/// GetDimensions, y leyendo esa textura como UAV a un RWBuffer con tipo
/// UNORM; y las dos ClearUnorderedAccessView. Todo comparado bit a bit con
/// su cuenta. El juez es el `.exe`: en Windows dice lo mismo.
#[test]
fn n5_3c_las_vistas_en_la_raiz_y_los_uav_de_textura_dan_los_bits_de_la_cuenta() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, VISTAS_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 7, "{texto}");
    assert!(texto.ends_with("vistas.exe: las vistas de D3D12 son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **N5.16, los render targets de FLOAT** (05-10): `hdr.exe`
/// (`prueba/hdr.cpp`, nuestro, de consola). A: un RGBA16F limpio en float y
/// con dos sumas guarda 3.25 y lo que resta; B: un R11G11B10F sin signo (el
/// -1 es 0); C: A leido como textura a 8 bits. Cada valor cabe exacto en su
/// formato y lo que tiene que salir son bits escritos a mano en el `.cpp`.
#[test]
fn n5_16_los_render_targets_de_float_guardan_lo_que_pasa_de_uno() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, HDR_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Ni un aviso: el cuadro de SV_VertexID (desplazamientos de enteros) se
    // traduce desde la VELOCIDAD (05-10), y PSLee, que LEE el render target
    // como textura, desde X2 (05-10: antes, "un PSO con texturas").
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert!(avisos.is_empty(), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.ends_with("hdr.exe: los render targets de float son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **Los UAV escritos desde un DIBUJO** (05-10): `prueba/uavpixel.exe`
/// (nuestro, `uavpixel.cpp`). A: cada pixel de la mitad izquierda escribe su
/// posicion en SU texel de un `RWTexture2D<uint>` (de una tabla); B: cada
/// pixel de una caja de 32 x 16 suma 1 con `InterlockedAdd` a un
/// `RWByteAddressBuffer` de la RAIZ (512); C: cada vertice escribe 100 + su
/// numero en un `RWBuffer<uint>`. Solo lo que no depende del orden de los
/// pixeles. Antes: lo escrito se perdia (y lo decia un aviso).
#[test]
fn los_uav_de_un_dibujo_quedan_escritos() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, UAVPIXEL_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Un aviso: sus sombreadores escriben UAV, y eso no se traduce a x86
    // (X1 traduce los enteros, no los UAV): se interpretan. Ninguno de UAV
    // perdidos.
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert_eq!(avisos, ["PROTON-X: un PSO cuyo sombreador lee o escribe un UAV: sus sombreadores se interpretan (el codigo nativo aun no lo sabe)"], "un aviso, dicho una vez: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.contains("  bien  B, InterlockedAdd en un RWByteAddressBuffer de la raiz: 512 pixeles cubiertos"), "{texto}");
    assert!(texto.ends_with("uavpixel.exe: los UAV de un dibujo son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **N5.16b -- lo que quedaba de los floats** (05-10, `prueba/flotante1.cpp`,
/// nuestro, de consola). A: un R32F y un R16F con sumas, de mas de 1 y
/// negativos, y el R32F leido como textura; B: un RWTexture2D de RGBA16F
/// limpio con ClearUnorderedAccessViewFloat, escrito por un CS (cuantizado
/// a half) y leido por otro; C: una rampa que cruza el plano cercano y el
/// lejano con DepthClipEnable FALSE (la Z sujeta al viewport) y TRUE. Lo que
/// tiene que salir, bits escritos a mano en el `.cpp`.
#[test]
fn n5_16b_los_floats_de_un_canal_sus_uav_y_sin_recorte_en_z() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, FLOTANTE1_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // X2 (05-10): ni el de texturas (su lectura va por el codigo nativo).
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert!(avisos.is_empty(), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 10, "{texto}");
    assert!(texto.ends_with("flotante1.exe: los floats de un canal, sus UAV y DepthClipEnable son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **El STENCIL** (05-10, la fila de la tabla 7.2 de la ESCALERA): marcar
/// con REPLACE y pintar solo alli con EQUAL; INCR_SAT y las mascaras de
/// lectura y escritura; la cara de delante y la de detras (en un D32S8X24);
/// y dibujos de SOLO profundidad con las tres operaciones (fallo de stencil,
/// de Z y las dos que pasan) en un R24G8 TYPELESS. Leido por el color con
/// sondas EQUAL. Antes el PSO lo apuntaba y no lo usaba: A, B, C y D, MAL.
#[test]
fn el_stencil_recorta_como_en_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, STENCIL_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Ningun aviso: el cuadro viene de un bufer de vertices (sin cuentas
    // enteras) y todo lo que pide el juez la casa lo hace.
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert!(avisos.is_empty(), "ningun aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 5, "{texto}");
    assert!(texto.ends_with("stencil.exe: el stencil de D3D12 es el de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **Lo que QUEDABA de N5.3d, N5.12b y N5.16b** (05-10, `prueba/restos.exe`,
/// nuestro, de consola): A, render targets de ENTEROS (R32_UINT, R8_UINT,
/// RGBA16_SINT, RGBA8_UINT con mascara y RG32_UINT; su limpieza hacia el
/// cero, la saturacion de lo que no cabe y uno leido con Load); B, un dibujo
/// SOLO con UAV (el viewport de 6 x 3: 18 pixeles); C, UAV desde un
/// sombreador de GEOMETRIA; D, el plano de stencil leido con
/// CopyTextureRegion (subrecurso 1) y con un SRV X24_TYPELESS_G8_UINT; E,
/// SV_StencilRef (OPTIONS ya dice que si). Bits escritos a mano en el `.cpp`.
#[test]
fn restos_enteros_solo_uav_gs_plano_de_stencil_y_stencilref() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, RESTOS_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Un aviso, el de verdad: B y C escriben UAV, y eso se interpreta (el
    // codigo nativo no toca UAV). A5 y D2 leen una textura: desde X2, en x86.
    // Ninguno de lo que antes se perdia o se negaba.
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert_eq!(
        avisos,
        ["PROTON-X: un PSO cuyo sombreador lee o escribe un UAV: sus sombreadores se interpretan (el codigo nativo aun no lo sabe)"],
        "{texto}"
    );
    assert_eq!(texto.matches("  bien  ").count(), 18, "{texto}");
    assert!(!texto.contains("  nota  "), "la casa dice que SV_StencilRef si: {texto}");
    assert!(texto.ends_with("restos.exe: los enteros, los UAV sin destino y del GS, el plano de stencil y SV_StencilRef son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **E2.5 -- las OLAS** (05-10, `prueba/olas.exe`, NUESTRO: la muestra
/// D3D12SM6WaveIntrinsics de Microsoft pinta como la GPU junte los pixeles
/// en olas, y eso no tiene una huella que comparar). El computo en olas de
/// 32 hilos seguidos con cada operacion de ola, y los pixeles en cuadros de
/// 2x2 con sus ayudantes, bit a bit; y OPTIONS1 dice los mismos 32
/// carriles. Un aviso y ninguno mas, el de verdad: el PSO de dibujo se
/// interpreta porque su PS usa las olas (su VS, que salta, ya se traduce:
/// X1).
#[test]
fn e2_5_las_olas_de_32_carriles_dan_los_bits_de_la_cuenta() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, OLAS_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert_eq!(avisos, ["PROTON-X: un PSO cuyo sombreador usa las olas (Wave*, Quad*: van de 32 en 32 carriles): sus sombreadores se interpretan (el codigo nativo aun no lo sabe)"], "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 15, "{texto}");
    assert!(texto.ends_with("olas.exe: las olas de D3D12 son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **E2.3b -- D3D12nBodyGravity** (05-10, `Samples/Desktop`, MIT): el
/// COMPUTO de verdad (10.000 particulas, la barrera DENTRO de un bucle, en
/// SU hilo y su cola de computo, con vallas entre las dos colas) y un
/// sombreador de GEOMETRIA que hace de cada punto un cuadro con un degradado
/// redondo. Pidio a la casa: el GS, puntos (POINTLIST), OPTIONS12 contestado
/// (EnhancedBarriers NO: va por ResourceBarrier), `rand`/`srand`, y que una
/// espera cumplida ceda el turno (su hilo de computo no soltaria nunca).
/// Corre su CS TRADUCIDO a x86-64 (`nativo_computo`, 50 veces el
/// interprete; su juez, `tests/nativo_computo.rs`, bit a bit y contra la
/// fisica en f64).
///
/// **Como se sabe** (R3: el CS suma floats en otro orden que la 3060, asi
/// que los bits no; lo que la fisica fija, si): tres Present distintos (la
/// simulacion avanza); en cada uno las DOS nubes, iguales a izquierda y
/// derecha (las dos mitades salen del mismo `srand(0)`) y centradas; y el
/// color: en el 0 ROJO (sin aceleracion aun: `velo.w` es 1e-8) y en el 2
/// tirando a AMARILLO (`velo.w` = |aceleracion| despues de un paso). Dos
/// avisos y ninguno mas: el VS lee un bufer (por el interprete) y algun
/// cuadro cruza el plano cercano o el lejano (sin recortar todavia, N5.15).
#[test]
fn e2_3b_nbodygravity_simula_en_su_hilo_y_dibuja_con_su_gs() {
    let (salio, texto, vistas, fotos) = correr_muestra(NBODY, "nbody", 3, &[0, 2]);
    assert_eq!(salio, 0xF00D, "presento hasta el tope del banco: {texto}");
    for l in texto.lines() {
        assert!(
            l == "PROTON-X: Draw: triangulos que cruzan el plano cercano o salen de la profundidad: sin recortar todavia, no se pintan",
            "un aviso que no se espera: {l}\n{texto}"
        );
    }
    assert_eq!(vistas.len(), 3);
    assert!(vistas[0] != vistas[1] && vistas[1] != vistas[2], "cada Present, la simulacion un paso mas: {vistas:?}");
    let mut verde_por_rojo = Vec::new();
    for (n, px, w, h) in &fotos {
        assert_eq!((*w, *h), (ANCHO, ALTO));
        let fondo = px[0] & 0xFF_FFFF;
        assert_eq!(fondo, 0x00_00_1A, "foto {n}: el fondo de la muestra, {{0, 0, 0.1}}");
        let (mut izq, mut der, mut sx, mut sy, mut r, mut g) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        for y in 0..*h {
            for x in 0..*w {
                let p = px[(y * w + x) as usize] & 0xFF_FFFF;
                if p == fondo {
                    continue;
                }
                if x < w / 2 {
                    izq += 1;
                } else {
                    der += 1;
                }
                (sx, sy) = (sx + x as u64, sy + y as u64);
                (r, g) = (r + (p >> 16 & 0xFF) as u64, g + (p >> 8 & 0xFF) as u64);
            }
        }
        let n_px = izq + der;
        assert!(n_px > 200_000, "foto {n}: las nubes, {n_px} pixeles");
        let simetria = izq as f64 / der as f64;
        assert!((0.9..1.1).contains(&simetria), "foto {n}: izquierda {izq} y derecha {der}");
        let (cx, cy) = (sx as f64 / n_px as f64, sy as f64 / n_px as f64);
        assert!((cx - 640.0).abs() < 20.0 && (cy - 360.0).abs() < 20.0, "foto {n}: el centro ({cx:.1}, {cy:.1})");
        verde_por_rojo.push(g as f64 / r as f64);
    }
    assert!(verde_por_rojo[0] < 0.3, "el Present 0, rojo: G/R {:.3}", verde_por_rojo[0]);
    assert!(verde_por_rojo[1] > 0.4, "el Present 2, amarillo (ya aceleran): G/R {:.3}", verde_por_rojo[1]);
}

/// El ESPACIO, pulsado y suelto (su scancode, 0x39: WM_KEYDOWN con VK_SPACE).
const ESPACIO: [u64; 2] = [1 << 8 | 1 << 9 | 0x39, 1 << 8 | 0x39];

/// **E2.4 -- D3D12ExecuteIndirect** (05-10, `Samples/Desktop`, MIT): 1024
/// triangulos, cada uno su propia orden INDIRECTA (la direccion de su CBV y
/// los argumentos de su Draw), y un CS que las CULLEA: `Append` en un UAV con
/// CONTADOR de las que caen en la franja central, y `ExecuteIndirect` con
/// ese contador como cuenta. Pidio a la casa: el contador de un UAV (la op
/// `Contador`, su numero en la ranura del descriptor), `ExecuteIndirect`
/// (corrido al ejecutar la lista: sus argumentos los escribe el computo de
/// antes) y `CreateCommandSignature` leida. Los triangulos empiezan a la
/// izquierda y entran a la franja poco a poco: la cuenta crece.
///
/// **Como se sabe** (lo que la propia muestra promete, "su huella, igual,
/// con el culling encendido y apagado"): dos corridas de 60 Present, una
/// con culling y otra con el ESPACIO pulsado al empezar (sin culling: las
/// 1024 ordenes, sin cuenta). DENTRO de la tijera del culling (x de 320 a
/// 960) las dos dan los MISMOS pixeles, bit a bit; FUERA, con culling, solo
/// el fondo, y sin el, triangulos. Probado que dice NO: con el contador
/// atascado, la franja sale vacia.
#[test]
fn e2_4_executeindirect_cullea_por_computo_y_dibuja_lo_mismo_dentro() {
    let (fondo, w) = (0x00_33_66u32, ANCHO);
    let (salio, texto, _, con) = correr_muestra_con(INDIRECT, "indirect", 60, &[30, 59], &[]);
    assert_eq!(salio, 0xF00D, "{texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    let (salio, texto, _, sin) = correr_muestra_con(INDIRECT, "indirect", 60, &[30, 59], &ESPACIO);
    assert_eq!((salio, texto.as_str()), (0xF00D, ""));
    for ((n, a, _, _), (_, b, _, _)) in con.iter().zip(&sin) {
        let (mut dentro_color, mut fuera_sin) = (0, 0);
        for (i, (&pa, &pb)) in a.iter().zip(b).enumerate() {
            let (x, pa, pb) = (i as u32 % w, pa & 0xFF_FFFF, pb & 0xFF_FFFF);
            if (320..960).contains(&x) {
                assert_eq!(pa, pb, "Present {n}, pixel ({x}, {}): con culling y sin el, distintos dentro de la franja", i as u32 / w);
                dentro_color += (pa != fondo) as u32;
            } else {
                assert_eq!(pa, fondo, "Present {n}, pixel ({x}, {}): con culling, fuera de la franja solo el fondo", i as u32 / w);
                fuera_sin += (pb != fondo) as u32;
            }
        }
        assert!(fuera_sin > 10_000, "Present {n}: sin culling hay triangulos fuera de la franja ({fuera_sin})");
        if *n == 59 {
            assert!(dentro_color > 10_000, "Present 59: ya han entrado triangulos a la franja ({dentro_color} pixeles)");
        }
    }
}

/// **E2.7 -- D3D12PredicationQueries** (05-10, `Samples/Desktop`, MIT): un
/// cuadro blanco LEJOS, uno translucido CERCA que pasa por delante, y la
/// caja del lejano dibujada en una consulta de OCLUSION BINARIA (sin color
/// ni Z); su resultado, con ResolveQueryData, decide con SetPredication
/// (EQUAL_ZERO) si el lejano se dibuja en el fotograma SIGUIENTE. Pidio a la
/// casa contar los pixeles que pasan la profundidad (`Cuenta::pasan`) entre
/// BeginQuery y EndQuery, y SetPredication de verdad (`consultas.rs`).
///
/// **Como se sabe** (la regla de la muestra, fotograma a fotograma): de
/// cada Present se MIDE donde esta el cuadro cercano (la fila 170, donde
/// solo esta el) y si tapaba entero al lejano (columnas 480 a 799); el
/// lejano se dibuja en el Present `n` si y solo si NO lo tapaba en el
/// `n - 1` (en el 0, nunca: el bufer de la consulta empieza a cero). Y los
/// colores, exactos: el cercano (alfa 0.65) sobre el blanco, o sobre el
/// fondo. Probado que dice NO: con SetPredication sin hacer, el lejano sale
/// desde el Present 0; con la consulta siempre VISIBLE (lo de antes), tambien.
#[test]
fn e2_7_predicationqueries_salta_el_cuadro_que_la_oclusion_dice_tapado() {
    let fotos: Vec<u32> = (0..=34).collect();
    let (salio, texto, _, fotos) = correr_muestra_con(PREDICA, "predica", 35, &fotos, &[]);
    assert_eq!(salio, 0xF00D, "{texto}");
    assert_eq!(texto, "", "ni un aviso ni un hueco que falte");
    assert_eq!(fotos.len(), 35);
    let w = ANCHO as usize;
    let fila = |px: &[u32], y: usize| px[y * w..(y + 1) * w].iter().map(|p| p & 0xFF_FFFF).collect::<Vec<u32>>();
    let (mut tapaba, mut vistos) = (None::<bool>, [0u32; 2]);
    for (n, px, _, _) in &fotos {
        // El cercano: sus columnas en la fila 170 (no es ni el fondo ni nada
        // del lejano, que empieza en la 200).
        let arriba = fila(px, 170);
        let cols: Vec<usize> = (0..w).filter(|&x| arriba[x] != 0x00_33_66).collect();
        let (izq, der) = (*cols.first().expect("el cercano siempre se ve"), *cols.last().unwrap());
        // El lejano en la fila del centro: dibujado, todo es blanco o el
        // cercano sobre blanco (rojo a tope: 0.65 + 0.35); si no, el fondo o
        // el cercano sobre el fondo (rojo 0.65 = 0xA6).
        let centro = fila(px, 360);
        let rojos: Vec<u32> = (480..800).map(|x| centro[x] >> 16).collect();
        let dibujado = rojos.iter().all(|&r| r == 0xFF);
        assert!(dibujado || rojos.iter().all(|&r| r == 0x00 || r == 0xA6), "Present {n}: el lejano, ni entero ni ausente: {rojos:?}");
        for x in 480..800 {
            let p = centro[x];
            let bajo_el_cercano = (izq..=der).contains(&x);
            let azul = p & 0xFF;
            let esperado = match (dibujado, bajo_el_cercano) {
                (true, true) => 0x59,  // 0.35 * 1 (el blanco)
                (true, false) => 0xFF, // el blanco solo
                (false, true) => 0x24, // 0.35 * 0.4 (el fondo)
                (false, false) => 0x66,
            };
            assert_eq!(azul, esperado, "Present {n}, pixel ({x}, 360): {p:#08x}");
        }
        match tapaba {
            None => assert!(!dibujado, "Present 0: el bufer de la consulta empieza a cero, y EQUAL_ZERO lo salta"),
            Some(t) => assert_eq!(dibujado, !t, "Present {n}: el lejano se dibuja si y solo si el cercano NO lo tapaba en el anterior"),
        }
        vistos[dibujado as usize] += 1;
        tapaba = Some(izq <= 480 && der >= 799);
    }
    assert!(vistos[0] > 5 && vistos[1] > 5, "se ven las dos cosas: saltado y dibujado ({vistos:?})");
}

/// **D4.4 -- las DERIVADAS y la MIP de un muestreo** (05-10,
/// `prueba/derivadas.exe`, NUESTRO). Las finas y las gruesas de un cuadro de
/// 2x2, la mip de `Sample` por sus derivadas (lambda 0 a 6, el ultimo con
/// tres ayudantes), la mezcla de dos mips (MIP_LINEAR), `SampleBias`,
/// `SampleLevel`, `SampleGrad`, `CalculateLevelOfDetail` (y `Unclamped`), y
/// lo que sujeta la mip: MostDetailedMip, ResourceMinLODClamp y el MaxLOD de
/// un muestreador de un monton. Bit a bit. Ni un aviso: desde X3 los dos PSO
/// van traducidos, en cuadros de 2x2.
#[test]
fn d4_4_las_derivadas_y_la_mip_de_un_muestreo_son_las_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, DERIVADAS_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Ni un aviso: desde X3 (06-10) los dos PSO (el que deriva y el que
    // calcula el LOD) van TRADUCIDOS, en cuadros de 2x2.
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert!(avisos.is_empty(), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 17, "{texto}");
    assert!(texto.ends_with("derivadas.exe: las derivadas y la mip de un muestreo son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **E2.1 -- listas grabadas desde VARIOS HILOS, y las colas con vallas**
/// (05-10, `prueba/multihilo.exe`, NUESTRO: la muestra D3D12Multithreading
/// pide SquidRoom.bin). A: cuatro hilos graban a la vez, a turnos, en el
/// mismo render target y en un pase de solo Z; en una llamada y en seis. B:
/// colas de computo y de copia que ESPERAN en la GPU a un valor que se da
/// despues, una cola que espera a la CPU con su lista reiniciada entretanto,
/// SetEventOnCompletion sin evento y SetEventOnMultipleFenceCompletion. C:
/// las reglas de Reset y Close de listas y allocators.
#[test]
fn e2_1_listas_de_varios_hilos_y_colas_que_esperan() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, MULTIHILO_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    // Los cuatro errores que C hace A PROPOSITO, dichos (y ninguno mas: el
    // cuadro de SV_VertexID y el CS de enteros se traducen a x86).
    let avisos: Vec<&str> = texto.lines().filter(|l| l.starts_with("PROTON-X:")).collect();
    assert_eq!(
        avisos,
        [
            "PROTON-X: una lista con un allocator con el que ya graba otra: en Windows es E_INVALIDARG",
            "PROTON-X: Reset de un allocator con una lista grabando con el: en Windows es E_FAIL",
            "PROTON-X: Reset de una lista que no se cerro: en Windows es E_FAIL",
            "PROTON-X: Close de una lista ya cerrada: en Windows es E_FAIL",
        ],
        "{texto}"
    );
    assert_eq!(texto.matches("  bien  ").count(), 17, "{texto}");
    assert!(texto.ends_with("multihilo.exe: las listas de varios hilos y las colas con vallas son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **Los UAV de texturas 3D y de ARRAYS** (06-10, `prueba/volumen.exe`,
/// NUESTRO): un 3D de 8 x 8 x 4 escrito entero por computo; una vista de
/// sus rebanadas 1 y 2 (lo de fuera no se escribe); un array de 3 capas con
/// 2 mips, por la vista de la mip 1 de las capas 1 y 2 (la capa 0 y la mip
/// 0, intactas); InterlockedAdd en un 3D; GetDimensions y lecturas, fuera
/// de la vista 0; y ClearUnorderedAccessViewUint por la vista de dos
/// rebanadas (o capas): todas ellas, y ninguna mas. Bit a bit. Ni un aviso: hasta el 06-10 el PSO de computo
/// no se creaba y sus Dispatch se perdian.
#[test]
fn los_uav_de_texturas_3d_y_de_arrays_son_los_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, VOLUMEN_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.lines().any(|l| l.starts_with("PROTON-X:")), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 10, "{texto}");
    assert!(texto.ends_with("volumen.exe: los UAV de texturas 3D y de arrays son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **Las ROOT SIGNATURES 1.1 y las de DENTRO del sombreador** (06-10,
/// `prueba/firmas.exe`, NUESTRO): la 1.1 que hace `dxc` (con banderas), la
/// 1.0, la del sombreador pasada a CreateRootSignature, un PSO de computo
/// SIN root signature (la de su sombreador), una DESC1 serializada, y
/// CheckFeatureSupport diciendo 1.1. Cada una corre el mismo CS y sus
/// constantes, su tabla y su UAV de la raiz caen en su sitio. Ni un aviso.
#[test]
fn las_root_signatures_1_1_y_las_del_sombreador_son_las_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, FIRMAS_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.lines().any(|l| l.starts_with("PROTON-X:")), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 6, "{texto}");
    assert!(texto.ends_with("firmas.exe: las root signatures 1.1 y las de dentro del sombreador son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **El COMPUTO de un posproceso** (06-10, `prueba/postpro.exe`, NUESTRO):
/// cada hilo elige SU textura de un array sin limite por un indice
/// calculado (el bindless), le suma lo que muestrea de la escena, lo
/// escribe en un RWTexture2D (creado SIN descripcion: la vista del recurso
/// entero) y cuenta con InterlockedAdd; y GetDimensions. Bit a bit. Ni un
/// aviso: corre TRADUCIDO, y la casa de antes lo leia nulo.
#[test]
fn el_computo_de_un_posproceso_es_el_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, POSTPRO_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.lines().any(|l| l.starts_with("PROTON-X:")), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 3, "{texto}");
    assert!(texto.ends_with("postpro.exe: el computo de un posproceso es el de Windows\r\n[salio 0x0]"), "{texto}");
}
