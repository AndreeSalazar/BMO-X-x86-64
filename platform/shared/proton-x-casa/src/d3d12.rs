//! **`d3d12.dll` de la casa** (P3a, 27-09): el dispositivo, la cola, la lista
//! de ordenes, los descriptores, los recursos y la valla -- con la CPU debajo.
//!
//! ```text
//!    D3D12CreateDevice               el dispositivo (sin adaptador: el de la casa)
//!    Device  CreateCommandQueue      una cola, SINCRONA: ExecuteCommandLists
//!            CreateCommandAllocator  corre las ordenes en el acto, asi que al
//!            CreateCommandList       volver ya estan hechas y la valla, cumplida
//!            CreateDescriptorHeap    descriptores de 32 bytes en memoria nuestra
//!            CreateRenderTargetView  el descriptor apunta al recurso
//!            CreateFence
//!            CreateDepthStencilView  (P3c4) como la de RTV: el recurso D32
//!            GetResourceAllocationInfo  lo que la CASA reserva (alineado a
//!                                    64 KiB), no lo que reserva un driver
//!    List    ClearRenderTargetView   se APUNTA; se hace en ExecuteCommandLists
//!            ClearDepthStencilView   (P3c4) igual; el float llega en xmm3
//!    Resource GetDesc                (P3c4) lo que la casa sabe de el
//!    GetCopyableFootprints, CopyTextureRegion  (P3c4) leer un render target
//!                                    desde la CPU: lo que hace --fotograma
//!            ResourceBarrier         nada: en la CPU no hay estado que cambiar
//!            Close, Reset
//!    Queue   ExecuteCommandLists, Signal
//!    Fence   GetCompletedValue, SetEventOnCompletion, Signal
//! ```
//!
//! **Lo que es de P3a y lo que no:** limpiar un blanco y presentarlo. La
//! tuberia (root signature, PSO, buferes, los `Set*` de la lista y los
//! `Draw`) es P3b2 y vive en `tuberia.rs`: aqui solo se cuelga de los huecos.
//! Desde P3b3 los sombreadores CORREN: los DXIL, en la CPU, y la trama de
//! `bmo_proton_x`. La 3060 entra cuando el DXIL pase a SASS (P3b4).
//!
//! **El ABI que no se ve:** los metodos de C++ que DEVUELVEN un struct
//! (`GetCPUDescriptorHandleForHeapStart`) lo hacen, en Windows x64, por un
//! puntero oculto detras de `this`, y devuelven ese puntero. Asi estan aqui.

use alloc::vec;
use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Com, Guid, E_INVALIDARG, E_NOINTERFACE, S_FALSE, S_OK};
use crate::tuberia::{self, Bufer, Estado, Vista};
use crate::{aviso, dir, hilos};

pub const DXGI_FORMAT_R8G8B8A8_UNORM: u32 = 28;
pub const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;

/// Lo que mide un descriptor en un monton de la casa.
const DESCRIPTOR: u64 = 32;

pub struct Dispositivo;
pub struct Cola;
pub struct Asignador;

/// Una orden apuntada en la lista.
enum Orden {
    /// Limpiar un recurso con este pixel (ya en SU formato; en una
    /// profundidad D32, los bits del float).
    Limpiar { recurso: u64, pixel: u32 },
    /// Un dibujo, con el estado de la lista TAL COMO ESTABA al pedirlo. Los
    /// buferes se leen al ejecutarse, como los lee la GPU.
    Dibujar { estado: Estado, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool },
    /// Un render target entero a un bufer (CopyTextureRegion).
    Copiar { rt: u64, bufer: u64, desde: u64, paso: u32 },
    /// Un bufer a una textura entera (CopyTextureRegion al reves: lo que
    /// hace `UpdateSubresources` de d3dx12 para subir una textura).
    Subir { textura: u64, bufer: u64, desde: u64, paso: u32 },
}

pub struct Lista {
    ordenes: Vec<Orden>,
    abierta: bool,
    /// El estado de dibujo: lo que los `Set*` van dejando.
    estado: Estado,
}

pub struct Monton {
    /// Los descriptores: 4 palabras cada uno; la primera, el recurso.
    ranuras: Vec<u64>,
}

/// Una imagen en la memoria de este proceso: lo que un back buffer ES aqui.
pub struct Recurso {
    pub ancho: u32,
    pub alto: u32,
    pub formato: u32,
    pub pixeles: Vec<u32>,
    /// Si es un bufer (CreateCommittedResource), sus bytes; una imagen no.
    pub bufer: Option<Bufer>,
    /// P3b4c.9 Z1: un back buffer de la cadena de intercambio (lo pone DXGI).
    pub cadena: bool,
    /// Z1: lo ultimo que se dibujo en el quedo en la PANTALLA, no en
    /// `pixeles`: su `Present` no copia nada.
    pub en_pantalla: bool,
}

pub struct Valla {
    valor: u64,
    /// `SetEventOnCompletion` de un valor que no ha llegado: (valor, evento).
    /// Desde P4 hay otros hilos que pueden hacer Signal despues.
    pendientes: Vec<(u64, u64)>,
}

/// Poner el valor de una valla y encender los eventos que ya tocan.
fn marcar(v: &mut Valla, valor: u64) {
    v.valor = valor;
    v.pendientes.retain(|&(x, ev)| {
        if x <= valor {
            hilos::encender_evento(ev);
            false
        } else {
            true
        }
    });
}

