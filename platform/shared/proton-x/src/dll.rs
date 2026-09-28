//! **Lo que da una DLL** (P5a, 28-09): su tabla de exportaciones.
//!
//! Un juego no trae solo su `.exe`: trae sus DLL (el motor, la fisica, el
//! video, el sonido). Cargar una es lo mismo que un `.exe` (colocar,
//! relocalizar, resolver lo que pide) y una cosa mas: saber lo que DA.
//!
//! ```text
//!    IMAGE_EXPORT_DIRECTORY   Base (el primer ordinal), NumberOfFunctions,
//!                             NumberOfNames, y tres tablas: las RVA de las
//!                             funciones (por ordinal - Base), las RVA de los
//!                             nombres (en orden alfabetico) y el ordinal de
//!                             cada nombre
//!    un REENVIO               una RVA que cae DENTRO del directorio no es
//!                             codigo: es "OTRA.dll.Funcion" (o "OTRA.#7"),
//!                             y se resuelve alli
//! ```
//!
//! Aqui va lo que se lee sin ejecutar nada, de la imagen ya colocada (las RVA
//! son offsets).

use alloc::string::String;
use alloc::vec::Vec;

use crate::cargar::Funcion;
use crate::pe::{u16_en, u32_en, Pe};
use crate::Fallo;

/// A donde lleva una exportacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destino {
    /// Codigo (o datos) de esta DLL, en esta RVA.
    Rva(u32),
    /// Otra DLL: `dll` (con `.dll`) y lo que se pide alli.
    Reenvio { dll: String, funcion: Funcion },
}

/// Una exportacion: su ordinal, su nombre (si lo tiene) y a donde lleva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exportacion {
    pub ordinal: u32,
    pub nombre: Option<String>,
    pub destino: Destino,
}

fn cadena(img: &[u8], o: usize) -> Result<String, Fallo> {
    let resto = img.get(o..).ok_or(Fallo::Corto("un nombre exportado"))?;
    let n = resto.iter().position(|&b| b == 0).ok_or(Fallo::Corto("el final de un nombre exportado"))?;
    Ok(resto[..n].iter().map(|&b| b as char).collect())
}

/// Un reenvio "OTRA.Funcion" / "OTRA.#7".
fn reenvio(t: &str) -> Option<Destino> {
    let (dll, f) = t.rsplit_once('.')?;
    let funcion = match f.strip_prefix('#') {
        Some(n) => Funcion::Ordinal(n.parse().ok()?),
        None => Funcion::Nombre(String::from(f)),
    };
    let mut dll = String::from(dll);
    if !dll.contains('.') {
        dll.push_str(".dll");
    }
    Some(Destino::Reenvio { dll, funcion })
}

/// **Las exportaciones** de una imagen colocada, en orden de ordinal.
pub fn exportaciones(pe: &Pe, img: &[u8]) -> Result<Vec<Exportacion>, Fallo> {
    let d = pe.exportaciones;
    if d.rva == 0 {
        return Ok(Vec::new());
    }
    let o = d.rva as usize;
    let base = u32_en(img, o + 16, "la base de los ordinales")?;
    let n_funciones = u32_en(img, o + 20, "cuantas funciones exporta")? as usize;
    let n_nombres = u32_en(img, o + 24, "cuantos nombres exporta")? as usize;
    let funciones = u32_en(img, o + 28, "la tabla de funciones")? as usize;
    let nombres = u32_en(img, o + 32, "la tabla de nombres")? as usize;
    let ordinales = u32_en(img, o + 36, "la tabla de ordinales")? as usize;
    if n_funciones > 0x1_0000 || n_nombres > n_funciones.max(1) * 4 {
        return Err(Fallo::Corto("una tabla de exportaciones imposible"));
    }
    let mut nombre_de: Vec<Option<String>> = alloc::vec![None; n_funciones];
    for k in 0..n_nombres {
        let rva = u32_en(img, nombres + 4 * k, "un nombre exportado")? as usize;
        let i = u16_en(img, ordinales + 2 * k, "un ordinal exportado")? as usize;
        if let Some(x) = nombre_de.get_mut(i) {
            *x = Some(cadena(img, rva)?);
        }
    }
    let mut v = Vec::new();
    for (i, nombre) in nombre_de.into_iter().enumerate() {
        let rva = u32_en(img, funciones + 4 * i, "una funcion exportada")?;
        if rva == 0 {
            continue; // un hueco en los ordinales
        }
        let destino = if rva >= d.rva && rva < d.rva + d.tam {
            reenvio(&cadena(img, rva as usize)?).ok_or(Fallo::Corto("un reenvio que no es DLL.funcion"))?
        } else {
            Destino::Rva(rva)
        };
        v.push(Exportacion { ordinal: base + i as u32, nombre, destino });
    }
    Ok(v)
}

/// **Buscar** lo que se pide entre las exportaciones: por nombre (exacto,
/// como Windows) o por ordinal.
pub fn buscar<'a>(exps: &'a [Exportacion], f: &Funcion) -> Option<&'a Destino> {
    exps.iter()
        .find(|e| match f {
            Funcion::Nombre(n) => e.nombre.as_deref() == Some(n.as_str()),
            Funcion::Ordinal(o) => e.ordinal == *o as u32,
        })
        .map(|e| &e.destino)
}

/// El nombre de fichero de una DLL pedida por nombre: con `.dll` si no lleva
/// extension, sin la ruta, en minusculas (el FAT32 de BMO-X guarda 8.3 y no
/// distingue).
pub fn fichero(dll: &str) -> String {
    let base = dll.rsplit(['\\', '/']).next().unwrap_or(dll);
    let mut f = String::from(base);
    if !f.contains('.') {
        f.push_str(".dll");
    }
    f.to_ascii_lowercase()
}
