//! **EL SELLO POR TRAMOS** -- que paginas de un bloque pasan a codigo con
//! `MEM_OP_SELLAR_HASTA`, y si una escritura del kernel toca lo sellado (V4
//! de `PLAN_LOS_DOCE_DIRECTORES`, 07-10).
//!
//! generacion: nieto
//!
//! [cuesta]  MAQUINA -- por herencia: el kernel remapea con su respuesta.
//!
//! Un bloque de codigo que solo crece: lo sellado es un PREFIJO de paginas
//! enteras, `[0, sellado)`. Sellar mas es sellar `[sellado, hasta)`; lo de
//! detras sigue siendo datos. Aqui va la CUENTA, pura y con banco; el
//! remapeo es de `ring0::obj::memory::sellar_hasta`.

/// Por que no se sella (los mismos numeros que `SELLAR_*` del kernel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSella {
    /// Ya esta sellado hasta ahi (o mas): no se repite ni se deshace.
    YaSellado = 2,
    /// Mas alla del final del bloque.
    Fuera = 6,
}

/// **Las paginas a sellar**: de `sellado` (lo ya sellado, paginas enteras) a
/// `hasta` subido a pagina entera, en un bloque de `bytes`; `u64::MAX` = el
/// bloque entero. `Ok((desde, hasta))` en bytes desde el principio.
pub fn tramo(sellado: u64, bytes: u64, hasta: u64, pagina: u64) -> Result<(u64, u64), NoSella> {
    let hasta = if hasta == u64::MAX { bytes } else { hasta };
    if hasta > bytes {
        return Err(NoSella::Fuera);
    }
    let hasta = hasta.div_ceil(pagina) * pagina;
    if hasta <= sellado {
        return Err(NoSella::YaSellado);
    }
    Ok((sellado, hasta))
}

/// **Una escritura desde `va` toca lo sellado** de un bloque que empieza en
/// `base` con `sellado` bytes de codigo: el kernel no escribe ahi en nombre
/// de nadie. Lo sellado es un prefijo, asi que basta con donde EMPIEZA.
pub fn toca_sellado(base: u64, sellado: u64, va: u64) -> bool {
    va < base + sellado
}
