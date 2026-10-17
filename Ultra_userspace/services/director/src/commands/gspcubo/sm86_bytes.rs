//! La pregunta: cuales son los bytes PREPARADOS de nuestros programas.
//! [carril] VERDE -- ninguna orden al aparato, ni lectura MMIO
//! [consumo] NADA -- pagina de 64 bytes bajo consulta
//!
//! Vive bajo el adaptador: la vista comun no sabe de SM86 ni del BSF.

use core::fmt::{self, Write};
use bmo_gpu_ga10x::sass::lente;
use bmo_verrano::lens::{self, Capture, Origin, Snapshot, Space};
use crate::desktop::Desktop;
use crate::commands::After;
use crate::scene::output::{INK_ERR, INK_PLAIN};

struct Output<'a>(&'a mut Desktop);
impl Write for Output<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.out.grid.text(s.as_bytes());
        Ok(())
    }
}

pub(crate) fn show(dsk: &mut Desktop, args: &[u8]) -> After {
    let Ok(args) = core::str::from_utf8(args) else { return no(dsk, "NO: argumentos invalidos\n"); };
    let mut words = args.split_ascii_whitespace();
    if words.next() != Some("bytes") { return no(dsk, "Uso: gpu verrano bytes vs|ps|bmox-vs|bmox-ps [offset decimal]\n"); }
    let selected = words.next().unwrap_or("vs");
    let offset = match words.next() {
        None => 0,
        Some(w) if w.bytes().all(|b| b.is_ascii_digit()) => match w.parse::<u64>() {
            Ok(v) => v,
            Err(_) => return no(dsk, "NO: offset decimal fuera de u64\n"),
        },
        _ => return no(dsk, "NO: offset decimal requerido\n"),
    };
    if words.next().is_some() { return no(dsk, "NO: sobran argumentos\n"); }
    let program = match selected {
        "bmox-vs" => super::BMOX12_VS,
        "bmox-ps" => super::BMOX12_PS,
        "vs" | "ps" => {
            let Some((vs, ps, _)) = bmo_bsf::Bsf::parse(super::SOBRE).ok().and_then(|b| super::programas(&b)) else {
                return no(dsk, "NO: BSF sin los programas esperados\n");
            };
            if selected == "vs" { vs } else { ps }
        }
        _ => return no(dsk, "Uso: gpu verrano bytes vs|ps|bmox-vs|bmox-ps [offset decimal]\n"),
    };
    let total = program.len() as u64;
    let Ok(n) = lens::page(total, offset) else { return no(dsk, "NO: offset despues del fin\n"); };
    if offset % lente::INSTRUCTION_BYTES as u64 != 0 { return no(dsk, "NO: offset debe ser multiplo de 16\n"); }
    // offset <= program.len(), comprobado arriba; no se transforma en VA GPU.
    let bytes = &program[offset as usize..offset as usize + n];
    let Ok(snapshot) = Snapshot::new(Origin::Prepared, Space::Resource, 0, total, offset, Capture::Bytes(bytes)) else {
        return no(dsk, "NO: ventana de bytes invalida\n");
    };
    let header = bmo_gpu_ga10x::raster::SPH * 4;
    dsk.out.grid.with_ink(INK_PLAIN);
    {
        let mut out = Output(dsk);
        let _ = writeln!(out, "VERRANO / NVIDIA SM86 / {selected}; SPH={header} bytes");
        let _ = out.write_str("Copia CPU preparada; no acredita envio ni ejecucion.\n");
        let _ = snapshot.write(&mut out);
        for (k, word) in bytes.chunks(lente::INSTRUCTION_BYTES).enumerate() {
            let at = offset + (k * lente::INSTRUCTION_BYTES) as u64;
            if at < header as u64 { continue; } // Cabecera: no es SASS.
            match lente::read(word, at - header as u64) {
                Ok(i) => {
                    let _ = writeln!(out, "SASS +{at:04x}: op={:03x} forma={} guarda={}", i.opcode, i.form, i.guard);
                    if let Some(d) = i.decoded {
                        let _ = writeln!(out, "  leida; stall={} wb={} rb={} wait={:02x}; sin juicio de programa", d.stall, d.write_barrier, d.read_barrier, d.wait_mask);
                    } else { let _ = out.write_str("  DESCONOCIDA: conserva todos sus bytes\n"); }
                }
                Err(_) => { let _ = out.write_str("  INSTRUCCION CORTADA: no se rellena ni interpreta\n"); }
            }
        }
        if let Some(next) = snapshot.next() {
            let _ = writeln!(out, "Continuar: gpu verrano bytes {selected} {next}");
        } else { let _ = out.write_str("FIN de esta consulta; no certifica haber leido paginas previas.\n"); }
    }
    crate::commands::datos::anotar(b"gpu verrano bytes", offset, b"PREPARADO CPU; no lectura de VRAM");
    dsk.field.n = 0;
    After::Settle
}

fn no(dsk: &mut Desktop, text: &str) -> After {
    dsk.out.grid.with_ink(INK_ERR);
    dsk.out.grid.text(text.as_bytes());
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
