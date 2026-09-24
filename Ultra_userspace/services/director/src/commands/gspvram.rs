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
//!
//! # `gpu directorio` (L1c3)
//!
//! La raiz de NUESTRO espacio de direcciones: el kernel pone a cero su pagina
//! (`vram::DIRECTORIO`, 65 MiB) y manda `SET_PAGE_DIRECTORY` con la direccion
//! y el espacio fijos; aqui se espera la respuesta del RM. Una vez por
//! arranque: despues el RM escribe en esa raiz.

use bmo_gpu_ga10x::control::{self, CABECERA_CONTROL, GSP_RM_CONTROL};
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

// == L1c3: EL DIRECTORIO ======================================================

#[derive(Clone, Copy, Default)]
struct Directorio {
    /// L1d0: las 4 entradas de la raiz, leidas DESPUES de que el RM la
    /// aceptara (`None` si no se pudo leer).
    raiz: [Option<u64>; 4],
    numero: u32,
    r: Option<control::Respuesta>,
    resultado: u32,
    espera_us: u64,
    no: u32,
}

static mut DIRECTORIO: Option<Directorio> = None;

fn directorio() -> Option<Directorio> {
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of!(DIRECTORIO) }
}

/// El RM contesto, pero no acepto el directorio.
pub(crate) const NO_DIRECTORIO_NEGADO: u32 = 0x127;

fn aceptado(d: &Directorio) -> bool {
    d.no == 0 && matches!(d.r, Some(r) if r.estado == 0) && d.resultado == 0
}

