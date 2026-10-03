//! **LA PASTILLA** -- la musica de fondo, escondida arriba en el centro.
//!
//! [consumo] LATE      mientras suena el fondo pide fotograma: ~10 por
//!                     segundo escondida (solo la rayita que late) y ~30
//!                     asomada o abierta (`desktop::pastilla::anima`), y lee
//!                     el medidor del maestro una vez por fotograma. Sin
//!                     fondo, nada (L6h)
//!
//! El propietario (03-10): *"una notificacion escondida en la pantalla con
//! animacion en tiempo real para que puedan manipular sin interrumpir, pausa
//! o reproducir y control del audio [...] y recomendacion"*. La maqueta de
//! HERMES la tiene; esta es la de verdad.
//!
//! ```text
//!    ESCONDIDA   una rayita de neon arriba, que LATE con lo que sale por
//!                el cable (el medidor del maestro, no un dibujo)
//!    ASOMA       al acercar el raton, o al cambiar de pieza: baja con
//!                rebote y dice que suena, con pausa y siguiente
//!    ABIERTA     con un clic: la onda en vivo, los medidores izquierdo y
//!                derecho, pausa, siguiente, volumen, y una recomendacion
//! ```
//!
//! Se pone como el globo: guarda lo que tapa al FINAL del fotograma y lo
//! devuelve al PRINCIPIO del siguiente. Aqui no se decide nada: llega la
//! [`Cara`] y se pinta; lo que se puede pulsar se devuelve en [`Sitios`].

use bmo_userland as bmo;

use crate::scene::globo::{mezcla, onda};

/// Las tres medidas: escondida, asomada, abierta.
const ESC: (u32, u32) = (180, 6);
const ASOMA: (u32, u32) = (320, 36);
const ABIERTA: (u32, u32) = (420, 156);
/// El resplandor, por fuera.
const BRILLO: u32 = 3;
const GUARDADO: usize = ((ABIERTA.0 + 2 * BRILLO) * (ABIERTA.1 + 2 * BRILLO)) as usize;
/// Lo que tarda en bajar o subir.
const CAMBIA_MS: u64 = 220;

// La paleta: el verde lima de la ONDA sobre el cristal de la ciudad.
const LIMA: u32 = 0x00B6_FF5C;
const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const TINTA: u32 = 0x00EA_F6FF;
const TENUE: u32 = 0x007E_8CA0;
const NEGRO: u32 = 0x0005_0710;
const CRISTAL_ARRIBA: u32 = 0x000C_1420;
const CRISTAL_ABAJO: u32 = 0x0005_0A10;
const ROJO: u32 = 0x00FF_3355;

/// Como esta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Modo {
    Escondida,
    Asoma,
    Abierta,
}

/// Lo que se ve, de un fotograma.
pub(crate) struct Cara<'a> {
    pub modo: Modo,
    /// Cuanto lleva en este modo, y cuanto lleva viva: las dos animan.
    pub en_modo_ms: u64,
    pub ms: u64,
    pub titulo: &'a [u8],
    pub pulsos: u32,
    pub volumen: u32,
    pub pausada: bool,
    pub recomendada: &'a [u8],
    /// Lo que mide el maestro: pico y fuerza, izquierdo y derecho, en dBFS
    /// (1/256). Es lo que de verdad sale por el cable.
    pub pico: [i32; 2],
    pub rms: [i32; 2],
}

/// Una caja que se puede pulsar: x, y, ancho, alto (ancho 0 = no esta).
pub(crate) type Caja = (u32, u32, u32, u32);

/// **Lo que se puede pulsar**, tal como quedo pintado.
#[derive(Clone, Copy, Default)]
pub(crate) struct Sitios {
    pub todo: Caja,
    pub pausa: Caja,
    pub siguiente: Caja,
    pub menos: Caja,
    pub mas: Caja,
    pub recomendada: Caja,
    pub cerrar: Caja,
}

/// Dentro de una caja?
pub(crate) fn en(c: Caja, x: u32, y: u32) -> bool {
    c.2 > 0 && x >= c.0 && y >= c.1 && x < c.0 + c.2 && y < c.1 + c.3
}

