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
use crate::tiendas::PUESTOS;
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

const FONDO_RIEL: Color = 0x0007_0F1C;
const FONDO_CANALES: Color = 0x000A_1626;
const FONDO_CENTRO: Color = 0x000C_1B2E;
const FONDO_PANEL: Color = 0x0008_1322;
const ELEGIDO: Color = 0x0014_3A30;
const TEXTO: Color = 0x00D6_F5E3;
const TENUE: Color = 0x007F_A392;
const BLANCO: Color = 0x00E8_FFF1;
const OSCURO: Color = 0x0006_1018;
/// El verde neon de la LUDOTECA (01-10: *"verde con azul oscuro total"*).
const NEON: Color = 0x0039_FF88;
const NEON2: Color = 0x0016_A85A;
const BORDE: Color = 0x0016_303F;
const ENCIMA: Color = 0x0012_2840;
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
    /// Que tiendas tienen ya su logo oficial en `ludoteca/logos/`.
    pub logos: &'a [bool],
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
            redondo(cv, -4, y + (h - pastilla) / 2, 8, pastilla, 3, p.color);
        }
        // La insignia: su fondo, y elegida, su color oficial lleno.
        let fondo = if elegida { p.color } else if encima { mezclar(p.color, p.fondo, 50, 256) } else { p.fondo };
        redondo(cv, x, y, w, h, r, fondo);
        let tinta = if elegida { p.fondo } else { p.color };
        iconos::pintar(cv, p.gesto, x + w / 2, y + h / 2, v.ms + i as u32 * 137, tinta, fondo);
        if i == 0 {
            cv.rect(x + 8, y + h + 7, w - 16, 2, BORDE);
        }
    }
}

