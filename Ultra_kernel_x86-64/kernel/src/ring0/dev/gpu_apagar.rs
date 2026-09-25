//! **APAGAR EL GSP EN ORDEN (L0c5)** -- que el siguiente arranque encuentre
//! la 3060 limpia. Los pasos, el mensaje y lo que tienen que decir estan en
//! `bmo_gpu_ga10x::descarga`; aqui se tocan los falcons.
//!
//! [carril]  ROJO      arranca firmware firmado (FWSEC-SB y el booter de
//!                     descarga) en dos falcons de la 3060
//! [consumo] NADA      corre por orden (`gpu apagar`) y antes de reiniciar
//!
//! [eje]     CORRECCION -- el orden de nouveau (`tu102_gsp_fini`): despedir,
//!           suspendido, FWSEC-SB, booter de descarga, WPR2 abajo
//!
//! # Por que (metal 24-09: tres veces `0x15`)
//!
//! Sin esto el GSP-RM y su WPR2 seguian vivos al reiniciar, y el booter del
//! arranque siguiente fallaba con `0x15`. nova-core: *"The GPU will need to be
//! reset before the driver can bind again"*. Tras cortar la corriente, bien.
//!
//! # Tres ordenes y una pregunta, como DESPERTAR
//!
//! ```text
//!    DESPEDIR    la RPC UNLOADING_GUEST_DRIVER (47), por el contrato
//!    CERRAR      tras despedir (suspendido o no): FWSEC-SB en el falcon del GSP
//!    DESCARGAR   tras FWSEC-SB (bien o no): el booter de descarga en el SEC2
//!    APAGADO     como va, leido EN VIVO de los falcons (ver `info`)
//! ```
//!
//! Quien espera entre medias es el escritorio, cediendo el turno: la
//! respuesta de la RPC, la suspension (hasta 2 s), y cada falcon parado.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::descarga as dc;
use bmo_gpu_ga10x::falcon as fa;
use bmo_gpu_ga10x::Registros;

use crate::ring0::dev::gpu_despertar as de;
use crate::ring0::dev::gpu_prestamo::{self as pr, Bar0};

/// El GSP no esta en el paso de antes (sin despertar, sin despedir, sin
/// FWSEC-SB...), o ese paso ya se dio.
pub const IOMMU_NO_APAGAR: u32 = 80;

/// La 3060 ya se apago en orden en este arranque: no trabaja hasta el
/// siguiente. Lo dice toda orden de trabajo pedida despues (ver `permitida`).
pub const IOMMU_NO_GSP_APAGADO: u32 = 82;

pub const APAGADO_DESPEDIDO: u64 = 1 << 0;
pub const APAGADO_SUSPENDIDO: u64 = 1 << 1;
pub const APAGADO_SB: u64 = 1 << 2;
pub const APAGADO_SB_PARADO: u64 = 1 << 3;
pub const APAGADO_SB_BIEN: u64 = 1 << 4;
pub const APAGADO_DESCARGADOR: u64 = 1 << 5;
pub const APAGADO_SEC2_PARADO: u64 = 1 << 6;
pub const APAGADO_HECHO: u64 = 1 << 7;
pub const APAGADO_SB_ERROR_SHIFT: u64 = 8;
pub const APAGADO_BUZON_SHIFT: u64 = 32;

/// Los pasos DADOS (los vivos se leen en `info`).
static ESTADO: AtomicU64 = AtomicU64::new(0);

fn bar0() -> Result<Bar0, u32> {
    match crate::ring0::dev::gpu::bar0() {
        0 => Err(crate::ring0::plat::iommu::IOMMU_NO_SIN_GPU),
        b => Ok(Bar0(b)),
    }
}

/// Si ya se le pidio al GSP-RM que se fuera: desde ahi, ni una RPC mas.
pub fn despedido() -> bool {
    ESTADO.load(Ordering::Acquire) & APAGADO_DESPEDIDO != 0
}

/// **Lo que se puede pedir tras la despedida.** Sin el GSP-RM, y con FWSEC-SB
/// y la descarga hechos, el motor grafico y los canales ya no existen: un
/// timbre no lo contesta nadie y cada trabajo esperaba su segundo entero
/// (metal 24-09 20:36: `giro` y `raster` 1000001 us, `NV_PGRAPH_*` =
/// 0xBADF1201). Quedan las de la IOMMU y la sonda (`op` 0x01..0x0A), las que
/// solo LEEN (0x16, 0x25, 0x30, 0x39), las del propio apagado (0x3B..0x3E) y
/// el PASE (0x44): se cierra al despedir, y CERRAR y ESTADO tienen que poder
/// decirlo despues; ABRIR lo niega el propio pase (`Aparato::apagado`).
pub const fn permitida(op: u64) -> bool {
    matches!(op, 0x01..=0x0A | 0x16 | 0x25 | 0x30 | 0x39 | 0x3B..=0x3E | 0x44)
}

