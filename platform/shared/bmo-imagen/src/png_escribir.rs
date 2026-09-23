//! **PNG para ESCRIBIR** (2026-09-22): la captura de pantalla de BMO-X, en el
//! formato que abre cualquier ordenador y ocupando lo que debe.
//!
//! generacion: nieto -- recibe los pixeles por una funcion, dos bufers y un
//! taller, y devuelve cuantos bytes escribio; no sabe de ficheros ni de
//! pantallas.
//!
//! ```text
//!    IHDR    RGB de 8 bits, sin entrelazar (tipo 2): lo que es una pantalla
//!    filtro  por FILA, el que menos suma de los cuatro que se prueban
//!            (ninguno, Sub, Up, Paeth) -- la regla de siempre de libpng. En
//!            una captura, Up deja las filas repetidas en ceros y Paeth los
//!            degradados de una foto en residuos chicos
//!    IDAT    UNO, con el flujo zlib entero de `deflar`
//!    IEND
//!    CRC     de cada chunk, calculado aqui con su tabla (ISO 3309)
//! ```
//!
//! Lo que sale se comprueba en el anfitrion con el LECTOR de esta misma
//! libreria (`png.rs`, con su propio inflate): escribir y volver a leer tiene
//! que dar los mismos pixeles, bit a bit. Y el dia que se estreno se abrio con
//! Pillow (zlib), que es el que abre medio mundo.

use crate::{deflar, lados, Error};

/// La fila mas ancha, en bytes RGB.
const FILA_MAX: usize = crate::LADO_MAX as usize * 3;

/// Taller que pide [`codificar`], en `u32`: el de `deflar` y dos filas.
pub const TALLER: usize = deflar::TALLER + (2 * FILA_MAX).div_ceil(4);

/// Los bytes de la imagen CRUDA (filtro + RGB por fila), que es lo que se
/// comprime. Quien llama da un bufer de al menos esto.
pub fn crudo(ancho: u32, alto: u32) -> usize {
    alto as usize * (1 + ancho as usize * 3)
}

/// **Lo mas que puede ocupar el PNG**: firma, IHDR, IDAT con la cota de
/// `deflar` e IEND.
pub fn cota(ancho: u32, alto: u32) -> usize {
    8 + 25 + 12 + deflar::cota(crudo(ancho, alto)) + 12
}

const CRC_TABLA: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
};

