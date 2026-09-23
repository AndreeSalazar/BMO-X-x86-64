//! **LA CAPTURA DE PANTALLA** (2026-09-22): Impr Pant, como en Windows, con
//! herramientas propias de punta a punta.
//!
//! [consumo] NADA      no corre en reposo: solo cuando se pulsa Impr Pant o se
//!                     escribe `captura`. Entonces lee el lienzo UNA vez, arma
//!                     el fichero y lo entrega (L6h)
//!
//! El propietario: *"no olvides tener captura de pantalla en la BMO-X por
//! completo, pero con herramientas propias como Windows"*.
//!
//! ```text
//!    Impr Pant          la pantalla entera
//!    Alt + Impr Pant    solo la ventana de delante (la de Windows)
//!    Ctrl+Shift+S       RECORTE: se arrastra un rectangulo con el raton
//!    Shift + Impr Pant  lo mismo (el Win+Shift+S de Windows; aqui el gestor es Ctrl)
//!    `captura`          desde Ejecutar, para un teclado sin la tecla
//!                       (`captura ventana`, `captura zona`)
//! ```
//!
//! # El camino entero, y de quien es cada tramo
//!
//! ```text
//!    la tecla     el puente USB le da codigo propio (`SC_IMPR`: era el `*` del
//!                 numpad) y el kernel la cuece al byte 0x95 (`KEY_IMPR`)
//!    los pixeles  el LIENZO del compositor, que es RAM: las apps ya estan
//!                 compuestas en el. Donde esta el cursor se lee lo que tapa
//!                 (`SaveUnder::debajo`): una captura no lleva el puntero
//!    el formato   BMP de 24 bits, de abajo arriba -- el de Paint. Lo abren
//!                 Windows sin nada y el visor de BMO-X (`bmo-imagen`)
//!    el fichero   `capturas/capNNNNN.bmp`, un bloque de una llamada
//!                 (`escribir_de`) y al disco al cerrar
//! ```
//!
//! # Por que BMP y no PNG ni QOI
//!
//! PNG pide deflate para ESCRIBIR y aqui solo hay inflate; QOI lo abre el
//! visor pero no Windows. BMP ocupa varias veces lo que un PNG y lo abre
//! todo el mundo sin programa: una captura que no se puede mirar en el otro
//! ordenador no le sirve al que la hizo. A 1920x1080 son 6.075 KiB.
//!
//! # Lo que NO hace, dicho
//!
//! * Con una app que se llevo la PANTALLA (`lend_screen`: `ray.bex`) el
//!   escritorio esta dormido y Impr Pant no llega: esa pantalla no es suya.
//! * El recorte no CONGELA la pantalla como el de Windows: se recorta lo que se
//!   ve al SOLTAR. Congelar pediria una copia de la pantalla entera (8 MiB) para
//!   cada recorte; lo que se ve al soltar es lo que se estaba mirando.
//!
//! # El recorte: una capa que se quita y se pone, como el cursor
//!
//! Mientras se arrastra, el cartel de arriba y el borde del rectangulo se pintan
//! ENCIMA de todo, y lo que tapan se guarda (`Capa`). La disciplina es la del
//! cursor por software, y en su orden: al empezar el fotograma se quita el
//! cursor y DESPUES la capa; al acabar se pone la capa y DESPUES el cursor. Al
//! soltar, la capa ya esta quitada: lo que se lee es la pantalla limpia.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene::chrome::Chrome;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{acento, paint_status, INK_BAD};

/// El byte de Impr Pant en la cola de teclas (`KEY_IMPR` del kernel,
/// `TECLA_IMPR` del ABI).
pub(crate) const TECLA_IMPR: u8 = 0x95;

/// La carpeta: la crea el build en el disco de datos (`ejemplos.ps1`).
const CARPETA: &[u8] = b"capturas";

/// El numero de la siguiente. 0 = no se sabe todavia: se mira la carpeta.
static mut SIGUIENTE: u32 = 0;

