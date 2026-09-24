//! **EL CONTRATO DE LA COLA DE LA CPU** -- la lista CERRADA de lo que puede
//! salir de este sistema hacia el GSP-RM. El kernel la pasa sobre cada mensaje
//! ya armado, antes de mover el `writePtr` y de tocar el timbre: lo que no esta
//! aqui no sale, lo haya armado quien lo haya armado.
//!
//! capa: puro -- mira bytes; no toca un registro (L8)
//!
//! [eje]     AISLAMIENTO -- el escritorio nunca manda bytes al GSP (dice CUAL
//!           pregunta, de una lista); esto es la segunda llave: aunque un
//!           camino del kernel armara otra cosa, el timbre no suena
//!
//! ```text
//!    SET_SYSTEM_INFO   72   antes de despertar (L0c4b2a)
//!    SET_REGISTRY      73   antes de despertar (L0c4b2a)
//!    GET_GSP_STATIC_INFO 65 L1a
//!    GSP_RM_ALLOC     103   L1b: SOLO nuestro cliente, dispositivo,
//!                           subdispositivo y espacio, con sus asas y clases;
//!                           y L1d2b: NUESTRO canal, con sus 368 B exactos
//!                           (su memoria, su motor, su espacio); y L1d3: su
//!                           copiador AMPERE_DMA_COPY_B sobre COPY2 (8 B)
//!    GSP_RM_CONTROL    76   SOLO las ordenes de `control::Control`, cada
//!                           una sobre SU objeto nuestro y con SUS parametros
//!                           exactos (el directorio de L1c3: su direccion y
//!                           su espacio, byte a byte)
//! ```
//!
//! Agrandar la lista es una decision, y se toma aqui: con su prueba.

use crate::canal;
use crate::copia;
use crate::control::{Control, CABECERA_CONTROL, GSP_RM_CONTROL};
use crate::estatica::GET_GSP_STATIC_INFO;
use crate::objeto::{Objeto, CABECERA_ALLOC, CLIENTE, GSP_RM_ALLOC};
use crate::orden::{SET_REGISTRY, SET_SYSTEM_INFO};
use crate::rpc::{Mensaje, CABECERA};

