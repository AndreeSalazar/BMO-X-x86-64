//! **BMO LUDOTECA** -- los juegos que compraste, como lineas que BMO-X sabe
//! juzgar (J0 de `docs/plan/PLAN_LA_LUDOTECA.md`, 01-10).
//!
//! generacion: nieto -- lineas y veredictos; no sabe de sockets, de TLS ni de GOG
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan Ring 3 y el banco
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina (la antena, o la red)
//! [cuesta]  DATO      una linea mal leida pone en la lista un juego que no es
//! [riesgo]  AJENO     cada linea la escribe quien habla con la tienda
//!
//! # Las tres lineas
//!
//! Quien habla con la tienda (hoy la antena; el dia del TLS, BMO-X mismo) le
//! da a la LUDOTECA solo esto, una cosa por linea:
//!
//! ```text
//!    JUEGO <id> <tienda> <titulo>              uno por juego que tienes
//!    FICHERO <id> <bytes> <sha256> <nombre>    lo que se puede traer, con su suma
//!                                              (el nombre al final: lleva espacios)
//!    MOTOR <id> <motor>                        el motor nativo, si se sabe
//! ```
//!
//! # El camino de cada juego
//!
//! ```text
//!    NATIVO     hay motor abierto que BMO-X compila (DOOM, Quake...): lo dice
//!               una linea MOTOR, o un FICHERO que es el dato de un motor
//!               (DOOM2.WAD, id1/PAK0.PAK)
//!    PROTON-X   no hay motor, pero trae un .exe de Windows (Cyberpunk)
//!    PENDIENTE  todavia no se sabe: faltan sus ficheros (la tienda dio el
//!               titulo y nada mas). Se juega AQUI o no se juega: sin
//!               streaming, por decision del propietario (01-10: "no quiero
//!               streaming, que mi PC lo aplique, o OM")
//! ```
//!
//! # Lista blanca, como `bmo-antena`
//!
//! ```text
//!    una linea de mas de 256 bytes, o con un byte no imprimible   Largo / NoAscii
//!    un verbo que no es JUEGO, FICHERO ni MOTOR                    Verbo
//!    un id fuera de [a-z0-9_-]{1,32}                               Id
//!    una tienda que no esta en la lista de `Tienda`                Tienda
//!    un titulo vacio o de mas de 128                               Titulo
//!    un nombre con `..`, que empieza en `/` o con otro byte        Nombre
//!    (o con dos espacios seguidos, o uno al principio o al final)
//!    unos bytes que no son un numero, o de mas de 1 TiB            Bytes
//!    una suma que no son 64 hex en minusculas                      Suma
//!    un motor que la tabla no conoce                               Motor
//! ```
//!
//! Y el JUEZ DE LA SUMA: lo que llega se cuenta y se resume con SHA-256 de
//! `bmo-cripto`; si el largo o la suma no cuadran con su FICHERO, no entra.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Lo mas largo que puede ser una linea, con su fin.
pub const LINEA_MAX: usize = 256;
/// Lo mas largo que puede ser un titulo.
pub const TITULO_MAX: usize = 128;
/// Lo mas largo que puede ser un nombre de fichero.
pub const NOMBRE_MAX: usize = 128;
/// Lo mas grande que puede ser un fichero: 1 TiB.
pub const BYTES_MAX: u64 = 1 << 40;
/// Cuantos juegos caben en una ludoteca.
pub const JUEGOS_MAX: usize = 4096;
/// Cuantos ficheros caben en una ludoteca.
pub const FICHEROS_MAX: usize = 65536;

/// Por que una linea no vale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falla {
    Largo,
    NoAscii,
    Verbo,
    Campos,
    Id,
    Tienda,
    Titulo,
    Nombre,
    Bytes,
    Suma,
    Motor,
    /// Un id de JUEGO que ya estaba.
    Repetido,
    /// Un FICHERO o un MOTOR de un juego que no se ha dicho antes.
    Huerfano,
    /// Mas juegos o ficheros de los que caben.
    Lleno,
}

/// De que tienda es: las globales, y lo libre (01-10: *"todas las tiendas
/// globales"*). La linea dice el nombre corto; la Biblioteca, el largo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tienda {
    Gog,
    Steam,
    Epic,
    /// Ubisoft Connect.
    Ubisoft,
    /// EA app.
    Ea,
    /// Battle.net.
    Battlenet,
    /// Microsoft Store / Xbox para PC.
    Microsoft,
    /// Rockstar Games Launcher.
    Rockstar,
    /// Amazon Games.
    Amazon,
    Itch,
    Humble,
    /// Libre: Freedoom, un shareware...
    Libre,
}

