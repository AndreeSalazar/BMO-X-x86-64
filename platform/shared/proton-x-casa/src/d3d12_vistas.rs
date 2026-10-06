//! **Las vistas de D3D12, todas** (02-10): SRV, RTV, DSV y UAV de cada
//! dimension (bufer, 1D, 2D, 3D, arrays, cubos, multimuestra) leen su
//! descripcion entera y la dejan en la ranura. Hasta hoy, una SRV que no
//! fuera TEXTURE2D se quedaba sin escribir (Cyberpunk crea cubos, arrays y
//! texturas 3D al montar su D3D12), y una RTV o DSV miraba el recurso y ya.
//!
//! ```text
//!    la ranura (32 B)   0 el recurso   1 la marca (SRV, RTV, DSV, UAV)
//!                       2 dimension | formato << 8 | mapeo << 24
//!                         (y en un SRV de bufer, N5.3: | paso << 40 |
//!                         crudo << 56; en uno de textura, D4.4: | sus
//!                         MipLevels << 40, 8 bits, 0 todas)
//!                       3 el subrecurso | rebanada 3D << 32 (o, en un
//!                         bufer, el primer elemento | elementos << 32; en
//!                         un SRV de textura, D4.4: su ResourceMinLODClamp,
//!                         el float, << 32)
//!    el subrecurso      mip + capa * mips, de la textura del recurso (en un
//!                       cubo, cada cara es una capa; en 3D, la mip)
//! ```
//!
//! Lo que hace el sombreador con cada una (arrays, cubos, 3D, mips, Load,
//! GetDimensions) esta en `bmo_proton_x::textura` (`Textura::muestrear_en`).

use crate::d3d12::{Recurso, Tex, DESC_SRV, DESC_UAV};
use crate::{aviso, com::de};

/// Marcas de la palabra 1 que no son de lectura (ver `d3d12::DESC_SRV`).
pub(crate) const DESC_RTV: u64 = 5;
pub(crate) const DESC_DSV: u64 = 6;

/// **Una vista, leida**: lo que vale para todas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Vista {
    /// La D3D12_*_DIMENSION de su clase (cada clase numera las suyas).
    pub dimension: u32,
    /// El DXGI_FORMAT de la vista (0: el del recurso).
    pub formato: u32,
    /// Shader4ComponentMapping (solo SRV; las demas, el de siempre).
    pub mapeo: u32,
    pub mip: u32,
    /// La primera capa (array, cara de cubo) o, en un bufer, nada.
    pub capa: u32,
    /// La primera rebanada de una RTV/UAV 3D.
    pub rebanada: u32,
    /// El primer elemento de una vista de bufer.
    pub elemento: u64,
    /// N5.3, un SRV de bufer: sus elementos, el paso de uno estructurado
    /// (StructureByteStride, hasta 2048) y si es crudo (D3D12_BUFFER_SRV_FLAG_RAW).
    pub elementos: u32,
    pub paso: u32,
    pub crudo: bool,
    /// D4.4, un SRV de textura: sus MipLevels (0 todas: el -1 de D3D, o mas
    /// de 255) y su ResourceMinLODClamp (el float, en bits).
    pub niveles: u32,
    pub lod_min: u32,
}

/// **Un SRV de bufer, leido de su ranura** (N5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VistaBufer {
    pub formato: u32,
    pub primero: u64,
    pub elementos: u32,
    pub paso: u32,
    pub crudo: bool,
}

/// Lo que guarda la ranura de un SRV de bufer (ver la cabecera).
pub(crate) fn leer_bufer(ranura: &[u64]) -> VistaBufer {
    let (w2, w3) = (ranura[2], ranura[3]);
    VistaBufer { formato: (w2 >> 8) as u32 & 0xFFFF, primero: w3 & 0xFFFF_FFFF, elementos: (w3 >> 32) as u32, paso: (w2 >> 40) as u32 & 0xFFF, crudo: (w2 >> 56) & 1 != 0 }
}