// -- Crear objetos ----------------------------------------------------------

fn dispositivo() -> u64 {
    let vt = vtabla::<{ com::DEVICE }>(&[
        (7, dir!(get_node_count)),
        (8, dir!(create_command_queue)),
        (9, dir!(create_command_allocator)),
        (10, dir!(tuberia::create_graphics_pipeline_state)),
        (12, dir!(create_command_list)),
        (14, dir!(create_descriptor_heap)),
        (15, dir!(get_descriptor_handle_increment_size)),
        (13, dir!(check_feature_support)),
        (16, dir!(tuberia::create_root_signature)),
        (18, dir!(create_shader_resource_view)),
        (20, dir!(create_render_target_view)),
        (21, dir!(create_render_target_view)),
        (22, dir!(create_sampler)),
        (25, dir!(get_resource_allocation_info)),
        (27, dir!(tuberia::create_committed_resource)),
        (36, dir!(create_fence)),
        (38, dir!(get_copyable_footprints)),
    ]);
    nuevo(com::DEVICE, vt, Dispositivo) as u64
}

/// La vtabla de todo recurso: Map y compania dicen por si mismos si el
/// recurso es un bufer.
fn vtabla_recurso() -> *const u64 {
    vtabla::<{ com::RESOURCE }>(&[(8, dir!(tuberia::map)), (9, dir!(tuberia::unmap)), (10, dir!(get_desc)), (11, dir!(tuberia::get_gpu_virtual_address))])
}

/// Un recurso nuevo (lo pide la cadena de intercambio de DXGI).
pub(crate) fn recurso(ancho: u32, alto: u32, formato: u32) -> u64 {
    let r = nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho, alto, formato, pixeles: vec![0; (ancho * alto) as usize], bufer: None, cadena: false, en_pantalla: false }) as u64;
    // Uno nuevo en la direccion de uno que se fue no hereda su limpieza.
    tuberia::olvidar_limpieza(r);
    r
}

/// Un recurso que es un bufer (CreateCommittedResource).
pub(crate) fn recurso_bufer(b: Bufer) -> u64 {
    nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho: b.bytes as u32, alto: 1, formato: 0, pixeles: Vec::new(), bufer: Some(b), cadena: false, en_pantalla: false }) as u64
}

/// El inicio de un bufer de la casa, o `None` si `this` es una imagen.
pub(crate) fn base_de_bufer(this: u64) -> Option<u64> {
    // SAFETY: `this` es un Recurso de la casa (lo dice su vtabla).
    unsafe { de::<Recurso>(this).bufer.as_ref().map(Bufer::base) }
}

/// `D3D12CreateDevice(adapter, nivel, riid, ppDevice)`. Con `ppDevice` nulo
/// solo pregunta si se podria: S_FALSE, como Windows.
extern "win64" fn d3d12_create_device(_adaptador: u64, _nivel: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return S_FALSE;
    }
    if !pide(riid, com::DEVICE) {
        aviso("D3D12CreateDevice: pide una interfaz de dispositivo que la casa no tiene (ID3D12Device1+?)");
        return E_NOINTERFACE;
    }
    dar(pp, dispositivo())
}

extern "win64" fn get_node_count(_this: u64) -> u32 {
    1
}

extern "win64" fn create_command_queue(_this: u64, _desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::QUEUE) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::QUEUE }>(&[(10, dir!(execute_command_lists)), (14, dir!(queue_signal))]);
    dar(pp, nuevo(com::QUEUE, vt, Cola) as u64)
}

extern "win64" fn create_command_allocator(_this: u64, _tipo: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::ALLOCATOR) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::ALLOCATOR }>(&[(8, dir!(allocator_reset))]);
    dar(pp, nuevo(com::ALLOCATOR, vt, Asignador) as u64)
}

/// `CreateCommandList(this, mascara, tipo, asignador, pso, riid, pp)`: nace
/// ABIERTA, como en Windows.
extern "win64" fn create_command_list(_this: u64, _mascara: u32, _tipo: u32, _asig: u64, pso: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::LIST) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::LIST }>(&[
        (9, dir!(list_close)),
        (10, dir!(list_reset)),
        (12, dir!(draw_instanced)),
        (13, dir!(draw_indexed_instanced)),
        (16, dir!(copy_texture_region)),
        (20, dir!(ia_set_primitive_topology)),
        (21, dir!(rs_set_viewports)),
        (22, dir!(rs_set_scissor_rects)),
        (25, dir!(set_pipeline_state)),
        (26, dir!(resource_barrier)),
        (28, dir!(set_descriptor_heaps)),
        (30, dir!(set_graphics_root_signature)),
        (32, dir!(set_graphics_root_descriptor_table)),
        (38, dir!(set_graphics_root_constant_buffer_view)),
        (43, dir!(ia_set_index_buffer)),
        (44, dir!(ia_set_vertex_buffers)),
        (46, dir!(om_set_render_targets)),
        (47, dir!(proton_x_clear_depth_stencil_view)),
        (48, dir!(clear_render_target_view)),
    ]);
    let estado = Estado { pso, ..Estado::default() };
    dar(pp, nuevo(com::LIST, vt, Lista { ordenes: Vec::new(), abierta: true, estado }) as u64)
}

