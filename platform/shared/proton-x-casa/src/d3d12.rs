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

use alloc::vec::Vec;

use crate::com::{self, dar, de, nuevo, pide, vtabla, Com, Guid, E_NOINTERFACE, E_OUTOFMEMORY, S_FALSE, S_OK};
use crate::d3d12_dispositivos as dv;
use crate::subrecursos::{self as sr, Almacen, Forma, Sub};
use crate::tuberia::{self, Bufer, Estado, Vista};
use crate::{aviso, dir, hilos};

pub const DXGI_FORMAT_R8G8B8A8_UNORM: u32 = 28;
pub const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;

/// Lo que mide un descriptor en un monton de la casa.
const DESCRIPTOR: u64 = 32;

/// El dispositivo: si se quito (`RemoveDevice`, Device5).
pub struct Dispositivo {
    quitado: bool,
}
/// Una cola: su D3D12_COMMAND_QUEUE_DESC (Type, Priority, Flags, NodeMask).
pub struct Cola {
    pub(crate) desc: [u8; 16],
}
pub struct Asignador;

/// Una orden apuntada en la lista.
pub(crate) enum Orden {
    /// Limpiar un recurso con este pixel (ya en SU formato; en una
    /// profundidad, los bits del float). `sub`: el subrecurso de la vista y
    /// su rebanada 3D << 32 (ver `d3d12_vistas`); 0, el de siempre.
    Limpiar { recurso: u64, sub: u64, pixel: u32 },
    /// 05-10: ClearDepthStencilView con CLEAR_FLAG_STENCIL: el plano de
    /// stencil de la vista, a `valor` (ver `d3d12_stencil`).
    LimpiarStencil { recurso: u64, sub: u64, valor: u8 },
    /// N5.16 (05-10): limpiar un render target de FLOAT (cuatro palabras
    /// por texel, ya cuantizadas a su formato). Se hace al ejecutarse, no se
    /// apunta como la de arriba: esa guarda UNA palabra.
    LimpiarTexel { recurso: u64, sub: u64, texel: [u32; 4] },
    /// Un dibujo, con el estado de la lista TAL COMO ESTABA al pedirlo. Los
    /// buferes se leen al ejecutarse, como los lee la GPU.
    Dibujar { estado: Estado, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool, primera_instancia: u32 },
    /// Una copia de CopyTextureRegion (02-10: cualquier subrecurso, con su
    /// caja; ver `d3d12_texturas`): de una textura a un bufer (leer), de un
    /// bufer a una textura (subir: `UpdateSubresources` de d3dx12), o entre
    /// dos texturas.
    Region(crate::d3d12_texturas::Region),
    /// Tanda 47 (ver `d3d12_resto`): `n` bytes de `src` a `dst`
    /// (CopyBufferRegion), un recurso entero en otro (CopyResource), el fin
    /// de una consulta (EndQuery) y sus resultados a un bufer
    /// (ResolveQueryData).
    Bytes { dst: u64, src: u64, n: u64 },
    Entero { dst: u64, src: u64 },
    Consulta { monton: u64, indice: u32, tipo: u32 },
    /// E2.7 (05-10, ver `consultas`): el BeginQuery de una consulta de
    /// oclusion, y SetPredication (`dir`: el u64 que mira, 0 sin
    /// predicacion; `op`: 0 EQUAL_ZERO, 1 NOT_EQUAL_ZERO).
    Empezar { monton: u64, indice: u32 },
    Predicar { dir: u64, op: u32 },
    /// Como `Bytes`, de AtomicCopyBufferUINT(64): la predicacion no la salta.
    Atomica { dst: u64, src: u64, n: u64 },
    /// N5.3c (05-10): ClearUnorderedAccessViewUint (`crudo`) o Float, con
    /// la ranura de la vista copiada al apuntarla (`d3d12_resto::limpiar_uav`).
    LimpiarUav { ranura: [u64; 4], valores: [u32; 4], crudo: bool },
    Resolver { monton: u64, desde: u32, n: u32, bufer: u64, off: u64 },
    /// Tanda 48: escribir un `u32` en una direccion de un bufer de la casa
    /// (WriteBufferImmediate).
    Escribir { dst: u64, valor: u32 },
    /// N5.5 (05-10): un `Dispatch(x, y, z)`, con el estado de COMPUTO de la
    /// lista tal como estaba al pedirlo (ver `computo.rs`).
    Despachar { estado: Estado, grupos: [u32; 3] },
    /// E2.4 (05-10): un `ExecuteIndirect`, con el estado de dibujo (o el de
    /// computo, si su firma despacha) tal como estaba; sus argumentos y su
    /// cuenta se LEEN al ejecutarse: los escribe el computo de antes.
    Indirecto { estado: Estado, firma: u64, max: u32, args: u64, args_off: u64, cuenta: u64, cuenta_off: u64 },
}

pub struct Lista {
    pub(crate) ordenes: Vec<Orden>,
    pub(crate) abierta: bool,
    /// El estado de dibujo: lo que los `Set*` van dejando.
    pub(crate) estado: Estado,
    /// D3D12_COMMAND_LIST_TYPE (GetType, tanda 47).
    pub(crate) tipo: u32,
    /// N5.5 (05-10): el estado de COMPUTO, aparte del de dibujo como en
    /// D3D12: su PSO, su root signature y lo que se le dio (`SetCompute*`).
    pub(crate) computo: Estado,
}

pub struct Monton {
    /// Los descriptores: 4 palabras cada uno; la primera, el recurso. Su
    /// DIRECCION en la memoria del proceso (`memoria::pedir_bufer`), no un
    /// `Vec` del monton del cargador (48 MiB, solo avanza): Cyberpunk pide
    /// uno de 1.000.000 de descriptores, el tope de D3D12 (32 MB), y el
    /// cargador entraba en panico (metal 02-10).
    pub(crate) ranuras: u64,
    /// Su D3D12_DESCRIPTOR_HEAP_DESC (GetDesc, tanda 47).
    pub(crate) desc: [u8; 16],
}

