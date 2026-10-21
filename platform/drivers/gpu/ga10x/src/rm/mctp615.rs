//! Transporte GSP 615.78.08 sin cifrado: MCTP/NVDM, GMCAPI y NVKV.
//! capa: puro -- recibe y produce bytes, sin autoridad de aparato.
//! [carril] AMARILLO
//! Formatos contrastados con OpenRM 615 (MIT): message_queue_priv.h,
//! gmcapi_base.h, nvkv.h, gmcapi_system_info.h y msgq_priv.h.

use crate::{es_error_pri, orden::Sistema};

pub const TX_HEAD: u32 = 0x0011_0C00;
pub const TX_TAIL: u32 = 0x0011_0C04;
pub const RX_HEAD: u32 = 0x0011_0C80;
pub const RX_TAIL: u32 = 0x0011_0C84;
pub const MAGIC: u32 = 0x4D43_5450;
pub const GMCAPI: u8 = 0x26;
pub const RPC: u8 = 0x25;
pub const INIT: u32 = 0x010001;
pub const GENERICO: u32 = 0x010002;
pub const HS: u32 = 0x010003;
pub const RESPUESTA: u32 = 0x0100_0000;
pub const ENVUELTA: usize = 24;
pub const GMC_CABECERA: usize = 40;
pub const MAX_BYTES: usize = 65536;
pub const MAX_RESPUESTA: usize = 48 * 1024;
pub const HUECOS: u32 = 63;
pub const PAGINA: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum No { Corta, Forma, Medida, Protocolo, Puntero, Llena, Correlacion, Estado(u32), Kv }

fn u32d(b: &[u8], o: usize) -> u32 { u32::from_le_bytes(b[o..o+4].try_into().unwrap()) }
fn u64d(b: &[u8], o: usize) -> u64 { u64::from_le_bytes(b[o..o+8].try_into().unwrap()) }
fn p32(b: &mut [u8], o: usize, v: u32) { b[o..o+4].copy_from_slice(&v.to_le_bytes()); }
fn p64(b: &mut [u8], o: usize, v: u64) { b[o..o+8].copy_from_slice(&v.to_le_bytes()); }

