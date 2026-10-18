//! ** MC1 de `docs/plan/PLAN_MUNDO.md` (11-10): MUNDO, LOS BLOQUES EN 3D
//! (`Ultra_userspace/apps/mundo/`), de TITAN++ a la lamina y de la lamina al
//! juez de VERRANO con profundidad y sin las caras de detras.
//!
//! ```text
//!    las caras      el mundo entero en pocas caras JUNTADAS (un suelo de
//!                   16 x 16 es una), y solo las que miran a la camara
//!    el sentido     cada cara horaria vista desde fuera: descartar las de
//!                   detras deja la MISMA imagen que dibujarlas todas
//!    andar          w, q/e: la camara anda y gira, y la imagen con ella
//!    picar          x quita el bloque que se mira, c pone uno de tablas
//! ```

use bmo_abi::syscalls::surface::SUP_EV_CARACTER;
use bmo_lower::emu::{cargar_bex, run_acotado};
use bmo_verrano::cpu::Cpu;
use bmo_verrano::lamina::{self, Lamina, Leido};
use bmo_verrano::{Backend, Cull, Frame, Image, Vertex, Viewport};
use core::sync::atomic::AtomicU32;
use std::path::Path;

/// 126 caras de 6, mas 12 vertices para la mira (MC4: el techo del DIRECTOR).
const CAPACIDAD: usize = 768;

fn bex() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/mundo");
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn letra(c: u8) -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | c as u64
}

/// MUNDO lanzado por el escritorio con estas teclas: el fotograma de cuando
/// ya las leyo todas, y lo que costo hasta el primero.
fn con(teclas: &str) -> (Vec<Vertex>, u64) {
    let mut m = cargar_bex(&bex()).unwrap();
    m.padre = 7;
    for c in teclas.bytes() {
        m.buzon_pendiente.push_back(letra(c));
    }
    let mut out = [Vertex::default(); CAPACIDAD];
    let mut primero = None;
    let mut despues = 0;
    for _ in 0..400 {
        let (sigue, acabo) = run_acotado(m, 2_000_000);
        m = sigue;
        assert!(!acabo, "MUNDO sigue: {}", m.console);
        let Some(&(base, _, bytes, _)) = m.ofertas.iter().find(|o| m.read_u64(o.0 + o.1) as u32 == lamina::MAGIA) else { continue };
        let w: Vec<AtomicU32> = (0..bytes / 4).map(|k| AtomicU32::new(m.read_u64(base + 4 * k) as u32)).collect();
        let l = Lamina::abrir(&w).expect("la lamina se abre");
        assert_eq!(l.capacidad() as usize, CAPACIDAD);
        assert_eq!(l.estado(), (true, Cull::Back), "MUNDO pide profundidad y sin las caras de detras");
        if let Leido::Fotograma { vertices, .. } = l.leer(&mut out) {
            primero.get_or_insert(m.pasos);
            // las teclas que quedan llegan en las vueltas que siguen
            if m.buzon_pendiente.is_empty() {
                despues += 1;
                if despues == 3 {
                    return (out[..vertices as usize].to_vec(), primero.unwrap());
                }
            }
        }
    }
    panic!("MUNDO no publico: {}", m.console);
}

const W: u32 = 640;
const H: u32 = 360;

fn dibuja(v: &[Vertex], cull: Cull) -> Vec<u32> {
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: v, viewport: Viewport { width: W, height: H }, depth: true, cull };
    let mut px = vec![0u32; (W * H) as usize];
    let mut z = vec![0f32; (W * H) as usize];
    let mut juez = Cpu { max_vertices: CAPACIDAD, ..Cpu::LA_3060 };
    juez.draw(&frame, &mut Image { pixels: &mut px, width: W, height: H, depth: Some(&mut z) }).unwrap();
    px
}

