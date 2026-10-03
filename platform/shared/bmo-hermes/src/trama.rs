//! **LO QUE VIAJA DENTRO** -- los mensajes de HERMES/1, una vez abierto el
//! cifrado. Seccion 4.3 de `docs/plan/PLAN_HERMES.md`.
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina
//! [cuesta]  DATO      un mensaje mal leido muestra o guarda lo que no es
//! [riesgo]  AJENO     cada byte lo escribe la otra punta
//!
//! # El formato
//!
//! El primer byte es el VERBO; lo demas, su cuerpo. Los numeros van en
//! big-endian, como en la red.
//!
//! ```text
//!    0x01 TEXTO     utf-8, de 1 a 2048 bytes
//!    0x02 ZUMBIDO   nada
//!    0x03 GUINO     n (1 byte), del catalogo cerrado
//!    0x04 OFERTA    id (4), bytes (8), sha256 (32), nombre utf-8 (1..255)
//!    0x05 SI        id (4)
//!    0x06 NO        id (4)
//!    0x07 TROZO     id (4), numero (4), datos (1..60 KiB)
//!    0x08 PIDE      que (1: muro, 2: canal, 3: pagina), nombre [a-z0-9_-]{1,32}
//!    0x09 REACCION  de quien (1: 0 mio, 1 tuyo), numero (8), emoji (1..32)
//! ```
//!
//! *** **El TIPO de un fichero no viaja.** La OFERTA dice nombre, bytes y
//! suma, y nada mas: lo que es el fichero lo dicen sus bytes, y los mira el
//! JUEZ en su jaula. Un nombre que dice `.jpg` no es un JPEG.
//!
//! ** **El TROZO es de 60 KiB y no de 64.** El plan decia 64, y no cabe: un
//! mensaje de Noise mide como mucho 65.535 bytes, menos 16 de etiqueta, menos
//! 9 de cabecera del trozo. 60 KiB deja sitio y es un numero redondo.
//!
//! # Lista blanca, como `bmo-pila`
//!
//! Lo que no esta escrito aqui que se acepta, se rechaza, y cada rechazo lleva
//! su nombre en [`Rechazo`]. Y [`escribir`] pasa lo que escribe por el MISMO
//! [`leer`] antes de devolverlo: esta punta no manda nada que ella misma
//! rechazaria.

use crate::Rechazo;

pub const TEXTO: u8 = 0x01;
pub const ZUMBIDO: u8 = 0x02;
pub const GUINO: u8 = 0x03;
pub const OFERTA: u8 = 0x04;
pub const SI: u8 = 0x05;
pub const NO: u8 = 0x06;
pub const TROZO: u8 = 0x07;
pub const PIDE: u8 = 0x08;
pub const REACCION: u8 = 0x09;

/// Lo mas largo que puede ser un TEXTO, en bytes de UTF-8.
pub const TEXTO_MAX: usize = 2048;
/// Cuantos guinos tiene el catalogo.
pub const GUINOS: u8 = 6;
/// Lo mas largo que puede ser el nombre de una OFERTA.
pub const NOMBRE_MAX: usize = 255;
/// Lo mas grande que puede ser un envio entero: 4 GiB menos un byte. Lo que
/// acepta AUTOMATICO (2 GiB) lo decide la app, no el protocolo.
pub const ENVIO_MAX: u64 = (1 << 32) - 1;
/// Lo mas grande que puede ser un trozo.
pub const TROZO_MAX: usize = 60 * 1024;
/// Lo mas largo que puede ser el nombre de lo que se PIDE.
pub const PEDIDO_MAX: usize = 32;
/// Lo mas largo que puede ser el emoji de una REACCION (las secuencias con
/// tono y uniones de dos personas llegan a ~28 bytes).
pub const EMOJI_MAX: usize = 32;
/// La cabecera de un trozo: verbo, id y numero.
pub const TROZO_CABECERA: usize = 9;

/// Que se pide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Que {
    Muro,
    Canal,
    Pagina,
}

