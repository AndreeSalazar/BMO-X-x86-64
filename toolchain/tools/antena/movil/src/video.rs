//! **Un video**: ffmpeg lo convierte EN VIVO a MPEG-1 + MP2 640x360 (lo que
//! BMO-X sabe mostrar) y la antena lo manda tal cual sale. Mismas banderas
//! que `antena.py`, y la misma frontera: ffmpeg solo lee FICHEROS.

use crate::charla::Tubo;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

pub const ANCHO: u32 = 640;
pub const ALTO: u32 = 360;

pub fn servir<T: Tubo>(t: &mut T, ruta: &Path, ffmpeg: &str) -> io::Result<()> {
    let escala = format!("scale={ANCHO}:{ALTO}:force_original_aspect_ratio=decrease,pad={ANCHO}:{ALTO}:(ow-iw)/2:(oh-ih)/2");
    let hijo = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-protocol_whitelist", "file,pipe", "-re", "-i"])
        .arg(ruta)
        .args(["-vf", &escala, "-c:v", "mpeg1video", "-b:v", "1200k", "-r", "30"])
        .args(["-c:a", "mp2", "-b:a", "128k", "-ac", "2", "-ar", "44100", "-f", "mpeg", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn();
    let mut hijo = match hijo {
        Ok(h) => h,
        Err(_) => {
            t.write_all(b"NO no hay ffmpeg en la antena (pkg install ffmpeg)\n")?;
            return Ok(());
        }
    };
    t.write_all(format!("VIDEO 0 mpeg1 {ANCHO}x{ALTO}\n").as_bytes())?;
    let mut salida = hijo.stdout.take().expect("stdout pedido");
    let (mut enviados, inicio) = (0u64, Instant::now());
    let mut b = vec![0u8; 16 * 1024];
    loop {
        let n = match salida.read(&mut b) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        // BMO-X cierra la conexion para decir PARA.
        if t.write_all(&b[..n]).is_err() {
            break;
        }
        enviados += n as u64;
    }
    let _ = hijo.kill();
    let _ = hijo.wait();
    let s = inicio.elapsed().as_secs_f64().max(0.001);
    println!("antena: video terminado, {enviados} bytes en {s:.0} s ({:.2} Mbit/s)", enviados as f64 * 8.0 / 1e6 / s);
    Ok(())
}
