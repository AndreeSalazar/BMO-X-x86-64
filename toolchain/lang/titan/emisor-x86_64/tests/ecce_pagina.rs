//! ** PL4 de `docs/plan/PLAN_ECCE.md` (16-10): EL PAGINADO DEL OFICIO de ECCE
//! (`Ultra_userspace/apps/ecce/src/pagina.titan`), corrido en el emulador.
//!
//! Un programa de prueba usa los modulos de ECCE tal cual (`guion` y
//! `pagina`), lee el guion de `datos/g.fou` y dice cada pagina: su primera
//! linea que se ve y, si empieza a mitad de un dialogo, la del nombre.
//!
//! ```text
//!    ===              salta de pagina; el blanco de arriba no cuenta
//!    el dialogo       se parte con el nombre y dos lineas arriba (y el MAS)
//!    o no se parte    si arriba quedarian menos de dos: pasa entero
//!    la escena        nunca sola al final de la pagina
//!    la accion        se parte dejando lo que cabe
//! ```

use std::path::Path;

use bmo_lower::emu::{cargar_bex, run};

const PRUEBA: &str = "mod main \"las paginas de un guion\"\nuse director\nmod guion\nmod pagina\n\nfn main()\n    let mut t: [byte] = []\n    if director.fichero(\"datos/g.fou\")\n        for i in range(director.medida())\n            push(mut t, byte(director.byte(i)))\n    let mut ind: [int] = []\n    let mut tl: [int] = []\n    let l = guion.lineas(t, 83, mut ind, mut tl)\n    let mut ih: [int] = []\n    let mut co: [int] = []\n    pagina.pagina(tl, mut ih, mut co)\n    for i in range(len(ih))\n        print(ih[i], \" \", co[i])\n";

/// Las paginas de `guion`: `(primera linea, linea del nombre o -1)`.
fn paginas(guion: &str) -> Vec<(i64, i64)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/ecce");
    let bex = bmo_titan_x86_64::build_package("src/main.titan", PRUEBA, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e));
    let mut m = cargar_bex(&bex).expect("el .bex carga");
    m.archivos.insert("datos/g.fou".into(), guion.as_bytes().to_vec());
    let m = run(m, 3_000_000_000);
    assert!(m.exited, "{}", m.console);
    m.console
        .lines()
        .map(|l| {
            let mut p = l.split(' ').map(|x| x.parse::<i64>().unwrap());
            (p.next().unwrap(), p.next().unwrap())
        })
        .collect()
}

/// "INT. A", un blanco y `n` lineas de accion seguidas (un parrafo).
fn con_accion(n: usize) -> String {
    let mut s = String::from("INT. A - DIA\n\n");
    for i in 0..n {
        s.push_str(&format!("pasa algo {i}\n"));
    }
    s
}

/// Con `n` lineas de accion, un blanco, ANA y diez frases.
fn con_dialogo(n: usize) -> String {
    let mut s = con_accion(n);
    s.push_str("\nANA\n");
    for i in 0..10 {
        s.push_str(&format!("frase {i}.\n"));
    }
    s.pop();
    s
}

#[test]
fn a_short_script_is_one_page() {
    assert_eq!(paginas("INT. A - DIA\n\nNada."), vec![(0, -1)]);
}

/// "===" salta: la pagina 2 empieza en el blanco de despues (que no cuenta).
#[test]
fn the_page_break_jumps() {
    assert_eq!(paginas("INT. A - DIA\n\nuno\n\n===\n\nINT. B - DIA\n\ndos"), vec![(0, -1), (5, -1)]);
}

/// ** El dialogo que no cabe se PARTE: 2 + 48 + 1 lineas llenan 51, y en las
/// 4 que quedan van ANA, dos frases y el (MAS). La pagina 2 empieza en la
/// tercera frase (la linea 54), y su nombre es el de la linea 51.
#[test]
fn a_long_dialogue_is_split_with_more_and_cont() {
    assert_eq!(paginas(&con_dialogo(48)), vec![(0, -1), (54, 51)]);
}

/// ** Con una linea menos de sitio arriba, quedaria UNA frase con el nombre:
/// no se parte, el dialogo entero pasa (sin CONT.).
#[test]
fn a_dialogue_with_one_line_on_top_moves_whole() {
    assert_eq!(paginas(&con_dialogo(49)), vec![(0, -1), (52, -1)]);
}

/// ** La escena no se queda sola al final: en la linea 53 no caben ella y
/// dos lineas mas, asi que empieza la pagina 2.
#[test]
fn a_scene_heading_is_never_last() {
    let mut s = con_accion(50);
    s.push_str("\nINT. B - NOCHE\n\nfin");
    assert_eq!(paginas(&s), vec![(0, -1), (53, -1)]);
}

/// ** La accion larga se parte por donde se llena la pagina: 2 + 53 = 55.
#[test]
fn a_long_action_is_split_where_the_page_ends() {
    assert_eq!(paginas(&con_accion(60)), vec![(0, -1), (55, -1)]);
}

/// ** Un dialogo de mas de una pagina se parte las veces que haga falta, y
/// cada pagina dice su nombre.
#[test]
fn a_dialogue_longer_than_a_page_keeps_its_name() {
    let mut s = String::from("INT. A - DIA\n\nANA\n");
    for i in 0..130 {
        s.push_str(&format!("frase {i}.\n"));
    }
    s.pop();
    let p = paginas(&s);
    assert_eq!(p.len(), 3, "{p:?}");
    assert!(p[1..].iter().all(|&(_, n)| n == 2), "{p:?}");
    // Pagina 1: 2 + ANA + 51 frases + MAS = 55; la 2: CONT + 53 + MAS.
    assert_eq!(p[1].0, 54);
    assert_eq!(p[2].0, 54 + 53);
}
