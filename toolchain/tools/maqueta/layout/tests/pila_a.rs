//! **La pila A de MAQUETA 3, segunda tanda (MA2)**: donde caen las cajas con
//! filas partidas, proporcion, `align-self`, desplazamientos negativos y puntos.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_layout::{lay, Laid, Rect};
use bmo_maqueta_node::parse;

fn run(src: &str) -> Laid {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e)));
    lay(&cascade(&doc).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e))))
}

fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn las_filas_se_parten_contra_el_ancho_propio() {
    // 60 + 8 + 60 + 8 + 60 = 196 <= 200: tres por fila, y el gap entre filas.
    let l = run("<maqueta><div class=\"f\"><div class=\"c\"></div><div class=\"c\"></div><div class=\"c\"></div><div class=\"c\"></div><div class=\"c\"></div></div></maqueta>\
                 <style>.f{display:flex;flex-direction:row;flex-wrap:wrap;gap:8px;width:200px} .c{width:60px;height:24px}</style>");
    let f = &l.root.children[0];
    assert_eq!(f.rect, r(0, 0, 200, 56), "dos filas de 24 y un gap");
    assert_eq!(f.children[2].rect, r(136, 0, 60, 24));
    assert_eq!(f.children[3].rect, r(0, 32, 60, 24), "la cuarta abre la segunda fila");
}

#[test]
fn align_content_reparte_las_filas_y_stretch_les_da_lo_que_sobra() {
    let src = |ac: &str| {
        format!("<maqueta><div class=\"f\"><div class=\"c\"></div><div class=\"c\"></div></div></maqueta>\
                 <style>.f{{display:flex;flex-direction:row;flex-wrap:wrap;width:60px;height:100px;align-items:start{ac}}} .c{{width:60px;height:20px}}</style>")
    };
    let l = run(&src(""));
    assert_eq!(l.root.children[0].children[1].rect.y, 50, "normal = stretch: cada fila mide 50");
    let l = run(&src(";align-content:end"));
    assert_eq!(l.root.children[0].children[0].rect.y, 60);
    let l = run(&src(";align-content:space-between"));
    assert_eq!(l.root.children[0].children[1].rect.y, 80);
}

#[test]
fn la_proporcion_saca_el_alto_del_ancho_dicho_o_del_que_llena() {
    let l = run("<maqueta><div class=\"a\"></div></maqueta><style>.a{width:120px;aspect-ratio:16 / 9}</style>");
    assert_eq!(l.root.children[0].rect, r(0, 0, 120, 68), "120 * 9 / 16 = 67,5 -> 68");
    let l = run("<maqueta><div class=\"p\"><div class=\"a\"></div></div></maqueta><style>.p{width:100px} .a{aspect-ratio:2}</style>");
    assert_eq!(l.root.children[0].children[0].rect, r(0, 0, 100, 50), "llena el ancho del padre y el alto sale de el");
    let l = run("<maqueta><div class=\"a\"></div></maqueta><style>.a{height:30px;aspect-ratio:3/2}</style>");
    assert_eq!(l.root.children[0].rect.w, 45);
}

#[test]
fn align_self_mueve_un_hijo_y_no_a_sus_hermanos() {
    let l = run("<maqueta><div class=\"f\"><div class=\"c\"></div><div class=\"c m\"></div></div></maqueta>\
                 <style>.f{display:flex;flex-direction:row;align-items:start;height:40px} .c{width:10px;height:10px} .m{align-self:end}</style>");
    assert_eq!(l.root.children[0].children[0].rect.y, 0);
    assert_eq!(l.root.children[0].children[1].rect.y, 30);
}

#[test]
fn una_absoluta_se_sale_de_su_ancla_con_negativos() {
    let l = run("<maqueta><div class=\"p\"><div class=\"i\"></div></div></maqueta>\
                 <style>.p{position:relative;width:50px;height:50px;padding:10px} .i{position:absolute;left:-8px;top:-4px;width:6px;height:6px}</style>");
    assert_eq!(l.root.children[0].children[0].rect, r(-8, -4, 6, 6));
    let l = run("<maqueta><div class=\"p\"><div class=\"i\"></div></div></maqueta>\
                 <style>.p{position:relative;width:50px;height:50px} .i{position:absolute;right:-2px;bottom:-2px;width:6px;height:6px}</style>");
    assert_eq!(l.root.children[0].children[0].rect, r(46, 46, 6, 6));
}

#[test]
fn los_puntos_cortan_al_compilar_y_lo_que_queda_cabe() {
    let l = run("<maqueta><span class=\"n\">Kepler-5600 b, el planeta del gato</span></maqueta>\
                 <style>.n{width:140px;font-size:14px;line-height:20px;color:#FFFFFF;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}</style>");
    let f = &l.root.children[0];
    let t = f.text.as_deref().unwrap();
    assert!(t.ends_with("...") && t.starts_with("Kepler"), "{t}");
    assert!(f.text_at.unwrap().w <= 140, "lo cortado cabe: {}", f.text_at.unwrap().w);
}
