//! **COMO se escribe en Rust.** Traduce; no decide.
//!
//! Lo que hay que dibujar lo dice `orden.rs`, y lo que cae dentro de un rect lo
//! dice `recorte.rs`. Aqui solo se convierte una lista en texto. Ese corte es lo
//! que permite que el recurso BEF y el reflejo en PPM sean **otro traductor** y
//! no otra deduccion.
//!
//! ## Por que codigo y no datos
//!
//! Una tabla habria pedido tipos --`Caja`, `Letras`, `Golpe`-- que tendrian que
//! existir **en los dos lados**: aqui para escribirlos y en Ring 3 para leerlos.
//! Eso es una segunda copia de un contrato, el fallo que este arbol ya pago con
//! `bmo.h`. Emitir llamadas no tiene contrato: usa `Pantalla`, que ya existe.
//!
//! ## [!] Lo que SALE tiene que ser ASCII puro
//!
//! Este fichero puede usar los simbolos de la casa; lo que emite **no**, porque
//! se convierte en un fuente de BMO-X. Se colo un simbolo una vez y lo cazo una
//! prueba, no una lectura.

use crate::literal::{pieza_literal, PX};
use crate::rust_pila_a::limite_de;
use bmo_maqueta_layout::{Laid, Rect};
use std::fmt::Write;

use crate::orden::{lista, Estado, Orden, Trazo};

/// Genera un modulo de Rust a partir de una maquetacion resuelta.
///
/// `origen` es la ruta del `.maqueta`, **relativa a la raiz del repositorio**.
/// Que sea relativa no es estetica: un artefacto cuya primera linea depende de
/// como se tecleo el comando **no se puede comparar**, y comparar es lo unico
/// que impide que la cara pintada y su `.maqueta` se separen.
pub fn modulo(origen: &str, l: &Laid) -> String {
    modulo_con_datos(origen, l, &[])
}

/// **El modulo de una maqueta con DATOS y LISTAS** (P2/H1/H2, 04-10).
///
/// ```text
///    una pieza con huecos        `Datos` (un campo por `{nombre}` y por
///    (`{nombre}`, `--dato-*`)    `--dato-*`), su `MUESTRA`, y `pintar(.., d)`
///                                con lo que llega al ejecutar: el texto se
///                                corta con `...` a su caja (`pieza_cabe`)
///    una maqueta con listas      `LISTA_<ID>`: donde va cada fila. La fila la
///    (`<usa repite>`)            pinta su propio modulo, tantas veces como
///                                datos haya: aqui NO se pintan las muestras
/// ```
///
/// `colores` son los `--dato-*` del fichero (`Document::datos`). Sin datos
/// ni listas, el mismo modulo de siempre, byte a byte.
pub fn modulo_con_datos(origen: &str, l: &Laid, colores: &[(String, u32)]) -> String {
    let podada = podar_listas(l);
    let datos = Datos::de(&podada, colores);
    // Lo de dentro de una ventana que se desplaza (H7) se pinta aparte.
    let ordenes = lista(&crate::desplaza::podar(&podada));
    let ventanas: Vec<String> = crate::desplaza::cajas(&podada)
        .iter()
        .enumerate()
        .map(|(k, f)| format!("desplazar_{}(p, ox, oy, 0{});", nombre_ventana(f, k), if datos.hay() { ", d" } else { "" }))
        .collect();
    let mut s = String::new();
    cabecera(&mut s, origen, l);
    datos.estaticos(&mut s);
    datos.cabecera(&mut s);
    listas(&mut s, l);
    pintar(&mut s, &ordenes, &datos, &ventanas);
    pintar_en(&mut s, &ordenes, &datos);
    // ** Los dibujos que animan (MAQUETA 3, S7).
    if crate::anima::hay(&podada) {
        pintar_en_como(&mut s, &crate::orden::lista_sin_anima(&crate::desplaza::podar(&podada)), &datos, "pintar_en_fijo");
        crate::anima::emitir(&mut s, &podada);
    }
    realce(&mut s, &ordenes, &datos);
    realce_animado(&mut s, &podada, &datos);
    desplazamientos(&mut s, &podada, &datos);
    golpe(&mut s, l);
    crate::rust_pila_a::puntero(&mut s, l);
    islas(&mut s, l);
    datos.tabla.emitir(&mut s);
    s
}

/// Lleva DATOS esta maqueta (sin contar los de las filas de sus listas, que
/// son de la fila)?
pub fn tiene_datos(l: &Laid, colores: &[(String, u32)]) -> bool {
    Datos::de(&podar_listas(l), colores).hay()
}

/// La maquetacion sin las filas de las listas: las pinta el aparato con sus
/// datos, no aqui con las muestras.
fn podar_listas(l: &Laid) -> Laid {
    fn ir(f: &mut bmo_maqueta_layout::Frame) {
        if f.repite.is_some() {
            f.children.clear();
        }
        f.children.iter_mut().for_each(ir);
    }
    let mut p = l.clone();
    ir(&mut p.root);
    p
}

// ------------------------------------------------------------------------
//  Los datos (H1/H2)
// ------------------------------------------------------------------------

/// Un texto que llega al ejecutar: donde empieza (su `text_at`), su campo,
/// hasta donde puede llegar y su muestra.
struct Hueco {
    x: i32,
    y: i32,
    nombre: String,
    max: u32,
    muestra: String,
}

#[derive(Default)]
struct Datos {
    huecos: Vec<Hueco>,
    colores: Vec<(String, u32)>,
    /// Las `<imagen dato>`: campo, ancho, alto, y sus pixeles de muestra.
    fotos: Vec<(String, u32, u32, std::sync::Arc<[u32]>)>,
    /// TODOS los pixeles que se embeben (fijos y muestras), en orden: el
    /// `static IMAGEN_n` de cada uno.
    pixeles: Vec<std::sync::Arc<[u32]>>,
    /// Las figuras que van a `static FIGURAS` (`tabla.rs`).
    tabla: crate::tabla::Tabla,
}

