//! **EL ARRANQUE ORQUESTADO** (2026-09-25) -- la pantalla del arranque
//! mientras `save mode` ARMADO prepara la RAM y despierta la 3060. Pedido del
//! propietario: *"el splash eso es muy especial porque ahi es mi GPU TIENE QUE
//! TOMAR TODO el control ... mientras la CPU esta preparando"*.
//!
//! ```text
//!    +--------------------------------------------------------------+
//!    |  BMO-X        ARRANQUE ORQUESTADO              PASO 23/52 44% |
//!    |  [#####|#####|#####|####|....|....|....|....|....|....|....]  |
//!    |  > vram                          // RELOJ DE LA CPU (TSC)     |
//!    |    que hace el paso, en tres lineas   0000_1A2B_3C4D  gsp     |
//!    |  // BITACORA                        ...                       |
//!    |  [ OK ] iommu      12 ms                                      |
//!    |  CPU: preparando la RAM y la 3060                  T+ 4.2 s  |
//!    +--------------------------------------------------------------+
//! ```
//!
//! [consumo] NADA      pinta una vez antes de cada paso del arranque, y
//!                     solo si `save mode` esta armado (L6h)
//!
//! Aqui no se sabe QUE paso es ni CUANDO: llega una [`Vista`] con todo
//! (`desktop::arranque` decide, esto solo pinta).

use bmo_userland as bmo;

use super::gato;
use super::globo::{mezcla, onda};

/// Como acabo (o va) cada paso, en la barra y en la bitacora.
pub(crate) const PENDIENTE: u8 = 0;
pub(crate) const BIEN: u8 = 1;
pub(crate) const YA: u8 = 2;
pub(crate) const SALTO: u8 = 3;
pub(crate) const MAL: u8 = 4;
pub(crate) const AHORA: u8 = 5;

pub(crate) const CIAN: u32 = 0x0000_F0FF;
pub(crate) const MAGENTA: u32 = 0x00FF_2BD6;
pub(crate) const VERDE: u32 = 0x0039_FF88;
const BLANCO: u32 = 0x00FF_FFFF;
const CLARO: u32 = 0x00C8_D6E5;
const TENUE: u32 = 0x0059_6B8A;
const ROJO: u32 = 0x00FF_3B5C;
pub(crate) const AMBAR: u32 = 0x00FF_C23D;
const FONDO: u32 = 0x0004_060B;
const REJILLA: u32 = 0x000A_1820;
/// La rejilla cuando la 3060 esta despierta: la pantalla BRILLA.
const REJILLA_VIVA: u32 = 0x0010_3A4C;
const PANEL: u32 = 0x0007_0B12;
const RAYA: u32 = 0x0005_080E;
const APAGADO: u32 = 0x0012_1C26;

/// Una linea de la bitacora: el paso, como salio, cuanto tardo y el reloj de
/// la CPU (TSC) al acabar.
#[derive(Clone, Copy)]
pub(crate) struct Hecho {
    pub(crate) nombre: &'static [u8],
    pub(crate) estado: u8,
    pub(crate) us: u64,
    pub(crate) tsc: u64,
}

/// Todo lo que se pinta en un instante.
pub(crate) struct Vista<'a> {
    /// Un estado por paso, en orden.
    pub(crate) estados: &'a [u8],
    /// El paso que va a correr (o lo que pasa ahora), y que hace.
    pub(crate) nombre: &'a [u8],
    pub(crate) que: &'a [u8],
    /// Lo ultimo que acabo, lo mas nuevo al final.
    pub(crate) hechos: &'a [Hecho],
    /// Desde que empezo el arranque orquestado.
    pub(crate) ms: u64,
    /// La linea de abajo: quien lleva la maquina.
    pub(crate) etapa: &'a [u8],
    pub(crate) acento: u32,
    /// Cuanto brilla el gato, sobre 256: 0 duerme (la 3060 no desperto).
    pub(crate) gato: u32,
}

fn azar(s: u64) -> u64 {
    let mut x = s.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ x >> 32
}

/// Los cuatro angulos de un HUD alrededor de `(x, y, w, h)`.
fn esquinas(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), largo: u32, color: u32) {
    let (x1, y1) = (x + w, y + h);
    for &(cx, cy, dx, dy) in &[(x, y, 0u32, 0u32), (x1 - largo, y, largo - 2, 0), (x, y1 - largo, 0, largo - 2), (x1 - largo, y1 - largo, largo - 2, largo - 2)] {
        p.rect(cx, cy + dy, largo, 2, color);
        p.rect(cx + dx, cy, 2, largo, color);
    }
}