/// **Los pixeles de una imagen** (tanda 45, 02-10): `n` palabras en memoria
/// del PROCESO (`memoria::pedir_pixeles`), que se usan como un `[u32]`.
/// Antes eran un `Vec<u32>` del monton del cargador, que mide 48 MiB y solo
/// avanza: un juego a 1080p (8 MiB cada render target) no cabe. No se
/// devuelven: ver `com::release`.
pub struct Pixeles {
    p: *mut u32,
    n: usize,
}

impl Pixeles {
    /// Los de algo que no es una imagen (un bufer, o una textura de bloques).
    fn ninguno() -> Self {
        Pixeles { p: core::ptr::NonNull::dangling().as_ptr(), n: 0 }
    }

    /// Los `n` primeros de la memoria de una textura, en `p` (a PAGINA).
    fn sobre(p: u64, n: usize) -> Self {
        Pixeles { p: p as *mut u32, n }
    }
}

impl core::ops::Deref for Pixeles {
    type Target = [u32];
    fn deref(&self) -> &[u32] {
        // SAFETY: `n` palabras de este proceso, a PAGINA, que no se sueltan
        // (o ninguna, con un puntero no nulo y alineado).
        unsafe { core::slice::from_raw_parts(self.p, self.n) }
    }
}

impl core::ops::DerefMut for Pixeles {
    fn deref_mut(&mut self) -> &mut [u32] {
        // SAFETY: como arriba; el Recurso es su unico propietario.
        unsafe { core::slice::from_raw_parts_mut(self.p, self.n) }
    }
}

/// Una imagen en la memoria de este proceso: lo que un back buffer ES aqui.
pub struct Recurso {
    pub ancho: u32,
    pub alto: u32,
    pub formato: u32,
    pub pixeles: Pixeles,
    /// Si es un bufer (CreateCommittedResource), sus bytes; una imagen no.
    pub bufer: Option<Bufer>,
    /// P3b4c.9 Z1: un back buffer de la cadena de intercambio (lo pone DXGI).
    pub cadena: bool,
    /// Z1: lo ultimo que se dibujo en el quedo en la PANTALLA, no en
    /// `pixeles`: su `Present` no copia nada.
    pub en_pantalla: bool,
    /// D3D12_HEAP_TYPE de su memoria (GetHeapProperties, tanda 47): 1
    /// DEFAULT, salvo lo que diga quien lo crea.
    pub tipo_monton: u32,
    /// 02-10: si es una textura, TODA ella (sus mips y capas); `pixeles` es
    /// la vista de su subrecurso 0 (lo que dibuja y presenta la casa).
    pub tex: Option<Tex>,
}

/// **Una textura entera** (02-10): su forma, donde va cada subrecurso
/// dentro de `datos` (ver `subrecursos`: los BC tal cual, lo demas a 4
/// bytes por texel), como se guarda, y las D3D12_RESOURCE_FLAGS con que se
/// creo (GetDesc).
pub struct Tex {
    pub forma: Forma,
    pub subs: Vec<Sub>,
    pub datos: u64,
    pub almacen: Almacen,
    pub banderas: u32,
}

impl Tex {
    /// El subrecurso `i` como palabras de 4 bytes (texeles, o bloques de 8 o
    /// 16 bytes: van a 8 dentro de `datos`, que empieza a pagina).
    pub fn palabras(&self, i: u32) -> Option<&'static [u32]> {
        let s = self.subs.get(i as usize)?;
        // SAFETY: `datos` es memoria de este proceso que no se suelta (ver
        // `Pixeles`), y el subrecurso cae dentro.
        Some(unsafe { core::slice::from_raw_parts((self.datos + s.desde) as *const u32, (s.bytes() / 4) as usize) })
    }
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
    let mut m = alloc::vec![
        (7, dir!(get_node_count)),
        (8, dir!(create_command_queue)),
        (9, dir!(create_command_allocator)),
        (10, dir!(tuberia::create_graphics_pipeline_state)),
        (12, dir!(create_command_list)),
        (14, dir!(create_descriptor_heap)),
        (15, dir!(get_descriptor_handle_increment_size)),
        (13, dir!(crate::d3d12_capacidades::check_feature_support)),
        (16, dir!(tuberia::create_root_signature)),
        (18, dir!(crate::d3d12_vistas::create_shader_resource_view)),
        (20, dir!(crate::d3d12_vistas::create_render_target_view)),
        (21, dir!(crate::d3d12_vistas::create_depth_stencil_view)),
        (22, dir!(create_sampler)),
        (25, dir!(get_resource_allocation_info)),
        (27, dir!(tuberia::create_committed_resource)),
        (28, dir!(crate::d3d12_montones::create_heap)),
        (29, dir!(crate::d3d12_montones::create_placed_resource)),
        (36, dir!(create_fence)),
        (37, dir!(get_device_removed_reason)),
        (38, dir!(get_copyable_footprints)),
        (43, dir!(get_adapter_luid)),
        // ID3D12Device1 a 10 (tanda 45): ver d3d12_dispositivos.rs.
        (44, dir!(dv::create_pipeline_library)),
        (45, dir!(dv::set_event_on_multiple_fence_completion)),
        (46, dir!(dv::set_residency_priority)),
        (47, dir!(dv::create_pipeline_state)),
        (48, dir!(crate::d3d12_montones::open_existing_heap_from_address)),
        (50, dir!(dv::enqueue_make_resident)),
        (51, dir!(dv::create_command_list1)),
        (53, dir!(dv::create_committed_resource1)),
        (54, dir!(dv::create_heap1)),
        (56, dir!(dv::get_resource_allocation_info1)),
        (58, dir!(dv::remove_device)),
        (59, dir!(dv::enumerate_meta_commands)),
        (64, dir!(dv::check_driver_matching_identifier)),
        (65, dir!(dv::set_background_processing_mode)),
        (68, dir!(dv::get_resource_allocation_info2)),
        (69, dir!(dv::create_committed_resource2)),
        (70, dir!(dv::create_placed_resource1)),
        (72, dir!(dv::get_copyable_footprints1)),
        (73, dir!(dv::create_shader_cache_session)),
        (74, dir!(dv::shader_cache_control)),
        (75, dir!(dv::create_command_queue1)),
        (76, dir!(dv::create_committed_resource3)),
        (77, dir!(dv::create_placed_resource2)),
    ];
    m.extend_from_slice(&crate::d3d12_resto::dispositivo());
    let vt = vtabla::<{ com::DEVICE }>(&m);
    nuevo(com::DEVICE, vt, Dispositivo { quitado: false }) as u64
}

