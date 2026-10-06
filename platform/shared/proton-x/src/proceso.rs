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

/// **El perfil de un juego de D: en ESTRATOS** (relevo 01-10, paso 3).
///
/// Un `.exe` del disco Personal (D:, solo lectura) no tiene donde escribir si
/// su "casa" es su carpeta: Cyberpunk se rendia en `redgalaxy::api::Init`
/// con APPDATA en `D:\...\bin\x64\AppData`. Su perfil de Windows vive en
/// ESTRATOS, uno por juego: `proton-x/<juego>/perfil` en el volumen, que es
/// `C:\proton-x\<juego>\perfil` para el `.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perfil {
    /// El nombre del juego: el de su `.exe` sin extension, en minusculas,
    /// con solo letras, cifras, `-` y `_`.
    pub juego: String,
    /// La ruta del perfil en el volumen (`proton-x/cyberpunk2077/perfil`).
    pub volumen: String,
    /// La capa de escritura sobre el disco Personal (`proton-x/<juego>/capa`).
    pub capa: String,
    /// Las marcas que ocultan nombres borrados del disco Personal.
    pub borrados: String,
    /// A9 (06-10): los `.bsf` de sus PSO, ya traducidos y comprobados
    /// (`proton-x/<juego>/bsf`): la 3060 no vuelve a traducir lo de ayer.
    pub bsf: String,
    /// A9b (06-10): los MAPAS de la CPU (lo que compilo de cada PSO, cifrado:
    /// `bmo_proton_x::cifra`), `proton-x/<juego>/mapas`: el DXIL de ayer no
    /// se vuelve a leer.
    pub mapas: String,
    /// La misma, como la ve el `.exe` (`C:\proton-x\cyberpunk2077\perfil`).
    pub windows: String,
}

/// Las carpetas de un perfil de Windows que se crean al arrancar, debajo de
/// la del perfil (como las tiene cualquier usuario de Windows).
pub const CARPETAS_DEL_PERFIL: [&str; 6] = [
    "AppData",
    "AppData/Local",
    "AppData/Roaming",
    "Documents",
    "Saved Games",
    "Temp",
];

/// **El perfil de `ruta_exe`** (la del volumen), si el `.exe` esta en D:. Un
/// `.exe` del volumen de BMO-X sigue con su carpeta (es escribible).
pub fn perfil_de(ruta_exe: &str) -> Option<Perfil> {
    crate::ficheros::en_personal(ruta_exe)?;
    let nombre = ruta_exe.rsplit(['/', '\\']).next()?;
    let tallo = nombre.rsplit_once('.').map_or(nombre, |(t, _)| t);
    let mut juego = String::new();
    for c in tallo.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            juego.push(c);
        } else if !juego.ends_with('-') {
            juego.push('-');
        }
    }
    let juego = String::from(juego.trim_matches('-'));
    if juego.is_empty() {
        return None;
    }
    let volumen = alloc::format!("proton-x/{juego}/perfil");
    let windows = alloc::format!("C:\\proton-x\\{juego}\\perfil");
    let capa = alloc::format!("proton-x/{juego}/capa");
    let borrados = alloc::format!("proton-x/{juego}/borrados");
    let bsf = alloc::format!("proton-x/{juego}/bsf");
    let mapas = alloc::format!("proton-x/{juego}/mapas");
    Some(Perfil {
        juego,
        volumen,
        capa,
        borrados,
        bsf,
        mapas,
        windows,
    })
}

impl Perfil {
    /// Las carpetas a crear en el volumen, cada una despues de su padre.
    pub fn carpetas(&self) -> Vec<String> {
        let mut v = alloc::vec![
            String::from("proton-x"),
            alloc::format!("proton-x/{}", self.juego),
            self.volumen.clone(),
            self.capa.clone(),
            self.borrados.clone(),
            self.bsf.clone(),
            self.mapas.clone(),
        ];
        v.extend(
            CARPETAS_DEL_PERFIL
                .iter()
                .map(|c| alloc::format!("{}/{c}", self.volumen)),
        );
        v
    }
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
        let _ = e.poner(
            &w("APPDATA"),
            Some(&w(&alloc::format!("{base}\\AppData\\Roaming"))),
        );
        let _ = e.poner(
            &w("LOCALAPPDATA"),
            Some(&w(&alloc::format!("{base}\\AppData\\Local"))),
        );
        e
    }

    /// **El de partida, con el perfil en otra parte** (`perfil`, ruta de
    /// Windows): USERPROFILE, APPDATA, LOCALAPPDATA, TEMP y TMP alli; PATH
    /// sigue en la carpeta del `.exe` (es donde estan sus DLL).
    pub fn de_bmo_con_perfil(dir_exe: &str, perfil: &str) -> Self {
        let mut e = Entorno::de_bmo(dir_exe);
        let p = perfil.trim_end_matches('\\');
        for (n, v) in [
            ("USERPROFILE", String::from(p)),
            ("APPDATA", alloc::format!("{p}\\AppData\\Roaming")),
            ("LOCALAPPDATA", alloc::format!("{p}\\AppData\\Local")),
            ("TEMP", alloc::format!("{p}\\Temp")),
            ("TMP", alloc::format!("{p}\\Temp")),
        ] {
            let _ = e.poner(&w(n), Some(&w(&v)));
        }
        e
    }

    /// El valor de `nombre` (sin el 0 del final).
    pub fn leer(&self, nombre: &[u16]) -> Option<&[u16]> {
        self.v
            .iter()
            .find(|(n, _)| igual(n, nombre))
            .map(|(_, v)| v.as_slice())
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
