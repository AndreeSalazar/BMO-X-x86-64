//! **LA VENTANA** -- el armazon de una comunidad, de izquierda a derecha:
//!
//! ```text
//!    riel      una burbuja por tienda, con su gesto vivo; la elegida se
//!              vuelve cuadrada y lleva su pastilla blanca
//!    canales   la tienda: su estandarte, un canal por juego, y abajo quien
//!              eres (el gato de BMO-X) y a que juegas
//!    centro    el juego FIJADO (su camino, su titulo con glitch, JUGAR) y su
//!              actividad como mensajes de las PIEZAS de BMO-X
//!    piezas    las que trabajan y las que esperan: nunca gente inventada
//! ```
//!
//! Todo se pinta entero cada fotograma que se ve: la superficie es nuestra y
//! el escritorio solo la compone.

use crate::canvas::Canvas;
use crate::catalogo::Catalogo;
use crate::iconos;
use crate::mates::{azar, entre, fase, onda, seno};
use crate::tiendas::{color_de, PUESTOS};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_ludoteca::Camino;

pub const ANCHO: u32 = 1280;
pub const ALTO: u32 = 760;

const RIEL: i32 = 72;
const CANALES: i32 = 240;
const PIEZAS: i32 = 232;
const X_CENTRO: i32 = RIEL + CANALES;
const W_CENTRO: i32 = ANCHO as i32 - X_CENTRO - PIEZAS;
const X_PIEZAS: i32 = ANCHO as i32 - PIEZAS;

const BURBUJA: i32 = 48;
const PASO_RIEL: i32 = 56;
const FILA_CANAL: i32 = 34;
const Y_CANALES: i32 = 48 + 84 + 40;
const CABECERA: i32 = 48;

const FONDO_RIEL: Color = 0x001E_1F22;
const FONDO_CANALES: Color = 0x002B_2D31;
const FONDO_CENTRO: Color = 0x0031_3338;
const FONDO_PANEL: Color = 0x0023_2428;
const BURBUJA_FONDO: Color = 0x0031_3338;
const ELEGIDO: Color = 0x0040_4249;
const TEXTO: Color = 0x00DB_DEE1;
const TENUE: Color = 0x0094_9BA4;
const BLANCO: Color = 0x00F2_F3F5;
const OSCURO: Color = 0x0011_1214;
const ROSA: Color = 0x00FF_2E88;
const AZUL: Color = 0x003D_A5FF;
const VERDE: Color = 0x0023_A55A;
const AMBAR: Color = 0x00F0_B232;
const GRIS: Color = 0x0080_848E;

/// Lo que hace falta para pintar un fotograma.
pub struct Vista<'a> {
    pub cat: &'a Catalogo,
    pub tienda: usize,
    /// Los juegos de la tienda (indices del catalogo) y el elegido entre ellos.
    pub visibles: &'a [usize],
    pub sel: usize,
    pub ms: u32,
    /// Cuando cambio la tienda y cuando el juego: las entradas se animan.
    pub desde_tienda: u32,
    pub desde_juego: u32,
    /// El puntero, si esta dentro.
    pub puntero: Option<(i32, i32)>,
    /// Lo ultimo que dijo JUGAR.
    pub aviso: &'a [u8],
    /// Que piezas estan: ESTRATOS montado, PROTON-X en el disco.
    pub estratos: bool,
    pub proton: bool,
}

/// Donde cayo un clic.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Golpe {
    Tienda(usize),
    Juego(usize),
    Jugar,
}

fn dentro(x: i32, y: i32, (bx, by, bw, bh): (i32, i32, i32, i32)) -> bool {
    x >= bx && y >= by && x < bx + bw && y < by + bh
}

fn caja_burbuja(i: usize) -> (i32, i32, i32, i32) {
    ((RIEL - BURBUJA) / 2, 12 + i as i32 * PASO_RIEL + if i > 0 { 8 } else { 0 }, BURBUJA, BURBUJA)
}

