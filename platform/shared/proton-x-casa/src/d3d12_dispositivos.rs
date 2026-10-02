//! **`ID3D12Device1` a `ID3D12Device10`** (tanda 45, 02-10).
//!
//! Cyberpunk, con el dispositivo hecho, pide `ID3D12Device1`, `4`, `8` y `10`
//! (`QueryInterface`), y la casa decia que no. Cada una hereda de la
//! anterior, asi que es el MISMO objeto con una vtabla mas larga (79 huecos,
//! los de `d3d12.idl`). Lo que hay aqui es lo de cada version que un motor
//! usa sin rayos ni malla (que la casa dice que no tiene):
//!
//! ```text
//!    1   CreatePipelineLibrary         DXGI_ERROR_UNSUPPORTED (documentado:
//!                                      el motor compila cada PSO, sin cache)
//!        SetEventOnMultipleFenceCompletion, SetResidencyPriority
//!    2   CreatePipelineState           el flujo de subobjetos, traducido al
//!                                      D3D12_GRAPHICS_PIPELINE_STATE_DESC
//!    3   EnqueueMakeResident           todo es residente: la valla, ya
//!    4   CreateCommandList1 (CERRADA), CreateCommittedResource1, CreateHeap1,
//!        GetResourceAllocationInfo1
//!    5   RemoveDevice, EnumerateMetaCommands (ninguno),
//!        CheckDriverMatchingIdentifier (sin rayos: tipo no soportado)
//!    6   SetBackgroundProcessingMode   no hay nada que medir
//!    8   GetResourceAllocationInfo2, CreateCommittedResource2,
//!        CreatePlacedResource1, GetCopyableFootprints1 (D3D12_RESOURCE_DESC1:
//!        el de siempre y 12 bytes de sampler feedback detras)
//!    9   CreateCommandQueue1; la cache de sombreadores, no (se dice)
//!    10  CreateCommittedResource3, CreatePlacedResource2 (con layout en vez
//!        de estado: la casa no tiene estados)
//! ```
//!
//! Lo demas (sesiones protegidas, rayos, monton desde una direccion,
//! recursos reservados) sigue siendo un hueco que se dice y sale.

use crate::com::{Guid, E_FAIL, E_INVALIDARG, S_OK};
use crate::{aviso, d3d12, d3d12_montones, hilos, tuberia};

/// DXGI_ERROR_UNSUPPORTED.
pub(crate) const DXGI_ERROR_UNSUPPORTED: i32 = 0x887A_0004_u32 as i32;
/// La medida de un D3D12_RESOURCE_DESC1: el D3D12_RESOURCE_DESC (52 bytes de
/// campos) y un D3D12_MIP_REGION (12), a 8.
const DESC1: usize = 64;

/// `CreatePipelineLibrary(this, blob, n, riid, pp)`: sin libreria, como un
/// driver que no la tiene (el motor cae a crear cada PSO).
pub(crate) extern "win64" fn create_pipeline_library(_this: u64, _blob: *const u8, _n: usize, _riid: *const Guid, pp: *mut u64) -> i32 {
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_UNSUPPORTED
}

/// `SetEventOnMultipleFenceCompletion(this, vallas, valores, n, banderas,
/// evento)`: banderas 0 = TODAS, 1 = CUALQUIERA. Si ya se cumple, el evento
/// se enciende YA; si falta UNA valla (o es una sola), se espera a esa (las
/// vallas solo suben). Esperar a varias a la vez, todavia no: se dice.
pub(crate) extern "win64" fn set_event_on_multiple_fence_completion(_this: u64, vallas: *const u64, valores: *const u64, n: u32, banderas: u32, evento: u64) -> i32 {
    if vallas.is_null() || valores.is_null() || n == 0 || banderas > 1 {
        return E_INVALIDARG;
    }
    // SAFETY: `n` vallas de la casa y `n` valores del `.exe`.
    let pares: alloc::vec::Vec<(u64, u64)> = (0..n as usize).map(|i| unsafe { (vallas.add(i).read_unaligned(), valores.add(i).read_unaligned()) }).collect();
    let faltan: alloc::vec::Vec<(u64, u64)> = pares.iter().copied().filter(|&(v, x)| d3d12::valor_de_valla(v) < x).collect();
    let hecho = if banderas == 0 { faltan.is_empty() } else { faltan.len() < pares.len() };
    if hecho {
        if evento != 0 {
            hilos::encender_evento(evento);
        }
        return S_OK;
    }
    if faltan.len() == 1 {
        return d3d12::set_event_on_completion(faltan[0].0, faltan[0].1, evento);
    }
    aviso("SetEventOnMultipleFenceCompletion esperando a varias vallas a la vez: todavia no");
    E_FAIL
}

