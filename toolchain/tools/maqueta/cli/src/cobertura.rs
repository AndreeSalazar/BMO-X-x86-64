//! **`maqueta --cobertura maqueta.html`** -- cuanto del CSS de una maqueta
//! acepta MAQUETA, medido con el compilador de verdad.
//!
//! El propietario (04-10): *"cuanto falta para ser igual o mejor que CSS?"*.
//! La primera respuesta se conto por NOMBRE de propiedad, y era generosa:
//! `padding: 8px 12px` contaba como aceptada porque `padding` existia, y el
//! padre la rechazaba (solo leia uno o cuatro valores). Un numero que nadie
//! ejecuta envejece igual que un valor puesto por prudencia.
//!
//! Esto no cuenta nombres: saca CADA declaracion de la maqueta (las de las
//! reglas y las de `style="..."`), la mete en `.a { ... }` con las variables
//! de `:root` que use, y se la da al padre. Lo que compila, cuenta.
//!
//! ```text
//!    cobertura   aceptadas / declaraciones, en tanto por ciento
//!    rechazos    por propiedad, los mas usados primero, con el motivo
//! ```

use std::collections::HashMap;

/// Lo que salio de medir una maqueta.
pub struct Informe {
    pub total: usize,
    pub aceptadas: usize,
    /// Plantillas de JavaScript (`${...}`): no son CSS, no cuentan.
    pub plantillas: usize,
    /// Por propiedad: (veces rechazada, el motivo de la primera).
    pub rechazos: Vec<(String, usize, String)>,
}

/// Las declaraciones de una maqueta, y la tabla de `:root`.
fn declaraciones(html: &str) -> (Vec<(String, String)>, HashMap<String, String>) {
    let mut css = String::new();
    let mut resto = html;
    while let Some(a) = resto.find("<style") {
        let Some(b) = resto[a..].find('>') else { break };
        let desde = a + b + 1;
        let Some(c) = resto[desde..].find("</style>") else { break };
        css.push_str(&resto[desde..desde + c]);
        css.push('\n');
        resto = &resto[desde + c..];
    }
    // Los comentarios fuera.
    while let Some(a) = css.find("/*") {
        match css[a..].find("*/") {
            Some(b) => css.replace_range(a..a + b + 2, " "),
            None => css.truncate(a),
        }
    }
    let mut decls = Vec::new();
    let mut raiz = HashMap::new();
    // Bloques sin bloques dentro: `selector { decls }`.
    let bytes = css.as_bytes();
    let mut inicio_sel = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                let cierre = css[i + 1..].find('}').map(|k| i + 1 + k);
                let anidado = css[i + 1..].find('{').map(|k| i + 1 + k);
                match (cierre, anidado) {
                    (Some(c), Some(a)) if a < c => {
                        // `@media ... {` o `@keyframes ... {`: se entra.
                        inicio_sel = i + 1;
                        i += 1;
                    }
                    (Some(c), _) => {
                        let selector = css[inicio_sel..i].trim();
                        let cuerpo = &css[i + 1..c];
                        let es_raiz = selector.contains(":root");
                        for d in cuerpo.split(';') {
                            if let Some((p, v)) = d.split_once(':') {
                                let (p, v) = (p.trim(), v.trim());
                                if p.starts_with("--") {
                                    if es_raiz {
                                        raiz.insert(p.to_string(), v.to_string());
                                    }
                                    continue;
                                }
                                if !p.is_empty() && !es_raiz {
                                    decls.push((p.to_string(), v.to_string()));
                                }
                            }
                        }
                        inicio_sel = c + 1;
                        i = c + 1;
                    }
                    _ => break,
                }
            }
            b'}' => {
                inicio_sel = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    // Y las de `style="..."`.
    let mut resto = html;
    while let Some(a) = resto.find("style=\"") {
        let desde = a + 7;
        let Some(b) = resto[desde..].find('"') else { break };
        for d in resto[desde..desde + b].split(';') {
            if let Some((p, v)) = d.split_once(':') {
                let (p, v) = (p.trim(), v.trim());
                if !p.is_empty() && !p.starts_with("--") {
                    decls.push((p.to_string(), v.to_string()));
                }
            }
        }
        resto = &resto[desde + b..];
    }
    (decls, raiz)
}

/// Las variables de `:root` que usa `valor`, con las que esas usan.
fn raiz_de(valor: &str, raiz: &HashMap<String, String>) -> String {
    let mut usadas: Vec<String> = Vec::new();
    let mut pendiente = vec![valor.to_string()];
    while let Some(v) = pendiente.pop() {
        let mut r = v.as_str();
        while let Some(a) = r.find("--") {
            let fin = r[a..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).map(|k| a + k).unwrap_or(r.len());
            let nombre = &r[a..fin];
            if let Some(def) = raiz.get(nombre) {
                if !usadas.iter().any(|u| u == nombre) {
                    usadas.push(nombre.to_string());
                    pendiente.push(def.clone());
                }
            }
            r = &r[fin.max(a + 2)..];
        }
    }
    usadas.iter().map(|n| format!("{n}:{};", raiz[n])).collect()
}

/// **Mide** una maqueta HTML.
pub fn medir(html: &str) -> Informe {
    let (decls, raiz) = declaraciones(html);
    let mut inf = Informe { total: 0, aceptadas: 0, plantillas: 0, rechazos: Vec::new() };
    let mut por: HashMap<String, (usize, String)> = HashMap::new();
    for (p, v) in decls {
        if v.contains("${") {
            inf.plantillas += 1;
            continue;
        }
        inf.total += 1;
        let r = raiz_de(&v, &raiz);
        let raiz_css = if r.is_empty() { String::new() } else { format!(":root{{{r}}} ") };
        let src = format!("<maqueta><div class=\"a\"></div></maqueta><style>{raiz_css}.a{{{p}:{v}}}</style>");
        match bmo_maqueta_node::parse(src.as_bytes()) {
            Ok(_) => inf.aceptadas += 1,
            Err(e) => {
                let motivo = e.first().map(|e| e.title.clone()).unwrap_or_default();
                let entrada = por.entry(p).or_insert((0, motivo));
                entrada.0 += 1;
            }
        }
    }
    let mut v: Vec<(String, usize, String)> = por.into_iter().map(|(p, (n, m))| (p, n, m)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    inf.rechazos = v;
    inf
}

/// Por diezmilesimas.
pub fn por_ciento(a: usize, b: usize) -> String {
    let d = (a * 10000).checked_div(b).unwrap_or(0);
    format!("{}.{:02} %", d / 100, d % 100)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cuenta_lo_que_compila_y_dice_lo_que_no() {
        let html = "<style>:root{--oro:#FFD45E; --f:\"IBM Plex\"}\n\
                    .a{color:var(--oro); padding:8px 12px; opacity:.5}\n\
                    @media (max-width:600px){ .b{border:1px solid var(--oro)} }</style>\
                    <div style=\"margin:4px; width:${w}px\"></div>";
        let inf = medir(html);
        assert_eq!(inf.total, 5);
        assert_eq!(inf.plantillas, 1);
        assert_eq!(inf.aceptadas, 3, "color con var, padding de dos y el borde dentro de @media");
        let nombres: Vec<&str> = inf.rechazos.iter().map(|r| r.0.as_str()).collect();
        assert!(nombres.contains(&"opacity") && nombres.contains(&"margin"));
    }
}
