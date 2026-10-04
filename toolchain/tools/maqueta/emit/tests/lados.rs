//! **ESCALON 1, pintado**: un borde distinto por lado sale en sus pixeles, y
//! la cara que viaja pinta lo mismo que la foto directa.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::{bef, foto, orden};
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

fn compilar(src: &str) -> Laid {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let l = lay(&c);
    let v = judge(&l, &c);
    assert!(v.is_empty(), "{}", render("x.maqueta", src.as_bytes(), &v));
    l
}

const FILA: &str = "<maqueta class=\"m\"><div class=\"fila\"></div></maqueta>\
<style>:root{--raya:#FF0000; --fondo:#0000FF}\
.m{width:20px; height:10px}\
.fila{width:18px; height:9px; background:var(--fondo); border-bottom:1px solid var(--raya); border-left:2px solid #00FF00}</style>";

#[test]
fn la_raya_de_abajo_y_el_lado_izquierdo_salen_donde_dicen() {
    let l = compilar(FILA);
    let f = foto::foto(&l);
    let px = |x: u32, y: u32| f.px[(y * f.ancho + x) as usize];
    assert_eq!(px(10, 9), 0xFF0000, "la ultima fila es la raya");
    assert_eq!(px(10, 8), 0x0000FF, "encima, el fondo");
    assert_eq!(px(0, 4), 0x00FF00, "el lado izquierdo, 2 px");
    assert_eq!(px(1, 4), 0x00FF00);
    assert_eq!(px(2, 4), 0x0000FF);
    assert_eq!(px(19, 0), 0x0000FF, "arriba y a la derecha no hay borde");
}

#[test]
fn los_lados_viajan_y_pintan_lo_mismo() {
    let l = compilar(FILA);
    let bytes = bef::escribir(&orden::lista(&l), &orden::golpes(&l), 20, 10).expect("cabe");
    let viajada = foto::foto_cara(&bytes).expect("se lee");
    assert_eq!(viajada.px, foto::foto(&l).px);
}

#[test]
fn un_borde_por_lado_en_una_caja_redonda_no_pasa_el_veredicto() {
    let src = "<maqueta class=\"m\"><div class=\"a\"></div></maqueta>\
<style>.m{width:40px; height:20px} .a{width:40px; height:20px; border-radius:6px; border-bottom:1px solid #FFFFFF}</style>";
    let doc = parse(src.as_bytes()).unwrap();
    let c = cascade(&doc).unwrap();
    let l = lay(&c);
    let v = render("x.maqueta", src.as_bytes(), &judge(&l, &c));
    assert!(v.contains("borde distinto por lado"), "{v}");
}

// -- P3b: la transicion, par a par ------------------------------------------

/// Los extremos de una transicion pintada par a par son los dos estados,
/// pixel a pixel -- aunque uno sea plano y el otro redondo (la caja plana se
/// pinta como pieza de radio 0, y son los mismos pixeles).
#[test]
fn los_extremos_de_la_transicion_son_los_estados() {
    let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/../pruebas/panel.maqueta");
    let e = bmo_maqueta_compone::compilar_estados(std::path::Path::new(ruta)).unwrap_or_else(|f| panic!("{}", f.render()));
    let b = e.de("abierta").unwrap();
    let pares = bmo_maqueta_emit::movimiento::pares(&e.reposo, b);
    let total = bmo_maqueta_emit::movimiento::duracion(&pares);
    assert_eq!(total, 600, "520 ms y la espera mas larga, 80");
    let lienzo = e.reposo.canvas;
    assert_eq!(foto::foto_en(&pares, lienzo, 0).px, foto::foto(&e.reposo).px, "al empezar, el reposo");
    assert_eq!(foto::foto_en(&pares, lienzo, total).px, foto::foto(b).px, "al acabar, abierta");
    let mitad = foto::foto_en(&pares, lienzo, total / 2).px;
    assert_ne!(mitad, foto::foto(&e.reposo).px);
    assert_ne!(mitad, foto::foto(b).px);
}

