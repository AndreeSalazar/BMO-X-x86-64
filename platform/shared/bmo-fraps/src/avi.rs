//! **EL VIDEO DE FRAPS-X: MJPEG en AVI** (13-10) -- el contenedor que abren
//! YouTube, VLC, Windows y ffmpeg sin pedir nada. Cada fotograma es un JPEG
//! (`bmo_imagen::jpeg_escribir`); esto los ordena en el fichero.
//!
//! ```text
//!    RIFF 'AVI '
//!      LIST 'hdrl'   avih (el video), LIST 'strl' (strh 'vids' 'MJPG',
//!                    strf: un BITMAPINFOHEADER)
//!      LIST 'movi'   '00dc' un fotograma, '00dc'... (uno VACIO = repetir el
//!                    de antes: lo que no llego a tiempo)
//!    idx1            uno por fotograma, sacado al final recorriendo 'movi'
//! ```
//!
//! Sin memoria dinamica: todo va en el bufer de quien graba. [`CABECERA`]
//! bytes al principio que [`Avi::terminar`] rellena al final (los medidas y
//! cuantos fotogramas: no se saben hasta entonces), y cada fotograma se
//! escribe en su sitio ([`Avi::hueco`]) y se cierra ([`Avi::cerrar`]).
//!
//! [!] El AVI de siempre (sin OpenDML): hasta 1 GiB, lo que leen todos.

/// Lo que mide todo lo de antes del primer fotograma.
pub const CABECERA: usize = 224;
/// Donde empieza el `'movi'` (su codigo): las posiciones del indice van desde aqui.
const MOVI: usize = CABECERA - 4;
/// Lo mas que mide un AVI que leen todos.
pub const MAX_BYTES: usize = 1 << 30;

/// Un video en marcha.
#[derive(Clone, Copy, Debug)]
pub struct Avi {
    pub ancho: u32,
    pub alto: u32,
    pub fps: u32,
    /// Donde va el siguiente fotograma.
    pub n: usize,
    pub fotogramas: u32,
    /// El fotograma mas grande (lo que pide el reproductor de bufer).
    mayor: u32,
}

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoAvi {
    /// No cabe en el bufer (o pasaria de [`MAX_BYTES`]).
    Lleno,
    /// Medidas o fps a cero.
    Medidas,
}

fn pon(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}
fn u32_(b: &mut [u8], o: usize, v: u32) {
    pon(b, o, &v.to_le_bytes());
}

impl Avi {
    /// Empezar un video en `out` (que tiene que medir al menos [`CABECERA`]).
    pub fn nuevo(out: &[u8], ancho: u32, alto: u32, fps: u32) -> Result<Avi, NoAvi> {
        if ancho == 0 || alto == 0 || fps == 0 || ancho > 0xFFFF || alto > 0xFFFF {
            return Err(NoAvi::Medidas);
        }
        if out.len() < CABECERA {
            return Err(NoAvi::Lleno);
        }
        Ok(Avi { ancho, alto, fps, n: CABECERA, fotogramas: 0, mayor: 0 })
    }

    /// Donde escribir el JPEG del siguiente fotograma: `out[hueco..]` (tras
    /// sus 8 bytes de trozo).
    pub fn hueco(&self) -> usize {
        self.n + 8
    }

    /// Cerrar el fotograma de `bytes` (ya escrito en [`Avi::hueco`]; 0 =
    /// repetir el de antes).
    pub fn cerrar(&mut self, out: &mut [u8], bytes: usize) -> Result<(), NoAvi> {
        let fin = self.n + 8 + bytes + (bytes & 1);
        // Lo que aun tiene que caber detras: el indice de todos.
        let indice = 8 + 16 * (self.fotogramas as usize + 1);
        if fin + indice > out.len() || fin + indice > MAX_BYTES {
            return Err(NoAvi::Lleno);
        }
        pon(out, self.n, b"00dc");
        u32_(out, self.n + 4, bytes as u32);
        if bytes & 1 == 1 {
            out[self.n + 8 + bytes] = 0;
        }
        self.n = fin;
        self.fotogramas += 1;
        self.mayor = self.mayor.max(bytes as u32);
        Ok(())
    }

