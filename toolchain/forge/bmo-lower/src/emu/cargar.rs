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
//! ** BEF2 desde el 2026-09-19 (B5 de `docs/plan/PLAN_BEF_NATIVO.md`): el
//! juez del contrato (`bef2::leer`) comprueba la imagen y aqui solo se COLOCA.
//! El camino de BEF1 se queda mientras `bex-link`, `bef-bootstrap` y los
//! demas escritores viejos no hayan mudado (B4); se despacha por el magic y
//! se ira entero en B6.

use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::sections::{SectionEntry, SectionKind};
use bmo_abi::bef2::{self, Region};

use super::Machine;

const PAGE: usize = 4096;

/// Monta `bex` y deja el `rip` en su punto de entrada. Un `.bex` malformado
/// es un `Err` con el motivo, no un panico a medias.
pub fn cargar_bex(bex: &[u8]) -> Result<Machine, String> {
    if bex.len() >= 4 && u32::from_le_bytes([bex[0], bex[1], bex[2], bex[3]]) == bef2::MAGIC {
        return cargar_bef2(bex);
    }
    cargar_bef1(bex)
}

/// BEF2: las cuatro regiones, cada una en su pagina, en el orden de la
/// cabecera; codigo y constantes de solo lectura, como en el kernel.
fn cargar_bef2(bex: &[u8]) -> Result<Machine, String> {
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

/// BEF1: la tabla de secciones. Se va en B6.
fn cargar_bef1(bex: &[u8]) -> Result<Machine, String> {
    let u64_en = |i: usize| -> Result<u64, String> {
        bex.get(i..i + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("el .bex se acaba en el byte {i}"))
    };
    let entrada = u64_en(24)? as usize;
    let tabla = u64_en(32)? as usize;
    let cuantas = bex.get(40..44)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
        .ok_or("el .bex no llega ni a la cabecera")?;

    let mut imagen = Vec::new();
    let mut base = [usize::MAX; 3];
    // ** Codigo y constantes: SOLO LECTURA, como los mapea el kernel (RX y
    // R+NX). Se apunta aqui y se entrega al final, con la imagen ya parcheada.
    let mut solo_lectura = Vec::new();
    for (kind, cod) in [
        (SectionKind::Code, 0usize),
        (SectionKind::RoData, 2usize),
        (SectionKind::Data, 1usize),
        (SectionKind::Bss, usize::MAX),
    ] {
        for i in 0..cuantas {
            let e = tabla + i * SectionEntry::SIZE;
            if *bex.get(e).ok_or("la tabla de secciones se sale del .bex")? != kind as u8 {
                continue;
            }
            let off = u64_en(e + 8)? as usize;
            let size = u64_en(e + 16)? as usize;
            let mem = u64_en(e + 24)? as usize;
            while !imagen.is_empty() && imagen.len() % PAGE != 0 {
                imagen.push(0xCC);
            }
            if cod != usize::MAX {
                base[cod] = imagen.len();
            }
            let desde = imagen.len();
            imagen.extend_from_slice(bex.get(off..off + size).ok_or("una seccion se sale del .bex")?);
            imagen.resize(imagen.len() + mem.saturating_sub(size), 0);
            if matches!(kind, SectionKind::Code | SectionKind::RoData) {
                let hasta = (imagen.len() + PAGE - 1) / PAGE * PAGE;
                solo_lectura.push((desde as u64, hasta as u64));
            }
        }
    }
    for i in 0..cuantas {
        let e = tabla + i * SectionEntry::SIZE;
        if bex[e] != SectionKind::Relocs as u8 {
            continue;
        }
        let off = u64_en(e + 8)? as usize;
        let size = u64_en(e + 16)? as usize;
        for k in 0..size / Relocation::SIZE {
            let r = off + k * Relocation::SIZE;
            let donde = u64_en(r)? as usize;
            let destino = u32::from_le_bytes(bex[r + 8..r + 12].try_into().unwrap()) as usize;
            let kind = bex[r + 12];
            let donde_sec = bex[r + 13] as usize;
            let addend = i64::from_le_bytes(bex[r + 16..r + 24].try_into().unwrap());
            if kind != RelocationKind::SeccionAbs64 as u8 {
                return Err(format!("reloc de tipo {kind}: el cargador solo aplica SeccionAbs64"));
            }
            let (Some(&b_donde), Some(&b_destino)) = (base.get(donde_sec), base.get(destino)) else {
                return Err("una reloc nombra una seccion que no existe".into());
            };
            let at = b_donde + donde;
            let valor = (b_destino as i64 + addend) as u64;
            imagen.get_mut(at..at + 8).ok_or("una reloc cae fuera de la imagen")?
                .copy_from_slice(&valor.to_le_bytes());
        }
    }
    let mut m = Machine::new(imagen);
    m.rip = entrada;
    m.solo_lectura = solo_lectura;
    Ok(m)
}
