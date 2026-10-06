//! **La root signature serializada** (P3b2, 27-09): la parte `RTS0` de un
//! contenedor DXBC.
//!
//! Una root signature dice que le llega a los sombreadores y por donde: en la
//! RAIZ (constantes de 32 bits o la direccion de un bufer) o por TABLAS de
//! descriptores. Un programa la escribe como estructura, la pasa por
//! `D3D12SerializeRootSignature` y le da los bytes a `CreateRootSignature`; y
//! muchos juegos la traen ya serializada (dentro del propio sombreador, o
//! compilada con `dxc -T rootsig_1_0`). Asi que la casa sabe las dos cosas:
//! escribir y leer los MISMOS bytes que Microsoft.
//!
//! ```text
//!    RTS0 (1.0)   version | n parametros | desde | n samplers | desde | banderas
//!    parametro    tipo | visibilidad | desde (su carga)
//!    carga        tabla: n rangos | desde -> rangos de 5 u32
//!                 constantes: registro | espacio | cuantas
//!                 CBV/SRV/UAV: registro | espacio
//!    sampler      13 u32 (D3D12_STATIC_SAMPLER_DESC)
//! ```
//!
//! Los desplazamientos van desde el inicio de la parte.
//!
//! **La 1.1** (version 2, 06-10): la que `dxc` mete en un sombreador con
//! `[RootSignature(...)]` y la que serializa Microsoft si se le pide. Lo
//! mismo con BANDERAS: cada rango, 6 u32 (las banderas entre el espacio y
//! su desplazamiento en la tabla); cada CBV/SRV/UAV de la raiz, 3 (las
//! banderas detras). Son pistas para el driver (`DATA_STATIC`,
//! `DESCRIPTORS_VOLATILE`...): se leen y la casa hace lo mismo con ellas o
//! sin ellas. La 1.2 (version 3, samplers con banderas) no: `dxc` aun no la
//! escribe y no hay un binario de Microsoft contra el que mirarla.
//!
//! ```text
//!    RTS0 (1.1)   lo de la 1.0, con
//!    rango        tipo | cuantos | registro | espacio | BANDERAS | desde
//!    CBV/SRV/UAV  registro | espacio | BANDERAS
//! ```

use alloc::vec::Vec;

use crate::dxbc;

pub const TABLA: u32 = 0;
pub const CONSTANTES: u32 = 1;
pub const CBV: u32 = 2;
pub const SRV: u32 = 3;
pub const UAV: u32 = 4;

/// `D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT`.
pub const CON_INPUT_LAYOUT: u32 = 1;

/// Un rango de una tabla de descriptores: tipo, cuantos, registro base,
/// espacio, y donde empieza en la tabla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rango {
    pub tipo: u32,
    pub cuantos: u32,
    pub registro: u32,
    pub espacio: u32,
    pub desde: u32,
}

/// Lo que va en un parametro de la raiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carga {
    Tabla(Vec<Rango>),
    Constantes { registro: u32, espacio: u32, cuantas: u32 },
    /// CBV, SRV o UAV: la direccion de un bufer, en la raiz.
    Descriptor { registro: u32, espacio: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parametro {
    pub tipo: u32,
    /// 0 = todos los sombreadores; 1 vertice ... 5 pixel.
    pub visibilidad: u32,
    pub carga: Carga,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Firma {
    pub parametros: Vec<Parametro>,
    /// Los samplers estaticos, tal cual (13 u32 cada uno).
    pub samplers: Vec<[u32; 13]>,
    pub banderas: u32,
}

/// Por que unos bytes no son una root signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoFirma {
    Contenedor,
    SinRts0,
    Version(u32),
    Corta,
    Tipo(u32),
}

fn u(p: &[u8], o: usize) -> Result<u32, NoFirma> {
    p.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(NoFirma::Corta)
}

/// **Leer** una root signature serializada (el contenedor entero).
pub fn leer(d: &[u8]) -> Result<Firma, NoFirma> {
    let partes = dxbc::partes(d).ok_or(NoFirma::Contenedor)?;
    let p = partes.iter().find(|(cc, _)| cc == b"RTS0").map(|(_, p)| *p).ok_or(NoFirma::SinRts0)?;
    let version = u(p, 0)?;
    if version != 1 && version != 2 {
        return Err(NoFirma::Version(version));
    }
    // 1.1: un rango mide 24 bytes y su desplazamiento va detras de las
    // banderas (+20); en la 1.0, 20 bytes y +16.
    let (rango, su_desde) = if version == 2 { (24, 20) } else { (20, 16) };
    let (n, desde, ns, desde_s, banderas) = (u(p, 4)?, u(p, 8)? as usize, u(p, 12)?, u(p, 16)? as usize, u(p, 20)?);
    let mut parametros = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let e = desde + 12 * i;
        let (tipo, visibilidad, carga) = (u(p, e)?, u(p, e + 4)?, u(p, e + 8)? as usize);
        let carga = match tipo {
            TABLA => {
                let (nr, dr) = (u(p, carga)?, u(p, carga + 4)? as usize);
                let mut v = Vec::with_capacity(nr as usize);
                for k in 0..nr as usize {
                    let r = dr + rango * k;
                    v.push(Rango { tipo: u(p, r)?, cuantos: u(p, r + 4)?, registro: u(p, r + 8)?, espacio: u(p, r + 12)?, desde: u(p, r + su_desde)? });
                }
                Carga::Tabla(v)
            }
            CONSTANTES => Carga::Constantes { registro: u(p, carga)?, espacio: u(p, carga + 4)?, cuantas: u(p, carga + 8)? },
            CBV | SRV | UAV => Carga::Descriptor { registro: u(p, carga)?, espacio: u(p, carga + 4)? },
            t => return Err(NoFirma::Tipo(t)),
        };
        parametros.push(Parametro { tipo, visibilidad, carga });
    }
    let mut samplers = Vec::with_capacity(ns as usize);
    for k in 0..ns as usize {
        let mut s = [0u32; 13];
        for (j, v) in s.iter_mut().enumerate() {
            *v = u(p, desde_s + 52 * k + 4 * j)?;
        }
        samplers.push(s);
    }
    Ok(Firma { parametros, samplers, banderas })
}