/// `SetResidencyPriority(this, n, objetos, prioridades)`: todo es residente.
pub(crate) extern "win64" fn set_residency_priority(_this: u64, _n: u32, _objetos: *const u64, _prioridades: *const u32) -> i32 {
    S_OK
}

/// `EnqueueMakeResident(this, banderas, n, objetos, valla, valor)`: todo es
/// residente ya, asi que la valla llega a su valor en seguida.
pub(crate) extern "win64" fn enqueue_make_resident(_this: u64, _banderas: u32, _n: u32, _objetos: *const u64, valla: u64, valor: u64) -> i32 {
    if valla == 0 {
        return E_INVALIDARG;
    }
    d3d12::fence_signal(valla, valor)
}

/// `CreateCommandList1(this, mascara, tipo, banderas, riid, pp)`: la lista
/// nace CERRADA y sin asignador (la diferencia con CreateCommandList).
pub(crate) extern "win64" fn create_command_list1(this: u64, mascara: u32, tipo: u32, _banderas: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    let r = d3d12::create_command_list(this, mascara, tipo, 0, 0, riid, pp);
    if r == S_OK {
        // SAFETY: la lista que acaba de dejar.
        d3d12::list_close(unsafe { *pp });
    }
    r
}

/// `CreateCommittedResource1(this, props, banderas, desc, estado, clear,
/// sesion, riid, pp)`: sin sesion protegida, como el de siempre.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_committed_resource1(_this: u64, _props: *const u8, _banderas: u32, desc: *const u8, _estado: u32, _clear: *const u8, _sesion: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    tuberia::crear_recurso(desc, riid, pp, None)
}

/// `CreateHeap1(this, desc, sesion, riid, pp)`.
pub(crate) extern "win64" fn create_heap1(this: u64, desc: *const u8, _sesion: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    d3d12_montones::create_heap(this, desc, riid, pp)
}

/// `GetResourceAllocationInfo1(this, ret, mascara, n, descs, info1)`.
pub(crate) extern "win64" fn get_resource_allocation_info1(_this: u64, ret: *mut u64, _mascara: u32, n: u32, descs: *const u8, info1: *mut u64) -> *mut u64 {
    d3d12::asignacion(ret, n, descs, 56, info1)
}

/// `GetResourceAllocationInfo2(this, ret, mascara, n, descs1, info1)`.
pub(crate) extern "win64" fn get_resource_allocation_info2(_this: u64, ret: *mut u64, _mascara: u32, n: u32, descs: *const u8, info1: *mut u64) -> *mut u64 {
    d3d12::asignacion(ret, n, descs, DESC1, info1)
}

/// `RemoveDevice(this)`: desde aqui, GetDeviceRemovedReason lo dice.
pub(crate) extern "win64" fn remove_device(this: u64) {
    d3d12::quitar_dispositivo(this);
}

/// `EnumerateMetaCommands(this, pn, descs)`: ninguno.
pub(crate) extern "win64" fn enumerate_meta_commands(_this: u64, n: *mut u32, _descs: *mut u8) -> i32 {
    if n.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el UINT del `.exe`.
    unsafe { n.write_unaligned(0) };
    S_OK
}

/// `CheckDriverMatchingIdentifier(this, tipo, id)`: el unico tipo son las
/// estructuras de aceleracion de rayos, y no hay rayos:
/// D3D12_DRIVER_MATCHING_IDENTIFIER_UNSUPPORTED_TYPE.
pub(crate) extern "win64" fn check_driver_matching_identifier(_this: u64, _tipo: u32, _id: *const u8) -> u32 {
    1
}

/// `SetBackgroundProcessingMode(this, modo, accion, evento, mas)`: no hay
/// trabajo de fondo que medir: el evento, ya, y no hacen falta mas medidas.
pub(crate) extern "win64" fn set_background_processing_mode(_this: u64, _modo: u32, _accion: u32, evento: u64, mas: *mut i32) -> i32 {
    if evento != 0 {
        hilos::encender_evento(evento);
    }
    if !mas.is_null() {
        // SAFETY: el BOOL del `.exe`.
        unsafe { mas.write_unaligned(0) };
    }
    S_OK
}