/// Contadores monotonicos de 32 bits, nunca reducirlos modulo antes de restar.
pub fn disponibles(head: u32, tail: u32) -> Result<u32, No> {
    if es_error_pri(head) || es_error_pri(tail) { return Err(No::Puntero); }
    let n = head.wrapping_sub(tail);
    if n > HUECOS { Err(No::Puntero) } else { Ok(n) }
}
pub fn paginas(bytes: usize) -> Result<u32, No> {
    if !(ENVUELTA..=MAX_BYTES).contains(&bytes) { return Err(No::Medida); }
    Ok(bytes.div_ceil(PAGINA) as u32)
}
pub fn cabecera_cola(b: &[u8]) -> Result<(), No> {
    if b.len() < 32 { return Err(No::Corta); }
    if b[..32] != crate::libos::cabecera_cpu_615() { return Err(No::Forma); }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marco { pub bytes: usize, pub paginas: u32, pub tipo: u8 }
impl Marco {
    /// Acepta cabecera parcial de 24 bytes para decidir cuanto copiar del anillo.
    pub fn leer(b: &[u8]) -> Result<Self, No> {
        if b.len() < ENVUELTA { return Err(No::Corta); }
        if u32d(b, 0) != MAGIC || u32d(b, 8) != 0xC000_0001 || u32d(b, 20) != 0 {
            return Err(No::Forma);
        }
        let tipo = (u32d(b, 12) >> 24) as u8;
        if u32d(b, 12) & 0x00FF_FFFF != 0x0010_DE7E || ![GMCAPI, RPC].contains(&tipo) {
            return Err(No::Protocolo);
        }
        let bytes = u32d(b, 4) as usize;
        let paginas = paginas(bytes)?;
        if u32d(b, 16) as usize != bytes - ENVUELTA { return Err(No::Medida); }
        Ok(Self { bytes, paginas, tipo })
    }
}

/// RPC encapsulada de diagnostico; no secuenciadores ni respuestas RM 570.
pub fn aviso_rpc(b: &[u8]) -> Result<u32, No> {
    let m = Marco::leer(b)?;
    if m.tipo != RPC || m.bytes > b.len() || m.bytes < ENVUELTA + 32 { return Err(No::Corta); }
    let d = &b[ENVUELTA..m.bytes];
    if u32d(d, 0) != crate::rpc::VERSION_3_0 || u32d(d, 4) != crate::rpc::FIRMA_VRPC
        || u32d(d, 8) as usize != d.len() { return Err(No::Forma); }
    let funcion = u32d(d, 12);
    if !crate::rpc::informativo(funcion) { return Err(No::Protocolo); }
    Ok(funcion)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gmc<'a> {
    pub comando: u32, pub respuesta: bool, pub secuencia: u64,
    pub estado_o_maximo: u32, pub datos: &'a [u8],
}
impl<'a> Gmc<'a> {
    pub fn leer(b: &'a [u8]) -> Result<Self, No> {
        let m = Marco::leer(b)?;
        if m.tipo != GMCAPI { return Err(No::Protocolo); }
        if m.bytes > b.len() || m.bytes < ENVUELTA + GMC_CABECERA { return Err(No::Corta); }
        let g = &b[ENVUELTA..m.bytes];
        let comando = u32d(g, 0);
        if comando & 0xFE00_0000 != 0 || g[20..40].iter().any(|x| *x != 0) { return Err(No::Forma); }
        if u32d(g, 4) as usize != g.len() - GMC_CABECERA { return Err(No::Medida); }
        Ok(Self { comando: comando & 0x00FF_FFFF, respuesta: comando & RESPUESTA != 0,
            secuencia: u64d(g, 8), estado_o_maximo: u32d(g, 16), datos: &g[GMC_CABECERA..] })
    }
    pub fn respuesta_init(&self, secuencia: u64) -> Result<Config, No> {
        if !self.respuesta || self.comando != INIT || self.secuencia != secuencia { return Err(No::Correlacion); }
        if self.estado_o_maximo != 0 { return Err(No::Estado(self.estado_o_maximo)); }
        if self.datos.len() > MAX_RESPUESTA { return Err(No::Medida); }
        config(self.datos)
    }
}

/// Constructor cerrado: solo GSP_INIT. No recibe comandos del escritorio.
pub fn iniciar(b: &mut [u8], secuencia: u64, sistema: &Sistema) -> Result<usize, No> {
    if b.len() < PAGINA { return Err(No::Corta); }
    b[..PAGINA].fill(0);
    let mut kv = Kv { b: &mut b[ENVUELTA + GMC_CABECERA..PAGINA], n: 0 };
    kv.imm(1, sistema.id)?; kv.imm(2, sistema.subid)?; kv.imm(3, sistema.revision as u32)?;
    for (k, v) in [(0x1010, sistema.bar0), (0x1011, sistema.bar1), (0x1012, sistema.bar3),
                   (0x1013, 0), (0x1020, sistema.bdf as u64), (0x1031, 0), (0x1032, crate::orden::MAX_USER_VA)] {
        kv.entero(k, v)?;
    }
    kv.imm(0x70, 1)?; // HOST_ARCH_X86_64
    kv.imm(0x71, 0)?; // HOST_OS_OTHER: BMO no se anuncia como Linux.
    kv.imm(0x78, PAGINA as u32)?;
    kv.imm(0x10, crate::orden::ESPEJO_PCI)?;
    kv.imm(0x11, crate::orden::ESPEJO_PCI_BYTES)?;
    kv.entero(0x1001, 0)?; // conserva las banderas de plataforma del perfil BMO
    kv.entero(0x1110, 3)?; // interfaces RPC/eventos inestables, necesarias para el RM
    kv.texto(0x1100, b"BMO-X\0")?;
    // Registro: las mismas entradas cerradas de BMO, no bytes de Ring 3.
    for (nombre, valor) in crate::orden::REGISTRO {
        kv.texto_cero(0x3070, nombre)?;
        kv.imm(0x3071, valor)?;
    }
    let n = kv.n;
    let total = ENVUELTA + GMC_CABECERA + n;
    p32(b, 0, MAGIC); p32(b, 4, total as u32); p32(b, 8, 0xC000_0001);
    p32(b, 12, (GMCAPI as u32) << 24 | 0x0010_DE7E);
    p32(b, 16, (GMC_CABECERA + n) as u32);
    p32(b, 24, INIT); p32(b, 28, n as u32); p64(b, 32, secuencia);
    p32(b, 40, MAX_RESPUESTA as u32);
    Ok(total)
}
struct Kv<'a> { b: &'a mut [u8], n: usize }
impl Kv<'_> {
    fn word(&mut self, v: u64) -> Result<(), No> {
        if self.n + 8 > self.b.len() { return Err(No::Medida); }
        p64(self.b, self.n, v); self.n += 8; Ok(())
    }
    fn imm(&mut self, k: u16, v: u32) -> Result<(), No> { self.word(k as u64 | (v as u64) << 32) }
    fn entero(&mut self, k: u16, v: u64) -> Result<(), No> { self.word(k as u64 | 2 << 28 | 1 << 32)?; self.word(v) }
    fn texto(&mut self, k: u16, texto: &[u8]) -> Result<(), No> {
        self.word(k as u64 | 3 << 28 | (texto.len() as u64) << 32)?;
        for c in texto.chunks(8) { let mut w = [0; 8]; w[..c.len()].copy_from_slice(c); self.word(u64::from_le_bytes(w))?; }
        Ok(())
    }
    fn texto_cero(&mut self, k: u16, texto: &[u8]) -> Result<(), No> {
        let mut t = [0; 256];
        if texto.len() >= t.len() { return Err(No::Medida); }
        t[..texto.len()].copy_from_slice(texto); self.texto(k, &t[..texto.len()+1])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config { pub claves: u32, pub cliente: u32, pub dispositivo: u32, pub subdispositivo: u32 }
/// NVKV acotado; valida tambien claves desconocidas antes de saltarlas.
pub fn config(b: &[u8]) -> Result<Config, No> {
    if b.len() % 8 != 0 || b.is_empty() { return Err(No::Kv); }
    let mut r = Config::default();
    let mut o = 0;
    while o < b.len() {
        let w = u64d(b, o); o += 8;
        let op = (w >> 28) & 15;
        let k = w as u16;
        let index = (w >> 16) & 0xFFF;
        let n = (w >> 32) as usize;
        let bytes = match op {
            0 => 0, 1 | 4 => n.checked_mul(4).ok_or(No::Kv)?,
            2 | 5 => n.checked_mul(8).ok_or(No::Kv)?, 3 => n, _ => return Err(No::Kv),
        };
        if op != 0 && n == 0 { return Err(No::Kv); }
        let ocupado = bytes.checked_add(7).ok_or(No::Kv)? / 8 * 8;
        if ocupado > b.len() - o { return Err(No::Kv); }
        let claves = if op == 1 || op == 2 { n } else { 1 };
        if (k as usize).checked_add(claves-1).ok_or(No::Kv)? > u16::MAX as usize { return Err(No::Kv); }
        for i in 0..claves {
            let valor = match op { 0 => n as u64, 1 => u32d(b, o+i*4) as u64, 2 => u64d(b, o+i*8), _ => 0 };
            if index == 0 && (op <= 2) {
                match k as usize + i {
                    0xFF0 => r.cliente = u32::try_from(valor).map_err(|_| No::Kv)?,
                    0xFF1 => r.dispositivo = u32::try_from(valor).map_err(|_| No::Kv)?,
                    0xFF2 => r.subdispositivo = u32::try_from(valor).map_err(|_| No::Kv)?, _ => {}
                }
            }
        }
        r.claves = r.claves.checked_add(claves as u32).ok_or(No::Kv)?;
        o += ocupado;
    }
    Ok(r)
}

#[cfg(test)]
#[path = "mctp615_tests.rs"]
mod tests;
