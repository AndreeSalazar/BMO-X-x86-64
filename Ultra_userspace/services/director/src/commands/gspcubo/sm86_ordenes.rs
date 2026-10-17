//! Consulta NVIDIA de ORDENES PREPARADAS, con carril y motivo.
//! [carril] VERDE -- plantilla CPU; no observa ni envia al canal real
//! [consumo] NADA -- pagina de 32 bytes bajo consulta
//!
//! El lector recorre desde cero para saber si una palabra es cabecera o
//! dato. Las direcciones son las planeadas por el constructor del ejemplo.

use core::fmt::{self, Write};
use bmo_gpu_ga10x::inspeccion::{self, empuje::Push};
use bmo_verrano::lens::{Capture, Origin, Snapshot, Space};
use crate::{commands::After, desktop::Desktop, scene::output::{INK_ERR, INK_PLAIN}};

struct Output<'a>(&'a mut Desktop);
impl Write for Output<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result { self.0.out.grid.text(text.as_bytes()); Ok(()) }
}

pub(crate) fn show(dsk: &mut Desktop, args: &[u8]) -> After {
    let Ok(args) = core::str::from_utf8(args) else { return no(dsk, "NO: argumentos invalidos\n"); };
    let mut words = args.split_ascii_whitespace();
    if words.next() != Some("ordenes") { return no(dsk, "Uso: gpu verrano ordenes [offset decimal]\n"); }
    let offset = match words.next() {
        None => 0,
        Some(s) if s.bytes().all(|b| b.is_ascii_digit()) => match s.parse::<u64>() {
            Ok(n) => n, Err(_) => return no(dsk, "NO: offset fuera de u64\n"),
        },
        _ => return no(dsk, "NO: offset decimal requerido\n"),
    };
    if words.next().is_some() { return no(dsk, "NO: sobran argumentos\n"); }
    let orders = inspeccion::verrano_template();
    let total = orders.n as u64 * 4;
    if offset > total || offset % 4 != 0 { return no(dsk, "NO: offset fuera de recurso o no multiplo de 4\n"); }
    let n = (total-offset).min(32) as usize;
    let mut bytes = [0u8; 32];
    for (k, b) in bytes[..n].iter_mut().enumerate() {
        let at = offset as usize + k;
        *b = (orders.o[at/4] >> ((at%4)*8)) as u8;
    }
    let Ok(snapshot) = Snapshot::new(Origin::Prepared, Space::Resource, 0, total, offset, Capture::Bytes(&bytes[..n])) else { return no(dsk, "NO: ventana invalida\n"); };
    dsk.out.grid.with_ink(INK_PLAIN);
    {
        let mut out = Output(dsk);
        let _ = out.write_str("NVIDIA AMPERE_B: PLANTILLA VERRANO 1280x720, un triangulo\nDirecciones planeadas; no captura del ultimo dibujo ni prueba de envio.\n");
        let _ = snapshot.write(&mut out);
        let mut context = inspeccion::metodos::Context::new();
        for packet in Push::from_words(&orders.o[..orders.n]) {
            match packet {
                Ok(packet) => {
                    for k in 0..packet.count {
                        let c = packet.command(k).unwrap();
                        let meaning = context.describe(c);
                        if (offset..offset+n as u64).contains(&(c.word as u64*4)) {
                            let _ = writeln!(out, "Paquete +{:04x} {}", packet.word*4, packet.mode.label());
                            let _ = inspeccion::explain(&mut out, c, meaning);
                        }
                    }
                }
                Err(e) => { let _ = writeln!(out, "NO +{:04x} raw={:08x}: {}", e.word*4, e.raw, e.reason.label()); break; }
            }
        }
        if let Some(next) = snapshot.next() { let _ = writeln!(out, "Continuar: gpu verrano ordenes {next}"); }
        else { let _ = out.write_str("FIN de ventana; no acredita ejecucion ni paginas anteriores.\n"); }
    }
    crate::commands::datos::anotar(b"gpu verrano ordenes", offset, b"PLANTILLA PREPARADA CPU, no captura de canal");
    dsk.field.n = 0;
    After::Settle
}

fn no(dsk: &mut Desktop, text: &str) -> After {
    dsk.out.grid.with_ink(INK_ERR); dsk.out.grid.text(text.as_bytes()); dsk.out.grid.with_ink(INK_PLAIN);
    dsk.field.n = 0; After::Settle
}
