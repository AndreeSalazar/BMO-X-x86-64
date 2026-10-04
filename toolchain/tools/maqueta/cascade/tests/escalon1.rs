//! **ESCALON 1** (04-10): los atajos, los lados y las variables de `:root`,
//! tal como los escriben las maquetas de `docs/arte/`. Cada prueba dice lo
//! que haria un navegador con el mismo CSS.

use bmo_maqueta_cascade::{cascade, Style};
use bmo_maqueta_diag::render;
use bmo_maqueta_node::parse;

fn one(sheet: &str) -> Style {
    let src = format!("<maqueta><div class=\"a\"></div></maqueta><style>{sheet}</style>");
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc).unwrap_or_else(|e| panic!("{}", render("t.maqueta", src.as_bytes(), &e)));
    c.root.children[0].style
}

/// El mensaje del padre, en una linea.
fn rechazo(sheet: &str) -> String {
    let src = format!("<maqueta><div class=\"a\"></div></maqueta><style>{sheet}</style>");
    let e = parse(src.as_bytes()).err().expect("esto tenia que fallar");
    render("t.maqueta", src.as_bytes(), &e).split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn padding_de_dos_y_tres_valores_como_css() {
    assert_eq!(one(".a{padding:8px 12px}").padding, [8, 12, 8, 12]);
    assert_eq!(one(".a{padding:4px 10px 6px}").padding, [4, 10, 6, 10]);
    assert_eq!(one(".a{padding:0 14px}").padding, [0, 14, 0, 14]);
    assert_eq!(one(".a{padding:1px 2px 3px 4px}").padding, [1, 2, 3, 4]);
}

#[test]
fn un_lado_pisa_solo_su_lado_y_el_atajo_pisa_todos() {
    assert_eq!(one(".a{padding:8px; padding-left:20px}").padding, [8, 8, 8, 20]);
    assert_eq!(one(".a{padding-left:20px; padding:8px}").padding, [8, 8, 8, 8], "el atajo despues pisa el lado");
}

#[test]
fn border_entero() {
    let s = one(".a{border:1px solid #2B2250}");
    assert_eq!(s.border_width, [1; 4]);
    assert_eq!(s.border_color, [Some(0x2B2250); 4]);
    assert_eq!(s.borde_uniforme(), Some((1, Some(0x2B2250))));
}

#[test]
fn la_raya_de_abajo_de_una_fila() {
    let s = one(".a{border-bottom:1px solid #333344}");
    assert_eq!(s.border_width, [0, 0, 1, 0]);
    assert_eq!(s.border_color, [None, None, Some(0x333344), None]);
    assert_eq!(s.borde_uniforme(), None);
}

#[test]
fn border_none_y_cero_quitan_el_grosor() {
    assert_eq!(one(".a{border:2px solid #FFFFFF; border:none}").border_width, [0; 4]);
    assert_eq!(one(".a{border:2px solid #FFFFFF; border-top:0}").border_width, [0, 2, 2, 2]);
}

#[test]
fn un_borde_transparente_ocupa_pero_no_pinta() {
    let s = one(".a{border-bottom:2px solid transparent}");
    assert_eq!(s.border_width, [0, 0, 2, 0]);
    assert_eq!(s.border_color, [None; 4]);
}

#[test]
fn background_pone_las_dos_largas() {
    let s = one(".a{background:#101018}");
    assert_eq!(s.background, Some(0x101018));
    assert_eq!(s.gradient, None);
    let s = one(".a{background:linear-gradient(90deg, #000000, #FFFFFF)}");
    assert_eq!(s.background, None, "como en CSS: el degradado quita el color");
    assert_eq!(s.gradient, Some((0x000000, 0xFFFFFF, false)));
    let s = one(".a{background:#101018; background:none}");
    assert_eq!((s.background, s.gradient), (None, None));
    assert_eq!(one(".a{background-color:#101018; background-color:transparent}").background, None);
}

#[test]
fn variables_de_root() {
    let s = one(":root{--oro:#FFD45E; --hueco:12px} .a{color:var(--oro); padding:var(--hueco) 4px; border:1px solid var(--oro)}");
    assert_eq!(s.color, Some(0xFFD45E));
    assert_eq!(s.padding, [12, 4, 12, 4]);
    assert_eq!(s.border_color, [Some(0xFFD45E); 4]);
}

#[test]
fn una_variable_con_reserva_y_una_que_usa_otra() {
    assert_eq!(one(".a{color:var(--nadie, #E6EDF6)}").color, Some(0xE6EDF6));
    assert_eq!(one(":root{--a:#123456; --b:var(--a)} .a{color:var(--b)}").color, Some(0x123456));
}

#[test]
fn lo_que_se_rechaza_dice_por_que() {
    assert!(rechazo(".a{border:1px dashed #FFFFFF}").contains("solid"));
    assert!(rechazo(".a{border:1px #FFFFFF}").contains("`solid`"), "sin estilo el navegador no lo pinta");
    assert!(rechazo(".a{border:solid #FFFFFF}").contains("grosor"), "sin grosor el navegador pone 3 px");
    assert!(rechazo(".a{border:1px solid}").contains("color"));
    assert!(rechazo(".a{border:1px solid currentColor}").contains("currentColor"));
    assert!(rechazo(".a{color:var(--nadie)}").contains("no esta definida"));
    assert!(rechazo(".a{--c:#FFFFFF}").contains(":root"), "fuera de :root se heredaria");
    assert!(rechazo(":root{width:4px}").contains("solo van variables"));
    assert!(rechazo(":root{--a:var(--b); --b:var(--a)} .a{color:var(--a)}").contains("a si misma"));
    assert!(rechazo(".a{padding:1px 2px 3px 4px 5px}").contains("cuatro"));
    assert!(rechazo(".a{background:#000000 linear-gradient(90deg, #000000, #FFFFFF)}").contains("UNA cosa"));
}

#[test]
fn transition_como_css() {
    let t = one(".a{transition: 240ms ease-in-out}").transicion.expect("dicha");
    assert_eq!((t.ms, t.retraso, t.curva), (240, 0, [420, 0, 580, 1000]));
    let t = one(".a{transition: all .3s cubic-bezier(.68, -.55, .27, 1.55) 50ms}").transicion.unwrap();
    assert_eq!((t.ms, t.retraso, t.curva), (300, 50, [680, -550, 270, 1550]));
    assert_eq!(one(".a{transition: 100ms}").transicion.unwrap().curva, [250, 100, 250, 1000], "sin curva, `ease`, como CSS");
    assert!(rechazo(".a{transition: width 1s}").contains("no es parte"));
    assert!(rechazo(".a{transition: 1s ease, 2s linear}").contains("una sola"));
    assert!(rechazo(".a{transition: 1s cubic-bezier(2, 0, 1, 1)}").contains("de 0 a 1"));
    assert!(rechazo(".a{width:-4px}").contains("negativo"));
}

#[test]
fn los_estados_se_leen_y_se_abren() {
    let src = "<maqueta><div class=\"a\"></div></maqueta><style>.a{width:10px} @estado grande { .a{width:90px} } @estado chico { .a{width:2px} }</style>";
    let doc = parse(src.as_bytes()).unwrap();
    assert_eq!(doc.estados(), vec!["grande".to_string(), "chico".to_string()]);
    let ancho = |e: Option<&str>| cascade(&doc.en_estado(e)).unwrap().root.children[0].style.width;
    assert_eq!(ancho(None), Some(10));
    assert_eq!(ancho(Some("grande")), Some(90));
    assert_eq!(ancho(Some("chico")), Some(2));
    assert!(rechazo("@media x { .a{width:1px} }").contains("@estado"));
    assert!(rechazo("@estado reposo { .a{width:1px} }").contains("no se declara"));
}
