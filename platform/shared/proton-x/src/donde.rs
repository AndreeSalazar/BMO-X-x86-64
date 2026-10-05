//! **Donde esta cada recurso en la root signature** (03-10, N5.1): de un
//! `t40, space1` que lee el sombreador de pixeles a "parametro 2, ranura 17
//! de su tabla".
//!
//! [carril]  VERDE     solo mira la firma; no toca la maquina ni las tablas
//! [cuesta]  DATO      una ranura mal contada da la textura de al lado
//! [riesgo]  ESPEJO    las reglas son las de D3D12 (rangos con `desde`
//!                     0xFFFFFFFF, sin medida, por etapa); el banco las
//!                     prueba con firmas hechas a mano
//! [consumo] NADA      por textura y por dibujo: unos pocos rangos
//!
//! Hasta el 03-10 la casa iba al reves: recorria las tablas y ponia cada
//! descriptor en su REGISTRO, solo del espacio 0 y de t0..t31 y s0..s15
//! ("una tabla con un espacio de registros que no es el 0, o de mas de 32:
//! se salta"). Ahora va desde lo que el sombreador LEE (sus ranuras,
//! [`crate::dxil::programa::Ranuras`]) y busca cada una:
//!
//! ```text
//!    una tabla     sus rangos del tipo (SRV, sampler...) y del espacio, con
//!                  el registro dentro: [registro, registro + cuantos).
//!                  `desde` 0xFFFFFFFF = justo tras el rango anterior;
//!                  `cuantos` 0xFFFFFFFF = sin medida (hasta el final)
//!    la etapa      un parametro se ve por todas (0) o por UNA: el t0 del de
//!                  vertices y el t0 del de pixeles pueden ser dos tablas
//!    los estaticos un sampler de la firma: registro, espacio y etapa
//! ```

use crate::dxil::programa::Lugar;
use crate::raiz::{Carga, Firma};

/// `D3D12_DESCRIPTOR_RANGE_TYPE`: los mismos numeros que las clases de
/// `createHandle` ([`crate::dxil::recursos`]).
pub const RANGO_SRV: u32 = 0;
pub const RANGO_UAV: u32 = 1;
pub const RANGO_CBV: u32 = 2;
pub const RANGO_MUESTREADOR: u32 = 3;

/// `D3D12_SHADER_VISIBILITY`: todas, la de vertices, la de geometria y la
/// de pixeles.
pub const VISTA_TODAS: u32 = 0;
pub const VISTA_VERTICES: u32 = 1;
/// E2.3b: la del sombreador de geometria.
pub const VISTA_GEOMETRIA: u32 = 4;
pub const VISTA_PIXELES: u32 = 5;

/// `D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND`, y el `cuantos` de un rango sin
/// medida (`UINT_MAX`).
const A_CONTINUACION: u32 = 0xFFFF_FFFF;
const SIN_MEDIDA: u32 = 0xFFFF_FFFF;

/// Si un parametro (o un sampler estatico) de `vista` lo ve la etapa de `l`.
/// Un lugar sin etapa (0) lo ve todo.
fn lo_ve(vista: u32, l: Lugar) -> bool {
    vista == VISTA_TODAS || l.vista == VISTA_TODAS || vista == l.vista
}

/// **El descriptor de `l` en las tablas**: `(parametro, ranura en su tabla)`,
/// o `None` si ninguna tabla lo tiene.
pub fn en_tabla(f: &Firma, tipo: u32, l: Lugar) -> Option<(usize, u64)> {
    for (k, p) in f.parametros.iter().enumerate() {
        let Carga::Tabla(rangos) = &p.carga else { continue };
        if !lo_ve(p.visibilidad, l) {
            continue;
        }
        let mut siguiente = Some(0u64);
        for r in rangos {
            // Tras uno sin medida, "a continuacion" no tiene donde ir: en
            // D3D12 eso no valida, y aqui no se encuentra nada en el.
            let Some(desde) = (if r.desde == A_CONTINUACION { siguiente } else { Some(r.desde as u64) }) else {
                siguiente = None;
                continue;
            };
            siguiente = if r.cuantos == SIN_MEDIDA { None } else { Some(desde + r.cuantos as u64) };
            if r.tipo != tipo || r.espacio != l.espacio || l.registro < r.registro {
                continue;
            }
            let dentro = (l.registro - r.registro) as u64;
            if r.cuantos == SIN_MEDIDA || dentro < r.cuantos as u64 {
                return Some((k, desde + dentro));
            }
        }
    }
    None
}

