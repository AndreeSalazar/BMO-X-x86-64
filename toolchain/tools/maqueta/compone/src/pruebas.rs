//! Las piezas, en memoria: cada prueba es un disco chico de mentira.

use super::*;

fn disco(ficheros: &[(&str, &str)]) -> impl Fn(&Path) -> Option<Vec<u8>> {
    let m: HashMap<PathBuf, Vec<u8>> = ficheros.iter().map(|(n, t)| (PathBuf::from(n), t.as_bytes().to_vec())).collect();
    move |p: &Path| m.get(p).cloned()
}

const FILA: &str = "<maqueta class=\"f\"><span id=\"nombre\" class=\"t\">Cartera</span></maqueta>\
<style>.f{width:120px; height:24px; background-color:#101018} .t{color:#E6EDF6}</style>";

#[test]
fn una_pieza_cae_en_su_sitio_con_su_medida() {
    let principal = "<maqueta class=\"m\"><div class=\"tapa\"></div><usa id=\"uno\" src=\"fila.maqueta\"/></maqueta>\
<style>.m{width:200px; height:60px; display:flex; flex-direction:column; padding:4px} .tapa{height:10px}</style>";
    let leer = disco(&[("p/principal.maqueta", principal), ("p/fila.maqueta", FILA)]);
    let l = compilar_con(Path::new("p/principal.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let usa = l.all().into_iter().find(|f| f.tag == Tag::Usa).expect("el usa");
    assert_eq!((usa.rect.x, usa.rect.y, usa.rect.w, usa.rect.h), (4, 14, 120, 24), "la medida es la de la pieza");
    let raiz = &usa.children[0];
    assert_eq!((raiz.rect.x, raiz.rect.y), (4, 14), "la pieza corrida a su sitio");
    assert!(l.hits().iter().any(|(id, r)| *id == "uno.nombre" && r.x >= 4 && r.y >= 14), "{:?}", l.hits());
}

#[test]
fn las_reglas_de_una_pieza_no_salen_de_ella() {
    // La principal tambien tiene una `.t`, roja; la de la pieza sigue siendo
    // la suya.
    let principal = "<maqueta class=\"m\"><span class=\"t\">x</span><usa src=\"fila.maqueta\"/></maqueta>\
<style>.m{width:200px; height:60px; display:flex; flex-direction:column} .t{color:#FF0000}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("fila.maqueta", FILA)]);
    let l = compilar_con(Path::new("principal.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let textos: Vec<Option<u32>> = l.all().iter().filter(|f| f.text.is_some()).map(|f| f.style.color).collect();
    assert_eq!(textos, vec![Some(0xFF0000), Some(0xE6EDF6)]);
}

#[test]
fn la_misma_pieza_dos_veces_necesita_dos_ids() {
    let sin = "<maqueta class=\"m\"><usa src=\"fila.maqueta\"/><usa src=\"fila.maqueta\"/></maqueta>\
<style>.m{width:200px; height:60px; display:flex; flex-direction:column}</style>";
    let leer = disco(&[("a.maqueta", sin), ("fila.maqueta", FILA)]);
    let f = compilar_con(Path::new("a.maqueta"), &leer).err().expect("ids repetidos");
    assert!(f.render().contains("sale dos veces"), "{}", f.render());
    let con = sin.replacen("<usa src", "<usa id=\"a\" src", 1).replacen("/><usa src", "/><usa id=\"b\" src", 1);
    let leer = disco(&[("a.maqueta", con.as_str()), ("fila.maqueta", FILA)]);
    let l = compilar_con(Path::new("a.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let ids: Vec<&str> = l.hits().iter().map(|h| h.0).filter(|i| i.contains('.')).collect();
    assert_eq!(ids, vec!["a.nombre", "b.nombre"]);
}

#[test]
fn una_pieza_que_no_compila_se_cuenta_en_su_fichero() {
    let mala = "<maqueta><h1>no</h1></maqueta>";
    let principal = "<maqueta class=\"m\"><usa src=\"mala.maqueta\"/></maqueta><style>.m{width:10px; height:10px}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("mala.maqueta", mala)]);
    let f = compilar_con(Path::new("principal.maqueta"), &leer).err().expect("tiene que fallar");
    assert_eq!(f.fichero, "mala.maqueta");
}

#[test]
fn una_pieza_que_se_usa_a_si_misma_es_un_error() {
    let a = "<maqueta class=\"m\"><usa src=\"b.maqueta\"/></maqueta><style>.m{width:10px; height:10px}</style>";
    let b = "<maqueta class=\"m\"><usa src=\"a.maqueta\"/></maqueta><style>.m{width:10px; height:10px}</style>";
    let leer = disco(&[("a.maqueta", a), ("b.maqueta", b)]);
    let f = compilar_con(Path::new("a.maqueta"), &leer).err().expect("ciclo");
    assert!(f.render().contains("a si misma"), "{}", f.render());
}

#[test]
fn una_pieza_mide_lo_que_mide() {
    let principal = "<maqueta class=\"m\"><usa class=\"u\" src=\"fila.maqueta\"/></maqueta>\
<style>.m{width:200px; height:60px} .u{width:50px}</style>";
    let leer = disco(&[("p.maqueta", principal), ("fila.maqueta", FILA)]);
    let f = compilar_con(Path::new("p.maqueta"), &leer).err().expect("medida pisada");
    assert!(f.render().contains("otra medida"), "{}", f.render());
}

#[test]
fn una_pieza_que_no_cabe_la_ve_el_veredicto_de_la_principal() {
    let principal = "<maqueta class=\"m\"><usa src=\"fila.maqueta\"/></maqueta><style>.m{width:60px; height:60px}</style>";
    let leer = disco(&[("p.maqueta", principal), ("fila.maqueta", FILA)]);
    let f = compilar_con(Path::new("p.maqueta"), &leer).err().expect("no cabe");
    assert_eq!(f.fichero, "p.maqueta");
}

#[test]
fn sin_piezas_es_lo_de_siempre() {
    let calc = include_str!("../../pruebas/calc.maqueta");
    let leer = disco(&[("calc.maqueta", calc)]);
    let l = compilar_con(Path::new("calc.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let doc = parse(calc.as_bytes()).unwrap();
    let c = cascade(&doc).unwrap();
    assert_eq!(l, lay(&c));
}

// -- P3a: los estados y la transicion --------------------------------------

const PANEL: &str = "<maqueta class=\"m\"><div class=\"c\"><span class=\"t\">Hola</span></div></maqueta>\
<style>.m{width:300px; height:200px} .c{width:100px; height:40px; background-color:#000000; transition: 400ms linear}\
.t{font-size:13px; color:#FFFFFF}\
@estado abierta { .c{width:260px; height:160px; background-color:#FFFFFF} }</style>";

#[test]
fn cada_estado_se_maqueta_y_se_juzga() {
    let leer = disco(&[("p.maqueta", PANEL)]);
    let e = compilar_estados_con(Path::new("p.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let caja = |l: &Laid| l.all()[1].rect;
    assert_eq!(caja(&e.reposo).w, 100);
    assert_eq!(caja(e.de("abierta").expect("el estado")).w, 260);
    // Un estado donde no cabe: no compila, y el error dice cual.
    let malo = PANEL.replace("width:260px; height:160px", "width:400px; height:160px");
    let leer = disco(&[("p.maqueta", malo.as_str())]);
    let f = compilar_estados_con(Path::new("p.maqueta"), &leer).err().expect("no cabe en abierta");
    assert!(f.render().contains("en el estado `abierta`"), "{}", f.render());
}

// -- LAS LISTAS (P2) -------------------------------------------------------

#[test]
fn una_lista_se_maqueta_con_todas_sus_filas_y_las_nombra() {
    let principal = "<maqueta class=\"m\"><usa id=\"l\" src=\"fila.maqueta\" repite=\"3\" entre=\"4\"/></maqueta>\
<style>.m{width:200px; height:100px; display:flex; flex-direction:column; padding:4px}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("fila.maqueta", FILA)]);
    let l = compilar_con(Path::new("principal.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let usa = l.all().into_iter().find(|f| f.tag == Tag::Usa).expect("el usa");
    assert_eq!((usa.rect.w, usa.rect.h), (120, 3 * 24 + 2 * 4), "tres filas y dos huecos");
    assert_eq!(usa.children.len(), 3);
    let ys: Vec<i32> = usa.children.iter().map(|f| f.rect.y).collect();
    assert_eq!(ys, vec![4, 32, 60], "cada una `alto + entre` mas abajo");
    let ids: Vec<&str> = l.hits().iter().map(|h| h.0).collect();
    assert_eq!(ids, vec!["l", "l.0.nombre", "l.1.nombre", "l.2.nombre"]);
}

#[test]
fn una_lista_que_no_cabe_entera_no_compila() {
    // Seis filas de 24 no caben en 100: se juzga lo peor, que esten todas.
    let principal = "<maqueta class=\"m\"><usa id=\"l\" src=\"fila.maqueta\" repite=\"6\"/></maqueta>\
<style>.m{width:200px; height:100px; display:flex; flex-direction:column}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("fila.maqueta", FILA)]);
    let f = compilar_con(Path::new("principal.maqueta"), &leer).err().expect("no cabe");
    assert!(f.render().contains("se sale"), "{}", f.render());
}

#[test]
fn una_lista_necesita_id() {
    let principal = "<maqueta class=\"m\"><usa src=\"fila.maqueta\" repite=\"2\"/></maqueta>\
<style>.m{width:200px; height:100px; display:flex; flex-direction:column}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("fila.maqueta", FILA)]);
    let f = compilar_con(Path::new("principal.maqueta"), &leer).err().expect("sin id");
    assert!(f.render().contains("una lista necesita `id`"), "{}", f.render());
}

// -- LA REJILLA (H6) y LAS IMAGENES (H4) -------------------------------------

#[test]
fn una_rejilla_va_de_izquierda_a_derecha_y_baja() {
    let principal = "<maqueta class=\"m\"><usa id=\"g\" src=\"fila.maqueta\" repite=\"3\" columnas=\"2\" entre=\"4\"/></maqueta>\
<style>.m{width:300px; height:100px; display:flex; flex-direction:column}</style>";
    let leer = disco(&[("principal.maqueta", principal), ("fila.maqueta", FILA)]);
    let l = compilar_con(Path::new("principal.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let usa = l.all().into_iter().find(|f| f.tag == Tag::Usa).expect("el usa");
    assert_eq!((usa.rect.w, usa.rect.h), (2 * 120 + 4, 2 * 24 + 4), "dos columnas, dos filas");
    let sitios: Vec<(i32, i32)> = usa.children.iter().map(|f| (f.rect.x, f.rect.y)).collect();
    assert_eq!(sitios, vec![(0, 0), (124, 0), (0, 28)]);
}

/// Un QOI de `w x h` de un solo color, escrito a mano.
fn qoi(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"qoif".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[4, 0]);
    for _ in 0..w * h {
        b.extend_from_slice(&[0xFF, 0x4D, 0xE3, 0x8E, 0xFF]);
    }
    b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    b
}

fn disco_con_imagen(principal: &str, img: Vec<u8>) -> impl Fn(&Path) -> Option<Vec<u8>> {
    let principal = principal.as_bytes().to_vec();
    move |p: &Path| match p.to_str() {
        Some("a.maqueta") => Some(principal.clone()),
        Some("i.qoi") => Some(img.clone()),
        _ => None,
    }
}

#[test]
fn una_imagen_mide_lo_que_mide_y_lleva_sus_pixeles() {
    let principal = "<maqueta class=\"m\"><imagen src=\"i.qoi\"/></maqueta><style>.m{display:flex}</style>";
    let l = compilar_con(Path::new("a.maqueta"), &disco_con_imagen(principal, qoi(5, 3))).unwrap_or_else(|f| panic!("{}", f.render()));
    let im = l.all().into_iter().find(|f| f.tag == Tag::Imagen).expect("la imagen");
    assert_eq!((im.rect.w, im.rect.h), (5, 3));
    let px = im.imagen.as_ref().expect("sus pixeles");
    assert_eq!(px.len(), 15);
    assert_eq!(px[0], 0xFF4D_E38E);
}

#[test]
fn una_imagen_embebida_grande_es_un_dato() {
    let principal = "<maqueta class=\"m\"><imagen src=\"i.qoi\"/></maqueta><style>.m{display:flex}</style>";
    let f = compilar_con(Path::new("a.maqueta"), &disco_con_imagen(principal, qoi(200, 2))).err().expect("demasiado grande");
    let t = f.render().split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(t.contains("una foto grande es un DATO"), "{t}");
}

#[test]
fn una_imagen_no_se_estira() {
    let principal = "<maqueta class=\"m\"><imagen class=\"i\" src=\"i.qoi\"/></maqueta><style>.m{display:flex} .i{width:10px}</style>";
    let f = compilar_con(Path::new("a.maqueta"), &disco_con_imagen(principal, qoi(5, 3))).err().expect("no escala");
    let t = f.render().split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(t.contains("BMO-X no escala"), "{t}");
}