    /// **Terminar**: el indice detras y la cabecera rellena. Devuelve lo que
    /// mide el fichero.
    pub fn terminar(&self, out: &mut [u8]) -> usize {
        let (w, h) = (self.ancho, self.alto);
        // El indice: recorriendo 'movi'.
        let mut i = self.n;
        pon(out, i, b"idx1");
        u32_(out, i + 4, 16 * self.fotogramas);
        i += 8;
        let mut p = CABECERA;
        while p < self.n {
            let mide = u32::from_le_bytes([out[p + 4], out[p + 5], out[p + 6], out[p + 7]]);
            pon(out, i, b"00dc");
            u32_(out, i + 4, 0x10); // AVIIF_KEYFRAME: todos lo son en MJPEG
            u32_(out, i + 8, (p - MOVI) as u32);
            u32_(out, i + 12, mide);
            i += 16;
            p += 8 + mide as usize + (mide as usize & 1);
        }
        let total = i;
        let us = 1_000_000 / self.fps;
        let mayor = self.mayor + 8;
        // RIFF 'AVI '
        pon(out, 0, b"RIFF");
        u32_(out, 4, (total - 8) as u32);
        pon(out, 8, b"AVI ");
        // LIST 'hdrl' (192)
        pon(out, 12, b"LIST");
        u32_(out, 16, 192);
        pon(out, 20, b"hdrl");
        // avih (56)
        pon(out, 24, b"avih");
        u32_(out, 28, 56);
        let avih = [us, mayor * self.fps, 0, 0x10, self.fotogramas, 0, 1, mayor, w, h, 0, 0, 0, 0];
        for (k, v) in avih.iter().enumerate() {
            u32_(out, 32 + 4 * k, *v);
        }
        // LIST 'strl' (116)
        pon(out, 88, b"LIST");
        u32_(out, 92, 116);
        pon(out, 96, b"strl");
        // strh (56)
        pon(out, 100, b"strh");
        u32_(out, 104, 56);
        pon(out, 108, b"vids");
        pon(out, 112, b"MJPG");
        let strh = [0u32, 0, 0, 1, self.fps, 0, self.fotogramas, mayor, u32::MAX, 0];
        for (k, v) in strh.iter().enumerate() {
            u32_(out, 116 + 4 * k, *v);
        }
        // rcFrame: 0, 0, ancho, alto (cuatro i16).
        pon(out, 156, &[0, 0, 0, 0]);
        pon(out, 160, &(w as u16).to_le_bytes());
        pon(out, 162, &(h as u16).to_le_bytes());
        // strf (40): BITMAPINFOHEADER.
        pon(out, 164, b"strf");
        u32_(out, 168, 40);
        u32_(out, 172, 40);
        u32_(out, 176, w);
        u32_(out, 180, h);
        pon(out, 184, &1u16.to_le_bytes());
        pon(out, 186, &24u16.to_le_bytes());
        pon(out, 188, b"MJPG");
        u32_(out, 192, w * h * 3);
        for k in 0..4 {
            u32_(out, 196 + 4 * k, 0);
        }
        // LIST 'movi'
        pon(out, 212, b"LIST");
        u32_(out, 216, (self.n - MOVI) as u32);
        pon(out, 220, b"movi");
        total
    }
}

