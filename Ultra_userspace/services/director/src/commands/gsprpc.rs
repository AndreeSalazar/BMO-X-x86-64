//! **`gpu estatica`: L1a, LA PRIMERA RPC.** Le pregunta al GSP-RM
//! `GET_GSP_STATIC_INFO` --el kernel arma la pregunta y toca su timbre-- y
//! espera su respuesta en la cola del GSP: el nombre de la 3060 segun el
//! GSP-RM, su VRAM, su bus, su L2, las tablas de BAR1 y BAR2, las regiones de
//! VRAM que deja usar, y las ASAS internas del RM (cliente, dispositivo,
//! subdispositivo), que son con las que L1b le pedira objetos.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o tras `gpu init`:
//!                     una orden y hasta 5 s esperando la respuesta
//!
//! Lo que llegue antes de la respuesta (NOCAT, LIBOS_PRINT...) se consume y se
//! cuenta, como hace nova-core (`receive_msg`).

use bmo_gpu_ga10x::estatica::{self, Estatica, BYTES};
use bmo_gpu_ga10x::rpc::{self, Mensaje};
use bmo_userland as bmo;

use super::gspcola::{cabecera, cola, mem, suma, ESCRITO, LEIDO_CPU, PAGINAS};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

const ESPERA_S: u64 = 5;
const MAX_TIPOS: usize = 6;

#[derive(Clone, Copy)]
struct Resumen {
    numero: u32,
    /// La respuesta, leida; y su `rpc_result`.
    e: Option<Estatica>,
    resultado: u32,
    espera_us: u64,
    /// Lo que llego antes, consumido.
    otros: [(u32, u32); MAX_TIPOS],
    n_otros: usize,
    /// El NO del kernel al preguntar, o el del escritorio.
    no: u32,
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

/// Motivo del escritorio (`gspinit.rs` va hasta 0x120).
pub(crate) const NO_RPC_SIN_RESPUESTA: u32 = 0x121;

/// **Preguntar y esperar la respuesta.** `Ok(rpc_result)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let mut r = Resumen { numero: 0, e: None, resultado: 0, espera_us: 0, otros: [(0, 0); MAX_TIPOS], n_otros: 0, no: 0 };
    match bmo::iommu_orden(bmo::IOMMU_OP_GSP_ESTATICA) {
        Ok(v) => r.numero = (v >> 32) as u32,
        Err(m) => {
            r.no = m;
            guardar(r);
            return Err(m);
        }
    }
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let fin = desde + hz * ESPERA_S;
    'fuera: while bmo::ciclos() < fin {
        let escrito = (mem(ESCRITO) & 0xFFFF_FFFF) % PAGINAS;
        let mut p = (mem(LEIDO_CPU) & 0xFFFF_FFFF) % PAGINAS;
        while p != escrito {
            let m = Mensaje::de(&cabecera(p));
            if !m.bien_formado() || suma(p, &m) != 0 {
                break 'fuera;
            }
            if m.funcion == estatica::GET_GSP_STATIC_INFO {
                let mut d = [0u8; BYTES];
                let n = m.datos().min(BYTES);
                for o in (0..n).step_by(8) {
                    let w = cola(p, (rpc::CABECERA + o) as u64).to_le_bytes();
                    let k = (n - o).min(8);
                    d[o..o + k].copy_from_slice(&w[..k]);
                }
                r.e = estatica::leer(&d);
                r.resultado = m.resultado;
                r.espera_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
            } else {
                match r.otros[..r.n_otros].iter_mut().find(|t| t.0 == m.funcion) {
                    Some(t) => t.1 += 1,
                    None if r.n_otros < MAX_TIPOS => {
                        r.otros[r.n_otros] = (m.funcion, 1);
                        r.n_otros += 1;
                    }
                    None => {}
                }
            }
            p = (p + m.paginas as u64) % PAGINAS;
            if bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_LEIDO, p).is_err() {
                break 'fuera;
            }
            if r.e.is_some() {
                break 'fuera;
            }
        }
        bmo::yield_screen();
    }
    if r.e.is_none() {
        r.no = NO_RPC_SIN_RESPUESTA;
    }
    guardar(r);
    if r.e.is_none() {
        return Err(NO_RPC_SIN_RESPUESTA);
    }
    Ok(r.resultado as u64)
}

