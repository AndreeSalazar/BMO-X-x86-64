//! **Quien es el proceso** (P4e, 27-09): su nombre de fichero, su linea de
//! ordenes y su entorno, como un CRT los pide al arrancar.
//!
//! ```text
//!    GetModuleFileNameW   "apps/juego.exe" del volumen -> "C:\apps\juego.exe"
//!                         (la vuelta de `ficheros::ruta`: la unidad es una)
//!    GetCommandLineW      "\"C:\apps\juego.exe\" lo que se escribio detras"
//!    el entorno           nombres sin mayusculas que cuenten, un bloque
//!                         "N=V\0...\0\0" ordenado, como Windows lo da
//! ```
//!
//! [`argumentos`] parte una linea como la parte el CRT de Microsoft (y la
//! `std` de Rust): el banco la usa para ver que lo que el `.exe` recibe es lo
//! que se escribio.
//!
//! El entorno de partida es corto y verdadero: `OS=Windows_NT` (el ABI que se
//! da), `PATH`, `TEMP` y `TMP` al directorio del `.exe` (el unico sitio que se
//! sabe escribible), `NUMBER_OF_PROCESSORS=1` (los hilos de la casa son M:1,
//! el mismo numero que `GetSystemInfo`), `PROCESSOR_ARCHITECTURE=AMD64`,
//! `USERPROFILE` al directorio del `.exe` (P4f4: su "casa" es su carpeta) y
//! `PROTON_X=1` para quien quiera saber donde esta. No hay `SystemRoot`: no
//! hay un Windows debajo.

use alloc::string::String;
use alloc::vec::Vec;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// **La ruta de Windows** de una ruta del volumen: `C:\` y barras al reves.
pub fn ruta_windows(vol: &str) -> String {
    // N2: lo de `d:` es del disco Personal, `D:\\` en Windows.
    let (mut s, vol) = match crate::ficheros::en_personal(vol) {
        Some(resto) => (String::from("D:\\"), resto),
        None => (String::from("C:\\"), vol),
    };
    s.push_str(&vol.trim_start_matches('/').replace('/', "\\"));
    s
}

/// **La linea de ordenes**: el `.exe` entre comillas (su ruta puede llevar
/// espacios) y lo que se escribio detras, tal cual.
pub fn linea(exe_windows: &str, resto: &str) -> String {
    let mut s = String::from("\"");
    s.push_str(exe_windows);
    s.push('"');
    let resto = resto.trim();
    if !resto.is_empty() {
        s.push(' ');
        s.push_str(resto);
    }
    s
}

/// **Partir una linea** como el CRT: el primero hasta la comilla que cierra (o
/// el primer espacio), sin escapes; los demas, con las reglas de las barras
/// (2n barras y comilla: n barras y la comilla abre o cierra; 2n+1: n y una
/// comilla de verdad) y `""` dentro de comillas es una comilla.
pub fn argumentos(l: &str) -> Vec<String> {
    let c: Vec<char> = l.chars().collect();
    let mut v = Vec::new();
    let mut i = 0;
    // El programa.
    let mut a = String::new();
    if c.first() == Some(&'"') {
        i = 1;
        while i < c.len() && c[i] != '"' {
            a.push(c[i]);
            i += 1;
        }
        i += 1;
    } else {
        while i < c.len() && c[i] != ' ' && c[i] != '\t' {
            a.push(c[i]);
            i += 1;
        }
    }
    v.push(a);
    loop {
        while i < c.len() && (c[i] == ' ' || c[i] == '\t') {
            i += 1;
        }
        if i >= c.len() {
            return v;
        }
        let mut a = String::new();
        let mut dentro = false;
        while i < c.len() && (dentro || (c[i] != ' ' && c[i] != '\t')) {
            if c[i] == '\\' {
                let n = c[i..].iter().take_while(|&&x| x == '\\').count();
                i += n;
                if c.get(i) == Some(&'"') {
                    a.extend(core::iter::repeat('\\').take(n / 2));
                    if n % 2 == 1 {
                        a.push('"');
                        i += 1;
                    }
                } else {
                    a.extend(core::iter::repeat('\\').take(n));
                }
            } else if c[i] == '"' {
                if dentro && c.get(i + 1) == Some(&'"') {
                    a.push('"');
                    i += 2;
                } else {
                    dentro = !dentro;
                    i += 1;
                }
            } else {
                a.push(c[i]);
                i += 1;
            }
        }
        v.push(a);
    }
}

