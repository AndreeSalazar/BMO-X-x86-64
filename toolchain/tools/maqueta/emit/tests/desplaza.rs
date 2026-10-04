//! H7: la ventana que se desplaza.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::{desplaza, rust};
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

fn juzgar(src: &str) -> (Laid, String) {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let l = lay(&c);
    let v = judge(&l, &c);
    let txt = render("x.maqueta", src.as_bytes(), &v).split_whitespace().collect::<Vec<_>>().join(" ");
    (l, if v.is_empty() { String::new() } else { txt })
}

const BIBLIOTECA: &str = include_str!("../../pruebas/biblioteca.maqueta");

#[test]
fn lo_de_dentro_pasa_del_alto_y_se_juzga_entero() {
    let (l, v) = juzgar(BIBLIOTECA);
    assert!(v.is_empty(), "{v}");
    let f = desplaza::cajas(&l)[0];
    assert_eq!(desplaza::ventana(f).h, 208);
    assert_eq!(desplaza::total(f), 4 + 8 * 40 + 7 * 6 + 4, "ocho pistas, siete huecos y el relleno");
    assert!(desplaza::barra(f, 0).is_some(), "no cabe: hay barra");
    let arriba = desplaza::barra(f, 0).unwrap().y;
    let abajo = desplaza::barra(f, desplaza::maximo(f)).unwrap();
    assert!(abajo.y > arriba && abajo.bottom() <= (desplaza::ventana(f).bottom() - 3), "la barra baja y no se sale");
}

#[test]
fn el_modulo_lleva_la_ventana_y_pintar_la_arranca() {
    let (l, _) = juzgar(BIBLIOTECA);
    let g = rust::modulo("b.maqueta", &l);
    assert!(g.contains("pub const DESPLAZA_LISTA: Desplaza = Desplaza { x: 14, y: 40, w: 256, h: 208, total: 370 };"), "{g}");
    assert!(g.contains("pub fn desplazar_lista(p: &bmo::Pantalla, ox: u32, oy: u32, desde: u32)"));
    let pintar = &g[g.find("pub fn pintar(").unwrap()..g.find("pub fn pintar_en(").unwrap()];
    assert!(pintar.contains("desplazar_lista(p, ox, oy, 0);"), "{pintar}");
    assert!(!pintar.contains("Kernel a medianoche"), "lo de dentro no se pinta suelto: {pintar}");
}

#[test]
fn una_ventana_sin_alto_ni_fondo_se_explica() {
    let (_, v) = juzgar(&BIBLIOTECA.replace(" height:200px;", "").replace(" background-color:#0D1118;", ""));
    assert!(v.contains("esta caja se desplaza (`overflow-y: auto`) y necesita un `height`"), "{v}");
    assert!(v.contains("un `background-color` liso"), "{v}");
}

#[test]
fn de_ancho_no_se_sale_aunque_se_desplace() {
    let (_, v) = juzgar(&BIBLIOTECA.replace("height:40px; padding:0 8px 0 8px;", "width:400px; height:40px; padding:0 8px 0 8px;"));
    assert!(v.contains("se sale de su padre"), "{v}");
}