fn canales(cv: &mut Canvas, v: &Vista) {
    let p = &PUESTOS[v.tienda];
    cv.rect(RIEL, 0, CANALES, ALTO as i32, FONDO_CANALES);
    // La cabecera: el nombre de la tienda.
    negrita(cv, RIEL + 16, 16, p.nombre().as_bytes(), BLANCO, 1);
    cv.disc(RIEL + CANALES - 20, 24, 4 + onda(v.ms, 1600) / 128, p.color);
    cv.rect(RIEL, CABECERA - 1, CANALES, 1, BORDE);
    // El estandarte: su degradado y dos luces que flotan.
    let est = (RIEL, CABECERA, CANALES, 84);
    // El estandarte en el color OFICIAL de la tienda: dentro de GOG, se ve GOG.
    cv.gradient(est.0, est.1, est.2, est.3, mezclar(p.color, p.fondo, 200, 256), p.fondo);
    let f1 = seno(crate::mates::fase(v.ms, 6000)) * 8 / 256;
    let f2 = seno(crate::mates::fase(v.ms + 3000, 6000)) * 8 / 256;
    luz(cv, RIEL + CANALES - 30, CABECERA + 6 + f1, 54, BLANCO, 14, est);
    luz(cv, RIEL + 60, CABECERA + 96 + f2, 44, OSCURO, 15, est);
    // Sobre un estandarte claro, letra oscura: se mira su brillo.
    let sobre = BLANCO;
    let mut t = [0u8; 32];
    let n = crate::fmt_num(v.visibles.len() as u64, &mut t);
    let k = negrita(cv, RIEL + 16, CABECERA + 60, &t[..n], sobre, 1);
    negrita(cv, RIEL + 16 + k, CABECERA + 60, if v.visibles.len() == 1 { b" juego" } else { b" juegos" }, sobre, 1);
    // El hueco del logo oficial: dicho, y listo para cuando llegue.
    let oficial = v.logos.get(v.tienda).copied().unwrap_or(false);
    cv.text(RIEL + 16, CABECERA + 14, if oficial { b"logo oficial: listo" } else { b"insignia propia" }, mezclar(BLANCO, p.fondo, 170, 256), 1);
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
            redondo(cv, x, y, w, h, 5, if elegido { ELEGIDO } else { ENCIMA });
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
            cv.disc(x + w - 14, y + h / 2, r, NEON);
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
    let ojos = if (v.ms % 6000) < 140 { OSCURO } else { NEON };
    gato(cv, RIEL + 30 - 15, yp + 28 - 18, 5, BLANCO, ojos);
    cv.disc(RIEL + 44, yp + 42, 6, FONDO_PANEL);
    cv.disc(RIEL + 44, yp + 42, 4, NEON);
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
    cv.rect(X_CENTRO + 48 + k, 12, 1, 24, BORDE);
    if let Some(i) = elegido {
        let (t, c) = camino_texto(v.cat.camino(i));
        let w = cv.text(X_CENTRO + 60 + k, 16, t, c, 1);
        cv.text(X_CENTRO + 60 + k + w, 16, b" . ", TENUE, 1);
        cv.text(X_CENTRO + 84 + k + w, 16, v.cat.l.juegos[i].tienda.nombre().as_bytes(), TENUE, 1);
    }
    cv.text(X_CENTRO + W_CENTRO - 40, 16, b"F4", GRIS, 1);
    // El gato de BMO-X en medio de la cabecera, en su cajita.
    let gx = X_CENTRO + W_CENTRO / 2 + 40;
    redondo(cv, gx - 4, 6, 34, 36, 6, OSCURO);
    gato(cv, gx + 2, 9, 6, NEON, if (v.ms % 6000) < 140 { OSCURO } else { BLANCO });
    cv.rect(X_CENTRO, CABECERA - 1, W_CENTRO, 1, BORDE);

    // -- el juego fijado --
    let (hx, hy, hw, hh) = caja_heroe();
    let (titulo, camino, tienda) = match elegido {
        Some(i) => {
            let j = &v.cat.l.juegos[i];
            (j.titulo.as_bytes(), Some(v.cat.camino(i)), j.tienda.nombre())
        }
        None => (&b"Sin juegos todavia"[..], None, PUESTOS[v.tienda].nombre()),
    };
    // El marco verde que late, y dentro el azul de la noche.
    let late = 25 + onda(v.ms, 2400) as u32 * 35 / 256;
    cv.glow(hx, hy, hw, hh, NEON, 10, late);
    redondo(cv, hx, hy, hw, hh, 12, NEON);
    redondo(cv, hx + 3, hy + 3, hw - 6, hh - 6, 10, 0x000A_1A2C);
    cv.gradient(hx + 8, hy + 8, hw - 16, hh - 16, mezclar(NEON2, 0x000A_1A2C, 46, 256), 0x000A_1A2C);
    recreativa(cv, hx + hw - 210, hy + 18, v.ms);
    // La pastilla del camino.
    if let Some(c) = camino {
        let (t, tc) = camino_texto(c);
        let w = (t.len() + tienda.len() + 3) as i32 * 8 + 20;
        redondo(cv, hx + 28, hy + 26, w, 24, 12, 0x0010_2A22);
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
    cv.text(hx + 29, ty, &titulo[..cuantas], 0x00C8_FFD8, escala);
    cv.text(hx + 28, ty, &titulo[..cuantas], 0x00C8_FFD8, escala);
    // JUGAR: un liquido que corre por dentro.
    let (bx, by, bw, bh) = caja_jugar();
    let puede = elegido.is_some_and(|i| v.cat.orden(i).is_some());
    if puede {
        let corre = fase(v.ms, 1400);
        for i in 0..bw {
            let t = ((i * 256 / bw + corre) % 256 - 128).abs() * 2;
            cv.rect(bx + i, by, 1, bh, mezclar(0x00A8_FFCC, NEON, (t / 3) as u32, 256));
        }
        // Las esquinas redondas: se recortan con el fondo.
        for (cx, cy) in [(bx, by), (bx + bw - 1, by), (bx, by + bh - 1), (bx + bw - 1, by + bh - 1)] {
            cv.put(cx, cy, 0x000A_1A2C);
        }
        let encima = v.puntero.is_some_and(|(px, py)| dentro(px, py, (bx, by, bw, bh)));
        if encima {
            cv.glow(bx, by, bw, bh, NEON, 6, 45);
        }
        negrita(cv, bx + (bw - 5 * 8 * 2) / 2, by + 5, b"JUGAR", OSCURO, 2);
    } else {
        redondo(cv, bx, by, bw, bh, 8, BORDE);
        let t: &[u8] = if elegido.is_some() { b"PENDIENTE" } else { b"--" };
        cv.text(bx + (bw - t.len() as i32 * 8) / 2, by + 13, t, TENUE, 1);
    }
    if let Some(i) = elegido {
        if let Some(o) = v.cat.orden(i) {
            cv.text_fit(bx + bw + 16, by + 13, o.as_bytes(), TENUE, hw - bw - 280);
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
        cv.disc(X_CENTRO + 40, yy + 18, 18, tinta(mezclar(*c, 0x0010_2A30, 150, 256)));
        negrita(cv, X_CENTRO + 36, yy + 10, &[*ini], tinta(BLANCO), 1);
        let w = negrita(cv, X_CENTRO + 72, yy + 2, quien, tinta(mezclar(*c, NEON, 90, 256)), 1);
        redondo(cv, X_CENTRO + 80 + w, yy + 2, 44, 16, 3, tinta(NEON));
        cv.text(X_CENTRO + 82 + w, yy + 2, b"PIEZA", tinta(OSCURO), 1);
        cv.text(X_CENTRO + 136 + w, yy + 2, b"hoy", tinta(TENUE), 1);
        cv.text_fit(X_CENTRO + 72, yy + 22, texto, tinta(TEXTO), W_CENTRO - 100);
        y += 52;
    }
    // Lo que contesto el escritorio a JUGAR.
    if !v.aviso.is_empty() {
        cv.text_fit(X_CENTRO + 72, y + 4, v.aviso, NEON, W_CENTRO - 100);
    }

    // -- la caja de abajo: las teclas, y el juez que "escribe" --
    let yb = ALTO as i32 - 70;
    for k in 0..3 {
        let s = seno(fase(v.ms + k * 150, 1200)).max(0) * 4 / 256;
        cv.disc(X_CENTRO + 24 + k as i32 * 8, yb + 6 - s, 2, TENUE);
    }
    cv.text(X_CENTRO + 52, yb - 2, b"el juez espera la suma de la tienda...", TENUE, 1);
    redondo(cv, X_CENTRO + 16, yb + 18, W_CENTRO - 32, 40, 8, 0x0010_2238);
    cv.text(X_CENTRO + 32, yb + 30, b"ENTER juega   flechas eligen   T tienda siguiente   Esc cierra", TENUE, 1);
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
            redondo(cv, X_PIEZAS + 8, y, PIEZAS - 16, 44, 5, ENCIMA);
        }
        cv.disc(X_PIEZAS + 32, y + 22, 16, mezclar(*c, 0x0010_2A30, 150, 256));
        negrita(cv, X_PIEZAS + 28, y + 14, &[*ini], BLANCO, 1);
        cv.disc(X_PIEZAS + 44, y + 34, 6, FONDO_CANALES);
        let vivo = *estado == VERDE;
        let r = if vivo { 3 + onda(v.ms + k as u32 * 300, 1600) / 128 } else { 4 };
        cv.disc(X_PIEZAS + 44, y + 34, r, *estado);
        negrita(cv, X_PIEZAS + 58, y + 6, nombre, TEXTO, 1);
        cv.text_fit(X_PIEZAS + 58, y + 24, hace, TENUE, PIEZAS - 66);
        y += 48;
    }
    estado(cv, v);
}