/// `(tienda, nombre corto en la linea, nombre largo)`.
const TIENDAS: [(Tienda, &str, &str); 12] = [
    (Tienda::Gog, "gog", "GOG"),
    (Tienda::Steam, "steam", "Steam"),
    (Tienda::Epic, "epic", "Epic Games"),
    (Tienda::Ubisoft, "ubisoft", "Ubisoft Connect"),
    (Tienda::Ea, "ea", "EA app"),
    (Tienda::Battlenet, "battlenet", "Battle.net"),
    (Tienda::Microsoft, "microsoft", "Microsoft Store"),
    (Tienda::Rockstar, "rockstar", "Rockstar Games"),
    (Tienda::Amazon, "amazon", "Amazon Games"),
    (Tienda::Itch, "itch", "itch.io"),
    (Tienda::Humble, "humble", "Humble"),
    (Tienda::Libre, "libre", "libre"),
];

impl Tienda {
    fn de(p: &[u8]) -> Option<Self> {
        TIENDAS.iter().find(|t| t.1.as_bytes() == p).map(|t| t.0)
    }

    /// Como se escribe en una linea JUEGO.
    pub fn corto(self) -> &'static str {
        TIENDAS.iter().find(|t| t.0 == self).map_or("", |t| t.1)
    }

    /// Como se muestra.
    pub fn nombre(self) -> &'static str {
        TIENDAS.iter().find(|t| t.0 == self).map_or("", |t| t.2)
    }

    /// Todas, en el orden de la Biblioteca.
    pub fn todas() -> impl Iterator<Item = Tienda> {
        TIENDAS.iter().map(|t| t.0)
    }
}

/// **Los motores nativos**: los que BMO-X compila, y el fichero de datos que
/// delata a cada uno. La tabla es de BMO-X, no de la tienda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motor {
    Doom,
    Heretic,
    Hexen,
    Quake,
}

/// `(motor, nombre en la linea MOTOR, datos que lo delatan)`.
const MOTORES: [(Motor, &str, &[&str]); 4] = [
    (Motor::Doom, "doom", &["doom.wad", "doom2.wad", "plutonia.wad", "tnt.wad", "freedoom1.wad", "freedoom2.wad"]),
    (Motor::Heretic, "heretic", &["heretic.wad"]),
    (Motor::Hexen, "hexen", &["hexen.wad"]),
    (Motor::Quake, "quake", &["id1/pak0.pak"]),
];

impl Motor {
    fn de(p: &[u8]) -> Option<Self> {
        MOTORES.iter().find(|m| m.1.as_bytes() == p).map(|m| m.0)
    }

    /// Como se llama en una linea MOTOR.
    pub fn nombre(self) -> &'static str {
        MOTORES.iter().find(|m| m.0 == self).map_or("", |m| m.1)
    }

    /// El motor cuyo dato es `nombre` (sin mirar mayusculas, y el fichero
    /// puede venir dentro de carpetas: `DOOM II/base/DOOM2.WAD`).
    pub fn por_dato(nombre: &str) -> Option<Self> {
        MOTORES.iter().find(|m| m.2.iter().any(|d| termina_en(nombre, d))).map(|m| m.0)
    }
}

/// Si `nombre` es `d` o acaba en `/d`, sin mirar mayusculas.
fn termina_en(nombre: &str, d: &str) -> bool {
    let (n, d) = (nombre.as_bytes(), d.as_bytes());
    if n.len() < d.len() || !n[n.len() - d.len()..].eq_ignore_ascii_case(d) {
        return false;
    }
    n.len() == d.len() || n[n.len() - d.len() - 1] == b'/'
}

/// Una linea leida. Los textos apuntan dentro de la linea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linea<'a> {
    Juego { id: &'a str, tienda: Tienda, titulo: &'a str },
    Fichero { id: &'a str, nombre: &'a str, bytes: u64, suma: [u8; 32] },
    Motor { id: &'a str, motor: Motor },
}