/// `D3D12_DESCRIPTOR_HEAP_DESC`: Type +0, NumDescriptors +4, Flags +8.
extern "win64" fn create_descriptor_heap(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() || !pide(riid, com::HEAP) {
        return E_NOINTERFACE;
    }
    // SAFETY: un D3D12_DESCRIPTOR_HEAP_DESC del `.exe`.
    let n = unsafe { (desc.add(4) as *const u32).read_unaligned() } as usize;
    let vt = vtabla::<{ com::HEAP }>(&[(9, dir!(get_cpu_descriptor_handle_for_heap_start)), (10, dir!(get_cpu_descriptor_handle_for_heap_start))]);
    dar(pp, nuevo(com::HEAP, vt, Monton { ranuras: vec![0; n.max(1) * (DESCRIPTOR / 8) as usize] }) as u64)
}

extern "win64" fn get_descriptor_handle_increment_size(_this: u64, _tipo: u32) -> u32 {
    DESCRIPTOR as u32
}

/// El `D3D12_CPU_DESCRIPTOR_HANDLE` es un struct de 8 bytes y va en un
/// registro: es la direccion de la ranura, y en ella se deja el recurso.
extern "win64" fn create_render_target_view(_this: u64, recurso: u64, _desc: *const u8, handle: u64) {
    if handle == 0 {
        return;
    }
    // SAFETY: `handle` es la direccion de una ranura de un monton de la casa
    // (GetCPUDescriptorHandleForHeapStart + n * incremento).
    unsafe { (handle as *mut u64).write(recurso) };
}

extern "win64" fn create_fence(_this: u64, inicial: u64, _banderas: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::FENCE) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::FENCE }>(&[
        (8, dir!(get_completed_value)),
        (9, dir!(set_event_on_completion)),
        (10, dir!(fence_signal)),
    ]);
    dar(pp, nuevo(com::FENCE, vt, Valla { valor: inicial, pendientes: Vec::new() }) as u64)
}

// -- El monton de descriptores ---------------------------------------------

/// Devuelve un struct: por el puntero oculto `ret` (ver la cabecera). Es
/// tambien el de GPU (`GetGPUDescriptorHandleForHeapStart`): aqui la "GPU"
/// lee la misma memoria, y la tabla de la raiz apunta a la misma ranura.
extern "win64" fn get_cpu_descriptor_handle_for_heap_start(this: u64, ret: *mut u64) -> *mut u64 {
    // SAFETY: `this` es un Monton de la casa; `ret`, el hueco del `.exe`.
    unsafe {
        let m = de::<Monton>(this);
        ret.write(m.ranuras.as_ptr() as u64);
    }
    ret
}

// -- Las vistas de lectura (texturas) y los muestreadores (29-09) -----------

/// La marca de la palabra 1 de un descriptor: que es.
pub(crate) const DESC_SRV: u64 = 1;
pub(crate) const DESC_MUESTREADOR: u64 = 2;

/// `CreateShaderResourceView(this, recurso, desc, handle)`: en la ranura, el
/// recurso y la marca de SRV. Una textura 2D de un nivel, con el mapeo de
/// componentes de siempre (D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, 0x1688).
extern "win64" fn create_shader_resource_view(_this: u64, recurso: u64, desc: *const u8, handle: u64) {
    if handle == 0 {
        return;
    }
    if !desc.is_null() {
        // SAFETY: un D3D12_SHADER_RESOURCE_VIEW_DESC del `.exe`: Format +0,
        // ViewDimension +4, Shader4ComponentMapping +8, la union +16.
        let (dimension, mapeo) = unsafe { ((desc.add(4) as *const u32).read_unaligned(), (desc.add(8) as *const u32).read_unaligned()) };
        if dimension != 4 {
            aviso("CreateShaderResourceView de algo que no es TEXTURE2D: todavia no");
            return;
        }
        if mapeo != 0x1688 {
            aviso("CreateShaderResourceView con otro mapeo de componentes: se lee el de siempre");
        }
    }
    // SAFETY: la ranura de un monton de la casa: 4 palabras.
    unsafe {
        let r = handle as *mut u64;
        r.write(recurso);
        r.add(1).write(DESC_SRV);
    }
}

/// `CreateSampler(this, desc, handle)`: D3D12_SAMPLER_DESC -- Filter +0,
/// AddressU +4, V +8, W +12, MipLODBias +16, MaxAnisotropy +20,
/// ComparisonFunc +24, BorderColor[4] +28. En la ranura: la marca, filtro y
/// U, V y el borde en 8 bits por canal.
extern "win64" fn create_sampler(_this: u64, desc: *const u8, handle: u64) {
    if handle == 0 || desc.is_null() {
        return;
    }
    // SAFETY: un D3D12_SAMPLER_DESC del `.exe` (52 B) y la ranura de la casa.
    unsafe {
        let w = |o: usize| (desc.add(o) as *const u32).read_unaligned();
        let borde = (0..4).fold(0u64, |a, k| a | ((f32::from_bits(w(28 + 4 * k)).clamp(0.0, 1.0) * 255.0 + 0.5) as u64) << (8 * k));
        let r = handle as *mut u64;
        r.write(0);
        r.add(1).write(DESC_MUESTREADOR);
        r.add(2).write(w(0) as u64 | (w(4) as u64) << 32);
        r.add(3).write(w(8) as u64 | borde << 32);
    }
}