impl Datos {
    fn de(l: &Laid, colores: &[(String, u32)]) -> Datos {
        let mut huecos = Vec::new();
        for f in l.all() {
            if let (Some(nombre), Some(t), Some(m)) = (&f.hueco, f.text_at, &f.text) {
                let max = (f.content.right() - t.x as i64).max(0) as u32;
                huecos.push(Hueco { x: t.x, y: t.y, nombre: nombre.clone(), max, muestra: m.clone() });
            }
        }
        let mut fotos: Vec<(String, u32, u32, std::sync::Arc<[u32]>)> = Vec::new();
        let mut pixeles: Vec<std::sync::Arc<[u32]>> = Vec::new();
        for f in l.all() {
            if let (bmo_maqueta_node::Tag::Imagen, Some(px)) = (f.tag, &f.imagen) {
                if !pixeles.iter().any(|p| std::sync::Arc::ptr_eq(p, px)) {
                    pixeles.push(px.clone());
                }
                if let Some(n) = &f.hueco {
                    if !fotos.iter().any(|(o, ..)| o == n) {
                        fotos.push((n.clone(), f.rect.w, f.rect.h, px.clone()));
                    }
                }
            }
        }
        Datos { huecos, colores: colores.to_vec(), fotos, pixeles, tabla: Default::default() }
    }

    fn hay(&self) -> bool {
        !self.huecos.is_empty() || !self.colores.is_empty() || !self.fotos.is_empty()
    }

    /// El numero del `static IMAGEN_n` de estos pixeles.
    fn imagen(&self, px: &std::sync::Arc<[u32]>) -> usize {
        self.pixeles.iter().position(|p| std::sync::Arc::ptr_eq(p, px)).unwrap_or(0)
    }

    /// Los `static IMAGEN_n`: los pixeles que se embeben, de 8 en 8 por linea.
    fn estaticos(&self, s: &mut String) {
        for (k, px) in self.pixeles.iter().enumerate() {
            let _ = writeln!(s, "/// Pixeles `0xAARRGGBB` (el alfa es un bit), embebidos al compilar (H4).\nstatic IMAGEN_{k}: [u32; {}] = [", px.len());
            for trozo in px.chunks(8) {
                let v: Vec<String> = trozo.iter().map(|c| format!("0x{c:08X}")).collect();
                let _ = writeln!(s, "    {},", v.join(", "));
            }
            s.push_str("];\n\n");
        }
    }

    /// `, d: &Datos` en las firmas de un modulo con datos.
    fn param(&self) -> &'static str {
        if self.hay() { ", d: &Datos" } else { "" }
    }

    fn hueco(&self, t: &Trazo) -> Option<&Hueco> {
        match t {
            Trazo::Letra { r, .. } => self.huecos.iter().find(|h| h.x == r.x && h.y == r.y),
            _ => None,
        }
    }

    /// Los campos de texto, sin repetir (un campo puede salir dos veces).
    fn textos(&self) -> Vec<&Hueco> {
        let mut v: Vec<&Hueco> = Vec::new();
        for h in &self.huecos {
            if !v.iter().any(|o| o.nombre == h.nombre) {
                v.push(h);
            }
        }
        v
    }

    /// Cada muestra de color, cambiada por su dato: `0x00RRGGBB` es
    /// `d.campo`, y con alfa (un resplandor), `(0xAA000000 | d.campo)`. La
    /// muestra no sale de otra forma en el fichero (lo comprueba el padre).
    fn colores(&self, linea: String) -> String {
        let mut s = linea;
        for (nombre, v) in &self.colores {
            let rgb = format!("{v:06X}");
            let mut fuera = String::with_capacity(s.len());
            let mut resto = s.as_str();
            while let Some(k) = resto.find("0x") {
                let (antes, desde) = resto.split_at(k);
                fuera.push_str(antes);
                let lit = desde.get(..10).unwrap_or("");
                let es = lit.len() == 10
                    && lit[2..].bytes().all(|b| b.is_ascii_hexdigit())
                    && lit[4..] == rgb
                    && !desde[10..].bytes().next().is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
                if es {
                    let alfa = &lit[2..4];
                    if alfa == "00" {
                        let _ = write!(fuera, "d.{nombre}");
                    } else {
                        let _ = write!(fuera, "(0x{alfa}00_0000 | d.{nombre})");
                    }
                    resto = &desde[10..];
                } else {
                    fuera.push_str("0x");
                    resto = &desde[2..];
                }
            }
            fuera.push_str(resto);
            s = fuera;
        }
        s
    }

    fn cabecera(&self, s: &mut String) {
        if !self.hay() {
            return;
        }
        let textos = self.textos();
        let vida = if textos.is_empty() && self.fotos.is_empty() { "" } else { "<'a>" };
        s.push_str(
            "// == LOS DATOS (H1) ==================================================\n\
             //\n\
             // Lo que llega al ejecutar. La maqueta se juzgo con la MUESTRA; lo que\n\
             // se juzga de un dato es su caja, y el texto que no quepa se corta con\n\
             // `...` en el aparato (`Pantalla::pieza_cabe`), sin maquetar nada.\n\
             \n",
        );
        let _ = writeln!(s, "/// Lo que pinta esta pieza y no se sabia al compilar.\n#[derive(Clone, Copy)]\npub struct Datos{vida} {{");
        for h in &textos {
            let _ = writeln!(s, "    /// `{{{}}}`: se corta a {} px.\n    pub {}: &'a [u8],", h.nombre, h.max, h.nombre);
        }
        for (n, _) in &self.colores {
            let _ = writeln!(s, "    /// `--dato-{n}`: un color `0x00RRGGBB`.\n    pub {n}: u32,");
        }
        for (n, w, h, _) in &self.fotos {
            let _ = writeln!(s, "    /// `<imagen dato=\"{n}\">`: {w}x{h} pixeles `0xAARRGGBB`. Si no miden eso, no se pinta.\n    pub {n}: &'a [u32],");
        }
        s.push_str("}\n\n");
        let _ = write!(
            s,
            "/// La muestra: con lo que se maqueto, se juzgo y sale en la foto.\npub const MUESTRA: Datos{} = Datos {{ ",
            if vida.is_empty() { "" } else { "<'static>" }
        );
        let mut campos: Vec<String> = textos.iter().map(|h| format!("{}: b{:?}", h.nombre, h.muestra)).collect();
        campos.extend(self.colores.iter().map(|(n, v)| format!("{n}: 0x{v:08X}")));
        campos.extend(self.fotos.iter().map(|(n, _, _, px)| format!("{n}: &IMAGEN_{}", self.imagen(px))));
        let _ = writeln!(s, "{} }};\n", campos.join(", "));
    }
}

