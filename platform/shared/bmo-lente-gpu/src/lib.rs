//! La pregunta: que bytes se capturaron, de donde y cuantos faltan.
//! generacion: abuelo -- bytes y ventanas sin depender de otra pieza
//! capa: puro -- recibe una copia estable; no lee direcciones ni aparatos
//! [carril] VERDE
//! [consumo] NADA -- solo cuando se pide una ventana
//!
//! La procedencia la declara quien captura; no acredita que hubo DMA ni
//! finalizacion. Una direccion es un rotulo, nunca un puntero que se sigue.

#![no_std]
#![forbid(unsafe_code)]

use core::fmt::{self, Write};

pub const PAGE_BYTES: usize = 64;
pub const ROW_BYTES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin { Prepared, Readback, RegisterSnapshot }
impl Origin {
    pub const fn label(self) -> &'static str {
        match self { Self::Prepared => "PREPARADO CPU", Self::Readback => "COPIA RECUPERADA", Self::RegisterSnapshot => "REGISTROS CAPTURADOS" }
    }
}

/// Dominios que no se deben presentar como si fueran la misma direccion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space { Resource, CpuVirtual, GpuVirtual, Registers }
impl Space {
    pub const fn label(self) -> &'static str {
        match self { Self::Resource => "RECURSO", Self::CpuVirtual => "VA CPU", Self::GpuVirtual => "VA GPU", Self::Registers => "REGISTROS" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture<'a> { Bytes(&'a [u8]), Unavailable }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Complete, Window, Unavailable }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Bounds, AddressOverflow, DifferentWindow, MissingBytes }

/// Intervalo del recurso [offset, offset + bytes.len()), no de la VRAM entera.
/// No reserva memoria ni conserva una vista de memoria que la GPU modifica.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot<'a> {
    origin: Origin,
    space: Space,
    base: u64,
    total: u64,
    offset: u64,
    capture: Capture<'a>,
}

impl<'a> Snapshot<'a> {
    pub fn new(origin: Origin, space: Space, base: u64, total: u64, offset: u64, capture: Capture<'a>) -> Result<Self, Error> {
        let n = match capture { Capture::Bytes(b) => b.len() as u64, Capture::Unavailable => 0 };
        if offset > total || n > total - offset { return Err(Error::Bounds); }
        // El ultimo byte puede estar en u64::MAX. El fin exclusivo no tiene
        // que convertirse en una direccion representable.
        if total > 0 && base.checked_add(total - 1).is_none() { return Err(Error::AddressOverflow); }
        Ok(Self { origin, space, base, total, offset, capture })
    }
    pub fn bytes(self) -> Option<&'a [u8]> {
        match self.capture { Capture::Bytes(b) => Some(b), Capture::Unavailable => None }
    }
    pub const fn offset(self) -> u64 { self.offset }
    pub const fn total(self) -> u64 { self.total }
    pub fn state(self) -> State {
        match self.capture {
            Capture::Unavailable => State::Unavailable,
            Capture::Bytes(b) if self.offset == 0 && b.len() as u64 == self.total => State::Complete,
            _ => State::Window,
        }
    }
    /// Solo avanza por bytes disponibles. Una captura vacia no sugiere un
    /// bucle infinito; una pagina final tampoco anuncia otra pagina.
    pub fn next(self) -> Option<u64> {
        let n = self.bytes()?.len() as u64;
        let end = self.offset + n;
        (n > 0 && end < self.total).then_some(end)
    }
    pub fn write(self, out: &mut impl Write) -> fmt::Result {
        let label = match self.state() { State::Complete => "COMPLETO", State::Window => "VENTANA", State::Unavailable => "INACCESIBLE" };
        writeln!(out, "{}; {} base={:016x}", self.origin.label(), self.space.label(), self.base)?;
        writeln!(out, "{label}; offset={} capturados={} total={}", self.offset, self.bytes().map_or(0, |b| b.len()), self.total)?;
        if let Some(bytes) = self.bytes() {
            for (k, row) in bytes.chunks(ROW_BYTES).enumerate() {
                let offset = self.offset + (k * ROW_BYTES) as u64;
                write!(out, "+{offset:016x}  ")?;
                for j in 0..ROW_BYTES {
                    if let Some(b) = row.get(j) { write!(out, "{b:02x} ")?; } else { out.write_str("   ")?; }
                }
                out.write_str("| ")?;
                for (j, b) in row.iter().enumerate() {
                    if j != 0 { out.write_char(' ')?; }
                    write!(out, "{b:08b}")?;
                }
                out.write_char('\n')?;
            }
        }
        if let Some(next) = self.next() { writeln!(out, "SIGUIENTE {next}")?; }
        Ok(())
    }
}

/// Bytes de la pagina que debe adquirir el llamador; EOF es una pagina
/// vacia valida, despues de EOF se rechaza. La lente no hace la lectura.
pub fn page(total: u64, offset: u64) -> Result<usize, Error> {
    Ok(total.checked_sub(offset).ok_or(Error::Bounds)?.min(PAGE_BYTES as u64) as usize)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Difference { pub count: usize, pub first: Option<u64> }

/// Compara dos ventanas que el llamador identifica como el mismo recurso.
/// Las bases pueden diferir (CPU/GPU); no demuestra identidad del recurso,
/// envio, sincronizacion ni finalizacion del trabajo.
pub fn compare(a: Snapshot<'_>, b: Snapshot<'_>) -> Result<Difference, Error> {
    if (a.total, a.offset) != (b.total, b.offset) { return Err(Error::DifferentWindow); }
    let (aa, bb) = (a.bytes().ok_or(Error::MissingBytes)?, b.bytes().ok_or(Error::MissingBytes)?);
    if aa.len() != bb.len() { return Err(Error::DifferentWindow); }
    let mut d = Difference { count: 0, first: None };
    for (k, (a_byte, b_byte)) in aa.iter().zip(bb).enumerate() {
        if a_byte != b_byte { d.count += 1; if d.first.is_none() { d.first = Some(a.offset + k as u64); } }
    }
    Ok(d)
}

#[cfg(test)]
mod tests;
