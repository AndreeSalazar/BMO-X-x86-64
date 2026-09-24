//! **`gpu motores`: L1d2a, LO QUE EL CANAL NECESITA SABER ANTES DE NACER** --
//! dos preguntas de solo lectura al GSP-RM sobre NUESTRO subdispositivo:
//! que motores tiene la 3060 (`GET_ENGINES_V2`), de donde sale el de COPIA al
//! que se atara el canal, y cuanto mide el bufer de metodos de ese canal
//! (`CE_GET_FAULT_METHOD_BUFFER_SIZE`), que se le presta en L1d2b.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     dos preguntas de hasta 5 s cada una
//!
//! Ninguna de las dos cambia nada en la 3060: el canal (L1d2c) se pide con
//! lo que digan.

use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL, MAX_MOTORES};
use bmo_userland as bmo;

use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy)]
struct Motores {
    lista: [u32; MAX_MOTORES],
    n: usize,
    motores: Result<Contestada, u32>,
    metodos: Result<Contestada, u32>,
}

static mut MOTORES: Option<Motores> = None;

fn ultimo() -> Option<Motores> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(MOTORES) }
}

fn bien(c: &Result<Contestada, u32>) -> bool {
    matches!(c, Ok(c) if c.bien())
}

/// **Las dos preguntas.** `Ok(el motor de copia elegido)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let mut d = [0u8; CABECERA_CONTROL + 4 + 4 * MAX_MOTORES];
    let motores = controlar(Control::Motores, &mut d);
    let (lista, n) = if bien(&motores) { control::motores(&d) } else { ([0; MAX_MOTORES], 0) };
    let metodos = match motores {
        Err(m) => Err(m),
        Ok(_) => controlar(Control::Metodos, &mut [0u8; CABECERA_CONTROL + 4]),
    };
    let u = Motores { lista, n, motores, metodos };
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(MOTORES) = Some(u) };
    for c in [u.motores, u.metodos] {
        match c {
            Err(m) => return Err(m),
            Ok(c) if !c.bien() => return Err(super::gspsalud::NO_CONTROL_NEGADO),
            Ok(_) => {}
        }
    }
    copia().map(|t| t as u64).ok_or(NO_MOTORES_SIN_COPIA)
}

/// Motivo del escritorio: la lista de motores no trae ninguno de copia.
pub(crate) const NO_MOTORES_SIN_COPIA: u32 = 0x129;

/// **El motor de COPIA del canal**: el primero de la lista (`NV2080_ENGINE_TYPE_COPYn`).
pub(crate) fn copia() -> Option<u32> {
    let u = ultimo()?;
    u.lista[..u.n].iter().copied().find(|t| control::motor(*t).0 == b"COPY")
}

/// Lo pregunta `save mode`: las dos contestadas y un motor de copia.
pub(crate) fn hecho() -> bool {
    ultimo().map_or(false, |u| bien(&u.motores) && bien(&u.metodos)) && copia().is_some()
}

/// `gpu motores`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "preguntandole al GSP-RM los motores de la 3060", INK_DIM);
    let r = preguntar();
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LOS MOTORES DE LA 3060, segun el GSP-RM: el canal ira sobre el de copia\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  no se supo que motores tiene: mira las filas `motores` y `metodos`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "motores", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn no(s: &mut Output, c: &Result<Contestada, u32>) -> bool {
    match c {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(*m));
        }
        Ok(c) if !c.bien() => {
            s.with_ink(INK_ERR);
            s.text(bmo_gpu_ga10x::objeto::estado(c.r.estado));
            s.text(b" (0x");
            s.hex(c.r.estado as u64, 2);
            s.text(b"; rpc_result 0x");
            s.hex(c.resultado as u64, 2);
            s.byte(b')');
        }
        Ok(_) => return false,
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    true
}

fn tras(s: &mut Output, c: &Contestada, orden: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"   ");
    s.text(orden);
    s.text(b" en ");
    s.dec(c.espera_us / 1000);
    s.text(b" ms (numero ");
    s.dec(c.numero as u64);
    s.byte(b')');
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Las filas `motores` y `metodos`**, si se pregunto.
pub(crate) fn fila(s: &mut Output) {
    let Some(u) = ultimo() else { return };
    campo(s, b"motores");
    if !no(s, &u.motores) {
        s.with_ink(INK_GOOD);
        s.dec(u.n as u64);
        s.text(b":");
        for &t in &u.lista[..u.n] {
            let (nombre, k) = control::motor(t);
            s.byte(b' ');
            s.text(nombre);
            s.dec(k as u64);
        }
        s.with_ink(INK_PLAIN);
        match copia() {
            Some(t) => {
                let (nombre, k) = control::motor(t);
                s.text(b"; el de copia del canal: ");
                s.text(nombre);
                s.dec(k as u64);
                s.text(b" (tipo 0x");
                s.hex(t as u64, 2);
                s.byte(b')');
            }
            None => {
                s.with_ink(INK_ERR);
                s.text(b"; NINGUNO de copia");
                s.with_ink(INK_PLAIN);
            }
        }
        if let Ok(c) = u.motores {
            tras(s, &c, b"GET_ENGINES_V2");
        }
    }
    campo(s, b"metodos");
    if !no(s, &u.metodos) {
        if let Ok(c) = u.metodos {
            s.with_ink(INK_GOOD);
            s.text(b"el bufer de metodos del canal: ");
            s.dec(c.r.valor as u64);
            s.text(b" B (0x");
            s.hex(c.r.valor as u64, 5);
            s.byte(b')');
            s.with_ink(INK_PLAIN);
            tras(s, &c, b"CE_GET_FAULT_METHOD_BUFFER_SIZE");
        }
    }
}