/// **Poner el directorio y esperar al RM.** `Ok(())` si lo acepto.
pub(crate) fn poner_directorio() -> Result<u64, u32> {
    let mut d = Directorio::default();
    match super::gsprpc::usable(vram::DIRECTORIO, 4 * vram::PALABRAS as u64) {
        None => d.no = NO_VRAM_SIN_REGIONES,
        Some(false) => d.no = NO_VRAM_NO_USABLE,
        Some(true) => match bmo::iommu_orden(bmo::IOMMU_OP_GPU_DIRECTORIO) {
            Ok(v) => d.numero = (v >> 32) as u32,
            Err(m) => d.no = m,
        },
    }
    if d.no == 0 {
        let mut b = [0u8; CABECERA_CONTROL + 4];
        let mut otros = super::gsprpc::Otros::default();
        match super::gsprpc::esperar(GSP_RM_CONTROL, &mut b, &mut otros) {
            Ok((m, us)) => {
                d.r = control::leer(&b);
                d.resultado = m.resultado;
                d.espera_us = us;
            }
            Err(no) => d.no = no,
        }
    }
    // L1d0: que escribio el RM en nuestra raiz al aceptarla.
    if aceptado(&d) {
        for k in 0..4 {
            d.raiz[k] = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_RAIZ, k as u64).ok();
        }
    }
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of_mut!(DIRECTORIO) = Some(d) };
    if aceptado(&d) {
        Ok(vram::DIRECTORIO)
    } else if d.no != 0 {
        Err(d.no)
    } else {
        Err(NO_DIRECTORIO_NEGADO)
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn directorio_puesto() -> bool {
    directorio().map_or(false, |d| aceptado(&d))
}

/// `gpu directorio`.
pub(crate) fn orden_directorio(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "poniendo el directorio de paginas", INK_DIM);
    let r = poner_directorio();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL RM ACEPTO NUESTRO DIRECTORIO DE PAGINAS: el espacio de la GPU ya tiene raiz, en tu VRAM\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el directorio no quedo puesto: mira la fila `pd`\n");
    }
    g.with_ink(INK_PLAIN);
    fila_directorio(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "directorio", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila `pd`**, si se pidio.
pub(crate) fn fila_directorio(s: &mut Output) {
    let Some(d) = directorio() else { return };
    campo(s, b"pd");
    if d.no != 0 {
        s.with_ink(INK_ERR);
        s.text(b"NO: ");
        s.text(super::iommu::motivo(d.no));
    } else if let Some(r) = d.r {
        s.with_ink(if aceptado(&d) { INK_GOOD } else { INK_ERR });
        s.text(b"raiz PD3 en 0x");
        s.hex(vram::DIRECTORIO, 9);
        s.text(b" (");
        s.dec(control::PD3_ENTRADAS as u64);
        s.text(b" entradas, a cero): ");
        s.text(bmo_gpu_ga10x::objeto::estado(r.estado));
        if r.estado != 0 {
            s.text(b" (0x");
            s.hex(r.estado as u64, 2);
            s.byte(b')');
        }
        s.with_ink(INK_PLAIN);
        s.text(b"   SET_PAGE_DIRECTORY en ");
        s.dec(d.espera_us / 1000);
        s.text(b" ms (numero ");
        s.dec(d.numero as u64);
        s.byte(b')');
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    // L1d0: la raiz, leida de vuelta. Lo que el RM colgo ahi es SUYO: L1d
    // mapea en las entradas que quedaron vacias, o bajo la suya sin pisarla.
    if d.raiz.iter().any(|e| e.is_some()) {
        campo(s, b"raiz");
        for (k, e) in d.raiz.iter().enumerate() {
            if k > 0 {
                s.text(b"; ");
            }
            s.byte(b'[');
            s.dec(k as u64);
            s.text(b"] ");
            match e.map(bmo_gpu_ga10x::mmu::pde) {
                None => s.text(b"sin leer"),
                Some(bmo_gpu_ga10x::mmu::Pde::Vacia) => s.text(b"vacia"),
                Some(bmo_gpu_ga10x::mmu::Pde::Vram(a)) => {
                    s.with_ink(INK_ECHO);
                    s.text(b"PD2 en VRAM 0x");
                    s.hex(a, 9);
                    s.with_ink(INK_PLAIN);
                }
                Some(bmo_gpu_ga10x::mmu::Pde::Sistema(a)) => {
                    s.text(b"PD2 en RAM 0x");
                    s.hex(a, 9);
                }
            }
        }
        s.byte(b'\n');
    }
}

// == L1d1: EL TRAMO ===========================================================

static mut TRAMO: Option<Result<u64, u32>> = None;

fn tramo() -> Option<Result<u64, u32>> {
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of!(TRAMO) }
}

fn tramo_sano(v: u64) -> bool {
    let (n, bien) = (v & 0xFFFF, (v >> 16) & 0xFFFF);
    n > 0 && n == bien
}

/// Las entradas se escribieron pero alguna no se releyo igual.
pub(crate) const NO_TRAMO_MAL: u32 = 0x128;

/// **Mapear el tramo** (L1d1): el kernel pone las tablas y relee.
pub(crate) fn mapear_tramo() -> Result<u64, u32> {
    let r = match super::gsprpc::usable(vram::TRAMO, 4096 * vram::TRAMO_PAGINAS as u64) {
        None => Err(NO_VRAM_SIN_REGIONES),
        Some(false) => Err(NO_VRAM_NO_USABLE),
        Some(true) => bmo::iommu_orden(bmo::IOMMU_OP_GPU_TRAMO),
    };
    // SAFETY: como `ultima`.
    unsafe { *core::ptr::addr_of_mut!(TRAMO) = Some(r) };
    match r {
        Ok(v) if tramo_sano(v) => Ok(v),
        Ok(_) => Err(NO_TRAMO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn tramo_puesto() -> bool {
    matches!(tramo(), Some(Ok(v)) if tramo_sano(v))
}

/// `gpu tramo`.
pub(crate) fn orden_tramo(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "mapeando el tramo en el espacio de la GPU", INK_DIM);
    let r = mapear_tramo();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LA GPU YA VE 64 KiB DE TU VRAM POR DIRECCION VIRTUAL: tablas escritas y releidas\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el tramo no quedo mapeado: mira la fila `tramo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila_tramo(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "tramo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila `tramo`**, si se pidio.
pub(crate) fn fila_tramo(s: &mut Output) {
    let Some(r) = tramo() else { return };
    campo(s, b"tramo");
    match r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(v) => {
            s.with_ink(if tramo_sano(v) { INK_GOOD } else { INK_ERR });
            s.text(b"VA 0x");
            s.hex(vram::TRAMO_VA, 9);
            s.text(b" -> VRAM 0x");
            s.hex(vram::TRAMO, 9);
            s.text(b", ");
            s.dec(vram::TRAMO_PAGINAS as u64);
            s.text(b" paginas: ");
            s.dec((v >> 16) & 0xFFFF);
            s.text(b" de ");
            s.dec(v & 0xFFFF);
            s.text(b" entradas releidas");
            s.with_ink(INK_ECHO);
            s.text(b"   PD2..PT en 0x");
            s.hex(vram::TABLAS[0], 9);
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}