/// Con `MUNDO_FOTOS=carpeta`, cada imagen a un PPM: para MIRARLA.
fn foto(nombre: &str, px: &[u32]) {
    let Ok(dir) = std::env::var("MUNDO_FOTOS") else { return };
    let mut b = format!("P6 {W} {H} 255\n").into_bytes();
    for p in px {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    std::fs::write(Path::new(&dir).join(format!("{nombre}.ppm")), b).unwrap();
}

/// Cuantos pixeles de cada tipo de bloque: (hierba, piedra, tablas, fondo).
fn cuenta(px: &[u32]) -> (usize, usize, usize, usize) {
    let fondo = dibuja(&[Vertex::default(); 3], Cull::None)[0];
    let (mut hierba, mut piedra, mut tablas, mut nada) = (0, 0, 0, 0);
    for &p in px {
        let (r, g, b) = ((p >> 16 & 0xFF) as i32, (p >> 8 & 0xFF) as i32, (p & 0xFF) as i32);
        if p == fondo {
            nada += 1;
        } else if g > r + 30 && g > b + 30 {
            hierba += 1;
        } else if (r - g).abs() < 12 && (g - b).abs() < 12 {
            piedra += 1;
        } else if r > 100 && g * 100 > r * 78 && r > b + 50 {
            // las tablas: claras, y mas amarillas que la tierra y el tronco
            tablas += 1;
        }
    }
    (hierba, piedra, tablas, nada)
}

#[test]
fn the_world_is_a_few_joined_faces_and_they_face_the_right_way() {
    let (v, coste) = con("");
    let tris = v.len() / 3;
    eprintln!("al abrir: {} caras del mundo + mira, {} instrucciones", (v.len() - 12) / 6, coste);
    assert!(v.len() % 6 == 0 && (6..=CAPACIDAD).contains(&v.len()), "{} vertices", v.len());
    let px = dibuja(&v, Cull::Back);
    let todas = dibuja(&v, Cull::None);
    let distintos = px.iter().zip(&todas).filter(|(a, b)| a != b).count();
    assert!(distintos * 200 < (W * H) as usize, "{distintos} pixeles distintos con y sin descarte: alguna cara va al reves");
    foto("abrir", &px);
    let (hierba, piedra, tablas, nada) = cuenta(&px);
    eprintln!("{tris} triangulos: hierba {hierba}, piedra {piedra}, tablas {tablas}, fondo {nada}");
    // el suelo llena la mitad de abajo; la torre de piedra se ve; el cielo
    // (el fondo) arriba
    assert!(hierba > 30_000, "hierba {hierba}");
    assert!(piedra > 1_000, "piedra {piedra}");
    assert!(nada > 30_000, "fondo {nada}");
    assert_eq!(tablas, 0);
}

#[test]
fn walking_and_turning_move_the_picture() {
    let (quieto, _) = con("");
    let (anda, _) = con("wwwwwwwwww");
    let (gira, _) = con("eeeeee");
    let a = dibuja(&quieto, Cull::Back);
    let b = dibuja(&anda, Cull::Back);
    let c = dibuja(&gira, Cull::Back);
    foto("anda", &b);
    foto("gira", &c);
    let cambia = |x: &[u32], y: &[u32]| x.iter().zip(y).filter(|(p, q)| p != q).count();
    assert!(cambia(&a, &b) > 5_000, "andar dos bloques cambia {} pixeles", cambia(&a, &b));
    assert!(cambia(&a, &c) > 20_000, "girar 90 grados cambia {} pixeles", cambia(&a, &c));
    // girando a la derecha (e) la torre, que estaba a la izquierda, sale
    let (_, piedra_antes, _, _) = cuenta(&a);
    let (_, piedra_despues, _, _) = cuenta(&c);
    eprintln!("piedra: {piedra_antes} mirando al fondo, {piedra_despues} mirando a +x");
    assert!(piedra_despues * 4 < piedra_antes, "piedra {piedra_despues} contra {piedra_antes}");
}

#[test]
fn picking_takes_a_block_away_and_putting_one_shows_planks() {
    // seis bloques hacia el fondo: la loma queda a 4,5 bloques, a la altura
    // de los ojos
    let ida = "wwwwwwwwwwwwwwwwwwwwwwwwwwwwww";
    let (antes, _) = con(ida);
    let (sin, _) = con(&format!("{ida}x"));
    let (con_tablas, _) = con(&format!("{ida}xc"));
    let a = dibuja(&antes, Cull::Back);
    let b = dibuja(&sin, Cull::Back);
    let c = dibuja(&con_tablas, Cull::Back);
    foto("lejos", &a);
    foto("quita", &b);
    foto("pone", &c);
    let cambia = |x: &[u32], y: &[u32]| x.iter().zip(y).filter(|(p, q)| p != q).count();
    eprintln!("caras: {} antes, {} sin el bloque, {} con tablas", antes.len() / 6, sin.len() / 6, con_tablas.len() / 6);
    assert_ne!(antes.len(), sin.len(), "quitar un bloque cambia las caras");
    assert!(cambia(&a, &b) > 500, "quitar cambia {} pixeles", cambia(&a, &b));
    let (_, _, tablas, _) = cuenta(&c);
    assert!(tablas > 1_000, "las tablas puestas se ven: {tablas}");
}

fn close_world(code: &[u8], keys: &str, saved: Option<&[u8]>) -> bmo_lower::emu::Machine {
    let mut m = cargar_bex(code).unwrap();
    m.padre = 7;
    if let Some(bytes) = saved {
        m.archivos.insert("datos/mundo.bmv".into(), bytes.to_vec());
    }
    for c in keys.bytes().chain([27]) {
        m.buzon_pendiente.push_back(letra(c));
    }
    let m = bmo_lower::emu::run(m, 100_000_000);
    assert!(m.exited, "MUNDO no cerro: {}", m.console);
    m
}

#[test]
fn edited_blocks_survive_closing_and_loading() {
    let code = bex();
    let untouched = close_world(&code, "", None);
    assert!(!untouched.archivos.contains_key("datos/mundo.bmv"));
    let initial = close_world(&code, "g", None);
    let baseline = &initial.archivos["datos/mundo.bmv"];
    let edited = close_world(&code, "wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwx", None);
    let saved = &edited.archivos["datos/mundo.bmv"];
    assert_eq!(saved.len(), 2056);
    assert_eq!(&saved[..8], &[66, 77, 87, 49, 16, 8, 16, 0]);
    assert!(saved[8..].iter().all(|b| *b <= 6));
    assert_ne!(saved, baseline, "el bloque quitado se guarda");
    let loaded = close_world(&code, "g", Some(saved));
    assert!(loaded.console.contains("mundo cargado"), "{}", loaded.console);
    assert_eq!(&loaded.archivos["datos/mundo.bmv"], saved);
}

#[test]
fn malformed_worlds_are_refused_and_preserved_on_clean_exit() {
    let code = bex();
    let initial = close_world(&code, "g", None);
    let valid = &initial.archivos["datos/mundo.bmv"];
    let mut bad_header = valid.clone();
    bad_header[0] = 0;
    let mut bad_block = valid.clone();
    bad_block[100] = 255;
    for bad in [&valid[..100], &bad_header[..], &bad_block[..]] {
        let m = close_world(&code, "", Some(bad));
        assert!(m.console.contains("se abre el mundo inicial"), "{}", m.console);
        assert!(!m.console.contains("mundo cargado"));
        assert_eq!(&m.archivos["datos/mundo.bmv"][..], bad);
    }
}

#[test]
fn failed_save_keeps_the_edited_world_open() {
    let mut m = cargar_bex(&bex()).unwrap();
    m.padre = 7;
    m.fallar_al_guardar("datos/mundo.bmv");
    for c in b"wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwx\x1b" {
        m.buzon_pendiente.push_back(letra(*c));
    }
    let (m, stopped) = run_acotado(m, 30_000_000);
    assert!(!stopped && !m.exited, "el fallo no debe cerrar: {}", m.console);
    assert!(m.console.contains("el disco NO guardo; sigue abierto"), "{}", m.console);
    assert!(!m.archivos.contains_key("datos/mundo.bmv"));
}

#[test]
fn pitch_moves_the_world_but_the_crosshair_stays_visible_and_centered() {
    let (level, _) = con("");
    let (up, _) = con("ii");
    let (down, _) = con("kk");
    let a = dibuja(&level, Cull::Back);
    let b = dibuja(&up, Cull::Back);
    let c = dibuja(&down, Cull::Back);
    foto("arriba", &b);
    foto("abajo", &c);
    let background = |px: &[u32]| cuenta(px).3;
    assert!(background(&b) > background(&a) + 10_000, "mirar arriba muestra mas cielo");
    assert!(background(&c) + 10_000 < background(&a), "mirar abajo muestra menos cielo");
    for px in [&a, &b, &c] {
        let white = (175..185).flat_map(|y| (315..325).map(move |x| y * W as usize + x))
            .filter(|&i| px[i] & 0xFFFFFF == 0xFFFFFF).count();
        assert!(white >= 20, "la mira se ve con descarte y profundidad: {white}");
    }
    // Incluso una cara que toque el plano cercano no tapa la mira: ambas
    // tienen z=0 y el HUD se presenta primero con profundidad LESS.
    let mut near = level.clone();
    for v in &mut near[12..] { v.position[2] = 0.0; }
    let px = dibuja(&near, Cull::Back);
    assert_eq!(px[(W * H / 2 + W / 2) as usize] & 0xFFFFFF, 0xFFFFFF);
}

#[test]
fn pitch_is_clamped_and_yaw_wraps() {
    let (limit, _) = con("iiiii");
    let (beyond, _) = con("iiiiiiiiiiiiiii");
    assert_eq!(dibuja(&limit, Cull::Back), dibuja(&beyond, Cull::Back));
    let (level, _) = con("");
    let (wrapped, _) = con(&"q".repeat(24));
    assert_eq!(dibuja(&level, Cull::Back), dibuja(&wrapped, Cull::Back));
}

#[test]
fn picking_and_placing_follow_pitch_instead_of_eye_height() {
    let code = bex();
    let initial = close_world(&code, "g", None);
    let baseline = &initial.archivos["datos/mundo.bmv"];
    let horizontal = close_world(&code, "x", None);
    assert!(!horizontal.archivos.contains_key("datos/mundo.bmv"), "sin inclinar, el suelo no es el blanco");
    let down = close_world(&code, "kkkx", None);
    let saved = &down.archivos["datos/mundo.bmv"];
    let ground = 8 + (16 + 3) * 16 + 8;
    assert_eq!(baseline[ground], 1);
    assert_eq!(saved[ground], 0, "se quita la hierba bajo el rayo inclinado");
    assert_eq!(saved.iter().zip(baseline).filter(|(a, b)| a != b).count(), 1);
    let placed = close_world(&code, "kkkc", None);
    let top = 8 + (2 * 16 + 3) * 16 + 8;
    assert_eq!(placed.archivos["datos/mundo.bmv"][top], 6, "tablas sobre el suelo apuntado");
}

fn published(m: &bmo_lower::emu::Machine) -> Option<Vec<Vertex>> {
    let &(base, _, bytes, _) = m.ofertas.iter().find(|o| m.read_u64(o.0 + o.1) as u32 == lamina::MAGIA)?;
    let w: Vec<AtomicU32> = (0..bytes / 4).map(|k| AtomicU32::new(m.read_u64(base + 4 * k) as u32)).collect();
    let l = Lamina::abrir(&w).unwrap();
    let mut vertices = vec![Vertex::default(); CAPACIDAD];
    let Leido::Fotograma { vertices: n, .. } = l.leer(&mut vertices) else { return None };
    vertices.truncate(n); Some(vertices)
}

#[test]
fn captured_mouse_looks_freely_and_escape_releases_before_closing() {
    let mut m = cargar_bex(&bex()).unwrap(); m.padre = 7; m.mouse_capture_allowed = true;
    let initial = loop {
        let (next, exited) = run_acotado(m, 2_000_000); m = next; assert!(!exited);
        if let Some(v) = published(&m) { break v; }
    };
    let event = |dx: i16, dy: i16| (1 << 59) | 0x100 | (dx as u16 as u64) << 16 | (dy as u16 as u64) << 32;
    m.buzon_pendiente.push_back(event(10, -20));
    let (mut m, exited) = run_acotado(m, 8_000_000); assert!(!exited);
    let moved = published(&m).unwrap();
    assert_ne!(dibuja(&moved, Cull::Back), dibuja(&initial, Cull::Back));
    assert_eq!(dibuja(&moved, Cull::Back)[(W * H / 2 + W / 2) as usize] & 0xFFFFFF, 0xFFFFFF);
    m.buzon_pendiente.push_back(event(-10, 20));
    let (mut m, exited) = run_acotado(m, 8_000_000); assert!(!exited);
    assert_eq!(dibuja(&published(&m).unwrap(), Cull::Back), dibuja(&initial, Cull::Back));
    m.buzon_pendiente.push_back(letra(27));
    let (mut m, exited) = run_acotado(m, 2_000_000); assert!(!exited, "Esc suelta, no cierra");
    assert!(m.console.contains("raton libre"), "{}", m.console);
    m.buzon_pendiente.push_back(letra(27));
    let (m, exited) = run_acotado(m, 2_000_000); assert!(exited && m.exited);
}