/// `CheckFeatureSupport(this, que, datos, medida)`. De lo que se pregunta
/// en el camino de HelloTexture: D3D12_FEATURE_ROOT_SIGNATURE (12) -- la
/// casa dice 1.0 (lo que lee su root signature), y d3dx12 convierte la 1.1
/// a 1.0. Lo demas, E_INVALIDARG (y se dice).
extern "win64" fn check_feature_support(_this: u64, que: u32, datos: *mut u8, medida: u32) -> i32 {
    const FEATURE_ROOT_SIGNATURE: u32 = 12;
    const ROOT_SIGNATURE_VERSION_1_0: u32 = 1;
    if que == FEATURE_ROOT_SIGNATURE && !datos.is_null() && medida >= 4 {
        // SAFETY: un D3D12_FEATURE_DATA_ROOT_SIGNATURE del `.exe`.
        unsafe { (datos as *mut u32).write_unaligned(ROOT_SIGNATURE_VERSION_1_0) };
        return S_OK;
    }
    aviso(&alloc::format!("ID3D12Device::CheckFeatureSupport({que}): todavia no se contesta"));
    E_INVALIDARG
}

// -- La lista de ordenes ----------------------------------------------------

extern "win64" fn list_close(this: u64) -> i32 {
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { de::<Lista>(this) };
    l.abierta = false;
    S_OK
}

/// `Reset(this, asignador, pso)`: la lista vuelve a nacer, con el estado de
/// dibujo a cero y ese PSO puesto (como en Windows).
extern "win64" fn list_reset(this: u64, _asignador: u64, pso: u64) -> i32 {
    // SAFETY: como arriba.
    let l = unsafe { de::<Lista>(this) };
    l.ordenes.clear();
    l.abierta = true;
    l.estado = Estado { pso, ..Estado::default() };
    S_OK
}

/// La lista de `this`, para los `Set*`.
///
/// # Safety
/// `this` es una Lista de la casa.
unsafe fn lista<'a>(this: u64) -> &'a mut Lista {
    de::<Lista>(this)
}

extern "win64" fn ia_set_primitive_topology(this: u64, topologia: u32) {
    // SAFETY: `this` es una Lista de la casa.
    unsafe { lista(this).estado.topologia = topologia };
}

/// `RSSetViewports(this, n, viewports)`: D3D12_VIEWPORT son 6 floats, por
/// PUNTERO (ningun float por valor: ver lib.rs). Se usa el primero.
extern "win64" fn rs_set_viewports(this: u64, n: u32, v: *const u8) {
    if n == 0 || v.is_null() {
        return;
    }
    if n > 1 {
        aviso("RSSetViewports con mas de uno: se usa el primero");
    }
    // SAFETY: `n` D3D12_VIEWPORT del `.exe`; `this`, una Lista de la casa.
    unsafe { lista(this).estado.viewport = tuberia::viewport_de(v) };
}

/// `RSSetScissorRects(this, n, rects)`: RECT son 4 LONG.
extern "win64" fn rs_set_scissor_rects(this: u64, n: u32, r: *const i32) {
    if n == 0 || r.is_null() {
        return;
    }
    // SAFETY: `n` RECT del `.exe`; `this`, una Lista de la casa.
    unsafe { lista(this).estado.tijera = core::array::from_fn(|k| r.add(k).read_unaligned()) };
}

extern "win64" fn set_pipeline_state(this: u64, pso: u64) {
    // SAFETY: `this` es una Lista de la casa.
    unsafe { lista(this).estado.pso = pso };
}

/// Cambiar de root signature borra lo que se le habia dado a la anterior.
extern "win64" fn set_graphics_root_signature(this: u64, raiz: u64) {
    // SAFETY: `this` es una Lista de la casa.
    let e = unsafe { &mut lista(this).estado };
    if e.raiz != raiz {
        e.cbv = Default::default();
    }
    e.raiz = raiz;
}

/// `SetGraphicsRootConstantBufferView(this, parametro, direccion)`.
extern "win64" fn set_graphics_root_constant_buffer_view(this: u64, parametro: u32, va: u64) {
    // SAFETY: `this` es una Lista de la casa.
    let e = unsafe { &mut lista(this).estado };
    match e.cbv.get_mut(parametro as usize) {
        Some(c) => *c = va,
        None => aviso("SetGraphicsRootConstantBufferView: un parametro mas alla de los que la casa guarda"),
    }
}

/// `D3D12_INDEX_BUFFER_VIEW` (16 B): direccion +0, bytes +8, formato +12.
extern "win64" fn ia_set_index_buffer(this: u64, v: *const u8) {
    // SAFETY: `this` es una Lista de la casa; `v`, una vista del `.exe` o nula.
    unsafe { lista(this).estado.indices = if v.is_null() { Vista::default() } else { vista(v) } };
}

/// `D3D12_VERTEX_BUFFER_VIEW` (16 B): direccion +0, bytes +8, paso +12. La
/// casa dibuja con la ranura 0; las demas se dicen.
extern "win64" fn ia_set_vertex_buffers(this: u64, desde: u32, n: u32, v: *const u8) {
    if desde != 0 || n != 1 {
        aviso("IASetVertexBuffers fuera de la ranura 0: todavia solo una");
    }
    if desde != 0 || n == 0 {
        return;
    }
    // SAFETY: `this` es una Lista de la casa; `v`, `n` vistas del `.exe` o nula.
    unsafe { lista(this).estado.vertices = if v.is_null() { Vista::default() } else { vista(v) } };
}

