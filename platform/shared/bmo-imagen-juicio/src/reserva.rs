//! **EL JUEZ DE LA RESERVA** -- la memoria que el juego pide EN MARCHA
//! (`VirtualAlloc`, sus montones: varios GiB), P0.4c de
//! `PLAN_LAS_TRES_GRANDES` (30-09).
//!
//! La casa (Ring 3) lleva la cuenta de Windows: reservar solo cuesta
//! DIRECCIONES, dentro de una ventana propia del proceso. El kernel solo
//! entrega y recoge PAGINAS, a peticion:
//!
//! ```text
//!    HACER(va, bytes)      las paginas de [va, va+bytes) que no estaban, a
//!                          cero, escribibles y sin ejecucion; juzgado contra
//!                          la RAM LIBRE AHORA menos el margen del kernel
//!    DESHACER(va, bytes)   las que estaban, desmapeadas, a cero y de vuelta
//! ```
//!
//! Esto dice si una peticion cabe: el rango (alineado, dentro de la ventana,
//! no mas de `max_por_vez` de una vez -- el kernel pone a cero cada pagina,
//! y un GiB de golpe seria un congelon) y la RAM de lo que falta.

/// Lo que el kernel le da al juez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitesReserva {
    pub ventana_base: u64,
    pub ventana_bytes: u64,
    pub pagina: u64,
    /// Lo mas que se hace de una vez (la casa parte lo grande).
    pub max_por_vez: u64,
    /// La RAM libre AHORA, y lo que el kernel se guarda para si.
    pub libre: u64,
    pub margen: u64,
}

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoReserva {
    /// 0 bytes.
    Vacia,
    /// `va` o `bytes` no van a pagina.
    Desalineada,
    /// Se sale de la ventana (o da la vuelta).
    FueraDeVentana,
    /// Mas de `max_por_vez` de una vez.
    DeMasDeUnaVez { pide: u64, max: u64 },
    /// No hay RAM para las paginas que faltan: pide y hay (lo libre menos
    /// el margen).
    SinRam { pide: u64, hay: u64 },
    /// Los limites no tienen sentido.
    LimitesMalos,
}

fn limites_buenos(l: &LimitesReserva) -> bool {
    l.pagina.is_power_of_two() && l.ventana_base % l.pagina == 0 && l.ventana_bytes % l.pagina == 0 && l.max_por_vez >= l.pagina
}

/// **El rango**: `[va, va + bytes)` alineado, dentro de la ventana, y de una
/// medida que se hace de una vez. Devuelve cuantas paginas son.
pub fn rango(va: u64, bytes: u64, l: &LimitesReserva) -> Result<u64, NoReserva> {
    if !limites_buenos(l) {
        return Err(NoReserva::LimitesMalos);
    }
    if bytes == 0 {
        return Err(NoReserva::Vacia);
    }
    if va % l.pagina != 0 || bytes % l.pagina != 0 {
        return Err(NoReserva::Desalineada);
    }
    let fin = va.checked_add(bytes).ok_or(NoReserva::FueraDeVentana)?;
    let ventana_fin = l.ventana_base.checked_add(l.ventana_bytes).ok_or(NoReserva::LimitesMalos)?;
    if va < l.ventana_base || fin > ventana_fin {
        return Err(NoReserva::FueraDeVentana);
    }
    if bytes > l.max_por_vez {
        return Err(NoReserva::DeMasDeUnaVez { pide: bytes, max: l.max_por_vez });
    }
    Ok(bytes / l.pagina)
}

/// **La RAM de lo que falta**: `nuevas` paginas (las del rango que no
/// estaban hechas), contra lo libre de ahora menos el margen.
pub fn ram(nuevas: u64, l: &LimitesReserva) -> Result<(), NoReserva> {
    let pide = nuevas.saturating_mul(l.pagina);
    let hay = l.libre.saturating_sub(l.margen);
    if pide > hay {
        return Err(NoReserva::SinRam { pide, hay });
    }
    Ok(())
}
