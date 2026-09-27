//! **Los ficheros de un `.exe`: rutas de Windows y un fichero abierto**
//! (P4d, 27-09).
//!
//! Un juego carga sus datos con `CreateFileW` + `ReadFile` + `SetFilePointerEx`.
//! La casa los sirve PAGANDO UNA VEZ: al abrir, el fichero entero viene a la
//! memoria (en BMO-X, `Archivo::leer_de` + `leer_en`: un bloque, un viaje), y
//! cada `ReadFile` es una copia; al cerrar uno que se escribio, sale entero.
//!
//! Aqui va lo que se dice sin punteros:
//!
//! ```text
//!    la ruta    "C:\\juego\\datos\\a.pak" o "datos\\a.pak" -> "juego/datos/a.pak":
//!               la unidad se quita (el volumen de BMO-X es uno), las barras
//!               se enderezan, "." y ".." se resuelven -- y un ".." que saldria
//!               del volumen se RECHAZA, no se recorta
//!    relativa   desde el directorio del `.exe` (su "directorio actual")
//!    abierto    los bytes, la posicion, si se escribio, y SetFilePointerEx
//!               con los tres metodos de Windows
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// Por que una ruta de Windows no es una ruta de BMO-X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoRuta {
    /// Un caracter que no es ASCII (el volumen de BMO-X es FAT32 8.3/ASCII).
    NoAscii,
    /// Un `..` que saldria del volumen.
    FueraDelVolumen,
    /// Vacia, o un dispositivo (`\\.\`, `CON`...).
    NoEsFichero,
}

/// **Una ruta de Windows (UTF-16) como ruta de BMO-X**, relativa a la raiz
/// del volumen. `dir` es el directorio actual (el del `.exe`).
pub fn ruta(w: &[u16], dir: &str) -> Result<String, NoRuta> {
    let mut s = String::with_capacity(w.len());
    for &c in w.iter().take_while(|&&c| c != 0) {
        if c >= 0x80 {
            return Err(NoRuta::NoAscii);
        }
        s.push(if c == b'\\' as u16 { '/' } else { c as u8 as char });
    }
    let mut t = s.as_str();
    if let Some(r) = t.strip_prefix("//?/") {
        t = r;
    }
    if t.starts_with("//") || t.is_empty() {
        return Err(NoRuta::NoEsFichero);
    }
    // "X:/..." o "/...": desde la raiz del volumen. Lo demas, desde `dir`.
    let (absoluta, resto) = match t.as_bytes() {
        [l, b':', rest @ ..] if l.is_ascii_alphabetic() => (true, core::str::from_utf8(rest).unwrap_or("")),
        [b'/', ..] => (true, t),
        _ => (false, t),
    };
    let mut partes: Vec<&str> = if absoluta { Vec::new() } else { dir.split('/').filter(|p| !p.is_empty()).collect() };
    for p in resto.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                if partes.pop().is_none() {
                    return Err(NoRuta::FueraDelVolumen);
                }
            }
            x => partes.push(x),
        }
    }
    if partes.is_empty() {
        return Err(NoRuta::NoEsFichero);
    }
    Ok(partes.join("/"))
}

/// `FILE_BEGIN`, `FILE_CURRENT`, `FILE_END`.
pub const DESDE_INICIO: u32 = 0;
pub const DESDE_AQUI: u32 = 1;
pub const DESDE_FIN: u32 = 2;

/// **Un fichero abierto**, entero en memoria.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Abierto {
    pub ruta: String,
    pub bytes: Vec<u8>,
    pub pos: u64,
    pub lee: bool,
    pub escribe: bool,
    /// Se escribio algo: al cerrar, sale entero.
    pub sucio: bool,
}

impl Abierto {
    /// `ReadFile`: hasta `dst.len()` bytes desde la posicion; 0 al final (que
    /// en Windows es EXITO con 0 leidos, no un error).
    pub fn leer(&mut self, dst: &mut [u8]) -> usize {
        let desde = (self.pos as usize).min(self.bytes.len());
        let n = dst.len().min(self.bytes.len() - desde);
        dst[..n].copy_from_slice(&self.bytes[desde..desde + n]);
        self.pos += n as u64;
        n
    }

    /// `WriteFile`: en la posicion; mas alla del final, el hueco va a ceros
    /// (como en Windows).
    pub fn escribir(&mut self, src: &[u8]) -> usize {
        let desde = self.pos as usize;
        if self.bytes.len() < desde + src.len() {
            self.bytes.resize(desde + src.len(), 0);
        }
        self.bytes[desde..desde + src.len()].copy_from_slice(src);
        self.pos += src.len() as u64;
        self.sucio = true;
        src.len()
    }

    /// `SetFilePointerEx`: la posicion nueva, o `None` si quedaria antes del
    /// principio o el metodo no existe (ERROR_NEGATIVE_SEEK /
    /// ERROR_INVALID_PARAMETER). Pasado el final se puede: Windows deja.
    pub fn mover(&mut self, dist: i64, metodo: u32) -> Option<u64> {
        let base = match metodo {
            DESDE_INICIO => 0i128,
            DESDE_AQUI => self.pos as i128,
            DESDE_FIN => self.bytes.len() as i128,
            _ => return None,
        };
        let nueva = base + dist as i128;
        if nueva < 0 {
            return None;
        }
        self.pos = nueva as u64;
        Some(self.pos)
    }
}
