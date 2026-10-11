//! ** M1 = V2 de `docs/plan/PLAN_VERRANO.md` (11-10): DOS CUBOS QUE SE TAPAN,
//! de TITAN++ (`ejemplos/nivel11/dos_cubos`) a la lamina, y de la lamina al
//! juez de VERRANO con profundidad.
//!
//! ```text
//!    la app        publica los dos cubos ENTEROS (24 triangulos) y pide
//!                  `director.profundidad(true)`: la cabecera lo dice
//!    el juez       con z-buffer y sin las caras de detras: el de delante tapa
//!                  al de detras; sin profundidad, gana el orden (mal)
//!    el sentido    las caras van horarias vistas desde fuera: descartar las
//!                  de detras deja la MISMA imagen que dibujarlas todas con
//!                  z-buffer (si estuvieran al reves, se verian las de dentro)
//! ```

use bmo_lower::emu::{cargar_bex, run_acotado};
use bmo_verrano::cpu::Cpu;
use bmo_verrano::lamina::{self, Lamina, Leido};
use bmo_verrano::{Backend, Cull, Frame, Image, Vertex, Viewport};
use core::sync::atomic::AtomicU32;
use std::path::Path;

/// El fotograma en que el cubo de detras esta justo DETRAS del de delante
/// (su centro x, -1,6 + f / 100, llega a -0,45).
const CRUCE: u32 = 115;

fn bex() -> Vec<u8> {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ejemplos/nivel11/dos_cubos");
    let src = std::fs::read_to_string(pkg.join("src/main.titan")).unwrap();
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// Corre la app hasta que publica el fotograma `f` (o uno despues): la
/// cabecera de su lamina, y sus vertices.
fn hasta(f: u32) -> ((bool, Cull), u32, Vec<Vertex>) {
    let mut m = cargar_bex(&bex()).unwrap();
    m.padre = 7;
    let mut out = [Vertex::default(); 72];
    for _ in 0..400 {
        let (sigue, acabo) = run_acotado(m, 2_000_000);
        m = sigue;
        assert!(!acabo, "la app sigue: {}", m.console);
        let Some(&(base, _, bytes, _)) = m.ofertas.iter().find(|o| m.read_u64(o.0 + o.1) as u32 == lamina::MAGIA) else { continue };
        let w: Vec<AtomicU32> = (0..bytes / 4).map(|k| AtomicU32::new(m.read_u64(base + 4 * k) as u32)).collect();
        let l = Lamina::abrir(&w).expect("la lamina se abre");
        assert_eq!(l.capacidad(), 72);
        if let Leido::Fotograma { fotograma, vertices } = l.leer(&mut out) {
            if fotograma >= f {
                assert_eq!(vertices, 72, "los dos cubos enteros");
                return (l.estado(), fotograma, out.to_vec());
            }
        }
    }
    panic!("no llego al fotograma {f}: {}", m.console);
}

const W: u32 = 640;
const H: u32 = 360;

fn dibuja(v: &[Vertex], depth: bool, cull: Cull) -> (Vec<u32>, u32) {
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: v, viewport: Viewport { width: W, height: H }, depth, cull };
    let mut px = vec![0u32; (W * H) as usize];
    let mut z = vec![0f32; (W * H) as usize];
    let mut juez = Cpu { max_vertices: 768, ..Cpu::LA_3060 };
    let s = juez.draw(&frame, &mut Image { pixels: &mut px, width: W, height: H, depth: Some(&mut z) }).unwrap();
    (px, s.triangles)
}

/// Rojo de delante (r alto, b bajo) y azul de detras (b alto, r bajo).
fn cuenta(px: &[u32]) -> (usize, usize) {
    let (mut rojo, mut azul) = (0, 0);
    for &p in px {
        let (r, b) = (p >> 16 & 0xFF, p & 0xFF);
        rojo += (r > 100 && b < 80) as usize;
        azul += (b > 100 && r < 80) as usize;
    }
    (rojo, azul)
}

#[test]
fn two_cubes_hide_each_other_through_the_lamina() {
    let (estado, f, v) = hasta(CRUCE);
    assert_eq!(estado, (true, Cull::Back), "la app pidio profundidad y sin las caras de detras");
    let (con, tris) = dibuja(&v, true, Cull::Back);
    assert!((4..=12).contains(&tris), "de 24 triangulos se dibujan los de delante: {tris}");
    // ** el sentido de las caras: descartar las de detras no cambia nada que
    // el z-buffer no hubiera tapado ya (salvo el borde de alguna arista)
    let (todas, _) = dibuja(&v, true, Cull::None);
    let distintos = con.iter().zip(&todas).filter(|(a, b)| a != b).count();
    assert!(distintos * 100 < (W * H) as usize, "{distintos} pixeles distintos: las caras irian al reves");
    // los dos se ven, y el de delante tapa: sin profundidad, el de detras
    // (que se dibuja despues) se le pone encima
    let (rojo, azul) = cuenta(&con);
    let (rojo_mal, _) = cuenta(&dibuja(&v, false, Cull::None).0);
    assert!(rojo > 2000 && azul > 200, "fotograma {f}: rojo {rojo}, azul {azul}");
    assert!(rojo > rojo_mal + 500, "con profundidad el de delante se ve entero: {rojo} contra {rojo_mal}");
    eprintln!("fotograma {f}: {tris} triangulos de 24; rojo {rojo} px, azul {azul} px; sin profundidad, rojo {rojo_mal}");
}