/// D3D12_SRV_DIMENSION: BUFFER 1, TEXTURE1D 2, 1DARRAY 3, 2D 4, 2DARRAY 5,
/// 2DMS 6, 2DMSARRAY 7, 3D 8, CUBE 9, CUBEARRAY 10, RAYTRACING 11.
pub(crate) const SRV_BUFER: u32 = 1;
pub(crate) const SRV_CUBO: u32 = 9;
pub(crate) const SRV_ACELERACION: u32 = 11;

const MAPEO: u32 = 0x1688;

fn u32_(p: *const u8, o: usize) -> u32 {
    // SAFETY: lo garantiza quien llama: la descripcion del `.exe`.
    unsafe { (p.add(o) as *const u32).read_unaligned() }
}

fn u64_(p: *const u8, o: usize) -> u64 {
    // SAFETY: como arriba.
    unsafe { (p.add(o) as *const u64).read_unaligned() }
}

/// **Un D3D12_SHADER_RESOURCE_VIEW_DESC**: Format +0, ViewDimension +4,
/// Shader4ComponentMapping +8, y la union desde +16 (MostDetailedMip
/// primero en las de textura; FirstArraySlice +24 en las de array, +16 en
/// 2DMSARRAY; First2DArrayFace +24 en CUBEARRAY; FirstElement +16 en un
/// bufer).
///
/// # Safety
/// `d` son los 40 bytes de la descripcion del `.exe`.
pub(crate) unsafe fn srv(d: *const u8) -> Result<Vista, &'static str> {
    let (formato, dimension, mapeo) = (u32_(d, 0), u32_(d, 4), u32_(d, 8));
    let mut v = Vista { dimension, formato, mapeo, ..Vista::default() };
    match dimension {
        // D3D12_BUFFER_SRV: FirstElement +16, NumElements +24,
        // StructureByteStride +28, Flags +32 (RAW = 1).
        SRV_BUFER => (v.elemento, v.elementos, v.paso, v.crudo) = (u64_(d, 16), u32_(d, 24), u32_(d, 28), u32_(d, 32) & 1 != 0),
        2 | 4 | 8 | SRV_CUBO => v.mip = u32_(d, 16),
        3 | 5 | 10 => (v.mip, v.capa) = (u32_(d, 16), u32_(d, 24)),
        6 => {}
        7 => v.capa = u32_(d, 16),
        SRV_ACELERACION => v.elemento = u64_(d, 16),
        _ => return Err("CreateShaderResourceView con una dimension que no es de D3D12"),
    }
    // D4.4: MipLevels (+20 en todas las de textura con mips) y
    // ResourceMinLODClamp, el ultimo de cada una: +24 en 1D, 3D y cubo; +28
    // en 2D; +32 en 1DARRAY y CUBEARRAY; +36 en 2DARRAY.
    let clamp = match dimension {
        2 | 8 | SRV_CUBO => Some(24),
        4 => Some(28),
        3 | 10 => Some(32),
        5 => Some(36),
        _ => None,
    };
    if let Some(o) = clamp {
        let n = u32_(d, 20);
        (v.niveles, v.lod_min) = (if n > 255 { 0 } else { n }, u32_(d, o));
    }
    Ok(v)
}

/// **Un D3D12_RENDER_TARGET_VIEW_DESC**: Format +0, ViewDimension +4 (BUFFER
/// 1, 1D 2, 1DARRAY 3, 2D 4, 2DARRAY 5, 2DMS 6, 2DMSARRAY 7, 3D 8) y la
/// union desde +8 (MipSlice primero; FirstArraySlice o FirstWSlice +12;
/// 2DMSARRAY: FirstArraySlice +8).
///
/// # Safety
/// `d` son los 32 bytes de la descripcion del `.exe`.
pub(crate) unsafe fn rtv(d: *const u8) -> Result<Vista, &'static str> {
    let (formato, dimension) = (u32_(d, 0), u32_(d, 4));
    let mut v = Vista { dimension, formato, mapeo: MAPEO, ..Vista::default() };
    match dimension {
        1 => v.elemento = u64_(d, 8),
        2 | 4 => v.mip = u32_(d, 8),
        3 | 5 => (v.mip, v.capa) = (u32_(d, 8), u32_(d, 12)),
        6 => {}
        7 => v.capa = u32_(d, 8),
        8 => (v.mip, v.rebanada) = (u32_(d, 8), u32_(d, 12)),
        _ => return Err("CreateRenderTargetView con una dimension que no es de D3D12"),
    }
    Ok(v)
}

