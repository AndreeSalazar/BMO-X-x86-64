//! **LAS RAMAS sobre una imagen** (`docs/plan/PLAN_LAS_RAMAS.md`, D5 (a)): la
//! tabla `bmo_estratos::ramas::Ramas`, publicada con el superbloque v2.
//!
//! ```text
//!    crear_rama     una rama nueva que empieza en una version que ya esta
//!    cambiar_rama   el superbloque pasa a seguir la punta de otra rama; la
//!                   de ahora se queda en la tabla. NO publica estrato: nadie
//!                   se vuelve antepasado de nadie
//!    punta_de       donde esta una rama, para mezclarla (`mezclar`)
//! ```
//!
//! ** LA SUBIDA A v2 ESCRIBE LAS DOS COPIAS del superbloque (`ESTRATOS.md`,
//! "la tabla de ramas"): con una sola, un kernel v1 montaria la copia vieja y
//! escribiria encima de la nueva. Con las dos, un kernel v1 no monta el
//! volumen -- y no puede borrar la tabla.

use super::*;
use es::ramas::Ramas;

fn tabla<R: Read + Seek>(r: &mut R, sb: &Superblock) -> Result<Option<Ramas>, String> {
    if sb.ramas.es_nulo() {
        return Ok(None);
    }
    let d = leer_objeto(r, &sb.ramas)?;
    Ramas::decode(&d).map(Some).map_err(|e| format!("tabla de ramas: {}", e.name()))
}

/// La tabla de ramas del volumen, si tiene.
pub fn leer_ramas<R: Read + Seek>(r: &mut R, disk_id: [u8; 32], generacion: u64) -> Result<Option<Ramas>, String> {
    let (sb, _) = abrir(r, disk_id, generacion)?;
    tabla(r, &sb)
}

/// La punta de la rama `nombre` (la de la actual es la del superbloque).
pub fn punta_de<R: Read + Seek>(r: &mut R, disk_id: [u8; 32], generacion: u64, nombre: &[u8]) -> Result<BlockPtr, String> {
    let (sb, _) = abrir(r, disk_id, generacion)?;
    let t = tabla(r, &sb)?.ok_or("el volumen no tiene ramas")?;
    t.punta(nombre, sb.estrato).map_err(|e| format!("{e:?}"))
}

/// Publica `t` (y, si `sigue` trae una, la punta que el superbloque pasa a
/// seguir) con el orden de siempre, y la subida a v2 en las DOS copias.
fn publicar_tabla<W: Almacen>(w: &mut W, disk_id: [u8; 32], generacion: u64, t: &Ramas, sigue: Option<BlockPtr>) -> Result<u64, String> {
    let (sb, cual) = abrir(w, disk_id, generacion)?;
    let mut tr = Transaccion::open(&sb, cual, true).map_err(|e| e.name().to_string())?;
    let lba = tr.reserve(1).map_err(|e| e.name().to_string())?;
    let p = escribir_objeto(w, lba, &t.encode())?;
    tr.cerrar_datos().map_err(|e| e.name().to_string())?;
    w.barrera().map_err(|e| format!("barrera antes del commit: {e}"))?;
    tr.barrera_hecha().map_err(|e| e.name().to_string())?;
    let (destino, nuevo) = tr.commit(sigue.unwrap_or(sb.estrato)).map_err(|e| e.name().to_string())?;
    let nuevo = nuevo.con_ramas(p);
    let escribe = |w: &mut W, lba: u64| -> Result<(), String> {
        let off = lba.checked_mul(BLOQUE as u64).ok_or("superbloque fuera de rango")?;
        w.seek(SeekFrom::Start(off)).and_then(|_| w.write_all(&nuevo.encode())).map_err(|e| format!("superbloque: {e}"))?;
        w.barrera().map_err(|e| format!("barrera del commit: {e}"))
    };
    escribe(w, destino)?;
    // ** LA SUBIDA: si el volumen era v1, la OTRA copia tambien pasa a v2. La
    // primera ya es valida y mas nueva: un corte aqui no pierde nada.
    if sb.version != es::VERSION_RAMAS {
        escribe(w, cual)?;
    }
    Ok(nuevo.generation)
}

/// **Una rama nueva**, `nombre`, que empieza en `desde` (o en la punta de
/// ahora). Si el volumen no tenia ramas, la de ahora pasa a llamarse
/// `actual`. Devuelve la generacion nueva.
pub fn crear_rama<W: Almacen>(
    w: &mut W,
    disk_id: [u8; 32],
    generacion: u64,
    nombre: &[u8],
    desde: Option<BlockPtr>,
    actual: &[u8],
) -> Result<u64, String> {
    let (sb, _) = abrir(w, disk_id, generacion)?;
    let mut t = match tabla(w, &sb)? {
        Some(t) => t,
        None => Ramas::nueva(actual).map_err(|e| format!("{e:?}"))?,
    };
    t.crear(nombre, desde.unwrap_or(sb.estrato)).map_err(|e| format!("{e:?}"))?;
    publicar_tabla(w, disk_id, generacion, &t, None)
}

/// **Cambiar a la rama `nombre`**: el superbloque sigue su punta, y la de
/// ahora se queda guardada en la tabla. Devuelve la generacion nueva.
pub fn cambiar_rama<W: Almacen>(w: &mut W, disk_id: [u8; 32], generacion: u64, nombre: &[u8]) -> Result<u64, String> {
    let (sb, _) = abrir(w, disk_id, generacion)?;
    let mut t = tabla(w, &sb)?.ok_or("el volumen no tiene ramas")?;
    let sigue = t.cambiar(nombre, sb.estrato).map_err(|e| format!("{e:?}"))?;
    publicar_tabla(w, disk_id, generacion, &t, Some(sigue))
}
