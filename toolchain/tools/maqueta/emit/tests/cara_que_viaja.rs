//! **La cara de la calculadora, emitida y LEIDA DE VUELTA.**
//!
//! El escalon 8 de `PLAN_MAQUETA.md` / 2 de `PLAN_LA_CARA_VIAJA.md`, comprobado
//! del unico modo que prueba algo: **el emisor escribe y el LECTOR abre**, y el
//! lector es el mismo codigo que correra en Ring 3.
//!
//! ## Por que la ida y vuelta y no unos bytes dorados
//!
//! Un fichero dorado dice *"salio lo mismo que la ultima vez"*, que es util para
//! cazar cambios y **no dice si lo que sale se puede leer**. La ida y vuelta
//! contesta la pregunta que importa: si el compositor va a poder pintar esto.
//!
//! [!] Y hay un limite que hay que decir: **las dos mitades las escribio el
//! mismo lado.** Que el lector acepte lo que el emisor produce no demuestra que
//! el formato sea bueno, demuestra que son consistentes. La prueba de verdad
//! llega cuando lo lea el escritorio en el Ryzen -- escalon 3.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::{bef, orden};
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

use bmo_maqueta_cara as cara;

const CALC: &str = include_str!("../../pruebas/calc.maqueta");

/// La cadena entera, veredicto incluido. Un emisor que aceptara una maquetacion
/// que el juez rechaza estaria emitiendo el fallo.
fn compilar(src: &str) -> Laid {
    let doc = parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc)
        .unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let l = lay(&c);
    let v = judge(&l, &c);
    assert!(v.is_empty(), "{}", render("x.maqueta", src.as_bytes(), &v));
    l
}

/// El lienzo de la calculadora, tomado de la caja raiz.
fn lienzo(l: &Laid) -> (i64, i64) {
    let r = l.all().first().expect("algo tiene que haber").rect;
    (r.w as i64, r.h as i64)
}

fn cara_de_la_calculadora() -> (Vec<u8>, usize, usize) {
    let l = compilar(CALC);
    let ordenes = orden::lista(&l);
    let golpes = orden::golpes(&l);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &golpes, w, h).expect("la calculadora tiene que caber");
    (bytes, ordenes.len(), golpes.len())
}

/// *** LA PRUEBA ENTERA EN UNA: se escribe, se abre, y lo que hay dentro es lo
/// que se metio.
#[test]
fn la_cara_de_la_calculadora_va_y_vuelve() {
    let (bytes, n_ordenes, n_golpes) = cara_de_la_calculadora();

    let c = cara::leer(&bytes, 1920, 1080).expect("el lector tiene que abrirla");
    assert_eq!(c.trazos(), n_ordenes, "no se perdio ni se invento un trazo");
    assert_eq!(c.golpes(), n_golpes, "ni un golpe");

    // Y el contenido, no solo las cuentas: un formato puede cuadrar en numero de
    // registros y traerlos todos a cero.
    let l = compilar(CALC);
    let ordenes = orden::lista(&l);
    for (i, o) in ordenes.iter().enumerate() {
        let p = c.trazo(i).expect("el trazo tiene que estar");
        let r = o.trazo.area();
        assert_eq!(
            (p.x as i64, p.y as i64, p.w as i64, p.h as i64),
            (r.x as i64, r.y as i64, r.w as i64, r.h as i64),
            "el trazo {i} (de {}) cambio de sitio al viajar",
            o.de
        );
        match &o.trazo {
            orden::Trazo::Rect { color, .. } => {
                assert_eq!(p.clase, cara::CLASE_RECT);
                assert_eq!(p.color, *color);
            }
            orden::Trazo::Texto { texto, color, .. } => {
                assert_eq!(p.clase, cara::CLASE_TEXTO);
                assert_eq!(p.color, *color);
                assert_eq!(p.texto, texto.as_bytes(), "las letras del trazo {i}");
            }
            otro => panic!("la calculadora solo tiene rect y letra de pixel, y salio {otro:?}"),
        }
    }
}

