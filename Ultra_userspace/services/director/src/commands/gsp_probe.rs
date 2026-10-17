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

/// **La foto honda** (selectores 24..32): lo que OpenRM mira, leido justo
/// antes de soltar el booter. Crudo, para comparar buenos y malos.
pub(super) fn honda(s: &mut Output) {
    let read = |sel: u64| bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | sel << 8);
    if read(32) != 1 {
        return;
    }
    let w: [u64; 8] = core::array::from_fn(|i| read(24 + i as u64));
    let mitad = |s: &mut Output, v: u64| s.hex(v & 0xFFFF_FFFF, 8);
    s.text(b"; HONDA: HWCFG2 SEC2/GSP 0x");
    mitad(s, w[0]);
    s.text(b"/0x");
    mitad(s, w[0] >> 32);
    s.text(b", DMA SEC2 0x");
    mitad(s, w[1]);
    s.text(b", ENGINE SEC2/GSP 0x");
    mitad(s, w[1] >> 32);
    s.text(b"/0x");
    mitad(s, w[2]);
    s.text(b", PTIMER 0x");
    s.hex(w[3] >> 32, 8);
    mitad(s, w[3]);
    s.text(b" (+");
    s.dec(w[4]);
    s.text(b" ns en 10 us, PLM 0x");
    mitad(s, w[2] >> 32);
    s.text(b"), ECC MMU 0x");
    mitad(s, w[5]);
    s.text(b"/0x");
    mitad(s, w[5] >> 32);
    s.text(b"/0x");
    mitad(s, w[6]);
    s.text(b", WPR2 0x");
    mitad(s, w[6] >> 32);
    s.text(b"/0x");
    mitad(s, w[7]);
    for (i, v) in w.iter().enumerate() {
        const N: [&[u8]; 8] = [
            b"gpu honda hwcfg2", b"gpu honda dma engine sec2", b"gpu honda engine gsp plm",
            b"gpu honda ptimer", b"gpu honda ptimer avance", b"gpu honda ecc tlb",
            b"gpu honda ecc fill wpr2 lo", b"gpu honda wpr2 hi",
        ];
        datos::anotar(N[i], *v, b"");
    }
}