fn caja_canal(k: usize) -> (i32, i32, i32, i32) {
    (RIEL + 8, Y_CANALES + 22 + k as i32 * FILA_CANAL, CANALES - 16, FILA_CANAL - 2)
}

fn caja_heroe() -> (i32, i32, i32, i32) {
    (X_CENTRO + 16, CABECERA + 16, W_CENTRO - 32, 232)
}

fn caja_jugar() -> (i32, i32, i32, i32) {
    let (x, y, _, h) = caja_heroe();
    (x + 28, y + h - 64, 156, 42)
}

/// **Que hay en `(x, y)`.**
pub fn golpe(x: i32, y: i32, n_juegos: usize) -> Option<Golpe> {
    if let Some(i) = (0..PUESTOS.len()).find(|&i| dentro(x, y, caja_burbuja(i))) {
        return Some(Golpe::Tienda(i));
    }
    if let Some(k) = (0..n_juegos).find(|&k| dentro(x, y, caja_canal(k))) {
        return Some(Golpe::Juego(k));
    }
    if n_juegos > 0 && dentro(x, y, caja_jugar()) {
        return Some(Golpe::Jugar);
    }
    None
}

// -- piezas de dibujo ---------------------------------------------------------

/// Una caja de esquinas redondas.
fn redondo(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
    let r = r.min(w / 2).min(h / 2).max(0);
    if r == 0 {
        cv.rect(x, y, w, h, c);
        return;
    }
    cv.rect(x + r, y, w - 2 * r, h, c);
    cv.rect(x, y + r, r, h - 2 * r, c);
    cv.rect(x + w - r, y + r, r, h - 2 * r, c);
    for (cx, cy) in [(x + r, y + r), (x + w - r - 1, y + r), (x + r, y + h - r - 1), (x + w - r - 1, y + h - r - 1)] {
        cv.disc(cx, cy, r, c);
    }
}

/// Texto "negrita": la misma letra dos veces, corrida un pixel.
fn negrita(cv: &mut Canvas, x: i32, y: i32, s: &[u8], c: Color, escala: i32) -> i32 {
    cv.text(x + 1, y, s, c, escala);
    cv.text(x, y, s, c, escala)
}

/// Un circulo de luz que se suma a lo que hay (las burbujas del estandarte).
fn luz(cv: &mut Canvas, cx: i32, cy: i32, r: i32, c: Color, fuerza: u32, recorte: (i32, i32, i32, i32)) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r && dentro(cx + dx, cy + dy, recorte) {
                cv.blend(cx + dx, cy + dy, c, fuerza, 100);
            }
        }
    }
}

fn camino_texto(c: Camino) -> (&'static [u8], Color) {
    match c {
        Camino::Nativo(_) => (b"NATIVO", 0x004D_E38F),
        Camino::ProtonX => (b"PROTON-X", 0x005E_F2E6),
        Camino::Pendiente => (b"PENDIENTE", 0x009A_A2B4),
    }
}

/// `Cyberpunk 2077` -> `cyberpunk-2077`, como un canal.
fn como_canal(titulo: &str, out: &mut [u8; 40]) -> usize {
    let mut n = 0;
    for b in titulo.bytes() {
        if n >= out.len() {
            break;
        }
        let c = match b {
            b'A'..=b'Z' => b + 32,
            b'a'..=b'z' | b'0'..=b'9' => b,
            b' ' | b'-' | b'_' => b'-',
            _ => continue,
        };
        if c == b'-' && (n == 0 || out[n - 1] == b'-') {
            continue;
        }
        out[n] = c;
        n += 1;
    }
    n
}

