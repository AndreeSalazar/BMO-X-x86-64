//! **El XML de un SVG**, leido lo justo: etiquetas, atributos, texto (para
//! `<style>`), comentarios, `<?xml ?>`, `<!DOCTYPE>` y `CDATA`. Las
//! entidades de siempre (`&amp;`, `&#38;`...) se resuelven; una entidad
//! DECLARADA en el `DOCTYPE` es error: es la puerta de las bombas de XML y un
//! dibujo no la necesita.

use crate::Falla;

/// Un elemento, con donde empieza en el texto (para los errores).
#[derive(Clone, Debug)]
pub struct Elemento {
    pub nombre: String,
    pub atributos: Vec<Atributo>,
    pub hijos: Vec<Elemento>,
    /// El texto de dentro, junto (solo lo lee `<style>`).
    pub texto: String,
    pub pos: usize,
    pub largo: usize,
    /// Su sitio entre sus hermanos: `(k, de)`, desde 1 (`:nth-child`).
    pub lugar: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct Atributo {
    pub nombre: String,
    pub valor: String,
    pub pos: usize,
    pub largo: usize,
}

impl Elemento {
    pub fn attr(&self, n: &str) -> Option<&str> {
        self.atributos.iter().find(|a| a.nombre == n).map(|a| a.valor.as_str())
    }
    pub fn atributo(&self, n: &str) -> Option<&Atributo> {
        self.atributos.iter().find(|a| a.nombre == n)
    }
}

struct Lector<'a> {
    s: &'a [u8],
    i: usize,
    fin: usize,
}

impl Lector<'_> {
    fn empieza(&self, p: &str) -> bool {
        self.s[self.i..self.fin].starts_with(p.as_bytes())
    }
    fn blancos(&mut self) {
        while self.i < self.fin && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    /// Salta hasta despues de `fin`; falso si no esta.
    fn hasta(&mut self, fin: &str) -> bool {
        while self.i < self.fin {
            if self.empieza(fin) {
                self.i += fin.len();
                return true;
            }
            self.i += 1;
        }
        false
    }
    fn nombre(&mut self) -> String {
        let ini = self.i;
        while self.i < self.fin && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'-' | b'_' | b':' | b'.')) {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[ini..self.i]).into_owned()
    }
}

/// Las cinco entidades de XML y las numericas.
pub fn entidades(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    let mut out = String::new();
    let mut resto = t;
    while let Some(k) = resto.find('&') {
        out.push_str(&resto[..k]);
        resto = &resto[k..];
        let Some(fin) = resto.find(';') else {
            out.push_str(resto);
            return out;
        };
        let e = &resto[1..fin];
        let c = match e {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            _ if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                resto = &resto[fin + 1..];
            }
            None => {
                out.push('&');
                resto = &resto[1..];
            }
        }
    }
    out.push_str(resto);
    out
}