/// `gpu estatica`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "preguntandole al GSP-RM", INK_DIM);
    let r = preguntar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(0) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL GSP-RM CONTESTO GET_GSP_STATIC_INFO: la primera RPC, pregunta y respuesta\n");
        }
        Ok(v) => {
            g.with_ink(INK_ERR);
            g.text(b"  el GSP-RM contesto con rpc_result 0x");
            g.hex(v, 8);
            g.byte(b'\n');
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.byte(b'\n');
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "estatica", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn mib(s: &mut Output, bytes: u64) {
    s.dec(bytes >> 20);
    s.text(b" MiB");
}

/// **Las filas de L1a**, si se pregunto.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"rpc");
    let Some(e) = r.e else {
        s.with_ink(INK_ERR);
        s.text(b"GET_GSP_STATIC_INFO sin respuesta: ");
        s.text(super::iommu::motivo(r.no));
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        return;
    };
    s.with_ink(if r.resultado == 0 { INK_GOOD } else { INK_ERR });
    s.text(b"GET_GSP_STATIC_INFO (numero ");
    s.dec(r.numero as u64);
    s.text(b") CONTESTADA en ");
    s.dec(r.espera_us / 1000);
    s.text(b" ms, rpc_result 0x");
    s.hex(r.resultado as u64, 8);
    s.with_ink(INK_PLAIN);
    if r.n_otros > 0 {
        s.text(b"; antes llego:");
        for t in &r.otros[..r.n_otros] {
            s.byte(b' ');
            s.text(rpc::nombre(t.0));
            s.text(b" x");
            s.dec(t.1 as u64);
        }
    }
    s.byte(b'\n');

    campo(s, b"nombre");
    s.with_ink(INK_GOOD);
    s.text(estatica::texto(&e.nombre));
    s.with_ink(INK_ECHO);
    s.text(b"  (");
    s.text(estatica::texto(&e.corto));
    s.text(if e.uefi { b", arranco por UEFI)" as &[u8] } else { b")" });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    campo(s, b"memoria");
    mib(s, e.vram);
    s.text(b" de VRAM, bus de ");
    s.dec(e.bus_bits as u64);
    s.text(b" bits, tipo ");
    s.dec(e.ram_tipo as u64);
    s.text(b"; L2 ");
    s.dec(e.l2 as u64 >> 10);
    s.text(b" KiB\n");

    campo(s, b"regiones");
    let usables = e.regiones[..e.n_regiones].iter().filter(|g| g.usable).count();
    s.dec(e.n_regiones as u64);
    s.text(b" regiones de VRAM, ");
    s.dec(usables as u64);
    s.text(b" usables");
    for g in e.regiones[..e.n_regiones].iter().filter(|g| g.usable).take(2) {
        s.text(b"; 0x");
        s.hex(g.base, 9);
        s.text(b"..0x");
        s.hex(g.limit, 9);
        s.text(b" (");
        mib(s, g.limit.saturating_sub(g.base) + 1);
        s.byte(b')');
    }
    s.byte(b'\n');

    campo(s, b"asas");
    s.with_ink(INK_ECHO);
    s.text(b"cliente 0x");
    s.hex(e.cliente as u64, 8);
    s.text(b", dispositivo 0x");
    s.hex(e.dispositivo as u64, 8);
    s.text(b", subdispositivo 0x");
    s.hex(e.subdispositivo as u64, 8);
    s.with_ink(INK_PLAIN);
    s.text(b"; BAR1 PDE 0x");
    s.hex(e.bar1_pde, 9);
    s.text(b", BAR2 PDE 0x");
    s.hex(e.bar2_pde, 9);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp rpc estatica", r.resultado as u64, b"");
}