/// El gato de BMO-X, uno de cada `paso` pixeles: para los avatares.
fn gato(cv: &mut Canvas, x: i32, y: i32, paso: u32, trazo: Color, ojos: Color) {
    use crate::gato::{EYES, HEIGHT, STROKE, WIDTH};
    let bit = |m: &[u8], fx: u32, fy: u32| {
        let i = (fy * WIDTH + fx) as usize;
        m[i / 8] >> (i % 8) & 1 == 1
    };
    for fy in (0..HEIGHT).step_by(paso as usize) {
        for fx in (0..WIDTH).step_by(paso as usize) {
            let (mut t, mut o) = (false, false);
            for dy in 0..paso {
                for dx in 0..paso {
                    if fx + dx < WIDTH && fy + dy < HEIGHT {
                        t |= bit(&STROKE, fx + dx, fy + dy);
                        o |= bit(&EYES, fx + dx, fy + dy);
                    }
                }
            }
            let (px, py) = (x + (fx / paso) as i32, y + (fy / paso) as i32);
            if o {
                cv.put(px, py, ojos);
            } else if t {
                cv.put(px, py, trazo);
            }
        }
    }
}

// -- las cuatro columnas --------------------------------------------------------

fn riel(cv: &mut Canvas, v: &Vista) {
    cv.rect(0, 0, RIEL, ALTO as i32, FONDO_RIEL);
    let crece = ((v.ms.wrapping_sub(v.desde_tienda)).min(220) * 256 / 220) as i32;
    for (i, p) in PUESTOS.iter().enumerate() {
        let (x, y, w, h) = caja_burbuja(i);
        let elegida = i == v.tienda;
        let encima = v.puntero.is_some_and(|(px, py)| dentro(px, py, (x, y, w, h)));
        // La forma: circulo; cuadrada con esquinas suaves si esta elegida
        // (y se transforma al elegirla) o si el puntero esta encima.
        let r = if elegida { entre(24, 15, crece) } else if encima { 16 } else { 24 };
        let pastilla = if elegida { entre(8, 40, crece) } else if encima { 20 } else { 0 };
        if pastilla > 0 {
            redondo(cv, -4, y + (h - pastilla) / 2, 8, pastilla, 3, BLANCO);
        }
        let fondo = if elegida { p.color } else if encima { mezclar(p.color, BURBUJA_FONDO, 60, 256) } else { BURBUJA_FONDO };
        redondo(cv, x, y, w, h, r, fondo);
        let tinta = if elegida { OSCURO } else { p.color };
        iconos::pintar(cv, p.gesto, x + w / 2, y + h / 2, v.ms + i as u32 * 137, tinta, fondo);
        if i == 0 {
            cv.rect(x + 8, y + h + 7, w - 16, 2, 0x0035_363C);
        }
    }
}

