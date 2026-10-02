//! **C3: EL REPOSO DE LA 3060** -- `gpu reposo` y `gpu reposo off`, y la
//! fila `reposo` de `gpu salud`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea; lo que pide es
//!                     justo que la 3060 GASTE menos
//!
//! Pedido del propietario (02-10): *"investigar en control de electricidad si
//! eso es como lo hace Windows para regular"*. En Windows la 3060 baja sola a
//! P8 (210 MHz, ~17 W) en reposo porque alli el RM corre en la CPU y mide; en
//! BMO-X la SALIDA del 02-10 la dejaba en P0 sin trabajo. Esto es el
//! EXPERIMENTO: decirle al GSP-RM "esta ociosa" con
//! `PERF_AGGRESSIVE_PSTATE_NOTIFY` (lo que hace el driver del sistema en la
//! funcion "KMD Aggressive P-state") y mirar el P-state antes y despues. Si
//! baja, el siguiente paso es que el kernel lo diga solo cuando la 3060 lleve
//! un rato sin trabajo. Si no, la fila lo dice con el estado del RM. Ver
//! `bmo_gpu_ga10x::control` (C3).

use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL};
use bmo_gpu_ga10x::objeto;
use bmo_userland as bmo;

use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo mas que se espera a que el P-state se mueva, y cada cuanto se mira.
const ESPERA_MS: u64 = 2000;
const PASO_MS: u64 = 100;

/// Lo que dejo la ultima vez.
#[derive(Clone, Copy)]
struct Reposo {
    /// `true` = "ociosa"; `false` = "ya no".
    ociosa: bool,
    r: Result<Contestada, u32>,
    /// El P-state antes y despues (0 = P0, 8 = P8).
    pstate: (Option<u8>, Option<u8>),
    /// Cuanto tardo en moverse (o lo que se espero, si no se movio).
    ms: u64,
}

static mut ULTIMO: Option<Reposo> = None;

fn ultimo() -> Option<Reposo> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ULTIMO) }
}

fn pstate() -> Option<u8> {
    super::gspsalud::preguntar().ok().and_then(|m| control::pstate(m as u32))
}

/// **Esperar a que el P-state deje de ser `antes`**, mirando cada
/// [`PASO_MS`] hasta [`ESPERA_MS`]: `(el ultimo leido, ms esperados)`.
fn esperar_cambio(antes: Option<u8>) -> (Option<u8>, u64) {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let t0 = bmo::ciclos();
    let mut ahora = antes;
    let mut ms = 0;
    while ms < ESPERA_MS {
        super::gsprelojes::dormir_ms(PASO_MS);
        ahora = pstate();
        ms = bmo::ciclos().wrapping_sub(t0) / (hz / 1000).max(1);
        if ahora != antes {
            break;
        }
    }
    (ahora, ms)
}

/// `gpu reposo` (`quitar` = `gpu reposo off`).
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, quitar: bool) -> After {
    dsk.field.n = 0;
    if !super::gspobjeto::listos() {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ERR);
        g.text(b"  gpu reposo: el GSP-RM y nuestros objetos aun no estan: `save mode` primero\n");
        g.with_ink(INK_PLAIN);
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, if quitar { "diciendole al GSP-RM que la 3060 ya no esta ociosa" } else { "diciendole al GSP-RM que la 3060 esta ociosa, y mirando su P-state" }, INK_DIM);
    let antes = pstate();
    let c = if quitar { Control::ReposoNo } else { Control::ReposoSi };
    let r = controlar(c, &mut [0u8; CABECERA_CONTROL + 24]);
    let (despues, ms) = esperar_cambio(antes);
    let u = Reposo { ociosa: !quitar, r, pstate: (antes, despues), ms };
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMO) = Some(u) };
    let g = &mut dsk.out.grid;
    let bien = matches!(r, Ok(c) if c.bien());
    let bajo = matches!((antes, despues), (Some(a), Some(d)) if d > a);
    g.with_ink(if bien && (quitar || bajo) { INK_GOOD } else { INK_ERR });
    g.text(match (quitar, bien, bajo) {
        (false, true, true) => b"  LA 3060 BAJO SOLA: el GSP-RM obedece al \"esta ociosa\", como en Windows\n" as &[u8],
        (false, true, false) => b"  el GSP-RM lo ACEPTO, pero el P-state no bajo. Si `gpu relojes` subio hace menos de 60 s, manda el: `gpu relojes off` y otra vez\n",
        (true, true, _) => b"  tope QUITADO: el GSP-RM vuelve a decidir el solo\n",
        (_, false, _) => b"  el GSP-RM NO acepto PERF_AGGRESSIVE_PSTATE_NOTIFY: mira la fila `reposo`\n",
    });
    g.with_ink(INK_PLAIN);
    fila(g);
    paint_status(p, &dsk.run_box, "reposo", INK_DIM);
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

/// **La fila `reposo`**: lo que se dijo, lo que contesto el RM, y el
/// P-state antes y despues, con lo que tardo.
pub(crate) fn fila(s: &mut Output) {
    let Some(u) = ultimo() else { return };
    campo(s, b"reposo");
    match u.r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(c) => {
            s.with_ink(if c.bien() { INK_GOOD } else { INK_ERR });
            s.text(if u.ociosa { b"\"ociosa\": " as &[u8] } else { b"\"ya no\": " });
            s.text(objeto::estado(if c.r.estado != 0 { c.r.estado } else { c.resultado }));
            s.with_ink(INK_ECHO);
            s.text(b" (0x");
            s.hex(c.r.estado as u64, 2);
            s.text(b"); P-state ");
            p_estado(s, u.pstate.0);
            s.text(b" -> ");
            p_estado(s, u.pstate.1);
            s.text(b" en ");
            s.dec(u.ms);
            s.text(b" ms");
            super::datos::anotar(b"gpu reposo estado", c.r.estado as u64, b"");
            super::datos::anotar(b"gpu reposo pstate antes", u.pstate.0.map_or(99, |k| k as u64), b"");
            super::datos::anotar(b"gpu reposo pstate despues", u.pstate.1.map_or(99, |k| k as u64), b"");
            super::datos::anotar(b"gpu reposo ms", u.ms, b"ms");
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}