/// Las listas (`<usa repite>`): donde va cada fila.
fn listas(s: &mut String, l: &Laid) {
    let ls: Vec<_> = l.all().into_iter().filter(|f| f.repite.is_some()).collect();
    if ls.is_empty() {
        return;
    }
    s.push_str(
        "// == LAS LISTAS (P2) =================================================\n\
         //\n\
         // La fila es su propia pieza (su modulo, con sus `Datos`); aqui solo se\n\
         // dice DONDE va cada una. Cuantas hay lo dice quien pinta, hasta `max`:\n\
         // la maqueta se juzgo con todas.\n\
         \n\
         /// Una lista: la primera fila en `(x, y)`, cada una `paso` mas abajo.\n\
         #[derive(Clone, Copy)]\n\
         pub struct Lista {\n\
         \x20   pub x: u32,\n\
         \x20   pub y: u32,\n\
         \x20   pub ancho: u32,\n\
         \x20   pub alto_fila: u32,\n\
         \x20   pub paso: u32,\n\
         \x20   pub max: usize,\n\
         \x20   /// Una rejilla (H6): cuantas por fila, y cada una `paso_x` a la derecha.\n\
         \x20   pub columnas: usize,\n\
         \x20   pub paso_x: u32,\n\
         }\n\
         \n\
         impl Lista {\n\
         \x20   /// Donde va la pieza `i`, relativo al origen.\n\
         \x20   pub const fn fila(&self, i: usize) -> (u32, u32) {\n\
         \x20       let c = if self.columnas == 0 { 1 } else { self.columnas };\n\
         \x20       (self.x + (i % c) as u32 * self.paso_x, self.y + (i / c) as u32 * self.paso)\n\
         \x20   }\n\
         }\n\
         \n",
    );
    for f in ls {
        let r = f.repite.expect("filtrado arriba");
        let (filas, cols) = (r.filas(), r.columnas.max(1).min(r.veces));
        let alto = (f.rect.h - r.entre * (filas - 1)) / filas;
        let ancho = (f.rect.w - r.entre * (cols - 1)) / cols;
        let id = f.id.as_deref().unwrap_or("lista").to_ascii_uppercase().replace(['.', '-'], "_");
        let que = if cols > 1 { format!("en {cols} columnas, ") } else { String::new() };
        let _ = writeln!(
            s,
            "/// `{}`: hasta {} piezas de {ancho}x{alto}, {que}{}x{} con todas.\n\
             pub const LISTA_{id}: Lista = Lista {{ x: {}, y: {}, ancho: {ancho}, alto_fila: {alto}, paso: {}, max: {}, columnas: {cols}, paso_x: {} }};\n",
            f.src.as_deref().unwrap_or("?"),
            r.veces,
            f.rect.w,
            f.rect.h,
            f.rect.x,
            f.rect.y,
            alto + r.entre,
            r.veces,
            ancho + r.entre
        );
    }
}

fn cabecera(s: &mut String, origen: &str, l: &Laid) {
    let _ = write!(
        s,
        "//! GENERADO POR MAQUETA DESDE `{origen}` -- NO EDITAR A MANO.\n\
         //!\n\
         //! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el\n\
         //!                     compositor se lo pide, y el compositor solo pinta si\n\
         //!                     algo cambio (L6h)\n\
         //!\n\
         //! Lo que se edita es el `.maqueta`. Cambiar esto es escribir una verdad\n\
         //! que la siguiente compilacion borra.\n\
         //!\n\
         //! Todas las coordenadas son relativas al origen que se pase, asi que\n\
         //! este modulo no sabe donde esta la ventana -- igual que no lo sabia el\n\
         //! `.maqueta`.\n\
         \n\
         #![allow(clippy::identity_op, clippy::erasing_op)]\n\
         // [!] `dead_code` aparte, y con motivo: este modulo ofrece la superficie\n\
         // ENTERA de la maquetacion --pintar, recortar, realzar, golpear, las\n\
         // islas-- y cual de esas usa la app es cosa de la app. Recortar lo que\n\
         // hoy no se llama obligaria a regenerar el dia que alguien lo use.\n\
         #![allow(dead_code)]\n\
         \n\
         // EL recorte de la casa, no uno propio: el mismo `Recorte` que usan el\n\
         // rasterizador y el kernel, medio abierto `[x0, x1)`. `bmo-dibujo` nacio\n\
         // porque hubo DOS --uno recortaba y otro descartaba-- y se tiraban 2.625\n\
         // de 8.775 rectangulos por fotograma.\n\
         use bmo_dibujo::Recorte;\n\
         use bmo_userland as bmo;\n\
         \n\
         /// La medida que MAQUETA dedujo del arbol. Nadie lo escribio.\n\
         pub const ANCHO: u32 = {};\n\
         pub const ALTO: u32 = {};\n\
         \n",
        l.canvas.0, l.canvas.1
    );
}