/// **Donde esta un cbuffer** (N5.2): tres sitios posibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cb {
    /// Un CBV en la raiz (`SetGraphicsRootConstantBufferView`): la direccion
    /// dada al parametro.
    Raiz(usize),
    /// Constantes de 32 bits en la raiz (`SetGraphicsRoot32BitConstants`):
    /// `cuantas`, desde la palabra `desde` de las de la raiz (ver
    /// [`constantes_desde`]).
    Constantes { desde: usize, cuantas: u32 },
    /// Un CBV en una tabla: `(parametro, ranura)`, como [`en_tabla`].
    Tabla(usize, u64),
}

/// **Donde guarda la casa las constantes del parametro `k`**: la palabra
/// en que empiezan, contando las de los parametros de constantes de antes.
/// Una root signature lleva a lo sumo 64 palabras en total, asi que caben
/// todas juntas.
pub fn constantes_desde(f: &Firma, k: usize) -> usize {
    f.parametros.iter().take(k).map(|p| if let Carga::Constantes { cuantas, .. } = p.carga { cuantas as usize } else { 0 }).sum()
}

/// **El cbuffer de `l`**: en la raiz o en una tabla; `None` si no esta.
pub fn cbuffer(f: &Firma, l: Lugar) -> Option<Cb> {
    for (k, p) in f.parametros.iter().enumerate() {
        if !lo_ve(p.visibilidad, l) {
            continue;
        }
        match p.carga {
            Carga::Descriptor { registro, espacio } if p.tipo == crate::raiz::CBV && (espacio, registro) == (l.espacio, l.registro) => return Some(Cb::Raiz(k)),
            Carga::Constantes { registro, espacio, cuantas } if (espacio, registro) == (l.espacio, l.registro) => return Some(Cb::Constantes { desde: constantes_desde(f, k), cuantas }),
            _ => {}
        }
    }
    en_tabla(f, RANGO_CBV, l).map(|(k, i)| Cb::Tabla(k, i))
}

