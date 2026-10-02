//! **`ID3D12Device::CreateHeap` y `CreatePlacedResource`: la memoria de la
//! tarjeta en montones** (02-10).
//!
//! Cyberpunk, con lo grafico ya montado, pide un monton de memoria
//! (`ID3D12Heap`) para colocar recursos dentro; la casa no tenia el hueco 28
//! y el juego salia con `0xC0DE001C`. Un motor grande no crea cada recurso
//! con su memoria (CreateCommittedResource): pide montones grandes y coloca
//! (CreatePlacedResource) los recursos en desplazamientos de ellos.
//!
//! ```text
//!    CreateHeap            el monton guarda su D3D12_HEAP_DESC (48 bytes):
//!                          SizeInBytes +0, Properties +8 (Type, CPUPage,
//!                          MemoryPool, CreationNodeMask, VisibleNodeMask),
//!                          Alignment +32, Flags +40
//!    ID3D12Heap::GetDesc   la devuelve (por el puntero oculto)
//!    CreatePlacedResource  un BUFER vive en la memoria del monton, en su
//!                          desplazamiento: dos en el mismo sitio SE VEN
//!                          (el "aliasing", como en Windows); una TEXTURA,
//!                          con su propia memoria, como una comprometida
//! ```
//!
//! **La memoria del monton** (tanda 44, 02-10): con la reserva, CreateHeap
//! toma DIRECCIONES de la ventana sin hacer ninguna pagina, y cada bufer
//! colocado hace solo las suyas. Antes cada bufer colocado era un `Vec` del
//! monton del cargador (48 MiB, solo avanza), y Cyberpunk lo agoto con uno de
//! 192 MiB: un panico en el cargador. Sin reserva (el banco viejo), el bufer
//! colocado tiene la suya, y que dos en el mismo sitio no se vean se dice.

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, E_OUTOFMEMORY, S_FALSE};
use crate::{aviso, dir};

/// D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT (64 KiB) y la de MSAA (4 MiB).
const ALINEADO: u64 = 0x1_0000;
const ALINEADO_MSAA: u64 = 0x40_0000;
/// D3D12_HEAP_TYPE: DEFAULT 1 ... CUSTOM 4, GPU_UPLOAD 5.
const TIPO_CUSTOM: u32 = 4;
const TIPO_MAXIMO: u32 = 5;
/// D3D12_RESOURCE_DIMENSION_BUFFER.
const DIMENSION_BUFFER: u32 = 1;

/// Lo de dentro de un ID3D12Heap: su descripcion, ya completada.
pub(crate) struct Monton {
    desc: [u8; 48],
    /// Sus direcciones en la ventana de reserva, o 0 si no hay reserva.
    base: u64,
    /// Su memoria YA esta hecha (`OpenExistingHeapFromAddress`: es la de un
    /// VirtualAlloc del `.exe`): colocar no hace paginas.
    hecha: bool,
    /// Sin reserva: los desplazamientos ya ocupados (para decir el aliasing).
    colocados: alloc::vec::Vec<u64>,
}

/// D3D12_HEAP_FLAG_DENY_BUFFERS, DENY_RT_DS_TEXTURES, DENY_NON_RT_DS_TEXTURES.
const NIEGA_BUFERES: u32 = 0x4;
const NIEGA_RT_DS: u32 = 0x40;
const NIEGA_OTRAS_TEXTURAS: u32 = 0x80;
/// D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET | ALLOW_DEPTH_STENCIL.
const ES_RT_DS: u32 = 0x1 | 0x2;

/// **Lo que un monton admite** (tanda 48, lo que dijo Windows el 02-10):
/// las banderas DENY_* del monton contra la clase de recurso. Un monton
/// ALLOW_ONLY_RT_DS_TEXTURES (0x84) no admite una textura sin RENDER_TARGET
/// ni DEPTH_STENCIL; Windows dice E_INVALIDARG y la casa lo dejaba pasar.
fn admite(banderas: u32, dimension: u32, flags_recurso: u32) -> bool {
    let niega = if dimension == DIMENSION_BUFFER {
        NIEGA_BUFERES
    } else if flags_recurso & ES_RT_DS != 0 {
        NIEGA_RT_DS
    } else {
        NIEGA_OTRAS_TEXTURAS
    };
    banderas & niega == 0
}

fn u32_de(p: &[u8], k: usize) -> u32 {
    u32::from_le_bytes([p[k], p[k + 1], p[k + 2], p[k + 3]])
}

fn u64_de(p: &[u8], k: usize) -> u64 {
    u64::from(u32_de(p, k)) | (u64::from(u32_de(p, k + 4)) << 32)
}

