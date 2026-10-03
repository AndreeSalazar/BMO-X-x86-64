//! **`COPY`**: la LIBRERIA de COBOL (2.8 de `PLAN_BANCA.md`, 2026-10-03).
//!
//! El propietario: *"me gustaria crear algo como libreria o WRAPPER para
//! simplificar por completo"*. En COBOL la libreria se llama COPYBOOK: un
//! trozo de fuente que cada programa trae con `COPY NOMBRE.` -- los registros
//! que dos programas tienen que acordar, y los parrafos que no se quieren
//! escribir dos veces. Es como lo hace un banco de verdad desde 1968.
//!
//! ```text
//!    COPY CABDATOS.          la linea entera se cambia por CABDATOS.cpy
//!    COPY "cablibro".        con comillas tambien, y sin mirar mayusculas
//!    COPY X REPLACING ...    NO todavia (2.8b): se dice, no se ignora
//!    COPY X OF LIB           NO: aqui no hay bibliotecas con nombre
//! ```
//!
//! ** Se expande ANTES de parsear, como el preprocesador del estandar: el
//! parser no sabe que existio un `COPY`. Por eso un error dentro de un
//! copybook lleva el numero de linea del fuente YA expandido; el mensaje de
//! un `COPY` que falla lleva el del fuente original.
//!
//! ** El frontend no lee el disco: quien llama pasa `buscar`, que dada un
//! nombre devuelve el texto. El compilador de linea de ordenes busca en
//! carpetas; las pruebas, en tablas con `include_str!`.

use crate::CobolError;

/// Lo mas hondo que se anida un `COPY` dentro de otro.
pub const HONDO_MAX: usize = 8;

/// Si la linea es un `COPY`, el nombre que pide. `Err` si es un `COPY` que
/// aun no se sabe hacer, y `Ok(None)` si no es un `COPY`.
fn copy_de(linea: &str) -> Result<Option<String>, String> {
    let t = linea.trim();
    // Comentario de formato fijo (`*` o `/` en la columna 7) o libre (`*>`).
    if t.starts_with('*') || t.starts_with('/') {
        return Ok(None);
    }
    let mayus = t.to_ascii_uppercase();
    if !(mayus.starts_with("COPY ") || mayus == "COPY" || mayus == "COPY.") {
        return Ok(None);
    }
    let resto = t[4..].trim();
    let Some(resto) = resto.strip_suffix('.') else {
        return Err("un COPY acaba en punto, en la misma linea: COPY NOMBRE.".into());
    };
    let palabras: Vec<&str> = resto.split_whitespace().collect();
    match palabras.as_slice() {
        [] => Err("COPY sin nombre".into()),
        [nombre] => {
            let n = nombre.trim_matches(|c| c == '"' || c == '\'');
            let valido = !n.is_empty() && n.len() <= 30 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if valido {
                Ok(Some(n.to_ascii_uppercase()))
            } else {
                Err(format!("el nombre de un copybook es letras, cifras y guiones (y 30 como mucho): {nombre}"))
            }
        }
        [_, w, ..] if w.eq_ignore_ascii_case("REPLACING") => {
            Err("COPY ... REPLACING aun no se sabe (2.8b de PLAN_BANCA): el texto se trae tal cual o no se trae".into())
        }
        [_, w, ..] if w.eq_ignore_ascii_case("OF") || w.eq_ignore_ascii_case("IN") => {
            Err("COPY ... OF/IN: aqui no hay bibliotecas con nombre, solo carpetas de copybooks".into())
        }
        _ => Err(format!("COPY con algo de mas: {resto}")),
    }
}

/// **Expande todos los `COPY`** de `fuente`, tambien los que vienen dentro de
/// un copybook (hasta [`HONDO_MAX`]). Un copybook que se trae a si mismo,
/// directa o indirectamente, es un error con su cadena.
pub fn expandir(fuente: &str, buscar: &mut dyn FnMut(&str) -> Option<String>) -> Result<String, CobolError> {
    let mut salida = String::with_capacity(fuente.len());
    let mut pila: Vec<String> = Vec::new();
    expandir_en(fuente, buscar, &mut pila, &mut salida, true)?;
    Ok(salida)
}

