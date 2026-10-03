//! **EL CENSO DE LOS SONIDOS** -- que hay en el disco que suene, y que dice
//! BMO-X de cada cosa.
//!
//! El propietario (2026-10-03): *"que encuentren TODO el disco en FAT32 y
//! ESTRATOS [...] encuentran formatos de sonidos y te dice que no son
//! oficiales y otras si"*.
//!
//! [`reconocer`](crate::reconocer) contesta una pregunta chica --se puede
//! tocar?-- con cuatro formatos. Esto contesta la del censo, que es otra:
//! **que es este fichero, y que dice BMO-X de el**. Por eso reconoce mas
//! (AAC, M4A, Opus, Vorbis, AIFF, MIDI, WMA) y separa tres cosas que un "no"
//! a secas mezclaria:
//!
//! ```text
//!    SUENA       oficial y hoy: un WAV PCM que `leer_wav` acepta
//!    OFICIAL     BMO-X lo tocara: su decodificador esta en el plan
//!    NO OFICIAL  se reconoce, y BMO-X no lo va a tocar, con su motivo
//! ```
//!
//! ** Y una cuarta, la que mas vale de un censo: **el nombre miente**. Un
//! `.mp3` que por dentro es un WAV, o un `.wav` que no es nada. Mirar el
//! contenido y no la extension es la regla del JUEZ de HERMES, aplicada aqui.
//!
//! [!] Como `reconocer`, esto mira el PRINCIPIO del fichero: es una ficha, no
//! un juicio. El sincronismo de MPEG (`0xFF Ex`) puede salir por casualidad, y
//! por eso un MP3 sin `ID3` y con la extension equivocada se cuenta como
//! "parece MP3", no como MP3 seguro. Lo que decide si algo suena es el
//! decodificador, cuando lo abre entero.

/// **El codec de un WAV**, leyendo solo su trozo `fmt `. No usa
/// [`crate::leer_wav`] a proposito: con la cabecera sola, el trozo `data` dice
/// medir mas de lo que hay y aquel contesta "sobre roto" de un WAV bueno.
fn codec_wav(b: &[u8]) -> Option<u16> {
    let mut off = 12usize;
    while off + 8 <= b.len() {
        let largo = u32::from_le_bytes([b[off + 4], b[off + 5], b[off + 6], b[off + 7]]) as u64;
        if &b[off..off + 4] == b"fmt " {
            if largo < 16 || b.len() < off + 10 {
                return None;
            }
            return Some(u16::from_le_bytes([b[off + 8], b[off + 9]]));
        }
        // Saltar el trozo (con su relleno), en u64: el largo lo escribe el fichero.
        let sig = off as u64 + 8 + largo + (largo & 1);
        if sig > b.len() as u64 {
            return None;
        }
        off = sig as usize;
    }
    None
}

/// Cuantos bytes del principio hacen falta para la ficha. Con menos tambien
/// contesta; con mas no sabe nada nuevo.
pub const CABECERA: usize = 64;

/// **Que es, por dentro.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    /// RIFF/WAVE. Puede ser PCM (suena) o llevar otro codec dentro.
    Wav,
    /// MPEG audio capa 3, con etiqueta `ID3` o por su sincronismo.
    Mp3,
    Flac,
    /// `OggS` con Vorbis dentro.
    Vorbis,
    /// `OggS` con Opus dentro.
    Opus,
    /// `OggS` con otra cosa (Theora es video, por ejemplo).
    Ogg,
    /// AAC suelto, en tramas ADTS.
    Aac,
    /// Una caja MP4 de audio (`ftyp` M4A, M4B, M4P...).
    M4a,
    /// `FORM` / `AIFF` o `AIFC`: PCM de Apple, big-endian.
    Aiff,
    /// `MThd`: no son muestras, son notas.
    Midi,
    /// El contenedor ASF de Windows Media.
    Wma,
    /// No se parece a ningun sonido.
    Nada,
}

impl Tipo {
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Wav => "WAV",
            Tipo::Mp3 => "MP3",
            Tipo::Flac => "FLAC",
            Tipo::Vorbis => "OGG Vorbis",
            Tipo::Opus => "OGG Opus",
            Tipo::Ogg => "OGG (sin audio conocido)",
            Tipo::Aac => "AAC",
            Tipo::M4a => "M4A",
            Tipo::Aiff => "AIFF",
            Tipo::Midi => "MIDI",
            Tipo::Wma => "WMA",
            Tipo::Nada => "no es sonido",
        }
    }

    /// Las extensiones que le corresponden, en minusculas y sin punto.
    fn extensiones(self) -> &'static [&'static str] {
        match self {
            Tipo::Wav => &["wav", "wave"],
            Tipo::Mp3 => &["mp3"],
            Tipo::Flac => &["flac"],
            Tipo::Vorbis | Tipo::Ogg => &["ogg", "oga"],
            Tipo::Opus => &["opus", "ogg"],
            Tipo::Aac => &["aac"],
            Tipo::M4a => &["m4a", "m4b", "m4p", "mp4"],
            Tipo::Aiff => &["aif", "aiff", "aifc"],
            Tipo::Midi => &["mid", "midi"],
            Tipo::Wma => &["wma", "asf"],
            Tipo::Nada => &[],
        }
    }
}

