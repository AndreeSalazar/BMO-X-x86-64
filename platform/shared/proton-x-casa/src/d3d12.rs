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
//! **Lo que es de P3a y lo que no:** limpiar un blanco y presentarlo. El
//! dibujo con sombreadores (CreateRootSignature, CreateGraphicsPipelineState,
//! DrawInstanced...) es P3b, y hoy cada uno de esos huecos dice su nombre y
//! sale (ver `com.rs`). El backend es la CPU; la 3060 entra por VERRANO cuando
//! haya algo que dibujar que no sea un color.
//!
//! **El ABI que no se ve:** los metodos de C++ que DEVUELVEN un struct
//! (`GetCPUDescriptorHandleForHeapStart`) lo hacen, en Windows x64, por un
//! puntero oculto detras de `this`, y devuelven ese puntero. Asi estan aqui.

use alloc::vec;
use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Com, Guid, E_INVALIDARG, E_NOINTERFACE, S_FALSE, S_OK};
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
}

pub struct Lista {
    ordenes: Vec<Orden>,
    abierta: bool,
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
        (12, dir!(create_command_list)),
        (14, dir!(create_descriptor_heap)),
        (15, dir!(get_descriptor_handle_increment_size)),
        (20, dir!(create_render_target_view)),
        (36, dir!(create_fence)),
    ]);
    nuevo(com::DEVICE, vt, Dispositivo) as u64
}

/// Un recurso nuevo (lo pide la cadena de intercambio de DXGI).
pub(crate) fn recurso(ancho: u32, alto: u32, formato: u32) -> u64 {
    let vt = vtabla::<{ com::RESOURCE }>(&[]);
    nuevo(com::RESOURCE, vt, Recurso { ancho, alto, formato, pixeles: vec![0; (ancho * alto) as usize] }) as u64
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
    if pso != 0 {
        aviso("CreateCommandList con un PSO: los estados de tuberia son P3b");
        return E_INVALIDARG;
    }
    let vt = vtabla::<{ com::LIST }>(&[
        (9, dir!(list_close)),
        (10, dir!(list_reset)),
        (26, dir!(resource_barrier)),
        (48, dir!(clear_render_target_view)),
    ]);
    dar(pp, nuevo(com::LIST, vt, Lista { ordenes: Vec::new(), abierta: true }) as u64)
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

extern "win64" fn list_reset(this: u64, _asignador: u64, pso: u64) -> i32 {
    if pso != 0 {
        aviso("ID3D12GraphicsCommandList::Reset con un PSO: los estados de tuberia son P3b");
        return E_INVALIDARG;
    }
    // SAFETY: como arriba.
    let l = unsafe { de::<Lista>(this) };
    l.ordenes.clear();
    l.abierta = true;
    S_OK
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
            match *o {
                Orden::Limpiar { recurso, pixel } => {
                    // SAFETY: un Recurso de la casa.
                    unsafe { de::<Recurso>(recurso).pixeles.fill(pixel) };
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
