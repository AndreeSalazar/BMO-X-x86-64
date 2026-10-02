//! **COM de la casa** (P3a, 27-09): la forma de un objeto de D3D12 y DXGI.
//!
//! D3D12 casi no se importa: `d3d12.dll` da DOS funciones (`D3D12CreateDevice`
//! y `D3D12SerializeRootSignature`) y todo lo demas son METODOS de objetos COM
//! -- punteros a una VTABLA de funciones, con `this` delante (lo midio X1: las
//! tablas de importaciones no lo ven). Aqui vive esa forma:
//!
//! ```text
//!    un objeto   [vtabla][interfaz][referencias][lo suyo...]
//!    la vtabla   los metodos en el ORDEN de la cabecera de Windows (d3d12.h,
//!                dxgi1_2.h): un hueco movido llama a otro metodo, y no falla
//!    un hueco    que la casa no tiene: su PROPIA funcion `falta`, que dice
//!                "ID3D12Device::CreateRootSignature no esta en la casa" y
//!                sale. Nunca un S_OK callado ni un salto a cero
//! ```
//!
//! Los nombres de los metodos (`M_*`, abajo) son los de las cabeceras de
//! Windows, en su orden: con ellos el aviso dice CUAL pidio el `.exe`.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::{aviso, plataforma};

pub const S_OK: i32 = 0;
pub const S_FALSE: i32 = 1;
pub const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;
pub const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
pub const E_FAIL: i32 = 0x8000_4005_u32 as i32;
pub const E_POINTER: i32 = 0x8000_4003_u32 as i32;

/// Un GUID tal como esta en memoria: Data1 (u32), Data2 y Data3 (u16) en
/// little-endian, y los ocho bytes de Data4.
pub type Guid = [u8; 16];

pub const fn guid(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Guid {
    let a = d1.to_le_bytes();
    let b = d2.to_le_bytes();
    let c = d3.to_le_bytes();
    [a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]]
}

pub const IID_IUNKNOWN: Guid = guid(0x0000_0000, 0x0000, 0x0000, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);

/// Las interfaces de la casa: su nombre, sus metodos y los IID que acepta
/// `QueryInterface` (la suya y las que hereda).
pub struct Interfaz {
    pub nombre: &'static str,
    pub metodos: &'static [&'static str],
    pub iids: &'static [Guid],
}

pub const DEVICE: usize = 0;
pub const QUEUE: usize = 1;
pub const ALLOCATOR: usize = 2;
pub const LIST: usize = 3;
pub const HEAP: usize = 4;
pub const RESOURCE: usize = 5;
pub const FENCE: usize = 6;
pub const FACTORY: usize = 7;
pub const SWAPCHAIN: usize = 8;
pub const ROOTSIG: usize = 9;
pub const PSO: usize = 10;
pub const BLOB: usize = 11;
pub const ADAPTER: usize = 12;
pub const OUTPUT: usize = 13;

