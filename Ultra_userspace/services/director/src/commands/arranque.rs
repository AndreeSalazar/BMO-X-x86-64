//! **EL ARRANQUE Y LAS CAIDAS** (11-10, V10 de `PLAN_LOS_DOCE_DIRECTORES`):
//! por que se reinicio la maquina la vez anterior y como estan los obreros,
//! dicho en el `save` y en `smp` -- antes solo en CABINA (F11), donde un
//! aviso del arranque se pierde entre los de despues.
//!
//! [carril]  VERDE     pinta lo que el kernel contesta; no decide nada
//! [cuesta]  DATO      pregunta a la maquina (INFO); una fila mal leida burla
//!                     al que mira, no a la maquina
//! [riesgo]  ESPEJO    desempaqueta bits que empaqueta el kernel; la forma esta
//!                     en `bmo-abi/syscalls/surface/informe.rs` y en un solo
//!                     sitio por campo
//! [consumo] NADA      solo corre cuando alguien escribe `save` o `smp`
//!
//! ```text
//!    INFO_REINICIO_FCH   el S5_RESET_STATUS del FCH, crudo (plat/porque.rs)
//!    INFO_REINICIO_MCA   bancos con error [0..32), bancos [32..64)
//!    INFO_SMP_CPU        el CR0/CR4 de los obreros contra el BSP
//! ```
//!
//! *** La tabla de bits del FCH es la de Linux y NO se vio en el metal: se
//! comprueba sola con un `reboot` de BMO-X, que tiene que dar el bit 19.

use bmo_userland as bmo;

use super::tabla::{fila, fila_cero, subregla};
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Los motivos del FCH, del mas grave al menos: el primero que este puesto es
/// "el motivo". La misma tabla que `plat/porque.rs` en el kernel.
const MOTIVOS: [(u32, &[u8]); 20] = [
    (23, b"APAGADO INTERNO de la CPU: triple fallo, o #MC sin CR4.MCE"),
    (27, b"SYNC FLOOD del data fabric: un error NO corregido"),
    (31, b"un sync flood pedido por software"),
    (30, b"un error de paridad"),
    (0, b"el pin THERMTRIP: la CPU se paso de temperatura"),
    (9, b"el limite termico interno de la CPU"),
    (25, b"el perro guardian del hardware"),
    (24, b"no arranco antes del plazo del firmware"),
    (29, b"el FCH y el MP1 no se pusieron de acuerdo"),
    (16, b"el pin RESET_L: el boton de reset"),
    (22, b"el pin KB_RST_L (el 8042)"),
    (17, b"un reset de PCI pedido por software"),
    (19, b"0x6 en 0xCF9: un reboot pedido (el de BMO-X)"),
    (18, b"0x4 en 0xCF9"),
    (20, b"0xE en 0xCF9"),
    (21, b"un cambio de estado ACPI"),
    (1, b"el boton de encendido, 4 segundos"),
    (2, b"el pin SHUTDOWN_L"),
    (4, b"apagado remoto (ASF)"),
    (26, b"reset remoto (ASF)"),
];

/// **El motivo del ultimo reinicio**, si el FCH lo dice: `(bit, texto)`.
pub(crate) fn motivo() -> Option<(u32, &'static [u8])> {
    let v = bmo::info(bmo::INFO_REINICIO_FCH);
    if v == u64::MAX || v == 0 || v == 0xFFFF_FFFF {
        return None;
    }
    MOTIVOS.iter().find(|(b, _)| v >> b & 1 != 0).copied()
}

/// El CR0/CR4 de los obreros: `(medido, obreros, distintos, sin cache, sin MCE)`.
fn cpu() -> (bool, u64, u64, u64, u64) {
    let v = bmo::info(bmo::INFO_SMP_CPU);
    let b = |k: u32| v >> k & 0xFF;
    (v >> 63 != 0, b(0), b(8), b(16), b(24))
}

