//! # LA LETRA -- la de las maquetas, escrita en casa
//!
//! El propietario, con la maqueta de HERMES y la de BANK CAT delante: *"HTML y
//! CSS, esos dos son motivos: me gustaria que mi BMO-X refleje las maquetas
//! que hiciste... que sea IGUAL"*. Medido el 2026-10-03, lo que mas separaba
//! la app de su maqueta no era el color ni la caja: era **la letra**. La
//! maqueta escribe con una sans proporcional y suavizada, en cinco tallas;
//! BMO-X escribia con la de 8 x 16, toda del mismo ancho y en escalera.
//!
//! ## Lo que es
//!
//! - **Proporcional**: la `i` no ocupa lo que la `m`.
//! - **De bordes suaves**: cada pixel del borde lleva la parte de tinta que
//!   le toca (0 a 255), y quien pinta la mezcla con su fondo.
//! - **De cualquier talla**: las letras son TRAZOS (lineas y arcos con una
//!   pluma redonda, `glifos.rs`), no pixeles; una talla nueva no es arte
//!   nuevo.
//! - **Encajada en la rejilla**: la base, la altura de la x y la de la
//!   mayuscula caen en pixel entero en cada talla (`Alturas`), que es lo que
//!   hace que una letra de 13 px se lea limpia y no borrosa.
//!
//! ## Lo que NO es
//!
//! No es IBM Plex ni ninguna otra: no hay un contorno de nadie aqui dentro.
//! La regla del propietario: *"no integrar objetos terceros, sino tomar la
//! inspiracion"*. La maqueta es la medida (el ESPEJO de cara dice cuanto se
//! parece), no la fuente.
//!
//! ## Como se usa
//!
//! ```text
//!    let mut letra = Letra::nueva();               // una por app: es la cache
//!    let e = Estilo::normal(14);
//!    let ancho = letra.escribir(b"Cartera", e, x, base, |x, y, a| {
//!        lienzo.mezclar(x, y, color, a)            // a: 0..=255 de tinta
//!    });
//! ```
//!
//! Cada glifo se rasteriza la PRIMERA vez que se pide (en su talla, su peso
//! y su cuarto de pixel) y se guarda: pintar un texto ya visto es copiar
//! alfas, no calcular trazos.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod glifos;
pub mod svg;
mod trazo;

pub use glifos::Acento;
/// El seno y el coseno de unos GRADOS enteros, en Q15 (32768 es 1): los
/// giros de las maquetas (`rotate(16deg)`).
pub use trazo::{coseno, seno};

use alloc::vec;
use alloc::vec::Vec;
use trazo::Trozo;

/// El peso de la pluma.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Peso {
    Normal,
    /// La `font-weight: 500` de los nombres de la maqueta.
    Media,
    Negrita,
}

impl Peso {
    /// Lo gruesa que es la pluma, en 1/16 de centesima de eme.
    fn pluma16(self) -> i32 {
        match self {
            Peso::Normal => 138,
            Peso::Media => 168,
            Peso::Negrita => 210,
        }
    }

    /// Lo minimo que mide la pluma, en 1/64 de pixel: por debajo de un pixel
    /// la letra se apaga en vez de afinarse.
    fn minimo64(self) -> i32 {
        match self {
            Peso::Normal => 66,
            Peso::Media => 82,
            Peso::Negrita => 104,
        }
    }

    /// Lo que la negrita se corre y se ensancha para no comerse sus huecos.
    fn holgura16(self) -> i32 {
        self.pluma16() - Peso::Normal.pluma16()
    }
}

/// **Como se escribe un texto**: talla en pixeles (la `font-size` del CSS),
/// peso, espacio entre letras (la `letter-spacing`, en 1/64 de pixel) y si va
/// todo en mayusculas (el `text-transform: uppercase` de los rotulos).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Estilo {
    pub px: u8,
    pub peso: Peso,
    pub espacio64: i32,
    pub mayusculas: bool,
}

impl Estilo {
    pub const fn normal(px: u8) -> Estilo {
        Estilo { px, peso: Peso::Normal, espacio64: 0, mayusculas: false }
    }

