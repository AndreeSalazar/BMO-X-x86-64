//! **Un CS de D3D12, preparado para despachar** (N5.5, 05-10): leer el
//! sombreador de computo de un PSO, compilarlo al Programa de la casa y
//! aplanar sus cbuffers.
//!
//! El `Dispatch(x, y, z)` mismo -- grupo a grupo, con sus barreras y sus
//! olas: `Programa::despachar` -- vive en PROMETEO desde el 08-10 (LB3b de
//! `docs/plan/PLAN_LAS_LIBRERIAS.md`): `platform/shared/prometeo/src/despacho.rs`.
//!
//! [carril]  VERDE     leer y compilar; no toca la maquina

use alloc::vec;
use alloc::vec::Vec;

use super::programa::Programa;

/// **Un sombreador de computo listo para despachar**: su programa (con los
/// cbuffers ya APLANADOS en un bloque, como los de un dibujo) y donde va
/// cada cbuffer en el (`lote::Bloque`, por su ranura).
#[derive(Debug, Clone, PartialEq)]
pub struct DeComputo {
    pub programa: Programa,
    pub constantes: Vec<crate::lote::Bloque>,
}

/// **Preparar el CS de un PSO de computo**: leerlo, compilarlo y aplanar sus
/// cbuffers. El texto dice por que no, si no.
pub fn preparar(cs: &[u8]) -> Result<DeComputo, alloc::string::String> {
    use alloc::format;
    let s = super::leer(cs).map_err(|e| format!("el CS no se lee: {e:?}"))?;
    if s.etapa != super::Etapa::Computo {
        return Err(format!("el sombreador de un PSO de computo es de otra etapa ({:?})", s.etapa));
    }
    if s.hilos.contains(&0) {
        return Err(alloc::string::String::from("un CS sin numthreads (su PSV0 no lo dice)"));
    }
    // D3D12: hasta 1024 hilos por grupo (x e y hasta 1024, z hasta 64).
    let [x, y, z] = s.hilos;
    if x > 1024 || y > 1024 || z > 64 || x as u64 * y as u64 * z as u64 > 1024 {
        return Err(format!("numthreads({x}, {y}, {z}): mas de lo que deja D3D12 (1024 hilos por grupo)"));
    }
    let mut programa = super::programa::compilar(&s).map_err(|e| format!("el CS no se sabe correr todavia: {e:?}"))?;
    let mut filas = vec![0u16; programa.ranuras.cbuffers.len()];
    programa.filas_por_cbuffer(&mut filas);
    let mut constantes = Vec::with_capacity(filas.len());
    let mut fila = 0u32;
    for &f in &filas {
        constantes.push(crate::lote::Bloque { fila: fila as u16, filas: f });
        fila += f as u32;
    }
    if fila > u16::MAX as u32 {
        return Err(format!("los cbuffers del CS leen {fila} filas: mas de las que caben"));
    }
    let bases: Vec<u16> = constantes.iter().map(|b| b.fila).collect();
    programa.aplanar(&bases);
    Ok(DeComputo { programa, constantes })
}
