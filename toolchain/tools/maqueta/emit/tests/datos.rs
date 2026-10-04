//! Lo que sale de una pieza con DATOS (H1/H2) y de una LISTA (P2).

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::rust;
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

fn juzgar(src: &str) -> (Laid, Vec<(String, u32)>, String) {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc).unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let l = lay(&c);
    let v = judge(&l, &c);
    let txt = render("x.maqueta", src.as_bytes(), &v).split_whitespace().collect::<Vec<_>>().join(" ");
    (l, doc.datos, if v.is_empty() { String::new() } else { txt })
}

const AMIGO: &str = include_str!("../../pruebas/amigo.maqueta");

#[test]
fn una_pieza_con_datos_sale_con_su_struct_y_su_muestra() {
    let (l, colores, v) = juzgar(AMIGO);
    assert!(v.is_empty(), "{v}");
    let g = rust::modulo_con_datos("pruebas/amigo.maqueta", &l, &colores);
    assert!(g.contains("pub struct Datos<'a> {"), "{g}");
    assert!(g.contains("pub nombre: &'a [u8],") && g.contains("pub color: u32,"));
    assert!(g.contains("pub const MUESTRA: Datos<'static> = Datos { nombre: b\"Ana Lopez\""));
    assert!(g.contains("pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32, d: &Datos)"));
}

#[test]
fn el_texto_del_dato_se_corta_a_su_caja_y_el_color_es_el_dato() {
    let (l, colores, _) = juzgar(AMIGO);
    let g = rust::modulo_con_datos("pruebas/amigo.maqueta", &l, &colores);
    assert!(g.contains("p.pieza_cabe(&bmo::Pieza::Letra {") && g.contains("texto: d.nombre"), "{g}");
    assert!(g.contains(", 170, ox as i32"), "el ancho de su caja: {g}");
    assert!(g.contains("c: d.color"), "{g}");
    // La muestra sale en `MUESTRA` y en ningun sitio de lo que se pinta.
    let pintar = &g[g.find("pub fn pintar(").expect("pintar")..];
    assert!(!pintar.contains("4DE38E"), "la muestra no queda en lo que se pinta: {pintar}");
}

#[test]
fn un_color_de_dato_con_alfa_conserva_su_fuerza() {
    let src = "<maqueta class=\"m\"></maqueta><style>:root { --dato-c: #4DE38E } \
.m { width:40px; height:40px; border-radius:8px; background-color:#101010; box-shadow:0 0 6px var(--dato-c) }</style>";
    let (l, colores, v) = juzgar(src);
    assert!(v.is_empty(), "{v}");
    let g = rust::modulo_con_datos("x.maqueta", &l, &colores);
    assert!(g.contains("| d.c)"), "el resplandor lleva alfa: {g}");
}

#[test]
fn un_hueco_sin_ancho_ni_letra_se_explica() {
    let (_, _, v) = juzgar("<maqueta class=\"m\"><span class=\"t\">{nombre|Ana}</span></maqueta>\
<style>.m{width:200px; height:40px} .t{color:#FFFFFF}</style>");
    assert!(v.contains("el hueco `{nombre}` necesita un `width`"), "{v}");
    assert!(v.contains("un `font-size`"), "{v}");
}

#[test]
fn sin_datos_el_modulo_de_siempre() {
    let (l, colores, _) = juzgar(include_str!("../../pruebas/calc.maqueta"));
    assert!(colores.is_empty());
    assert_eq!(rust::modulo_con_datos("c.maqueta", &l, &colores), rust::modulo("c.maqueta", &l));
    assert!(!rust::modulo("c.maqueta", &l).contains("Datos"));
}