    pub const fn media(px: u8) -> Estilo {
        Estilo { px, peso: Peso::Media, espacio64: 0, mayusculas: false }
    }

    pub const fn negrita(px: u8) -> Estilo {
        Estilo { px, peso: Peso::Negrita, espacio64: 0, mayusculas: false }
    }

    /// El `letter-spacing` en milesimas de eme (`.14em` son 140).
    pub const fn espaciado(mut self, milesimas: i32) -> Estilo {
        self.espacio64 = milesimas * self.px as i32 * 64 / 1000;
        self
    }

    pub const fn mayusculas(mut self) -> Estilo {
        self.mayusculas = true;
        self
    }
}

/// **Donde cae la linea base** dentro de una caja de texto de `alto` pixeles,
/// como la pone un navegador: la letra ocupa 1,3 emes (1,025 por encima de
/// la base y 0,275 por debajo) y lo que sobra del `line-height` se reparte
/// arriba y abajo. Con esto un texto cae en la MISMA fila que en la maqueta.
pub fn base_en_caja(px: u8, alto: i32) -> i32 {
    let px = px as i32;
    ((alto * 1000 - 1300 * px) / 2 + 1025 * px + 500).div_euclid(1000)
}

/// La caja que un texto de talla `px` ocupa con `line-height: normal`.
pub fn alto_normal(px: u8) -> i32 {
    (1300 * px as i32 + 500) / 1000
}

// ---------------------------------------------------------------------------
// LAS ALTURAS: el encaje en la rejilla
// ---------------------------------------------------------------------------

/// Las cinco cotas del dibujo (en 1/16 de centesima de eme) y a que fila de
/// pixel van (en 1/64 de pixel, respecto a la base). Entre dos cotas, en
/// linea recta.
#[derive(Clone, Copy, Debug)]
struct Alturas {
    px: i32,
    pluma64: i32,
    cotas: [(i32, i32); 5],
}

impl Alturas {
    fn de(px: u8, peso: Peso) -> Alturas {
        let px = px as i32;
        let pluma64 = (peso.pluma16() * px / 25).max(peso.minimo64());
        let media = pluma64 / 2;
        let redondo = |c: i32| (c * px + 50) / 100;
        let cap = redondo(70);
        let xh = redondo(52).min(cap - 1).max(1);
        let asc = redondo(74).max(cap);
        let desc = redondo(20).max(1);
        Alturas {
            px,
            pluma64,
            cotas: [
                (-74 * 16, -asc * 64 + media),
                (-70 * 16, -cap * 64 + media),
                (-52 * 16, -xh * 64 + media),
                (0, -media),
                (20 * 16, desc * 64 - media),
            ],
        }
    }

    fn x(&self, x16: i32) -> i32 {
        x16 * self.px / 25
    }

    fn y(&self, y16: i32) -> i32 {
        let c = &self.cotas;
        let escala = |d: i32| d * self.px / 25;
        if y16 <= c[0].0 {
            return c[0].1 + escala(y16 - c[0].0);
        }
        if y16 >= c[4].0 {
            return c[4].1 + escala(y16 - c[4].0);
        }
        for k in 0..4 {
            let (a, b) = (c[k], c[k + 1]);
            if y16 <= b.0 {
                return a.1 + (b.1 - a.1) * (y16 - a.0) / (b.0 - a.0);
            }
        }
        c[4].1
    }
}

// ---------------------------------------------------------------------------
// LA PLUMA: cuanto cubre un trazo a un pixel
// ---------------------------------------------------------------------------