/// La vtabla de todo recurso: Map y compania dicen por si mismos si el
/// recurso es un bufer.
fn vtabla_recurso() -> *const u64 {
    vtabla::<{ com::RESOURCE }>(&[
        (8, dir!(tuberia::map)),
        (9, dir!(tuberia::unmap)),
        (10, dir!(get_desc)),
        (11, dir!(tuberia::get_gpu_virtual_address)),
        (12, dir!(crate::d3d12_resto::write_to_subresource)),
        (13, dir!(crate::d3d12_resto::read_from_subresource)),
        (14, dir!(crate::d3d12_resto::get_heap_properties)),
        (16, dir!(get_desc1)),
    ])
}

/// **Una imagen nueva**: un back buffer de la cadena de intercambio
/// (`cadena`) o un render target de un nivel. `None` si no hay memoria.
pub(crate) fn recurso(ancho: u32, alto: u32, formato: u32, cadena: bool) -> Option<u64> {
    let banderas = if Almacen::de(formato) == Almacen::Flotante { BANDERA_PROFUNDIDAD } else { BANDERA_RT };
    recurso_forma(Forma::plana(ancho, alto, formato), cadena, banderas)
}

/// D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET y ALLOW_DEPTH_STENCIL.
pub(crate) const BANDERA_RT: u32 = 1;
pub(crate) const BANDERA_PROFUNDIDAD: u32 = 2;

/// **Una textura nueva, de cualquier forma** (02-10): todos sus
/// subrecursos en una memoria del proceso, a cero. Va a memoria que la 3060
/// sabe usar (ver `memoria::pedir_pixeles`) lo que ella usaria hoy: la
/// cadena y las texturas de color chicas de un subrecurso; una
/// profundidad, un BC o una con mips, no.
pub(crate) fn recurso_forma(forma: Forma, cadena: bool, banderas: u32) -> Option<u64> {
    let (subs, total) = sr::disposicion(&forma);
    let almacen = Almacen::de(forma.formato);
    let color = matches!(almacen, Almacen::Rgba8 | Almacen::Bgra8);
    let prestable = cadena || (color && subs.len() == 1 && total <= crate::memoria::TEXTURA_PRESTABLE);
    // 05-10: con stencil, su plano va detras (`d3d12_stencil`).
    let datos = crate::memoria::pedir_pixeles((total + crate::d3d12_stencil::bytes_de_mas(&forma, total)).max(4), prestable)?;
    let pixeles = match almacen {
        Almacen::Bloques(_) => Pixeles::ninguno(),
        // N5.16: un float de 2-4 canales son cuatro palabras por texel.
        _ => Pixeles::sobre(datos, forma.ancho as usize * forma.alto as usize * (almacen.elemento().0 / 4) as usize),
    };
    let tex = Some(Tex { forma, subs, datos, almacen, banderas });
    crate::pulso::contar(crate::pulso::Cosa::Recurso, 0);
    let r = nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho: forma.ancho, alto: forma.alto, formato: forma.formato, pixeles, bufer: None, cadena, en_pantalla: false, tipo_monton: 1, tex }) as u64;
    // Uno nuevo en la direccion de uno que se fue no hereda su limpieza.
    tuberia::olvidar_limpieza(r);
    Some(r)
}

/// Un recurso que es un bufer (CreateCommittedResource).
pub(crate) fn recurso_bufer(b: Bufer) -> u64 {
    crate::pulso::contar(crate::pulso::Cosa::Recurso, 0);
    nuevo(com::RESOURCE, vtabla_recurso(), Recurso { ancho: b.bytes as u32, alto: 1, formato: 0, pixeles: Pixeles::ninguno(), bufer: Some(b), cadena: false, en_pantalla: false, tipo_monton: 1, tex: None }) as u64
}

/// El inicio de un bufer de la casa, o `None` si `this` es una imagen.
pub(crate) fn base_de_bufer(this: u64) -> Option<u64> {
    // SAFETY: `this` es un Recurso de la casa (lo dice su vtabla).
    unsafe { de::<Recurso>(this).bufer.as_ref().map(Bufer::base) }
}

