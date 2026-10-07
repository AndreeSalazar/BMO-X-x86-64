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
//! ## Las listas (P2, 04-10)
//!
//! `<usa id="amigos" src="amigo.maqueta" repite="6" entre="4"/>`: la pieza
//! hasta 6 veces en columna. Se maqueta y se juzga con TODAS (lo peor que
//! puede pasar), y se injertan todas con sus ids numerados
//! (`amigos.0.fila`). Cuantas hay de verdad lo dice el aparato: el emisor
//! saca `LISTA_AMIGOS` y no pinta las filas, que pinta el modulo de la fila
//! con sus datos.
//!
//! Los ESTADOS se compilan
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
        // ** UNA LISTA (P2): la fila, `veces` veces en columna. Se maqueta y
        // se juzga con TODAS: lo peor que puede pasar es que esten todas.
        let (w, h) = match nodo.repite {
            Some(r) => r.medida(pieza.canvas),
            None => pieza.canvas,
        };
        if nodo.repite.is_some() && nodo.id.is_none() {
            errores.push(Error::new(
                nodo.span,
                "una lista necesita `id`",
                "es como la encuentra el aparato (sale como `LISTA_<ID>` en el \
                 codigo generado), y como se nombran sus filas: `id.0.boton`, \
                 `id.1.boton`...",
                "por ejemplo `<usa id=\"amigos\" src=\"amigo.maqueta\" repite=\"8\"/>`.",
            ));
        }
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

    // ** LAS IMAGENES (H4): se leen aqui, con el lector del aparato, y su
    // caja mide lo que miden ellas.
    let mut imagenes: HashMap<usize, std::sync::Arc<[u32]>> = HashMap::new();
    let mut usos = Vec::new();
    recoger_tag(&mut c.root, Tag::Imagen, &mut usos);
    for u in usos {
        let nodo = bajar(&mut c.root, &u);
        match imagen(nodo, &dir, leer) {
            Ok(px) => {
                imagenes.insert(nodo.span.start, px);
            }
            Err(e) => errores.push(e),
        }
    }
    // ** LOS DIBUJOS DE FICHERO (MAQUETA 3, S6): `<svg src="logo.svg">`, un
    // SVG de internet tal cual, leido y juzgado por el lector de SVG. Antes de
    // maquetar: sin medida en su regla, mide lo que dice su fichero.
    let mut usos = Vec::new();
    recoger_tag(&mut c.root, Tag::Svg, &mut usos);
    for u in usos {
        let nodo = bajar(&mut c.root, &u);
        let Some(src) = nodo.src.clone() else { continue };
        match leer(&dir.join(&src)) {
            None => errores.push(Error::new(nodo.span, &format!("no se puede leer `{src}`"), "el dibujo se lee al compilar, relativo a este fichero.", "revisar el camino.")),
            Some(b) if b.len() > DIBUJO_MAX => errores.push(Error::new(
                nodo.span,
                &format!("`{src}` mide {} KiB, y un dibujo cabe en {} KiB", b.len() / 1024, DIBUJO_MAX / 1024),
                "el dibujo va DENTRO del codigo generado; uno de mas de un mega es una ilustracion, no un icono.",
                "simplificarlo en el editor, o pintarlo como `<imagen>`.",
            )),
            Some(b) => match bmo_maqueta_dibujo::leer_fichero(&src, &b, nodo.span) {
                Ok(s) => nodo.dibujo = Some(bmo_maqueta_dibujo::Dibujo(std::sync::Arc::new(s))),
                Err(e) => errores.extend(e),
            },
        }
    }
    if !errores.is_empty() {
        return Err(fallo(ruta, &fuente, errores));
    }

    let mut l = lay(&c);
    let juicio = bmo_maqueta_verdict::judge(&l, &c);
    if !juicio.is_empty() {
        return Err(fallo(ruta, &fuente, juicio));
    }
    injertar(&mut l.root, &mut piezas);
    poner_imagenes(&mut l.root, &imagenes);

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
    recoger_tag(n, Tag::Usa, out)
}

/// Los caminos (por indices de hijo) hasta cada caja de la etiqueta `tag`.
fn recoger_tag(n: &mut Styled, tag: Tag, out: &mut Vec<Vec<usize>>) {
    fn ir(n: &Styled, tag: Tag, camino: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if n.tag == tag {
            out.push(camino.clone());
        }
        for (k, c) in n.children.iter().enumerate() {
            camino.push(k);
            ir(c, tag, camino, out);
            camino.pop();
        }
    }
    ir(n, tag, &mut Vec::new(), out);
}

/// Lo mas que mide el fichero de un `<svg src>`.
pub const DIBUJO_MAX: usize = 1 << 20;

/// El lado mas grande de una imagen EMBEBIDA: lo que se mete en el codigo
/// generado. Un icono o un avatar caben; una foto grande es un DATO.
pub const IMAGEN_MAX: u32 = 128;

