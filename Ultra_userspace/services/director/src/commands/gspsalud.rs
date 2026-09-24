//! **`gpu salud`: LA 3060 POR DENTRO** -- la temperatura del sensor y el
//! enlace PCIe (lecturas del kernel, sin el GSP-RM), y el P-state preguntado
//! al GSP-RM por `GSP_RM_CONTROL` sobre NUESTRO subdispositivo (L1b).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     dos lecturas y, con el RM, una pregunta de hasta 5 s
//!
//! Lo mismo, resumido, esta siempre en el panel (`scene::lateral_gsp`): esto
//! es la version con los crudos, para el INFORME.
//!
//! Los VATIOS de la 3060 no salen aqui, y no por olvido: la orden del RM que
//! los da no la publica NVIDIA (ver `bmo_gpu_ga10x::salud`).

use bmo_gpu_ga10x::control::{self, CABECERA_CONTROL, GSP_RM_CONTROL};
use bmo_gpu_ga10x::salud;
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy, Default)]
struct Pstate {
    numero: u32,
    r: Option<control::Respuesta>,
    resultado: u32,
    espera_us: u64,
    no: u32,
}

static mut PSTATE: Option<Pstate> = None;

fn ultimo() -> Option<Pstate> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(PSTATE) }
}

/// Motivo del escritorio: el RM contesto, pero la orden de control no salio.
pub(crate) const NO_CONTROL_NEGADO: u32 = 0x123;

/// **Preguntar el P-state.** `Ok(la mascara)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let mut u = Pstate::default();
    let mut otros = Otros::default();
    match bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_CONTROL, 0) {
        Ok(v) => u.numero = (v >> 32) as u32,
        Err(m) => u.no = m,
    }
    if u.no == 0 {
        let mut d = [0u8; CABECERA_CONTROL + 4];
        match esperar(GSP_RM_CONTROL, &mut d, &mut otros) {
            Ok((m, us)) => {
                u.r = control::leer(&d);
                u.resultado = m.resultado;
                u.espera_us = us;
            }
            Err(no) => u.no = no,
        }
    }
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(PSTATE) = Some(u) };
    match u.r {
        _ if u.no != 0 => Err(u.no),
        Some(r) if r.estado == 0 && u.resultado == 0 => {
            if let Some(k) = control::pstate(r.valor) {
                crate::scene::lateral_gsp::pstate(k);
            }
            Ok(r.valor as u64)
        }
        _ => Err(NO_CONTROL_NEGADO),
    }
}

/// Lo pregunta `save mode`: el P-state ya se contesto.
pub(crate) fn hecho() -> bool {
    ultimo().map_or(false, |u| u.no == 0 && u.resultado == 0 && matches!(u.r, Some(r) if r.estado == 0))
}

/// `gpu salud`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    // El P-state solo con el GSP-RM y nuestros objetos: sin ellos, las
    // lecturas del kernel valen igual.
    if super::gspobjeto::listos() {
        paint_status(p, &dsk.run_box, "preguntandole el P-state al GSP-RM", INK_DIM);
        let _ = preguntar();
    }
    let g = &mut dsk.out.grid;
    g.with_ink(INK_GOOD);
    g.text(b"  LA 3060 POR DENTRO: temperatura y enlace en solo lectura; el P-state, al RM\n");
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "salud", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas**: `temp` y `pcie` siempre que haya 3060; `pstate` si se
/// pregunto.
pub(crate) fn fila(s: &mut Output) {
    if bmo::info(bmo::INFO_GPU_CHIP) & bmo::GPU_HALLADA == 0 {
        return;
    }
    let crudo = bmo::info(bmo::INFO_GPU_SALUD) as u32;
    campo(s, b"temp");
    match salud::lectura(crudo) {
        Some((g, fe)) => {
            s.with_ink(if g >= 83 { INK_ERR } else { INK_GOOD });
            s.dec(g as u64);
            s.text(b" grados");
            s.with_ink(INK_PLAIN);
            if fe == salud::Fe::Probable {
                s.text(b" (PROBABLE: bit 29 caido, 31 puesto; la historia del panel lo dira)");
            }
        }
        None => s.text(b"sin dato (el bit de validez del sensor, caido)"),
    }
    s.with_ink(INK_ECHO);
    s.text(b"   sensor 0x020460 = 0x");
    s.hex(crudo as u64, 8);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    let e = bmo::info(bmo::INFO_GPU_SALUD | 1 << 8);
    campo(s, b"pcie");
    match salud::enlace(e as u16, (e >> 32) as u32) {
        Some(l) => {
            let gts = |s: &mut Output, gen: u8| {
                let v = salud::gts(gen);
                s.dec(v as u64 / 10);
                s.byte(b'.');
                s.dec(v as u64 % 10);
                s.text(b" GT/s");
            };
            s.text(b"Gen");
            s.dec(l.gen as u64);
            s.text(b" x");
            s.dec(l.ancho as u64);
            s.text(b" (");
            gts(s, l.gen);
            s.text(b") de Gen");
            s.dec(l.gen_max as u64);
            s.text(b" x");
            s.dec(l.ancho_max as u64);
            if l.gen < l.gen_max {
                s.with_ink(INK_ECHO);
                s.text(b"; en Gen1 porque arranca asi: subirlo es del RM");
                s.with_ink(INK_PLAIN);
            }
        }
        None => s.text(b"sin capacidad PCI Express leida"),
    }
    s.byte(b'\n');

    if let Some(u) = ultimo() {
        campo(s, b"pstate");
        if u.no != 0 {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(u.no));
        } else if let Some(r) = u.r {
            let bien = r.estado == 0 && u.resultado == 0;
            s.with_ink(if bien { INK_GOOD } else { INK_ERR });
            match control::pstate(r.valor) {
                Some(k) if bien => {
                    s.byte(b'P');
                    s.dec(k as u64);
                    s.text(if k == 0 { b" (a todo lo que da)" as &[u8] } else if k >= 8 { b" (reposo)" } else { b"" });
                }
                _ => {
                    s.text(bmo_gpu_ga10x::objeto::estado(r.estado));
                    s.text(b" (0x");
                    s.hex(r.estado as u64, 2);
                    s.byte(b')');
                }
            }
            s.with_ink(INK_PLAIN);
            s.text(b"   PERF_GET_CURRENT_PSTATE en ");
            s.dec(u.espera_us / 1000);
            s.text(b" ms (numero ");
            s.dec(u.numero as u64);
            s.text(b"), mascara 0x");
            s.hex(r.valor as u64, 4);
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}
