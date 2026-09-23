//! **El lector**: de bytes que nadie ha comprobado a un [`Bsf`] en el que cada
//! fila, cada desplazamiento y cada hash ya se miraron. Por capas, de la mas
//! barata a la mas cara, y el primer NO para.
//!
//! 1. **Forma**: magia, version, medidas, techos.
//! 2. **Indice**: el hash de la cabecera + las filas. A partir de aqui las
//!    filas son las que escribio el fabricante.
//! 3. **Disposicion y referencias**: cada desplazamiento es el canonico, cada
//!    relleno y cada reservado es cero, cada modulo tiene sus filas en orden,
//!    cada entrada cae dentro de su codigo, cada codigo lleva el hash de SU
//!    SPIR-V.
//! 4. **Contenido**: el hash de cada SPIR-V y de cada codigo -- pero cuando
//!    se TOMA, no al abrir. [`ModuleView::spirv`] y [`TargetView::code`]
//!    devuelven los bytes solo si su hash cuadra, y no hay otra forma de
//!    llegar a ellos: **se comprueba todo lo que se usa, y nada se usa sin
//!    comprobar**. Medido el 23-09 en el Ryzen: las capas 1 a 3 cuestan
//!    0,5 us y el hash de todos los blobs de un modulo 7 us; hashear al abrir
//!    lo que no se va a ejecutar era pagar 14 veces lo que cuesta mirar.
//!    [`Bsf::verify_all`] las paga todas de golpe, para quien fabrica.
//!
//! La capa 5, releer el SPIR-V y re-emitir, es [`Bsf::deep`] y
//! [`Bsf::reproduce`]: cuesta lo que traducir, y la decide quien consume.
//!
//! [consumo]  NADA   corre cuando alguien abre un BSF

use crate::*;

const SPIRV_MAGIC: u32 = 0x0723_0203;

/// Un BSF comprobado hasta la capa 3 (la 4 se paga al tomar cada blob).
#[derive(Clone, Copy, Debug)]
pub struct Bsf<'a> {
    bytes: &'a [u8],
    nm: usize,
    nb: usize,
    nt: usize,
}

/// Un modulo de un [`Bsf`].
#[derive(Clone, Copy, Debug)]
pub struct ModuleView<'a> {
    bytes: &'a [u8],
    row: usize,
    tabla_b: usize,
    tabla_t: usize,
}

/// El codigo de un modulo para una maquina.
#[derive(Clone, Copy, Debug)]
pub struct TargetView<'a> {
    bytes: &'a [u8],
    row: usize,
}

fn zero(b: &[u8], from: usize, to: usize) -> Result<(), Fault> {
    match b[from..to].iter().position(|&x| x != 0) {
        Some(k) => Err(Fault::at(What::NotZero, from + k)),
        None => Ok(()),
    }
}

/// Un nombre: ASCII imprimible sin espacios, y ceros hasta el final del campo.
fn ascii(b: &[u8], from: usize, len: usize, field: usize, what: What) -> Result<(), Fault> {
    if b[from..from + len].iter().any(|&c| !(0x21..0x7F).contains(&c)) {
        return Err(Fault::at(what, from));
    }
    zero(b, from + len, from + field)
}