struct Guardado {
    px: [u32; GUARDADO],
    caja: Caja,
    puesto: bool,
}

static mut GUARDADO_: Guardado = Guardado { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn guardado() -> &'static mut Guardado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(GUARDADO_) }
}

/// **Quita la pastilla**: devuelve lo que tapaba. Al PRINCIPIO del fotograma.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    let g = guardado();
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

/// De dBFS (1/256) a 0..=256, con -60 dB como suelo.
fn nivel(db: i32) -> u32 {
    ((db + 60 * 256).clamp(0, 60 * 256) as u32 * 256) / (60 * 256)
}

/// Las medidas de un modo.
fn medida(m: Modo) -> (u32, u32) {
    match m {
        Modo::Escondida => ESC,
        Modo::Asoma => ASOMA,
        Modo::Abierta => ABIERTA,
    }
}

/// **Pone la pastilla** arriba en el centro. Devuelve lo que se puede pulsar.
pub(crate) fn poner(p: &bmo::Pantalla, c: &Cara) -> Sitios {
    let g = guardado();
    let mut s = Sitios::default();
    if g.puesto || p.ancho < ABIERTA.0 + 40 || p.alto < ABIERTA.1 + 40 {
        return s;
    }
    // ** El GESTO: baja con rebote (pasa del 100 % y vuelve), como entra el
    // globo pero hacia abajo, que es por donde sale una pastilla.
    let (w_fin, h_fin) = medida(c.modo);
    let t = (c.en_modo_ms * 1000 / CAMBIA_MS).min(1000);
    let permil = if t < 700 { 300 + t * 800 / 700 } else { 1100 - (t - 700) * 100 / 300 };
    let w = (w_fin as u64 * permil.min(1000) / 1000).max(ESC.0 as u64) as u32;
    let h = ((h_fin as u64 * permil / 1000) as u32).max(ESC.1).min(ABIERTA.1);
    let x = (p.ancho - w) / 2;
    let y = 0;

    // Lo que se guarda: la pastilla y su resplandor.
    let x0 = x.saturating_sub(BRILLO);
    let (ww, hh) = ((w + 2 * BRILLO).min(p.ancho - x0), (h + BRILLO).min(p.alto));
    g.caja = (x0, y, ww, hh);
    p.sincronizar_lectura();
    for dy in 0..hh {
        for dx in 0..ww {
            g.px[(dy * ww + dx) as usize] = p.read(x0 + dx, y + dy);
        }
    }
    g.puesto = true;
    p.marcar(x0, y, ww, hh);
    let debajo = |px: u32, py: u32| -> u32 {
        if px < x0 || py < y || px >= x0 + ww || py >= y + hh {
            return 0;
        }
        g.px[((py - y) * ww + (px - x0)) as usize]
    };
    let punto = |px: u32, py: u32, color: u32| {
        if px >= x0 && py >= y && px < x0 + ww && py < y + hh {
            p.punto_ya_marcado(px, py, color);
        }
    };

    // Lo que LATE: la fuerza de lo que sale, de los dos lados.
    let fuerza = nivel(c.rms[0].max(c.rms[1]));
    let pico = nivel(c.pico[0].max(c.pico[1]));
    let neon = if c.pausada { TENUE } else { mezcla(mezcla(LIMA, CIAN, onda(c.ms, 3000) / 3), TINTA, pico / 3) };

    // 1. El RESPLANDOR por los lados y por abajo, que crece con el pico.
    for d in 1..=BRILLO {
        let alfa = ([0, 140, 70, 30][d as usize] * (96 + pico) / 352) as u32;
        for dy in 0..h + d {
            punto(x - d, dy, mezcla(debajo(x - d, dy), neon, alfa));
            punto(x + w - 1 + d, dy, mezcla(debajo(x + w - 1 + d, dy), neon, alfa));
        }
        for dx in 0..w {
            punto(x + dx, h - 1 + d, mezcla(debajo(x + dx, h - 1 + d), neon, alfa));
        }
    }

    // 2. El CRISTAL: degradado, scanlines, mezclado con lo de debajo; las dos
    // esquinas de abajo cortadas, y el borde de neon.
    let corte = (h / 3).min(10);
    let fuera = |dx: u32, dy: u32| {
        let desde_abajo = h - 1 - dy;
        desde_abajo < corte && (dx < corte - desde_abajo || w - 1 - dx < corte - desde_abajo)
    };
    for dy in 0..h {
        let fila = mezcla(CRISTAL_ARRIBA, CRISTAL_ABAJO, dy * 256 / h.max(1));
        let fila = if dy % 2 == 1 { mezcla(fila, NEGRO, 50) } else { fila };
        for dx in 0..w {
            if fuera(dx, dy) {
                continue;
            }
            let borde = dx == 0 || dx == w - 1 || dy == h - 1 || fuera(dx.saturating_sub(1), dy) || fuera(dx + 1, dy) || fuera(dx, dy + 1);
            let color = if borde { neon } else { mezcla(debajo(x + dx, dy), fila, 230) };
            punto(x + dx, dy, color);
        }
    }
    s.todo = (x, y, w, h);
    // Lo de dentro, solo cuando ya mide lo suyo: antes no cabria en lo guardado.
    if c.modo == Modo::Escondida || permil < 1000 {
        // Escondida: la rayita sola, con un brillo que corre al compas.
        if h >= 3 {
            let corre = (c.ms / 6) % (w as u64 + 40);
            for dx in 0..w {
                if (dx as u64 + 40).abs_diff(corre + 20) < 20 {
                    punto(x + dx, h - 2, mezcla(neon, TINTA, 120 + fuerza / 2));
                }
            }
        }
        return s;
    }

    let gw = bmo::GLIFO_ANCHO;
    if c.modo == Modo::Asoma {
        // 3a. ASOMA: el ecualizador chico, lo que suena, pausa y siguiente.
        barras(p, x + 12, 8, 5, 18, c, neon);
        let n = (((w - 12 - 40 - 64) / gw) as usize).min(c.titulo.len());
        p.texto_bytes(x + 46, 10, &c.titulo[..n], if c.pausada { TENUE } else { TINTA });
        s.pausa = boton(p, x + w - 62, 6, 24, 22, if c.pausada { b">" } else { b"||" }, neon);
        s.siguiente = boton(p, x + w - 34, 6, 24, 22, b">>", neon);
        return s;
    }

    // 3b. ABIERTA.
    let n = (((w - 60) / gw) as usize).min(c.titulo.len());
    p.texto_bytes(x + 16, 10, &c.titulo[..n], TINTA);
    s.cerrar = boton(p, x + w - 30, 8, 20, 18, b"x", TENUE);
    let mut sub = [0u8; 48];
    let k = linea_sub(&mut sub, c);
    p.texto_bytes(x + 16, 28, &sub[..k], if c.pausada { ROJO } else { TENUE });

    // La ONDA en vivo: un osciloscopio cuya amplitud es la fuerza medida y
    // su temblor el pico. Si esta en pausa, una raya quieta.
    let (ox, oy, ow, oh) = (x + 16, 50u32, w - 32 - 46, 44u32);
    p.rect(ox, oy, ow, oh, NEGRO);
    let medio = oy + oh / 2;
    let amp = if c.pausada { 0 } else { (fuerza * (oh / 2 - 2)) / 256 };
    let mut antes = medio;
    for i in 0..ow {
        let fase = (c.ms / 4 + i as u64 * 7) % 360;
        let tri = onda(fase * 1000 / 360, 1000) as i32 - 128;
        let temblor = onda(c.ms / 3 + (i as u64 * 37) % 97, 97) as i32 - 128;
        let v = tri * amp as i32 / 128 + temblor * (pico as i32 / 40) / 128;
        let py = (medio as i32 + v).clamp(oy as i32 + 1, (oy + oh - 2) as i32) as u32;
        let (a, b) = if py < antes { (py, antes) } else { (antes, py) };
        let col = mezcla(LIMA, MAGENTA, i * 256 / ow);
        p.rect(ox + i, a, 1, b - a + 1, if c.pausada { TENUE } else { col });
        antes = py;
    }
    // Los dos MEDIDORES, izquierdo y derecho, con su pico.
    for lado in 0..2u32 {
        let (mx, mw) = (x + w - 16 - 40 + lado * 22, 16u32);
        p.rect(mx, oy, mw, oh, NEGRO);
        let f = nivel(c.rms[lado as usize]) * oh / 256;
        let pk = nivel(c.pico[lado as usize]) * oh / 256;
        if f > 0 {
            p.rect(mx + 2, oy + oh - f, mw - 4, f, if c.pausada { TENUE } else { LIMA });
        }
        if pk > 1 {
            p.rect(mx + 1, oy + oh - pk, mw - 2, 2, if pk * 256 / oh > 240 { ROJO } else { TINTA });
        }
    }

    // Los MANDOS.
    let by = oy + oh + 10;
    s.pausa = boton(p, x + 16, by, 44, 24, if c.pausada { b">" } else { b"||" }, neon);
    s.siguiente = boton(p, x + 66, by, 44, 24, b">>", neon);
    s.menos = boton(p, x + 128, by, 32, 24, b"-", neon);
    let barra = (x + 166, by + 9, 120u32, 6u32);
    p.rect(barra.0, barra.1, barra.2, barra.3, NEGRO);
    p.rect(barra.0, barra.1, barra.2 * c.volumen.min(100) / 100, barra.3, neon);
    s.mas = boton(p, x + 292, by, 32, 24, b"+", neon);

    // La RECOMENDACION: la siguiente de las tranquilas, de la casa.
    let ry = by + 32;
    let pre: &[u8] = b"prueba: ";
    let cx = p.texto_bytes(x + 16, ry, pre, TENUE);
    let n = (((x + w - 40 - cx) / gw) as usize).min(c.recomendada.len());
    let fin = p.texto_bytes(cx, ry, &c.recomendada[..n], LIMA);
    p.texto_bytes(fin + gw, ry, b">", LIMA);
    s.recomendada = (x + 16, ry, fin + 2 * gw - x - 16, bmo::GLIFO_ALTO);
    s
}

