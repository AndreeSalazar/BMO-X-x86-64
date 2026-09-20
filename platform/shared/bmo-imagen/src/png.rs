//! **PNG** -- el formato que trae todo lo que no es una foto: capturas,
//! iconos, lo que exporta cualquier programa. Eddi (20-09): *"visualizar
//! imagenes por completo"*.
//!
//! generacion: nieto -- recibe bytes y un bufer, y devuelve medidas o un
//! motivo; no sabe donde se pinta.
//!
//! ## Lo que se lee, y lo que se rechaza con nombre
//!
//! ```text
//!    profundidad 8, tipos 0 (gris), 2 (RGB), 3 (paleta), 4 (gris+alfa), 6 (RGBA)
//!    filtros 0..4 (ninguno, sub, up, average, paeth)
//!    tRNS: la paleta con alfa, o la clave de color en gris/RGB
//!    varios IDAT, sin pegarlos (el inflate los recorre)
//!
//!    Variante:  profundidad 1/2/4/16, entrelazado Adam7, otro filtro/compresion
//! ```
//!
//! Profundidad 16 y Adam7 se dejan fuera a proposito: lo primero no lo produce
//! casi nadie y lo segundo pide siete pasadas y un bufer entero; los dos son
//! un `Variante` con su frase, no una imagen a medias.
//!
//! ## Como cae en el bufer sin guardar la imagen cruda
//!
//! Una imagen cruda de PNG mide `alto * (1 + ancho * bytes_por_pixel)`: para
//! 1024x1024 RGBA son 4 MiB y un byte por fila mas que el bufer de pixeles. En
//! vez de guardarla, los bytes que salen del inflate se van juntando FILA A
//! FILA en dos bufers del taller (la fila anterior y la actual, 4 KiB cada
//! uno): cuando una fila esta completa se desfiltra contra la anterior y se
//! escribe ya como pixeles en `dst`. Asi el taller mide 41 KiB fijos, sea la
//! imagen del tamano que sea.
//!
//! ## Lo que un fichero hostil no consigue
//!
//! Toda medida se compara con lo que llego antes de usarla (`chunk` que se
//! sale, IHDR corto, paleta mas corta que el indice, mas bytes de los que
//! prometen las medidas). El inflate ya rechaza distancias fuera de la ventana.
//! Y el CRC de cada chunk NO se comprueba: una imagen que llega entera y con
//! sus medidas bien se juzga por lo que pinta, y comprobar el CRC cuesta
//! recorrer el fichero dos veces para descubrir lo mismo que ya se descubre
//! al pintarla.

use crate::inflate::{self, Fallo};
use crate::{be32, lados, Error, Formato, Medidas, LADO_MAX, OPACO};

/// La fila mas larga: un byte de filtro y cuatro por pixel.
const FILA_MAX: usize = 1 + LADO_MAX as usize * 4;
/// Bytes de taller que pide un PNG: la ventana del inflate y dos filas.
pub const TALLER: usize = inflate::VENTANA + 2 * FILA_MAX;
/// IDAT como mucho: 64 es lo que admite el inflate por trozos.
const IDAT_MAX: usize = 64;

const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Que es y cuanto mide, del IHDR. Los tipos y profundidades que no se
/// decodifican se dicen aqui, antes de pedir un bufer.
pub fn medir(b: &[u8]) -> Result<Medidas, Error> {
    if b.len() < 8 || b[..8] != FIRMA {
        return Err(Error::NoEsImagen);
    }
    // El IHDR es el primer chunk, siempre: 13 bytes.
    if b.len() < 8 + 8 + 13 + 4 {
        return Err(Error::Corto);
    }
    if &b[12..16] != b"IHDR" || be32(b, 8) != Some(13) {
        return Err(Error::Variante);
    }
    let w = be32(b, 16).ok_or(Error::Corto)?;
    let h = be32(b, 20).ok_or(Error::Corto)?;
    lados(w, h)?;
    let (profundidad, tipo, compresion, filtro, entrelazado) = (b[24], b[25], b[26], b[27], b[28]);
    if profundidad != 8 || compresion != 0 || filtro != 0 || entrelazado != 0 {
        return Err(Error::Variante);
    }
    if !matches!(tipo, 0 | 2 | 3 | 4 | 6) {
        return Err(Error::Variante);
    }
    Ok(Medidas { ancho: w, alto: h, formato: Formato::Png })
}

