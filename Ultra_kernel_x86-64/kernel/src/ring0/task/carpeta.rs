//! **LA APP CARPETA** (KA1 de `docs/plan/PLAN_EL_KERNEL_DE_LA_APP.md`, 15-10).
//!
//! [carril]  AMARILLO  lee el directorio de la app y el principio de cada pieza
//! [consumo] NADA      en reposo: trabaja una vez, al lanzar un `nucleo.bex`
//!
//! > El propietario: *"son datos en .bex en general en una carpeta [...] al
//! > ser validado ese mini kernel de la app, SABE que hacer"*.
//!
//! Una app puede ser UNA CARPETA: `apps/ecce/` con su `nucleo.bex` -- el que
//! arranca, el "kernel de la app" -- y sus PIEZAS, un `.bex` (o `.ibx`) por
//! funcionalidad. Esto hace las dos cosas que el kernel principal le debe:
//!
//! ```text
//!    el nombre    el proceso se llama como la CARPETA (`ecce`), no
//!                 `nucleo`: es lo que dicen la ventana, la barra y Alt+Tab
//!    la puerta    ANTES de admitir el nucleo, cada pieza de la carpeta pasa
//!                 por la misma puerta que un `.bex` suelto (`bmo-bex-gate`:
//!                 cabecera, tabla, regiones y anexos dentro del fichero).
//!                 Una que no pasa para la app ENTERA, y CABINA dice cual y
//!                 por que. Una app no arranca a medias
//! ```
//!
//! ** Lo que NO hace todavia, dicho: la lente y los papeles de cada pieza
//! (B1 y A6) se juzgan cuando la pieza se CARGA (KA4 sobre EL_TOQUE), que es
//! cuando hay codigo mapeado que leer. Hoy ninguna app carga piezas: lo que
//! hay es la carpeta, su nombre y la puerta. Y solo el volumen de DATOS
//! (FAT32): una carpeta en ESTRATOS se lanza como un `.bex` suelto, y se dice.

use bmo_bex_gate as gate;

use super::launch::Fallo;

/// El que arranca en una app carpeta.
pub const NUCLEO: &str = "nucleo.bex";

/// Piezas que se miran como mucho: mas que esto en una carpeta es un error
/// de quien la armo, y se dice en vez de leer sin fin.
const MAX_PIEZAS: usize = 64;

/// El prologo de una pieza: lo mismo que lee el cargador (`launch::PROLOGO`).
const PROLOGO: usize = 2048;

/// **Si `path` es el nucleo de una app carpeta**, la carpeta (`apps/ecce`).
pub fn carpeta_de(path: &str) -> Option<&str> {
    let i = path.rfind(|c| c == '/' || c == '\\')?;
    path[i + 1..].eq_ignore_ascii_case(NUCLEO).then(|| &path[..i])
}

/// **El nombre del programa**: el ultimo componente de la ruta, o el de la
/// carpeta si es su nucleo (`apps/ecce/nucleo.bex` es `ecce`).
pub fn nombre_de(path: &str) -> &str {
    let base = carpeta_de(path).unwrap_or(path);
    match base.rfind(|c| c == '/' || c == '\\') {
        Some(i) => &base[i + 1..],
        None => base,
    }
}

/// **Todas las piezas por la puerta**, si `path` es un nucleo. `Ok(n)` con
/// las piezas que pasaron (0 si no es una app carpeta); `Err` en la primera
/// que no, con su motivo y su numero en CABINA.
pub fn revisar_piezas(path: &str) -> Result<usize, Fallo> {
    let Some(carpeta) = carpeta_de(path) else { return Ok(0) };
    let Some((arranque, cluster)) = crate::ring0::fsys::fs::dir_de(carpeta) else {
        crate::ring0::cabina::info("carpeta", "la carpeta del nucleo no esta en DATOS: sin piezas que mirar", 0);
        return Ok(0);
    };
    let mut buenas = 0usize;
    let mut prologo = [0u8; PROLOGO];
    for n in 0..MAX_PIEZAS * 4 {
        let Some((nombre83, es_dir, _)) = crate::ring0::fsys::fs::entrada_de(arranque, cluster, n) else { break };
        let ext = &nombre83[8..11];
        let es_pieza = !es_dir
            && (ext.eq_ignore_ascii_case(b"BEX") || ext.eq_ignore_ascii_case(b"IBX"))
            && !nombre83[..8].eq_ignore_ascii_case(b"NUCLEO  ");
        if !es_pieza {
            continue;
        }
        if buenas == MAX_PIEZAS {
            crate::ring0::cabina::fault("carpeta", "RECHAZADA: la app trae mas piezas de las que se miran", MAX_PIEZAS as u64);
            return Err(Fallo::PiezaMala);
        }
        // `carpeta/NOMBRE.EXT`: la carpeta cabe en la linea de lanzar (128) y
        // el nombre 8.3 en doce.
        let mut ruta = [0u8; 160];
        let mut l = 0usize;
        let mut pon = |b: &[u8], ruta: &mut [u8; 160]| {
            let k = b.len().min(ruta.len() - l);
            ruta[l..l + k].copy_from_slice(&b[..k]);
            l += k;
        };
        pon(carpeta.as_bytes(), &mut ruta);
        pon(b"/", &mut ruta);
        let corto = nombre83[..8].iter().position(|&c| c == b' ').unwrap_or(8);
        pon(&nombre83[..corto], &mut ruta);
        pon(b".", &mut ruta);
        pon(ext, &mut ruta);
        let Ok(ruta) = core::str::from_utf8(&ruta[..l]) else {
            crate::ring0::cabina::fault("carpeta", "RECHAZADA: una pieza con un nombre que no se puede escribir", buenas as u64);
            return Err(Fallo::PiezaMala);
        };
        let (leidos, tam) = match crate::ring0::fsys::fs::load_prefijo(ruta, &mut prologo) {
            Ok(x) => x,
            Err(e) => {
                crate::ring0::cabina::fault("carpeta", e.name(), buenas as u64);
                crate::ring0::cabina::fault("carpeta", "RECHAZADA: una pieza de la app no se pudo leer", buenas as u64);
                return Err(Fallo::PiezaMala);
            }
        };
        if let Err(f) = gate::revisar(&prologo[..leidos], tam) {
            crate::ring0::cabina::fault("carpeta", f.nombre(), buenas as u64);
            crate::ring0::cabina::fault("carpeta", "RECHAZADA: una PIEZA de la app no pasa la puerta (su numero, en la de arriba)", buenas as u64);
            return Err(Fallo::PiezaMala);
        }
        buenas += 1;
    }
    crate::ring0::cabina::info("carpeta", "app carpeta: piezas que pasaron la puerta", buenas as u64);
    Ok(buenas)
}