/// `GetAdapterLuid(this, ret)`: el LUID del adaptador (el de su GetDesc), por
/// el puntero oculto (devuelve una estructura).
extern "win64" fn get_adapter_luid(_this: u64, ret: *mut u64) -> *mut u64 {
    // SAFETY: el LUID oculto del `.exe`.
    unsafe { ret.write_unaligned(crate::dxgi::LUID) };
    ret
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

/// `GetDeviceRemovedReason(this)`: S_OK mientras el dispositivo vive; tras
/// un `RemoveDevice`, DXGI_ERROR_DEVICE_REMOVED.
extern "win64" fn get_device_removed_reason(this: u64) -> i32 {
    // SAFETY: `this` es el Dispositivo de la casa.
    if unsafe { de::<Dispositivo>(this) }.quitado {
        0x887A_0005_u32 as i32
    } else {
        S_OK
    }
}

/// Lo que hace `RemoveDevice` (Device5).
pub(crate) fn quitar_dispositivo(this: u64) {
    // SAFETY: como arriba.
    unsafe { de::<Dispositivo>(this) }.quitado = true;
}

pub(crate) extern "win64" fn create_command_queue(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::QUEUE) {
        return E_NOINTERFACE;
    }
    let mut m = alloc::vec![(10, dir!(execute_command_lists)), (14, dir!(queue_signal))];
    m.extend_from_slice(&crate::d3d12_resto::cola());
    let vt = vtabla::<{ com::QUEUE }>(&m);
    let mut d = [0u8; 16];
    if !desc.is_null() {
        // SAFETY: un D3D12_COMMAND_QUEUE_DESC del `.exe` (16 B).
        unsafe { core::ptr::copy_nonoverlapping(desc, d.as_mut_ptr(), 16) };
    }
    dar(pp, nuevo(com::QUEUE, vt, Cola { desc: d }) as u64)
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
pub(crate) extern "win64" fn create_command_list(_this: u64, _mascara: u32, tipo: u32, _asig: u64, pso: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::LIST) {
        return E_NOINTERFACE;
    }
    let mut m = alloc::vec![
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
    ];
    m.extend_from_slice(&crate::d3d12_resto::lista());
    m.extend_from_slice(&crate::d3d12_lista2::lista());
    let vt = vtabla::<{ com::LIST }>(&m);
    let (estado, computo) = estados_al_empezar(pso);
    dar(pp, nuevo(com::LIST, vt, Lista { ordenes: Vec::new(), abierta: true, estado, tipo, computo }) as u64)
}

/// El estado de dibujo y el de computo de una lista recien creada o
/// reiniciada con `pso`: el PSO va al suyo.
pub(crate) fn estados_al_empezar(pso: u64) -> (Estado, Estado) {
    if crate::d3d12_resto::es_computo(pso) {
        (Estado::default(), Estado { pso, ..Estado::default() })
    } else {
        (Estado { pso, ..Estado::default() }, Estado::default())
    }
}

/// `D3D12_DESCRIPTOR_HEAP_DESC`: Type +0, NumDescriptors +4, Flags +8.
extern "win64" fn create_descriptor_heap(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if desc.is_null() || !pide(riid, com::HEAP) {
        return E_NOINTERFACE;
    }
    // SAFETY: un D3D12_DESCRIPTOR_HEAP_DESC del `.exe`.
    let n = unsafe { (desc.add(4) as *const u32).read_unaligned() } as usize;
    let vt = vtabla::<{ com::HEAP }>(&[(8, dir!(crate::d3d12_resto::get_desc_monton)), (9, dir!(get_cpu_descriptor_handle_for_heap_start)), (10, dir!(get_cpu_descriptor_handle_for_heap_start))]);
    let mut d = [0u8; 16];
    // SAFETY: como arriba (16 B).
    unsafe { core::ptr::copy_nonoverlapping(desc, d.as_mut_ptr(), 16) };
    // A cero, como el `vec!` de antes; pedido al proceso, como en Windows.
    let Some(ranuras) = crate::memoria::pedir_bufer(n.max(1) as u64 * DESCRIPTOR) else {
        crate::aviso("CreateDescriptorHeap: no hay memoria para los descriptores: E_OUTOFMEMORY");
        return E_OUTOFMEMORY;
    };
    dar(pp, nuevo(com::HEAP, vt, Monton { ranuras, desc: d }) as u64)
}

extern "win64" fn get_descriptor_handle_increment_size(_this: u64, _tipo: u32) -> u32 {
    DESCRIPTOR as u32
}

extern "win64" fn create_fence(_this: u64, inicial: u64, _banderas: u32, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::FENCE) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::FENCE }>(&[
        (8, dir!(get_completed_value)),
        (9, dir!(set_event_on_completion)),
        (10, dir!(fence_signal)),
        (11, dir!(get_creation_flags)),
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
        ret.write(m.ranuras);
    }
    ret
}

// -- Las vistas de lectura (texturas) y los muestreadores (29-09) -----------

/// La marca de la palabra 1 de un descriptor: que es.
pub(crate) const DESC_SRV: u64 = 1;
pub(crate) const DESC_MUESTREADOR: u64 = 2;
/// Tanda 47: una vista de constantes (CBV: la direccion y la medida) y una
/// de acceso desordenado (UAV: el recurso).
pub(crate) const DESC_CBV: u64 = 3;
pub(crate) const DESC_UAV: u64 = 4;
pub(crate) const DESCRIPTOR_BYTES: u64 = DESCRIPTOR;

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
        // La palabra 0 (el recurso de un SRV) lleva aqui la ComparisonFunc
        // (+24): la de los muestreadores de sombras (03-10).
        r.write(w(24) as u64);
        r.add(1).write(DESC_MUESTREADOR);
        r.add(2).write(w(0) as u64 | (w(4) as u64) << 32);
        r.add(3).write(w(8) as u64 | borde << 32);
    }
}

// -- La lista de ordenes ----------------------------------------------------

pub(crate) extern "win64" fn list_close(this: u64) -> i32 {
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
    (l.estado, l.computo) = estados_al_empezar(pso);
    S_OK
}

/// La lista de `this`, para los `Set*`.
///
/// # Safety
/// `this` es una Lista de la casa.
pub(crate) unsafe fn lista<'a>(this: u64) -> &'a mut Lista {
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
    // Uno de computo no se dibuja: el grafico de antes sigue (tanda 47), y
    // va al estado de computo (N5.5).
    if crate::d3d12_resto::es_computo(pso) {
        // SAFETY: `this` es una Lista de la casa.
        unsafe { lista(this).computo.pso = pso };
        return;
    }
    // SAFETY: `this` es una Lista de la casa.
    unsafe { lista(this).estado.pso = pso };
}

/// Cambiar de root signature borra lo que se le habia dado a la anterior.
pub(crate) extern "win64" fn set_graphics_root_signature(this: u64, raiz: u64) {
    // SAFETY: `this` es una Lista de la casa.
    let e = unsafe { &mut lista(this).estado };
    if e.raiz != raiz {
        e.cbv = Default::default();
        e.raiz32 = Default::default();
    }
    e.raiz = raiz;
}

