//! Los DATOS (H1) y las LISTAS (P2) en el padre: lo que nombra y lo que
//! rechaza, por el mensaje.

use bmo_maqueta_diag::render;
use bmo_maqueta_node::{hueco, parse};

fn errs(src: &str) -> String {
    let raw = match parse(src.as_bytes()) {
        Ok(_) => panic!("esto tenia que fallar:\n{src}"),
        Err(e) => render("t.maqueta", src.as_bytes(), &e),
    };
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ok(src: &str) -> bmo_maqueta_node::Document {
    match parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    }
}

#[test]
fn un_hueco_es_nombre_y_muestra() {
    assert_eq!(hueco("{nombre|Ana Lopez}"), Some(("nombre", "Ana Lopez")));
    assert_eq!(hueco("{nota}"), Some(("nota", "nota")), "sin muestra, el nombre es la muestra");
    assert_eq!(hueco("{Nombre}"), None, "el nombre sale como campo de Rust: minusculas");
    assert_eq!(hueco("hola {x}"), None, "es TODO el texto");
}

#[test]
fn el_texto_del_hueco_lleva_la_muestra_y_el_nodo_el_nombre() {
    let d = ok("<maqueta><span>{nombre|Ana Lopez}</span></maqueta>");
    let span = &d.root.children[0];
    assert_eq!(span.text.as_deref(), Some("Ana Lopez"));
    assert_eq!(span.hueco.as_deref(), Some("nombre"));
}

#[test]
fn un_hueco_a_medias_se_rechaza_con_el_ejemplo() {
    let e = errs("<maqueta><span>hola {nombre}</span></maqueta>");
    assert!(e.contains("es TODO el texto de su caja"), "{e}");
}

#[test]
fn una_lista_dice_cuantas_filas_y_su_hueco() {
    let d = ok("<maqueta><usa id=\"l\" src=\"fila.maqueta\" repite=\"8\" entre=\"4\"/></maqueta>");
    let r = d.root.children[0].repite.expect("repite");
    assert_eq!((r.veces, r.entre), (8, 4));
}

#[test]
fn una_lista_tiene_tope_y_entre_sin_repite_no_es_nada() {
    let e = errs("<maqueta><usa src=\"fila.maqueta\" repite=\"0\"/></maqueta>");
    assert!(e.contains("de 1 a 64"), "{e}");
    let e = errs("<maqueta><usa src=\"fila.maqueta\" entre=\"4\"/></maqueta>");
    assert!(e.contains("dice `entre` o `columnas` pero no `repite`"), "{e}");
}

#[test]
fn un_color_de_dato_se_apunta_con_su_muestra() {
    let d = ok("<maqueta class=\"m\"></maqueta><style>:root { --dato-color: #4DE38E } .m { background-color: var(--dato-color) }</style>");
    assert_eq!(d.datos, vec![("color".to_string(), 0x4DE38E)]);
}

#[test]
fn la_muestra_no_puede_salir_de_otra_forma() {
    // Si sale a mano, el emisor no sabria cual de los dos es el dato.
    let e = errs("<maqueta class=\"m\"></maqueta><style>:root { --dato-color: #4DE38E } .m { background-color: #4de38e }</style>");
    assert!(e.contains("es la muestra de `--dato-color` y sale aqui tambien"), "{e}");
}
