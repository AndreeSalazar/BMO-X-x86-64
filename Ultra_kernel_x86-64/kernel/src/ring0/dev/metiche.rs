//! **EL METICHE** (26-09) -- BMO-X le pregunta a TODO el hardware lo que el
//! hardware apunto por su cuenta y nadie le pregunto: sus errores.
//!
//! [carril]  VERDE     solo LEE el espacio de configuracion PCI de cada funcion
//! [consumo] NADA      corre al arrancar y cada vez que se pide (`metiche`, el
//!                     informe): ~8.000 lecturas de configuracion, unos ms
//!
//! # Por que existe
//!
//! El propietario: *"que BMO-X sea demasiado metiche: por CABINA preguntar
//! TODO en hardware ... la maquina trabaje para ti"* (lo de Grace Hopper). El
//! 0x15 del booter mostro el porque: la tarjeta sabia algo que nadie le
//! pregunto, y se tardo cinco arranques en ir a buscarlo. Aqui se pregunta
//! siempre, a todos, sin esperar a que algo falle.
//!
//! # Lo que se pregunta (v1: el bus, que une a todos)
//!
//! ```text
//!    Status (0x06)       bits que el aparato pone al ver un error: paridad
//!                        (8, 15), aborto del destino (11, 12), aborto del
//!                        maestro (13), error del sistema (14)
//!    PCIe Device Status  corregible, no fatal, fatal, peticion no soportada
//!                        (bits 0..3 del registro de estado del dispositivo)
//!    AER (0x0001)        el detalle: que error NO corregible y que corregible
//!                        vio el enlace (los registros de estado)
//! ```
//!
//! # *** Chisme de la sesion ANTERIOR
//!
//! Los bits de AER son "pegajosos" (RW1CS en la especificacion PCIe): no los
//! borra un reinicio en caliente. Lo que se lee al arrancar puede ser lo que
//! paso ANTES de este arranque -- justo la clase de dato que le faltaba al
//! 0x15. Por eso se pregunta al arrancar Y cada vez que se pide: la diferencia
//! dice que es viejo y que es de esta sesion.
//!
//! # [!] Lo que NO hace, a proposito
//!
//! Esos bits se borran ESCRIBIENDO un 1 encima. BMO-X no los borra: un
//! metiche que borra lo que escucho destruye la prueba. Se leen y se cuentan.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::dev::pci;

/// Cuantas funciones con chisme se guardan (las demas se cuentan).
pub const MAX: usize = 16;

/// Los bits del registro Status que son errores.
const STATUS_ERRORES: u16 = 1 << 8 | 1 << 11 | 1 << 12 | 1 << 13 | 1 << 14 | 1 << 15;
/// Los bits de error del Device Status de PCIe.
const DEVSTA_ERRORES: u16 = 0b1111;

/// `funciones | con_aer << 16 | con_chisme << 32 | preguntas << 48`.
static RESUMEN: AtomicU64 = AtomicU64::new(0);
/// Por funcion: `bdf | status << 16 | devsta << 32 | vendor << 48`.
static QUIEN: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];
/// Por funcion: `aer no corregible | aer corregible << 32`.
static AER: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];
/// Lo que dijo la PRIMERA pregunta (al arrancar): `con_chisme`.
static AL_ARRANCAR: AtomicU64 = AtomicU64::new(u64::MAX);

/// El Device Status de PCIe de una funcion, si tiene la capacidad (id 0x10).
fn devsta(bus: u8, dev: u8, func: u8) -> Option<u16> {
    if pci::cfg_read32(bus, dev, func, 0x04) >> 16 & 0x10 == 0 {
        return None;
    }
    let mut p = (pci::cfg_read32(bus, dev, func, 0x34) & 0xFC) as u8;
    let mut vueltas = 0;
    while p >= 0x40 && p <= 0xF8 && vueltas < 48 {
        let c = pci::cfg_read32(bus, dev, func, p);
        if c & 0xFF == 0x10 {
            return Some((pci::cfg_read32(bus, dev, func, p + 0x08) >> 16) as u16);
        }
        p = ((c >> 8) & 0xFC) as u8;
        vueltas += 1;
    }
    None
}

