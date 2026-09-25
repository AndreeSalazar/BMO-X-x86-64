//! **FRAPS-X** (2026-09-25): el contador de FPS encima de todo, y el banco de
//! pruebas. Pedido del propietario: *"dale al FRAPS-X, el contador de FPS y
//! las capturas"*.
//!
//! [consumo] NADA      apagado no mide ni pinta. Encendido: una lectura de una
//!                     cabecera por vuelta y UN repintado de su esquina por
//!                     segundo ([`anima`]), que es cuando cambia el numero (L6h)
//!
//! ```text
//!    Ctrl+Shift+F    el contador: arriba-izq -> arriba-der -> abajo-der ->
//!                    abajo-izq -> fuera (el F12 de FRAPS; aqui F12 es Datos)
//!    Ctrl+Shift+B    el BANCO: empieza; otra vez, para y lo guarda en
//!                    `capturas/banNNNNN.csv` (el F11 de FRAPS)
//!    `fraps`         lo mismo desde Ejecutar: `fraps`, `fraps banco`
//!    Impr Pant       la captura, que ya existia (`desktop::captura`)
//! ```
//!
//! # Que se mide
//!
//! Lo de DELANTE: la app a pantalla completa si la hay; si no, la app con el
//! foco; si no, **el escritorio mismo**. Una app se mide por la SECUENCIA de
//! su superficie --lo que ella publica, que es lo que FRAPS contaba de un
//! juego--; el escritorio, por las vueltas que de verdad pintan. La cuenta y
//! los bajos del 1 % son de `bmo-fraps`, probado en el anfitrion.
//!
//! # Como se pone
//!
//! Como el globo y el brillo: se guarda lo que tapa al FINAL del fotograma y
//! se devuelve al PRINCIPIO del siguiente (`globo::quitar_capas`). Y SI se
//! pinta con una app a pantalla completa: es justo donde hace falta.

use bmo_fraps::{Banco, Medidor, Segundo};
use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene::{acento, paint_status, INK, INK_BAD, INK_DIM};

/// A cuanto de la orilla de la pantalla.
const MARGEN: u32 = 14;
const GUARDADO: usize = crate::scene::fraps::GUARDADO;

/// A quien se mide.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Que {
    Nadie,
    Escritorio,
    /// El hueco de la tabla y su tid: el tid distingue una app que se fue de
    /// la que ocupo su hueco despues.
    App(usize, u32),
}

struct Estado {
    /// 0 = apagado; 1..=4, la esquina.
    esquina: u8,
    hz: u64,
    que: Que,
    seq: u32,
    /// Vueltas del escritorio que pintaron desde la ultima mirada.
    pintadas: u32,
    medidor: Medidor,
    ultimo: Option<Segundo>,
    /// Hay un numero nuevo que pintar.
    cambio: bool,
    banco: Option<Banco>,
    /// A quien media el banco, para el CSV: `tid 7` o `escritorio`.
    banco_que: [u8; 16],
    banco_que_n: usize,
    // La capa.
    puesta: bool,
    caja: (u32, u32, u32, u32),
    px: [u32; GUARDADO],
    siguiente_csv: u32,
}

static mut ESTADO: Estado = Estado {
    esquina: 0,
    hz: 0,
    que: Que::Nadie,
    seq: 0,
    pintadas: 0,
    medidor: Medidor::nuevo(),
    ultimo: None,
    cambio: false,
    banco: None,
    banco_que: [0; 16],
    banco_que_n: 0,
    puesta: false,
    caja: (0, 0, 0, 0),
    px: [0; GUARDADO],
    siguiente_csv: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el bucle.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn hz() -> u64 {
    let e = estado();
    if e.hz == 0 {
        e.hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    }
    e.hz
}

/// Mide algo? Apagado y sin banco, no se toca ni una cabecera.
fn midiendo() -> bool {
    let e = estado();
    e.esquina != 0 || e.banco.is_some()
}

/// **Ctrl+Shift+F**: la siguiente esquina, o fuera.
pub(crate) fn alternar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let e = estado();
    e.esquina = (e.esquina + 1) % 5;
    e.cambio = true;
    dsk.tick.actividad = true;
    let dice = match e.esquina {
        0 => "FRAPS-X fuera (Ctrl+Shift+F lo vuelve a poner)",
        1 => "FRAPS-X arriba a la izquierda (Ctrl+Shift+F lo mueve)",
        2 => "FRAPS-X arriba a la derecha",
        3 => "FRAPS-X abajo a la derecha",
        _ => "FRAPS-X abajo a la izquierda",
    };
    if dsk.win.visible {
        paint_status(p, &dsk.run_box, dice, acento());
    }
}

