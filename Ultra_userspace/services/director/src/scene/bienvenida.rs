//! **LA BIENVENIDA** -- el panel de 8 bits de "El emisor salta" (S4i de
//! `PLAN_EL_SONIDO.md`).
//!
//! [consumo] LATE      ~8 s a ~30 fotogramas por segundo, una vez por
//!                     arranque (`desktop::bienvenida::anima`); lee el
//!                     medidor del maestro una vez por fotograma. Despues,
//!                     nada (L6h)
//!
//! ```text
//!    +--  ------------------------------------------------  --+
//!    |   .        *            B M O - X   (letra a letra,   |
//!    |    /\_/\        .        salta al compas)              |
//!    |   ( o.o )   <- bota     > El emisor salta_             |
//!    |    > ^ <      a 128     128 pulsos, compuesta aqui     |
//!    |   ~~~~~~~                # # # # # # # # # # #  <- el   |
//!    |  tecla o clic            # # # # # # # # # # #  medidor |
//!    +--  ===========================(lo que queda)=====  --+
//! ```
//!
//! Todo en BLOQUES, como una consola de 8 bits: las estrellas son cuadros de
//! 2 y 3 pixeles, el gato es la mascara de la intro (`scene::gato`) bajada a
//! bloques de 4, y las letras son la fuente de siempre ampliada a enteros.
//! Las barras NO son un dibujo: siguen al medidor del maestro, lo que de
//! verdad sale por el cable (sin tubo, se quedan en un bloque).
//!
//! Se pone como la pastilla: guarda lo que tapa al FINAL del fotograma y lo
//! devuelve al PRINCIPIO del siguiente. Crece al llegar y se encoge hacia
//! arriba, hasta donde vive la pastilla, al irse.

use bmo_userland as bmo;

use crate::scene::gato;
use crate::scene::globo::{mezcla, onda};

/// El panel entero.
const W: u32 = 600;
const H: u32 = 340;
const GUARDADO: usize = (W * H) as usize;
/// A donde se encoge: la pastilla asomada (`scene::pastilla`).
const PASTILLA: (u32, u32) = (320, 36);
/// El grueso del borde y el lado de un bloque del gato.
const BORDE: u32 = 3;
const BLOQUE: u32 = 4;

// La paleta: la de una consola de 8 bits, con el lima de la ONDA.
const NOCHE_ARRIBA: u32 = 0x000B_0E1F;
const NOCHE_ABAJO: u32 = 0x001E_1038;
const BLANCO: u32 = 0x00FF_F1E8;
const CIAN: u32 = 0x0029_ADFF;
const ROSA: u32 = 0x00FF_77A8;
const MAGENTA: u32 = 0x00FF_2BD6;
const LIMA: u32 = 0x00B6_FF5C;
const VERDE: u32 = 0x0000_E436;
const AMARILLO: u32 = 0x00FF_EC27;
const ROJO: u32 = 0x00FF_004D;
const TENUE: u32 = 0x0083_769C;
const SOMBRA: u32 = 0x0026_0B3A;
const APAGADO: u32 = 0x002A_2440;
/// Los colores del titulo, que van rotando letra a letra.
const ARCOIRIS: [u32; 7] = [ROJO, 0x00FF_A300, AMARILLO, VERDE, CIAN, 0x0083_76FF, ROSA];

/// Lo que se ve, de un fotograma.
pub(crate) struct Cara<'a> {
    /// Desde que empezo a sonar: el compas 0 de la pieza.
    pub ms: u64,
    /// Lo que lleva crecido y lo que lleva ido, de 0 a 1000.
    pub crece: u64,
    pub se_va: u64,
    /// Lo que queda antes de minimizarse, de 1000 a 0.
    pub queda: u64,
    pub pulsos: u32,
    pub titulo: &'a [u8],
    pub suena: bool,
    /// Lo que mide el maestro, pico y fuerza, en dBFS (1/256).
    pub pico: [i32; 2],
    pub rms: [i32; 2],
}

struct Guardado {
    px: [u32; GUARDADO],
    caja: (u32, u32, u32, u32),
    puesto: bool,
}

