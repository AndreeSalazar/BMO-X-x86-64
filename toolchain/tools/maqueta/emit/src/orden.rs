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
    /// Una `<imagen>` (H4): sus pixeles, recortados al radio de su caja.
    /// `dato` = llega al ejecutar (estos pixeles son su muestra).
    Imagen { r: Rect, radio: u32, px: std::sync::Arc<[u32]>, dato: Option<String> },
    /// **Una figura de SVG** (MAQUETA 3): caminos en 1/64 px; pluma redonda
    /// de `pluma` (1/64) o relleno (`pluma == 0`) con su regla; su tinta y
    /// su opacidad. Lo que MAQUETA 2 ya sabia pintar sigue saliendo como
    /// `Linea` y `Relleno` (ver [`trazos_del_dibujo`]).
    Figura { caminos: Vec<Vec<(i32, i32)>>, cerrados: Vec<bool>, pluma: i32, tinta: TintaFija, alfa: u8, par_impar: bool },
}

/// La tinta de una [`Trazo::Figura`], con sus paradas suyas.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TintaFija {
    Liso(u32),
    Lineal { de: (i32, i32), a: (i32, i32), paradas: Vec<bmo_pinta::Parada> },
    Radial { centro: (i32, i32), eje_x: (i32, i32), eje_y: (i32, i32), paradas: Vec<bmo_pinta::Parada> },
}

