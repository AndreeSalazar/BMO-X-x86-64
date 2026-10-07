//! **WASAPI: el aparato** (03-10, N4.5): `MMDeviceEnumerator` y lo que cuelga
//! de el -- el dispositivo de salida que un juego busca para sonar.
//!
//! [carril]  AMARILLO  abre el sonido de la plataforma (en BMO-X, reclama el
//!                     audifono para este proceso)
//! [cuesta]  APARATO   un aparato mal descrito y el juego se queda mudo o
//!                     abre otro formato
//! [riesgo]  ESPEJO    los huecos y los GUID son los de mmdeviceapi.h,
//!                     audioclient.h y functiondiscoverykeys_devpkey.h; el
//!                     banco los recorre por la vtabla como un `.exe`
//! [consumo] NADA      solo cuando el juego pregunta por el sonido
//!
//! # Por que
//!
//! En el metal (03-10, cuarta corrida) Cyberpunk pidio
//! `CoCreateInstance {BCDE0395-E52F-467C-8E3D-C4579291692E}` -- el
//! `MMDeviceEnumerator` -- y la casa contesto "no tengo esa clase": su motor
//! de sonido (Wwise) se quedo sin aparato y el juego, MUDO. Y sus videos (la
//! entrada, Bink) cuelgan del reloj del sonido.
//!
//! ```text
//!    el enumerador     un aparato de SALIDA, activo, por defecto en los tres
//!                      papeles; de entrada (micro), ninguno: E_NOTFOUND
//!    el aparato        Activate da un IAudioClient (`wasapi_flujo`); su
//!                      nombre, su formato y sus altavoces, por propiedades
//!    sin aparato       si la plataforma no tiene sonido (o el audifono lo
//!                      tiene otro), el aparato EXISTE igual y su reloj corre
//!                      sin sonar: el juego sigue a su paso, mudo, en vez de
//!                      buscar otro camino que nadie ha probado
//! ```

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::com::{self, dar, guid, nuevo, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, E_POINTER, S_OK};
use crate::{aviso, dir, plataforma};

/// `CLSID_MMDeviceEnumerator`.
pub const CLSID_ENUMERADOR: Guid = guid(0xBCDE_0395, 0xE52F, 0x467C, [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]);
pub const IID_ENUMERADOR: Guid = guid(0xA956_64D2, 0x9614, 0x4F35, [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6]);
pub const IID_COLECCION: Guid = guid(0x0BD7_A1BE, 0x7A1A, 0x44DB, [0x83, 0x97, 0xCC, 0x53, 0x92, 0x38, 0x7B, 0x5E]);
pub const IID_APARATO: Guid = guid(0xD666_063F, 0x1587, 0x4E43, [0x81, 0xF1, 0xB9, 0x48, 0xE8, 0x07, 0x36, 0x3F]);
pub const IID_PUNTA: Guid = guid(0x1BE0_9788, 0x6894, 0x4089, [0x85, 0x86, 0x9A, 0x2A, 0x6C, 0x26, 0x5A, 0xC5]);
pub const IID_PROPIEDADES: Guid = guid(0x886D_8EEB, 0x8CF2, 0x4446, [0x8D, 0x02, 0xCD, 0xBA, 0x1D, 0xBD, 0xCF, 0x99]);

/// `E_NOTFOUND` (HRESULT_FROM_WIN32(ERROR_NOT_FOUND)) y el de una propiedad
/// que no se deja escribir.
const E_NOTFOUND: i32 = 0x8007_0490_u32 as i32;
const STG_E_ACCESSDENIED: i32 = 0x8003_0005_u32 as i32;

/// `EDataFlow` y `DEVICE_STATE_ACTIVE`.
const SALIDA: u32 = 0;
const TODOS: u32 = 2;
const ACTIVO: u32 = 1;

/// La frecuencia si la plataforma no tiene sonido: la de un PC cualquiera.
pub const HZ_SIN_APARATO: u32 = 48_000;

pub const M_ENUMERADOR: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "EnumAudioEndpoints", "GetDefaultAudioEndpoint", "GetDevice",
    "RegisterEndpointNotificationCallback", "UnregisterEndpointNotificationCallback",
];
pub const M_COLECCION: &[&str] = &["QueryInterface", "AddRef", "Release", "GetCount", "Item"];
pub const M_APARATO: &[&str] = &["QueryInterface", "AddRef", "Release", "Activate", "OpenPropertyStore", "GetId", "GetState"];
pub const M_PUNTA: &[&str] = &["QueryInterface", "AddRef", "Release", "GetDataFlow"];
pub const M_PROPIEDADES: &[&str] = &["QueryInterface", "AddRef", "Release", "GetCount", "GetAt", "GetValue", "SetValue", "Commit"];