/// **Lo que dice BMO-X**, con su motivo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veredicto {
    /// Oficial, y suena hoy (cuando el tubo suene): PCM en un sobre.
    Suena,
    /// Oficial: BMO-X lo tocara. El `&str` dice que falta.
    Oficial(&'static str),
    /// Se reconoce y no se va a tocar. El `&str` dice por que.
    NoOficial(&'static str),
    /// No es sonido (o no se parece a ninguno).
    NoEsSonido,
}

impl Veredicto {
    /// La palabra corta, para una columna.
    pub fn palabra(self) -> &'static str {
        match self {
            Veredicto::Suena => "SUENA",
            Veredicto::Oficial(_) => "OFICIAL",
            Veredicto::NoOficial(_) => "NO OFICIAL",
            Veredicto::NoEsSonido => "NO ES SONIDO",
        }
    }

    /// El motivo, para quien lo lee.
    pub fn motivo(self) -> &'static str {
        match self {
            Veredicto::Suena => "PCM en un sobre: cero decodificador",
            Veredicto::Oficial(m) | Veredicto::NoOficial(m) => m,
            Veredicto::NoEsSonido => "por dentro no se parece a ningun sonido",
        }
    }
}

/// **La ficha de un fichero.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ficha {
    pub tipo: Tipo,
    pub veredicto: Veredicto,
    /// La extension dice una cosa y el contenido otra.
    pub nombre_miente: bool,
    /// Solo por el sincronismo de MPEG, sin `ID3`: puede ser casualidad.
    pub dudoso: bool,
}

/// La extension de `nombre`, en minusculas, en `dst`. Vacia si no tiene.
fn extension<'a>(nombre: &str, dst: &'a mut [u8; 8]) -> &'a str {
    let Some(i) = nombre.rfind('.') else { return "" };
    let e = &nombre.as_bytes()[i + 1..];
    if e.is_empty() || e.len() > dst.len() || !e.iter().all(u8::is_ascii_alphanumeric) {
        return "";
    }
    for (k, b) in e.iter().enumerate() {
        dst[k] = b.to_ascii_lowercase();
    }
    // Solo ASCII alfanumerico: no puede fallar.
    core::str::from_utf8(&dst[..e.len()]).unwrap_or("")
}

/// Todas las extensiones de sonido que se conocen: para saber si un fichero
/// que no es sonido ESTA DICIENDO que lo es.
const EXT_SONIDO: &[&str] = &["wav", "wave", "mp3", "flac", "ogg", "oga", "opus", "aac", "m4a", "m4b", "m4p", "aif", "aiff", "aifc", "mid", "midi", "wma"];

/// **Que es por dentro**, mirando el principio.
pub fn tipo(b: &[u8]) -> (Tipo, bool) {
    let empieza = |off: usize, m: &[u8]| b.len() >= off + m.len() && &b[off..off + m.len()] == m;
    if empieza(0, b"RIFF") && empieza(8, b"WAVE") {
        return (Tipo::Wav, false);
    }
    if empieza(0, b"fLaC") {
        return (Tipo::Flac, false);
    }
    if empieza(0, b"OggS") {
        // La primera pagina lleva la cabecera del codec a partir del byte 28.
        if empieza(28, b"\x01vorbis") {
            return (Tipo::Vorbis, false);
        }
        if empieza(28, b"OpusHead") {
            return (Tipo::Opus, false);
        }
        return (Tipo::Ogg, false);
    }
    if empieza(0, b"FORM") && (empieza(8, b"AIFF") || empieza(8, b"AIFC")) {
        return (Tipo::Aiff, false);
    }
    if empieza(0, b"MThd") {
        return (Tipo::Midi, false);
    }
    // El GUID de la cabecera de ASF: 30 26 B2 75 8E 66 CF 11.
    if empieza(0, &[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]) {
        return (Tipo::Wma, false);
    }
    if empieza(4, b"ftyp") && (empieza(8, b"M4A") || empieza(8, b"M4B") || empieza(8, b"M4P")) {
        return (Tipo::M4a, false);
    }
    if empieza(0, b"ID3") {
        return (Tipo::Mp3, false);
    }
    if b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0 {
        // Doce unos: la capa decide. `00` en la capa es ADTS (AAC); `01` es
        // la capa 3 de MPEG, que es MP3. Las capas 1 y 2 tambien son MPEG
        // audio y se cuentan con MP3, con la duda dicha.
        let capa = (b[1] >> 1) & 0b11;
        if capa == 0 && b[1] & 0xF0 == 0xF0 {
            return (Tipo::Aac, false);
        }
        if capa != 0 {
            return (Tipo::Mp3, true);
        }
    }
    (Tipo::Nada, false)
}