/// **Las lineas de `smp`**: el veredicto de la CPU de los obreros, y el
/// motivo del reinicio anterior. Solo lo que dice algo: sin obreros medidos,
/// nada.
pub(crate) fn lineas_smp(s: &mut Output) {
    let (medido, n, distintos, sin_cache, sin_mce) = cpu();
    if medido && n > 0 {
        if distintos == 0 {
            s.with_ink(INK_GOOD);
            s.text(b"  cpu: los ");
            s.dec(n);
            s.text(b" obreros con el CR0 y el CR4 del BSP\n");
        } else {
            s.with_ink(INK_ECHO);
            s.text(b"  cpu: ");
            s.dec(distintos);
            s.text(b" de ");
            s.dec(n);
            s.text(b" obreros con otro CR0/CR4 que el BSP (F11 dice cual)\n");
        }
        if sin_cache > 0 {
            s.with_ink(INK_ERR);
            s.text(b"  [!] ");
            s.dec(sin_cache);
            s.text(b" con la CACHE APAGADA (CR0.CD/NW): van 10 a 100 veces mas lentos\n");
        }
        if sin_mce > 0 {
            s.with_ink(INK_ERR);
            s.text(b"  [!] ");
            s.dec(sin_mce);
            s.text(b" sin CR4.MCE: un error de maquina en uno REINICIA el PC (V10)\n");
        }
        s.with_ink(INK_PLAIN);
    }
    if let Some((bit, que)) = motivo() {
        s.with_ink(if bit == 19 { INK_ECHO } else { INK_ERR });
        s.text(b"  el reinicio anterior: ");
        s.text(que);
        s.text(b" (bit ");
        s.dec(bit as u64);
        s.text(b")\n");
        s.with_ink(INK_PLAIN);
    }
}

/// **El bloque del `save`**: la caja negra, el motivo del reinicio, los
/// bancos MCA y la CPU de los obreros, juntos -- las cuatro preguntas de
/// "por que se murio la vez anterior".
pub(crate) fn report(s: &mut Output) {
    subregla(s, b"el arranque y las caidas -- por que se reinicio la vez anterior");
    fila(s, b"generacion", bmo::info(bmo::INFO_CAIDA_GENERACION), b"",
         b"arranques que vio la caja negra. Si SUBE, la RAM sobrevive al reinicio");
    fila(s, b"recuperado", bmo::info(bmo::INFO_CAIDA_RECUPERADO), b"bytes",
         b"de la sesion anterior, ya en informe/CAIDA.TXT. 0 = sin rastro");
    let fch = bmo::info(bmo::INFO_REINICIO_FCH);
    if fch == u64::MAX {
        fila(s, b"fch", 0, b"", b"esta CPU no guarda el motivo (o no es AMD Zen)");
    } else {
        fila(s, b"fch crudo", fch, b"", b"S5_RESET_STATUS: un bit por motivo (tabla de Linux)");
        match motivo() {
            Some((bit, que)) => fila(s, b"motivo", bit as u64, b"bit", que),
            None => fila(s, b"motivo", 0, b"", b"el FCH no dice ninguno"),
        }
    }
    let mca = bmo::info(bmo::INFO_REINICIO_MCA);
    fila_cero(s, b"mca errores", mca & 0xFFFF_FFFF,
              b"bancos con un error de maquina guardado (F11: [reinicio])");
    fila(s, b"mca bancos", mca >> 32, b"", b"los que tiene esta CPU");
    let (medido, n, distintos, sin_cache, sin_mce) = cpu();
    if medido {
        fila(s, b"obreros cr", n, b"", b"medidos al levantarlos (`smp all`)");
        fila_cero(s, b"cr distinto", distintos, b"obreros con otro CR0/CR4 que el BSP");
        fila_cero(s, b"sin cache", sin_cache, b"obreros con CR0.CD o NW: la cache APAGADA");
        fila_cero(s, b"sin mce", sin_mce, b"obreros sin CR4.MCE: una #MC alli reinicia el PC");
    } else {
        fila(s, b"obreros cr", 0, b"", b"sin medir: nadie escribio `smp all` en este arranque");
    }
}
