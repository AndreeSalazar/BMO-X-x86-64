//! **QUE se dibuja.** Una lista de trazos, en orden de pintado, sin saber a
//! donde van.
//!
//! ## Por que esto se saco de `rust.rs`
//!
//! El emisor mezclaba dos preguntas en cada funcion: *que hay que dibujar* y
//! *como se escribe eso en Rust*. Mientras hubo un solo destino no se notaba.
//! Con tres --Rust, el recurso BEF, y el reflejo en PPM-- cada uno habria vuelto
//! a deducir lo primero, y **tres deducciones de la misma cosa son tres sitios
//! donde puede salir distinta**.
//!
//! Ahora la lista se calcula UNA vez y los destinos la escriben. `rust.rs` no
//! decide nada: traduce.
//!
//! ## * Y es lo que hace diagnosticable un fotograma
//!
//! Un recorte que falla no da un error: da basura en pantalla, o un trozo que no
//! se repinta. Con la lista fuera, la pregunta *"por que este fotograma salio
//! mal"* deja de ser una lectura del generado y pasa a ser **filtrar una lista y
//! mirarla** -- que es lo que hace `recorte::dentro`, y lo que prueban sus
//! pruebas sin arrancar nada.

use bmo_maqueta_layout::{Frame, Laid, Rect};

/// Un trazo: lo que una cara pinta. Los dos de siempre (rect y letra de
/// pixel) y, desde MAQUETA 2 (04-10), las piezas SUAVES de `bmo-pinta`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Trazo {
    /// Un rectangulo macizo.
    Rect { r: Rect, color: u32 },
    /// Letras de PIXEL (8 x 16): la caja no dijo `font-size`. `r` es donde
    /// empiezan y cuanto ocupan.
    Texto { r: Rect, texto: String, color: u32 },
    Caja { r: Rect, radio: u32, color: u32 },
    Borde { r: Rect, radio: u32, grosor: u32, color: u32 },
    Resplandor { r: Rect, radio: u32, alcance: u32, argb: u32 },
    Degradado { r: Rect, radio: u32, de: u32, a: u32, vertical: bool },
    /// La letra de la casa. `r` es su caja de texto (alto = `line-height`).
    Letra { r: Rect, texto: String, color: u32, px: u8, peso: u16, espacio: i32, mayusculas: bool },
    /// Un `<path>` con `stroke`: sus polilineas YA aplanadas, en 1/64 px.
    Linea { caminos: Vec<Vec<(i32, i32)>>, cerrados: Vec<bool>, grosor64: i32, color: u32 },
    /// Un `<path>` con `fill`.
    Relleno { caminos: Vec<Vec<(i32, i32)>>, color: u32 },
}

impl Trazo {
    /// El area que toca. Es lo unico que hace falta para recortar.
    pub fn area(&self) -> Rect {
        match self {
            Trazo::Rect { r, .. } | Trazo::Texto { r, .. } => *r,
            _ => {
                let (x, y, w, h) = self.con_pieza(|p| bmo_pinta::caja_de(p)).unwrap_or((0, 0, 0, 0));
                Rect { x, y, w: w.max(0) as u32, h: h.max(0) as u32 }
            }
        }
    }