/// Una vista de bufer (de vertices o de indices: la misma forma).
///
/// # Safety
/// 16 bytes legibles del `.exe`.
unsafe fn vista(v: *const u8) -> Vista {
    Vista {
        va: (v as *const u64).read_unaligned(),
        bytes: (v.add(8) as *const u32).read_unaligned(),
        paso_o_formato: (v.add(12) as *const u32).read_unaligned(),
    }
}

/// `OMSetRenderTargets(this, n, handles, uno_solo, dsv)`: el destino es el
/// recurso del primer descriptor. Con `uno_solo` los n descriptores son
/// consecutivos desde `handles[0]`; sin el, `handles` es un array. Para el
/// primero da lo mismo.
extern "win64" fn om_set_render_targets(this: u64, n: u32, handles: *const u64, _uno_solo: i32, dsv: *const u64) {
    if n > 1 {
        aviso("OMSetRenderTargets con mas de un destino: todavia uno");
    }
    // SAFETY: `this` es una Lista de la casa; `handles` y `dsv`, descriptores
    // del `.exe` (o nulos), y cada uno una ranura de un monton de la casa.
    unsafe {
        let e = &mut lista(this).estado;
        e.rtv = if n == 0 || handles.is_null() { 0 } else { (handles.read_unaligned() as *const u64).read() };
        e.dsv = if dsv.is_null() { 0 } else { (dsv.read_unaligned() as *const u64).read() };
    }
}

extern "win64" fn draw_instanced(this: u64, vertices: u32, instancias: u32, primero: u32, _primera_instancia: u32) {
    dibujar(this, vertices, instancias, primero, 0, false);
}

extern "win64" fn draw_indexed_instanced(this: u64, indices: u32, instancias: u32, primero: u32, base: i32, _primera_instancia: u32) {
    dibujar(this, indices, instancias, primero, base, true);
}

fn dibujar(this: u64, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool) {
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { lista(this) };
    let estado = l.estado.clone();
    l.ordenes.push(Orden::Dibujar { estado, cuantos, instancias, primero, base, indexado });
}

/// En la CPU no hay caches de la GPU que vaciar ni estados de memoria que
/// cambiar: la barrera no tiene nada que hacer, y no es un atajo.
extern "win64" fn resource_barrier(_this: u64, _n: u32, _barreras: *const u8) {}

/// `SetDescriptorHeaps`: aqui la tabla de la raiz ya apunta a la ranura
/// misma (el identificador de GPU es su direccion): no hay nada que atar.
extern "win64" fn set_descriptor_heaps(_this: u64, _n: u32, _montones: *const u64) {}

/// `SetGraphicsRootDescriptorTable(this, parametro, handle GPU)`.
extern "win64" fn set_graphics_root_descriptor_table(this: u64, parametro: u32, handle: u64) {
    // SAFETY: `this` es una Lista de la casa.
    let e = unsafe { &mut lista(this).estado };
    match e.tablas.get_mut(parametro as usize) {
        Some(t) => *t = handle,
        None => aviso("SetGraphicsRootDescriptorTable con un parametro de mas de 16"),
    }
}

/// Un canal de 0.0..1.0 a UNORM8: redondeado al mas cercano, como D3D.
fn unorm8(c: f32) -> u32 {
    let c = if c.is_nan() { 0.0 } else { c.clamp(0.0, 1.0) };
    (c * 255.0 + 0.5) as u32
}

/// `ClearRenderTargetView(this, handle, color[4], n, rects)`. Con rectangulos,
/// todavia no: lo dice.
extern "win64" fn clear_render_target_view(this: u64, handle: u64, color: *const f32, n: u32, _rects: *const u8) {
    if n != 0 {
        aviso("ClearRenderTargetView con rectangulos: todavia limpia solo el recurso entero");
        return;
    }
    if handle == 0 || color.is_null() {
        return;
    }
    // SAFETY: el descriptor es una ranura de la casa (ver arriba); `color`,
    // cuatro floats del `.exe`.
    let (recurso, rgba) = unsafe {
        let c = core::slice::from_raw_parts(color, 4);
        ((handle as *const u64).read(), [unorm8(c[0]), unorm8(c[1]), unorm8(c[2]), unorm8(c[3])])
    };
    if recurso == 0 {
        aviso("ClearRenderTargetView sobre un descriptor sin CreateRenderTargetView");
        return;
    }
    // SAFETY: el descriptor guarda un Recurso de la casa.
    let formato = unsafe { de::<Recurso>(recurso).formato };
    let [r, g, b, a] = rgba;
    // En memoria, R8G8B8A8 es R,G,B,A y B8G8R8A8 es B,G,R,A.
    let pixel = match formato {
        DXGI_FORMAT_B8G8R8A8_UNORM => a << 24 | r << 16 | g << 8 | b,
        _ => a << 24 | b << 16 | g << 8 | r,
    };
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { de::<Lista>(this) };
    l.ordenes.push(Orden::Limpiar { recurso, pixel });
}

// -- La profundidad (P3c4) --------------------------------------------------

// `ClearDepthStencilView(this, handle, banderas, float profundidad, u8
// stencil, n, rects)`: el float llega en xmm3 (Windows x64) y la casa es
// soft-float en Ring 3 (ver lib.rs): sus BITS pasan al cuarto registro
// entero, r9, y se salta a la de verdad. La pila (stencil, n, rects) no se
// toca.
core::arch::global_asm!(
    ".globl proton_x_clear_depth_stencil_view",
    "proton_x_clear_depth_stencil_view:",
    "movd r9d, xmm3",
    "jmp {f}",
    f = sym clear_depth_stencil_view,
);
extern "C" {
    fn proton_x_clear_depth_stencil_view();
}

