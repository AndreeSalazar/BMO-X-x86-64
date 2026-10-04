//! # MAQUETA -- COMPONE, un CONSUMIDOR (no una generacion)
//!
//! generacion: ninguna -- es un CONSUMIDOR de la cadena, como `emit`
//!
//! El propietario (04-10): *"podemos tener multiples maqueta_1 hasta el
//! infinito, abstraidas por la maqueta principal"*. Esto es la primera mitad:
//! **componer**. Una maqueta pone otra dentro con `<usa src="fila.maqueta"/>`.
//!
//! ## La ley que lo ordena: una pieza no sabe que tiene padre (L7)
//!
//! ```text
//!    1. la PIEZA se compila SOLA: su cascada, su maquetacion y su veredicto.
//!       Sus reglas no salen de ella y las de la principal no entran.
//!    2. para la principal, `<usa>` es una caja HOJA de la medida exacta que la
//!       pieza calculo. Ponerle otra medida, `padding` o borde es un error:
//!       una pieza mide lo que mide.
//!    3. la principal se maqueta y se JUZGA con esas hojas -- lo que juzga de
//!       ella (que cabe, que mide algo) no depende de lo de dentro.
//!    4. DESPUES se injerta cada pieza ya maquetada en su hoja, corrida a su
//!       sitio. Los emisores reciben un arbol y no se enteran de las piezas.
//! ```
//!
//! Los `id` de una pieza puesta en un `<usa id="p">` salen como `p.boton`: la
//! misma pieza dos veces no choca consigo misma en la tabla de golpeo.
//!
//! ## Lo que no hace (todavia)
//!
//! Repetir una pieza N veces con N de la ejecucion. Los ESTADOS se compilan
//! aqui (`compilar_estados`); la transicion entre ellos se empareja y se
//! mezcla trazo a trazo en `emit` (`movimiento.rs`). Ver
//! `docs/plan/PLAN_MAQUETA.md`, seccion 6d.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bmo_maqueta_cascade::{cascade, Styled};
use bmo_maqueta_diag::{render, Error, Span};
use bmo_maqueta_layout::{lay, Frame, Laid, Rect};
use bmo_maqueta_node::{parse, Tag};

/// Lo hondo que se compone: una pieza dentro de otra, hasta aqui.
pub const HONDO: usize = 8;

/// **Un fallo, con el fichero donde esta.** Una pieza que no compila se
/// cuenta en SU fichero, no en el de quien la usa.
#[derive(Debug)]
pub struct Fallo {
    pub fichero: String,
    pub fuente: Vec<u8>,
    pub errores: Vec<Error>,
}

impl Fallo {
    pub fn render(&self) -> String {
        render(&self.fichero, &self.fuente, &self.errores)
    }
}

/// **Compila una maqueta y sus piezas**, leyendo del disco.
pub fn compilar(ruta: &Path) -> Result<Laid, Fallo> {
    compilar_con(ruta, &|p: &Path| std::fs::read(p).ok())
}

/// Lo mismo, con quien lee los ficheros: las pruebas leen de memoria.
pub fn compilar_con(ruta: &Path, leer: &dyn Fn(&Path) -> Option<Vec<u8>>) -> Result<Laid, Fallo> {
    let mut pila = Vec::new();
    una(ruta, leer, &mut pila, None)
}

/// **Una maqueta en TODOS sus estados** (P3): el reposo y cada `@estado`,
/// cada uno maquetado ENTERO y juzgado. Un texto que no cabe en "abierta" no
/// compila, aunque en el reposo quepa.
pub struct Estados {
    pub reposo: Laid,
    pub otros: Vec<(String, Laid)>,
}

impl Estados {
    /// El estado por su nombre (`reposo` incluido).
    pub fn de(&self, nombre: &str) -> Option<&Laid> {
        if nombre == "reposo" {
            return Some(&self.reposo);
        }
        self.otros.iter().find(|(n, _)| n == nombre).map(|(_, l)| l)
    }
}