/// Lo que Windows dice de una descripcion: `None` si no vale, o la misma
/// con lo que el runtime rellena (alineado 0 = 64 KiB, nodos 0 = el 1).
fn completar(mut d: [u8; 48]) -> Option<[u8; 48]> {
    let (medida, tipo, pagina, piscina, alineado) = (u64_de(&d, 0), u32_de(&d, 8), u32_de(&d, 12), u32_de(&d, 16), u64_de(&d, 32));
    if medida == 0 || tipo == 0 || tipo > TIPO_MAXIMO {
        return None;
    }
    // Solo el CUSTOM dice la pagina de CPU y la piscina; los demas, UNKNOWN.
    if (tipo == TIPO_CUSTOM) != (pagina != 0 && piscina != 0) {
        return None;
    }
    let alineado = match alineado {
        0 => ALINEADO,
        ALINEADO | ALINEADO_MSAA => alineado,
        _ => return None,
    };
    d[32..40].copy_from_slice(&alineado.to_le_bytes());
    for k in [20, 24] {
        if u32_de(&d, k) == 0 {
            d[k..k + 4].copy_from_slice(&1u32.to_le_bytes());
        }
    }
    Some(d)
}

/// `CreateHeap(this, pDesc, riid, ppvHeap)`. Con `ppvHeap` nulo solo
/// pregunta si se podria: S_FALSE, como Windows.
pub(crate) extern "win64" fn create_heap(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    let mut d = [0u8; 48];
    // SAFETY: un D3D12_HEAP_DESC (48 bytes) del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(desc, d.as_mut_ptr(), 48) };
    let Some(d) = completar(d) else {
        return E_INVALIDARG;
    };
    if pp.is_null() {
        return S_FALSE;
    }
    if !pide(riid, com::MEMORIA) {
        return E_NOINTERFACE;
    }
    let base = match crate::memoria::reservar_direcciones(u64_de(&d, 0)) {
        Some(b) => b,
        None if crate::memoria::hay_reserva() => {
            aviso("CreateHeap: no queda hueco en la ventana de reserva: E_OUTOFMEMORY");
            return E_OUTOFMEMORY;
        }
        None => 0,
    };
    let vt = vtabla::<{ com::MEMORIA }>(&[(8, dir!(get_desc))]);
    dar(pp, nuevo(com::MEMORIA, vt, Monton { desc: d, base, hecha: false, colocados: alloc::vec::Vec::new() }) as u64)
}

/// D3D12_CPU_PAGE_PROPERTY_WRITE_BACK y D3D12_MEMORY_POOL_L0.
const PAGINA_WRITE_BACK: u32 = 3;
const PISCINA_L0: u32 = 1;
/// D3D12_HEAP_FLAG_SHARED | SHARED_CROSS_ADAPTER | ALLOW_ONLY_BUFFERS.
const BANDERAS_DE_DIRECCION: u32 = 0x1 | 0x20 | 0xC0;

/// **`OpenExistingHeapFromAddress(this, direccion, riid, pp)`** (Device3,
/// tanda 46): un monton cuya memoria ES la de un `VirtualAlloc` del `.exe`.
/// Como Windows (y vkd3d-proton, que lo copia): la direccion es la BASE de
/// su region, la region entera esta hecha con una sola proteccion, y el
/// monton mide eso, es CUSTOM con paginas WRITE_BACK en L0 y solo admite
/// buferes. Lo que se coloque en el escribe en esa misma memoria.
pub(crate) extern "win64" fn open_existing_heap_from_address(_this: u64, direccion: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    let Some(medida) = (direccion != 0).then(|| crate::memoria::region_entera(direccion)).flatten() else {
        return E_INVALIDARG;
    };
    if pp.is_null() {
        return E_INVALIDARG;
    }
    if !pide(riid, com::MEMORIA) {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
        return E_NOINTERFACE;
    }
    let mut d = [0u8; 48];
    d[0..8].copy_from_slice(&medida.to_le_bytes());
    for (k, v) in [(8, TIPO_CUSTOM), (12, PAGINA_WRITE_BACK), (16, PISCINA_L0), (20, 1), (24, 1), (40, BANDERAS_DE_DIRECCION)] {
        d[k..k + 4].copy_from_slice(&v.to_le_bytes());
    }
    d[32..40].copy_from_slice(&ALINEADO.to_le_bytes());
    let vt = vtabla::<{ com::MEMORIA }>(&[(8, dir!(get_desc))]);
    dar(pp, nuevo(com::MEMORIA, vt, Monton { desc: d, base: direccion, hecha: true, colocados: alloc::vec::Vec::new() }) as u64)
}

/// `ID3D12Heap::GetDesc(this, ret)`: la estructura por el puntero oculto.
extern "win64" fn get_desc(this: u64, ret: *mut u8) -> *mut u8 {
    // SAFETY: `this` es un Monton de la casa (su vtabla); `ret`, 48 bytes
    // del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(de::<Monton>(this).desc.as_ptr(), ret, 48) };
    ret
}

