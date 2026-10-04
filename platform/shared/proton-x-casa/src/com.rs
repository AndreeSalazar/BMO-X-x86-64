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
pub const E_OUTOFMEMORY: i32 = 0x8007_000E_u32 as i32;

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
/// ID3D12Heap: memoria de la tarjeta donde se colocan recursos (no confundir
/// con HEAP, el monton de DESCRIPTORES).
pub const MEMORIA: usize = 14;
/// ID3D12QueryHeap y ID3D12CommandSignature (tanda 47).
pub const CONSULTAS: usize = 15;
pub const FIRMA: usize = 16;
/// 03-10 (N4.5): WASAPI, el sonido del juego (`wasapi`, `wasapi_flujo`).
/// Desde aqui, ninguna hereda de ID3D12Object ni de IDXGIObject.
pub const PRIMERA_DE_SONIDO: usize = 17;
pub const MM_ENUMERADOR: usize = 17;
pub const MM_COLECCION: usize = 18;
pub const MM_APARATO: usize = 19;
pub const MM_PUNTA: usize = 20;
pub const PROPIEDADES: usize = 21;
pub const CLIENTE: usize = 22;
pub const RENDER: usize = 23;
pub const RELOJ: usize = 24;
pub const VOLUMEN: usize = 25;
pub const SESION: usize = 26;
pub const VOLUMEN_FLUJO: usize = 27;
pub const VOLUMEN_CANALES: usize = 28;
/// Cuantas interfaces tiene la casa.
const CUANTAS: usize = 29;
const IID_CONSULTAS: Guid = guid(0x0d9658ae, 0xed45, 0x469e, [0xa6, 0x1d, 0x97, 0x0e, 0xc5, 0x83, 0xca, 0xb4]);
const IID_FIRMA: Guid = guid(0xc36a797c, 0xec80, 0x4f0a, [0x89, 0x85, 0xa7, 0xb2, 0x47, 0x50, 0x82, 0xd1]);