/// **Un mensaje de HERMES/1**, prestado del buffer que se leyo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mensaje<'a> {
    Texto(&'a str),
    Zumbido,
    Guino(u8),
    Oferta { id: u32, bytes: u64, suma: &'a [u8; 32], nombre: &'a str },
    Si(u32),
    No(u32),
    Trozo { id: u32, numero: u32, datos: &'a [u8] },
    Pide { que: Que, nombre: &'a str },
    /// `tuyo`: el mensaje al que se reacciona lo mando quien RECIBE esta
    /// reaccion. `numero`: su numero en esa direccion (ver
    /// [`crate::noise::Canal::enviados`]).
    Reaccion { tuyo: bool, numero: u64, emoji: &'a str },
}

fn u32_de(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn u64_de(b: &[u8]) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[..8]);
    u64::from_be_bytes(x)
}

fn utf8(b: &[u8]) -> Result<&str, Rechazo> {
    core::str::from_utf8(b).map_err(|_| Rechazo::Utf8)
}

/// Los caracteres de control que no se muestran nunca. `\n` si vale en un
/// TEXTO; en un nombre, nada de esto.
fn es_control(c: char) -> bool {
    (c as u32) < 0x20 || c == '\u{7F}' || ('\u{80}'..='\u{9F}').contains(&c)
}

/// Los que dan la vuelta al texto: con ellos, `foto\u{202E}gpj.exe` se ve como
/// `fotoexe.jpg`. En un nombre de fichero no tienen nada que hacer.
fn es_giro(c: char) -> bool {
    ('\u{202A}'..='\u{202E}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c)
}

/// **Es un emoji, y no texto?** No hay catalogo aqui (ese lo tiene la app);
/// se pide algo mas chico y que no se puede burlar con letras: ni un
/// caracter de control, ni un espacio, ni una letra ASCII, y al menos uno
/// fuera de ASCII. `1` + `U+FE0F` + `U+20E3` (el teclado) pasa; `hola` no.
fn es_emoji(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= EMOJI_MAX
        && s.chars().all(|c| !es_control(c) && !c.is_whitespace() && !c.is_ascii_alphabetic())
        && s.chars().any(|c| !c.is_ascii())
}

/// **Lee un mensaje.** `b` es lo que salio de [`crate::noise::Canal::abrir`].
pub fn leer(b: &[u8]) -> Result<Mensaje<'_>, Rechazo> {
    let (&verbo, c) = b.split_first().ok_or(Rechazo::Vacio)?;
    match verbo {
        TEXTO => {
            if c.is_empty() || c.len() > TEXTO_MAX {
                return Err(Rechazo::Medida);
            }
            let s = utf8(c)?;
            if s.chars().any(|ch| es_control(ch) && ch != '\n') {
                return Err(Rechazo::Control);
            }
            Ok(Mensaje::Texto(s))
        }
        ZUMBIDO => {
            if !c.is_empty() {
                return Err(Rechazo::Medida);
            }
            Ok(Mensaje::Zumbido)
        }
        GUINO => {
            if c.len() != 1 {
                return Err(Rechazo::Medida);
            }
            if c[0] == 0 || c[0] > GUINOS {
                return Err(Rechazo::Guino);
            }
            Ok(Mensaje::Guino(c[0]))
        }
        OFERTA => {
            if c.len() < 4 + 8 + 32 + 1 || c.len() > 4 + 8 + 32 + NOMBRE_MAX {
                return Err(Rechazo::Medida);
            }
            let bytes = u64_de(&c[4..12]);
            if bytes == 0 || bytes > ENVIO_MAX {
                return Err(Rechazo::Bytes);
            }
            let suma: &[u8; 32] = c[12..44].try_into().map_err(|_| Rechazo::Medida)?;
            let nombre = utf8(&c[44..])?;
            if nombre.chars().any(|ch| es_control(ch) || es_giro(ch)) {
                return Err(Rechazo::Nombre);
            }
            Ok(Mensaje::Oferta { id: u32_de(&c[..4]), bytes, suma, nombre })
        }
        SI | NO => {
            if c.len() != 4 {
                return Err(Rechazo::Medida);
            }
            let id = u32_de(c);
            Ok(if verbo == SI { Mensaje::Si(id) } else { Mensaje::No(id) })
        }
        TROZO => {
            if c.len() < 8 + 1 || c.len() > 8 + TROZO_MAX {
                return Err(Rechazo::Medida);
            }
            Ok(Mensaje::Trozo { id: u32_de(&c[..4]), numero: u32_de(&c[4..8]), datos: &c[8..] })
        }
        PIDE => {
            if c.len() < 2 || c.len() > 1 + PEDIDO_MAX {
                return Err(Rechazo::Medida);
            }
            let que = match c[0] {
                1 => Que::Muro,
                2 => Que::Canal,
                3 => Que::Pagina,
                _ => return Err(Rechazo::Que),
            };
            if !c[1..].iter().all(|&x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == b'_' || x == b'-') {
                return Err(Rechazo::Nombre);
            }
            // Ya se comprobo que es ASCII: no puede fallar.
            let nombre = utf8(&c[1..])?;
            Ok(Mensaje::Pide { que, nombre })
        }
        REACCION => {
            if c.len() < 1 + 8 + 1 || c.len() > 1 + 8 + EMOJI_MAX {
                return Err(Rechazo::Medida);
            }
            let tuyo = match c[0] {
                0 => false,
                1 => true,
                _ => return Err(Rechazo::Bandera),
            };
            let emoji = utf8(&c[9..])?;
            if !es_emoji(emoji) {
                return Err(Rechazo::NoEsEmoji);
            }
            Ok(Mensaje::Reaccion { tuyo, numero: u64_de(&c[1..9]), emoji })
        }
        _ => Err(Rechazo::Verbo),
    }
}

struct Pluma<'a> {
    dst: &'a mut [u8],
    n: usize,
}

impl Pluma<'_> {
    fn mete(&mut self, b: &[u8]) -> Result<(), Rechazo> {
        let fin = self.n + b.len();
        self.dst.get_mut(self.n..fin).ok_or(Rechazo::SinSitio)?.copy_from_slice(b);
        self.n = fin;
        Ok(())
    }
}

