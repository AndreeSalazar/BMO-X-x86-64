//! Leer y escribir PNG. Leer, con el decodificador de la casa (`bmo-imagen`,
//! el del visor); escribir, sin comprimir (bloques "stored" de zlib): son
//! fotos de trabajo, no se guardan en el repositorio.

use crate::Imagen;
use std::io::Write;

/// **Lee** un PNG.
pub fn leer(ruta: &str) -> Result<Imagen, String> {
    let b = std::fs::read(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let m = bmo_imagen::medir(&b).map_err(|e| format!("{ruta}: {}", e.motivo()))?;
    let mut px = vec![0u32; (m.ancho * m.alto) as usize];
    let mut taller = vec![0u8; bmo_imagen::TALLER];
    bmo_imagen::decodificar_con(&b, &mut px, &mut taller).map_err(|e| format!("{ruta}: {}", e.motivo()))?;
    for p in px.iter_mut() {
        *p &= 0x00FF_FFFF;
    }
    Ok(Imagen { ancho: m.ancho as usize, alto: m.alto as usize, px })
}

fn crc(d: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in d {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// **Escribe** un PNG de 24 bits.
pub fn escribir(ruta: &str, im: &Imagen) -> Result<(), String> {
    let (w, h) = (im.ancho, im.alto);
    let mut crudo = Vec::with_capacity(h * (1 + 3 * w));
    for y in 0..h {
        crudo.push(0u8);
        for x in 0..w {
            let c = im.px[y * w + x];
            crudo.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
        }
    }
    let mut z = vec![0x78u8, 0x01];
    let trozos: Vec<&[u8]> = crudo.chunks(65535).collect();
    for (i, t) in trozos.iter().enumerate() {
        z.push((i + 1 == trozos.len()) as u8);
        let l = t.len() as u16;
        z.extend_from_slice(&l.to_le_bytes());
        z.extend_from_slice(&(!l).to_le_bytes());
        z.extend_from_slice(t);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &crudo {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut f = std::fs::File::create(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    let mut todo = b"\x89PNG\r\n\x1a\n".to_vec();
    for (t, d) in [(&b"IHDR"[..], ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
        todo.extend_from_slice(&(d.len() as u32).to_be_bytes());
        let mut td = t.to_vec();
        td.extend_from_slice(&d);
        todo.extend_from_slice(&td);
        todo.extend_from_slice(&crc(&td).to_be_bytes());
    }
    f.write_all(&todo).map_err(|e| format!("{ruta}: {e}"))
}
