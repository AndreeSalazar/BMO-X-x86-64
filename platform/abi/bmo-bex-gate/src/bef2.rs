//! **La puerta, para BEF2.** Las mismas comprobaciones que
//! `bmo_abi::bef2::lector`, escritas aqui sin `alloc` y sin dependencias
//! porque esto corre en Ring 0. Una prueba de `bmo-abi` ata las dos copias.
//!
//! # Lo que esto hace, y por que el kernel no cambia
//!
//! BEF2 no tiene tabla de secciones: tiene CUATRO REGIONES en sitio fijo de la
//! cabecera y una tabla de ANEXOS. El cargador del kernel, en cambio, sabe
//! recorrer "secciones con tipo" desde hace meses y esta probado.
//!
//! ** Asi que esto PRESENTA las regiones y los anexos como secciones: mismo
//! tipo (`CODE`, `RODATA`, `DATA`, `BSS`, `RELOCS`...), mismo `indice` con el
//! que la firma las nombra. El kernel recibe lo que ya sabia recibir y no se
//! entera del cambio de formato -- que es justo lo que hace que este paso se
//! pueda dar sin tocar Ring 0.

use super::*;

/// `b"BEF2"`.
pub const MAGIC: u32 = u32::from_le_bytes(*b"BEF2");
/// La cabecera de BEF2.
pub const CABECERA2: usize = 64;
/// Una entrada de la tabla de anexos.
pub const ANEXO: usize = 16;
/// Tope de anexos.
pub const MAX_ANEXOS: usize = 16;
/// El ABI que habla.
pub const ABI: u8 = 2;

/// Banderas de la cabecera.
pub const EJECUTABLE: u8 = 1 << 0;
pub const OBJETO: u8 = 1 << 1;
pub const QUIERE_PANTALLA: u8 = 1 << 2;
pub const BANDERAS: u8 = EJECUTABLE | OBJETO | QUIERE_PANTALLA;

/// Lo que el kernel sabe preservar en un cambio de contexto: x87 y SSE.
/// Crece el dia que `trap.rs` guarde el estado ancho, y NO antes.
pub const XCR0_PRESERVADO: u64 = (1 << 0) | (1 << 1);

/// Tipos de anexo.
pub const ANEXO_RELOCS: u8 = 0x01;
pub const ANEXO_FIRMA: u8 = 0x02;
pub const ANEXO_REQUISITOS: u8 = 0x03;
pub const ANEXO_RECURSOS: u8 = 0x04;
pub const ANEXO_MANIFIESTO: u8 = 0x05;
pub const ANEXO_KATANAS: u8 = 0x06;
pub const ANEXO_SIMBOLOS: u8 = 0x07;

/// El tipo de seccion con el que se le presenta al kernel cada anexo. Un anexo
/// que este kernel no conoce se presenta como `DESCONOCIDO`, y `se_carga` dice
/// que no: data para otro, se salta.
const DESCONOCIDO: u8 = 0x7F;

fn kind_de_anexo(tipo: u8) -> u8 {
    match tipo {
        ANEXO_RELOCS => RELOCS,
        ANEXO_FIRMA => SIGNATURE,
        ANEXO_REQUISITOS => REQUISITOS,
        ANEXO_RECURSOS => 0x0B,
        ANEXO_MANIFIESTO => 0x09,
        ANEXO_KATANAS => 0x16,
        ANEXO_SIMBOLOS => 0x08,
        _ => DESCONOCIDO,
    }
}

/// Un trozo de la cabecera: `{offset u32, bytes u32}`.
fn tramo(prologo: &[u8], o: usize) -> Option<(u64, u64)> {
    Some((u32_en(prologo, o)? as u64, u32_en(prologo, o + 4)? as u64))
}

/// Cuantos anexos declara.
fn anexos(prologo: &[u8]) -> usize {
    u32_en(prologo, 20).unwrap_or(0) as usize
}

/// La entrada `i` de la tabla de anexos: `(tipo, offset, bytes)`.
fn anexo(prologo: &[u8], i: usize) -> Option<(u8, u64, u64)> {
    let e = CABECERA2 + i * ANEXO;
    let tipo = *prologo.get(e)?;
    // El relleno de la entrada es cero, o no es una entrada de este formato.
    if *prologo.get(e + 1)? | *prologo.get(e + 2)? | *prologo.get(e + 3)? != 0 {
        return None;
    }
    if u32_en(prologo, e + 12)? != 0 {
        return None;
    }
    Some((
        tipo,
        u32_en(prologo, e + 4)? as u64,
        u32_en(prologo, e + 8)? as u64,
    ))
}

