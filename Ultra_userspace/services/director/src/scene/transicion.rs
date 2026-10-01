//! **ABRIR Y CERRAR, COMO UN TUBO DE RAYOS CATODICOS** (2026-09-25) -- el
//! marco de neon de una ventana que nace o que se va:
//!
//! ```text
//!    ABRE    un punto -> una linea -> el marco entero       (240 ms)
//!    CIERRA  el marco -> una linea -> un punto que se apaga (240 ms)
//! ```
//!
//! [consumo] LATE      los 240 ms de cada transicion (260 al cambiar de
//!                     caja), a ~30 fotogramas por
//!                     segundo (`desktop::transicion::anima`); sin ventanas
//!                     que nazcan o se vayan, nada (L6h)
//!
//! ** Y MAXIMIZAR, RESTAURAR Y ENCAJAR (01-10). Pedido: *"solucionar el bug
//! de maximizar, se ven tirones"* y *"que sea dinamico y con
//! animacion con glitch por completo"*. El contenido ya NO se mueve por
//! fotogramas: la ventana salta limpia a su caja nueva en UNO, y por encima
//! corre esto:
//!
//! ```text
//!    CAMBIA  el marco viaja de la caja vieja a la nueva       (260 ms)
//!            con frenada (sale rapido, llega suave); dos fantasmas rosa
//!            y azul a los lados (la separacion RGB) que se juntan al
//!            llegar; y dos rafagas de franjas que saltan dentro de la
//!            ventana, con el contenido de verdad corrido y tintado
//! ```
//!
//! Aqui no se sabe QUE ventana es ni CUANDO: llega su caja, si abre o cierra y
//! la edad (`desktop::transicion` decide, esto solo pinta). Se pone como el
//! globo: guarda lo que tapa --las cuatro tiras del marco de ESTE fotograma--
//! y lo devuelve al principio del siguiente.

use bmo_userland as bmo;

use super::globo::mezcla;

/// Lo que dura abrir o cerrar.
pub(crate) const DURA_MS: u64 = 240;
/// Lo que dura un cambio de caja (maximizar, restaurar, encajar).
pub(crate) const DURA_CAMBIO_MS: u64 = 260;
/// El marco: 2 px de neon y 2 de resplandor por fuera.
const GROSOR: u32 = 4;
/// Dos transiciones a la vez (una que se va y otra que llega).
pub(crate) const HUECOS: usize = 2;
/// Lo que se guarda por hueco: el marco y sus dos fantasmas de una 4K, y las
/// franjas del glitch. Lo que no cabe no se pinta (nunca se pinta sin guardar).
const GUARDADO: usize = 192 * 1024;
/// Tiras por hueco: tres marcos de cuatro y las franjas.
const TIRAS: usize = 16;

const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const AZUL: u32 = 0x003D_A5FF;
const BLANCO: u32 = 0x00FF_FFFF;

type Caja = (u32, u32, u32, u32);

struct Hueco {
    px: [u32; GUARDADO],
    tiras: [Caja; TIRAS],
    n: usize,
    usados: usize,
    puesto: bool,
}

const VACIO: Hueco = Hueco { px: [0; GUARDADO], tiras: [(0, 0, 0, 0); TIRAS], n: 0, usados: 0, puesto: false };
static mut HUECOS_: [Hueco; HUECOS] = [VACIO; HUECOS];

fn hueco(k: usize) -> &'static mut Hueco {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut (*core::ptr::addr_of_mut!(HUECOS_))[k] }
}

/// **Quita las transiciones**: devuelve lo que tapaban, al reves de como se
/// pusieron. Al PRINCIPIO del fotograma, el ultimo de las capas.
///
/// Dentro de un hueco tambien al reves: las tiras se pisan (un fantasma
/// encima del marco, una franja encima de los dos) y la que se guardo
/// PRIMERO es la que tiene el fondo de verdad, asi que se devuelve la ultima.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    for k in (0..HUECOS).rev() {
        let h = hueco(k);
        if !h.puesto {
            continue;
        }
        let mut fin = h.usados;
        for &(x, y, w, t) in h.tiras[..h.n].iter().rev() {
            let mut i = fin - (w * t) as usize;
            fin = i;
            p.marcar(x, y, w, t);
            for dy in 0..t {
                for dx in 0..w {
                    p.punto_ya_marcado(x + dx, y + dy, h.px[i]);
                    i += 1;
                }
            }
        }
        h.n = 0;
        h.usados = 0;
        h.puesto = false;
    }
}

/// Guarda lo que hay bajo `t` y devuelve donde empieza, o `None` si no cabe.
fn guardar(p: &bmo::Pantalla, h: &mut Hueco, t: Caja) -> Option<usize> {
    let tam = (t.2 * t.3) as usize;
    if tam == 0 || h.n >= TIRAS || h.usados + tam > GUARDADO {
        return None;
    }
    let desde = h.usados;
    let mut i = desde;
    for dy in 0..t.3 {
        for dx in 0..t.2 {
            h.px[i] = p.read(t.0 + dx, t.1 + dy);
            i += 1;
        }
    }
    h.tiras[h.n] = t;
    h.n += 1;
    h.usados = i;
    h.puesto = true;
    Some(desde)
}

