//! **`gpu gr`: M5 G0, LO QUE PIDE EL MOTOR GRAFICO.** Le pregunta al GSP-RM,
//! sobre sus asas INTERNAS (las de L1a), que buferes de contexto necesita GR0
//! y de que medida (`INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO`), y pinta
//! como los reparte nouveau: la receta de `r570_gr_get_ctxbufs_and_zcull_info`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     una pregunta de hasta 5 s
//!
//! Es el primer peldano del contexto de oro (G0..G4, en `bmo_gpu_ga10x::gr`):
//! no cambia nada en la 3060. Con estas medidas G2 buscara sitio en la VRAM.

use bmo_gpu_ga10x::control::{self, CABECERA_CONTROL, GSP_RM_CONTROL};
use bmo_gpu_ga10x::gr::{self, Bufer};
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy)]
struct Gr {
    numero: u32,
    estado: u32,
    resultado: u32,
    espera_us: u64,
    buferes: Option<[Bufer; gr::N]>,
}

static mut ULTIMO: Option<Result<Gr, u32>> = None;

fn ultimo() -> Option<Result<Gr, u32>> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ULTIMO) }
}

/// Motivos del escritorio.
/// Sin las asas internas de L1a (`gpu estatica`).
pub(crate) const NO_GR_SIN_ASAS: u32 = 0x130;
/// El RM contesto pero no dio los buferes (su NV_STATUS, en la fila).
pub(crate) const NO_GR_NEGADO: u32 = 0x131;

fn bien(g: &Gr) -> bool {
    g.estado == 0 && g.resultado == 0 && g.buferes.is_some()
}

fn preguntar_() -> Result<Gr, u32> {
    let (cliente, sub) = super::gsprpc::asas().ok_or(NO_GR_SIN_ASAS)?;
    let numero = (bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_GR, cliente as u64 | (sub as u64) << 32)? >> 32) as u32;
    let mut d = [0u8; CABECERA_CONTROL + gr::MEDIDA];
    let (m, espera_us) = esperar(GSP_RM_CONTROL, &mut d, &mut Otros::default())?;
    let r = control::leer(&d).ok_or(NO_GR_NEGADO)?;
    let buferes = if r.estado == 0 { gr::buferes(&d) } else { None };
    Ok(Gr { numero, estado: r.estado, resultado: m.resultado, espera_us, buferes })
}

/// **G0: preguntar.** `Ok(lo que ocupan, en bytes)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let r = preguntar_();
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMO) = Some(r) };
    match r {
        Ok(g) if bien(&g) => Ok(g.buferes.map_or(0, |t| gr::total(&t))),
        Ok(_) => Err(NO_GR_NEGADO),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn hecho() -> bool {
    matches!(ultimo(), Some(Ok(g)) if bien(&g))
}

/// Los buferes, para G2.
pub(crate) fn buferes() -> Option<[Bufer; gr::N]> {
    match ultimo() {
        Some(Ok(g)) if bien(&g) => g.buferes,
        _ => None,
    }
}

// == G2: LOS BUFERES EN VRAM, MAPEADOS ======================================

static mut MEMORIA: Option<Result<u64, u32>> = None;

fn memoria() -> Option<Result<u64, u32>> {
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of!(MEMORIA) }
}

/// Sin los buferes de G0 (`gpu gr`), o no caben en lo que G2 mapea.
pub(crate) const NO_GR_SIN_BUFERES: u32 = 0x133;
/// Las entradas se escribieron pero alguna no se releyo igual.
pub(crate) const NO_GR_MAPEO_MAL: u32 = 0x134;

fn memoria_sana(v: u64) -> bool {
    let (n, bien) = (v & 0xFFFF, (v >> 16) & 0xFFFF);
    n > 0 && n == bien
}