/// **Un D3D12_DEPTH_STENCIL_VIEW_DESC**: Format +0, ViewDimension +4 (1D 1,
/// 1DARRAY 2, 2D 3, 2DARRAY 4, 2DMS 5, 2DMSARRAY 6), Flags +8, y la union
/// desde +12 (MipSlice; FirstArraySlice +16; 2DMSARRAY: FirstArraySlice +12).
///
/// # Safety
/// `d` son los 24 bytes de la descripcion del `.exe`.
pub(crate) unsafe fn dsv(d: *const u8) -> Result<Vista, &'static str> {
    let (formato, dimension) = (u32_(d, 0), u32_(d, 4));
    let mut v = Vista { dimension, formato, mapeo: MAPEO, ..Vista::default() };
    match dimension {
        1 | 3 => v.mip = u32_(d, 12),
        2 | 4 => (v.mip, v.capa) = (u32_(d, 12), u32_(d, 16)),
        5 => {}
        6 => v.capa = u32_(d, 12),
        _ => return Err("CreateDepthStencilView con una dimension que no es de D3D12"),
    }
    Ok(v)
}

/// **Un D3D12_UNORDERED_ACCESS_VIEW_DESC**: Format +0, ViewDimension +4
/// (BUFFER 1, 1D 2, 1DARRAY 3, 2D 4, 2DARRAY 5, 2DMS 6, 2DMSARRAY 7, 3D 8)
/// y la union desde +8 (FirstElement de un bufer; MipSlice; FirstArraySlice
/// o FirstWSlice +12).
///
/// # Safety
/// `d` son los 40 bytes de la descripcion del `.exe`.
pub(crate) unsafe fn uav(d: *const u8) -> Result<Vista, &'static str> {
    let (formato, dimension) = (u32_(d, 0), u32_(d, 4));
    let mut v = Vista { dimension, formato, mapeo: MAPEO, ..Vista::default() };
    match dimension {
        // D3D12_BUFFER_UAV (N5.5, 05-10): FirstElement, NumElements,
        // StructureByteStride, CounterOffsetInBytes y Flags (RAW = 1), como el
        // SRV de bufer: el computo lo escribe.
        1 => (v.elemento, v.elementos, v.paso, v.crudo) = (u64_(d, 8), u32_(d, 16), u32_(d, 20), u32_(d, 32) & 1 != 0),
        2 | 4 => v.mip = u32_(d, 8),
        3 | 5 => (v.mip, v.capa) = (u32_(d, 8), u32_(d, 12)),
        6 => {}
        7 => v.capa = u32_(d, 8),
        8 => (v.mip, v.rebanada) = (u32_(d, 8), u32_(d, 12)),
        _ => return Err("CreateUnorderedAccessView con una dimension que no es de D3D12"),
    }
    Ok(v)
}

/// El subrecurso de `(mip, capa)` en `t`: `mip + capa * mips`, dentro.
pub(crate) fn sub(t: &Tex, mip: u32, capa: u32) -> u32 {
    let f = &t.forma;
    mip.min(f.mips - 1) + capa.min(f.capas() - 1) * f.mips
}

