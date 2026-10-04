//! **Las DLL del juego que se cargan EN VIVO, cargadas al arrancar** (04-10,
//! N4.4).
//!
//! [carril]  AMARILLO  decide que codigo ajeno entra en la imagen
//! [cuesta]  DATO      una DLL de mas corre su DllMain sin que nadie la pida
//! [riesgo]  ESPEJO    lo que haria `LoadLibrary` en Windows, pero antes: la
//!                     imagen se declara al kernel de una vez
//! [consumo] NADA      una vez al arrancar: listar la carpeta y leer las
//!                     secciones de datos del `.exe`
//!
//! Diez corridas murieron saltando a 0: Cyberpunk carga
//! `amd_fidelityfx_dx12` (FSR 3.1) con LoadLibrary, guarda
//! `GetProcAddress(h, "ffxDispatch")` y lo llama sin mirar. El lector del
//! nulo (`el_nulo.rs`) lo encontro. Aqui se arregla de raiz:
//!
//! ```text
//!    1  las .dll de la carpeta del .exe que no son de la casa ni estan ya
//!    2  de esas, las que el .exe NOMBRA en sus datos (bmo_proton_x::en_vivo)
//!    3  de esas, las que se pueden resolver ENTERAS: cada importacion de la
//!       casa la tiene la casa, y cada DLL que piden esta en la carpeta. Una
//!       que no, se dice con lo que le falta y NO se carga: una DLL opcional
//!       no puede impedir que el juego arranque
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::Permiso;
use bmo_userland as bmo;

use crate::di;

fn es_dll(n: &str) -> bool {
    n.len() > 4 && n[n.len() - 4..].eq_ignore_ascii_case(".dll")
}

/// Las `.dll` de la carpeta `dir` (con su `/` final), menos las de `fuera`.
fn de_la_carpeta(dir: &[u8], fuera: &[String]) -> Vec<String> {
    let sin = dir.strip_suffix(b"/").unwrap_or(dir);
    let Ok(d) = bmo::Directorio::open(sin) else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let mut n = [0u8; 256];
    while let Some((k, carpeta, _)) = d.siguiente_largo(&mut n) {
        let nombre = String::from_utf8_lossy(&n[..k]).into_owned();
        if carpeta
            || !es_dll(&nombre)
            || bmo_proton_x_casa::modulos::es_de_la_casa(&nombre)
            || fuera.iter().any(|x| x.eq_ignore_ascii_case(&nombre))
        {
            continue;
        }
        v.push(nombre);
    }
    v
}

/// Lo que le falta a `dll` (y a las DLL del juego que pide) para resolverse
/// entera. Vacio = se puede cargar.
fn le_falta(dll: &str, dir: &[u8], vistos: &mut Vec<String>) -> Vec<String> {
    if vistos.iter().any(|v| v.eq_ignore_ascii_case(dll)) {
        return Vec::new();
    }
    vistos.push(String::from(dll));
    let Ok(m) = crate::mirar(&crate::cargador::junto(dir, dll)) else {
        return alloc::vec![format!("{dll} no se lee")];
    };
    let mut falta = Vec::new();
    for i in &m.imps {
        if bmo_proton_x_casa::modulos::es_de_la_casa(&i.dll) {
            if !bmo_proton_x_casa::la_casa_tiene(&i.dll, &i.funcion) {
                falta.push(format!("{}!{}", i.dll, i.funcion));
            }
        } else if bmo::Archivo::reflejar(&crate::cargador::junto(dir, &i.dll)).is_err() {
            if !falta.iter().any(|f| f == &i.dll) {
                falta.push(i.dll.clone());
            }
        } else {
            falta.extend(le_falta(&i.dll, dir, vistos));
        }
    }
    falta
}

/// **Las DLL que el `.exe` de `ruta` (ya mirado: `pe`) carga en vivo** y
/// se pueden cargar, en el orden de la carpeta. `ya`: las que ya entran.
pub(crate) fn pedidas_en_vivo(ruta: &[u8], pe: &bmo_proton_x::Pe, dir: &[u8], ya: &[String]) -> Vec<String> {
    let cand = de_la_carpeta(dir, ya);
    if cand.is_empty() {
        return Vec::new();
    }
    let Ok(a) = bmo::Archivo::reflejar(ruta) else {
        return Vec::new();
    };
    let mut si = alloc::vec![false; cand.len()];
    for s in pe.secciones.iter().filter(|s| s.permiso() != Permiso::Codigo && s.tam_en_fichero > 0) {
        let r = crate::con_seccion(&a, s, |trozo| {
            Ok(bmo_proton_x::en_vivo::nombrados(trozo, &cand, &mut || bmo::yield_screen()))
        });
        if let Ok(v) = r {
            for (k, x) in v.into_iter().enumerate() {
                si[k] |= x;
            }
        }
    }
    let mut v = Vec::new();
    for (d, _) in cand.iter().zip(&si).filter(|(_, s)| **s) {
        let mut vistos: Vec<String> = ya.to_vec();
        let falta = le_falta(d, dir, &mut vistos);
        if falta.is_empty() {
            di(&format!("PROTON-X: en vivo: {d} (el .exe la nombra y esta junto a el): se carga con las demas\n"));
            v.push(d.clone());
        } else {
            let mut t = format!("PROTON-X: en vivo: {d} (el .exe la nombra) NO se carga: le faltan {}:", falta.len());
            for f in falta.iter().take(12) {
                t.push(' ');
                t.push_str(f);
            }
            if falta.len() > 12 {
                t.push_str(" ...");
            }
            t.push('\n');
            di(&t);
        }
    }
    v
}