    /// **La pieza del pintor** que describe este trazo (las suaves), prestada
    /// el tiempo de `f`. `None` para los de siempre (rect y letra de pixel).
    pub fn con_pieza<R>(&self, f: impl FnOnce(&bmo_pinta::Pieza) -> R) -> Option<R> {
        use bmo_pinta::Pieza;
        let caja = |r: &Rect| (r.x, r.y, r.w as i32, r.h as i32);
        match self {
            Trazo::Rect { .. } | Trazo::Texto { .. } => None,
            Trazo::Caja { r, radio, color } => {
                let (x, y, w, h) = caja(r);
                Some(f(&Pieza::Caja { x, y, w, h, r: *radio as i32, c: *color }))
            }
            Trazo::Borde { r, radio, grosor, color } => {
                let (x, y, w, h) = caja(r);
                Some(f(&Pieza::Borde { x, y, w, h, r: *radio as i32, grosor: *grosor as i32, c: *color }))
            }
            Trazo::Resplandor { r, radio, alcance, argb } => {
                let (x, y, w, h) = caja(r);
                Some(f(&Pieza::Resplandor { x, y, w, h, r: *radio as i32, alcance: *alcance as i32, argb: *argb }))
            }
            Trazo::Degradado { r, radio, de, a, vertical } => {
                let (x, y, w, h) = caja(r);
                Some(f(&Pieza::Degradado { x, y, w, h, r: *radio as i32, de: *de, a: *a, vertical: *vertical }))
            }
            Trazo::Letra { r, texto, color, px, peso, espacio, mayusculas } => Some(f(&Pieza::Letra {
                x: r.x,
                y: r.y,
                alto: r.h as i32,
                texto: texto.as_bytes(),
                c: *color,
                px: *px,
                peso: *peso,
                espacio: *espacio,
                mayusculas: *mayusculas,
            })),
            Trazo::Linea { caminos, cerrados, grosor64, color } => {
                let v: Vec<&[(i32, i32)]> = caminos.iter().map(|c| c.as_slice()).collect();
                Some(f(&Pieza::Trazo { caminos: &v, cerrados, grosor64: *grosor64, c: *color }))
            }
            Trazo::Relleno { caminos, color } => {
                let v: Vec<&[(i32, i32)]> = caminos.iter().map(|c| c.as_slice()).collect();
                Some(f(&Pieza::Relleno { caminos: &v, c: *color }))
            }
        }
    }
}

/// Cuando se dibuja este trazo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Estado {
    /// Siempre.
    Reposo,
    /// Solo mientras el puntero esta encima de la caja.
    Encima,
}

/// Un trazo, de quien es, y cuando toca.
///
/// `de` no es decoracion: va al comentario del codigo generado y es por donde se
/// sigue un fotograma raro hasta la caja que lo causo.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Orden {
    pub trazo: Trazo,
    pub de: String,
    pub estado: Estado,
}

/// La lista entera, en orden de pintado -- que es el orden del fichero.
pub fn lista(l: &Laid) -> Vec<Orden> {
    let mut out = Vec::new();
    for f in l.all() {
        let de = nombre_de(f);
        trazos_de(f, Estado::Reposo, &de, &mut out);
        trazos_de(f, Estado::Encima, &de, &mut out);
    }
    out
}

