//! **`gpu canal`: L1d2b y L1d2c, EL PRIMER CANAL DE LA 3060.** Un
//! `AMPERE_CHANNEL_GPFIFO_A` colgado de NUESTRO dispositivo, sobre NUESTRO
//! espacio (L1c1) y con su memoria en el tramo (L1d1); despues, atado al
//! motor de copia COPY2 (`BIND`) y metido en su lista de ejecucion
//! (`GPFIFO_SCHEDULE`); y por ultimo su FICHA (`GET_WORK_SUBMIT_TOKEN`), lo
//! que se escribira en el timbre en L1d2d.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     cuatro RPC de hasta 5 s cada una
//!
//! Todo es FIJO en el crate (`bmo_gpu_ga10x::canal`) y el contrato del kernel
//! compara cada byte: el escritorio solo dice QUE paso toca y espera la
//! respuesta. Antes de pedirlo se comprueba lo que L1d2a pregunto: que COPY2
//! existe y que el bufer de metodos mide lo que el crate presta. Si el RM
//! dice otra cosa, el canal NO se pide.
//!
//! Tres pasos de `save mode` (`canal`, `encender`, `ficha`): cada uno pide
//! el de antes, y el primero que falla dice su motivo en su fila.
//!
//! # `gpu copia` (L1d2d y L1d3)
//!
//! Dos pasos mas (`copiador`, `copia`): el objeto de copia colgado del canal,
//! y la primera vez que la 3060 EJECUTA algo nuestro -- 4 KiB del tramo a
//! otra pagina del tramo, por direcciones virtuales, con un semaforo al
//! final (`bmo_gpu_ga10x::copia`).

use bmo_gpu_ga10x::canal;
use bmo_gpu_ga10x::copia;
use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL, GSP_RM_CONTROL};
use bmo_gpu_ga10x::objeto::{self, CABECERA_ALLOC};
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// El `GSP_RM_ALLOC` del canal, contestado.
#[derive(Clone, Copy)]
struct Pedido {
    r: objeto::Respuesta,
    resultado: u32,
    espera_us: u64,
    numero: u32,
}

impl Pedido {
    fn bien(&self) -> bool {
        self.r.estado == 0 && self.resultado == 0
    }
}

#[derive(Clone, Copy, Default)]
struct Canal {
    pedido: Option<Result<Pedido, u32>>,
    atar: Option<Result<Contestada, u32>>,
    programar: Option<Result<Contestada, u32>>,
    ficha: Option<Result<Contestada, u32>>,
    otros: Otros,
    /// L1d3: el GSP_RM_ALLOC del copiador.
    copiador: Option<Result<Pedido, u32>>,
    /// L1d3: el `Ok` empaquetado de la copia, o el NO.
    copia: Option<Result<u64, u32>>,
    /// L1d3, si la copia no salio: las palabras de `copia::DIAGNOSTICO`
    /// (`None` la que no se pudo leer), y lo que dijo el GSP-RM despues.
    diag: Option<([Option<u32>; copia::DIAGNOSTICO.len()], Otros)>,
}

static mut CANAL: Option<Canal> = None;

fn canal_() -> Canal {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(CANAL) }.unwrap_or_default()
}

fn con(f: impl FnOnce(&mut Canal)) {
    // SAFETY: como `canal_`.
    f(unsafe { (*core::ptr::addr_of_mut!(CANAL)).get_or_insert_with(Canal::default) })
}

fn bien(c: &Option<Result<Contestada, u32>>) -> bool {
    matches!(c, Some(Ok(c)) if c.bien())
}

/// Motivos del escritorio.
/// L1d2a no se pregunto, o no trae COPY2.
pub(crate) const NO_CANAL_SIN_MOTORES: u32 = 0x12A;
/// El RM dice que el bufer de metodos mide otra cosa que lo que se presta.
pub(crate) const NO_CANAL_METODOS: u32 = 0x12B;
/// El RM contesto, pero NO dio el canal.
pub(crate) const NO_CANAL_NEGADO: u32 = 0x12C;
/// El RM contesto, pero NO dio el copiador.
pub(crate) const NO_COPIADOR_NEGADO: u32 = 0x12D;
/// Sin la ficha del canal (`ficha`) no se toca el timbre.
pub(crate) const NO_COPIA_SIN_FICHA: u32 = 0x12E;
/// La 3060 recibio el timbre pero la copia no salio entera (la fila `copia`).
pub(crate) const NO_COPIA_MAL: u32 = 0x12F;

/// **L1d2b: pedir el canal y esperar al RM.** `Ok(su asa)`.
pub(crate) fn pedir() -> Result<u64, u32> {
    let r = pedir_();
    con(|c| {
        c.pedido = Some(r);
        // Un canal nuevo: lo de despues, de cero.
        c.atar = None;
        c.programar = None;
        c.ficha = None;
        c.copiador = None;
        c.copia = None;
        c.diag = None;
    });
    match r {
        Ok(p) if p.bien() => Ok(canal::CANAL as u64),
        Ok(_) => Err(NO_CANAL_NEGADO),
        Err(m) => Err(m),
    }
}

