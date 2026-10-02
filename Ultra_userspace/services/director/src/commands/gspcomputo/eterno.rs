//! **E7: la prueba del vigilante** -- la orden `gpu eterno`. Manda a la 3060
//! un trabajo que NO ACABA NUNCA (un `BRA` a si mismo, a proposito) y dice lo
//! que hizo el vigilante del kernel: si lo corto, en cuanto, y como acabo.
//!
//! [consumo] NADA      solo cuando el propietario lo teclea, y con `ya`: deja
//!                     el canal de GR fuera hasta reiniciar

use bmo_gpu_ga10x::eterno as et;
use bmo_gpu_ga10x::vigilante::{desempaquetar as final_de, Final, Paso};
use bmo_userland as bmo;

use super::super::After;
use super::{estado, hasta_el_lienzo, NO_TRABAJO_SIN_FICHA};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// `gpu eterno` (`ya` = hacerlo; sin el, solo se explica).
pub(crate) fn orden_eterno(dsk: &mut Desktop, p: &bmo::Pantalla, ya: bool) -> After {
    if !ya {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_PLAIN);
        g.text(b"  gpu eterno: manda a la 3060 un trabajo que NO ACABA NUNCA (un BRA a si mismo), a proposito: el VIGILANTE del kernel (E7) lo tiene que cortar al segundo.\n");
        g.text(b"  Deja el canal de GR FUERA hasta reiniciar (cubo, fractal y apps diran NO): es la ULTIMA prueba de una sesion. Para hacerlo: `gpu eterno ya`\n");
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "la 3060 gira a proposito: el vigilante tiene que cortarla", INK_DIM);
    let r = hasta_el_lienzo()
        .and_then(|_| estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA))
        .and_then(|ficha| bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_ETERNO, ficha));
    let g = &mut dsk.out.grid;
    match r {
        Ok(v) => {
            let (lanzado, pagado, cortado, us, f) = et::desempaquetar(v);
            g.with_ink(if et::sano(v) { INK_GOOD } else { INK_ERR });
            g.text(if et::sano(v) { b"  EL VIGILANTE CORTO EL TRABAJO ETERNO (E7): la 3060 dejo de girar\n" } else { b"  la prueba del vigilante NO salio\n" });
            g.with_ink(INK_PLAIN);
            g.text(b"  lanzado ");
            g.dec(lanzado as u64);
            g.text(b"  pagado ");
            g.dec(pagado as u64);
            g.text(b" (no debe)  cortado ");
            g.dec(cortado as u64);
            g.text(b"  esperado ");
            g.dec(us / 1000);
            g.text(b" ms\n  final: ");
            match final_de(f) {
                Some(Final::Rc(xid)) => {
                    g.text(b"el GSP-RM hizo RC del canal de GR, Xid ");
                    g.dec(xid as u64);
                }
                Some(Final::Quieto(Paso::Parar)) => g.text(b"el GR quedo QUIETO tras PARAR (STOP_CHANNEL), sin RC"),
                Some(Final::Quieto(Paso::Escalar)) => g.text(b"el GR quedo QUIETO tras ESCALAR (RC_WATCHDOG_TIMEOUT)"),
                Some(Final::Sigue) => g.text(b"NI RC NI QUIETO tras los dos pasos: la 3060 sigue girando -- reinicia"),
                None => g.text(b"sin corte"),
            }
            g.text(b"\n  el canal de GR queda fuera hasta reiniciar; mira la cabina (`E7:`)\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  el trabajo eterno no salio; motivo ");
            g.dec(m as u64);
            g.text(b" (sin el lienzo, sin ficha, o el GR ya ocupado o muerto)\n");
        }
    }
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