fn canales(cv: &mut Canvas, v: &Vista) {
    let p = &PUESTOS[v.tienda];
    cv.rect(RIEL, 0, CANALES, ALTO as i32, FONDO_CANALES);
    // La cabecera: el nombre de la tienda.
    negrita(cv, RIEL + 16, 16, p.nombre().as_bytes(), BLANCO, 1);
    cv.disc(RIEL + CANALES - 20, 24, 5, p.color);
    cv.rect(RIEL, CABECERA - 1, CANALES, 1, 0x001F_2023);
    // El estandarte: su degradado y dos luces que flotan.
    let est = (RIEL, CABECERA, CANALES, 84);
    cv.gradient(est.0, est.1, est.2, est.3, p.color, p.color2);
    let f1 = seno(crate::mates::fase(v.ms, 6000)) * 8 / 256;
    let f2 = seno(crate::mates::fase(v.ms + 3000, 6000)) * 8 / 256;
    luz(cv, RIEL + CANALES - 30, CABECERA + 6 + f1, 54, BLANCO, 14, est);
    luz(cv, RIEL + 60, CABECERA + 96 + f2, 44, OSCURO, 15, est);
    // Sobre un estandarte claro, letra oscura: se mira su brillo.
    let brillo = ((p.color >> 16 & 255) * 3 + (p.color >> 8 & 255) * 6 + (p.color & 255)) / 10;
    let sobre = if brillo > 170 { OSCURO } else { BLANCO };
    let mut t = [0u8; 32];
    let n = crate::fmt_num(v.visibles.len() as u64, &mut t);
    let k = negrita(cv, RIEL + 16, CABECERA + 60, &t[..n], sobre, 1);
    negrita(cv, RIEL + 16 + k, CABECERA + 60, if v.visibles.len() == 1 { b" juego" } else { b" juegos" }, sobre, 1);
    // Los canales: uno por juego.
    cv.text(RIEL + 16, Y_CANALES, b"JUEGOS", TENUE, 1);
    for (k, &i) in v.visibles.iter().enumerate() {
        let (x, y, w, h) = caja_canal(k);
        if y + h > ALTO as i32 - 120 {
            break;
        }
        let elegido = k == v.sel;
        let encima = v.puntero.is_some_and(|(px, py)| dentro(px, py, (x, y, w, h)));
        if elegido || encima {
            redondo(cv, x, y, w, h, 5, if elegido { ELEGIDO } else { 0x0035_373C });
        }
        let j = &v.cat.l.juegos[i];
        cv.text(x + 8, y + 9, b"#", GRIS, 1);
        let mut nombre = [0u8; 40];
        let n = como_canal(&j.titulo, &mut nombre);
        let tinta = if elegido { BLANCO } else { TENUE };
        if elegido {
            negrita(cv, x + 26, y + 9, &nombre[..n.min(22)], tinta, 1);
        } else {
            cv.text(x + 26, y + 9, &nombre[..n.min(22)], tinta, 1);
        }
        // PROTON-X lleva su punto vivo: es lo que se esta construyendo.
        if v.cat.camino(i) == Camino::ProtonX {
            let r = 3 + onda(v.ms, 1600) / 128;
            cv.disc(x + w - 14, y + h / 2, r, 0x005E_F2E6);
        }
    }
    if v.visibles.is_empty() {
        cv.text(RIEL + 16, Y_CANALES + 28, b"nada de esta tienda", GRIS, 1);
        cv.text(RIEL + 16, Y_CANALES + 48, b"todavia", GRIS, 1);
    }
    // Quien eres: el gato de BMO-X, y a que juegas.
    let yp = ALTO as i32 - 56;
    cv.rect(RIEL, yp, CANALES, 56, FONDO_PANEL);
    cv.disc(RIEL + 30, yp + 28, 19, OSCURO);
    let ojos = if (v.ms % 6000) < 140 { OSCURO } else { 0x005E_F2E6 };
    gato(cv, RIEL + 30 - 15, yp + 28 - 18, 5, BLANCO, ojos);
    cv.disc(RIEL + 44, yp + 42, 6, FONDO_PANEL);
    cv.disc(RIEL + 44, yp + 42, 4, VERDE);
    negrita(cv, RIEL + 58, yp + 12, b"BMO-X", BLANCO, 1);
    let jugando: &[u8] = v.visibles.get(v.sel).map_or(b"en la LUDOTECA", |&i| v.cat.l.juegos[i].titulo.as_bytes());
    cv.text_fit(RIEL + 58, yp + 30, jugando, TENUE, CANALES - 70);
}