/// **Ctrl+Shift+B**: empieza el banco, o lo para y lo guarda.
pub(crate) fn banco(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let e = estado();
    let ahora = bmo::ciclos();
    match e.banco.take() {
        None => {
            e.banco = Some(Banco::nuevo(ahora));
            e.banco_que_n = nombre(e.que, &mut e.banco_que);
            // Si el contador estaba fuera, se pone: un banco que no se ve
            // correr no se sabe si esta corriendo.
            if e.esquina == 0 {
                e.esquina = 1;
            }
            e.cambio = true;
            dsk.tick.actividad = true;
            if dsk.win.visible {
                paint_status(p, &dsk.run_box, "BANCO en marcha: Ctrl+Shift+B lo para y lo guarda", acento());
            }
        }
        Some(b) => {
            e.cambio = true;
            dsk.tick.actividad = true;
            let r = b.resumen(ahora, hz());
            guardar(dsk, p, &r);
        }
    }
}

/// Como se llama lo que se mide, para el CSV y la caja.
fn nombre(q: Que, dst: &mut [u8; 16]) -> usize {
    let mut n = 0;
    let mut pon = |s: &[u8]| {
        for &c in s {
            if n < dst.len() {
                dst[n] = c;
                n += 1;
            }
        }
    };
    match q {
        Que::Nadie => pon(b"nada"),
        Que::Escritorio => pon(b"escritorio"),
        Que::App(_, tid) => {
            pon(b"tid ");
            let mut d = [0u8; 10];
            let k = crate::text::decimal(tid as u64, &mut d);
            pon(&d[..k]);
        }
    }
    n
}

/// **El banco, a `capturas/banNNNNN.csv`** y dicho en Ejecutar.
fn guardar(dsk: &mut Desktop, p: &bmo::Pantalla, r: &bmo_fraps::Resumen) {
    let e = estado();
    let mut fila = [0u8; 160];
    let nf = bmo_fraps::fila_csv(&e.banco_que[..e.banco_que_n], r, &mut fila);
    let n = siguiente_csv();
    let mut ruta = *b"capturas/ban00000.csv";
    let mut d = n;
    for k in (12..17).rev() {
        ruta[k] = b'0' + (d % 10) as u8;
        d /= 10;
    }
    let bien = match bmo::Archivo::create(&ruta) {
        Ok(a) => {
            let c = bmo_fraps::CSV_CABECERA;
            a.write(c) == c.len() && a.write(&fila[..nf]) == nf && a.close()
        }
        Err(_) => false,
    };
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { crate::scene::output::INK_GOOD } else { crate::scene::output::INK_ERR });
    g.text(b"  [fraps] BANCO ");
    g.text(&e.banco_que[..e.banco_que_n]);
    g.text(b": ");
    g.dec(r.ms / 1000);
    g.text(b" s, ");
    g.dec(r.fotogramas);
    g.text(b" fotogramas  min ");
    decimas(g, r.min10);
    g.text(b"  media ");
    decimas(g, r.media10);
    g.text(b"  max ");
    decimas(g, r.max10);
    g.text(b"  1% bajo ");
    decimas(g, r.bajo1_10);
    g.text(b"  0,1% bajo ");
    decimas(g, r.bajo01_10);
    g.text(b"\n");
    if bien {
        e.siguiente_csv = n + 1;
        g.text(b"  [fraps] guardado en ");
        g.text(&ruta);
        g.text(b"\n");
    } else {
        g.text(b"  [fraps] NO se pudo guardar el CSV (falta capturas/ o no cabe)\n");
    }
    g.with_ink(crate::scene::output::INK_PLAIN);
    dsk.tick.repaint_field = true;
    if dsk.win.visible {
        paint_status(p, &dsk.run_box, if bien { "banco guardado en capturas/" } else { "el banco no se guardo" }, if bien { acento() } else { INK_BAD });
    }
}

fn decimas(g: &mut crate::scene::output::Output, v: u32) {
    g.dec((v / 10) as u64);
    g.byte(b'.');
    g.byte(b'0' + (v % 10) as u8);
}

/// El numero del siguiente CSV: se sigue por el mas alto que haya.
fn siguiente_csv() -> u32 {
    let e = estado();
    if e.siguiente_csv != 0 {
        return e.siguiente_csv;
    }
    let mut mayor = 0u32;
    if let Ok(dir) = bmo::Directorio::open(b"capturas") {
        while let Some(x) = dir.next() {
            let nom = &x.name;
            if x.es_dir || !nom[..3].eq_ignore_ascii_case(b"BAN") || !nom[3..8].iter().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let v = nom[3..8].iter().fold(0u32, |v, &c| v * 10 + (c - b'0') as u32);
            mayor = mayor.max(v);
        }
    }
    (mayor + 1).min(99_999)
}

/// **El escritorio pinto una vuelta.** Lo llama `paint` al final de cada
/// fotograma que pinta.
pub(crate) fn pinto() {
    let e = estado();
    e.pintadas = e.pintadas.saturating_add(1);
}

