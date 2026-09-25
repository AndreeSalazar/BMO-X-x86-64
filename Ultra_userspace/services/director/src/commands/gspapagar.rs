//! **`gpu apagar`: L0c5, EL GSP APAGADO EN ORDEN.** Lo contrario de `gpu
//! despertar`: el GSP-RM se despide, FWSEC-SB cierra lo suyo y el booter de
//! DESCARGA baja la WPR2. Asi el siguiente arranque encuentra la 3060 limpia
//! y su booter no sale con `0x15`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o antes de
//!                     `reboot`: una RPC y dos firmwares, hasta ~12 s
//!
//! # El orden (nouveau, `tu102_gsp_fini`)
//!
//! ```text
//!    despedir    UNLOADING_GUEST_DRIVER (47); se espera su respuesta O que
//!                se suspenda, lo que llegue antes (hasta 5 s)
//!    suspendido  el MAILBOX0 del GSP = 0x80000000 (hasta 2 s)
//!    cerrar      FWSEC-SB en el falcon del GSP; se espera que pare (5 s)
//!    descargar   el booter de descarga en el SEC2; se espera que pare (5 s)
//!    hecho       la WPR2 abajo (0x1FA828 = 0)
//! ```
//!
//! Como nouveau, un paso de en medio que no sale (sin respuesta, sin
//! suspenderse, SB con error) NO para el apagado: se apunta y se sigue.
//!
//! Los pasos los da el kernel (`dev/gpu_apagar.rs`) y los lee EN VIVO de los
//! falcons (`IOMMU_OP_GSP_APAGADO`); aqui solo se espera entre medias,
//! cediendo el turno. Despues del primer paso el GSP-RM ya no contesta: el
//! kernel no deja salir ni una RPC mas.

use bmo_gpu_ga10x::descarga as dc;
use bmo_userland as bmo;

use super::gsprpc::{esperar_o, Otros};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

// Los bits de `IOMMU_OP_GSP_APAGADO` (espejo de `dev/gpu_apagar.rs`).
const DESPEDIDO: u64 = 1 << 0;
const SUSPENDIDO: u64 = 1 << 1;
const SB: u64 = 1 << 2;
const SB_PARADO: u64 = 1 << 3;
const SB_BIEN: u64 = 1 << 4;
const DESCARGADOR: u64 = 1 << 5;
const SEC2_PARADO: u64 = 1 << 6;
const HECHO: u64 = 1 << 7;
const SB_ERROR_SHIFT: u64 = 8;
const BUZON_SHIFT: u64 = 32;

/// Motivos del escritorio (`gspcomputo.rs` va hasta 0x144).
pub(crate) const NO_APAGAR_NO_SUSPENDE: u32 = 0x145;
pub(crate) const NO_APAGAR_SB_NO_ACABA: u32 = 0x146;
pub(crate) const NO_APAGAR_SB_MAL: u32 = 0x147;
pub(crate) const NO_APAGAR_NO_DESCARGA: u32 = 0x148;

/// Lo que dejo el ultimo intento.
#[derive(Clone, Copy)]
struct Resumen {
    /// El `rpc_result` de la despedida y lo que tardo en contestar.
    respuesta: Option<(u32, u64)>,
    otros: Otros,
    /// Lo que tardo todo, en ms.
    ms: u64,
    /// El primer paso que no salio y no paro el apagado.
    aviso: Option<u32>,
    r: Result<u64, u32>,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

fn info() -> u64 {
    bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_APAGADO, 0).unwrap_or(0)
}

/// Esperar, cediendo el turno, hasta `ms`, a que el apagado cumpla.
fn esperar_bits(ms: u64, cumple: impl Fn(u64) -> bool) -> bool {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let fin = bmo::ciclos() + hz / 1000 * ms;
    loop {
        if cumple(info()) {
            return true;
        }
        if bmo::ciclos() >= fin {
            return false;
        }
        bmo::yield_screen();
    }
}

/// Si hay algo que apagar: el GSP-RM se vio despierto y no esta ya apagado.
pub(crate) fn hace_falta() -> bool {
    super::gsp::despierto() && info() & HECHO == 0
}

/// Lo pregunta `save mode` y la fila: el GSP quedo apagado en orden.
pub(crate) fn hecho() -> bool {
    info() & HECHO != 0
}