const IID_OBJECT: Guid = guid(0xc4fec28f, 0x7966, 0x4e95, [0x9f, 0x94, 0xf4, 0x31, 0xcb, 0x56, 0xc3, 0xb8]);
const IID_DEVICECHILD: Guid = guid(0x905db94b, 0xa00c, 0x4140, [0x9d, 0xf5, 0x2b, 0x64, 0xca, 0x9e, 0xa3, 0x57]);
const IID_PAGEABLE: Guid = guid(0x63ee58fb, 0x1268, 0x4835, [0x86, 0xda, 0xf0, 0x08, 0xce, 0x62, 0xf0, 0xd6]);
const IID_COMMANDLIST: Guid = guid(0x7116d91c, 0xe7e4, 0x47ce, [0xb8, 0xc6, 0xec, 0x81, 0x68, 0xf4, 0x37, 0xe5]);
pub const IID_DEVICE: Guid = guid(0x189819f1, 0x1db6, 0x4b57, [0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7]);
/// ID3D12Device1 a ID3D12Device10 (tanda 45: Cyberpunk pide la 1, la 4, la
/// 8 y la 10), de `d3d12.idl` (mingw-w64 y vkd3d-proton dicen los mismos).
/// Cada una hereda de la anterior: un objeto, una vtabla de 79 huecos.
const IID_DEVICES: [Guid; 10] = [
    guid(0x77acce80, 0x638e, 0x4e65, [0x88, 0x95, 0xc1, 0xf2, 0x33, 0x86, 0x86, 0x3e]),
    guid(0x30baa41e, 0xb15b, 0x475c, [0xa0, 0xbb, 0x1a, 0xf5, 0xc5, 0xb6, 0x43, 0x28]),
    guid(0x81dadc15, 0x2bad, 0x4392, [0x93, 0xc5, 0x10, 0x13, 0x45, 0xc4, 0xaa, 0x98]),
    guid(0xe865df17, 0xa9ee, 0x46f9, [0xa4, 0x63, 0x30, 0x98, 0x31, 0x5a, 0xa2, 0xe5]),
    guid(0x8b4f173b, 0x2fea, 0x4b80, [0x8f, 0x58, 0x43, 0x07, 0x19, 0x1a, 0xb9, 0x5d]),
    guid(0xc70b221b, 0x40e4, 0x4a17, [0x89, 0xaf, 0x02, 0x5a, 0x07, 0x27, 0xa6, 0xdc]),
    guid(0x5c014b53, 0x68a1, 0x4b9b, [0x8b, 0xd1, 0xdd, 0x60, 0x46, 0xb9, 0x35, 0x8b]),
    guid(0x9218e6bb, 0xf944, 0x4f7e, [0xa7, 0x5c, 0xb1, 0xb2, 0xc7, 0xb7, 0x01, 0xf3]),
    guid(0x4c80e962, 0xf032, 0x4f60, [0xbc, 0x9e, 0xeb, 0xc2, 0xcf, 0xa1, 0xd8, 0x3c]),
    guid(0x517f8718, 0xaa66, 0x49f9, [0xb0, 0x2b, 0xa7, 0xab, 0x89, 0xc0, 0x60, 0x31]),
];
pub const IID_QUEUE: Guid = guid(0x0ec870a6, 0x5d7e, 0x4c22, [0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed]);
pub const IID_ALLOCATOR: Guid = guid(0x6102dee4, 0xaf59, 0x4b09, [0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24]);
pub const IID_LIST: Guid = guid(0x5b160d0f, 0xac1b, 0x4185, [0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55]);
/// ID3D12GraphicsCommandList1 a 10 (tanda 48), de d3d12.idl (mingw-w64 hasta
/// la 7; vkd3d-proton la 8, la 9 y la 10). Un objeto, 86 huecos.
const IID_LISTAS: [Guid; 10] = [
    guid(0x553103fb, 0x1fe7, 0x4557, [0xbb, 0x38, 0x94, 0x6d, 0x7d, 0x0e, 0x7c, 0xa7]),
    guid(0x38c3e585, 0xff17, 0x412c, [0x91, 0x50, 0x4f, 0xc6, 0xf9, 0xd7, 0x2a, 0x28]),
    guid(0x6fda83a7, 0xb84c, 0x4e38, [0x9a, 0xc8, 0xc7, 0xbd, 0x22, 0x01, 0x6b, 0x3d]),
    guid(0x8754318e, 0xd3a9, 0x4541, [0x98, 0xcf, 0x64, 0x5b, 0x50, 0xdc, 0x48, 0x74]),
    guid(0x55050859, 0x4024, 0x474c, [0x87, 0xf5, 0x64, 0x72, 0xea, 0xee, 0x44, 0xea]),
    guid(0xc3827890, 0xe548, 0x4cfa, [0x96, 0xcf, 0x56, 0x89, 0xa9, 0x37, 0x0f, 0x80]),
    guid(0xdd171223, 0x8b61, 0x4769, [0x90, 0xe3, 0x16, 0x0c, 0xcd, 0xe4, 0xe2, 0xc1]),
    guid(0xee936ef9, 0x599d, 0x4d28, [0x93, 0x8e, 0x23, 0xc4, 0xad, 0x05, 0xce, 0x51]),
    guid(0x34ed2808, 0xffe6, 0x4c2b, [0xb1, 0x1a, 0xca, 0xbd, 0x2b, 0x0c, 0x59, 0xe1]),
    guid(0x7013c015, 0xd161, 0x4b63, [0xa0, 0x8c, 0x23, 0x85, 0x52, 0xdd, 0x8a, 0xcc]),
];
const IID_RESOURCE1: Guid = guid(0x9d5e227a, 0x4430, 0x4161, [0x88, 0xb3, 0x3e, 0xca, 0x6b, 0xb1, 0x6e, 0x19]);
const IID_RESOURCE2: Guid = guid(0xbe36ec3b, 0xea85, 0x4aeb, [0xa4, 0x5a, 0xe9, 0xd7, 0x64, 0x04, 0xa4, 0x95]);
const IID_FENCE1: Guid = guid(0x433685fe, 0xe22b, 0x4ca0, [0xa8, 0xdb, 0xb5, 0xb4, 0xf4, 0xdd, 0x0e, 0x4a]);
const IID_MEMORIA1: Guid = guid(0x572f7389, 0x2168, 0x49e3, [0x96, 0x93, 0xd6, 0xdf, 0x58, 0x71, 0xbf, 0x6d]);
const IID_SWAPCHAIN4: Guid = guid(0x3d585d5a, 0xbd4a, 0x489e, [0xb1, 0xf4, 0x3d, 0xbc, 0xb6, 0x45, 0x2f, 0xfb]);
const IID_FACTORY7: Guid = guid(0xa4966eed, 0x76db, 0x44da, [0x84, 0xc1, 0xee, 0x9a, 0x7a, 0xfb, 0x20, 0xa8]);
pub const IID_HEAP: Guid = guid(0x8efb471d, 0x616c, 0x4f49, [0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51]);
pub const IID_RESOURCE: Guid = guid(0x696442be, 0xa72e, 0x4059, [0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad]);
pub const IID_FENCE: Guid = guid(0x0a753dcf, 0xc4d8, 0x4b91, [0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76]);
pub const IID_FACTORY1: Guid = guid(0x770aae78, 0xf26f, 0x4dba, [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87]);
pub const IID_FACTORY2: Guid = guid(0x50c83a1c, 0xe072, 0x4c48, [0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0]);
pub const IID_ROOTSIG: Guid = guid(0xc54a6b66, 0x72df, 0x4ee8, [0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14]);
pub const IID_PSO: Guid = guid(0x765a30f3, 0xf624, 0x4c6f, [0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45]);
// 02-10: el monton de memoria (d3d12.h), lo que Cyberpunk crea en seguida.
pub const IID_MEMORIA: Guid = guid(0x6b3b2502, 0x6e51, 0x45b3, [0x90, 0xee, 0x98, 0x84, 0x26, 0x5e, 0x8d, 0xf3]);
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

