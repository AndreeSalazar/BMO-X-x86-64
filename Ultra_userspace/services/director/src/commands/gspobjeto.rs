//! **`gpu objetos`: L1b, NUESTROS OBJETOS EN EL RM.** Le pide al GSP-RM, por
//! `GSP_RM_ALLOC`, un cliente, un dispositivo y un subdispositivo propios, uno
//! detras de otro: cada uno es hijo del anterior. Las asas son fijas
//! (`bmo_gpu_ga10x::objeto`); el kernel arma cada pregunta, el escritorio solo
//! dice cual y espera la respuesta en la cola del GSP.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o tras `gpu init`:
//!                     tres ordenes y hasta 5 s esperando cada respuesta
//!
//! Pedirlos otra vez en el mismo arranque contesta `ya existia`
//! (`NV_ERR_INSERT_DUPLICATE_NAME`): el objeto sigue ahi y vale igual.

use bmo_gpu_ga10x::objeto::{self, Objeto, Respuesta, CABECERA_ALLOC, YA_EXISTE};
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy, Default)]
struct Uno {
    numero: u32,
    r: Option<Respuesta>,
    resultado: u32,
    espera_us: u64,
    /// El NO del kernel al pedir, o el del escritorio al esperar.
    no: u32,
}

#[derive(Clone, Copy, Default)]
struct Resumen {
    uno: [Uno; 3],
    /// Cuantos se pidieron (se para en el primero que no sale).
    n: usize,
    otros: Otros,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

fn guardar(r: Resumen) {
    // SAFETY: como `resumen`.
    unsafe { *core::ptr::addr_of_mut!(RESUMEN) = Some(r) };
}

/// La numero de la pregunta que CREO cada objeto (con `NV_OK`), para todo el
/// arranque: pedirlos otra vez contesta `ya existia` y eso no lo borra.
static mut CREADO: [u32; 3] = [0; 3];

fn creado() -> [u32; 3] {
    // SAFETY: como `resumen`.
    unsafe { *core::ptr::addr_of!(CREADO) }
}

/// Existe: lo creo ahora, o ya estaba de antes. El GSP-RM pone el mismo
/// `NV_STATUS` en el `rpc_result` (metal 24-09 10:52: 0x19 en los dos).
fn vale(u: &Uno) -> bool {
    u.no == 0
        && matches!(u.r, Some(r) if (r.estado == 0 || r.estado == YA_EXISTE)
            && (u.resultado == 0 || u.resultado == r.estado))
}

/// Motivo del escritorio: el RM contesto, pero NO dio el objeto.
pub(crate) const NO_OBJ_NEGADO: u32 = 0x122;

/// **Pedir los tres.** `Ok(())` si los tres existen.
pub(crate) fn pedir() -> Result<(), u32> {
    let mut r = Resumen::default();
    for k in 0..Objeto::TODOS.len() {
        let u = &mut r.uno[k];
        r.n = k + 1;
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_OBJETO, k as u64) {
            Ok(v) => u.numero = (v >> 32) as u32,
            Err(m) => u.no = m,
        }
        if u.no == 0 {
            let mut d = [0u8; CABECERA_ALLOC];
            match esperar(objeto::GSP_RM_ALLOC, &mut d, &mut r.otros) {
                Ok((m, us)) => {
                    u.r = objeto::leer(&d);
                    u.resultado = m.resultado;
                    u.espera_us = us;
                }
                Err(no) => u.no = no,
            }
        }
        if vale(u) && matches!(u.r, Some(x) if x.estado == 0) {
            // SAFETY: como `resumen`.
            unsafe { (*core::ptr::addr_of_mut!(CREADO))[k] = u.numero };
        }
        if !vale(u) {
            let no = if u.no != 0 { u.no } else { NO_OBJ_NEGADO };
            guardar(r);
            return Err(no);
        }
    }
    guardar(r);
    Ok(())
}

/// `gpu objetos`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "pidiendole objetos al GSP-RM", INK_DIM);
    let r = pedir();
    let g = &mut dsk.out.grid;
    match r {
        Ok(()) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL RM TIENE NUESTROS OBJETOS: cliente, dispositivo y subdispositivo, pedidos por GSP_RM_ALLOC\n");
        }
        Err(_) => {
            g.with_ink(INK_ERR);
            g.text(b"  el GSP-RM no dio los tres objetos: mira las filas `obj`\n");
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "objetos", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas de L1b**, si se pidieron.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    for (k, u) in r.uno[..r.n].iter().enumerate() {
        let o = Objeto::TODOS[k];
        campo(s, [b"obj cli" as &[u8], b"obj disp", b"obj sub"][k]);
        s.with_ink(INK_ECHO);
        s.text(o.nombre());
        s.text(b" 0x");
        s.hex(o.asa() as u64, 8);
        s.text(b" (clase 0x");
        s.hex(o.forma().3 as u64, 4);
        s.text(b"): ");
        if u.no != 0 {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(u.no));
        } else if let Some(x) = u.r {
            s.with_ink(if vale(u) { INK_GOOD } else { INK_ERR });
            s.text(objeto::estado(x.estado));
            if x.estado != 0 {
                s.text(b" (0x");
                s.hex(x.estado as u64, 2);
                s.byte(b')');
            }
            if u.resultado != 0 {
                s.text(b", rpc_result 0x");
                s.hex(u.resultado as u64, 8);
            }
            s.with_ink(INK_PLAIN);
            s.text(b" en ");
            s.dec(u.espera_us / 1000);
            s.text(b" ms (numero ");
            s.dec(u.numero as u64);
            s.byte(b')');
            let c = creado()[k];
            if x.estado == YA_EXISTE && c != 0 {
                s.text(b"; CREADO con NV_OK en la numero ");
                s.dec(c as u64);
            }
        }
        s.with_ink(INK_PLAIN);
        if k + 1 == r.n {
            r.otros.escribir(s);
        }
        s.byte(b'\n');
    }
    let listos = r.uno[..r.n].iter().filter(|u| vale(u)).count();
    super::datos::anotar(b"gpu gsp rpc objetos", listos as u64, b"de 3");
}