/// Los trazos de una caja en un estado. Devuelve nada si en ese estado no
/// cambia -- una caja sin `:hover` no aporta ni una orden a `Encima`.
fn trazos_de(f: &Frame, estado: Estado, de: &str, out: &mut Vec<Orden>) {
    let s = match estado {
        Estado::Reposo => f.style,
        Estado::Encima => match f.hover {
            Some(h) => h,
            None => return,
        },
    };
    // Sin `id` no hay forma de pedir el realce de esta caja, asi que sus
    // ordenes de `Encima` no las podria disparar nadie.
    if estado == Estado::Encima && f.id.is_none() {
        return;
    }

    let mut push = |trazo| {
        out.push(Orden {
            trazo,
            de: de.to_string(),
            estado,
        })
    };

    // El borde primero y el fondo encima: dos rects concentricos, que es como
    // lo escribe `calc.rs` a mano. Cuatro tiras darian el mismo dibujo y cuatro
    // veces mas ordenes que recortar.
    // ** MAQUETA 2: una caja con radio, resplandor o degradado se pinta con
    // las piezas SUAVES. Sin nada de eso, exactamente como antes (la
    // calculadora no cambia ni un pixel).
    let suave = s.border_radius > 0 || s.shadow.is_some() || s.gradient.is_some();
    let radio = s.border_radius;

    if let Some((alcance, argb)) = s.shadow {
        push(Trazo::Resplandor { r: f.rect, radio, alcance, argb });
    }
    if !suave {
        match s.borde_uniforme() {
            Some((d, borde)) => {
                if let (Some(color), true) = (borde, d > 0) {
                    push(Trazo::Rect { r: f.rect, color });
                }
                if let Some(color) = s.background {
                    // Con el borde transparente el fondo llega hasta fuera,
                    // como en CSS (`background-clip: border-box`).
                    let d = if borde.is_some() { d } else { 0 };
                    push(Trazo::Rect {
                        r: Rect {
                            x: f.rect.x + d as i32,
                            y: f.rect.y + d as i32,
                            w: f.rect.w.saturating_sub(d * 2),
                            h: f.rect.h.saturating_sub(d * 2),
                        },
                        color,
                    });
                }
            }
            // ** UN BORDE DISTINTO POR LADO (escalon 1): el fondo entero y
            // encima cada lado, un `rect` por lado. Arriba y abajo de punta a
            // punta; los lados, entre los dos. Con 1 px -- lo que usan las
            // maquetas, la raya bajo una fila -- es exactamente el dibujo de
            // CSS; con grosores grandes y colores distintos, CSS corta la
            // esquina en diagonal y aqui no (ver `LA_MAQUETA_EXIGE.md`).
            None => {
                if let Some(color) = s.background {
                    push(Trazo::Rect { r: f.rect, color });
                }
                let [t, r, b, l] = s.border_width;
                let (x, y, w, h) = (f.rect.x, f.rect.y, f.rect.w, f.rect.h);
                let lados = [
                    (0, Rect { x, y, w, h: t }),
                    (2, Rect { x, y: y + h as i32 - b as i32, w, h: b }),
                    (3, Rect { x, y: y + t as i32, w: l, h: h.saturating_sub(t + b) }),
                    (1, Rect { x: x + w as i32 - r as i32, y: y + t as i32, w: r, h: h.saturating_sub(t + b) }),
                ];
                for (k, rr) in lados {
                    if let (Some(color), true) = (s.border_color[k], rr.w > 0 && rr.h > 0) {
                        push(Trazo::Rect { r: rr, color });
                    }
                }
            }
        }
    } else {
        // El fondo va por debajo del borde entero, como en CSS
        // (`background-clip: border-box`): asi no queda una costura oscura
        // entre las dos curvas suaves.
        if let Some((de, a, vertical)) = s.gradient {
            push(Trazo::Degradado { r: f.rect, radio, de, a, vertical });
        } else if let Some(color) = s.background {
            push(Trazo::Caja { r: f.rect, radio, color });
        }
        // Suave quiere el borde IGUAL en los cuatro lados: uno distinto con
        // radio lo rechaza el veredicto (K), asi que aqui solo llega el igual.
        if let Some((grosor, Some(color))) = s.borde_uniforme() {
            if grosor > 0 {
                push(Trazo::Borde { r: f.rect, radio, grosor, color });
            }
        }
    }
    if let (Some(t), Some(r), Some(color)) = (&f.text, f.text_at, s.color) {
        match s.font_size.and_then(|p| u8::try_from(p).ok()) {
            Some(px) => push(Trazo::Letra {
                r,
                texto: t.clone(),
                color,
                px,
                peso: if s.font_weight == 0 { 400 } else { s.font_weight },
                espacio: s.letter_spacing,
                mayusculas: s.uppercase,
            }),
            None => push(Trazo::Texto { r, texto: t.clone(), color }),
        }
    }
    // Un dibujo: sus caminos, con la pluma y el relleno del `<svg>` (como en
    // SVG: primero el relleno, luego el trazo).
    if let Some(vb) = f.view_box {
        for d in f.children.iter().filter_map(|c| c.d.as_deref()) {
            let (caminos, cerrados) = aplanar(d, vb, f.content);
            if let Some(color) = s.fill {
                push(Trazo::Relleno { caminos: caminos.clone(), color });
            }
            if let Some(color) = s.stroke {
                let sw = if s.stroke_width == 0 { 64 } else { s.stroke_width } as i64;
                let grosor64 = (sw * f.content.w as i64 / vb[2].max(1) as i64) as i32;
                push(Trazo::Linea { caminos, cerrados, grosor64, color });
            }
        }
    }
}

