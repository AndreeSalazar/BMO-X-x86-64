//! **E7: EL VIGILANTE, EN EL KERNEL** -- un trabajo del GR que no vuelve en su
//! plazo se CORTA, como el TDR de Windows pero sin reiniciar la GPU entera:
//! solo el canal de GR. La decision y los mensajes son de
//! `bmo_gpu_ga10x::vigilante`; aqui, el reloj, la cola y el registro.
//!
//! [carril]  ROJO      pide al GSP-RM que pare (y si no, que recupere) el
//!                     canal de GR, y lee `NV_PGRAPH_STATUS`
//! [consumo] NADA      solo tras una espera VENCIDA: un trabajo bueno no lo
//!                     despierta nunca
//!
//! [eje]     AISLAMIENTO -- se corta el canal de GR y NADA MAS: el de copia,
//!           la pantalla y el resto del sistema siguen
//!
//! ```text
//!    cada espera de un trabajo del GR     vigilar(..): si VENCIO (lanzado, sin
//!                                         pagar, y pasado su plazo) y el canal
//!                                         no estaba ya muerto, el corte
//!    el corte                             1 PARAR, y si el GR sigue ocupado y
//!                                         sin RC, 2 ESCALAR; acaba con el RC
//!                                         del GSP, con el GR quieto, o "sigue"
//!    despues                              el canal de GR, MUERTO hasta
//!                                         reiniciar (P3b4c): todo trabajo del
//!                                         GR dice NO al instante
//!    la prueba (`gpu eterno`)             un computo que salta a si mismo
//! ```
//!
//! Volver a levantar el canal de GR sin reiniciar es E7b: pedirle al RM otro
//! canal y rehacer su contexto (G1..G4, S1).

use core::sync::atomic::{AtomicU32, Ordering};

use bmo_gpu_ga10x::vigilante::{self as vg, Final, Paso, Tras};

use super::{esperando, gr_muerto, gr_ocupado, marcar_muerto, BLUR_ENTRADA, BLUR_EN_MARCHA, IOMMU_NO_BLUR, IOMMU_NO_BLUR_PREPARAR, LIENZO_HECHO};
use crate::ring0::dev::gpu_prestamo::Bar0;

/// Cuantos cortes en este arranque.
static CORTES: AtomicU32 = AtomicU32::new(0);

/// **Tras esperar un trabajo del GR.** `que` = lo que dice la cabina si
/// vencio (nombra el trabajo; el valor son los us esperados). Si vencio, y el
/// canal no estaba ya muerto (un RC del GSP, Xid 13 o 69: entonces no hay
/// nada que cortar), el corte: `Some(final empaquetado)`.
pub(super) fn vigilar(que: &'static str, lanzado: bool, pagado: bool, us: u64, plazo: u64) -> Option<u64> {
    if !vg::vencido(lanzado, pagado, us, plazo) {
        return None;
    }
    crate::ring0::cabina::warn("gpu", que, us);
    if let Some(xid) = gr_muerto() {
        crate::ring0::cabina::warn("gpu", "E7: el canal de GR ya estaba MUERTO (RC del GSP-RM): nada que cortar; Xid", xid as u64);
        return None;
    }
    Some(cortar())
}