/// **G2: los buferes en VRAM, mapeados.** El reparto lo hace el escritorio
/// con lo que dijo G0; el kernel solo recibe cuanto mapear y hasta donde poner
/// a cero. Antes, que todo caiga en VRAM que el GSP-RM dio como usable.
pub(crate) fn mapear() -> Result<u64, u32> {
    let r = (|| {
        let t = buferes().ok_or(NO_GR_SIN_BUFERES)?;
        let (_, bytes, cero) = gr::repartir(&t).ok_or(NO_GR_SIN_BUFERES)?;
        match super::gsprpc::usable(gr::VRAM, bytes) {
            None => return Err(super::gspvram::NO_VRAM_SIN_REGIONES),
            Some(false) => return Err(super::gspvram::NO_VRAM_NO_USABLE),
            Some(true) => {}
        }
        bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_GR_MEMORIA, bytes | cero << 32)
    })();
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(MEMORIA) = Some(r) };
    match r {
        Ok(v) if memoria_sana(v) => Ok(v),
        Ok(_) => Err(NO_GR_MAPEO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn mapeado() -> bool {
    matches!(memoria(), Some(Ok(v)) if memoria_sana(v))
}

/// `gpu grmem`.
pub(crate) fn orden_memoria(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "poniendo los buferes de GR en tu VRAM", INK_DIM);
    let r = mapear();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LOS BUFERES DEL MOTOR GRAFICO YA ESTAN EN TU VRAM Y LA GPU LOS VE (G2 de M5)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  los buferes de GR no quedaron mapeados: mira la fila `gr memoria`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "grmem", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu gr`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "preguntandole al GSP-RM que necesita el motor grafico", INK_DIM);
    let r = preguntar();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL MOTOR GRAFICO DIJO LO QUE NECESITA: los buferes de su contexto de oro (G0 de M5)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el GSP-RM no dijo los buferes de GR: mira la fila `gr`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "gr", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn kib(s: &mut Output, b: u64) {
    if b >= 1 << 20 && b % (1 << 20) == 0 {
        s.dec(b >> 20);
        s.text(b" MiB");
    } else {
        s.dec((b + 1023) >> 10);
        s.text(b" KiB");
    }
}

/// **Las filas `gr` y `gr bufer`**, si se pregunto.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = ultimo() else { return };
    campo(s, b"gr");
    let g = match r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
            return;
        }
        Ok(g) => g,
    };
    match g.buferes {
        Some(t) if bien(&g) => {
            s.with_ink(INK_GOOD);
            s.dec(gr::N as u64);
            s.text(b" buferes para el contexto de oro de GR0, ");
            kib(s, gr::total(&t));
            s.text(b" en VRAM");
            s.with_ink(INK_PLAIN);
        }
        _ => {
            s.with_ink(INK_ERR);
            s.text(bmo_gpu_ga10x::objeto::estado(g.estado));
            s.text(b" (0x");
            s.hex(g.estado as u64, 2);
            s.byte(b')');
            s.with_ink(INK_PLAIN);
        }
    }
    s.with_ink(INK_ECHO);
    s.text(b"   KGR_GET_CONTEXT_BUFFERS_INFO en ");
    s.dec(g.espera_us / 1000);
    s.text(b" ms (numero ");
    s.dec(g.numero as u64);
    s.byte(b')');
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    if let Some(t) = g.buferes {
        for b in &t {
            campo(s, b"gr bufer");
            s.text(b.nombre);
            s.text(b": ");
            kib(s, b.medida);
            s.with_ink(INK_ECHO);
            s.text(b" (el RM dijo ");
            s.dec(b.medida_rm as u64);
            s.text(b" B), paginas de ");
            kib(s, 1 << b.pagina);
            s.text(b", alineado a ");
            kib(s, 1 << b.alinear);
            if b.global {
                s.text(b", global");
            }
            if b.iniciar {
                s.text(b", lo llena el RM");
            }
            if b.ro {
                s.text(b", solo lectura");
            }
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
        super::datos::anotar(b"gpu gr total", gr::total(&t), b"B");
    }
    fila_memoria(s);
}

/// **La fila `gr memoria`** (G2), si se pidio; y donde quedo cada bufer.
fn fila_memoria(s: &mut Output) {
    let Some(r) = memoria() else { return };
    campo(s, b"gr memoria");
    match r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(v) => {
            s.with_ink(if memoria_sana(v) { INK_GOOD } else { INK_ERR });
            s.text(b"VRAM 0x");
            s.hex(gr::VRAM, 9);
            s.text(b" -> VA 0x");
            s.hex(gr::VA, 9);
            s.text(b": ");
            s.dec((v >> 16) & 0xFFFF);
            s.text(b" de ");
            s.dec(v & 0xFFFF);
            s.text(b" entradas releidas");
            s.with_ink(INK_ECHO);
            s.text(b"; ");
            s.dec(v >> 32);
            s.text(b" paginas a cero (las que llena el RM); tablas en 0x");
            s.hex(gr::TABLAS, 9);
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    if let Some((c, _, _)) = buferes().and_then(|t| gr::repartir(&t)) {
        campo(s, b"gr donde");
        s.with_ink(INK_ECHO);
        for (k, x) in c.iter().enumerate() {
            if k > 0 {
                s.text(b", ");
            }
            s.text(x.b.nombre);
            s.text(b" +0x");
            s.hex(x.off, 7);
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}