fn pedir_() -> Result<Pedido, u32> {
    if super::gspmotores::copia() != Some(canal::MOTOR) {
        return Err(NO_CANAL_SIN_MOTORES);
    }
    if super::gspmotores::metodos() != Some(canal::METODOS as u32) {
        return Err(NO_CANAL_METODOS);
    }
    let numero = (bmo::iommu_orden(bmo::IOMMU_OP_GPU_CANAL)? >> 32) as u32;
    let mut d = [0u8; CABECERA_ALLOC];
    let mut otros = Otros::default();
    let r = esperar(objeto::GSP_RM_ALLOC, &mut d, &mut otros);
    con(|c| c.otros = otros);
    let (m, espera_us) = r?;
    let r = objeto::leer(&d).ok_or(NO_CANAL_NEGADO)?;
    Ok(Pedido { r, resultado: m.resultado, espera_us, numero })
}

/// Lo pregunta `save mode`.
pub(crate) fn pedido() -> bool {
    matches!(canal_().pedido, Some(Ok(p)) if p.bien())
}

/// Una orden que ENCIENDE el canal, por su puerta, y su respuesta.
fn encender_con(c: Control) -> Result<Contestada, u32> {
    let que = Control::TODOS.iter().position(|&k| k == c).unwrap_or(0) as u64;
    let numero = (bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CANAL_ORDEN, que)? >> 32) as u32;
    let mut d = [0u8; CABECERA_CONTROL + 4];
    let (m, espera_us) = esperar(GSP_RM_CONTROL, &mut d, &mut Otros::default())?;
    let r = control::leer(&d).ok_or(super::gspsalud::NO_CONTROL_NEGADO)?;
    Ok(Contestada { r, resultado: m.resultado, espera_us, numero })
}

/// **L1d2c: BIND a COPY2 y GPFIFO_SCHEDULE**, en ese orden (como
/// `r535_chan_start` de nouveau: sin atar, no hay a que lista meterlo).
pub(crate) fn encender() -> Result<u64, u32> {
    let atar = encender_con(Control::Atar);
    con(|c| {
        c.atar = Some(atar);
        c.programar = None;
        c.ficha = None;
    });
    if !bien(&Some(atar)) {
        return Err(atar.err().unwrap_or(super::gspsalud::NO_CONTROL_NEGADO));
    }
    let programar = encender_con(Control::Programar);
    con(|c| c.programar = Some(programar));
    if !bien(&Some(programar)) {
        return Err(programar.err().unwrap_or(super::gspsalud::NO_CONTROL_NEGADO));
    }
    Ok(canal::MOTOR as u64)
}

/// Lo pregunta `save mode`.
pub(crate) fn encendido() -> bool {
    let c = canal_();
    bien(&c.atar) && bien(&c.programar)
}

/// **L1d2d (la mitad que pregunta): la FICHA del timbre.** `Ok(la ficha)`.
pub(crate) fn preguntar_ficha() -> Result<u64, u32> {
    let f = controlar(Control::Ficha, &mut [0u8; CABECERA_CONTROL + 4]);
    con(|c| c.ficha = Some(f));
    let f = f?;
    if !f.bien() {
        return Err(super::gspsalud::NO_CONTROL_NEGADO);
    }
    Ok(f.r.valor as u64)
}

/// Lo pregunta `save mode`.
pub(crate) fn ficha_leida() -> bool {
    bien(&canal_().ficha)
}