/// El nombre (id) del aparato, el que dan `GetId` y espera `GetDevice`.
const ID: &str = "{0.0.0.00000000}.{b0b0b0b0-0000-4000-8000-424d4f2d5821}";

pub(crate) struct Estado {
    /// La frecuencia del aparato (`None`: aun no se pregunto).
    hz: Option<u32>,
    /// Si la plataforma abrio su sonido (y hay que cerrarlo al acabar).
    abierto: bool,
    /// Los objetos, uno de cada (se crean al pedirlos).
    aparato: u64,
    punta: u64,
    propiedades: u64,
    /// Los IAudioClient creados, para el latido (`wasapi_flujo::latir`).
    pub(crate) flujos: Vec<u64>,
    /// El flujo que suena por el aparato de verdad (0: ninguno).
    pub(crate) propietario: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: la casa corre en un hilo a la vez (ver `Global` en lib.rs).
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { hz: None, abierto: false, aparato: 0, punta: 0, propiedades: 0, flujos: Vec::new(), propietario: 0 }));

pub(crate) fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    if e.abierto {
        if let Some(s) = plataforma().sonido {
            (s.cerrar)();
        }
    }
    *e = Estado { hz: None, abierto: false, aparato: 0, punta: 0, propiedades: 0, flujos: Vec::new(), propietario: 0 };
}

/// **La frecuencia del aparato**: la primera vez se abre el sonido de la
/// plataforma; si no hay, [`HZ_SIN_APARATO`] y un reloj que no suena.
pub(crate) fn hz() -> u32 {
    let e = estado();
    if let Some(h) = e.hz {
        return h;
    }
    let abierto = plataforma().sonido.and_then(|s| (s.abrir)());
    e.abierto = abierto.is_some();
    let h = abierto.filter(|&h| h > 0).unwrap_or(HZ_SIN_APARATO);
    if !e.abierto {
        aviso("WASAPI: la plataforma no tiene sonido (o el audifono lo tiene otro): el juego corre con un reloj que no suena");
    }
    e.hz = Some(h);
    h
}

/// Si suena de verdad (hay aparato abierto).
pub(crate) fn hay_aparato() -> bool {
    hz();
    estado().abierto
}

/// **`CoCreateInstance(CLSID_MMDeviceEnumerator)`**: el enumerador, o
/// E_NOINTERFACE si pide otra interfaz.
pub(crate) fn crear_enumerador(iid: *const Guid, sale: *mut u64) -> i32 {
    if !com::pide(iid, com::MM_ENUMERADOR) {
        return E_NOINTERFACE;
    }
    let vt = vtabla::<{ com::MM_ENUMERADOR }>(&[
        (3, dir!(enum_audio_endpoints)),
        (4, dir!(get_default_audio_endpoint)),
        (5, dir!(get_device)),
        (6, dir!(sin_avisos)),
        (7, dir!(sin_avisos)),
    ]);
    dar(sale, nuevo(com::MM_ENUMERADOR, vt, ()) as u64)
}

/// El aparato (uno, el mismo siempre).
fn aparato() -> u64 {
    let e = estado();
    if e.aparato == 0 {
        let vt = vtabla::<{ com::MM_APARATO }>(&[(0, dir!(aparato_query_interface)), (3, dir!(activate)), (4, dir!(open_property_store)), (5, dir!(get_id)), (6, dir!(get_state))]);
        e.aparato = nuevo(com::MM_APARATO, vt, ()) as u64;
    }
    e.aparato
}

/// Un texto en memoria de `CoTaskMemAlloc` (UTF-16 con su cero): lo suelta
/// el `.exe` con `CoTaskMemFree` (o `PropVariantClear`).
pub(crate) fn texto_de_tarea(s: &str) -> u64 {
    let w: Vec<u16> = s.encode_utf16().chain(core::iter::once(0)).collect();
    let p = crate::dll_chicas::co_task_mem_alloc(2 * w.len() as u64);
    if p != 0 {
        // SAFETY: `2 * w.len()` bytes recien pedidos.
        unsafe { core::ptr::copy_nonoverlapping(w.as_ptr(), p as *mut u16, w.len()) };
    }
    p
}

// -- IMMDeviceEnumerator ----------------------------------------------------

extern "win64" fn enum_audio_endpoints(_this: u64, flujo: u32, estados: u32, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    let n = ((flujo == SALIDA || flujo == TODOS) && estados & ACTIVO != 0) as u32;
    let vt = vtabla::<{ com::MM_COLECCION }>(&[(3, dir!(get_count)), (4, dir!(item))]);
    dar(sale, nuevo(com::MM_COLECCION, vt, n) as u64)
}

