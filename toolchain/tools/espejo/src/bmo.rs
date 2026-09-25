//! **EL LADO DE BMO**: compilar con los emisores de verdad y ejecutar en el
//! emulador de x86-64. El mismo camino que el metro (`toolchain/tools/metro`),
//! con una diferencia: aqui un fallo no es un error del programa, es un DATO.
//!
//! Cada fallo dice en que FASE paso, porque cada fase manda a mirar a un sitio:
//!
//! ```text
//!    Compila   el frontend o el emisor dijeron que no    -> toolchain/lang/*
//!    Enlaza    el enlazador no junto el objeto           -> tools/bmo-enlazar
//!    Revienta  el compilador ENTRO EN PANICO             -> un bug, no un "aun no"
//!    Emula     el emulador no sabe una instruccion, o
//!              el programa no termina                    -> forge/bmo-lower/emu
//! ```

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

/// Instrucciones como mucho. Un caso del espejo es chico: si no acaba, algo
/// da vueltas donde no debe.
const LIMITE: usize = 20_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fase {
    Compila,
    Enlaza,
    Revienta,
    Emula,
}

impl Fase {
    pub fn nombre(self) -> &'static str {
        match self {
            Fase::Compila => "no compila",
            Fase::Enlaza => "no enlaza",
            Fase::Revienta => "el compilador revienta",
            Fase::Emula => "no corre en el emulador",
        }
    }
}

/// Lo que salio de un programa: lo que imprimio y cuantas instrucciones costo.
///
/// Sin codigo de salida: en BMO-X `EXIT` no lleva argumento
/// (`bmo_lower::task::exit`), asi que lo que devuelve `main` no viaja.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Salida {
    pub texto: String,
    pub pasos: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lengua {
    C,
    Cpp,
}

/// **Compila y ejecuta** `fuente` con BMO. `ruta` es donde vive (para los
/// `#include` de al lado).
pub fn correr(lengua: Lengua, fuente: &str, ruta: &Path) -> Result<Salida, (Fase, String)> {
    let bex = compilar(lengua, fuente, ruta)?;
    let maquina = bmo_lower::emu::cargar_bex(&bex).map_err(|e| (Fase::Enlaza, e))?;
    let m = catch_unwind(AssertUnwindSafe(move || bmo_lower::emu::run(maquina, LIMITE))).map_err(|e| {
        (Fase::Emula, texto_panico(e).unwrap_or_else(|| "el emulador se paro".into()))
    })?;
    if !m.exited {
        return Err((Fase::Emula, "se acabo el codigo sin llegar a EXIT".into()));
    }
    Ok(Salida { texto: m.console.clone(), pasos: m.pasos })
}

/// Solo compilar: para el corpus de terceros, donde no hay nada que ejecutar.
pub fn compilar(lengua: Lengua, fuente: &str, ruta: &Path) -> Result<Vec<u8>, (Fase, String)> {
    let nombre = ruta.file_name().and_then(|n| n.to_str()).unwrap_or("unidad").to_string();
    let objeto = catch_unwind(AssertUnwindSafe(|| match lengua {
        Lengua::C => bmo_c_x86_64::compile_object_with_preprocessor(
            fuente,
            ruta,
            bmo_c_x86_64::CStandard::C11,
            bmo_c_x86_64::Libc::Copia,
        )
        .map_err(|e| format!("linea {}: {}", e.line, e.message)),
        Lengua::Cpp => bmo_cpp_x86_64::compilar_fichero(fuente, ruta, true)
            .map_err(|e| format!("linea {}: {}", e.line, e.message)),
    }))
    .map_err(|e| (Fase::Revienta, texto_panico(e).unwrap_or_else(|| "panico sin mensaje".into())))?
    .map_err(|e| (Fase::Compila, e))?;
    catch_unwind(AssertUnwindSafe(|| bmo_enlazar::enlazar(&[(nombre, objeto)])))
        .map_err(|e| (Fase::Revienta, texto_panico(e).unwrap_or_else(|| "el enlazador revento".into())))?
        .map_err(|e| (Fase::Enlaza, format!("{e:?}")))
}

fn texto_panico(e: Box<dyn std::any::Any + Send>) -> Option<String> {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
}
