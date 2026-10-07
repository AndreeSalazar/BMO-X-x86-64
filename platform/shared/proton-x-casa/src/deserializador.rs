//! **Los deserializadores de root signatures** (A3 del contador de DX12,
//! 06-10): `D3D12CreateRootSignatureDeserializer` y
//! `D3D12CreateVersionedRootSignatureDeserializer`. Un juego (o su motor)
//! que guarda sus firmas ya serializadas las LEE de vuelta con ellos: los
//! bytes entran, y salen las estructuras de Windows (`D3D12_ROOT_SIGNATURE_DESC`,
//! o la `D3D12_VERSIONED_ROOT_SIGNATURE_DESC` en la version que se pida).
//! Hasta hoy, E_NOTIMPL.
//!
//! ```text
//!    VERSIONED (48 B)   Version +0 | la DESC en +8:
//!    DESC (40 B)        NumParameters +0 | pParameters +8 | NumStaticSamplers +16
//!                       | pStaticSamplers +24 | Flags +32
//!    ROOT_PARAMETER     tipo +0 | union +8 (tabla: n +8, rangos +16;
//!      (32 B)           constantes: registro, espacio, cuantas; descriptor:
//!                       registro, espacio, y en la 1.1 sus Flags) | visibilidad +24
//!    RANGO              1.0: tipo, cuantos, registro, espacio, desde (20 B)
//!                       1.1: ... espacio, FLAGS, desde (24 B)
//!    SAMPLER            1.0 y 1.1: 13 u32 (52 B); 1.2: y sus Flags (56 B)
//! ```
//!
//! Cada version se arma una vez, en un bloque propio del objeto (los
//! punteros apuntan dentro de el), y vive lo que el objeto: como en Windows,
//! lo que devuelven es del deserializador.

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::raiz::{self, Carga, Firma};

use crate::aviso;
use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, S_OK};

/// `D3D12_ROOT_SIGNATURE_VERSION_1_2` (la 1.0 y la 1.1, en `raiz`).
const VERSION_1_2: u32 = 3;

pub struct Deserializador {
    firma: Firma,
    /// Lo armado para la 1.0, la 1.1 y la 1.2 (en palabras de 8 bytes: los
    /// punteros de dentro quedan alineados), cuando se pide.
    hechas: [Option<Vec<u64>>; 3],
}

/// **Armar** la firma en la `version` que se pide: la VERSIONED entera,
/// con sus parametros, sus rangos y sus samplers detras.
fn armar(f: &Firma, version: u32) -> Vec<u64> {
    let f = if version == raiz::VERSION_1_0 { f.en_1_0() } else { f.en_1_1() };
    let v11 = version != raiz::VERSION_1_0;
    let (mide_rango, mide_sampler) = (if v11 { 24 } else { 20 }, if version == VERSION_1_2 { 56 } else { 52 });
    let n = f.parametros.len();
    let rangos: usize = f.parametros.iter().map(|p| if let Carga::Tabla(r) = &p.carga { r.len() } else { 0 }).sum();
    let en_parametros = 48;
    let en_rangos = en_parametros + 32 * n;
    let en_samplers = (en_rangos + mide_rango * rangos).next_multiple_of(8);
    let total = en_samplers + mide_sampler * f.samplers.len();
    let mut palabras = vec![0u64; total.div_ceil(8)];
    let base = palabras.as_mut_ptr() as *mut u8;
    let dir = base as u64;
    // SAFETY: todo lo que se escribe cae en `[0, total)` de `palabras`.
    let pon = |o: usize, x: u32| unsafe { (base.add(o) as *mut u32).write_unaligned(x) };
    let pon64 = |o: usize, x: u64| unsafe { (base.add(o) as *mut u64).write_unaligned(x) };
    pon(0, version);
    pon(8, n as u32);
    pon64(16, if n > 0 { dir + en_parametros as u64 } else { 0 });
    pon(24, f.samplers.len() as u32);
    pon64(32, if f.samplers.is_empty() { 0 } else { dir + en_samplers as u64 });
    pon(40, f.banderas);
    let mut r = en_rangos;
    for (i, p) in f.parametros.iter().enumerate() {
        let e = en_parametros + 32 * i;
        pon(e, p.tipo);
        pon(e + 24, p.visibilidad);
        match &p.carga {
            Carga::Tabla(t) => {
                pon(e + 8, t.len() as u32);
                pon64(e + 16, if t.is_empty() { 0 } else { dir + r as u64 });
                for x in t {
                    for (k, v) in [x.tipo, x.cuantos, x.registro, x.espacio].into_iter().enumerate() {
                        pon(r + 4 * k, v);
                    }
                    if v11 {
                        pon(r + 16, x.banderas);
                    }
                    pon(r + mide_rango - 4, x.desde);
                    r += mide_rango;
                }
            }
            Carga::Constantes { registro, espacio, cuantas } => {
                pon(e + 8, *registro);
                pon(e + 12, *espacio);
                pon(e + 16, *cuantas);
            }
            Carga::Descriptor { registro, espacio, banderas } => {
                pon(e + 8, *registro);
                pon(e + 12, *espacio);
                if v11 {
                    pon(e + 16, *banderas);
                }
            }
        }
    }
    for (k, s) in f.samplers.iter().enumerate() {
        for (j, &x) in s.iter().enumerate() {
            pon(en_samplers + mide_sampler * k + 4 * j, x);
        }
    }
    palabras
}

