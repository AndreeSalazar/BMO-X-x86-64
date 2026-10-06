//! **El plano de STENCIL de la casa** (05-10): donde vive el byte de stencil
//! de cada texel de un D24_UNORM_S8_UINT o un D32_FLOAT_S8X24_UINT (y sus
//! familias TYPELESS), como se limpia y de donde sale la referencia.
//!
//! [carril]  VERDE     memoria de la textura y el estado de la lista
//! [cuesta]  DATO      un plano mal puesto pisa la profundidad de al lado
//! [riesgo]  ESPEJO    las reglas son las de D3D12; el juez es `stencil.exe`
//! [consumo] MEMORIA   un byte mas por texel en los formatos con stencil
//!
//! ```text
//!    lo interno   la profundidad, 4 bytes por texel (un float), como antes;
//!                 DETRAS, el plano de stencil: un byte por texel de cada
//!                 subrecurso, en el mismo orden (el del subrecurso s
//!                 empieza en `total + s.desde / 4`)
//!    limpiar      ClearDepthStencilView con CLEAR_FLAG_STENCIL (y el
//!                 BeginningAccess CLEAR de BeginRenderPass): YA, al
//!                 ejecutarse (la Z se apunta para quien dibuje; el plano
//!                 no lo usa la 3060)
//!    referencia   OMSetStencilRef (las dos caras) y OMSetFrontAndBackStencilRef
//!                 (Lista8, una cada una); se guardan en el estado de la
//!                 lista y cada Draw las lleva consigo
//! ```
//!
//! La prueba y las operaciones son de `bmo_proton_x::stencil` (puro) y las
//! hace la trama. Desde el 05-10 el plano se COPIA (CopyTextureRegion de
//! los subrecursos del PLANO 1: los de D3D12, `mips * capas` mas alla de
//! los de la profundidad, con su huella de un byte por texel, R8_TYPELESS)
//! y se LEE con un SRV X24_TYPELESS_G8_UINT o X32_TYPELESS_G8X24_UINT (el
//! stencil en G, como entero: `textura::Como::Stencil8`). Juez:
//! `prueba/restos.exe` (D y E).

use crate::aviso;
use crate::subrecursos::Forma;

/// Los formatos con plano de stencil: R32G8X24_TYPELESS, D32_FLOAT_S8X24_UINT,
/// R32_FLOAT_X8X24_TYPELESS y X32_TYPELESS_G8X24_UINT (19-22); R24G8_TYPELESS,
/// D24_UNORM_S8_UINT, R24_UNORM_X8_TYPELESS y X24_TYPELESS_G8_UINT (44-47).
pub(crate) fn con_stencil(formato: u32) -> bool {
    matches!(formato, 19..=22 | 44..=47)
}

/// Los bytes que una textura pide DETRAS de lo interno (`total`) para su
/// plano: la cuarta parte (lo interno de una profundidad son 4 bytes por
/// texel), o nada si su formato no tiene stencil. A palabras enteras
/// (05-10): el SRV del plano lo lee de cuatro en cuatro bytes.
pub(crate) fn bytes_de_mas(forma: &Forma, total: u64) -> u64 {
    if con_stencil(forma.formato) {
        (total / 4).div_ceil(4) * 4
    } else {
        0
    }
}

/// **El plano de stencil del subrecurso `sub`** de un recurso (el de una
/// vista DSV: el subrecurso, y la rebanada << 32), con sus medidas. `None`:
/// no es una textura con stencil, o no tiene ese subrecurso.
pub(crate) fn plano(recurso: u64, sub: u64) -> Option<(&'static mut [u8], u32, u32)> {
    let t = crate::d3d12_vistas::tex(recurso)?;
    if !con_stencil(t.forma.formato) || t.almacen.elemento() != (4, 1) {
        return None;
    }
    let total = t.subs.last().map(|s| s.desde + s.bytes())?;
    let s = *t.subs.get(sub as u32 as usize)?;
    let rebanada = (sub >> 32) as u32;
    if rebanada >= s.hondo {
        return None;
    }
    let n = s.ancho as usize * s.alto as usize;
    let p = (t.datos + total + (s.desde + rebanada as u64 * s.fila * s.filas as u64) / 4) as *mut u8;
    // SAFETY: `recurso_forma` pidio `total + total / 4` bytes para esta
    // textura (`bytes_de_mas`), y el subrecurso cae dentro de su cuarta parte.
    Some((unsafe { core::slice::from_raw_parts_mut(p, n) }, s.ancho, s.alto))
}

