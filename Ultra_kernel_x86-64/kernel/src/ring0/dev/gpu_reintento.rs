//! **EL REINTENTO LIMPIO DEL GSP** (2026-09-29) -- tras un `0x15` del booter,
//! deshacer lo que el booter dejo a medias y volver a subir, SIN reiniciar.
//!
//! [carril]  ROJO      arranca firmware firmado (FWSEC-SB, el booter de
//!                     descarga, FWSEC-FRTS y el booter) en dos falcons
//! [consumo] NADA      corre por orden (`gpu reintentar`), nunca solo
//!
//! # Por que (EL_0x15.md, seccion 10)
//!
//! El 0x15 del 29-09 llega con la tarjeta fria, la RAM sana (`ram prueba`, 0
//! errores) y lo que lee el booter intacto (el vigia, IGUAL). Lo que queda es
//! la propia tarjeta o el MOMENTO. Y el propietario lo pregunto: *"en que
//! momento pide GSP para arrancar?"*. En `save mode`, entre FWSEC-FRTS y el
//! booter pasan SEGUNDOS (se copian y comprueban los 60 MB del GSP-RM, con un
//! `save` antes de cada paso); en nova-core, milisegundos. El reintento sube
//! con FRTS y el booter SEGUIDOS -- y si sale, el momento queda marcado.
//!
//! # El orden: el del apagado, y luego el del arranque
//!
//! ```text
//!    CERRAR     FWSEC-SB en el falcon del GSP (nouveau, `tu102_gsp_fini`)
//!    DESCARGAR  el booter de descarga en el SEC2: baja la WPR2
//!    SUBIR      con la WPR2 ABAJO: el estado del despertar a cero y
//!               FWSEC-FRTS otra vez; el escritorio sigue EN SEGUIDA con
//!               despertar y el booter, por el camino de siempre
//! ```
//!
//! Solo con un booter que se paro con ERROR (MAILBOX0 del SEC2 != 0) y el
//! RISC-V del GSP sin arrancar: con el GSP-RM vivo, esto lo mataria.
//!
//! [!] Sin probar en el metal al escribirse: no se sabe si el booter de
//! descarga acepta un GSP que nunca arranco. Si la WPR2 no baja, SUBIR se
//! niega y lo dice; lo que queda es lo de siempre, cortar la corriente.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::descarga as dc;
use bmo_gpu_ga10x::falcon as fa;
use bmo_gpu_ga10x::Registros;

use crate::ring0::dev::gpu_despertar as de;
use crate::ring0::dev::gpu_prestamo::{self as pr, Bar0};

/// No hay un booter parado con error que deshacer, o el paso de antes no se
/// dio (o ya se dio este).
pub const IOMMU_NO_REINTENTO: u32 = 91;

pub const REINTENTO_SB: u64 = 1 << 0;
pub const REINTENTO_SB_PARADO: u64 = 1 << 1;
pub const REINTENTO_DESCARGADOR: u64 = 1 << 2;
pub const REINTENTO_SEC2_PARADO: u64 = 1 << 3;
pub const REINTENTO_WPR2_ABAJO: u64 = 1 << 4;
pub const REINTENTO_FRTS: u64 = 1 << 5;
pub const REINTENTO_VECES_SHIFT: u64 = 8;
pub const REINTENTO_BUZON_SHIFT: u64 = 32;

/// Los pasos DADOS de este intento, y cuantos intentos (8..15).
static ESTADO: AtomicU64 = AtomicU64::new(0);

fn bar0() -> Result<Bar0, u32> {
    match crate::ring0::dev::gpu::bar0() {
        0 => Err(crate::ring0::plat::iommu::IOMMU_NO_SIN_GPU),
        b => Ok(Bar0(b)),
    }
}

fn no(que: &str) -> Result<u64, u32> {
    crate::ring0::cabina::warn("gpu", que, 0);
    Err(IOMMU_NO_REINTENTO)
}