/// `CreatePlacedResource(this, pHeap, HeapOffset, pDesc, InitialState,
/// pOptimizedClearValue, riid, ppvResource)`: el desplazamiento tiene que
/// caber en el monton y estar alineado a 64 KiB (o a 4 KiB, el de los
/// recursos chicos). Un bufer, ademas, tiene que caber ENTERO, y vive en la
/// memoria del monton; lo demas, como uno comprometido.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_placed_resource(_this: u64, monton: u64, desde: u64, desc: *const u8, _estado: u32, _clear: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if monton == 0 || desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un ID3D12Heap que dio la casa.
    let m = unsafe { de::<Monton>(monton) };
    let medida = u64_de(&m.desc, 0);
    if desde >= medida || desde % 0x1000 != 0 {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_RESOURCE_DESC del `.exe`: Dimension +0, Width +16.
    let (dimension, ancho) = unsafe { (desc.cast::<u32>().read_unaligned(), desc.add(16).cast::<u64>().read_unaligned()) };
    // SAFETY: el mismo D3D12_RESOURCE_DESC: Flags +48.
    if !admite(u32_de(&m.desc, 40), dimension, unsafe { desc.add(48).cast::<u32>().read_unaligned() }) {
        return E_INVALIDARG;
    }
    if dimension == DIMENSION_BUFFER && m.base != 0 {
        // Uno que no cabe desde ahi: E_INVALIDARG, como Windows (no es algo
        // que le falte a la casa: no se avisa).
        if ancho == 0 || ancho > medida - desde {
            return E_INVALIDARG;
        }
        if !m.hecha && !crate::memoria::hacer_paginas(m.base + desde, ancho) {
            aviso("CreatePlacedResource: el kernel no tiene RAM para el bufer: E_OUTOFMEMORY");
            return E_OUTOFMEMORY;
        }
        let r = crate::tuberia::crear_recurso(desc, riid, pp, Some(m.base + desde));
        crate::d3d12_resto::apuntar_monton(r, pp, m.desc[8..].as_ptr());
        return r;
    }
    // Tanda 48: lo que no cabe desde su desplazamiento es E_INVALIDARG, como
    // los drivers nativos (el inventario de Cyberpunk: un monton de dos mips
    // y una textura de seis). Con la cuenta de la casa (`d3d12_medidas`).
    // SAFETY: el D3D12_RESOURCE_DESC del `.exe`.
    let (necesita, _) = unsafe { crate::d3d12_medidas::medida(desc) };
    if necesita > medida - desde {
        return E_INVALIDARG;
    }
    if m.colocados.contains(&desde) {
        aviso("CreatePlacedResource: dos recursos en el mismo sitio de un monton; en la casa no comparten memoria");
    } else {
        m.colocados.push(desde);
    }
    let r = crate::tuberia::crear_recurso(desc, riid, pp, None);
    crate::d3d12_resto::apuntar_monton(r, pp, m.desc[8..].as_ptr());
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn desc(medida: u64, tipo: u32, alineado: u64) -> [u8; 48] {
        let mut d = [0u8; 48];
        d[0..8].copy_from_slice(&medida.to_le_bytes());
        d[8..12].copy_from_slice(&tipo.to_le_bytes());
        d[32..40].copy_from_slice(&alineado.to_le_bytes());
        d
    }

    #[test]
    fn una_buena_se_completa_y_las_malas_no_valen() {
        let d = completar(desc(1 << 20, 1, 0)).unwrap();
        assert_eq!(u64_de(&d, 32), ALINEADO);
        assert_eq!((u32_de(&d, 20), u32_de(&d, 24)), (1, 1));
        assert!(completar(desc(0, 1, 0)).is_none());
        assert!(completar(desc(1 << 20, 0, 0)).is_none());
        assert!(completar(desc(1 << 20, 1, 12345)).is_none());
        // CUSTOM sin pagina de CPU ni piscina, no.
        assert!(completar(desc(1 << 20, TIPO_CUSTOM, 0)).is_none());
    }

    #[test]
    fn las_banderas_del_monton_dicen_que_clase_de_recurso_entra() {
        // ALLOW_ALL (0): todo.
        assert!(admite(0, DIMENSION_BUFFER, 0) && admite(0, 3, 0) && admite(0, 3, 1));
        // ALLOW_ONLY_BUFFERS (0xC0).
        assert!(admite(0xC0, DIMENSION_BUFFER, 0) && !admite(0xC0, 3, 0) && !admite(0xC0, 3, 1));
        // ALLOW_ONLY_NON_RT_DS_TEXTURES (0x44): lo que la tanda 48 quiso.
        assert!(!admite(0x44, DIMENSION_BUFFER, 0) && admite(0x44, 3, 0) && !admite(0x44, 3, 2));
        // ALLOW_ONLY_RT_DS_TEXTURES (0x84): lo que la tanda 48 puso, y Windows dijo que no.
        assert!(!admite(0x84, DIMENSION_BUFFER, 0) && !admite(0x84, 3, 0) && admite(0x84, 3, 1));
    }
}