impl<'a> Bsf<'a> {
    /// **Comprueba** la forma, el indice y la disposicion (capas 1 a 3).
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Fault> {
        // 1. Forma.
        if bytes.len() < HEADER_BYTES {
            return Err(Fault::at(What::Short, 0));
        }
        if u32le(bytes, 0) != MAGIC {
            return Err(Fault::at(What::Magic, 0));
        }
        if u16le(bytes, 4) != VERSION {
            return Err(Fault::at(What::Version(u16le(bytes, 4)), 4));
        }
        if u16le(bytes, 6) as usize != HEADER_BYTES {
            return Err(Fault::at(What::HeaderBytes, 6));
        }
        let declared = u32le(bytes, 8);
        if declared as usize != bytes.len() || bytes.len() > MAX_BYTES {
            return Err(Fault::at(What::TotalBytes { declared, actual: bytes.len() }, 8));
        }
        let (nm, nb, nt) = (u16le(bytes, 12) as usize, u16le(bytes, 14) as usize, u16le(bytes, 16) as usize);
        if nm == 0 || nm > MAX_MODULES || nb > nm * MAX_BINDINGS || nt > nm * MAX_TARGETS {
            return Err(Fault::at(What::Limits, 12));
        }
        zero(bytes, 18, 32)?;
        let tabla_b = HEADER_BYTES + nm * MODULE_BYTES;
        let tabla_t = tabla_b + nb * BINDING_BYTES;
        let tables_end = tabla_t + nt * TARGET_BYTES;
        let blobs = blobs_start(nm, nb, nt);
        if blobs > bytes.len() {
            return Err(Fault::at(What::Layout, tables_end));
        }

        // 2. El indice.
        if index_hash(bytes, tables_end)[..] != bytes[32..64] {
            return Err(Fault::at(What::IndexHash, 32));
        }
        zero(bytes, tables_end, blobs)?;

        // 3. Disposicion y referencias.
        let bsf = Bsf { bytes, nm, nb, nt };
        let mut cursor = blobs;
        let (mut fb, mut ft) = (0usize, 0usize);
        for i in 0..nm {
            let r = HEADER_BYTES + i * MODULE_BYTES;
            let name_len = bytes[r + 1] as usize;
            if name_len == 0 || name_len > MAX_NAME {
                return Err(Fault::at(What::Name, r + 1));
            }
            zero(bytes, r + 2, r + 4)?;
            ascii(bytes, r + 48, name_len, MAX_NAME, What::Name)?;
            let name = &bytes[r + 48..r + 48 + name_len];
            if (0..i).any(|j| bsf.module(j).name() == name) {
                return Err(Fault::at(What::Name, r + 48));
            }
            let (m_fb, m_nb) = (u16le(bytes, r + 16) as usize, u16le(bytes, r + 18) as usize);
            let (m_ft, m_nt) = (u16le(bytes, r + 20) as usize, u16le(bytes, r + 22) as usize);
            if m_fb != fb || m_ft != ft || m_nb > MAX_BINDINGS || m_nt > MAX_TARGETS || fb + m_nb > nb || ft + m_nt > nt {
                return Err(Fault::at(What::Order, r + 16));
            }
            zero(bytes, r + 42, r + 48)?;
            zero(bytes, r + 112, r + 128)?;

            // Su SPIR-V.
            let (off, len) = (u32le(bytes, r + 24) as usize, u32le(bytes, r + 28) as usize);
            cursor = blob(bytes, cursor, off, len, r + 24)?;
            if len < 20 || len % 4 != 0 || u32le(bytes, off) != SPIRV_MAGIC {
                return Err(Fault::at(What::SpirvMagic, off));
            }

            // Sus buffers: en orden estricto, con acceso y clase que existen.
            let mut antes: Option<(u32, u32)> = None;
            for k in 0..m_nb {
                let b = tabla_b + (fb + k) * BINDING_BYTES;
                let key = (u32le(bytes, b), u32le(bytes, b + 4));
                if antes.is_some_and(|a| a >= key) {
                    return Err(Fault::at(What::Order, b));
                }
                antes = Some(key);
                let (storage, access) = (bytes[b + 8], bytes[b + 9]);
                if storage > 1 || access > (READS | WRITES) || (access & WRITES != 0 && storage == 0) {
                    return Err(Fault::at(What::Access, b + 8));
                }
                zero(bytes, b + 10, b + 12)?;
                zero(bytes, b + 20, b + 24)?;
            }

            // Sus objetivos.
            let spirv_hash = &bytes[r + 80..r + 112];
            let mut antes: Option<(u16, u16)> = None;
            for k in 0..m_nt {
                let t = tabla_t + (ft + k) * TARGET_BYTES;
                let key = (u16le(bytes, t), u16le(bytes, t + 2));
                if key.0 == 0 || key.1 == 0 || u32le(bytes, t + 4) & !cpu::KNOWN != 0 {
                    return Err(Fault::at(What::Target, t));
                }
                if antes.is_some_and(|a| a >= key) {
                    return Err(Fault::at(What::Order, t));
                }
                antes = Some(key);
                let (off, len) = (u32le(bytes, t + 8) as usize, u32le(bytes, t + 12) as usize);
                cursor = blob(bytes, cursor, off, len, t + 8)?;
                if len == 0 || u32le(bytes, t + 16) as usize >= len || u32le(bytes, t + 20) as usize >= len {
                    return Err(Fault::at(What::Entry, t + 16));
                }
                slots(bytes, t, fb, m_nb, tabla_b)?;
                ascii(bytes, t + 48, (0..MAX_EMITTER).take_while(|&j| bytes[t + 48 + j] != 0).count(), MAX_EMITTER, What::Target)?;
                if &bytes[t + 64..t + 96] != spirv_hash {
                    return Err(Fault::at(What::StaleCode, t + 64));
                }
            }
            fb += m_nb;
            ft += m_nt;
        }
        if fb != nb || ft != nt {
            return Err(Fault::at(What::Order, 14));
        }
        if cursor != bytes.len() {
            return Err(Fault::at(What::Layout, cursor));
        }

        Ok(bsf)
    }