fn centro(cv: &mut Canvas, v: &Vista) {
    cv.rect(X_CENTRO, 0, W_CENTRO, ALTO as i32, FONDO_CENTRO);
    let elegido = v.visibles.get(v.sel).copied();
    // La cabecera del canal.
    cv.text(X_CENTRO + 18, 16, b"#", GRIS, 1);
    let mut nombre = [0u8; 40];
    let n = elegido.map_or(0, |i| como_canal(&v.cat.l.juegos[i].titulo, &mut nombre));
    let k = negrita(cv, X_CENTRO + 36, 16, if n > 0 { &nombre[..n] } else { b"bienvenida" }, BLANCO, 1);
    cv.rect(X_CENTRO + 48 + k, 12, 1, 24, 0x003F_4147);
    if let Some(i) = elegido {
        let (t, c) = camino_texto(v.cat.camino(i));
        let w = cv.text(X_CENTRO + 60 + k, 16, t, c, 1);
        cv.text(X_CENTRO + 60 + k + w, 16, b" . ", TENUE, 1);
        cv.text(X_CENTRO + 84 + k + w, 16, v.cat.l.juegos[i].tienda.nombre().as_bytes(), TENUE, 1);
    }
    cv.text(X_CENTRO + W_CENTRO - 40, 16, b"F4", GRIS, 1);
    cv.rect(X_CENTRO, CABECERA - 1, W_CENTRO, 1, 0x0026_272B);

    // -- el juego fijado --
    let (hx, hy, hw, hh) = caja_heroe();
    let (color, titulo, camino, tienda) = match elegido {
        Some(i) => {
            let j = &v.cat.l.juegos[i];
            (color_de(j.tienda), j.titulo.as_bytes(), Some(v.cat.camino(i)), j.tienda.nombre())
        }
        None => (PUESTOS[v.tienda].color, &b"Sin juegos todavia"[..], None, PUESTOS[v.tienda].nombre()),
    };
    redondo(cv, hx, hy, hw, hh, 12, 0x002B_2D31);
    cv.gradient(hx + 6, hy + 6, hw - 12, hh - 12, mezclar(color, 0x002B_2D31, 150, 256), 0x002B_2D31);
    // Las capas que flotan detras: profundidad sin una imagen inventada.
    let f = seno(fase(v.ms, 6000)) * 10 / 256;
    let capa = (hx + hw - 300, hy + 28 + f, 250, 150);
    for j in 0..capa.3 {
        for i in [0, capa.2 - 1] {
            cv.blend(capa.0 + i, capa.1 + j, BLANCO, 16, 100);
        }
    }
    for i in 0..capa.2 {
        for j in [0, capa.3 - 1] {
            cv.blend(capa.0 + i, capa.1 + j, BLANCO, 16, 100);
        }
    }
    luz(cv, hx + hw - 120, hy + 60 - f, 70, color, 10, (hx, hy, hw, hh));
    // La pastilla del camino.
    if let Some(c) = camino {
        let (t, tc) = camino_texto(c);
        let w = (t.len() + tienda.len() + 3) as i32 * 8 + 20;
        redondo(cv, hx + 28, hy + 26, w, 24, 12, mezclar(OSCURO, color, 150, 256));
        let r = 3 + onda(v.ms, 1800) / 128;
        cv.disc(hx + 40, hy + 38, r, tc);
        let k = cv.text(hx + 50, hy + 30, t, tc, 1);
        cv.text(hx + 50 + k, hy + 30, b" . ", TENUE, 1);
        cv.text(hx + 74 + k, hy + 30, tienda.as_bytes(), BLANCO, 1);
    }
    // El titulo: se revela letra a letra al elegirlo, y cada 4,2 s da glitch.
    let revela = (v.ms.wrapping_sub(v.desde_juego).min(600) * 256 / 600) as usize;
    let cuantas = (titulo.len() * revela / 256).max(if revela > 0 { 1 } else { 0 });
    let escala = if titulo.len() > 22 { 2 } else { 3 };
    let ty = hy + 70;
    let g = v.ms % 4200;
    if (3780..3990).contains(&g) || revela < 200 {
        let d = 2 + (azar(v.ms / 40) % 4) as i32;
        cv.text(hx + 28 - d, ty, &titulo[..cuantas], ROSA, escala);
        cv.text(hx + 28 + d, ty, &titulo[..cuantas], AZUL, escala);
    }
    cv.text(hx + 29, ty, &titulo[..cuantas], BLANCO, escala);
    cv.text(hx + 28, ty, &titulo[..cuantas], BLANCO, escala);
    // JUGAR: un liquido que corre por dentro.
    let (bx, by, bw, bh) = caja_jugar();
    let p = &PUESTOS[v.tienda];
    // El boton lleva los colores de la tienda DEL JUEGO, no la del riel.
    let suya = elegido.and_then(|i| PUESTOS.iter().find(|q| q.tienda == Some(v.cat.l.juegos[i].tienda))).unwrap_or(p);
    let puede = elegido.is_some_and(|i| v.cat.orden(i).is_some());
    if puede {
        let corre = fase(v.ms, 1400);
        for i in 0..bw {
            let t = ((i * 256 / bw + corre) % 256 - 128).abs() * 2;
            cv.rect(bx + i, by, 1, bh, mezclar(suya.color2, suya.color, t as u32, 256));
        }
        // Las esquinas redondas: se recortan con el fondo.
        for (cx, cy) in [(bx, by), (bx + bw - 1, by), (bx, by + bh - 1), (bx + bw - 1, by + bh - 1)] {
            cv.put(cx, cy, mezclar(color, 0x002B_2D31, 40, 256));
        }
        let encima = v.puntero.is_some_and(|(px, py)| dentro(px, py, (bx, by, bw, bh)));
        if encima {
            cv.glow(bx, by, bw, bh, suya.color, 6, 40);
        }
        negrita(cv, bx + (bw - 5 * 8 * 2) / 2, by + 5, b"JUGAR", OSCURO, 2);
    } else {
        redondo(cv, bx, by, bw, bh, 8, 0x003F_4147);
        let t: &[u8] = if elegido.is_some() { b"PENDIENTE" } else { b"--" };
        cv.text(bx + (bw - t.len() as i32 * 8) / 2, by + 13, t, TENUE, 1);
    }
    if let Some(i) = elegido {
        if let Some(o) = v.cat.orden(i) {
            cv.text_fit(bx + bw + 16, by + 13, o.as_bytes(), mezclar(BLANCO, color, 200, 256), hw - bw - 60);
        }
    }

    // -- la actividad: lo que dijeron las piezas --
    let mut y = hy + hh + 20;
    for (k, (quien, ini, c, texto)) in mensajes(v, elegido).iter().enumerate() {
        if quien.is_empty() {
            continue;
        }
        let entra = (v.ms.wrapping_sub(v.desde_juego) as i32 - 120 * k as i32).clamp(0, 300) * 256 / 300;
        if entra == 0 {
            continue;
        }
        let dy = (256 - entra) * 14 / 256;
        let yy = y + dy;
        let tinta = |c: Color| mezclar(c, FONDO_CENTRO, entra as u32, 256);
        cv.disc(X_CENTRO + 40, yy + 18, 18, tinta(*c));
        negrita(cv, X_CENTRO + 36, yy + 10, &[*ini], tinta(OSCURO), 1);
        let w = negrita(cv, X_CENTRO + 72, yy + 2, quien, tinta(*c), 1);
        redondo(cv, X_CENTRO + 80 + w, yy + 2, 44, 16, 3, tinta(p.color));
        cv.text(X_CENTRO + 82 + w, yy + 2, b"PIEZA", tinta(OSCURO), 1);
        cv.text(X_CENTRO + 136 + w, yy + 2, b"hoy", tinta(TENUE), 1);
        cv.text_fit(X_CENTRO + 72, yy + 22, texto, tinta(TEXTO), W_CENTRO - 100);
        y += 52;
    }
    // Lo que contesto el escritorio a JUGAR.
    if !v.aviso.is_empty() {
        cv.text_fit(X_CENTRO + 72, y + 4, v.aviso, 0x005E_F2E6, W_CENTRO - 100);
    }

    // -- la caja de abajo: las teclas, y el juez que "escribe" --
    let yb = ALTO as i32 - 70;
    for k in 0..3 {
        let s = seno(fase(v.ms + k * 150, 1200)).max(0) * 4 / 256;
        cv.disc(X_CENTRO + 24 + k as i32 * 8, yb + 6 - s, 2, TENUE);
    }
    cv.text(X_CENTRO + 52, yb - 2, b"el juez espera la suma de la tienda...", TENUE, 1);
    redondo(cv, X_CENTRO + 16, yb + 18, W_CENTRO - 32, 40, 8, 0x0038_3A40);
    cv.text(X_CENTRO + 32, yb + 30, b"ENTER juega   flechas eligen   T tienda siguiente   Esc cierra", 0x006D_6F78, 1);
}