/// **El fondo**: negro azulado, una rejilla y los angulos, a pantalla
/// completa. `luz` sobre 256: mientras la 3060 duerme se va APAGANDO paso a
/// paso (de 110 hacia el negro); cuando despierta, 256: la rejilla en neon.
pub(crate) fn fondo(p: &bmo::Pantalla, luz: u32) {
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    let rejilla = if luz >= 256 { REJILLA_VIVA } else { mezcla(FONDO, REJILLA, luz * 2) };
    for x in (0..p.ancho).step_by(48) {
        p.rect(x, 0, 1, p.alto, rejilla);
    }
    for y in (0..p.alto).step_by(48) {
        p.rect(0, y, p.ancho, 1, rejilla);
    }
    let angulos = if luz >= 256 { CIAN } else { mezcla(FONDO, TENUE, luz * 2) };
    esquinas(p, (12, 12, p.ancho.saturating_sub(24), p.alto.saturating_sub(24)), 48, angulos);
    p.texto(28, 24, "BMO-X // RING 3 // EL DIRECTOR ORQUESTA EL ARRANQUE", angulos);
}

/// **El gato de la intro** (`scene::gato`), en `(x, y)`: a `escala` enteros,
/// o a la MITAD si `escala` es 0 (un pixel si alguno de los cuatro lo es: asi
/// no se pierde un trazo de un pixel). `luz` sobre 256: 0 duerme en gris; con
/// luz, el trazo en neon y un HALO alrededor que crece con ella, mezclado con
/// `fondo`. Los ojos, en `ojos`.
pub(crate) fn pintar_gato(p: &bmo::Pantalla, x: u32, y: u32, escala: u32, luz: u32, ojos: u32, fondo: u32) {
    let bit = |m: &[u8], fx: u32, fy: u32| {
        let i = (fy * gato::WIDTH + fx) as usize;
        m[i / 8] >> (i % 8) & 1 == 1
    };
    let (paso, lado) = if escala == 0 { (2, 1) } else { (1, escala) };
    // Cada pixel del gato: (px, py, es ojo).
    let cada = |f: &mut dyn FnMut(u32, u32, bool)| {
        for fy in (0..gato::HEIGHT).step_by(paso as usize) {
            for fx in (0..gato::WIDTH).step_by(paso as usize) {
                let mut trazo = false;
                let mut ojo = false;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().take(if paso == 2 { 4 } else { 1 }) {
                    if fx + dx < gato::WIDTH && fy + dy < gato::HEIGHT {
                        ojo |= bit(&gato::EYES, fx + dx, fy + dy);
                        trazo |= bit(&gato::STROKE, fx + dx, fy + dy);
                    }
                }
                if ojo || trazo {
                    f(x + fx / paso * lado, y + fy / paso * lado, ojo);
                }
            }
        }
    };
    let neon = mezcla(CIAN, MAGENTA, 70);
    if luz == 0 {
        cada(&mut |px, py, ojo| p.rect(px, py, lado, lado, if ojo { 0x0023_4A5A } else { 0x0030_3A48 }));
        return;
    }
    // El halo: de fuera adentro, cada capa mas cerca y mas viva.
    for (r, a) in [(3 * lado + 2, 48u32), (2 * lado + 1, 110), (lado, 190)] {
        let c = mezcla(fondo, neon, a * luz / 256);
        cada(&mut |px, py, _| p.rect(px.saturating_sub(r), py.saturating_sub(r), lado + 2 * r, lado + 2 * r, c));
    }
    // El trazo: neon, y blanco del todo en el golpe (luz 256).
    let trazo = if luz >= 256 { BLANCO } else { mezcla(neon, BLANCO, luz / 2) };
    cada(&mut |px, py, ojo| p.rect(px, py, lado, lado, if ojo { ojos } else { trazo }));
}