/// **Escribir una vista en su ranura** (la de `handle`): el recurso, la
/// marca y la vista (ver la cabecera). Con la vista, el subrecurso que
/// toca, si el recurso es una textura.
pub(crate) fn poner(handle: u64, recurso: u64, marca: u64, v: &Vista) {
    let w2 = v.dimension as u64 & 0xFF | (v.formato as u64 & 0xFFFF) << 8 | (v.mapeo as u64 & 0xFFFF) << 24 | (v.paso as u64 & 0xFFF) << 40 | (v.crudo as u64) << 56 | (v.niveles as u64 & 0xFF) << 40;
    let w3 = match tex(recurso) {
        // D4.4: un SRV no tiene rebanada: ahi va su ResourceMinLODClamp.
        Some(t) => sub(t, v.mip, v.capa) as u64 | (if marca == DESC_SRV { v.lod_min } else { v.rebanada } as u64) << 32,
        // Un bufer: el primer elemento en 32 bits (4 mil millones de
        // elementos bastan) y cuantos detras.
        None => v.elemento & 0xFFFF_FFFF | (v.elementos as u64) << 32,
    };
    // SAFETY: la ranura de un monton de la casa: 4 palabras.
    unsafe {
        let r = handle as *mut u64;
        r.write(recurso);
        r.add(1).write(marca);
        r.add(2).write(w2);
        r.add(3).write(w3);
    }
}

/// Lo que guarda una ranura: `(dimension, formato, mapeo)` y `(subrecurso,
/// rebanada)`.
pub(crate) fn leer(ranura: &[u64]) -> ((u32, u32, u32), (u32, u32)) {
    let (w2, w3) = (ranura[2], ranura[3]);
    ((w2 as u32 & 0xFF, (w2 >> 8) as u32 & 0xFFFF, (w2 >> 24) as u32 & 0xFFFF), (w3 as u32, (w3 >> 32) as u32))
}

/// D4.4: lo de las mips de un SRV de textura: sus MipLevels (`u32::MAX`,
/// todas) y su ResourceMinLODClamp.
pub(crate) fn mips_de(ranura: &[u64]) -> (u32, f32) {
    let n = ((ranura[2] >> 40) & 0xFF) as u32;
    (if n == 0 { u32::MAX } else { n }, f32::from_bits((ranura[3] >> 32) as u32))
}

/// La textura de un recurso de la casa (o `None`: un bufer, o nulo).
pub(crate) fn tex(recurso: u64) -> Option<&'static Tex> {
    if recurso == 0 {
        return None;
    }
    // SAFETY: lo que llega a una vista son Recursos de la casa.
    unsafe { de::<Recurso>(recurso) }.tex.as_ref()
}

/// `CreateShaderResourceView(this, recurso, desc, handle)`: cualquier
/// dimension. Sin descripcion, la vista de todo el recurso (su formato, su
/// mip mas detallada, su primera capa).
pub(crate) extern "win64" fn create_shader_resource_view(_this: u64, recurso: u64, desc: *const u8, handle: u64) {
    if handle == 0 {
        return;
    }
    let v = if desc.is_null() {
        if recurso == 0 {
            aviso("CreateShaderResourceView nulo sin descripcion: en Windows es un error");
            return;
        }
        let dimension = match tex(recurso) {
            None => SRV_BUFER,
            Some(t) => match (t.forma.dimension, t.forma.capas() > 1) {
                (crate::subrecursos::DIM_TEXTURA1D, false) => 2,
                (crate::subrecursos::DIM_TEXTURA1D, true) => 3,
                (crate::subrecursos::DIM_TEXTURA3D, _) => 8,
                (_, false) => 4,
                (_, true) => 5,
            },
        };
        Vista { dimension, mapeo: MAPEO, ..Vista::default() }
    } else {
        // SAFETY: la descripcion del `.exe`.
        match unsafe { srv(desc) } {
            Ok(v) => v,
            Err(m) => {
                aviso(m);
                return;
            }
        }
    };
    if v.dimension == SRV_ACELERACION {
        aviso("CreateShaderResourceView de una estructura de aceleracion (trazado de rayos): la casa no traza rayos");
    }
    poner(handle, recurso, DESC_SRV, &v);
}

/// `CreateRenderTargetView(this, recurso, desc, handle)`.
pub(crate) extern "win64" fn create_render_target_view(_this: u64, recurso: u64, desc: *const u8, handle: u64) {
    vista_de_destino(recurso, desc, handle, DESC_RTV, rtv);
}

