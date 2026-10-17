//! Volcado completo de un fichero SM86 en el anfitrion; no abre una GPU.
//! --header 128 para un programa grafico con SPH; 0 para SASS sin cabecera.
use std::{env, fs::File, io::{self, Read, Write}};
use bmo_gpu_ga10x::sass::lente;
use bmo_verrano::lens::{Capture, Origin, Snapshot, Space, PAGE_BYTES};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("Uso: lente FICHERO [--header BYTES]")?;
    let header = match args.next().as_deref() {
        None => 0,
        Some("--header") => args.next().ok_or("falta medida de cabecera")?.parse::<u64>()?,
        _ => return Err("opcion desconocida".into()),
    };
    if args.next().is_some() { return Err("sobran argumentos".into()); }
    if header % 16 != 0 { return Err("la cabecera debe ser multiplo de 16".into()); }
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    if header > total { return Err("cabecera mayor que el fichero".into()); }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "NVIDIA SM86; FICHERO CPU; cabecera declarada={header}; no prueba ejecucion")?;
    let mut offset = 0;
    let mut buffer = [0u8; PAGE_BYTES];
    let mut words = 0;
    let mut unknown = 0;
    let mut cut = 0;
    while offset < total {
        let n = (total - offset).min(PAGE_BYTES as u64) as usize;
        file.read_exact(&mut buffer[..n])?;
        let s = Snapshot::new(Origin::Prepared, Space::Resource, 0, total, offset, Capture::Bytes(&buffer[..n])).map_err(|e| format!("ventana invalida: {e:?}"))?;
        let mut text = String::new();
        s.write(&mut text)?;
        out.write_all(text.as_bytes())?;
        for (k, b) in buffer[..n].chunks(16).enumerate() {
            let at = offset + (k * 16) as u64;
            if at < header { continue; }
            match lente::read(b, at - header) {
                Ok(i) => {
                    words += 1;
                    writeln!(out, "SASS +{at:016x} lo={:016x} hi={:016x} op={:03x} forma={} guarda={}", i.lo, i.hi, i.opcode, i.form, i.guard)?;
                    if let Some(d) = i.decoded {
                        writeln!(out, "  LEIDA sin juicio; stall={} wb={} rb={} wait={:02x}; escribe={:?} lee={:?}", d.stall, d.write_barrier, d.read_barrier, d.wait_mask, d.writes, d.reads)?;
                    } else { unknown += 1; writeln!(out, "  DESCONOCIDA; bytes conservados")?; }
                }
                Err(e) => { cut += 1; writeln!(out, "  INCOMPLETA +{at:016x}: {e:?}")?; }
            }
        }
        offset += n as u64;
    }
    // No anuncia lectura completa si el fichero crecio durante el recorrido.
    if file.read(&mut buffer[..1])? != 0 || file.metadata()?.len() != total {
        return Err("el fichero cambio de medida durante la lectura".into());
    }
    writeln!(out, "TOTAL {offset}/{total} bytes; palabras={words}; desconocidas={unknown}; cortadas={cut}")?;
    writeln!(out, "Cobertura de bytes completa; semantica y ejecucion no certificadas; copia de fichero sin garantia atomica.")?;
    Ok(())
}

fn main() {
    if let Err(e) = run() { eprintln!("NO: {e}"); std::process::exit(1); }
}