/// **LA 3060 DESPERTO**: un fotograma de la animacion (`t` de 0 a 1000) --
/// el gato grande en el centro, que se enciende de golpe en blanco y se queda
/// en neon, una raya que barre de arriba abajo y el titulo que se escribe
/// solo. Pinta solo su caja: el fondo negro lo pone quien la llama.
pub(crate) fn despertar(p: &bmo::Pantalla, t: u32) {
    const ESCALA: u32 = 2;
    let (gw, gh) = (gato::WIDTH * ESCALA, gato::HEIGHT * ESCALA);
    let w = gw + 400;
    let h = gh + 150;
    let x0 = p.ancho.saturating_sub(w) / 2;
    let y0 = p.alto.saturating_sub(h) / 2;
    p.rect(x0, y0, w, h, FONDO);
    // El golpe: sube a todo en 120 ms (blanco), y se asienta en el neon.
    let luz = if t < 120 { 256 } else { 256 - (t - 120).min(500) * 90 / 500 };
    let ojos = if t < 250 { BLANCO } else { mezcla(AMBAR, CIAN, ((t - 250).min(400) * 256 / 400) as u32) };
    let gx = x0 + (w - gw) / 2;
    let gy = y0 + 10;
    pintar_gato(p, gx, gy, ESCALA, luz, ojos, FONDO);
    // La onda del golpe: un marco que sale del gato y se apaga al llegar al
    // borde de la caja (que se limpia en cada fotograma: no deja rastro).
    if t < 600 {
        let (cx, cy) = (gx + gw / 2, gy + gh / 2);
        let (rw, rh) = (w / 2 * t / 600, h / 2 * t / 600);
        let c = mezcla(FONDO, if t < 300 { BLANCO } else { MAGENTA }, 256 - t * 256 / 600);
        let (ax, ay) = (cx.saturating_sub(rw).max(x0), cy.saturating_sub(rh).max(y0));
        let (bw, bh) = ((cx + rw).min(x0 + w - 2) - ax, (cy + rh).min(y0 + h - 2) - ay);
        p.rect(ax, ay, bw, 2, c);
        p.rect(ax, ay + bh, bw + 2, 2, c);
        p.rect(ax, ay, 2, bh, c);
        p.rect(ax + bw, ay, 2, bh, c);
    }
    // La raya que barre al gato, como un escaner.
    let ry = gy + (gh * (t % 500) / 500);
    p.rect(x0 + 60, ry, w - 120, 2, mezcla(FONDO, CIAN, 200));
    // El titulo, letra a letra, con su sombra magenta.
    const TITULO: &str = "LA 3060 DESPERTO";
    let n = (TITULO.len() as u32 * t.min(600) / 600) as usize;
    let tw = bmo::Pantalla::ancho_escala(TITULO, 3);
    let tx = x0 + w.saturating_sub(tw) / 2;
    let ty = gy + gh + 30;
    p.texto_escala(tx + 3, ty + 2, &TITULO[..n], MAGENTA, 3);
    p.texto_escala(tx, ty, &TITULO[..n], if t < 200 { BLANCO } else { CIAN }, 3);
    if t > 500 {
        let s = "el GSP responde: desde aqui la 3060 trabaja";
        p.texto(x0 + w.saturating_sub(bmo::Pantalla::ancho_escala(s, 1)) / 2, ty + 60, s, CLARO);
    }
}

/// Donde va el panel: centrado, hasta 1040x620.
pub(crate) fn caja(p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    let w = p.ancho.saturating_sub(32).min(1040);
    let h = p.alto.saturating_sub(32).min(620);
    ((p.ancho - w) / 2, (p.alto - h) / 2, w, h)
}

/// El marco del panel: 2 px con el degradado cian -> magenta corriendo.
fn borde(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), ms: u64) {
    const TROZOS: u32 = 40;
    let giro = ms / 8;
    for k in 0..TROZOS {
        let c = |d: u64| mezcla(CIAN, MAGENTA, onda(d * 1000 / TROZOS as u64 + giro, 1000));
        let (a, b) = (w * k / TROZOS, w * (k + 1) / TROZOS);
        p.rect(x + a, y, b - a, 2, c(k as u64));
        p.rect(x + a, y + h - 2, b - a, 2, c((2 * TROZOS - k) as u64));
        let (a, b) = (h * k / TROZOS, h * (k + 1) / TROZOS);
        p.rect(x, y + a, 2, b - a, c((3 * TROZOS - k) as u64));
        p.rect(x + w - 2, y + a, 2, b - a, c((TROZOS + k) as u64));
    }
}

/// **El logo que tiembla**: tres copias corridas (magenta, cian,
/// blanco) que tiemblan distinto en cada pintada, y dos cortes de neon.
fn logo(p: &bmo::Pantalla, x: u32, y: u32, semilla: u64) {
    const ESCALA: u32 = 6;
    let r = azar(semilla);
    let j = (r % 4) as u32 + 1;
    p.texto_escala(x.saturating_sub(j + 1), y, "BMO-X", MAGENTA, ESCALA);
    p.texto_escala(x + j + 1, y + 1, "BMO-X", CIAN, ESCALA);
    p.texto_escala(x, y, "BMO-X", BLANCO, ESCALA);
    let ancho = bmo::Pantalla::ancho_escala("BMO-X", ESCALA);
    for k in 0..2u64 {
        let q = azar(semilla ^ (k + 1) * 0x51);
        let cy = y + (q % (16 * ESCALA as u64 - 4)) as u32;
        let cx = x + (q >> 8) as u32 % (ancho / 2);
        let cw = 40 + (q >> 16) as u32 % (ancho / 2);
        p.rect(cx, cy, cw, 2 + (q >> 24) as u32 % 3, if k == 0 { CIAN } else { MAGENTA });
    }
}