/// **Escribe un mensaje** en `dst` y lo pasa por [`leer`] antes de darlo por
/// bueno. Devuelve cuantos bytes ocupa.
pub fn escribir(m: &Mensaje<'_>, dst: &mut [u8]) -> Result<usize, Rechazo> {
    let mut p = Pluma { dst, n: 0 };
    match *m {
        Mensaje::Texto(s) => {
            p.mete(&[TEXTO])?;
            p.mete(s.as_bytes())?;
        }
        Mensaje::Zumbido => p.mete(&[ZUMBIDO])?,
        Mensaje::Guino(n) => p.mete(&[GUINO, n])?,
        Mensaje::Oferta { id, bytes, suma, nombre } => {
            p.mete(&[OFERTA])?;
            p.mete(&id.to_be_bytes())?;
            p.mete(&bytes.to_be_bytes())?;
            p.mete(suma)?;
            p.mete(nombre.as_bytes())?;
        }
        Mensaje::Si(id) => {
            p.mete(&[SI])?;
            p.mete(&id.to_be_bytes())?;
        }
        Mensaje::No(id) => {
            p.mete(&[NO])?;
            p.mete(&id.to_be_bytes())?;
        }
        Mensaje::Trozo { id, numero, datos } => {
            p.mete(&[TROZO])?;
            p.mete(&id.to_be_bytes())?;
            p.mete(&numero.to_be_bytes())?;
            p.mete(datos)?;
        }
        Mensaje::Pide { que, nombre } => {
            let q = match que {
                Que::Muro => 1,
                Que::Canal => 2,
                Que::Pagina => 3,
            };
            p.mete(&[PIDE, q])?;
            p.mete(nombre.as_bytes())?;
        }
        Mensaje::Reaccion { tuyo, numero, emoji } => {
            p.mete(&[REACCION, tuyo as u8])?;
            p.mete(&numero.to_be_bytes())?;
            p.mete(emoji.as_bytes())?;
        }
    }
    let n = p.n;
    let hecho = &p.dst[..n];
    let vuelta = leer(hecho)?;
    // Lo que se escribio y lo que se lee tienen que ser lo mismo.
    if vuelta != *m {
        return Err(Rechazo::Medida);
    }
    Ok(n)
}

#[cfg(test)]
#[path = "trama_pruebas.rs"]
mod pruebas;