/// **La caja de este instante** al abrir o cerrar: `(x, y, w, h)` escalada
/// desde el centro, y cuanto brilla (256 = entero).
fn caja_en(c: Caja, abre: bool, edad_ms: u64) -> (Caja, u32) {
    let t = (edad_ms.min(DURA_MS) * 1000 / DURA_MS) as u32; // 0..1000
    // Al cerrar, la misma pelicula al reves.
    let t = if abre { t } else { 1000 - t };
    // Primero el ancho (0..400), despues el alto (400..1000).
    let fw = (t * 1000 / 400).min(1000);
    let fh = if t < 400 { 0 } else { (t - 400) * 1000 / 600 };
    let (x, y, w, h) = c;
    let ww = (w as u64 * fw as u64 / 1000).max(2) as u32;
    let hh = (h as u64 * fh as u64 / 1000).max(2) as u32;
    let caja = (x + (w - ww.min(w)) / 2, y + (h - hh.min(h)) / 2, ww.min(w), hh.min(h));
    // Brilla del todo mientras es linea; se desvanece al llegar al marco.
    let brillo = if t < 400 { 256 } else { 256 - (t - 400) * 100 / 600 };
    (caja, brillo)
}

/// **Un marco de neon** en `caja`, de `grosor` px, guardando lo que tapa.
/// Con `grosor` 4 lleva resplandor; con 2, es la linea sola (un fantasma).
fn marco(p: &bmo::Pantalla, h: &mut Hueco, caja: Caja, grosor: u32, neon: u32, brillo: u32) {
    let (x, y, w, hh) = caja;
    if w < 2 || hh < 2 {
        return;
    }
    let (x0, y0) = (x.saturating_sub(grosor / 2), y.saturating_sub(grosor / 2));
    let x1 = (x + w + grosor / 2).min(p.ancho);
    let y1 = (y + hh + grosor / 2).min(p.alto);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // Las tiras: arriba y abajo enteras; los lados, entre medias.
    let alto_t = grosor.min(y1 - y0);
    let mut tiras = [(0u32, 0u32, 0u32, 0u32); 4];
    let mut n = 0;
    let mut tira = |t: Caja| {
        if t.2 > 0 && t.3 > 0 && n < 4 {
            tiras[n] = t;
            n += 1;
        }
    };
    tira((x0, y0, x1 - x0, alto_t));
    if y1 - y0 > 2 * grosor {
        tira((x0, y1 - grosor, x1 - x0, grosor));
        let lado = (y1 - grosor) - (y0 + grosor);
        tira((x0, y0 + grosor, grosor.min(x1 - x0), lado));
        tira((x1.saturating_sub(grosor).max(x0), y0 + grosor, grosor.min(x1 - x0), lado));
    } else if y1 - y0 > alto_t {
        tira((x0, y0 + alto_t, x1 - x0, y1 - y0 - alto_t));
    }
    for &(tx, ty, tw, th) in &tiras[..n] {
        let Some(mut i) = guardar(p, h, (tx, ty, tw, th)) else { return };
        // Pintar: el centro de la tira en neon (casi blanco si brilla
        // entero), los bordes como resplandor.
        p.marcar(tx, ty, tw, th);
        for dy in 0..th {
            for dx in 0..tw {
                let (px, py) = (tx + dx, ty + dy);
                // A cuanto esta del borde ideal del marco (0 = encima).
                let dxb = px.abs_diff(x).min(px.abs_diff(x + w - 1));
                let dyb = py.abs_diff(y).min(py.abs_diff(y + hh - 1));
                let dentro_x = px >= x && px < x + w;
                let dentro_y = py >= y && py < y + hh;
                let d = if dentro_x && dentro_y { dxb.min(dyb) } else if dentro_x { dyb } else if dentro_y { dxb } else { dxb.max(dyb) };
                let alfa = match d {
                    0 => 255,
                    1 => 200,
                    2 => 90,
                    _ => 30,
                } * brillo / 256;
                let color = if d == 0 { mezcla(neon, BLANCO, brillo / 2) } else { neon };
                p.punto_ya_marcado(px, py, mezcla(h.px[i], color, alfa));
                i += 1;
            }
        }
    }
}

/// Un numero que salta: el mismo `semilla` da el mismo.
fn azar(semilla: u64) -> u32 {
    let mut z = semilla.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as u32
}

