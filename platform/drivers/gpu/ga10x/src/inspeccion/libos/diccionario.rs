//! ELF64 LE acotado: inventario y VA a offset, sobre una Fuente a saltos.
//! capa: puro -- no ejecuta firmware ni interpreta punteros del host
//! Fuente: OpenRM 570.144 libelf.c::LibosElfMapVirtual, libos_log.h.
//! [carril] VERDE
//! [consumo] NADA -- consulta explicita de un fichero, sin aparato

use crate::elf::Fuente;
use super::Diccionario;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Lectura, Formato, Limites, NoMapeada, Ambigua, SinNul }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seccion {
    pub nombre: [u8; 64],
    pub n: usize,
    pub tipo: u32,
    pub va: u64,
    pub offset: u64,
    pub bytes: u64,
}

pub struct Elf<F> {
    fuente: F,
    bytes: u64,
    sh: u64,
    ph: u64,
    sn: u16,
    pn: u16,
    nombres: u16,
}

fn u16n(b: &[u8], o: usize) -> u16 { u16::from_le_bytes([b[o], b[o+1]]) }
fn u32n(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o+1], b[o+2], b[o+3]])
}
fn u64n(b: &[u8], o: usize) -> u64 {
    let mut a = [0; 8]; a.copy_from_slice(&b[o..o+8]); u64::from_le_bytes(a)
}
fn cabe(o: u64, n: u64, fin: u64) -> bool {
    o.checked_add(n).is_some_and(|v| v <= fin)
}

impl<F: Fuente> Elf<F> {
    pub fn nuevo(fuente: F, bytes: u64) -> Result<Self, Error> {
        let mut e = Self { fuente, bytes, sh: 0, ph: 0, sn: 0, pn: 0, nombres: 0 };
        let mut h = [0; 64]; e.leer(0, &mut h)?;
        if h[..7] != *b"\x7fELF\x02\x01\x01" || u16n(&h, 52) != 64 {
            return Err(Error::Formato);
        }
        e.ph = u64n(&h, 32); e.sh = u64n(&h, 40);
        e.pn = u16n(&h, 56); e.sn = u16n(&h, 60); e.nombres = u16n(&h, 62);
        if e.sn == 0 || e.sn > 64 || e.pn > 64 || e.nombres >= e.sn
            || u16n(&h, 58) != 64 || (e.pn != 0 && u16n(&h, 54) != 56)
            || !cabe(e.sh, e.sn as u64 * 64, bytes)
            || !cabe(e.ph, e.pn as u64 * 56, bytes) { return Err(Error::Limites); }
        Ok(e)
    }

    fn leer(&mut self, o: u64, b: &mut [u8]) -> Result<(), Error> {
        if !cabe(o, b.len() as u64, self.bytes) { return Err(Error::Limites); }
        if !self.fuente.leer(o, b) { return Err(Error::Lectura); }
        Ok(())
    }
    fn shdr(&mut self, i: u16) -> Result<[u8;64], Error> {
        if i >= self.sn { return Err(Error::Limites); }
        let mut h = [0;64]; self.leer(self.sh + i as u64*64, &mut h)?; Ok(h)
    }
    pub fn secciones(&self) -> u16 { self.sn }
    pub fn seccion(&mut self, i: u16) -> Result<Seccion, Error> {
        let s = self.shdr(i)?; let nombres = self.shdr(self.nombres)?;
        if u32n(&nombres, 4) != 3 { return Err(Error::Formato); }
        let (offset, bytes) = (u64n(&s,24), u64n(&s,32));
        let tipo = u32n(&s,4);
        if tipo != 8 && !cabe(offset, bytes, self.bytes) { return Err(Error::Limites); }
        let (tabla, tam) = (u64n(&nombres,24),u64n(&nombres,32));
        let en = u32n(&s,0) as u64;
        if en >= tam || !cabe(tabla,tam,self.bytes) { return Err(Error::Limites); }
        let mut nombre = [0;64];
        for n in 0..nombre.len() {
            if en+n as u64 >= tam { return Err(Error::SinNul); }
            self.leer(tabla+en+n as u64, &mut nombre[n..n+1])?;
            if nombre[n] == 0 {
                return Ok(Seccion { nombre, n, tipo, va: u64n(&s,16), offset, bytes });
            }
        }
        Err(Error::SinNul)
    }

    /// Primero PHDR, luego SHDR; nunca convierte BSS/NOBITS en bytes del fichero.
    pub fn mapear(&mut self, va: u64, n: u64) -> Result<u64, Error> {
        if n == 0 || va.checked_add(n).is_none() { return Err(Error::Limites); }
        let mut hallado = None;
        for i in 0..self.pn {
            let mut p = [0;56]; self.leer(self.ph+i as u64*56,&mut p)?;
            if u32n(&p,0) != 1 { continue; }
            self.candidato(va,n,u64n(&p,16),u64n(&p,8),u64n(&p,32),&mut hallado)?;
        }
        if let Some(o) = hallado { return Ok(o); }
        for i in 0..self.sn {
            let p = self.shdr(i)?;
            if matches!(u32n(&p,4),0|8) { continue; }
            self.candidato(va,n,u64n(&p,16),u64n(&p,24),u64n(&p,32),&mut hallado)?;
        }
        hallado.ok_or(Error::NoMapeada)
    }
    fn candidato(&self, va:u64, n:u64, base:u64, offset:u64, bytes:u64,
        hallado:&mut Option<u64>) -> Result<(),Error> {
        let Some(d) = va.checked_sub(base) else { return Ok(()) };
        if !cabe(d,n,bytes) { return Ok(()); }
        let o = offset.checked_add(d).ok_or(Error::Limites)?;
        if !cabe(o,n,self.bytes) { return Err(Error::Limites); }
        if hallado.is_some_and(|anterior| anterior != o) { return Err(Error::Ambigua); }
        *hallado = Some(o); Ok(())
    }
    pub fn virtual_leer(&mut self, va:u64, b:&mut[u8]) -> Result<(),Error> {
        let o = self.mapear(va,b.len() as u64)?; self.leer(o,b)
    }
    /// Acotada por el buffer del llamador; no acepta cadenas sin terminador.
    pub fn texto(&mut self, va:u64, b:&mut[u8]) -> Result<usize,Error> {
        for i in 0..b.len() {
            let a = va.checked_add(i as u64).ok_or(Error::Limites)?;
            self.virtual_leer(a,&mut b[i..i+1])?;
            if b[i] == 0 { return Ok(i); }
        }
        Err(Error::SinNul)
    }
    pub fn metadata(&mut self, va:u64) -> Result<Metadata,Error> {
        let mut h = [0;24]; self.virtual_leer(va,&mut h)?;
        let extendida = u64n(&h,0) == 0x8000_0000_0000_0000;
        let remoto = if extendida { u64n(&h,8) } else { 0 };
        if extendida {
            self.virtual_leer(va.checked_add(16).ok_or(Error::Limites)?,&mut h)?;
        }
        Ok(Metadata { archivo: u64n(&h,0), formato:u64n(&h,8), linea:u32n(&h,16),
            argumentos:h[20], nivel:h[21], especial:h[22], remoto })
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Metadata {
    pub archivo:u64, pub formato:u64, pub linea:u32, pub argumentos:u8,
    pub nivel:u8, pub especial:u8, pub remoto:u64,
}

impl<F:Fuente> Diccionario for Elf<F> {
    fn argumentos(&mut self, va:u64) -> Option<u8> { self.metadata(va).ok().map(|m|m.argumentos) }
}