impl Deserializador {
    /// La VERSIONED de la `version` (1, 2 o 3), armada la primera vez.
    fn en(&mut self, version: u32) -> *const u8 {
        let i = (version - 1) as usize;
        if self.hechas[i].is_none() {
            self.hechas[i] = Some(armar(&self.firma, version));
        }
        self.hechas[i].as_ref().map_or(core::ptr::null(), |v| v.as_ptr() as *const u8)
    }
}

/// `D3D12CreateRootSignatureDeserializer(datos, medida, riid, pp)` y la de
/// versiones: los bytes de una root signature (un contenedor con su RTS0)
/// leidos con `raiz::leer`; si no lo son, E_INVALIDARG, como Windows.
fn crear(datos: *const u8, medida: usize, riid: *const Guid, pp: *mut u64, interfaz: usize) -> i32 {
    if !pide(riid, interfaz) {
        crate::d3d12::nada(pp);
        return E_NOINTERFACE;
    }
    if datos.is_null() || medida == 0 {
        crate::d3d12::nada(pp);
        return E_INVALIDARG;
    }
    // SAFETY: `medida` bytes del `.exe`.
    let d = unsafe { core::slice::from_raw_parts(datos, medida) };
    let firma = match raiz::leer(d) {
        Ok(f) => f,
        Err(_) => {
            aviso("D3D12Create(Versioned)RootSignatureDeserializer: esos bytes no son una root signature (de la 1.0 o la 1.1)");
            crate::d3d12::nada(pp);
            return E_INVALIDARG;
        }
    };
    let t = Deserializador { firma, hechas: [None, None, None] };
    let o = if interfaz == com::DESERIALIZADOR {
        nuevo(com::DESERIALIZADOR, vtabla::<{ com::DESERIALIZADOR }>(&[(3, crate::dir!(get_root_signature_desc))]), t) as u64
    } else {
        let vt = vtabla::<{ com::DESERIALIZADOR_V }>(&[(3, crate::dir!(get_root_signature_desc_at_version)), (4, crate::dir!(get_unconverted_root_signature_desc))]);
        nuevo(com::DESERIALIZADOR_V, vt, t) as u64
    };
    dar(pp, o)
}

pub(crate) extern "win64" fn d3d12_create_root_signature_deserializer(datos: *const u8, medida: usize, riid: *const Guid, pp: *mut u64) -> i32 {
    crear(datos, medida, riid, pp, com::DESERIALIZADOR)
}

pub(crate) extern "win64" fn d3d12_create_versioned_root_signature_deserializer(datos: *const u8, medida: usize, riid: *const Guid, pp: *mut u64) -> i32 {
    crear(datos, medida, riid, pp, com::DESERIALIZADOR_V)
}

/// `ID3D12RootSignatureDeserializer::GetRootSignatureDesc`: la de la 1.0 (la
/// DESC de dentro de la VERSIONED).
extern "win64" fn get_root_signature_desc(this: u64) -> *const u8 {
    // SAFETY: un Deserializador de la casa.
    let d = unsafe { de::<Deserializador>(this) };
    d.en(raiz::VERSION_1_0).wrapping_add(8)
}

/// `ID3D12VersionedRootSignatureDeserializer::GetRootSignatureDescAtVersion(version, pp)`:
/// la 1.0, la 1.1 o la 1.2 (convertida si hace falta); otra, E_INVALIDARG.
extern "win64" fn get_root_signature_desc_at_version(this: u64, version: u32, pp: *mut u64) -> i32 {
    if pp.is_null() {
        return E_INVALIDARG;
    }
    if !(raiz::VERSION_1_0..=VERSION_1_2).contains(&version) {
        // SAFETY: un puntero del `.exe`.
        unsafe { *pp = 0 };
        return E_INVALIDARG;
    }
    // SAFETY: un Deserializador de la casa; y un puntero del `.exe`.
    unsafe { *pp = de::<Deserializador>(this).en(version) as u64 };
    S_OK
}

/// `GetUnconvertedRootSignatureDesc`: en la version en que vino.
extern "win64" fn get_unconverted_root_signature_desc(this: u64) -> *const u8 {
    // SAFETY: un Deserializador de la casa.
    let d = unsafe { de::<Deserializador>(this) };
    let v = d.firma.version;
    d.en(v)
}
