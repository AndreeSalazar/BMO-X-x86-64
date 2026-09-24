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
/// Ordenes que se guardan para las filas; el resto, en el fichero.
const MAX_ORDENES: usize = 48;
const RUTA: &[u8] = b"datos/gspsec.bin";

#[derive(Clone, Copy)]
struct Resumen {
    numero: u32,
    /// `(bufferSizeDWord, cmdIndex)`.
    cabecera: (u32, u32),
    ordenes: [Orden; MAX_ORDENES],
    n: usize,
    /// Leidas en total (pueden ser mas de las que caben arriba).
    total: u32,
    /// Por tipo (el opcode).
    tipos: [u32; 9],
    roto: Option<NoSe>,
    guardada: bool,
}

static mut RESUMEN: Option<Resumen> = None;
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
        ordenes: [Orden::Resetear; MAX_ORDENES],
        n: 0,
        total: 0,
        tipos: [0; 9],
        roto: None,
        guardada: false,
    };
    for x in sq::ordenes(datos) {
        match x {
            Ok(o) => {
                if r.n < MAX_ORDENES {
                    r.ordenes[r.n] = o;
                    r.n += 1;
                }
                r.tipos[o.codigo()] += 1;
                r.total += 1;
            }
            Err(e) => r.roto = Some(e),
        }
    }
    if r.roto.is_none() && r.total < r.cabecera.1 {
        r.roto = Some(NoSe::Corta);
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

/// **Las filas de L0c4b2b**, si se leyo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"secuen");
    s.with_ink(if r.roto.is_none() { INK_GOOD } else { INK_ERR });
    s.dec(r.total as u64);
    s.text(b" ordenes");
    s.with_ink(INK_PLAIN);
    s.text(b" (cmdIndex ");
    s.dec(r.cabecera.1 as u64);
    s.text(b", buffer ");
    s.dec(r.cabecera.0 as u64);
    s.text(b" palabras, mensaje numero ");
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
    for (i, o) in r.ordenes[..r.n].iter().enumerate() {
        campo(s, b"orden");
        s.with_ink(INK_ECHO);
        if i < 9 {
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
    if r.total as usize > r.n {
        campo(s, b"orden");
        s.with_ink(INK_ECHO);
        s.text(b"y ");
        s.dec((r.total as usize - r.n) as u64);
        s.text(b" mas: todas en datos/gspsec.bin\n");
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
            NoSe::Corta => s.text(b"SE PARO: una orden no cabe en el mensaje, o faltan ordenes de las que dice cmdIndex"),
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
    }
    super::datos::anotar(b"gpu gsp secuenciador ordenes", r.total as u64, b"");
}
