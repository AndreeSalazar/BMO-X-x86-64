//! **`gpu secuenciador`: L0c4b2b, LO QUE EL GSP PIDE QUE HAGA LA CPU.** Lee
//! el `GSP_RUN_CPU_SEQUENCER` que espera en la cola del GSP --donde lo dejo
//! `gpu vaciar`-- y dice sus ordenes una a una: que registro, que valor, que
//! esperar y cuando mover el nucleo del GSP.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: unas 1000
//!                     preguntas de 8 bytes y un fichero
//!
//! # Lo que NO hace
//!
//! Correr ninguna orden ni mover un puntero: el mensaje se queda en la cola.
//! Correrlas es L0c4b2c, y en el kernel. Aqui se mira primero, como en cada
//! escalon: el mensaje entero, crudo, queda en `datos/gspsec.bin`.

use bmo_gpu_ga10x::rpc::{self, Mensaje};
use bmo_gpu_ga10x::secuenciador::{self as sq, NoSe, Orden};
use bmo_userland as bmo;

use super::gspcola::{cabecera, cola, mem, suma, LEIDO_CPU, PAGINAS};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo mas que mide un mensaje (16 paginas, el tope de nova-core).
const MAX_BYTES: usize = 16 * 4096;
/// Ordenes que se guardan (el del metal trajo 420); de mas, solo cuentan.
const MAX_ORDENES: usize = 1024;
/// Las que se muestran al principio y al final.
const PUNTAS: usize = 12;
/// Las unidades que se cuentan, en el orden de `sq::unidad`.
const UNIDADES: [&[u8]; 6] = [b"PMC", b"PFB", b"falcon GSP", b"PGC6/BSI", b"falcon SEC2", b"?"];
const RUTA: &[u8] = b"datos/gspsec.bin";

#[derive(Clone, Copy)]
struct Resumen {
    numero: u32,
    /// `(bufferSizeDWord, cmdIndex)`.
    cabecera: (u32, u32),
    n: usize,
    /// Cuantas ordenes tocan cada unidad (`UNIDADES`).
    unidades: [u32; 6],
    /// Leidas en total (pueden ser mas de las que caben arriba).
    total: u32,
    /// Por tipo (el opcode).
    tipos: [u32; 9],
    roto: Option<NoSe>,
    guardada: bool,
}

static mut RESUMEN: Option<Resumen> = None;
/// Las ordenes leidas, para las filas.
static mut ORDENES: [Orden; MAX_ORDENES] = [Orden::Resetear; MAX_ORDENES];
/// Los datos del mensaje, leidos de la cola.
static mut DATOS: [u8; MAX_BYTES] = [0; MAX_BYTES];

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

/// Motivos del escritorio (`gspvaciar.rs` va hasta 0x11D).
pub(crate) const NO_SEC_NO_HAY: u32 = 0x11E;
pub(crate) const NO_SEC_MAL: u32 = 0x11F;

/// **Leer el secuenciador**, sin tocarlo. `Ok(ordenes)`.
pub(crate) fn leer() -> Result<u64, u32> {
    if bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO == 0 {
        return Err(super::gspcola::NO_COLA_SIN_GSP);
    }
    let p = (mem(LEIDO_CPU) & 0xFFFF_FFFF) % PAGINAS;
    let m = Mensaje::de(&cabecera(p));
    if !m.bien_formado() || m.funcion != rpc::SECUENCIADOR {
        return Err(NO_SEC_NO_HAY);
    }
    if suma(p, &m) != 0 {
        return Err(NO_SEC_MAL);
    }
    // SAFETY: como `resumen`; nadie mas toca DATOS.
    let d = unsafe { &mut *core::ptr::addr_of_mut!(DATOS) };
    let n = m.datos().min(MAX_BYTES);
    for o in (0..n).step_by(8) {
        let w = cola(p, (rpc::CABECERA + o) as u64).to_le_bytes();
        let k = (n - o).min(8);
        d[o..o + k].copy_from_slice(&w[..k]);
    }
    let datos = &d[..n];
    let mut r = Resumen {
        numero: m.numero,
        cabecera: sq::cabecera(datos).unwrap_or((0, 0)),
        n: 0,
        unidades: [0; 6],
        total: 0,
        tipos: [0; 9],
        roto: None,
        guardada: false,
    };
    // SAFETY: como `resumen`; nadie mas toca ORDENES.
    let guardadas = unsafe { &mut *core::ptr::addr_of_mut!(ORDENES) };
    for x in sq::ordenes(datos) {
        match x {
            Ok(o) => {
                if r.n < MAX_ORDENES {
                    guardadas[r.n] = o;
                    r.n += 1;
                }
                if let Some(reg) = o.registro() {
                    let u = sq::unidad(reg);
                    r.unidades[UNIDADES.iter().position(|&x| x == u).unwrap_or(5)] += 1;
                }
                r.tipos[o.codigo()] += 1;
                r.total += 1;
            }
            Err(e) => r.roto = Some(e),
        }
    }
    r.guardada = match bmo::Archivo::create(RUTA) {
        Ok(a) => a.write(datos) == n && a.close(),
        Err(_) => false,
    };
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(r);
    }
    if r.roto.is_some() {
        return Err(NO_SEC_MAL);
    }
    Ok(r.total as u64)
}

