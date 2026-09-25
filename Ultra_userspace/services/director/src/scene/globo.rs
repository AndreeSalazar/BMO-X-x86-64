//! **EL GLOBO DEL PUNTERO** -- un consejo o un dato que sigue al raton unos
//! segundos. Estilo (25-09, pedido: *"cartoon pero cyberpunk ... neon ...
//! texturas epicas"*): cristal oscuro con scanlines y trama, borde de neon que
//! late entre cian y magenta con su resplandor, esquinas cortadas, una placa
//! amarilla con el tema, y una colita hacia el puntero.
//!
//! [consumo] LATE      mientras vive (20 s) pide ~30 fotogramas por segundo
//!                     (`desktop::globo::anima`); muerto, nada (L6h)
//!
//! Aqui no se sabe QUE decir ni CUANDO: llega el texto y la EDAD del globo
//! (`desktop::globo` decide, esto solo pinta), por la misma regla que
//! `sugerir`. Toda la animacion sale de la edad:
//!
//! ```text
//!    0..350 ms     ENTRA: se abre de lado con rebote (pasa del 100 % y vuelve)
//!    vida          flota +-2 px, el neon late, un destello corre por el borde,
//!                  el texto se escribe con las dos ultimas letras en glitch
//!    ultimos 300   SALE: se cierra hacia la colita
//! ```
//!
//! Se pone como el cursor (`cursor::SaveUnder`): guarda lo que va a tapar al
//! FINAL del fotograma, justo antes de la capa del recorte y del puntero, y lo
//! devuelve al PRINCIPIO del siguiente. Y lo guardado es tambien el fondo del
//! cristal: el globo se MEZCLA con lo de debajo, no lo tapa.

use bmo_userland as bmo;

/// Letras que caben en la linea.
pub(crate) const LETRAS: usize = 64;
/// Letras por segundo de la maquina de escribir.
const LETRAS_POR_S: u64 = 40;

const PAD: u32 = 12;
const ANCHO_MAX: u32 = LETRAS as u32 * bmo::GLIFO_ANCHO + 2 * PAD;
const BARRA: u32 = 3;
/// Alto del cristal: aire arriba (la placa lo pisa), el texto, la barrita.
const ALTO: u32 = 14 + bmo::GLIFO_ALTO + 9 + BARRA + 8;
/// El resplandor, por fuera del borde.
const BRILLO: u32 = 4;
/// Lo que sube la placa y la colita por encima del cristal.
const ARRIBA: u32 = 16;
/// Lo que flota.
const FLOTA: u32 = 2;
/// Las esquinas cortadas (arriba a la derecha, abajo a la izquierda).
const CORTE: u32 = 9;

const MARCO_W: u32 = ANCHO_MAX + 2 * BRILLO + 8;
const MARCO_H: u32 = ALTO + ARRIBA + 2 * BRILLO + 2 * FLOTA;
const GUARDADO: usize = (MARCO_W * MARCO_H) as usize;

const ENTRA_MS: u64 = 350;
const SALE_MS: u64 = 300;

// La paleta: la de la ciudad de noche.
const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const AMARILLO: u32 = 0x00FC_EE0A;
const TINTA: u32 = 0x00EA_F6FF;
const TINTA_SOMBRA: u32 = 0x0012_5A73;
const NEGRO: u32 = 0x0008_0410;
const CRISTAL_ARRIBA: u32 = 0x001C_0A3C;
const CRISTAL_ABAJO: u32 = 0x0008_1632;

struct Globo {
    px: [u32; GUARDADO],
    caja: (u32, u32, u32, u32),
    puesto: bool,
}

static mut GLOBO: Globo = Globo { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn globo() -> &'static mut Globo {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(GLOBO) }
}

