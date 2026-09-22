//! Tabla de simbolos BEF -- para debug + dynamic linking + backtraces.
//!
//! Reemplaza `.symtab`/`.dynsym` (ELF) y `IMAGE_SYMBOL` (PE/COFF). Una sola
//! tabla con visibilidad y binding explicitos.


use crate::bmo_abi::primitives::{bx_u32, bx_u64, bx_u8};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolKind {
    /// Marcador (no apunta a nada).
    NoType = 0x00,
    /// Funcion.
    Function = 0x01,
    /// Dato (variable global, constante).
    Object = 0x02,
    /// Seccion entera (simbolo sintetizado).
    Section = 0x03,
    /// Archivo de origen (debug).
    File = 0x04,
    /// TLS slot.
    Tls = 0x05,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolBinding {
    /// Local -- solo visible dentro del modulo.
    Local = 0x00,
    /// Global -- visible para linking dinamico.
    Global = 0x01,
    /// Weak -- global pero puede ser sobrescrito.
    Weak = 0x02,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolVisibility {
    /// Default -- visible segun binding.
    Default = 0x00,
    /// Hidden -- global pero no exportable a otros modulos.
    Hidden = 0x01,
    /// Internal -- solo el linker lo ve.
    Internal = 0x02,
    /// Protected -- visible global pero no preemptable.
    Protected = 0x03,
}

/// Una entrada del symbol table -- 32 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Symbol {
    /// Offset al string del nombre (en seccion Symbols).
    pub name_off: bx_u32,
    /// Hash **FNV-1a de 32 bits** del nombre. Ver [`name_hash`].
    ///
    /// [!] Este campo decia "Hash BLAKE3-32" y **nadie lo calculaba**: no habia
    /// ni una funcion de hash en toda la crate. Corregido el 2026-08-14 al
    /// escribir el primer productor de esta seccion -- se puso el hash que de
    /// verdad se calcula en vez de dejar el nombre del que no.
    pub name_hash: bx_u32,
    /// Direccion virtual relativa al base.
    pub virt_addr: bx_u64,
    /// Medida en bytes.
    pub size: bx_u64,
    /// `SymbolKind`.
    pub kind: bx_u8,
    /// `SymbolBinding`.
    pub binding: bx_u8,
    /// `SymbolVisibility`.
    pub visibility: bx_u8,
    /// Indice de seccion donde vive, en la TABLA de secciones del fichero
    /// (0xFF = ABS, 0xFE = COMMON, [`SECTION_UNDEFINED`] = no vive aqui).
    pub section_idx: bx_u8,
    /// Reservado.
    pub _reserved: bx_u32,
}
const _: () = assert!(core::mem::size_of::<Symbol>() == 32);

/// **El hash de un nombre de simbolo: FNV-1a de 32 bits.**
///
/// # Por que FNV-1a y no algo criptografico
///
/// Este hash no protege nada: sirve para **descartar rapido** antes de comparar
/// la cadena. Quien busca `SHA1_Update` en una tabla de mil simbolos compara mil
/// enteros y hace un `strcmp` sobre el que casa. Un hash criptografico costaria
/// mas que el `strcmp` que ahorra.
///
/// Y tiene que poder calcularse **dentro del kernel**: seis lineas, sin
/// reservas, sin tablas. Un BLAKE3 en `no_std` y sin `alloc` seria meter una
/// dependencia en el anillo cero para elegir entre dos cadenas.
///
/// [!] Una colision NO es un fallo: el que busca compara el nombre igualmente.
/// El hash decide a quien NO mirar, nunca a quien aceptar.
/// `section_idx` of a symbol this object USES and does not define: the linker
/// has to find it in another object. Only legal in an object
/// ([`crate::bef::header::BefFlags::OBJECT`]); a `.bex` with one is refused.
/// 0xFF and 0xFE were already taken by ABS and COMMON.
pub const SECTION_UNDEFINED: bx_u8 = 0xFD;

/// **Los bytes de una tabla de simbolos**: cabecera (`TablaCadenas`), entradas
/// y cadenas. Es lo que viaja en el anexo `SIMBOLOS` de BEF2 -- de un objeto,
/// para enlazar; de un ejecutable, para que el DIRECTOR anote una autopsia.
pub fn en_bytes(entradas: &[Symbol], cadenas: &[u8]) -> alloc::vec::Vec<u8> {
    let cab = TablaCadenas::de(entradas.len() as u32);
    let mut data = alloc::vec::Vec::with_capacity(
        TablaCadenas::SIZE + entradas.len() * Symbol::SIZE + cadenas.len(),
    );
    data.extend_from_slice(&cab.count.to_le_bytes());
    data.extend_from_slice(&cab._reserved.to_le_bytes());
    for e in entradas {
        // Campo a campo y en little-endian: los mismos bytes que `#[repr(C)]`
        // en x86-64, sin depender de que el struct este alineado en memoria.
        data.extend_from_slice(&e.name_off.to_le_bytes());
        data.extend_from_slice(&e.name_hash.to_le_bytes());
        data.extend_from_slice(&e.virt_addr.to_le_bytes());
        data.extend_from_slice(&e.size.to_le_bytes());
        data.push(e.kind);
        data.push(e.binding);
        data.push(e.visibility);
        data.push(e.section_idx);
        data.extend_from_slice(&e._reserved.to_le_bytes());
    }
    data.extend_from_slice(cadenas);
    data
}

pub fn name_hash(name: &str) -> bx_u32 {
    let mut h: u32 = 0x811C_9DC5; // offset basis
    for b in name.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193); // prime
    }
    h
}