/// **1. DESPEDIR**: la RPC, con el GSP-RM despierto. `Ok(pagina | numero
/// << 32)` de la cola, como toda RPC; la respuesta la espera el escritorio.
pub fn despedir() -> Result<u64, u32> {
    if de::info_despierto() & de::DESPIERTO_VISTO == 0 || ESTADO.load(Ordering::Acquire) & APAGADO_DESPEDIDO != 0 {
        return Err(IOMMU_NO_APAGAR);
    }
    // ** El pase se cierra ANTES: su lienzo no se queda prestado a una 3060
    // que ya no va a copiar.
    crate::ring0::dev::pase_gpu::cerrar_por_apagado();
    let r = crate::ring0::dev::gpu_libos::enviar(dc::pedir)?;
    ESTADO.fetch_or(APAGADO_DESPEDIDO, Ordering::AcqRel);
    crate::ring0::cabina::count("gpu", "L0c5: UNLOADING_GUEST_DRIVER al GSP-RM; numero", r >> 32);
    Ok(r)
}

/// **2+3+4. CERRAR**: tras la despedida, FWSEC-SB (que resetea el falcon del
/// GSP antes de cargarse). Como nouveau (`tu102_gsp_fini`), NO exige que el
/// GSP-RM se haya suspendido: el escritorio espera hasta 2 s a que su MAILBOX0
/// diga 0x80000000, y si no, se sigue igual -- lo que importa es que la WPR2
/// acabe abajo. Si se suspendio, queda apuntado.
pub fn cerrar() -> Result<u64, u32> {
    let e = ESTADO.load(Ordering::Acquire);
    if e & APAGADO_DESPEDIDO == 0 || e & APAGADO_SB != 0 {
        return Err(IOMMU_NO_APAGAR);
    }
    let mut r = bar0()?;
    if let Ok((_, m0, _)) = fa::como_va(&mut r, fa::GSP) {
        if m0 == dc::SUSPENDIDO {
            ESTADO.fetch_or(APAGADO_SUSPENDIDO, Ordering::AcqRel);
        }
    }
    let firma = pr::fwsec_sb()?;
    ESTADO.fetch_or(APAGADO_SB, Ordering::AcqRel);
    Ok(firma)
}

/// **5. DESCARGAR**: tras FWSEC-SB, el booter de descarga. Como nouveau,
/// aunque SB no acabara bien (su error queda en `info`): el booter es quien
/// baja la WPR2.
pub fn descargar(fichero: Option<&mut dyn crate::ring0::dev::gpu_gsp::Fichero>) -> Result<u64, u32> {
    let v = ESTADO.load(Ordering::Acquire);
    if v & APAGADO_SB == 0 || v & APAGADO_DESCARGADOR != 0 {
        return Err(IOMMU_NO_APAGAR);
    }
    // Lo que dijo SB, antes de que el SEC2 lo tape: queda en ESTADO.
    let sb = info() & (APAGADO_SB_PARADO | APAGADO_SB_BIEN | 0xFFFF << APAGADO_SB_ERROR_SHIFT);
    let firma = de::descargador(fichero)?;
    ESTADO.fetch_or(APAGADO_DESCARGADOR | sb, Ordering::AcqRel);
    Ok(firma)
}

/// `APAGADO`: 0 despedido | 1 SUSPENDIDO (vivo) | 2 FWSEC-SB arrancado | 3
/// su falcon PARADO (vivo) | 4 SB BIEN (vivo) | 5 descargador arrancado | 6
/// SEC2 PARADO (vivo) | 7 HECHO: la WPR2 abajo (vivo) |
/// `8..23` el error de SB | `32..63` MAILBOX0 del falcon que toque.
pub fn info() -> u64 {
    let mut v = ESTADO.load(Ordering::Acquire);
    let Ok(mut r) = bar0() else { return v };
    if v & APAGADO_DESPEDIDO != 0 && v & APAGADO_SB == 0 {
        if let Ok((_, m0, _)) = fa::como_va(&mut r, fa::GSP) {
            v |= (m0 as u64) << APAGADO_BUZON_SHIFT;
            if m0 == dc::SUSPENDIDO {
                v |= APAGADO_SUSPENDIDO;
            }
        }
    }
    if v & APAGADO_SB != 0 && v & APAGADO_DESCARGADOR == 0 {
        if let Ok((parado, m0, _)) = fa::como_va(&mut r, fa::GSP) {
            v |= (m0 as u64) << APAGADO_BUZON_SHIFT;
            if parado {
                v |= APAGADO_SB_PARADO;
                let err = r.leer(dc::SB_ERROR);
                v |= (err as u64 & 0xFFFF) << APAGADO_SB_ERROR_SHIFT;
                if !bmo_gpu_ga10x::es_error_pri(err) && dc::sb_bien(err) {
                    v |= APAGADO_SB_BIEN;
                }
            }
        }
    }
    if v & APAGADO_DESCARGADOR != 0 {
        if let Ok((parado, m0, _)) = fa::como_va(&mut r, fa::SEC2) {
            v |= (m0 as u64) << APAGADO_BUZON_SHIFT;
            if parado {
                v |= APAGADO_SEC2_PARADO;
                if dc::descargado(r.leer(dc::WPR2_HI)) {
                    v |= APAGADO_HECHO;
                    if ESTADO.load(Ordering::Acquire) & APAGADO_HECHO == 0 {
                        ESTADO.fetch_or(APAGADO_HECHO, Ordering::AcqRel);
                        crate::ring0::cabina::count("gpu", "L0c5: EL GSP APAGADO EN ORDEN: la WPR2 abajo", 1);
                    }
                }
            }
        }
    }
    v
}