/// Las regiones que EXISTEN, en orden, con el indice de hash que les da la
/// firma: `(kind, offset, file_size, mem_size, indice)`.
fn region(prologo: &[u8], n: usize) -> Option<(u8, u64, u64, u64, usize)> {
    let (c_off, c_len) = tramo(prologo, 24)?;
    let (r_off, r_len) = tramo(prologo, 32)?;
    let (d_off, d_len) = tramo(prologo, 40)?;
    let ceros = u32_en(prologo, 48)? as u64;
    let mut k = 0usize;
    for (kind, off, len, mem, indice) in [
        (CODE, c_off, c_len, c_len, 0usize),
        (RODATA, r_off, r_len, r_len, 1),
        (DATA, d_off, d_len, d_len, 2),
        (BSS, 0, 0, ceros, 3),
    ] {
        if mem == 0 {
            continue;
        }
        if k == n {
            return Some((kind, off, len, mem, indice));
        }
        k += 1;
    }
    None
}

/// Cuantas regiones tiene.
fn cuantas_regiones(prologo: &[u8]) -> usize {
    (0..4).filter(|n| region(prologo, *n).is_some()).count()
}

/// La seccion `i` tal y como la ve el kernel: primero las regiones, luego los
/// anexos en el orden de su tabla.
pub(crate) fn seccion(prologo: &[u8], i: usize) -> Option<Seccion> {
    let regiones = cuantas_regiones(prologo);
    if i < regiones {
        let (kind, off, file_size, mem_size, indice) = region(prologo, i)?;
        return Some(Seccion {
            indice,
            kind,
            flags: if kind == CODE { SECCION_FLAG_EXEC } else { 0 },
            file_offset: off,
            file_size,
            mem_size,
            alignment: 16,
        });
    }
    let n = i - regiones;
    let (tipo, off, bytes) = anexo(prologo, n)?;
    Some(Seccion {
        // ** El indice con el que la FIRMA nombra un anexo: `0x80 | n`. Es el
        // mismo byte que escribe `bmo_abi::bef2`, y por eso `landing.rs` lo
        // encuentra sin cambiar una linea.
        indice: 0x80 | n,
        kind: kind_de_anexo(tipo),
        flags: 0,
        file_offset: off,
        file_size: bytes,
        mem_size: bytes,
        alignment: 8,
    })
}

/// Cuantas secciones ve el kernel: las regiones mas los anexos.
pub(crate) fn cuantas(prologo: &[u8]) -> usize {
    cuantas_regiones(prologo) + anexos(prologo)
}