/// Los nombres de los golpes llegan enteros, que es lo unico que el programa
/// recibe cuando alguien pulsa. Si esto se rompiera, los botones existirian y no
/// se sabria cual es cual.
#[test]
fn los_botones_llegan_con_su_nombre() {
    let l = compilar(CALC);
    let golpes = orden::golpes(&l);
    let ordenes = orden::lista(&l);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &golpes, w, h).unwrap();
    let c = cara::leer(&bytes, 1920, 1080).unwrap();

    assert!(c.golpes() > 0, "la calculadora tiene botones");
    for (i, g) in golpes.iter().enumerate() {
        let p = c.golpe(i).unwrap();
        assert_eq!(p.nombre, g.nombre.as_bytes(), "el golpe {i}");
        assert_eq!((p.x as i64, p.w as i64), (g.r.x as i64, g.r.w as i64));
    }
}

/// **El medida, medido y no estimado.**
///
/// `PLAN_LA_CARA_VIAJA.md` seccion 3 predijo **~950 bytes** para esta cara,
/// contando a mano sobre la calculadora ya compilada. Esta prueba lo mide de
/// verdad y deja el numero escrito.
///
/// * El tope es generoso a proposito --no es un presupuesto, es un despertador--:
/// lo que tiene que saltar es que la cara se ponga de decenas de KiB, que
/// significaria que dejo de ser "el resultado" y volvio a ser "el documento".
#[test]
fn una_cara_entera_cabe_en_pocos_kilobytes() {
    let (bytes, trazos, golpes) = cara_de_la_calculadora();
    let n = bytes.len();
    std::println!("la cara de la calculadora: {n} B  ({trazos} trazos, {golpes} golpes)");
    assert!(
        n < 8 * 1024,
        "una cara son datos, no un documento: {n} B es demasiado"
    );
    // Y que no sea absurdamente chica, que seria la signal de que se emitio
    // vacia y las cuentas cuadran solas.
    assert!(n > cara::CABECERA, "no puede ser solo la cabecera");
}

/// **Un lienzo que no cabe en la pantalla se rechaza al LEER, no al escribir.**
///
/// Es la comprobacion 5, y prueba la separacion que sostiene el esquema: el
/// emisor corre en el anfitrion y **no sabe** en que pantalla se pintara. El que
/// lo sabe es el lector, y por eso la pantalla es un parametro suyo.
#[test]
fn la_pantalla_la_pone_quien_lee_y_no_quien_escribe() {
    let (bytes, _, _) = cara_de_la_calculadora();
    assert!(cara::leer(&bytes, 1920, 1080).is_ok());
    assert_eq!(
        cara::leer(&bytes, 32, 32).unwrap_err(),
        cara::Falta::LienzoMasGrandeQueLaPantalla
    );
}

/// **Un byte cambiado y el lector lo dice o lo aguanta, pero no estalla.**
///
/// Se corrompe la cara de la calculadora byte a byte. Es la misma prueba que le
/// hace `bmo-bex-gate` a su cabecera, y esta aqui por el mismo motivo: en Ring 3
/// un panico del compositor no es un test rojo, **es el escritorio caido**.
#[test]
fn la_cara_de_la_calculadora_corrompida_no_tumba_al_lector() {
    let (base, _, _) = cara_de_la_calculadora();
    for i in 0..base.len() {
        for v in [0x00u8, 0xFF] {
            let mut b = base.clone();
            if b[i] == v {
                continue;
            }
            b[i] = v;
            let _ = cara::leer(&b, 1920, 1080);
        }
    }
}

// ---------------------------------------------------------------------------
// MAQUETA 2 (04-10): la cara SUAVE -- radio, resplandor, degradado, la letra
// de la casa y un camino de SVG -- tambien viaja sin perder un pixel.
// ---------------------------------------------------------------------------

const TARJETA: &str = include_str!("../../pruebas/tarjeta.maqueta");

#[test]
fn la_tarjeta_trae_todas_las_piezas_suaves() {
    let l = compilar(TARJETA);
    let o = orden::lista(&l);
    let hay = |f: fn(&orden::Trazo) -> bool| o.iter().any(|x| f(&x.trazo));
    assert!(hay(|t| matches!(t, orden::Trazo::Caja { .. })), "caja redonda");
    assert!(hay(|t| matches!(t, orden::Trazo::Borde { .. })), "borde redondo");
    assert!(hay(|t| matches!(t, orden::Trazo::Resplandor { .. })), "resplandor");
    assert!(hay(|t| matches!(t, orden::Trazo::Degradado { .. })), "degradado");
    assert!(hay(|t| matches!(t, orden::Trazo::Letra { mayusculas: true, .. })), "el rotulo");
    assert!(hay(|t| matches!(t, orden::Trazo::Linea { .. })), "el icono");
}

