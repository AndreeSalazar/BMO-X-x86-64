//! **`ID3D12Object` e `IDXGIObject`, para TODOS los objetos de la casa**
//! (tanda 47, 02-10).
//!
//! Por el inventario contra vkd3d-proton: los huecos 3 a 7 de cada interfaz
//! (datos privados, nombre, dispositivo, padre) no los tenia NINGUN objeto, y
//! un motor grande llama `SetName` a cada cosa que crea (para sus volcados y
//! para PIX): seria un `0xC0DE....` por objeto. Aqui, una vez para todos:
//!
//! ```text
//!    D3D12 (ID3D12Object)       3 GetPrivateData  4 SetPrivateData
//!                               5 SetPrivateDataInterface  6 SetName
//!    hijos del dispositivo      7 GetDevice: el dispositivo, por su riid
//!    DXGI (IDXGIObject)         3 SetPrivateData  4 SetPrivateDataInterface
//!                               5 GetPrivateData  6 GetParent
//! ```
//!
//! Los datos van por (objeto, GUID), como en vkd3d-proton. `SetName` NO es
//! `WKPDID_D3DDebugObjectNameW` (eso hace vkd3d-proton): en Windows, tras
//! SetName, GetPrivateData de ese GUID dice DXGI_ERROR_NOT_FOUND y 0 bytes
//! (tanda47 en la 3060 del propietario, 02-10). La casa guarda el nombre
//! con una clave suya, que el `.exe` no conoce; una interfaz guardada
//! lleva su AddRef, y GetPrivateData da otra. El padre DXGI: del adaptador y
//! de la cadena, la fabrica; de la salida, el adaptador; la fabrica no tiene.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::com::{self, Guid, E_INVALIDARG, E_NOINTERFACE, S_OK};
use crate::dir;

const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002_u32 as i32;
const DXGI_ERROR_MORE_DATA: i32 = 0x887A_0003_u32 as i32;
/// La clave del nombre de `SetName`: de la casa ("BMOX" y "nombre"), no
/// WKPDID_D3DDebugObjectNameW, que Windows no llena con SetName.
const NOMBRE: Guid = com::guid(0x424d_4f58, 0x6e6f, 0x6d62, [0x72, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);

enum Dato {
    Bytes(Vec<u8>),
    /// Un IUnknown del `.exe` (o de la casa), con su referencia.
    Interfaz(u64),
}

struct Estado {
    datos: Vec<(u64, Guid, Dato)>,
    /// El ultimo dispositivo, fabrica y adaptador que se crearon.
    dispositivo: u64,
    fabrica: u64,
    adaptador: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y nada de aqui
// cede el turno (las llamadas al `.exe` son AddRef/Release/QueryInterface).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { datos: Vec::new(), dispositivo: 0, fabrica: 0, adaptador: 0 }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.datos.clear();
    e.dispositivo = 0;
    e.fabrica = 0;
    e.adaptador = 0;
}

/// Lo llama `com::nuevo`: quien es el dispositivo, la fabrica y el adaptador.
pub(crate) fn nacio(interfaz: usize, obj: u64) {
    let e = estado();
    match interfaz {
        com::DEVICE => e.dispositivo = obj,
        com::FACTORY => e.fabrica = obj,
        com::ADAPTER => e.adaptador = obj,
        _ => {}
    }
}

/// **Los huecos de la familia de `I`** (los pone `com::vtabla` antes de los
/// suyos, que mandan).
pub(crate) fn genericos(i: usize) -> &'static [(usize, u64)] {
    struct Tablas(UnsafeCell<[Option<[(usize, u64); 5]>; 3]>);
    // SAFETY: como `Global`.
    unsafe impl Sync for Tablas {}
    static T: Tablas = Tablas(UnsafeCell::new([None, None, None]));
    // SAFETY: como `Global`; se llena una vez.
    let t = unsafe { &mut *T.0.get() };
    let (k, n) = match i {
        com::BLOB => return &[],
        com::DEVICE => (0, 4),
        com::FACTORY | com::ADAPTER | com::OUTPUT | com::SWAPCHAIN => (2, 4),
        _ => (1, 5),
    };
    let fila = t[k].get_or_insert_with(|| {
        if k == 2 {
            [(3, dir!(set_private_data)), (4, dir!(set_private_data_interface)), (5, dir!(get_private_data)), (6, dir!(get_parent)), (0, 0)]
        } else {
            [(3, dir!(get_private_data)), (4, dir!(set_private_data)), (5, dir!(set_private_data_interface)), (6, dir!(set_name)), (7, dir!(get_device))]
        }
    });
    &fila[..n]
}

/// La funcion del hueco `k` de la vtabla de `obj`.
///
/// # Safety
/// `obj` es un objeto COM vivo.
unsafe fn hueco(obj: u64, k: usize) -> u64 {
    (*(obj as *const *const u64)).add(k).read()
}

fn add_ref(obj: u64) {
    // SAFETY: un IUnknown que se guardo vivo (con su referencia).
    unsafe {
        let f: extern "win64" fn(u64) -> u32 = core::mem::transmute(hueco(obj, 1) as usize);
        f(obj);
    }
}

fn release(obj: u64) {
    // SAFETY: como arriba.
    unsafe {
        let f: extern "win64" fn(u64) -> u32 = core::mem::transmute(hueco(obj, 2) as usize);
        f(obj);
    }
}

fn quitar(obj: u64, g: &Guid) {
    let e = estado();
    if let Some(i) = e.datos.iter().position(|(o, x, _)| *o == obj && x == g) {
        if let (_, _, Dato::Interfaz(p)) = e.datos.remove(i) {
            release(p);
        }
    }
}

