//! **Lo que mide un recurso** (tanda 48, 02-10): la cuenta de
//! `GetResourceAllocationInfo` y de si un recurso colocado cabe en su monton.
//!
//! La leccion del cuelgue del inventario de Cyberpunk en vkd3d-proton 2.4: el
//! juego creo un monton con lo que median DOS mips y coloco una textura de
//! SEIS. La cuenta tiene que crecer con las mips, las capas y el formato
//! (monotona), y un recurso que no cabe desde su desplazamiento es
//! E_INVALIDARG, "como los drivers nativos" (vkd3d-proton, commit 72d9b322).
//! Hasta hoy la casa daba 4 bytes por pixel de la mip 0 a todo.
//!
//! ```text
//!    bufer             su ancho, a 64 KiB
//!    textura           la suma de sus mips (cada una la mitad, minimo 1),
//!                      por capas (1D/2D) o por profundidad que tambien se
//!                      parte (3D), por muestras; los BC en bloques de 4x4;
//!                      a 64 KiB, o a 4 MiB con MSAA
//! ```
//!
//! No son las cifras de un driver (la 3060 alinea y rellena a su manera):
//! son las de la CASA, que es quien reserva, y por eso valen para decidir si
//! algo cabe.

/// D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT y la de MSAA.
pub(crate) const ALINEACION: u64 = 0x1_0000;
const ALINEACION_MSAA: u64 = 0x40_0000;

/// D3D12_RESOURCE_DIMENSION.
const BUFER: u32 = 1;
const TEXTURA3D: u32 = 4;

/// **Lo que ocupa un elemento** de un DXGI_FORMAT: `(bytes, lado del bloque)`
/// -- un pixel (lado 1) o un bloque comprimido de 4x4 (lado 4).
pub(crate) fn elemento(formato: u32) -> (u64, u64) {
    match formato {
        1..=4 => (16, 1),
        5..=8 => (12, 1),
        9..=22 => (8, 1),
        23..=47 | 67..=69 | 87..=93 => (4, 1),
        48..=59 | 85 | 86 | 115 => (2, 1),
        60..=65 => (1, 1),
        // BC1 y BC4: 8 bytes por bloque; BC2, BC3, BC5, BC6H y BC7: 16.
        70..=72 | 79..=81 => (8, 4),
        73..=78 | 82..=84 | 94..=99 => (16, 4),
        // Lo que la casa no conoce (video, R1): como uno de 4 bytes.
        _ => (4, 1),
    }
}

/// `(bytes, alineacion)` de un D3D12_RESOURCE_DESC (o DESC1: empieza igual).
/// Dimension +0, Alignment +8, Width +16, Height +24, DepthOrArraySize +28,
/// MipLevels +30, Format +32, SampleDesc.Count +36.
///
/// # Safety
/// `d` son los 56 bytes de un D3D12_RESOURCE_DESC del `.exe`.
pub(crate) unsafe fn medida(d: *const u8) -> (u64, u64) {
    let u32_ = |o: usize| (d.add(o) as *const u32).read_unaligned();
    let u16_ = |o: usize| (d.add(o) as *const u16).read_unaligned() as u64;
    let (dimension, ancho) = (u32_(0), (d.add(16) as *const u64).read_unaligned());
    if dimension == BUFER {
        return (ancho.div_ceil(ALINEACION) * ALINEACION, ALINEACION);
    }
    let (alto, capas, mips, formato, muestras) = (u32_(24).max(1) as u64, u16_(28).max(1), u16_(30), u32_(32), u32_(36).max(1) as u64);
    let ancho = ancho.max(1);
    // MipLevels 0 = la cadena entera, hasta 1x1.
    let mips = if mips == 0 { 64 - ancho.max(alto).leading_zeros() as u64 } else { mips };
    let (bytes_elem, lado) = elemento(formato);
    let es_3d = dimension == TEXTURA3D;
    let mut total = 0u64;
    for m in 0..mips {
        let (w, h) = ((ancho >> m).max(1), (alto >> m).max(1));
        let profundidad = if es_3d { (capas >> m).max(1) } else { 1 };
        total = total.saturating_add(w.div_ceil(lado) * h.div_ceil(lado) * bytes_elem * profundidad);
    }
    if !es_3d {
        total = total.saturating_mul(capas);
    }
    total = total.saturating_mul(muestras);
    let alineacion = if muestras > 1 { ALINEACION_MSAA } else { ALINEACION };
    (total.div_ceil(alineacion) * alineacion, alineacion)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn desc(dimension: u32, ancho: u64, alto: u32, capas: u16, mips: u16, formato: u32) -> [u8; 56] {
        let mut d = [0u8; 56];
        d[0..4].copy_from_slice(&dimension.to_le_bytes());
        d[16..24].copy_from_slice(&ancho.to_le_bytes());
        d[24..28].copy_from_slice(&alto.to_le_bytes());
        d[28..30].copy_from_slice(&capas.to_le_bytes());
        d[30..32].copy_from_slice(&mips.to_le_bytes());
        d[32..36].copy_from_slice(&formato.to_le_bytes());
        d[36..40].copy_from_slice(&1u32.to_le_bytes());
        d
    }

    fn m(d: &[u8; 56]) -> u64 {
        // SAFETY: 56 bytes.
        unsafe { medida(d.as_ptr()) }.0
    }

    #[test]
    fn crece_con_las_mips_las_capas_y_el_formato() {
        // 1024x1024 RGBA8: 4 MiB la mip 0; la cadena entera, un tercio mas.
        let una = m(&desc(3, 1024, 1024, 1, 1, 28));
        assert_eq!(una, 4 << 20);
        let dos = m(&desc(3, 1024, 1024, 1, 2, 28));
        let seis = m(&desc(3, 1024, 1024, 1, 6, 28));
        let todas = m(&desc(3, 1024, 1024, 1, 0, 28));
        assert!(una < dos && dos < seis && seis <= todas, "{una} {dos} {seis} {todas}");
        // El inventario de Cyberpunk: dos mips NO caben donde van seis.
        assert!(dos < seis);
        // BC1 (8 bytes por bloque de 4x4): un octavo de RGBA8.
        assert_eq!(m(&desc(3, 1024, 1024, 1, 1, 71)), 512 << 10);
        // Seis capas, seis veces.
        assert_eq!(m(&desc(3, 1024, 1024, 6, 1, 28)), 6 * una);
        // Un bufer: su ancho, a 64 KiB.
        assert_eq!(m(&desc(1, 100, 1, 1, 1, 0)), 64 << 10);
    }
}