/// **La puerta para una imagen BEF2.**
pub(crate) fn revisar(prologo: &[u8], tam_fichero: usize) -> Result<Revisada<'_>, Falta> {
    if prologo.len() < CABECERA2 {
        return Err(Falta::NoLlegaNiALaCabecera);
    }
    if *prologo.get(4).ok_or(Falta::NoLlegaNiALaCabecera)? != ABI {
        return Err(Falta::OtraVersionDelAbi);
    }
    let banderas = *prologo.get(5).ok_or(Falta::NoLlegaNiALaCabecera)?;
    if banderas & !BANDERAS != 0 {
        return Err(Falta::PideAlgoQueNadieImplementa);
    }
    let ejecutable = banderas & EJECUTABLE != 0;
    if banderas & OBJETO != 0 {
        // Un objeto no se carga: se enlaza. El mensaje manda a la herramienta.
        return Err(Falta::EsUnObjetoSinEnlazar);
    }
    if !ejecutable {
        return Err(Falta::NoEsEjecutable);
    }
    // Los reservados a cero: o es basura, o es de una version que este kernel
    // no entiende. En los dos casos no se carga.
    if u16_en(prologo, 6).ok_or(Falta::NoLlegaNiALaCabecera)? != 0
        || u64_en(prologo, 56).ok_or(Falta::NoLlegaNiALaCabecera)? != 0
    {
        return Err(Falta::CabeceraInvalida);
    }
    // ** El estado de CPU que el programa pide. Un bit que este kernel no
    // guarda en un cambio de contexto se corromperia en silencio a la primera
    // interrupcion; se rechaza con nombre, que es la mejora.
    let xcr0 = u64_en(prologo, 8).ok_or(Falta::NoLlegaNiALaCabecera)?;
    if xcr0 & !XCR0_PRESERVADO != 0 {
        return Err(Falta::ExtensionDeCpuQueNoSePreserva);
    }
    let entry_offset = u32_en(prologo, 16).ok_or(Falta::NoLlegaNiALaCabecera)? as u64;
    let cuantos_anexos = anexos(prologo);
    if cuantos_anexos > MAX_ANEXOS {
        return Err(Falta::DemasiadasSecciones);
    }
    let total = u32_en(prologo, 52).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    if total > tam_fichero {
        return Err(Falta::ImagenIncompleta);
    }
    let fin_tabla = CABECERA2 + cuantos_anexos * ANEXO;
    if fin_tabla > prologo.len() {
        return Err(Falta::TablaFueraDeLoLeido);
    }
    if fin_tabla > total {
        return Err(Falta::TablaFueraDelFichero);
    }

    let rev = Revisada {
        prologo,
        tabla: CABECERA2,
        cuantas: cuantas(prologo),
        entry_offset,
        formato: Formato::Bef2,
        fin_tabla,
    };

    // -- Una region VACIA no se presenta al kernel, pero su offset tiene que
    // caer dentro del fichero igual que en `bef2::lector`: la pasada hostil
    // encontro una de 0 bytes en el 65536 de un fichero de 704, y los dos
    // jueces tienen que contestar lo mismo.
    for o in [24usize, 32, 40] {
        let (off, len) = tramo(prologo, o).ok_or(Falta::NoLlegaNiALaCabecera)?;
        if off.saturating_add(len) > total as u64 {
            return Err(Falta::SeccionFueraDelFichero);
        }
    }

    // -- Cada trozo dentro del fichero, y ninguno pisando a otro -------------
    let mut hay_codigo = false;
    let mut tam_codigo = 0u64;
    let mut hay_firma = false;
    for s in rev.secciones() {
        if s.kind == DESCONOCIDO && s.file_size == 0 {
            return Err(Falta::SeccionInvalida);
        }
        if s.kind == BSS {
            continue;
        }
        if s.file_size == 0 {
            return Err(Falta::SeccionInvalida);
        }
        let fin = s
            .file_offset
            .checked_add(s.file_size)
            .ok_or(Falta::SeccionInvalida)?;
        if fin > total as u64 {
            return Err(Falta::SeccionFueraDelFichero);
        }
        // La cabecera y su tabla ocupan bytes como cualquier otra cosa.
        if s.file_offset < fin_tabla as u64 {
            return Err(Falta::SeccionesSeSolapan);
        }
        if s.kind == CODE {
            hay_codigo = true;
            tam_codigo = s.file_size;
        }
        if s.kind == SIGNATURE {
            hay_firma = true;
        }
    }
    let n = rev.cuantas();
    for i in 0..n {
        let Some(a) = rev.seccion(i) else { continue };
        if a.kind == BSS || a.file_size == 0 {
            continue;
        }
        let fa = a.file_offset.saturating_add(a.file_size);
        for j in i + 1..n {
            let Some(b) = rev.seccion(j) else { continue };
            if b.kind == BSS || b.file_size == 0 {
                continue;
            }
            let fb = b.file_offset.saturating_add(b.file_size);
            if a.file_offset < fb && b.file_offset < fa {
                return Err(Falta::SeccionesSeSolapan);
            }
        }
    }

    if !hay_codigo {
        return Err(Falta::SinCodigo);
    }
    if entry_offset >= tam_codigo {
        return Err(Falta::EntryFueraDelCodigo);
    }
    // ** En BEF2 la firma NO es opcional: sin ella no hay con que comprobar que
    // lo que aterrizo es lo que se escribio, y el kernel aplica relocs que
    // vienen del mismo fichero.
    if !hay_firma {
        return Err(Falta::CabeceraQueSeDesmiente);
    }
    Ok(rev)
}
