//! **El escritor.** Lo que se le da se escribe en la disposicion canonica, se
//! le ponen sus hashes, y antes de devolver se LEE con el mismo lector que
//! usara el consumidor: el escritor no se cree a si mismo.
//!
//! [consumo]  NADA   corre al fabricar

use crate::*;

/// Un objetivo: el codigo de un modulo para una maquina.
#[derive(Clone, Copy, Debug)]
pub struct TargetIn<'a> {
    pub kind: u16,
    pub abi: u16,
    /// Bits de [`cpu`] que el codigo necesita.
    pub requires: u32,
    pub code: &'a [u8],
    pub init: u32,
    pub main: u32,
    pub frame_words: u32,
    /// Ranura `i` de la tabla de buffers del codigo = fila `slots[i]` de los
    /// buffers de su modulo.
    pub slots: &'a [u8],
    /// Quien lo emitio, en ASCII (informativo; lo que decide es `abi`).
    pub emitter: &'a [u8],
}

/// Un modulo: un punto de entrada, su interfaz y sus objetivos.
#[derive(Clone, Copy, Debug)]
pub struct ModuleIn<'a> {
    /// `ExecutionModel` de SPIR-V: 5 = `GLCompute`.
    pub model: u8,
    pub name: &'a [u8],
    pub local_size: [u32; 3],
    /// Bit `n`: el SPIR-V declara la capacidad `n` (`n < 64`).
    pub capabilities: u64,
    /// Cuantas capacidades de numero 64 o mas declara.
    pub caps_high: u16,
    pub spirv: &'a [u8],
    /// Ordenados por `(set, binding)`, sin repetir.
    pub bindings: &'a [Binding],
    /// Ordenados por `(kind, abi)`, sin repetir.
    pub targets: &'a [TargetIn<'a>],
}

/// Cuantos bytes mide el BSF de estos modulos.
pub fn size(modules: &[ModuleIn]) -> Result<usize, Fault> {
    let limits = Fault::at(What::Limits, 0);
    if modules.is_empty() || modules.len() > MAX_MODULES {
        return Err(limits);
    }
    let mut nb = 0;
    let mut nt = 0;
    for m in modules {
        if m.bindings.len() > MAX_BINDINGS || m.targets.len() > MAX_TARGETS {
            return Err(limits);
        }
        nb += m.bindings.len();
        nt += m.targets.len();
    }
    let mut cursor = blobs_start(modules.len(), nb, nt);
    for m in modules {
        cursor = align(cursor) + m.spirv.len();
        for t in m.targets {
            cursor = align(cursor) + t.code.len();
        }
        if cursor > MAX_BYTES {
            return Err(limits);
        }
    }
    Ok(cursor)
}

/// **Escribe** el BSF en `out` y devuelve cuantos bytes mide. `out` chico se
/// dice con cuanto hace falta.
pub fn write(modules: &[ModuleIn], out: &mut [u8]) -> Result<usize, Fault> {
    let n = size(modules)?;
    if out.len() < n {
        return Err(Fault::at(What::NoRoom { need: n }, 0));
    }
    let out = &mut out[..n];
    out.fill(0);
    let nb: usize = modules.iter().map(|m| m.bindings.len()).sum();
    let nt: usize = modules.iter().map(|m| m.targets.len()).sum();

    put32(out, 0, MAGIC);
    put16(out, 4, VERSION);
    put16(out, 6, HEADER_BYTES as u16);
    put32(out, 8, n as u32);
    put16(out, 12, modules.len() as u16);
    put16(out, 14, nb as u16);
    put16(out, 16, nt as u16);

    let tabla_b = HEADER_BYTES + modules.len() * MODULE_BYTES;
    let tabla_t = tabla_b + nb * BINDING_BYTES;
    let mut cursor = blobs_start(modules.len(), nb, nt);
    let (mut fb, mut ft) = (0usize, 0usize);
    for (i, m) in modules.iter().enumerate() {
        let r = HEADER_BYTES + i * MODULE_BYTES;
        if m.name.is_empty() || m.name.len() > MAX_NAME {
            return Err(Fault::at(What::Name, r));
        }
        out[r] = m.model;
        out[r + 1] = m.name.len() as u8;
        for k in 0..3 {
            put32(out, r + 4 + 4 * k, m.local_size[k]);
        }
        put16(out, r + 16, fb as u16);
        put16(out, r + 18, m.bindings.len() as u16);
        put16(out, r + 20, ft as u16);
        put16(out, r + 22, m.targets.len() as u16);
        let spirv_off = align(cursor);
        put32(out, r + 24, spirv_off as u32);
        put32(out, r + 28, m.spirv.len() as u32);
        out[r + 32..r + 40].copy_from_slice(&m.capabilities.to_le_bytes());
        put16(out, r + 40, m.caps_high);
        out[r + 48..r + 48 + m.name.len()].copy_from_slice(m.name);
        let spirv_hash = bmo_hash::hash(m.spirv);
        out[r + 80..r + 112].copy_from_slice(&spirv_hash);
        out[spirv_off..spirv_off + m.spirv.len()].copy_from_slice(m.spirv);
        cursor = spirv_off + m.spirv.len();

        for b in m.bindings {
            let r = tabla_b + fb * BINDING_BYTES;
            put32(out, r, b.set);
            put32(out, r + 4, b.binding);
            out[r + 8] = b.storage as u8;
            out[r + 9] = b.access;
            put32(out, r + 12, b.base_bytes);
            put32(out, r + 16, b.stride);
            fb += 1;
        }
        for t in m.targets {
            let r = tabla_t + ft * TARGET_BYTES;
            if t.slots.len() > MAX_BINDINGS || t.emitter.len() > MAX_EMITTER {
                return Err(Fault::at(What::Target, r));
            }
            put16(out, r, t.kind);
            put16(out, r + 2, t.abi);
            put32(out, r + 4, t.requires);
            let code_off = align(cursor);
            put32(out, r + 8, code_off as u32);
            put32(out, r + 12, t.code.len() as u32);
            put32(out, r + 16, t.init);
            put32(out, r + 20, t.main);
            put32(out, r + 24, t.frame_words);
            out[r + 28] = t.slots.len() as u8;
            out[r + 32..r + 48].fill(0xFF);
            out[r + 32..r + 32 + t.slots.len()].copy_from_slice(t.slots);
            out[r + 48..r + 48 + t.emitter.len()].copy_from_slice(t.emitter);
            out[r + 64..r + 96].copy_from_slice(&spirv_hash);
            out[r + 96..r + 128].copy_from_slice(&bmo_hash::hash(t.code));
            out[code_off..code_off + t.code.len()].copy_from_slice(t.code);
            cursor = code_off + t.code.len();
            ft += 1;
        }
    }
    let h = index_hash(out, tabla_t + nt * TARGET_BYTES);
    out[32..64].copy_from_slice(&h);

    // El escritor no se cree a si mismo.
    Bsf::parse(out)?.verify_all()?;
    Ok(n)
}

fn put16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
