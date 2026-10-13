//! **FRAPS-X GRABA** (13-10): la pantalla a un video, para subirlo y
//! ensenarlo. Pedido del propietario: *"fijate si FRAPS-X graba bien ... para
//! grabar y subir a YouTube y presentar algo"*. No grababa: contaba FPS, hacia
//! el banco y la captura PNG. Ahora graba.
//!
//! [consumo] NADA      apagado no lee nada. Grabando: un JPEG del lienzo por
//!                     fotograma que toca (lo que cueste: se dice al parar)
//!
//! ```text
//!    Ctrl+Shift+R      empieza; otra vez, para y guarda (el F9 de FRAPS)
//!    `fraps grabar [fps] [mitad] [calidad N]`   lo mismo desde Ejecutar
//!    el fichero        capturas/vidNNNNN.avi: MJPEG en AVI (`bmo_fraps::avi`),
//!                      lo que abren YouTube, VLC y Windows tal cual
//! ```
//!
//! # Que se graba
//!
//! El LIENZO del compositor (RAM: las apps ya compuestas en el, el juego
//! incluido, con el contador de FRAPS-X y el cursor), al acabar cada
//! fotograma (`paint::compose`). Cada fotograma del video es el del instante
//! `k / fps` (`avi::toca`): si en ese hueco no se pinto nada, va VACIO (el
//! reproductor repite el de antes, y el fichero no crece); si el que graba
//! llega tarde, los que se salto van vacios: el video dura lo que duro de
//! verdad.
//!
//! [!] Con la 3060 mandando en la pantalla (la pantalla directa, Z1) el
//! fotograma del juego no pasa por el lienzo: se graba lo de debajo. Para
//! grabar el juego, sin pantalla directa.
//!
//! [!] El JPEG lo hace ESTE nucleo, en la vuelta del compositor: a 1920x1080
//! cuesta decenas de ms por fotograma, y el escritorio va a ese paso mientras
//! graba. `mitad` (960x540) cuesta la cuarta parte. NVENC (la 3060, PLAN_LA_
//! LUDOTECA 14) es la que lo hara sin gastar CPU.

use bmo_fraps::avi::{self, Avi};
use bmo_userland as bmo;

use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{acento, paint_status, INK_BAD};

/// Lo que se pide para el video (y la mitad si no hay, hasta [`MINIMO`]).
const PEDIDO: u64 = 512 << 20;
const MINIMO: u64 = 32 << 20;
/// Lo que se escribe al disco por llamada.
const TRAMO: u64 = 16 << 20;

/// Como se graba.
#[derive(Clone, Copy)]
pub(crate) struct Opciones {
    pub fps: u32,
    pub mitad: bool,
    pub calidad: u8,
}

impl Opciones {
    pub(crate) const POR_DEFECTO: Opciones = Opciones { fps: 30, mitad: false, calidad: 85 };

    /// `[fps] [mitad] [calidad N]`, en cualquier orden.
    pub(crate) fn de(arg: &[u8]) -> Option<Opciones> {
        let mut o = Opciones::POR_DEFECTO;
        let mut palabras = arg.split(|&c| c == b' ').filter(|w| !w.is_empty());
        while let Some(w) = palabras.next() {
            match w {
                b"mitad" => o.mitad = true,
                b"calidad" => o.calidad = numero(palabras.next()?)?.clamp(10, 100) as u8,
                n => o.fps = numero(n)?.clamp(1, 60),
            }
        }
        Some(o)
    }
}

fn numero(w: &[u8]) -> Option<u32> {
    if w.is_empty() || w.len() > 4 || !w.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(w.iter().fold(0u32, |v, &c| v * 10 + (c - b'0') as u32))
}

struct Grabando {
    video: bmo::Memoria,
    bytes: usize,
    avi: Avi,
    /// El lienzo a la mitad (con `mitad`).
    chico: Option<bmo::Memoria>,
    op: Opciones,
    desde: u64,
    hz: u64,
    /// Fotogramas del video ya puestos (con los vacios).
    puestos: u64,
    /// Con imagen, y el tiempo de CPU que costaron.
    con_imagen: u64,
    ciclos_jpeg: u64,
    /// Se pinto algo desde el ultimo fotograma con imagen.
    pintado: bool,
    lleno: bool,
}

static mut GRABANDO: Option<Grabando> = None;
static mut SIGUIENTE: u32 = 0;

fn estado() -> &'static mut Option<Grabando> {
    // SAFETY: el DIRECTOR es un hilo: nadie mas lo toca.
    unsafe { &mut *core::ptr::addr_of_mut!(GRABANDO) }
}

/// Si esta grabando.
pub(crate) fn grabando() -> bool {
    estado().is_some()
}

