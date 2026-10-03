//! **EL JUEZ DE FUERA**: GnuCOBOL (`cobc`), si esta. Se compila con `-x` (un
//! ejecutable), los copybooks por `-I`, y se ejecuta en una carpeta de
//! trabajo con los datos al lado, con 10 s de reloj.
//!
//! Sin `cobc` el espejo lo dice y NO inventa un veredicto.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Veredicto {
    Salida(String),
    /// GnuCOBOL no lo acepta: la primera linea de error.
    NoCompila(String),
    NoTermina,
    Revienta,
}

/// `cobc` en el PATH (con `.exe` en Windows).
pub fn buscar() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).flat_map(|d| [d.join("cobc"), d.join("cobc.exe")]).find(|p| p.is_file())
}

/// **Compila y ejecuta** `fuente` con GnuCOBOL, en `tmp`.
pub fn correr(cobc: &Path, fuente: &Path, copias: &[PathBuf], entrada: &str, tmp: &Path) -> Veredicto {
    let exe = tmp.join(format!("caso{}", std::env::consts::EXE_SUFFIX));
    let _ = std::fs::remove_file(&exe);
    let mut c = Command::new(cobc);
    c.arg("-x").arg("-o").arg(&exe);
    for d in copias {
        c.arg("-I").arg(d);
    }
    let r = match c.arg(fuente).stdin(Stdio::null()).output() {
        Ok(r) => r,
        Err(e) => return Veredicto::NoCompila(format!("no se pudo lanzar cobc: {e}")),
    };
    if !r.status.success() {
        let err = String::from_utf8_lossy(&r.stderr);
        let linea = err.lines().find(|l| l.contains("error")).unwrap_or("error").trim();
        // La ruta entera no dice nada: desde el nombre del fichero.
        let linea = linea.rsplit('/').next().unwrap_or(linea);
        return Veredicto::NoCompila(linea.chars().take(160).collect());
    }
    let mut hijo = match Command::new(&exe).current_dir(tmp).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(h) => h,
        Err(e) => return Veredicto::NoCompila(format!("no arranco: {e}")),
    };
    if let Some(mut si) = hijo.stdin.take() {
        let _ = si.write_all(entrada.as_bytes());
    }
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
    match hijo.wait_with_output() {
        Ok(o) if o.status.code().is_some() => Veredicto::Salida(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(_) => Veredicto::Revienta,
        Err(_) => Veredicto::NoTermina,
    }
}
