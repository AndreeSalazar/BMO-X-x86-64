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