/// `Kernel a medianoche` -> `fondo  88 pulsos  vol 70` (o `en pausa`).
fn linea_sub(b: &mut [u8; 48], c: &Cara) -> usize {
    let mut n = 0;
    let mut pon = |t: &[u8]| {
        for &x in t {
            if n < b.len() {
                b[n] = x;
                n += 1;
            }
        }
    };
    if c.pausada {
        pon(b"en pausa  ");
    } else {
        pon(b"de fondo  ");
    }
    let num = |v: u32, pon: &mut dyn FnMut(&[u8])| {
        let mut d = [0u8; 10];
        let mut k = d.len();
        let mut v = v;
        loop {
            k -= 1;
            d[k] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        pon(&d[k..]);
    };
    num(c.pulsos, &mut pon);
    pon(b" pulsos  vol ");
    num(c.volumen, &mut pon);
    n
}

/// El ecualizador chico de la asomada: `n` barras que siguen al medidor.
fn barras(p: &bmo::Pantalla, x: u32, y: u32, n: u32, alto: u32, c: &Cara, color: u32) {
    for i in 0..n {
        let lado = (i % 2) as usize;
        let base = if c.pausada { 2 } else { nivel(c.rms[lado]) * alto / 256 };
        let baila = if c.pausada { 0 } else { onda(c.ms + i as u64 * 130, 420 + i as u64 * 60) * (alto / 3) / 256 };
        let a = (base + baila).clamp(2, alto);
        p.rect(x + i * 6, y + alto - a, 4, a, color);
    }
}

/// Un boton de cristal con su letra en el centro. Devuelve su caja.
fn boton(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, letra: &[u8], neon: u32) -> Caja {
    p.rect(x, y, w, h, mezcla(CRISTAL_ARRIBA, neon, 40));
    p.rect(x, y, w, 1, neon);
    p.rect(x, y + h - 1, w, 1, neon);
    p.rect(x, y, 1, h, neon);
    p.rect(x + w - 1, y, 1, h, neon);
    let tw = letra.len() as u32 * bmo::GLIFO_ANCHO;
    p.texto_bytes(x + (w.saturating_sub(tw)) / 2, y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2, letra, TINTA);
    (x, y, w, h)
}