/// **Ctrl+Shift+R / `fraps grabar`**: empezar, o parar y guardar.
pub(crate) fn alternar(dsk: &mut Desktop, p: &bmo::Pantalla, op: Opciones) {
    if grabando() {
        return parar(dsk, p);
    }
    let (w, h) = medidas(p, op.mitad);
    let mut pedido = PEDIDO;
    let video = loop {
        if let Some(m) = bmo::Memoria::request(pedido) {
            break m;
        }
        pedido /= 2;
        if pedido < MINIMO {
            return decir(dsk, p, b"  [fraps] NO se puede grabar: sin memoria para el video\n", false);
        }
    };
    // SAFETY: el bloque mide `pedido` bytes y es de este proceso.
    let buf = unsafe { core::slice::from_raw_parts(video.base(), pedido as usize) };
    let Ok(a) = Avi::nuevo(buf, w, h, op.fps) else {
        return decir(dsk, p, b"  [fraps] NO se puede grabar: las medidas de la pantalla no valen\n", false);
    };
    let chico = if op.mitad {
        match bmo::Memoria::request(4 * w as u64 * h as u64) {
            Some(m) => Some(m),
            None => return decir(dsk, p, b"  [fraps] NO se puede grabar: sin memoria para la mitad\n", false),
        }
    } else {
        None
    };
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    *estado() = Some(Grabando { video, bytes: pedido as usize, avi: a, chico, op, desde: bmo::ciclos(), hz, puestos: 0, con_imagen: 0, ciclos_jpeg: 0, pintado: true, lleno: false });
    let mut t = Linea::new();
    t.pon(b"  [fraps] GRABANDO ");
    t.num(w as u64);
    t.pon(b"x");
    t.num(h as u64);
    t.pon(b" a ");
    t.num(op.fps as u64);
    t.pon(b" fps, calidad ");
    t.num(op.calidad as u64);
    t.pon(b" (hasta ");
    t.num(pedido >> 20);
    t.pon(b" MiB). Ctrl+Shift+R para parar y guardar\n");
    decir(dsk, p, t.bytes(), true);
}

fn medidas(p: &bmo::Pantalla, mitad: bool) -> (u32, u32) {
    if mitad {
        (p.ancho / 2, p.alto / 2)
    } else {
        (p.ancho, p.alto)
    }
}

/// **Al acabar cada vuelta del compositor** (`paint::compose`): `pinto` si
/// este fotograma cambio el lienzo. Pone los fotogramas que tocan.
pub(crate) fn vuelta(p: &bmo::Pantalla, pinto: bool) {
    let Some(g) = estado().as_mut() else { return };
    g.pintado |= pinto;
    if g.lleno {
        return;
    }
    let toca = avi::toca(g.desde, bmo::ciclos(), g.hz, g.op.fps) + 1;
    // SAFETY: el bloque del video mide `g.bytes`, y es de este proceso.
    let buf = unsafe { core::slice::from_raw_parts_mut(g.video.base(), g.bytes) };
    while g.puestos < toca {
        // El ultimo de los que tocan lleva la imagen (si hubo cambios); los de
        // antes, vacios: no llegaron a tiempo.
        let imagen = g.puestos + 1 == toca && (g.pintado || g.con_imagen == 0);
        let n = if imagen {
            let t0 = bmo::ciclos();
            let r = jpeg(g, p, buf);
            g.ciclos_jpeg += bmo::ciclos() - t0;
            match r {
                Some(n) => {
                    g.con_imagen += 1;
                    g.pintado = false;
                    n
                }
                None => {
                    g.lleno = true;
                    return;
                }
            }
        } else {
            0
        };
        if g.avi.cerrar(buf, n).is_err() {
            g.lleno = true;
            return;
        }
        g.puestos += 1;
    }
}

/// El JPEG del lienzo en el hueco del siguiente fotograma.
fn jpeg(g: &mut Grabando, p: &bmo::Pantalla, buf: &mut [u8]) -> Option<usize> {
    let (w, h) = (g.avi.ancho, g.avi.alto);
    p.sincronizar_lectura();
    // SAFETY: el lienzo mide `stride x alto` pixeles (el de la pantalla).
    let lienzo = unsafe { core::slice::from_raw_parts(p.lienzo, (p.stride * p.alto) as usize) };
    let hueco = g.avi.hueco();
    let dst = buf.get_mut(hueco..)?;
    let (px, paso) = match &g.chico {
        Some(m) => {
            // SAFETY: el bloque mide `4 * w * h` (pedido al empezar).
            let chico = unsafe { core::slice::from_raw_parts_mut(m.base() as *mut u32, (w * h) as usize) };
            for y in 0..h {
                for x in 0..w {
                    let o = (2 * y * p.stride + 2 * x) as usize;
                    let (a, b, c, d) = (lienzo[o], lienzo[o + 1], lienzo[o + p.stride as usize], lienzo[o + p.stride as usize + 1]);
                    let media = |s: u32| ((a >> s & 0xFF) + (b >> s & 0xFF) + (c >> s & 0xFF) + (d >> s & 0xFF) + 2) / 4;
                    chico[(y * w + x) as usize] = media(16) << 16 | media(8) << 8 | media(0);
                }
            }
            (&*chico, w)
        }
        None => (lienzo, p.stride),
    };
    bmo_imagen::jpeg_escribir::codificar(px, w, h, paso, false, g.op.calidad, dst).ok()
}