/// `CreateCommittedResource2(this, props, banderas, desc1, estado, clear,
/// sesion, riid, pp)`: el D3D12_RESOURCE_DESC1 empieza como el de siempre.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_committed_resource2(_this: u64, _props: *const u8, _banderas: u32, desc1: *const u8, _estado: u32, _clear: *const u8, _sesion: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    tuberia::crear_recurso(desc1, riid, pp, None)
}

/// `CreatePlacedResource1(this, monton, desde, desc1, estado, clear, riid, pp)`.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_placed_resource1(this: u64, monton: u64, desde: u64, desc1: *const u8, estado: u32, clear: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    d3d12_montones::create_placed_resource(this, monton, desde, desc1, estado, clear, riid, pp)
}

/// `GetCopyableFootprints1(this, desc1, ...)`: como el de siempre.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn get_copyable_footprints1(this: u64, desc1: *const u8, primero: u32, n: u32, desde: u64, huellas: *mut u8, filas: *mut u32, bytes_fila: *mut u64, total: *mut u64) {
    d3d12::get_copyable_footprints(this, desc1, primero, n, desde, huellas, filas, bytes_fila, total)
}

/// `CreateShaderCacheSession(this, desc, riid, pp)`: la cache de sombreadores
/// de la aplicacion, no hay (se dice: el motor deberia seguir sin ella).
pub(crate) extern "win64" fn create_shader_cache_session(_this: u64, _desc: *const u8, _riid: *const Guid, pp: *mut u64) -> i32 {
    aviso("CreateShaderCacheSession: la casa no tiene cache de sombreadores: DXGI_ERROR_UNSUPPORTED");
    if !pp.is_null() {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
    }
    DXGI_ERROR_UNSUPPORTED
}

/// `ShaderCacheControl(this, tipos, control)`: lo mismo (en Windows, solo en
/// modo desarrollador).
pub(crate) extern "win64" fn shader_cache_control(_this: u64, _tipos: u32, _control: u32) -> i32 {
    aviso("ShaderCacheControl: la casa no tiene cache de sombreadores: DXGI_ERROR_UNSUPPORTED");
    DXGI_ERROR_UNSUPPORTED
}

/// `CreateCommandQueue1(this, desc, creador, riid, pp)`.
pub(crate) extern "win64" fn create_command_queue1(this: u64, desc: *const u8, _creador: *const Guid, riid: *const Guid, pp: *mut u64) -> i32 {
    d3d12::create_command_queue(this, desc, riid, pp)
}

/// `CreateCommittedResource3(this, props, banderas, desc1, layout, clear,
/// sesion, n_formatos, formatos, riid, pp)`.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_committed_resource3(_this: u64, _props: *const u8, _banderas: u32, desc1: *const u8, _layout: u32, _clear: *const u8, _sesion: u64, _n_formatos: u32, _formatos: *const u32, riid: *const Guid, pp: *mut u64) -> i32 {
    tuberia::crear_recurso(desc1, riid, pp, None)
}

/// `CreatePlacedResource2(this, monton, desde, desc1, layout, clear,
/// n_formatos, formatos, riid, pp)`.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn create_placed_resource2(this: u64, monton: u64, desde: u64, desc1: *const u8, _layout: u32, clear: *const u8, _n_formatos: u32, _formatos: *const u32, riid: *const Guid, pp: *mut u64) -> i32 {
    d3d12_montones::create_placed_resource(this, monton, desde, desc1, 0, clear, riid, pp)
}

// -- CreatePipelineState: el flujo de subobjetos ---------------------------

/// La medida del D3D12_GRAPHICS_PIPELINE_STATE_DESC (ver `tuberia::pso_de`).
const PSO: usize = 656;

