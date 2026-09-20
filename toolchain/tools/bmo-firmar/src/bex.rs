//! **EL FICHERO: donde esta la firma y que es lo que se firma.**
//!
//! [carril]  VERDE     una herramienta del anfitrion: si se equivoca, lo dice
//!                     en la consola antes de tocar nada
//! [cuesta]  NADA      no viaja a la maquina: corre en el anfitrion
//! [riesgo]  ESPEJO    hay DOS lectores del anexo de firma --el juez de
//!                     `bmo_abi::bef2` y `ring0/task/landing.rs`-- y el
//!                     compilador no puede comprobar que coincidan
//!
//! # Que es exactamente lo que se firma
//!
//! No los bytes del `.bex`. **La CADENA**: el BLAKE3 de los digests declarados,
//! concatenados en el orden en que estan guardados.
//!
//! ```text
//!    anexo FIRMA = [cuantos][algoritmo] [que pad digest] x n [sig pubkey]
//!                   <--- 8 --->          <---- 40 ---->      <-- 96 -->
//!
//!    cadena = BLAKE3( digest_0 || digest_1 || ... || digest_n-1 )
//! ```
//!
//! ** Y ESO ATA EL CONJUNTO, no cada pieza por separado. Cambiar un byte del
//! codigo rompe su digest, que rompe la cadena, que rompe la firma. **Quitar
//! una region o un anexo** tambien: sobra o falta un digest y la cadena es otra.
//!
//! # BEF2 (2026-09-19): firmar es REESCRIBIR, no estampar
//!
//! En BEF1 el escritor reservaba 96 bytes a cero y esto los rellenaba en
//! sitio. En BEF2 el anexo de firma mide exactamente lo que lleva: sin firma
//! de autor no hay hueco, y la imagen se reabre (`Escritor::de_imagen`), se le
//! pone la firma y se construye otra vez. Los digests salen de los MISMOS
//! bytes en el MISMO orden, asi que la cadena que se firmo es la cadena que
//! queda escrita -- y `main.rs` lo comprueba releyendo con `bmo-firma`, que es
//! el crate del kernel, antes de dar el fichero por bueno.
//!
//! # [!] POR QUE LOS DIGESTS SE LEEN DEL FICHERO Y NO SE RECALCULAN
//!
//! El kernel hashea los bytes que el fichero DECLARA, en el orden en que estan
//! guardados. Esto hace lo mismo, en este orden:
//!
//! ```text
//!    1. COMPROBAR  cada digest declarado contra los bytes reales  -> o NO se firma
//!    2. FIRMAR     la cadena, sobre los digests TAL COMO ESTAN
//! ```
//!
//! *** El 1 es lo que impide firmar un fichero roto. **Una firma sobre un
//! binario corrupto no lo arregla: lo acredita.** Y lo hace el juez del
//! contrato (`bef2::leer`), que rechaza con nombre cualquier hash que no
//! cuadre: no hay forma de tener un `Bex` abierto sin haber pasado por ahi.

use bmo_abi::bef2::{self, Escritor, ANEXO_FIRMA};

/// Lo que hay que saber de un `.bex` para firmarlo.
pub struct Bex {
    /// Donde empieza el anexo de firma en el fichero.
    pub sec_off: usize,
    /// Cuanto mide.
    pub sec_len: usize,
    /// Cuantos digests declara.
    pub cuantos: usize,
}

impl Bex {
    /// Abre la imagen POR EL JUEZ: si llega aqui, cada digest ya cuadra con
    /// sus bytes y las regiones no se pisan. Un `.bex` que no pasa se dice con
    /// el nombre de su falta.
    pub fn abrir(b: &[u8]) -> Result<Bex, String> {
        let v = bef2::leer(b).map_err(|f| format!("este .bex no vale: {}", f.nombre()))?;
        let a = v
            .anexos()
            .find(|a| a.tipo == ANEXO_FIRMA)
            .ok_or("este .bex no trae anexo de firma")?;
        let sec_off = a.tramo.offset as usize;
        let sec_len = a.tramo.bytes as usize;
        let cuantos = u32::from_le_bytes(b[sec_off..sec_off + 4].try_into().unwrap()) as usize;
        Ok(Bex { sec_off, sec_len, cuantos })
    }

    /// Los bytes del anexo de firma.
    pub fn seccion<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        &b[self.sec_off..self.sec_off + self.sec_len]
    }

    /// **PASO 1: cada digest declarado contra los bytes reales.** Es lo que
    /// hace `abrir`; se vuelve a pasar por el juez para que quien lo llame no
    /// dependa de recordar que `abrir` ya lo hizo.
    pub fn comprobar(&self, b: &[u8]) -> Result<(), String> {
        bef2::leer(b).map(|_| ()).map_err(|f| {
            format!(
                "{} -- este .bex esta roto o lo tocaron, y firmarlo seria acreditarlo",
                f.nombre()
            )
        })
    }

    /// **PASO 2: la cadena**, sobre los digests tal como estan guardados.
    pub fn cadena(&self, b: &[u8]) -> [u8; 32] {
        bef2::leer(b)
            .ok()
            .and_then(|v| v.cadena_de_hashes())
            .expect("abrir ya comprobo que hay firma")
    }

    /// El algoritmo que trae hoy: 0 = solo integridad, 1 = Ed25519.
    pub fn algo(&self, b: &[u8]) -> u32 {
        u32::from_le_bytes(b[self.sec_off + 4..self.sec_off + 8].try_into().unwrap())
    }

    /// **Pone la firma de autor y devuelve la imagen reescrita.** Lo que el
    /// programa ES no cambia: regiones, entrada, relocs, requisitos y el resto
    /// de anexos viajan tal cual (`Escritor::de_imagen`).
    pub fn estampar(&self, b: &[u8], sig: &[u8; 64], pk: &[u8; 32]) -> Result<Vec<u8>, String> {
        let mut e = Escritor::de_imagen(b).map_err(|f| format!("no se reabre: {}", f.nombre()))?;
        e.ed25519(*sig, *pk);
        e.construir().map_err(|e| format!("no se reescribe: {e}"))
    }
}