/// **El sampler estatico de `l`**, si la firma lo trae.
pub fn estatico(f: &Firma, l: Lugar) -> Option<&[u32; 13]> {
    // D3D12_STATIC_SAMPLER_DESC: ShaderRegister +10, RegisterSpace +11,
    // ShaderVisibility +12 (en u32).
    f.samplers.iter().find(|s| s[10] == l.registro && s[11] == l.espacio && lo_ve(s[12], l))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::raiz::{Parametro, Rango, TABLA};
    use alloc::vec;

    fn tabla(vista: u32, rangos: &[Rango]) -> Parametro {
        Parametro { tipo: TABLA, visibilidad: vista, carga: Carga::Tabla(rangos.to_vec()) }
    }
    fn rango(tipo: u32, cuantos: u32, registro: u32, espacio: u32, desde: u32) -> Rango {
        Rango { tipo, cuantos, registro, espacio, desde }
    }
    fn lugar(espacio: u32, registro: u32, vista: u32) -> Lugar {
        Lugar { espacio, registro, vista }
    }

    /// Lo que trae Cyberpunk (a grandes rasgos): un cbuffer en la raiz, y
    /// tablas con rangos sin medida en espacios altos.
    fn firma() -> Firma {
        Firma {
            parametros: vec![
                Parametro { tipo: crate::raiz::CBV, visibilidad: 0, carga: Carga::Descriptor { registro: 0, espacio: 0 } },
                tabla(VISTA_PIXELES, &[rango(RANGO_SRV, 4, 0, 0, 0), rango(RANGO_SRV, 8, 40, 1, A_CONTINUACION), rango(RANGO_SRV, SIN_MEDIDA, 0, 3, A_CONTINUACION)]),
                tabla(VISTA_VERTICES, &[rango(RANGO_SRV, 2, 0, 0, 5)]),
                tabla(VISTA_TODAS, &[rango(RANGO_MUESTREADOR, 16, 0, 0, 0), rango(RANGO_MUESTREADOR, 4, 20, 1, 100)]),
            ],
            samplers: vec![{
                let mut s = [0u32; 13];
                s[10] = 7;
                s[11] = 2;
                s[12] = VISTA_PIXELES;
                s
            }],
            banderas: 0,
        }
    }

    #[test]
    fn cada_lugar_en_su_ranura() {
        let f = firma();
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(0, 3, VISTA_PIXELES)), Some((1, 3)));
        // El segundo rango va "a continuacion" del primero (4 ranuras).
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(1, 40, VISTA_PIXELES)), Some((1, 4)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(1, 47, VISTA_PIXELES)), Some((1, 11)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(1, 48, VISTA_PIXELES)), None, "pasado el rango");
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(1, 39, VISTA_PIXELES)), None, "antes del rango");
        // Sin medida: tras los 12 de antes, y tan lejos como pida.
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(3, 1000, VISTA_PIXELES)), Some((1, 1012)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(2, 0, VISTA_PIXELES)), None, "un espacio que no esta");
        assert_eq!(en_tabla(&f, RANGO_MUESTREADOR, lugar(1, 21, VISTA_PIXELES)), Some((3, 101)));
        assert_eq!(en_tabla(&f, RANGO_UAV, lugar(0, 0, VISTA_PIXELES)), None, "otro tipo");
    }

    #[test]
    fn el_t0_de_cada_etapa_es_el_suyo() {
        let f = firma();
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(0, 0, VISTA_PIXELES)), Some((1, 0)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(0, 0, VISTA_VERTICES)), Some((2, 5)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(0, 2, VISTA_VERTICES)), None, "la del de vertices mide 2");
        // Los samplers los ven las dos.
        assert_eq!(en_tabla(&f, RANGO_MUESTREADOR, lugar(0, 15, VISTA_VERTICES)), Some((3, 15)));
    }

    #[test]
    fn los_estaticos_por_registro_espacio_y_etapa() {
        let f = firma();
        assert!(estatico(&f, lugar(2, 7, VISTA_PIXELES)).is_some());
        assert!(estatico(&f, lugar(0, 7, VISTA_PIXELES)).is_none(), "otro espacio");
        assert!(estatico(&f, lugar(2, 7, VISTA_VERTICES)).is_none(), "solo lo ve el de pixeles");
    }

    #[test]
    fn los_cbuffers_en_la_raiz_en_constantes_y_en_tablas() {
        use crate::raiz::{CBV, CONSTANTES};
        let f = Firma {
            parametros: vec![
                Parametro { tipo: CONSTANTES, visibilidad: VISTA_VERTICES, carga: Carga::Constantes { registro: 1, espacio: 0, cuantas: 4 } },
                Parametro { tipo: CBV, visibilidad: VISTA_TODAS, carga: Carga::Descriptor { registro: 0, espacio: 0 } },
                Parametro { tipo: CONSTANTES, visibilidad: VISTA_PIXELES, carga: Carga::Constantes { registro: 1, espacio: 0, cuantas: 2 } },
                tabla(VISTA_TODAS, &[rango(RANGO_SRV, 3, 0, 0, 0), rango(RANGO_CBV, 2, 4, 7, A_CONTINUACION)]),
            ],
            samplers: vec![],
            banderas: 0,
        };
        assert_eq!(cbuffer(&f, lugar(0, 0, VISTA_PIXELES)), Some(Cb::Raiz(1)));
        assert_eq!(cbuffer(&f, lugar(0, 1, VISTA_VERTICES)), Some(Cb::Constantes { desde: 0, cuantas: 4 }));
        // El b1 del de pixeles es OTRO parametro: sus palabras van tras las 4.
        assert_eq!(cbuffer(&f, lugar(0, 1, VISTA_PIXELES)), Some(Cb::Constantes { desde: 4, cuantas: 2 }));
        assert_eq!(cbuffer(&f, lugar(7, 5, VISTA_PIXELES)), Some(Cb::Tabla(3, 4)));
        assert_eq!(cbuffer(&f, lugar(7, 6, VISTA_PIXELES)), None);
        assert_eq!(cbuffer(&f, lugar(0, 2, VISTA_PIXELES)), None);
        assert_eq!(constantes_desde(&f, 3), 6);
    }

    #[test]
    fn rangos_que_desbordan_no_rompen() {
        let f = Firma {
            parametros: vec![tabla(0, &[rango(RANGO_SRV, SIN_MEDIDA, 0, 0, 0xFFFF_FFF0), rango(RANGO_SRV, 4, 0, 1, A_CONTINUACION)])],
            samplers: vec![],
            banderas: 0,
        };
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(0, 0xFFFF_FFFE, 0)), Some((0, 0xFFFF_FFF0 + 0xFFFF_FFFE)));
        assert_eq!(en_tabla(&f, RANGO_SRV, lugar(1, 0, 0)), None, "a continuacion de uno sin medida");
    }
}