/// Raiz cuadrada entera, por abajo.
fn raiz(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// La distancia (en 1/64 de pixel) del punto `p` al segmento `a`-`b`.
fn distancia(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> i32 {
    let (px, py) = ((p.0 - a.0) as i64, (p.1 - a.1) as i64);
    let (bx, by) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
    let l2 = bx * bx + by * by;
    let t = px * bx + py * by;
    let d2 = if l2 == 0 || t <= 0 {
        px * px + py * py
    } else if t >= l2 {
        let (qx, qy) = (px - bx, py - by);
        qx * qx + qy * qy
    } else {
        let cruz = px * by - py * bx;
        ((cruz as i128 * cruz as i128) / l2 as i128) as i64
    };
    raiz(d2 as u64) as i32
}

/// La tinta de un pixel, de 0 a 64, a `d` (1/64 px) del borde: el borde es
/// una rampa de UN pixel centrada en el contorno.
fn tinta(r: i32, d: i32) -> i32 {
    (32 + r - d).clamp(0, 64)
}

/// El realce: un borde de letra clara sobre fondo oscuro, mezclado en lineal,
/// se ve mas fino de lo que es. Se sube la tinta de los pixeles a medias (no
/// los llenos ni los vacios), que es lo que hace un navegador al suavizar.
fn realce(t: i32) -> u8 {
    let t = t + t * (64 - t) / 160;
    (t * 255 / 64).clamp(0, 255) as u8
}

/// Un trazo ya en pixeles: el segmento y su radio (1/64 px).
#[derive(Clone, Copy)]
struct Linea {
    a: (i32, i32),
    b: (i32, i32),
    r: i32,
}

/// **La tinta de unos trazos**, en la caja que los cubre: `(x0, y0, w, h,
/// alfas)`. Cada pixel lleva la del trazo que mas lo cubre (dos trazos que
/// se cruzan no se suman: la tinta no se pinta dos veces).
fn rasterizar(lineas: &[Linea]) -> (i32, i32, i32, i32, Vec<u8>) {
    if lineas.is_empty() {
        return (0, 0, 0, 0, Vec::new());
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for l in lineas {
        x0 = x0.min(l.a.0.min(l.b.0) - l.r);
        y0 = y0.min(l.a.1.min(l.b.1) - l.r);
        x1 = x1.max(l.a.0.max(l.b.0) + l.r);
        y1 = y1.max(l.a.1.max(l.b.1) + l.r);
    }
    let (px0, py0) = ((x0 - 32).div_euclid(64), (y0 - 32).div_euclid(64));
    let (px1, py1) = ((x1 + 32).div_euclid(64) + 1, (y1 + 32).div_euclid(64) + 1);
    let (w, h) = (px1 - px0, py1 - py0);
    let mut alfa = vec![0u8; (w * h) as usize];
    for l in lineas {
        let lx0 = ((l.a.0.min(l.b.0) - l.r - 32).div_euclid(64)).max(px0);
        let ly0 = ((l.a.1.min(l.b.1) - l.r - 32).div_euclid(64)).max(py0);
        let lx1 = ((l.a.0.max(l.b.0) + l.r + 32).div_euclid(64) + 1).min(px1);
        let ly1 = ((l.a.1.max(l.b.1) + l.r + 32).div_euclid(64) + 1).min(py1);
        for py in ly0..ly1 {
            for px in lx0..lx1 {
                let c = (px * 64 + 32, py * 64 + 32);
                let t = tinta(l.r, distancia(c, l.a, l.b));
                if t == 0 {
                    continue;
                }
                let k = ((py - py0) * w + (px - px0)) as usize;
                let v = realce(t);
                if v > alfa[k] {
                    alfa[k] = v;
                }
            }
        }
    }
    (px0, py0, w, h, alfa)
}

/// **Una linea de pluma redonda** por unos puntos (en 1/64 de pixel), de
/// `grosor64` de ancho, suavizada: el gato de BANK CAT y los iconos de la
/// maqueta. `pon(x, y, alfa)` recibe cada pixel con su tinta.
pub fn pluma(puntos: &[(i32, i32)], grosor64: i32, cerrada: bool, mut pon: impl FnMut(i32, i32, u8)) {
    let r = (grosor64 / 2).max(16);
    let mut lineas: Vec<Linea> = puntos.windows(2).map(|p| Linea { a: p[0], b: p[1], r }).collect();
    if cerrada && puntos.len() > 2 {
        lineas.push(Linea { a: puntos[puntos.len() - 1], b: puntos[0], r });
    }
    if puntos.len() == 1 {
        lineas.push(Linea { a: puntos[0], b: puntos[0], r });
    }
    let (x0, y0, w, h, alfa) = rasterizar(&lineas);
    for j in 0..h {
        for i in 0..w {
            let a = alfa[(j * w + i) as usize];
            if a != 0 {
                pon(x0 + i, y0 + j, a);
            }
        }
    }
}

/// Los puntos de un arco de elipse (centro y radios en 1/64 de pixel; de
/// `a0` a `a1` grados, 0 a la derecha y 90 ARRIBA), para [`pluma`].
pub fn arco(cx: i32, cy: i32, rx: i32, ry: i32, a0: i32, a1: i32, out: &mut Vec<(i32, i32)>) {
    let largo = (a1 - a0).abs() * rx.max(ry) / 64;
    let pasos = (largo / 160).clamp(6, 180);
    for k in 0..=pasos {
        let a = a0 + (a1 - a0) * k / pasos;
        out.push((cx + rx * trazo::coseno(a) / 32768, cy - ry * trazo::seno(a) / 32768));
    }
}

// ---------------------------------------------------------------------------
// LOS GLIFOS RASTERIZADOS, y la cache
// ---------------------------------------------------------------------------

/// Un glifo hecho pixeles: su caja respecto a la pluma (x a la derecha del
/// origen, y respecto a la base) y su tinta.
#[derive(Clone, Debug)]
pub struct Imagen {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Lo que avanza la pluma, en 1/64 de pixel.
    pub avance64: i32,
    pub alfa: Vec<u8>,
}

/// Cuartos de pixel: el glifo se rasteriza en cuatro posiciones, y cada letra
/// cae en la suya. Sin esto, redondear cada letra al pixel entero hace que
/// el espacio entre letras baile.
const FASES: usize = 4;

/// El aire que se le agrega a cada letra, en 1/16 de centesima de eme.
const AIRE16: i32 = 48;

struct Talla {
    px: u8,
    peso: Peso,
    alt: Alturas,
    hechos: [u64; 4 * FASES],
    glifos: Vec<Option<Imagen>>,
}

/// **La letra con su cache.** Una por app.
pub struct Letra {
    tallas: Vec<Talla>,
}

impl Default for Letra {
    fn default() -> Self {
        Letra::nueva()
    }
}

/// De un texto (Latin-1 o UTF-8 de dos bytes) a bytes Latin-1.
struct Letras<'a> {
    s: &'a [u8],
    i: usize,
}

impl Iterator for Letras<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        let b = *self.s.get(self.i)?;
        self.i += 1;
        if b < 0x80 {
            return Some(b);
        }
        // UTF-8 de dos bytes que cabe en Latin-1 (C2 xx, C3 xx).
        if (b == 0xC2 || b == 0xC3) && self.s.get(self.i).is_some_and(|&n| (0x80..0xC0).contains(&n)) {
            let n = self.s[self.i];
            self.i += 1;
            return Some(((b & 0x1F) << 6) | (n & 0x3F));
        }
        // Otro UTF-8 (mas alla de Latin-1), si de verdad lo es: se salta
        // entero y se dice '?'. Si no, es un byte Latin-1 (la n con tilde,
        // 0xF1, tambien empieza como un UTF-8 de cuatro bytes).
        if (0xC4..0xF8).contains(&b) {
            let mas = if b < 0xE0 { 1 } else if b < 0xF0 { 2 } else { 3 };
            let sigue = (0..mas).all(|k| self.s.get(self.i + k).is_some_and(|&n| (0x80..0xC0).contains(&n)));
            if sigue {
                self.i += mas;
                return Some(b'?');
            }
        }
        Some(b)
    }
}

