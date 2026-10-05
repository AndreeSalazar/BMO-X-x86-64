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
    let (salio, dicho, _) = correr_exe(&uno, exe, true, &[]);
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
    // El UNICO aviso, y es de velocidad, no de lo que se ve: un PSO que
    // muestrea va por el interprete, no por el codigo nativo (P3b3b).
    assert_eq!(
        texto,
        "PROTON-X: un PSO con texturas: sus sombreadores se interpretan (el codigo nativo aun no muestrea)\n",
        "ni otro aviso ni un hueco que falte"
    );
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
    assert_eq!(
        texto,
        "PROTON-X: un PSO con texturas: sus sombreadores se interpretan (el codigo nativo aun no muestrea)\n",
        "ni otro aviso ni un hueco que falte (antes: `createHandle con un registro CALCULADO`)"
    );
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
