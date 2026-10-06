//! **LAS TRES RAICES DE UNA MEZCLA, SIN `alloc`** (`docs/plan/PLAN_LAS_RAMAS.md`,
//! R4c-2b): la BASE --el antepasado comun-- y las raices de base, A y B.
//!
//! [carril]  VERDE     solo lee
//! [consumo] NADA      corre cuando alguien pide mezclar
//!
//! ```text
//!    los antepasados   por ANCHURA desde cada punta, siguiendo los DOS
//!                      padres (D2). La tabla que presta quien llama es a la
//!                      vez la cola: llena, deja de crecer
//!    la base           el primero de A que B tambien tiene (`mezcla::base`)
//! ```
//!
//! ** Vive aqui y no en el kernel por la regla de `read.rs`: un solo recorrido
//! para todo el sistema. El kernel lo corre con sus tablas prestadas, y
//! `estratos-mezcla` con las suyas, y lo compara con su recorrido con `Vec`
//! en cada mezcla.

use crate::objects::{BlockPtr, BLOQUE};
use crate::read::Fuente;
use crate::{Estrato, FormatError, SegundoPadre, ESTRATO_LEN};

/// Por que no salen las tres raices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoHay {
    /// Un estrato no se pudo leer, o no cuadra con su suma.
    Formato(FormatError),
    /// El segundo padre apunta a algo que no es el estrato que se apunto.
    SegundoPadre,
    /// La rama que entra ya esta dentro de la de ahora: nada que mezclar.
    NadaQueMezclar,
    /// Las dos ramas no comparten historia: sin base no hay mezcla de tres.
    SinBase,
}

impl From<FormatError> for NoHay {
    fn from(e: FormatError) -> Self {
        NoHay::Formato(e)
    }
}

/// Lee el estrato de `p` y lo comprueba contra su suma.
pub fn leer_estrato(src: &mut dyn Fuente, p: &BlockPtr, bloque: &mut [u8; BLOQUE]) -> Result<Estrato, NoHay> {
    if !src.bloque(p.lba, bloque) {
        return Err(FormatError::Io.into());
    }
    let (i, f) = (p.off as usize, p.off as usize + p.len as usize);
    let d = bloque.get(i..f).ok_or(FormatError::BadField)?;
    if !p.verifica(d) {
        return Err(FormatError::BadChecksum.into());
    }
    Ok(Estrato::decode(d)?)
}

/// El segundo padre como puntero entero: se lee su estrato y la huella tiene
/// que cuadrar (es lo que `SegundoPadre` guarda en vez del hash entero).
pub fn segundo_padre(src: &mut dyn Fuente, s: &SegundoPadre, bloque: &mut [u8; BLOQUE]) -> Result<BlockPtr, NoHay> {
    if !src.bloque(s.lba, bloque) {
        return Err(FormatError::Io.into());
    }
    let (i, f) = (s.off as usize, s.off as usize + ESTRATO_LEN);
    let d = bloque.get(i..f).ok_or(FormatError::BadField)?;
    let p = BlockPtr::nuevo(s.lba, s.off, d);
    if !s.es(&p) {
        return Err(NoHay::SegundoPadre);
    }
    Estrato::decode(d)?;
    Ok(p)
}

fn mismo(a: &BlockPtr, b: &BlockPtr) -> bool {
    a.lba == b.lba && a.off == b.off
}

/// **Los antepasados de `desde`** --el mismo incluido-- por anchura, en `t`.
/// Devuelve cuantos. Llena, la tabla deja de crecer y lo de detras no se mira.
pub fn antepasados(src: &mut dyn Fuente, desde: BlockPtr, t: &mut [BlockPtr], bloque: &mut [u8; BLOQUE]) -> Result<usize, NoHay> {
    fn meter(t: &mut [BlockPtr], n: &mut usize, p: BlockPtr) {
        if !p.es_nulo() && *n < t.len() && !t[..*n].iter().any(|v| mismo(v, &p)) {
            t[*n] = p;
            *n += 1;
        }
    }
    let mut n = 0;
    meter(t, &mut n, desde);
    let mut cabeza = 0;
    while cabeza < n {
        let e = leer_estrato(src, &t[cabeza], bloque)?;
        cabeza += 1;
        meter(t, &mut n, e.padre);
        if let Some(s) = e.segundo {
            let p = segundo_padre(src, &s, bloque)?;
            meter(t, &mut n, p);
        }
    }
    Ok(n)
}

/// **Las tres raices**: `[base, ahora, otra]`. `de_a` y `de_b` son las tablas
/// de antepasados de cada punta; su largo es cuanto se mira hacia atras.
pub fn raices(
    src: &mut dyn Fuente,
    ahora: BlockPtr,
    otra: BlockPtr,
    de_a: &mut [BlockPtr],
    de_b: &mut [BlockPtr],
    bloque: &mut [u8; BLOQUE],
) -> Result<[BlockPtr; 3], NoHay> {
    let na = antepasados(src, ahora, de_a, bloque)?;
    if de_a[..na].iter().any(|p| mismo(p, &otra)) {
        return Err(NoHay::NadaQueMezclar);
    }
    let nb = antepasados(src, otra, de_b, bloque)?;
    let (de_a, de_b) = (&de_a[..na], &de_b[..nb]);
    // La regla de `mezcla::base`: el primero de A que tambien tiene B.
    let base = *de_a.iter().find(|p| de_b.iter().any(|q| mismo(p, q))).ok_or(NoHay::SinBase)?;
    Ok([leer_estrato(src, &base, bloque)?.raiz, leer_estrato(src, &ahora, bloque)?.raiz, leer_estrato(src, &otra, bloque)?.raiz])
}
