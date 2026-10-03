//! **Lo que le faltaba a D3D12, contado contra vkd3d-proton** (tanda 47,
//! 02-10).
//!
//! Pedido del propietario: *"investiga como se hicieron, para inspirarse en
//! vkd3d-proton"*, en vez de un muro por cada vez que se corre Cyberpunk. Se
//! contaron los huecos de cada interfaz de la casa contra los que contesta
//! vkd3d-proton (todos): aqui va lo que un motor llama ANTES de su primer
//! fotograma o en cada uno, con lo que la casa sabe hacer:
//!
//! ```text
//!    dispositivo  CreateConstantBufferView, CreateUnorderedAccessView,
//!                 CopyDescriptors(Simple), GetCustomHeapProperties (los de
//!                 una tarjeta con su VRAM, como vkd3d-proton sin UMA),
//!                 MakeResident/Evict y SetStablePowerState (todo residente,
//!                 nada que fijar), CreateQueryHeap, CreateCommandSignature,
//!                 CreateComputePipelineState (se guarda; aun no corre)
//!    cola         Wait (la cola es sincrona), GetTimestampFrequency (ns),
//!                 GetClockCalibration, GetDesc, los marcadores de PIX
//!    lista        GetType, ClearState, CopyBufferRegion, CopyResource,
//!                 ResolveSubresource (una muestra: copiar), Begin/EndQuery y
//!                 ResolveQueryData, marcadores; lo que el sombreador de la
//!                 casa aun no ve (computo, constantes de raiz, SRV/UAV de
//!                 raiz, ExecuteIndirect, predicacion), apuntado y DICHO
//!    recurso      WriteToSubresource, ReadFromSubresource, GetHeapProperties
//!    otros        GetDesc del monton de descriptores y de la cola,
//!                 GetCachedBlob del PSO (un blob propio que la casa ignora
//!                 al volver: como un driver nuevo, recompila)
//! ```
//!
//! Lo que NO esta, a proposito: recursos reservados (tiled), los compartidos
//! entre procesos, sesiones protegidas, rayos y meta-ordenes. Siguen siendo
//! un hueco que se dice y sale.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, S_OK};
use crate::d3d12::{Cola, Lista, Monton, Orden, Recurso, DESCRIPTOR_BYTES, DESC_CBV};
use crate::tuberia::Estado;
use crate::{aviso, dir};

/// Los huecos de ID3D12Device que pone esto.
pub(crate) fn dispositivo() -> [(usize, u64); 13] {
    [
        (11, dir!(create_compute_pipeline_state)),
        (17, dir!(create_constant_buffer_view)),
        (19, dir!(crate::d3d12_vistas::create_unordered_access_view)),
        (23, dir!(copy_descriptors)),
        (24, dir!(copy_descriptors_simple)),
        (26, dir!(get_custom_heap_properties)),
        (34, dir!(make_resident)),
        (35, dir!(make_resident)),
        (39, dir!(create_query_heap)),
        (40, dir!(set_stable_power_state)),
        (41, dir!(create_command_signature)),
        (42, dir!(get_resource_tiling)),
        (63, dir!(get_raytracing_prebuild_info)),
    ]
}

/// Los de ID3D12CommandQueue.
pub(crate) fn cola() -> [(usize, u64); 7] {
    [
        (11, dir!(marcador)),
        (12, dir!(marcador)),
        (13, dir!(fin_de_evento)),
        (15, dir!(queue_wait)),
        (16, dir!(get_timestamp_frequency)),
        (17, dir!(get_clock_calibration)),
        (18, dir!(get_desc_cola)),
    ]
}

/// Los de ID3D12GraphicsCommandList.
pub(crate) fn lista() -> [(usize, u64); 32] {
    [
        (8, dir!(get_type)),
        (11, dir!(clear_state)),
        (14, dir!(dispatch)),
        (15, dir!(copy_buffer_region)),
        (17, dir!(copy_resource)),
        (19, dir!(resolve_subresource)),
        (23, dir!(om_set_blend_factor)),
        (24, dir!(om_set_stencil_ref)),
        (27, dir!(execute_bundle)),
        (29, dir!(de_computo1)),
        (31, dir!(de_computo2)),
        (33, dir!(de_computo3)),
        (34, dir!(root_32bit_constant)),
        (35, dir!(de_computo4)),
        (36, dir!(root_32bit_constants)),
        (37, dir!(de_computo2)),
        (39, dir!(de_computo2)),
        (40, dir!(root_descriptor)),
        (41, dir!(de_computo2)),
        (42, dir!(root_descriptor)),
        (45, dir!(so_set_targets)),
        (49, dir!(clear_uav)),
        (50, dir!(clear_uav)),
        (51, dir!(discard_resource)),
        (52, dir!(begin_query)),
        (53, dir!(end_query)),
        (54, dir!(resolve_query_data)),
        (55, dir!(set_predication)),
        (56, dir!(marcador)),
        (57, dir!(marcador)),
        (58, dir!(fin_de_evento)),
        (59, dir!(execute_indirect)),
    ]
}

