//! **T1c y T2a: el triangulo por el PIPELINE 3D** -- las ordenes del
//! escritorio, su panel y sus filas. Aparte de `gspcomputo.rs` por el censo
//! modular (L6a): es un hijo, asi que ve su estado sin hacerlo publico.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     un trabajo de la 3060 y sus filas; en reposo no hace nada

use bmo_gpu_ga10x::{color3d, raster};
use bmo_userland as bmo;

use super::super::tabla::campo;
use super::super::After;
use super::{con, estado, hasta_el_lienzo, no, panel, Computo, Vista, NO_COLOR3D_MAL, NO_RASTER_MAL, NO_TRABAJO_SIN_FICHA, PANEL_ABIERTO};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// **T1c: el triangulo por el rasterizador** de la 3060.
pub(crate) fn dibujar_raster() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_RASTER, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.raster = Some(r));
    match r {
        Ok(v) if raster::sano(v) => Ok(v),
        Ok(_) => Err(NO_RASTER_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn raster_hecho() -> bool {
    matches!(estado().raster, Some(Ok(v)) if raster::sano(v))
}

/// `gpu raster`: el triangulo por el pipeline 3D, a pantalla completa.
pub(crate) fn orden_raster(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "el rasterizador de la 3060 dibuja un triangulo", INK_DIM);
    let r = dibujar_raster();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Raster));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  EL TRIANGULO POR EL RASTERIZADOR DE TU 3060 (M5 T1c): mira la fila `raster`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el triangulo 3D no salio: mira la fila `raster`\n");
    }
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// **T2a: el triangulo con color**, mezclado por el rasterizador.
pub(crate) fn dibujar_color3d() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COLOR_3D, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.color3d = Some(r));
    match r {
        Ok(v) if color3d::sano(v) => Ok(v),
        Ok(_) => Err(NO_COLOR3D_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn color3d_hecho() -> bool {
    matches!(estado().color3d, Some(Ok(v)) if color3d::sano(v))
}

/// `gpu color`: el triangulo con color por el pipeline 3D, a pantalla completa.
pub(crate) fn orden_color3d(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "el rasterizador de la 3060 mezcla tres colores", INK_DIM);
    let r = dibujar_color3d();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Color3d));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  EL TRIANGULO CON COLOR, MEZCLADO POR EL RASTERIZADOR DE TU 3060 (M5 T2a): mira la fila `color`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el triangulo con color no salio: mira la fila `color`\n");
    }
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// Las filas `raster` y `color`.
pub(super) fn fila(s: &mut Output, c: &Computo) {
    if let Some(r) = c.raster {
        campo(s, b"raster");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, pagado, _, lanzado, gpu_us, _) = raster::desempaquetar(v);
                if raster::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL RASTERIZADOR DE LA 3060 DIBUJO EL TRIANGULO: ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el triangulo 3D NO salio como dice el juez: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles donde el juez dice");
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo de la clase 3D ");
                s.text(if pagado { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b"   en ");
                s.dec(gpu_us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
                // Si no volvio: lo que el GSP conto de la 3060 (Xid, fallo de pagina).
                if !raster::sano(v) {
                    escalera(s);
                        super::super::gspcola::avisos(s, 4);
                }
            }
        }
    }
    if let Some(r) = c.color3d {
        campo(s, b"color");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, pagado, _, lanzado, gpu_us, _) = color3d::desempaquetar(v);
                if color3d::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL RASTERIZADOR MEZCLO LOS TRES COLORES: ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el triangulo con color NO salio como dice el juez: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles como dice el juez");
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo de la clase 3D ");
                s.text(if pagado { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b"   en ");
                s.dec(gpu_us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
                if !color3d::sano(v) {
                    escalera(s);
                        super::super::gspcola::avisos(s, 4);
                }
            }
        }
    }
}

/// **Hasta donde llego el dibujo** (la escalera de semaforos) y como quedo el
/// motor grafico, si no volvio.
fn escalera(s: &mut Output) {
    let leer = |k: u64| bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIAG_3D, k).ok();
    let Some(e) = leer(0) else { return };
    campo(s, b"escalera");
    for (bit, nombre) in [(1, b"estado" as &[u8]), (2, b"vertices"), (4, b"dibujo")] {
        s.text(nombre);
        s.with_ink(if e & bit != 0 { INK_GOOD } else { INK_ERR });
        s.text(if e & bit != 0 { b" SI  " as &[u8] } else { b" NO  " });
        s.with_ink(INK_PLAIN);
    }
    s.with_ink(INK_ECHO);
    s.text(match e & 7 {
        0 => b"-> se paro ANTES del dibujo: en el estado 3D o en la limpieza" as &[u8],
        1 => b"-> se paro en los VERTICES: el programa de vertice o lo que lo alimenta",
        3 => b"-> los vertices SI; se paro al RASTERIZAR o en el programa de PIXEL",
        7 => b"-> todos pagados: el dibujo acabo, lo que falla es el juez",
        _ => b"-> fuera de orden",
    });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    // Los escalones del estado: uno detras de cada metodo. El ultimo pagado
    // seguido dice que metodo NO acepto AMPERE_B.
    campo(s, b"estado");
    let pagados = leer(4).unwrap_or(0) | leer(5).unwrap_or(0) << 32;
    s.dec(pagados.count_ones() as u64);
    s.text(b" de ");
    s.dec(raster::N_ESCALONES as u64);
    s.text(b" escalones pagados; ");
    match raster::culpable(pagados) {
        None => {
            s.with_ink(INK_ERR);
            s.text(b"ni la limpieza de T1a paso (el canal ya estaba roto?)");
        }
        Some(m) if pagados.count_ones() == raster::N_ESCALONES => {
            s.with_ink(INK_GOOD);
            s.text(m.as_bytes());
        }
        Some(m) => {
            s.text(b"el metodo que lo rompe: ");
            s.with_ink(INK_ERR);
            s.text(m.as_bytes());
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    campo(s, b"gr");
    for (k, nombre) in [(1, b"INTR 0x" as &[u8]), (2, b"   EXCEPTION 0x"), (3, b"   STATUS 0x")] {
        s.text(nombre);
        s.hex(leer(k).unwrap_or(0), 8);
    }
    s.text(b"   (NV_PGRAPH_*, leidos al rendirse)\n");
}