/// **La tarjeta de estado**, abajo a la derecha: lo que es verdad ahora.
fn estado(cv: &mut Canvas, v: &Vista) {
    let (x, y, w, h) = (X_PIEZAS + 8, ALTO as i32 - 112, PIEZAS - 16, 100);
    redondo(cv, x, y, w, h, 6, NEON);
    redondo(cv, x + 1, y + 1, w - 2, h - 2, 5, OSCURO);
    cv.text(x + 10, y + 8, b"LUDOTECA 01", NEON, 1);
    cv.rect(x + 10, y + 26, w - 20, 1, BORDE);
    let mut n = [0u8; 8];
    let k = crate::fmt_num(v.cat.l.juegos.len() as u64, &mut n);
    let a = cv.text(x + 10, y + 34, b"JUEGOS: ", TENUE, 1);
    cv.text(x + 10 + a, y + 34, &n[..k], TEXTO, 1);
    let a = cv.text(x + 10, y + 52, b"PROTON-X: ", TENUE, 1);
    cv.text(x + 10 + a, y + 52, if v.proton { b"listo" } else { b"falta" }, TEXTO, 1);
    let a = cv.text(x + 10, y + 70, b"ESTRATOS: ", TENUE, 1);
    cv.text(x + 10 + a, y + 70, if v.estratos { b"montado" } else { b"sin montar" }, TEXTO, 1);
    // Un cursor que parpadea, como una terminal viva.
    if (v.ms / 500) % 2 == 0 {
        cv.rect(x + w - 18, y + 72, 8, 12, NEON);
    }
}