fn u32_de(p: *const u8, o: usize) -> u32 {
    // SAFETY: lo garantiza quien llama: una estructura del `.exe`.
    unsafe { (p.add(o) as *const u32).read_unaligned() }
}

fn u64_de(p: *const u8, o: usize) -> u64 {
    // SAFETY: como arriba.
    unsafe { (p.add(o) as *const u64).read_unaligned() }
}

fn poner_u32(p: *mut u8, o: usize, v: u32) {
    // SAFETY: como arriba.
    unsafe { (p.add(o) as *mut u32).write_unaligned(v) };
}

// -- El dispositivo ----------------------------------------------------------

/// Los PSO de computo que se crearon (para que una lista no los dibuje).
struct Computos(UnsafeCell<Vec<u64>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Computos {}
static COMPUTOS: Computos = Computos(UnsafeCell::new(Vec::new()));

pub(crate) fn es_computo(pso: u64) -> bool {
    // SAFETY: ver `Computos`.
    pso != 0 && unsafe { &*COMPUTOS.0.get() }.contains(&pso)
}

pub(crate) fn reiniciar() {
    // SAFETY: ver `Computos`.
    unsafe { (*COMPUTOS.0.get()).clear() };
}

/// Un PSO de computo: su root signature y su sombreador, guardados. La casa
/// aun no corre computo (Dispatch lo dice).
pub struct Computo {
    pub raiz: u64,
    pub cs: Vec<u8>,
}

/// `CreateComputePipelineState(this, desc, riid, pp)`:
/// D3D12_COMPUTE_PIPELINE_STATE_DESC -- pRootSignature +0, CS +8 (puntero y
/// medida), NodeMask +24, CachedPSO +32, Flags +48.
extern "win64" fn create_compute_pipeline_state(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    crate::pulso::contar(crate::pulso::Cosa::Pso, 0);
    if !pide(riid, com::PSO) {
        return E_NOINTERFACE;
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    let (raiz, cs, n) = (u64_de(desc, 0), u64_de(desc, 8), u64_de(desc, 16) as usize);
    if raiz == 0 || cs == 0 || n == 0 {
        return E_INVALIDARG;
    }
    // SAFETY: `n` bytes del sombreador, del `.exe`.
    let cs = unsafe { core::slice::from_raw_parts(cs as *const u8, n) }.to_vec();
    let vt = vtabla::<{ com::PSO }>(&[(8, dir!(get_cached_blob))]);
    let obj = nuevo(com::PSO, vt, Computo { raiz, cs }) as u64;
    // SAFETY: ver `Computos`.
    unsafe { (*COMPUTOS.0.get()).push(obj) };
    dar(pp, obj)
}

/// `CreateConstantBufferView(this, desc, handle)`: D3D12_CONSTANT_BUFFER_VIEW_DESC
/// -- BufferLocation +0, SizeInBytes +8. En la ranura: la direccion, la marca
/// y la medida (un desc nulo deja la vista vacia).
extern "win64" fn create_constant_buffer_view(_this: u64, desc: *const u8, handle: u64) {
    if handle == 0 {
        return;
    }
    let (va, n) = if desc.is_null() { (0, 0) } else { (u64_de(desc, 0), u32_de(desc, 8) as u64) };
    // SAFETY: la ranura de un monton de la casa: 4 palabras.
    unsafe {
        let r = handle as *mut u64;
        r.write(va);
        r.add(1).write(DESC_CBV);
        r.add(2).write(n);
        r.add(3).write(0);
    }
}

/// `CopyDescriptorsSimple(this, n, destino, origen, tipo)`: `n` ranuras
/// seguidas (pueden pisarse: como memmove).
extern "win64" fn copy_descriptors_simple(_this: u64, n: u32, destino: u64, origen: u64, _tipo: u32) {
    if n == 0 || destino == 0 || origen == 0 {
        return;
    }
    // SAFETY: `n` ranuras de montones de la casa.
    unsafe { core::ptr::copy(origen as *const u8, destino as *mut u8, (n as u64 * DESCRIPTOR_BYTES) as usize) };
}

/// `CopyDescriptors(this, n_dst, dst, medidas_dst, n_src, src, medidas_src,
/// tipo)`: tramos de ranuras a tramos de ranuras, en orden; una lista de
/// medidas nula = tramos de una.
#[allow(clippy::too_many_arguments)]
extern "win64" fn copy_descriptors(_this: u64, n_dst: u32, dst: *const u64, medidas_dst: *const u32, n_src: u32, src: *const u64, medidas_src: *const u32, _tipo: u32) {
    let ranuras = |n: u32, inicios: *const u64, medidas: *const u32| -> Vec<u64> {
        let mut v = Vec::new();
        for i in 0..n as usize {
            // SAFETY: `n` inicios (y medidas, si no es nulo) del `.exe`.
            let (a, m) = unsafe { (inicios.add(i).read_unaligned(), if medidas.is_null() { 1 } else { medidas.add(i).read_unaligned() }) };
            v.extend((0..m as u64).map(|k| a + k * DESCRIPTOR_BYTES));
        }
        v
    };
    if dst.is_null() || src.is_null() {
        return;
    }
    let (d, s) = (ranuras(n_dst, dst, medidas_dst), ranuras(n_src, src, medidas_src));
    if d.len() != s.len() {
        aviso("CopyDescriptors con tantos destinos como origenes distintos: en Windows es un error");
    }
    for (a, b) in d.iter().zip(&s) {
        // SAFETY: ranuras de montones de la casa (32 B cada una).
        unsafe { core::ptr::copy(*b as *const u8, *a as *mut u8, DESCRIPTOR_BYTES as usize) };
    }
}

/// `GetCustomHeapProperties(this, ret, nodos, tipo)`: lo que es cada tipo en
/// una tarjeta con su propia memoria (la 3060): DEFAULT en L1 sin CPU,
/// UPLOAD combinado en L0, READBACK cacheado en L0.
extern "win64" fn get_custom_heap_properties(_this: u64, ret: *mut u8, _nodos: u32, tipo: u32) -> *mut u8 {
    let (pagina, piscina) = match tipo {
        1 => (1, 2),
        2 => (2, 1),
        3 => (3, 1),
        _ => (0, 0),
    };
    for (o, v) in [(0, 4), (4, pagina), (8, piscina), (12, 1), (16, 1)] {
        poner_u32(ret, o, v);
    }
    ret
}

/// `GetResourceTiling(this, recurso, n_tiles, mips_empaquetadas, forma,
/// n_subrecursos, primero, subrecursos)`: la casa no tiene recursos
/// reservados (anuncia TiledResourcesTier NOT_SUPPORTED), y de uno que no lo
/// es Windows dice cero tiles: todo a cero, y cero subrecursos escritos.
#[allow(clippy::too_many_arguments)]
extern "win64" fn get_resource_tiling(_this: u64, _r: u64, n: *mut u32, mips: *mut u8, forma: *mut u8, n_sub: *mut u32, _primero: u32, _sub: *mut u8) {
    // SAFETY: los punteros del `.exe` que no son nulos: un UINT, un
    // D3D12_PACKED_MIP_INFO (12 B), un D3D12_TILE_SHAPE (12 B) y un UINT.
    unsafe {
        if !n.is_null() {
            n.write_unaligned(0);
        }
        if !mips.is_null() {
            core::ptr::write_bytes(mips, 0, 12);
        }
        if !forma.is_null() {
            core::ptr::write_bytes(forma, 0, 12);
        }
        if !n_sub.is_null() {
            n_sub.write_unaligned(0);
        }
    }
}

/// `GetRaytracingAccelerationStructurePrebuildInfo(this, desc, info)`: sin
/// rayos (RaytracingTier NOT_SUPPORTED), medidas cero, y se dice.
extern "win64" fn get_raytracing_prebuild_info(_this: u64, _d: *const u8, info: *mut u8) {
    aviso("GetRaytracingAccelerationStructurePrebuildInfo: la casa anuncia RaytracingTier NOT_SUPPORTED: medidas cero");
    if !info.is_null() {
        // SAFETY: un D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO (24 B).
        unsafe { core::ptr::write_bytes(info, 0, 24) };
    }
}

/// MakeResident, Evict: todo es residente, siempre.
extern "win64" fn make_resident(_this: u64, _n: u32, _objetos: *const u64) -> i32 {
    S_OK
}

/// SetStablePowerState: los relojes los decide el GSP-RM (`gpu relojes`).
extern "win64" fn set_stable_power_state(_this: u64, _fijar: i32) -> i32 {
    S_OK
}

/// Un monton de consultas: lo que mide cada una y sus resultados.
pub struct Consultas {
    paso: usize,
    datos: Vec<u8>,
}

/// `CreateQueryHeap(this, desc, riid, pp)`: D3D12_QUERY_HEAP_DESC -- Type +0,
/// Count +4, NodeMask +8.
extern "win64" fn create_query_heap(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    if !pide(riid, com::CONSULTAS) {
        return E_NOINTERFACE;
    }
    let (tipo, n) = (u32_de(desc, 0), u32_de(desc, 4) as usize);
    // Lo que mide el resultado de cada tipo (D3D12_QUERY_DATA_*).
    let paso = match tipo {
        0 | 1 | 5 => 8,
        2 => 88,
        3 => 16,
        4 => 32,
        7 => 112,
        _ => return E_INVALIDARG,
    };
    if n == 0 {
        return E_INVALIDARG;
    }
    let vt = vtabla::<{ com::CONSULTAS }>(&[]);
    dar(pp, nuevo(com::CONSULTAS, vt, Consultas { paso, datos: alloc::vec![0; n * paso] }) as u64)
}

/// Una firma de ordenes indirectas: lo que trae (la casa aun no las corre).
pub struct Firma {
    pub paso: u32,
    pub argumentos: Vec<u8>,
}

/// `CreateCommandSignature(this, desc, raiz, riid, pp)`:
/// D3D12_COMMAND_SIGNATURE_DESC -- ByteStride +0, NumArgumentDescs +4,
/// pArgumentDescs +8 (16 B cada uno), NodeMask +16.
extern "win64" fn create_command_signature(_this: u64, desc: *const u8, _raiz: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() {
        return E_INVALIDARG;
    }
    if !pide(riid, com::FIRMA) {
        return E_NOINTERFACE;
    }
    let (paso, n, args) = (u32_de(desc, 0), u32_de(desc, 4) as usize, u64_de(desc, 8));
    if n == 0 || args == 0 {
        return E_INVALIDARG;
    }
    // SAFETY: `n` D3D12_INDIRECT_ARGUMENT_DESC del `.exe`.
    let argumentos = unsafe { core::slice::from_raw_parts(args as *const u8, 16 * n) }.to_vec();
    let vt = vtabla::<{ com::FIRMA }>(&[]);
    dar(pp, nuevo(com::FIRMA, vt, Firma { paso, argumentos }) as u64)
}

// -- La cola -----------------------------------------------------------------

/// SetMarker / BeginEvent (de la cola y de la lista): son para PIX.
extern "win64" fn marcador(_this: u64, _meta: u32, _datos: *const u8, _n: u32) {}

extern "win64" fn fin_de_evento(_this: u64) {}

/// `Wait(this, valla, valor)`: la cola es sincrona; lo que la valla espere
/// lo pondra quien la marque, y la cola no tiene nada pendiente que retener.
extern "win64" fn queue_wait(_this: u64, valla: u64, _valor: u64) -> i32 {
    if valla == 0 {
        return E_INVALIDARG;
    }
    S_OK
}

/// La frecuencia de los sellos de tiempo: nanosegundos, como el contador de
/// rendimiento de la casa (QueryPerformanceFrequency dice lo mismo).
extern "win64" fn get_timestamp_frequency(_this: u64, f: *mut u64) -> i32 {
    if f.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el UINT64 del `.exe`.
    unsafe { f.write_unaligned(1_000_000_000) };
    S_OK
}

/// `GetClockCalibration(this, gpu, cpu)`: el mismo reloj para los dos.
extern "win64" fn get_clock_calibration(_this: u64, gpu: *mut u64, cpu: *mut u64) -> i32 {
    if gpu.is_null() || cpu.is_null() {
        return E_INVALIDARG;
    }
    let t = (crate::plataforma().ahora_ns)();
    // SAFETY: dos UINT64 del `.exe`.
    unsafe {
        gpu.write_unaligned(t);
        cpu.write_unaligned(t);
    }
    S_OK
}

/// `GetDesc(this, ret)` de la cola: su D3D12_COMMAND_QUEUE_DESC.
extern "win64" fn get_desc_cola(this: u64, ret: *mut u8) -> *mut u8 {
    // SAFETY: una Cola de la casa; 16 bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(de::<Cola>(this).desc.as_ptr(), ret, 16) };
    ret
}

/// `GetDesc(this, ret)` del monton de descriptores.
pub(crate) extern "win64" fn get_desc_monton(this: u64, ret: *mut u8) -> *mut u8 {
    // SAFETY: un Monton de la casa; 16 bytes del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(de::<Monton>(this).desc.as_ptr(), ret, 16) };
    ret
}

/// `GetCachedBlob(this, pp)`: un blob propio. Si vuelve en `CachedPSO`, la
/// casa lo ignora y compila otra vez, como un driver que cambio.
pub(crate) extern "win64" fn get_cached_blob(_this: u64, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: el `ID3DBlob **` del `.exe`.
    unsafe { *pp = crate::tuberia::blob(b"BMOX-PSO".to_vec()) };
    S_OK
}

// -- La lista ----------------------------------------------------------------

fn l<'a>(this: u64) -> &'a mut Lista {
    // SAFETY: `this` es una Lista de la casa (lo dice su vtabla).
    unsafe { de::<Lista>(this) }
}

extern "win64" fn get_type(this: u64) -> u32 {
    l(this).tipo
}

/// `ClearState(this, pso)`: el estado como recien creada, con ese PSO.
extern "win64" fn clear_state(this: u64, pso: u64) {
    let pso = if es_computo(pso) { 0 } else { pso };
    l(this).estado = Estado { pso, ..Estado::default() };
}

extern "win64" fn dispatch(_this: u64, _x: u32, _y: u32, _z: u32) {
    aviso("Dispatch: el computo de D3D12 aun no corre en la casa: se salta");
}

/// `(base, bytes)` del bufer `r`, o `None` si no es un bufer de la casa.
fn bufer(r: u64) -> Option<(u64, u64)> {
    if r == 0 {
        return None;
    }
    // SAFETY: un Recurso de la casa.
    let x = unsafe { de::<Recurso>(r) };
    x.bufer.as_ref().map(|b| (b.base(), b.bytes as u64))
}

/// `CopyBufferRegion(this, dst, desde_dst, src, desde_src, n)`.
extern "win64" fn copy_buffer_region(this: u64, dst: u64, desde_dst: u64, src: u64, desde_src: u64, n: u64) {
    match (bufer(dst), bufer(src)) {
        (Some((d, nd)), Some((s, ns))) if desde_dst.checked_add(n).is_some_and(|f| f <= nd) && desde_src.checked_add(n).is_some_and(|f| f <= ns) => {
            l(this).ordenes.push(Orden::Bytes { dst: d + desde_dst, src: s + desde_src, n });
        }
        (Some(_), Some(_)) => aviso("CopyBufferRegion fuera de un bufer: en Windows es un error"),
        _ => aviso("CopyBufferRegion entre algo que no es un bufer de la casa"),
    }
}

pub(crate) extern "win64" fn copy_resource(this: u64, dst: u64, src: u64) {
    if dst != 0 && src != 0 {
        l(this).ordenes.push(Orden::Entero { dst, src });
    }
}

/// `ResolveSubresource(this, dst, sub, src, sub, formato)`: la casa no tiene
/// MSAA (una muestra): resolver es copiar.
extern "win64" fn resolve_subresource(this: u64, dst: u64, _sd: u32, src: u64, _ss: u32, _formato: u32) {
    copy_resource(this, dst, src);
}

/// `OMSetBlendFactor(this, factor[4])` (N5.11): el de los factores
/// BLEND_FACTOR de la mezcla; nulo es (1, 1, 1, 1).
extern "win64" fn om_set_blend_factor(this: u64, f: *const f32) {
    // SAFETY: `this` es una Lista de la casa; `f`, 4 floats del `.exe` o nulo.
    unsafe { crate::d3d12::lista(this).estado.factor_mezcla = (!f.is_null()).then(|| [f.read_unaligned(), f.add(1).read_unaligned(), f.add(2).read_unaligned(), f.add(3).read_unaligned()]) };
}

extern "win64" fn om_set_stencil_ref(_this: u64, _r: u32) {}

extern "win64" fn execute_bundle(_this: u64, _b: u64) {
    aviso("ExecuteBundle: los bundles aun no corren en la casa: se salta");
}

/// Los `SetCompute*`: el computo no corre todavia (Dispatch lo dice), asi
/// que lo que se le da a su raiz no tiene a quien llegar. Cuatro formas.
extern "win64" fn de_computo1(_this: u64, _a: u64) {}
extern "win64" fn de_computo2(_this: u64, _a: u32, _b: u64) {}
extern "win64" fn de_computo3(_this: u64, _a: u32, _b: u32, _c: u32) {}
extern "win64" fn de_computo4(_this: u64, _a: u32, _b: u32, _c: *const u8, _d: u32) {}

/// `SetGraphicsRoot32BitConstant(this, parametro, valor, desde)` (N5.2).
extern "win64" fn root_32bit_constant(this: u64, parametro: u32, valor: u32, desde: u32) {
    constantes_de_raiz(this, parametro, desde, &[valor]);
}

/// `SetGraphicsRoot32BitConstants(this, parametro, n, datos, desde)` (N5.2).
extern "win64" fn root_32bit_constants(this: u64, parametro: u32, n: u32, datos: *const u8, desde: u32) {
    if datos.is_null() || n as usize > crate::cbuffers::PALABRAS {
        aviso("SetGraphicsRoot32BitConstants sin datos (o con mas de 64): se tira");
        return;
    }
    // SAFETY: `n` u32 del `.exe` (sin alinear, por si acaso).
    let v: Vec<u32> = (0..n as usize).map(|k| unsafe { (datos as *const u32).add(k).read_unaligned() }).collect();
    constantes_de_raiz(this, parametro, desde, &v);
}

/// Las constantes, a la raiz de la lista (las guarda `cbuffers::poner`).
fn constantes_de_raiz(this: u64, parametro: u32, desde: u32, valores: &[u32]) {
    // SAFETY: `this` es una Lista de la casa.
    let e = unsafe { &mut crate::d3d12::lista(this).estado };
    if e.raiz == 0 {
        aviso("SetGraphicsRoot32BitConstant(s) sin SetGraphicsRootSignature: en Windows es un error, y se tira");
        return;
    }
    // SAFETY: un RootSignature de la casa.
    let firma = unsafe { &de::<crate::tuberia::RootSignature>(e.raiz).firma };
    if let Err(m) = crate::cbuffers::poner(e, firma, parametro as usize, desde as usize, valores) {
        aviso(m);
    }
}

extern "win64" fn root_descriptor(_this: u64, _parametro: u32, _va: u64) {
    aviso("SetGraphicsRootShaderResourceView/UnorderedAccessView: el sombreador de la casa aun no los ve");
}

extern "win64" fn so_set_targets(_this: u64, _desde: u32, n: u32, _v: *const u8) {
    if n != 0 {
        aviso("SOSetTargets: stream output, no hay");
    }
}

extern "win64" fn clear_uav(_this: u64, _gpu: u64, _cpu: u64, _r: u64, _v: *const u32, _n: u32, _rects: *const u8) {
    aviso("ClearUnorderedAccessView: aun no; el recurso queda como estaba");
}

extern "win64" fn discard_resource(_this: u64, _r: u64, _region: *const u8) {}

extern "win64" fn begin_query(_this: u64, _monton: u64, _tipo: u32, _i: u32) {}

/// `EndQuery(this, monton, tipo, indice)`: se APUNTA; el resultado se pone al
/// ejecutarse (un sello de tiempo, entonces).
extern "win64" fn end_query(this: u64, monton: u64, tipo: u32, i: u32) {
    if monton != 0 {
        l(this).ordenes.push(Orden::Consulta { monton, indice: i, tipo });
    }
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn resolve_query_data(this: u64, monton: u64, _tipo: u32, desde: u32, n: u32, bufer: u64, off: u64) {
    if monton != 0 && bufer != 0 {
        l(this).ordenes.push(Orden::Resolver { monton, desde, n, bufer, off });
    }
}

extern "win64" fn set_predication(_this: u64, bufer: u64, _off: u64, _op: u32) {
    if bufer != 0 {
        aviso("SetPredication: la casa dibuja siempre (sin predicacion)");
    }
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn execute_indirect(_this: u64, _firma: u64, _max: u32, _args: u64, _off: u64, _cuenta: u64, _off_cuenta: u64) {
    aviso("ExecuteIndirect: las ordenes indirectas aun no corren en la casa: se saltan");
}

/// **Lo que corre una orden de la tanda 47** (lo llama `d3d12::ejecutar_listas`).
pub(crate) fn ejecutar(o: &Orden) {
    match *o {
        // SAFETY: tramos comprobados al apuntar, de buferes de la casa (que
        // no se liberan).
        Orden::Bytes { dst, src, n } => unsafe { core::ptr::copy(src as *const u8, dst as *mut u8, n as usize) },
        Orden::Entero { dst, src } => copiar_entero(dst, src),
        Orden::Consulta { monton, indice, tipo } => consulta(monton, indice, tipo),
        Orden::Resolver { monton, desde, n, bufer: b, off } => resolver(monton, desde, n, b, off),
        // SAFETY: comprobado al apuntar: cuatro bytes de un bufer de la casa.
        Orden::Escribir { dst, valor } => unsafe { (dst as *mut u32).write_unaligned(valor) },
        _ => {}
    }
}

fn copiar_entero(dst: u64, src: u64) {
    match (bufer(dst), bufer(src)) {
        (Some((d, nd)), Some((s, ns))) => {
            // SAFETY: dos buferes de la casa; lo que quepa en los dos.
            unsafe { core::ptr::copy(s as *const u8, d as *mut u8, nd.min(ns) as usize) };
        }
        (None, None) => {
            crate::tuberia::aplicar_limpieza(src);
            crate::tuberia::olvidar_limpieza(dst);
            // SAFETY: dos Recursos de la casa (texturas).
            let (d, s) = unsafe { (de::<Recurso>(dst), de::<Recurso>(src)) };
            match (&d.tex, &s.tex) {
                // 02-10: la textura ENTERA (todas sus mips y capas), si se
                // guardan igual: mismas medidas por dentro.
                (Some(td), Some(ts)) if td.almacen.elemento() == ts.almacen.elemento() && td.subs == ts.subs => {
                    let n: u64 = ts.subs.iter().map(|x| x.bytes()).sum();
                    // SAFETY: las dos memorias de texturas de la casa, de `n` bytes.
                    unsafe { core::ptr::copy(ts.datos as *const u8, td.datos as *mut u8, n as usize) };
                }
                _ => aviso("CopyResource entre texturas de otra forma: en Windows es un error"),
            }
        }
        _ => aviso("CopyResource entre un bufer y una imagen: en Windows es un error"),
    }
}

fn consulta(monton: u64, i: u32, tipo: u32) {
    // SAFETY: un monton de consultas de la casa.
    let m = unsafe { de::<Consultas>(monton) };
    let o = i as usize * m.paso;
    let Some(r) = m.datos.get_mut(o..o + m.paso) else {
        aviso("EndQuery fuera de su monton de consultas");
        return;
    };
    r.fill(0);
    let v: u64 = match tipo {
        // OCCLUSION y BINARY: VISIBLE. Un 0 haria que el motor no dibujara
        // lo que no sabe si se ve.
        0 | 1 => 1,
        2 => (crate::plataforma().ahora_ns)(),
        _ => 0,
    };
    r[..8].copy_from_slice(&v.to_le_bytes());
}

fn resolver(monton: u64, desde: u32, n: u32, b: u64, off: u64) {
    // SAFETY: un monton de consultas de la casa.
    let m = unsafe { de::<Consultas>(monton) };
    let (a, z) = (desde as usize * m.paso, (desde as usize + n as usize) * m.paso);
    let Some(datos) = m.datos.get(a..z) else {
        aviso("ResolveQueryData fuera de su monton de consultas");
        return;
    };
    match bufer(b) {
        Some((base, bytes)) if off.checked_add(datos.len() as u64).is_some_and(|f| f <= bytes) => {
            // SAFETY: dentro del bufer de la casa.
            unsafe { core::ptr::copy_nonoverlapping(datos.as_ptr(), (base + off) as *mut u8, datos.len()) };
        }
        _ => aviso("ResolveQueryData a algo que no es un bufer de la casa, o no cabe"),
    }
}

// -- El recurso --------------------------------------------------------------

/// Apuntar en el recurso recien creado en `pp` el tipo de su monton (de un
/// D3D12_HEAP_PROPERTIES, Type +0), si salio.
pub(crate) fn apuntar_monton(r: i32, pp: *mut u64, props: *const u8) {
    if r != S_OK || pp.is_null() || props.is_null() {
        return;
    }
    // SAFETY: el recurso que se acaba de dejar en `pp`.
    unsafe { de::<Recurso>(*pp).tipo_monton = u32_de(props, 0) };
}

/// `GetHeapProperties(this, props, banderas)`.
pub(crate) extern "win64" fn get_heap_properties(this: u64, props: *mut u8, banderas: *mut u32) -> i32 {
    // SAFETY: un Recurso de la casa.
    let tipo = unsafe { de::<Recurso>(this) }.tipo_monton;
    if !props.is_null() {
        for (o, v) in [(0, tipo), (4, 0), (8, 0), (12, 1), (16, 1)] {
            poner_u32(props, o, v);
        }
    }
    if !banderas.is_null() {
        // SAFETY: el D3D12_HEAP_FLAGS del `.exe`.
        unsafe { banderas.write_unaligned(0) };
    }
    S_OK
}

/// La caja de un D3D12_BOX (left, top, front, right, bottom, back), o el
/// recurso entero: `(x, y, ancho, alto)` en pixeles (bytes en un bufer).
fn caja(b: *const u8, ancho: u32, alto: u32) -> Option<(u32, u32, u32, u32)> {
    if b.is_null() {
        return Some((0, 0, ancho, alto));
    }
    let (x0, y0, x1, y1) = (u32_de(b, 0), u32_de(b, 4), u32_de(b, 12), u32_de(b, 16));
    (x0 < x1 && y0 < y1 && x1 <= ancho && y1 <= alto).then(|| (x0, y0, x1 - x0, y1 - y0))
}

/// Copiar entre la memoria del `.exe` (`p`: filas de `paso` bytes,
/// rebanadas de `capa`) y un recurso de la casa. `escribir`: del `.exe` al
/// recurso. Una textura, cualquier subrecurso y caja (02-10; ver
/// `d3d12_texturas::con_el_exe`).
fn filas(this: u64, sub: u32, b: *const u8, p: *mut u8, paso: u32, capa: u32, escribir: bool) -> i32 {
    if p.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un Recurso de la casa.
    let r = unsafe { de::<Recurso>(this) };
    if let Some(bf) = r.bufer.as_ref() {
        let Some((x, _, n, _)) = caja(b, bf.bytes as u32, 1) else { return E_INVALIDARG };
        if sub != 0 {
            return E_INVALIDARG;
        }
        // SAFETY: `n` bytes del `.exe` y del bufer, dentro de los dos.
        unsafe {
            let a = (bf.base() + x as u64) as *mut u8;
            if escribir {
                core::ptr::copy(p, a, n as usize);
            } else {
                core::ptr::copy(a, p, n as usize);
            }
        }
        return S_OK;
    }
    // SAFETY: `p` es memoria del `.exe` que cubre la caja con esos pasos.
    match unsafe { crate::d3d12_texturas::con_el_exe(this, sub, b, p, paso, capa, escribir) } {
        Ok(()) => S_OK,
        Err(m) => {
            aviso(&alloc::format!("WriteToSubresource/ReadFromSubresource: {m}: E_INVALIDARG"));
            E_INVALIDARG
        }
    }
}

/// `WriteToSubresource(this, sub, caja, origen, paso_fila, paso_capa)`.
pub(crate) extern "win64" fn write_to_subresource(this: u64, sub: u32, b: *const u8, origen: *const u8, paso: u32, capa: u32) -> i32 {
    filas(this, sub, b, origen as *mut u8, paso, capa, true)
}

/// `ReadFromSubresource(this, destino, paso_fila, paso_capa, sub, caja)`.
pub(crate) extern "win64" fn read_from_subresource(this: u64, destino: *mut u8, paso: u32, capa: u32, sub: u32, b: *const u8) -> i32 {
    filas(this, sub, b, destino, paso, capa, false)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_huecos_no_se_repiten_y_caben() {
        for (lista, n) in [(&dispositivo()[..], 79), (&cola()[..], 19), (&lista()[..], 60)] {
            let mut h: Vec<usize> = lista.iter().map(|x| x.0).collect();
            h.sort_unstable();
            h.dedup();
            assert_eq!(h.len(), lista.len());
            assert!(h.iter().all(|&k| k > 7 && k < n));
        }
    }

    #[test]
    fn las_propiedades_de_cada_monton() {
        let mut p = [0u8; 20];
        let u = |p: &[u8; 20], o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        get_custom_heap_properties(0, p.as_mut_ptr(), 0, 2);
        assert_eq!((u(&p, 0), u(&p, 4), u(&p, 8)), (4, 2, 1));
        get_custom_heap_properties(0, p.as_mut_ptr(), 0, 1);
        assert_eq!((u(&p, 0), u(&p, 4), u(&p, 8)), (4, 1, 2));
    }
}
