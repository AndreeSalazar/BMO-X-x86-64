//! **El lector de SVG**, contra dibujos escritos a mano: lo que se pinta, lo
//! que se rechaza (con su lista), y la animacion en pasos.

use bmo_maqueta_diag::Span;
use bmo_maqueta_dibujo::{figuras, leer_dentro, leer_fichero, pasos, Herencia, Tinta};

fn ancla() -> Span {
    Span::new(0, 1, 1, 1)
}

fn dentro(svg: &str, vb: [f64; 4]) -> bmo_maqueta_dibujo::Svg {
    let s = svg.as_bytes();
    leer_dentro(s, 0, s.len(), Some(vb), "").unwrap_or_else(|e| panic!("{e:#?}"))
}

fn fichero(svg: &str) -> Result<bmo_maqueta_dibujo::Svg, Vec<bmo_maqueta_diag::Error>> {
    leer_fichero("prueba.svg", svg.as_bytes(), ancla())
}

#[test]
fn un_circulo_cae_en_su_caja_con_su_radio() {
    let s = dentro(r##"<circle cx="12" cy="12" r="10" fill="#5EF2E6"/>"##, [0.0, 0.0, 24.0, 24.0]);
    let (f, e) = figuras(&s, &Herencia::default(), (100.0, 50.0, 48.0, 48.0));
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].tinta, Tinta::Liso(0x5EF2E6));
    assert!(!f[0].par_impar, "SVG rellena `nonzero`");
    for &(x, y) in &f[0].caminos[0] {
        let r = ((x - 124.0).powi(2) + (y - 74.0).powi(2)).sqrt();
        assert!((r - 20.0).abs() < 0.2, "{r}");
    }
}

