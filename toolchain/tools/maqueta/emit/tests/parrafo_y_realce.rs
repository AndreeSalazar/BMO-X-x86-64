//! H3 (el parrafo, partido al compilar) y H8 (el realce con su transicion).

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::rust;
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

const NOTA: &str = include_str!("../../pruebas/nota.maqueta");

#[test]
fn un_parrafo_mide_sus_lineas_y_sale_una_letra_por_linea() {
    let (l, v) = juzgar(NOTA);
    assert!(v.is_empty(), "{v}");
    let t = l.all().into_iter().find(|f| f.style.parrafo).expect("el parrafo");
    assert_eq!(t.rect.h, 5 * 20, "cinco lineas de 20");
    let g = rust::modulo("nota.maqueta", &l);
    let pintar = &g[g.find("pub fn pintar(").expect("pintar")..g.find("pub fn pintar_en(").expect("pintar_en")];
    assert_eq!(pintar.matches("bmo::Pieza::Letra").count(), 1 + 5, "el rotulo y las cinco lineas: {pintar}");
}

#[test]
fn un_parrafo_sin_ancho_no_se_puede_partir() {
    let (_, v) = juzgar(&NOTA.replace("width:206px; ", ""));
    assert!(v.contains("este parrafo necesita un `width`"), "{v}");
}

const BOTON: &str = "<maqueta class=\"m\"><div class=\"b\" id=\"b\">Hola</div></maqueta>\
<style>.m{display:flex; padding:10px; background-color:#000000} \
.b{padding:8px; border-radius:8px; background-color:#202020; font-size:14px; line-height:18px; color:#FFFFFF; transition:200ms ease-out} \
.b:hover{background-color:#4DE38E}</style>";

#[test]
fn un_realce_con_transition_sale_mezclado() {
    let (l, v) = juzgar(BOTON);
    assert!(v.is_empty(), "{v}");
    let g = rust::modulo("b.maqueta", &l);
    assert!(g.contains("\"b\" => 200,"), "{g}");
    assert!(g.contains("pub fn realce_en(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str, ms: u32, sale: bool)"), "{g}");
    assert!(g.contains("p.pieza_entre(&bmo::Pieza::Caja"), "{g}");
}

#[test]
fn sin_transition_no_sale_realce_animado() {
    let (l, _) = juzgar(&BOTON.replace(" transition:200ms ease-out", ""));
    assert!(!rust::modulo("b.maqueta", &l).contains("realce_en"));
}