/// **Parar y guardar** en `capturas/vidNNNNN.avi`.
fn parar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let Some(mut g) = estado().take() else { return };
    // SAFETY: como en `vuelta`.
    let buf = unsafe { core::slice::from_raw_parts_mut(g.video.base(), g.bytes) };
    let total = g.avi.terminar(buf) as u64;
    let n = siguiente();
    let mut ruta = *b"capturas/vid00000.avi";
    let mut d = n;
    for k in (12..17).rev() {
        ruta[k] = b'0' + (d % 10) as u8;
        d /= 10;
    }
    let t0 = bmo::ciclos();
    let guardado = match bmo::Archivo::create(&ruta) {
        Ok(a) => {
            let mut hecho = 0u64;
            while hecho < total {
                let k = (total - hecho).min(TRAMO);
                if a.escribir_de(&g.video, hecho, k) != k {
                    break;
                }
                hecho += k;
            }
            hecho == total && a.close()
        }
        Err(_) => false,
    };
    let ms_disco = (bmo::ciclos() - t0) * 1000 / g.hz;
    g.chico = None;
    let mut t = Linea::new();
    if !guardado {
        t.pon(b"  [fraps] NO se pudo guardar ");
        t.pon(&ruta);
        t.pon(b" (falta capturas/ o no cabe en el disco)\n");
        return decir(dsk, p, t.bytes(), false);
    }
    unsafe { SIGUIENTE = n + 1 };
    let segundos = g.puestos / g.op.fps.max(1) as u64;
    t.pon(b"  [fraps] GUARDADO ");
    t.pon(&ruta);
    t.pon(b": ");
    t.num(g.avi.ancho as u64);
    t.pon(b"x");
    t.num(g.avi.alto as u64);
    t.pon(b", ");
    t.num(segundos);
    t.pon(b" s, ");
    t.num(total >> 20);
    t.pon(b" MiB; ");
    t.num(g.con_imagen);
    t.pon(b" fotogramas con imagen de ");
    t.num(g.puestos);
    if g.con_imagen > 0 {
        t.pon(b"; el JPEG ");
        t.num(g.ciclos_jpeg * 1000 / g.hz / g.con_imagen);
        t.pon(b" ms cada uno");
    }
    t.pon(b"; disco ");
    t.num(ms_disco);
    t.pon(b" ms");
    if g.lleno {
        t.pon(b" (se LLENO: lo de despues no se grabo)");
    }
    t.pon(b"\n");
    decir(dsk, p, t.bytes(), true);
}

/// El numero del siguiente `vidNNNNN.avi`: el mayor de la carpeta + 1.
fn siguiente() -> u32 {
    // SAFETY: un hilo.
    let n = unsafe { SIGUIENTE };
    if n != 0 {
        return n;
    }
    let mut mayor = 0u32;
    if let Ok(dir) = bmo::Directorio::open(b"capturas") {
        while let Some(x) = dir.next() {
            let nom = &x.name;
            if x.es_dir || !nom[..3].eq_ignore_ascii_case(b"VID") || !nom[3..8].iter().all(|c| c.is_ascii_digit()) {
                continue;
            }
            mayor = mayor.max(nom[3..8].iter().fold(0u32, |v, &c| v * 10 + (c - b'0') as u32));
        }
    }
    (mayor + 1).min(99_999)
}

fn decir(dsk: &mut Desktop, p: &bmo::Pantalla, texto: &[u8], bien: bool) {
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(texto);
    g.with_ink(INK_PLAIN);
    let corto = if bien { if grabando() { "FRAPS-X grabando" } else { "FRAPS-X: video guardado en capturas/" } } else { "FRAPS-X no graba" };
    paint_status(p, &dsk.run_box, corto, if bien { acento() } else { INK_BAD });
}

/// Una linea sin memoria dinamica.
struct Linea {
    b: [u8; 256],
    n: usize,
}

impl Linea {
    fn new() -> Self {
        Linea { b: [0; 256], n: 0 }
    }
    fn pon(&mut self, s: &[u8]) {
        let k = s.len().min(self.b.len() - self.n);
        self.b[self.n..self.n + k].copy_from_slice(&s[..k]);
        self.n += k;
    }
    fn num(&mut self, v: u64) {
        let mut d = [0u8; 20];
        let (mut i, mut v) = (20, v);
        loop {
            i -= 1;
            d[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.pon(&d[i..]);
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}
