//! **M5 (G0): LO QUE PIDE EL MOTOR GRAFICO** -- la primera pieza del contexto
//! de GR0 bajo el GSP-RM: preguntarle al RM que buferes de contexto necesita
//! el motor grafico, y de que medida, y calcular como los reparte nouveau.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- es la receta de `r570_gr_get_ctxbufs_and_zcull_info`
//!           y `r535_gr_get_ctxbuf_info` de nouveau; un bufer de menos o de
//!           otra medida y el RM no hace el contexto de oro
//!
//! # El camino (nouveau `r535_gr_oneinit`), y donde esta cada paso
//!
//! ```text
//!    G0  preguntar los buferes: INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO
//!        (0x20800A32) sobre el subdispositivo INTERNO del RM          (aqui)
//!    G1  un canal en GR0 (lista 0), como el de L1d2b
//!    G2  los buferes en VRAM, mapeados en nuestro espacio
//!    G3  PROMOTE_CTX (0x2080012B) con cada bufer
//!    G4  AMPERE_B (0xC797) en ese canal: el RM hace el contexto de ORO
//! ```
//!
//! # La respuesta (r570.144, `nvrm/gr.h` de nouveau)
//!
//! `engineContextBuffersInfo[8]` (uno por motor grafico), cada uno con
//! `engine[0x1A]` de `{ size, alignment }` (8 B): 8 x 26 x 8 = 1664 B. Solo
//! cuenta el motor 0. Los indices son `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_
//! PROPERTIES_ENGINE_ID_*`.
//!
//! # Por que el cliente INTERNO
//!
//! Es una orden `INTERNAL`: nouveau la manda sobre `gsp->internal.device.
//! subdevice`, las asas del RM que da `GET_GSP_STATIC_INFO` (L1a, fila
//! `asas`). El contrato la deja salir SOLO esta, con sus 1664 B a cero: es una
//! pregunta y no cambia nada.

use crate::control::{CABECERA_CONTROL, GSP_RM_CONTROL};
use crate::orden;

/// `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO`.
pub const BUFERES: u32 = 0x2080_0A32;
/// `NV2080_CTRL_INTERNAL_GR_MAX_ENGINES`.
const MOTORES: usize = 8;
/// `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_COUNT` en r570.
pub const IDS: usize = 0x1A;
/// Lo que miden los parametros.
pub const MEDIDA: usize = MOTORES * IDS * 8;

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **La pregunta**, sobre el cliente y el subdispositivo INTERNOS del RM.
pub fn pedir(hueco: &mut [u8], numero: u32, cliente: u32, subdispositivo: u32) -> Option<usize> {
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL + MEDIDA, |d| {
        poner(d, 0, cliente);
        poner(d, 4, subdispositivo);
        poner(d, 8, BUFERES);
        poner(d, 16, MEDIDA as u32);
        // Los parametros, a cero (`componer` ya los dejo asi).
    })
}

/// **Puede salir?** Solo esta orden, con su medida y sus parametros a cero;
/// con cualquier asa distinta de 0 (las del RM, que da L1a). `d` son los
/// datos del mensaje (cabecera de control y parametros).
pub fn permitida(d: &[u8]) -> bool {
    if d.len() < CABECERA_CONTROL + MEDIDA {
        return false;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    u(0) != 0
        && u(4) != 0
        && u(8) == BUFERES
        && u(16) as usize == MEDIDA
        && d[CABECERA_CONTROL..CABECERA_CONTROL + MEDIDA].iter().all(|&b| b == 0)
}

/// Un bufer que pide el motor grafico, como lo prepara nouveau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bufer {
    /// `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_*`.
    pub id: u8,
    /// `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`.
    pub promover: u8,
    pub nombre: &'static [u8],
    /// Lo que dijo el RM.
    pub medida_rm: u32,
    pub alineado_rm: u32,
    /// Lo que se reserva: la de MAIN lleva 64 cabeceras de subcontexto mas.
    pub medida: u64,
    /// Paginas de 2^`pagina` (12, 16 o 21) y alineada a 2^`alinear`.
    pub pagina: u8,
    pub alinear: u8,
    /// Uno para todos los canales (se hace una vez, en el de oro).
    pub global: bool,
    /// El RM lo rellena al promoverlo.
    pub iniciar: bool,
    /// Solo lectura para la GPU.
    pub ro: bool,
}

/// La tabla de `r535_gr_get_ctxbuf_info`: `(id, promover, nombre, global,
/// iniciar, ro)`. Los `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*` de la r570
/// (`nvrm/gpu.h` de nouveau): MAIN 0, PATCH 2, BUNDLE_CB 3, PAGEPOOL 4,
/// ATTRIBUTE_CB 5, RTV_CB_GLOBAL 6, FECS_EVENT 9, PRIV_ACCESS_MAP 10 (y en G3,
/// detras de este, UNRESTRICTED_PRIV_ACCESS_MAP 11 con su misma memoria).
pub const TABLA: [(u8, u8, &[u8], bool, bool, bool); 8] = [
    (0x00, 0, b"MAIN", false, true, false),
    (0x10, 2, b"PATCH", false, true, false),
    (0x11, 3, b"BUNDLE_CB", true, false, false),
    (0x0D, 4, b"PAGEPOOL", true, false, false),
    (0x13, 5, b"ATTRIBUTE_CB", true, false, false),
    (0x14, 6, b"RTV_CB_GLOBAL", true, false, false),
    (0x17, 9, b"FECS_EVENT", true, true, false),
    (0x18, 10, b"PRIV_ACCESS_MAP", true, true, true),
];