/// Los mensajes de las piezas para el juego elegido: lo que es VERDAD hoy.
fn mensajes(v: &Vista, elegido: Option<usize>) -> [(&'static [u8], u8, Color, &'static [u8]); 3] {
    const NADA: (&[u8], u8, Color, &[u8]) = (b"", b' ', 0, b"");
    match elegido.map(|i| v.cat.camino(i)) {
        Some(Camino::ProtonX) => [
            (b"PROTON-X", b'P', 0x005E_F2E6, b"Galaxy arranca: redgalaxy::api::Init vuelve sin error."),
            (b"la casa", b'C', 0x00B4_8CFF, b"secur32.dll es de la casa: curl tiene su tabla SSPI."),
            (b"ESTRATOS", b'E', 0x004D_E38F, b"Los ficheros grandes se leen a la carta: solo el trozo que se pide."),
        ],
        Some(Camino::Nativo(_)) => [
            (b"el juez", b'J', 0x00FF_C24D, b"Su motor vive en BMO-X: se juega sin Windows de por medio."),
            (b"BMO-X", b'B', 0x004D_E38F, b"ENTER lo lanza desde el escritorio, como un run tecleado."),
            NADA,
        ],
        Some(Camino::Pendiente) => [(b"la LUDOTECA", b'L', 0x009A_A2B4, b"La tienda dio el titulo; faltan sus ficheros para saber como se juega."), NADA, NADA],
        None => [
            (b"la LUDOTECA", b'L', 0x009A_A2B4, b"Pon tus lineas en ludoteca/ludoteca.txt, o deja el juego en D: o en apps/."),
            NADA,
            NADA,
        ],
    }
}