// ------------------------------------------------------------------------
//  Un trazo, escrito
// ------------------------------------------------------------------------

/// La llamada que pinta este trazo, con su dato si lleva (H1).
fn llamada_con(t: &Trazo, d: &Datos, limite: &str) -> String {
    if let (Trazo::Imagen { px, dato, .. }, Some(l)) = (t, t.con_pieza(pieza_literal)) {
        let de = match dato {
            Some(n) => format!("d.{n}"),
            None => format!("&IMAGEN_{}", d.imagen(px)),
        };
        return format!("p.pieza(&{}, ox as i32, oy as i32, {limite});", l.replace(PX, &de));
    }
    let l = match (d.hueco(t), t.con_pieza(pieza_literal)) {
        (Some(h), Some(l)) => {
            let l = l.replacen(&format!("texto: b{:?}", h.muestra), &format!("texto: d.{}", h.nombre), 1);
            format!("p.pieza_cabe(&{l}, {}, ox as i32, oy as i32, {limite});", h.max)
        }
        (None, Some(l)) => format!("p.pieza(&{l}, ox as i32, oy as i32, {limite});"),
        _ => llamada(t),
    };
    d.colores(l)
}

/// La llamada que pinta este trazo, sin recortar.
fn llamada(t: &Trazo) -> String {
    if let Some(l) = t.con_pieza(pieza_literal) {
        // [!] Una imagen en una TRANSICION todavia no (P3c): `&[]` no pinta
        // nada, y la CLI no deja llegar aqui una maqueta con imagenes y estados.
        return format!("p.pieza(&{}, ox as i32, oy as i32, None);", l.replace(PX, "&[]"));
    }
    match t {
        Trazo::Rect { r, color } => format!(
            "p.rect(ox + {}, oy + {}, {}, {}, 0x{color:08X});",
            r.x, r.y, r.w, r.h
        ),
        Trazo::Texto { r, texto, color } => format!(
            "p.texto(ox + {}, oy + {}, {texto:?}, 0x{color:08X});",
            r.x, r.y
        ),
        // Las suaves salieron arriba, por `con_pieza`.
        _ => String::new(),
    }
}

/// Una racha de figuras para `static FIGURAS` (`tabla.rs`); con datos, nunca.
fn racha(d: &Datos, ordenes: &[&Orden], i: usize) -> Option<(usize, usize)> {
    if d.hay() {
        return None;
    }
    d.tabla.racha(ordenes, i)
}

fn area(t: &Trazo) -> Rect {
    t.area()
}

// ------------------------------------------------------------------------
//  Los cuatro pintados
// ------------------------------------------------------------------------

fn pintar(s: &mut String, ordenes: &[Orden], d: &Datos, ventanas: &[String]) {
    let _ = writeln!(
        s,
        "/// Pinta la maquetacion entera con su esquina superior izquierda en\n\
         /// `(ox, oy)`. El orden es el del fichero, que ES el orden de pintado.\n\
         pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32{}) {{",
        d.param()
    );
    if d.hay() {
        s.push_str("    let _ = d;\n");
    }
    let mut ultimo = String::new();
    let reposo: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Reposo).collect();
    let mut i = 0;
    while i < reposo.len() {
        let o = reposo[i];
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        if let Some((a, b)) = racha(d, &reposo, i) {
            let _ = writeln!(s, "{}", crate::tabla::bucle(a, b));
            i += b - a;
            continue;
        }
        let _ = writeln!(s, "    {}", llamada_con(&o.trazo, d, &limite_de("None", o.recorte)));
        i += 1;
    }
    for v in ventanas {
        let _ = writeln!(s, "    {v}");
    }
    s.push_str("}\n\n");
}

/// ** El pintado RECORTADO, que es lo que hace barato reparar un danio.
fn pintar_en(s: &mut String, ordenes: &[Orden], d: &Datos) {
    pintar_en_como(s, ordenes, d, "pintar_en");
}