fn expandir_en(
    fuente: &str,
    buscar: &mut dyn FnMut(&str) -> Option<String>,
    pila: &mut Vec<String>,
    salida: &mut String,
    de_fuera: bool,
) -> Result<(), CobolError> {
    for (k, linea) in fuente.lines().enumerate() {
        // El numero de linea solo vale en el fuente de fuera; dentro de un
        // copybook se nombra el copybook.
        let donde = if de_fuera { k + 1 } else { 0 };
        let dentro_de = |m: String| match pila.last() {
            Some(c) => format!("en el copybook {c}, linea {}: {m}", k + 1),
            None => m,
        };
        match copy_de(linea) {
            Ok(None) => {
                salida.push_str(linea);
                salida.push('\n');
            }
            Ok(Some(nombre)) => {
                if pila.contains(&nombre) {
                    let cadena = pila.join(" -> ");
                    return Err(CobolError::new(donde, dentro_de(format!("COPY en circulo: {cadena} -> {nombre}"))));
                }
                if pila.len() >= HONDO_MAX {
                    return Err(CobolError::new(donde, dentro_de(format!("COPY anidado mas de {HONDO_MAX} veces"))));
                }
                let Some(texto) = buscar(&nombre) else {
                    return Err(CobolError::new(donde, dentro_de(format!("no encuentro el copybook {nombre} ({nombre}.cpy)"))));
                };
                pila.push(nombre);
                expandir_en(&texto, buscar, pila, salida, false).map_err(|e| {
                    if de_fuera {
                        CobolError::new(donde, e.message)
                    } else {
                        e
                    }
                })?;
                pila.pop();
            }
            Err(m) => return Err(CobolError::new(donde, dentro_de(m))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tabla<'a>(t: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        move |n| t.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.to_string())
    }

    #[test]
    fn un_copy_se_cambia_por_su_texto() {
        let mut b = tabla(&[("DATOS", "01 A PIC 9.\n01 B PIC 9.")]);
        let s = expandir("WORKING-STORAGE SECTION.\n       COPY DATOS.\nPROCEDURE DIVISION.\n", &mut b).unwrap();
        assert_eq!(s, "WORKING-STORAGE SECTION.\n01 A PIC 9.\n01 B PIC 9.\nPROCEDURE DIVISION.\n");
    }

    #[test]
    fn con_comillas_y_minusculas_tambien() {
        let mut b = tabla(&[("LIBRO", "X.")]);
        assert_eq!(expandir("copy \"libro\".\n", &mut b).unwrap(), "X.\n");
        assert_eq!(expandir("Copy 'Libro'.\n", &mut b).unwrap(), "X.\n");
    }

    #[test]
    fn un_comentario_que_dice_copy_no_es_un_copy() {
        let mut b = tabla(&[]);
        let f = "      * COPY NADA.\n*> COPY NADA.\n";
        assert_eq!(expandir(f, &mut b).unwrap(), f);
    }

    #[test]
    fn anidados_si_y_en_circulo_no() {
        let mut b = tabla(&[("A", "a1\nCOPY B.\na2"), ("B", "b")]);
        assert_eq!(expandir("COPY A.\n", &mut b).unwrap(), "a1\nb\na2\n");
        let mut c = tabla(&[("A", "COPY B."), ("B", "COPY A.")]);
        let e = expandir("x\nCOPY A.\n", &mut c).unwrap_err();
        assert_eq!(e.line, 2, "el error lleva la linea del COPY de fuera");
        assert!(e.message.contains("A -> B -> A"), "{}", e.message);
    }

    #[test]
    fn lo_que_aun_no_se_sabe_se_dice() {
        let mut b = tabla(&[("A", "x")]);
        for (f, trozo) in [
            ("COPY A REPLACING ==X== BY ==Y==.\n", "REPLACING"),
            ("COPY A OF LIBRERIA.\n", "OF/IN"),
            ("COPY A\n", "acaba en punto"),
            ("COPY .\n", "sin nombre"),
            ("COPY ../etc/x.\n", "letras, cifras"),
            ("COPY NADA.\n", "no encuentro el copybook NADA"),
        ] {
            let e = expandir(f, &mut b).unwrap_err();
            assert!(e.message.contains(trozo), "{f:?}: {}", e.message);
            assert_eq!(e.line, 1);
        }
    }

    #[test]
    fn el_error_de_dentro_dice_de_que_copybook_viene() {
        let mut b = tabla(&[("A", "uno\nCOPY B REPLACING X BY Y.")]);
        let e = expandir("COPY A.\n", &mut b).unwrap_err();
        assert!(e.message.starts_with("en el copybook A, linea 2:"), "{}", e.message);
    }
}