static mut GUARDADO_: Guardado = Guardado { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn guardado() -> &'static mut Guardado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(GUARDADO_) }
}

/// **Quita el panel**: devuelve lo que tapaba. Al PRINCIPIO del fotograma.
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

/// Lo que se mete la fila `dy` por cada lado: las esquinas en ESCALERA, de
/// dos niveles, como un marco de 8 bits.
fn sangria(dy: u32, h: u32) -> u32 {
    let d = dy.min(h.saturating_sub(1 + dy));
    if d < 4 {
        8
    } else if d < 8 {
        4
    } else {
        0
    }
}

/// Un bloque lleno, sin marcar (la caja entera ya esta marcada).
fn bloque(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for dy in 0..h {
        for dx in 0..w {
            p.punto_ya_marcado(x + dx, y + dy, color);
        }
    }
}

/// Por donde va el pulso, de 0 a 1000 (0 = el golpe del bombo).
fn en_el_pulso(ms: u64, pulsos: u32) -> u64 {
    (ms * pulsos.max(1) as u64 % 60_000) * 1000 / 60_000
}

/// Un salto: 0 en el golpe, `alto` a mitad de pulso (una parabola).
fn salto(b: u64, alto: u64) -> u32 {
    (alto * 4 * b * (1000 - b.min(1000)) / 1_000_000) as u32
}

