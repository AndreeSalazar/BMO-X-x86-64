//! **EL COMPUTO DE UNA APP: su QMD y su pegado** (LI2f, 13-10) -- lo puro del
//! despacho de un cuerpo de computo que manda una app (el que emite
//! `bmo-proton-x-sm86` con `Abi::Registros`): el QMD de su rejilla y su
//! grupo, con su memoria compartida, y el PEGADO -- las precargas del cuerpo
//! puestas con un MOV cada una, con el valor que sabe el KERNEL al despachar
//! (la VA y el tope de cada UAV y SRV, sus medidas, las filas del cbuffer)
//! --, y detras el cuerpo, con su EXIT.
//!
//! capa: puro -- palabras y campos; la RAM, la IOMMU y el GPFIFO los toca el
//! kernel
//!
//! ```text
//!    el pegado   MOV Rk, valor   (una por precarga, cada una con 6 ciclos:
//!                                 el cuerpo empieza con todo escrito)
//!                el cuerpo       (juzgado: R0..R6, y R7 con sus permisos)
//!    el QMD      V03_00 (`motores::sombreador::qmd_rejilla`), y ademas:
//!                CTA_RASTER_* = los grupos (x, y, z)
//!                CTA_THREAD_DIMENSION* = los hilos del grupo
//!                SHARED_MEMORY_SIZE (561:544) = la compartida, a 256 B
//!                *_SM_CONFIG_SHARED_MEM_SIZE = la configuracion de la SM que
//!                la cabe, mas el KiB que Ampere reserva por grupo: 8, 16, 32
//!                o 64 KiB, en (KiB / 4) + 1 (la cuenta de NVK,
//!                `gv100_sm_config_smem_size`)
//! ```
//!
//! Los techos son los de D3D12 (y los de la 3060): hasta 1024 hilos por
//! grupo (1024 x 1024 x 64), 65535 grupos por eje, y 32 KiB de compartida.
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: el primer despacho de una app lo
//! dira. El QMD de la compartida y de la rejilla en 3D es nuevo; el resto,
//! el de `blur` y `fractal`, que si corrieron.

use crate::motores::sombreador::{campo, qmd_rejilla, QMD_PALABRAS};
use crate::trabajos::cubo::mov;

/// Hilos por grupo, como mucho (D3D12 y la 3060).
pub const MAX_HILOS: u32 = 1024;
/// Grupos por eje, como mucho (D3D12).
pub const MAX_GRUPOS: u32 = 65535;
/// La compartida de un grupo, como mucho (`groupshared` de D3D12).
pub const MAX_COMPARTIDA: u32 = 32 * 1024;
/// Lo que Ampere reserva de la compartida por grupo (sm_8x: 1 KiB).
pub const RESERVA_POR_GRUPO: u32 = 1024;

/// Lo que se despacha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Despacho {
    pub grupos: [u32; 3],
    pub hilos: [u32; 3],
    /// Bytes de compartida del grupo.
    pub compartida: u32,
    /// Registros por hilo (los del cuerpo).
    pub registros: u32,
}

/// Por que un despacho no se puede.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoDespacha {
    /// Los hilos del grupo: 0, o mas de los que caben.
    Hilos,
    /// Los grupos: 0, o mas de 65535 en un eje.
    Grupos,
    /// La compartida pasa de 32 KiB.
    Compartida,
    /// Mas registros de los que da la 3060.
    Registros,
    /// El pegado no cabe donde va.
    NoCabe,
    /// El cuerpo no acaba en EXIT, o esta vacio.
    Cuerpo,
}

impl NoDespacha {
    /// La frase (el lente: cada NO, dicho).
    pub fn texto(self) -> &'static str {
        match self {
            NoDespacha::Hilos => "los hilos de un grupo: 0, o mas de 1024 (1024 x 1024 x 64)",
            NoDespacha::Grupos => "los grupos: 0, o mas de 65535 en un eje",
            NoDespacha::Compartida => "la compartida de un grupo pasa de 32 KiB",
            NoDespacha::Registros => "mas registros por hilo de los que da la 3060",
            NoDespacha::NoCabe => "el programa pegado no cabe en su hueco",
            NoDespacha::Cuerpo => "el cuerpo esta vacio o no acaba en EXIT",
        }
    }
}

/// La configuracion de la SM para `bytes` de compartida por grupo (con su
/// reserva): (KiB / 4) + 1 de 8, 16, 32 o 64 KiB (la de NVK).
pub fn config_compartida(bytes: u32) -> u64 {
    let total = bytes + RESERVA_POR_GRUPO;
    let kib = [8u32, 16, 32, 64].into_iter().find(|&k| total <= k * 1024).unwrap_or(64);
    (kib / 4 + 1) as u64
}