/// Lo que hay que dibujar de un caracter: su glifo base y su acento.
fn partes(c: u8) -> Option<(glifos::Glifo, Option<(Acento, bool, i32)>)> {
    if let Some(gl) = glifos::glifo(c) {
        if c != 0x01 {
            return Some((gl, None));
        }
    }
    let (base, acento) = glifos::compuesta(c)?;
    let gl = glifos::glifo(base)?;
    let mayuscula = base.is_ascii_uppercase();
    Some((gl, Some((acento, mayuscula, glifos::centro(base, gl.avance)))))
}

/// Los trozos de un acento, en centesimas de eme.
fn acento(a: Acento, mayuscula: bool, c: i32, out: &mut Vec<Trozo>) {
    let alto = if mayuscula { -84 } else { -64 };
    let mut t = Vec::new();
    match a {
        Acento::Agudo => trazo::leer_en(&mut t, "L 0 0 9 -13", c - 4, alto + 4),
        Acento::Grave => trazo::leer_en(&mut t, "L 9 0 0 -13", c - 5, alto + 4),
        Acento::Tilde => trazo::leer_en(&mut t, "A -5 0 5 4 180 0 ; A 5 0 5 4 180 360", c, alto - 2),
        Acento::Dieresis => trazo::leer_en(&mut t, "P -8 0 ; P 8 0", c, alto - 2),
        Acento::Cedilla => trazo::leer_en(&mut t, "L 0 0 0 7 ; A -1 12 6 5 80 -170", c, 0),
    }
    out.extend(t);
}