impl TintaFija {
    pub fn tinta(&self) -> bmo_pinta::Tinta<'_> {
        use bmo_pinta::Tinta;
        match self {
            TintaFija::Liso(c) => Tinta::Liso(*c),
            TintaFija::Lineal { de, a, paradas } => Tinta::Lineal { de: *de, a: *a, paradas },
            TintaFija::Radial { centro, eje_x, eje_y, paradas } => Tinta::Radial { centro: *centro, eje_x: *eje_x, eje_y: *eje_y, paradas },
        }
    }
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

    /// La pieza del pintor de este trazo, y un `Rect` como CAJA de radio 0
    /// (los mismos pixeles): lo que pide una transicion, que lo mezcla todo
    /// como piezas. `None` solo para la letra de pixel.
    pub fn con_pieza_o_caja<R>(&self, f: impl FnOnce(&bmo_pinta::Pieza) -> R) -> Option<R> {
        match self {
            Trazo::Rect { r, color } => Some(f(&bmo_pinta::Pieza::Caja { x: r.x, y: r.y, w: r.w as i32, h: r.h as i32, r: 0, c: *color })),
            otro => otro.con_pieza(f),
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
            Trazo::Imagen { r, radio, px, .. } => {
                let (x, y, w, h) = caja(r);
                Some(f(&Pieza::Imagen { x, y, w, h, r: *radio as i32, px }))
            }
            Trazo::Figura { caminos, cerrados, pluma, tinta, alfa, par_impar } => {
                let v: Vec<&[(i32, i32)]> = caminos.iter().map(|c| c.as_slice()).collect();
                Some(f(&Pieza::Figura { caminos: &v, cerrados, pluma: *pluma, tinta: tinta.tinta(), alfa: *alfa, par_impar: *par_impar }))
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
    /// (MAQUETA 3, MA2) Lo que un `overflow: hidden` deja ver de este trazo,
    /// si lo corta a medias: el pintor lo recorta ahi. `None` = entero.
    pub recorte: Option<Rect>,
}

/// La lista entera, en orden de pintado -- que es el orden del fichero.
pub fn lista(l: &Laid) -> Vec<Orden> {
    lista_con(l, false)
}

/// La lista SIN los dibujos que animan (S7): lo que hay debajo de ellos.
pub fn lista_sin_anima(l: &Laid) -> Vec<Orden> {
    lista_con(l, true)
}

fn lista_con(l: &Laid, sin_anima: bool) -> Vec<Orden> {
    let mut out = Vec::new();
    // ** Capa a capa, y con lo que el `overflow` de arriba deja ver (MA2,
    // `capas.rs`). Sin `z-index` ni recortes, el orden y los trazos de siempre.
    let cajas = crate::capas::cajas(l);
    for c in &cajas {
        let de = nombre_de(c.f);
        let fuera = sin_anima && crate::anima::de(c.f).is_some();
        trazos_de(c, Estado::Reposo, &de, fuera, &mut out);
        trazos_de(c, Estado::Encima, &de, fuera, &mut out);
    }
    // El contorno, encima de todo, como en CSS.
    for c in &cajas {
        for t in crate::capas::contorno(c.f) {
            if let Some((trazo, recorte)) = crate::capas::recortar(t, c.corte) {
                out.push(Orden { trazo, de: nombre_de(c.f), estado: Estado::Reposo, recorte });
            }
        }
    }
    out
}

/// Los trazos de una caja en un estado. Devuelve nada si en ese estado no
/// cambia -- una caja sin `:hover` no aporta ni una orden a `Encima`.
fn trazos_de(c: &crate::capas::Caja, estado: Estado, de: &str, sin_dibujo: bool, out: &mut Vec<Orden>) {
    let f = c.f;
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

    for (ranura, trazo) in trazos_de_estilo(f, &s, es_suave(&s)) {
        if sin_dibujo && matches!(ranura, Ranura::Relleno(_) | Ranura::Linea(_)) {
            continue;
        }
        // El texto y el dibujo son CONTENIDO: los recorta tambien la propia caja.
        let corte = match ranura {
            Ranura::Texto | Ranura::Renglon(_) | Ranura::Relleno(_) | Ranura::Linea(_) => c.corte_propio,
            _ => c.corte,
        };
        if let Some((trazo, recorte)) = crate::capas::recortar(trazo, corte) {
            out.push(Orden { trazo, de: de.to_string(), estado, recorte });
        }
    }
}

/// **Donde va cada trazo de una caja** (P3b): lo que permite emparejar los
/// trazos de la MISMA caja en dos estados. El orden de la enumeracion es el
/// de pintado.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Ranura {
    Brillo,
    /// El borde plano de los cuatro lados iguales (va DEBAJO del fondo).
    BordePlano,
    Fondo,
    /// Un lado de un borde plano distinto por lado.
    Lado(u8),
    /// El borde suave (va ENCIMA del fondo).
    Borde,
    /// Los pixeles de una `<imagen>` (H4), encima de su caja.
    Imagen,
    Texto,
    /// La linea `k` (desde la segunda) de un parrafo (H3).
    Renglon(u16),
    Relleno(u16),
    Linea(u16),
}

/// Una caja con radio, resplandor o degradado se pinta con las piezas SUAVES.
pub fn es_suave(s: &bmo_maqueta_layout::Style) -> bool {
    s.border_radius > 0 || s.shadow.is_some() || s.gradient.is_some()
}

/// **Los trazos de una caja con un estilo**, cada uno en su ranura. `suave`
/// se puede FORZAR (P3b): para mezclar una caja plana con una redonda, las
/// dos se describen con piezas suaves -- una caja de radio 0 pinta los mismos
/// pixeles que su rect.
pub fn trazos_de_estilo(f: &Frame, s: &bmo_maqueta_layout::Style, suave: bool) -> Vec<(Ranura, Trazo)> {
    let s = *s;
    let mut out: Vec<(Ranura, Trazo)> = Vec::new();
    // La ranura de lo que se empuja ahora: una `Cell` para que el cierre que
    // empuja y quien la cambia no se pisen.
    let ranura = std::cell::Cell::new(Ranura::Brillo);
    let mut push = |trazo: Trazo| out.push((ranura.get(), trazo));
    macro_rules! en {
        ($r:expr) => {
            ranura.set($r)
        };
    }

    // El borde primero y el fondo encima: dos rects concentricos, que es como
    // lo escribe `calc.rs` a mano. Cuatro tiras darian el mismo dibujo y cuatro
    // veces mas ordenes que recortar.
    // ** MAQUETA 2: una caja con radio, resplandor o degradado se pinta con
    // las piezas SUAVES. Sin nada de eso, exactamente como antes (la
    // calculadora no cambia ni un pixel).
    let radio = s.border_radius;

    if let Some((alcance, argb)) = s.shadow {
        en!(Ranura::Brillo);
        push(Trazo::Resplandor { r: f.rect, radio, alcance, argb });
    }
    if !suave {
        // [!] (07-10, HM2) El borde IGUAL se pinta como un rect macizo con el
        // fondo encima: sin fondo, eso dejaba la caja entera del color del
        // borde, y en CSS por dentro se ve lo de detras. Sin fondo, va por
        // lados (la rama de abajo), que pinta solo el anillo.
        match s.borde_uniforme().filter(|_| s.background.is_some()) {
            Some((d, borde)) => {
                if let (Some(color), true) = (borde, d > 0) {
                    en!(Ranura::BordePlano);
                    push(Trazo::Rect { r: f.rect, color });
                }
                if let Some(color) = s.background {
                    en!(Ranura::Fondo);
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
                    en!(Ranura::Fondo);
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
                        en!(Ranura::Lado(k as u8));
                        push(Trazo::Rect { r: rr, color });
                    }
                }
            }
        }
    } else {
        // El fondo va por debajo del borde entero, como en CSS
        // (`background-clip: border-box`): asi no queda una costura oscura
        // entre las dos curvas suaves.
        en!(Ranura::Fondo);
        if let Some((de, a, vertical)) = s.gradient {
            push(Trazo::Degradado { r: f.rect, radio, de, a, vertical });
        } else if let Some(color) = s.background {
            push(Trazo::Caja { r: f.rect, radio, color });
        }
        // Suave quiere el borde IGUAL en los cuatro lados: uno distinto con
        // radio lo rechaza el veredicto (K), asi que aqui solo llega el igual.
        if let Some((grosor, Some(color))) = s.borde_uniforme() {
            if grosor > 0 {
                en!(Ranura::Borde);
                push(Trazo::Borde { r: f.rect, radio, grosor, color });
            }
        }
    }
    if let (bmo_maqueta_node::Tag::Imagen, Some(px)) = (f.tag, &f.imagen) {
        en!(Ranura::Imagen);
        push(Trazo::Imagen { r: f.rect, radio, px: px.clone(), dato: f.hueco.clone() });
    }
    if let (Some(t), Some(r), Some(color)) = (&f.text, f.text_at, s.color) {
        en!(Ranura::Texto);
        match s.font_size.and_then(|p| u8::try_from(p).ok()) {
            Some(px) => {
                // Un parrafo (H3): una letra por linea, partido con la MISMA
                // regla que lo midio (`measure::lineas`).
                let ls = bmo_maqueta_layout::measure::lineas(&s, t);
                let alto = r.h / ls.len().max(1) as u32;
                for (k, linea) in ls.into_iter().enumerate() {
                    if k > 0 {
                        en!(Ranura::Renglon(k as u16));
                    }
                    push(Trazo::Letra {
                        r: Rect { x: r.x, y: r.y + (k as u32 * alto) as i32, w: r.w, h: alto },
                        texto: linea,
                        color,
                        px,
                        peso: if s.font_weight == 0 { 400 } else { s.font_weight },
                        espacio: s.letter_spacing,
                        mayusculas: s.uppercase,
                    });
                }
            }
            None => push(Trazo::Texto { r, texto: t.clone(), color }),
        }
    }
    // ** Un dibujo (MAQUETA 3): lo aplana el lector de SVG en la caja de
    // contenido, con lo que hereda de ESTA regla (la de reposo o la de
    // `:hover`). Animado, aqui va su primer paso; los demas los pinta
    // `pintar_anima` (S7).
    if let Some(d) = &f.dibujo {
        let anima = crate::anima::de(f).is_some();
        for (k, t) in figuras_como_trazos(&figuras_en(d, &s, f.content), anima).into_iter().enumerate() {
            en!(match t {
                Trazo::Linea { .. } => Ranura::Linea(k as u16),
                _ => Ranura::Relleno(k as u16),
            });
            push(t);
        }
    }
    drop(push);
    out
}