fn bytes_por_pixel(tipo: u8) -> usize {
    match tipo {
        0 | 3 => 1,
        4 => 2,
        2 => 3,
        _ => 4,
    }
}

/// Lo que hace falta para convertir una fila desfiltrada en pixeles.
struct Paleta<'a> {
    tipo: u8,
    plte: &'a [u8],
    trns: &'a [u8],
}

impl Paleta<'_> {
    fn pixel(&self, f: &[u8], x: usize) -> Result<u32, Error> {
        Ok(match self.tipo {
            0 => {
                let g = f[x] as u32;
                let alfa = if self.trns.len() >= 2 && self.trns[1] as u32 == g && self.trns[0] == 0 { 0 } else { OPACO };
                alfa | (g << 16) | (g << 8) | g
            }
            2 => {
                let (r, g, b) = (f[x * 3] as u32, f[x * 3 + 1] as u32, f[x * 3 + 2] as u32);
                let clave = self.trns.len() >= 6
                    && self.trns[1] as u32 == r
                    && self.trns[3] as u32 == g
                    && self.trns[5] as u32 == b
                    && self.trns[0] == 0
                    && self.trns[2] == 0
                    && self.trns[4] == 0;
                (if clave { 0 } else { OPACO }) | (r << 16) | (g << 8) | b
            }
            3 => {
                let i = f[x] as usize;
                // *** EL INDICE CONTRA LA PALETA: un indice fuera es la
                // lectura de mas clasica de los visores. Aqui es `Corto`.
                if i * 3 + 2 >= self.plte.len() {
                    return Err(Error::Corto);
                }
                let (r, g, b) = (self.plte[i * 3] as u32, self.plte[i * 3 + 1] as u32, self.plte[i * 3 + 2] as u32);
                let alfa = match self.trns.get(i) {
                    Some(&a) => crate::bit_de_alfa(a as u32),
                    None => OPACO,
                };
                alfa | (r << 16) | (g << 8) | b
            }
            4 => {
                let (g, a) = (f[x * 2] as u32, f[x * 2 + 1] as u32);
                crate::bit_de_alfa(a) | (g << 16) | (g << 8) | g
            }
            _ => {
                let (r, g, b, a) = (f[x * 4] as u32, f[x * 4 + 1] as u32, f[x * 4 + 2] as u32, f[x * 4 + 3] as u32);
                crate::bit_de_alfa(a) | (r << 16) | (g << 8) | b
            }
        })
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Desfiltra `fila` (byte de filtro delante) contra `previa`, en sitio.
fn desfiltra(fila: &mut [u8], previa: &[u8], bpp: usize) -> Result<(), Error> {
    let filtro = fila[0];
    let n = fila.len() - 1;
    for i in 0..n {
        let a = if i >= bpp { fila[1 + i - bpp] } else { 0 };
        let b = previa[1 + i];
        let c = if i >= bpp { previa[1 + i - bpp] } else { 0 };
        let x = fila[1 + i];
        fila[1 + i] = match filtro {
            0 => x,
            1 => x.wrapping_add(a),
            2 => x.wrapping_add(b),
            3 => x.wrapping_add(((a as u16 + b as u16) / 2) as u8),
            4 => x.wrapping_add(paeth(a, b, c)),
            _ => return Err(Error::Variante),
        };
    }
    Ok(())
}

/// El que junta los bytes del inflate en filas y las va pintando.
struct Filas<'a> {
    ancho: usize,
    alto: usize,
    bpp: usize,
    fila: usize,
    pos: usize,
    actual: &'a mut [u8],
    previa: &'a mut [u8],
    dst: &'a mut [u32],
    paleta: Paleta<'a>,
    fallo: Option<Error>,
}

