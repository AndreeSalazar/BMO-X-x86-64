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
//!
//! ## ** EL CODIFICADOR SE PUEDE PAUSAR (2026-09-23)
//!
//! La captura congelaba el escritorio 1.068 ms en el Ryzen. [`Codificador`] lo
//! parte en tres fases, cada una con presupuesto:
//!
//! ```text
//!    0. la FOTO      la pone quien llama: las filas RGB en su sitio de
//!                    `crudo_buf` ([`fila`]), de una vez -- la pantalla cambia
//!    1. FILTRAR      de ABAJO a ARRIBA y EN EL SITIO: la fila de arriba sigue
//!                    cruda cuando se filtra la de abajo, y dentro de la fila se
//!                    va de derecha a izquierda. Sin bufer de filas
//!    2. COMPRIMIR    `deflar::Deflar::paso`, el mismo flujo que de una vez
//!    3. CERRAR       el CRC del IDAT, tambien a trozos, e IEND
//! ```
//!
//! El filtro mide los cuatro candidatos en UNA pasada (antes eran cuatro, byte
//! a byte y empaquetado en `u32`: 120 ms de los 266 en el anfitrion).

use crate::{deflar, lados, Error};

/// La fila mas ancha, en bytes RGB.
const FILA_MAX: usize = crate::LADO_MAX as usize * 3;

/// La fila de arriba de la primera: ceros, como dice el estandar.
static CEROS: [u8; FILA_MAX] = [0; FILA_MAX];

/// Taller que pide [`codificar`], en `u32`: el de `deflar`. Las filas se
/// filtran en el sitio y ya no piden taller.
pub const TALLER: usize = deflar::TALLER;

/// **Donde va la fila `y` CRUDA** (RGB, 3 bytes por pixel) dentro de
/// `crudo_buf`. Quien llama pone aqui la foto antes de [`Codificador::avanzar`].
pub fn fila(crudo_buf: &mut [u8], ancho: u32, y: u32) -> &mut [u8] {
    let fw = ancho as usize * 3;
    let base = y as usize * (1 + fw) + 1;
    &mut crudo_buf[base..base + fw]
}

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
    crc_sigue(0xFFFF_FFFF, b) ^ 0xFFFF_FFFF
}