/// **Pone el panel.** Devuelve su caja (ancho 0 = no cabia).
pub(crate) fn poner(p: &bmo::Pantalla, c: &Cara) -> (u32, u32, u32, u32) {
    let g = guardado();
    if g.puesto || p.ancho < W + 40 || p.alto < H + 40 {
        return (0, 0, 0, 0);
    }
    // ** La CAJA de este fotograma: crece con rebote desde el centro, y al
    // irse se encoge acelerando hacia arriba, a donde vive la pastilla.
    let (cx, cy0) = (p.ancho / 2, (p.alto - H) * 2 / 5);
    let (w, h, y) = if c.se_va > 0 {
        let u = c.se_va * c.se_va / 1000;
        let l = |a: u32, b: u32| (a as i64 + (b as i64 - a as i64) * u as i64 / 1000) as u32;
        (l(W, PASTILLA.0), l(H, PASTILLA.1), l(cy0, 0))
    } else {
        let t = c.crece;
        let s = if t < 700 { 300 + t * 800 / 700 } else { 1100 - (t - 700) * 100 / 300 };
        let (w, h) = ((W as u64 * s.min(1000) / 1000) as u32, (H as u64 * s.min(1080) / 1000).min(H as u64) as u32);
        (w, h, cy0 + (H - h) / 2)
    };
    let (w, h) = (w.max(24), h.max(20));
    let x = cx - w / 2;

    // Lo que se guarda.
    g.caja = (x, y, w, h);
    p.sincronizar_lectura();
    for dy in 0..h {
        for dx in 0..w {
            g.px[(dy * w + dx) as usize] = p.read(x + dx, y + dy);
        }
    }
    g.puesto = true;
    p.marcar(x, y, w, h);

    let b = en_el_pulso(c.ms, c.pulsos);
    // El borde cambia de color con el compas: cian a magenta y vuelta.
    let neon = mezcla(CIAN, MAGENTA, onda(c.ms, 60_000 * 4 / c.pulsos.max(1) as u64));
    let neon = if b < 120 { mezcla(neon, BLANCO, 120) } else { neon };

    // 1. El CIELO, fila a fila, con el borde en escalera.
    for dy in 0..h {
        let s = sangria(dy, h);
        let fila = mezcla(NOCHE_ARRIBA, NOCHE_ABAJO, dy * 256 / h);
        let fila = if dy % 3 == 2 { mezcla(fila, 0, 40) } else { fila };
        if dy < BORDE || dy + BORDE >= h {
            p.rect(x + s, y + dy, w - 2 * s, 1, neon);
            continue;
        }
        let dentro = if dy >= BORDE && dy + BORDE < h { BORDE + sangria(dy - BORDE, h - 2 * BORDE) } else { s };
        let dentro = dentro.max(s + BORDE).min(w / 2);
        p.rect(x + s, y + dy, dentro - s, 1, neon);
        p.rect(x + dentro, y + dy, w - 2 * dentro, 1, fila);
        p.rect(x + w - dentro, y + dy, dentro - s, 1, neon);
    }

    // 2. Las ESTRELLAS: tres capas que pasan a distinta velocidad (la de
    // delante, mas grande y mas rapida), y que titilan.
    for i in 0..44u32 {
        let hs = i.wrapping_mul(2_654_435_761);
        let capa = i % 3;
        let vel = [18u64, 40, 85][capa as usize];
        let lado = [2u32, 2, 3][capa as usize];
        let sx = ((hs % W) as u64 + W as u64 * 1000 - (c.ms * vel / 1000) % W as u64) % W as u64;
        let sy = (hs >> 12) % (H - 24) + 10;
        let (px, py) = (sx as u32 * w / W, sy * h / H);
        if px < 10 || py < 8 || px + 10 + lado > w || py + 8 + lado > h {
            continue;
        }
        let brillo = onda(c.ms + i as u64 * 137, 700 + (i as u64 % 5) * 160);
        let color = match i % 4 {
            0 => CIAN,
            1 => ROSA,
            _ => BLANCO,
        };
        bloque(p, x + px, y + py, lado, lado, mezcla(APAGADO, color, 60 + brillo * 3 / 4));
    }

    // Mientras crece o se va: el titulo solo, a la escala que quepa.
    if c.se_va > 0 || c.crece < 1000 {
        let esc = (5 * h / H).min(5);
        let tw = 5 * bmo::GLIFO_ANCHO * esc;
        if esc >= 1 && tw + 16 < w && bmo::GLIFO_ALTO * esc + 8 < h {
            p.texto_escala(x + (w - tw) / 2, y + (h - bmo::GLIFO_ALTO * esc) / 2, "BMO-X", LIMA, esc);
        }
        return (x, y, w, h);
    }

    // 3. El GATO en bloques, que BOTA: toca el suelo en cada golpe del bombo.
    let alto_salto = salto(b, 22);
    let (gx, gy) = (x + 24, y + 62 + 22 - alto_salto);
    // La sombra en el suelo: chica cuando esta arriba.
    let sw = 120 - alto_salto * 3;
    bloque(p, gx + (152 - sw) / 2, y + 262, sw, 6, SOMBRA);
    // El suelo: rayas que corren hacia la izquierda, como una carretera.
    for k in 0..9u32 {
        let off = ((c.ms / 12) % 24) as u32;
        let rx = (k * 24 + 24 - off) % 216;
        if rx + 12 < 200 {
            bloque(p, x + 12 + rx, y + 274, 12, 4, if k % 2 == 0 { MAGENTA } else { APAGADO });
        }
    }
    let parpadea = c.ms % 3_200 < 140;
    let trazo = mezcla(CIAN, MAGENTA, 70 + salto(b, 120));
    let bit = |m: &[u8], fx: u32, fy: u32| {
        let i = (fy * gato::WIDTH + fx) as usize;
        m[i / 8] >> (i % 8) & 1 == 1
    };
    for by in 0..gato::HEIGHT / BLOQUE {
        for bx in 0..gato::WIDTH / BLOQUE {
            let (mut n_trazo, mut ojo) = (0u32, false);
            for dy in 0..BLOQUE {
                for dx in 0..BLOQUE {
                    let (fx, fy) = (bx * BLOQUE + dx, by * BLOQUE + dy);
                    ojo |= bit(&gato::EYES, fx, fy);
                    n_trazo += bit(&gato::STROKE, fx, fy) as u32;
                }
            }
            // Un bloque es trazo si lo es al menos un cuarto de el: con uno
            // solo el gato engorda, con la mitad se rompen las lineas finas.
            let color = if ojo && !parpadea {
                AMARILLO
            } else if n_trazo * 4 >= BLOQUE * BLOQUE || (ojo && parpadea) {
                trazo
            } else {
                continue;
            };
            bloque(p, gx + bx * BLOQUE, gy + by * BLOQUE, BLOQUE, BLOQUE, color);
        }
    }

    // 4. BMO-X, letra a letra: cada una salta un poco despues que la
    // anterior (una ola al compas), con su sombra y su color rotando.
    let (tx, ty) = (x + 222, y + 26);
    let esc = 5;
    for (k, ch) in b"BMO-X".iter().enumerate() {
        let bk = (b + 1000 - (k as u64 * 110) % 1000) % 1000;
        let sube = salto(bk, 12);
        let lx = tx + k as u32 * (bmo::GLIFO_ANCHO * esc + 4);
        let ly = ty + 12 - sube;
        p.glifo_escala(lx + 5, ly + 5, *ch, SOMBRA, esc);
        let color = ARCOIRIS[(k + (c.ms / 234) as usize) % ARCOIRIS.len()];
        p.glifo_escala(lx, ly, *ch, color, esc);
    }

    // 5. Lo que suena, con su cursor que parpadea, y de donde sale.
    let ny = y + 126;
    let mut fx = tx;
    for &ch in c.titulo.iter().take(22) {
        p.glifo_escala(fx, ny, ch, LIMA, 2);
        fx += bmo::GLIFO_ANCHO * 2;
    }
    if (c.ms / 400) % 2 == 0 {
        bloque(p, fx + 4, ny + 2, 12, 28, LIMA);
    }
    let mut sub = [0u8; 64];
    let n = linea_sub(&mut sub, c.pulsos, c.suena);
    p.texto_bytes(tx, y + 168, &sub[..n], if c.suena { TENUE } else { ROJO });

    // 6. El ECUALIZADOR en bloques: la fuerza de cada lado, mas un baile
    // propio de cada barra, mas el golpe del bombo.
    let (bx0, by0, filas) = (tx, y + 300, 11u32);
    for k in 0..11u32 {
        let lado = (k % 2) as usize;
        let n = if c.suena {
            let base = nivel(c.rms[lado]) * filas / 256;
            let baila = onda(c.ms + k as u64 * 97, 260 + k as u64 * 37) * 3 / 256;
            let golpe = if b < 150 { 2 } else { 0 };
            (base + baila + golpe).clamp(1, filas)
        } else {
            1
        };
        let pico = nivel(c.pico[lado]) * filas / 256;
        for j in 0..filas {
            let color = if j < n {
                if j < 6 {
                    VERDE
                } else if j < 9 {
                    AMARILLO
                } else {
                    ROJO
                }
            } else if c.suena && j == pico.min(filas - 1) && pico > n {
                BLANCO
            } else {
                APAGADO
            };
            bloque(p, bx0 + k * 32, by0 - (j + 1) * 10, 26, 8, color);
        }
    }

    // 7. Abajo: como se va, y lo que le queda antes de irse sola.
    p.texto_bytes(x + 16, y + h - 34, b"tecla o clic: a la pastilla", TENUE);
    let (lx, lw) = (x + 16, w - 32);
    let lleno = (lw as u64 * c.queda / 1000) as u32;
    bloque(p, lx, y + h - 12, lw, 3, APAGADO);
    bloque(p, lx, y + h - 12, lleno, 3, neon);
    (x, y, w, h)
}

/// `128 pulsos, compuesta aqui mismo` (en este procesador, al llegar).
fn linea_sub(b: &mut [u8; 64], pulsos: u32, suena: bool) -> usize {
    let mut n = 0;
    let mut pon = |t: &[u8]| {
        for &x in t {
            if n < b.len() {
                b[n] = x;
                n += 1;
            }
        }
    };
    if !suena {
        pon(b"sin tubo de audio: solo se ve");
        return n;
    }
    let mut d = [0u8; 10];
    let mut k = d.len();
    let mut v = pulsos;
    loop {
        k -= 1;
        d[k] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    pon(&d[k..]);
    pon(b" pulsos, compuesta aqui mismo");
    n
}