impl Letra {
    pub const fn nueva() -> Letra {
        Letra { tallas: Vec::new() }
    }

    fn talla(&mut self, px: u8, peso: Peso) -> usize {
        if let Some(k) = self.tallas.iter().position(|t| t.px == px && t.peso == peso) {
            return k;
        }
        let mut glifos = Vec::new();
        glifos.resize_with(256 * FASES, || None);
        self.tallas.push(Talla { px, peso, alt: Alturas::de(px, peso), hechos: [0; 4 * FASES], glifos });
        self.tallas.len() - 1
    }

    /// **El glifo de `c`** en esta talla y peso, desplazado `fase` cuartos de
    /// pixel. `None` si la letra no existe (ni siquiera como '?').
    pub fn glifo(&mut self, c: u8, px: u8, peso: Peso, fase: usize) -> Option<&Imagen> {
        let k = self.talla(px, peso);
        let i = fase % FASES * 256 + c as usize;
        let t = &mut self.tallas[k];
        if t.hechos[i / 64] & (1 << (i % 64)) == 0 {
            t.hechos[i / 64] |= 1 << (i % 64);
            t.glifos[i] = Talla::hacer(&t.alt, peso, c, (fase % FASES) as i32 * 16);
        }
        self.tallas[k].glifos[i].as_ref()
    }

    /// **Lo que mide** un texto, en pixeles.
    pub fn medir(&mut self, s: &[u8], e: Estilo) -> i32 {
        let mut pluma64 = 0;
        let mut n = 0;
        for c in (Letras { s, i: 0 }) {
            let c = if e.mayusculas { mayuscula(c) } else { c };
            let av = match self.glifo(c, e.px, e.peso, 0) {
                Some(g) => g.avance64,
                None => continue,
            };
            pluma64 += av + e.espacio64;
            n += 1;
        }
        if n > 0 {
            pluma64 -= e.espacio64;
        }
        (pluma64 + 32) >> 6
    }

    /// **Escribe** `s` con la base en la fila `base` y empezando en `x`.
    /// `pon(x, y, alfa)` recibe cada pixel con tinta. Devuelve el ancho.
    pub fn escribir(&mut self, s: &[u8], e: Estilo, x: i32, base: i32, mut pon: impl FnMut(i32, i32, u8)) -> i32 {
        let mut pluma64 = x * 64;
        let mut n = 0;
        for c in (Letras { s, i: 0 }) {
            let c = if e.mayusculas { mayuscula(c) } else { c };
            let fase = ((pluma64 & 63) / 16) as usize;
            let origen = pluma64 >> 6;
            let Some(g) = self.glifo(c, e.px, e.peso, fase) else { continue };
            for j in 0..g.h {
                let fila = &g.alfa[(j * g.w) as usize..((j + 1) * g.w) as usize];
                for (i, &a) in fila.iter().enumerate() {
                    if a != 0 {
                        pon(origen + g.x + i as i32, base + g.y + j, a);
                    }
                }
            }
            pluma64 += g.avance64 + e.espacio64;
            n += 1;
        }
        if n > 0 {
            pluma64 -= e.espacio64;
        }
        ((pluma64 + 32) >> 6) - x
    }

