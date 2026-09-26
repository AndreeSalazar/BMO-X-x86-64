//! **C2: LOS RELOJES DE LA 3060** -- `gpu relojes` y `gpu relojes off`, y la
//! fila `relojes` de `gpu salud`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea; la subida dura
//!                     `control::SUBIDA_SEGUNDOS` y baja sola
//!
//! Pedido del propietario (25-09): *"dale a subir los relojes de la 3060"*.
//!
//! # Lo que se puede, dicho con la historia delante
//!
//! Desde Maxwell 2 los relojes solo los mueve firmware FIRMADO, y por eso
//! nouveau se quedo ocho anios en los de arranque. Con el GSP-RM vivo, quien
//! los mueve es el; BMO-X solo le PIDE `PERF_BOOST` (lo unico que NVIDIA
//! publica para esto). Y como NVIDIA no publica como leer los MHz, lo que dice
//! si funciono es el mismo fotograma de `gpu pantalla`, medido antes y
//! despues. Ver `bmo_gpu_ga10x::control` y `docs/maestro/NVIDIA_HISTORIA.md`.

use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL, SUBIDA_SEGUNDOS};
use bmo_gpu_ga10x::objeto;
use bmo_userland as bmo;

use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Los fotogramas de cada medida: ~0,2 s de la 3060, y mas que la rampa de
/// un reloj.
const FOTOGRAMAS: u32 = 48;

/// Lo que dejo la ultima vez.
#[derive(Clone, Copy)]
struct Subida {
    /// `true` = se pidio subir; `false` = quitar.
    arriba: bool,
    r: Result<Contestada, u32>,
    /// El P-state antes y despues (0 = P0).
    pstate: (Option<u8>, Option<u8>),
    /// `(us de la 3060 por fotograma, fps en decimas)`, antes y despues.
    antes: Option<(u64, u64)>,
    despues: Option<(u64, u64)>,
    /// Cuando se pidio (ciclos), para decir cuanto le queda.
    desde: u64,
}

static mut ULTIMA: Option<Subida> = None;

fn ultima() -> Option<Subida> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ULTIMA) }
}

fn pstate() -> Option<u8> {
    super::gspsalud::preguntar().ok().and_then(|m| control::pstate(m as u32))
}

fn medir() -> Option<(u64, u64)> {
    if !super::gspcomputo::pantalla_hecha() {
        return None;
    }
    super::gspcomputo::medir_pantalla(FOTOGRAMAS).ok()
}

/// Espera durmiendo (la rampa del reloj no se acelera girando).
fn dormir_ms(ms: u64) {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz == 0 {
        return;
    }
    let hasta = bmo::ciclos() + hz / 1000 * ms;
    while bmo::ciclos() < hasta {
        bmo::wait(0, 0, 4_000_000);
    }
}

/// **Exigir** (VERRANO V1c, `gpu verrano banco exige`): los relojes al
/// maximo SIN medir la pantalla -- lo pide quien va a dar trabajo seguido a
/// la 3060 y no quiere que lo haga a relojes de reposo. Se apunta como la
/// ultima subida (la fila `relojes` lo dice). `(el RM lo acepto, el P-state
/// antes, despues)`; `None` si el GSP-RM y nuestros objetos aun no estan.
pub(crate) fn exigir() -> Option<(bool, Option<u8>, Option<u8>)> {
    if !super::gspobjeto::listos() {
        return None;
    }
    let antes = pstate();
    let r = controlar(Control::RelojesArriba, &mut [0u8; CABECERA_CONTROL + 8]);
    dormir_ms(100);
    let despues = pstate();
    let s = Subida { arriba: true, r, pstate: (antes, despues), antes: None, despues: None, desde: bmo::ciclos() };
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMA) = Some(s) };
    Some((matches!(r, Ok(c) if c.bien()), antes, despues))
}