    /// **La capa 4 entera**: el hash de cada SPIR-V y de cada codigo. Para
    /// quien fabrica o archiva; quien ejecuta paga solo lo que toma.
    pub fn verify_all(&self) -> Result<(), Fault> {
        for m in self.modules() {
            m.spirv()?;
            for t in m.targets() {
                t.code()?;
            }
        }
        Ok(())
    }

    /// Los bytes enteros, tal cual llegaron.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn module_count(&self) -> usize {
        self.nm
    }

    /// El modulo `i` (`i < module_count()`).
    pub fn module(&self, i: usize) -> ModuleView<'a> {
        assert!(i < self.nm);
        let tabla_b = HEADER_BYTES + self.nm * MODULE_BYTES;
        ModuleView { bytes: self.bytes, row: HEADER_BYTES + i * MODULE_BYTES, tabla_b, tabla_t: tabla_b + self.nb * BINDING_BYTES }
    }

    pub fn modules(&self) -> impl Iterator<Item = ModuleView<'a>> + '_ {
        (0..self.nm).map(|i| self.module(i))
    }

    /// El modulo que se llama `name`.
    pub fn find(&self, name: &[u8]) -> Option<ModuleView<'a>> {
        self.modules().find(|m| m.name() == name)
    }

    /// Cuantos objetivos hay en total (de todos los modulos).
    pub fn target_count(&self) -> usize {
        self.nt
    }
}

/// Un blob empieza donde la disposicion canonica dice, con relleno cero antes,
/// y cabe. Devuelve donde acaba.
fn blob(bytes: &[u8], cursor: usize, off: usize, len: usize, at: usize) -> Result<usize, Fault> {
    if off != align(cursor) || off.checked_add(len).map_or(true, |e| e > bytes.len()) {
        return Err(Fault::at(What::Layout, at));
    }
    zero(bytes, cursor, off)?;
    Ok(off + len)
}

/// Las ranuras de un objetivo: cada una es una fila de SU modulo, ninguna dos
/// veces, el resto `0xFF`; y todo buffer que el codigo toca tiene ranura.
fn slots(bytes: &[u8], t: usize, fb: usize, nb: usize, tabla_b: usize) -> Result<(), Fault> {
    let n = bytes[t + 28] as usize;
    if n > nb {
        return Err(Fault::at(What::Target, t + 28));
    }
    zero(bytes, t + 29, t + 32)?;
    let s = &bytes[t + 32..t + 48];
    for i in 0..n {
        if s[i] as usize >= nb || s[..i].contains(&s[i]) {
            return Err(Fault::at(What::Target, t + 32 + i));
        }
    }
    if let Some(k) = s[n..].iter().position(|&x| x != 0xFF) {
        return Err(Fault::at(What::Target, t + 32 + n + k));
    }
    for k in 0..nb {
        let access = bytes[tabla_b + (fb + k) * BINDING_BYTES + 9];
        if access != 0 && !s[..n].contains(&(k as u8)) {
            return Err(Fault::at(What::Target, t + 32));
        }
    }
    Ok(())
}

impl<'a> ModuleView<'a> {
    /// `ExecutionModel` de SPIR-V.
    pub fn model(&self) -> u8 {
        self.bytes[self.row]
    }

