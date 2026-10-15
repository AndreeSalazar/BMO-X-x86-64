//! **LA LAMINA PRESTADA** (C1 de `docs/plan/EL_FOCO.md`, S1 de
//! `docs/plan/PLAN_VERRANO.md`, 15-10): un VRN1 cuyos vertices NO viajan en
//! el paquete. Van 16 B -- donde esta la ranura de la lamina en el escritorio
//! y cuanto mide -- y el kernel la PRESTA a la 3060 solo lectura mientras
//! dibuja. Salio de `tuberia.rs` por L6a: lo de C1, junto.

use super::tuberia::{cerrar_paquete_1, Dibujo, BYTES_VERTICE, CABECERA_1};

/// ** C1 (S1 de PLAN_VERRANO, 15-10): bit 10 del estado (+36) de un VRN1 --
/// los vertices NO van en el paquete: los DATOS son un descriptor de 16 B
/// (`va` de la ranura de la lamina en el escritorio, `bytes`), y el kernel la
/// PRESTA a la 3060 solo lectura mientras dibuja (`Prestada`). Un kernel de
/// antes rechaza el bit (todo bit desde el 10 era un NO): no lo malinterpreta.
pub const ESTADO_PRESTADA: u32 = 1 << 10;
/// Lo que mide el descriptor de una lamina prestada.
pub const DESCRIPTOR_PRESTADA: usize = 16;

/// ** C1: los vertices de una LAMINA PRESTADA a la 3060 (ver
/// [`ESTADO_PRESTADA`]): donde estan en el escritorio, cuanto miden, y la
/// VA donde los ve la 3060 -- 0 hasta que el kernel la presta --.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Prestada {
    pub va_app: u64,
    pub bytes: u64,
    pub gpu_va: u64,
}

/// **El descriptor de un paquete** (`datos`, sus 16 B): la ranura y cuanto
/// mide -- exactamente los `n` vertices de V0 --. `None` si no se sostiene.
pub(super) fn leer_descriptor(datos: &[u8], n: usize) -> Option<Prestada> {
    let palabra = |k: usize| datos.get(k..k + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8])));
    let (va_app, bytes) = (palabra(0)?, palabra(8)?);
    (va_app != 0 && bytes == (n * BYTES_VERTICE) as u64).then_some(Prestada { va_app, bytes, gpu_va: 0 })
}

/// ** C1 (15-10): un VRN1 cuyos vertices son la ranura de una LAMINA que la
/// 3060 leera PRESTADA (la `va_app` del escritorio, `n` vertices de V0): el
/// paquete lleva 16 B de descriptor en vez de `n * 32` de vertices. `None` si
/// no se sostiene (con indices, o `dibujo.vertices` no es `n`).
pub fn escribir_paquete_prestado(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], va_app: u64, n: usize, dibujo: Dibujo) -> Option<usize> {
    let d = CABECERA_1 + vs.len() + ps.len();
    if n == 0 || va_app == 0 || out.len() < d + DESCRIPTOR_PRESTADA || dibujo.indices.is_some() || dibujo.vertices as usize != n {
        return None;
    }
    out[d..d + 8].copy_from_slice(&va_app.to_le_bytes());
    out[d + 8..d + 16].copy_from_slice(&((n * BYTES_VERTICE) as u64).to_le_bytes());
    let dibujo = Dibujo { prestada: Some(Prestada { va_app, bytes: (n * BYTES_VERTICE) as u64, gpu_va: 0 }), ..dibujo };
    cerrar_paquete_1(out, ficha, vs, ps, n, DESCRIPTOR_PRESTADA, dibujo)
}