pub fn compilar_estados(ruta: &Path) -> Result<Estados, Fallo> {
    compilar_estados_con(ruta, &|p: &Path| std::fs::read(p).ok())
}

pub fn compilar_estados_con(ruta: &Path, leer: &dyn Fn(&Path) -> Option<Vec<u8>>) -> Result<Estados, Fallo> {
    let reposo = compilar_con(ruta, leer)?;
    let fuente = leer(ruta).unwrap_or_default();
    let nombres = parse(&fuente).map(|d| d.estados()).unwrap_or_default();
    let mut otros = Vec::new();
    for n in nombres {
        let mut pila = Vec::new();
        let l = una(ruta, leer, &mut pila, Some(&n)).map_err(|mut f| {
            for e in &mut f.errores {
                e.title = format!("en el estado `{n}`: {}", e.title);
            }
            f
        })?;
        otros.push((n, l));
    }
    Ok(Estados { reposo, otros })
}

fn fallo(ruta: &Path, fuente: &[u8], errores: Vec<Error>) -> Fallo {
    Fallo { fichero: ruta.display().to_string(), fuente: fuente.to_vec(), errores }
}

fn una(ruta: &Path, leer: &dyn Fn(&Path) -> Option<Vec<u8>>, pila: &mut Vec<PathBuf>, estado: Option<&str>) -> Result<Laid, Fallo> {
    let Some(fuente) = leer(ruta) else {
        return Err(Fallo {
            fichero: ruta.display().to_string(),
            fuente: Vec::new(),
            errores: vec![Error::new(Span::new(0, 0, 1, 1), "no se puede leer este fichero", "no existe, o no se deja leer.", "revisar el camino.")],
        });
    };
    let doc = parse(&fuente).map_err(|e| fallo(ruta, &fuente, e))?.en_estado(estado);
    let mut c = cascade(&doc).map_err(|e| fallo(ruta, &fuente, e))?;

    // Cada `<usa>`: su pieza, compilada sola, y su medida puesta en la hoja.
    pila.push(ruta.to_path_buf());
    let mut piezas: HashMap<usize, Laid> = HashMap::new();
    let mut errores = Vec::new();
    let dir = ruta.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut usos = Vec::new();
    recoger(&mut c.root, &mut usos);
    for u in usos {
        // `recoger` devolvio el camino (indices de hijo) hasta cada `<usa>`, y
        // aqui se baja otra vez por el: el arbol no cambia de forma entre medias.
        let nodo = bajar(&mut c.root, &u);
        let src = nodo.src.clone().unwrap_or_default();
        let camino = dir.join(&src);
        if pila.iter().any(|p| p == &camino) {
            errores.push(Error::new(
                nodo.span,
                &format!("`{src}` se usa a si misma"),
                "una pieza que (por otras) acaba conteniendose no tiene medida: \
                 seria infinita.",
                "romper el ciclo.",
            ));
            continue;
        }
        if pila.len() >= HONDO {
            errores.push(Error::new(
                nodo.span,
                &format!("demasiadas piezas una dentro de otra (mas de {HONDO})"),
                "cada nivel es una maqueta entera; tantos niveles casi siempre son \
                 un ciclo que pasa por caminos distintos.",
                "aplanar: que la principal use las piezas directamente.",
            ));
            continue;
        }
        // Una pieza va siempre en SU reposo: sus estados son suyos (P3b).
        let pieza = una(&camino, leer, pila, None)?;
        let (w, h) = pieza.canvas;
        let s = &mut nodo.style;
        let dijo = |v: Option<u32>, real: u32| v.is_some_and(|v| v != real);
        if dijo(s.width, w) || dijo(s.height, h) {
            errores.push(Error::new(
                nodo.span,
                &format!("`{src}` mide {w}x{h}, y aqui se le pide otra medida"),
                "una pieza mide lo que ELLA calcula: se compila sola, y estirarla \
                 desde fuera haria que lo juzgado no fuera lo pintado.",
                "quitar `width`/`height` de este `<usa>`, o cambiar la pieza.",
            ));
        }
        if s.padding != [0; 4] || s.border_width != [0; 4] {
            errores.push(Error::new(
                nodo.span,
                "un `<usa>` no lleva `padding` ni borde",
                "su caja ES la pieza; un relleno o un borde aqui la descolocaria \
                 de lo que se juzgo.",
                "ponerlo en la pieza, o en un `<div>` alrededor.",
            ));
        }
        s.width = Some(w);
        s.height = Some(h);
        piezas.insert(nodo.span.start, pieza);
    }
    pila.pop();
    if !errores.is_empty() {
        return Err(fallo(ruta, &fuente, errores));
    }

    let mut l = lay(&c);
    let juicio = bmo_maqueta_verdict::judge(&l, &c);
    if !juicio.is_empty() {
        return Err(fallo(ruta, &fuente, juicio));
    }
    injertar(&mut l.root, &mut piezas);

    // Dos `id` iguales despues de injertar: la tabla de golpeo no sabria a
    // quien contestar.
    let mut vistos: HashMap<String, Span> = HashMap::new();
    let mut repetidos = Vec::new();
    for f in l.all() {
        if let Some(id) = &f.id {
            if vistos.insert(id.clone(), f.span).is_some() {
                repetidos.push(Error::new(
                    f.span,
                    &format!("el id `{id}` sale dos veces al componer"),
                    "el id es la clave de la tabla de golpeo: con dos iguales un clic \
                     contesta lo que no es. Al poner la misma pieza dos veces, sus ids \
                     se repiten.",
                    "dar un `id` a cada `<usa>`: los de dentro salen como `usa.id`.",
                ));
            }
        }
    }
    if !repetidos.is_empty() {
        return Err(fallo(ruta, &fuente, repetidos));
    }
    Ok(l)
}