impl Symbol {
    pub const SIZE: usize = 32;

    pub fn kind(&self) -> Option<SymbolKind> {
        match self.kind {
            0x00 => Some(SymbolKind::NoType),
            0x01 => Some(SymbolKind::Function),
            0x02 => Some(SymbolKind::Object),
            0x03 => Some(SymbolKind::Section),
            0x04 => Some(SymbolKind::File),
            0x05 => Some(SymbolKind::Tls),
            _ => None,
        }
    }

    pub fn binding(&self) -> Option<SymbolBinding> {
        match self.binding {
            0x00 => Some(SymbolBinding::Local),
            0x01 => Some(SymbolBinding::Global),
            0x02 => Some(SymbolBinding::Weak),
            _ => None,
        }
    }
}

/// Vista del symbol table.
pub struct SymbolTable<'a> {
    pub entries: &'a [Symbol],
    pub strings: &'a [u8],
}

impl<'a> SymbolTable<'a> {
    pub fn parse(section_bytes: &'a [u8], entry_count: u32) -> Result<Self, &'static str> {
        let needed = entry_count as usize * Symbol::SIZE;
        if section_bytes.len() < needed {
            return Err("symbol table demasiado chica");
        }
        let raw_ptr = section_bytes.as_ptr();
        if (raw_ptr as usize) % core::mem::align_of::<Symbol>() != 0 {
            return Err("symbol table pointer mal alineado");
        }
        let ptr = raw_ptr as *const Symbol;
        let entries = unsafe { core::slice::from_raw_parts(ptr, entry_count as usize) };
        let strings = &section_bytes[needed..];
        Ok(Self { entries, strings })
    }

    /// The name, NUL-terminated.
    ///
    /// ** 2026-09-17: this read a two-byte LENGTH PREFIX, and the only producer
    /// of this section in the tree --BMO C, `codegen/bex.rs::seccion_de_simbolos`--
    /// writes names ending in ZERO. Nobody called this, so nothing broke; the
    /// day the linker did, every name would have been read two bytes off. A
    /// format with a writer and a reader that never met is not defined, only
    /// written (the lesson of `TablaCadenas`). The contract is the producer's.
    pub fn name_of(&self, sym: &Symbol) -> Option<&'a str> {
        let rest = self.strings.get(sym.name_off as usize..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        core::str::from_utf8(&rest[..end]).ok()
    }
}

/// **La cabecera de una tabla con cadenas detras**: cuantas entradas de
/// medida fijo vienen, y despues los nombres.
///
/// ```text
///   [TablaCadenas][entrada; count][cadenas]
///                  ^^^^^^^^^^^^^^  `name_off` es relativo a AQUI
/// ```
///
/// Vivia en `sections.rs` (BEF1) y se quedo porque los simbolos la usan: los
/// bytes del anexo SIMBOLOS no cambiaron al cambiar el contenedor.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct TablaCadenas {
    /// Cuantas entradas de medida fijo vienen detras.
    pub count: bx_u32,
    /// Reservado. **Debe ser cero** -- misma regla que el resto del formato:
    /// un campo futuro no puede heredar basura de un productor de hoy.
    pub _reserved: bx_u32,
}
const _: () = assert!(core::mem::size_of::<TablaCadenas>() == 8);

impl TablaCadenas {
    pub const SIZE: usize = 8;

    pub const fn de(count: u32) -> Self {
        Self { count, _reserved: 0 }
    }

    /// Lee la cabecera y devuelve `(count, donde_empiezan_las_cadenas)`.
    ///
    /// `None` si no da ni para la cabecera, o si el numero de entradas que
    /// declara no cabe en lo que mide. Un `count` inventado haria que el lector
    /// recorriera cadenas creyendo que son entradas -- que es exactamente el
    /// fallo que esta cabecera viene a cerrar.
    pub fn leer(data: &[u8], medida_de_entrada: usize) -> Option<(usize, usize)> {
        if data.len() < Self::SIZE {
            return None;
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let fin = Self::SIZE.checked_add(count.checked_mul(medida_de_entrada)?)?;
        if fin > data.len() {
            return None;
        }
        Some((count, fin))
    }
}