extern "win64" fn get_default_audio_endpoint(_this: u64, flujo: u32, _papel: u32, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    if flujo != SALIDA {
        // SAFETY: un puntero del `.exe`.
        unsafe { sale.write(0) };
        return E_NOTFOUND;
    }
    com::add_ref_de(aparato());
    dar(sale, aparato())
}

extern "win64" fn get_device(_this: u64, id: *const u16, sale: *mut u64) -> i32 {
    if sale.is_null() || id.is_null() {
        return E_POINTER;
    }
    let mut n = 0;
    // SAFETY: un texto del `.exe` con su cero.
    while unsafe { id.add(n).read() } != 0 && n < 256 {
        n += 1;
    }
    // SAFETY: como arriba, `n` caracteres.
    let dado = unsafe { core::slice::from_raw_parts(id, n) };
    if dado.iter().copied().eq(ID.encode_utf16()) {
        com::add_ref_de(aparato());
        dar(sale, aparato())
    } else {
        // SAFETY: un puntero del `.exe`.
        unsafe { sale.write(0) };
        E_NOTFOUND
    }
}

/// Avisos de cambios de aparato: aqui no cambia nunca ninguno.
extern "win64" fn sin_avisos(_this: u64, _cliente: u64) -> i32 {
    S_OK
}

// -- IMMDeviceCollection ----------------------------------------------------

extern "win64" fn get_count(this: u64, sale: *mut u32) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: una coleccion de la casa; un puntero del `.exe`.
    unsafe { sale.write(*com::de::<u32>(this)) };
    S_OK
}

extern "win64" fn item(this: u64, i: u32, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: una coleccion de la casa.
    if i >= unsafe { *com::de::<u32>(this) } {
        // SAFETY: un puntero del `.exe`.
        unsafe { sale.write(0) };
        return E_INVALIDARG;
    }
    com::add_ref_de(aparato());
    dar(sale, aparato())
}

// -- IMMDevice e IMMEndpoint ------------------------------------------------

/// `QueryInterface` del aparato: el mismo objeto para IMMDevice, y OTRO
/// (con su vtabla) para IMMEndpoint, como en Windows.
extern "win64" fn aparato_query_interface(this: u64, riid: *const Guid, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    if com::pide(riid, com::MM_APARATO) {
        com::add_ref_de(this);
        return dar(sale, this);
    }
    if com::pide(riid, com::MM_PUNTA) {
        let e = estado();
        if e.punta == 0 {
            let vt = vtabla::<{ com::MM_PUNTA }>(&[(3, dir!(get_data_flow))]);
            e.punta = nuevo(com::MM_PUNTA, vt, ()) as u64;
        }
        return dar(sale, e.punta);
    }
    aviso(&alloc::format!("IMMDevice::QueryInterface {}: la casa no la tiene", crate::com_basico::clsid_texto(riid as *const u8)));
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(0) };
    E_NOINTERFACE
}

extern "win64" fn get_data_flow(_this: u64, sale: *mut u32) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(SALIDA) };
    S_OK
}

/// `Activate(iid, contexto, parametros, sale)`: un IAudioClient (1, 2 o 3).
extern "win64" fn activate(_this: u64, iid: *const Guid, _ctx: u32, _params: u64, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    if com::pide(iid, com::CLIENTE) {
        return dar(sale, crate::wasapi_flujo::nuevo_cliente());
    }
    aviso(&alloc::format!("IMMDevice::Activate {}: la casa no la tiene (solo IAudioClient)", crate::com_basico::clsid_texto(iid as *const u8)));
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(0) };
    E_NOINTERFACE
}

extern "win64" fn open_property_store(_this: u64, _acceso: u32, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    let e = estado();
    if e.propiedades == 0 {
        let vt = vtabla::<{ com::PROPIEDADES }>(&[(3, dir!(prop_get_count)), (4, dir!(prop_get_at)), (5, dir!(prop_get_value)), (6, dir!(prop_set_value)), (7, dir!(prop_commit))]);
        e.propiedades = nuevo(com::PROPIEDADES, vt, ()) as u64;
    }
    com::add_ref_de(e.propiedades);
    dar(sale, e.propiedades)
}

extern "win64" fn get_id(_this: u64, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(texto_de_tarea(ID)) };
    S_OK
}

extern "win64" fn get_state(_this: u64, sale: *mut u32) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(ACTIVO) };
    S_OK
}

// -- IPropertyStore ---------------------------------------------------------