/// Los dos estados de AER, si la funcion tiene la capacidad extendida 0x0001.
fn aer(bus: u8, dev: u8, func: u8) -> Option<(u32, u32)> {
    let mut caps = [pci::CapExt { id: 0, version: 0, offset: 0 }; 16];
    let n = pci::caps_extendidas(bus, dev, func, &mut caps);
    let c = caps[..n].iter().find(|c| c.id == 0x0001)?;
    let unc = pci::cfg_read32_ext(bus, dev, func, c.offset + 0x04)?;
    let cor = pci::cfg_read32_ext(bus, dev, func, c.offset + 0x10)?;
    Some((unc, cor))
}

/// **PREGUNTAR a todos.** Solo lee. Devuelve cuantas funciones confesaron.
pub fn preguntar() -> u32 {
    let (mut funciones, mut con_aer, mut chismes) = (0u64, 0u64, 0u32);
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            if pci::cfg_read32(bus, dev, 0, 0x00) == 0xFFFF_FFFF {
                continue;
            }
            let multi = (pci::cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0x80 != 0;
            for func in 0u8..if multi { 8 } else { 1 } {
                let vd = pci::cfg_read32(bus, dev, func, 0x00);
                if vd == 0xFFFF_FFFF {
                    continue;
                }
                funciones += 1;
                let status = (pci::cfg_read32(bus, dev, func, 0x04) >> 16) as u16 & STATUS_ERRORES;
                let devsta = devsta(bus, dev, func).unwrap_or(0) & DEVSTA_ERRORES;
                let a = aer(bus, dev, func);
                if a.is_some() {
                    con_aer += 1;
                }
                let (unc, cor) = a.unwrap_or((0, 0));
                if status == 0 && devsta == 0 && unc == 0 && cor == 0 {
                    continue;
                }
                let bdf = (bus as u64) << 8 | (dev as u64) << 3 | func as u64;
                if (chismes as usize) < MAX {
                    let k = chismes as usize;
                    QUIEN[k].store(bdf | (status as u64) << 16 | (devsta as u64) << 32 | ((vd & 0xFFFF) as u64) << 48, Ordering::Release);
                    AER[k].store(unc as u64 | (cor as u64) << 32, Ordering::Release);
                }
                chismes += 1;
            }
        }
    }
    let preguntas = (RESUMEN.load(Ordering::Acquire) >> 48).wrapping_add(1) & 0xFFFF;
    RESUMEN.store(funciones & 0xFFFF | (con_aer & 0xFFFF) << 16 | (chismes as u64) << 32 | preguntas << 48, Ordering::Release);
    let primera = AL_ARRANCAR.compare_exchange(u64::MAX, chismes as u64, Ordering::AcqRel, Ordering::Acquire).is_ok();
    // A CABINA: queda en la caja negra aunque nadie mire el informe.
    if primera {
        let c = crate::ring0::cabina::count;
        c("metiche", "funciones preguntadas (solo lectura)", funciones);
        c("metiche", "  ...con AER, el detalle del enlace", con_aer);
        for k in 0..(chismes as usize).min(MAX) {
            crate::ring0::cabina::warn("metiche", "CONFIESA errores: bdf | status << 16 | devsta << 32 | vendor << 48", QUIEN[k].load(Ordering::Acquire));
            crate::ring0::cabina::warn("metiche", "  ...AER no corregible | corregible << 32 (pegajosos: pueden ser de ANTES)", AER[k].load(Ordering::Acquire));
        }
        if chismes == 0 {
            c("metiche", "nadie confiesa un error en el bus", 0);
        }
    }
    chismes
}

/// `INFO_METICHE`: selector 0 PREGUNTA otra vez y da el resumen
/// (`funciones | con_aer << 16 | con_chisme << 32 | preguntas << 48`); 1 lo
/// que confesaron AL ARRANCAR; `2 + 2k` y `3 + 2k` la funcion `k` (ver
/// `QUIEN` y `AER`).
pub fn info(sel: u64) -> u64 {
    match sel >> 8 {
        0 => {
            preguntar();
            RESUMEN.load(Ordering::Acquire)
        }
        1 => AL_ARRANCAR.load(Ordering::Acquire),
        s if s >= 2 && ((s - 2) / 2) < MAX as u64 => {
            let k = ((s - 2) / 2) as usize;
            if s % 2 == 0 { QUIEN[k].load(Ordering::Acquire) } else { AER[k].load(Ordering::Acquire) }
        }
        _ => 0,
    }
}
