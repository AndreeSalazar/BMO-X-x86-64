//! **`gpu vram`: L1c2, LA CPU ESCRIBE EN LA VRAM.** Por la ventana PRAMIN de
//! BAR0, una pagina en 64 MiB (`bmo_gpu_ga10x::vram::PRUEBA`): guardada,
//! escrita con un patron, releida y devuelta, con la ventana como estaba.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     unas cinco mil lecturas y escrituras de BAR0 (~ms)
//!
//! Antes de pedirla se mira que esos 64 MiB caen en VRAM que el GSP-RM dio como
//! USABLE (L1a): sin esa respuesta, o fuera de ella, no se pide. La direccion
//! no la elige el escritorio: el kernel solo acepta esa.

use bmo_gpu_ga10x::vram;
use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo ultimo: el `Ok` empaquetado del kernel, o el NO.
static mut ULTIMA: Option<Result<u64, u32>> = None;

fn ultima() -> Option<Result<u64, u32>> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ULTIMA) }
}

/// Motivos del escritorio: sin las regiones de L1a, o la pagina fuera de ellas.
pub(crate) const NO_VRAM_SIN_REGIONES: u32 = 0x124;
pub(crate) const NO_VRAM_NO_USABLE: u32 = 0x125;
/// La prueba corrio y algo no cuadro (el patron, lo devuelto o la ventana).
pub(crate) const NO_VRAM_MAL: u32 = 0x126;

fn sana(v: u64) -> bool {
    let (buenas, devueltas, ventana, _) = vram::desempaquetar(v);
    buenas as usize == vram::PALABRAS && devueltas as usize == vram::PALABRAS && ventana
}

/// **La prueba.** `Ok(el empaquetado)` si todo cuadro.
pub(crate) fn probar() -> Result<u64, u32> {
    let r = match super::gsprpc::usable(vram::PRUEBA, 4 * vram::PALABRAS as u64) {
        None => Err(NO_VRAM_SIN_REGIONES),
        Some(false) => Err(NO_VRAM_NO_USABLE),
        Some(true) => bmo::iommu_orden(bmo::IOMMU_OP_GPU_VRAM),
    };
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMA) = Some(r) };
    match r {
        Ok(v) if sana(v) => Ok(v),
        Ok(_) => Err(NO_VRAM_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn hecha() -> bool {
    matches!(ultima(), Some(Ok(v)) if sana(v))
}

/// `gpu vram`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "escribiendo en la VRAM por PRAMIN", INK_DIM);
    let r = probar();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LA CPU ESCRIBIO EN LA VRAM de tu 3060 y la dejo como estaba\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  la prueba de la VRAM no salio: mira la fila `vram`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "vram", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila**, si se probo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = ultima() else { return };
    campo(s, b"vram");
    match r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(v) => {
            let (buenas, devueltas, ventana, antes) = vram::desempaquetar(v);
            s.with_ink(if sana(v) { INK_GOOD } else { INK_ERR });
            s.text(b"PRAMIN en 0x");
            s.hex(vram::PRUEBA, 9);
            s.text(b": ");
            s.dec(buenas as u64);
            s.text(b" de 1024 palabras escritas y releidas");
            s.with_ink(INK_PLAIN);
            s.text(b"; devueltas ");
            s.dec(devueltas as u64);
            s.with_ink(INK_ECHO);
            s.text(b"; ventana 0x");
            s.hex(antes as u64, 4);
            s.text(if ventana { b" devuelta" as &[u8] } else { b" NO devuelta" });
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}
