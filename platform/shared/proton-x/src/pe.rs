//! **Leer un PE32+**: el veredicto de PROTON-X y la forma del `.exe`.
//!
//! El veredicto es el MISMO que da `toolchain/tools/rayosx`, con sus textos
//! (salvo la maquina desconocida, que alli lleva su numero): lo que la
//! herramienta dice en Windows es lo que este cargador hace en BMO-X.
//!
//! Los desplazamientos son los del formato PE/COFF de Microsoft para PE32+:
//!
//! ```text
//!    0x3C              donde empieza la firma `PE\0\0`
//!    firma + 4         cabecera COFF: maquina, secciones, ... medida de la opcional
//!    firma + 24        cabecera opcional: magia 0x20B, entrada (+16), base
//!                      (+24, u64), medida de la imagen (+56), de las
//!                      cabeceras (+60), directorios (+108 cuantos, +112 ellos)
//!    tras la opcional  las secciones, 40 bytes cada una
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use crate::Fallo;

pub const MAQUINA_AMD64: u16 = 0x8664;
pub const MAGIA_PE32_MAS: u16 = 0x20B;

/// Los directorios que mira el cargador (su indice en la tabla).
const DIR_IMPORTACIONES: usize = 1;
const DIR_EXCEPCIONES: usize = 3;
const DIR_RELOCALIZACIONES: usize = 5;
const DIR_TLS: usize = 9;
const DIR_CLR: usize = 14;

const SE_EJECUTA: u32 = 0x2000_0000;
/// COFF `IMAGE_FILE_RELOCS_STRIPPED`: el enlazador le quito las
/// relocalizaciones y el `.exe` SOLO corre en su base.
const RELOCS_QUITADAS: u16 = 0x0001;
const SE_ESCRIBE: u32 = 0x8000_0000;

/// Una seccion del `.exe`: donde va en la imagen y de donde sale del fichero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seccion {
    pub nombre: String,
    pub rva: u32,
    pub tam_virtual: u32,
    pub desde: u32,
    pub tam_en_fichero: u32,
    pub caracteristicas: u32,
}

/// Como queda una seccion en memoria. Nunca las dos cosas a la vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permiso {
    /// R+X, sin W: se SELLA (`MEM_OP_SELLAR`).
    Codigo,
    /// R+W, sin X.
    Datos,
    /// Solo R.
    Lectura,
}

impl Seccion {
    pub fn permiso(&self) -> Permiso {
        if self.caracteristicas & SE_EJECUTA != 0 {
            Permiso::Codigo
        } else if self.caracteristicas & SE_ESCRIBE != 0 {
            Permiso::Datos
        } else {
            Permiso::Lectura
        }
    }

    /// Lo que ocupa en la imagen (una seccion sin medida virtual usa la del
    /// fichero, como hace el cargador de Windows).
    pub fn tam_en_imagen(&self) -> u32 {
        if self.tam_virtual == 0 {
            self.tam_en_fichero
        } else {
            self.tam_virtual
        }
    }
}

/// Un directorio: RVA y medida; `rva == 0` es que no esta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Directorio {
    pub rva: u32,
    pub tam: u32,
}

/// La forma de un `.exe` que PROTON-X acepta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pe {
    /// La base que eligio el enlazador.
    pub base: u64,
    /// La entrada, como RVA.
    pub entrada: u32,
    pub tam_imagen: u32,
    pub tam_cabeceras: u32,
    pub secciones: Vec<Seccion>,
    /// La tabla de EXPORTACIONES (directorio 0): lo que una DLL da (P5a).
    pub exportaciones: Directorio,
    pub importaciones: Directorio,
    pub relocalizaciones: Directorio,
    pub tls: Directorio,
    /// `.pdata`: una RUNTIME_FUNCTION por funcion, para desenrollar (P4c).
    pub excepciones: Directorio,
    /// La cabecera COFF dice `RELOCS_STRIPPED`: no se puede mover de su base.
    /// Sin esa bandera y sin `.reloc`, se mueve sin corregir nada (todo es
    /// relativo a RIP), que es lo que hace el cargador de Windows.
    pub relocs_quitadas: bool,
    /// La cabecera COFF dice `IMAGE_FILE_DLL`: es una DLL, y su entrada es
    /// `DllMain` (P5a).
    pub es_dll: bool,
}

pub(crate) fn u16_en(d: &[u8], o: usize, que: &'static str) -> Result<u16, Fallo> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Fallo::Corto(que))
}

pub(crate) fn u32_en(d: &[u8], o: usize, que: &'static str) -> Result<u32, Fallo> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(Fallo::Corto(que))
}

pub(crate) fn u64_en(d: &[u8], o: usize, que: &'static str) -> Result<u64, Fallo> {
    d.get(o..o + 8).map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])).ok_or(Fallo::Corto(que))
}