/// `CreateDepthStencilView(this, recurso, desc, handle)`.
pub(crate) extern "win64" fn create_depth_stencil_view(_this: u64, recurso: u64, desc: *const u8, handle: u64) {
    vista_de_destino(recurso, desc, handle, DESC_DSV, dsv);
}

/// `CreateUnorderedAccessView(this, recurso, contador, desc, handle)`. Con
/// un `contador` (E2.4, 05-10: el de `Append`/`Consume`), su numero en la
/// ranura (ver [`contador_de`]).
pub(crate) extern "win64" fn create_unordered_access_view(_this: u64, recurso: u64, contador: u64, desc: *const u8, handle: u64) {
    if contador == 0 || desc.is_null() || handle == 0 {
        vista_de_destino(recurso, desc, handle, DESC_UAV, uav);
        return;
    }
    // SAFETY: la descripcion del `.exe`.
    let mut v = match unsafe { uav(desc) } {
        Ok(v) => v,
        Err(m) => {
            aviso(m);
            return;
        }
    };
    // D3D12_BUFFER_UAV.CounterOffsetInBytes (+24 de la descripcion).
    v.mapeo = contador_de(contador, u64_(desc, 24));
    poner(handle, recurso, DESC_UAV, &v);
}

/// **Los contadores de los UAV** (E2.4, 05-10): el recurso y el
/// desplazamiento de cada `pCounterResource` de CreateUnorderedAccessView.
/// La ranura (32 B) ya esta llena: guarda su NUMERO en los 16 bits del
/// mapeo, que un UAV no usa, y asi viaja con ella cuando se copia
/// (CopyDescriptors). El 0 es "sin contador".
struct Contadores(core::cell::UnsafeCell<alloc::vec::Vec<(u64, u64)>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Contadores {}
static CONTADORES: Contadores = Contadores(core::cell::UnsafeCell::new(alloc::vec::Vec::new()));

fn contadores() -> &'static mut alloc::vec::Vec<(u64, u64)> {
    // SAFETY: ver `Contadores`; nadie guarda la referencia.
    unsafe { &mut *CONTADORES.0.get() }
}

pub(crate) fn reiniciar() {
    contadores().clear();
}

/// El numero del contador `(recurso, desplazamiento)` (el mismo si ya
/// estaba); 0 si ya hay 65535 (y se dice: el UAV queda sin contador).
fn contador_de(recurso: u64, desplazamiento: u64) -> u32 {
    let c = contadores();
    if let Some(i) = c.iter().position(|&x| x == (recurso, desplazamiento)) {
        return i as u32 + 1;
    }
    if c.len() >= 0xFFFF {
        aviso("CreateUnorderedAccessView: mas de 65535 contadores distintos: este se queda sin el");
        return 0;
    }
    c.push((recurso, desplazamiento));
    c.len() as u32
}

/// El contador de numero `n` (el de la ranura de un UAV): `(recurso,
/// desplazamiento)`.
pub(crate) fn contador(n: u32) -> Option<(u64, u64)> {
    n.checked_sub(1).and_then(|i| contadores().get(i as usize).copied())
}

fn vista_de_destino(recurso: u64, desc: *const u8, handle: u64, marca: u64, leer: unsafe fn(*const u8) -> Result<Vista, &'static str>) {
    if handle == 0 {
        return;
    }
    let v = if desc.is_null() {
        Vista { mapeo: MAPEO, ..Vista::default() }
    } else {
        // SAFETY: la descripcion del `.exe`.
        match unsafe { leer(desc) } {
            Ok(v) => v,
            Err(m) => {
                aviso(m);
                return;
            }
        }
    };
    poner(handle, recurso, marca, &v);
}