/// **Quita el globo**: devuelve lo que tapaba. Al PRINCIPIO del fotograma,
/// despues del cursor y de la capa del recorte. Si no estaba, no hace nada.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    let g = globo();
    if !g.puesto {
        return;
    }
    let (x, y, w, h) = g.caja;
    p.marcar(x, y, w, h);
    for dy in 0..h {
        for dx in 0..w {
            p.punto_ya_marcado(x + dx, y + dy, g.px[(dy * w + dx) as usize]);
        }
    }
    g.puesto = false;
}

/// Lo que se ve, de un fotograma.
pub(crate) struct Cara<'a> {
    /// En la placa amarilla: de QUE habla (`LA 3060`, `sabias`, `atajo`).
    pub titulo: &'a [u8],
    pub texto: &'a [u8],
    /// Cuanto lleva vivo y cuanto va a vivir, en ms.
    pub edad_ms: u64,
    pub vida_ms: u64,
}

/// `a` hacia `b` en `t` de 256.
fn mezcla(a: u32, b: u32, t: u32) -> u32 {
    let t = t.min(256);
    let c = |s: u32| (((a >> s) & 0xFF) * (256 - t) + ((b >> s) & 0xFF) * t) >> 8;
    c(16) << 16 | c(8) << 8 | c(0)
}

/// Una onda triangular de 0 a 256 con periodo `periodo` ms.
fn onda(ms: u64, periodo: u64) -> u32 {
    let f = (ms % periodo) * 512 / periodo;
    (if f < 256 { f } else { 512 - f }) as u32
}

/// Dentro del cristal, con las dos esquinas cortadas.
fn dentro(dx: i32, dy: i32, w: i32, h: i32) -> bool {
    let c = CORTE as i32;
    dx >= 0 && dy >= 0 && dx < w && dy < h && !(dy < c && dx > w - 1 - c + dy) && !(h - 1 - dy < c && dx < c - (h - 1 - dy))
}