fn es_id(p: &[u8]) -> bool {
    (1..=32).contains(&p.len()) && p.iter().all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Un nombre de fichero: relativo, sin `..`, con `/` entre carpetas.
fn es_nombre(p: &[u8]) -> bool {
    (1..=NOMBRE_MAX).contains(&p.len())
        && p[0] != b'/'
        && p.iter().all(|&b| b.is_ascii_alphanumeric() || b"._-/+()' &,!".contains(&b))
        && !p.windows(2).any(|w| w == b"  ")
        && p.split(|&b| b == b'/').all(|parte| {
            !parte.is_empty() && parte != b".." && parte != b"." && parte[0] != b' ' && parte[parte.len() - 1] != b' '
        })
}

fn numero(p: &[u8]) -> Option<u64> {
    if p.is_empty() || p.len() > 13 || (p.len() > 1 && p[0] == b'0') {
        return None;
    }
    let mut n = 0u64;
    for &b in p {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n * 10 + (b - b'0') as u64;
    }
    (n <= BYTES_MAX).then_some(n)
}

fn suma(p: &[u8]) -> Option<[u8; 32]> {
    if p.len() != 64 {
        return None;
    }
    let mut s = [0u8; 32];
    for (i, par) in p.chunks(2).enumerate() {
        let v = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        s[i] = v(par[0])? << 4 | v(par[1])?;
    }
    Some(s)
}

/// Parte en `n` campos por espacios simples; el ultimo se lleva el resto.
fn campos<const N: usize>(l: &[u8]) -> Option<[&[u8]; N]> {
    let mut out = [&l[..0]; N];
    let mut resto = l;
    for (k, o) in out.iter_mut().enumerate() {
        if k == N - 1 {
            *o = resto;
        } else {
            let i = resto.iter().position(|&b| b == b' ')?;
            *o = &resto[..i];
            resto = &resto[i + 1..];
        }
    }
    Some(out)
}

/// Un trozo de una linea ya juzgada como ASCII imprimible: es `str` sin mas.
fn t(p: &[u8]) -> &str {
    core::str::from_utf8(p).unwrap_or("")
}

/// **Lee una linea.** Admite su fin (`\n` o `\r\n`) o ninguno.
pub fn leer(l: &[u8]) -> Result<Linea<'_>, Falla> {
    if l.len() > LINEA_MAX {
        return Err(Falla::Largo);
    }
    let l = l.strip_suffix(b"\n").unwrap_or(l);
    let l = l.strip_suffix(b"\r").unwrap_or(l);
    if l.iter().any(|&b| !(0x20..0x7F).contains(&b)) {
        return Err(Falla::NoAscii);
    }
    let i = l.iter().position(|&b| b == b' ').ok_or(Falla::Verbo)?;
    let (verbo, resto) = (&l[..i], &l[i + 1..]);
    match verbo {
        b"JUEGO" => {
            let [id, tienda, titulo] = campos::<3>(resto).ok_or(Falla::Campos)?;
            if !es_id(id) {
                return Err(Falla::Id);
            }
            let tienda = Tienda::de(tienda).ok_or(Falla::Tienda)?;
            if titulo.is_empty() || titulo.len() > TITULO_MAX || titulo[0] == b' ' {
                return Err(Falla::Titulo);
            }
            Ok(Linea::Juego { id: t(id), tienda, titulo: t(titulo) })
        }
        b"FICHERO" => {
            let [id, bytes, s, nombre] = campos::<4>(resto).ok_or(Falla::Campos)?;
            if !es_id(id) {
                return Err(Falla::Id);
            }
            let bytes = numero(bytes).ok_or(Falla::Bytes)?;
            let suma = suma(s).ok_or(Falla::Suma)?;
            if !es_nombre(nombre) {
                return Err(Falla::Nombre);
            }
            Ok(Linea::Fichero { id: t(id), nombre: t(nombre), bytes, suma })
        }
        b"MOTOR" => {
            let [id, motor] = campos::<2>(resto).ok_or(Falla::Campos)?;
            if !es_id(id) {
                return Err(Falla::Id);
            }
            Ok(Linea::Motor { id: t(id), motor: Motor::de(motor).ok_or(Falla::Motor)? })
        }
        _ => Err(Falla::Verbo),
    }
}

/// Como se juega un juego en BMO-X.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camino {
    Nativo(Motor),
    ProtonX,
    /// Sin motor ni `.exe` conocidos todavia.
    Pendiente,
}

/// Un juego de la ludoteca.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Juego {
    pub id: String,
    pub tienda: Tienda,
    pub titulo: String,
    pub motor: Option<Motor>,
}

/// Un fichero que se puede traer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fichero {
    pub id: String,
    pub nombre: String,
    pub bytes: u64,
    pub suma: [u8; 32],
}

/// **La ludoteca entera**: lo que dijeron las lineas, ya juzgado.
#[derive(Clone, Debug, Default)]
pub struct Ludoteca {
    pub juegos: Vec<Juego>,
    pub ficheros: Vec<Fichero>,
}