    /// **Escribe recortado**: si no cabe en `max` pixeles, lo que quepa y
    /// tres puntos (el `text-overflow: ellipsis` de la maqueta).
    pub fn escribir_cabe(&mut self, s: &[u8], e: Estilo, x: i32, base: i32, max: i32, mut pon: impl FnMut(i32, i32, u8)) -> i32 {
        if self.medir(s, e) <= max {
            return self.escribir(s, e, x, base, pon);
        }
        let puntos = self.medir(b"...", e);
        let mut corte = s.len();
        while corte > 0 && self.medir(&s[..corte], e) + puntos > max {
            corte -= 1;
            // No partir un UTF-8 por la mitad.
            while corte > 0 && (0x80..0xC0).contains(&s[corte]) {
                corte -= 1;
            }
        }
        let a = self.escribir(&s[..corte], e, x, base, &mut pon);
        a + self.escribir(b"...", e, x + a, base, pon)
    }
}

/// La mayuscula de un byte Latin-1 (con las acentuadas).
fn mayuscula(c: u8) -> u8 {
    match c {
        b'a'..=b'z' => c - 32,
        0xE0..=0xFE if c != 0xF7 => c - 32,
        _ => c,
    }
}

impl Talla {
    fn hacer(alt: &Alturas, peso: Peso, c: u8, fase64: i32) -> Option<Imagen> {
        let (gl, acento_de) = match partes(c) {
            Some(p) => p,
            None => partes(b'?')?,
        };
        let mut trozos = Vec::new();
        trazo::leer(&mut trozos, gl.trazos);
        if let Some((a, may, centro)) = acento_de {
            acento(a, may, centro, &mut trozos);
        }
        // El aire entre letras: tres centesimas mas que el dibujo (repartidas a
        // los dos lados) y, en las tallas chicas, un poco mas aun. Es lo que
        // hace un navegador al encajar una letra de 12 px: si no, `lm` se
        // pega y la palabra se lee como una mancha.
        let corre16 = peso.holgura16() / 2 + AIRE16 / 2;
        let r = alt.pluma64 / 2;
        let mut lineas: Vec<Linea> = trozos
            .iter()
            .map(|t| {
                let p = |q: (i32, i32)| (alt.x(q.0 + corre16) + fase64, alt.y(q.1));
                let r = if t.punto { (r * 13 / 10).max(54) } else { r };
                Linea { a: p(t.a), b: p(t.b), r }
            })
            .collect();
        // EL ENCAJE EN X: el palo mas largo de la letra (la `l`, los de la
        // `H`, el de la `d`) se corre lo justo para que su borde izquierdo
        // caiga en el borde de un pixel. Sin esto, un palo de 1,7 px centrado
        // en un pixel entero son DOS columnas a medias: gris, no tinta. Solo
        // en tallas chicas: a partir de 3 px de pluma ya no se nota.
        if alt.pluma64 < 192 {
            let palo = lineas.iter().filter(|l| l.a.0 == l.b.0 && l.a.1 != l.b.1).max_by_key(|l| (l.a.1 - l.b.1).abs());
            if let Some(p) = palo {
                let resto = (p.a.0 - p.r).rem_euclid(64);
                let corre = if resto >= 32 { 64 - resto } else { -resto };
                for l in lineas.iter_mut() {
                    l.a.0 += corre;
                    l.b.0 += corre;
                }
            }
        }
        let (x, y, w, h, alfa) = rasterizar(&lineas);
        let chica = (15 - alt.px).max(0) * 3;
        let avance64 = alt.x(gl.avance * 16 + peso.holgura16() + AIRE16) + chica;
        Some(Imagen { x, y, w, h, avance64, alfa })
    }
}

#[cfg(test)]
mod pruebas;
