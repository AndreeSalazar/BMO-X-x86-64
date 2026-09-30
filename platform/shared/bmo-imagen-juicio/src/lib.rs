//! **EL JUEZ DE LA DECLARACION DE IMAGEN** -- una app que va a cargar un
//! `.exe` de Windows y sus DLL declara TODAS sus partes de una vez; esto dice
//! si caben y donde va cada una (P0.4b de `PLAN_LAS_TRES_GRANDES`).
//!
//! generacion: nieto
//!
//! [cuesta]  MAQUINA -- por herencia: no toca hardware ni tiene un `unsafe`,
//!           pero el kernel mapea con su respuesta.
//!
//! # POR QUE (30-09)
//!
//! Un proceso tiene, como mucho, ocho bloques vivos de 64 MiB en una ventana
//! de 512 MiB (`obj/memory.rs`). Cyberpunk son unas 55 partes (el `.exe` y
//! sus 26 DLL, codigo y datos de cada una), una de ellas de 70 MiB
//! (`libxess.dll`). No se suben los topes para todos: la app que SABE lo que
//! necesita lo DECLARA, esto lo juzga UNA vez, y lo concedido corre sin mas
//! preguntas (la burocracia se paga al principio).
//!
//! # LO QUE DECIDE
//!
//! ```text
//!    las partes     de mas, de 0 bytes, o fuera de orden -> no
//!    la RAM         lo que pide, contra lo LIBRE AHORA menos el margen del
//!                   kernel: DINAMICO, no un tope escrito -> no, con los dos
//!                   numeros
//!    la ventana     lo que ocupa en VA, contra la ventana de imagenes -> no
//!    y si cabe      donde va cada parte: cada PE empieza alineado, y sus
//!                   tramos (codigo, datos, codigo...) van SEGUIDOS (lo que
//!                   el cargador de la app exige)
//! ```
//!
//! **Sin una constante de medida**: los limites los pone el kernel. Un juez
//! que no puede inventarse el techo no puede equivocarse en el techo. Y
//! **sin asignar**: el sitio de cada parte se recalcula con [`donde`], la
//! misma cuenta para el kernel y para la app.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Una parte declarada: de que PE es (0, 1, 2...: en orden), si es su
/// codigo o sus datos, y cuantos bytes mide en memoria.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parte {
    pub pe: u16,
    pub codigo: bool,
    pub bytes: u64,
}

/// Lo que el kernel le da al juez para juzgar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limites {
    /// Cuantas partes caben en su tabla.
    pub max_partes: usize,
    /// La RAM libre AHORA, y lo que el kernel se guarda para si.
    pub libre: u64,
    pub margen: u64,
    /// La ventana de imagenes en la VA del proceso.
    pub ventana_base: u64,
    pub ventana_bytes: u64,
    /// La pagina (4 KiB) y donde empieza cada PE (64 KiB, como en Windows).
    pub pagina: u64,
    pub alineacion: u64,
}

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motivo {
    /// No declara nada.
    Vacia,
    /// Mas partes de las que caben en la tabla.
    DeMas { partes: usize, max: usize },
    /// La parte `i` mide 0.
    ParteVacia { i: usize },
    /// La parte `i` rompe el orden: los PE van 0, 1, 2... y, dentro de cada
    /// uno, sus tramos alternan codigo y datos (dos seguidos del mismo
    /// permiso serian uno solo).
    Desordenada { i: usize },
    /// No hay RAM: pide `pide` bytes y hay `hay` (lo libre menos el margen).
    SinRam { pide: u64, hay: u64 },
    /// No cabe en la ventana: ocupa `pide` bytes de VA y la ventana tiene `hay`.
    SinVentana { pide: u64, hay: u64 },
    /// Los limites no tienen sentido (pagina o alineacion que no son
    /// potencias de dos, o una alineacion menor que la pagina).
    LimitesMalos,
}