/// **Las franjas del glitch** dentro de `caja`: `cuantas` bandas finas de lo
/// que YA hay pintado, corridas a un lado y tintadas de rosa o de azul. La
/// `semilla` cambia cada ~40 ms, asi que saltan.
fn franjas(p: &bmo::Pantalla, h: &mut Hueco, caja: Caja, cuantas: u32, semilla: u64, fuerza: u32) {
    let (x, y, w, hh) = caja;
    if w < 32 || hh < 32 {
        return;
    }
    let ancho = w.min(p.ancho.saturating_sub(x));
    for b in 0..cuantas {
        let r = azar(semilla * 7 + b as u64);
        let alto = 2 + r % 5;
        let fy = y + 4 + (r >> 8) % (hh - 8).max(1);
        if fy + alto > (y + hh).min(p.alto) {
            continue;
        }
        let salto = (4 + (r >> 20) % 14) as i32 * if r & 1 == 0 { 1 } else { -1 };
        let tinte = if r & 2 == 0 { MAGENTA } else { AZUL };
        let Some(desde) = guardar(p, h, (x, fy, ancho, alto)) else { return };
        p.marcar(x, fy, ancho, alto);
        for dy in 0..alto {
            let fila = desde + (dy * ancho) as usize;
            for dx in 0..ancho {
                // El pixel de al lado, de la copia guardada (no del lienzo
                // ya corrido): la franja se desplaza entera.
                let ox = (dx as i32 - salto).clamp(0, ancho as i32 - 1) as usize;
                let c = mezcla(h.px[fila + ox], tinte, fuerza);
                p.punto_ya_marcado(x + dx, fy + dy, c);
            }
        }
    }
}

/// **Pone la transicion** del hueco `k`: la ventana `caja`, que abre o cierra,
/// de `edad_ms`. Al FINAL del fotograma, debajo de las demas capas.
pub(crate) fn poner(p: &bmo::Pantalla, k: usize, caja: Caja, abre: bool, edad_ms: u64) {
    let h = hueco(k);
    if h.puesto || caja.2 < 4 || caja.3 < 4 || edad_ms >= DURA_MS {
        return;
    }
    p.sincronizar_lectura();
    let (c, brillo) = caja_en(caja, abre, edad_ms);
    marco(p, h, c, GROSOR, if abre { CIAN } else { MAGENTA }, brillo);
    // Al nacer, una rafaga corta sobre la ventana ya entera.
    if abre && (170..215).contains(&edad_ms) {
        franjas(p, h, caja, 2, edad_ms / 40 + caja.0 as u64, 110);
    }
}

/// Frenada: sale rapido y llega suave. `t` y lo devuelto en milesimas.
fn frena(t: u32) -> u32 {
    let u = 1000 - t.min(1000) as u64;
    (1000 - u * u * u / 1_000_000) as u32
}

fn entre(a: u32, b: u32, e: u32) -> u32 {
    (a as i64 + (b as i64 - a as i64) * e as i64 / 1000) as u32
}

/// **Pone un cambio de caja** del hueco `k`: de `viejo` a `nuevo`, de
/// `edad_ms`. La ventana YA esta pintada en `nuevo`; esto es el marco que
/// viaja, sus fantasmas y el glitch.
pub(crate) fn poner_cambio(p: &bmo::Pantalla, k: usize, viejo: Caja, nuevo: Caja, edad_ms: u64) {
    let h = hueco(k);
    if h.puesto || edad_ms >= DURA_CAMBIO_MS {
        return;
    }
    p.sincronizar_lectura();
    let t = (edad_ms * 1000 / DURA_CAMBIO_MS) as u32;
    let e = frena(t);
    let caja = (entre(viejo.0, nuevo.0, e), entre(viejo.1, nuevo.1, e), entre(viejo.2, nuevo.2, e), entre(viejo.3, nuevo.3, e));
    let crece = nuevo.2 as u64 * nuevo.3 as u64 >= viejo.2 as u64 * viejo.3 as u64;
    let neon = if crece { CIAN } else { MAGENTA };
    // El brillo: entero al salir, se apaga en el ultimo tercio.
    let brillo = if t < 650 { 256 } else { 256 - (t - 650) * 200 / 350 };
    // -- las rafagas: al salir y un poco antes de llegar --
    let semilla = (edad_ms / 40) ^ ((nuevo.0 as u64) << 8);
    if t < 260 {
        franjas(p, h, nuevo, 3, semilla, 120);
    } else if (560..720).contains(&t) {
        franjas(p, h, nuevo, 2, semilla, 90);
    }
    // -- los fantasmas: la separacion RGB, que se junta al llegar --
    let s = (1000 - e) * 10 / 1000 + 1;
    let corre = |c: Caja, dx: i32| ((c.0 as i32 + dx).max(0) as u32, c.1, c.2, c.3);
    marco(p, h, corre(caja, -(s as i32)), 2, MAGENTA, brillo * 3 / 4);
    marco(p, h, corre(caja, s as i32), 2, AZUL, brillo * 3 / 4);
    // -- y el marco --
    marco(p, h, caja, GROSOR, neon, brillo);
}
