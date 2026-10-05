//! **Las constantes de un dibujo** (03-10, N5.2): cada cbuffer que leen los
//! sombreadores, de donde diga la root signature, en su sitio del bloque.
//!
//! [carril]  VERDE     lee memoria de la casa (buferes y montones de
//!                     descriptores); no toca la maquina
//! [cuesta]  DATO      un cbuffer de otro sitio da otras matrices: la
//!                     geometria sale en cualquier parte
//! [riesgo]  ESPEJO    donde esta cada cbuffer lo decide `bmo_proton_x::donde`
//!                     con las reglas de D3D12; el banco lo prueba con firmas
//!                     hechas a mano
//! [consumo] NADA      solo al dibujar: con un cbuffer entero se presta; con
//!                     varios, una copia de las filas que se leen
//!
//! Hasta el 03-10 la casa solo sabia del b0 del espacio 0, y solo como CBV
//! en la raiz: un sombreador con b1, o con constantes de 32 bits en la raiz
//! (`SetGraphicsRoot32BitConstants`, que se decian y se tiraban), o con su
//! CBV en una tabla, no se dibujaba.
//!
//! ```text
//!    el enlace      dice que cbuffers leen los dos y cuantas filas de cada
//!                   uno (`Enlace::constantes`), ya aplanados
//!    cada uno       un CBV en la raiz (su direccion), constantes de 32 bits
//!                   (`Estado::raiz32`) o un CBV en una tabla (su ranura)
//!    el bloque      uno detras de otro; lo que no trae, 0
//! ```

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::donde::{self, Cb};
use bmo_proton_x::lote::{self, Enlace};
use bmo_proton_x::raiz::{Carga, Firma};

use crate::tuberia::{resolver_hasta, Estado};

/// Las palabras de 32 bits que guarda la raiz de una lista: el maximo de una
/// root signature (64 DWORD entre todo lo que lleva).
pub const PALABRAS: usize = 64;

/// **Las constantes de 32 bits de la raiz de una lista**, las de todos sus
/// parametros una tras otra (`donde::constantes_desde` dice donde empieza
/// cada uno).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palabras(pub [u32; PALABRAS]);

impl Default for Palabras {
    fn default() -> Self {
        Palabras([0; PALABRAS])
    }
}

/// **El bloque de constantes de un dibujo**, o por que no hay.
pub(crate) fn del_dibujo(firma: &Firma, e: &Estado, en: &Enlace) -> Result<Cow<'static, [u8]>, String> {
    de_ranuras(firma, e, &en.ranuras.cbuffers, &en.constantes)
}

/// [`del_dibujo`] con los cbuffers y sus bloques sueltos: lo que tiene
/// tambien un sombreador de computo (N5.5), que no tiene enlace.
pub(crate) fn de_ranuras(firma: &Firma, e: &Estado, cbuffers: &[bmo_proton_x::dxil::programa::Lugar], constantes: &[lote::Bloque]) -> Result<Cow<'static, [u8]>, String> {
    let mut trozos: Vec<Cow<'static, [u8]>> = Vec::with_capacity(constantes.len());
    for (l, b) in cbuffers.iter().zip(constantes) {
        let quiere = b.filas as usize * 16;
        let nombre = || format!("el cbuffer b{} del espacio {}", l.registro, l.espacio);
        let trozo = match donde::cbuffer(firma, *l) {
            Some(Cb::Raiz(k)) => resolver_hasta(e.cbv.get(k).copied().unwrap_or(0), quiere).map(Cow::Borrowed),
            Some(Cb::Constantes { desde, cuantas }) => {
                let p = e.raiz32.0.get(desde..desde + cuantas as usize).ok_or_else(|| format!("{}: mas constantes de las que guarda la casa ({PALABRAS})", nombre()))?;
                Some(Cow::Owned(p.iter().flat_map(|x| x.to_le_bytes()).collect()))
            }
            Some(Cb::Tabla(k, i)) => cbv_de_tabla(e.tablas.get(k).copied().unwrap_or(0), i).and_then(|(va, n)| resolver_hasta(va, quiere.min(n))).map(Cow::Borrowed),
            None => return Err(format!("los sombreadores leen {} y la root signature no lo tiene", nombre())),
        };
        trozos.push(trozo.ok_or_else(|| format!("{} no es un bufer de la casa (o no se le dio direccion)", nombre()))?);
    }
    // Uno solo, entero: tal cual, sin copiar (lo de siempre con el b0).
    if let ([t], [b]) = (trozos.as_slice(), constantes) {
        if b.fila == 0 && t.len() >= b.filas as usize * 16 {
            return Ok(t.clone());
        }
    }
    Ok(Cow::Owned(lote::juntar_constantes(constantes, |i| trozos.get(i).map(|t| &t[..]))))
}

