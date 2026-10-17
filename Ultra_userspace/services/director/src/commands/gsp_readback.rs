//! Presentacion de pares vivos cuya INFO confunde ausencia con dos ceros.
//! [consumo] NADA
use crate::scene::output::Output;

pub(super) fn mailboxes(s: &mut Output, pair: u64) {
    if pair == 0 {
        s.text(b"sin dato distinguible (cero o lectura fallida)");
        return;
    }
    s.text(b"MAILBOX0/1 crudos 0x");
    s.hex(pair as u32 as u64, 8);
    s.text(b"/0x");
    s.hex(pair >> 32, 8);
    if bmo_gpu_ga10x::es_error_pri(pair as u32)
        || bmo_gpu_ga10x::es_error_pri((pair >> 32) as u32) {
        s.text(b" (error PRI; no respuesta del firmware)");
    }
}

pub(super) fn autopsy(s: &mut Output, attempt: u64) -> bool {
    if attempt == 1 { return true; }
    s.text(b"autopsia sin sello de intento: no unir campos previos con la primera parada; detalle omitido\n");
    false
}
