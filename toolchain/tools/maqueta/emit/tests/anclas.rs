//! H5 (`position: relative`, `right`/`bottom`) y H6 (`border-radius: 50%`).

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
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

const CARA: &str = "<maqueta class=\"m\"><div class=\"cara\"><div class=\"punto\"></div></div></maqueta>\
<style>.m{display:flex; padding:20px; background-color:#000000} \
.cara{position:relative; width:40px; height:40px; border-radius:50%; background-color:#4DE38E} \
.punto{position:absolute; right:0; bottom:0; width:10px; height:10px; border-radius:50%; background-color:#FFFFFF}</style>";

#[test]
fn una_absoluta_se_ancla_a_su_relative_por_abajo_a_la_derecha() {
    let (l, v) = juzgar(CARA);
    assert!(v.is_empty(), "{v}");
    let punto = l.all().into_iter().find(|f| f.rect.w == 10).expect("el punto");
    assert_eq!((punto.rect.x, punto.rect.y), (20 + 40 - 10, 20 + 40 - 10));
}

#[test]
fn el_cincuenta_por_ciento_es_la_mitad_del_lado_corto() {
    let (l, _) = juzgar(CARA);
    let radios: Vec<u32> = l.all().iter().filter(|f| f.rect.w == 40 || f.rect.w == 10).map(|f| f.style.border_radius).collect();
    assert_eq!(radios, vec![20, 5]);
}

#[test]
fn una_relative_no_se_corre() {
    let (_, v) = juzgar(&CARA.replace("position:relative;", "position:relative; top:4px;"));
    assert!(v.contains("una caja `relative` no se corre"), "{v}");
}

#[test]
fn otro_porcentaje_sigue_sin_existir() {
    let src = CARA.replace("border-radius:50%; background-color:#4DE38E", "border-radius:25%; background-color:#4DE38E");
    let e = match parse(src.as_bytes()) {
        Ok(_) => panic!("25% no vale"),
        Err(e) => render("x.maqueta", src.as_bytes(), &e),
    };
    assert!(e.contains("unidad no soportada"), "{e}");
}
