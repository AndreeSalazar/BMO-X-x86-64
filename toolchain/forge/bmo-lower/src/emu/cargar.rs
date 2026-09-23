//! **Un `.bex` entero, montado en el emulador como lo monta el cargador del
//! kernel** (2026-09-18).
//!
//! Esta funcion estaba COPIADA en cuatro bancos de prueba (C, C++, Ada y
//! `bmo-enlazar`), cada uno con su version. Aqui vive la canonica, al lado del
//! emulador, y es la que usa el metro del emisor (`toolchain/tools/metro`).
//!
//! Hace lo que el cargador: las regiones en paginas de 4 KiB (codigo, luego
//! constantes, datos y ceros), y los relocs resueltos contra la base de cada
//! una.
//!
//! ** BEF2 (2026-09-19, `docs/plan/PLAN_BEF_NATIVO.md`): el juez del contrato
//! (`bef2::leer`) comprueba la imagen y aqui solo se COLOCA. El camino de BEF1
//! se fue en B6.

use bmo_abi::bef2::{self, Region};

use super::Machine;

const PAGE: usize = 4096;

/// Monta `bex` y deja el `rip` en su punto de entrada. Un `.bex` malformado
/// es un `Err` con el motivo, no un panico a medias.
pub fn cargar_bex(bex: &[u8]) -> Result<Machine, String> {
    let v = bef2::leer(bex).map_err(|f| format!("BEF2: {}", f.nombre()))?;
    let mut imagen = Vec::new();
    let mut solo_lectura = Vec::new();
    // Donde cayo cada region, por su numero (el que usan los relocs).
    let mut base = [usize::MAX; 4];
    for r in [Region::Codigo, Region::Constantes, Region::Datos, Region::Ceros] {
        let bytes = v.region(r);
        let ceros = if matches!(r, Region::Ceros) { v.ceros as usize } else { 0 };
        if bytes.is_empty() && ceros == 0 {
            continue;
        }
        while !imagen.is_empty() && imagen.len() % PAGE != 0 {
            // `0xCC` y no cero: si el flujo se sale del codigo, la maquina
            // para en vez de seguir por basura interpretable.
            imagen.push(0xCC);
        }
        base[r as usize] = imagen.len();
        let desde = imagen.len();
        imagen.extend_from_slice(bytes);
        imagen.resize(imagen.len() + ceros, 0);
        if matches!(r, Region::Codigo | Region::Constantes) {
            let hasta = (imagen.len() + PAGE - 1) / PAGE * PAGE;
            solo_lectura.push((desde as u64, hasta as u64));
        }
    }
    // Los relocs ya los comprobo el juez: cada uno cae dentro de su region.
    for r in v.relocs() {
        let (donde, destino) = (base[r.donde as usize], base[r.destino as usize]);
        if donde == usize::MAX || destino == usize::MAX {
            return Err("un reloc nombra una region que este .bex no lleva".into());
        }
        let at = donde + r.offset as usize;
        let valor = (destino as u64).wrapping_add(r.addend);
        imagen[at..at + 8].copy_from_slice(&valor.to_le_bytes());
    }
    let mut m = Machine::new(imagen);
    m.rip = base[Region::Codigo as usize] + v.entrada as usize;
    m.solo_lectura = solo_lectura;
    Ok(m)
}

impl Machine {
    /// ** LLAMAR a una funcion del codigo cargado (2026-09-23, el emisor de
    /// SPIR-V): empuja como direccion de vuelta el FINAL del codigo y salta a
    /// `rip`. Cuando la funcion hace `ret`, `run` se para solo -- es la forma
    /// de ejecutar un trozo sin fingir un programa entero con su `exit`.
    pub fn llamar(&mut self, rip: usize) {
        let fin = self.code.len() as u64;
        self.regs[super::RSP] = self.regs[super::RSP].wrapping_sub(8);
        let sp = self.regs[super::RSP];
        self.escribir(sp, &fin.to_le_bytes());
        self.rip = rip;
    }
}