/// **El corte**: los pasos, mirando la cola del GSP y el motor grafico.
fn cortar() -> u64 {
    use bmo_gpu_ga10x::canal::GR;
    crate::ring0::cabina::warn("gpu", "E7: EL VIGILANTE CORTA el canal de GR (lo que haria el TDR de Windows): 1 PARAR (STOP_CHANNEL); canal", GR.chid as u64);
    let mut r = Bar0(crate::ring0::dev::gpu::bar0());
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let mut paso = Paso::Parar;
    let fin = loop {
        if super::super::gpu_libos::cortar_gr(paso).is_err() {
            crate::ring0::cabina::warn("gpu", "E7: el paso del corte NO salio hacia el GSP-RM (1 parar, 2 escalar); se mira igual", paso as u64 + 1);
        }
        let desde = crate::ring0::task::scheduler::rdtsc();
        let tras = loop {
            let us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
            let rc = super::super::gpu_libos::rc_del_canal(GR.chid);
            let estado = bmo_gpu_ga10x::Registros::leer(&mut r, vg::ESTADO_GR);
            match vg::tras(paso, rc, estado, us) {
                Tras::Esperar => esperando(us),
                t => break t,
            }
        };
        match tras {
            Tras::Siguiente(p) => {
                crate::ring0::cabina::warn("gpu", "E7: el GR sigue OCUPADO y sin RC tras PARAR: 2 ESCALAR (RC_WATCHDOG_TIMEOUT, el del RM de NVIDIA); NV_PGRAPH_STATUS", bmo_gpu_ga10x::Registros::leer(&mut r, vg::ESTADO_GR) as u64);
                paso = p;
            }
            Tras::Acabo(f) => break f,
            Tras::Esperar => {}
        }
    };
    match fin {
        Final::Rc(xid) => marcar_muerto(xid, "E7: el GSP-RM hizo RC del canal de GR tras el corte (RC_TRIGGERED); Xid"),
        Final::Quieto(Paso::Parar) => marcar_muerto(0, "E7: el GR quedo QUIETO tras PARAR, sin RC: el canal, parado y fuera de su lista; Xid"),
        Final::Quieto(Paso::Escalar) => marcar_muerto(0, "E7: el GR quedo QUIETO tras ESCALAR, sin RC_TRIGGERED a la vista; Xid"),
        Final::Sigue => {
            crate::ring0::cabina::warn("gpu", "E7: ni RC ni GR quieto tras los DOS pasos: la 3060 SIGUE GIRANDO -- hay que reiniciar; NV_PGRAPH_STATUS", bmo_gpu_ga10x::Registros::leer(&mut r, vg::ESTADO_GR) as u64);
            marcar_muerto(0, "E7: el canal de GR, dado por perdido; Xid");
        }
    }
    let v = vg::empaquetar(fin);
    let n = CORTES.fetch_add(1, Ordering::AcqRel) + 1;
    crate::ring0::cabina::count("gpu", "E7: cortes del vigilante en este arranque", n as u64);
    v
}

/// **E7: el trabajo ETERNO** (`gpu eterno`): un computo que salta a si mismo,
/// a proposito, para que el vigilante lo corte. `ficha` = la de S3.
/// `Ok(eterno::empaquetar(..))`. Deja el canal de GR MUERTO hasta reiniciar:
/// es la ULTIMA prueba de una sesion.
pub fn eterno(ficha: u64) -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    if bar0 == 0 || !LIENZO_HECHO.load(Ordering::Acquire) || !bmo_gpu_ga10x::computo::ficha_valida(ficha) {
        return Err(IOMMU_NO_BLUR);
    }
    let e = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(e) || gr_ocupado() {
        return Err(IOMMU_NO_BLUR);
    }
    let r = eterno_(bar0, ficha as u32, e);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    r
}

fn eterno_(bar0: u64, ficha: u32, e: u32) -> Result<u64, u32> {
    use bmo_gpu_ga10x::eterno as et;
    let mut r = Bar0(bar0);
    if !et::preparar(&mut r, e) {
        crate::ring0::cabina::warn("gpu", "E7: el tramo del trabajo eterno no quedo preparado; no se toca el timbre", 0);
        return Err(IOMMU_NO_BLUR_PREPARAR);
    }
    crate::ring0::cabina::warn("gpu", "E7: el trabajo ETERNO, a proposito: un BRA a si mismo en el GR; lo tiene que cortar el vigilante; entrada", e as u64);
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = et::lanzar(&mut r, ficha, e);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(e), Ordering::Release);
    }
    let (mut qmd, mut fin, mut us) = (0, 0, 0);
    while lanzado && us < vg::PLAZO_US {
        (_, qmd, fin) = et::mirar(&mut r);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if et::pagado(qmd, fin) {
            break;
        }
        esperando(us);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let pagado = et::pagado(qmd, fin);
    let corte = vigilar("E7: el trabajo ETERNO no volvio en su plazo (como debe); us", lanzado, pagado, us, vg::PLAZO_US);
    let v = et::empaquetar(lanzado, pagado, corte.is_some(), us, corte.unwrap_or(0));
    if et::sano(v) {
        crate::ring0::cabina::count("gpu", "E7: EL VIGILANTE CORTO EL TRABAJO ETERNO: la 3060 dejo de girar (final del corte)", corte.unwrap_or(0));
    } else {
        crate::ring0::cabina::warn("gpu", "E7: la prueba del vigilante NO salio (lanzado | pagado << 1 | cortado << 2 | us << 8 | final << 32)", v);
    }
    Ok(v)
}