/// **Hacer la captura.** `ventana`: solo la de delante (Alt + Impr Pant).
pub(crate) fn tomar(dsk: &mut Desktop, p: &bmo::Pantalla, ventana: bool) {
    let (x0, y0, w, h) = if ventana {
        match dsk.win.focus.actual().and_then(|v| caja(dsk, v)) {
            Some(c) => recortar(c, p),
            None => (0, 0, p.ancho, p.alto),
        }
    } else {
        (0, 0, p.ancho, p.alto)
    };
    guardar(dsk, p, x0, y0, w, h);
}

/// **Guardar un rectangulo de la pantalla** en `capturas/`. Lo usan los tres:
/// la pantalla, la ventana y el recorte.
fn guardar(dsk: &mut Desktop, p: &bmo::Pantalla, x0: u32, y0: u32, w: u32, h: u32) {
    let t0 = bmo::ciclos();
    if w == 0 || h == 0 {
        return decir(dsk, p, b"  [captura] nada que capturar: la zona no se ve\n", false);
    }

    // -- El BMP entero en un bloque: cabecera de 54 y las filas a 4 bytes --
    let fila = ((w * 3 + 3) & !3) as u64;
    let total = 54 + fila * h as u64;
    let Some(bloque) = bmo::Memoria::request(total) else {
        return decir(dsk, p, b"  [captura] sin memoria para la imagen\n", false);
    };
    // SAFETY: un bloque de este proceso de `total` bytes; se escribe dentro.
    let b = unsafe { core::slice::from_raw_parts_mut(bloque.base(), total as usize) };
    cabecera(b, w, h, total, fila);
    p.sincronizar_lectura();
    for r in 0..h {
        // De abajo arriba: la primera fila del fichero es la ULTIMA de la imagen.
        let y = y0 + h - 1 - r;
        let mut i = 54 + (r as u64 * fila) as usize;
        for x in x0..x0 + w {
            let c = dsk.save_under.debajo(x, y).unwrap_or_else(|| p.read(x, y));
            b[i] = c as u8;
            b[i + 1] = (c >> 8) as u8;
            b[i + 2] = (c >> 16) as u8;
            i += 3;
        }
    }

    // -- El nombre y el disco --
    let n = siguiente();
    let mut ruta = *b"capturas/cap00000.bmp";
    let mut d = n;
    for k in (12..17).rev() {
        ruta[k] = b'0' + (d % 10) as u8;
        d /= 10;
    }
    let guardada = match bmo::Archivo::create(&ruta) {
        Ok(a) => a.escribir_de(&bloque, 0, total) == total && a.close(),
        Err(_) => false,
    };
    drop(bloque);
    let ms = (bmo::ciclos().saturating_sub(t0)) * 1000 / bmo::info(bmo::INFO_TSC_HZ).max(1);

    let mut t = Linea::new();
    if !guardada {
        t.pon(b"  [captura] NO se pudo guardar ");
        t.pon(&ruta);
        t.pon(b" (falta la carpeta capturas/ o no cabe; ver CABINA)\n");
        return decir(dsk, p, t.bytes(), false);
    }
    unsafe { SIGUIENTE = n + 1 };
    t.pon(b"  [captura] ");
    t.pon(&ruta);
    t.pon(b"  ");
    t.num(w as u64);
    t.pon(b"x");
    t.num(h as u64);
    t.pon(b"  ");
    t.num(total / 1024);
    t.pon(b" KiB  en ");
    t.num(ms);
    t.pon(b" ms\n");
    decir(dsk, p, t.bytes(), true);
}

/// Un renglon para la salida de Ejecutar, sin asignar memoria.
struct Linea {
    b: [u8; 112],
    n: usize,
}

