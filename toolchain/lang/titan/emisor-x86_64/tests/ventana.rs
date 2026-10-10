//! ** F1 DE `docs/plan/EL_FOCO.md` (TA2 de `PLAN_LA_TINTA`), EN EL
//! ANFITRION: un programa de TITAN++ abre una VENTANA de BMO-X -- la
//! superficie BSUP de `bmo_abi::syscalls::surface::superficie`, la misma que
//! escriben `roja.inti` y `roja.h` -- y pinta en ella.
//!
//! El emulador de la casa hace de kernel: contesta quien nos lanzo (`padre`,
//! el escritorio de mentira) y apunta lo que se le OFRECE (`ofertas`). Lo que
//! se mira es lo que el DIRECTOR veria: la cabecera, los pixeles y el buzon,
//! palabra a palabra, de la memoria del programa.

use bmo_abi::syscalls::surface::{SUP_BGRA32, SUP_BUZON_CABECERA, SUP_BUZON_RANURA, SUP_CABECERA, SUP_CAMPO_SECUENCIA, SUP_MAGIC};
use bmo_lower::emu::{cargar_bex, run, Machine};

fn bex(toml: &str, main: &str) -> Vec<u8> {
    bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

fn correr(bex: &[u8], padre: u64, pasos: usize) -> Machine {
    let mut m = cargar_bex(bex).expect("el .bex carga");
    m.padre = padre;
    run(m, pasos)
}

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

/// Lo que el escritorio recibio: la superficie, su base y sus bytes.
fn la_superficie(m: &Machine) -> (u64, u64) {
    assert_eq!(m.ofertas.len(), 1, "una oferta, al padre");
    let (base, desde, bytes, destino) = m.ofertas[0];
    assert_eq!((desde, destino), (0, m.padre));
    (base, bytes)
}

fn u32_en(m: &Machine, a: u64) -> u32 {
    m.read_u64(a) as u32
}

const AZUL: u32 = 0xFF00_00FF;
const VERDE: u32 = 0xFF00_FF00;
const ROJO: u32 = 0xFFFF_0000;
const BLANCO: u32 = 0xFFFF_FFFF;

const DIBUJO: &str = "mod main \"una ventana de 8 x 4\"
use director

fn main()
    if not director.ventana(8, 4)
        print(\"nadie compone\")
        return
    director.rect(0, 0, 8, 4, 255)
    director.rect(6, 2, 10, 10, 65280)
    director.rect(-3, -3, 4, 4, 255 * 65536)
    director.rect(2, 1, 0, 3, 16777215)
    director.pixel(7, 0, 16777215)
    director.pixel(-1, 0, 0)
    director.pixel(8, 0, 0)
    director.pixel(0, 4, 0)
    let mut fila: [int; 10] = [0; 10]
    for x in range(10)
        fila[x] = x * 65536 + 16777216
    director.fila(3, fila)
    director.fila(4, fila)
    director.fila(-1, fila)
    director.presenta()
    print(\"otra ventana: \", director.ventana(2, 2))
";

/// ** Sin nadie que componga, `director.ventana` dice que no: el programa lo
/// sabe, no ofrece nada, y pintar sin ventana no rompe nada.
#[test]
fn from_the_shell_there_is_no_window() {
    let m = correr(&bex(SCREEN, DIBUJO), 0, 400_000);
    assert!(m.exited);
    assert_eq!(m.console, "nadie compone\n");
    assert!(m.ofertas.is_empty());
}

/// ** Lanzado por el escritorio: la superficie se ofrece ENTERA -- la
/// cabecera BSUP con sus ocho campos, el buzon a cero y sus 64 ranuras --; y
/// cada pixel es el que se pinto, recortado a la ventana; la secuencia, en 1
/// (una vez `presenta`); y una segunda ventana, no.
#[test]
fn launched_by_the_desktop_it_paints_its_window() {
    let m = correr(&bex(SCREEN, DIBUJO), 7, 2_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "otra ventana: false\n");
    let (s, bytes) = la_superficie(&m);
    let (an, al) = (8u64, 4u64);
    let buzon = SUP_CABECERA + an * al * 4;
    assert_eq!(bytes, buzon + SUP_BUZON_CABECERA + 64 * SUP_BUZON_RANURA);
    let campo = |i: u64| u32_en(&m, s + 4 * i) as u64;
    assert_eq!([campo(0), campo(1), campo(2), campo(3), campo(4), campo(6), campo(7)], [SUP_MAGIC, an, al, an, SUP_BGRA32, buzon, 64]);
    assert_eq!(campo(SUP_CAMPO_SECUENCIA), 1, "presentada una vez");
    assert_eq!((m.read_u64(s + buzon), m.read_u64(s + buzon + 8)), (0, 0), "el buzon: cabeza, cola y estado a cero");
    // La cola privada: el asa y lo ofrecido, DETRAS de lo que ve el escritorio.
    assert_eq!(m.read_u64(s + bytes + 8), bytes);
    let pixel = |x: u64, y: u64| u32_en(&m, s + SUP_CABECERA + 4 * (y * an + x));
    for y in 0..al {
        for x in 0..an {
            let quiere = if y == 3 {
                0xFF00_0000 | (x as u32) << 16
            } else if x == 0 && y == 0 {
                ROJO
            } else if x == 7 && y == 0 {
                BLANCO
            } else if x >= 6 && y >= 2 {
                VERDE
            } else {
                AZUL
            };
            assert_eq!(pixel(x, y), quiere, "el pixel ({}, {})", x, y);
        }
    }
}

/// ** Lo que el DIRECTOR acepta de una superficie: un ancho de 0, o uno de
/// mas, es un NO al correr (sin oferta); y la ventana mas grande, si.
#[test]
fn the_sizes_it_refuses() {
    for (an, al, va) in [(0, 10, false), (10, 0, false), (4097, 1, false), (-5, 5, false), (4096, 1, true)] {
        let src = format!("mod main \"medidas\"\nuse director\n\nfn main()\n    print(director.ventana({}, {}))\n", an, al);
        let m = correr(&bex(SCREEN, &src), 7, 20_000_000);
        assert!(m.exited, "{}", m.console);
        assert_eq!(m.console, format!("{}\n", va), "{} x {}", an, al);
        assert_eq!(m.ofertas.len(), va as usize);
    }
}

/// ** Los NO del compilador, cada uno en su sitio.
#[test]
fn what_the_compiler_says() {
    let no = |main: &str| -> String {
        let e = bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).expect_err("no compila");
        format!("{:?}", e)
    };
    let cabeza = "mod main \"x\"\nuse director\n\nfn main()\n";
    // Un pixel no da nada.
    assert!(no(&format!("{}    let p = director.pixel(1, 2, 3)\n", cabeza)).contains("no devuelve nada"));
    // La fila es de int, no de f32.
    assert!(no(&format!("{}    let t: [f32; 4] = [0.0; 4]\n    director.fila(0, t)\n", cabeza)).contains("tabla de int"));
    // Las medidas, int.
    assert!(no(&format!("{}    print(director.ventana(1.5, 2))\n", cabeza)).contains("ANCHO"));
    // Cuantos valores.
    assert!(no(&format!("{}    director.rect(1, 2, 3)\n", cabeza)).contains("pide 5 valores"));
    // Un nombre que no existe, con su sugerencia.
    assert!(no(&format!("{}    director.presente()\n", cabeza)).contains("presenta"));
}

/// ** EL CERTIFICADO: la ventana abre la puerta de la PANTALLA en la linea
/// de su primera llamada.
#[test]
fn the_certificate_names_the_screen() {
    use bmo_titan_contrato::certificate::{Certificate, Door};
    let bex = bex(SCREEN, DIBUJO);
    let cert = Certificate::read(bmo_verify::declaracion::manifiesto(&bex).expect("el manifiesto")).expect("su certificado");
    assert_eq!(cert.line_of(Door::Screen), Some(5));
    assert_eq!(cert.line_of(Door::Gpu), None);
}

/// ** EL EJEMPLO DEL BANCO (`nivel11/ventana`), lanzado por el escritorio:
/// el degradado, el fondo y la barra donde la dejo el ultimo fotograma; y
/// 601 presentaciones (la primera y una por fotograma).
#[test]
fn the_bank_window_paints_its_bar() {
    let pkg = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join("nivel11").join("ventana");
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("el ejemplo");
    let bex = bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e));
    let m = correr(&bex, 7, 200_000_000);
    assert!(m.exited, "{}", m.console);
    let (s, _) = la_superficie(&m);
    let pixel = |x: u64, y: u64| u32_en(&m, s + SUP_CABECERA + 4 * (y * 320 + x));
    assert_eq!(m.console, "");
    assert_eq!(u32_en(&m, s + 4 * SUP_CAMPO_SECUENCIA), 601);
    assert_eq!((pixel(0, 0), pixel(319, 39)), (0xFFFF_0000, 0xFFFF_FC00), "el degradado: el verde de 0 a 252");
    let fondo = 0xFF00_0000 | 20 << 16 | 30 << 8 | 60;
    assert_eq!((pixel(5, 150), pixel(0, 40)), (fondo, fondo));
    // El ultimo fotograma (599): la barra en x = 1198 % 300 = 298.
    assert_eq!((pixel(298, 100), pixel(317, 139)), (BLANCO, BLANCO));
    assert_eq!((pixel(297, 100), pixel(296, 120), pixel(298, 140)), (fondo, fondo, fondo), "la de antes, borrada");
}