/// **El reloj del video**: que fotograma toca ahora. Con `fps` fijos, el
/// fotograma `k` es el del instante `k / fps`; si el que graba llega tarde,
/// los que se salto van VACIOS (el reproductor repite): el video dura lo que
/// duro de verdad.
pub const fn toca(desde: u64, ahora: u64, hz: u64, fps: u32) -> u64 {
    if ahora < desde || hz == 0 {
        return 0;
    }
    ((ahora - desde) as u128 * fps as u128 / hz as u128) as u64
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;

    #[test]
    fn la_cabecera_cuadra() {
        // 12 (RIFF) + 8 (LIST) + 192 (hdrl) + 12 (LIST movi) = 224.
        assert_eq!(12 + 8 + 192 + 12, CABECERA);
        assert_eq!(4 + (8 + 56) + (8 + 116), 192);
        assert_eq!(4 + (8 + 56) + (8 + 40), 116);
    }

    #[test]
    fn tres_fotogramas_y_uno_vacio() {
        let mut out = vec![0u8; 4096];
        let mut a = Avi::nuevo(&out, 64, 48, 30).unwrap();
        for (k, n) in [100usize, 0, 33, 50].into_iter().enumerate() {
            let h = a.hueco();
            out[h..h + n].fill(k as u8 + 1);
            a.cerrar(&mut out, n).unwrap();
        }
        let total = a.terminar(&mut out);
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()) as usize, total - 8);
        assert_eq!(u32::from_le_bytes(out[48..52].try_into().unwrap()), 4, "avih: 4 fotogramas");
        // El indice: cuatro entradas, la segunda VACIA, la tercera impar con su relleno.
        let idx = total - 16 * 4;
        let entrada = |k: usize| (u32::from_le_bytes(out[idx + 16 * k + 8..idx + 16 * k + 12].try_into().unwrap()), u32::from_le_bytes(out[idx + 16 * k + 12..idx + 16 * k + 16].try_into().unwrap()));
        assert_eq!(entrada(0), (4, 100));
        assert_eq!(entrada(1), (4 + 108, 0));
        assert_eq!(entrada(2), (4 + 108 + 8, 33));
        assert_eq!(entrada(3), (4 + 108 + 8 + 42, 50));
    }

    #[test]
    fn lleno_y_medidas() {
        let out = vec![0u8; 300];
        let mut o2 = out.clone();
        let mut a = Avi::nuevo(&out, 8, 8, 30).unwrap();
        assert_eq!(a.cerrar(&mut o2, 100), Err(NoAvi::Lleno));
        assert_eq!(Avi::nuevo(&out, 0, 8, 30).unwrap_err(), NoAvi::Medidas);
        assert_eq!(Avi::nuevo(&out[..100], 8, 8, 30).unwrap_err(), NoAvi::Lleno);
    }

    /// Un video de verdad: 60 fotogramas JPEG (una barra que cruza), dos
    /// vacios. `BMO_AVI_DIR=/tmp cargo test` lo deja para ffprobe y VLC.
    #[test]
    fn un_video_de_verdad() {
        let (w, h) = (160u32, 90u32);
        let mut out = vec![0u8; 4 << 20];
        let mut a = Avi::nuevo(&out, w, h, 30).unwrap();
        let mut px = vec![0u32; (w * h) as usize];
        for f in 0..60u32 {
            for (k, p) in px.iter_mut().enumerate() {
                let x = k as u32 % w;
                *p = if x / 8 == f % (w / 8) { 0xFFCC00 } else { 0x102040 + (x << 8 & 0xFF00) };
            }
            let hueco = a.hueco();
            let n = if f == 20 || f == 21 { 0 } else { bmo_imagen::jpeg_escribir::codificar(&px, w, h, w, false, 85, &mut out[hueco..]).unwrap() };
            a.cerrar(&mut out, n).unwrap();
        }
        let total = a.terminar(&mut out);
        assert_eq!(a.fotogramas, 60);
        if let Ok(dir) = std::env::var("BMO_AVI_DIR") {
            std::fs::write(std::format!("{dir}/fraps.avi"), &out[..total]).unwrap();
        }
    }

    #[test]
    fn el_reloj() {
        assert_eq!(toca(1000, 1000, 1000, 30), 0);
        assert_eq!(toca(1000, 2000, 1000, 30), 30);
        assert_eq!(toca(1000, 1500, 1000, 30), 15);
        assert_eq!(toca(1000, 500, 1000, 30), 0);
    }
}