fn pon(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// **Serializar**: la parte RTS0 en el orden en que la escribe Microsoft
/// (cabecera, parametros, sus cargas, y los rangos de cada tabla detras de su
/// carga; los samplers al final), dentro de un contenedor con su huella.
pub fn serializar(f: &Firma) -> Vec<u8> {
    let n = f.parametros.len();
    let desde = 24;
    let mut cargas: Vec<u8> = Vec::new();
    let mut desde_carga = Vec::with_capacity(n);
    let base = desde + 12 * n;
    for par in &f.parametros {
        desde_carga.push(base + cargas.len());
        match &par.carga {
            Carga::Tabla(rangos) => {
                pon(&mut cargas, rangos.len() as u32);
                let rangos_desde = (base + cargas.len() + 4) as u32;
                pon(&mut cargas, rangos_desde);
                for r in rangos {
                    for x in [r.tipo, r.cuantos, r.registro, r.espacio, r.desde] {
                        pon(&mut cargas, x);
                    }
                }
            }
            Carga::Constantes { registro, espacio, cuantas } => {
                for x in [*registro, *espacio, *cuantas] {
                    pon(&mut cargas, x);
                }
            }
            Carga::Descriptor { registro, espacio } => {
                pon(&mut cargas, *registro);
                pon(&mut cargas, *espacio);
            }
        }
    }
    let desde_s = base + cargas.len();
    let mut p = Vec::new();
    for x in [1, n as u32, desde as u32, f.samplers.len() as u32, desde_s as u32, f.banderas] {
        pon(&mut p, x);
    }
    for (par, &c) in f.parametros.iter().zip(&desde_carga) {
        pon(&mut p, par.tipo);
        pon(&mut p, par.visibilidad);
        pon(&mut p, c as u32);
    }
    p.extend_from_slice(&cargas);
    for s in &f.samplers {
        for &x in s {
            pon(&mut p, x);
        }
    }
    dxbc::contenedor(&[(*b"RTS0", &p)])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La MISMA firma (`prueba/firmas.hlsl`), hecha por `dxc`: en 1.1 (con
    /// banderas en los rangos y en el UAV de la raiz), en 1.0, y la que
    /// mete dentro de un sombreador ([RootSignature], en 1.1).
    const RS_11: &[u8] = include_bytes!("../prueba/firmas_11.rts0");
    const RS_10: &[u8] = include_bytes!("../prueba/firmas_10.rts0");
    const CS: &[u8] = include_bytes!("../prueba/firmas_cs.dxil");

    #[test]
    fn la_1_1_de_dxc_se_lee_como_la_1_0_de_la_misma_firma() {
        let (f11, f10, fcs) = (leer(RS_11).unwrap(), leer(RS_10).unwrap(), leer(CS).unwrap());
        // Las banderas son pistas para el driver: la firma es la misma.
        assert_eq!(f11, f10);
        assert_eq!(fcs, f10);
        let tabla = alloc::vec![Rango { tipo: 0, cuantos: 2, registro: 0, espacio: 0, desde: u32::MAX }, Rango { tipo: 1, cuantos: 1, registro: 1, espacio: 0, desde: u32::MAX }];
        assert_eq!(
            f11.parametros,
            [
                Parametro { tipo: CONSTANTES, visibilidad: 0, carga: Carga::Constantes { registro: 0, espacio: 0, cuantas: 4 } },
                Parametro { tipo: TABLA, visibilidad: 0, carga: Carga::Tabla(tabla) },
                Parametro { tipo: UAV, visibilidad: 0, carga: Carga::Descriptor { registro: 0, espacio: 0 } },
            ]
        );
        // Una version que no se sabe (la 1.2, 3), dicha.
        let mut v3 = RS_11.to_vec();
        let i = v3.windows(4).position(|w| w == b"RTS0").unwrap() + 8;
        v3[i] = 3;
        assert_eq!(leer(&v3), Err(NoFirma::Version(3)));
    }
}
