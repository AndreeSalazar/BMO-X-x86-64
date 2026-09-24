//! **`gpu canalgr`: M5 G1, EL CANAL DE GR0.** El segundo canal de BMO-X en la
//! 3060, con la MISMA receta que el de copia (VISTO en el metal el 24-09 a
//! las 14:55): `canal::GR`, chid 2, motor GR0, en la lista 0. Pedirlo
//! (`GSP_RM_ALLOC`), atarlo a GR0 (`BIND`) y meterlo en su lista
//! (`GPFIFO_SCHEDULE`), como `r535_chan_ramfc_write` de nouveau.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     tres RPC de hasta 5 s cada una
//!
//! No se le manda trabajo todavia: sin el contexto de oro (G2..G4) el motor
//! grafico no tiene de donde cargar su estado. Este canal es donde G3 lo
//! promovera y G4 creara la clase 3D.

use bmo_gpu_ga10x::canal::GR;
use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL, GSP_RM_CONTROL};
use bmo_gpu_ga10x::objeto::{self, CABECERA_ALLOC};
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::gspsalud::Contestada;
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy)]
struct Pedido {
    r: objeto::Respuesta,
    resultado: u32,
    espera_us: u64,
    numero: u32,
}

#[derive(Clone, Copy, Default)]
struct CanalGr {
    pedido: Option<Result<Pedido, u32>>,
    atar: Option<Result<Contestada, u32>>,
    programar: Option<Result<Contestada, u32>>,
}

static mut ESTADO: Option<CanalGr> = None;

fn estado() -> CanalGr {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ESTADO) }.unwrap_or_default()
}

fn con(f: impl FnOnce(&mut CanalGr)) {
    // SAFETY: como `estado`.
    f(unsafe { (*core::ptr::addr_of_mut!(ESTADO)).get_or_insert_with(CanalGr::default) })
}

/// El RM contesto, pero NO dio el canal de GR0.
pub(crate) const NO_CANALGR_NEGADO: u32 = 0x132;

fn bien(c: &Option<Result<Contestada, u32>>) -> bool {
    matches!(c, Some(Ok(c)) if c.bien())
}

fn pedido_bien(p: &Option<Result<Pedido, u32>>) -> bool {
    matches!(p, Some(Ok(p)) if p.r.estado == 0 && p.resultado == 0)
}

