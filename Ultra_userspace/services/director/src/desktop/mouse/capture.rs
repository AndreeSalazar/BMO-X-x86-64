//! Movimiento relativo de la app con foco. Nunca se cede la entrada fisica.
//! [consumo] LATE -- consulta el foco y los contadores en cada vuelta del escritorio.
use bmo_userland as bmo;
use crate::desktop::{Desktop, Ventana};
use bmo_golpe::relative::Capture;

static mut CAPTURE: Capture = Capture::new();
static mut BUTTONS: u8 = 0;
pub(crate) fn active() -> bool { unsafe { (*core::ptr::addr_of!(CAPTURE)).owner() != 0 } }

pub(crate) fn update(dsk: &mut Desktop, p: &bmo::Pantalla, input: &bmo::Entrada, buttons: u8) -> bool {
    let target = match dsk.win.focus.actual() {
        Some(Ventana::App(i)) if !dsk.win.switcher_painted && !dsk.table.minimizada(i as usize) && !super::super::captura::recortando() => {
            dsk.table.get(i as usize).filter(|s| s.mouse_flags() & bmo::SUP_RATON_PEDIDO != 0).map(|s| (i as usize, s.tid))
        }
        _ => None,
    };
    // SAFETY: solo el hilo del escritorio toca este estado, antes de pintar.
    let state = unsafe { &mut *core::ptr::addr_of_mut!(CAPTURE) };
    let old = state.owner();
    let owner = target.map_or(0, |(_, tid)| tid);
    if old != owner { dsk.save_under.lift(p); }
    if old != 0 && old != owner {
        // La app anterior debe volver a pedir al recuperar el foco: evita
        // recapturar el cursor al cerrar un menu sin gesto del propietario.
        if let Some(i) = dsk.table.ventana_de(old) { if let Some(s) = dsk.table.get(i) { s.mouse_grant(false); } }
    }
    // Sin app que pida captura no se hace otra consulta al kernel.
    state.sample(owner, if owner != 0 { input.relativo() } else { 0 });
    let Some((i, _)) = target else { return false };
    let Some(s) = dsk.table.get(i) else { return false };
    dsk.tick.button_before = buttons & 1 != 0;
    dsk.tick.derecho_before = buttons & 2 != 0;
    s.mouse_grant(true);
    if old != owner { unsafe { BUTTONS = buttons }; return true; }
    let (dx, dy) = state.motion();
    if dx != 0 || dy != 0 || unsafe { BUTTONS != buttons } {
        let event = bmo::SUP_EV_RELATIVO | 0x100 | buttons as u64
            | (dx as u16 as u64) << 16 | (dy as u16 as u64) << 32;
        if s.publicar(event) { state.delivered(dx, dy); unsafe { BUTTONS = buttons }; }
    }
    true
}
