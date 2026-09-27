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
//!    List    ClearRenderTargetView   se APUNTA; se hace en ExecuteCommandLists
//!            ResourceBarrier         nada: en la CPU no hay estado que cambiar
//!            Close, Reset
//!    Queue   ExecuteCommandLists, Signal
//!    Fence   GetCompletedValue, SetEventOnCompletion, Signal
//! ```
//!
//! **Lo que es de P3a y lo que no:** limpiar un blanco y presentarlo. La
//! tuberia (root signature, PSO, buferes, los `Set*` de la lista y los
//! `Draw`) es P3b2 y vive en `tuberia.rs`: aqui solo se cuelga de los huecos.
//! Los sombreadores todavia no corren (P3b3). El backend es la CPU; la 3060
//! entra por VERRANO cuando haya algo que dibujar que no sea un color.
//!
//! **El ABI que no se ve:** los metodos de C++ que DEVUELVEN un struct
//! (`GetCPUDescriptorHandleForHeapStart`) lo hacen, en Windows x64, por un
//! puntero oculto detras de `this`, y devuelven ese puntero. Asi estan aqui.

use alloc::vec;
use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Com, Guid, E_NOINTERFACE, S_FALSE, S_OK};
use crate::tuberia::{self, Bufer, Estado, Vista};
use crate::{aviso, dir, kernel32};

pub const DXGI_FORMAT_R8G8B8A8_UNORM: u32 = 28;
pub const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;

/// Lo que mide un descriptor en un monton de la casa.
const DESCRIPTOR: u64 = 32;

pub struct Dispositivo;
pub struct Cola;
pub struct Asignador;

/// Una orden apuntada en la lista.
enum Orden {
    /// Limpiar un recurso con este pixel (ya en SU formato).
    Limpiar { recurso: u64, pixel: u32 },
    /// Un dibujo, con el estado de la lista TAL COMO ESTABA al pedirlo. Los
    /// buferes se leen al ejecutarse, como los lee la GPU.
    Dibujar { estado: Estado, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool },
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
}

pub struct Valla {
    valor: u64,
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
        (16, dir!(tuberia::create_root_signature)),
        (20, dir!(create_render_target_view)),
        (27, dir!(tuberia::create_committed_resource)),
        (36, dir!(create_fence)),
    ]);
    nuevo(com::DEVICE, vt, Dispositivo) as u64
}

/// La vtabla de todo recurso: Map y compania dicen por si mismos si el
/// recurso es un bufer.
fn vtabla_recurso() -> *const u64 {
    vtabla::<{ com::RESOURCE }>(&[(8, dir!(tuberia::map)), (9, dir!(tuberia::unmap)), (11, dir!(tuberia::get_gpu_virtual_address))])
}

/// Un recurso nuevo (lo pide la cadena de intercambio de DXGI).
pub(crate) fn recurso(ancho: u32, alto: u32, formato: u32) -> u64 {
    nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho, alto, formato, pixeles: vec![0; (ancho * alto) as usize], bufer: None }) as u64
}

/// Un recurso que es un bufer (CreateCommittedResource).
pub(crate) fn recurso_bufer(b: Bufer) -> u64 {
    nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho: b.bytes as u32, alto: 1, formato: 0, pixeles: Vec::new(), bufer: Some(b) }) as u64
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
        (20, dir!(ia_set_primitive_topology)),
        (21, dir!(rs_set_viewports)),
        (22, dir!(rs_set_scissor_rects)),
        (25, dir!(set_pipeline_state)),
        (26, dir!(resource_barrier)),
        (30, dir!(set_graphics_root_signature)),
        (38, dir!(set_graphics_root_constant_buffer_view)),
        (43, dir!(ia_set_index_buffer)),
        (44, dir!(ia_set_vertex_buffers)),
        (46, dir!(om_set_render_targets)),
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
    let vt = vtabla::<{ com::HEAP }>(&[(9, dir!(get_cpu_descriptor_handle_for_heap_start))]);
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
    dar(pp, nuevo(com::FENCE, vt, Valla { valor: inicial }) as u64)
}

// -- El monton de descriptores ---------------------------------------------

/// Devuelve un struct: por el puntero oculto `ret` (ver la cabecera).
extern "win64" fn get_cpu_descriptor_handle_for_heap_start(this: u64, ret: *mut u64) -> *mut u64 {
    // SAFETY: `this` es un Monton de la casa; `ret`, el hueco del `.exe`.
    unsafe {
        let m = de::<Monton>(this);
        ret.write(m.ranuras.as_ptr() as u64);
    }
    ret
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
    if !dsv.is_null() {
        aviso("OMSetRenderTargets con profundidad: se ignora todavia");
    }
    // SAFETY: `this` es una Lista de la casa; `handles`, descriptores del
    // `.exe`, y cada uno una ranura de un monton de la casa.
    unsafe {
        lista(this).estado.rtv = if n == 0 || handles.is_null() { 0 } else { (handles.read_unaligned() as *const u64).read() };
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

// -- La cola, el asignador y la valla ---------------------------------------

/// `ExecuteCommandLists(this, n, listas)`: en el acto, en orden.
extern "win64" fn execute_command_lists(_this: u64, n: u32, listas: *const u64) {
    for i in 0..n as usize {
        // SAFETY: `n` punteros a listas de la casa.
        let l = unsafe { de::<Lista>(listas.add(i).read()) };
        if l.abierta {
            aviso("ExecuteCommandLists con una lista sin Close: en Windows es un error, y no se corre");
            continue;
        }
        for o in &l.ordenes {
            match o {
                Orden::Limpiar { recurso, pixel } => {
                    // SAFETY: un Recurso de la casa.
                    unsafe { de::<Recurso>(*recurso).pixeles.fill(*pixel) };
                }
                Orden::Dibujar { estado, cuantos, instancias, primero, base, indexado } => {
                    tuberia::ejecutar_dibujo(estado, *cuantos, *instancias, *primero, *base, *indexado);
                }
            }
        }
    }
}

/// La cola es sincrona: cuando se pide `Signal`, todo lo anterior YA termino.
extern "win64" fn queue_signal(_this: u64, valla: u64, valor: u64) -> i32 {
    // SAFETY: una Valla de la casa.
    unsafe { de::<Valla>(valla).valor = valor };
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
    unsafe { de::<Valla>(this).valor = valor };
    S_OK
}

/// `SetEventOnCompletion(this, valor, evento)`: si ya se llego, el evento se
/// enciende YA. Si no, en una cola sincrona no va a llegar nunca: se dice.
extern "win64" fn set_event_on_completion(this: u64, valor: u64, evento: u64) -> i32 {
    // SAFETY: como arriba.
    let hecho = unsafe { de::<Valla>(this).valor } >= valor;
    if !hecho {
        aviso("SetEventOnCompletion: la valla espera un valor que nadie ha pedido todavia");
        return crate::com::E_FAIL;
    }
    kernel32::encender_evento(evento);
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