/// **La recreativa**: una maquina de salon dibujada a trazo verde, con su
/// pantalla viva (una linea que baja y un parpadeo) y la palanca que se mueve.
fn recreativa(cv: &mut Canvas, x: i32, y: i32, ms: u32) {
    let t = |cv: &mut Canvas, a: (i32, i32), b: (i32, i32)| {
        cv.line(a, b, NEON);
        cv.line((a.0 + 1, a.1), (b.0 + 1, b.1), NEON);
    };
    let (w, l) = (120, 24); // ancho del frente y fondo del lateral
    // El lateral, en perspectiva.
    for &(a, b) in &[((x + w, y + 4), (x + w + l, y - 6)), ((x + w + l, y - 6), (x + w + l, y + 186)), ((x + w + l, y + 186), (x + w, y + 196))] {
        t(cv, a, b);
    }
    // El cuerpo.
    cv.rect(x + 2, y + 2, w - 2, 194, 0x000E_2230);
    for &(a, b) in &[((x, y), (x + w, y)), ((x, y), (x, y + 196)), ((x + w, y), (x + w, y + 196)), ((x, y + 196), (x + w, y + 196))] {
        t(cv, a, b);
    }
    // La marquesina con su nombre.
    t(cv, (x, y + 28), (x + w, y + 28));
    cv.text(x + 12, y + 7, b"LUDOTECA", NEON, 1);
    // La pantalla: un hueco oscuro, una linea que baja y un parpadeo.
    let (sx, sy, sw, sh) = (x + 14, y + 38, w - 28, 66);
    cv.rect(sx, sy, sw, sh, 0x0004_0C12);
    let baja = (ms % 1600) as i32 * sh / 1600;
    cv.rect(sx, sy + baja, sw, 2, mezclar(NEON, 0x0004_0C12, 120, 256));
    let brillo = 100 + (azar(ms / 90) % 80);
    cv.text(sx + 10, sy + 14, b"BMO-X", mezclar(NEON, 0x0004_0C12, brillo, 256), 1);
    cv.text(sx + 10, sy + 36, b"INSERT", mezclar(NEON, 0x0004_0C12, if (ms / 400) % 2 == 0 { 220 } else { 60 }, 256), 1);
    // El tablero de mandos, inclinado hacia fuera.
    t(cv, (x, y + 112), (x - 8, y + 128));
    t(cv, (x + w, y + 112), (x + w + 8, y + 128));
    t(cv, (x - 8, y + 128), (x + w + 8, y + 128));
    t(cv, (x, y + 112), (x + w, y + 112));
    // La palanca, que se mueve, y tres botones.
    let lado = seno(fase(ms, 1300)) * 4 / 256;
    t(cv, (x + 26, y + 122), (x + 26 + lado, y + 110));
    cv.disc(x + 26 + lado, y + 109, 4, NEON);
    for k in 0..3 {
        cv.disc(x + 58 + k * 16, y + 120, 4, if (ms / 300) % 3 == k as u32 { BLANCO } else { NEON });
    }
    // La puerta de las monedas.
    cv.rect(x + 44, y + 146, 32, 34, 0x0008_1822);
    t(cv, (x + 44, y + 146), (x + 76, y + 146));
    t(cv, (x + 44, y + 180), (x + 76, y + 180));
    cv.rect(x + 52, y + 156, 2, 10, NEON);
    cv.rect(x + 66, y + 156, 2, 10, NEON);
    // El numero en el lateral.
    cv.text(x + w + 6, y + 60, b"0", NEON, 1);
    cv.text(x + w + 6, y + 78, b"1", NEON, 1);
}

/// **Un fotograma entero.**
pub fn pintar(cv: &mut Canvas, v: &Vista) {
    centro(cv, v);
    canales(cv, v);
    riel(cv, v);
    piezas(cv, v);
}