/// La orden `i` leida (para decir en cual se paro L0c4b2c) y cuantas hay.
pub(crate) fn orden_n(i: usize) -> Option<Orden> {
    let r = resumen()?;
    // SAFETY: como `resumen`.
    (i < r.n).then(|| unsafe { (*core::ptr::addr_of!(ORDENES))[i] })
}

pub(crate) fn total() -> u32 {
    resumen().map_or(0, |r| r.total)
}

/// Lo pregunta `save mode`: leido entero, y entendido.
pub(crate) fn leido() -> bool {
    resumen().map_or(false, |r| r.roto.is_none() && r.total > 0)
}

/// `gpu secuenciador`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "leyendo el secuenciador del GSP", INK_DIM);
    let r = leer();
    let g = &mut dsk.out.grid;
    match r {
        Ok(n) => {
            g.with_ink(INK_GOOD);
            g.text(b"  EL SECUENCIADOR: ");
            g.dec(n);
            g.text(b" ordenes leidas, ninguna corrida; crudo en datos/gspsec.bin\n");
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
    paint_status(p, &dsk.run_box, "secuenciador", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

const TIPOS: [&[u8]; 9] = [b"ESCRIBIR", b"MODIFICAR", b"ESPERAR", b"RETRASO", b"LEER", b"CORE_RESET", b"CORE_START", b"CORE_WAIT_FOR_HALT", b"CORE_RESUME"];

/// Una fila `orden`: su numero, su nombre, lo que toca y la unidad.
pub(crate) fn una(s: &mut Output, i: usize, o: &Orden) {
    campo(s, b"orden");
    s.with_ink(INK_ECHO);
    for _ in 0..(3usize.saturating_sub(digitos(i + 1))) {
        s.byte(b' ');
    }
    s.dec(i as u64 + 1);
    s.byte(b' ');
    s.with_ink(if matches!(o, Orden::Reanudar) { INK_GOOD } else { INK_PLAIN });
    s.text(o.nombre());
    s.with_ink(INK_PLAIN);
    match *o {
        Orden::Escribir { reg, valor } => {
            s.text(b" 0x");
            s.hex(reg as u64, 8);
            s.text(b" <- 0x");
            s.hex(valor as u64, 8);
        }
        Orden::Modificar { reg, mascara, valor } => {
            s.text(b" 0x");
            s.hex(reg as u64, 8);
            s.text(b": los bits 0x");
            s.hex(mascara as u64, 8);
            s.text(b" <- 0x");
            s.hex(valor as u64, 8);
        }
        Orden::Esperar { reg, mascara, valor, plazo_us } => {
            s.text(b" 0x");
            s.hex(reg as u64, 8);
            s.text(b" & 0x");
            s.hex(mascara as u64, 8);
            s.text(b" == 0x");
            s.hex(valor as u64, 8);
            s.text(b", hasta ");
            s.dec(if plazo_us == 0 { 4_000_000 } else { plazo_us as u64 });
            s.text(b" us");
        }
        Orden::Retraso { us } => {
            s.byte(b' ');
            s.dec(us as u64);
            s.text(b" us");
        }
        Orden::Guardar { reg, indice } => {
            s.text(b" 0x");
            s.hex(reg as u64, 8);
            s.text(b" -> guardado ");
            s.dec(indice as u64);
        }
        _ => {}
    }
    if let Some(reg) = o.registro() {
        s.with_ink(INK_ECHO);
        s.text(b"   (");
        s.text(sq::unidad(reg));
        s.byte(b')');
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');
}

fn digitos(mut n: usize) -> usize {
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// **Las filas de L0c4b2b**, si se leyo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    // SAFETY: como `resumen`.
    let ordenes = unsafe { &*core::ptr::addr_of!(ORDENES) };
    let ordenes = &ordenes[..r.n];
    campo(s, b"secuen");
    s.with_ink(if r.roto.is_none() { INK_GOOD } else { INK_ERR });
    s.dec(r.total as u64);
    s.text(b" ordenes");
    s.with_ink(INK_PLAIN);
    s.text(b" (cmdIndex ");
    s.dec(r.cabecera.1 as u64);
    s.text(b" palabras de un buffer de ");
    s.dec(r.cabecera.0 as u64);
    s.text(b"; mensaje numero ");
    s.dec(r.numero as u64);
    s.text(b"):");
    for (k, &c) in r.tipos.iter().enumerate() {
        if c > 0 {
            s.byte(b' ');
            s.text(TIPOS[k]);
            s.text(b" x");
            s.dec(c as u64);
        }
    }
    s.with_ink(INK_ECHO);
    s.text(if r.guardada { b"   -> datos/gspsec.bin" as &[u8] } else { b"   (NO se pudo guardar)" });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // Que unidades toca: lo que L0c4b2c dejara escribir al kernel.
    campo(s, b"toca");
    let mut primera = true;
    for (k, &c) in r.unidades.iter().enumerate() {
        if c == 0 {
            continue;
        }
        if !primera {
            s.text(b", ");
        }
        primera = false;
        s.with_ink(if k == 5 { INK_ERR } else { INK_PLAIN });
        s.text(UNIDADES[k]);
        s.text(b" x");
        s.dec(c as u64);
        s.with_ink(INK_PLAIN);
    }
    if primera {
        s.text(b"ningun registro");
    }
    s.byte(b'\n');

    // Donde caen las del nucleo.
    campo(s, b"nucleo");
    let mut hay = false;
    for (i, o) in ordenes.iter().enumerate() {
        if o.registro().is_none() && !matches!(o, Orden::Retraso { .. }) {
            if hay {
                s.text(b", ");
            }
            hay = true;
            s.with_ink(if matches!(o, Orden::Reanudar) { INK_GOOD } else { INK_PLAIN });
            s.text(o.nombre());
            s.with_ink(INK_PLAIN);
            s.text(b" en la ");
            s.dec(i as u64 + 1);
        }
    }
    if !hay {
        s.text(b"ninguna");
    }
    s.byte(b'\n');

    // Las primeras y las ultimas; el resto, en el fichero.
    if ordenes.len() <= 2 * PUNTAS {
        for (i, o) in ordenes.iter().enumerate() {
            una(s, i, o);
        }
    } else {
        for (i, o) in ordenes[..PUNTAS].iter().enumerate() {
            una(s, i, o);
        }
        campo(s, b"orden");
        s.with_ink(INK_ECHO);
        s.text(b"   ... ");
        s.dec((ordenes.len() - 2 * PUNTAS) as u64);
        s.text(b" en medio: todas en datos/gspsec.bin\n");
        s.with_ink(INK_PLAIN);
        let desde = ordenes.len() - PUNTAS;
        for (i, o) in ordenes[desde..].iter().enumerate() {
            una(s, desde + i, o);
        }
    }
    if r.total as usize > r.n {
        campo(s, b"orden");
        s.with_ink(INK_ECHO);
        s.text(b"y ");
        s.dec((r.total as usize - r.n) as u64);
        s.text(b" mas que no caben aqui: todas en datos/gspsec.bin\n");
        s.with_ink(INK_PLAIN);
    }
    if let Some(e) = r.roto {
        campo(s, b"orden");
        s.with_ink(INK_ERR);
        match e {
            NoSe::Opcode(op) => {
                s.text(b"SE PARO en un opcode que r570.144 no tiene: ");
                s.dec(op as u64);
            }
            NoSe::Corta => s.text(b"SE PARO: una orden no cabe en las cmdIndex palabras, o el mensaje no llega a ellas"),
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
    }
    super::datos::anotar(b"gpu gsp secuenciador ordenes", r.total as u64, b"");
}
