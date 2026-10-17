//! Reparto puro de la captura: un bloque temporal si cabe en una peticion.
//! Sin ampliar el cupo del kernel; las imagenes grandes conservan dos bloques.
use bmo_imagen::png_escribir as png;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub workshop: usize,
    pub raw: usize,
    pub capacity: usize,
    pub png_offset: usize,
    pub work_bytes: usize,
    pub output_bytes: usize,
}

impl Layout {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 || width > bmo_imagen::LADO_MAX || height > bmo_imagen::LADO_MAX {
            return None;
        }
        let workshop = png::TALLER.checked_mul(4)?;
        let raw = png::crudo(width, height);
        let capacity = png::cota(width, height);
        let work = workshop.checked_add(raw)?;
        let total = work.checked_add(capacity)?;
        // KIND_MEMORIA concede hasta 64 MiB por peticion. Por encima se
        // mantiene el reparto anterior: no reducir las medidas admitidas.
        let shared = total <= 64 * 1024 * 1024;
        Some(Self { workshop, raw, capacity, png_offset: if shared { work } else { 0 },
            work_bytes: if shared { total } else { work }, output_bytes: if shared { 0 } else { capacity } })
    }
}