/// `pintar_en` con otro nombre y otras ordenes: `pintar_en_fijo` (S7) es
/// lo mismo SIN los dibujos que animan -- lo que hay debajo de ellos.
fn pintar_en_como(s: &mut String, ordenes: &[Orden], d: &Datos, nombre: &str) {
    if nombre != "pintar_en" {
        let _ = writeln!(s, "/// Lo mismo que `pintar_en`, sin los dibujos que animan (S7): lo que hay
/// DEBAJO de ellos, para repintarlo antes de cada paso.");
    }
    s.push_str(
        "/// Repinta SOLO lo que cae dentro de `(cx, cy, cw, ch)`, en coordenadas\n\
         /// de pantalla. Para devolver el fondo de un area sin repintarlo todo.\n\
         ///\n\
         /// ** Por que existe, con el numero: devolver fondo preguntando el color\n\
         /// PIXEL A PIXEL cuesta ~325.000 escrituras por borrado, que a los\n\
         /// ~300 MB/s medidos en el Ryzen son 4,33 ms -- la cuarta parte de un\n\
         /// fotograma de 60 Hz, y arrastrar hace uno por evento de raton. Esto son\n\
         /// unas pocas llamadas a `rect`, que escriben por filas.\n\
         ///\n\
         /// El recorte es el `Recorte` de `bmo-dibujo`, medio abierto `[x0, x1)`:\n\
         /// uno solo para las tres orillas.\n\
         ///\n\
         /// Los rectangulos se RECORTAN; el texto entra entero o no entra, porque\n\
         /// medio glifo no se puede pintar.\n\
         pub fn ",
    );
    let _ = write!(s, "{nombre}(p: &bmo::Pantalla, ox: u32, oy: u32, cx: u32, cy: u32, cw: u32, ch: u32");
    let _ = writeln!(s, "{}) {{\n    let limite = Recorte::nuevo(cx as i32, cy as i32, cw as i32, ch as i32);", d.param());
    if d.hay() {
        s.push_str("    let _ = d;\n");
    }
    let reposo: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Reposo).collect();
    if reposo.is_empty() {
        s.push_str("    let _ = (p, ox, oy, limite);\n");
    }
    let mut ultimo = String::new();
    let mut i = 0;
    while i < reposo.len() {
        let o = reposo[i];
        i += 1;
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        if let Some((a, b)) = racha(d, &reposo, i - 1) {
            let _ = writeln!(s, "{}", crate::tabla::bucle_en(a, b));
            i += b - a - 1;
            continue;
        }
        let r = area(&o.trazo);
        let caja = format!(
            "Recorte::nuevo(ox as i32 + {}, oy as i32 + {}, {}, {})",
            r.x, r.y, r.w, r.h
        );
        match &o.trazo {
            Trazo::Rect { color, .. } => {
                let linea = format!(
                    "    let c = {caja}.interseccion(&limite);\n\
                     \x20   if !c.vacio() {{\n\
                     \x20       p.rect(c.x0 as u32, c.y0 as u32, c.ancho() as u32, c.alto() as u32, 0x{color:08X});\n\
                     \x20   }}"
                );
                let _ = writeln!(s, "{}", d.colores(linea));
            }
            Trazo::Texto { texto, color, .. } => {
                let _ = writeln!(
                    s,
                    "    if !{caja}.interseccion(&limite).vacio() {{\n\
                     \x20       p.texto(ox + {}, oy + {}, {texto:?}, 0x{color:08X});\n\
                     \x20   }}",
                    r.x, r.y
                );
            }
            // Las suaves, enteras pero RECORTADAS al pintar: un borde suave
            // mezclado dos veces se oscurece.
            otro => {
                if otro.con_pieza(pieza_literal).is_some() {
                    let l = llamada_con(otro, d, &limite_de("Some(limite)", o.recorte));
                    let _ = writeln!(s, "    if !{caja}.interseccion(&limite).vacio() {{\n        {l}\n    }}");
                }
            }
        }
    }
    s.push_str("}\n\n");
}

/// El estado "encima", que es todo lo que MAQUETA sabe de animacion.
///
/// ** No recoloca nada, porque no puede: el padre no deja que una regla `:hover`
/// toque mas que pintura. Por eso cuesta un rect y no un recalculo.
fn realce(s: &mut String, ordenes: &[Orden], d: &Datos) {
    let _ = writeln!(
        s,
        "/// Repinta la caja `id` con sus colores de `:hover`. Llamalo cuando el\n\
         /// puntero entre, y `pintar` cuando salga.\n\
         pub fn realce(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str{}) {{",
        d.param()
    );
    if d.hay() {
        s.push_str("    let _ = d;\n");
    }
    let encima: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Encima).collect();
    if encima.is_empty() {
        s.push_str("    let _ = (p, ox, oy, id);\n");
    }
    let mut abierto = String::new();
    for o in encima {
        if o.de != abierto {
            if !abierto.is_empty() {
                s.push_str("        return;\n    }\n");
            }
            // `de` es `#k_c`; el `id` del golpeo es `k_c`.
            let _ = writeln!(s, "    if id == {:?} {{", o.de.trim_start_matches('#'));
            abierto = o.de.clone();
        }
        let _ = writeln!(s, "        {}", llamada_con(&o.trazo, d, &limite_de("None", o.recorte)));
    }
    if !abierto.is_empty() {
        s.push_str("        return;\n    }\n");
    }
    s.push_str("}\n\n");
}

/// ** H8 (04-10): EL REALCE CON SU TRANSICION. Una caja con `:hover` y
/// `transition` no cambia de golpe: se mezcla pieza a pieza con su curva,
/// como una transicion de estado (P3b), y con la MISMA mezcla. Sin ninguna
/// caja asi, no se emite nada (los modulos de siempre no cambian).
fn realce_animado(s: &mut String, l: &Laid, d: &Datos) {
    let cajas: Vec<_> = l
        .all()
        .into_iter()
        .filter(|f| f.id.is_some() && f.hover.is_some() && f.style.transicion.is_some())
        .collect();
    if cajas.is_empty() {
        return;
    }
    s.push_str(
        "/// Lo que tarda el realce de `id` en entrar (o en salir), en ms. 0 si\n\
         /// cambia de golpe (su caja no dijo `transition`).\n\
         pub fn realce_dura(id: &str) -> u32 {\n\
         \x20   match id {\n",
    );
    let mut pares = Vec::new();
    for f in &cajas {
        let mut encima = (*f).clone();
        encima.style = f.hover.expect("filtrado arriba");
        encima.children.clear();
        let mut quieta = (*f).clone();
        quieta.children.clear();
        let entra = crate::movimiento::pares_caja(&quieta, &encima);
        let sale = crate::movimiento::pares_caja(&encima, &quieta);
        let id = f.id.clone().expect("filtrado arriba");
        let _ = writeln!(s, "        {id:?} => {},", crate::movimiento::duracion(&entra));
        // Lo de DENTRO de la caja, quieto y encima: repintar su fondo lo
        // taparia.
        let dentro: Vec<Orden> = f
            .children
            .iter()
            .flat_map(|c| lista(&Laid { root: c.clone(), canvas: l.canvas }))
            .filter(|o| o.estado == Estado::Reposo)
            .collect();
        pares.push((id, entra, sale, dentro));
    }
    let _ = writeln!(
        s,
        "        _ => 0,\n    }}\n}}\n\n\
         /// **El realce de `id` a los `ms` de empezar** a entrar (o a salir, con\n\
         /// `sale`), mezclado pieza a pieza con la curva de su `transition` (H8).\n\
         /// Pasado [`realce_dura`] pinta el final. Quien llama pide fotogramas\n\
         /// MIENTRAS dure y devuelve antes lo de debajo (`pintar_en`): las piezas\n\
         /// suaves mezclan con lo que hay.\n\
         pub fn realce_en(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str, ms: u32, sale: bool{}) {{",
        d.param()
    );
    s.push_str("    let _ = (p, ox, oy, ms, sale);\n");
    if d.hay() {
        s.push_str("    let _ = d;\n");
    }
    for (id, entra, sale, dentro) in pares {
        let _ = writeln!(s, "    if id == {id:?} {{\n        if !sale {{");
        for par in &entra {
            s.push_str(&d.colores(par_rust(par)));
        }
        s.push_str("        } else {\n");
        for par in &sale {
            s.push_str(&d.colores(par_rust(par)));
        }
        s.push_str("        }\n");
        for o in &dentro {
            let _ = writeln!(s, "        {}", llamada_con(&o.trazo, d, &limite_de("None", o.recorte)));
        }
        s.push_str("        return;\n    }\n");
    }
    s.push_str("}\n\n");
}