pub static INTERFACES: [Interfaz; CUANTAS] = [
    Interfaz {
        nombre: "ID3D12Device",
        metodos: M_ID3D12DEVICE,
        iids: &[IID_DEVICE, IID_DEVICES[0], IID_DEVICES[1], IID_DEVICES[2], IID_DEVICES[3], IID_DEVICES[4], IID_DEVICES[5], IID_DEVICES[6], IID_DEVICES[7], IID_DEVICES[8], IID_DEVICES[9], IID_OBJECT],
    },
    Interfaz { nombre: "ID3D12CommandQueue", metodos: M_ID3D12COMMANDQUEUE, iids: &[IID_QUEUE, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12CommandAllocator", metodos: M_ID3D12COMMANDALLOCATOR, iids: &[IID_ALLOCATOR, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz {
        nombre: "ID3D12GraphicsCommandList",
        metodos: M_ID3D12GRAPHICSCOMMANDLIST,
        iids: &[IID_LIST, IID_LISTAS[0], IID_LISTAS[1], IID_LISTAS[2], IID_LISTAS[3], IID_LISTAS[4], IID_LISTAS[5], IID_LISTAS[6], IID_LISTAS[7], IID_LISTAS[8], IID_LISTAS[9], IID_COMMANDLIST, IID_DEVICECHILD, IID_OBJECT],
    },
    Interfaz { nombre: "ID3D12DescriptorHeap", metodos: M_ID3D12DESCRIPTORHEAP, iids: &[IID_HEAP, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12Resource", metodos: M_ID3D12RESOURCE, iids: &[IID_RESOURCE, IID_RESOURCE1, IID_RESOURCE2, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12Fence", metodos: M_ID3D12FENCE, iids: &[IID_FENCE, IID_FENCE1, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "IDXGIFactory6", metodos: M_IDXGIFACTORY6, iids: &[IID_FACTORY7, IID_FACTORY6, IID_FACTORY5, IID_FACTORY4, IID_FACTORY3, IID_FACTORY2, IID_FACTORY1, IID_FACTORY, IID_DXGIOBJECT] },
    Interfaz { nombre: "IDXGISwapChain3", metodos: M_IDXGISWAPCHAIN3, iids: &[IID_SWAPCHAIN4, IID_SWAPCHAIN3, IID_SWAPCHAIN2, IID_SWAPCHAIN1, IID_SWAPCHAIN, IID_DEVICESUBOBJECT, IID_DXGIOBJECT] },
    Interfaz { nombre: "ID3D12RootSignature", metodos: M_ID3D12ROOTSIGNATURE, iids: &[IID_ROOTSIG, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12PipelineState", metodos: M_ID3D12PIPELINESTATE, iids: &[IID_PSO, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3DBlob", metodos: M_ID3D10BLOB, iids: &[IID_BLOB] },
    Interfaz { nombre: "IDXGIAdapter4", metodos: M_IDXGIADAPTER1, iids: &[IID_ADAPTER4, IID_ADAPTER3, IID_ADAPTER2, IID_ADAPTER1, IID_ADAPTER, IID_DXGIOBJECT] },
    Interfaz { nombre: "IDXGIOutput6", metodos: M_IDXGIOUTPUT6, iids: &[IID_OUTPUT6, IID_OUTPUT5, IID_OUTPUT4, IID_OUTPUT3, IID_OUTPUT2, IID_OUTPUT1, IID_OUTPUT, IID_DXGIOBJECT] },
    Interfaz { nombre: "ID3D12Heap", metodos: M_ID3D12HEAP, iids: &[IID_MEMORIA, IID_MEMORIA1, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12QueryHeap", metodos: M_PAGEABLE, iids: &[IID_CONSULTAS, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "ID3D12CommandSignature", metodos: M_PAGEABLE, iids: &[IID_FIRMA, IID_PAGEABLE, IID_DEVICECHILD, IID_OBJECT] },
    Interfaz { nombre: "IMMDeviceEnumerator", metodos: crate::wasapi::M_ENUMERADOR, iids: &[crate::wasapi::IID_ENUMERADOR] },
    Interfaz { nombre: "IMMDeviceCollection", metodos: crate::wasapi::M_COLECCION, iids: &[crate::wasapi::IID_COLECCION] },
    Interfaz { nombre: "IMMDevice", metodos: crate::wasapi::M_APARATO, iids: &[crate::wasapi::IID_APARATO] },
    Interfaz { nombre: "IMMEndpoint", metodos: crate::wasapi::M_PUNTA, iids: &[crate::wasapi::IID_PUNTA] },
    Interfaz { nombre: "IPropertyStore", metodos: crate::wasapi::M_PROPIEDADES, iids: &[crate::wasapi::IID_PROPIEDADES] },
    Interfaz {
        nombre: "IAudioClient3",
        metodos: crate::wasapi_flujo::M_CLIENTE,
        iids: &[crate::wasapi_flujo::IID_CLIENTE3, crate::wasapi_flujo::IID_CLIENTE2, crate::wasapi_flujo::IID_CLIENTE],
    },
    Interfaz { nombre: "IAudioRenderClient", metodos: crate::wasapi_flujo::M_RENDER, iids: &[crate::wasapi_flujo::IID_RENDER] },
    Interfaz { nombre: "IAudioClock", metodos: crate::wasapi_flujo::M_RELOJ, iids: &[crate::wasapi_flujo::IID_RELOJ] },
    Interfaz { nombre: "ISimpleAudioVolume", metodos: crate::wasapi_flujo::M_VOLUMEN, iids: &[crate::wasapi_flujo::IID_VOLUMEN] },
    Interfaz { nombre: "IAudioSessionControl2", metodos: crate::wasapi_flujo::M_SESION, iids: &[crate::wasapi_flujo::IID_SESION2, crate::wasapi_flujo::IID_SESION] },
    Interfaz { nombre: "IAudioStreamVolume", metodos: crate::wasapi_flujo::M_VOLUMEN_CANALES, iids: &[crate::wasapi_flujo::IID_VOLUMEN_FLUJO] },
    Interfaz { nombre: "IChannelAudioVolume", metodos: crate::wasapi_flujo::M_VOLUMEN_CANALES, iids: &[crate::wasapi_flujo::IID_VOLUMEN_CANALES] },
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

fn faltas_de<const I: usize>() -> [u64; HUECOS] {
    faltas!(I; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59
        60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87)
}

/// Los huecos de la vtabla mas larga (ID3D12GraphicsCommandList10: 86), con margen.
pub(crate) const HUECOS: usize = 88;

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

/// `AddRef` de un objeto de la casa, desde dentro (lo que se da dos veces).
pub(crate) fn add_ref_de(obj: u64) {
    add_ref(obj as *mut Cabecera);
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

struct Vtablas(UnsafeCell<[*const u64; CUANTAS]>);
// SAFETY: un hilo (ver `Global` en lib.rs).
unsafe impl Sync for Vtablas {}
static VTABLAS: Vtablas = Vtablas(UnsafeCell::new([core::ptr::null(); CUANTAS]));

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
    // Tanda 47: ID3D12Object / IDXGIObject de su familia; los suyos mandan.
    for &(hueco, f) in crate::com_objeto::genericos(I) {
        if hueco < n {
            v[hueco] = f;
        }
    }
    // Tanda 48: las fallas documentadas (ver `fallas`), tambien debajo.
    crate::fallas::aplicar::<I>(&mut v);
    for &(hueco, f) in metodos {
        v[hueco] = f;
    }
    cache[I] = Box::leak(v.into_boxed_slice()).as_ptr();
    cache[I]
}

/// **Que es un hueco de una vtabla** (el censo del ABI, 03-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hueco {
    /// La casa lo hace.
    Hace,
    /// Una falla documentada (`fallas.rs`): el HRESULT de Windows sin esa
    /// funcion, dicho.
    Falla,
    /// Un `falta`: dice cual es y sale.
    Falta,
}

macro_rules! faltas_por_indice {
    ($i:expr; $($n:literal)*) => {
        match $i {
            $($n => faltas_de::<$n>(),)*
            _ => [0; HUECOS],
        }
    };
}

/// **El censo del ABI**: de cada interfaz cuya vtabla ya se armo (la casa
/// crea cada una la primera vez que da un objeto de ella), su nombre y lo
/// que es cada metodo, por hueco. Lo usa `tests/abi.rs` para escribir
/// `docs/maestro/D3D12_MAESTRO.md`.
pub fn censo() -> Vec<(&'static str, Vec<(&'static str, Hueco)>)> {
    // SAFETY: un hilo; solo se lee.
    let cache = unsafe { &*VTABLAS.0.get() };
    let mut v = Vec::new();
    for (i, &vt) in cache.iter().enumerate() {
        if vt.is_null() {
            continue;
        }
        let faltas = faltas_por_indice!(i; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28);
        let fallas = crate::fallas::direcciones(i);
        let metodos = INTERFACES[i].metodos;
        let huecos = metodos
            .iter()
            .enumerate()
            .map(|(h, &m)| {
                // SAFETY: la vtabla de la casa mide lo que su lista de metodos.
                let d = unsafe { vt.add(h).read() };
                let estado = if d == faltas[h] {
                    Hueco::Falta
                } else if d == fallas[h] {
                    Hueco::Falla
                } else {
                    Hueco::Hace
                };
                (m, estado)
            })
            .collect();
        v.push((INTERFACES[i].nombre, huecos));
    }
    v
}

/// **Un objeto nuevo** de la interfaz `I`, con una referencia.
pub fn nuevo<T>(interfaz: usize, vt: *const u64, t: T) -> *mut Com<T> {
    let p = Box::leak(Box::new(Com { vtabla: vt, interfaz: interfaz as u32, refs: 1, t }));
    crate::com_objeto::nacio(interfaz, p as *mut Com<T> as u64);
    p
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
    // ID3D12Device1 a ID3D12Device10 (tanda 45).
    "CreatePipelineLibrary", "SetEventOnMultipleFenceCompletion", "SetResidencyPriority",
    "CreatePipelineState", "OpenExistingHeapFromAddress", "OpenExistingHeapFromFileMapping",
    "EnqueueMakeResident", "CreateCommandList1", "CreateProtectedResourceSession",
    "CreateCommittedResource1", "CreateHeap1", "CreateReservedResource1",
    "GetResourceAllocationInfo1", "CreateLifetimeTracker", "RemoveDevice",
    "EnumerateMetaCommands", "EnumerateMetaCommandParameters", "CreateMetaCommand",
    "CreateStateObject", "GetRaytracingAccelerationStructurePrebuildInfo",
    "CheckDriverMatchingIdentifier", "SetBackgroundProcessingMode", "AddToStateObject",
    "CreateProtectedResourceSession1", "GetResourceAllocationInfo2", "CreateCommittedResource2",
    "CreatePlacedResource1", "CreateSamplerFeedbackUnorderedAccessView", "GetCopyableFootprints1",
    "CreateShaderCacheSession", "ShaderCacheControl", "CreateCommandQueue1",
    "CreateCommittedResource3", "CreatePlacedResource2", "CreateReservedResource2",
];
/// Lo de un ID3D12Pageable sin metodos propios (QueryHeap, CommandSignature).
pub const M_PAGEABLE: &[&str] = &["QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData", "SetPrivateDataInterface", "SetName", "GetDevice"];
pub const M_ID3D12HEAP: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetDesc",
    // ID3D12Heap1 (tanda 48).
    "GetProtectedResourceSession",
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
    // ID3D12GraphicsCommandList1 a 10 (tanda 48).
    "AtomicCopyBufferUINT", "AtomicCopyBufferUINT64", "OMSetDepthBounds", "SetSamplePositions",
    "ResolveSubresourceRegion", "SetViewInstanceMask", "WriteBufferImmediate",
    "SetProtectedResourceSession", "BeginRenderPass", "EndRenderPass", "InitializeMetaCommand",
    "ExecuteMetaCommand", "BuildRaytracingAccelerationStructure",
    "EmitRaytracingAccelerationStructurePostbuildInfo", "CopyRaytracingAccelerationStructure",
    "SetPipelineState1", "DispatchRays", "RSSetShadingRate", "RSSetShadingRateImage",
    "DispatchMesh", "Barrier", "OMSetFrontAndBackStencilRef", "RSSetDepthBias",
    "IASetIndexBufferStripCutValue", "SetProgram", "DispatchGraph",
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
    // ID3D12Resource1 y 2 (tanda 48).
    "GetProtectedResourceSession", "GetDesc1",
];
pub const M_ID3D12FENCE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetPrivateData", "SetPrivateData",
    "SetPrivateDataInterface", "SetName", "GetDevice", "GetCompletedValue",
    "SetEventOnCompletion", "Signal",
    // ID3D12Fence1 (tanda 48).
    "GetCreationFlags",
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
    // IDXGIFactory7 (tanda 48).
    "RegisterAdaptersChangedEvent", "UnregisterAdaptersChangedEvent",
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
    // IDXGISwapChain4 (tanda 48).
    "SetHDRMetaData",
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
