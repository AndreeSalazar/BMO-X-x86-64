//! **La DECLARACION DE IMAGEN, desde Ring 3** (P0.4b.3, 30-09): lo que usa
//! PROTON-X para cargar un `.exe` de Windows y sus DLL cuando no caben en los
//! ocho bloques de [`crate::Memoria`].
//!
//! ```text
//!    Imagen::declarar(&partes)   la tabla va en un bloque propio (por la
//!                                puerta no viajan punteros), se declara, y el
//!                                bloque se suelta: el kernel ya la leyo
//!    imagen.parte(i)             donde quedo la parte i (escribible, sin X)
//!    imagen.sellar(i)            una parte de CODIGO a R+X sin W (W^X)
//! ```
//!
//! Se juzga UNA vez, contra la RAM libre de ese momento: si no hay, el NO
//! dice cuanta pide y cuanta hay ([`NoImagen`]). Una declaracion por proceso;
//! vive lo que vive el proceso (no hay soltar: al morir, el kernel la
//! devuelve a cero).

use crate::*;

/// Una parte que se declara: de que PE (0, 1, 2... en orden), si es su
/// codigo, y cuantos bytes mide en memoria. De cada PE, el codigo primero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParteImagen {
    pub pe: u16,
    pub codigo: bool,
    pub bytes: u64,
}

/// Por que no: el motivo (`IMAGEN_*` de `bmo-abi`) y su valor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoImagen {
    pub motivo: u32,
    pub valor: u64,
}

impl NoImagen {
    /// La frase del NO (sin alloc: los numeros van aparte, en
    /// [`NoImagen::pide_mib`], [`NoImagen::hay_mib`] y `valor`).
    pub fn frase(&self) -> &'static str {
        match self.motivo {
            1 => "este proceso ya declaro su imagen",
            2 => "la tabla de partes no esta en un bloque propio",
            3 => "de mas partes, o ninguna (valor: cuantas)",
            4 => "una parte mide 0 bytes (valor: cual)",
            5 => "una parte fuera de orden: de cada PE, codigo y luego datos (valor: cual)",
            6 => "no hay RAM: pide / hay, en MiB (lo libre menos el margen del kernel)",
            7 => "no cabe en la ventana de imagenes: pide / hay, en MiB",
            8 => "ya hay imagenes declaradas en todas las ranuras del kernel",
            9 => "el kernel se quedo sin marcos a mitad (deshecho)",
            10 => "un mapeo fallo a mitad (deshecho)",
            11 => "esa parte no existe",
            12 => "esa parte es de DATOS: los datos no se ejecutan",
            13 => "esa parte ya estaba sellada",
            14 => "sin NX en la CPU: sellar no garantiza nada",
            15 => "sellar: el remapeo fallo; la parte queda desmapeada",
            _ => "un motivo que este userland no conoce",
        }
    }

    /// Con `IMAGEN_SIN_RAM` y `IMAGEN_SIN_VENTANA`: lo que pide, en MiB.
    pub fn pide_mib(&self) -> u64 {
        self.valor >> 32
    }

    /// Y lo que hay, en MiB.
    pub fn hay_mib(&self) -> u64 {
        self.valor & 0xFFFF_FFFF
    }
}

/// **Una imagen declarada y concedida.**
pub struct Imagen {
    n: usize,
}

/// Lo que mide una parte en la tabla (`bmo_abi::...::IMAGEN_PARTE_BYTES`).
const PARTE_BYTES: usize = 16;

fn no(st: Status) -> NoImagen {
    NoImagen { motivo: if st.flags == 0 { st.code } else { st.flags }, valor: st.value }
}

impl Imagen {
    /// **Declarar** todas las partes de una vez.
    pub fn declarar(partes: &[ParteImagen]) -> Result<Self, NoImagen> {
        let bytes = (partes.len().max(1) * PARTE_BYTES) as u64;
        let Some(tabla) = Memoria::request(bytes) else {
            return Err(NoImagen { motivo: 2, valor: 0 });
        };
        for (i, p) in partes.iter().enumerate() {
            let mut e = [0u8; PARTE_BYTES];
            e[..8].copy_from_slice(&p.bytes.to_le_bytes());
            e[8..10].copy_from_slice(&p.pe.to_le_bytes());
            e[10] = p.codigo as u8;
            // SAFETY: `partes.len() * 16` bytes de un bloque nuestro.
            unsafe { core::ptr::copy_nonoverlapping(e.as_ptr(), tabla.base().add(i * PARTE_BYTES), PARTE_BYTES) };
        }
        let st = invoke(CURRENT_TASK, OP_IMAGEN_DECLARAR, tabla.base() as u64, partes.len() as u64, 0);
        // `tabla` se suelta al salir: el kernel ya la leyo dentro del syscall.
        if st.ok() {
            Ok(Self { n: partes.len() })
        } else {
            Err(no(st))
        }
    }

    /// Cuantas partes tiene.
    pub fn partes(&self) -> usize {
        self.n
    }

    /// **Donde quedo la parte `i`**: escribible y sin ejecucion hasta que se
    /// selle.
    pub fn parte(&self, i: usize) -> Option<*mut u8> {
        invoke(CURRENT_TASK, OP_IMAGEN_PARTE, i as u64, 0, 0).valor().map(|v| v as *mut u8)
    }

    /// **Sellar la parte de codigo `i`**: R+X sin W, irreversible. Desde aqui
    /// no se escribe -- ni desde este proceso ni por el kernel en su nombre.
    pub fn sellar(&self, i: usize) -> Result<(), NoImagen> {
        let st = invoke(CURRENT_TASK, OP_IMAGEN_SELLAR, i as u64, 0, 0);
        if st.ok() {
            Ok(())
        } else {
            Err(no(st))
        }
    }
}