/// `order_base_2`: el menor `k` con `2^k >= v`.
const fn orden_base_2(v: u64) -> u8 {
    if v <= 1 {
        0
    } else {
        (64 - (v - 1).leading_zeros()) as u8
    }
}

/// **Los buferes de la respuesta** (`d` son los datos del mensaje), en el
/// orden de [`TABLA`], con la medida, la pagina y la alineacion que calcula
/// nouveau. `None` si la respuesta no llega entera.
pub fn buferes(d: &[u8]) -> Option<[Bufer; 8]> {
    let p = d.get(CABECERA_CONTROL..CABECERA_CONTROL + MEDIDA)?;
    let u = |o: usize| u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
    let mut t = [Bufer::default(); 8];
    for (k, &(id, promover, nombre, global, iniciar, ro)) in TABLA.iter().enumerate() {
        // Motor 0, entrada `id`.
        let (medida_rm, alineado_rm) = (u(8 * id as usize), u(8 * id as usize + 4));
        let mut medida = medida_rm as u64;
        if id == 0x00 {
            // MAIN: las cabeceras de 64 subcontextos detras.
            medida = (medida + 0xFFF) & !0xFFF;
            medida += 64 * 0x1000;
        }
        let pagina = if medida >= 1 << 21 {
            21
        } else if medida >= 1 << 16 {
            16
        } else {
            12
        };
        let alinear = if id == 0x13 { orden_base_2(medida) } else { pagina };
        t[k] = Bufer { id, promover, nombre, medida_rm, alineado_rm, medida, pagina, alinear, global, iniciar, ro };
    }
    Some(t)
}

/// Lo que ocupan todos, alineados: lo que G2 tendra que buscar en VRAM.
pub fn total(t: &[Bufer; 8]) -> u64 {
    t.iter().fold(0u64, |acc, b| {
        let a = 1u64 << b.alinear;
        ((acc + a - 1) & !(a - 1)) + b.medida
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn la_pregunta_va_al_cliente_interno_y_suma_cero() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 21, 0xC200_0006, 0xABCD_2080).unwrap();
        assert_eq!(n, CABECERA + CABECERA_CONTROL + 1664);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero), (GSP_RM_CONTROL, 21));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        assert!(permitida(&h[CABECERA..n]));
        // Con un parametro distinto de cero, u otra orden, no.
        let mut otra = h;
        otra[CABECERA + CABECERA_CONTROL + 7] = 1;
        assert!(!permitida(&otra[CABECERA..n]));
        let mut otra = h;
        otra[CABECERA + 8] = 0x33;
        assert!(!permitida(&otra[CABECERA..n]));
    }

    #[test]
    fn los_buferes_como_nouveau() {
        let mut d = [0u8; CABECERA_CONTROL + MEDIDA];
        let pon = |d: &mut [u8], id: usize, medida: u32| {
            let o = CABECERA_CONTROL + 8 * id;
            d[o..o + 4].copy_from_slice(&medida.to_le_bytes());
        };
        pon(&mut d, 0x00, 0x0003_1234); // MAIN
        pon(&mut d, 0x13, 0x0030_0000); // ATTRIBUTE_CB, 3 MiB
        pon(&mut d, 0x18, 0x1000); // PRIV_ACCESS_MAP
        // El motor 1 no cuenta.
        pon(&mut d, IDS, 0xFFFF_FFFF);
        let t = buferes(&d).unwrap();
        assert_eq!((t[0].nombre, t[0].promover), (b"MAIN" as &[u8], 0));
        assert_eq!((t[7].promover, t[6].promover, t[1].promover), (10, 9, 2));
        assert_eq!(t[0].medida, 0x0003_2000 + 64 * 0x1000, "alineada a 4 KiB y 64 cabeceras");
        assert_eq!((t[0].pagina, t[0].alinear), (16, 16));
        assert_eq!(t[4].nombre, b"ATTRIBUTE_CB");
        assert_eq!((t[4].pagina, t[4].alinear), (21, 22), "order_base_2 de 3 MiB");
        assert_eq!((t[7].pagina, t[7].ro, t[7].iniciar), (12, true, true));
        assert!(total(&t) >= t[0].medida + t[4].medida + t[7].medida);
        assert_eq!(orden_base_2(1 << 21), 21);
        assert_eq!(orden_base_2((1 << 21) + 1), 22);
    }
}
