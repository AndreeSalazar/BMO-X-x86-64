//! **EL CATALOGO** -- de donde salen los juegos de la LUDOTECA.
//!
//! ```text
//!    1  ludoteca/ludoteca.txt   las lineas JUEGO / FICHERO / MOTOR (J0), de
//!                               ESTRATOS primero (Archivo::leer_de), si existe
//!    2  si no: lo que HAY       Cyberpunk en D: (su .exe) y DOOM en apps/:
//!                               se mira el disco, no se inventa nada
//! ```
//!
//! Y como se juega cada uno -- lo que JUGAR le pide al escritorio:
//!
//! ```text
//!    NATIVO (doom)   apps/doom.bex
//!    PROTON-X        personal diario <el .exe, en D:>
//!    PENDIENTE       nada: el boton lo dice
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use bmo_ludoteca::{Camino, Fichero, Juego, Ludoteca, Motor, Tienda};
use bmo_userland as bmo;

/// Lo mas que se lee del fichero de lineas.
const LINEAS_MAX: usize = 16 * 1024;

const CYBERPUNK: &[u8] = b"d:Cyberpunk 2077/bin/x64/Cyberpunk2077.exe";
const DOOM: &[u8] = b"apps/doom.bex";

/// De donde salio lo que se ve.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    Fichero,
    Discos,
}

pub struct Catalogo {
    pub l: Ludoteca,
    pub origen: Origen,
}

/// La medida de un fichero, si esta (sin traerlo: el kernel lo refleja).
fn medida(ruta: &[u8]) -> Option<u64> {
    let a = bmo::Archivo::reflejar(ruta).ok()?;
    let m = a.size();
    a.close();
    Some(m)
}

fn de_fichero() -> Option<Ludoteca> {
    let f = bmo::Archivo::leer_de(b"ludoteca/ludoteca.txt").ok()?;
    let n = (f.size() as usize).min(LINEAS_MAX);
    let mut buf = alloc::vec![0u8; n];
    let mut k = 0;
    while k < n {
        let r = f.read(&mut buf[k..]);
        if r == 0 {
            break;
        }
        k += r;
    }
    Ludoteca::cargar(&buf[..k]).ok()
}

fn de_los_discos() -> Ludoteca {
    let mut l = Ludoteca::default();
    if let Some(m) = medida(CYBERPUNK) {
        l.juegos.push(Juego { id: "cyberpunk2077".into(), tienda: Tienda::Gog, titulo: "Cyberpunk 2077".into(), motor: None });
        // La suma todavia no se sabe: el juez la pedira a la tienda (J2).
        l.ficheros.push(Fichero { id: "cyberpunk2077".into(), nombre: "Cyberpunk 2077/bin/x64/Cyberpunk2077.exe".into(), bytes: m, suma: [0; 32] });
    }
    if medida(DOOM).is_some() {
        l.juegos.push(Juego { id: "doom".into(), tienda: Tienda::Libre, titulo: "DOOM".into(), motor: Some(Motor::Doom) });
    }
    l
}

impl Catalogo {
    pub fn abrir() -> Catalogo {
        match de_fichero() {
            Some(l) => Catalogo { l, origen: Origen::Fichero },
            None => Catalogo { l: de_los_discos(), origen: Origen::Discos },
        }
    }

    /// Los juegos de `tienda` (`None`: todos), en el orden de las lineas.
    pub fn de(&self, tienda: Option<Tienda>) -> Vec<usize> {
        (0..self.l.juegos.len()).filter(|&i| tienda.map_or(true, |t| self.l.juegos[i].tienda == t)).collect()
    }

    pub fn camino(&self, i: usize) -> Camino {
        self.l.camino(&self.l.juegos[i].id).unwrap_or(Camino::Pendiente)
    }

    /// **Lo que JUGAR le pide al escritorio**, o `None` si no hay como.
    pub fn orden(&self, i: usize) -> Option<String> {
        let j = &self.l.juegos[i];
        match self.camino(i) {
            Camino::Nativo(Motor::Doom) => Some(String::from_utf8_lossy(DOOM).into_owned()),
            Camino::Nativo(_) => None,
            Camino::ProtonX => {
                let exe = self.l.ficheros_de(&j.id).find(|f| f.nombre.to_ascii_lowercase().ends_with(".exe"))?;
                Some(alloc::format!("personal diario {}", exe.nombre))
            }
            Camino::Pendiente => None,
        }
    }
}
