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
    let ordenes = lista(l);
    let mut s = String::new();
    cabecera(&mut s, origen, l);
    pintar(&mut s, &ordenes);
    pintar_en(&mut s, &ordenes);
    realce(&mut s, &ordenes);
    golpe(&mut s, l);
    islas(&mut s, l);
    s
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

/// **Una pieza suave escrita en Rust**, para `bmo::Pieza` (la `Pieza` de
/// `bmo-pinta`, que `bmo-userland` reexporta).
fn pieza_literal(p: &bmo_pinta::Pieza) -> String {
    use bmo_pinta::Pieza;
    let caminos = |c: &[&[(i32, i32)]]| -> String {
        let v: Vec<String> = c
            .iter()
            .map(|s| format!("&[{}]", s.iter().map(|(x, y)| format!("({x}, {y})")).collect::<Vec<_>>().join(", ")))
            .collect();
        format!("&[{}]", v.join(", "))
    };
    match *p {
        Pieza::Caja { x, y, w, h, r, c } => format!("bmo::Pieza::Caja {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, c: 0x{c:08X} }}"),
        Pieza::Borde { x, y, w, h, r, grosor, c } => {
            format!("bmo::Pieza::Borde {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, grosor: {grosor}, c: 0x{c:08X} }}")
        }
        Pieza::Resplandor { x, y, w, h, r, alcance, argb } => {
            format!("bmo::Pieza::Resplandor {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, alcance: {alcance}, argb: 0x{argb:08X} }}")
        }
        Pieza::Degradado { x, y, w, h, r, de, a, vertical } => format!(
            "bmo::Pieza::Degradado {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, de: 0x{de:08X}, a: 0x{a:08X}, vertical: {vertical} }}"
        ),
        Pieza::Letra { x, y, alto, texto, c, px, peso, espacio, mayusculas } => format!(
            "bmo::Pieza::Letra {{ x: {x}, y: {y}, alto: {alto}, texto: b{:?}, c: 0x{c:08X}, px: {px}, peso: {peso}, espacio: {espacio}, mayusculas: {mayusculas} }}",
            String::from_utf8_lossy(texto)
        ),
        Pieza::Trazo { caminos: cs, cerrados, grosor64, c } => format!(
            "bmo::Pieza::Trazo {{ caminos: {}, cerrados: &{:?}, grosor64: {grosor64}, c: 0x{c:08X} }}",
            caminos(cs),
            cerrados
        ),
        Pieza::Relleno { caminos: cs, c } => format!("bmo::Pieza::Relleno {{ caminos: {}, c: 0x{c:08X} }}", caminos(cs)),
    }
}

/// La llamada que pinta este trazo, sin recortar.
fn llamada(t: &Trazo) -> String {
    if let Some(l) = t.con_pieza(pieza_literal) {
        return format!("p.pieza(&{l}, ox as i32, oy as i32, None);");
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

fn area(t: &Trazo) -> Rect {
    t.area()
}

// ------------------------------------------------------------------------
//  Los cuatro pintados
// ------------------------------------------------------------------------

fn pintar(s: &mut String, ordenes: &[Orden]) {
    s.push_str(
        "/// Pinta la maquetacion entera con su esquina superior izquierda en\n\
         /// `(ox, oy)`. El orden es el del fichero, que ES el orden de pintado.\n\
         pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32) {\n",
    );
    let mut ultimo = String::new();
    for o in ordenes.iter().filter(|o| o.estado == Estado::Reposo) {
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        let _ = writeln!(s, "    {}", llamada(&o.trazo));
    }
    s.push_str("}\n\n");
}

/// ** El pintado RECORTADO, que es lo que hace barato reparar un danio.
fn pintar_en(s: &mut String, ordenes: &[Orden]) {
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
         pub fn pintar_en(p: &bmo::Pantalla, ox: u32, oy: u32, cx: u32, cy: u32, cw: u32, ch: u32) {\n\
         \x20   let limite = Recorte::nuevo(cx as i32, cy as i32, cw as i32, ch as i32);\n",
    );
    let reposo: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Reposo).collect();
    if reposo.is_empty() {
        s.push_str("    let _ = (p, ox, oy, limite);\n");
    }
    let mut ultimo = String::new();
    for o in reposo {
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        let r = area(&o.trazo);
        let caja = format!(
            "Recorte::nuevo(ox as i32 + {}, oy as i32 + {}, {}, {})",
            r.x, r.y, r.w, r.h
        );
        match &o.trazo {
            Trazo::Rect { color, .. } => {
                let _ = writeln!(
                    s,
                    "    let c = {caja}.interseccion(&limite);\n\
                     \x20   if !c.vacio() {{\n\
                     \x20       p.rect(c.x0 as u32, c.y0 as u32, c.ancho() as u32, c.alto() as u32, 0x{color:08X});\n\
                     \x20   }}"
                );
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
                if let Some(l) = otro.con_pieza(pieza_literal) {
                    let _ = writeln!(
                        s,
                        "    if !{caja}.interseccion(&limite).vacio() {{\n\
                         \x20       p.pieza(&{l}, ox as i32, oy as i32, Some(limite));\n\
                         \x20   }}"
                    );
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
fn realce(s: &mut String, ordenes: &[Orden]) {
    s.push_str(
        "/// Repinta la caja `id` con sus colores de `:hover`. Llamalo cuando el\n\
         /// puntero entre, y `pintar` cuando salga.\n\
         pub fn realce(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str) {\n",
    );
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
        let _ = writeln!(s, "        {}", llamada(&o.trazo));
    }
    if !abierto.is_empty() {
        s.push_str("        return;\n    }\n");
    }
    s.push_str("}\n\n");
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
    let mut s = modulo(origen, reposo);
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