/// Los formatos `*_SRGB` de DXGI: R8G8B8A8, BC1, BC2, BC3, B8G8R8A8,
/// B8G8R8X8 y BC7.
pub(crate) fn es_srgb(formato: u32) -> bool {
    matches!(formato, 29 | 72 | 75 | 78 | 91 | 93 | 99)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn palabras(w: &[u32]) -> [u8; 40] {
        let mut d = [0u8; 40];
        for (k, x) in w.iter().enumerate() {
            d[4 * k..4 * k + 4].copy_from_slice(&x.to_le_bytes());
        }
        d
    }

    #[test]
    fn cada_dimension_lee_lo_suyo() {
        // SRV de un array 2D: formato 71 (BC1), mip 2, capa 5.
        let d = palabras(&[71, 5, 0x1688, 0, 2, 1, 5, 1]);
        // SAFETY: 40 bytes.
        let v = unsafe { srv(d.as_ptr()) }.unwrap();
        assert_eq!((v.dimension, v.formato, v.mip, v.capa), (5, 71, 2, 5));
        // SRV de un cubo: mip 1; de un array de cubos: la primera cara.
        let d = palabras(&[0, 9, 0x1688, 0, 1, 3]);
        assert_eq!(unsafe { srv(d.as_ptr()) }.unwrap().mip, 1);
        let d = palabras(&[0, 10, 0x1688, 0, 0, 1, 12, 2]);
        assert_eq!(unsafe { srv(d.as_ptr()) }.unwrap().capa, 12);
        // SRV de un bufer: el primer elemento (u64 en +16).
        let d = palabras(&[0, 1, 0x1688, 0, 7, 0, 100, 16]);
        assert_eq!(unsafe { srv(d.as_ptr()) }.unwrap().elemento, 7);
        // RTV 3D: mip 1, rebanada 4; RTV de array 2D: mip 0, capa 3.
        let d = palabras(&[28, 8, 1, 4, 1]);
        let v = unsafe { rtv(d.as_ptr()) }.unwrap();
        assert_eq!((v.mip, v.rebanada), (1, 4));
        let d = palabras(&[28, 5, 0, 3, 1, 0]);
        assert_eq!(unsafe { rtv(d.as_ptr()) }.unwrap().capa, 3);
        // DSV de array 2D: Flags +8, MipSlice +12, FirstArraySlice +16.
        let d = palabras(&[40, 4, 0, 2, 6, 1]);
        let v = unsafe { dsv(d.as_ptr()) }.unwrap();
        assert_eq!((v.mip, v.capa), (2, 6));
        // Una dimension que no existe.
        let d = palabras(&[0, 12]);
        assert!(unsafe { srv(d.as_ptr()) }.is_err());
        assert!(es_srgb(99) && es_srgb(29) && !es_srgb(28));
    }

    /// N5.3: un SRV de bufer deja en su ranura el primer elemento, cuantos,
    /// el paso y si es crudo; y se leen tal cual.
    #[test]
    fn el_srv_de_bufer_guarda_su_vista_entera() {
        let mut ranura = [0u64; 4];
        // Estructurado: desde el 5, 7 elementos de 20 bytes.
        let d = palabras(&[0, SRV_BUFER, 0x1688, 0, 5, 0, 7, 20, 0]);
        create_shader_resource_view(0, 0, d.as_ptr(), ranura.as_mut_ptr() as u64);
        assert_eq!(ranura[1], DESC_SRV);
        assert_eq!(leer_bufer(&ranura), VistaBufer { formato: 0, primero: 5, elementos: 7, paso: 20, crudo: false });
        assert_eq!(leer(&ranura).0 .0, SRV_BUFER, "la dimension se sigue leyendo igual");
        // Crudo (R32_TYPELESS, 39, y la bandera RAW): 64 palabras.
        let d = palabras(&[39, SRV_BUFER, 0x1688, 0, 0, 0, 64, 0, 1]);
        create_shader_resource_view(0, 0, d.as_ptr(), ranura.as_mut_ptr() as u64);
        assert_eq!(leer_bufer(&ranura), VistaBufer { formato: 39, primero: 0, elementos: 64, paso: 0, crudo: true });
    }
}
