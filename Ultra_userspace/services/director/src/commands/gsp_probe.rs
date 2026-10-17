//! G0: imprime la foto previa del GSP sin leer otra vez registros vivos.
//! [consumo] NADA -- solo al pedir informe o guardar.

use bmo_userland as bmo;
use bmo_gpu_ga10x::arranque::boot_probe::BootProbe;
use crate::scene::output::Output;
use crate::commands::datos;

pub(super) fn row(s: &mut Output) {
    let read = |sel: u64| bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | sel << 8);
    let stamp = read(23);
    let Some((probe, expected)) = BootProbe::from_report(read(21), read(22), stamp) else { return; };
    s.text(b"; GSP ANTES: MAILBOX0/1 0x");
    s.hex(probe.mailboxes as u32 as u64, 8);
    s.text(b"/0x");
    s.hex(probe.mailboxes >> 32, 8);
    s.text(b", LIBOS esperado 0x");
    s.hex(expected, 16);
    s.text(if !probe.arguments_readable() {
        b" (INACCESIBLE ya antes del booter)" as &[u8]
    } else if probe.arguments_match(expected) {
        b" (argumentos intactos)"
    } else {
        b" (argumentos distintos; no acredita causa)"
    });
    s.text(b", CPUCTL/BCR 0x");
    s.hex(probe.controls as u32 as u64, 8);
    s.text(b"/0x");
    s.hex(probe.controls >> 32, 8);
    datos::anotar(b"gpu booter gsp antes buzones", probe.mailboxes, b"");
    datos::anotar(b"gpu booter gsp antes controles", probe.controls, b"");
    datos::anotar(b"gpu booter gsp antes argumentos iguales", probe.arguments_match(expected) as u64, b"");
}

/// **La traza del SEC2** al verlo parado (selectores 24..40, `TRACEPC`): donde
/// estaba el booter cuando se rindio. Sin leer, no se escribe nada.
pub(super) fn traza(s: &mut Output) {
    let read = |sel: u64| bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | sel << 8);
    let cab = read(24);
    if cab >> 63 == 0 {
        return;
    }
    let n = ((cab >> 48) & 0xFF).min(32);
    s.text(b"; TRAZA del SEC2: SCTL 0x");
    s.hex(cab & 0xFFFF_FFFF, 8);
    s.text(b", ");
    s.dec(n);
    s.text(b" PC:");
    for i in 0..n {
        let par = read(25 + i / 2);
        s.text(b" ");
        s.hex(if i % 2 == 0 { par & 0xFFFF_FFFF } else { par >> 32 }, 5);
    }
    datos::anotar(b"gpu booter traza cabecera", cab, b"");
    datos::anotar(b"gpu booter traza primera", read(25), b"");
}
