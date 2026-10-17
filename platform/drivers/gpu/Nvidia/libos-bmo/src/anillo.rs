//! LIBOS 570.144: copias de logs, nunca memoria ni registros de la GPU.
//! capa: puro -- el llamador entrega una copia y un diccionario de la MISMA build
//! [carril] VERDE
//! [consumo] NADA -- lectura explicita, sin asignaciones ni unsafe
//!
//! Fuente: OpenRM 570.144, liblogdecode.c::libosExtractLog_ReadRecord y
//! libos_log.h::libosLogMetadata. Sin diccionario no se adivina argc ni
//! se buscan supuestos registros en palabras que pueden ser argumentos.

pub const MAX_ARGS: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Longitud,
    Truncado,
    SinDiccionario(u64),
    Argumentos(u8),
    TimestampCero,
}

/// El encabezado bruto no demuestra que toda la GPU haya estado inactiva.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resumen {
    pub put: u64,
    pub capacidad: u64,
    pub retenidas: u64,
    pub sobrescritas: u64,
}

pub fn resumen(copia: &[u8]) -> Result<Resumen, Error> {
    if copia.len() < 24 || copia.len() % 8 != 0 {
        return Err(Error::Longitud);
    }
    resumen_put(palabra(copia, 0), copia.len() as u64)
}

/// Encabezado leido por el kernel; `bytes` es el contrato de SU reserva.
pub fn resumen_put(put: u64, bytes: u64) -> Result<Resumen, Error> {
    if bytes < 24 || bytes % 8 != 0 { return Err(Error::Longitud); }
    let capacidad = bytes / 8 - 1;
    Ok(Resumen { put, capacidad, retenidas: put.min(capacidad),
        sobrescritas: put.saturating_sub(capacidad) })
}

fn palabra(b: &[u8], i: usize) -> u64 {
    let mut n = [0; 8];
    n.copy_from_slice(&b[i * 8..i * 8 + 8]);
    u64::from_le_bytes(n)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registro {
    pub timestamp: u64,
    pub metadata_va: u64,
    pub argumentos: [u64; MAX_ARGS],
    pub cuantos: u8,
}

/// El diccionario resuelve argc; debe rechazar VA no contenidas en su ELF.
pub trait Diccionario {
    fn argumentos(&mut self, va: u64) -> Option<u8>;
}

pub struct Lector<'a> {
    copia: &'a [u8],
    capacidad: u64,
    cursor: u64,
    piso: u64,
}

impl<'a> Lector<'a> {
    pub fn nuevo(copia: &'a [u8]) -> Result<Self, Error> {
        let r = resumen(copia)?;
        Ok(Self { copia, capacidad: r.capacidad, cursor: r.put,
            piso: r.put.saturating_sub(r.capacidad) })
    }

    fn anterior(&self, cursor: &mut u64) -> Result<u64, Error> {
        if *cursor <= self.piso { return Err(Error::Truncado); }
        *cursor -= 1;
        Ok(palabra(self.copia, 1 + (*cursor % self.capacidad) as usize))
    }

    /// Cabecera del registro MAS RECIENTE, aun sin diccionario. No avanza.
    pub fn cabecera(&self) -> Result<Option<(u64, u64)>, Error> {
        if self.cursor == self.piso { return Ok(None); }
        let mut c = self.cursor;
        let ts = self.anterior(&mut c)?;
        let va = self.anterior(&mut c)?;
        Ok(Some((ts, va)))
    }

    /// Hacia atras; ante un error NO avanza ni intenta resincronizar a ojo.
    pub fn siguiente(&mut self, d: &mut impl Diccionario) -> Result<Option<Registro>, Error> {
        let Some((timestamp, metadata_va)) = self.cabecera()? else { return Ok(None) };
        if timestamp == 0 { return Err(Error::TimestampCero); }
        let cuantos = d.argumentos(metadata_va).ok_or(Error::SinDiccionario(metadata_va))?;
        if cuantos as usize > MAX_ARGS { return Err(Error::Argumentos(cuantos)); }
        let mut c = self.cursor - 2;
        let mut argumentos = [0; MAX_ARGS];
        for i in (0..cuantos as usize).rev() {
            argumentos[i] = self.anterior(&mut c)?;
        }
        self.cursor = c;
        Ok(Some(Registro { timestamp, metadata_va, argumentos, cuantos }))
    }
}