/// El nombre de una ventana en el codigo: su `id`, o su numero.
fn nombre_ventana(f: &bmo_maqueta_layout::Frame, k: usize) -> String {
    match &f.id {
        Some(id) => id.to_ascii_lowercase().replace(['.', '-'], "_"),
        None => k.to_string(),
    }
}

/// ** H7 (04-10): LAS VENTANAS QUE SE DESPLAZAN. Por cada una, su medida
/// (`DESPLAZA_<ID>`) y `desplazar_<id>(p, ox, oy, desde)`: limpia la ventana,
/// pinta lo de dentro corrido y recortado, y la barra. La MISMA cuenta que la
/// foto (`desplaza.rs`).
fn desplazamientos(s: &mut String, l: &Laid, d: &Datos) {
    use crate::desplaza;
    let cajas = desplaza::cajas(l);
    if cajas.is_empty() {
        return;
    }
    s.push_str(
        "// == LO QUE SE DESPLAZA (H7) =========================================\n\
         //\n\
         // Lo de dentro se maqueto y se juzgo ENTERO; aqui se mueve UN numero.\n\
         // `pintar` lo deja en su arranque (desde 0); para moverlo, el aparato\n\
         // llama a `desplazar_<id>` con lo que haya bajado (hasta `max()`).\n\
         \n\
         /// Una ventana: donde esta, cuanto se ve (`h`) y cuanto hay (`total`).\n\
         #[derive(Clone, Copy)]\n\
         pub struct Desplaza {\n\
         \x20   pub x: u32,\n\
         \x20   pub y: u32,\n\
         \x20   pub w: u32,\n\
         \x20   pub h: u32,\n\
         \x20   pub total: u32,\n\
         }\n\
         \n\
         impl Desplaza {\n\
         \x20   /// Lo mas que se puede bajar.\n\
         \x20   pub const fn max(&self) -> u32 {\n\
         \x20       self.total.saturating_sub(self.h)\n\
         \x20   }\n\
         }\n\
         \n",
    );
    for (k, f) in cajas.iter().enumerate() {
        let nombre = nombre_ventana(f, k);
        let mayus = nombre.to_ascii_uppercase();
        let v = desplaza::ventana(f);
        let total = desplaza::total(f);
        let fondo = f.style.background.unwrap_or(0);
        let _ = writeln!(
            s,
            "pub const DESPLAZA_{mayus}: Desplaza = Desplaza {{ x: {}, y: {}, w: {}, h: {}, total: {total} }};\n\n\
             /// **La ventana `{nombre}` bajada `desde` pixeles** (H7).\n\
             pub fn desplazar_{nombre}(p: &bmo::Pantalla, ox: u32, oy: u32, desde: u32{}) {{\n\
             \x20   let desde = desde.min(DESPLAZA_{mayus}.max());\n\
             \x20   p.pieza(&bmo::Pieza::Caja {{ x: {}, y: {}, w: {}, h: {}, r: {}, c: 0x{fondo:08X} }}, ox as i32, oy as i32, None);\n\
             \x20   let limite = Recorte::nuevo(ox as i32 + {}, oy as i32 + {}, {}, {});\n\
             \x20   let (ox0, oy0) = (ox as i32, oy as i32);\n\
             \x20   let (ox, oy) = (ox0, oy0 - desde as i32);\n\
             \x20   let _ = (ox, oy, limite);",
            v.x, v.y, v.w, v.h,
            d.param(),
            v.x, v.y, v.w, v.h, desplaza::radio(f),
            v.x, v.y, v.w, v.h,
        );
        if d.hay() {
            s.push_str("    let _ = d;\n");
        }
        for o in desplaza::contenido(f, l.canvas) {
            let _ = writeln!(s, "    {}", llamada_con(&o.trazo, d, &limite_de("Some(limite)", o.recorte)));
        }
        // La barra: la misma cuenta que `desplaza::barra`, con `desde` de
        // verdad.
        if let Some(b0) = desplaza::barra(f, 0) {
            let pista = v.h - 6;
            let _ = writeln!(
                s,
                "    let y = {} + desde * {} / {};\n\
                 \x20   p.pieza(&bmo::Pieza::Caja {{ x: {}, y: y as i32, w: {}, h: {}, r: {}, c: 0x{:08X} }}, ox0, oy0, None);",
                b0.y,
                pista - b0.h,
                total - v.h,
                b0.x,
                b0.w,
                b0.h,
                b0.w / 2,
                desplaza::color_barra(f)
            );
        }
        s.push_str("}\n\n");
    }
}