#[test]
fn sin_fill_rellena_de_negro_como_svg_y_la_maqueta_puede_heredar() {
    let s = dentro(r##"<rect width="10" height="10"/>"##, [0.0, 0.0, 10.0, 10.0]);
    let (f, _) = figuras(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert_eq!(f[0].tinta, Tinta::Liso(0));
    let h = Herencia { fill: Some(None), stroke: Some(Some(0xFFD45E)), stroke_width: Some(2.0), ..Default::default() };
    let (f, _) = figuras(&s, &h, (0.0, 0.0, 10.0, 10.0));
    assert_eq!(f.len(), 1, "solo la pluma");
    assert_eq!(f[0].tinta, Tinta::Liso(0xFFD45E));
}

#[test]
fn la_pluma_redonda_es_la_de_la_casa_y_la_recta_es_su_contorno() {
    let r = dentro(r##"<path d="M2 2 L20 2" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"##, [0.0, 0.0, 24.0, 24.0]);
    let (f, _) = figuras(&r, &Herencia::default(), (0.0, 0.0, 48.0, 48.0));
    assert!((f[0].pluma - 4.0).abs() < 1e-9, "grosor escalado x2: {}", f[0].pluma);
    let b = dentro(r##"<path d="M2 2 L20 2" stroke="#fff" stroke-width="2" fill="none"/>"##, [0.0, 0.0, 24.0, 24.0]);
    let (f, _) = figuras(&b, &Herencia::default(), (0.0, 0.0, 48.0, 48.0));
    assert_eq!(f[0].pluma, 0.0, "punta recta: contorno relleno");
    assert!(!f[0].par_impar);
    let xs: Vec<f64> = f[0].caminos[0].iter().map(|p| p.0).collect();
    assert!((xs.iter().cloned().fold(f64::MAX, f64::min) - 4.0).abs() < 1e-9, "sin punta: empieza donde el camino");
}

#[test]
fn un_fichero_de_illustrator_con_clases_y_degradado() {
    let s = fichero(r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- Generator: Adobe Illustrator -->
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 100 50" width="200" height="100">
  <defs><style>.cls-1{fill:url(#g);}.cls-2{fill:none;stroke:#3da5ff;stroke-miterlimit:10;}</style>
    <linearGradient id="g" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff" stop-opacity=".5"/></linearGradient></defs>
  <title>logo</title>
  <rect class="cls-1" width="100" height="50" data-name="fondo"/>
  <polyline class="cls-2" points="0 0 50 50 100 0"/>
</svg>"##)
    .unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(s.medida, Some((200.0, 100.0)));
    let (f, e) = figuras(&s, &Herencia::default(), (0.0, 0.0, 200.0, 100.0));
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(f.len(), 2);
    match &f[0].tinta {
        Tinta::Lineal { de, a, paradas } => {
            assert!((de.0 - 0.0).abs() < 1e-9 && (a.0 - 200.0).abs() < 1e-9, "{de:?} {a:?}");
            assert_eq!(paradas.len(), 2);
            assert!((paradas[1].alfa - 0.5).abs() < 1e-9);
        }
        t => panic!("{t:?}"),
    }
}

#[test]
fn lo_que_no_se_pinta_se_dice_entero_y_con_su_linea() {
    let e = fichero("<svg viewBox=\"0 0 10 10\">\n<text>hola</text>\n<g filter=\"url(#f)\"><circle r=\"3\"/></g>\n<mask id=\"m\"/>\n<blink/>\n</svg>").unwrap_err();
    let t: Vec<&str> = e.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(e.len(), 4, "{t:#?}");
    assert!(t[0].starts_with("prueba.svg:2:1: `<text>`"), "{t:?}");
    assert!(t.iter().any(|x| x.contains("filter")));
    assert!(t.iter().any(|x| x.contains("<mask>")));
    assert!(t.iter().any(|x| x.contains("<blink>")));
}

#[test]
fn la_opacidad_de_un_grupo_de_dos_es_error_y_la_de_uno_se_mezcla() {
    let uno = dentro(r##"<g opacity=".5"><circle cx="5" cy="5" r="4" fill="red"/></g>"##, [0.0, 0.0, 10.0, 10.0]);
    let (f, e) = figuras(&uno, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert!(e.is_empty());
    assert!((f[0].alfa - 0.5).abs() < 1e-9);
    let dos = dentro(r##"<g opacity=".5"><circle cx="5" cy="5" r="4" fill="red"/><circle cx="6" cy="5" r="4" fill="blue"/></g>"##, [0.0, 0.0, 10.0, 10.0]);
    let (_, e) = figuras(&dos, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert_eq!(e.len(), 1);
    assert!(e[0].title.contains("pinta 2 cosas"), "{}", e[0].title);
}

#[test]
fn use_y_symbol_se_aplanan() {
    let s = dentro(r##"<defs><symbol id="p" viewBox="0 0 2 2"><rect width="2" height="2" fill="lime"/></symbol></defs><use href="#p" x="5" y="5" width="4" height="4"/>"##, [0.0, 0.0, 10.0, 10.0]);
    let (f, e) = figuras(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert!(e.is_empty(), "{e:?}");
    let xs: Vec<f64> = f[0].caminos[0].iter().map(|p| p.0).collect();
    assert_eq!(xs.iter().cloned().fold(f64::MAX, f64::min), 5.0);
    assert_eq!(xs.iter().cloned().fold(f64::MIN, f64::max), 9.0);
}

#[test]
fn el_viewbox_se_encaja_centrado_como_el_navegador() {
    let s = dentro(r##"<rect width="20" height="10" fill="red"/>"##, [0.0, 0.0, 20.0, 10.0]);
    let (f, _) = figuras(&s, &Herencia::default(), (0.0, 0.0, 100.0, 100.0));
    let ys: Vec<f64> = f[0].caminos[0].iter().map(|p| p.1).collect();
    assert_eq!(ys.iter().cloned().fold(f64::MAX, f64::min), 25.0);
    assert_eq!(ys.iter().cloned().fold(f64::MIN, f64::max), 75.0);
}

#[test]
fn el_discontinuo_parte_la_pluma() {
    let s = dentro(r#"<line x1="0" y1="5" x2="10" y2="5" stroke="red" stroke-dasharray="2 3" stroke-linecap="round" stroke-linejoin="round"/>"#, [0.0, 0.0, 10.0, 10.0]);
    let (f, _) = figuras(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert_eq!(f[0].caminos.len(), 2);
}

#[test]
fn un_giro_eterno_sale_en_pasos_de_siete_grados_y_medio() {
    let s = dentro(r##"<rect x="2" y="2" width="6" height="6" fill="red"><animateTransform attributeName="transform" type="rotate" from="0 5 5" to="360 5 5" dur="2s" repeatCount="indefinite"/></rect>"##, [0.0, 0.0, 10.0, 10.0]);
    let p = pasos(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0)).expect("anima").unwrap_or_else(|e| panic!("{e:#?}"));
    assert!(p.repite);
    assert_eq!(p.ciclo_ms, 2000);
    assert!(p.tiempos.len() >= 49, "{}", p.tiempos.len());
    // A un cuarto de vuelta, el cuadrado esta girado 90 grados: la misma caja.
    let k = p.tiempos.iter().position(|&t| t == 500).unwrap();
    let primero = p.figuras[k][0].caminos[0][0];
    assert!((primero.0 - 8.0).abs() < 1e-6 && (primero.1 - 2.0).abs() < 1e-6, "{primero:?}");
}

#[test]
fn keyframes_de_css_con_origen_en_la_figura() {
    let s = fichero(r##"<svg viewBox="0 0 10 10"><style>@keyframes gira { to { transform: rotate(360deg) } } .g { transform-box: fill-box; transform-origin: center; animation: gira 1s linear infinite }</style><rect class="g" x="2" y="2" width="6" height="6" fill="red"/></svg>"##).unwrap_or_else(|e| panic!("{e:#?}"));
    let p = pasos(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0)).expect("anima").unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(p.ciclo_ms, 1000);
    let k = p.tiempos.iter().position(|&t| t == 250).unwrap();
    let primero = p.figuras[k][0].caminos[0][0];
    assert!((primero.0 - 8.0).abs() < 1e-6 && (primero.1 - 2.0).abs() < 1e-6, "{primero:?}");
}

#[test]
fn un_radio_que_late_tiene_los_mismos_puntos_en_cada_paso() {
    let s = dentro(r##"<circle cx="5" cy="5" r="2" fill="red"><animate attributeName="r" values="2;4;2" dur="1s" repeatCount="indefinite"/></circle>"##, [0.0, 0.0, 10.0, 10.0]);
    let p = pasos(&s, &Herencia::default(), (0.0, 0.0, 100.0, 100.0)).unwrap().unwrap();
    let n = p.figuras[0][0].caminos[0].len();
    assert!(p.figuras.iter().all(|f| f[0].caminos[0].len() == n));
}

#[test]
fn un_d_que_cambia_de_forma_es_error() {
    let s = dentro(r##"<path fill="red"><animate attributeName="d" values="M0 0 L5 5 L0 5 Z;M0 0 C1 1 2 2 5 5 L0 5 Z" dur="1s" repeatCount="indefinite"/></path>"##, [0.0, 0.0, 10.0, 10.0]);
    let e = pasos(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0)).unwrap().unwrap_err();
    assert!(e[0].title.contains("cambia de forma"), "{}", e[0].title);
}

#[test]
fn un_clip_rectangular_recorta_exacto_y_uno_concavo_se_dice() {
    let s = dentro(r##"<defs><clipPath id="c"><rect width="5" height="10"/></clipPath></defs><g clip-path="url(#c)"><rect width="10" height="10" fill="red"/></g>"##, [0.0, 0.0, 10.0, 10.0]);
    let (f, e) = figuras(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert!(e.is_empty(), "{e:?}");
    let xs: Vec<f64> = f[0].caminos[0].iter().map(|p| p.0).collect();
    assert_eq!(xs.iter().cloned().fold(f64::MIN, f64::max), 5.0);
    let mal = dentro(r##"<clipPath id="c"><path d="M0 0 L10 0 L5 3 L10 10 L0 10 Z"/></clipPath><rect clip-path="url(#c)" width="10" height="10"/>"##, [0.0, 0.0, 10.0, 10.0]);
    let (_, e) = figuras(&mal, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    assert!(e[0].title.contains("no es convexo"), "{e:?}");
}

#[test]
fn steps_salta_y_nth_child_escalona() {
    let s = fichero(r##"<svg viewBox="0 0 30 10"><style>
      @keyframes late { 0% { opacity: 1 } 50% { opacity: .2 } 100% { opacity: 1 } }
      circle { fill: red; animation: late 1s steps(2) infinite }
      circle:nth-child(3) { animation-delay: .5s }
    </style><circle cx="5" cy="5" r="4"/><circle cx="15" cy="5" r="4"/></svg>"##).unwrap_or_else(|e| panic!("{e:#?}"));
    let p = pasos(&s, &Herencia::default(), (0.0, 0.0, 30.0, 10.0)).expect("anima").unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(p.ciclo_ms, 1000);
    // El `<style>` cuenta como hermano: el segundo circulo es el hijo 3.
    let alfa_en = |ms: u32, i: usize| {
        let k = p.tiempos.iter().rposition(|&t| t <= ms).unwrap();
        p.figuras[k][i].alfa
    };
    assert!((alfa_en(0, 0) - 1.0).abs() < 1e-9);
    // Con saltos no se mezcla: de 1 a 0,6 de golpe a los 250 ms, y a 0,2 a
    // los 500 (dos saltos por tramo).
    assert!((alfa_en(249, 0) - 1.0).abs() < 1e-9, "{}", alfa_en(249, 0));
    assert!((alfa_en(250, 0) - 0.6).abs() < 1e-9, "{}", alfa_en(250, 0));
    assert!((alfa_en(600, 0) - 0.2).abs() < 1e-9, "{}", alfa_en(600, 0));
    // El segundo va medio ciclo detras.
    assert!((alfa_en(100, 1) - 0.2).abs() < 1e-9, "{}", alfa_en(100, 1));
}

#[test]
fn lo_que_se_sale_del_svg_no_se_ve_como_en_el_navegador() {
    // Un circulo mas grande que el `viewBox`: el navegador lo corta en la
    // caja del `<svg>` (`overflow: hidden`), y BMO-X tambien.
    let s = dentro(r##"<circle cx="5" cy="5" r="8" fill="red"/><rect x="2" y="2" width="2" height="2" fill="blue"/>"##, [0.0, 0.0, 10.0, 10.0]);
    let (f, _) = figuras(&s, &Herencia::default(), (0.0, 0.0, 10.0, 10.0));
    for &(x, y) in &f[0].caminos[0] {
        assert!((-1e-9..=10.0 + 1e-9).contains(&x) && (-1e-9..=10.0 + 1e-9).contains(&y), "({x}, {y}) fuera");
    }
    // El que cabe sale como siempre, sin recortar.
    assert_eq!(f[1].caminos[0].len(), 4);
}
