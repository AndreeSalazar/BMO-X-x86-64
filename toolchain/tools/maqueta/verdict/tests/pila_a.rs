//! **Las comprobaciones M a S** (MAQUETA 3, pila A, MA2): cada una con el caso
//! que caza y el que deja pasar.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_layout::lay;
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

fn veredicto(src: &str) -> String {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e)));
    render("t.maqueta", src.as_bytes(), &judge(&lay(&c), &c)).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn limpio(src: &str) {
    let v = veredicto(src);
    assert!(v.is_empty(), "esto tenia que salir limpio:\n{v}");
}

#[test]
fn el_fichero_de_la_pila_a_sale_limpio() {
    limpio(include_str!("../../pruebas/pila_a.maqueta"));
}

#[test]
fn m_lo_que_un_overflow_recorta_puede_salirse_y_sin_el_no() {
    let src = |o: &str| format!("<maqueta><div class=\"p\"><div class=\"h\"></div></div></maqueta><style>.p{{width:50px;height:20px{o}}} .h{{width:90px;height:20px}}</style>");
    assert!(veredicto(&src("")).contains("se sale de su padre"));
    limpio(&src(";overflow:hidden"));
    limpio(&src(";overflow-x:clip"));
}

#[test]
fn m_el_texto_de_pixel_no_se_corta_a_medias() {
    let v = veredicto("<maqueta><div class=\"p\"><span class=\"t\">largo largo</span></div></maqueta>\
                       <style>.p{width:40px;height:16px;overflow:hidden} .t{width:88px;color:#FFFFFF}</style>");
    assert!(v.contains("texto de pixel quedaria cortado"), "{v}");
}

#[test]
fn m_una_absoluta_anclada_fuera_del_recorte() {
    let src = |pos: &str| format!("<maqueta><div class=\"p\"><div class=\"a\"></div></div></maqueta>\
        <style>.p{{width:50px;height:50px;overflow:hidden{pos}}} .a{{position:absolute;left:10px;top:10px;width:5px;height:5px}}</style>");
    assert!(veredicto(&src("")).contains("se ancla fuera de la caja que la recorta"));
    limpio(&src(";position:relative"));
}

#[test]
fn m_un_eje_hidden_con_el_otro_visible_se_desplazaria() {
    let v = veredicto("<maqueta><div class=\"p\"><div class=\"h\"></div></div></maqueta>\
                       <style>.p{width:50px;height:20px;overflow-x:hidden} .h{width:40px;height:30px}</style>");
    assert!(v.contains("el otro eje se desplazaria"), "{v}");
}

#[test]
fn n_los_puntos_piden_recorte_y_una_linea() {
    let src = |css: &str| format!("<maqueta><span class=\"n\">Kepler</span></maqueta><style>.n{{width:80px;font-size:14px;line-height:20px;color:#FFFFFF;text-overflow:ellipsis{css}}}</style>");
    assert!(veredicto(&src(";overflow:hidden")).contains("`white-space: nowrap`"));
    assert!(veredicto(&src(";white-space:nowrap")).contains("aqui no corta nada"));
    limpio(&src(";overflow:hidden;white-space:nowrap"));
}

#[test]
fn o_una_mano_sin_nombre() {
    let src = |id: &str| format!("<maqueta><div class=\"b\"{id}></div></maqueta><style>.b{{width:20px;height:20px;cursor:pointer}}</style>");
    assert!(veredicto(&src("")).contains("`cursor: pointer` en una caja sin `id`"));
    limpio(&src(" id=\"ok\""));
}

#[test]
fn p_la_capa_solo_en_una_posicionada_y_nunca_una_dentro_de_otra() {
    assert!(veredicto("<maqueta><div class=\"a\"></div></maqueta><style>.a{width:5px;height:5px;z-index:2}</style>").contains("`z-index` en una caja estatica"));
    let v = veredicto("<maqueta><div class=\"a\"><div class=\"b\"></div></div></maqueta>\
                       <style>.a{position:relative;width:20px;height:20px;z-index:1} .b{position:relative;width:5px;height:5px;z-index:2}</style>");
    assert!(v.contains("una capa dentro de otra"), "{v}");
}

#[test]
fn q_partir_filas_pide_la_medida_y_align_content_pide_partir() {
    assert!(veredicto("<maqueta><div class=\"f\"><div class=\"c\"></div></div></maqueta><style>.f{display:flex;flex-direction:row;flex-wrap:wrap} .c{width:5px;height:5px}</style>")
        .contains("la medida contra la que se parte"));
    assert!(veredicto("<maqueta><div class=\"f\"><div class=\"c\"></div></div></maqueta><style>.f{display:flex;flex-direction:row;align-content:start} .c{width:5px;height:5px}</style>")
        .contains("`align-content` sin `flex-wrap: wrap`"));
}

#[test]
fn r_la_proporcion_no_pelea_con_dos_medidas_ni_con_stretch() {
    assert!(veredicto("<maqueta><div class=\"a\"></div></maqueta><style>.a{width:10px;height:10px;aspect-ratio:2}</style>").contains("con `width` y `height` dichos"));
    let src = |al: &str| format!("<maqueta><div class=\"f\"><div class=\"a\"></div></div></maqueta><style>.f{{display:flex;flex-direction:row{al}}} .a{{width:20px;aspect-ratio:2}}</style>");
    assert!(veredicto(&src("")).contains("en un hijo flex que se estiraria"));
    limpio(&src(";align-items:start"));
}

#[test]
fn s_el_contorno_tiene_que_caber_en_el_lienzo() {
    let src = |pad: &str| format!("<maqueta><div class=\"p\"><div class=\"b\"></div></div></maqueta><style>.p{{padding:{pad}}} .b{{width:20px;height:20px;outline:2px solid #5EF2E6;outline-offset:2px}}</style>");
    assert!(veredicto(&src("2px")).contains("el contorno se sale del lienzo"));
    limpio(&src("4px"));
}