/// R8_TYPELESS: la huella de un subrecurso del plano 1 (un byte por texel).
pub(crate) const HUELLA_PLANO: u32 = 60;

/// **Copiar la `caja` del subrecurso `sub` del PLANO 1** (05-10; `sub`
/// cuenta desde el primero del plano) con la memoria `l` de una huella de
/// un byte por texel: `hacia`, de la huella al plano (subirlo); si no, leerlo.
///
/// # Safety
/// `l.p` vale para `l.bytes` bytes (leer, y escribir si no es `hacia`).
pub(crate) unsafe fn mover_plano(recurso: u64, sub: u32, caja: [u32; 6], l: &crate::d3d12_texturas::Lineal, hacia: bool) -> Result<(), &'static str> {
    let (s, w, h) = plano(recurso, sub as u64).ok_or("CopyTextureRegion de un plano de stencil que la textura no tiene")?;
    let [x0, y0, z0, x1, y1, z1] = caja;
    if x0 >= x1 || y0 >= y1 || x1 > w || y1 > h || (z0, z1) != (0, 1) {
        return Err("una caja que sale del plano de stencil");
    }
    let n = (x1 - x0) as usize;
    if (y1 - y0 - 1) as u64 * l.fila + n as u64 > l.bytes {
        return Err("la memoria de la copia es mas corta que la caja");
    }
    for y in y0..y1 {
        let fila = &mut s[(y * w + x0) as usize..][..n];
        let lin = l.p.add(((y - y0) as u64 * l.fila) as usize);
        if hacia {
            core::ptr::copy_nonoverlapping(lin, fila.as_mut_ptr(), n);
        } else {
            core::ptr::copy_nonoverlapping(fila.as_ptr(), lin, n);
        }
    }
    Ok(())
}

/// **La textura de un SRV del plano de stencil** (05-10: X24_TYPELESS_G8_UINT
/// o X32_TYPELESS_G8X24_UINT, el PlaneSlice 1): el plano del subrecurso 0,
/// un byte por texel. De una textura con mips o capas, todavia no.
pub(crate) fn textura(recurso: u64, mapeo: u32) -> Result<bmo_proton_x::textura::Textura<'static>, &'static str> {
    use bmo_proton_x::textura::{Clase, Como, Textura};
    let t = crate::d3d12_vistas::tex(recurso).ok_or("un SRV de stencil sobre un bufer: en Windows es un error (se lee como nulo)")?;
    if t.forma.subrecursos() != 1 {
        return Err("un SRV del plano de stencil de una textura con mips o capas: todavia no (se lee como nulo)");
    }
    let (s, ancho, alto) = plano(recurso, 0).ok_or("un SRV de stencil sobre una textura sin plano de stencil (se lee como nulo)")?;
    // SAFETY: el plano empieza en palabra (lo interno son palabras) y
    // `bytes_de_mas` lo pidio a palabras enteras.
    let texeles = unsafe { core::slice::from_raw_parts(s.as_ptr() as *const u32, s.len().div_ceil(4)) };
    Ok(Textura { texeles, ancho, alto, como: Como::Stencil8, srgb: false, mapeo, mips: 1, capas: 1, hondo: 1, clase: Clase::Plana, mip: 0, capa: 0, niveles: u32::MAX, lod_min: 0.0, vista: None })
}

/// **Limpiar el plano** (`Orden::LimpiarStencil`, al ejecutar la lista).
pub(crate) fn limpiar(recurso: u64, sub: u64, valor: u8) {
    match plano(recurso, sub) {
        Some((s, _, _)) => s.fill(valor),
        None => aviso("ClearDepthStencilView con CLEAR_FLAG_STENCIL sobre un DSV sin stencil (un D32, o un subrecurso que no tiene): no hay plano que limpiar"),
    }
}

