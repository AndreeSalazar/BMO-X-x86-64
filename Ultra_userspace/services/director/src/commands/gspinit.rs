//! **`gpu init`: L0c4b2c, CORRER EL SECUENCIADOR HASTA `GSP_INIT_DONE`.**
//! Le dice al kernel "sigue" tramo a tramo (1 ms cada uno) hasta que el
//! secuenciador acaba --con su CORE_RESUME el GSP-RM vuelve--, y despues lee
//! la cola del GSP consumiendo lo que llegue hasta su `GSP_INIT_DONE`: el
//! GSP-RM de la 570.144 ARRANCADO en tu 3060.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: unos cientos de
//!                     tramos de 1 ms y hasta 10 s esperando al GSP-RM
//!
//! # Lo que NO hace
//!
//! Elegir que se escribe: el kernel lee el secuenciador el mismo y solo deja
//! tocar el falcon del GSP. Ni contestar nada despues de `GSP_INIT_DONE`: eso
//! es L1, hablar con el GSP-RM por RPC.

use bmo_gpu_ga10x::rpc::{self, Mensaje};
use bmo_userland as bmo;

use super::gspcola::{cabecera, mem, suma, ESCRITO, LEIDO_CPU, PAGINAS};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo mas que se da al secuenciador entero (el kernel pone el plazo de cada
/// orden; esto es solo el techo) y al GSP-RM para decir `GSP_INIT_DONE`.
const TECHO_S: u64 = 30;
const INIT_S: u64 = 10;
const MAX_TIPOS: usize = 8;

#[derive(Clone, Copy)]
struct Resumen {
    /// Lo ultimo de `INFO_GPU_DESPIERTO_BUZON` selector 2.
    sec: u64,
    tramos: u32,
    /// El NO del kernel, si lo hubo.
    no: u32,
    /// Lo que dijo el GSP-RM despues, antes de su GSP_INIT_DONE.
    tipos: [(u32, u32); MAX_TIPOS],
    n_tipos: usize,
    init_done: bool,
    /// `BAR1_BLOCK | BAR2_BLOCK << 32` antes de correr y tras GSP_INIT_DONE.
    bar_antes: u64,
    bar_despues: u64,
    /// Si se le devolvio BAR1 a la pantalla (L0c4b3a): el `Ok` del kernel o
    /// su NO; y como quedo.
    bar_devuelta: Option<Result<u64, u32>>,
    bar_final: u64,
    /// Los us desde CORE_RESUME hasta GSP_INIT_DONE.
    espera_us: u64,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

fn guardar(r: Resumen) {
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(r);
    }
}

/// Motivo del escritorio (`gspsecuencia.rs` va hasta 0x11F).
pub(crate) const NO_INIT_NO_LLEGA: u32 = 0x120;

/// `BAR1_BLOCK | BAR2_BLOCK << 32`, en vivo (selector 3).
fn bars() -> u64 {
    bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 3 << 8)
}

fn sec() -> u64 {
    bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 2 << 8)
}

/// **Correrlo y esperar a `GSP_INIT_DONE`** (el paso de `save mode`).
/// `Ok(ordenes corridas)`.
pub(crate) fn correr() -> Result<u64, u32> {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let mut r = Resumen {
        sec: 0,
        tramos: 0,
        no: 0,
        tipos: [(0, 0); MAX_TIPOS],
        n_tipos: 0,
        init_done: false,
        espera_us: 0,
        bar_antes: bars(),
        bar_despues: 0,
        bar_devuelta: None,
        bar_final: 0,
    };
    let fin = bmo::ciclos() + hz * TECHO_S;
    loop {
        r.tramos += 1;
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_SECUENCIAR, 0) {
            Ok(v) if v >> 24 & bmo::SEC_HECHO != 0 => {
                r.sec = v;
                break;
            }
            Ok(v) => r.sec = v,
            Err(m) => {
                r.sec = sec();
                r.no = m;
                guardar(r);
                return Err(m);
            }
        }
        if bmo::ciclos() >= fin {
            r.no = NO_INIT_NO_LLEGA;
            guardar(r);
            return Err(NO_INIT_NO_LLEGA);
        }
        bmo::yield_screen();
    }
    // El GSP-RM volvio: lo que diga, consumido, hasta su GSP_INIT_DONE.
    let desde = bmo::ciclos();
    let fin = desde + hz * INIT_S;
    'fuera: while bmo::ciclos() < fin {
        let escrito = (mem(ESCRITO) & 0xFFFF_FFFF) % PAGINAS;
        let mut p = (mem(LEIDO_CPU) & 0xFFFF_FFFF) % PAGINAS;
        while p != escrito {
            let m = Mensaje::de(&cabecera(p));
            if !m.bien_formado() || suma(p, &m) != 0 {
                break 'fuera;
            }
            match r.tipos[..r.n_tipos].iter_mut().find(|t| t.0 == m.funcion) {
                Some(t) => t.1 += 1,
                None if r.n_tipos < MAX_TIPOS => {
                    r.tipos[r.n_tipos] = (m.funcion, 1);
                    r.n_tipos += 1;
                }
                None => {}
            }
            p = (p + m.paginas as u64) % PAGINAS;
            if bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_LEIDO, p).is_err() {
                break 'fuera;
            }
            if m.funcion == rpc::INIT_DONE {
                r.init_done = true;
                r.espera_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
                break 'fuera;
            }
        }
        bmo::yield_screen();
    }
    r.bar_despues = bars();
    // ** Si el GSP-RM se quedo con BAR1, se le devuelve la del GOP: sin ella
    // la pantalla no ve lo que pinta la CPU (metal 24-09 09:54).
    if r.bar_despues as u32 != r.bar_antes as u32 {
        r.bar_devuelta = Some(bmo::iommu_orden(bmo::IOMMU_OP_GSP_BAR1));
    }
    r.bar_final = bars();
    guardar(r);
    // Al panel: la casilla I, verde o roja.
    crate::scene::lateral_gsp::init(r.init_done);
    if !r.init_done {
        return Err(NO_INIT_NO_LLEGA);
    }
    Ok((r.sec & 0xFFFF) as u64)
}