impl Filas<'_> {
    fn byte(&mut self, b: u8) -> Result<(), Fallo> {
        let largo = 1 + self.ancho * self.bpp;
        if self.fila >= self.alto {
            // Mas bytes de los que las medidas prometen: se ignoran. Un
            // flujo con cola no es una imagen distinta.
            return Ok(());
        }
        self.actual[self.pos] = b;
        self.pos += 1;
        if self.pos == largo {
            if let Err(e) = desfiltra(&mut self.actual[..largo], &self.previa[..largo], self.bpp) {
                self.fallo = Some(e);
                return Err(Fallo::NoCabe);
            }
            let y = self.fila;
            for x in 0..self.ancho {
                match self.paleta.pixel(&self.actual[1..largo], x) {
                    Ok(p) => self.dst[y * self.ancho + x] = p,
                    Err(e) => {
                        self.fallo = Some(e);
                        return Err(Fallo::NoCabe);
                    }
                }
            }
            core::mem::swap(&mut self.actual, &mut self.previa);
            self.fila += 1;
            self.pos = 0;
        }
        Ok(())
    }
}

/// **Decodifica un PNG ya medido en `dst`**, con un `taller` de al menos
/// [`TALLER`] bytes.
pub fn decodificar(b: &[u8], m: &Medidas, dst: &mut [u32], taller: &mut [u8]) -> Result<(), Error> {
    if taller.len() < TALLER {
        return Err(Error::SinTaller);
    }
    let tipo = b[25];
    let bpp = bytes_por_pixel(tipo);

    // -- Los chunks: donde esta cada cosa, sin creerse ningun tamano ------
    let mut idat: [&[u8]; IDAT_MAX] = [&[]; IDAT_MAX];
    let mut cuantos = 0usize;
    let mut plte: &[u8] = &[];
    let mut trns: &[u8] = &[];
    let mut o = 8usize;
    let mut fin = false;
    while !fin {
        let len = be32(b, o).ok_or(Error::Corto)? as usize;
        let tipo_chunk = b.get(o + 4..o + 8).ok_or(Error::Corto)?;
        let datos = b.get(o + 8..o + 8 + len).ok_or(Error::Corto)?;
        match tipo_chunk {
            b"IDAT" => {
                if cuantos == IDAT_MAX {
                    return Err(Error::Variante);
                }
                idat[cuantos] = datos;
                cuantos += 1;
            }
            b"PLTE" => plte = datos,
            b"tRNS" => trns = datos,
            b"IEND" => fin = true,
            _ => {}
        }
        // + 4 del CRC, que no se comprueba (ver la cabecera).
        o = o.checked_add(12 + len).ok_or(Error::Corto)?;
        if o > b.len() {
            return Err(Error::Corto);
        }
    }
    if cuantos == 0 {
        return Err(Error::Corto);
    }
    if tipo == 3 && plte.len() < 3 {
        return Err(Error::Corto);
    }

    // -- El taller: la ventana del inflate y las dos filas -----------------
    let (ventana, filas) = taller.split_at_mut(inflate::VENTANA);
    let (actual, previa) = filas.split_at_mut(FILA_MAX);
    for x in previa.iter_mut() {
        *x = 0;
    }
    let (ancho, alto) = (m.ancho as usize, m.alto as usize);
    let mut f = Filas {
        ancho,
        alto,
        bpp,
        fila: 0,
        pos: 0,
        actual: &mut actual[..FILA_MAX],
        previa: &mut previa[..FILA_MAX],
        dst: &mut dst[..ancho * alto],
        paleta: Paleta { tipo, plte, trns },
        fallo: None,
    };
    let r = inflate::inflar_zlib(&idat[..cuantos], ventana, &mut |byte| f.byte(byte));
    if let Some(e) = f.fallo {
        return Err(e);
    }
    match r {
        Ok(_) => {}
        Err(Fallo::Corto) => return Err(Error::Corto),
        Err(Fallo::Malformado) => return Err(Error::Variante),
        Err(Fallo::NoCabe) => return Err(Error::NoCabe),
    }
    // Menos filas de las prometidas: la imagen esta cortada.
    if f.fila < alto {
        return Err(Error::Corto);
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Escritos por Pillow en el anfitrion (`pruebas/*.png`): la prueba es
    // contra OTRO escritor, no contra la idea que este fichero tiene del
    // formato. Los 15 pixeles de `rgba.png` son
    // `(x*40+10, y*80+5, (x*y*30)%256, 255 si (x+y)%3 != 0 sino 0)`.
    const RGBA: &[u8] = include_bytes!("../pruebas/rgba.png");
    const RGB: &[u8] = include_bytes!("../pruebas/rgb.png");
    const GRIS: &[u8] = include_bytes!("../pruebas/gris.png");
    const GRIS_ALFA: &[u8] = include_bytes!("../pruebas/gris_alfa.png");
    const PALETA: &[u8] = include_bytes!("../pruebas/paleta.png");
    const PALETA_TRNS: &[u8] = include_bytes!("../pruebas/paleta_trns.png");
    // `rgba.png` con el byte de entrelazado a 1 (Pillow no escribe Adam7).
    const ENTRELAZADO: &[u8] = include_bytes!("../pruebas/entrelazado.png");
    const GRIS16: &[u8] = include_bytes!("../pruebas/gris16.png");
    const GRANDE: &[u8] = include_bytes!("../pruebas/grande.png");

    fn esperado(x: u32, y: u32) -> (u32, u32, u32, u32) {
        (x * 40 + 10, y * 80 + 5, (x * y * 30) % 256, if (x + y) % 3 != 0 { 255 } else { 0 })
    }

    fn dec(b: &[u8]) -> Result<(Medidas, Vec<u32>), Error> {
        let mut dst = vec![0u32; 300 * 200];
        let mut taller = vec![0u8; TALLER];
        let m = crate::decodificar_con(b, &mut dst, &mut taller)?;
        dst.truncate((m.ancho * m.alto) as usize);
        Ok((m, dst))
    }

    #[test]
    fn rgba_con_su_alfa_de_un_bit() {
        let (m, px) = dec(RGBA).unwrap();
        assert_eq!((m.ancho, m.alto, m.formato), (5, 3, Formato::Png));
        for y in 0..3 {
            for x in 0..5 {
                let (r, g, b, a) = esperado(x, y);
                let alfa = if a >= 128 { OPACO } else { 0 };
                assert_eq!(px[(y * 5 + x) as usize], alfa | (r << 16) | (g << 8) | b, "({x},{y})");
            }
        }
    }

    #[test]
    fn rgb_sale_opaco() {
        let (m, px) = dec(RGB).unwrap();
        assert_eq!((m.ancho, m.alto), (5, 3));
        for y in 0..3 {
            for x in 0..5 {
                let (r, g, b, _) = esperado(x, y);
                assert_eq!(px[(y * 5 + x) as usize], OPACO | (r << 16) | (g << 8) | b);
            }
        }
    }

    #[test]
    fn gris_y_gris_con_alfa() {
        let (_, px) = dec(GRIS).unwrap();
        assert_eq!(px.len(), 15);
        for p in &px {
            let (r, g, b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            assert!(r == g && g == b, "un gris tiene los tres canales iguales: {p:#x}");
            assert_eq!(p & OPACO, OPACO);
        }
        let (_, px) = dec(GRIS_ALFA).unwrap();
        // El alfa viene de la imagen: (0,0) y (1,2) son transparentes.
        assert_eq!(px[0] & OPACO, 0);
        assert_eq!(px[1] & OPACO, OPACO);
        assert_eq!(px[2 * 5 + 1] & OPACO, 0);
    }

    #[test]
    fn paleta_con_y_sin_transparencia() {
        let (m, px) = dec(PALETA).unwrap();
        assert_eq!((m.ancho, m.alto), (5, 3));
        assert!(px.iter().all(|p| p & OPACO == OPACO));
        // Los colores son los de la imagen (Pillow los cuantiza a 8 con
        // error): el pixel (4,2) es mas rojo y mas verde que el (0,0).
        let (p, q) = (px[2 * 5 + 4], px[0]);
        assert!((p >> 16) & 0xFF > (q >> 16) & 0xFF && (p >> 8) & 0xFF > (q >> 8) & 0xFF, "{p:#x} {q:#x}");
        let (_, px) = dec(PALETA_TRNS).unwrap();
        // La entrada 0 de la paleta es transparente: al menos un pixel lo es.
        assert!(px.iter().any(|p| p & OPACO == 0));
        assert!(px.iter().any(|p| p & OPACO == OPACO));
    }

    #[test]
    fn una_imagen_grande_con_codigos_dinamicos_y_filtros() {
        let (m, px) = dec(GRANDE).unwrap();
        assert_eq!((m.ancho, m.alto), (300, 200));
        for &(x, y) in &[(0u32, 0u32), (299, 0), (0, 199), (299, 199), (150, 100), (7, 133)] {
            let e = OPACO | (((x * 255) / 299) << 16) | (((y * 255) / 199) << 8) | ((x + y) % 256);
            assert_eq!(px[(y * 300 + x) as usize], e, "({x},{y})");
        }
    }

    /// El logo de INTI (`docs/arte/inti.png`) a 256 de lado: RGBA con alfa de
    /// verdad, 92 KB de IDAT en varios chunks y todos los filtros.
    #[test]
    fn el_logo_de_inti() {
        let mut dst = vec![0u32; 256 * 256];
        let mut taller = vec![0u8; TALLER];
        let m = crate::decodificar_con(include_bytes!("../pruebas/inti256.png"), &mut dst, &mut taller).unwrap();
        assert_eq!((m.ancho, m.alto), (256, 256));
        let opacos = dst.iter().filter(|p| *p & OPACO == OPACO).count();
        assert!(opacos > 1000 && opacos < 256 * 256, "tiene fondo transparente y figura: {opacos}");
    }

    #[test]
    fn lo_que_no_se_lee_se_dice_con_nombre() {
        assert_eq!(dec(ENTRELAZADO).err(), Some(Error::Variante));
        assert_eq!(dec(GRIS16).err(), Some(Error::Variante));
        assert_eq!(crate::medir(ENTRELAZADO).err(), Some(Error::Variante));
    }

    #[test]
    fn sin_taller_no_se_decodifica_un_png_pero_si_se_mide() {
        let mut dst = vec![0u32; 64];
        assert_eq!(crate::decodificar(RGBA, &mut dst).err(), Some(Error::SinTaller));
        assert_eq!(crate::medir(RGBA).unwrap().formato, Formato::Png);
    }

    #[test]
    fn un_png_cortado_es_corto() {
        for n in [8, 20, 33, 40, RGBA.len() - 20, RGBA.len() - 1] {
            match dec(&RGBA[..n]) {
                Err(Error::Corto) | Err(Error::Variante) => {}
                otro => panic!("con {n} bytes: {otro:?}"),
            }
        }
    }

    #[test]
    fn un_byte_cambiado_no_revienta() {
        for i in 8..GRANDE.len() {
            let mut b = GRANDE.to_vec();
            b[i] ^= 0x5A;
            let _ = dec(&b);
        }
    }

    #[test]
    fn el_indice_de_paleta_fuera_es_corto() {
        // Se acorta la PLTE a una entrada: todo indice > 0 se sale.
        let mut b = PALETA.to_vec();
        let o = b.windows(4).position(|w| w == b"PLTE").unwrap() - 4;
        let len = u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize;
        // Se sustituye el chunk por uno de 3 bytes: mismo hueco, PLTE de 1
        // entrada y el resto como un chunk desconocido que se salta.
        assert!(len > 3);
        b[o..o + 4].copy_from_slice(&3u32.to_be_bytes());
        let resto = len - 3;
        // Tras los 3 bytes de paleta y su "CRC" (4 bytes de datos viejos), el
        // hueco restante se declara como chunk `zzzz` de (resto - 12) bytes.
        if resto >= 12 {
            let r = o + 8 + 3 + 4;
            b[r..r + 4].copy_from_slice(&((resto - 12) as u32).to_be_bytes());
            b[r + 4..r + 8].copy_from_slice(b"zzzz");
            assert_eq!(dec(&b).err(), Some(Error::Corto));
        }
    }
}
