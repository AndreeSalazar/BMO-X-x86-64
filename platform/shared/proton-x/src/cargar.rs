//! **Colocar, relocalizar y resolver**: del fichero a la imagen que se ejecuta.
//!
//! Lo que el cargador de Windows hace con un `.exe`, sin la memoria: aqui la
//! imagen es un `Vec` y quien carga de verdad la copia a sus bloques.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::pe::{u16_en, u32_en, u64_en, Pe, Permiso};
use crate::Fallo;

/// Relocalizacion que no hace nada (relleno del bloque).
const RELOC_NADA: u16 = 0;
/// La unica que trae un PE32+ x86-64: sumar la distancia a un `u64`.
const RELOC_DIR64: u16 = 10;

/// La pagina de BMO-X: un bloque se pide, se sella y se suelta en paginas.
pub const PAGINA: u32 = 4096;

/// **Como se parte la imagen en DOS bloques seguidos**: `codigo` bytes desde
/// la RVA 0 (cabeceras y secciones de codigo: se SELLA, R+X) y `datos` bytes
/// detras (lo demas: R+W, sin X).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partes {
    pub codigo: u32,
    pub datos: u32,
}

/// **Partir la imagen para BMO-X** (P1c, 27-09). El kernel sella BLOQUES
/// enteros (`MEM_OP_SELLAR`) y pone los que se piden seguidos uno detras de
/// otro; asi que la imagen va en dos: lo que se ejecuta delante, lo demas
/// detras. Una seccion que NO es codigo y cae en las paginas del codigo no
/// se puede separar de el: se dice, y no se carga ejecutable a medias.
///
/// Las cabeceras van en el bloque del codigo: quedan R+X, sin W. Como en
/// Windows, nadie las escribe; a diferencia de Windows, se podrian ejecutar
/// (no hay nada que ejecutar en ellas). `.rdata` va con los datos: R+W, no
/// solo R. Las dos cosas son de tener dos bloques y no uno por permiso.
pub fn partir(pe: &Pe) -> Result<Partes, Fallo> {
    let arriba = |x: u32| x.div_ceil(PAGINA) * PAGINA;
    let mut corte = arriba(pe.tam_cabeceras.max(1));
    for s in pe.secciones.iter().filter(|s| s.permiso() == Permiso::Codigo) {
        corte = corte.max(arriba(s.rva + s.tam_en_imagen()));
    }
    if let Some(s) = pe.secciones.iter().find(|s| s.permiso() != Permiso::Codigo && s.rva < corte) {
        return Err(Fallo::NoSeParte(s.nombre.clone()));
    }
    let total = arriba(pe.tam_imagen);
    if corte > total {
        return Err(Fallo::Corto("la imagen: el codigo pasa de su medida"));
    }
    Ok(Partes { codigo: corte, datos: total - corte })
}

/// **La imagen, en su base**: cabeceras y secciones en su RVA, el resto a
/// cero, y las relocalizaciones aplicadas si `base` no es la del enlazador.
pub fn colocar(pe: &Pe, d: &[u8], base: u64) -> Result<Vec<u8>, Fallo> {
    if pe.tls.rva != 0 {
        return Err(Fallo::PideTls);
    }
    let mut img = vec![0u8; pe.tam_imagen as usize];
    let h = (pe.tam_cabeceras as usize).min(d.len()).min(img.len());
    img[..h].copy_from_slice(&d[..h]);
    for s in &pe.secciones {
        let n = s.tam_en_fichero.min(s.tam_en_imagen()) as usize;
        let (desde, rva) = (s.desde as usize, s.rva as usize);
        img[rva..rva + n].copy_from_slice(&d[desde..desde + n]);
    }
    relocalizar(pe, &mut img, base)?;
    Ok(img)
}