#[test]
fn la_cara_suave_que_viaja_pinta_los_mismos_pixeles() {
    let l = compilar(TARJETA);
    let ordenes = orden::lista(&l);
    let golpes = orden::golpes(&l);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &golpes, w, h).expect("la tarjeta tiene que caber");
    let directa = bmo_maqueta_emit::foto::foto(&l);
    let viajada = bmo_maqueta_emit::foto::foto_cara(&bytes).expect("la cara escrita se tiene que leer");
    assert_eq!((directa.ancho, directa.alto), (viajada.ancho, viajada.alto));
    let distintos = directa.px.iter().zip(&viajada.px).filter(|(a, b)| a != b).count();
    assert_eq!(distintos, 0, "la cara perdio {distintos} pixeles al viajar");
    // Y no es una foto negra: hay oro (la cifra) y verde (el boton).
    assert!(directa.px.iter().any(|&c| c == 0x00FF_D45E));
    assert!(directa.px.iter().any(|&c| c == 0x004D_E38F));
}

#[test]
fn una_cara_suave_corrompida_no_tumba_al_lector() {
    let l = compilar(TARJETA);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&orden::lista(&l), &orden::golpes(&l), w, h).unwrap();
    // Cada byte, cambiado de tres formas: el lector dice que no o lee, pero
    // nunca revienta, y lo que lee se pinta sin escribir fuera.
    for i in 0..bytes.len() {
        for v in [0u8, 0xFF, bytes[i] ^ 0x5A] {
            let mut b = bytes.clone();
            b[i] = v;
            let _ = cara::leer(&b, u16::MAX, u16::MAX);
            // Pintarla entera cada vez es caro: una de cada 37 se pinta.
            if i % 37 == 0 {
                let _ = bmo_maqueta_emit::foto::foto_cara(&b);
            }
        }
    }
}

// ------------------------------------------------------------------------
//  MAQUETA 3 (06-10): los dibujos de SVG viajan con su tinta
// ------------------------------------------------------------------------

/// `pruebas/dibujos.maqueta`, por `compone` (lee su `<svg src>`).
fn dibujos() -> Laid {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pruebas/dibujos.maqueta");
    bmo_maqueta_compone::compilar(&ruta).unwrap_or_else(|f| panic!("{}", f.render()))
}

#[test]
fn los_dibujos_viajan_y_pintan_los_mismos_pixeles() {
    let l = dibujos();
    let ordenes = orden::lista(&l);
    assert!(ordenes.iter().any(|o| matches!(o.trazo, orden::Trazo::Figura { .. })), "hay figuras de MAQUETA 3");
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &orden::golpes(&l), w, h).expect("los dibujos tienen que caber");
    let c = cara::leer(&bytes, 1920, 1080).expect("se lee");
    assert!((0..c.trazos()).any(|i| c.trazo(i).unwrap().clase == cara::CLASE_FIGURA));
    let directa = bmo_maqueta_emit::foto::foto(&l);
    let viajada = bmo_maqueta_emit::foto::foto_cara(&bytes).expect("la cara escrita se tiene que leer");
    let distintos = directa.px.iter().zip(&viajada.px).filter(|(a, b)| a != b).count();
    assert_eq!(distintos, 0, "los dibujos perdieron {distintos} pixeles al viajar");
    // El degradado de la rueda: hay cian y magenta mezclados, no un color.
    let colores: std::collections::HashSet<u32> = directa.px.iter().copied().collect();
    assert!(colores.len() > 200, "con degradados hay muchos colores: {}", colores.len());
}

#[test]
fn unos_dibujos_corrompidos_no_tumban_al_lector() {
    let l = dibujos();
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&orden::lista(&l), &orden::golpes(&l), w, h).unwrap();
    for i in (0..bytes.len()).step_by(3) {
        for v in [0u8, 0xFF, bytes[i] ^ 0x5A] {
            let mut b = bytes.clone();
            b[i] = v;
            if cara::leer(&b, u16::MAX, u16::MAX).is_ok() && i % 41 == 0 {
                let _ = bmo_maqueta_emit::foto::foto_cara(&b);
            }
        }
    }
}