/// `gpu relojes` (`quitar` = `gpu relojes off`).
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, quitar: bool) -> After {
    dsk.field.n = 0;
    if !super::gspobjeto::listos() {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ERR);
        g.text(b"  gpu relojes: el GSP-RM y nuestros objetos aun no estan: `save mode` primero\n");
        g.with_ink(INK_PLAIN);
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, if quitar { "quitando la subida de relojes" } else { "midiendo la 3060, subiendo sus relojes y midiendo otra vez" }, INK_DIM);
    let antes_p = pstate();
    let antes = if quitar { None } else { medir() };
    let c = if quitar { Control::RelojesNormales } else { Control::RelojesArriba };
    let r = controlar(c, &mut [0u8; CABECERA_CONTROL + 8]);
    // La rampa: el RM cambia de nivel en unos ms; se le dan 100.
    dormir_ms(100);
    let despues_p = pstate();
    let despues = if quitar { None } else { medir() };
    let s = Subida { arriba: !quitar, r, pstate: (antes_p, despues_p), antes, despues, desde: bmo::ciclos() };
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMA) = Some(s) };
    // Las medidas pintaron la pantalla entera: el escritorio, otra vez.
    if antes.is_some() || despues.is_some() {
        crate::repintar_escritorio(p, dsk, "relojes");
    }
    let g = &mut dsk.out.grid;
    let bien = matches!(s.r, Ok(c) if c.bien());
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(match (quitar, bien) {
        (false, true) => b"  LA 3060 AL MAXIMO: el GSP-RM acepto PERF_BOOST\n" as &[u8],
        (true, true) => b"  subida QUITADA: los relojes vuelven a lo que decida el GSP-RM\n",
        (_, false) => b"  el GSP-RM NO acepto PERF_BOOST: mira la fila `relojes`\n",
    });
    g.with_ink(INK_PLAIN);
    fila(g);
    paint_status(p, &dsk.run_box, "relojes", INK_DIM);
    After::Settle
}

fn p_estado(s: &mut Output, k: Option<u8>) {
    match k {
        Some(k) => {
            s.byte(b'P');
            s.dec(k as u64);
        }
        None => s.text(b"?"),
    }
}

/// **La fila `relojes`**: lo que se pidio, lo que contesto el RM, el P-state
/// y el fotograma antes y despues.
pub(crate) fn fila(s: &mut Output) {
    let Some(u) = ultima() else { return };
    campo(s, b"relojes");
    match u.r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(c) => {
            s.with_ink(if c.bien() { INK_GOOD } else { INK_ERR });
            s.text(if u.arriba { b"PERF_BOOST al maximo: " as &[u8] } else { b"PERF_BOOST quitado: " });
            s.text(objeto::estado(if c.r.estado != 0 { c.r.estado } else { c.resultado }));
            s.with_ink(INK_ECHO);
            s.text(b" (0x");
            s.hex(c.r.estado as u64, 2);
            s.text(b") en ");
            s.dec(c.espera_us / 1000);
            s.text(b" ms; P-state ");
            p_estado(s, u.pstate.0);
            s.text(b" -> ");
            p_estado(s, u.pstate.1);
            if let (Some(a), Some(d)) = (u.antes, u.despues) {
                s.text(b"; el fotograma: 3060 ");
                s.dec(a.0);
                s.text(b" -> ");
                s.dec(d.0);
                s.text(b" us, ");
                s.dec(a.1 / 10);
                s.text(b" -> ");
                s.dec(d.1 / 10);
                s.text(b" fps");
                if d.0 > 0 {
                    s.text(b" (x");
                    s.dec(a.0 / d.0);
                    s.byte(b'.');
                    s.dec(a.0 * 10 / d.0 % 10);
                    s.byte(b')');
                }
            } else if u.arriba {
                s.text(b"; sin `pantalla` hecha no hay fotograma que medir");
            }
            if u.arriba && c.bien() {
                let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
                let pasados = bmo::ciclos().wrapping_sub(u.desde) / hz;
                let quedan = (SUBIDA_SEGUNDOS as u64).saturating_sub(pasados);
                s.text(b"; baja sola en ");
                s.dec(quedan);
                s.text(b" s");
            }
            super::datos::anotar(b"gpu relojes estado", c.r.estado as u64, b"");
            if let (Some(a), Some(d)) = (u.antes, u.despues) {
                super::datos::anotar(b"gpu relojes antes us", a.0, b"");
                super::datos::anotar(b"gpu relojes despues us", d.0, b"");
            }
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}
