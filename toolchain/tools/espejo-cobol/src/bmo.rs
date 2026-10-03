//! **EL LADO DE BMO**: los `COPY` expandidos, el emisor de x86-64 de verdad y
//! el emulador. Como el ESPEJO de C, un fallo aqui es un DATO con su FASE.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

/// Instrucciones como mucho: un caso del espejo es chico.
const LIMITE: usize = 50_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    Copy,
    Compila,
    Revienta,
    Emula,
}

impl Fase {
    pub fn nombre(self) -> &'static str {
        match self {
            Fase::Copy => "un COPY no se expande",
            Fase::Compila => "BMO no lo compila",
            Fase::Revienta => "el compilador revienta",
            Fase::Emula => "no corre en el emulador",
        }
    }
}

/// **Compila y ejecuta** con BMO COBOL. `copias`: donde buscar copybooks.
/// `entrada`: lo que lee `ACCEPT`. `datos`: ficheros `(ruta, bytes)` para
/// `OPEN INPUT`.
pub fn correr(fuente: &str, copias: &[PathBuf], entrada: &str, datos: &[(String, Vec<u8>)]) -> Result<String, (Fase, String)> {
    let mut buscar = |n: &str| {
        let nombres = [format!("{n}.cpy"), format!("{}.cpy", n.to_ascii_lowercase()), format!("{n}.CPY")];
        copias.iter().flat_map(|c| nombres.iter().map(move |x| c.join(x))).find_map(|r| std::fs::read_to_string(r).ok())
    };
    let fuente = bmo_cobol_front::copia::expandir(fuente, &mut buscar).map_err(|e| (Fase::Copy, format!("linea {}: {}", e.line, e.message)))?;
    let bex = catch_unwind(AssertUnwindSafe(|| bmo_cobol_x86_64::compile_source_to_bex(&fuente)))
        .map_err(|e| (Fase::Revienta, panico(e)))?
        .map_err(|e| (Fase::Compila, format!("linea {}: {}", e.line, e.message)))?;
    let mut m = bmo_lower::emu::cargar_bex(&bex).map_err(|e| (Fase::Emula, e))?;
    if !entrada.is_empty() {
        m.poner_entrada(entrada);
    }
    for (ruta, bytes) in datos {
        m.poner_archivo(ruta, bytes);
    }
    let m = catch_unwind(AssertUnwindSafe(move || bmo_lower::emu::run(m, LIMITE))).map_err(|e| (Fase::Emula, panico(e)))?;
    if !m.exited {
        return Err((Fase::Emula, "no llego a STOP RUN (o pidio mas entrada de la que habia)".into()));
    }
    Ok(m.console.clone())
}

fn panico(e: Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panico sin mensaje".into())
}

/// Los ficheros de una carpeta de datos, con la ruta que el programa usa
/// (`datos/x.txt`).
pub fn datos_de(carpeta: &Path, prefijo: &str) -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    if let Ok(d) = std::fs::read_dir(carpeta) {
        for e in d.flatten() {
            let p = e.path();
            if p.is_file() {
                if let (Some(n), Ok(b)) = (p.file_name().and_then(|n| n.to_str()), std::fs::read(&p)) {
                    v.push((format!("{prefijo}/{n}"), b));
                }
            }
        }
    }
    v
}
