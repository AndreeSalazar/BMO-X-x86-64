//! **El CSS de dentro de un SVG**: el `<style>` de un fichero y el atributo
//! `style`. Lo justo para lo que exportan los editores (Illustrator escribe
//! `.cls-1{fill:#...}`, Inkscape `style="fill:..."`): selectores de una
//! pieza -- `etiqueta`, `.clase`, `#id`, juntos (`path.a.b`) y en grupo con
//! comas -- con su especificidad, `!important`, y `@keyframes` para S7.
//!
//! Un selector con espacios, `>`, `:` o `[` no es una regla que se pueda
//! ignorar: pinta algo. Es error, con el selector dicho.

/// Una declaracion: `fill: #fff`.
#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub prop: String,
    pub valor: String,
    pub importante: bool,
}

/// `etiqueta.clase#id:nth-child(2n+1)`, cada parte opcional.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Selector {
    pub etiqueta: Option<String>,
    pub clases: Vec<String>,
    pub id: Option<String>,
    /// `:first-child`, `:last-child`, `:nth-child(An+B)` como `(A, B)`; el
    /// ultimo se marca con `A = i64::MIN`.
    pub lugares: Vec<(i64, i64)>,
}

/// `An+B`, `odd`, `even` o un numero.
fn an_b(t: &str) -> Option<(i64, i64)> {
    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    match t.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    match t.split_once('n') {
        None => t.parse().ok().map(|b| (0, b)),
        Some((a, b)) => {
            let a = match a {
                "" | "+" => 1,
                "-" => -1,
                a => a.parse().ok()?,
            };
            let b = if b.is_empty() { 0 } else { b.parse().ok()? };
            Some((a, b))
        }
    }
}