// ------------------------------------------------------------------------
//  Golpeo e islas
// ------------------------------------------------------------------------

fn golpe(s: &mut String, l: &Laid) {
    let hits = l.hits();
    s.push_str(
        "/// Que `id` hay bajo `(px, py)`, con la maquetacion puesta en `(ox, oy)`.\n\
         ///\n\
         /// ** Sale de la MISMA pasada que `pintar`, asi que no hay una segunda\n\
         /// aritmetica que pueda discrepar: el boton que se dibuja aqui responde\n\
         /// aqui, por construccion y no por cuidado.\n\
         pub fn golpe(ox: u32, oy: u32, px: u32, py: u32) -> Option<&'static str> {\n",
    );
    if hits.is_empty() {
        s.push_str("    let _ = (ox, oy, px, py);\n");
    }
    for (id, r) in &hits {
        let _ = writeln!(
            s,
            "    if px >= ox + {} && px < ox + {} && py >= oy + {} && py < oy + {} {{\n\
             \x20       return Some({id:?});\n\
             \x20   }}",
            r.x,
            r.right(),
            r.y,
            r.bottom()
        );
    }
    s.push_str("    None\n}\n\n");

    s.push_str(
        "/// Esta `(px, py)` dentro de la maquetacion?\n\
         pub fn dentro(ox: u32, oy: u32, px: u32, py: u32) -> bool {\n\
         \x20   px >= ox && px < ox + ANCHO && py >= oy && py < oy + ALTO\n\
         }\n\n",
    );
}

fn islas(s: &mut String, l: &Laid) {
    let islas = l.islands();
    let _ = writeln!(
        s,
        "/// Los huecos que rellena otro proceso: nombre, x, y, ancho, alto.\n\
         ///\n\
         /// Relativos al origen. Una isla es una superficie de `PLAN_DIRECTOR.md`\n\
         /// vista desde la maqueta: aqui solo se dice DONDE va.\n\
         pub const ISLAS: [(&str, u32, u32, u32, u32); {}] = [",
        islas.len()
    );
    for (nombre, r) in &islas {
        let _ = writeln!(s, "    ({nombre:?}, {}, {}, {}, {}),", r.x, r.y, r.w, r.h);
    }
    s.push_str("];\n\n");

    s.push_str(
        "/// El rect de una isla por su nombre: x, y, ancho, alto.\n\
         pub fn isla(nombre: &str) -> Option<(u32, u32, u32, u32)> {\n\
         \x20   let mut k = 0;\n\
         \x20   while k < ISLAS.len() {\n\
         \x20       let (n, x, y, w, h) = ISLAS[k];\n\
         \x20       if n == nombre {\n\
         \x20           return Some((x, y, w, h));\n\
         \x20       }\n\
         \x20       k += 1;\n\
         \x20   }\n\
         \x20   None\n\
         }\n\n",
    );

    s.push_str(
        "/// Repinta el fondo de una isla, para borrar lo que hubiera dentro.\n\
         ///\n\
         /// ** Existe para que quien rellena la isla NO tenga que saber su color.\n\
         /// Copiarlo en Rust seria una segunda verdad, y el dia que cambie el\n\
         /// `.maqueta` una de las dos se quedaria vieja sin avisar.\n\
         pub fn limpiar_isla(p: &bmo::Pantalla, ox: u32, oy: u32, nombre: &str) {\n",
    );
    let con_fondo: Vec<_> = l
        .all()
        .into_iter()
        .filter(|f| f.island.is_some() && f.style.background.is_some())
        .collect();
    if con_fondo.is_empty() {
        s.push_str("    let _ = (p, ox, oy, nombre);\n");
    }
    for f in con_fondo {
        let n = f.island.as_deref().expect("filtrado arriba");
        let fondo = f.style.background.expect("filtrado arriba");
        let [t, r, b, l] = f.style.border_width;
        let _ = writeln!(
            s,
            "    if nombre == {n:?} {{\n\
             \x20       p.rect(ox + {}, oy + {}, {}, {}, 0x{fondo:08X});\n\
             \x20       return;\n\
             \x20   }}",
            f.rect.x + l as i32,
            f.rect.y + t as i32,
            f.rect.w.saturating_sub(l + r),
            f.rect.h.saturating_sub(t + b),
        );
    }
    s.push_str("}\n");
}

// ------------------------------------------------------------------------
//  Los estados y sus transiciones (P3b, 04-10)
// ------------------------------------------------------------------------

/// **El modulo con sus ESTADOS**: lo de siempre (el reposo) y, si el
/// `.maqueta` declara `@estado`, las transiciones entre todos, pintadas en
/// el aparato par a par (`movimiento`). Sin estados, EXACTAMENTE `modulo`.
pub fn modulo_con_estados(origen: &str, reposo: &Laid, otros: &[(String, Laid)]) -> String {
    modulo_entero(origen, reposo, otros, &[])
}