const CLEAR_FLAG_DEPTH: u32 = 1;

extern "win64" fn clear_depth_stencil_view(this: u64, handle: u64, banderas: u32, bits: u32, _stencil: u8, n: u32, _rects: *const u8) {
    if n != 0 {
        aviso("ClearDepthStencilView con rectangulos: todavia limpia solo el recurso entero");
        return;
    }
    if handle == 0 || banderas & CLEAR_FLAG_DEPTH == 0 {
        return;
    }
    // SAFETY: el descriptor es una ranura de la casa (CreateDepthStencilView).
    let recurso = unsafe { (handle as *const u64).read() };
    if recurso == 0 {
        aviso("ClearDepthStencilView sobre un descriptor sin CreateDepthStencilView");
        return;
    }
    // SAFETY: `this` es una Lista de la casa.
    unsafe { de::<Lista>(this) }.ordenes.push(Orden::Limpiar { recurso, pixel: bits });
}

/// `GetDesc(this, ret)`: el D3D12_RESOURCE_DESC (56 B) por el puntero oculto.
extern "win64" fn get_desc(this: u64, ret: *mut u8) -> *mut u8 {
    // SAFETY: `this` es un Recurso de la casa.
    let r = unsafe { de::<Recurso>(this) };
    let (dimension, layout, banderas, ancho) = match (&r.bufer, r.formato) {
        (Some(b), _) => (1u32, 1u32, 0u32, b.bytes as u64),
        (None, tuberia::FMT_D32_FLOAT) => (3, 0, 2, r.ancho as u64), // ALLOW_DEPTH_STENCIL
        (None, _) => (3, 0, 1, r.ancho as u64),                      // ALLOW_RENDER_TARGET
    };
    // SAFETY: 56 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(ret, 0, 56);
        let u = |o: usize, v: u32| (ret.add(o) as *mut u32).write_unaligned(v);
        u(0, dimension);
        (ret.add(16) as *mut u64).write_unaligned(ancho);
        u(24, r.alto);
        (ret.add(28) as *mut u16).write_unaligned(1);
        (ret.add(30) as *mut u16).write_unaligned(1);
        u(32, r.formato);
        u(36, 1);
        u(44, layout);
        u(48, banderas);
    }
    ret
}

/// `GetResourceAllocationInfo(this, ret, mascara, n, descs)`: lo que la CASA
/// reserva (cada recurso a 64 KiB, 4 bytes por pixel en una textura), en el
/// D3D12_RESOURCE_ALLOCATION_INFO (medida, alineacion) oculto. No son las
/// cifras de un driver (la 3060 da otras: el DICCIONARIO de EPICX).
extern "win64" fn get_resource_allocation_info(_this: u64, ret: *mut u64, _mascara: u32, n: u32, descs: *const u8) -> *mut u64 {
    const ALINEACION: u64 = 65536;
    let mut total = 0u64;
    for i in 0..n as usize {
        // SAFETY: `n` D3D12_RESOURCE_DESC del `.exe` (56 B cada uno).
        let (dimension, ancho, alto) = unsafe {
            let d = descs.add(56 * i);
            ((d as *const u32).read_unaligned(), (d.add(16) as *const u64).read_unaligned(), (d.add(24) as *const u32).read_unaligned())
        };
        let bytes = if dimension == 1 { ancho } else { ancho * alto as u64 * 4 };
        total += bytes.div_ceil(ALINEACION) * ALINEACION;
    }
    // SAFETY: 16 bytes del `.exe`.
    unsafe {
        ret.write_unaligned(total);
        ret.add(1).write_unaligned(ALINEACION);
    }
    ret
}

// -- Leer un render target desde la CPU (P3c4, el --fotograma de BMOX-12) ---

/// Lo que mide una fila de una textura en un bufer: D3D12 la alinea a 256
/// (D3D12_TEXTURE_DATA_PITCH_ALIGNMENT).
const PASO_DE_FILA: u32 = 256;