/// El CRC sin la ultima vuelta de bits: para llevarlo a trozos.
fn crc_sigue(mut c: u32, b: &[u8]) -> u32 {
    for &x in b {
        c = CRC_TABLA[((c ^ x as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c
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

/// **Escribe un PNG** de `ancho` x `alto` en `dst`, de una vez, y devuelve
/// cuantos bytes ocupa. `pixel(x, y)` da `0x00RRGGBB`. `crudo_buf` mide al
/// menos [`crudo`]; `taller` al menos [`TALLER`]; `dst` al menos [`cota`] (con
/// menos puede caber, y si no cabe es `NoCabe`: nunca un PNG a medias).
pub fn codificar(
    ancho: u32,
    alto: u32,
    pixel: &mut dyn FnMut(u32, u32) -> u32,
    crudo_buf: &mut [u8],
    taller: &mut [u32],
    dst: &mut [u8],
) -> Result<usize, Error> {
    let mut c = Codificador::nuevo(ancho, alto, crudo_buf, taller, dst)?;
    for y in 0..alto {
        let f = fila(crudo_buf, ancho, y);
        for x in 0..ancho as usize {
            let v = pixel(x as u32, y);
            f[x * 3] = (v >> 16) as u8;
            f[x * 3 + 1] = (v >> 8) as u8;
            f[x * 3 + 2] = v as u8;
        }
    }
    loop {
        if let Estado::Hecho(n) = c.avanzar(crudo_buf, taller, dst, usize::MAX)? {
            return Ok(n);
        }
    }
}

/// En que va un [`Codificador`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Estado {
    /// Queda. `por_mil` es cuanto va, para pintarlo.
    Sigue { por_mil: u32 },
    /// Entero: el PNG son los primeros `n` bytes de `dst`.
    Hecho(usize),
}

/// **Un PNG a medias.** Los bufers no viven aqui: se le pasan en cada
/// [`Codificador::avanzar`] y tienen que ser LOS MISMOS de principio a fin.
pub struct Codificador {
    ancho: u32,
    alto: u32,
    /// 1 filtrar, 2 comprimir, 3 cerrar.
    fase: u8,
    /// Filas que QUEDAN por filtrar (se va de abajo a arriba).
    quedan: u32,
    z: deflar::Deflar,
    /// Donde empieza el IDAT, y los bytes del flujo zlib cuando acaba.
    idat: usize,
    zlib: usize,
    /// El CRC del IDAT, llevado a trozos, y por donde va.
    crc: u32,
    crc_hecho: usize,
}

impl Codificador {
    /// **Empieza**: comprueba los bufers y escribe la firma y el IHDR. La foto
    /// tiene que estar en `crudo_buf` (ver [`fila`]) antes de avanzar.
    pub fn nuevo(ancho: u32, alto: u32, crudo_buf: &[u8], taller: &[u32], dst: &mut [u8]) -> Result<Self, Error> {
        lados(ancho, alto)?;
        if crudo_buf.len() < crudo(ancho, alto) || dst.len() < 8 + 25 + 12 + 12 + 8 {
            return Err(Error::NoCabe);
        }
        if taller.len() < TALLER {
            return Err(Error::SinTaller);
        }
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
        Ok(Self {
            ancho,
            alto,
            fase: 1,
            quedan: alto,
            z: deflar::Deflar::nuevo(),
            idat: n,
            zlib: 0,
            crc: 0xFFFF_FFFF,
            crc_hecho: 0,
        })
    }

    /// **Avanza unos `presupuesto` bytes de trabajo** y vuelve.
    pub fn avanzar(&mut self, crudo_buf: &mut [u8], taller: &mut [u32], dst: &mut [u8], presupuesto: usize) -> Result<Estado, Error> {
        let n_crudo = crudo(self.ancho, self.alto);
        let fw = self.ancho as usize * 3;
        let mut resta = presupuesto;
        if self.fase == 1 {
            while self.quedan > 0 && resta > 0 {
                let y = self.quedan as usize - 1;
                filtrar_fila(crudo_buf, y, fw);
                self.quedan -= 1;
                resta = resta.saturating_sub(fw);
            }
            if self.quedan > 0 {
                return Ok(self.sigue(n_crudo));
            }
            self.fase = 2;
        }
        let desde = self.idat + 8;
        let fin = dst.len().saturating_sub(12 + 4);
        if fin <= desde {
            return Err(Error::NoCabe);
        }
        if self.fase == 2 {
            if resta == 0 {
                return Ok(self.sigue(n_crudo));
            }
            match self.z.paso(&crudo_buf[..n_crudo], taller, &mut dst[desde..fin], resta)? {
                None => return Ok(self.sigue(n_crudo)),
                Some(z) => {
                    self.zlib = z;
                    dst[self.idat..self.idat + 4].copy_from_slice(&(z as u32).to_be_bytes());
                    dst[self.idat + 4..self.idat + 8].copy_from_slice(b"IDAT");
                    self.crc_hecho = self.idat + 4;
                    self.fase = 3;
                    resta = resta.saturating_sub(n_crudo);
                }
            }
        }
        // -- 3. El CRC del IDAT (tipo + datos), a trozos, e IEND --
        let hasta_crc = desde + self.zlib;
        let k = (hasta_crc - self.crc_hecho).min(resta.max(64 * 1024));
        self.crc = crc_sigue(self.crc, &dst[self.crc_hecho..self.crc_hecho + k]);
        self.crc_hecho += k;
        if self.crc_hecho < hasta_crc {
            return Ok(self.sigue(n_crudo));
        }
        let mut n = hasta_crc;
        let pon = |d: &mut [u8], b: &[u8], n: &mut usize| {
            d[*n..*n + b.len()].copy_from_slice(b);
            *n += b.len();
        };
        pon(dst, &(self.crc ^ 0xFFFF_FFFF).to_be_bytes(), &mut n);
        pon(dst, &0u32.to_be_bytes(), &mut n);
        pon(dst, b"IEND", &mut n);
        pon(dst, &crc(b"IEND").to_be_bytes(), &mut n);
        Ok(Estado::Hecho(n))
    }

    /// Cuanto va, por mil: filtrar pesa 300, comprimir 650 y cerrar 50.
    fn sigue(&self, n_crudo: usize) -> Estado {
        let alto = self.alto.max(1) as u64;
        let por_mil = match self.fase {
            1 => (alto - self.quedan as u64) * 300 / alto,
            2 => 300 + self.z.hecho() as u64 * 650 / n_crudo.max(1) as u64,
            _ => 950 + (self.crc_hecho as u64 * 50 / (self.idat + 8 + self.zlib).max(1) as u64),
        };
        Estado::Sigue { por_mil: por_mil.min(999) as u32 }
    }
}

/// **Filtra la fila `y` EN SU SITIO** con el que menos suma de los cuatro.
///
/// La fila de arriba (`y - 1`) tiene que seguir CRUDA: por eso se va de abajo
/// a arriba. Y dentro de la fila se escribe de derecha a izquierda, para que el
/// vecino de la izquierda (`a`) siga crudo cuando se le necesita.
fn filtrar_fila(buf: &mut [u8], y: usize, fw: usize) {
    let base = y * (1 + fw);
    let (antes, ahora) = buf.split_at_mut(base);
    let arriba: &[u8] = if y == 0 { &CEROS[..fw] } else { &antes[base - fw..base] };
    let cur = &mut ahora[1..1 + fw];
    // -- Los cuatro costes en UNA pasada (Average no: casi nunca gana en una
    // pantalla y cuesta otra pasada) --
    let (mut s0, mut s1, mut s2, mut s4) = (0u32, 0u32, 0u32, 0u32);
    let abs = |r: u8| (r as i8).unsigned_abs() as u32;
    for i in 0..fw.min(3) {
        let (x, b) = (cur[i], arriba[i]);
        s0 += abs(x);
        s1 += abs(x);
        s2 += abs(x.wrapping_sub(b));
        s4 += abs(x.wrapping_sub(paeth(0, b, 0)));
    }
    for i in 3..fw {
        let (x, a, b, c) = (cur[i], cur[i - 3], arriba[i], arriba[i - 3]);
        s0 += abs(x);
        s1 += abs(x.wrapping_sub(a));
        s2 += abs(x.wrapping_sub(b));
        s4 += abs(x.wrapping_sub(paeth(a, b, c)));
    }
    // El primero que menos suma, en el orden de siempre: ninguno, Sub, Up, Paeth.
    let mut mejor = (0u8, s0);
    for (f, s) in [(1u8, s1), (2, s2), (4, s4)] {
        if s < mejor.1 {
            mejor = (f, s);
        }
    }
    let f = mejor.0;
    // -- Y se escribe, de derecha a izquierda --
    match f {
        0 => {}
        1 => {
            for i in (3..fw).rev() {
                cur[i] = cur[i].wrapping_sub(cur[i - 3]);
            }
        }
        2 => {
            for i in 0..fw {
                cur[i] = cur[i].wrapping_sub(arriba[i]);
            }
        }
        _ => {
            for i in (3..fw).rev() {
                cur[i] = cur[i].wrapping_sub(paeth(cur[i - 3], arriba[i], arriba[i - 3]));
            }
            for i in 0..fw.min(3) {
                cur[i] = cur[i].wrapping_sub(paeth(0, arriba[i], 0));
            }
        }
    }
    ahora[0] = f;
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

    /// ** PAUSAR NO CAMBIA EL FICHERO: el PNG a trozos es el mismo, byte a byte,
    /// que el de una vez, para presupuestos chicos y raros.
    #[test]
    fn a_trozos_da_el_mismo_png() {
        let (w, h) = (333u32, 77u32);
        let f = |x: u32, y: u32| (x * 7 + y * 13) & 0xFF | ((x ^ y) & 0xFF) << 8 | ((x / 9) << 16);
        let mut crudo_buf = vec![0u8; crudo(w, h)];
        let mut taller = vec![0u32; TALLER];
        let mut dst = vec![0u8; cota(w, h)];
        let mut px = |x: u32, y: u32| f(x, y) & 0x00FF_FFFF;
        let n1 = codificar(w, h, &mut px, &mut crudo_buf, &mut taller, &mut dst).unwrap();
        let de_una = dst[..n1].to_vec();
        for presupuesto in [1usize, 500, 4096, 100_000] {
            let mut crudo_buf = vec![0u8; crudo(w, h)];
            let mut taller = vec![0u32; TALLER];
            let mut dst = vec![0u8; cota(w, h)];
            for y in 0..h {
                let fl = fila(&mut crudo_buf, w, y);
                for x in 0..w as usize {
                    let v = f(x as u32, y);
                    fl[x * 3] = (v >> 16) as u8;
                    fl[x * 3 + 1] = (v >> 8) as u8;
                    fl[x * 3 + 2] = v as u8;
                }
            }
            let mut c = Codificador::nuevo(w, h, &crudo_buf, &taller, &mut dst).unwrap();
            let (mut vueltas, mut ultimo) = (0, 0u32);
            let n2 = loop {
                vueltas += 1;
                match c.avanzar(&mut crudo_buf, &mut taller, &mut dst, presupuesto).unwrap() {
                    Estado::Hecho(n) => break n,
                    Estado::Sigue { por_mil } => {
                        assert!(por_mil >= ultimo, "el progreso no puede ir hacia atras");
                        ultimo = por_mil;
                    }
                }
            };
            assert!(vueltas > 1, "con {presupuesto} no se partio");
            assert_eq!(&dst[..n2], &de_una[..], "a trozos de {presupuesto} sale otro PNG");
        }
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