fn piezas(cv: &mut Canvas, v: &Vista) {
    cv.rect(X_PIEZAS, 0, PIEZAS, ALTO as i32, FONDO_CANALES);
    let lista: [(&[u8], u8, Color, Color, &[u8]); 6] = [
        (b"PROTON-X", b'P', 0x005E_F2E6, if v.proton { VERDE } else { GRIS }, if v.proton { b"listo en sys/" } else { b"no esta en sys/" }),
        (b"ESTRATOS", b'E', 0x004D_E38F, if v.estratos { VERDE } else { AMBAR }, if v.estratos { b"F: montado" } else { b"sin montar" }),
        (b"el juez", b'J', 0x00FF_C24D, VERDE, b"comprueba sumas"),
        (b"la 3060", b'3', 0x00A6_FF3B, VERDE, b"pinta el escritorio"),
        (b"la ANTENA", b'A', 0x00FF_8A5C, AMBAR, b"sin conectar"),
        (b"TLS 1.3", b'T', 0x00FF_4F8B, GRIS, b"el muro: por hacer"),
    ];
    let mut y = 20;
    for (k, (nombre, ini, c, estado, hace)) in lista.iter().enumerate() {
        if k == 0 || k == 4 {
            cv.text(X_PIEZAS + 16, y + 8, if k == 0 { b"TRABAJANDO - 4" } else { b"ESPERANDO - 2" }, TENUE, 1);
            y += 34;
        }
        let encima = v.puntero.is_some_and(|(px, py)| dentro(px, py, (X_PIEZAS + 8, y, PIEZAS - 16, 44)));
        if encima {
            redondo(cv, X_PIEZAS + 8, y, PIEZAS - 16, 44, 5, 0x0035_373C);
        }
        cv.disc(X_PIEZAS + 32, y + 22, 16, *c);
        negrita(cv, X_PIEZAS + 28, y + 14, &[*ini], OSCURO, 1);
        cv.disc(X_PIEZAS + 44, y + 34, 6, FONDO_CANALES);
        let vivo = *estado == VERDE;
        let r = if vivo { 3 + onda(v.ms + k as u32 * 300, 1600) / 128 } else { 4 };
        cv.disc(X_PIEZAS + 44, y + 34, r, *estado);
        negrita(cv, X_PIEZAS + 58, y + 6, nombre, TEXTO, 1);
        cv.text_fit(X_PIEZAS + 58, y + 24, hace, TENUE, PIEZAS - 66);
        y += 48;
    }
}

/// **Un fotograma entero.**
pub fn pintar(cv: &mut Canvas, v: &Vista) {
    centro(cv, v);
    canales(cv, v);
    riel(cv, v);
    piezas(cv, v);
}
