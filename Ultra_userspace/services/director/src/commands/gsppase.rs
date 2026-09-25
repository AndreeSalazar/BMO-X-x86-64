//! **`gpu pase`: EL PASE DE LA GPU, P1** (2026-09-25) -- se abre, se mira el
//! buzon, se cierra y se mira la lapida. Sin fotogramas todavia: el latido del
//! VBLANK que los lleva es P2 (`docs/plan/PLAN_LA_3060_AFINADA.md`, 5c).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea
//!
//! Lo que se comprueba, en orden:
//!
//! ```text
//!    ABRIR           el kernel presta el lienzo PARA QUEDARSE y mapea el buzon
//!    el buzon        MAGIA `BGPU`, VERSION y la medida de la pantalla
//!    CERRAR          el lienzo devuelto
//!    la lapida       la MISMA VA: MAGIA a cero y ESTADO = "lo cerro quien lo abrio"
//!    ESTADO          cerrado, con ese motivo
//! ```
//!
//! Si el volcador estaba armado (`save mode` o `gpu volcado`), se suelta antes
//! -- el pase y el volcador prestan lo mismo -- y se vuelve a armar al acabar.

use bmo_pase_gpu::{buzon, orden, pase, radar};
use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

// La suborden viaja por la puerta con la forma del crate: que userland y el
// kernel digan lo mismo del numero de la orden se comprueba AL COMPILAR.
const _: () = assert!(bmo::IOMMU_OP_GPU_PASE == 0x44);

/// Una palabra del buzon, leida de la memoria que el kernel mapeo.
fn palabra(va: u64, k: usize) -> u32 {
    // SAFETY: `va` es la que devolvio ABRIR (una pagina mapeada, y al cerrar
    // una lapida: nunca un hueco); `k` < `buzon::PALABRAS`, dentro de ella.
    unsafe { core::ptr::read_volatile((va as *const u32).add(k)) }
}

fn linea(s: &mut Output, bien: bool, texto: &[u8]) {
    s.with_ink(if bien { INK_GOOD } else { INK_ERR });
    s.text(if bien { b"  si  " as &[u8] } else { b"  NO  " });
    s.with_ink(INK_PLAIN);
    s.text(texto);
    s.byte(b'\n');
}

/// `gpu pase`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "el pase de la GPU: abrir, mirar el buzon, cerrar", INK_DIM);
    let lienzo = super::gspvolcado::lienzo();
    let s = &mut dsk.out.grid;
    if lienzo == 0 {
        linea(s, false, b"el escritorio pinta directo al panel: sin lienzo no hay pase");
        dsk.field.n = 0;
        return After::Settle;
    }
    let volcaba = p.volcando_por_gpu();
    if volcaba {
        p.volcar_por_cpu();
    }
    let bien = probar(s, p, lienzo);
    if volcaba {
        let _ = p.volcar_por_gpu();
    }
    s.with_ink(if bien { INK_GOOD } else { INK_ERR });
    s.text(if bien {
        b"  EL PASE ABRE Y CIERRA: el lienzo prestado una vez, el buzon mapeado y la lapida en su sitio\n" as &[u8]
    } else {
        b"  el pase no cumplio todo: mira la linea NO de arriba\n"
    });
    s.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, "pase", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn probar(s: &mut Output, p: &bmo::Pantalla, lienzo: u64) -> bool {
    let va = match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PASE, orden::abrir(lienzo)) {
        Ok(va) => va,
        Err(m) => {
            let e = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PASE, orden::ESTADO << 60).unwrap_or(0);
            s.with_ink(INK_ERR);
            s.text(b"  NO  el pase no se abrio: ");
            match pase::NoPase::desde_codigo(orden::no(e)) {
                Some(n) => s.text(n.texto().as_bytes()),
                None => s.text(super::iommu::motivo(m)),
            }
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
            return false;
        }
    };
    let mut bien = va == orden::BUZON_VA;
    linea(s, bien, b"ABRIR: el lienzo prestado para quedarse, el buzon en su VA");
    let medidas = palabra(va, buzon::campo::MEDIDAS);
    let forma = palabra(va, buzon::campo::MAGIA) == buzon::MAGIA
        && palabra(va, buzon::campo::VERSION) == buzon::VERSION
        && palabra(va, buzon::campo::ESTADO) == 0
        && medidas == (p.ancho | p.alto << 16);
    linea(s, forma, b"el buzon: MAGIA BGPU, su version, abierto y la medida de la pantalla");
    bien &= forma;
    let cerrado = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PASE, orden::CERRAR << 60).is_ok();
    linea(s, cerrado, b"CERRAR: el lienzo devuelto");
    let motivo = radar::Motivo::CerradoPorElPropietario.codigo();
    let lapida = palabra(va, buzon::campo::MAGIA) == 0 && palabra(va, buzon::campo::ESTADO) == motivo;
    linea(s, lapida, b"la lapida: la misma VA sin magia, con el motivo (y sin fallo de pagina)");
    let e = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PASE, orden::ESTADO << 60).unwrap_or(0);
    let estado = !orden::abierto(e) && orden::motivo(e) == motivo;
    linea(s, estado, b"ESTADO: cerrado, porque lo cerro quien lo abrio");
    bien && cerrado && lapida && estado
}