impl Selector {
    /// (ids, clases, etiquetas), como CSS.
    pub fn especificidad(&self) -> (u32, u32, u32) {
        (self.id.is_some() as u32, (self.clases.len() + self.lugares.len()) as u32, self.etiqueta.is_some() as u32)
    }
    pub fn casa(&self, etiqueta: &str, clases: &[&str], id: Option<&str>, (k, n): (usize, usize)) -> bool {
        self.etiqueta.as_deref().is_none_or(|e| e == etiqueta)
            && self.clases.iter().all(|c| clases.contains(&c.as_str()))
            && self.id.as_deref().is_none_or(|i| Some(i) == id)
            && self.lugares.iter().all(|&(a, b)| {
                let k = k as i64;
                if a == i64::MIN {
                    return k == n as i64;
                }
                if a == 0 {
                    return k == b;
                }
                (k - b) % a == 0 && (k - b) / a >= 0
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Regla {
    pub selector: Selector,
    pub decls: Vec<Decl>,
}

/// Un `@keyframes`: sus fotogramas `(donde 0..=1, declaraciones)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fotogramas {
    pub nombre: String,
    pub pasos: Vec<(f64, Vec<Decl>)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hoja {
    pub reglas: Vec<Regla>,
    pub fotogramas: Vec<Fotogramas>,
}

fn sin_comentarios(t: &str) -> String {
    let mut out = String::new();
    let mut resto = t;
    while let Some(k) = resto.find("/*") {
        out.push_str(&resto[..k]);
        resto = match resto[k + 2..].find("*/") {
            Some(f) => &resto[k + 2 + f + 2..],
            None => "",
        };
    }
    out.push_str(resto);
    out
}

/// **Las declaraciones** de un bloque o de un `style="..."`.
pub fn declaraciones(t: &str) -> Vec<Decl> {
    sin_comentarios(t)
        .split(';')
        .filter_map(|d| {
            let (p, v) = d.split_once(':')?;
            let v = v.trim();
            let (v, importante) = match v.strip_suffix("!important") {
                Some(v) => (v.trim(), true),
                None => (v, false),
            };
            let p = p.trim().to_ascii_lowercase();
            (!p.is_empty()).then(|| Decl { prop: p, valor: v.to_string(), importante })
        })
        .collect()
}

fn selector(t: &str) -> Result<Selector, String> {
    let entero = t.trim();
    let (t, pseudo) = match entero.find(':') {
        Some(k) => (&entero[..k], &entero[k..]),
        None => (entero, ""),
    };
    if t.contains(|c: char| c.is_whitespace() || ">+~:[]()".contains(c)) || (t.is_empty() && pseudo.is_empty()) {
        return Err(entero.to_string());
    }
    let mut s = Selector::default();
    for p in pseudo.split(':').filter(|p| !p.is_empty()) {
        let l = match p {
            "first-child" => (0, 1),
            "last-child" => (i64::MIN, 0),
            _ => match p.strip_prefix("nth-child(").and_then(|r| r.strip_suffix(')')).and_then(an_b) {
                Some(ab) => ab,
                None => return Err(entero.to_string()),
            },
        };
        s.lugares.push(l);
    }
    let t = if t.is_empty() { "*" } else { t };
    if t == "*" {
        return Ok(s);
    }
    let mut parte = String::new();
    let mut tipo = 'e';
    let cerrar = |tipo: char, parte: &mut String, s: &mut Selector| {
        if parte.is_empty() {
            return;
        }
        let v = std::mem::take(parte);
        match tipo {
            '.' => s.clases.push(v),
            '#' => s.id = Some(v),
            _ => s.etiqueta = Some(v),
        }
    };
    for c in t.chars() {
        if c == '.' || c == '#' {
            cerrar(tipo, &mut parte, &mut s);
            tipo = c;
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            parte.push(c);
        } else if c == '*' && parte.is_empty() && tipo == 'e' {
        } else {
            return Err(t.to_string());
        }
    }
    cerrar(tipo, &mut parte, &mut s);
    Ok(s)
}

/// El cuerpo `{ ... }` que empieza en `t[ini]` (`{`): devuelve su texto y
/// donde sigue.
fn bloque(t: &str, ini: usize) -> Option<(&str, usize)> {
    let b = t.as_bytes();
    let mut nivel = 0;
    for k in ini..b.len() {
        match b[k] {
            b'{' => nivel += 1,
            b'}' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some((&t[ini + 1..k], k + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// **Lee una hoja.** Los errores son los textos que no se pueden leer.
pub fn hoja(t: &str) -> (Hoja, Vec<String>) {
    let t = sin_comentarios(t);
    let mut h = Hoja::default();
    let mut malos = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let Some(abre) = t[i..].find('{').map(|k| k + i) else {
            if !t[i..].trim().is_empty() {
                malos.push(t[i..].trim().to_string());
            }
            break;
        };
        let cabeza = t[i..abre].trim().to_string();
        let Some((cuerpo, sigue)) = bloque(&t, abre) else {
            malos.push(format!("{cabeza} {{ (sin cerrar)"));
            break;
        };
        i = sigue;
        if let Some(nombre) = cabeza.strip_prefix("@keyframes").or_else(|| cabeza.strip_prefix("@-webkit-keyframes")) {
            let mut f = Fotogramas { nombre: nombre.trim().to_string(), pasos: Vec::new() };
            let mut j = 0;
            while let Some(a) = cuerpo[j..].find('{').map(|k| k + j) {
                let donde = cuerpo[j..a].trim().to_string();
                let Some((dentro, s2)) = bloque(cuerpo, a) else { break };
                j = s2;
                let decls = declaraciones(dentro);
                for d in donde.split(',') {
                    let d = d.trim();
                    let v = match d {
                        "from" => Some(0.0),
                        "to" => Some(1.0),
                        _ => d.strip_suffix('%').and_then(|n| n.trim().parse::<f64>().ok()).map(|n| n / 100.0),
                    };
                    match v {
                        Some(v) if (0.0..=1.0).contains(&v) => f.pasos.push((v, decls.clone())),
                        _ => malos.push(format!("@keyframes {}: `{d}`", f.nombre)),
                    }
                }
            }
            f.pasos.sort_by(|a, b| a.0.total_cmp(&b.0));
            h.fotogramas.push(f);
            continue;
        }
        if cabeza.starts_with("@font-face") {
            // Una letra para un `<text>`, que es error por su cuenta.
            continue;
        }
        if cabeza.starts_with('@') {
            malos.push(cabeza);
            continue;
        }
        let decls = declaraciones(cuerpo);
        for s in cabeza.split(',') {
            match selector(s) {
                Ok(sel) => h.reglas.push(Regla { selector: sel, decls: decls.clone() }),
                Err(m) => malos.push(m),
            }
        }
    }
    (h, malos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_hoja_de_illustrator_se_lee() {
        let (h, malos) = hoja(".cls-1{fill:#5ef2e6;}.cls-2,.cls-3{fill:none;stroke:#fff;stroke-width:2px}/* x */ path.a#b{opacity:.5 !important}");
        assert!(malos.is_empty(), "{malos:?}");
        assert_eq!(h.reglas.len(), 4);
        assert_eq!(h.reglas[3].selector.especificidad(), (1, 1, 1));
        assert!(h.reglas[3].decls[0].importante);
        assert!(h.reglas[0].selector.casa("circle", &["cls-1"], None, (1, 1)));
    }

    #[test]
    fn un_selector_de_varias_piezas_es_error() {
        let (_, malos) = hoja("g path { fill: red } a:hover{fill:blue} @media (x) { .a{} }");
        assert_eq!(malos.len(), 3, "{malos:?}");
    }

    #[test]
    fn el_sitio_entre_hermanos() {
        let (h, malos) = hoja("circle:nth-child(2n+1){fill:red} rect:last-child{fill:blue} :first-child{opacity:.5} g:nth-child(3){}");
        assert!(malos.is_empty(), "{malos:?}");
        let c = &h.reglas[0].selector;
        assert!(c.casa("circle", &[], None, (3, 8)) && !c.casa("circle", &[], None, (2, 8)));
        assert!(h.reglas[1].selector.casa("rect", &[], None, (4, 4)));
        assert!(!h.reglas[1].selector.casa("rect", &[], None, (3, 4)));
        assert!(h.reglas[2].selector.casa("path", &[], None, (1, 4)));
        assert!(h.reglas[3].selector.casa("g", &[], None, (3, 9)));
        assert_eq!(h.reglas[0].selector.especificidad(), (0, 1, 1));
    }

    #[test]
    fn los_fotogramas_se_ordenan() {
        let (h, malos) = hoja("@keyframes gira { to { transform: rotate(360deg) } from { transform: rotate(0deg) } 50%, 75% { opacity: .2 } }");
        assert!(malos.is_empty());
        let f = &h.fotogramas[0];
        assert_eq!(f.nombre, "gira");
        assert_eq!(f.pasos.iter().map(|p| p.0).collect::<Vec<_>>(), vec![0.0, 0.5, 0.75, 1.0]);
    }
}