fn mayus(c: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&c) {
        c - 32
    } else {
        c
    }
}

fn igual(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| mayus(x) == mayus(y))
}

/// El orden del bloque: por nombre, sin mayusculas que cuenten.
fn antes(a: &[u16], b: &[u16]) -> core::cmp::Ordering {
    a.iter().map(|&c| mayus(c)).cmp(b.iter().map(|&c| mayus(c)))
}

/// Un nombre que `SetEnvironmentVariableW` no acepta: vacio o con `=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NombreMalo;

/// **El entorno del proceso**, en UTF-16 como lo guarda Windows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entorno {
    v: Vec<(Vec<u16>, Vec<u16>)>,
}

impl Entorno {
    pub const fn vacio() -> Self {
        Entorno { v: Vec::new() }
    }

    /// El de partida, con `dir_exe` como ruta de Windows (`C:\apps`).
    pub fn de_bmo(dir_exe: &str) -> Self {
        let mut e = Entorno::vacio();
        for (n, val) in [
            ("OS", "Windows_NT"),
            ("PATH", dir_exe),
            ("TEMP", dir_exe),
            ("TMP", dir_exe),
            ("NUMBER_OF_PROCESSORS", "1"),
            ("PROCESSOR_ARCHITECTURE", "AMD64"),
            ("PROTON_X", "1"),
            ("USERPROFILE", dir_exe),
        ] {
            let _ = e.poner(&w(n), Some(&w(val)));
        }
        // Tanda 14a: las dos de AppData, bajo el perfil, como en cualquier
        // Windows (SHGetFolderPathW las contesta de aqui).
        let base = dir_exe.trim_end_matches('\\');
        let _ = e.poner(&w("APPDATA"), Some(&w(&alloc::format!("{base}\\AppData\\Roaming"))));
        let _ = e.poner(&w("LOCALAPPDATA"), Some(&w(&alloc::format!("{base}\\AppData\\Local"))));
        e
    }

    /// El valor de `nombre` (sin el 0 del final).
    pub fn leer(&self, nombre: &[u16]) -> Option<&[u16]> {
        self.v.iter().find(|(n, _)| igual(n, nombre)).map(|(_, v)| v.as_slice())
    }

    /// Poner (o con `None`, quitar) `nombre`. Guarda el nombre como se
    /// escribio la primera vez, como Windows.
    pub fn poner(&mut self, nombre: &[u16], valor: Option<&[u16]>) -> Result<(), NombreMalo> {
        if nombre.is_empty() || nombre.contains(&(b'=' as u16)) {
            return Err(NombreMalo);
        }
        let i = self.v.iter().position(|(n, _)| igual(n, nombre));
        match (i, valor) {
            (Some(i), Some(v)) => self.v[i].1 = v.to_vec(),
            (Some(i), None) => {
                self.v.remove(i);
            }
            (None, Some(v)) => {
                let en = self.v.partition_point(|(n, _)| antes(n, nombre).is_lt());
                self.v.insert(en, (nombre.to_vec(), v.to_vec()));
            }
            // Quitar lo que no esta: Windows dice que si.
            (None, None) => {}
        }
        Ok(())
    }

    /// **El bloque** de `GetEnvironmentStringsW`: `N=V\0` cada uno y un `\0`
    /// mas; vacio, dos ceros.
    pub fn bloque(&self) -> Vec<u16> {
        let mut b = Vec::new();
        for (n, v) in &self.v {
            b.extend_from_slice(n);
            b.push(b'=' as u16);
            b.extend_from_slice(v);
            b.push(0);
        }
        if b.is_empty() {
            b.push(0);
        }
        b.push(0);
        b
    }
}
