//! **Los ficheros de un `.exe`: rutas de Windows y un fichero abierto**
//! (P4d, 27-09).
//!
//! Un juego carga sus datos con `CreateFileW` + `ReadFile` + `SetFilePointerEx`.
//! Los ficheros chicos se sirven PAGANDO UNA VEZ: al abrir, el contenido viene
//! a memoria y cada `ReadFile` es una copia. Los grandes se miden al abrir y
//! se leen por rangos bajo demanda, para no reservar varios GiB por `.archive`.
//! Al cerrar, un fichero escrito sale entero.
//!
//! Aqui va lo que se dice sin punteros:
//!
//! ```text
//!    la ruta    "C:\\juego\\datos\\a.pak" o "datos\\a.pak" -> "juego/datos/a.pak":
//!               la unidad se quita (el volumen de BMO-X es uno), las barras
//!               se enderezan, "." y ".." se resuelven -- y un ".." que saldria
//!               del volumen se RECHAZA, no se recorta
//!    relativa   desde el directorio del `.exe` (su "directorio actual")
//!    abierto    los bytes o la medida a la carta, la posicion, si se escribio,
//!               y SetFilePointerEx con los tres metodos de Windows
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

/// **El disco Personal** (N2, 29-09): `D:\...` de Windows es `d:...` en
/// BMO-X -- el NTFS de Windows, montado SOLO PARA LEER (`dev/disk/ajeno.rs`).
/// Toda otra letra sigue siendo el volumen de BMO-X.
pub const PERSONAL: &str = "d:";

/// Lo de dentro de D: si `vol` es una ruta del disco Personal.
pub fn en_personal(vol: &str) -> Option<&str> {
    vol.strip_prefix(PERSONAL)
}

/// La ruta de Windows ya resuelta: si es de D: y sus trozos.
fn resolver(w: &[u16], dir: &str) -> Result<(bool, Vec<String>), NoRuta> {
    let fin = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    let mut s = String::with_capacity(fin);
    for c in char::decode_utf16(w[..fin].iter().copied()) {
        let c = c.map_err(|_| NoRuta::NoAscii)?;
        s.push(if c == '\\' { '/' } else { c });
    }
    let mut t = s.as_str();
    if let Some(r) = t.strip_prefix("//?/") {
        t = r;
    }
    if t.starts_with("//") || t.is_empty() {
        return Err(NoRuta::NoEsFichero);
    }
    // El directorio actual puede ser de D: (un `.exe` que vive alli).
    let (dir_personal, dir) = match en_personal(dir) {
        Some(d) => (true, d),
        None => (false, dir),
    };
    // "X:/..." o "/...": desde la raiz (la de D:, o la del volumen). Lo demas,
    // desde `dir`.
    let (personal, absoluta, resto) = match t.as_bytes() {
        [l, b':', rest @ ..] if l.is_ascii_alphabetic() => (l.eq_ignore_ascii_case(&b'd'), true, core::str::from_utf8(rest).unwrap_or("")),
        [b'/', ..] => (dir_personal, true, t),
        _ => (dir_personal, false, t),
    };
    // El volumen de BMO-X es FAT32 8.3: alli, solo ASCII. D: es NTFS: UTF-8.
    if !personal && !t.is_ascii() {
        return Err(NoRuta::NoAscii);
    }
    let mut partes: Vec<String> =
        if absoluta { Vec::new() } else { dir.split('/').filter(|p| !p.is_empty()).map(String::from).collect() };
    for p in resto.split('/') {
        match p {
            "" | "." => {}
            // Un `..` no sale de su volumen: ni de BMO-X, ni de D:.
            ".." => {
                if partes.pop().is_none() {
                    return Err(NoRuta::FueraDelVolumen);
                }
            }
            x => partes.push(String::from(x)),
        }
    }
    Ok((personal, partes))
}

fn unir(personal: bool, partes: &[String]) -> String {
    let mut r = String::from(if personal { PERSONAL } else { "" });
    r.push_str(&partes.join("/"));
    r
}

/// **Una ruta de Windows (UTF-16) como ruta de BMO-X**, relativa a la raiz
/// del volumen (o `d:...`, del disco Personal). `dir` es el directorio actual
/// (el del `.exe`).
pub fn ruta(w: &[u16], dir: &str) -> Result<String, NoRuta> {
    let (personal, partes) = resolver(w, dir)?;
    if partes.is_empty() {
        return Err(NoRuta::NoEsFichero);
    }
    Ok(unir(personal, &partes))
}