/// Por que NO.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum No {
    /// Corto, o sin la forma de un mensaje (firma, version, medida).
    Forma,
    /// Una funcion que no esta en la lista.
    Funcion(u32),
    /// Un `GSP_RM_ALLOC` que no es uno de nuestros tres objetos.
    Objeto,
    /// Un `GSP_RM_CONTROL` fuera de nuestro subdispositivo o de la lista.
    Control,
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// **El mensaje entero (cabecera de 80 B y datos) puede salir?**
pub fn permitido(m: &[u8]) -> Result<u32, No> {
    if m.len() < CABECERA {
        return Err(No::Forma);
    }
    let h = Mensaje::de(m[..CABECERA].try_into().map_err(|_| No::Forma)?);
    if !h.bien_formado() || m.len() < CABECERA + h.datos() {
        return Err(No::Forma);
    }
    let d = &m[CABECERA..CABECERA + h.datos()];
    match h.funcion {
        SET_SYSTEM_INFO | SET_REGISTRY | GET_GSP_STATIC_INFO => Ok(h.funcion),
        GSP_RM_ALLOC => {
            if d.len() < 16 {
                return Err(No::Objeto);
            }
            let (cliente, padre, asa, clase) = (u32_de(d, 0), u32_de(d, 4), u32_de(d, 8), u32_de(d, 12));
            let nuestro = Objeto::TODOS.iter().any(|o| {
                let f = o.forma();
                (f.0, f.1, f.2, f.3) == (cliente, padre, asa, clase)
            });
            // El canal y su copiador: su forma Y sus parametros, byte a byte.
            let exacto = |f: (u32, u32, u32, u32, usize), parametros: fn(&mut [u8]) -> usize| {
                let mut esperados = [0u8; canal::MEDIDA];
                let n = parametros(&mut esperados[..f.4]);
                (f.0, f.1, f.2, f.3) == (cliente, padre, asa, clase)
                    && u32_de(d, 20) as usize == n
                    && d.len() >= CABECERA_ALLOC + n
                    && d[CABECERA_ALLOC..CABECERA_ALLOC + n] == esperados[..n]
            };
            let es_el_canal = exacto(canal::forma(), canal::parametros);
            let es_el_copiador = exacto(copia::forma(), copia::parametros);
            if nuestro || es_el_canal || es_el_copiador {
                Ok(h.funcion)
            } else {
                Err(No::Objeto)
            }
        }
        GSP_RM_CONTROL => {
            if d.len() < CABECERA_CONTROL {
                return Err(No::Control);
            }
            let (cliente, objeto, cmd, medida) = (u32_de(d, 0), u32_de(d, 4), u32_de(d, 8), u32_de(d, 16) as usize);
            let bien = Control::TODOS.iter().any(|&c| {
                let (c_cmd, c_medida, c_objeto) = c.forma();
                cliente == CLIENTE
                    && objeto == c_objeto
                    && cmd == c_cmd
                    && medida == c_medida
                    && d.len() >= CABECERA_CONTROL
                    && c.iguales(&d[CABECERA_CONTROL..])
            });
            if bien {
                Ok(h.funcion)
            } else {
                Err(No::Control)
            }
        }
        f => Err(No::Funcion(f)),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{control, estatica, objeto, orden};

    #[test]
    fn lo_de_la_lista_sale() {
        let mut h = [0u8; 4096];
        let n = estatica::pregunta(&mut h, 2).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(65));
        for o in Objeto::TODOS {
            let n = objeto::pedir(&mut h, 3, o).unwrap();
            assert_eq!(permitido(&h[..n]), Ok(103));
        }
        for c in Control::TODOS {
            let n = control::pedir(&mut h, 4, c).unwrap();
            assert_eq!(permitido(&h[..n]), Ok(76));
        }
        let n = orden::registro(&mut h, 1).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(73));
        let n = crate::canal::pedir(&mut h, 12).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(103));
    }

    #[test]
    fn otro_canal_no_sale() {
        let mut h = [0u8; 4096];
        // Nuestro canal con OTRO motor (COPY0, una GRCE).
        let n = crate::canal::pedir(&mut h, 13).unwrap();
        h[CABECERA + CABECERA_ALLOC + 128] = 0x09;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // Con el bufer de metodos en otra IOVA.
        let n = crate::canal::pedir(&mut h, 14).unwrap();
        h[CABECERA + CABECERA_ALLOC + 219] ^= 0x01;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // Con la instancia en otra VRAM.
        let n = crate::canal::pedir(&mut h, 15).unwrap();
        h[CABECERA + CABECERA_ALLOC + 146] ^= 0x10;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // Privilegiado (bit 5 de flags).
        let n = crate::canal::pedir(&mut h, 16).unwrap();
        h[CABECERA + CABECERA_ALLOC + 20] |= 0x20;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // El copiador sobre OTRO motor, o colgado de otro padre.
        let n = crate::copia::pedir(&mut h, 18).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(103));
        h[CABECERA + CABECERA_ALLOC + 4] = 0x09;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        let n = crate::copia::pedir(&mut h, 19).unwrap();
        h[CABECERA + 4] ^= 0x01;
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // Un BIND a otro motor.
        let n = control::pedir(&mut h, 17, Control::Atar).unwrap();
        h[CABECERA + 24] = 0x01;
        assert_eq!(permitido(&h[..n]), Err(No::Control));
    }

    #[test]
    fn lo_de_fuera_no_sale() {
        let mut h = [0u8; 4096];
        // Una funcion que no esta: FREE (10).
        let n = orden::componer(&mut h, 5, 10, 16, |_| {}).unwrap();
        assert_eq!(permitido(&h[..n]), Err(No::Funcion(10)));
        // Un ALLOC de otra clase (un canal, 0xC56F) con nuestras asas.
        let n = objeto::pedir(&mut h, 6, Objeto::Subdispositivo).unwrap();
        h[CABECERA + 12..CABECERA + 16].copy_from_slice(&0xC56Fu32.to_le_bytes());
        assert_eq!(permitido(&h[..n]), Err(No::Objeto));
        // Un CONTROL sobre las asas INTERNAS del RM, no las nuestras.
        let n = control::pedir(&mut h, 7, Control::Pstate).unwrap();
        h[CABECERA..CABECERA + 4].copy_from_slice(&0xC200_0006u32.to_le_bytes());
        assert_eq!(permitido(&h[..n]), Err(No::Control));
        // Una orden de control que no esta en la lista.
        let n = control::pedir(&mut h, 8, Control::Pstate).unwrap();
        h[CABECERA + 8..CABECERA + 12].copy_from_slice(&0x2080_0101u32.to_le_bytes());
        assert_eq!(permitido(&h[..n]), Err(No::Control));
        // El directorio en OTRA direccion de VRAM: los parametros no son los
        // fijos.
        let n = control::pedir(&mut h, 9, Control::Directorio).unwrap();
        h[CABECERA + 24] ^= 0x10;
        assert_eq!(permitido(&h[..n]), Err(No::Control));
        // El directorio sobre el subdispositivo, no sobre el dispositivo.
        let n = control::pedir(&mut h, 10, Control::Directorio).unwrap();
        h[CABECERA + 4..CABECERA + 8].copy_from_slice(&crate::objeto::SUBDISPOSITIVO.to_le_bytes());
        assert_eq!(permitido(&h[..n]), Err(No::Control));
        // Corto, o sin forma.
        assert_eq!(permitido(&h[..40]), Err(No::Forma));
        assert_eq!(permitido(&[0u8; 200]), Err(No::Forma));
    }
}
