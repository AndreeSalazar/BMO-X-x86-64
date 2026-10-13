//! **EL PARTE DEL CANAL DE GR MUERTO** (13-10): lo que el GSP-RM conto al
//! matarlo, guardado en cuanto se ve, para que no se pierda.
//!
//! capa: kernel -- lee la cola del GSP (sin moverla) por `gpu_libos`; el
//! formato lo sabe `bmo_gpu_ga10x::rc`
//!
//! [carril]  VERDE     solo lee y guarda; no toca la 3060
//! [consumo] NADA      corre al morir el canal, una vez
//!
//! # Por que existe
//!
//! El 13-10 la 3060 murio en el primer lote de Cyberpunk 2077 y lo unico que
//! quedo fue "esta MUERTO": ni el Xid, ni el motor, ni la direccion. Y el
//! mensaje del GSP se puede perder (un `gpu vaciar`, o la cola llena de
//! NOCAT). Aqui se guarda la PRIMERA vez que se ve, y la app lo lee por
//! `INFO`:
//!
//! ```text
//!    INFO_GPU_RC        bit 63 hay parte; [0..8) Xid; [8..16) motor;
//!                       [16..24) tipo de fallo de memoria; [24..32) nivel;
//!                       [32..40) quien lo dijo (1 el GSP-RM, 2 el vigilante
//!                       sin RC a la vista); bit 40 hay frase
//!    INFO_GPU_RC_VA     la direccion del fallo de memoria (Xid 31)
//!    INFO_GPU_RC_TEXTO  | trozo << 8: 8 bytes de la frase de NVIDIA
//! ```

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bmo_gpu_ga10x::rc;

/// El GSP-RM lo conto (`RC_TRIGGERED`).
pub const POR_EL_GSP: u64 = 1;
/// El vigilante lo dio por muerto sin ver un `RC_TRIGGERED`.
pub const POR_EL_VIGILANTE: u64 = 2;

static PARTE: AtomicU64 = AtomicU64::new(0);
static VA: AtomicU64 = AtomicU64::new(0);
/// La frase, escrita UNA vez (`ESCRITA` la cierra) y leida despues.
static mut FRASE: [u8; rc::TEXTO] = [0; rc::TEXTO];
static FRASE_N: AtomicU64 = AtomicU64::new(0);
static ESCRITA: AtomicBool = AtomicBool::new(false);

/// **Guardar el parte del canal `chid`**, si aun no se guardo. `quien`:
/// [`POR_EL_GSP`] o [`POR_EL_VIGILANTE`]. Lo dice en CABINA con nombres.
pub(super) fn guardar(chid: u32, quien: u64) {
    if PARTE.load(Ordering::Acquire) != 0 {
        return;
    }
    let p = super::super::gpu_libos::parte_del_canal(chid);
    let (xid, motor, tipo, nivel, va) = p.rc.map_or((0, 0, 0, 0, 0), |r| (r.xid, r.motor, r.tipo_fallo, r.nivel, r.va));
    let quien = if p.rc.is_some() { POR_EL_GSP } else { quien };
    if let Some(t) = p.texto {
        if !ESCRITA.swap(true, Ordering::AcqRel) {
            // SAFETY: `ESCRITA` deja escribir a uno solo, una vez; nadie lee
            // antes de que `FRASE_N` diga cuanto hay.
            unsafe { (&mut *core::ptr::addr_of_mut!(FRASE))[..t.n].copy_from_slice(t.frase()) };
            FRASE_N.store(t.n as u64, Ordering::Release);
        }
    }
    VA.store(va, Ordering::Release);
    let hay_frase = (FRASE_N.load(Ordering::Acquire) != 0) as u64;
    PARTE.store(1 << 63 | (xid & 0xFF) as u64 | ((motor & 0xFF) as u64) << 8 | ((tipo & 0xFF) as u64) << 16 | ((nivel & 0xFF) as u64) << 24 | (quien & 0xFF) << 32 | hay_frase << 40, Ordering::Release);
    let c = crate::ring0::cabina::warn;
    if p.rc.is_some() {
        c("gpu", rc::nombre_xid(xid), xid as u64);
        c("gpu", "13-10: el parte del GSP-RM: motor (1 = GR0)", motor as u64);
        if xid == 31 || va != 0 {
            c("gpu", "13-10: el fallo de memoria fue en la VA", va);
            c("gpu", rc::nombre_fallo(tipo), tipo as u64);
        }
    } else {
        c("gpu", "13-10: el canal se dio por muerto SIN un RC_TRIGGERED del GSP-RM a la vista (el vigilante lo corto, o el mensaje se perdio)", quien);
    }
}

/// `INFO_GPU_RC`.
pub fn info() -> u64 {
    PARTE.load(Ordering::Acquire)
}

/// `INFO_GPU_RC_VA`.
pub fn info_va() -> u64 {
    VA.load(Ordering::Acquire)
}

/// `INFO_GPU_RC_TEXTO`: 8 bytes de la frase desde `8 * trozo`, en
/// little-endian; 0 cuando se acaba.
pub fn info_texto(trozo: u64) -> u64 {
    let n = FRASE_N.load(Ordering::Acquire) as usize;
    let desde = (trozo as usize).saturating_mul(8);
    if desde >= n {
        return 0;
    }
    // SAFETY: escrita una vez antes de publicar `FRASE_N`; aqui solo se lee
    // hasta `n`.
    let f = unsafe { &(&*core::ptr::addr_of!(FRASE))[..n] };
    let mut w = [0u8; 8];
    for (k, b) in f[desde..n.min(desde + 8)].iter().enumerate() {
        w[k] = *b;
    }
    u64::from_le_bytes(w)
}