/// Las claves (PROPERTYKEY: GUID + pid) que se contestan.
const FMTID_DISPOSITIVO: Guid = guid(0xA45C_254E, 0xDF1C, 0x4EFD, [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0]);
const FMTID_INTERFAZ: Guid = guid(0x026E_516E, 0xB814, 0x414B, [0x83, 0xCD, 0x85, 0x6D, 0x6F, 0xEF, 0x48, 0x22]);
const FMTID_MOTOR: Guid = guid(0xF19F_064D, 0x082C, 0x4E27, [0xBC, 0x73, 0x68, 0x82, 0xA1, 0xBB, 0x8E, 0x4C]);
const FMTID_PUNTA: Guid = guid(0x1DA5_D803, 0xD492, 0x4EDD, [0x8C, 0x23, 0xE0, 0xC0, 0xFF, 0xEE, 0x7F, 0x0E]);

/// `VARTYPE` de un PROPVARIANT.
const VT_UI4: u16 = 19;
const VT_LPWSTR: u16 = 31;
const VT_BLOB: u16 = 65;

/// `EndpointFormFactor`: Headphones.
const AURICULARES: u32 = 3;

/// Lo que vale una clave: un PROPVARIANT (24 bytes) ya escrito.
fn valor(clave: &Guid, pid: u32, pv: *mut u8) {
    // SAFETY: un PROPVARIANT del `.exe`: 24 bytes.
    let pon = |vt: u16, a: u64, b: u64| unsafe {
        core::ptr::write_bytes(pv, 0, 24);
        (pv as *mut u16).write_unaligned(vt);
        (pv.add(8) as *mut u64).write_unaligned(a);
        (pv.add(16) as *mut u64).write_unaligned(b);
    };
    let nombre = if hay_aparato() { "Audifono USB (BMO-X)" } else { "Sin aparato (BMO-X)" };
    match (*clave, pid) {
        // PKEY_Device_FriendlyName (14), PKEY_Device_DeviceDesc (2),
        // PKEY_DeviceInterface_FriendlyName (2).
        (FMTID_DISPOSITIVO, 14) | (FMTID_DISPOSITIVO, 2) | (FMTID_INTERFAZ, 2) => pon(VT_LPWSTR, texto_de_tarea(nombre), 0),
        // PKEY_AudioEngine_DeviceFormat: el WAVEFORMATEXTENSIBLE del aparato.
        (FMTID_MOTOR, 0) => {
            let f = bmo_proton_x::pcm::Formato { hz: hz(), canales: 2, bits: 16, flotante: false, alinea: 4, mascara: 3 }.a_waveformatextensible();
            let p = crate::dll_chicas::co_task_mem_alloc(f.len() as u64);
            if p != 0 {
                // SAFETY: 40 bytes recien pedidos.
                unsafe { core::ptr::copy_nonoverlapping(f.as_ptr(), p as *mut u8, f.len()) };
            }
            // VT_BLOB: cbSize en +8, pBlobData en +16.
            pon(VT_BLOB, f.len() as u64, p)
        }
        // PKEY_AudioEndpoint_FormFactor (0), _PhysicalSpeakers (3), _GUID (4).
        (FMTID_PUNTA, 0) => pon(VT_UI4, AURICULARES as u64, 0),
        (FMTID_PUNTA, 3) => pon(VT_UI4, 3, 0),
        (FMTID_PUNTA, 4) => pon(VT_LPWSTR, texto_de_tarea("{b0b0b0b0-0000-4000-8000-424d4f2d5821}"), 0),
        // Lo demas: VT_EMPTY y S_OK, como Windows con una clave que no hay.
        _ => pon(0, 0, 0),
    }
}

extern "win64" fn prop_get_count(_this: u64, sale: *mut u32) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(0) };
    S_OK
}

extern "win64" fn prop_get_at(_this: u64, _i: u32, _clave: *mut u8) -> i32 {
    E_INVALIDARG
}

extern "win64" fn prop_get_value(_this: u64, clave: *const u8, pv: *mut u8) -> i32 {
    if clave.is_null() || pv.is_null() {
        return E_POINTER;
    }
    // SAFETY: un PROPERTYKEY del `.exe`: GUID y pid (20 bytes).
    let (g, pid) = unsafe { ((clave as *const Guid).read_unaligned(), (clave.add(16) as *const u32).read_unaligned()) };
    valor(&g, pid, pv);
    S_OK
}

extern "win64" fn prop_set_value(_this: u64, _clave: *const u8, _pv: *const u8) -> i32 {
    STG_E_ACCESSDENIED
}

extern "win64" fn prop_commit(_this: u64) -> i32 {
    S_OK
}