/// **Lee los elementos** de `s[ini..fin]`. Devuelve los de primer nivel.
pub fn leer(s: &[u8], ini: usize, fin: usize, fallas: &mut Vec<Falla>) -> Vec<Elemento> {
    let mut l = Lector { s, i: ini, fin };
    let mut pila: Vec<Elemento> = Vec::new();
    let mut raiz: Vec<Elemento> = Vec::new();
    let colgar = |pila: &mut Vec<Elemento>, raiz: &mut Vec<Elemento>, e: Elemento| match pila.last_mut() {
        Some(p) => p.hijos.push(e),
        None => raiz.push(e),
    };
    while l.i < l.fin {
        if l.empieza("<!--") {
            let p = l.i;
            if !l.hasta("-->") {
                fallas.push(Falla::en(p, 4, "un comentario `<!--` sin cerrar", "el fichero se acaba dentro del comentario.", "cerrarlo con `-->`."));
            }
        } else if l.empieza("<![CDATA[") {
            l.i += 9;
            let p = l.i;
            let cerrado = l.hasta("]]>");
            let hasta = if cerrado { l.i - 3 } else { l.i };
            if let Some(e) = pila.last_mut() {
                e.texto.push_str(&String::from_utf8_lossy(&s[p..hasta]));
            }
        } else if l.empieza("<?") {
            l.hasta("?>");
        } else if l.empieza("<!DOCTYPE") || l.empieza("<!doctype") {
            let p = l.i;
            // Con `[` dentro, hay declaraciones: hasta `]>`.
            let mut k = l.i;
            let mut con_corchete = false;
            while k < l.fin && s[k] != b'>' {
                if s[k] == b'[' {
                    con_corchete = true;
                    break;
                }
                k += 1;
            }
            if con_corchete {
                let dentro_ini = k;
                l.i = k;
                l.hasta("]>");
                if String::from_utf8_lossy(&s[dentro_ini..l.i]).contains("<!ENTITY") {
                    fallas.push(Falla::en(p, 9, "este SVG declara entidades en su `DOCTYPE`", "una entidad declarada se expande al leer, y es la puerta de las bombas de XML (una entidad que vale mil de otra). Un dibujo no las necesita.", "quitar el bloque `[ ... ]` del `DOCTYPE`; los editores lo escriben solo para Illustrator."));
                }
            } else {
                l.hasta(">");
            }
        } else if l.empieza("</") {
            let p = l.i;
            l.i += 2;
            let n = l.nombre();
            l.hasta(">");
            match pila.pop() {
                Some(mut e) if e.nombre == n => {
                    e.largo = l.i - e.pos;
                    colgar(&mut pila, &mut raiz, e);
                }
                Some(e) => {
                    fallas.push(Falla::en(p, l.i - p, &format!("se cierra `</{n}>` pero lo abierto es `<{}>`", e.nombre), "el SVG tiene que estar bien anidado: un navegador lo repara a su manera y BMO-X no adivina.", &format!("cerrar `</{}>` aqui.", e.nombre)));
                    colgar(&mut pila, &mut raiz, e);
                }
                None => fallas.push(Falla::en(p, l.i - p, &format!("`</{n}>` cierra algo que no esta abierto"), "no hay ninguna etiqueta abierta en este punto.", "borrar el cierre.")),
            }
        } else if l.empieza("<") {
            let p = l.i;
            l.i += 1;
            let nombre = l.nombre();
            if nombre.is_empty() {
                fallas.push(Falla::en(p, 1, "un `<` suelto", "en XML `<` abre una etiqueta; como texto se escribe `&lt;`.", "escribir `&lt;`."));
                continue;
            }
            let mut e = Elemento { nombre, atributos: Vec::new(), hijos: Vec::new(), texto: String::new(), pos: p, largo: 0, lugar: (1, 1) };
            let mut cerrada = false;
            loop {
                l.blancos();
                if l.i >= l.fin {
                    fallas.push(Falla::en(p, 1, &format!("`<{}` se quedo sin cerrar el `>`", e.nombre), "la etiqueta empieza y el dibujo se acaba.", "cerrar con `>` o con `/>`."));
                    break;
                }
                if l.empieza("/>") {
                    l.i += 2;
                    cerrada = true;
                    break;
                }
                if l.empieza(">") {
                    l.i += 1;
                    break;
                }
                let ap = l.i;
                let n = l.nombre();
                if n.is_empty() {
                    fallas.push(Falla::en(l.i, 1, "dentro de la etiqueta hay algo que no es un atributo", "dentro de `<...>` solo van `nombre=\"valor\"`.", "revisar las comillas."));
                    l.i += 1;
                    continue;
                }
                l.blancos();
                if !l.empieza("=") {
                    fallas.push(Falla::en(ap, n.len(), &format!("el atributo `{n}` no tiene valor"), "en XML todo atributo lleva valor.", &format!("`{n}=\"...\"`.")));
                    continue;
                }
                l.i += 1;
                l.blancos();
                let q = if l.i < l.fin { s[l.i] } else { 0 };
                if q != b'"' && q != b'\'' {
                    fallas.push(Falla::en(l.i, 1, "el valor del atributo tiene que ir entre comillas", "sin comillas no se sabe donde acaba.", "por ejemplo `r=\"4\"`."));
                    continue;
                }
                l.i += 1;
                let vi = l.i;
                while l.i < l.fin && s[l.i] != q {
                    l.i += 1;
                }
                let valor = entidades(&String::from_utf8_lossy(&s[vi..l.i]));
                l.i = (l.i + 1).min(l.fin);
                e.atributos.push(Atributo { nombre: n, valor, pos: ap, largo: l.i - ap });
            }
            if cerrada {
                e.largo = l.i - p;
                colgar(&mut pila, &mut raiz, e);
            } else {
                pila.push(e);
            }
        } else {
            let p = l.i;
            while l.i < l.fin && s[l.i] != b'<' {
                l.i += 1;
            }
            if let Some(e) = pila.last_mut() {
                e.texto.push_str(&entidades(&String::from_utf8_lossy(&s[p..l.i])));
            }
        }
    }
    while let Some(mut e) = pila.pop() {
        fallas.push(Falla::en(e.pos, 1, &format!("`<{}>` se abrio y no se cerro", e.nombre), "un SVG roto se lee de mil maneras; BMO-X no elige una.", &format!("escribir `</{}>`.", e.nombre)));
        e.largo = l.fin - e.pos;
        colgar(&mut pila, &mut raiz, e);
    }
    numerar(&mut raiz);
    raiz
}

/// El sitio de cada elemento entre sus hermanos.
pub fn numerar(es: &mut [Elemento]) {
    let n = es.len();
    for (k, e) in es.iter_mut().enumerate() {
        e.lugar = (k + 1, n);
        numerar(&mut e.hijos);
    }
}
