//! **EL MOSAICO** (HUD 4, 2026-09-22): ninguna ventana tapa a otra, y no se
//! ordena a mano.
//!
//! [consumo] NADA      una comparacion por vuelta (QUE ventanas hay); solo
//!                     recoloca cuando eso CAMBIA, y lo cambia una mano (L6h)
//!
//! El motivo, uno: **ninguna ventana tapa a otra, y no hay que ordenarlas.**
//! Super+T lo enciende y lo apaga.
//!
//! # El reparto: MAESTRO y PILA
//!
//! ```text
//!    una ventana        el area util entera
//!    dos o mas          la primera a la izquierda (la mitad), las demas
//!                       apiladas a la derecha, con HUECOS entre todas
//! ```
//!
//! Es el de dwm y el `master` de Hyprland. Se eligio frente al `dwindle`
//! (partir siempre la ultima en dos) porque es el que se PREDICE sin mirar:
//! la grande a la izquierda, lo demas a la derecha. La primera es la app si hay
//! una --lo que se quiere grande suele ser el juego o la ventana de trabajo--,
//! y si no, Ejecutar, que es la casa.
//!
//! # Lo que NO hace, dicho
//!
//! * Una ventana con minimo mayor que su hueco (Ejecutar pide 760x428) se
//!   queda en su minimo y puede asomar sobre la de al lado: se prefiere eso a
//!   dejarla inservible.
//! * Arrastrar una ventana con el mosaico puesto la saca del sitio hasta la
//!   siguiente vez que cambie que ventanas hay. No se pelea con la mano.
//! * Una app a pantalla completa no entra: esta encima de todo por definicion.

use bmo_userland as bmo;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::desktop::{Desktop, Ventana};
use crate::scene::chrome::{area_util, Chrome, HUECO};
use crate::scene::surface::MAX;

static ENCENDIDO: AtomicBool = AtomicBool::new(false);
/// Que ventanas habia la ultima vez que se coloco: un bit por id. `u32::MAX`
/// obliga a colocar en la vuelta siguiente.
static FIRMA: AtomicU32 = AtomicU32::new(u32::MAX);

pub(crate) fn encendido() -> bool {
    ENCENDIDO.load(Ordering::Relaxed)
}

/// **Super+T.** Encenderlo coloca ya; apagarlo deja cada ventana donde esta.
pub(crate) fn alternar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let on = !encendido();
    ENCENDIDO.store(on, Ordering::Relaxed);
    FIRMA.store(u32::MAX, Ordering::Relaxed);
    if on {
        seguir(dsk, p);
    }
}

/// El area util cambio (la barra lateral): la vuelta siguiente reparte otra vez.
pub(crate) fn recolocar() {
    FIRMA.store(u32::MAX, Ordering::Relaxed);
}

/// Si el mosaico esta puesto y las ventanas que hay CAMBIARON, se recolocan.
/// Lo llama el bucle antes del borde de foco.
pub(crate) fn seguir(dsk: &mut Desktop, p: &bmo::Pantalla) {
    if !encendido() || dsk.table.alguna_a_pantalla_completa() {
        return;
    }
    let (lista, n) = cuales(dsk);
    let mut firma = 0u32;
    for v in &lista[..n] {
        firma |= 1 << (v.id() as u32).min(31);
    }
    if firma == FIRMA.load(Ordering::Relaxed) {
        return;
    }
    FIRMA.store(firma, Ordering::Relaxed);
    if n == 0 {
        return;
    }
    let (ax, ay, aw, ah) = area_util(p);
    for (k, &v) in lista[..n].iter().enumerate() {
        let (x, y, w, h) = if n == 1 {
            (ax, ay, aw, ah)
        } else {
            let mw = aw.saturating_sub(HUECO) / 2;
            if k == 0 {
                (ax, ay, mw, ah)
            } else {
                let pila = (n - 1) as u32;
                let alto = ah.saturating_sub(HUECO * (pila - 1)) / pila;
                let i = (k - 1) as u32;
                (ax + mw + HUECO, ay + i * (alto + HUECO), aw - mw - HUECO, alto)
            }
        };
        if let Some(c) = marco(dsk, v) {
            c.colocar(x, y, w, h);
        }
        match v {
            Ventana::Run => dsk.run_box.relayout(),
            Ventana::Data => dsk.win.data.relayout(),
            Ventana::App(i) => {
                if let Some(s) = dsk.table.get_mut(i as usize) {
                    s.repaint_all();
                }
            }
            _ => {}
        }
    }
    crate::repintar_escritorio(p, dsk, "mosaico");
    // Las del sistema las repinta el borde de foco, en su orden.
    dsk.win.foco_pintado = None;
}

/// Las que entran en el mosaico, en su orden: primero las apps, luego las del
/// sistema. Abiertas y sin minimizar.
fn cuales(dsk: &mut Desktop) -> ([Ventana; 16], usize) {
    let mut l = [Ventana::Run; 16];
    let mut n = 0;
    for i in 0..MAX {
        let v = Ventana::App(i as u8);
        let ok = match dsk.table.get_mut(i) {
            Some(s) => !s.chrome.minimized && !s.chrome.is_fullscreen(),
            None => false,
        };
        if ok && n < l.len() {
            l[n] = v;
            n += 1;
        }
    }
    for v in Ventana::TODAS {
        if !dsk.win.abierta(v) {
            continue;
        }
        let quieta = marco(dsk, v).map(|c| !c.minimized).unwrap_or(false);
        if quieta && n < l.len() {
            l[n] = v;
            n += 1;
        }
    }
    (l, n)
}

/// El marco de una ventana, para moverlo.
fn marco(dsk: &mut Desktop, v: Ventana) -> Option<&mut Chrome> {
    match v {
        Ventana::Run => Some(&mut dsk.run_box.chrome),
        Ventana::Data => Some(&mut dsk.win.data.chrome),
        Ventana::Cabina => Some(&mut dsk.win.cabina.chrome),
        Ventana::Estructura => Some(&mut dsk.win.estructura.chrome),
        Ventana::Cpu => Some(&mut dsk.win.cpu.chrome),
        Ventana::Mem => Some(&mut dsk.win.mem.chrome),
        Ventana::Sound => Some(&mut dsk.win.sound.chrome),
        Ventana::App(i) => dsk.table.get_mut(i as usize).map(|s| &mut s.chrome),
    }
}