    pub fn name(&self) -> &'a [u8] {
        &self.bytes[self.row + 48..self.row + 48 + self.bytes[self.row + 1] as usize]
    }

    pub fn local_size(&self) -> [u32; 3] {
        [u32le(self.bytes, self.row + 4), u32le(self.bytes, self.row + 8), u32le(self.bytes, self.row + 12)]
    }

    /// Bit `n`: el SPIR-V declara la capacidad `n`.
    pub fn capabilities(&self) -> u64 {
        u64le(self.bytes, self.row + 32)
    }

    /// Cuantas capacidades de numero 64 o mas.
    pub fn caps_high(&self) -> u16 {
        u16le(self.bytes, self.row + 40)
    }

    /// **Su SPIR-V, si es el del hash** (capa 4 de este blob). Cada llamada
    /// hashea: se toma una vez.
    pub fn spirv(&self) -> Result<&'a [u8], Fault> {
        let b = self.spirv_raw();
        if bmo_hash::hash(b)[..] != *self.spirv_hash() {
            return Err(Fault::at(What::SpirvHash, self.row + 80));
        }
        Ok(b)
    }

    /// Cuantos bytes mide su SPIR-V (sin tomarlo).
    pub fn spirv_len(&self) -> usize {
        u32le(self.bytes, self.row + 28) as usize
    }

    fn spirv_raw(&self) -> &'a [u8] {
        let off = u32le(self.bytes, self.row + 24) as usize;
        &self.bytes[off..off + self.spirv_len()]
    }

    /// Donde empieza su fila en el fichero.
    pub fn row_offset(&self) -> usize {
        self.row
    }

    /// Donde empieza su SPIR-V en el fichero.
    pub fn spirv_offset(&self) -> usize {
        u32le(self.bytes, self.row + 24) as usize
    }

    pub fn spirv_hash(&self) -> &'a [u8; 32] {
        self.bytes[self.row + 80..self.row + 112].try_into().unwrap()
    }

    pub fn binding_count(&self) -> usize {
        u16le(self.bytes, self.row + 18) as usize
    }

    /// La fila `k` de sus buffers.
    pub fn binding(&self, k: usize) -> Binding {
        assert!(k < self.binding_count());
        let b = self.tabla_b + (u16le(self.bytes, self.row + 16) as usize + k) * BINDING_BYTES;
        let x = self.bytes;
        Binding { set: u32le(x, b), binding: u32le(x, b + 4), storage: x[b + 8] == 1, access: x[b + 9], base_bytes: u32le(x, b + 12), stride: u32le(x, b + 16) }
    }

    pub fn bindings(&self) -> impl Iterator<Item = Binding> + '_ {
        (0..self.binding_count()).map(|k| self.binding(k))
    }

    pub fn targets(&self) -> impl Iterator<Item = TargetView<'a>> + '_ {
        let first = u16le(self.bytes, self.row + 20) as usize;
        let n = u16le(self.bytes, self.row + 22) as usize;
        (first..first + n).map(|k| TargetView { bytes: self.bytes, row: self.tabla_t + k * TARGET_BYTES })
    }

    /// **El codigo para ESTA maquina**: su `kind`, su `abi`, y que no pida a la
    /// CPU nada que `cpu_has` no tenga. `None` no es un fallo: es el JIT.
    pub fn target(&self, kind: u16, abi: u16, cpu_has: u32) -> Option<TargetView<'a>> {
        self.targets().find(|t| t.kind() == kind && t.abi() == abi && t.requires() & !cpu_has == 0)
    }
}

impl<'a> TargetView<'a> {
    pub fn kind(&self) -> u16 {
        u16le(self.bytes, self.row)
    }

    pub fn abi(&self) -> u16 {
        u16le(self.bytes, self.row + 2)
    }

    pub fn requires(&self) -> u32 {
        u32le(self.bytes, self.row + 4)
    }

    /// **Su codigo, si es el del hash** (capa 4 de este blob). Cada llamada
    /// hashea: se toma una vez, justo antes de sellarlo.
    pub fn code(&self) -> Result<&'a [u8], Fault> {
        let off = u32le(self.bytes, self.row + 8) as usize;
        let b = &self.bytes[off..off + self.code_len()];
        if bmo_hash::hash(b)[..] != *self.code_hash() {
            return Err(Fault::at(What::CodeHash, self.row + 96));
        }
        Ok(b)
    }

    /// Cuantos bytes mide su codigo (sin tomarlo).
    pub fn code_len(&self) -> usize {
        u32le(self.bytes, self.row + 12) as usize
    }

    /// Desplazamiento de `init` dentro de [`TargetView::code`].
    pub fn init(&self) -> usize {
        u32le(self.bytes, self.row + 16) as usize
    }

    /// Desplazamiento de `main` dentro de [`TargetView::code`].
    pub fn main(&self) -> usize {
        u32le(self.bytes, self.row + 20) as usize
    }

    pub fn frame_words(&self) -> usize {
        u32le(self.bytes, self.row + 24) as usize
    }

    /// Ranura `i` de la tabla del codigo = fila `slots()[i]` de su modulo.
    pub fn slots(&self) -> &'a [u8] {
        &self.bytes[self.row + 32..self.row + 32 + self.bytes[self.row + 28] as usize]
    }

    pub fn emitter(&self) -> &'a [u8] {
        let f = &self.bytes[self.row + 48..self.row + 64];
        &f[..f.iter().position(|&c| c == 0).unwrap_or(f.len())]
    }

    pub fn code_hash(&self) -> &'a [u8; 32] {
        self.bytes[self.row + 96..self.row + 128].try_into().unwrap()
    }
}
