//! **EL C DE TERCEROS, PELADO CAPA A CAPA.**
//!
//! Un juego de verdad (vkQuake, Quake, OpenBW...) no se puede EJECUTAR aqui:
//! le faltan SDL, Vulkan y medio sistema. Pero se puede COMPILAR fichero a
//! fichero, y el primer error de cada uno dice que le falta a BMO. Es lo que el
//! 25-09 salio a mano con vkQuake 0.50; esto lo hace solo.
//!
//! # Como se pela
//!
//! La primera capa casi siempre son cabeceras del sistema (`sys/types.h`,
//! `SDL.h`). Esas no son del LENGUAJE, asi que se tapan: se trabaja sobre una
//! COPIA de la carpeta y cada cabecera que falta se crea VACIA en ella, y se
//! vuelve a compilar. Lo que queda debajo ya es el lenguaje, o la API que la
//! cabecera traia (un tipo sin declarar) -- y se dice cual de las dos.
//!
//! Los errores se agrupan por su FORMA: sin numeros, sin nombres propios. Diez
//! ficheros parados por `la medida de un array tiene que ser constante` son una
//! sola fila con un 10, y esa fila es lo que hay que arreglar primero.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::bmo::{self, Fase, Lengua};

/// Lo que dijo un fichero.
pub struct Fichero {
    pub nombre: String,
    pub resultado: Result<(), (Fase, String)>,
    /// Cabeceras que hubo que tapar para llegar hasta aqui.
    pub tapadas: usize,
}

/// **Pela `dir`**: cada `.c` (o `.cpp`) compilado con cabeceras tapadas.
pub fn pelar(dir: &Path, lengua: Lengua, trabajo: &Path) -> Result<Vec<Fichero>, String> {
    let copia = trabajo.join("corpus");
    let _ = std::fs::remove_dir_all(&copia);
    copiar(dir, &copia).map_err(|e| format!("no se pudo copiar {}: {e}", dir.display()))?;
    let ext = match lengua {
        Lengua::C => "c",
        Lengua::Cpp => "cpp",
    };
    let mut fuentes: Vec<PathBuf> = std::fs::read_dir(&copia)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some(ext))
        .collect();
    fuentes.sort();
    let mut out = Vec::new();
    for f in fuentes {
        let nombre = f.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let texto = match std::fs::read(&f) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                out.push(Fichero { nombre, resultado: Err((Fase::Compila, e.to_string())), tapadas: 0 });
                continue;
            }
        };
        let mut tapadas = 0;
        let resultado = loop {
            match bmo::compilar(lengua, &texto, &f) {
                Ok(_) => break Ok(()),
                Err((Fase::Compila, m)) if tapadas < 80 => match falta(&m) {
                    Some(cab) => {
                        let tapa = copia.join(Path::new(&cab).file_name().unwrap_or_default());
                        if tapa.exists() {
                            break Err((Fase::Compila, m));
                        }
                        let _ = std::fs::write(&tapa, b"");
                        tapadas += 1;
                    }
                    None => break Err((Fase::Compila, m)),
                },
                Err(e) => break Err(e),
            }
        };
        out.push(Fichero { nombre, resultado, tapadas });
    }
    Ok(out)
}

/// La cabecera que falta, si el error es ese.
fn falta(m: &str) -> Option<String> {
    let i = m.find("file not found: ")?;
    let resto = &m[i + "file not found: ".len()..];
    Some(resto.split_whitespace().next()?.trim_matches(|c| c == '"' || c == '<' || c == '>').to_string())
}

/// **La forma de un error**: sin numeros ni nombres. Es la clave por la que se
/// agrupan.
pub fn forma(m: &str) -> String {
    let m = m.strip_prefix("linea ").map(|r| r.split_once(": ").map_or(r, |x| x.1)).unwrap_or(m);
    let mut out = String::new();
    let mut dentro: Option<char> = None;
    for c in m.chars() {
        match dentro {
            Some(fin) => {
                if c == fin {
                    dentro = None;
                    out.push('X');
                    out.push(c);
                }
            }
            None => match c {
                '`' | '\'' | '"' => {
                    dentro = Some(c);
                    out.push(c);
                }
                '0'..='9' => {
                    if !out.ends_with('N') {
                        out.push('N');
                    }
                }
                _ => out.push(c),
            },
        }
    }
    out.chars().take(120).collect()
}

/// **Las filas**: `(forma, cuantos, un ejemplo)`, de la que mas para a la que menos.
pub fn agrupar(fs: &[Fichero]) -> Vec<(String, usize, String)> {
    let mut g: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for f in fs {
        if let Err((fase, m)) = &f.resultado {
            let clave = format!("{}: {}", fase.nombre(), forma(m));
            let e = g.entry(clave).or_insert((0, f.nombre.clone()));
            e.0 += 1;
        }
    }
    let mut v: Vec<_> = g.into_iter().map(|(k, (n, ej))| (k, n, ej)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

fn copiar(de: &Path, a: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(a)?;
    for e in std::fs::read_dir(de)? {
        let e = e?;
        let p = e.path();
        if p.is_file() {
            std::fs::copy(&p, a.join(e.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_forma_quita_numeros_y_nombres() {
        assert_eq!(
            forma("linea 122: expected type, got Ident(\"size_t\")"),
            "expected type, got Ident(\"X\")"
        );
        assert_eq!(forma("#if: cannot evaluate '0 (0)'"), "#if: cannot evaluate 'X'");
        assert_eq!(forma("grupo 83 /7 no emitido"), "grupo N /N no emitido");
    }

    #[test]
    fn la_cabecera_que_falta_se_lee_del_error() {
        assert_eq!(falta("linea 32: #include: file not found: sys/types.h").as_deref(), Some("sys/types.h"));
        assert_eq!(falta("otra cosa"), None);
    }
}
