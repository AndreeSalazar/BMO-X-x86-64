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

/// Lo que llego antes de la respuesta, consumido: `(funcion, veces)`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Otros {
    pub t: [(u32, u32); MAX_TIPOS],
    pub n: usize,
}

impl Otros {
    fn contar(&mut self, funcion: u32) {
        match self.t[..self.n].iter_mut().find(|t| t.0 == funcion) {
            Some(t) => t.1 += 1,
            None if self.n < MAX_TIPOS => {
                self.t[self.n] = (funcion, 1);
                self.n += 1;
            }
            None => {}
        }
    }

    /// `; antes llego: X xN ...`, si llego algo.
    pub(crate) fn escribir(&self, s: &mut Output) {
        if self.n == 0 {
            return;
        }
        s.text(b"; antes llego:");
        for t in &self.t[..self.n] {
            s.byte(b' ');
            s.text(rpc::nombre(t.0));
            s.text(b" x");
            s.dec(t.1 as u64);
        }
    }
}

#[derive(Clone, Copy)]
struct Resumen {
    numero: u32,
    /// La respuesta, leida; y su `rpc_result`.
    e: Option<Estatica>,
    resultado: u32,
    espera_us: u64,
    otros: Otros,
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

/// **Esperar la respuesta `funcion`** en la cola del GSP (hasta 5 s): sus
/// datos en `d` (hasta donde quepan), y el mensaje y los us que tardo.
/// Lo demas que llegue se consume y se cuenta en `otros`.
pub(crate) fn esperar(funcion: u32, d: &mut [u8], otros: &mut Otros) -> Result<(Mensaje, u64), u32> {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let fin = desde + hz * ESPERA_S;
    while bmo::ciclos() < fin {
        let escrito = (mem(ESCRITO) & 0xFFFF_FFFF) % PAGINAS;
        let mut p = (mem(LEIDO_CPU) & 0xFFFF_FFFF) % PAGINAS;
        while p != escrito {
            let m = Mensaje::de(&cabecera(p));
            if !m.bien_formado() || suma(p, &m) != 0 {
                return Err(NO_RPC_SIN_RESPUESTA);
            }
            let mia = m.funcion == funcion;
            if mia {
                let n = m.datos().min(d.len());
                for o in (0..n).step_by(8) {
                    let w = cola(p, (rpc::CABECERA + o) as u64).to_le_bytes();
                    let k = (n - o).min(8);
                    d[o..o + k].copy_from_slice(&w[..k]);
                }
            } else {
                otros.contar(m.funcion);
            }
            p = (p + m.paginas as u64) % PAGINAS;
            if bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_LEIDO, p).is_err() {
                return Err(NO_RPC_SIN_RESPUESTA);
            }
            if mia {
                return Ok((m, (bmo::ciclos() - desde) * 1_000_000 / hz));
            }
        }
        bmo::yield_screen();
    }
    Err(NO_RPC_SIN_RESPUESTA)
}

/// **Preguntar y esperar la respuesta.** `Ok(rpc_result)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let mut r = Resumen { numero: 0, e: None, resultado: 0, espera_us: 0, otros: Otros::default(), no: 0 };
    match bmo::iommu_orden(bmo::IOMMU_OP_GSP_ESTATICA) {
        Ok(v) => r.numero = (v >> 32) as u32,
        Err(m) => {
            r.no = m;
            guardar(r);
            return Err(m);
        }
    }
    let mut d = [0u8; BYTES];
    match esperar(estatica::GET_GSP_STATIC_INFO, &mut d, &mut r.otros) {
        Ok((m, us)) => {
            r.e = estatica::leer(&d);
            r.resultado = m.resultado;
            r.espera_us = us;
        }
        Err(no) => r.no = no,
    }
    if r.e.is_none() && r.no == 0 {
        r.no = NO_RPC_SIN_RESPUESTA;
    }
    guardar(r);
    if r.e.is_none() {
        return Err(r.no);
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
    r.otros.escribir(s);
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

    // El bus, el tipo y la L2 no salen como dicen sus nombres (ver
    // `estatica.rs`): van crudos, con las mascaras, para leerlos en metal.
    campo(s, b"memoria");
    mib(s, e.vram);
    s.text(b" de VRAM");
    s.with_ink(INK_ECHO);
    s.text(b"; crudo: bus ");
    s.dec(e.bus_bits as u64);
    s.text(b", tipo ");
    s.dec(e.ram_tipo as u64);
    s.text(b", L2 ");
    s.dec(e.l2 as u64);
    s.text(b", fbio 0x");
    s.hex(e.fbio, 4);
    s.text(b", fbp 0x");
    s.hex(e.fbp, 4);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

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
