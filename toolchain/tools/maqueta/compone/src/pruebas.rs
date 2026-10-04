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

#[test]
fn la_transicion_va_de_uno_a_otro() {
    let leer = disco(&[("p.maqueta", PANEL)]);
    let e = compilar_estados_con(Path::new("p.maqueta"), &leer).unwrap();
    let b = e.de("abierta").unwrap();
    assert_eq!(transicion::duracion(b), 400);
    let w = |ms| transicion::en(&e.reposo, b, ms).all()[1].rect.w;
    assert_eq!(w(0), 100);
    assert_eq!(w(200), 180, "a la mitad, en lineal, a la mitad");
    assert_eq!(w(400), 260);
    assert_eq!(w(9999), 260, "despues del final, el final");
    let fondo = transicion::en(&e.reposo, b, 200).all()[1].style.background;
    assert_eq!(fondo, Some(0x808080), "el color, mezclado");
    assert_eq!(transicion::en(&e.reposo, b, 400), b.clone(), "al final ES el estado de llegada");
}

#[test]
fn el_rebote_se_pasa_y_vuelve() {
    let src = PANEL.replace("400ms linear", "400ms cubic-bezier(.34, 1.56, .64, 1)");
    let leer = disco(&[("p.maqueta", src.as_str())]);
    let e = compilar_estados_con(Path::new("p.maqueta"), &leer).unwrap();
    let b = e.de("abierta").unwrap();
    let anchos: Vec<u32> = (0..=40).map(|k| transicion::en(&e.reposo, b, k * 10).all()[1].rect.w).collect();
    assert!(anchos.iter().any(|&w| w > 260), "se pasa de largo: {anchos:?}");
    assert_eq!(*anchos.last().unwrap(), 260, "y acaba donde tiene que acabar");
}

#[test]
fn una_caja_que_cambia_de_sitio_en_la_lista_se_empareja_por_lo_que_es() {
    // En `fuera` la primera caja pasa a absoluta: en la lista de hijos va al
    // final, y aun asi se mezcla consigo misma.
    let src = "<maqueta class=\"m\"><div class=\"a\"></div><div class=\"b\"></div></maqueta>\
<style>.m{width:300px; height:200px; display:flex; flex-direction:column} .a{width:50px; height:20px; background-color:#FF0000; transition: 100ms linear} .b{width:60px; height:20px; background-color:#00FF00}\
@estado fuera { .a{position:absolute; left:200px; top:150px} }</style>";
    let leer = disco(&[("p.maqueta", src)]);
    let e = compilar_estados_con(Path::new("p.maqueta"), &leer).unwrap_or_else(|f| panic!("{}", f.render()));
    let b = e.de("fuera").unwrap();
    let mitad = transicion::en(&e.reposo, b, 50);
    let roja = mitad.all().into_iter().find(|f| f.style.background == Some(0xFF0000)).expect("la roja");
    assert_eq!((roja.rect.x, roja.rect.y), (100, 75), "a medio camino entre (0, 0) y (200, 150)");
}