/// `v` en hexadecimal, `cifras` cifras, con `_` cada cuatro.
fn hex(v: u64, cifras: u32, b: &mut [u8; 24]) -> usize {
    let mut n = 0;
    for k in (0..cifras).rev() {
        b[n] = b"0123456789ABCDEF"[(v >> (4 * k) & 0xF) as usize];
        n += 1;
        if k > 0 && k % 4 == 0 {
            b[n] = b'_';
            n += 1;
        }
    }
    n
}

/// Un texto corto que se arma por trozos.
pub(crate) struct Tira {
    b: [u8; 160],
    n: usize,
}

impl Tira {
    pub(crate) fn nueva() -> Self {
        Tira { b: [0; 160], n: 0 }
    }

    pub(crate) fn t(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }

    pub(crate) fn d(&mut self, v: u64) -> &mut Self {
        let mut dig = [0u8; 20];
        let (mut v, mut k) = (v, dig.len());
        loop {
            k -= 1;
            dig[k] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.t(&dig[k..])
    }

    pub(crate) fn s(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// Un texto en lineas de `cols` letras, cortando en espacios; hasta `max`.
fn envolver(p: &bmo::Pantalla, x: u32, y: u32, texto: &[u8], cols: usize, max: u32, color: u32) {
    let mut resto = texto;
    for fila in 0..max {
        while resto.first() == Some(&b' ') {
            resto = &resto[1..];
        }
        if resto.is_empty() || cols == 0 {
            return;
        }
        let mut corte = resto.len().min(cols);
        if corte < resto.len() {
            if let Some(k) = resto[..corte].iter().rposition(|&c| c == b' ') {
                if k > 0 {
                    corte = k;
                }
            }
        }
        p.texto_bytes(x, y + fila * 18, &resto[..corte], color);
        resto = &resto[corte..];
    }
}

fn color_de(estado: u8, i: usize, total: usize, ms: u64) -> u32 {
    let neon = mezcla(CIAN, MAGENTA, (i * 256 / total.max(1)) as u32);
    match estado {
        BIEN => neon,
        YA => mezcla(neon, PANEL, 110),
        SALTO => 0x0024_3340,
        MAL => ROJO,
        AHORA => {
            if ms / 250 % 2 == 0 {
                BLANCO
            } else {
                AMBAR
            }
        }
        _ => APAGADO,
    }
}

/// **El panel**: todo lo que cambia de un paso a otro. Solo se vuelca su
/// caja; el fondo (de la CPU, o de la 3060) queda como estaba.
pub(crate) fn panel(p: &bmo::Pantalla, v: &Vista) {
    let c @ (x, y, w, h) = caja(p);
    p.rect(x, y, w, h, PANEL);
    for yy in (y..y + h).step_by(3) {
        p.rect(x, yy, w, 1, RAYA);
    }
    borde(p, c, v.ms);
    esquinas(p, (x + 8, y + 8, w - 16, h - 16), 24, v.acento);
    let mut b = [0u8; 24];

    // La cabecera: el logo, que es esto y por donde va.
    // El gato de la intro, a la mitad: duerme en gris hasta que la 3060
    // despierta, y entonces brilla. El que dice "funciona".
    let ojos = if v.gato == 0 { 0 } else { v.acento };
    pintar_gato(p, x + 28, y + 26, 0, v.gato, ojos, PANEL);
    let lx = x + 28 + gato::WIDTH / 2 + 20;
    logo(p, lx, y + 28, v.ms / 70 ^ v.estados.len() as u64);
    let tx = lx + bmo::Pantalla::ancho_escala("BMO-X", 6) + 28;
    p.texto_escala(tx, y + 34, "ARRANQUE ORQUESTADO", v.acento, 2);
    p.texto(tx, y + 72, "la CPU prepara la RAM y despierta la 3060;", CLARO);
    p.texto(tx, y + 92, "en cuanto la 3060 puede, toma la pantalla", CLARO);
    let total = v.estados.len();
    let hechos = v.estados.iter().filter(|&&e| e != PENDIENTE && e != AHORA).count();
    let pct = hechos * 100 / total.max(1);
    let mut l = Tira::nueva();
    l.t(b"PASO ").d(hechos as u64).t(b"/").d(total as u64).t(b"  ").d(pct as u64).t(b"%");
    let ancho_pct = l.n as u32 * 16;
    p.texto_escala(x + w - 32 - ancho_pct, y + 34, core::str::from_utf8(l.s()).unwrap_or(""), BLANCO, 2);

    // La barra: un trozo por paso.
    let (bx, by, bw) = (x + 32, y + 140, w - 64);
    if total > 0 {
        for (i, &e) in v.estados.iter().enumerate() {
            let a = bw * i as u32 / total as u32;
            let z = bw * (i as u32 + 1) / total as u32;
            p.rect(bx + a, by, (z - a).saturating_sub(2).max(1), 22, color_de(e, i, total, v.ms));
        }
        let lleno = bw * hechos as u32 / total as u32;
        for k in 0..16u32 {
            let (a, z) = (lleno * k / 16, lleno * (k + 1) / 16);
            p.rect(bx + a, by + 28, z - a, 2, mezcla(CIAN, MAGENTA, k * 16));
        }
    }

    // El paso de ahora: su nombre en grande, un cursor que late y que hace.
    // Lo que cabe a la izquierda de la columna del reloj.
    let izq = (w * 3 / 5).saturating_sub(32 + 32 + 40);
    let ny = y + 190;
    let fin = p.texto_escala(bx, ny, "> ", v.acento, 2);
    let fin = p.texto_escala(fin, ny, core::str::from_utf8(v.nombre).unwrap_or("?"), BLANCO, 2);
    if v.ms / 400 % 2 == 0 {
        p.rect(fin + 4, ny + 2, 14, 28, v.acento);
    }
    envolver(p, bx + 32, ny + 44, v.que, (izq / 8) as usize, 3, TENUE);

    // La bitacora: lo ultimo que acabo.
    let ly = y + 316;
    p.texto(bx, ly, "// BITACORA", v.acento);
    let filas = ((h.saturating_sub(316 + 80)) / 18).min(12) as usize;
    let desde = v.hechos.len().saturating_sub(filas);
    for (k, hc) in v.hechos[desde..].iter().enumerate() {
        let fy = ly + 24 + k as u32 * 18;
        let (marca, color): (&[u8], u32) = match hc.estado {
            BIEN => (b"[ OK ]", VERDE),
            YA => (b"[ YA ]", CIAN),
            MAL => (b"[ NO ]", ROJO),
            _ => (b"[ -- ]", TENUE),
        };
        p.texto_bytes(bx, fy, marca, color);
        p.texto_bytes(bx + 64, fy, hc.nombre, if hc.estado == MAL { ROJO } else { CLARO });
        if hc.estado == BIEN || hc.estado == MAL {
            let mut t = Tira::nueva();
            if hc.us >= 10_000 {
                t.d(hc.us / 1000).t(b" ms");
            } else {
                t.d(hc.us).t(b" us");
            }
            p.texto_bytes(bx + 220, fy, t.s(), TENUE);
        }
    }

    // A la derecha, el reloj de la CPU al acabar cada paso: el latido real.
    let rx = x + w * 3 / 5 + 8;
    p.rect(rx - 16, y + 190, 1, h.saturating_sub(190 + 70), REJILLA);
    p.texto(rx, y + 190, "// RELOJ DE LA CPU (TSC)", v.acento);
    let ahora = bmo::ciclos();
    let k = hex(ahora, 16, &mut b);
    p.texto_bytes(rx, y + 214, &b[..k], BLANCO);
    p.texto(rx + 8 * 20, y + 214, "ahora", AMBAR);
    let filas = ((h.saturating_sub(214 + 90)) / 18).min(16) as usize;
    let desde = v.hechos.len().saturating_sub(filas);
    for (k, hc) in v.hechos[desde..].iter().rev().enumerate() {
        let fy = y + 238 + k as u32 * 18;
        let n = hex(hc.tsc, 16, &mut b);
        let color = mezcla(VERDE, PANEL, (k as u32 * 14).min(200));
        p.texto_bytes(rx, fy, &b[..n], color);
        p.texto_bytes(rx + 8 * 20, fy, hc.nombre, mezcla(TENUE, PANEL, (k as u32 * 10).min(180)));
    }

    // Abajo: quien lleva la maquina, y el tiempo.
    let fy = y + h - 40;
    p.rect(x + 24, fy - 10, w - 48, 1, REJILLA);
    p.texto_bytes(bx, fy, v.etapa, v.acento);
    let mut t = Tira::nueva();
    t.t(b"T+ ").d(v.ms / 1000).t(b".").d(v.ms / 100 % 10).t(b" s");
    p.texto_bytes(x + w - 32 - t.n as u32 * 8, fy, t.s(), TENUE);
}