/// **G1: pedir el canal de GR0 y esperar al RM.** `Ok(su asa)`.
pub(crate) fn pedir() -> Result<u64, u32> {
    let r = (|| {
        let numero = (bmo::iommu_orden(bmo::IOMMU_OP_GPU_CANAL_GR)? >> 32) as u32;
        let mut d = [0u8; CABECERA_ALLOC];
        let (m, espera_us) = esperar(objeto::GSP_RM_ALLOC, &mut d, &mut Otros::default())?;
        let r = objeto::leer(&d).ok_or(NO_CANALGR_NEGADO)?;
        Ok(Pedido { r, resultado: m.resultado, espera_us, numero })
    })();
    con(|c| {
        c.pedido = Some(r);
        c.atar = None;
        c.programar = None;
    });
    if pedido_bien(&Some(r)) {
        Ok(GR.asa as u64)
    } else {
        Err(r.err().unwrap_or(NO_CANALGR_NEGADO))
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn pedido() -> bool {
    pedido_bien(&estado().pedido)
}

fn encender_con(c: Control) -> Result<Contestada, u32> {
    let que = Control::TODOS.iter().position(|&k| k == c).unwrap_or(0) as u64;
    let numero = (bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CANAL_ORDEN, que)? >> 32) as u32;
    let mut d = [0u8; CABECERA_CONTROL + 4];
    let (m, espera_us) = esperar(GSP_RM_CONTROL, &mut d, &mut Otros::default())?;
    let r = control::leer(&d).ok_or(super::gspsalud::NO_CONTROL_NEGADO)?;
    Ok(Contestada { r, resultado: m.resultado, espera_us, numero })
}

/// **G1: BIND a GR0 y GPFIFO_SCHEDULE**, en ese orden.
pub(crate) fn encender() -> Result<u64, u32> {
    let atar = encender_con(Control::AtarGr);
    con(|c| {
        c.atar = Some(atar);
        c.programar = None;
    });
    if !bien(&Some(atar)) {
        return Err(atar.err().unwrap_or(super::gspsalud::NO_CONTROL_NEGADO));
    }
    let programar = encender_con(Control::ProgramarGr);
    con(|c| c.programar = Some(programar));
    if !bien(&Some(programar)) {
        return Err(programar.err().unwrap_or(super::gspsalud::NO_CONTROL_NEGADO));
    }
    Ok(GR.motor as u64)
}

/// Lo pregunta `save mode`.
pub(crate) fn encendido() -> bool {
    let c = estado();
    bien(&c.atar) && bien(&c.programar)
}

/// `gpu canalgr`: pedirlo y encenderlo, parando en lo primero que no sale.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "pidiendole al GSP-RM el canal de GR0", INK_DIM);
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !encendido() {
        r = encender();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL MOTOR GRAFICO TIENE NUESTRO CANAL: atado a GR0 y en su lista (G1 de M5)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el canal de GR0 no quedo encendido: mira las filas `canal gr` y `atado gr`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "canalgr", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn no(s: &mut Output, m: u32) {
    s.with_ink(INK_ERR);
    s.text(b"NO: ");
    s.text(super::iommu::motivo(m));
    s.with_ink(INK_PLAIN);
}

fn estado_rm(s: &mut Output, ok: bool, estado: u32, resultado: u32, us: u64, numero: u32, orden: &[u8]) {
    s.with_ink(if ok { INK_GOOD } else { INK_ERR });
    s.text(objeto::estado(estado));
    if estado != 0 {
        s.text(b" (0x");
        s.hex(estado as u64, 2);
        s.byte(b')');
    }
    if resultado != 0 && resultado != estado {
        s.text(b", rpc_result 0x");
        s.hex(resultado as u64, 8);
    }
    s.with_ink(INK_ECHO);
    s.text(b"   ");
    s.text(orden);
    s.text(b" en ");
    s.dec(us / 1000);
    s.text(b" ms (numero ");
    s.dec(numero as u64);
    s.byte(b')');
    s.with_ink(INK_PLAIN);
}

/// **Las filas `canal gr` y `atado gr`**, si se pidio.
pub(crate) fn fila(s: &mut Output) {
    let c = estado();
    let Some(p) = c.pedido else { return };
    campo(s, b"canal gr");
    match p {
        Err(m) => no(s, m),
        Ok(p) => {
            s.with_ink(INK_ECHO);
            s.text(b"0x");
            s.hex(GR.asa as u64, 8);
            s.text(b" (chid ");
            s.dec(GR.chid as u64);
            s.text(b", GR0, instancia 0x");
            s.hex(GR.instancia, 9);
            s.text(b", GPFIFO VA 0x");
            s.hex(GR.gpfifo_va, 9);
            s.text(b"): ");
            estado_rm(s, pedido_bien(&Some(Ok(p))), p.r.estado, p.resultado, p.espera_us, p.numero, b"GSP_RM_ALLOC");
        }
    }
    s.byte(b'\n');
    for (nombre, orden, x) in [(b"atado gr" as &[u8], b"BIND GR0" as &[u8], c.atar), (b"en lista gr", b"GPFIFO_SCHEDULE", c.programar)] {
        let Some(x) = x else { continue };
        campo(s, nombre);
        match x {
            Err(m) => no(s, m),
            Ok(k) => estado_rm(s, k.bien(), k.r.estado, k.resultado, k.espera_us, k.numero, orden),
        }
        s.byte(b'\n');
    }
}