/// Los pasos, como `tu102_gsp_fini`: lo que no sale se APUNTA (`r.aviso`) y
/// se sigue, porque lo unico que cuenta es que la WPR2 acabe abajo.
fn pasos(r: &mut Resumen) -> Result<u64, u32> {
    if info() & DESPEDIDO == 0 {
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_DESPEDIR, 0)?;
        // ** La respuesta O la suspension, lo que llegue antes (25-09). En el
        // metal (20:36 y 21:36) la 570.144 se suspende SIN contestar, y se
        // esperaban los 5 s enteros: 5 de los 7 del apagado. La cola se sigue
        // vaciando mientras (18 NOCAT llegaron ahi).
        let mut d = [0u8; dc::BYTES];
        match esperar_o(dc::UNLOADING_GUEST_DRIVER, &mut d, &mut r.otros, || info() & SUSPENDIDO != 0) {
            Ok((m, us)) => r.respuesta = Some((m.resultado, us)),
            Err(_) if info() & SUSPENDIDO != 0 => {}
            Err(m) => avisar(r, m),
        }
    }
    if info() & SB == 0 {
        if !esperar_bits(dc::ESPERA_SUSPENDIDO_MS, |v| v & SUSPENDIDO != 0) {
            avisar(r, NO_APAGAR_NO_SUSPENDE);
        }
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_CERRAR, 0)?;
    }
    if info() & DESCARGADOR == 0 {
        if !esperar_bits(5000, |v| v & SB_PARADO != 0) {
            avisar(r, NO_APAGAR_SB_NO_ACABA);
        } else if info() & SB_BIEN == 0 {
            avisar(r, NO_APAGAR_SB_MAL);
        }
        bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_DESCARGAR, 0)?;
    }
    if !esperar_bits(5000, |v| v & SEC2_PARADO != 0) || info() & HECHO == 0 {
        return Err(NO_APAGAR_NO_DESCARGA);
    }
    Ok(info())
}

/// El primer tropiezo que no para el apagado.
fn avisar(r: &mut Resumen, m: u32) {
    r.aviso = r.aviso.or(Some(m));
}

/// **Apagarlo**, entero. Lo llama tambien `reboot` (y ya no `save mode`: ver
/// `pasos.rs`).
pub(crate) fn apagar() -> Result<u64, u32> {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut r = Resumen { respuesta: None, otros: Otros::default(), ms: 0, aviso: None, r: Ok(0) };
    r.r = pasos(&mut r);
    r.ms = (bmo::ciclos() - desde) * 1000 / hz;
    // SAFETY: como `resumen`.
    unsafe { *core::ptr::addr_of_mut!(RESUMEN) = Some(r) };
    r.r
}

/// `gpu apagar`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !super::gsp::despierto() {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ECHO);
        g.text(b"  el GSP no se desperto en este arranque: no hay nada que apagar\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu apagar") {
        dsk.field.n = 0;
        return After::Settle;
    }
    // Antes del apagado: la 3060 deja de volcar el escritorio y devuelve el lienzo.
    p.volcar_por_cpu();
    paint_status(p, &dsk.run_box, "apagando el GSP en orden: despedida, FWSEC-SB y el booter de descarga", INK_DIM);
    let r = apagar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(_) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL GSP APAGADO EN ORDEN: la WPR2 abajo; el siguiente arranque encuentra la 3060 limpia\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.text(b" -- si pasa, corta la corriente unos segundos antes de volver a arrancar\n");
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "apagar", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn paso(s: &mut Output, hecho: bool, nombre: &[u8]) {
    s.with_ink(if hecho { INK_GOOD } else { INK_ECHO });
    s.text(if hecho { b" +" as &[u8] } else { b" -" });
    s.text(nombre);
}

/// **La fila `apagado`**, si se intento.
pub(crate) fn fila(s: &mut Output) {
    let v = info();
    if v & DESPEDIDO == 0 && resumen().is_none() {
        return;
    }
    campo(s, b"apagado");
    match resumen().map(|r| r.r) {
        _ if v & HECHO != 0 => {
            s.with_ink(INK_GOOD);
            s.text(b"EL GSP APAGADO EN ORDEN ");
        }
        Some(Err(m)) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
            s.byte(b' ');
        }
        _ => {
            s.with_ink(INK_ECHO);
            s.text(b"a medias ");
        }
    }
    paso(s, v & DESPEDIDO != 0, b"despedido");
    paso(s, v & (SUSPENDIDO | SB) != 0, b"suspendido");
    paso(s, v & SB_PARADO != 0, b"sb");
    paso(s, v & SB_BIEN != 0, b"sb-bien");
    paso(s, v & SEC2_PARADO != 0, b"descarga");
    paso(s, v & HECHO != 0, b"wpr2-abajo");
    s.with_ink(INK_ECHO);
    if v & SB_PARADO != 0 {
        s.text(b"   error SB 0x");
        s.hex(v >> SB_ERROR_SHIFT & 0xFFFF, 4);
    }
    s.text(b", MAILBOX0 0x");
    s.hex(v >> BUZON_SHIFT, 8);
    if let Some(r) = resumen() {
        match r.respuesta {
            Some((resultado, us)) => {
                s.text(b"; contesto en ");
                s.dec(us / 1000);
                s.text(b" ms (rpc_result 0x");
                s.hex(resultado as u64, 8);
                s.byte(b')');
            }
            None if v & (SUSPENDIDO | SB) != 0 => s.text(b"; se suspendio sin contestar la despedida (no se espera)"),
            None if v & DESPEDIDO != 0 => s.text(b"; la despedida sin respuesta"),
            None => {}
        }
        r.otros.escribir(s);
        if let Some(m) = r.aviso {
            s.text(b"; por el camino: ");
            s.text(super::iommu::motivo(m));
        }
        s.text(b"; todo en ");
        s.dec(r.ms);
        s.text(b" ms");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp apagado", v, b"");
}