/// **L1d3: pedir el copiador y esperar al RM.** `Ok(su asa)`.
pub(crate) fn pedir_copiador() -> Result<u64, u32> {
    let r = (|| {
        let numero = (bmo::iommu_orden(bmo::IOMMU_OP_GPU_COPIADOR)? >> 32) as u32;
        let mut d = [0u8; CABECERA_ALLOC];
        let (m, espera_us) = esperar(objeto::GSP_RM_ALLOC, &mut d, &mut Otros::default())?;
        let r = objeto::leer(&d).ok_or(NO_COPIADOR_NEGADO)?;
        Ok(Pedido { r, resultado: m.resultado, espera_us, numero })
    })();
    con(|c| {
        c.copiador = Some(r);
        c.copia = None;
    });
    match r {
        Ok(p) if p.bien() => Ok(copia::COPIADOR as u64),
        Ok(_) => Err(NO_COPIADOR_NEGADO),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn copiador_listo() -> bool {
    matches!(canal_().copiador, Some(Ok(p)) if p.bien())
}

/// **L1d2d y L1d3: la primera copia** -- el kernel prepara el tramo, pone
/// GP_PUT, toca el timbre con la ficha y espera el semaforo.
pub(crate) fn copiar() -> Result<u64, u32> {
    let r = match canal_().ficha {
        Some(Ok(f)) if f.bien() => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COPIA, f.r.valor as u64),
        _ => Err(NO_COPIA_SIN_FICHA),
    };
    con(|c| c.copia = Some(r));
    if !matches!(r, Ok(v) if copia::sana(v)) && !matches!(r, Err(NO_COPIA_SIN_FICHA)) {
        diagnosticar();
    }
    match r {
        Ok(v) if copia::sana(v) => Ok(v),
        Ok(_) => Err(NO_COPIA_MAL),
        Err(m) => Err(m),
    }
}

/// **Si la copia no sale**: que dejo el RM en la instancia del canal, que
/// hay en el USERD y el GPFIFO, y que dijo el GSP-RM en el segundo de despues
/// (un canal caido llega como mensaje). Todo lectura.
fn diagnosticar() {
    let mut p = [None; copia::DIAGNOSTICO.len()];
    for (k, &(_, dir)) in copia::DIAGNOSTICO.iter().enumerate() {
        p[k] = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LEER, dir).ok().map(|v| v as u32);
    }
    let mut otros = Otros::default();
    super::gsprpc::barrer(&mut otros, 1000);
    con(|c| c.diag = Some((p, otros)));
}

/// Lo pregunta `save mode`.
pub(crate) fn copia_hecha() -> bool {
    matches!(canal_().copia, Some(Ok(v)) if copia::sana(v))
}