/// `SetGraphicsRootConstantBufferView(this, parametro, direccion)`.
pub(crate) extern "win64" fn set_graphics_root_constant_buffer_view(this: u64, parametro: u32, va: u64) {
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

/// `D3D12_VERTEX_BUFFER_VIEW` (16 B): direccion +0, bytes +8, paso +12. N5.13
/// (05-10): las 16 ranuras, desde `desde`; con `v` nulo, se quitan.
extern "win64" fn ia_set_vertex_buffers(this: u64, desde: u32, n: u32, v: *const u8) {
    if desde as u64 + n as u64 > 16 {
        aviso("IASetVertexBuffers mas alla de la ranura 15: en Windows es un error, y no se hace");
        return;
    }
    // SAFETY: `this` es una Lista de la casa; `v`, `n` vistas del `.exe` o nula.
    let e = unsafe { &mut lista(this).estado };
    for i in 0..n as usize {
        // SAFETY: como arriba: la vista `i` de las `n`.
        e.vertices[desde as usize + i] = if v.is_null() { Vista::default() } else { unsafe { vista(v.add(16 * i)) } };
    }
}

/// Una vista de bufer (de vertices o de indices: la misma forma).
///
/// # Safety
/// 16 bytes legibles del `.exe`.
pub(crate) unsafe fn vista(v: *const u8) -> Vista {
    Vista {
        va: (v as *const u64).read_unaligned(),
        bytes: (v.add(8) as *const u32).read_unaligned(),
        paso_o_formato: (v.add(12) as *const u32).read_unaligned(),
    }
}

/// `OMSetRenderTargets(this, n, handles, uno_solo, dsv)`: los destinos
/// (N5.8: hasta 8). Con `uno_solo` los n descriptores son consecutivos desde
/// `handles[0]`; sin el, `handles` es un array.
pub(crate) extern "win64" fn om_set_render_targets(this: u64, n: u32, handles: *const u64, uno_solo: i32, dsv: *const u64) {
    if n > 8 {
        aviso("OMSetRenderTargets con mas de 8 destinos: en Windows es un error; se toman 8");
    }
    // SAFETY: `this` es una Lista de la casa; `handles` y `dsv`, descriptores
    // del `.exe` (o nulos), y cada uno una ranura de un monton de la casa:
    // el recurso en la palabra 0, el subrecurso de la vista en la 3.
    unsafe {
        let e = &mut lista(this).estado;
        let ranura = |h: u64| ((h as *const u64).read(), (h as *const u64).add(3).read());
        (e.rtv, e.rtv_sub) = if n == 0 || handles.is_null() { (0, 0) } else { ranura(handles.read_unaligned()) };
        e.rtv_otros = [(0, 0); 7];
        for i in 1..(n as usize).min(8) {
            if handles.is_null() {
                break;
            }
            let h = if uno_solo != 0 { handles.read_unaligned() + i as u64 * DESCRIPTOR as u64 } else { handles.add(i).read_unaligned() };
            e.rtv_otros[i - 1] = if h == 0 { (0, 0) } else { ranura(h) };
        }
        (e.dsv, e.dsv_sub) = if dsv.is_null() { (0, 0) } else { ranura(dsv.read_unaligned()) };
    }
}

extern "win64" fn draw_instanced(this: u64, vertices: u32, instancias: u32, primero: u32, primera_instancia: u32) {
    dibujar(this, vertices, instancias, primero, 0, false, primera_instancia);
}

extern "win64" fn draw_indexed_instanced(this: u64, indices: u32, instancias: u32, primero: u32, base: i32, primera_instancia: u32) {
    dibujar(this, indices, instancias, primero, base, true, primera_instancia);
}

fn dibujar(this: u64, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool, primera_instancia: u32) {
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { lista(this) };
    let estado = l.estado.clone();
    l.ordenes.push(Orden::Dibujar { estado, cuantos, instancias, primero, base, indexado, primera_instancia });
}

/// En la CPU no hay caches de la GPU que vaciar ni estados de memoria que
/// cambiar: la barrera no tiene nada que hacer, y no es un atajo.
extern "win64" fn resource_barrier(_this: u64, _n: u32, _barreras: *const u8) {}

/// `SetDescriptorHeaps`: aqui la tabla de la raiz ya apunta a la ranura
/// misma (el identificador de GPU es su direccion): no hay nada que atar.
extern "win64" fn set_descriptor_heaps(_this: u64, _n: u32, _montones: *const u64) {}

/// `SetGraphicsRootDescriptorTable(this, parametro, handle GPU)`.
pub(crate) extern "win64" fn set_graphics_root_descriptor_table(this: u64, parametro: u32, handle: u64) {
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
pub(crate) extern "win64" fn clear_render_target_view(this: u64, handle: u64, color: *const f32, n: u32, _rects: *const u8) {
    if n != 0 {
        aviso("ClearRenderTargetView con rectangulos: todavia limpia solo el recurso entero");
        return;
    }
    if handle == 0 || color.is_null() {
        return;
    }
    // SAFETY: el descriptor es una ranura de la casa (ver arriba); `color`,
    // cuatro floats del `.exe`.
    let (recurso, sub, c) = unsafe {
        let c = core::slice::from_raw_parts(color, 4);
        ((handle as *const u64).read(), (handle as *const u64).add(3).read(), [c[0], c[1], c[2], c[3]])
    };
    if recurso == 0 {
        aviso("ClearRenderTargetView sobre un descriptor sin CreateRenderTargetView");
        return;
    }
    // SAFETY: el descriptor guarda un Recurso de la casa.
    let formato = unsafe { de::<Recurso>(recurso).formato };
    let [r, g, b, a] = c.map(unorm8);
    // En memoria, R8G8B8A8 es R,G,B,A y B8G8R8A8 es B,G,R,A; un float, R
    // (N5.16b: cuantizado al formato de la vista: un R16_FLOAT de un
    // R16_TYPELESS es un half).
    let pixel = match Almacen::de(formato) {
        Almacen::Bgra8 => a << 24 | r << 16 | g << 8 | b,
        Almacen::Flotante => {
            // SAFETY: la ranura del descriptor (4 palabras; ver arriba).
            let vista = crate::d3d12_vistas::leer(unsafe { core::slice::from_raw_parts(handle as *const u64, 4) }).0 .1;
            bmo_proton_x::formato_ia::cuantizar(if vista != 0 { vista } else { formato }, c)[0].to_bits()
        }
        Almacen::Bloques(_) => {
            aviso("ClearRenderTargetView de una textura BC: en Windows es un error");
            return;
        }
        Almacen::Rgba8 => a << 24 | b << 16 | g << 8 | r,
        Almacen::Flotantes4 => {
            // El color en float, cuantizado al formato de VERDAD de la
            // textura (un R11G11B10 no guarda signo ni alfa).
            let texel = bmo_proton_x::formato_ia::cuantizar(Almacen::nativo(formato), c).map(f32::to_bits);
            // SAFETY: `this` es una Lista de la casa.
            unsafe { de::<Lista>(this) }.ordenes.push(Orden::LimpiarTexel { recurso, sub, texel });
            return;
        }
    };
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { de::<Lista>(this) };
    l.ordenes.push(Orden::Limpiar { recurso, sub, pixel });
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
const CLEAR_FLAG_STENCIL: u32 = 2;

pub(crate) extern "win64" fn clear_depth_stencil_view(this: u64, handle: u64, banderas: u32, bits: u32, stencil: u8, n: u32, _rects: *const u8) {
    if n != 0 {
        aviso("ClearDepthStencilView con rectangulos: todavia limpia solo el recurso entero");
        return;
    }
    if handle == 0 || banderas & (CLEAR_FLAG_DEPTH | CLEAR_FLAG_STENCIL) == 0 {
        return;
    }
    // SAFETY: el descriptor es una ranura de la casa (CreateDepthStencilView).
    let (recurso, sub) = unsafe { ((handle as *const u64).read(), (handle as *const u64).add(3).read()) };
    if recurso == 0 {
        aviso("ClearDepthStencilView sobre un descriptor sin CreateDepthStencilView");
        return;
    }
    // SAFETY: `this` es una Lista de la casa.
    let l = unsafe { de::<Lista>(this) };
    if banderas & CLEAR_FLAG_DEPTH != 0 {
        l.ordenes.push(Orden::Limpiar { recurso, sub, pixel: bits });
    }
    // 05-10: el stencil (antes se tiraba): su byte llega por la pila, tal
    // cual (el puente solo toca r9).
    if banderas & CLEAR_FLAG_STENCIL != 0 {
        l.ordenes.push(Orden::LimpiarStencil { recurso, sub, valor: stencil });
    }
}

/// `ID3D12Resource2::GetDesc1(this, ret)`: el D3D12_RESOURCE_DESC1 (64 B): el
/// de siempre y la region de mips de sampler feedback, a cero.
extern "win64" fn get_desc1(this: u64, ret: *mut u8) -> *mut u8 {
    get_desc(this, ret);
    // SAFETY: los 64 bytes del `.exe`.
    unsafe { core::ptr::write_bytes(ret.add(56), 0, 8) };
    ret
}

/// `GetDesc(this, ret)`: el D3D12_RESOURCE_DESC (56 B) por el puntero
/// oculto. Una textura, con su forma de verdad (02-10): dimension, mips,
/// capas o profundidad, y las banderas con que se creo.
pub(crate) extern "win64" fn get_desc(this: u64, ret: *mut u8) -> *mut u8 {
    // SAFETY: `this` es un Recurso de la casa.
    let r = unsafe { de::<Recurso>(this) };
    let (dimension, layout, banderas, ancho, alto, hondo, mips, formato) = match (&r.bufer, &r.tex) {
        (Some(b), _) => (1u32, 1u32, 0u32, b.bytes as u64, 1, 1, 1, 0),
        (None, Some(t)) => {
            let f = t.forma;
            (f.dimension, 0, t.banderas, f.ancho as u64, f.alto, f.hondo as u16, f.mips as u16, f.formato)
        }
        (None, None) => (3, 0, BANDERA_RT, r.ancho as u64, r.alto, 1, 1, r.formato),
    };
    // SAFETY: 56 bytes del `.exe`.
    unsafe {
        core::ptr::write_bytes(ret, 0, 56);
        let u = |o: usize, v: u32| (ret.add(o) as *mut u32).write_unaligned(v);
        u(0, dimension);
        (ret.add(16) as *mut u64).write_unaligned(ancho);
        u(24, alto);
        (ret.add(28) as *mut u16).write_unaligned(hondo);
        (ret.add(30) as *mut u16).write_unaligned(mips);
        u(32, formato);
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
    asignacion(ret, n, descs, 56, core::ptr::null_mut())
}

/// **La cuenta de GetResourceAllocationInfo, 1 y 2**: `n` descripciones de
/// `paso` bytes (56 la de siempre, 64 la DESC1), y si `info1` no es nulo,
/// un D3D12_RESOURCE_ALLOCATION_INFO1 por recurso (Offset, Alignment,
/// SizeInBytes: 24 bytes), uno detras de otro como los pondria un monton.
/// Tanda 48: cada recurso con `d3d12_medidas::medida` (sus mips, capas,
/// formato y muestras), cada uno en su alineacion, y la del conjunto, la
/// mayor.
pub(crate) fn asignacion(ret: *mut u64, n: u32, descs: *const u8, paso: usize, info1: *mut u64) -> *mut u64 {
    let (mut total, mut alineacion) = (0u64, crate::d3d12_medidas::ALINEACION);
    for i in 0..n as usize {
        // SAFETY: `n` descripciones del `.exe`, de `paso` bytes cada una.
        let (medida, alin) = unsafe { crate::d3d12_medidas::medida(descs.add(paso * i)) };
        let desde = total.div_ceil(alin) * alin;
        if !info1.is_null() {
            // SAFETY: `n` D3D12_RESOURCE_ALLOCATION_INFO1 del `.exe`.
            unsafe {
                let e = info1.add(3 * i);
                e.write_unaligned(desde);
                e.add(1).write_unaligned(alin);
                e.add(2).write_unaligned(medida);
            }
        }
        total = desde + medida;
        alineacion = alineacion.max(alin);
    }
    // SAFETY: 16 bytes del `.exe`.
    unsafe {
        ret.write_unaligned(total);
        ret.add(1).write_unaligned(alineacion);
    }
    ret
}

// -- Leer un render target desde la CPU (P3c4, el --fotograma de BMOX-12) ---

/// `GetCopyableFootprints(this, desc, primero, n, desde, huellas, filas,
/// bytes_fila, total)`: como quedan `n` subrecursos de una textura en un
/// bufer (02-10: cualquier formato, mips, capas y 3D; ver
/// `subrecursos::huellas`), cada fila a 256 y cada subrecurso a 512, y el
/// total SIN el relleno de la ultima fila, como D3D12. Un bufer es una
/// fila. D3D12_PLACED_SUBRESOURCE_FOOTPRINT (32 B): Offset +0, Format +8,
/// Width +12, Height +16, Depth +20, RowPitch +24.
#[allow(clippy::too_many_arguments)]
pub(crate) extern "win64" fn get_copyable_footprints(_this: u64, desc: *const u8, primero: u32, n: u32, desde: u64, huellas: *mut u8, filas: *mut u32, bytes_fila: *mut u64, total: *mut u64) {
    if desc.is_null() {
        return;
    }
    // SAFETY: un D3D12_RESOURCE_DESC del `.exe` (56 B).
    let (dimension, ancho, formato) = unsafe { ((desc as *const u32).read_unaligned(), (desc.add(16) as *const u64).read_unaligned(), (desc.add(32) as *const u32).read_unaligned()) };
    let todas: Vec<sr::Huella> = if dimension == 1 {
        let h = sr::Huella { desde: 0, ancho: ancho as u32, alto: 1, hondo: 1, paso: ancho, filas: 1, fila: ancho };
        alloc::vec![h; n.min(1) as usize]
    } else {
        // SAFETY: como arriba.
        match unsafe { Forma::de(desc) } {
            Some(f) if primero.saturating_add(n) <= f.subrecursos() => sr::huellas(&f, primero, n).0,
            _ => {
                aviso("GetCopyableFootprints de una textura imposible, o de subrecursos que no tiene: todo a 0xFF..., como D3D12");
                // SAFETY: los punteros del `.exe` que no son nulos.
                unsafe {
                    if !total.is_null() {
                        total.write_unaligned(u64::MAX);
                    }
                }
                return;
            }
        }
    };
    let suma = todas.last().map_or(0, |h| h.desde + h.paso * (h.filas as u64 * h.hondo as u64 - 1) + h.fila);
    // SAFETY: los punteros del `.exe` que no son nulos (uno por subrecurso).
    unsafe {
        for (i, h) in todas.iter().enumerate() {
            if !huellas.is_null() {
                let e = huellas.add(32 * i);
                core::ptr::write_bytes(e, 0, 32);
                (e as *mut u64).write_unaligned(desde + h.desde);
                let u = |o: usize, v: u32| (e.add(o) as *mut u32).write_unaligned(v);
                u(8, formato);
                u(12, h.ancho);
                u(16, h.alto);
                u(20, h.hondo);
                u(24, h.paso as u32);
            }
            if !filas.is_null() {
                filas.add(i).write_unaligned(h.filas);
            }
            if !bytes_fila.is_null() {
                bytes_fila.add(i).write_unaligned(h.fila);
            }
        }
        if !total.is_null() {
            total.write_unaligned(suma);
        }
    }
}

/// `CopyTextureRegion(this, destino, x, y, z, origen, caja)`: dos
/// D3D12_TEXTURE_COPY_LOCATION (un subrecurso, o una huella en un bufer) y
/// una D3D12_BOX del origen (nula: todo). Se APUNTA; se hace en
/// ExecuteCommandLists, como la GPU (ver `d3d12_texturas::hacer`).
extern "win64" fn copy_texture_region(this: u64, destino: *const u8, x: u32, y: u32, z: u32, origen: *const u8, caja: *const u8) {
    if destino.is_null() || origen.is_null() {
        return;
    }
    // SAFETY: dos D3D12_TEXTURE_COPY_LOCATION del `.exe` (48 B), y su
    // D3D12_BOX (24 B) si no es nula.
    let (d, o, caja) = unsafe {
        let caja = (!caja.is_null()).then(|| core::array::from_fn(|k| (caja.add(4 * k) as *const u32).read_unaligned()));
        (crate::d3d12_texturas::Ubicacion::de(destino), crate::d3d12_texturas::Ubicacion::de(origen), caja)
    };
    let (Some(destino), Some(origen)) = (d, o) else {
        aviso("CopyTextureRegion con un tipo de ubicacion que no es de D3D12");
        return;
    };
    // SAFETY: `this` es una Lista de la casa.
    unsafe { de::<Lista>(this) }.ordenes.push(Orden::Region(crate::d3d12_texturas::Region { destino, en: [x, y, z], origen, caja }));
}

// -- La cola, el asignador y la valla ---------------------------------------

/// `ExecuteCommandLists(this, n, listas)`: en el acto, en orden.
extern "win64" fn execute_command_lists(_this: u64, n: u32, listas: *const u64) {
    let empezo = (crate::plataforma().ahora_ns)();
    ejecutar_listas(n, listas);
    crate::dxgi::dibujado((crate::plataforma().ahora_ns)().saturating_sub(empezo));
}

fn ejecutar_listas(n: u32, listas: *const u64) {
    crate::pulso::contar(crate::pulso::Cosa::Lista, 0);
    for i in 0..n as usize {
        // SAFETY: `n` punteros a listas de la casa.
        let l = unsafe { de::<Lista>(listas.add(i).read()) };
        if l.abierta {
            aviso("ExecuteCommandLists con una lista sin Close: en Windows es un error, y no se corre");
            continue;
        }
        // E2.7: toda lista empieza sin consultas abiertas ni predicacion.
        crate::consultas::al_empezar_lista();
        let mut saltar = false;
        for o in &l.ordenes {
            if saltar && crate::consultas::predicable(o) {
                continue;
            }
            match o {
                Orden::Predicar { dir, op } => saltar = crate::consultas::salta(*dir, *op),
                // ** P3b4c: la limpieza se APUNTA, no se hace: la hace quien
                // dibuje (la 3060 en su dibujo; la CPU al empezar el suyo), o
                // quien lea los pixeles antes (Present, CopyTextureRegion).
                // Asi, con la 3060 dibujando, la CPU no llena 3,6 MB por
                // limpieza y por fotograma.
                Orden::Limpiar { recurso, sub: 0, pixel } => tuberia::limpieza_pendiente(*recurso, *pixel),
                // La de otro subrecurso (una mip, una capa): ya.
                Orden::Limpiar { recurso, sub, pixel } => match tuberia::destino(*recurso, *sub) {
                    Some((px, _, _)) => px.fill(*pixel),
                    None => aviso("ClearRenderTargetView/ClearDepthStencilView de un subrecurso que la textura no tiene"),
                },
                Orden::LimpiarTexel { recurso, sub, texel } => {
                    if *sub == 0 {
                        tuberia::olvidar_limpieza(*recurso);
                    }
                    match tuberia::destino(*recurso, *sub) {
                        Some((px, _, _)) => px.chunks_exact_mut(4).for_each(|t| t.copy_from_slice(texel)),
                        None => aviso("ClearRenderTargetView de un subrecurso que la textura no tiene"),
                    }
                }
                Orden::Dibujar { estado, cuantos, instancias, primero, base, indexado, primera_instancia } => {
                    tuberia::ejecutar_dibujo(estado, *cuantos, *instancias, *primero, *base, *indexado, *primera_instancia);
                }
                Orden::Region(c) => {
                    if let Err(m) = crate::d3d12_texturas::hacer(c) {
                        aviso(&alloc::format!("CopyTextureRegion no se hace: {m}"));
                    }
                }
                o => crate::d3d12_resto::ejecutar(o),
            }
        }
        crate::consultas::al_acabar_lista();
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
    valor_de_valla(this)
}

/// `ID3D12Fence1::GetCreationFlags`: D3D12_FENCE_FLAG_NONE (la casa no
/// comparte vallas).
extern "win64" fn get_creation_flags(_this: u64) -> u32 {
    0
}

/// El valor de la valla `v` (de la casa).
pub(crate) fn valor_de_valla(v: u64) -> u64 {
    // SAFETY: una Valla de la casa.
    unsafe { de::<Valla>(v).valor }
}

pub(crate) extern "win64" fn fence_signal(this: u64, valor: u64) -> i32 {
    // SAFETY: como arriba.
    marcar(unsafe { de::<Valla>(this) }, valor);
    S_OK
}

/// `SetEventOnCompletion(this, valor, evento)`: si ya se llego, el evento se
/// enciende YA; si no, cuando un Signal (de la cola o de otro hilo) llegue.
/// Con evento nulo, Windows ESPERA ahi mismo: aqui se cede el turno hasta que
/// llegue (o hasta el bloqueo mutuo, que se dice).
pub(crate) extern "win64" fn set_event_on_completion(this: u64, valor: u64, evento: u64) -> i32 {
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

/// Dejar 0 donde el `.exe` espera una interfaz que no se le da.
fn nada(pp: *mut u64) {
    if !pp.is_null() {
        // SAFETY: el puntero a interfaz del `.exe`.
        unsafe { pp.write_unaligned(0) };
    }
}

/// ** LAS EXPORTACIONES DE d3d12.dll QUE FALTABAN (01-10). Cyberpunk, tras
/// D3D12CreateDevice, salto a la direccion 0: un GetProcAddress que la casa
/// contestaba con NULL y el `.exe` llamaba igual. Contestan lo que Windows
/// sin las capas de depuracion del SDK: no hay interfaz de depuracion
/// (E_NOINTERFACE, y 0 en el puntero), no hay funciones experimentales.
extern "win64" fn d3d12_get_debug_interface(_riid: *const Guid, pp: *mut u64) -> i32 {
    nada(pp);
    E_NOINTERFACE
}

extern "win64" fn d3d12_get_interface(_clsid: *const Guid, _riid: *const Guid, pp: *mut u64) -> i32 {
    nada(pp);
    E_NOINTERFACE
}

extern "win64" fn d3d12_enable_experimental_features(_n: u32, _iids: *const Guid, _cfg: u64, _medidas: *const u32) -> i32 {
    E_NOINTERFACE
}

/// Leer una firma raiz ya serializada: la casa todavia no; se dice.
extern "win64" fn d3d12_create_root_signature_deserializer(_datos: u64, _medida: usize, _riid: *const Guid, pp: *mut u64) -> i32 {
    aviso("D3D12Create(Versioned)RootSignatureDeserializer: la casa todavia no lee firmas serializadas");
    nada(pp);
    E_NOTIMPL
}

const E_NOTIMPL: i32 = 0x8000_4001_u32 as i32;

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "D3D12CreateDevice" => dir!(d3d12_create_device),
        "D3D12GetDebugInterface" => dir!(d3d12_get_debug_interface),
        "D3D12GetInterface" => dir!(d3d12_get_interface),
        "D3D12EnableExperimentalFeatures" => dir!(d3d12_enable_experimental_features),
        "D3D12CreateRootSignatureDeserializer" | "D3D12CreateVersionedRootSignatureDeserializer" => dir!(d3d12_create_root_signature_deserializer),
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