/// **1. CERRAR**: FWSEC-SB, con el booter parado con error.
pub fn cerrar() -> Result<u64, u32> {
    if crate::ring0::dev::gpu_apagar::despedido() {
        return no("REINTENTO: el GSP ya se apago en orden; nada que reintentar");
    }
    if !de::booter_fallo() {
        return no("REINTENTO: no hay un booter parado con ERROR (o el GSP-RM esta vivo): no se toca");
    }
    let e = ESTADO.load(Ordering::Acquire);
    if e & REINTENTO_SB != 0 && e & REINTENTO_FRTS == 0 {
        return no("REINTENTO: ya hay uno a medias; `gpu reintentar` lo sigue");
    }
    // Un intento nuevo: los pasos a cero, la cuenta sube.
    let veces = ((e >> REINTENTO_VECES_SHIFT) & 0xFF) + 1;
    ESTADO.store(veces.min(0xFF) << REINTENTO_VECES_SHIFT, Ordering::Release);
    crate::ring0::cabina::info("gpu", "REINTENTO LIMPIO del GSP: FWSEC-SB, intento", veces);
    let firma = pr::fwsec_sb()?;
    ESTADO.fetch_or(REINTENTO_SB, Ordering::AcqRel);
    Ok(firma)
}

/// **2. DESCARGAR**: el booter de descarga, con el fichero ya abierto.
pub fn descargar(fichero: Option<&mut dyn crate::ring0::dev::gpu_gsp::Fichero>) -> Result<u64, u32> {
    let e = ESTADO.load(Ordering::Acquire);
    if e & REINTENTO_SB == 0 || e & REINTENTO_DESCARGADOR != 0 {
        return no("REINTENTO: DESCARGAR fuera de orden");
    }
    let firma = de::descargador(fichero)?;
    ESTADO.fetch_or(REINTENTO_DESCARGADOR, Ordering::AcqRel);
    Ok(firma)
}

/// **3. SUBIR**: con el SEC2 parado y la WPR2 ABAJO, el despertar a cero y
/// FWSEC-FRTS otra vez. `Ok(frts.desde)` en cuanto FRTS arranca.
pub fn subir() -> Result<u64, u32> {
    let e = ESTADO.load(Ordering::Acquire);
    if e & REINTENTO_DESCARGADOR == 0 || e & REINTENTO_FRTS != 0 {
        return no("REINTENTO: SUBIR fuera de orden");
    }
    let v = info();
    if v & REINTENTO_SEC2_PARADO == 0 || v & REINTENTO_WPR2_ABAJO == 0 {
        return no("REINTENTO: la WPR2 NO bajo con el booter de descarga: no se sube encima (corta la corriente)");
    }
    de::rearmar();
    let desde = pr::fwsec_correr()?;
    ESTADO.fetch_or(REINTENTO_FRTS, Ordering::AcqRel);
    crate::ring0::cabina::count("gpu", "REINTENTO: la WPR2 abajo y FWSEC-FRTS otra vez; ahora despertar y el booter, SEGUIDOS", desde);
    Ok(desde)
}

/// `REINTENTO`: 0 SB arrancado | 1 su falcon PARADO (vivo) | 2 descargador
/// arrancado | 3 SEC2 PARADO (vivo) | 4 la WPR2 ABAJO (vivo) | 5 FRTS otra vez
/// | `8..15` intentos | `32..63` MAILBOX0 del falcon que toque.
pub fn info() -> u64 {
    let mut v = ESTADO.load(Ordering::Acquire);
    let Ok(mut r) = bar0() else { return v };
    if v & REINTENTO_SB != 0 && v & REINTENTO_DESCARGADOR == 0 {
        if let Ok((parado, m0, _)) = fa::como_va(&mut r, fa::GSP) {
            v |= if parado { REINTENTO_SB_PARADO } else { 0 } | (m0 as u64) << REINTENTO_BUZON_SHIFT;
        }
    }
    if v & REINTENTO_DESCARGADOR != 0 && v & REINTENTO_FRTS == 0 {
        if let Ok((parado, m0, _)) = fa::como_va(&mut r, fa::SEC2) {
            v |= (m0 as u64) << REINTENTO_BUZON_SHIFT;
            if parado {
                v |= REINTENTO_SEC2_PARADO;
                if dc::descargado(r.leer(dc::WPR2_HI)) {
                    v |= REINTENTO_WPR2_ABAJO;
                }
            }
        }
    }
    v
}
