//! **La pila A de MAQUETA 3 en los emisores (MA2)**: el recorte de un
//! `overflow`, las capas, el contorno, las zonas del puntero y los golpes.

use bmo_maqueta_emit::orden::{self, Trazo};
use bmo_maqueta_emit::{bef, rust};
use bmo_maqueta_layout::{Laid, Rect};

fn pila() -> Laid {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pruebas/pila_a.maqueta");
    bmo_maqueta_compone::compilar(&ruta).unwrap_or_else(|f| panic!("{}", f.render()))
}

fn de_texto(src: &str) -> Laid {
    let doc = bmo_maqueta_node::parse(src.as_bytes()).unwrap();
    bmo_maqueta_layout::lay(&bmo_maqueta_cascade::cascade(&doc).unwrap())
}

#[test]
fn lo_que_el_overflow_corta_a_medias_lleva_su_recorte_y_lo_demas_no() {
    let ordenes = orden::lista(&pila());
    let con: Vec<_> = ordenes.iter().filter(|o| o.recorte.is_some()).collect();
    // La caja grande (redonda, se sale por la derecha) y la insignia (se sale
    // por arriba a la izquierda): las dos recortadas al marco, por dentro. Y
    // el nombre con sus `...`: la tinta de la letra asoma un pelo de lo que
    // mide, y su propia caja la recorta (como CSS).
    assert_eq!(con.len(), 3, "{con:#?}");
    assert!(con[..2].iter().all(|o| matches!(o.trazo, Trazo::Caja { .. }) && o.recorte == Some(Rect { x: 20, y: 20, w: 220, h: 60 })));
    assert!(matches!(&con[2].trazo, Trazo::Letra { texto, .. } if texto.ends_with("...")));
    assert_eq!(con[2].recorte, Some(Rect { x: 20, y: 94, w: 140, h: 20 }));
}

#[test]
fn un_rect_que_se_sale_se_corta_exacto_al_compilar() {
    let l = de_texto("<maqueta><div class=\"p\"><div class=\"h\"></div></div></maqueta>\
                      <style>.p{width:50px;height:20px;overflow:hidden} .h{width:90px;height:20px;background-color:#FF0000}</style>");
    let o = orden::lista(&l);
    let rojo = o.iter().find(|o| matches!(o.trazo, Trazo::Rect { color: 0xFF0000, .. })).unwrap();
    assert_eq!(rojo.trazo, Trazo::Rect { r: Rect { x: 0, y: 0, w: 50, h: 20 }, color: 0xFF0000 });
    assert_eq!(rojo.recorte, None, "un rect no necesita que el pintor recorte");
}

#[test]
fn la_capa_mas_alta_se_pinta_despues_aunque_se_escribiera_antes() {
    let o = orden::lista(&pila());
    let k = |c: u32| o.iter().position(|o| matches!(o.trazo, Trazo::Rect { color, .. } if color == c)).unwrap();
    assert!(k(0x00FF2E88) > k(0x007EE787), "la rosa (z-index 2) encima de la verde (1)");
}

#[test]
fn el_contorno_sale_al_final_redondo_o_en_cuatro_rects() {
    let o = orden::lista(&pila());
    let n = o.len();
    // El del boton (radio 8 + 2 de separacion + 2 de grosor) y los cuatro del plano.
    assert!(matches!(o[n - 5].trazo, Trazo::Borde { radio: 12, grosor: 2, color: 0x5EF2E6, .. }), "{:?}", o[n - 5].trazo);
    assert!(o[n - 4..].iter().all(|o| matches!(o.trazo, Trazo::Rect { color: 0xFFD45E, .. })));
}

#[test]
fn el_modulo_recorta_y_dice_la_forma_del_puntero() {
    let m = rust::modulo("pruebas/pila_a.maqueta", &pila());
    assert!(m.contains("Some(Recorte::nuevo(ox as i32 + 20, oy as i32 + 20, 220, 60))"), "pintar, recortado al marco");
    assert!(m.contains("Some(Recorte::nuevo(ox as i32 + 20, oy as i32 + 20, 220, 60).interseccion(&limite))"), "y pintar_en, con su limite");
    assert!(m.contains("pub fn puntero(ox: u32, oy: u32, px: u32, py: u32) -> &'static str {"));
    assert!(m.contains("\"pointer\"),"));
    // Sin `cursor`, el modulo de siempre.
    let calc = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pruebas/calc.maqueta")).unwrap();
    assert!(!rust::modulo("c", &de_texto(&calc)).contains("fn puntero"));
}

#[test]
fn pointer_events_none_saca_la_caja_y_lo_de_dentro_de_los_golpes() {
    let l = de_texto("<maqueta><div class=\"p\"><div class=\"b\" id=\"dentro\"></div></div><div class=\"b\" id=\"fuera\"></div></maqueta>\
                      <style>.p{width:20px;height:20px;pointer-events:none} .b{width:10px;height:10px}</style>");
    let g: Vec<_> = orden::golpes(&l).into_iter().map(|g| g.nombre).collect();
    assert_eq!(g, vec!["#fuera".to_string()]);
}

#[test]
fn la_cara_que_viaja_no_lleva_recortes_todavia_y_lo_dice() {
    let l = pila();
    let r = bef::escribir(&orden::lista(&l), &orden::golpes(&l), l.canvas.0 as i64, l.canvas.1 as i64);
    assert!(matches!(r, Err(bef::NoCabe::Recorte { .. })), "{r:?}");
}

/// HM2 (07-10): las piezas comunes del HUD, cada una sola y las cuatro juntas.
#[test]
fn las_piezas_del_hud_compilan_y_se_juzgan_limpias() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pruebas");
    for f in ["hud/marco.maqueta", "hud/lectura.maqueta", "hud/regla.maqueta", "hud/barra.maqueta", "hud.maqueta"] {
        bmo_maqueta_compone::compilar(&dir.join(f)).unwrap_or_else(|e| panic!("{f}: {}", e.render()));
    }
}

/// [!] (07-10) Un borde sin fondo es un ANILLO: por dentro se ve lo de detras,
/// como en CSS. Antes salia una caja maciza del color del borde.
#[test]
fn un_borde_sin_fondo_pinta_solo_el_anillo() {
    let l = de_texto("<maqueta><div class=\"f\"><div class=\"p\"></div></div></maqueta>\
                      <style>.f{padding:4px;background-color:#000000} .p{width:20px;height:4px;border-width:1px;border-color:#2B2250}</style>");
    let foto = bmo_maqueta_emit::foto::foto(&l);
    let px = |x: u32, y: u32| foto.px[(y * foto.ancho + x) as usize] & 0x00FF_FFFF;
    assert_eq!(px(4, 4), 0x2B2250, "el borde");
    assert_eq!(px(10, 7), 0x000000, "y por dentro, el fondo de detras");
}