/// `GetCopyableFootprints(this, desc, primero, n, desde, huellas, filas,
/// bytes_fila, total)`: como queda una textura en un bufer. La casa sabe de
/// un subrecurso 2D de 4 bytes por pixel (RGBA8, BGRA8, D32): cada fila a
/// 256, y el total SIN el relleno de la ultima fila, como D3D12. Lo demas se
/// dice y se da como un bufer de una fila.
extern "win64" fn get_copyable_footprints(_this: u64, desc: *const u8, primero: u32, n: u32, desde: u64, huellas: *mut u8, filas: *mut u32, bytes_fila: *mut u64, total: *mut u64) {
    if desc.is_null() {
        return;
    }
    // SAFETY: un D3D12_RESOURCE_DESC del `.exe` (56 B).
    let (dimension, ancho, alto, formato) = unsafe {
        let u = |o: usize| (desc.add(o) as *const u32).read_unaligned();
        (u(0), (desc.add(16) as *const u64).read_unaligned(), u(24), u(32))
    };
    let textura = dimension == 3 && matches!(formato, DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM | tuberia::FMT_D32_FLOAT);
    if dimension == 3 && !textura {
        aviso("GetCopyableFootprints de una textura que no es de 4 bytes por pixel: todavia no");
    }
    if n > 1 || primero != 0 {
        aviso("GetCopyableFootprints de mas de un subrecurso: todavia solo el 0");
    }
    let (fila, n_filas, paso) = if textura {
        let fila = ancho * 4;
        (fila, alto, fila.div_ceil(PASO_DE_FILA as u64) * PASO_DE_FILA as u64)
    } else {
        (ancho, 1, ancho)
    };
    // SAFETY: los punteros del `.exe` que no son nulos (uno por subrecurso).
    unsafe {
        if !huellas.is_null() {
            core::ptr::write_bytes(huellas, 0, 32);
            (huellas as *mut u64).write_unaligned(desde);
            let u = |o: usize, v: u32| (huellas.add(o) as *mut u32).write_unaligned(v);
            u(8, formato);
            u(12, ancho as u32);
            u(16, n_filas);
            u(20, 1);
            u(24, paso as u32);
        }
        if !filas.is_null() {
            filas.write_unaligned(n_filas);
        }
        if !bytes_fila.is_null() {
            bytes_fila.write_unaligned(fila);
        }
        if !total.is_null() {
            total.write_unaligned(paso * (n_filas as u64 - 1) + fila);
        }
    }
}

/// `CopyTextureRegion(this, destino, x, y, z, origen, caja)`: lo de BMOX-12,
/// un render target entero (SUBRESOURCE_INDEX 0) a un bufer
/// (PLACED_FOOTPRINT). D3D12_TEXTURE_COPY_LOCATION: pResource +0, Type +8, y
/// +16 la huella (Offset, Format, Width, Height, Depth, RowPitch) o el
/// indice. Se APUNTA; se hace en ExecuteCommandLists, como la GPU.
extern "win64" fn copy_texture_region(this: u64, destino: *const u8, x: u32, y: u32, z: u32, origen: *const u8, caja: *const u8) {
    if destino.is_null() || origen.is_null() {
        return;
    }
    // SAFETY: dos D3D12_TEXTURE_COPY_LOCATION del `.exe` (48 B).
    let (bufer, tipo_d, desde, paso, rt, tipo_o, sub) = unsafe {
        let u64_ = |p: *const u8, o: usize| (p.add(o) as *const u64).read_unaligned();
        let u32_ = |p: *const u8, o: usize| (p.add(o) as *const u32).read_unaligned();
        (u64_(destino, 0), u32_(destino, 8), u64_(destino, 16), u32_(destino, 40), u64_(origen, 0), u32_(origen, 8), u32_(origen, 16))
    };
    if !caja.is_null() || (x, y, z) != (0, 0, 0) {
        aviso("CopyTextureRegion con caja o desplazamiento: todavia no");
        return;
    }
    // Un bufer a una textura (subir: UpdateSubresources), o una imagen a un
    // bufer (leer: READBACK).
    if tipo_d == 0 && tipo_o == 1 {
        // SAFETY: las mismas dos D3D12_TEXTURE_COPY_LOCATION, al reves.
        let (textura, sub_d, bufer, desde, paso) = unsafe {
            let u64_ = |p: *const u8, o: usize| (p.add(o) as *const u64).read_unaligned();
            let u32_ = |p: *const u8, o: usize| (p.add(o) as *const u32).read_unaligned();
            (u64_(destino, 0), u32_(destino, 16), u64_(origen, 0), u64_(origen, 16), u32_(origen, 40))
        };
        if sub_d != 0 {
            aviso("CopyTextureRegion a un subrecurso que no es el 0 (mipmaps, arrays): todavia no");
            return;
        }
        // SAFETY: `this` es una Lista de la casa.
        unsafe { de::<Lista>(this) }.ordenes.push(Orden::Subir { textura, bufer, desde, paso });
        return;
    }
    if tipo_d != 1 || tipo_o != 0 || sub != 0 {
        aviso("CopyTextureRegion: una imagen entera (subrecurso 0) a un bufer, o un bufer a una textura; otra cosa, todavia no");
        return;
    }
    // SAFETY: `this` es una Lista de la casa.
    unsafe { de::<Lista>(this) }.ordenes.push(Orden::Copiar { rt, bufer, desde, paso });
}

/// Hacer la copia apuntada: cada fila del render target (sus bytes tal como
/// estan en memoria) en el bufer, a `paso` bytes una de otra.
fn copiar(rt: u64, bufer: u64, desde: u64, paso: u32) {
    // SAFETY: dos Recurso de la casa (lo que un `.exe` da a CopyTextureRegion).
    let (r, b) = unsafe { (de::<Recurso>(rt), de::<Recurso>(bufer)) };
    let Some(destino) = b.bufer.as_ref() else {
        aviso("CopyTextureRegion a algo que no es un bufer");
        return;
    };
    let fila = r.ancho as u64 * 4;
    if r.bufer.is_some() || (paso as u64) < fila || desde + paso as u64 * (r.alto as u64).saturating_sub(1) + fila > destino.bytes as u64 {
        aviso("CopyTextureRegion: la huella no cabe en el bufer (o el origen no es una imagen)");
        return;
    }
    tuberia::aplicar_limpieza(rt);
    for (y, px) in r.pixeles.chunks_exact(r.ancho as usize).enumerate() {
        let o = destino.base() + desde + y as u64 * paso as u64;
        for (k, p) in px.iter().enumerate() {
            // SAFETY: dentro del bufer de la casa: se comprobo arriba.
            unsafe { ((o + 4 * k as u64) as *mut u32).write_unaligned(*p) };
        }
    }
}