/// Una caja plana que pasa a redonda: en el primer instante, los mismos
/// pixeles que el rect plano.
#[test]
fn una_caja_plana_y_su_pieza_de_radio_cero_pintan_igual() {
    let src = "<maqueta class=\"m\"><div class=\"c\"></div></maqueta>\
<style>.m{width:60px; height:40px} .c{width:40px; height:20px; background-color:#3366FF; border:1px solid #FFFFFF; transition: 100ms linear}\
@estado r { .c{border-radius:8px} }</style>";
    let dir = std::env::temp_dir().join(format!("maqueta-p3b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("c.maqueta");
    std::fs::write(&ruta, src).unwrap();
    let e = bmo_maqueta_compone::compilar_estados(&ruta).unwrap_or_else(|f| panic!("{}", f.render()));
    let pares = bmo_maqueta_emit::movimiento::pares(&e.reposo, e.de("r").unwrap());
    assert_eq!(foto::foto_en(&pares, e.reposo.canvas, 0).px, foto::foto(&e.reposo).px);
    let _ = std::fs::remove_dir_all(&dir);
}

// Lo que antes probaba la mezcla por cajas (P3a), ahora sobre la UNICA
// mezcla que hay: la de piezas, par a par.

fn estados(src: &str, nombre: &str) -> bmo_maqueta_compone::Estados {
    let dir = std::env::temp_dir().join(format!("maqueta-mov-{}-{nombre}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("m.maqueta");
    std::fs::write(&ruta, src).unwrap();
    let e = bmo_maqueta_compone::compilar_estados(&ruta).unwrap_or_else(|f| panic!("{}", f.render()));
    let _ = std::fs::remove_dir_all(&dir);
    e
}

/// La caja de color `c` a los `ms`, como la pinta la transicion.
fn caja_en(pares: &[bmo_maqueta_emit::movimiento::Par], c: u32, ms: u32) -> (i32, i32, i32, u32) {
    use bmo_maqueta_emit::orden::Trazo;
    for p in pares {
        let es = |t: &Option<Trazo>| matches!(t, Some(Trazo::Rect { color, .. }) | Some(Trazo::Caja { color, .. }) if *color == c);
        if es(&p.a) || es(&p.b) {
            let k = bmo_pinta::avance(ms, p.retraso, p.dura, p.curva);
            let m = p.a.as_ref().unwrap().con_pieza_o_caja(|a| p.b.as_ref().unwrap().con_pieza_o_caja(|b| match bmo_pinta::entre_piezas(a, b, k) {
                bmo_pinta::Pieza::Caja { x, y, w, c, .. } => (x, y, w, c),
                _ => unreachable!(),
            }));
            return m.flatten().unwrap();
        }
    }
    panic!("no hay caja de ese color")
}

const PANEL: &str = "<maqueta class=\"m\"><div class=\"c\"></div></maqueta>\
<style>.m{width:300px; height:200px} .c{width:100px; height:40px; background-color:#000000; transition: 400ms linear}\
@estado abierta { .c{width:260px; height:160px; background-color:#FFFFFF} }</style>";

#[test]
fn la_transicion_va_de_uno_a_otro() {
    let e = estados(PANEL, "a");
    let pares = bmo_maqueta_emit::movimiento::pares(&e.reposo, e.de("abierta").unwrap());
    assert_eq!(bmo_maqueta_emit::movimiento::duracion(&pares), 400);
    assert_eq!(caja_en(&pares, 0x000000, 0).2, 100);
    let (_, _, w, c) = caja_en(&pares, 0x000000, 200);
    assert_eq!((w, c), (180, 0x808080), "a la mitad, en lineal, a la mitad: medida y color");
    assert_eq!(caja_en(&pares, 0x000000, 400).2, 260);
    assert_eq!(caja_en(&pares, 0x000000, 9999).2, 260, "despues del final, el final");
}

#[test]
fn el_rebote_se_pasa_y_vuelve() {
    let e = estados(&PANEL.replace("400ms linear", "400ms cubic-bezier(.34, 1.56, .64, 1)"), "b");
    let pares = bmo_maqueta_emit::movimiento::pares(&e.reposo, e.de("abierta").unwrap());
    let anchos: Vec<i32> = (0..=40).map(|k| caja_en(&pares, 0x000000, k * 10).2).collect();
    assert!(anchos.iter().any(|&w| w > 260), "se pasa de largo: {anchos:?}");
    assert_eq!(*anchos.last().unwrap(), 260, "y acaba donde tiene que acabar");
}

#[test]
fn una_caja_que_cambia_de_sitio_en_la_lista_se_empareja_por_lo_que_es() {
    let src = "<maqueta class=\"m\"><div class=\"a\"></div><div class=\"b\"></div></maqueta>\
<style>.m{width:300px; height:200px; display:flex; flex-direction:column} .a{width:50px; height:20px; background-color:#FF0000; transition: 100ms linear} .b{width:60px; height:20px; background-color:#00FF00}\
@estado fuera { .a{position:absolute; left:200px; top:150px} }</style>";
    let e = estados(src, "c");
    let pares = bmo_maqueta_emit::movimiento::pares(&e.reposo, e.de("fuera").unwrap());
    let (x, y, _, _) = caja_en(&pares, 0xFF0000, 50);
    assert_eq!((x, y), (100, 75), "a medio camino entre (0, 0) y (200, 150)");
}