/// **El modulo entero**: datos y listas (`modulo_con_datos`) y, si los hay,
/// estados. [!] Una pieza con DATOS no lleva estados todavia (es P3c): quien
/// llama lo rechaza antes.
pub fn modulo_entero(origen: &str, reposo: &Laid, otros: &[(String, Laid)], colores: &[(String, u32)]) -> String {
    let mut s = modulo_con_datos(origen, reposo, colores);
    if otros.is_empty() {
        return s;
    }
    let mut todos: Vec<(&str, &Laid)> = vec![("reposo", reposo)];
    todos.extend(otros.iter().map(|(n, l)| (n.as_str(), l)));
    let mut pares_de = Vec::new();
    for (i, (na, la)) in todos.iter().enumerate() {
        for (j, (nb, lb)) in todos.iter().enumerate() {
            if i != j {
                pares_de.push((*na, *nb, crate::movimiento::pares(la, lb)));
            }
        }
    }
    let nombres: Vec<String> = todos.iter().map(|(n, _)| format!("{n:?}")).collect();
    let _ = write!(
        s,
        "\n// == LOS ESTADOS (P3) ================================================\n\
         //\n\
         // Cada estado se maqueto ENTERO en el anfitrion y se juzgo. Aqui no se\n\
         // maqueta nada: una transicion mezcla, par a par, piezas ya calculadas\n\
         // (`bmo_pinta::entre_piezas`), con la curva de cada caja\n\
         // (`bmo_pinta::avance`). La foto de la transicion del anfitrion\n\
         // (`maqueta --foto --tira`) sale de los MISMOS pares: es su oraculo.\n\
         \n\
         pub const ESTADOS: [&str; {}] = [{}];\n\
         \n\
         /// Lo que tarda ir de `de` a `a`, en ms. 0 si no hay tal transicion.\n\
         pub fn duracion(de: &str, a: &str) -> u32 {{\n\
         \x20   match (de, a) {{\n",
        nombres.len(),
        nombres.join(", ")
    );
    for (a, b, pares) in &pares_de {
        let _ = writeln!(s, "        ({a:?}, {b:?}) => {},", crate::movimiento::duracion(pares));
    }
    s.push_str(
        "        _ => 0,\n    }\n}\n\n\
         /// **Pinta la transicion de `de` a `a` a los `ms` de empezar.** Pasado\n\
         /// [`duracion`], pinta `a` tal cual. Quien llama pide fotogramas\n\
         /// MIENTRAS dure y devuelve el fondo antes de cada uno: las piezas\n\
         /// suaves mezclan con lo que hay debajo.\n\
         pub fn pintar_transicion(p: &bmo::Pantalla, ox: u32, oy: u32, de: &str, a: &str, ms: u32) {\n\
         \x20   match (de, a) {\n",
    );
    for (k, (a, b, _)) in pares_de.iter().enumerate() {
        let _ = writeln!(s, "        ({a:?}, {b:?}) => transicion_{k}(p, ox, oy, ms),");
    }
    s.push_str(
        "        _ => {}\n    }\n}\n\n\
         /// **Pinta un estado entero**: el reposo es `pintar`; los demas, el final\n\
         /// de ir a ellos desde el reposo.\n\
         pub fn pintar_estado(p: &bmo::Pantalla, ox: u32, oy: u32, estado: &str) {\n\
         \x20   if estado == \"reposo\" {\n\
         \x20       pintar(p, ox, oy);\n\
         \x20   } else {\n\
         \x20       pintar_transicion(p, ox, oy, \"reposo\", estado, u32::MAX);\n\
         \x20   }\n\
         }\n",
    );
    for (k, (a, b, pares)) in pares_de.iter().enumerate() {
        let _ = write!(s, "\n/// De `{a}` a `{b}`.\nfn transicion_{k}(p: &bmo::Pantalla, ox: u32, oy: u32, ms: u32) {{\n    let _ = (p, ox, oy, ms);\n");
        for par in pares {
            s.push_str(&par_rust(par));
        }
        s.push_str("}\n");
    }
    s
}

/// Un par, en Rust: mezclado si se puede, y si no, el que toca a esa altura.
fn par_rust(par: &crate::movimiento::Par) -> String {
    use crate::movimiento::Par;
    let Par { a, b, retraso, dura, curva, de } = par;
    let k = format!(
        "bmo::avance(ms, {retraso}, {dura}, [{}, {}, {}, {}])",
        curva[0], curva[1], curva[2], curva[3]
    );
    let mut s = format!("    // {de}\n");
    let pieza = |t: &Trazo| t.con_pieza_o_caja(pieza_literal);
    match (a, b) {
        (Some(Trazo::Texto { r: ra, texto, color: ca }), Some(Trazo::Texto { r: rb, color: cb, .. })) => {
            let _ = writeln!(
                s,
                "    {{\n        let k = {k};\n        p.texto((ox as i32 + bmo::entre_i({}, {}, k)) as u32, (oy as i32 + bmo::entre_i({}, {}, k)) as u32, {texto:?}, bmo::entre_color(0x{ca:08X}, 0x{cb:08X}, k));\n    }}",
                ra.x, rb.x, ra.y, rb.y
            );
        }
        (Some(ta), Some(tb)) => match (pieza(ta), pieza(tb)) {
            (Some(la), Some(lb)) => {
                let _ = writeln!(s, "    p.pieza_entre(&{la}, &{lb}, {k}, ox as i32, oy as i32);");
            }
            _ => {
                let _ = writeln!(
                    s,
                    "    if {k}.clamp(0, 1000) >= 500 {{\n        {}\n    }} else {{\n        {}\n    }}",
                    llamada(tb),
                    llamada(ta)
                );
            }
        },
        (Some(ta), None) => {
            let _ = writeln!(s, "    if {k}.clamp(0, 1000) < 500 {{\n        {}\n    }}", llamada(ta));
        }
        (None, Some(tb)) => {
            let _ = writeln!(s, "    if {k}.clamp(0, 1000) >= 500 {{\n        {}\n    }}", llamada(tb));
        }
        (None, None) => {}
    }
    s
}