/// El motivo por el que una maquina queda FUERA. Los textos son los de
/// `rayosx` (`MAQUINAS_FUERA`).
fn maquina_fuera(m: u16) -> Option<&'static str> {
    Some(match m {
        0x14C => "x86 de 32 bits (pediria WOW64: un segundo mundo entero)",
        0xAA64 => "ARM64 (otras instrucciones: pediria traducirlas, como FEX o Rosetta)",
        0xA641 => "ARM64EC (ARM con llamadas de x64: sigue siendo ARM)",
        0xA64E => "ARM64X (dos binarios en uno, ARM64 y ARM64EC)",
        0x1C4 => "ARM de 32 bits",
        _ => return None,
    })
}

/// **Leer un `.exe`**: el veredicto de PROTON-X primero, la forma despues.
pub fn leer(d: &[u8]) -> Result<Pe, Fallo> {
    leer_con_medida(d, d.len() as u64)
}

/// **Solo las CABECERAS** (el censo, 29-09): `d` es el principio del fichero
/// y `medida` lo que mide entero. Un `.exe` de 60 MB se juzga sin traerlo:
/// las secciones se comprueban contra `medida`, no contra lo leido.
pub fn leer_cabeceras(d: &[u8], medida: u64) -> Result<Pe, Fallo> {
    leer_con_medida(d, medida)
}

fn leer_con_medida(d: &[u8], medida: u64) -> Result<Pe, Fallo> {
    if d.get(..2) != Some(b"MZ") {
        return Err(Fallo::NoEsPe);
    }
    let e = u32_en(d, 0x3C, "la firma PE")? as usize;
    if d.get(e..e + 4) != Some(b"PE\0\0") {
        return Err(Fallo::NoEsPe);
    }
    let maquina = u16_en(d, e + 4, "la maquina")?;
    let nsec = u16_en(d, e + 6, "el numero de secciones")? as usize;
    let tam_opcional = u16_en(d, e + 20, "la medida de la cabecera opcional")? as usize;
    let opc = e + 24;
    let magia = u16_en(d, opc, "la magia")?;
    if let Some(m) = maquina_fuera(maquina) {
        return Err(Fallo::Fuera(m));
    }
    if maquina != MAQUINA_AMD64 {
        return Err(Fallo::Fuera("maquina desconocida: no es x86-64"));
    }
    if magia != MAGIA_PE32_MAS {
        return Err(Fallo::Fuera("cabecera PE32 (de 32 bits) con maquina AMD64: no es un PE32+"));
    }
    let ndirs = u32_en(d, opc + 108, "el numero de directorios")? as usize;
    let dir = |i: usize| -> Result<Directorio, Fallo> {
        if i >= ndirs {
            return Ok(Directorio::default());
        }
        Ok(Directorio { rva: u32_en(d, opc + 112 + 8 * i, "un directorio")?, tam: u32_en(d, opc + 116 + 8 * i, "un directorio")? })
    };
    if dir(DIR_CLR)?.rva != 0 {
        return Err(Fallo::Fuera(".NET: lleva IL, no instrucciones x86-64 (pediria una CLR)"));
    }
    let tam_imagen = u32_en(d, opc + 56, "la medida de la imagen")?;
    let tam_cabeceras = u32_en(d, opc + 60, "la medida de las cabeceras")?;
    let mut secciones = Vec::with_capacity(nsec);
    for k in 0..nsec {
        let s = opc + tam_opcional + 40 * k;
        let crudo = d.get(s..s + 8).ok_or(Fallo::Corto("el nombre de una seccion"))?;
        let nombre: String = crudo.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
        let sec = Seccion {
            nombre,
            tam_virtual: u32_en(d, s + 8, "una seccion")?,
            rva: u32_en(d, s + 12, "una seccion")?,
            tam_en_fichero: u32_en(d, s + 16, "una seccion")?,
            desde: u32_en(d, s + 20, "una seccion")?,
            caracteristicas: u32_en(d, s + 36, "una seccion")?,
        };
        if sec.caracteristicas & SE_EJECUTA != 0 && sec.caracteristicas & SE_ESCRIBE != 0 {
            return Err(Fallo::EscribeYEjecuta(sec.nombre));
        }
        let fin = sec.rva as u64 + sec.tam_en_imagen() as u64;
        if fin > tam_imagen as u64 {
            return Err(Fallo::Seccion { nombre: sec.nombre, motivo: "no cabe en la imagen" });
        }
        let fin_fichero = sec.desde as u64 + sec.tam_en_fichero.min(sec.tam_en_imagen()) as u64;
        if fin_fichero > medida {
            return Err(Fallo::Seccion { nombre: sec.nombre, motivo: "sus datos pasan del final del fichero" });
        }
        secciones.push(sec);
    }
    Ok(Pe {
        base: u64_en(d, opc + 24, "la base")?,
        entrada: u32_en(d, opc + 16, "la entrada")?,
        tam_imagen,
        tam_cabeceras,
        secciones,
        exportaciones: dir(0)?,
        importaciones: dir(DIR_IMPORTACIONES)?,
        relocalizaciones: dir(DIR_RELOCALIZACIONES)?,
        tls: dir(DIR_TLS)?,
        excepciones: dir(DIR_EXCEPCIONES)?,
        relocs_quitadas: u16_en(d, e + 22, "las caracteristicas COFF")? & RELOCS_QUITADAS != 0,
        es_dll: u16_en(d, e + 22, "las caracteristicas COFF")? & 0x2000 != 0,
    })
}