/// **Las figuras de un dibujo** en una caja, con lo que hereda de `s` (el
/// primer paso si anima).
pub fn figuras_en(d: &bmo_maqueta_dibujo::Svg, s: &bmo_maqueta_layout::Style, caja: Rect) -> Vec<bmo_maqueta_dibujo::Figura> {
    let h = bmo_maqueta_cascade::herencia(s);
    let c = (caja.x as f64, caja.y as f64, caja.w as f64, caja.h as f64);
    match bmo_maqueta_dibujo::pasos(d, &h, c) {
        Some(Ok(p)) => p.figuras.into_iter().next().unwrap_or_default(),
        _ => bmo_maqueta_dibujo::figuras(d, &h, c).0,
    }
}

/// Un pixel con decimales a 1/64, en multiplos de 4 (1/16 de pixel: la
/// precision con la que VIAJA una cara).
pub fn a64(v: f64) -> i32 {
    ((v * 16.0).round() as i32) * 4
}

/// **Las figuras del lector, como trazos.** Lo que MAQUETA 2 ya pintaba --
/// una pluma redonda lisa y opaca, un relleno par-impar liso y opaco --
/// sale con sus piezas de siempre; lo demas, como `Figura`.
pub fn trazos_del_dibujo(figs: &[bmo_maqueta_dibujo::Figura]) -> Vec<Trazo> {
    figuras_como_trazos(figs, false)
}

