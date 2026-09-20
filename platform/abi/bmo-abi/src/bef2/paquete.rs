//! **Una app es UN fichero**: el `.bex` con sus recursos dentro.
//!
//! [carril]  AMARILLO  lo que empaquete mal lo rechaza el lector
//! [cuesta]  TAREA     una app sin su icono o sin su WAD
//! [riesgo]  UNICO     aqui se decide que sobrevive al reempaquetado
//!
//! En BEF1 esto recorria la tabla de secciones copiandolas una a una. En BEF2
//! no hace falta: [`Escritor::de_imagen`] reabre la imagen entera, y lo unico
//! que hay que decir es **que se anade**. La firma se rehace sola, que es
//! obligatorio: sus hashes describian la disposicion de antes.

use alloc::vec::Vec;

use super::{leer, Escritor, Falta, ANEXO_RECURSOS};
use crate::bef::recursos;

/// Mete (o reemplaza) los recursos de una imagen y la reescribe.
pub fn empaquetar(bex: &[u8], lista: &[(&str, &[u8])]) -> Result<Vec<u8>, &'static str> {
    let mut e = Escritor::de_imagen(bex).map_err(|f: Falta| f.nombre())?;
    if !lista.is_empty() {
        e.anexo(ANEXO_RECURSOS, recursos::construir(lista)?);
    }
    e.construir()
}

/// Los bytes del directorio de recursos, si la imagen los trae.
pub fn seccion_recursos(bex: &[u8]) -> Option<&[u8]> {
    leer(bex).ok()?.anexo(ANEXO_RECURSOS)
}

/// El directorio ya validado, en un paso.
pub fn directorio(bex: &[u8]) -> Option<recursos::Directorio<'_>> {
    recursos::Directorio::nuevo(seccion_recursos(bex)?)
}

/// **Donde estan los recursos dentro del fichero**: `(offset, bytes)`.
///
/// Lo necesita quien tiene el `.bex` en el disco y no en memoria -- una app
/// que abre su propia imagen y lee solo el trozo que le importa.
pub fn localizar_recursos(bex: &[u8]) -> Option<(u64, u64)> {
    let v = leer(bex).ok()?;
    let a = v.anexos().find(|a| a.tipo == ANEXO_RECURSOS)?;
    Some((a.tramo.offset as u64, a.tramo.bytes as u64))
}
