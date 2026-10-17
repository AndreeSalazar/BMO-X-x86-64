//! Lee una copia FIFO NVIDIA o la plantilla de VERRANO; no abre GPU.
use std::{env, fs::File, io::{self, Read, Write}};
use bmo_gpu_ga10x::inspeccion::{self, empuje::Push};
use bmo_verrano::lens::{Capture, Origin, Snapshot, Space, PAGE_BYTES};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = if let Some(path) = args.next() {
        if args.next().is_some() { return Err("Uso: ordenes [COPIA_FIFO.bin]; sin fichero muestra la plantilla".into()); }
        // Techo del visor anfitrion, no del dispositivo. No cargar un fichero
        // arbitrariamente grande en memoria por un diagnostico accidental.
        let mut b = Vec::new(); File::open(path)?.take(1024*1024+1).read_to_end(&mut b)?;
        if b.len() > 1024*1024 { return Err("el visor admite hasta 1 MiB por segmento".into()); }
        b
    } else {
        let o = inspeccion::verrano_template();
        o.o[..o.n].iter().flat_map(|w| w.to_le_bytes()).collect()
    };
    let stdout = io::stdout(); let mut out = stdout.lock();
    writeln!(out, "COPIA CPU NVIDIA; clases declaradas por SET_OBJECT, no observadas")?;
    writeln!(out, "Sin fichero: plantilla fija; con fichero: copia dada, sin identidad atomica ni clase observada.")?;
    for (k, bytes) in input.chunks(PAGE_BYTES).enumerate() {
        let s = Snapshot::new(Origin::Prepared, Space::Resource, 0, input.len() as u64, (k*PAGE_BYTES) as u64, Capture::Bytes(bytes)).map_err(|e| format!("ventana: {e:?}"))?;
        let mut text = String::new(); s.write(&mut text)?; out.write_all(text.as_bytes())?;
    }
    let mut packets = 0; let mut commands = 0;
    let mut context = inspeccion::metodos::Context::new();
    let parsed = Push::from_bytes(&input).map_err(|e| format!("+{}: {}", e.word*4, e.reason.label()))?;
    for packet in parsed {
        let p = packet.map_err(|e| format!("+{:x} raw={:08x}: {} (declarados {}, disponibles {})", e.word*4, e.raw, e.reason.label(), e.declared, e.available))?;
        packets += 1;
        writeln!(out, "PAQUETE +{:04x} {} raw={:08x} datos={}", p.word*4, p.mode.label(), p.raw, p.count)?;
        for k in 0..p.count {
            let c = p.command(k).unwrap(); let meaning = context.describe(c);
            let mut text = String::new(); inspeccion::explain(&mut text, c, meaning)?;
            out.write_all(text.as_bytes())?; commands += 1;
        }
    }
    writeln!(out, "TOTAL {} bytes; {packets} paquetes; {commands} metodos; recorrido completo del segmento, sin acreditar ejecucion", input.len())?;
    Ok(())
}

fn main() { if let Err(e) = run() { eprintln!("NO: {e}"); std::process::exit(1); } }
