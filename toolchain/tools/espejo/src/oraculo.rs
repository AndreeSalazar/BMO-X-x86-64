//! **EL ORACULO**: GCC y Clang, los dos si estan.
//!
//! Un caso solo JUZGA a BMO si los compiladores de fuera **coinciden** entre
//! ellos. Si GCC y Clang dan salidas distintas, el programa depende de algo
//! que el estandar no fija (un comportamiento indefinido, un orden de
//! evaluacion) y no es justo pedirle a BMO que acierte una de las dos: el caso
//! se aparta con su motivo, no cuenta como fallo.
//!
//! Con uno solo instalado, ese es el juez; sin ninguno, el espejo lo dice y
//! no inventa un veredicto.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::bmo::Lengua;

/// Un compilador de fuera.
#[derive(Clone, Debug)]
pub struct Externo {
    pub nombre: &'static str,
    exe: PathBuf,
}

/// Lo que dijo uno de fuera.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Veredicto {
    Salida { texto: String },
    NoCompila(String),
    NoTermina,
}

/// **Los que hay** para esta lengua, en orden: GCC y despues Clang.
pub fn buscar(lengua: Lengua) -> Vec<Externo> {
    let nombres: &[&'static str] = match lengua {
        Lengua::C => &["gcc", "clang"],
        Lengua::Cpp => &["g++", "clang++"],
    };
    nombres
        .iter()
        .filter_map(|n| donde(n).map(|exe| Externo { nombre: n, exe }))
        .collect()
}

/// El ejecutable en el PATH (con `.exe` en Windows).
fn donde(nombre: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for candidato in [nombre.to_string(), format!("{nombre}.exe")] {
            let p = dir.join(&candidato);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

impl Externo {
    /// **Compila `fuente` y la ejecuta.** `tmp` es una carpeta de trabajo.
    ///
    /// Sin optimizar, y con UN aviso hecho error (los demas se ignoran): se compara lo que el programa HACE, y `-O0`
    /// es lo que mas se parece a como emite BMO. `-fno-builtin` para que un
    /// `printf("x\n")` no se vuelva `puts` a escondidas -- da igual para la
    /// salida, pero deja el binario mas cerca del programa escrito.
    pub fn correr(&self, lengua: Lengua, fuente: &Path, tmp: &Path) -> Veredicto {
        let exe = tmp.join(format!("caso_{}{}", self.nombre.replace('+', "p"), std::env::consts::EXE_SUFFIX));
        let _ = std::fs::remove_file(&exe);
        let estandar = match lengua {
            Lengua::C => "-std=c11",
            Lengua::Cpp => "-std=c++17",
        };
        let r = Command::new(&self.exe)
            // `-Werror=return-type`: una funcion que no devuelve nada y se usa
            // es comportamiento indefinido, y el reductor la fabrica al quitar
            // lineas. Que el oraculo la rechace deja fuera ese atajo.
            .args([estandar, "-O0", "-Werror=return-type", "-fno-builtin", "-o"])
            .arg(&exe)
            .arg(fuente)
            .arg("-lm")
            .stdin(Stdio::null())
            .output();
        let r = match r {
            Ok(r) => r,
            Err(e) => return Veredicto::NoCompila(format!("no se pudo lanzar {}: {e}", self.nombre)),
        };
        if !r.status.success() {
            let err = String::from_utf8_lossy(&r.stderr);
            let linea = err.lines().find(|l| l.contains("error")).unwrap_or("error").trim();
            return Veredicto::NoCompila(linea.chars().take(160).collect());
        }
        ejecutar(&exe)
    }
}

/// Ejecuta con un reloj: un caso que no acaba en 10 s no acaba.
fn ejecutar(exe: &Path) -> Veredicto {
    let mut hijo = match Command::new(exe).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(h) => h,
        Err(e) => return Veredicto::NoCompila(format!("no arranco: {e}")),
    };
    let desde = Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if desde.elapsed() > Duration::from_secs(10) => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return Veredicto::NoTermina;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => return Veredicto::NoTermina,
        }
    }
    let salida = match hijo.wait_with_output() {
        Ok(o) => o,
        Err(_) => return Veredicto::NoTermina,
    };
    Veredicto::Salida { texto: String::from_utf8_lossy(&salida.stdout).into_owned() }
}