fn crc(b: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in b {
        c = CRC_TABLA[((c ^ x as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// **Escribe un PNG** de `ancho` x `alto` en `dst` y devuelve cuantos bytes
/// ocupa. `pixel(x, y)` da `0x00RRGGBB`. `crudo_buf` mide al menos
/// [`crudo`]; `taller` al menos [`TALLER`]; `dst` al menos [`cota`] (con menos
/// puede caber, y si no cabe es `NoCabe`: nunca un PNG a medias).
pub fn codificar(
    ancho: u32,
    alto: u32,
    pixel: &mut dyn FnMut(u32, u32) -> u32,
    crudo_buf: &mut [u8],
    taller: &mut [u32],
    dst: &mut [u8],
) -> Result<usize, Error> {
    lados(ancho, alto)?;
    let n_crudo = crudo(ancho, alto);
    if crudo_buf.len() < n_crudo || dst.len() < 8 + 25 + 12 + 12 {
        return Err(Error::NoCabe);
    }
    if taller.len() < TALLER {
        return Err(Error::SinTaller);
    }
    let (t_deflar, t_filas) = taller.split_at_mut(deflar::TALLER);
    let fw = ancho as usize * 3;

    // -- 1. Las filas, filtradas --
    // Las dos filas sin filtrar viven en el taller, como bytes sacados de u32:
    // la de arriba en `0..fw` y la de ahora en `fw..2fw`.
    let mut anterior_valida = false;
    for y in 0..alto as usize {
        // La fila de ahora, en RGB, en la segunda mitad del taller de filas.
        for x in 0..ancho as usize {
            let c = pixel(x as u32, y as u32);
            let i = fw + x * 3;
            poner(t_filas, i, (c >> 16) as u8);
            poner(t_filas, i + 1, (c >> 8) as u8);
            poner(t_filas, i + 2, c as u8);
        }
        let cur = |i: usize, t: &[u32]| tomar(t, fw + i);
        let arriba = |i: usize, t: &[u32]| if anterior_valida { tomar(t, i) } else { 0 };
        // El filtro que menos suma, midiendo los residuos como con signo.
        let mut mejor = 0u8;
        let mut coste_mejor = u64::MAX;
        for f in 0..5u8 {
            if f == 3 {
                continue; // Average: casi nunca gana en una pantalla y cuesta una pasada
            }
            let mut coste = 0u64;
            for i in 0..fw {
                let x = cur(i, t_filas);
                let a = if i >= 3 { cur(i - 3, t_filas) } else { 0 };
                let b = arriba(i, t_filas);
                let c = if i >= 3 { arriba(i - 3, t_filas) } else { 0 };
                let r = match f {
                    0 => x,
                    1 => x.wrapping_sub(a),
                    2 => x.wrapping_sub(b),
                    _ => x.wrapping_sub(paeth(a, b, c)),
                };
                coste += (r as i8).unsigned_abs() as u64;
                if coste >= coste_mejor {
                    break;
                }
            }
            if coste < coste_mejor {
                coste_mejor = coste;
                mejor = f;
            }
        }
        let base = y * (1 + fw);
        crudo_buf[base] = mejor;
        for i in 0..fw {
            let x = cur(i, t_filas);
            let a = if i >= 3 { cur(i - 3, t_filas) } else { 0 };
            let b = arriba(i, t_filas);
            let c = if i >= 3 { arriba(i - 3, t_filas) } else { 0 };
            crudo_buf[base + 1 + i] = match mejor {
                0 => x,
                1 => x.wrapping_sub(a),
                2 => x.wrapping_sub(b),
                _ => x.wrapping_sub(paeth(a, b, c)),
            };
        }
        // La de ahora pasa a ser la de arriba.
        for i in 0..fw {
            let v = tomar(t_filas, fw + i);
            poner(t_filas, i, v);
        }
        anterior_valida = true;
    }

    // -- 2. Firma, IHDR y la cabecera del IDAT --
    let mut n = 0usize;
    let pon = |d: &mut [u8], b: &[u8], n: &mut usize| {
        d[*n..*n + b.len()].copy_from_slice(b);
        *n += b.len();
    };
    pon(dst, &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], &mut n);
    let mut ihdr = [0u8; 17];
    ihdr[..4].copy_from_slice(b"IHDR");
    ihdr[4..8].copy_from_slice(&ancho.to_be_bytes());
    ihdr[8..12].copy_from_slice(&alto.to_be_bytes());
    ihdr[12] = 8; // bits por canal
    ihdr[13] = 2; // RGB
    // 14, 15, 16: compresion 0, filtro 0, sin entrelazar
    pon(dst, &13u32.to_be_bytes(), &mut n);
    pon(dst, &ihdr, &mut n);
    pon(dst, &crc(&ihdr).to_be_bytes(), &mut n);
    let idat = n;
    n += 8; // largo y "IDAT", que se escriben al saber el largo

    // -- 3. El flujo zlib, directo en su sitio --
    let fin = dst.len().saturating_sub(12 + 4);
    if fin <= n {
        return Err(Error::NoCabe);
    }
    let z = deflar::comprimir(&crudo_buf[..n_crudo], t_deflar, &mut dst[n..fin])?;
    dst[idat..idat + 4].copy_from_slice(&(z as u32).to_be_bytes());
    dst[idat + 4..idat + 8].copy_from_slice(b"IDAT");
    let c = crc(&dst[idat + 4..n + z]);
    n += z;
    pon(dst, &c.to_be_bytes(), &mut n);

    // -- 4. IEND --
    pon(dst, &0u32.to_be_bytes(), &mut n);
    pon(dst, b"IEND", &mut n);
    pon(dst, &crc(b"IEND").to_be_bytes(), &mut n);
    Ok(n)
}

fn tomar(t: &[u32], i: usize) -> u8 {
    (t[i / 4] >> ((i % 4) * 8)) as u8
}

fn poner(t: &mut [u32], i: usize, v: u8) {
    let s = (i % 4) * 8;
    t[i / 4] = (t[i / 4] & !(0xFF << s)) | ((v as u32) << s);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Escribe con `codificar` y lo vuelve a leer con el LECTOR de la casa.
    fn ida_y_vuelta(w: u32, h: u32, f: impl Fn(u32, u32) -> u32) -> usize {
        let mut crudo_buf = vec![0u8; crudo(w, h)];
        let mut taller = vec![0u32; TALLER];
        let mut dst = vec![0u8; cota(w, h)];
        let mut px = |x: u32, y: u32| f(x, y) & 0x00FF_FFFF;
        let n = codificar(w, h, &mut px, &mut crudo_buf, &mut taller, &mut dst).expect("codifica");
        let png = &dst[..n];
        let m = crate::medir(png).expect("se mide");
        assert_eq!((m.ancho, m.alto, m.formato), (w, h, crate::Formato::Png));
        let mut vuelta = vec![0u32; (w * h) as usize];
        let mut t = vec![0u8; crate::TALLER];
        crate::decodificar_con(png, &mut vuelta, &mut t).expect("se lee");
        for y in 0..h {
            for x in 0..w {
                let esperado = 0xFF00_0000 | (f(x, y) & 0x00FF_FFFF);
                assert_eq!(vuelta[(y * w + x) as usize], esperado, "pixel ({x},{y}) de {w}x{h}");
            }
        }
        n
    }

    #[test]
    fn un_pixel() {
        ida_y_vuelta(1, 1, |_, _| 0x00AB_CDEF);
    }

    #[test]
    fn un_color_liso_comprime_casi_todo() {
        let n = ida_y_vuelta(640, 400, |_, _| 0x0009_080F);
        assert!(n < 2_000, "un liso de 640x400 ocupo {n} bytes");
    }

    #[test]
    fn un_degradado() {
        ida_y_vuelta(300, 200, |x, y| (x << 16) | (y << 8) | ((x + y) & 0xFF));
    }

    /// Ruido: no comprime, y entonces tienen que salir bloques TAL CUAL. Lo
    /// que se comprueba es que se lean bien y que no pase de la cota.
    #[test]
    fn el_ruido_va_tal_cual_y_se_lee_igual() {
        let ruido = |x: u32, y: u32| {
            let mut v = (x as u64 * 2_654_435_761 + y as u64 * 40_503) ^ 0x9E37_79B9;
            v ^= v >> 13;
            v = v.wrapping_mul(0x5851_F42D_4C95_7F2D);
            (v >> 20) as u32
        };
        let n = ida_y_vuelta(257, 131, ruido);
        assert!(n <= cota(257, 131));
    }

    /// Una pantalla de escritorio de mentira: fondo, una ventana y texto.
    #[test]
    fn una_pantalla_de_mentira_comprime_mucho() {
        let n = ida_y_vuelta(1024, 600, |x, y| {
            if (200..800).contains(&x) && (100..500).contains(&y) {
                if y < 126 { 0x0025_1F44 } else if (x / 8 + y / 16) % 7 == 0 { 0x00E6_EDF6 } else { 0x001A_1631 }
            } else {
                0x0016_1236 + (y / 8)
            }
        });
        let crudo_bytes = crudo(1024, 600);
        assert!(n * 20 < crudo_bytes, "la pantalla de mentira ocupo {n} de {crudo_bytes} en crudo");
    }

    /// La mas ancha que se puede, de una fila: la fila de 4096 cabe en el taller.
    #[test]
    fn la_fila_mas_ancha() {
        ida_y_vuelta(crate::LADO_MAX, 2, |x, y| x ^ (y << 12));
    }

    #[test]
    fn sin_sitio_no_hay_png_a_medias() {
        let mut crudo_buf = vec![0u8; crudo(64, 64)];
        let mut taller = vec![0u32; TALLER];
        let mut dst = vec![0u8; 100];
        let mut px = |x: u32, y: u32| x * 977 ^ y * 131;
        let r = codificar(64, 64, &mut px, &mut crudo_buf, &mut taller, &mut dst);
        assert_eq!(r, Err(Error::NoCabe));
    }

    /// ** LAS DOS CAPTURAS DEL RYZEN (22-09), re-escritas por BMO-X. Se leen
    /// de `docs/evidencia/` con el lector de la casa, se escriben con este
    /// codificador y se vuelven a leer: mismos pixeles. Y se dice cuanto ocupan
    /// contra el BMP que salio aquella noche (6.220.854 bytes).
    ///
    /// `ignore` porque en modo de pruebas (sin optimizar) son unos segundos:
    /// `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn las_capturas_del_ryzen() {
        let fuentes: [(&str, &[u8]); 2] = [
            ("12 (la ciudad)", include_bytes!("../../../../docs/evidencia/12-captura-escritorio-ciudad.png")),
            ("13 (panel y ventanas)", include_bytes!("../../../../docs/evidencia/13-captura-panel-cabina-sonido.png")),
        ];
        for (nombre, png) in fuentes {
            let m = crate::medir(png).unwrap();
            let mut px = vec![0u32; (m.ancho * m.alto) as usize];
            let mut t = vec![0u8; crate::TALLER];
            crate::decodificar_con(png, &mut px, &mut t).unwrap();
            let w = m.ancho;
            let n = ida_y_vuelta(m.ancho, m.alto, |x, y| px[(y * w + x) as usize]);
            let bmp = 54 + ((w * 3 + 3) & !3) as usize * m.alto as usize;
            std::println!("{nombre}: {n} bytes, el BMP {bmp} ({:.1} %)", n as f64 * 100.0 / bmp as f64);
            if std::env::var("BMO_PNG_MUESTRA").is_ok() {
                let mut crudo_buf = vec![0u8; crudo(m.ancho, m.alto)];
                let mut taller = vec![0u32; TALLER];
                let mut dst = vec![0u8; cota(m.ancho, m.alto)];
                let mut f = |x: u32, y: u32| px[(y * w + x) as usize] & 0x00FF_FFFF;
                let k = codificar(m.ancho, m.alto, &mut f, &mut crudo_buf, &mut taller, &mut dst).unwrap();
                let ruta = std::format!("{}/bmo_png_{}.png", std::env::var("BMO_PNG_MUESTRA").unwrap(), &nombre[..2]);
                std::fs::write(&ruta, &dst[..k]).unwrap();
                std::println!("  escrito {ruta}");
            }
        }
    }
}
