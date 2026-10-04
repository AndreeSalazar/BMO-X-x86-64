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
//!    3  de esas, las que tienen en la carpeta cada DLL del juego que piden.
//!       Lo que piden a la CASA y la casa no tiene no las para (04-10, la
//!       duodecima corrida: la de FSR se quedaba fuera por eso y el salto a 0
//!       seguia): se les da una TRAMPA con nombre, que dice si la llaman
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::Permiso;
use bmo_userland as bmo;

use crate::di_y_diario as di;

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

/// Lo que le falta a `dll` (y a las DLL del juego que pide): `(DLL del juego
/// que no estan, funciones de la casa que iran con trampa)`.
fn le_falta(dll: &str, dir: &[u8], vistos: &mut Vec<String>) -> (Vec<String>, Vec<String>) {
    let (mut dlls, mut funciones) = (Vec::new(), Vec::new());
    if vistos.iter().any(|v| v.eq_ignore_ascii_case(dll)) {
        return (dlls, funciones);
    }
    vistos.push(String::from(dll));
    let Ok(m) = crate::mirar(&crate::cargador::junto(dir, dll)) else {
        dlls.push(format!("{dll} (no se lee)"));
        return (dlls, funciones);
    };
    for i in &m.imps {
        if bmo_proton_x_casa::modulos::es_de_la_casa(&i.dll) {
            if !bmo_proton_x_casa::la_casa_tiene(&i.dll, &i.funcion) {
                funciones.push(format!("{}!{}", i.dll, i.funcion));
            }
        } else if bmo::Archivo::reflejar(&crate::cargador::junto(dir, &i.dll)).is_err() {
            if !dlls.iter().any(|f| f == &i.dll) {
                dlls.push(i.dll.clone());
            }
        } else {
            let (d, f) = le_falta(&i.dll, dir, vistos);
            dlls.extend(d);
            funciones.extend(f);
        }
    }
    (dlls, funciones)
}

fn lista(t: &mut String, v: &[String]) {
    lista_hasta(t, v, 12);
}

fn lista_hasta(t: &mut String, v: &[String], tope: usize) {
    for f in v.iter().take(tope) {
        t.push(' ');
        t.push_str(f);
    }
    if v.len() > tope {
        t.push_str(" ...");
    }
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
    // Las que estan y NO se nombran tambien se dicen: si la que se busca
    // esta aqui, el .exe no la nombra como se esperaba.
    let no: Vec<String> = cand.iter().zip(&si).filter(|(_, s)| !**s).map(|(d, _)| d.clone()).collect();
    if !no.is_empty() {
        let mut t = format!("PROTON-X: en vivo: {} .dll junto al .exe que el .exe NO nombra (no se cargan):", no.len());
        lista_hasta(&mut t, &no, 64);
        t.push('\n');
        di(&t);
    }
    let mut v = Vec::new();
    for (d, _) in cand.iter().zip(&si).filter(|(_, s)| **s) {
        let mut vistos: Vec<String> = ya.to_vec();
        let (dlls, funciones) = le_falta(d, dir, &mut vistos);
        if dlls.is_empty() {
            let mut t = format!("PROTON-X: en vivo: {d} (el .exe la nombra y esta junto a el): se carga con las demas");
            if !funciones.is_empty() {
                t.push_str(&format!("; {} funcion(es) de la casa iran con TRAMPA:", funciones.len()));
                lista(&mut t, &funciones);
            }
            t.push('\n');
            di(&t);
            v.push(d.clone());
        } else {
            let mut t = format!("PROTON-X: en vivo: {d} (el .exe la nombra) NO se carga: pide DLL que no estan junto al .exe:");
            lista(&mut t, &dlls);
            t.push('\n');
            di(&t);
        }
    }
    v
}

/// **Resolver una DLL cargada en vivo**: lo de la casa, o una TRAMPA con
/// nombre por cada funcion que la casa no tiene; y se dice cuales.
pub(crate) fn resolver(
    m: &crate::cargador::Modulo,
    img: &mut [u8],
    imps: &[bmo_proton_x::Importacion],
) -> Result<(), bmo_proton_x::Fallo> {
    let trampeadas = core::cell::RefCell::new(Vec::new());
    let r = bmo_proton_x::resolver(img, imps, |d, f| {
        bmo_proton_x_casa::tabla_o_trampa(d, f).map(|(x, t)| {
            if t {
                trampeadas.borrow_mut().push(format!("{d}!{f}"));
            }
            x
        })
    });
    let trampeadas = trampeadas.into_inner();
    let corto = m.nombre.rsplit('/').next().unwrap_or(&m.nombre);
    let mut t = format!(
        "PROTON-X: en vivo: {corto} CARGADA en {:#x} ({} importaciones, {} con trampa)",
        m.base,
        imps.len(),
        trampeadas.len()
    );
    lista_hasta(&mut t, &trampeadas, 16);
    t.push('\n');
    di(&t);
    r
}