/// **Un `<path>` aplanado** a la caja de su `<svg>`: sus curvas en tramos
/// rectos (con el lector de la casa, el MISMO del aparato), y cada punto del
/// `viewBox` a 1/64 de pixel. Toda la matematica, aqui, en el anfitrion.
pub fn aplanar(d: &str, vb: [u32; 4], caja: Rect) -> (Vec<Vec<(i32, i32)>>, Vec<bool>) {
    let (vx, vy, vw, vh) = (vb[0] as i64 * 64, vb[1] as i64 * 64, vb[2].max(1) as i64, vb[3].max(1) as i64);
    let mut caminos = Vec::new();
    let mut cerrados = Vec::new();
    for sub in bmo_letra::svg::camino(d) {
        let p = sub
            .puntos
            .iter()
            .map(|&(x, y)| {
                // En 1/16 de pixel (multiplos de 4): la precision con la que
                // VIAJA una cara. Asi la cara pintada aqui y la que viaja son
                // los mismos pixeles, no casi los mismos.
                let a16 = |v: i64| ((v + 2).div_euclid(4) * 4) as i32;
                (
                    a16(caja.x as i64 * 64 + (x as i64 - vx) * caja.w as i64 / vw),
                    a16(caja.y as i64 * 64 + (y as i64 - vy) * caja.h as i64 / vh),
                )
            })
            .collect();
        caminos.push(p);
        cerrados.push(sub.cerrado);
    }
    (caminos, cerrados)
}

/// **Una region que se puede pulsar**, y como se llama.
///
/// # Por que vive aqui y no en el emisor que la escribe
///
/// Por lo mismo que `lista`: es una respuesta a *"que hay"*, no a *"como se
/// escribe"*. Si cada destino dedujera por su cuenta que cajas son pulsables,
/// serian **tres deducciones de la misma cosa** -- y el dia que una se
/// desviara, el boton estaria en un sitio en el codigo generado y en otro en el
/// recurso, con el mismo `.maqueta` de origen.
///
/// [!] Y OJO CON EL NOMBRE: esto **no** es `bmo-golpe`, que es la RESTA --
/// convertir un clic de pantalla en un clic de app-- y vive en Ring 3. Esto es
/// *donde* se puede pulsar y *como se llama*, decidido en el anfitrion. Dos
/// preguntas, dos sitios, y la palabra se repite porque el plan la usa asi.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Golpe {
    pub r: Rect,
    /// El `#id` de la caja, con la almohadilla incluida.
    pub nombre: String,
}

/// Las regiones pulsables: **las cajas que tienen `#id`, y solo esas**.
///
/// * El criterio no es un invento de aqui: es el mismo que usa `trazos_de` para
/// decidir si una caja aporta ordenes de `Encima`. Una caja sin `id` no puede
/// recibir su realce **porque nadie puede nombrarla para pedirlo**, y por la
/// misma razon nadie puede recibir su pulsacion.
///
/// > Si las dos reglas se separaran, habria cajas que se iluminan al pasar por
/// > encima y no hacen nada al pulsarlas.
pub fn golpes(l: &Laid) -> Vec<Golpe> {
    let mut out = Vec::new();
    for f in l.all() {
        if let Some(id) = &f.id {
            out.push(Golpe {
                r: f.rect,
                nombre: format!("#{id}"),
            });
        }
    }
    out
}

/// Como se llama una caja: su `id`, su isla, o su etiqueta.
pub fn nombre_de(f: &Frame) -> String {
    if let Some(id) = &f.id {
        return format!("#{id}");
    }
    if let Some(n) = &f.island {
        return format!("isla {n}");
    }
    f.tag.name().to_string()
}