/// **Un `D3D12_DEPTH_STENCIL_DESC2`** (60 B, el subobjeto 26 de un flujo:
/// las mascaras van en cada cara, `D3D12_DEPTH_STENCILOP_DESC1` de 20 B en
/// +16 y +36) al `D3D12_DEPTH_STENCIL_DESC` de siempre (52 B). El PSO de la
/// casa lleva UNA mascara de lectura y una de escritura: si las caras las
/// tienen distintas, se dice y el PSO no se crea.
pub(crate) fn desc2_a_desc(q: &[u8]) -> Result<[u8; 52], &'static str> {
    let mut d = [0u8; 52];
    d[..16].copy_from_slice(&q[..16]);
    d[20..36].copy_from_slice(&q[16..32]);
    d[36..52].copy_from_slice(&q[36..52]);
    let encendido = q[12..16] != [0; 4];
    if encendido && q[32..34] != q[52..54] {
        return Err("CreatePipelineState con mascaras de stencil distintas en cada cara (DEPTH_STENCIL2): todavia no");
    }
    d[16..18].copy_from_slice(&q[32..34]);
    Ok(d)
}

/// `OMSetStencilRef(this, ref)`: la de las dos caras. D3D12 la usa en 8 bits.
pub(crate) extern "win64" fn om_set_stencil_ref(this: u64, r: u32) {
    om_set_front_and_back_stencil_ref(this, r, r);
}

/// `ID3D12GraphicsCommandList8::OMSetFrontAndBackStencilRef(this, delante,
/// detras)`: una referencia a cada cara.
pub(crate) extern "win64" fn om_set_front_and_back_stencil_ref(this: u64, delante: u32, detras: u32) {
    // SAFETY: `this` es una Lista de la casa.
    unsafe { crate::d3d12::lista(this) }.estado.stencil_ref = [delante as u8, detras as u8];
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_formatos_con_stencil_y_lo_que_piden_de_mas() {
        for f in [19, 20, 21, 22, 44, 45, 46, 47] {
            assert!(con_stencil(f), "{f}");
        }
        // D32_FLOAT, D16 y R32_FLOAT no tienen.
        for f in [40, 41, 55, 0, 28] {
            assert!(!con_stencil(f), "{f}");
        }
        // 64 x 64 de D24S8: 16 KiB de profundidad y 4 KiB de plano.
        assert_eq!(bytes_de_mas(&Forma::plana(64, 64, 45), 64 * 64 * 4), 64 * 64);
        assert_eq!(bytes_de_mas(&Forma::plana(64, 64, 40), 64 * 64 * 4), 0);
    }

    #[test]
    fn el_desc2_pasa_al_de_siempre_con_sus_mascaras() {
        let mut q = [0u8; 60];
        let mut u = |o: usize, v: u32| q[o..o + 4].copy_from_slice(&v.to_le_bytes());
        // Z encendida, LESS; stencil; delante REPLACE/EQUAL, detras INCR/ALWAYS.
        for (o, v) in [(0, 1), (4, 1), (8, 2), (12, 1), (16, 1), (20, 1), (24, 3), (28, 3), (36, 1), (40, 1), (44, 7), (48, 8)] {
            u(o, v);
        }
        q[32] = 0x0F;
        q[33] = 0xF0;
        q[52] = 0x0F;
        q[53] = 0xF0;
        let d = desc2_a_desc(&q).unwrap();
        let s = bmo_proton_x::stencil::Stencil::de_desc(&d).unwrap().unwrap();
        assert_eq!((s.delante.pasa, s.delante.funcion, s.detras.pasa, s.detras.funcion), (3, 3, 7, 8));
        assert_eq!((s.delante.lectura, s.delante.escritura, s.detras.escritura), (0x0F, 0xF0, 0xF0));
        assert_eq!(d[..12], q[..12]);
        // Mascaras distintas por cara: se dice.
        q[53] = 0xFF;
        assert!(desc2_a_desc(&q).is_err());
    }
}