/// **Pone el globo** junto al puntero `(ax, ay)`: abajo a la derecha, o donde
/// quepa. Al FINAL del fotograma, antes de la capa del recorte y del cursor.
pub(crate) fn poner(p: &bmo::Pantalla, ax: u32, ay: u32, c: &Cara) {
    let g = globo();
    if g.puesto || p.ancho < MARCO_W + 32 || p.alto < MARCO_H + 32 {
        return;
    }
    let ms = c.edad_ms;
    let queda = c.vida_ms.saturating_sub(ms);
    let n = (c.texto.len().min(LETRAS)) as u32;
    let lleno = (n.max(c.titulo.len() as u32 + 4) * bmo::GLIFO_ANCHO + 2 * PAD).min(ANCHO_MAX);
    // ** El ancho de ahora: ENTRA con rebote (al 20 %, pasa al 108 % y vuelve
    // al 100 %) y SALE cerrandose. Es lo que lo hace de dibujo animado.
    let permil = if ms < ENTRA_MS {
        let t = ms * 1000 / ENTRA_MS;
        if t < 700 { 200 + t * 880 / 700 } else { 1080 - (t - 700) * 80 / 300 }
    } else if queda < SALE_MS {
        queda * 1000 / SALE_MS
    } else {
        1000
    };
    let w = ((lleno as u64 * permil / 1000) as u32).max(2 * CORTE + 2);
    let h = ALTO;
    // Flota: arriba y abajo, suave.
    let flota = onda(ms, 1400) * 2 * FLOTA / 256;
    // Abajo a la derecha del puntero; si no cabe, al otro lado (y sin colita).
    let derecha = ax + 18 + lleno + BRILLO < p.ancho;
    let abajo = ay + 24 + ARRIBA + h + 2 * FLOTA + BRILLO < p.alto;
    let bx = if derecha { ax + 18 } else { ax.saturating_sub(lleno + 12) };
    let by = if abajo { ay + 24 + ARRIBA } else { ay.saturating_sub(h + 2 * FLOTA + 12) } + flota;
    let colita = derecha && abajo;

    // Lo que se guarda: el cristal, su resplandor y, encima, placa y colita.
    let x0 = bx.saturating_sub(BRILLO);
    let y0 = by.saturating_sub(ARRIBA + BRILLO);
    let ww = (w + 2 * BRILLO + 8).min(p.ancho - x0).min(MARCO_W);
    let hh = (h + ARRIBA + 2 * BRILLO).min(p.alto - y0).min(MARCO_H);
    g.caja = (x0, y0, ww, hh);
    p.sincronizar_lectura();
    for dy in 0..hh {
        for dx in 0..ww {
            g.px[(dy * ww + dx) as usize] = p.read(x0 + dx, y0 + dy);
        }
    }
    g.puesto = true;
    p.marcar(x0, y0, ww, hh);
    let debajo = |x: u32, y: u32| -> u32 {
        if x < x0 || y < y0 || x >= x0 + ww || y >= y0 + hh {
            return 0;
        }
        g.px[((y - y0) * ww + (x - x0)) as usize]
    };
    let punto = |x: u32, y: u32, color: u32| {
        if x >= x0 && y >= y0 && x < x0 + ww && y < y0 + hh {
            p.punto_ya_marcado(x, y, color);
        }
    };

    // El neon de ahora: late entre cian y magenta cada 2 s.
    let neon = mezcla(CIAN, MAGENTA, onda(ms, 2000));
    let (wi, hi) = (w as i32, h as i32);

    // 1. El RESPLANDOR: anillos por fuera, cada vez mas tenues.
    // Un pixel del anillo brilla si el punto del cristal mas cercano existe
    // (en las esquinas cortadas, no: el resplandor sigue el corte).
    let anillo = |dx: i32, dy: i32, alfa: u32| {
        let (x, y) = (bx as i32 + dx, by as i32 + dy);
        if x >= 0 && y >= 0 && dentro(dx.clamp(0, wi - 1), dy.clamp(0, hi - 1), wi, hi) {
            punto(x as u32, y as u32, mezcla(debajo(x as u32, y as u32), neon, alfa));
        }
    };
    for d in 1..=BRILLO as i32 {
        let alfa = [0, 150, 90, 45, 20][d as usize];
        for dx in -d..wi + d {
            anillo(dx, -d, alfa);
            anillo(dx, hi - 1 + d, alfa);
        }
        for dy in -d + 1..hi - 1 + d {
            anillo(-d, dy, alfa);
            anillo(wi - 1 + d, dy, alfa);
        }
    }

    // 2. El CRISTAL: degradado de arriba abajo, scanlines, una trama en
    // diagonal, y mezclado con lo de debajo (se ve a traves). El borde, neon.
    let barrido = (ms / 3) % (w as u64 + 60);
    for dy in 0..hi {
        let fila = mezcla(CRISTAL_ARRIBA, CRISTAL_ABAJO, (dy * 256 / hi) as u32);
        let fila = if dy % 2 == 1 { mezcla(fila, NEGRO, 60) } else { fila };
        for dx in 0..wi {
            if !dentro(dx, dy, wi, hi) {
                continue;
            }
            let borde = !dentro(dx - 1, dy, wi, hi)
                || !dentro(dx + 1, dy, wi, hi)
                || !dentro(dx, dy - 1, wi, hi)
                || !dentro(dx, dy + 1, wi, hi);
            let (x, y) = (bx + dx as u32, by + dy as u32);
            let color = if borde {
                // Y el DESTELLO: un tramo blanco que corre por el borde de arriba.
                let cerca = dy <= 1 && (dx as u64 + 60).abs_diff(barrido + 30) < 30;
                if cerca { mezcla(neon, TINTA, 200) } else { neon }
            } else {
                let base = if (dx + dy) % 9 == 0 { mezcla(fila, neon, 26) } else { fila };
                mezcla(debajo(x, y), base, 224)
            };
            punto(x, y, color);
        }
    }

    // 3. La COLITA hacia el puntero: una cuna de cristal con orillas de neon,
    // de base sobre el borde de arriba y punta arriba a la izquierda.
    if colita && w > 3 * CORTE {
        let alto = ARRIBA - 1;
        for k in 0..alto {
            let izq = bx + 6 - 8 * k / alto;
            let der = bx + 34 - 35 * k / alto;
            let y = by - 1 - k;
            let relleno = mezcla(CRISTAL_ARRIBA, neon, 40);
            for x in izq..=der.max(izq) {
                let orilla = x <= izq + 1 || x + 1 >= der;
                punto(x, y, if orilla { neon } else { mezcla(debajo(x, y), relleno, 230) });
            }
        }
    }

    // Lo de dentro, solo con el globo abierto del todo.
    if permil < 900 {
        return;
    }

    // 4. La PLACA amarilla con el tema, pisando el borde de arriba.
    let tx = bx + if colita { 40 } else { 14 };
    let tw = c.titulo.len() as u32 * bmo::GLIFO_ANCHO + 12;
    if tx + tw + 4 < bx + w {
        p.rect(tx + 3, by - 9 + 3, tw, bmo::GLIFO_ALTO + 2, MAGENTA);
        p.rect(tx, by - 9, tw, bmo::GLIFO_ALTO + 2, AMARILLO);
        p.texto_bytes(tx + 6, by - 8, c.titulo, NEGRO);
    }

    // 5. El TEXTO, letra a letra, con sombra de neon; las dos ultimas letras
    // recien escritas salen en glitch (magenta, corridas un pixel).
    let ty = by + 14;
    let cabe = ((w - 2 * PAD) / bmo::GLIFO_ANCHO) as usize;
    let escritas = ((ms.saturating_sub(ENTRA_MS)) * LETRAS_POR_S / 1000) as usize;
    let hasta = escritas.min(c.texto.len()).min(cabe);
    let apagandose = queda < 3000;
    let tinta = if apagandose { mezcla(TINTA, CRISTAL_ABAJO, 128) } else { TINTA };
    // Mientras se escribe; escrito del todo, ya no hay glitch.
    let firmes = if hasta < c.texto.len().min(cabe) { hasta.saturating_sub(2) } else { hasta };
    p.texto_bytes(bx + PAD + 1, ty + 1, &c.texto[..firmes], TINTA_SOMBRA);
    let cx = p.texto_bytes(bx + PAD, ty, &c.texto[..firmes], tinta);
    if hasta > firmes {
        p.texto_bytes(cx - 1, ty, &c.texto[firmes..hasta], CIAN);
        p.texto_bytes(cx + 1, ty, &c.texto[firmes..hasta], MAGENTA);
    }
    if hasta < c.texto.len().min(cabe) && (ms / 200) % 2 == 0 {
        // El cursor de la maquina de escribir, parpadeando.
        let k = cx + (hasta - firmes) as u32 * bmo::GLIFO_ANCHO;
        p.rect(k, ty + 2, 2, bmo::GLIFO_ALTO - 4, AMARILLO);
    }

    // 6. La BARRITA: lo que le queda, en degradado de cian a magenta, con la
    // punta encendida.
    let bary = by + h - 8 - BARRA;
    let largo = w - 2 * PAD;
    let lleno_b = (largo as u64 * queda / c.vida_ms.max(1)) as u32;
    p.rect(bx + PAD, bary, largo, BARRA, mezcla(CRISTAL_ABAJO, NEGRO, 128));
    let tramos = 16u32;
    for t in 0..tramos {
        let (a, b) = (lleno_b * t / tramos, lleno_b * (t + 1) / tramos);
        if b > a {
            p.rect(bx + PAD + a, bary, b - a, BARRA, mezcla(CIAN, MAGENTA, t * 256 / tramos));
        }
    }
    if lleno_b > 2 {
        p.rect(bx + PAD + lleno_b - 2, bary - 1, 3, BARRA + 2, TINTA);
    }
}