/// **Los pixeles de una `<imagen>`**, y su medida puesta en la caja.
fn imagen(nodo: &mut Styled, dir: &Path, leer: &dyn Fn(&Path) -> Option<Vec<u8>>) -> Result<std::sync::Arc<[u32]>, Error> {
    let (span, src, es_dato) = (nodo.span, nodo.src.clone(), nodo.hueco.is_some());
    let s = &mut nodo.style;
    let mal = |titulo: &str, por: &str, en: &str| Err(Error::new(span, titulo, por, en));
    if s.padding != [0; 4] || s.border_width != [0; 4] {
        return mal(
            "una `<imagen>` no lleva `padding` ni borde",
            "su caja SON sus pixeles; un relleno o un borde los descolocaria.",
            "ponerlo en un `<div>` alrededor.",
        );
    }
    let (px, w, h) = match src {
        Some(src) => {
            let Some(bytes) = leer(&dir.join(&src)) else {
                return mal(&format!("no se puede leer `{src}`"), "la imagen se lee al compilar, relativa a este fichero.", "revisar el camino.");
            };
            let m = match bmo_imagen::medir(&bytes) {
                Ok(m) => m,
                Err(e) => return mal(&format!("`{src}` no es una imagen que se sepa leer ({e:?})"), "se lee con `bmo-imagen`: QOI, BMP o PNG.", "convertirla a `.qoi`."),
            };
            let mut px = vec![0u32; (m.ancho * m.alto) as usize];
            let mut taller = vec![0u8; bmo_imagen::TALLER];
            if let Err(e) = bmo_imagen::decodificar_con(&bytes, &mut px, &mut taller) {
                return mal(&format!("`{src}` no se deja leer entera ({e:?})"), "un fichero roto no se embebe a medias.", "volver a exportarla.");
            }
            (px, m.ancho, m.alto)
        }
        // Un dato sin muestra: la caja dice su medida y la foto pinta un
        // damero apagado, para que se vea que ahi va algo.
        None => {
            let (Some(w), Some(h)) = (s.width, s.height) else {
                return mal(
                    "una `<imagen dato>` sin muestra tiene que decir su `width` y su `height`",
                    "los pixeles llegan al ejecutar; la caja es lo que se juzga, y tiene \
                     que tener medida. Con `src` de muestra, mide lo que la muestra.",
                    "por ejemplo `.mini { width:220px; height:124px }`.",
                );
            };
            let damero = (0..w * h).map(|k| if ((k % w) / 8 + (k / w) / 8) % 2 == 0 { 0xFF2A_3242 } else { 0xFF22_2936 }).collect();
            (damero, w, h)
        }
    };
    let dijo = |v: Option<u32>, real: u32| v.is_some_and(|v| v != real);
    if dijo(s.width, w) || dijo(s.height, h) {
        return mal(
            &format!("la imagen mide {w}x{h}, y su caja pide otra medida"),
            "BMO-X no escala: una imagen se pinta pixel a pixel, y estirarla haria \
             que lo juzgado no fuera lo pintado.",
            "quitar `width`/`height`, o exportar la imagen a la medida.",
        );
    }
    if !es_dato && (w > IMAGEN_MAX || h > IMAGEN_MAX) {
        return mal(
            &format!("la imagen mide {w}x{h}: embebida, como mucho {IMAGEN_MAX}x{IMAGEN_MAX}"),
            "sus pixeles van DENTRO del codigo generado. Un icono o un avatar caben; \
             una foto grande es un DATO, que llega al ejecutar.",
            "`<imagen dato=\"foto\" src=\"muestra.qoi\"/>`, o una imagen mas chica.",
        );
    }
    s.width = Some(w);
    s.height = Some(h);
    Ok(px.into())
}

/// Pone a cada `<imagen>` maquetada sus pixeles.
fn poner_imagenes(f: &mut Frame, imagenes: &HashMap<usize, std::sync::Arc<[u32]>>) {
    if f.tag == Tag::Imagen && f.imagen.is_none() {
        f.imagen = imagenes.get(&f.span.start).cloned();
    }
    for c in &mut f.children {
        poner_imagenes(c, imagenes);
    }
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
            match f.repite {
                // Una lista: la fila en cada sitio, nombrada con su numero.
                Some(r) => {
                    f.children = (0..r.veces)
                        .map(|k| {
                            let mut fila = p.root.clone();
                            let prefijo = format!("{}.{k}", f.id.as_deref().unwrap_or("lista"));
                            let (dx, dy) = r.sitio(k, p.canvas);
                            correr(&mut fila, f.rect.x + dx as i32, f.rect.y + dy as i32, Some(&prefijo));
                            fila
                        })
                        .collect();
                }
                None => {
                    let mut raiz = p.root;
                    correr(&mut raiz, f.rect.x, f.rect.y, f.id.as_deref());
                    f.children = vec![raiz];
                }
            }
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
