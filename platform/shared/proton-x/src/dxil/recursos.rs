//! **Los recursos de un sombreador, de su parte PSV0** (03-10, N5.1): cada
//! textura, muestreador, cbuffer y UAV con su ESPACIO y sus registros.
//!
//! [carril]  VERDE     lee bytes del contenedor; no toca la maquina
//! [cuesta]  DATO      un espacio mal leido da la textura de otro
//! [riesgo]  ESPEJO    el orden lo pone dxc (y FXC no tiene PSV0); el banco lo
//!                     compara con lo que dice `dxc -dumpbin` de un sombreador
//!                     con tres espacios
//! [consumo] NADA      una vez por sombreador, al crear el PSO
//!
//! `dx.op.createHandle(clase, rangeId, indice, ...)` dice la CLASE, el numero
//! del rango DENTRO de su clase y el REGISTRO absoluto, pero no el espacio:
//! ese esta en los metadatos `dx.resources` (numeros de LLVM, pesados de
//! leer) y, plano, en la parte PSV0 del contenedor:
//!
//! ```text
//!    PSV0      u32 medida de la cabecera, la cabecera, u32 cuantos recursos,
//!              u32 medida de cada uno (16 o 24), y cada uno:
//!              tipo, espacio, primer registro, ultimo registro (, clase, banderas)
//!    el orden  los cbuffers, los muestreadores, los SRV y los UAV, cada clase
//!              en el orden de su rangeId (medido con dxc: ver `pruebas`)
//! ```
//!
//! Hasta el 03-10 la casa ignoraba el espacio y solo admitia t0..t31 y
//! s0..s15: el primer sombreador de Cyberpunk con un recurso mas alla se
//! quedaba sin correr ("un recurso fuera de t0..t31 o s0..s15").

use alloc::vec::Vec;

/// Las clases de `createHandle`.
pub const SRV: u8 = 0;
pub const UAV: u8 = 1;
pub const CBV: u8 = 2;
pub const MUESTREADOR: u8 = 3;

/// **Un recurso declarado**: su clase, su espacio y sus registros (el ultimo
/// incluido; un array sin medida llega hasta `u32::MAX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recurso {
    pub clase: u8,
    pub espacio: u32,
    pub desde: u32,
    pub hasta: u32,
}

fn u32_en(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// La clase de `createHandle` de un `PSVResourceType`.
fn clase(tipo: u32) -> Option<u8> {
    Some(match tipo {
        1 => MUESTREADOR,
        2 => CBV,
        3..=5 => SRV,
        6..=9 => UAV,
        _ => return None,
    })
}

/// **Los recursos de una parte PSV0**, en su orden. `None` si no se lee.
pub fn de_psv0(p: &[u8]) -> Option<Vec<Recurso>> {
    let cabecera = u32_en(p, 0)? as usize;
    let mut o = 4 + cabecera;
    let n = u32_en(p, o)? as usize;
    o += 4;
    if n == 0 {
        return Some(Vec::new());
    }
    let paso = u32_en(p, o)? as usize;
    o += 4;
    if paso < 16 || n > 4096 {
        return None;
    }
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let r = o + i * paso;
        // Un tipo que no se conoce (0, "invalido") se guarda igual: cuenta en
        // el orden de su clase... pero sin clase no hay orden. Se salta.
        let Some(clase) = clase(u32_en(p, r)?) else { continue };
        v.push(Recurso { clase, espacio: u32_en(p, r + 4)?, desde: u32_en(p, r + 8)?, hasta: u32_en(p, r + 12)? });
    }
    Some(v)
}

/// **El recurso `rango` de la clase `clase`** (lo que dice `createHandle`).
pub fn rango(recursos: &[Recurso], clase: u8, rango: u32) -> Option<Recurso> {
    recursos.iter().filter(|r| r.clase == clase).nth(rango as usize).copied()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La PSV0 de `prueba/espacios.dxil` (dxc 1.9, ps_6_0): un cbuffer, dos
    /// muestreadores (s20 en space1 y s2) y tres SRV (t3, t40 en space1 y el
    /// array t7..t10 en space2). `dxc -dumpbin` lo dice asi; el orden de los
    /// muestreadores, por rangeId: s20 es el 0 y s2 el 1.
    #[test]
    fn la_psv0_de_dxc_con_tres_espacios() {
        let d = include_bytes!("../../prueba/espacios.dxil");
        let s = crate::dxil::leer(d).unwrap();
        assert_eq!(
            s.recursos,
            [
                Recurso { clase: CBV, espacio: 0, desde: 0, hasta: 0 },
                Recurso { clase: MUESTREADOR, espacio: 1, desde: 20, hasta: 20 },
                Recurso { clase: MUESTREADOR, espacio: 0, desde: 2, hasta: 2 },
                Recurso { clase: SRV, espacio: 0, desde: 3, hasta: 3 },
                Recurso { clase: SRV, espacio: 1, desde: 40, hasta: 40 },
                Recurso { clase: SRV, espacio: 2, desde: 7, hasta: 10 },
            ]
        );
        assert_eq!(rango(&s.recursos, SRV, 2).map(|r| (r.espacio, r.desde)), Some((2, 7)));
        assert_eq!(rango(&s.recursos, MUESTREADOR, 0).map(|r| (r.espacio, r.desde)), Some((1, 20)));
        assert_eq!(rango(&s.recursos, UAV, 0), None);
    }

    #[test]
    fn una_psv0_rota_no_se_lee() {
        assert_eq!(de_psv0(&[]), None);
        assert_eq!(de_psv0(&[4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), Some(Vec::new()), "sin recursos");
        // Dice tener uno de 8 bytes: menos de los 16 que mide.
        assert_eq!(de_psv0(&[0, 0, 0, 0, 1, 0, 0, 0, 8, 0, 0, 0]), None);
    }
}