/// A quien toca medir ahora: la de pantalla completa, la del foco, o el
/// escritorio.
fn delante(dsk: &Desktop) -> Que {
    for i in 0..crate::scene::surface::MAX {
        if let Some(s) = dsk.table.get(i) {
            if s.chrome.is_fullscreen() {
                return Que::App(i, s.tid);
            }
        }
    }
    if let Some(Ventana::App(i)) = dsk.win.focus.actual() {
        if let Some(s) = dsk.table.get(i as usize) {
            if !s.chrome.minimized {
                return Que::App(i as usize, s.tid);
            }
        }
    }
    Que::Escritorio
}

/// **Una vuelta del bucle**: cuantos fotogramas nuevos tiene lo de delante,
/// y al medidor y al banco. Apagado, no hace nada.
pub(crate) fn vuelta(dsk: &Desktop) {
    if !midiendo() {
        return;
    }
    let e = estado();
    let ahora = bmo::ciclos();
    let hz = hz();
    let que = delante(dsk);
    if que != e.que {
        // Cambio lo de delante: se empieza de cero, o el hueco entre uno y
        // otro contaria como un tiron de la nueva.
        e.que = que;
        e.medidor.reiniciar();
        e.ultimo = None;
        e.cambio = true;
        e.pintadas = 0;
        e.seq = match que {
            Que::App(i, _) => dsk.table.get(i).and_then(|s| s.secuencia()).unwrap_or(0),
            _ => 0,
        };
        e.medidor.fotogramas(0, ahora, hz);
        return;
    }
    let n = match que {
        Que::Nadie => 0,
        Que::Escritorio => core::mem::take(&mut e.pintadas),
        Que::App(i, _) => match dsk.table.get(i).and_then(|s| s.secuencia()) {
            Some(s) => {
                let d = s.wrapping_sub(e.seq);
                e.seq = s;
                // Un salto absurdo es un contador reiniciado, no mil fotogramas.
                if d > bmo_ritmo::SALTO_MAXIMO { 0 } else { d }
            }
            None => 0,
        },
    };
    if let Some(b) = e.banco.as_mut() {
        b.fotogramas(n, ahora, hz);
    }
    if let Some(s) = e.medidor.fotogramas(n, ahora, hz) {
        e.ultimo = Some(s);
        e.cambio = true;
        if let Some(b) = e.banco.as_mut() {
            b.segundo(s);
        }
    }
}

/// **Pide fotograma** cuando hay un numero nuevo que mostrar: uno por segundo.
pub(crate) fn anima() -> bool {
    let e = estado();
    (e.esquina != 0 || e.puesta) && e.cambio
}

/// **Quita el contador**: devuelve lo que tapaba. Al PRINCIPIO del fotograma.
pub(crate) fn quitar(p: &bmo::Pantalla) {
    let e = estado();
    if !e.puesta {
        return;
    }
    let (x, y, w, h) = e.caja;
    p.marcar(x, y, w, h);
    let mut k = 0usize;
    for dy in 0..h {
        for dx in 0..w {
            p.punto_ya_marcado(x + dx, y + dy, e.px[k]);
            k += 1;
        }
    }
    e.puesta = false;
}

/// **Pone el contador** en su esquina. Al FINAL del fotograma, con las otras
/// capas. La cara la pinta `scene::fraps`.
pub(crate) fn poner(p: &bmo::Pantalla) {
    let e = estado();
    e.cambio = false;
    if e.esquina == 0 || e.puesta {
        return;
    }
    let w = crate::scene::fraps::ANCHO;
    let h = crate::scene::fraps::alto(e.banco.is_some());
    if p.ancho < w + 2 * MARGEN || p.alto < h + 2 * MARGEN {
        return;
    }
    let (x, y) = match e.esquina {
        1 => (MARGEN, MARGEN),
        2 => (p.ancho - w - MARGEN, MARGEN),
        3 => (p.ancho - w - MARGEN, p.alto - h - MARGEN),
        _ => (MARGEN, p.alto - h - MARGEN),
    };
    // Guardar TODO antes de pintar nada.
    p.sincronizar_lectura();
    let mut k = 0usize;
    for dy in 0..h {
        for dx in 0..w {
            e.px[k] = p.read(x + dx, y + dy);
            k += 1;
        }
    }
    e.caja = (x, y, w, h);
    e.puesta = true;
    let mut nom = [0u8; 16];
    let n = nombre(e.que, &mut nom);
    let v = crate::scene::fraps::Vista {
        fps10: e.ultimo.map(|s| s.fps10),
        que: &nom[..n],
        ultimo_us: e.ultimo.map_or(0, |s| s.ultimo_us),
        peor_us: e.ultimo.map_or(0, |s| s.peor_us),
        banco_s: e.banco.as_ref().map(|b| b.segundos()),
        tinta: (INK, INK_DIM, INK_BAD, acento()),
    };
    crate::scene::fraps::pintar(p, e.caja, &e.px[..(w * h) as usize], &v);
}
