//! **BMO C writes an OBJECT (`.bo`)** -- E2 of `docs/plan/PLAN_EL_ENLAZADOR.md`.
//!
//! [fase]     IMAGEN
//! [aparece]  BANCO
//! [carril]   VERDE -- lo caza `cargo test`: el banco lee el objeto con el contrato
//!
//! Es la fase IMAGEN --colocar secciones y cerrar referencias-- y su fallo
//! aparece en el BANCO: `src/tests/objeto.rs` lee el objeto con el mismo
//! contrato que usara el enlazador, asi que una reloc mal puesta se ve en
//! `cargo test` y no tres pasos mas alla, al enlazar.
//!
//! An image (`.bex`) and an object share every byte the codegen emits. What
//! changes is WHO closes the references:
//!
//! ```text
//!                           image (.bex)                object (.bo)
//!    call f  (f here)       codegen writes rel32        codegen writes rel32
//!    call f  (f elsewhere)  ERROR "no hay enlazado"     Rel32 -> undefined f
//!    lea [rip+string]       codegen, loader's pages     Rel32 -> .rodata + off
//!    lea [rip+global]       codegen, loader's pages     Rel32 -> .data/.bss + off
//!    lea [rip+extern g]     (it had its own copy)       Rel32 -> undefined g
//!    pointer in data        Reloc (region+offset)       Enlace::Region, the same
//!    pointer to extern      ERROR                       Abs64 -> undefined
//! ```
//!
//! ** The rip-relative ones are the reason this file exists. The codegen
//! computes them assuming the loader's layout of ONE unit -- code, then rodata
//! on the next page, then data. Linked with other units, other code sits in
//! between and every one of those distances would read the wrong bytes without
//! failing. In an object they are left at zero and handed to the linker.
//!
//! The contract these bytes follow is `bmo_abi::bef2::objeto` (a BEF2 with
//! the OBJETO flag, symbols and ENLACE as anexos), and `compile_to_object`
//! refuses to return anything `objeto::read` would reject.

use super::*;
use bmo_abi::bef2::objeto::{Clase, Enlace};
use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility, SECTION_UNDEFINED};

/// What a reference left for the linker points at.
#[derive(Clone, Debug)]
pub(super) enum Destino {
    /// A place inside one of this unit's own regions.
    Region(Region, u64),
    /// A name: defined in this unit or, if not, in another one.
    Simbolo(String),
}

/// Compiles ONE unit to an object instead of a program.
pub fn compile_to_object(program: &Program) -> Result<Vec<u8>> {
    let mut cg = Codegen::new(TargetProfile::Ring3App);
    cg.objeto = true;
    cg.enlace = program.enlace.clone();
    cg.emit_program(program)?;
    let bytes = cg.build_object();
    // The codegen must not hand out what the contract refuses: it is checked
    // here, once, with the reason -- not discovered by the linker later.
    if let Err(falta) = bmo_abi::bef2::objeto::read(&bytes) {
        return Err(CError::new(0, format!(
            "el objeto emitido no cumple el contrato (bef2::objeto): {falta:?} -- esto es un bug del compilador"
        )));
    }
    Ok(bytes)
}

impl Codegen {
    /// Is `name` a function this unit only knows by its prototype?
    pub(super) fn solo_prototipo(&self, name: &str) -> bool {
        !self.known_functions.contains(name) && self.enlace.prototipos.iter().any(|(n, ..)| n == name)
    }