const IID_OBJECT: Guid = guid(0xc4fec28f, 0x7966, 0x4e95, [0x9f, 0x94, 0xf4, 0x31, 0xcb, 0x56, 0xc3, 0xb8]);
const IID_DEVICECHILD: Guid = guid(0x905db94b, 0xa00c, 0x4140, [0x9d, 0xf5, 0x2b, 0x64, 0xca, 0x9e, 0xa3, 0x57]);
const IID_PAGEABLE: Guid = guid(0x63ee58fb, 0x1268, 0x4835, [0x86, 0xda, 0xf0, 0x08, 0xce, 0x62, 0xf0, 0xd6]);
const IID_COMMANDLIST: Guid = guid(0x7116d91c, 0xe7e4, 0x47ce, [0xb8, 0xc6, 0xec, 0x81, 0x68, 0xf4, 0x37, 0xe5]);
pub const IID_DEVICE: Guid = guid(0x189819f1, 0x1db6, 0x4b57, [0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7]);
pub const IID_QUEUE: Guid = guid(0x0ec870a6, 0x5d7e, 0x4c22, [0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed]);
pub const IID_ALLOCATOR: Guid = guid(0x6102dee4, 0xaf59, 0x4b09, [0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24]);
pub const IID_LIST: Guid = guid(0x5b160d0f, 0xac1b, 0x4185, [0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55]);
pub const IID_HEAP: Guid = guid(0x8efb471d, 0x616c, 0x4f49, [0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51]);
pub const IID_RESOURCE: Guid = guid(0x696442be, 0xa72e, 0x4059, [0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad]);
pub const IID_FENCE: Guid = guid(0x0a753dcf, 0xc4d8, 0x4b91, [0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76]);
pub const IID_FACTORY1: Guid = guid(0x770aae78, 0xf26f, 0x4dba, [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87]);
pub const IID_FACTORY2: Guid = guid(0x50c83a1c, 0xe072, 0x4c48, [0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0]);
pub const IID_ROOTSIG: Guid = guid(0xc54a6b66, 0x72df, 0x4ee8, [0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14]);
pub const IID_PSO: Guid = guid(0x765a30f3, 0xf624, 0x4c6f, [0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45]);
pub const IID_BLOB: Guid = guid(0x8ba5fb08, 0x5195, 0x40e2, [0xac, 0x58, 0x0d, 0x98, 0x9c, 0x3a, 0x01, 0x02]);
// P3c4: DXGI hasta Factory6 y SwapChain3, y el adaptador (comprobados con
// las cabeceras de Windows del crate `windows` 0.58).
const IID_DXGIOBJECT: Guid = guid(0xaec22fb8, 0x76f3, 0x4639, [0x9b, 0xe0, 0x28, 0xeb, 0x43, 0xa6, 0x7a, 0x2e]);
const IID_DEVICESUBOBJECT: Guid = guid(0x3d3e0379, 0xf9de, 0x4d58, [0xbb, 0x6c, 0x18, 0xd6, 0x29, 0x92, 0xf1, 0xa6]);
const IID_FACTORY: Guid = guid(0x7b7166ec, 0x21c7, 0x44ae, [0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69]);
pub const IID_FACTORY3: Guid = guid(0x25483823, 0xcd46, 0x4c7d, [0x86, 0xca, 0x47, 0xaa, 0x95, 0xb8, 0x37, 0xbd]);
pub const IID_FACTORY4: Guid = guid(0x1bc6ea02, 0xef36, 0x464f, [0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a]);
pub const IID_FACTORY5: Guid = guid(0x7632e1f5, 0xee65, 0x4dca, [0x87, 0xfd, 0x84, 0xcd, 0x75, 0xf8, 0x83, 0x8d]);
pub const IID_FACTORY6: Guid = guid(0xc1b6694f, 0xff09, 0x44a9, [0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17]);
pub const IID_ADAPTER: Guid = guid(0x2411e7e1, 0x12ac, 0x4ccf, [0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0]);
pub const IID_ADAPTER1: Guid = guid(0x29038f61, 0x3839, 0x4626, [0x91, 0xfd, 0x08, 0x68, 0x79, 0x01, 0x1a, 0x05]);
// 02-10: Cyberpunk pide el 2 (dxgi1_2.h); el 3 y el 4 (dxgi1_4.h, dxgi1_6.h)
// son los que un motor pide despues para la memoria de video.
pub const IID_ADAPTER2: Guid = guid(0x0aa1ae0a, 0xfa0e, 0x4b84, [0x86, 0x44, 0xe0, 0x5f, 0xf8, 0xe5, 0xac, 0xb5]);
pub const IID_ADAPTER3: Guid = guid(0x645967a4, 0x1392, 0x4310, [0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd]);
pub const IID_ADAPTER4: Guid = guid(0x3c8d99d1, 0x4fbf, 0x4181, [0xa8, 0x2c, 0xaf, 0x66, 0xbf, 0x7b, 0xd2, 0x4e]);
// 01-10: la salida (el monitor) hasta Output6, de las cabeceras publicas de
// DXGI (dxgi.h, dxgi1_2.h ... dxgi1_6.h).
const IID_OUTPUT: Guid = guid(0xae02eedb, 0xc735, 0x4690, [0x8d, 0x52, 0x5a, 0x8d, 0xc2, 0x02, 0x13, 0xaa]);
const IID_OUTPUT1: Guid = guid(0x00cddea8, 0x939b, 0x4b83, [0xa3, 0x40, 0xa6, 0x85, 0x22, 0x66, 0x66, 0xcc]);
const IID_OUTPUT2: Guid = guid(0x595e39d1, 0x2724, 0x4663, [0x99, 0xb1, 0xda, 0x96, 0x9d, 0xe2, 0x83, 0x64]);
const IID_OUTPUT3: Guid = guid(0x8a6bb301, 0x7e7e, 0x41f4, [0xa8, 0xe0, 0x5b, 0x32, 0xf7, 0xf9, 0x9b, 0x18]);
const IID_OUTPUT4: Guid = guid(0xdc7dca35, 0x2196, 0x414d, [0x9f, 0x53, 0x61, 0x78, 0x84, 0x03, 0x2a, 0x60]);
const IID_OUTPUT5: Guid = guid(0x80a07424, 0xab52, 0x42eb, [0x83, 0x3c, 0x0c, 0x42, 0xfd, 0x28, 0x2d, 0x98]);
pub const IID_OUTPUT6: Guid = guid(0x068346e8, 0xaaec, 0x4b84, [0xad, 0xd7, 0x13, 0x7f, 0x51, 0x3f, 0x77, 0xa1]);
const IID_SWAPCHAIN: Guid = guid(0x310d36a0, 0xd2e7, 0x4c0a, [0xaa, 0x04, 0x6a, 0x9d, 0x23, 0xb8, 0x88, 0x6a]);
pub const IID_SWAPCHAIN2: Guid = guid(0xa8be2ac4, 0x199f, 0x4946, [0xb3, 0x31, 0x79, 0x59, 0x9f, 0xb9, 0x8d, 0xe7]);
pub const IID_SWAPCHAIN3: Guid = guid(0x94d99bdb, 0xf1f8, 0x4ab0, [0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1]);
pub const IID_SWAPCHAIN1: Guid = guid(0x790a45f7, 0x0d42, 0x4876, [0x98, 0x3a, 0x0a, 0x55, 0xcf, 0xe6, 0xf4, 0xaa]);