fn relocalizar(pe: &Pe, img: &mut [u8], base: u64) -> Result<(), Fallo> {
    let distancia = base.wrapping_sub(pe.base);
    if distancia == 0 {
        return Ok(());
    }
    if pe.relocalizaciones.rva == 0 {
        // Sin tabla: no hay nada que corregir, y se mueve igual -- salvo que
        // el enlazador diga que las QUITO (teb.exe no trae `.reloc` y no le
        // hace falta: todo lo suyo es relativo a RIP).
        return if pe.relocs_quitadas { Err(Fallo::SinRelocalizaciones) } else { Ok(()) };
    }
    let mut p = pe.relocalizaciones.rva as usize;
    let fin = p + pe.relocalizaciones.tam as usize;
    while p + 8 <= fin {
        let pagina = u32_en(img, p, "un bloque de relocalizaciones")?;
        let tam = u32_en(img, p + 4, "un bloque de relocalizaciones")? as usize;
        if tam < 8 || p + tam > fin {
            return Err(Fallo::Corto("un bloque de relocalizaciones entero"));
        }
        for k in (p + 8..p + tam).step_by(2) {
            let e = u16_en(img, k, "una relocalizacion")?;
            let (tipo, rva) = (e >> 12, pagina + (e & 0xFFF) as u32);
            match tipo {
                RELOC_NADA => {}
                RELOC_DIR64 => {
                    let o = rva as usize;
                    let v = u64_en(img, o, "el valor a relocalizar")?.wrapping_add(distancia);
                    img[o..o + 8].copy_from_slice(&v.to_le_bytes());
                }
                _ => return Err(Fallo::Relocalizacion { rva, tipo }),
            }
        }
        p += tam;
    }
    Ok(())
}

/// Una funcion importada: por nombre o por numero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Funcion {
    Nombre(String),
    Ordinal(u16),
}

impl fmt::Display for Funcion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Funcion::Nombre(n) => write!(f, "{n}"),
            Funcion::Ordinal(o) => write!(f, "#{o}"),
        }
    }
}

/// Lo que el `.exe` pide, y la RANURA de la IAT donde va su direccion: el
/// codigo llama con `call [rip+x]` a esa ranura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Importacion {
    pub dll: String,
    pub funcion: Funcion,
    pub ranura: u32,
}

fn cadena(img: &[u8], o: usize) -> Result<String, Fallo> {
    let resto = img.get(o..).ok_or(Fallo::Corto("un nombre importado"))?;
    let n = resto.iter().position(|&b| b == 0).ok_or(Fallo::Corto("el final de un nombre importado"))?;
    Ok(resto[..n].iter().map(|&b| b as char).collect())
}

/// **Lo que pide**, leido de la imagen ya colocada (las RVA son offsets).
pub fn importaciones(pe: &Pe, img: &[u8]) -> Result<Vec<Importacion>, Fallo> {
    let mut v = Vec::new();
    if pe.importaciones.rva == 0 {
        return Ok(v);
    }
    let mut p = pe.importaciones.rva as usize;
    loop {
        let nombres = u32_en(img, p, "un descriptor de importacion")?;
        let nombre_dll = u32_en(img, p + 12, "un descriptor de importacion")?;
        let iat = u32_en(img, p + 16, "un descriptor de importacion")?;
        if nombres == 0 && nombre_dll == 0 && iat == 0 {
            break;
        }
        let dll = cadena(img, nombre_dll as usize)?;
        let mut lista = if nombres != 0 { nombres } else { iat } as usize;
        let mut ranura = iat;
        loop {
            let e = u64_en(img, lista, "una entrada importada")?;
            if e == 0 {
                break;
            }
            let funcion = if e >> 63 != 0 {
                Funcion::Ordinal(e as u16)
            } else {
                // +2: el `hint` va delante del nombre.
                Funcion::Nombre(cadena(img, (e & 0x7FFF_FFFF) as usize + 2)?)
            };
            v.push(Importacion { dll: dll.clone(), funcion, ranura });
            lista += 8;
            ranura += 8;
        }
        p += 20;
    }
    Ok(v)
}

/// **Resolver contra la tabla de la casa.** `tabla(dll, funcion)` da la
/// direccion de la funcion de la casa, o `None`. Si falta UNA, no se escribe
/// nada y vuelven TODAS las que faltan: el `.exe` no arranca a medias.
pub fn resolver(img: &mut [u8], imps: &[Importacion], tabla: impl Fn(&str, &Funcion) -> Option<u64>) -> Result<(), Fallo> {
    let mut faltan = Vec::new();
    let mut hechas = Vec::with_capacity(imps.len());
    for i in imps {
        match tabla(&i.dll, &i.funcion) {
            Some(dir) => hechas.push((i.ranura as usize, dir)),
            None => faltan.push(i.clone()),
        }
    }
    if !faltan.is_empty() {
        return Err(Fallo::Faltan(faltan));
    }
    for (o, dir) in hechas {
        let r = img.get_mut(o..o + 8).ok_or(Fallo::Corto("una ranura de la IAT"))?;
        r.copy_from_slice(&dir.to_le_bytes());
    }
    Ok(())
}