    pub(super) fn build_object(&mut self) -> Vec<u8> {
        use bmo_abi::bef2::{self, Escritor};

        let all = core::mem::take(&mut self.code);
        let code = all[..self.instruction_end].to_vec();
        let rodata = all[self.instruction_end..self.string_data_end].to_vec();
        let data = all[self.string_data_end..].to_vec();
        let data_len = data.len() as u64;

        // ** BEF2 (2026-09-19): un objeto es un BEF2 con la bandera OBJETO. Las
        // cuatro regiones con su medida; los simbolos nombran REGIONES (0
        // codigo, 1 constantes, 2 datos, 3 ceros), no indices de una tabla.
        let mut b = Escritor::objeto();
        if self.quiere_pantalla && !self.sabe_componerse {
            b.quiere_pantalla();
        }
        let mut regiones: Vec<(Region, u64)> = vec![(Region::Codigo, code.len() as u64)];
        b.codigo(code);
        if !rodata.is_empty() {
            regiones.push((Region::Constantes, rodata.len() as u64));
            b.constantes(rodata);
        }
        if !data.is_empty() {
            regiones.push((Region::Datos, data_len));
            b.datos(data);
        }
        if self.bss_len > 0 {
            regiones.push((Region::Ceros, self.bss_len as u64));
            b.ceros(self.bss_len as u32);
        }

        let mut entradas: Vec<Symbol> = Vec::new();
        let mut cadenas: Vec<u8> = Vec::new();
        let mut por_nombre: HashMap<String, u32> = HashMap::new();
        let mut por_region: HashMap<u8, u32> = HashMap::new();
        let mut push = |entradas: &mut Vec<Symbol>, nombre: &str, kind: SymbolKind, local: bool, sec: u8, off: u64, size: u64| -> u32 {
            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(nombre.as_bytes());
            cadenas.push(0);
            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(nombre),
                virt_addr: off,
                size,
                kind: kind as u8,
                binding: if local { SymbolBinding::Local } else { SymbolBinding::Global } as u8,
                visibility: SymbolVisibility::Default as u8,
                section_idx: sec,
                _reserved: 0,
            });
            (entradas.len() - 1) as u32
        };

        // One Local symbol per region: what a reference "inside this unit"
        // is relative to.
        for (r, len) in &regiones {
            let n = match r {
                Region::Codigo => ".code",
                Region::Constantes => ".rodata",
                Region::Datos => ".data",
                Region::Ceros => ".bss",
            };
            let i = push(&mut entradas, n, SymbolKind::Section, true, *r as u8, 0, *len);
            por_region.insert(*r as u8, i);
        }
        let hay = |r: Region| regiones.iter().any(|(k, _)| *k == r);

        // Functions, in code order. A function the program did not write --a
        // synthesized one, like `__bmo_syscall_stub`-- is this unit's private
        // copy, and so is a `static` one.
        let escritas = self.known_functions.clone();
        let mut funcs: Vec<(usize, String)> = self.function_offsets.iter().map(|(n, o)| (*o, n.clone())).collect();
        funcs.sort();
        for (i, (off, n)) in funcs.iter().enumerate() {
            let fin = funcs.get(i + 1).map(|e| e.0).unwrap_or(self.instruction_end);
            let local = self.enlace.estaticos.contains(n) || !escritas.contains(n);
            let s = push(&mut entradas, n, SymbolKind::Function, local, Region::Codigo as u8, *off as u64, (fin - off) as u64);
            por_nombre.insert(n.clone(), s);
        }

        // Globals, in memory order. `func.name` statics and the compiler's own
        // `__bmo_*` buffers are private; an extern-only one is not defined here
        // (its bytes are dead padding: every emission path needs the name to
        // exist, and 8 bytes is cheaper than teaching all of them).
        let mut globs: Vec<(u32, String)> = self
            .global_offsets
            .iter()
            .filter(|(n, _)| !self.enlace.solo_externos.contains(*n))
            .map(|(n, (o, _))| (*o, n.clone()))
            .collect();
        globs.sort();
        let fin_total = data_len + self.bss_len as u64;
        for (i, (off, n)) in globs.iter().enumerate() {
            let off = *off as u64;
            let fin = globs.get(i + 1).map(|e| e.0 as u64).unwrap_or(fin_total);
            let (region, rel, lim) = if off < data_len {
                (Region::Datos, off, data_len)
            } else {
                (Region::Ceros, off - data_len, fin_total)
            };
            if !hay(region) {
                continue;
            }
            let local = self.enlace.estaticos.contains(n) || n.contains('.') || n.starts_with("__bmo");
            let s = push(&mut entradas, n, SymbolKind::Object, local, region as u8, rel, fin.min(lim) - off);
            por_nombre.insert(n.clone(), s);
        }

        // Undefined names, sorted so the object is the same bytes every time.
        let mut faltan: Vec<String> = self
            .obj_rel32
            .iter()
            .map(|(_, d)| d)
            .chain(self.obj_abs64.iter().map(|(_, d, _)| d))
            .filter_map(|d| match d {
                Destino::Simbolo(n) if !por_nombre.contains_key(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        faltan.sort();
        faltan.dedup();
        for n in faltan {
            let kind = if self.enlace.solo_externos.contains(&n) { SymbolKind::Object } else { SymbolKind::Function };
            let s = push(&mut entradas, &n, kind, false, SECTION_UNDEFINED, 0, 0);
            por_nombre.insert(n, s);
        }

        // -- Enlaces: lo que el enlazador resuelve. --
        let mut enlaces: Vec<Enlace> = Vec::new();
        // Los punteros a una region de esta misma unidad (`p = &tabla`): los
        // mismos relocs del ejecutable, con nombre de enlace.
        for r in core::mem::take(&mut self.relocs) {
            enlaces.push(Enlace {
                clase: Clase::Region,
                donde: r.donde,
                offset: r.offset,
                simbolo: r.destino as u32,
                addend: r.addend as i64,
            });
        }
        let resolver = |d: &Destino| -> (u32, i64) {
            match d {
                Destino::Region(r, off) => (por_region[&(*r as u8)], *off as i64),
                Destino::Simbolo(n) => (por_nombre[n], 0),
            }
        };
        for (at, d) in &self.obj_rel32 {
            let (sym, base) = resolver(d);
            enlaces.push(Enlace {
                clase: Clase::Rel32,
                donde: Region::Codigo,
                offset: *at as u32,
                simbolo: sym,
                addend: base - 4,
            });
        }
        for (at, d, suma) in &self.obj_abs64 {
            let (sym, base) = resolver(d);
            enlaces.push(Enlace {
                clase: Clase::Abs64,
                donde: Region::Datos,
                offset: *at,
                simbolo: sym,
                addend: base + suma,
            });
        }

        b.anexo(bef2::ANEXO_SIMBOLOS, bmo_abi::bef::symbols::en_bytes(&entradas, &cadenas));
        if !enlaces.is_empty() {
            let mut raw = Vec::with_capacity(enlaces.len() * bef2::objeto::ENLACE);
            for e in &enlaces {
                raw.extend_from_slice(&e.a_bytes());
            }
            b.anexo(bef2::ANEXO_ENLACE, raw);
        }
        b.construir().unwrap_or_default()
    }
}
