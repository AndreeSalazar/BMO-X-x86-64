//! **ABRIR Y CERRAR: cuando y de quien** (2026-09-25). Pedido: *"dale todo
//! animado, abrir y cerrar ventanas"*.
//!
//! [consumo] LATE      solo mientras dura una transicion ([`anima`], que
//!                     mira el reloj); en reposo, mirar ~11 cajas por
//!                     fotograma que ya pinta, sin puertas (L6h)
//!
//! No se engancha en cada sitio que abre o cierra una ventana --F7, F10, un
//! clic en la barra, una app que nace, una que muere, minimizar...-- porque
//! son muchos y el que falte seria la ventana que se abre sin efecto. Se mira
//! lo que SE VE: en cada fotograma que pinta, que ventanas tienen caja
//! (`foco::caja`) y cuales la tenian el anterior. La que aparece, ABRE; la
//! que desaparece, CIERRA con la caja que tenia.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene::surface::MAX;
use crate::scene::transicion::{self as tr, DURA_MS, HUECOS};

const FOTOGRAMA_MS: u64 = 33;
/// Las del sistema y las apps.
const VENTANAS: usize = Ventana::TODAS.len() + MAX;

type Caja = (u32, u32, u32, u32);

#[derive(Clone, Copy)]
struct Viva {
    caja: Caja,
    abre: bool,
    desde: u64,
}

struct Estado {
    por_ms: u64,
    /// Lo que se vio el fotograma anterior, por ventana.
    antes: [Option<Caja>; VENTANAS],
    /// La primera vez no se anima nada: lo abierto al arrancar ya estaba.
    mirado: bool,
    vivas: [Option<Viva>; HUECOS],
    pintado: u64,
}

static mut ESTADO: Estado = Estado { por_ms: 0, antes: [None; VENTANAS], mirado: false, vivas: [None; HUECOS], pintado: 0 };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn ventana(k: usize) -> Ventana {
    if k < Ventana::TODAS.len() {
        Ventana::TODAS[k]
    } else {
        Ventana::App((k - Ventana::TODAS.len()) as u8)
    }
}

/// Una caja que se anima: ni a pantalla completa ni ridicula.
fn animable(c: Caja, p: &bmo::Pantalla) -> bool {
    c.2 >= 24 && c.3 >= 24 && c.2 < p.ancho && c.3 < p.alto
}

fn nacer(e: &mut Estado, caja: Caja, abre: bool, ahora: u64) {
    // En el hueco libre, o en el mas viejo.
    let k = (0..HUECOS).find(|&k| e.vivas[k].is_none()).unwrap_or_else(|| {
        (0..HUECOS).min_by_key(|&k| e.vivas[k].map_or(0, |v| v.desde)).unwrap_or(0)
    });
    e.vivas[k] = Some(Viva { caja, abre, desde: ahora });
}

/// **Las transiciones de este fotograma**: mira que abrio y que cerro, y
/// pinta las vivas. Al FINAL del fotograma que pinta, debajo del destello.
pub(crate) fn poner(dsk: &Desktop, p: &bmo::Pantalla, tapado: bool) {
    let e = estado();
    let ahora = bmo::ciclos();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    }
    for k in 0..VENTANAS {
        let hoy = crate::desktop::foco::caja(dsk, ventana(k));
        if e.mirado {
            match (e.antes[k], hoy) {
                (None, Some(c)) if animable(c, p) => nacer(e, c, true, ahora),
                (Some(c), None) if animable(c, p) => nacer(e, c, false, ahora),
                _ => {}
            }
        }
        e.antes[k] = hoy;
    }
    e.mirado = true;
    if tapado {
        return;
    }
    for k in 0..HUECOS {
        if let Some(v) = e.vivas[k] {
            let ms = ahora.wrapping_sub(v.desde) / e.por_ms;
            if ms >= DURA_MS {
                e.vivas[k] = None;
                continue;
            }
            e.pintado = ahora;
            tr::poner(p, k, v.caja, v.abre, ms);
        }
    }
}

/// **Pide fotograma** mientras vive una transicion. Solo lee el reloj.
pub(crate) fn anima() -> bool {
    let e = estado();
    e.vivas.iter().any(|v| v.is_some()) && bmo::ciclos().wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}
