//! Perfil escribible por juego para ejecutables cargados desde D:.
//!
//! Separa de `main.rs` la preparacion de `proton-x/<juego>` en ESTRATOS:
//! perfil Windows y capa privada de archivos, sin escribir sobre el juego.

use super::{bmo, di};
use alloc::format;

/// Crea las carpetas del perfil si ESTRATOS esta montado y es escribible.
/// `None` conserva el modo de solo lectura cuando el volumen no esta listo.
pub(super) fn en_estratos(nombre: &str) -> Option<bmo_proton_x::proceso::Perfil> {
    let p = bmo_proton_x::proceso::perfil_de(nombre)?;
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 || bmo::info(bmo::INFO_ES_ESCRIBIBLE) == 0 {
        di(&format!(
            "PROTON-X: sin ESTRATOS escribible: el perfil de {} se queda en su carpeta (D:, solo lectura)\n",
            p.juego
        ));
        return None;
    }
    // Crear una que ya esta devuelve 0 (y lo cuenta CABINA): se cuentan las nuevas.
    let nuevas = p
        .carpetas()
        .iter()
        .filter(|c| bmo::estratos::crear_carpeta(c.as_bytes()) != 0)
        .count();
    di(&format!(
        "PROTON-X: el perfil de {} en ESTRATOS: {} ({} carpeta(s) nueva(s)) = {}\n",
        p.juego, p.volumen, nuevas, p.windows
    ));
    Some(p)
}
