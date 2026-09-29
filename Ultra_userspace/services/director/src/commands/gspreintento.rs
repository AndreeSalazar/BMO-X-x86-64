//! **`gpu reintentar`: EL REINTENTO LIMPIO DEL GSP** (2026-09-29). Pedido del
//! propietario: *"prepara el reintento limpio del GSP"* -- tras un `0x15` del
//! booter, deshacer lo que dejo a medias y volver a subir SIN reiniciar.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: cuatro firmwares
//!                     firmados, hasta ~20 s si todo espera su plazo entero
//!
//! # El orden
//!
//! ```text
//!    cerrar      FWSEC-SB en el falcon del GSP; se espera que pare (5 s).
//!                Si no carga (metal 29-09: tras el 0x15 ese falcon queda
//!                cerrado), se salta, como nouveau cuando SB falla
//!    descargar   el booter de descarga en el SEC2; se espera que pare (5 s)
//!                y la WPR2 tiene que quedar ABAJO -- si no, se para aqui
//!    subir       el despertar a cero y FWSEC-FRTS otra vez; se espera la
//!                WPR2 montada (2 s) y, SIN PAUSA, despertar y el booter
//! ```
//!
//! La ultima linea es la pregunta del propietario: *"en que momento pide GSP
//! para arrancar?"*. En `save mode` entre FRTS y el booter pasan segundos; aqui
//! van seguidos, como en nova-core. La fila `autopsia` dice cuantos ms fueron
//! (`de FRTS al booter`) en los dos casos: si el reintento sale y `save mode`
//! no, el momento importa.
//!
//! Los pasos los da el kernel (`dev/gpu_reintento.rs`) y los lee EN VIVO;
//! aqui se espera entre medias, cediendo el turno, como en `gpu apagar`.

use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

// Los bits de `IOMMU_OP_GSP_REINTENTO` (espejo de `dev/gpu_reintento.rs`).
const SB: u64 = 1 << 0;
const SB_PARADO: u64 = 1 << 1;
const DESCARGADOR: u64 = 1 << 2;
const SEC2_PARADO: u64 = 1 << 3;
const WPR2_ABAJO: u64 = 1 << 4;
const FRTS: u64 = 1 << 5;
const SB_SALTADO: u64 = 1 << 6;
const VECES_SHIFT: u64 = 8;
const BUZON_SHIFT: u64 = 32;

pub(crate) const NO_REINTENTO_SB: u32 = 0x152;
pub(crate) const NO_REINTENTO_DESCARGA: u32 = 0x153;
pub(crate) const NO_REINTENTO_WPR2: u32 = 0x154;
pub(crate) const NO_REINTENTO_FRTS: u32 = 0x155;

/// Lo que dejo el ultimo intento: el paso en que se paro y lo que tardo.
static mut ULTIMO: Option<(Result<u64, u32>, u64)> = None;

fn info() -> u64 {
    bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_REINTENTO, 0).unwrap_or(0)
}

/// Esperar, cediendo el turno, hasta `ms`, a que `lee()` cumpla.
fn esperar(ms: u64, lee: impl Fn() -> u64, cumple: impl Fn(u64) -> bool) -> bool {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let fin = bmo::ciclos() + hz / 1000 * ms;
    loop {
        if cumple(lee()) {
            return true;
        }
        if bmo::ciclos() >= fin {
            return false;
        }
        bmo::yield_screen();
    }
}

fn pasos() -> Result<u64, u32> {
    if info() & SB == 0 || info() & FRTS != 0 {
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_REINTENTO_CERRAR, 0)?;
    }
    if info() & DESCARGADOR == 0 {
        if !esperar(5000, info, |v| v & SB_PARADO != 0) {
            return Err(NO_REINTENTO_SB);
        }
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_REINTENTO_DESCARGAR, 0)?;
    }
    if !esperar(5000, info, |v| v & SEC2_PARADO != 0) {
        return Err(NO_REINTENTO_DESCARGA);
    }
    if info() & WPR2_ABAJO == 0 {
        return Err(NO_REINTENTO_WPR2);
    }
    bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_REINTENTO_SUBIR, 0)?;
    // FWSEC-FRTS monta la WPR2 y se para: en el metal, ~ms.
    let fwsec = || bmo::info(bmo::INFO_GPU_FWSEC);
    if !esperar(2000, fwsec, |v| v & bmo::FWSEC_PARADO != 0 && v & bmo::FWSEC_WPR2 != 0) {
        return Err(NO_REINTENTO_FRTS);
    }
    // Y SIN PAUSA: despertar y el booter, por el camino de siempre (que
    // guarda los logs del GSP, salga como salga).
    super::gsp::despertar()
}

/// **Reintentar**, entero.
pub(crate) fn reintentar() -> Result<u64, u32> {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let r = pasos();
    // SAFETY: el escritorio es un solo hilo.
    unsafe { *core::ptr::addr_of_mut!(ULTIMO) = Some((r, (bmo::ciclos() - desde) * 1000 / hz)) };
    r
}

/// `gpu reintentar`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if super::gsp::despierto() {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ECHO);
        g.text(b"  el GSP ya desperto en este arranque: no hay nada que reintentar\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu reintentar") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "reintento limpio: FWSEC-SB, booter de descarga, FRTS y el booter SEGUIDOS", INK_DIM);
    let r = reintentar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(_) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL GSP DESPERTO AL REINTENTAR: sin reiniciar, con FRTS y el booter seguidos\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.text(b"\n");
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    super::gsp::fila_despierto(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "reintentar", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn paso(s: &mut Output, hecho: bool, nombre: &[u8]) {
    s.with_ink(if hecho { INK_GOOD } else { INK_ECHO });
    s.text(if hecho { b" +" as &[u8] } else { b" -" });
    s.text(nombre);
}

/// **La fila `reintento`**, si se intento.
pub(crate) fn fila(s: &mut Output) {
    let v = info();
    let veces = (v >> VECES_SHIFT) & 0xFF;
    if veces == 0 {
        return;
    }
    campo(s, b"reintento");
    s.with_ink(INK_ECHO);
    s.text(b"intento ");
    s.dec(veces);
    if v & SB_SALTADO != 0 {
        s.with_ink(INK_ECHO);
        s.text(b" ~sb(saltado: el falcon del GSP cerrado)");
    } else {
        paso(s, v & (SB_PARADO | DESCARGADOR) != 0, b"sb");
    }
    paso(s, v & (SEC2_PARADO | FRTS) != 0, b"descarga");
    paso(s, v & (WPR2_ABAJO | FRTS) != 0, b"wpr2-abajo");
    paso(s, v & FRTS != 0, b"frts");
    paso(s, super::gsp::despierto(), b"gsp");
    s.with_ink(INK_ECHO);
    s.text(b"   MAILBOX0 0x");
    s.hex(v >> BUZON_SHIFT, 8);
    // SAFETY: como `reintentar`.
    if let Some((r, ms)) = unsafe { *core::ptr::addr_of!(ULTIMO) } {
        if let Err(m) = r {
            s.text(b"; se paro: ");
            s.text(super::iommu::motivo(m));
        }
        s.text(b"; todo en ");
        s.dec(ms);
        s.text(b" ms");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp reintento", v, b"");
}