/// El veredicto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Veredicto {
    /// Cabe: la RAM que se entrega (paginas enteras) y la VA que ocupa.
    Concedida { ram: u64, va: u64 },
    Negada(Motivo),
}

fn subir(v: u64, a: u64) -> Option<u64> {
    Some(v.checked_add(a - 1)? & !(a - 1))
}

fn limites_buenos(l: &Limites) -> bool {
    l.pagina.is_power_of_two() && l.alineacion.is_power_of_two() && l.alineacion >= l.pagina
}

/// **El sitio de cada parte**, en orden, desde el principio de la ventana:
/// `f(i, desplazamiento, bytes_mapeados)`. Devuelve lo que ocupa todo, o
/// `None` si la cuenta desborda.
fn recorrer(partes: &[Parte], l: &Limites, mut f: impl FnMut(usize, u64, u64)) -> Option<u64> {
    let mut fin = 0u64;
    let mut pe_actual: Option<u16> = None;
    for (i, p) in partes.iter().enumerate() {
        let paginas = subir(p.bytes, l.pagina)?;
        // Un PE nuevo empieza alineado; dentro del mismo, seguido.
        let ini = if pe_actual == Some(p.pe) { fin } else { subir(fin, l.alineacion)? };
        pe_actual = Some(p.pe);
        f(i, ini, paginas);
        fin = ini.checked_add(paginas)?;
    }
    Some(fin)
}

/// **Juzgar una declaracion.**
pub fn juzgar(partes: &[Parte], l: &Limites) -> Veredicto {
    use Motivo::*;
    if !limites_buenos(l) {
        return Veredicto::Negada(LimitesMalos);
    }
    if partes.is_empty() {
        return Veredicto::Negada(Vacia);
    }
    if partes.len() > l.max_partes {
        return Veredicto::Negada(DeMas { partes: partes.len(), max: l.max_partes });
    }
    // El orden: PE que no bajan ni saltan; dentro de cada uno, tramos que
    // alternan codigo y datos (P0.4b.6: `bink2w64.dll` trae `.rdata` entre
    // dos codigos, y su imagen es codigo, datos, codigo, datos).
    let mut antes: Option<Parte> = None;
    for (i, p) in partes.iter().enumerate() {
        if p.bytes == 0 {
            return Veredicto::Negada(ParteVacia { i });
        }
        let bien = match antes {
            None => p.pe == 0,
            Some(a) if a.pe == p.pe => a.codigo != p.codigo,
            Some(a) => a.pe.checked_add(1) == Some(p.pe),
        };
        if !bien {
            return Veredicto::Negada(Desordenada { i });
        }
        antes = Some(*p);
    }
    let mut ram = 0u64;
    let Some(va) = recorrer(partes, l, |_, _, n| ram = ram.saturating_add(n)) else {
        return Veredicto::Negada(SinVentana { pide: u64::MAX, hay: l.ventana_bytes });
    };
    let hay = l.libre.saturating_sub(l.margen);
    if ram > hay {
        return Veredicto::Negada(SinRam { pide: ram, hay });
    }
    if va > l.ventana_bytes {
        return Veredicto::Negada(SinVentana { pide: va, hay: l.ventana_bytes });
    }
    Veredicto::Concedida { ram, va }
}

/// **Donde va la parte `i`** (su VA y cuantos bytes se le mapean), con la
/// misma cuenta que [`juzgar`]. Solo tiene sentido si la declaracion se
/// concedio; si no, o si `i` no existe, `None`.
pub fn donde(partes: &[Parte], l: &Limites, i: usize) -> Option<(u64, u64)> {
    if !matches!(juzgar(partes, l), Veredicto::Concedida { .. }) {
        return None;
    }
    let mut r = None;
    recorrer(partes, l, |k, ini, n| {
        if k == i {
            r = Some((ini, n));
        }
    })?;
    let (ini, n) = r?;
    Some((l.ventana_base.checked_add(ini)?, n))
}

pub mod reserva;

#[cfg(test)]
mod pruebas;