/// Lo pregunta `save mode`: el GSP-RM dijo `GSP_INIT_DONE`.
pub(crate) fn listo() -> bool {
    resumen().map_or(false, |r| r.init_done)
}

/// `gpu init`, a mano: un save antes y otro despues, por si la pantalla se
/// queda quieta (metal 24-09 09:54, era BAR1: desde L0c4b3a se devuelve). En
/// `save mode` es el paso `init`, seguido de `estatica` y `objetos`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu init") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "corriendo el secuenciador del GSP", INK_DIM);
    let r = correr();
    // Con BAR1 devuelta, lo que habia en pantalla era de antes: todo otra vez.
    crate::repintar_escritorio(p, dsk, "init");
    // Y con el GSP-RM arrancado, su primera RPC (L1a): que diga quien es.
    if r.is_ok() {
        paint_status(p, &dsk.run_box, "preguntandole al GSP-RM", INK_DIM);
        if super::gsprpc::preguntar() == Ok(0) {
            // Y con el RM contestando, NUESTROS objetos (L1b).
            paint_status(p, &dsk.run_box, "pidiendole objetos al GSP-RM", INK_DIM);
            if super::gspobjeto::paso().is_ok() {
                // Y con los objetos, su primera orden de control: el P-state;
                // y el espacio de direcciones (L1c1), que no la bloquea.
                let _ = super::gspsalud::preguntar();
                let _ = super::gspobjeto::paso_espacio();
            }
        }
    }
    let g = &mut dsk.out.grid;
    match r {
        Ok(n) => {
            g.with_ink(INK_GOOD);
            g.text(b"  GSP_INIT_DONE: ");
            g.dec(n);
            g.text(b" ordenes corridas y el GSP-RM de la 570.144 ARRANCADO en tu 3060\n");
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
    super::gsprpc::fila(&mut dsk.out.grid);
    super::gspobjeto::fila(&mut dsk.out.grid);
    super::gspsalud::fila(&mut dsk.out.grid);
    // El save de despues, pase lo que pase con la pantalla.
    let guardado = super::save_maestro::maestro(dsk, crate::DEFAULT_DUMP, p.rayo()).is_ok();
    let g = &mut dsk.out.grid;
    g.with_ink(if guardado { INK_ECHO } else { INK_ERR });
    g.text(if guardado {
        b"  lo de arriba, tambien en el save (datos/): si la pantalla se queda quieta, esta ahi\n" as &[u8]
    } else {
        b"  el save de despues NO se pudo escribir\n"
    });
    g.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, "init", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu bar1`: devolverle BAR1 a la pantalla a mano (lo hace `gpu init` solo
/// si la ve cambiada).
pub(crate) fn orden_bar1(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let r = bmo::iommu_orden(bmo::IOMMU_OP_GSP_BAR1);
    crate::repintar_escritorio(p, dsk, "bar1");
    let g = &mut dsk.out.grid;
    match r {
        Ok(v) if v as u32 == (v >> 32) as u32 => {
            g.with_ink(INK_ECHO);
            g.text(b"  BAR1 ya era la de antes del secuenciador: 0x");
            g.hex(v & 0xFFFF_FFFF, 8);
            g.byte(b'\n');
        }
        Ok(v) => {
            g.with_ink(INK_GOOD);
            g.text(b"  BAR1 DEVUELTA a la pantalla: 0x");
            g.hex(v >> 32, 8);
            g.text(b" (la del GSP-RM) -> 0x");
            g.hex(v & 0xFFFF_FFFF, 8);
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
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas de L0c4b2c**, si se intento.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    let (i, fase, como, dato) = ((r.sec & 0xFFFF) as usize, r.sec >> 16 & 0xFF, r.sec >> 24 & 0xFF, (r.sec >> 32) as u32);
    campo(s, b"corrio");
    if como & bmo::SEC_HECHO != 0 {
        s.with_ink(INK_GOOD);
        s.dec(i as u64);
        s.text(b" de ");
        s.dec(super::gspsecuencia::total() as u64);
        s.text(b" ordenes CORRIDAS");
        s.with_ink(INK_PLAIN);
        s.text(b" en ");
        s.dec(r.tramos as u64);
        s.text(b" tramos de 1 ms; CORE_RESUME: el GSP-RM volvio y el mensaje se consumio\n");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"SE PARO en la orden ");
        s.dec(i as u64 + 1);
        s.text(match como & 0x3F {
            1 => b": toca un registro FUERA del falcon del GSP -- no se escribio NADA" as &[u8],
            2 => b": su espera paso del plazo (el dato: lo ultimo que leyo)",
            3 => b": el registro no contesta",
            4 => b": el falcon no se dejo (motivo del falcon abajo)",
            5 => b": el SEC2 acabo CORE_RESUME con MAILBOX0 distinto de 0",
            _ => b": el techo de 30 s o el kernel dijo que no",
        });
        if dato != 0 {
            s.text(b"; dato 0x");
            s.hex(dato as u64, 8);
        }
        if como & 0x3F == 0 && r.no != 0 {
            s.text(b"; ");
            s.text(super::iommu::motivo(r.no));
        }
        if fase != 0 {
            s.text(b"; CORE_RESUME en la fase ");
            s.dec(fase);
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        if let Some(o) = super::gspsecuencia::orden_n(i) {
            super::gspsecuencia::una(s, i, &o);
        }
    }
    if como & bmo::SEC_HECHO == 0 {
        return;
    }
    campo(s, b"listo");
    if r.init_done {
        s.with_ink(INK_GOOD);
        s.text(b"GSP_INIT_DONE LLEGO: el GSP-RM de la 570.144 esta ARRANCADO");
        s.with_ink(INK_PLAIN);
        s.text(b" (");
        s.dec(r.espera_us / 1000);
        s.text(b" ms tras CORE_RESUME)");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"GSP_INIT_DONE NO llego en 10 s");
        s.with_ink(INK_PLAIN);
    }
    if r.n_tipos > 0 {
        s.text(b"; dijo:");
        for t in &r.tipos[..r.n_tipos] {
            s.byte(b' ');
            s.with_ink(if t.0 == rpc::INIT_DONE { INK_GOOD } else { INK_ECHO });
            s.text(rpc::nombre(t.0));
            s.with_ink(INK_PLAIN);
            s.text(b" x");
            s.dec(t.1 as u64);
        }
    }
    s.byte(b'\n');

    // BAR1: por donde la CPU pinta el GOP. Si el GSP-RM la hizo virtual, lo
    // que pinta la CPU ya no cae donde mira la pantalla.
    campo(s, b"bar1");
    let virtual_ = |v: u64| v as u32 & 1 << 31 != 0;
    let (a, d, f) = (r.bar_antes, r.bar_despues, r.bar_final);
    s.text(b"antes 0x");
    s.hex(a & 0xFFFF_FFFF, 8);
    s.text(b", despues 0x");
    s.hex(d & 0xFFFF_FFFF, 8);
    s.text(b"  (BAR2 0x");
    s.hex(a >> 32, 8);
    s.text(b" -> 0x");
    s.hex(d >> 32, 8);
    s.text(b"): ");
    match r.bar_devuelta {
        Some(Ok(_)) if f as u32 == a as u32 => {
            s.with_ink(INK_GOOD);
            s.text(if virtual_(d) && !virtual_(a) {
                b"el GSP-RM la puso VIRTUAL y se le DEVOLVIO la del GOP: la pantalla vuelve a ver a la CPU" as &[u8]
            } else {
                b"el GSP-RM la CAMBIO y se le DEVOLVIO la del GOP: la pantalla vuelve a ver a la CPU"
            });
        }
        Some(Ok(_)) => {
            s.with_ink(INK_ERR);
            s.text(b"se escribio la de antes y NO se quedo: ahora 0x");
            s.hex(f & 0xFFFF_FFFF, 8);
        }
        Some(Err(m)) => {
            s.with_ink(INK_ERR);
            s.text(b"CAMBIO y no se pudo devolver: ");
            s.text(super::iommu::motivo(m));
        }
        None if a as u32 != d as u32 => {
            s.with_ink(INK_ECHO);
            s.text(b"BAR1 CAMBIO al arrancar el GSP-RM");
        }
        None => {
            s.with_ink(INK_GOOD);
            s.text(b"BAR1 igual: si la pantalla se paro, no fue esto");
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp init done", r.init_done as u64, b"");
}