/// `gpu canal`: los tres pasos, parando en el primero que no sale.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "pidiendole un canal al GSP-RM", INK_DIM);
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !encendido() {
        paint_status(p, &dsk.run_box, "atando el canal a COPY2 y programandolo", INK_DIM);
        r = encender();
    }
    if r.is_ok() && !ficha_leida() {
        r = preguntar_ficha();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LA 3060 TIENE NUESTRO CANAL: atado a COPY2, en su lista de ejecucion y con su ficha para el timbre\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el canal no quedo encendido: mira las filas `canal`, `atado` y `ficha`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "canal", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu copia`: el copiador y la primera copia (L1d3), con el canal ya
/// encendido y su ficha.
pub(crate) fn orden_copia(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 copia una pagina de VRAM por su canal", INK_DIM);
    let mut r = if copiador_listo() { Ok(0) } else { pedir_copiador() };
    if r.is_ok() && !copia_hecha() {
        r = copiar();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LA 3060 EJECUTO NUESTRO PRIMER TRABAJO: copio 4 KiB de VRAM a VRAM por su canal y pago el semaforo\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  la copia no salio: mira las filas `copiador` y `copia` (y `iommu`)\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "copia", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn no(s: &mut Output, m: u32) {
    s.with_ink(INK_ERR);
    s.text(b"NO: ");
    s.text(super::iommu::motivo(m));
    s.with_ink(INK_PLAIN);
}

fn tras(s: &mut Output, espera_us: u64, numero: u32, orden: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"   ");
    s.text(orden);
    s.text(b" en ");
    s.dec(espera_us / 1000);
    s.text(b" ms (numero ");
    s.dec(numero as u64);
    s.byte(b')');
    s.with_ink(INK_PLAIN);
}

fn estado(s: &mut Output, ok: bool, estado: u32, resultado: u32) {
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
    s.with_ink(INK_PLAIN);
}

/// **Las filas `canal`, `atado` y `ficha`**, las que se pidieron.
pub(crate) fn fila(s: &mut Output) {
    let c = canal_();
    let Some(pedida) = c.pedido else { return };
    campo(s, b"canal");
    match pedida {
        Err(m) => no(s, m),
        Ok(p) => {
            s.with_ink(INK_ECHO);
            s.text(b"0x");
            s.hex(canal::CANAL as u64, 8);
            s.text(b" (clase 0x");
            s.hex(canal::AMPERE_CHANNEL_GPFIFO_A as u64, 4);
            s.text(b", chid ");
            s.dec(canal::CHID as u64);
            s.text(b"): ");
            estado(s, p.bien(), p.r.estado, p.resultado);
            tras(s, p.espera_us, p.numero, b"GSP_RM_ALLOC");
            c.otros.escribir(s);
        }
    }
    s.byte(b'\n');
    if let Ok(p) = pedida {
        if p.bien() {
            campo(s, b"memoria");
            s.with_ink(INK_ECHO);
            s.text(b"instancia y RAMFC 0x");
            s.hex(canal::INSTANCIA, 9);
            s.text(b", USERD 0x");
            s.hex(canal::USERD, 9);
            s.text(b", GPFIFO 0x");
            s.hex(canal::GPFIFO, 9);
            s.text(b" (VA 0x");
            s.hex(canal::GPFIFO_VA, 9);
            s.text(b", ");
            s.dec(canal::GPFIFO_ENTRADAS as u64);
            s.text(b" entradas); metodos en la IOVA 0x");
            s.hex(canal::IOVA_METODOS, 8);
            s.text(b" (");
            s.dec(canal::METODOS);
            s.text(b" B)");
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
    for (nombre, orden, x) in [(b"atado" as &[u8], b"BIND" as &[u8], c.atar), (b"en lista", b"GPFIFO_SCHEDULE", c.programar)] {
        let Some(x) = x else { continue };
        campo(s, nombre);
        match x {
            Err(m) => no(s, m),
            Ok(k) => {
                if nombre == b"atado" {
                    let (m, n) = control::motor(canal::MOTOR);
                    s.text(m);
                    s.dec(n as u64);
                    s.text(b" (tipo 0x");
                    s.hex(canal::MOTOR as u64, 2);
                    s.text(b"): ");
                }
                estado(s, k.bien(), k.r.estado, k.resultado);
                tras(s, k.espera_us, k.numero, orden);
            }
        }
        s.byte(b'\n');
    }
    if let Some(f) = c.ficha {
        campo(s, b"ficha");
        match f {
            Err(m) => no(s, m),
            Ok(k) if k.bien() => {
                s.with_ink(INK_GOOD);
                s.text(b"0x");
                s.hex(k.r.valor as u64, 8);
                s.with_ink(INK_PLAIN);
                s.text(b": lo que se escribira en el timbre (L1d2d)");
                tras(s, k.espera_us, k.numero, b"GET_WORK_SUBMIT_TOKEN");
            }
            Ok(k) => {
                estado(s, false, k.r.estado, k.resultado);
                tras(s, k.espera_us, k.numero, b"GET_WORK_SUBMIT_TOKEN");
            }
        }
        s.byte(b'\n');
    }
    if let Some(x) = c.copiador {
        campo(s, b"copiador");
        match x {
            Err(m) => no(s, m),
            Ok(p) => {
                s.with_ink(INK_ECHO);
                s.text(b"0x");
                s.hex(copia::COPIADOR as u64, 8);
                s.text(b" (clase 0x");
                s.hex(copia::AMPERE_DMA_COPY_B as u64, 4);
                s.text(b", COPY2, en el canal): ");
                estado(s, p.bien(), p.r.estado, p.resultado);
                tras(s, p.espera_us, p.numero, b"GSP_RM_ALLOC");
            }
        }
        s.byte(b'\n');
    }
    if let Some(x) = c.copia {
        campo(s, b"copia");
        match x {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenas, gp_get, pagado, lanzada, us) = copia::desempaquetar(v);
                s.with_ink(if copia::sana(v) { INK_GOOD } else { INK_ERR });
                if copia::sana(v) {
                    s.text(b"LA 3060 COPIO: ");
                }
                s.dec(buenas as u64);
                s.text(b" de 1024 palabras de VA 0x");
                s.hex(copia::va(copia::ORIGEN), 9);
                s.text(b" a VA 0x");
                s.hex(copia::va(copia::DESTINO), 9);
                s.with_ink(INK_PLAIN);
                s.text(if pagado { b"; semaforo PAGADO" as &[u8] } else { b"; semaforo SIN PAGAR" });
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                if !lanzada {
                    s.text(b", la MMU no se invalido o el GP_PUT no se releyo: timbre SIN tocar");
                }
                s.with_ink(INK_ECHO);
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
            }
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
    if let Some((p, otros)) = c.diag {
        campo(s, b"diag");
        s.with_ink(INK_ECHO);
        for (k, &(nombre, _)) in copia::DIAGNOSTICO.iter().enumerate() {
            if k > 0 {
                s.text(b", ");
            }
            s.text(nombre);
            match p[k] {
                Some(v) => {
                    s.text(b" 0x");
                    s.hex(v as u64, 8);
                }
                None => s.text(b" ?"),
            }
        }
        s.with_ink(INK_PLAIN);
        if otros.n == 0 {
            s.text(b"; el GSP-RM no dijo nada en 1 s");
        } else {
            s.text(b"; el GSP-RM dijo despues:");
            for t in &otros.t[..otros.n] {
                s.byte(b' ');
                s.text(bmo_gpu_ga10x::rpc::nombre(t.0));
                s.text(b" x");
                s.dec(t.1 as u64);
            }
        }
        s.byte(b'\n');
    }
    let pasos = [pedido(), bien(&c.atar), bien(&c.programar), bien(&c.ficha), copiador_listo(), copia_hecha()]
        .iter()
        .filter(|&&b| b)
        .count();
    super::datos::anotar(b"gpu canal pasos", pasos as u64, b"de 6");
}