impl Ludoteca {
    /// **Carga un texto de lineas.** Las lineas vacias y las que empiezan
    /// por `#` no cuentan. La primera que no vale para todo: `(numero, falla)`,
    /// contando desde 1. Una ludoteca a medias no se da por buena.
    pub fn cargar(texto: &[u8]) -> Result<Self, (usize, Falla)> {
        let mut l = Ludoteca::default();
        for (n, linea) in texto.split(|&b| b == b'\n').enumerate() {
            let linea = linea.strip_suffix(b"\r").unwrap_or(linea);
            if linea.is_empty() || linea[0] == b'#' {
                continue;
            }
            l.meter(leer(linea).map_err(|f| (n + 1, f))?).map_err(|f| (n + 1, f))?;
        }
        Ok(l)
    }

    /// Mete una linea ya leida, con las reglas de la ludoteca entera.
    pub fn meter(&mut self, linea: Linea<'_>) -> Result<(), Falla> {
        match linea {
            Linea::Juego { id, tienda, titulo } => {
                if self.juego(id).is_some() {
                    return Err(Falla::Repetido);
                }
                if self.juegos.len() >= JUEGOS_MAX {
                    return Err(Falla::Lleno);
                }
                self.juegos.push(Juego { id: id.into(), tienda, titulo: titulo.into(), motor: None });
            }
            Linea::Fichero { id, nombre, bytes, suma } => {
                if self.juego(id).is_none() {
                    return Err(Falla::Huerfano);
                }
                if self.ficheros.len() >= FICHEROS_MAX {
                    return Err(Falla::Lleno);
                }
                self.ficheros.push(Fichero { id: id.into(), nombre: nombre.into(), bytes, suma });
            }
            Linea::Motor { id, motor } => {
                let j = self.juegos.iter_mut().find(|j| j.id == id).ok_or(Falla::Huerfano)?;
                j.motor = Some(motor);
            }
        }
        Ok(())
    }

    pub fn juego(&self, id: &str) -> Option<&Juego> {
        self.juegos.iter().find(|j| j.id == id)
    }

    /// Los ficheros de un juego.
    pub fn ficheros_de<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Fichero> + 'a {
        self.ficheros.iter().filter(move |f| f.id == id)
    }

    /// **Como se juega**: la linea MOTOR manda; si no, un dato de motor entre
    /// sus ficheros; si no, un `.exe` es PROTON-X; si no, pendiente.
    pub fn camino(&self, id: &str) -> Option<Camino> {
        let j = self.juego(id)?;
        if let Some(m) = j.motor.or_else(|| self.ficheros_de(id).find_map(|f| Motor::por_dato(&f.nombre))) {
            return Some(Camino::Nativo(m));
        }
        if self.ficheros_de(id).any(|f| termina_en_ext(&f.nombre, ".exe")) {
            return Some(Camino::ProtonX);
        }
        Some(Camino::Pendiente)
    }
}

fn termina_en_ext(nombre: &str, ext: &str) -> bool {
    let (n, e) = (nombre.as_bytes(), ext.as_bytes());
    n.len() > e.len() && n[n.len() - e.len()..].eq_ignore_ascii_case(e)
}

/// Lo que dice el juez de un fichero traido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Veredicto {
    Bien,
    /// Llegaron mas o menos bytes de los que dijo su linea.
    Largo,
    /// El largo cuadra; la suma, no.
    Suma,
}

/// **El juez de la suma**, por trozos: lo que llega por la red entra a
/// pedazos y no hace falta tenerlo entero.
pub struct Juez {
    sha: bmo_cripto::Sha256,
    contados: u64,
    bytes: u64,
    suma: [u8; 32],
}

impl Juez {
    pub fn para(f: &Fichero) -> Self {
        Juez { sha: bmo_cripto::Sha256::nuevo(), contados: 0, bytes: f.bytes, suma: f.suma }
    }

    /// Mete un trozo. Devuelve `false` en cuanto se pasa del largo: no hace
    /// falta seguir trayendo lo que ya no va a entrar.
    pub fn mete(&mut self, trozo: &[u8]) -> bool {
        self.contados = self.contados.saturating_add(trozo.len() as u64);
        if self.contados > self.bytes {
            return false;
        }
        self.sha.mete(trozo);
        true
    }

    pub fn veredicto(self) -> Veredicto {
        if self.contados != self.bytes {
            return Veredicto::Largo;
        }
        if bmo_cripto::iguales(&self.sha.cierra(), &self.suma) {
            Veredicto::Bien
        } else {
            Veredicto::Suma
        }
    }
}

/// El juez de una vez, para lo que ya esta entero.
pub fn juzgar(f: &Fichero, datos: &[u8]) -> Veredicto {
    let mut j = Juez::para(f);
    j.mete(datos);
    j.veredicto()
}

#[cfg(test)]
mod pruebas;