/// La direccion y la medida del CBV de la ranura `i` de la tabla que empieza
/// en `base` (`CreateConstantBufferView` deja: direccion, marca, medida).
fn cbv_de_tabla(base: u64, i: u64) -> Option<(u64, usize)> {
    if base == 0 {
        return None;
    }
    // SAFETY: la ranura `i` de un monton de la casa (la tabla la puso el
    // `.exe` con un identificador de la casa; la firma dice que es de ella).
    let r = unsafe { core::slice::from_raw_parts((base + i * crate::d3d12::DESCRIPTOR_BYTES) as *const u64, 4) };
    (r[1] == crate::d3d12::DESC_CBV && r[0] != 0).then_some((r[0], r[2] as usize))
}

/// **`SetGraphicsRoot32BitConstant(s)`**: `valores` desde la palabra `desde`
/// del parametro `parametro`, a donde la casa guarda las de la raiz. El texto
/// dice por que no, si no.
pub(crate) fn poner(e: &mut Estado, firma: &Firma, parametro: usize, desde: usize, valores: &[u32]) -> Result<(), &'static str> {
    let Some(Carga::Constantes { cuantas, .. }) = firma.parametros.get(parametro).map(|p| &p.carga) else {
        return Err("SetGraphicsRoot32BitConstant(s) a un parametro que no es de constantes: en Windows es un error, y se tira");
    };
    if desde + valores.len() > *cuantas as usize {
        return Err("SetGraphicsRoot32BitConstant(s) con mas constantes de las que tiene el parametro: se tira");
    }
    let base = donde::constantes_desde(firma, parametro) + desde;
    let sitio = e.raiz32.0.get_mut(base..base + valores.len()).ok_or("SetGraphicsRoot32BitConstant(s): mas de 64 palabras en la raiz: se tira")?;
    sitio.copy_from_slice(valores);
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec;
    use bmo_proton_x::raiz::{Parametro, CBV, CONSTANTES};

    fn firma() -> Firma {
        Firma {
            parametros: vec![
                Parametro { tipo: CBV, visibilidad: 0, carga: Carga::Descriptor { registro: 0, espacio: 0 } },
                Parametro { tipo: CONSTANTES, visibilidad: 0, carga: Carga::Constantes { registro: 1, espacio: 0, cuantas: 3 } },
                Parametro { tipo: CONSTANTES, visibilidad: 0, carga: Carga::Constantes { registro: 2, espacio: 0, cuantas: 4 } },
            ],
            samplers: vec![],
            banderas: 0,
        }
    }

    #[test]
    fn las_constantes_de_la_raiz_van_a_su_sitio() {
        let (f, mut e) = (firma(), Estado::default());
        assert_eq!(poner(&mut e, &f, 2, 1, &[7, 8, 9]), Ok(()));
        assert_eq!(poner(&mut e, &f, 1, 0, &[1, 2, 3]), Ok(()));
        assert_eq!(&e.raiz32.0[..7], &[1, 2, 3, 0, 7, 8, 9], "el parametro 2 empieza tras las 3 del 1");
        assert!(poner(&mut e, &f, 2, 2, &[1, 2, 3]).is_err(), "se sale de sus 4");
        assert!(poner(&mut e, &f, 0, 0, &[1]).is_err(), "el 0 es un CBV");
        assert!(poner(&mut e, &f, 9, 0, &[1]).is_err());
    }
}