/// Los caminos (por indices de hijo) hasta cada `<usa>`.
fn recoger(n: &mut Styled, out: &mut Vec<Vec<usize>>) {
    fn ir(n: &Styled, camino: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if n.tag == Tag::Usa {
            out.push(camino.clone());
        }
        for (k, c) in n.children.iter().enumerate() {
            camino.push(k);
            ir(c, camino, out);
            camino.pop();
        }
    }
    ir(n, &mut Vec::new(), out);
}

fn bajar<'a>(n: &'a mut Styled, camino: &[usize]) -> &'a mut Styled {
    match camino.split_first() {
        None => n,
        Some((&k, resto)) => bajar(&mut n.children[k], resto),
    }
}

/// Pone cada pieza en su hoja, corrida a su sitio y con los ids nombrados.
fn injertar(f: &mut Frame, piezas: &mut HashMap<usize, Laid>) {
    if f.tag == Tag::Usa {
        if let Some(p) = piezas.remove(&f.span.start) {
            let mut raiz = p.root;
            correr(&mut raiz, f.rect.x, f.rect.y, f.id.as_deref());
            f.children = vec![raiz];
        }
        return;
    }
    for c in &mut f.children {
        injertar(c, piezas);
    }
}

fn correr(f: &mut Frame, dx: i32, dy: i32, prefijo: Option<&str>) {
    let mover = |r: &mut Rect| {
        r.x += dx;
        r.y += dy;
    };
    mover(&mut f.rect);
    mover(&mut f.content);
    if let Some(t) = &mut f.text_at {
        mover(t);
    }
    if let Some(p) = prefijo {
        if let Some(id) = &f.id {
            f.id = Some(format!("{p}.{id}"));
        }
        if let Some(isla) = &f.island {
            f.island = Some(format!("{p}.{isla}"));
        }
    }
    for c in &mut f.children {
        correr(c, dx, dy, prefijo);
    }
}

#[cfg(test)]
mod pruebas;
