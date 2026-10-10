//! ** LB7b DE `docs/plan/PLAN_LAS_LIBRERIAS.md`, EN EL ANFITRION: un programa
//! de TITAN++ que habla con el DIRECTOR (`use director`, y `screen` en su
//! Titan.toml) publica fotogramas en la LAMINA de VERRANO, y el LECTOR de
//! VERRANO (`bmo_verrano::lamina`) los lee de esa misma memoria.
//!
//! El emulador de la casa hace de kernel: contesta quien nos lanzo (`padre`,
//! el director de mentira) y apunta lo que se le OFRECE (`ofertas`). Sin padre
//! -- lanzado desde el shell -- la lamina no se ofrece a nadie y el programa
//! lo sabe.

use bmo_lower::emu::{cargar_bex, run, run_acotado, Machine};
use bmo_verrano::lamina::{self, Lamina, Leido};
use bmo_verrano::Vertex;
use core::sync::atomic::{AtomicU32, Ordering};
use std::path::Path;

/// Un paquete escrito aqui: su Titan.toml y su `src/main.titan`.
fn bex(toml: &str, main: &str) -> Vec<u8> {
    bmo_titan_x86_64::build_package("src/main.titan", main, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

/// El paquete de un ejemplo del banco.
fn bex_del_banco(nivel: &str, nombre: &str) -> Vec<u8> {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join(nivel).join(nombre);
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("el ejemplo");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// Correrlo, lanzado por `padre` (0: desde el shell).
fn correr(bex: &[u8], padre: u64, pasos: usize) -> Machine {
    let mut m = cargar_bex(bex).expect("el .bex carga");
    m.padre = padre;
    run(m, pasos)
}

/// Lo que el director de mentira recibio: la lamina, palabra a palabra, de la
/// memoria del programa -- lo que el escritorio veria --.
fn la_lamina(m: &Machine) -> Vec<AtomicU32> {
    let l: Vec<_> = m.ofertas.iter().filter(|o| es_lamina(m, o)).collect();
    assert_eq!(l.len(), 1, "UNA lamina ofrecida, al padre: {:?}", m.ofertas);
    let &(base, desde, bytes, destino) = l[0];
    assert_eq!((desde, destino), (0, m.padre));
    (0..bytes / 4).map(|k| AtomicU32::new(m.read_u64(base + 4 * k) as u32)).collect()
}

/// Una oferta es la lamina si empieza por su magia (Q0a4: el cubo ofrece
/// tambien su VENTANA, una superficie BSUP).
fn es_lamina(m: &Machine, &(base, desde, _, _): &(u64, u64, u64, u64)) -> bool {
    m.read_u64(base + desde) as u32 == lamina::MAGIA
}

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

const TRIANGULO: &str = "mod main \"un triangulo en la lamina\"
use director

fn main()
    if not director.lamina(6)
        print(\"nadie compone\")
        return
    let p: [f32; 12] = [0.0, 0.5, 0.0, 1.0, 0.5, -0.5, 0.0, 1.0, -0.5, -0.5, 0.0, 1.0]
    let c: [f32; 12] = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0]
    for f in range(3)
        if director.publica(f, 3, p, c)
            print(\"publicado \", f)
        director.espera(16)
    print(\"y con uno de mas: \", director.publica(3, 4, p, c))
";

/// ** Sin nadie que componga, `director.lamina` dice que no: el programa lo
/// sabe y no ofrece nada.
#[test]
fn from_the_shell_nobody_composes() {
    let m = correr(&bex(SCREEN, TRIANGULO), 0, 200_000);
    assert!(m.exited);
    assert_eq!(m.console, "nadie compone\n");
    assert!(m.ofertas.is_empty());
}

/// ** Lanzado por el escritorio, la lamina se ofrece entera -- su cabecera
/// escrita ANTES --, cada fotograma se publica con su sello y su secuencia, y
/// el lector de VERRANO lee el ultimo, ENTERO, de esa misma memoria.
#[test]
fn launched_by_the_desktop_it_publishes_and_verrano_reads_it() {
    let m = correr(&bex(SCREEN, TRIANGULO), 7, 400_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.console, "publicado 0\npublicado 1\npublicado 2\ny con uno de mas: false\n");
    let w = la_lamina(&m);
    assert_eq!(w.len() * 4, lamina::bytes_para(6));
    let l = Lamina::abrir(&w).expect("la lamina que escribio TITAN++ se abre");
    assert_eq!(l.capacidad(), 6);
    let mut out = [Vertex::default(); 6];
    assert_eq!(l.leer(&mut out), Leido::Fotograma { fotograma: 2, vertices: 3 });
    let quiere = [([0.0, 0.5, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]), ([0.5, -0.5, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]), ([-0.5, -0.5, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0])];
    for (k, (p, c)) in quiere.iter().enumerate() {
        assert_eq!(out[k].position, *p, "vertice {}", k);
        assert_eq!(out[k].color, *c, "vertice {}", k);
    }
    // Tres publicaciones: la secuencia en 3, los dos sellos PARES (nadie
    // quedo escribiendo) y el de la ranura 1 -- la de los fotogramas 0 y 2 --
    // en 4.
    assert_eq!(w[lamina::CAMPO_SECUENCIA].load(Ordering::Relaxed), 3);
    assert_eq!(w[lamina::CAMPO_SELLO].load(Ordering::Relaxed), 2);
    assert_eq!(w[lamina::CAMPO_SELLO + 1].load(Ordering::Relaxed), 4);
}

/// ** EL CERTIFICADO lo dice (DL7): hablar con el director abre la puerta de
/// la PANTALLA, en la linea de su primera llamada; el juez de la puerta de
/// carga lo lee del .bex y, si `screen` no se concede, dice esa linea.
#[test]
fn the_certificate_names_the_screen_and_its_line() {
    use bmo_titan_contrato::certificate::{judge, Certificate, Door, Verdict};
    use bmo_titan_contrato::{Permission, Permissions};
    let bex = bex(SCREEN, TRIANGULO);
    let cert = Certificate::read(bmo_verify::declaracion::manifiesto(&bex).expect("el manifiesto")).expect("su certificado");
    assert_eq!(cert.line_of(Door::Screen), Some(5));
    assert_eq!(cert.line_of(Door::Gpu), None);
    let asked = Permissions::NONE.with(Permission::Screen);
    assert_eq!(judge(&cert, asked, asked), Verdict::Agrees);
    assert!(matches!(judge(&cert, asked, Permissions::NONE), Verdict::Ungranted(u) if u.door == Door::Screen && u.line == 5));
}

/// Lo que dibuja VERRANO del fotograma `f` de bmo_cubo: por cada cara de la
/// tanda que mira, sus tres vertices en recorte con el color de la cara.
fn la_tanda(f: u32) -> Vec<Vertex> {
    bmo_cubo::tanda::de_fotograma(f, 1280, 720).expect("la tanda cabe").tris().iter().flat_map(|t| t.clip.iter().map(move |&p| Vertex { position: p, color: t.color })).collect()
}

/// Los vertices de la ranura `k`, como estan (la ranura que el lector no
/// mira guarda el fotograma de antes).
fn ranura(w: &[AtomicU32], k: usize) -> (u32, Vec<Vertex>) {
    let cap = w[lamina::CAMPO_CAPACIDAD].load(Ordering::Relaxed) as usize;
    let n = w[lamina::CAMPO_VERTICES + k].load(Ordering::Relaxed) as usize;
    let f = w[lamina::CAMPO_FOTOGRAMA + k].load(Ordering::Relaxed);
    let base = lamina::CABECERA / 4 + k * cap * 8;
    let b = |i: usize, j: usize| f32::from_bits(w[base + 8 * i + j].load(Ordering::Relaxed));
    (f, (0..n).map(|i| Vertex { position: [b(i, 0), b(i, 1), b(i, 2), b(i, 3)], color: [b(i, 4), b(i, 5), b(i, 6), b(i, 7)] }).collect())
}

fn igual(f: u32, da: &[Vertex], quiere: &[Vertex]) {
    assert_eq!(da.len(), quiere.len(), "fotograma {}: cuantos vertices", f);
    for (k, (a, b)) in da.iter().zip(quiere).enumerate() {
        assert_eq!(a.position.map(f32::to_bits), b.position.map(f32::to_bits), "fotograma {}, vertice {}: la posicion", f, k);
        assert_eq!(a.color.map(f32::to_bits), b.color.map(f32::to_bits), "fotograma {}, vertice {}: el color", f, k);
    }
}

/// ** EL CUBO QUE GIRA, EN LA LAMINA (LB7b): `nivel11/cubo_gira` lanzado por
/// el escritorio cuenta cada fotograma con sus gpu fn -- al correr, en la CPU,
/// su reserva (LB4) -- y lo publica. El escritorio de mentira lee la lamina
/// MIENTRAS la app corre, cada `LEE_CADA` pasos, con el lector de VERRANO:
/// todo lo que el lector da por bueno es un fotograma ENTERO, el de su
/// publicacion, y es la tanda de bmo_cubo, bit a bit -- los 360 de una vuelta
/// --; y al final, la otra ranura guarda el de antes, tambien entero.
#[test]
fn the_spinning_cube_published_by_titan_is_the_tanda_of_bmo_cubo() {
    let bex = bex_del_banco("nivel11", "cubo_gira");
    let mut m = cargar_bex(&bex).expect("el .bex carga");
    m.padre = 7;
    let mut vistos: Vec<u32> = Vec::new();
    let mut publicados = 0;
    let mut out = [Vertex::default(); 24];
    for _ in 0..PASOS_DEL_CUBO / LEE_CADA {
        // La app no acaba (publica diez minutos de cubo): se corre a plazos.
        let (sigue, acabo) = run_acotado(m, LEE_CADA);
        m = sigue;
        assert!(!acabo, "la app sigue publicando: {}", m.console);
        if !m.ofertas.iter().any(|o| es_lamina(&m, o)) {
            continue;
        }
        let w = la_lamina(&m);
        let l = Lamina::abrir(&w).expect("la lamina del cubo se abre");
        assert_eq!(l.capacidad(), 24);
        // La publicacion `s` es el fotograma `(s - 1) % 360`: la app da
        // vueltas de 360. Y la secuencia no va hacia atras.
        let s = w[lamina::CAMPO_SECUENCIA].load(Ordering::Relaxed);
        assert!(s >= publicados, "la secuencia va hacia atras: {} tras {}", s, publicados);
        publicados = s;
        match l.leer(&mut out) {
            Leido::Nada => assert_eq!(s, 0),
            Leido::Fotograma { fotograma, vertices } => {
                assert_eq!(fotograma, (s - 1) % 360, "la publicacion {}", s);
                igual(fotograma, &out[..vertices], &la_tanda(fotograma));
                if vistos.last() != Some(&fotograma) {
                    vistos.push(fotograma);
                }
            }
            otro => panic!("el lector de VERRANO dio {:?}", otro),
        }
    }
    // Una vuelta ENTERA: el escritorio mira mas a menudo de lo que dura un
    // fotograma, asi que los ve todos.
    vistos.sort_unstable();
    vistos.dedup();
    assert_eq!(vistos, (0..360).collect::<Vec<u32>>(), "los 360 fotogramas, vistos y bit a bit");
    assert!(publicados > 360, "y la vuelta siguiente empieza: {}", publicados);
    // ** Q0a4: y su VENTANA, ofrecida ANTES y tomada antes de la lamina (si
    // no, la oferta de la lamina la habria sustituido): 640 x 360.
    let (base, desde, _, _) = m.ofertas[0];
    assert!(!es_lamina(&m, &m.ofertas[0]), "la primera oferta es la ventana");
    assert_eq!((m.read_u64(base + desde + 4) as u32, m.read_u64(base + desde + 8) as u32), (640, 360));
    // Al final, la otra ranura: el fotograma de antes, entero.
    let w = la_lamina(&m);
    let s = w[lamina::CAMPO_SECUENCIA].load(Ordering::Relaxed) as usize;
    let (ultimo, vs) = ranura(&w, s % 2);
    igual(ultimo, &vs, &la_tanda(ultimo));
    let (antes, vs) = ranura(&w, (s + 1) % 2);
    assert_eq!((antes + 1) % 360, ultimo);
    igual(antes, &vs, &la_tanda(antes));
    std::eprintln!("el cubo de TITAN++: {} fotogramas publicados en {} pasos; el lector vio {} distintos", s, PASOS_DEL_CUBO, vistos.len());
}

/// Los pasos del emulador para una vuelta del cubo y el principio de la
/// siguiente (cada fotograma, unos 500 000: sus gpu fn corren en la CPU), y
/// cada cuanto mira el escritorio.
const PASOS_DEL_CUBO: usize = 184_000_000;
const LEE_CADA: usize = 200_000;
