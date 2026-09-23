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
//!    `captura`          lo mismo desde Ejecutar, para un teclado sin la tecla
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
//! * No recorta con el raton (el Win+Shift+S de Windows). Pantalla o ventana.

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
    let t0 = bmo::ciclos();
    let (x0, y0, w, h) = if ventana {
        match dsk.win.focus.actual().and_then(|v| caja(dsk, v)) {
            Some(c) => recortar(c, p),
            None => (0, 0, p.ancho, p.alto),
        }
    } else {
        (0, 0, p.ancho, p.alto)
    };
    if w == 0 || h == 0 {
        return decir(dsk, p, b"  [captura] nada que capturar: la ventana no se ve\n", false);
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
