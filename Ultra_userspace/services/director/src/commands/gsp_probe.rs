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
