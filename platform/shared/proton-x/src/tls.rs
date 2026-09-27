//! **El TLS de un `.exe`: `__declspec(thread)` y sus callbacks** (P4, 27-09).
//!
//! Una variable `__declspec(thread)` (o `thread_local` de C++) vive en un
//! bloque POR HILO. El compilador de Microsoft la lee asi:
//!
//! ```text
//!    mov eax, [_tls_index]          el indice de ESTE modulo
//!    mov rcx, gs:[0x58]             ThreadLocalStoragePointer del TEB
//!    mov rcx, [rcx + rax*8]         el bloque de este modulo en este hilo
//!    mov eax, [rcx + desplazamiento]
//! ```
//!
//! El directorio 9 del PE (`IMAGE_TLS_DIRECTORY64`, 40 bytes) dice que copiar
//! en cada bloque nuevo y a quien avisar:
//!
//! ```text
//!    +0   StartAddressOfRawData   la plantilla (VA)
//!    +8   EndAddressOfRawData
//!    +16  AddressOfIndex          donde el cargador deja el indice (VA)
//!    +24  AddressOfCallBacks      lista de funciones, acabada en 0 (VA)
//!    +32  SizeOfZeroFill          ceros detras de la plantilla
//! ```
//!
//! Las VA van YA relocalizadas en la imagen colocada: por eso esto lee la
//! imagen de `colocar`, no el fichero. Un `.exe` es el unico modulo: su
//! indice es 0. Los callbacks se llaman con (base, motivo, 0): 1 al empezar
//! el proceso, 2 al nacer un hilo y 3 al acabar.

use alloc::vec::Vec;

use crate::pe::{u32_en, u64_en, Pe};
use crate::Fallo;

pub const DLL_PROCESS_ATTACH: u32 = 1;
pub const DLL_THREAD_ATTACH: u32 = 2;
pub const DLL_THREAD_DETACH: u32 = 3;

/// El offset de `ThreadLocalStoragePointer` en el TEB de x64.
pub const TEB_TLS_POINTER: usize = 0x58;

/// **Lo que el TLS de un `.exe` pide**, ya leido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tls {
    /// Los bytes con los que empieza el bloque de cada hilo.
    pub plantilla: Vec<u8>,
    /// Cuantos ceros van detras.
    pub ceros: u32,
    /// La RVA donde se escribe el indice (un `u32`).
    pub indice_rva: u32,
    /// Las VA de los callbacks, en orden.
    pub callbacks: Vec<u64>,
}

impl Tls {
    /// Lo que mide el bloque de un hilo.
    pub fn bytes(&self) -> usize {
        self.plantilla.len() + self.ceros as usize
    }

    /// El bloque de un hilo nuevo: la plantilla y los ceros.
    pub fn bloque(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(self.bytes());
        b.extend_from_slice(&self.plantilla);
        b.resize(self.bytes(), 0);
        b
    }
}

/// **Leer el TLS** de la imagen colocada en `base`. `None` si no trae.
pub fn leer(pe: &Pe, img: &[u8], base: u64) -> Result<Option<Tls>, Fallo> {
    if pe.tls.rva == 0 {
        return Ok(None);
    }
    let d = pe.tls.rva as usize;
    if pe.tls.tam < 40 {
        return Err(Fallo::Tls("el directorio mide menos de 40 bytes"));
    }
    let rva = |va: u64| -> Result<usize, Fallo> {
        va.checked_sub(base).filter(|&r| r <= img.len() as u64).map(|r| r as usize).ok_or(Fallo::Tls("una direccion fuera de la imagen"))
    };
    let (ini, fin) = (u64_en(img, d, "el TLS")?, u64_en(img, d + 8, "el TLS")?);
    let indice = u64_en(img, d + 16, "el TLS")?;
    let lista = u64_en(img, d + 24, "el TLS")?;
    let ceros = u32_en(img, d + 32, "el TLS")?;
    let plantilla = if ini == 0 && fin == 0 {
        Vec::new()
    } else {
        let (a, b) = (rva(ini)?, rva(fin)?);
        if b < a {
            return Err(Fallo::Tls("la plantilla acaba antes de empezar"));
        }
        img[a..b].to_vec()
    };
    let indice_rva = rva(indice)?;
    if indice_rva + 4 > img.len() {
        return Err(Fallo::Tls("el indice cae fuera de la imagen"));
    }
    let mut callbacks = Vec::new();
    if lista != 0 {
        let mut o = rva(lista)?;
        loop {
            let f = u64_en(img, o, "la lista de callbacks del TLS")?;
            if f == 0 {
                break;
            }
            rva(f)?;
            callbacks.push(f);
            o += 8;
            if callbacks.len() > 256 {
                return Err(Fallo::Tls("mas de 256 callbacks: la lista no acaba"));
            }
        }
    }
    Ok(Some(Tls { plantilla, ceros, indice_rva: indice_rva as u32, callbacks }))
}