/// Hacer la subida apuntada: cada fila del bufer (a `paso` bytes una de
/// otra) a la textura, tal cual.
fn subir(textura: u64, bufer: u64, desde: u64, paso: u32) {
    // SAFETY: dos Recurso de la casa (lo que un `.exe` da a CopyTextureRegion).
    let (t, b) = unsafe { (de::<Recurso>(textura), de::<Recurso>(bufer)) };
    let Some(origen) = b.bufer.as_ref() else {
        aviso("CopyTextureRegion desde algo que no es un bufer");
        return;
    };
    let fila = t.ancho as u64 * 4;
    if t.bufer.is_some() || (paso as u64) < fila || desde + paso as u64 * (t.alto as u64).saturating_sub(1) + fila > origen.bytes as u64 {
        aviso("CopyTextureRegion: la huella no cabe en el bufer (o el destino no es una textura)");
        return;
    }
    tuberia::olvidar_limpieza(textura);
    for (y, px) in t.pixeles.chunks_exact_mut(t.ancho as usize).enumerate() {
        let o = origen.base() + desde + y as u64 * paso as u64;
        for (k, p) in px.iter_mut().enumerate() {
            // SAFETY: dentro del bufer de la casa: se comprobo arriba.
            *p = unsafe { ((o + 4 * k as u64) as *const u32).read_unaligned() };
        }
    }
}

// -- La cola, el asignador y la valla ---------------------------------------

/// `ExecuteCommandLists(this, n, listas)`: en el acto, en orden.
extern "win64" fn execute_command_lists(_this: u64, n: u32, listas: *const u64) {
    let empezo = (crate::plataforma().ahora_ns)();
    ejecutar_listas(n, listas);
    crate::dxgi::dibujado((crate::plataforma().ahora_ns)().saturating_sub(empezo));
}

fn ejecutar_listas(n: u32, listas: *const u64) {
    for i in 0..n as usize {
        // SAFETY: `n` punteros a listas de la casa.
        let l = unsafe { de::<Lista>(listas.add(i).read()) };
        if l.abierta {
            aviso("ExecuteCommandLists con una lista sin Close: en Windows es un error, y no se corre");
            continue;
        }
        for o in &l.ordenes {
            match o {
                // ** P3b4c: la limpieza se APUNTA, no se hace: la hace quien
                // dibuje (la 3060 en su dibujo; la CPU al empezar el suyo), o
                // quien lea los pixeles antes (Present, CopyTextureRegion).
                // Asi, con la 3060 dibujando, la CPU no llena 3,6 MB por
                // limpieza y por fotograma.
                Orden::Limpiar { recurso, pixel } => tuberia::limpieza_pendiente(*recurso, *pixel),
                Orden::Dibujar { estado, cuantos, instancias, primero, base, indexado } => {
                    tuberia::ejecutar_dibujo(estado, *cuantos, *instancias, *primero, *base, *indexado);
                }
                Orden::Copiar { rt, bufer, desde, paso } => copiar(*rt, *bufer, *desde, *paso),
                Orden::Subir { textura, bufer, desde, paso } => subir(*textura, *bufer, *desde, *paso),
            }
        }
    }
}

/// La cola es sincrona: cuando se pide `Signal`, todo lo anterior YA termino.
extern "win64" fn queue_signal(_this: u64, valla: u64, valor: u64) -> i32 {
    // SAFETY: una Valla de la casa.
    marcar(unsafe { de::<Valla>(valla) }, valor);
    S_OK
}

extern "win64" fn allocator_reset(_this: u64) -> i32 {
    S_OK
}

extern "win64" fn get_completed_value(this: u64) -> u64 {
    // SAFETY: una Valla de la casa.
    unsafe { de::<Valla>(this).valor }
}

extern "win64" fn fence_signal(this: u64, valor: u64) -> i32 {
    // SAFETY: como arriba.
    marcar(unsafe { de::<Valla>(this) }, valor);
    S_OK
}

/// `SetEventOnCompletion(this, valor, evento)`: si ya se llego, el evento se
/// enciende YA; si no, cuando un Signal (de la cola o de otro hilo) llegue.
/// Con evento nulo, Windows ESPERA ahi mismo: aqui se cede el turno hasta que
/// llegue (o hasta el bloqueo mutuo, que se dice).
extern "win64" fn set_event_on_completion(this: u64, valor: u64, evento: u64) -> i32 {
    // SAFETY: como arriba.
    let v = unsafe { de::<Valla>(this) };
    if v.valor >= valor {
        if evento != 0 {
            hilos::encender_evento(evento);
        }
        return S_OK;
    }
    if evento == 0 {
        aviso("SetEventOnCompletion sin evento sobre un valor que no ha llegado: todavia no");
        return crate::com::E_FAIL;
    }
    v.pendientes.push((valor, evento));
    S_OK
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "D3D12CreateDevice" => dir!(d3d12_create_device),
        _ => return None,
    })
}

/// El recurso de un objeto de la casa (para DXGI).
///
/// # Safety
/// `p` es un Recurso de la casa.
pub(crate) unsafe fn recurso_de<'a>(p: u64) -> &'a Recurso {
    &(*(p as *const Com<Recurso>)).t
}