fn leer_guid(g: *const Guid) -> Option<Guid> {
    // SAFETY: un GUID del `.exe`, si no es nulo.
    (!g.is_null()).then(|| unsafe { g.read_unaligned() })
}

extern "win64" fn set_private_data(this: u64, g: *const Guid, n: u32, datos: *const u8) -> i32 {
    let Some(g) = leer_guid(g) else { return E_INVALIDARG };
    if n != 0 && datos.is_null() {
        return E_INVALIDARG;
    }
    quitar(this, &g);
    if !datos.is_null() {
        // SAFETY: `n` bytes del `.exe`.
        let v = unsafe { core::slice::from_raw_parts(datos, n as usize) }.to_vec();
        estado().datos.push((this, g, Dato::Bytes(v)));
    }
    S_OK
}

extern "win64" fn set_private_data_interface(this: u64, g: *const Guid, p: u64) -> i32 {
    let Some(g) = leer_guid(g) else { return E_INVALIDARG };
    quitar(this, &g);
    if p != 0 {
        add_ref(p);
        estado().datos.push((this, g, Dato::Interfaz(p)));
    }
    S_OK
}

extern "win64" fn get_private_data(this: u64, g: *const Guid, n: *mut u32, datos: *mut u8) -> i32 {
    let Some(g) = leer_guid(g) else { return E_INVALIDARG };
    if n.is_null() {
        return E_INVALIDARG;
    }
    let Some((_, _, d)) = estado().datos.iter().find(|(o, x, _)| *o == this && *x == g) else {
        // SAFETY: el UINT del `.exe`.
        unsafe { n.write_unaligned(0) };
        return DXGI_ERROR_NOT_FOUND;
    };
    let (bytes, interfaz): (&[u8], Option<u64>) = match d {
        Dato::Bytes(v) => (v, None),
        Dato::Interfaz(p) => (&[], Some(*p)),
    };
    let medida = if interfaz.is_some() { 8 } else { bytes.len() as u32 };
    // SAFETY: el UINT del `.exe`.
    let cabe = unsafe { n.read_unaligned() };
    // SAFETY: como arriba.
    unsafe { n.write_unaligned(medida) };
    if datos.is_null() {
        return S_OK;
    }
    if cabe < medida {
        return DXGI_ERROR_MORE_DATA;
    }
    // SAFETY: `cabe` >= `medida` bytes del `.exe`.
    unsafe {
        match interfaz {
            Some(p) => {
                add_ref(p);
                (datos as *mut u64).write_unaligned(p);
            }
            None => core::ptr::copy_nonoverlapping(bytes.as_ptr(), datos, bytes.len()),
        }
    }
    S_OK
}

/// `SetName(this, nombre)`: el nombre (UTF-16 con su 0), con la clave de la
/// casa; un nombre nulo lo quita.
extern "win64" fn set_name(this: u64, nombre: *const u16) -> i32 {
    quitar(this, &NOMBRE);
    if nombre.is_null() {
        return S_OK;
    }
    let mut n = 0usize;
    // SAFETY: una cadena UTF-16 terminada en 0 del `.exe`.
    unsafe {
        while nombre.add(n).read_unaligned() != 0 {
            n += 1;
        }
        let v = core::slice::from_raw_parts(nombre as *const u8, 2 * (n + 1)).to_vec();
        estado().datos.push((this, NOMBRE, Dato::Bytes(v)));
    }
    S_OK
}

/// QueryInterface sobre `obj`, por su vtabla.
fn pedir(obj: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return E_INVALIDARG;
    }
    if obj == 0 {
        // SAFETY: el `void **` del `.exe`.
        unsafe { *pp = 0 };
        return E_NOINTERFACE;
    }
    // SAFETY: un objeto de la casa vivo (los de la casa no se liberan).
    unsafe {
        let f: extern "win64" fn(u64, *const Guid, *mut u64) -> i32 = core::mem::transmute(hueco(obj, 0) as usize);
        f(obj, riid, pp)
    }
}

/// `GetDevice(this, riid, pp)` de un hijo del dispositivo.
pub(crate) extern "win64" fn get_device(_this: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    pedir(estado().dispositivo, riid, pp)
}

/// `GetParent(this, riid, pp)` de un objeto DXGI.
extern "win64" fn get_parent(this: u64, riid: *const Guid, pp: *mut u64) -> i32 {
    // SAFETY: un objeto de la casa: su interfaz va tras la vtabla (`Com`).
    let interfaz = unsafe { (this as *const u32).add(2).read() } as usize;
    let e = estado();
    let padre = match interfaz {
        com::ADAPTER | com::SWAPCHAIN => e.fabrica,
        com::OUTPUT => e.adaptador,
        _ => 0,
    };
    pedir(padre, riid, pp)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_familia_tiene_sus_huecos() {
        let h = |i| genericos(i).iter().map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(h(com::DEVICE), [3, 4, 5, 6]);
        assert_eq!(h(com::LIST), [3, 4, 5, 6, 7]);
        assert_eq!(h(com::FACTORY), [3, 4, 5, 6]);
        assert!(h(com::BLOB).is_empty());
        // El 3 de D3D12 es GetPrivateData; el de DXGI, SetPrivateData.
        assert_eq!(genericos(com::DEVICE)[0].1, dir!(get_private_data));
        assert_eq!(genericos(com::ADAPTER)[0].1, dir!(set_private_data));
    }
}