impl Linea {
    fn new() -> Self {
        Self { b: [0; 112], n: 0 }
    }
    fn pon(&mut self, s: &[u8]) {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn num(&mut self, v: u64) {
        let mut d = [0u8; 10];
        let k = crate::text::decimal(v, &mut d);
        self.pon(&d[..k]);
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// La cabecera de un BMP de 24 bits sin comprimir, de abajo arriba.
fn cabecera(b: &mut [u8], w: u32, h: u32, total: u64, fila: u64) {
    let mut pon32 = |i: usize, v: u32| b[i..i + 4].copy_from_slice(&v.to_le_bytes());
    pon32(2, total as u32);
    pon32(6, 0);
    pon32(10, 54); // donde empiezan los pixeles
    pon32(14, 40); // BITMAPINFOHEADER
    pon32(18, w);
    pon32(22, h); // positivo = de abajo arriba
    pon32(30, 0); // BI_RGB: sin comprimir
    pon32(34, (fila * h as u64) as u32);
    pon32(38, 2835); // 72 ppp, en pixeles por metro
    pon32(42, 2835);
    pon32(46, 0);
    pon32(50, 0);
    b[0] = b'B';
    b[1] = b'M';
    b[26..28].copy_from_slice(&1u16.to_le_bytes()); // planos
    b[28..30].copy_from_slice(&24u16.to_le_bytes()); // bits por pixel
}

/// **El numero de la siguiente**: la primera vez se mira la carpeta y se sigue
/// por la mas alta que haya. Asi no se pisa una captura de otra sesion.
fn siguiente() -> u32 {
    let n = unsafe { SIGUIENTE };
    if n != 0 {
        return n;
    }
    let mut mayor = 0u32;
    if let Ok(dir) = bmo::Directorio::open(CARPETA) {
        while let Some(e) = dir.next() {
            let nom = &e.name;
            if e.es_dir || !nom[..3].eq_ignore_ascii_case(b"CAP") {
                continue;
            }
            let mut v = 0u32;
            if nom[3..8].iter().all(|c| c.is_ascii_digit()) {
                for &c in &nom[3..8] {
                    v = v * 10 + (c - b'0') as u32;
                }
                mayor = mayor.max(v);
            }
        }
    }
    (mayor + 1).min(99_999)
}

/// Donde esta una ventana, si se ve. Una app a pantalla completa es la pantalla.
fn caja(dsk: &mut Desktop, v: Ventana) -> Option<(u32, u32, u32, u32)> {
    let de = |c: &Chrome| (!c.minimized).then_some((c.x, c.y, c.width, c.height));
    match v {
        Ventana::Run => dsk
            .win
            .visible
            .then_some((dsk.run_box.x, dsk.run_box.y, dsk.run_box.w(), dsk.run_box.h())),
        Ventana::Data if dsk.win.data_open => de(&dsk.win.data.chrome),
        Ventana::Cabina if dsk.win.cabina_open => de(&dsk.win.cabina.chrome),
        Ventana::Estructura if dsk.win.estructura_open => de(&dsk.win.estructura.chrome),
        Ventana::Cpu if dsk.win.cpu_open => de(&dsk.win.cpu.chrome),
        Ventana::Mem if dsk.win.mem_open => de(&dsk.win.mem.chrome),
        Ventana::Sound if dsk.win.sound_open => de(&dsk.win.sound.chrome),
        Ventana::App(i) => dsk.table.get_mut(i as usize).and_then(|s| {
            if s.chrome.is_fullscreen() {
                None
            } else {
                de(&s.chrome)
            }
        }),
        _ => None,
    }
}

/// Lo que de la ventana cae dentro de la pantalla.
fn recortar((x, y, w, h): (u32, u32, u32, u32), p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    let x = x.min(p.ancho);
    let y = y.min(p.alto);
    (x, y, w.min(p.ancho - x), h.min(p.alto - y))
}

/// Lo dice en la salida de Ejecutar y, si se ve, en su linea de estado.
fn decir(dsk: &mut Desktop, p: &bmo::Pantalla, texto: &[u8], bien: bool) {
    dsk.out.grid.with_ink(if bien { INK_GOOD } else { INK_ERR });
    dsk.out.grid.text(texto);
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.tick.repaint_field = true;
    if dsk.win.visible {
        if bien {
            paint_status(p, &dsk.run_box, "captura guardada en capturas/", acento());
        } else {
            paint_status(p, &dsk.run_box, "la captura no se guardo", INK_BAD);
        }
    }
}

// ===================================================================
//  EL RECORTE (Ctrl+Shift+S)
// ===================================================================

/// Lo que dura un recorte: si esta en marcha, donde se pulso y donde va.
struct Recorte {
    activo: bool,
    ancla: Option<(u32, u32)>,
    punta: (u32, u32),
}

static mut RECORTE: Recorte = Recorte { activo: false, ancla: None, punta: (0, 0) };

/// Se esta recortando? Mientras si, el raton y las teclas son del recorte.
pub(crate) fn recortando() -> bool {
    unsafe { (*core::ptr::addr_of!(RECORTE)).activo }
}

/// **Ctrl+Shift+S**: empieza. El cartel sale en el fotograma siguiente.
pub(crate) fn empezar(dsk: &mut Desktop) {
    unsafe {
        *core::ptr::addr_of_mut!(RECORTE) = Recorte { activo: true, ancla: None, punta: (0, 0) };
    }
    dsk.tick.actividad = true;
}

/// ESC: se deja como estaba. La capa se quita al empezar el fotograma, asi que
/// basta con no volver a ponerla.
pub(crate) fn cancelar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    unsafe { (*core::ptr::addr_of_mut!(RECORTE)).activo = false };
    capa_quitar(p);
    decir(dsk, p, b"  [captura] recorte cancelado\n", false);
}

/// **El raton, mientras se recorta.** Pulsar ancla una esquina, arrastrar
/// mueve la otra, soltar guarda. `antes`: el boton en el fotograma anterior.
pub(crate) fn raton(dsk: &mut Desktop, p: &bmo::Pantalla, x: u32, y: u32, boton: bool, antes: bool) {
    let r = unsafe { &mut *core::ptr::addr_of_mut!(RECORTE) };
    let (x, y) = (x.min(p.ancho.saturating_sub(1)), y.min(p.alto.saturating_sub(1)));
    if boton && !antes {
        r.ancla = Some((x, y));
        r.punta = (x, y);
    } else if boton {
        r.punta = (x, y);
    } else if antes {
        let Some((ax, ay)) = r.ancla else { return };
        r.activo = false;
        // La capa se quito al empezar este fotograma (se solto el boton, asi que
        // hubo entrada); por si acaso, otra vez: es idempotente.
        capa_quitar(p);
        let (x0, y0) = (ax.min(x), ay.min(y));
        let (w, h) = (ax.max(x) - x0 + 1, ay.max(y) - y0 + 1);
        if w < 4 || h < 4 {
            return decir(dsk, p, b"  [captura] recorte demasiado chico: arrastra un rectangulo\n", false);
        }
        guardar(dsk, p, x0, y0, w, h);
    }
}

// -- La capa: el cartel y el borde, con lo que tapan guardado ---------------

const BORDE: u32 = 2;
const CARTEL_W: u32 = 560;
const CARTEL_H: u32 = 28;
/// Lo que tapan el cartel y un borde de 2 px alrededor de una pantalla 4K.
const GUARDADO: usize = (CARTEL_W * CARTEL_H) as usize + (2 * (3840 + 2160) * BORDE) as usize;

struct Capa {
    puesta: bool,
    cajas: [(u32, u32, u32, u32); 5],
    n: usize,
    px: [u32; GUARDADO],
}

static mut CAPA: Capa = Capa { puesta: false, cajas: [(0, 0, 0, 0); 5], n: 0, px: [0; GUARDADO] };

/// **Quita la capa**: devuelve lo que tapaba. Al PRINCIPIO del fotograma,
/// DESPUES de quitar el cursor. Si no estaba puesta no hace nada.
pub(crate) fn capa_quitar(p: &bmo::Pantalla) {
    let c = unsafe { &mut *core::ptr::addr_of_mut!(CAPA) };
    if !c.puesta {
        return;
    }
    let mut k = 0usize;
    for &(x, y, w, h) in &c.cajas[..c.n] {
        p.marcar(x, y, w, h);
        for dy in 0..h {
            for dx in 0..w {
                p.punto_ya_marcado(x + dx, y + dy, c.px[k]);
                k += 1;
            }
        }
    }
    c.puesta = false;
}

/// **Pone la capa** si se esta recortando: guarda lo que va a tapar y pinta el
/// cartel y el borde. Al FINAL del fotograma, ANTES de poner el cursor.
pub(crate) fn capa_poner(p: &bmo::Pantalla) {
    let c = unsafe { &mut *core::ptr::addr_of_mut!(CAPA) };
    let r = unsafe { &*core::ptr::addr_of!(RECORTE) };
    if !r.activo || c.puesta {
        return;
    }
    // Las cajas: el cartel arriba en el centro y, si hay ancla, los 4 bordes.
    let cw = CARTEL_W.min(p.ancho);
    let cx = (p.ancho - cw) / 2;
    c.cajas[0] = (cx, 12, cw, CARTEL_H);
    c.n = 1;
    let mut medida = None;
    if let Some((ax, ay)) = r.ancla {
        let (px_, py_) = r.punta;
        let (x0, y0) = (ax.min(px_), ay.min(py_));
        let (w, h) = (ax.max(px_) - x0 + 1, ay.max(py_) - y0 + 1);
        medida = Some((w, h));
        if w > 2 * BORDE && h > 2 * BORDE {
            c.cajas[1] = (x0, y0, w, BORDE);
            c.cajas[2] = (x0, y0 + h - BORDE, w, BORDE);
            c.cajas[3] = (x0, y0 + BORDE, BORDE, h - 2 * BORDE);
            c.cajas[4] = (x0 + w - BORDE, y0 + BORDE, BORDE, h - 2 * BORDE);
            c.n = 5;
        }
    }
    // Guardar lo de debajo, todo antes de pintar nada: el cartel y un borde
    // pueden cruzarse, y guardar despues de pintar el primero guardaria el borde.
    p.sincronizar_lectura();
    let mut k = 0usize;
    for &(x, y, w, h) in &c.cajas[..c.n] {
        for dy in 0..h {
            for dx in 0..w {
                if k < GUARDADO {
                    c.px[k] = p.read(x + dx, y + dy);
                }
                k += 1;
            }
        }
    }
    if k > GUARDADO {
        // No cabe (una pantalla mas grande que 4K): no se pinta nada antes que
        // pintar algo que luego no se sabria devolver.
        c.n = 0;
        return;
    }
    c.puesta = true;
    for &(x, y, w, h) in &c.cajas[1..c.n] {
        p.rect(x, y, w, h, acento());
    }
    let (x, y, w, h) = c.cajas[0];
    p.rect(x, y, w, h, acento());
    p.rect(x + 1, y + 1, w - 2, h - 2, crate::scene::estilo::estilo().barra_fondo);
    let mut t = Linea::new();
    match medida {
        None => t.pon(b"RECORTE   arrastra con el raton   -   ESC cancela"),
        Some((mw, mh)) => {
            t.pon(b"RECORTE   ");
            t.num(mw as u64);
            t.pon(b" x ");
            t.num(mh as u64);
            t.pon(b"   suelta para guardar   -   ESC cancela");
        }
    }
    let tw = t.bytes().len() as u32 * bmo::GLIFO_ANCHO;
    p.texto_bytes(x + w.saturating_sub(tw) / 2, y + (h - bmo::GLIFO_ALTO) / 2, t.bytes(), crate::scene::INK);
}