/// **El QMD** de un despacho: el programa en `programa`, RELEASE0 con `paga`
/// en `sem` al acabar la rejilla.
pub fn qmd(programa: u64, d: &Despacho, sem: u64, paga: u32) -> Result<[u32; QMD_PALABRAS], NoDespacha> {
    let [hx, hy, hz] = d.hilos;
    if hx == 0 || hy == 0 || hz == 0 || hx > 1024 || hy > 1024 || hz > 64 || hx * hy * hz > MAX_HILOS {
        return Err(NoDespacha::Hilos);
    }
    if d.grupos.iter().any(|&g| g == 0 || g > MAX_GRUPOS) {
        return Err(NoDespacha::Grupos);
    }
    if d.compartida > MAX_COMPARTIDA {
        return Err(NoDespacha::Compartida);
    }
    if d.registros == 0 || d.registros > 255 {
        return Err(NoDespacha::Registros);
    }
    let mut q = qmd_rejilla(programa, hx, d.grupos[0], d.grupos[1], sem, paga, d.registros);
    campo(&mut q, 463, 448, d.grupos[2] as u64); // CTA_RASTER_DEPTH
    campo(&mut q, 623, 608, hy as u64); // CTA_THREAD_DIMENSION1
    campo(&mut q, 639, 624, hz as u64); // CTA_THREAD_DIMENSION2
    campo(&mut q, 561, 544, d.compartida.next_multiple_of(256) as u64); // SHARED_MEMORY_SIZE
    let c = config_compartida(d.compartida);
    campo(&mut q, 567, 562, c); // MIN_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 574, 569, c); // MAX_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 662, 657, c); // TARGET_SM_CONFIG_SHARED_MEM_SIZE
    Ok(q)
}

/// **El pegado**: en `out`, un MOV por cada `(registro, valor)` de `cargas`
/// y detras el `cuerpo`. Devuelve cuantas instrucciones.
pub fn pegar(out: &mut [(u64, u64)], cargas: &[(u8, u32)], cuerpo: &[(u64, u64)]) -> Result<usize, NoDespacha> {
    if cuerpo.last().is_none_or(|&(lo, _)| lo & 0x1FF != 0x14D) {
        return Err(NoDespacha::Cuerpo);
    }
    let n = cargas.len() + cuerpo.len();
    if n > out.len() {
        return Err(NoDespacha::NoCabe);
    }
    for (k, &(r, v)) in cargas.iter().enumerate() {
        out[k] = mov(r as u64, v);
    }
    out[cargas.len()..n].copy_from_slice(cuerpo);
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motores::sombreador::leer_campo;
    use crate::sass::juez::{juzgar_drenado, Contexto, RESERVADOS};

    fn d() -> Despacho {
        Despacho { grupos: [4, 3, 2], hilos: [16, 4, 2], compartida: 2048, registros: 40 }
    }

    #[test]
    fn el_qmd_dice_la_rejilla_el_grupo_y_la_compartida() {
        let q = qmd(0x2_0000_0000, &d(), 0x2_0000_1040, 0x3060_C0C0).unwrap();
        let f = |hi, lo| leer_campo(&q, hi, lo);
        assert_eq!((f(415, 384), f(431, 416), f(463, 448)), (4, 3, 2));
        assert_eq!((f(607, 592), f(623, 608), f(639, 624)), (16, 4, 2));
        assert_eq!(f(561, 544), 2048);
        assert_eq!((f(567, 562), f(574, 569), f(662, 657)), (3, 3, 3), "2 KiB + 1: la de 8 KiB");
        assert_eq!(f(656, 648), 40);
        assert_eq!(f(1567, 1536) | f(1584, 1568) << 32, 0x2_0000_0000);
    }

    #[test]
    fn la_compartida_elige_su_configuracion() {
        assert_eq!(config_compartida(0), 3);
        assert_eq!(config_compartida(7 * 1024), 3);
        assert_eq!(config_compartida(7 * 1024 + 1), 5, "con su KiB de reserva ya no cabe en 8");
        assert_eq!(config_compartida(32 * 1024), 17, "32 KiB + 1: la de 64");
        let mut x = d();
        x.compartida = 300;
        assert_eq!(leer_campo(&qmd(0, &x, 0, 0).unwrap(), 561, 544), 512, "a 256 B");
    }

    #[test]
    fn los_techos_dicen_que_no() {
        let no = |f: &dyn Fn(&mut Despacho)| {
            let mut x = d();
            f(&mut x);
            qmd(0, &x, 0, 0).unwrap_err()
        };
        assert_eq!(no(&|x| x.hilos = [1024, 2, 1]), NoDespacha::Hilos);
        assert_eq!(no(&|x| x.hilos = [1, 1, 65]), NoDespacha::Hilos);
        assert_eq!(no(&|x| x.grupos = [0, 1, 1]), NoDespacha::Grupos);
        assert_eq!(no(&|x| x.grupos = [1, 65536, 1]), NoDespacha::Grupos);
        assert_eq!(no(&|x| x.compartida = 32 * 1024 + 4), NoDespacha::Compartida);
        assert_eq!(no(&|x| x.registros = 0), NoDespacha::Registros);
        assert!(!NoDespacha::Grupos.texto().is_empty());
    }

    /// El pegado: los MOV y el cuerpo, y el juez lo da por bueno (el cuerpo
    /// lee lo cargado justo despues del ultimo MOV: sus 6 ciclos bastan).
    #[test]
    fn el_pegado_carga_y_el_juez_lo_deja() {
        use bmo_sm86::codifica::{self as c, Fuente};
        // R4 = R2 + R3, y EXIT (con su control, el de ALU).
        let cuerpo = [c::iadd3(4, 2, c::r(3), c::ALU), c::exit(c::ALU)];
        let mut out = [(0, 0); 8];
        let n = pegar(&mut out, &[(2, 7), (3, 0x1000)], &cuerpo).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out[0].0 >> 32, 7);
        juzgar_drenado(&out[..n], &Contexto { registros: 8 + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("{b}"));
        assert_eq!(pegar(&mut out[..3], &[(2, 7), (3, 1)], &cuerpo), Err(NoDespacha::NoCabe));
        assert_eq!(pegar(&mut out, &[], &cuerpo[..1]), Err(NoDespacha::Cuerpo));
    }
}
