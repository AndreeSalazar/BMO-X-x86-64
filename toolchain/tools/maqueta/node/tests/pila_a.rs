//! **La pila A de MAQUETA 3, segunda tanda (MA2)**: lo que el padre aprende a
//! nombrar -- `LA_MAQUETA_EXIGE.md` 3e.

use bmo_maqueta_diag::render;
use bmo_maqueta_node::{parse, Prop, Value};

fn decls(css: &str) -> Vec<(Prop, Value)> {
    let src = format!("<maqueta><div class=\"a\"></div></maqueta><style>.a{{{css}}}</style>");
    match parse(src.as_bytes()) {
        Ok(d) => d.rules[0].decls.iter().map(|d| (d.prop, d.value)).collect(),
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    }
}

fn errs(css: &str) -> String {
    let src = format!("<maqueta><div class=\"a\"></div></maqueta><style>.a{{{css}}}</style>");
    match parse(src.as_bytes()) {
        Ok(_) => panic!("esto tenia que fallar: {css}"),
        Err(e) => render("t.maqueta", src.as_bytes(), &e).split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

#[test]
fn los_desplazamientos_admiten_negativos_y_inset_tambien() {
    assert_eq!(decls("left:-8px;top:0"), vec![(Prop::Left, Value::Signed(-8)), (Prop::Top, Value::Signed(0))]);
    let v = decls("inset:-3px 4px");
    assert_eq!(v[0], (Prop::Top, Value::Signed(-3)));
    assert_eq!(v[1], (Prop::Right, Value::Signed(4)));
    assert_eq!(v[3], (Prop::Left, Value::Signed(4)));
    assert!(errs("left:-1.5px").contains("ENTEROS"));
}

#[test]
fn la_capa_es_un_entero_sin_signo() {
    assert_eq!(decls("z-index:30"), vec![(Prop::ZIndex, Value::Count(30))]);
    assert!(errs("z-index:-1").contains("DEBAJO del fondo de su padre"));
}

#[test]
fn la_proporcion_se_lee_con_y_sin_espacios() {
    assert_eq!(decls("aspect-ratio:16 / 9"), vec![(Prop::AspectRatio, Value::Ratio(16, 9))]);
    assert_eq!(decls("aspect-ratio:16/9"), vec![(Prop::AspectRatio, Value::Ratio(16, 9))]);
    assert_eq!(decls("aspect-ratio:1"), vec![(Prop::AspectRatio, Value::Ratio(1, 1))]);
    assert!(errs("aspect-ratio:1.5").contains("`A / B` o `N`"));
}

#[test]
fn el_contorno_quiere_sus_tres_partes_o_none() {
    assert_eq!(decls("outline:2px solid #5EF2E6"), vec![(Prop::Outline, Value::Outline { w: 2, color: 0x5EF2E6 })]);
    assert_eq!(decls("outline:none"), vec![(Prop::Outline, Value::Outline { w: 0, color: 0 })]);
    assert_eq!(decls("outline-offset:-2px"), vec![(Prop::OutlineOffset, Value::Signed(-2))]);
    assert!(errs("outline:2px dashed #5EF2E6").contains("otra pluma"));
    assert!(errs("outline:2px solid").contains("le falta algo"));
}

#[test]
fn overflow_es_un_atajo_de_los_dos_ejes() {
    use bmo_maqueta_node::Keyword;
    assert_eq!(decls("overflow:hidden"), vec![(Prop::OverflowX, Value::Word(Keyword::Hidden)), (Prop::OverflowY, Value::Word(Keyword::Hidden))]);
    assert_eq!(decls("overflow:clip auto"), vec![(Prop::OverflowX, Value::Word(Keyword::Clip)), (Prop::OverflowY, Value::Word(Keyword::Auto))]);
    assert!(errs("overflow:scroll").contains("no es un valor de `overflow`"));
}

#[test]
fn el_puntero_y_las_filas_tienen_sus_palabras() {
    use bmo_maqueta_node::Keyword;
    assert_eq!(decls("cursor:not-allowed"), vec![(Prop::Cursor, Value::Word(Keyword::NotAllowed))]);
    assert_eq!(decls("pointer-events:none"), vec![(Prop::PointerEvents, Value::Word(Keyword::None))]);
    assert_eq!(decls("flex-wrap:wrap"), vec![(Prop::FlexWrap, Value::Word(Keyword::Wrap))]);
    assert_eq!(decls("align-self:center"), vec![(Prop::AlignSelf, Value::Word(Keyword::Center))]);
    assert!(errs("cursor:grab").contains("no es un valor de `cursor`"));
}