pub static INTERFACES: [Interfaz; 14] = [
    Interfaz { nombre: "ID3D12Device", metodos: M_ID3D12DEVICE, iids: &[IID_DEVICE, IID_OBJECT] },
    Interfaz { nombre: "ID3D12CommandQueue", metodos: M_ID3D12COMMANDQUEUE, iids: &[IID_QUEUE, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12CommandAllocator", metodos: M_ID3D12COMMANDALLOCATOR, iids: &[IID_ALLOCATOR, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12GraphicsCommandList", metodos: M_ID3D12GRAPHICSCOMMANDLIST, iids: &[IID_LIST, IID_COMMANDLIST, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12DescriptorHeap", metodos: M_ID3D12DESCRIPTORHEAP, iids: &[IID_HEAP, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12Resource", metodos: M_ID3D12RESOURCE, iids: &[IID_RESOURCE, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12Fence", metodos: M_ID3D12FENCE, iids: &[IID_FENCE, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "IDXGIFactory6", metodos: M_IDXGIFACTORY6, iids: &[IID_FACTORY6, IID_FACTORY5, IID_FACTORY4, IID_FACTORY3, IID_FACTORY2, IID_FACTORY1, IID_FACTORY, IID_DXGIOBJECT] },
    Interfaz { nombre: "IDXGISwapChain3", metodos: M_IDXGISWAPCHAIN3, iids: &[IID_SWAPCHAIN3, IID_SWAPCHAIN2, IID_SWAPCHAIN1, IID_SWAPCHAIN, IID_DEVICESUBOBJECT, IID_DXGIOBJECT] },
    Interfaz { nombre: "ID3D12RootSignature", metodos: M_ID3D12ROOTSIGNATURE, iids: &[IID_ROOTSIG, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12PipelineState", metodos: M_ID3D12PIPELINESTATE, iids: &[IID_PSO, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3DBlob", metodos: M_ID3D10BLOB, iids: &[IID_BLOB] },
    Interfaz { nombre: "IDXGIAdapter4", metodos: M_IDXGIADAPTER1, iids: &[IID_ADAPTER4, IID_ADAPTER3, IID_ADAPTER2, IID_ADAPTER1, IID_ADAPTER, IID_DXGIOBJECT] },
    Interfaz { nombre: "IDXGIOutput6", metodos: M_IDXGIOUTPUT6, iids: &[IID_OUTPUT6, IID_OUTPUT5, IID_OUTPUT4, IID_OUTPUT3, IID_OUTPUT2, IID_OUTPUT1, IID_OUTPUT, IID_DXGIOBJECT] },
];

/// **La cabecera de todo objeto de la casa.** `repr(C)` y delante: el `.exe`
/// solo mira el primer puntero (la vtabla); lo demas es nuestro.
#[repr(C)]
pub struct Com<T> {
    vtabla: *const u64,
    interfaz: u32,
    refs: u32,
    pub t: T,
}

/// Lo que se sabe de cualquier objeto sin saber su `T`.
#[repr(C)]
struct Cabecera {
    vtabla: *const u64,
    interfaz: u32,
    refs: u32,
}

/// **Un hueco que la casa no tiene.** Uno por (interfaz, metodo): el aviso
/// dice cual, y el proceso sale con `0xC0DE0000 | interfaz << 8 | metodo`.
extern "win64" fn falta<const I: usize, const S: usize>() -> ! {
    let i = &INTERFACES[I];
    aviso(&format!("{}::{} (hueco {}) no esta en la casa", i.nombre, i.metodos.get(S).copied().unwrap_or("?"), S));
    (plataforma().salir)(0xC0DE_0000 | (I as u32) << 8 | S as u32)
}

macro_rules! faltas {
    ($i:ident; $($s:literal)*) => { [$(falta::<$i, $s> as *const () as usize as u64),*] };
}

fn faltas_de<const I: usize>() -> [u64; 60] {
    faltas!(I; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59)
}

extern "win64" fn query_interface(this: *mut Cabecera, riid: *const Guid, ppv: *mut u64) -> i32 {
    if ppv.is_null() {
        return E_POINTER;
    }
    // SAFETY: `this` es un objeto de la casa; `riid`, un GUID del `.exe`.
    let (i, iid) = unsafe { ((*this).interfaz as usize, riid.read_unaligned()) };
    if iid == IID_IUNKNOWN || INTERFACES[i].iids.contains(&iid) {
        add_ref(this);
        // SAFETY: `ppv` es un puntero del `.exe` a donde dejar la interfaz.
        unsafe { *ppv = this as u64 };
        S_OK
    } else {
        // Dicho (02-10): un "no" en silencio aqui es un camino del `.exe` que
        // no se ve (un ID3D12Device5 que no hay y el juego sigue sin el).
        aviso(&format!("{}::QueryInterface {}: la casa no la tiene", INTERFACES[i].nombre, crate::com_basico::clsid_texto(riid as *const u8)));
        // SAFETY: como arriba.
        unsafe { *ppv = 0 };
        E_NOINTERFACE
    }
}

extern "win64" fn add_ref(this: *mut Cabecera) -> u32 {
    // SAFETY: un objeto de la casa.
    unsafe {
        (*this).refs += 1;
        (*this).refs
    }
}

/// Un objeto que llega a cero no se libera: vive lo que el proceso. Un `.exe`
/// que use un objeto soltado no pisa memoria ajena (P3a no reusa nada).
extern "win64" fn release(this: *mut Cabecera) -> u32 {
    // SAFETY: un objeto de la casa.
    unsafe {
        (*this).refs = (*this).refs.saturating_sub(1);
        (*this).refs
    }
}

struct Vtablas(UnsafeCell<[*const u64; 14]>);
// SAFETY: un hilo (ver `Global` en lib.rs).
unsafe impl Sync for Vtablas {}
static VTABLAS: Vtablas = Vtablas(UnsafeCell::new([core::ptr::null(); 14]));

/// **La vtabla de la interfaz `I`**: IUnknown, los `metodos` que la casa
/// tiene (hueco, direccion), y un `falta` en todos los demas. Se arma una vez.
pub fn vtabla<const I: usize>(metodos: &[(usize, u64)]) -> *const u64 {
    // SAFETY: un hilo; ninguna llamada al `.exe` dentro.
    let cache = unsafe { &mut *VTABLAS.0.get() };
    if !cache[I].is_null() {
        return cache[I];
    }
    let n = INTERFACES[I].metodos.len();
    let mut v: Vec<u64> = faltas_de::<I>()[..n].to_vec();
    v[0] = crate::dir!(query_interface);
    v[1] = crate::dir!(add_ref);
    v[2] = crate::dir!(release);
    for &(hueco, f) in metodos {
        v[hueco] = f;
    }
    cache[I] = Box::leak(v.into_boxed_slice()).as_ptr();
    cache[I]
}

/// **Un objeto nuevo** de la interfaz `I`, con una referencia.
pub fn nuevo<T>(interfaz: usize, vt: *const u64, t: T) -> *mut Com<T> {
    Box::leak(Box::new(Com { vtabla: vt, interfaz: interfaz as u32, refs: 1, t }))
}

/// Lo de dentro de un objeto de la casa.
///
/// # Safety
/// `p` es un `Com<T>` de la casa, del `T` que se dice.
pub unsafe fn de<'a, T>(p: u64) -> &'a mut T {
    &mut (*(p as *mut Com<T>)).t
}

/// El IID que pide el `.exe`, y si es uno de los de `I`.
pub fn pide(riid: *const Guid, i: usize) -> bool {
    if riid.is_null() {
        return false;
    }
    // SAFETY: un GUID del `.exe`.
    let g = unsafe { riid.read_unaligned() };
    g == IID_IUNKNOWN || INTERFACES[i].iids.contains(&g)
}

/// Dejar un objeto en el `void **ppv` del `.exe`.
pub fn dar(ppv: *mut u64, obj: u64) -> i32 {
    if ppv.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe` a donde dejar la interfaz.
    unsafe { *ppv = obj };
    S_OK
}

// -- Los metodos de cada interfaz, en el orden de las cabeceras de Windows --

pub const M_ID3D12DEVICE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetNodeCount", "CreateCommandQueue",
    "CreateCommandAllocator", "CreateGraphicsPipelineState", "CreateComputePipelineState",
    "CreateCommandList", "CheckFeatureSupport", "CreateDescriptorHeap",
    "GetDescriptorHandleIncrementSize", "CreateRootSignature", "CreateConstantBufferView",
    "CreateShaderResourceView", "CreateUnorderedAccessView", "CreateRenderTargetView",
    "CreateDepthStencilView", "CreateSampler", "CopyDescriptors", "CopyDescriptorsSimple",
    "GetResourceAllocationInfo", "GetCustomHeapProperties", "CreateCommittedResource",
    "CreateHeap", "CreatePlacedResource", "CreateReservedResource", "CreateSharedHandle",
    "OpenSharedHandle", "OpenSharedHandleByName", "MakeResident", "Evict", "CreateFence",
    "GetDeviceRemovedReason", "GetCopyableFootprints", "CreateQueryHeap", "SetStablePowerState",
    "CreateCommandSignature", "GetResourceTiling", "GetAdapterLuid",
];
pub const M_ID3D12COMMANDQUEUE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "UpdateTileMappings", "CopyTileMappings",
    "ExecuteCommandLists", "SetMarker", "BeginEvent", "EndEvent", "Signal", "Wait",
    "GetTimestampFrequency", "GetClockCalibration", "GetDesc",
];
pub const M_ID3D12COMMANDALLOCATOR: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "Reset",
];
pub const M_ID3D12GRAPHICSCOMMANDLIST: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetType", "Close", "Reset",
    "ClearState", "DrawInstanced", "DrawIndexedInstanced", "Dispatch", "CopyBufferRegion",
    "CopyTextureRegion", "CopyResource", "CopyTiles", "ResolveSubresource",
    "IASetPrimitiveTopology", "RSSetViewports", "RSSetScissorRects", "OMSetBlendFactor",
    "OMSetStencilRef", "SetPipelineState", "ResourceBarrier", "ExecuteBundle",
    "SetDescriptorHeaps", "SetComputeRootSignature", "SetGraphicsRootSignature",
    "SetComputeRootDescriptorTable", "SetGraphicsRootDescriptorTable",
    "SetComputeRoot32BitConstant", "SetGraphicsRoot32BitConstant",
    "SetComputeRoot32BitConstants", "SetGraphicsRoot32BitConstants",
    "SetComputeRootConstantBufferView", "SetGraphicsRootConstantBufferView",
    "SetComputeRootShaderResourceView", "SetGraphicsRootShaderResourceView",
    "SetComputeRootUnorderedAccessView", "SetGraphicsRootUnorderedAccessView",
    "IASetIndexBuffer", "IASetVertexBuffers", "SOSetTargets", "OMSetRenderTargets",
    "ClearDepthStencilView", "ClearRenderTargetView", "ClearUnorderedAccessViewUint",
    "ClearUnorderedAccessViewFloat", "DiscardResource", "BeginQuery", "EndQuery",
    "ResolveQueryData", "SetPredication", "SetMarker", "BeginEvent", "EndEvent",
    "ExecuteIndirect",
];
pub const M_ID3D12DESCRIPTORHEAP: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetDesc",
    "GetCPUDescriptorHandleForHeapStart", "GetGPUDescriptorHandleForHeapStart",
];
pub const M_ID3D12RESOURCE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "Map", "Unmap", "GetDesc",
    "GetGPUVirtualAddress", "WriteToSubresource", "ReadFromSubresource", "GetHeapProperties",
];
pub const M_ID3D12FENCE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetCompletedValue",
    "SetEventOnCompletion", "Signal",
];
pub const M_IDXGIFACTORY6: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "SetPrivateData", "SetPrivateDataInterface",
    "GetPrivateData", "GetParent", "EnumAdapters", "MakeWindowAssociation",
    "GetWindowAssociation", "CreateSwapChain", "CreateSoftwareAdapter", "EnumAdapters1",
    "IsCurrent", "IsWindowedStereoEnabled", "CreateSwapChainForHwnd",
    "CreateSwapChainForCoreWindow", "GetSharedResourceAdapterLuid",
    "RegisterStereoStatusWindow", "RegisterStereoStatusEvent", "UnregisterStereoStatus",
    "RegisterOcclusionStatusWindow", "RegisterOcclusionStatusEvent",
    "UnregisterOcclusionStatus", "CreateSwapChainForComposition",
    // IDXGIFactory3, 4, 5 y 6 (P3c4)
    "GetCreationFlags", "EnumAdapterByLuid", "EnumWarpAdapter", "CheckFeatureSupport",
    "EnumAdapterByGpuPreference",
];
pub const M_IDXGIADAPTER1: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "SetPrivateData", "SetPrivateDataInterface",
    "GetPrivateData", "GetParent", "EnumOutputs", "GetDesc", "CheckInterfaceSupport", "GetDesc1",
    // IDXGIAdapter2, 3 y 4 (02-10)
    "GetDesc2", "RegisterHardwareContentProtectionTeardownStatusEvent",
    "UnregisterHardwareContentProtectionTeardownStatus", "QueryVideoMemoryInfo",
    "SetVideoMemoryReservation", "RegisterVideoMemoryBudgetChangeNotificationEvent",
    "UnregisterVideoMemoryBudgetChangeNotification", "GetDesc3",
];
pub const M_IDXGIOUTPUT6: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "SetPrivateData", "SetPrivateDataInterface",
    "GetPrivateData", "GetParent", "GetDesc", "GetDisplayModeList", "FindClosestMatchingMode",
    "WaitForVBlank", "TakeOwnership", "ReleaseOwnership", "GetGammaControlCapabilities",
    "SetGammaControl", "GetGammaControl", "SetDisplaySurface", "GetDisplaySurfaceData",
    "GetFrameStatistics",
    // IDXGIOutput1 a 6 (01-10)
    "GetDisplayModeList1", "FindClosestMatchingMode1", "GetDisplaySurfaceData1",
    "DuplicateOutput", "SupportsOverlays", "CheckOverlaySupport",
    "CheckOverlayColorSpaceSupport", "DuplicateOutput1", "GetDesc1",
    "CheckHardwareCompositionSupport",
];
pub const M_IDXGISWAPCHAIN3: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "SetPrivateData", "SetPrivateDataInterface",
    "GetPrivateData", "GetParent", "GetDevice", "Present", "GetBuffer", "SetFullscreenState",
    "GetFullscreenState", "GetDesc", "ResizeBuffers", "ResizeTarget", "GetContainingOutput",
    "GetFrameStatistics", "GetLastPresentCount", "GetDesc1", "GetFullscreenDesc", "GetHwnd",
    "GetCoreWindow", "Present1", "IsTemporaryMonoSupported", "GetRestrictToOutput",
    "SetBackgroundColor", "GetBackgroundColor", "SetRotation", "GetRotation",
    // IDXGISwapChain2 y 3 (P3c4)
    "SetSourceSize", "GetSourceSize", "SetMaximumFrameLatency", "GetMaximumFrameLatency",
    "GetFrameLatencyWaitableObject", "SetMatrixTransform", "GetMatrixTransform",
    "GetCurrentBackBufferIndex", "CheckColorSpaceSupport", "SetColorSpace1", "ResizeBuffers1",
];
pub const M_ID3D12ROOTSIGNATURE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice",
];
pub const M_ID3D12PIPELINESTATE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetCachedBlob",
];
pub const M_ID3D10BLOB: &[&str] = &["QueryInterface", "AddRef", "Release", "GetBufferPointer", "GetBufferSize"];