/// Un D3D12_GRAPHICS_PIPELINE_STATE_DESC con lo que D3D12 da a un subobjeto
/// que no viene: la mezcla, el rasterizador y la profundidad de
/// `CD3DX12_*(D3D12_DEFAULT)`, la mascara de muestras entera y una muestra.
fn por_defecto() -> [u8; PSO] {
    let mut d = [0u8; PSO];
    let mut u = |o: usize, v: u32| d[o..o + 4].copy_from_slice(&v.to_le_bytes());
    for i in 0..8 {
        let rt = 128 + 40 * i;
        // SrcBlend ONE, DestBlend ZERO, BlendOp ADD, lo mismo en alfa, LogicOp NOOP.
        for (k, v) in [(8, 2), (12, 1), (16, 1), (20, 2), (24, 1), (28, 1), (32, 4)] {
            u(rt + k, v);
        }
        u(rt + 36, 0xF);
    }
    u(448, u32::MAX);
    // FillMode SOLID, CullMode BACK, DepthClipEnable.
    u(452, 3);
    u(456, 3);
    u(476, 1);
    // DepthEnable, DepthWriteMask ALL, DepthFunc LESS; las mascaras de stencil
    // a 0xFF; cada cara KEEP, KEEP, KEEP, ALWAYS.
    u(496, 1);
    u(500, 1);
    u(504, 2);
    u(512, 0xFFFF);
    for cara in [516, 532] {
        for (k, v) in [(0, 1), (4, 1), (8, 1), (12, 8)] {
            u(cara + k, v);
        }
    }
    u(616, 1);
    d
}

/// **El flujo, al descriptor de siempre.** Cada subobjeto va a 8 (es un
/// `alignas(void*)` de d3dx12): su tipo (u32) y lo de dentro, a 8 si lleva
/// punteros o a 4 si no. Lo que la casa no sabe leer no se salta: sin saber
/// su medida, lo que viene detras no se encuentra.
///
/// # Safety
/// `p` son `n` bytes del `.exe`.
unsafe fn pso_de_flujo(p: *const u8, n: usize) -> Result<[u8; PSO], &'static str> {
    let mut d = por_defecto();
    let mut profundidad_dicha = false;
    let mut o = 0usize;
    while o < n {
        if n - o < 4 {
            return Err("CreatePipelineState: un flujo que se corta");
        }
        let tipo = (p.add(o) as *const u32).read_unaligned();
        // (alineacion de lo de dentro, su medida, donde va en el descriptor)
        let (alinea, medida, destino): (usize, usize, Option<usize>) = match tipo {
            0 => (8, 8, Some(0)),
            1..=5 => (8, 16, Some(8 + 16 * (tipo as usize - 1))),
            6 => return Err("CreatePipelineState de computo: todavia no"),
            7 => (8, 32, Some(88)),
            8 => (4, 328, Some(120)),
            9 => (4, 4, Some(448)),
            10 | 26 => (4, 44, Some(452)),
            11 => (4, 52, Some(496)),
            12 => (8, 16, Some(552)),
            13 => (4, 4, Some(568)),
            14 => (4, 4, Some(572)),
            15 => (4, 36, None),
            16 => (4, 4, Some(612)),
            17 => (4, 8, Some(616)),
            18 => (4, 4, Some(624)),
            19 => (8, 16, Some(632)),
            20 => (4, 4, Some(648)),
            21 => (4, 56, None),
            22 => (8, 24, None),
            23 | 24 => return Err("CreatePipelineState con sombreadores de malla: no hay (OPTIONS7 lo dice)"),
            _ => return Err("CreatePipelineState con un subobjeto que la casa no sabe leer"),
        };
        let dentro = o + alinea;
        let fin = dentro.checked_add(medida).ok_or("CreatePipelineState: un flujo que se sale")?;
        if fin > n {
            return Err("CreatePipelineState: un flujo que se sale de su medida");
        }
        let q = core::slice::from_raw_parts(p.add(dentro), medida);
        match (tipo, destino) {
            (_, Some(k)) => d[k..k + medida].copy_from_slice(q),
            // D3D12_RT_FORMAT_ARRAY: los 8 formatos y cuantos.
            (15, _) => {
                d[580..612].copy_from_slice(&q[..32]);
                d[576..580].copy_from_slice(&q[32..36]);
            }
            // D3D12_DEPTH_STENCIL_DESC1: el de siempre y DepthBoundsTestEnable.
            (21, _) => d[496..548].copy_from_slice(&q[..52]),
            // D3D12_VIEW_INSTANCING_DESC: una vista (o ninguna) es lo de siempre.
            (22, _) if u32::from_le_bytes([q[0], q[1], q[2], q[3]]) > 1 => return Err("CreatePipelineState con varias vistas (view instancing): todavia no"),
            _ => {}
        }
        if tipo == 26 {
            // D3D12_RASTERIZER_DESC1: el DepthBias es un float; la casa no lo usa.
            let f = f32::from_le_bytes([q[12], q[13], q[14], q[15]]);
            d[464..468].copy_from_slice(&(f as i32).to_le_bytes());
        }
        profundidad_dicha |= matches!(tipo, 11 | 21);
        o = (fin + 7) & !7;
    }
    // Sin formato de profundidad ni subobjeto que la pida, no hay prueba.
    if !profundidad_dicha && d[612..616] == [0; 4] {
        d[496..500].copy_from_slice(&0u32.to_le_bytes());
    }
    Ok(d)
}

