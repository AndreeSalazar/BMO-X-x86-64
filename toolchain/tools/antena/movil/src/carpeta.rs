//! **La carpeta**: lo unico que la antena sirve. Videos (`v1`, `v2`...) y
//! paginas ya maquetadas (`.lamina`, `p1`, `p2`...); un nombre de fichero no
//! viaja, viaja su id.

use std::fs;
use std::path::{Path, PathBuf};

pub const EXTENSIONES: [&str; 6] = [".mp4", ".mkv", ".webm", ".mov", ".mpg", ".avi"];

/// **El titulo como lo acepta BMO-X**: ASCII imprimible y 160 como mucho.
pub fn limpio(texto: &str) -> String {
    let t: String = texto.chars().map(|c| if (' '..='~').contains(&c) { c } else { '?' }).take(bmo_antena::TEXTO_MAX).collect();
    if t.is_empty() {
        "?".into()
    } else {
        t
    }
}

/// **La ruta real, si es un fichero NORMAL de DENTRO de la carpeta.** Un
/// enlace que lleva fuera, o un `..`, da `None`.
pub fn dentro(carpeta: &Path, nombre: &str) -> Option<PathBuf> {
    let raiz = fs::canonicalize(carpeta).ok()?;
    let ruta = fs::canonicalize(carpeta.join(nombre)).ok()?;
    (ruta.starts_with(&raiz) && ruta.is_file()).then_some(ruta)
}

fn termina(nombre: &str, en: &[&str]) -> bool {
    let n = nombre.to_ascii_lowercase();
    en.iter().any(|e| n.ends_with(e))
}

/// **`[(id, fichero)]`**, ordenado: los videos y luego las paginas, 64 como
/// mucho (`LISTA_MAX`).
pub fn catalogo(carpeta: &Path) -> Vec<(String, String)> {
    let mut nombres: Vec<String> = match fs::read_dir(carpeta) {
        Ok(d) => d.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| dentro(carpeta, n).is_some()).collect(),
        Err(_) => Vec::new(),
    };
    nombres.sort();
    let videos = nombres.iter().filter(|n| termina(n, &EXTENSIONES));
    let paginas = nombres.iter().filter(|n| termina(n, &[".lamina"]));
    let mut lista: Vec<(String, String)> = videos.enumerate().map(|(i, n)| (format!("v{}", i + 1), n.clone())).collect();
    lista.extend(paginas.enumerate().map(|(i, n)| (format!("p{}", i + 1), n.clone())));
    lista.truncate(bmo_antena::LISTA_MAX as usize);
    lista
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Una carpeta nueva y vacia para una prueba.
    pub(crate) fn carpeta_de(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bmo-antena-{}-{}", nombre, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn el_titulo_sale_limpio() {
        assert_eq!(limpio("Gato con botas.mp4"), "Gato con botas.mp4");
        assert_eq!(limpio("cancion\u{f1}a\n.mp4"), "cancion?a?.mp4");
        assert_eq!(limpio(""), "?");
        assert_eq!(limpio(&"x".repeat(500)).len(), bmo_antena::TEXTO_MAX);
    }

    #[test]
    fn el_catalogo_da_ids_y_no_nombres() {
        let d = carpeta_de("catalogo");
        for n in ["b.mp4", "a.MKV", "nota.txt", "pagina.lamina", "z.webm"] {
            fs::write(d.join(n), b"x").unwrap();
        }
        fs::create_dir(d.join("sub.mp4")).unwrap();
        let c = catalogo(&d);
        let ids: Vec<(&str, &str)> = c.iter().map(|(i, n)| (i.as_str(), n.as_str())).collect();
        assert_eq!(ids, [("v1", "a.MKV"), ("v2", "b.mp4"), ("v3", "z.webm"), ("p1", "pagina.lamina")]);
    }

    #[cfg(unix)]
    #[test]
    fn un_enlace_que_sale_de_la_carpeta_no_se_sirve() {
        let d = carpeta_de("enlace");
        let fuera = carpeta_de("enlace-fuera").join("secreto.mp4");
        fs::write(&fuera, b"x").unwrap();
        std::os::unix::fs::symlink(&fuera, d.join("trampa.mp4")).unwrap();
        assert!(dentro(&d, "trampa.mp4").is_none());
        assert!(dentro(&d, "../x").is_none());
        assert!(catalogo(&d).is_empty());
    }
}