/// Con `siempre_figura`, todas como `Figura`: los pasos de una animacion se
/// mezclan punto a punto, y eso solo lo hace la figura.
pub fn figuras_como_trazos(figs: &[bmo_maqueta_dibujo::Figura], siempre_figura: bool) -> Vec<Trazo> {
    use bmo_maqueta_dibujo::Tinta as T;
    figs.iter()
        .map(|f| {
            let caminos: Vec<Vec<(i32, i32)>> = f.caminos.iter().map(|c| c.iter().map(|p| (a64(p.0), a64(p.1))).collect()).collect();
            if let Some(color) = f.de_siempre().filter(|_| !siempre_figura) {
                return if f.pluma > 0.0 {
                    Trazo::Linea { caminos, cerrados: f.cerrados.clone(), grosor64: a64(f.pluma).max(4), color }
                } else {
                    Trazo::Relleno { caminos, color }
                };
            }
            let paradas = |ps: &[bmo_maqueta_dibujo::Parada]| -> Vec<bmo_pinta::Parada> {
                ps.iter().map(|p| bmo_pinta::Parada { en: (p.en * 1000.0).round().clamp(0.0, 1000.0) as u16, c: p.c, alfa: (p.alfa * 255.0).round().clamp(0.0, 255.0) as u8 }).collect()
            };
            let pt = |p: (f64, f64)| (a64(p.0), a64(p.1));
            let tinta = match &f.tinta {
                T::Liso(c) => TintaFija::Liso(*c),
                T::Lineal { de, a, paradas: ps } => TintaFija::Lineal { de: pt(*de), a: pt(*a), paradas: paradas(ps) },
                T::Radial { centro, eje_x, eje_y, paradas: ps } => TintaFija::Radial { centro: pt(*centro), eje_x: pt(*eje_x), eje_y: pt(*eje_y), paradas: paradas(ps) },
            };
            Trazo::Figura {
                caminos,
                cerrados: f.cerrados.clone(),
                pluma: if f.pluma > 0.0 { a64(f.pluma).max(4) } else { 0 },
                tinta,
                alfa: (f.alfa * 255.0).round().clamp(0.0, 255.0) as u8,
                par_impar: f.par_impar,
            }
        })
        .collect()
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
    // `pointer-events: none` (MA2), suyo o heredado: no se pulsa.
    for c in crate::capas::cajas(l).into_iter().filter(|c| !c.sin_puntero) {
        let f = c.f;
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