/// **La ficha de un fichero**: su `cabecera` (los primeros [`CABECERA`]
/// bytes, o los que tenga) y su `nombre`, solo para ver si miente.
pub fn ficha(cabecera: &[u8], nombre: &str) -> Ficha {
    let (t, dudoso) = tipo(cabecera);
    let veredicto = match t {
        // Un WAV que `leer_wav` acepta es PCM: suena. Con solo la cabecera,
        // `leer_wav` puede quedarse corto de bytes; lo que mira es el formato.
        Tipo::Wav => match codec_wav(cabecera) {
            Some(1) | Some(0xFFFE) => Veredicto::Suena,
            Some(3) => Veredicto::Oficial("un WAV de coma flotante: el amplificador lo baja a enteros"),
            Some(0x55) => Veredicto::Oficial("un MP3 metido en un WAV: falta el decodificador de MP3 (M2)"),
            Some(_) => Veredicto::NoOficial("un WAV con un codec raro dentro: se convierte fuera"),
            None => Veredicto::Oficial("un WAV cuyo trozo `fmt ` no esta al principio: lo dira leer_wav entero"),
        },
        Tipo::Mp3 => Veredicto::Oficial("falta el decodificador propio de MP3 (M2 de PLAN_MEDIOS)"),
        Tipo::Flac => Veredicto::Oficial("sin perdida; su decodificador va despues de MP3"),
        Tipo::Vorbis => Veredicto::Oficial("su decodificador va despues de MP3"),
        Tipo::Opus => Veredicto::Oficial("el de las llamadas; va con la voz de HERMES (H12)"),
        Tipo::Aac | Tipo::M4a => Veredicto::Oficial("el sonido de los .mp4: va con el CANAL (M6 de PLAN_LA_3060)"),
        Tipo::Aiff => Veredicto::Oficial("PCM como el WAV, al reves (big-endian): barato"),
        Tipo::Ogg => Veredicto::NoOficial("un OGG sin Vorbis ni Opus no es sonido que BMO-X sepa leer"),
        Tipo::Midi => Veredicto::NoOficial("son notas, no muestras: pide un sintetizador"),
        Tipo::Wma => Veredicto::NoOficial("formato cerrado de Microsoft, sin especificacion libre"),
        Tipo::Nada => Veredicto::NoEsSonido,
    };
    let mut buf = [0u8; 8];
    let ext = extension(nombre, &mut buf);
    let nombre_miente = if t == Tipo::Nada {
        EXT_SONIDO.contains(&ext)
    } else {
        !ext.is_empty() && !t.extensiones().contains(&ext)
    };
    Ficha { tipo: t, veredicto, nombre_miente, dudoso }
}

/// **El resumen de un censo**: cuantos de cada cosa.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Cuenta {
    pub ficheros: u32,
    pub suenan: u32,
    pub oficiales: u32,
    pub no_oficiales: u32,
    pub mienten: u32,
    pub por_tipo: [u32; 12],
}

impl Cuenta {
    /// Apunta una ficha. Lo que no es sonido cuenta en `ficheros` y en nada mas.
    pub fn meter(&mut self, f: &Ficha) {
        self.ficheros = self.ficheros.saturating_add(1);
        let k = match f.veredicto {
            Veredicto::Suena => &mut self.suenan,
            Veredicto::Oficial(_) => &mut self.oficiales,
            Veredicto::NoOficial(_) => &mut self.no_oficiales,
            Veredicto::NoEsSonido => return self.contar_mentira(f),
        };
        *k = k.saturating_add(1);
        let i = f.tipo as usize;
        self.por_tipo[i] = self.por_tipo[i].saturating_add(1);
        self.contar_mentira(f);
    }

    fn contar_mentira(&mut self, f: &Ficha) {
        if f.nombre_miente {
            self.mienten = self.mienten.saturating_add(1);
        }
    }

    /// Cuantos ficheros de sonido (de cualquier veredicto).
    pub fn sonidos(&self) -> u32 {
        self.suenan + self.oficiales + self.no_oficiales
    }
}

#[cfg(test)]
#[path = "censo_pruebas.rs"]
mod pruebas;