/// `CreatePipelineState(this, desc, riid, pp)`: D3D12_PIPELINE_STATE_STREAM_DESC
/// (SizeInBytes +0, pPipelineStateSubobjectStream +8), traducido y creado
/// como un PSO grafico.
pub(crate) extern "win64" fn create_pipeline_state(this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_PIPELINE_STATE_STREAM_DESC del `.exe`, y su flujo.
    let r = unsafe {
        let (n, p) = ((desc as *const usize).read_unaligned(), (desc.add(8) as *const u64).read_unaligned() as *const u8);
        if p.is_null() || n == 0 {
            return E_INVALIDARG;
        }
        pso_de_flujo(p, n)
    };
    match r {
        Ok(d) => tuberia::create_graphics_pipeline_state(this, d.as_ptr(), riid, pp),
        Err(m) => {
            aviso(m);
            E_INVALIDARG
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un subobjeto: el tipo, el relleno hasta su alineacion, lo de dentro y
    /// el relleno hasta 8.
    fn sub(f: &mut alloc::vec::Vec<u8>, tipo: u32, alinea: usize, dentro: &[u8]) {
        f.extend_from_slice(&tipo.to_le_bytes());
        f.resize(f.len() + alinea - 4, 0);
        f.extend_from_slice(dentro);
        f.resize((f.len() + 7) & !7, 0);
    }

    #[test]
    fn un_flujo_se_traduce_y_lo_que_no_viene_es_lo_de_d3d12() {
        let mut f = alloc::vec::Vec::new();
        sub(&mut f, 0, 8, &0x1234u64.to_le_bytes());
        sub(&mut f, 14, 4, &3u32.to_le_bytes());
        let mut rt = [0u8; 36];
        rt[..4].copy_from_slice(&28u32.to_le_bytes());
        rt[32..].copy_from_slice(&1u32.to_le_bytes());
        sub(&mut f, 15, 4, &rt);
        // SAFETY: el flujo de arriba.
        let d = unsafe { pso_de_flujo(f.as_ptr(), f.len()) }.unwrap();
        let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        assert_eq!(u64::from_le_bytes(d[..8].try_into().unwrap()), 0x1234);
        assert_eq!((u(572), u(576), u(580)), (3, 1, 28));
        // Los de por defecto: mascara de muestras, CullMode BACK, una muestra,
        // y sin formato de profundidad, la prueba apagada.
        assert_eq!((u(448), u(456), u(616), u(496)), (u32::MAX, 3, 1, 0));
    }

    #[test]
    fn lo_que_no_se_sabe_leer_no_se_salta() {
        let mut f = alloc::vec::Vec::new();
        sub(&mut f, 99, 4, &[0; 4]);
        // SAFETY: el flujo de arriba.
        assert!(unsafe { pso_de_flujo(f.as_ptr(), f.len()) }.is_err());
        let mut g = alloc::vec::Vec::new();
        sub(&mut g, 6, 8, &[0; 16]);
        // SAFETY: como arriba.
        assert!(unsafe { pso_de_flujo(g.as_ptr(), g.len()) }.is_err());
        // Uno que dice ser mas largo que el flujo.
        // SAFETY: como arriba.
        assert!(unsafe { pso_de_flujo(g.as_ptr(), 12) }.is_err());
    }
}

/// Que la vtabla del dispositivo tiene los 79 huecos de ID3D12Device10.
#[cfg(test)]
#[test]
fn el_dispositivo_tiene_los_huecos_de_la_10() {
    assert_eq!(crate::com::M_ID3D12DEVICE.len(), 79);
    assert_eq!(crate::com::M_ID3D12DEVICE[47], "CreatePipelineState");
    assert_eq!(crate::com::M_ID3D12DEVICE[77], "CreatePlacedResource2");
}