/// **Como [`ruta`], pero la RAIZ del volumen es una ruta** (`""`, o `d:` la
/// de D:): para lo que nombra carpetas (`C:\\`, `..` desde `window`, `.` en
/// la raiz). Vacia o un dispositivo (`\\\\.\\`) sigue sin ser nada.
pub fn ruta_o_raiz(w: &[u16], dir: &str) -> Result<String, NoRuta> {
    let fin = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    let dispositivo = w.len() >= 2 && fin >= 2 && [w[0], w[1]].iter().all(|&c| c == b'\\' as u16 || c == b'/' as u16) && !(fin >= 4 && w[2] == b'?' as u16);
    if fin == 0 || dispositivo {
        return Err(NoRuta::NoEsFichero);
    }
    let (personal, partes) = resolver(w, dir)?;
    Ok(unir(personal, &partes))
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
    /// Una CARPETA abierta (FILE_FLAG_BACKUP_SEMANTICS, P4f3): sin bytes.
    pub carpeta: bool,
    /// ** A LA CARTA (01-10, Cyberpunk): un fichero grande abierto para
    /// leer NO se trae entero; esta es su medida, `bytes` se queda vacio y
    /// cada lectura trae solo su trozo (`Plataforma::trozos`). Lo pidio un
    /// fichero de 46 MB contra un monton de 25 (y los `.archive` son GB).
    pub a_la_carta: Option<u64>,
}

impl Abierto {
    /// La medida del fichero: la de sus bytes, o la de uno a la carta.
    pub fn medida(&self) -> u64 {
        self.a_la_carta.unwrap_or(self.bytes.len() as u64)
    }

    /// Lo que toca leer a la carta desde la posicion: `(desde, cuantos)`.
    pub fn trozo(&self, quiere: usize) -> (u64, usize) {
        let m = self.medida();
        let desde = self.pos.min(m);
        (desde, (quiere as u64).min(m - desde) as usize)
    }

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
            DESDE_FIN => self.medida() as i128,
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

// -- P4f3: las carpetas ---------------------------------------------------------------

/// **Una entrada de una carpeta** del volumen, como la da quien lista.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entrada {
    pub nombre: String,
    pub carpeta: bool,
    pub bytes: u64,
    /// `[creado, escrito, leido]` en FILETIME (100 ns desde 1601); 0 = no
    /// se sabe (01-10: hoy solo las da el disco Personal, NTFS).
    pub fechas: [u64; 3],
    /// Los atributos de Windows tal cual (ARCHIVE, READONLY...); 0 = no se
    /// saben, y se dan NORMAL o DIRECTORY.
    pub atributos: u32,
}

fn igual_sin_mayusculas(a: u8, b: u8) -> bool {
    a.eq_ignore_ascii_case(&b)
}

/// **Los comodines de Windows**: `*` cualquier cosa (tambien nada), `?` un
/// caracter, sin mayusculas que cuenten. Y la regla vieja de Windows: `*.*`
/// es "todo", aunque el nombre no tenga punto.
pub fn comodin(patron: &str, nombre: &str) -> bool {
    if patron == "*.*" || patron == "*" {
        return true;
    }
    let (p, n) = (patron.as_bytes(), nombre.as_bytes());
    // El de siempre, con vuelta atras al ultimo `*`.
    let (mut i, mut j, mut estrella, mut marca) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == b'?' || igual_sin_mayusculas(p[i], n[j])) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            estrella = Some(i);
            marca = j;
            i += 1;
        } else if let Some(e) = estrella {
            i = e + 1;
            marca += 1;
            j = marca;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}

/// **Partir lo que se busca** (`FindFirstFileW("datos\\*.pak")`): la carpeta
/// del volumen donde mirar y el patron del ultimo trozo.
pub fn partir_patron(w: &[u16], dir: &str) -> Result<(String, String), NoRuta> {
    let fin = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    let w = &w[..fin];
    let corte = w.iter().rposition(|&c| c == b'\\' as u16 || c == b'/' as u16);
    let (carpeta, patron) = match corte {
        Some(k) => (&w[..k + 1], &w[k + 1..]),
        None => (&w[..0], w),
    };
    if patron.is_empty() {
        return Err(NoRuta::NoEsFichero);
    }
    let patron: String = char::decode_utf16(patron.iter().copied()).collect::<Result<String, _>>().map_err(|_| NoRuta::NoEsFichero)?;
    // La carpeta: "" es el directorio actual; "X:\\" o "\\" la raiz (la de
    // D:, `d:`, si es de alli).
    let carpeta = if carpeta.is_empty() {
        String::from(dir.trim_matches('/'))
    } else {
        let mut c: Vec<u16> = carpeta.to_vec();
        c.push(b'.' as u16);
        ruta_o_raiz(&c, dir)?
    };
    // Fuera de D:, el patron sigue siendo ASCII (FAT32 8.3).
    if en_personal(&carpeta).is_none() && !patron.is_ascii() {
        return Err(NoRuta::NoEsFichero);
    }
    Ok((carpeta, patron))
}
