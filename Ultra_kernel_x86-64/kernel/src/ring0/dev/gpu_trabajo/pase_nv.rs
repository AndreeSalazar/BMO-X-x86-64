//! **EL MOTOR DEL PASE PARA LA 3060** (P1, 2026-09-25) -- lo que el pase
//! neutro (`dev/pase_gpu.rs`) le pregunta a una GPU, contestado por la GA106.
//!
//! [carril]  ROJO      presta el lienzo de un proceso a la 3060 (SOLO LECTURA) y
//!                     escribe sus tablas de pagina
//! [consumo] NADA      corre al abrir y al cerrar un pase; nada por fotograma
//!
//! # *** El unico sitio donde el pase sabe que es una NVIDIA
//!
//! `dev/pase_gpu.rs` no nombra ni un modulo de aqui (regla N del guardian
//! `la-3060`). Una GPU nueva trae su propio `Motor` en su carpeta y la puerta
//! lo elige; este fichero ni se entera.
//!
//! # El lienzo va donde iba el del volcador
//!
//! La misma IOVA y las mismas tablas (`bmo_gpu_ga10x::volcado`): el pase ES el
//! volcador armado con el buzon en vez de un syscall por caja. Por eso los
//! dos se excluyen: si uno presta, el otro dice "ocupado" -- y el volcado de
//! UNA vez tambien, que presta y devuelve la misma IOVA.

use bmo_gpu_ga10x::volcado as vl;
use bmo_pase_gpu::buzon::Medidas;

use super::pantalla::la_pantalla;
use super::PAGINA;
use crate::ring0::dev::gpu_prestamo::Bar0;
use crate::ring0::dev::pase_gpu::{Aparato, Motor};
use crate::ring0::plat::iommu as io;

pub struct Nv;

static MOTOR: Nv = Nv;

/// **El motor de la 3060, si hay una 3060 con la que hablar.** Lo pide la
/// puerta (`syscall/op_maquina.rs`), que no mira registros: lo mira esto.
pub fn motor() -> Option<&'static dyn Motor> {
    (crate::ring0::dev::gpu::bar0() != 0).then_some(&MOTOR as &'static dyn Motor)
}

impl Motor for Nv {
    fn nombre(&self) -> &'static str {
        "  ...prestado a la 3060 (GA106, motor de copia de L1d3); KiB"
    }

    fn aparato(&self) -> Aparato {
        let g = io::info_gpu();
        Aparato {
            apagado: crate::ring0::dev::gpu_apagar::despedido(),
            iommu_traducida: g & io::IOMMU_GPU_TRADUCIDA != 0 && g & io::IOMMU_GPU_RELEIDA != 0,
            pantalla_alcanzable: crate::ring0::dev::gpu_despertar::bar1_fisica(),
            copiador: crate::ring0::dev::gpu::bar0() != 0 && crate::ring0::dev::gpu_libos::timbre_de_copia().is_some(),
            vblank_armado: crate::ring0::dev::vblank::activo(),
            ocupado: super::volcado::armado(),
        }
    }

    fn medidas(&self) -> Option<(Medidas, u64)> {
        let p = la_pantalla()?;
        vl::cabe(&p).then_some((Medidas { ancho: p.ancho, alto: p.alto }, p.bytes()))
    }

    fn prestar(&self, fisica: u64, bytes: u64) -> bool {
        let (bar0, Some(p)) = (crate::ring0::dev::gpu::bar0(), la_pantalla()) else { return false };
        if bar0 == 0 || p.bytes() != bytes || fisica % PAGINA != 0 {
            return false;
        }
        let mut r = Bar0(bar0);
        if super::volcado::asegurar_mapas(&mut r, &p).is_err() {
            return false;
        }
        let paginas = bytes.div_ceil(PAGINA);
        if io::prestar_gpu(vl::IOVA, fisica, paginas, false).is_err() {
            crate::ring0::cabina::warn("gpu", "pase: el lienzo no se pudo prestar a la 3060; fisica", fisica);
            return false;
        }
        if !vl::invalidar_mmu(&mut r) {
            let _ = io::devolver_gpu(vl::IOVA, paginas);
            crate::ring0::cabina::warn("gpu", "pase: la MMU de la 3060 no confirmo la invalidacion; lienzo devuelto", 0);
            return false;
        }
        true
    }

    fn devolver(&self, bytes: u64) {
        let paginas = bytes.div_ceil(PAGINA);
        if io::devolver_gpu(vl::IOVA, paginas).is_err() {
            crate::ring0::cabina::warn("gpu", "pase: el lienzo NO se pudo devolver (la invalidacion no contesto)", paginas);
        }
    }
}
