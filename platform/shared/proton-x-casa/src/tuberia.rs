//! **La tuberia de D3D12 de la casa** (P3b2, 27-09): root signature, PSO,
//! buferes, y lo que cada `Draw` ve.
//!
//! ```text
//!    D3D12SerializeRootSignature  la estructura del .exe -> RTS0 (los MISMOS
//!                                 bytes que Microsoft: bmo_proton_x::raiz)
//!    CreateRootSignature          RTS0 -> la firma, leida
//!    CreateGraphicsPipelineState  los dos DXIL (o SM5 de FXC: P3c3) leidos
//!                                 (bmo_proton_x::dxil),
//!                                 y el input layout CRUZADO con la firma de
//!                                 entrada del vertice: una semantica que el
//!                                 sombreador no tiene es E_INVALIDARG
//!    CreateCommittedResource      un BUFER en memoria de este proceso,
//!                                 alineado a 256; su "direccion de GPU" es
//!                                 la de la CPU
//!    Map / Unmap / GetGPUVirtualAddress
//!    la lista                     IASetPrimitiveTopology, IASetVertexBuffers,
//!                                 IASetIndexBuffer, RSSetViewports,
//!                                 RSSetScissorRects, OMSetRenderTargets,
//!                                 SetPipelineState, SetGraphicsRootSignature,
//!                                 SetGraphicsRootConstantBufferView,
//!                                 DrawInstanced, DrawIndexedInstanced
//! ```
//!
//! **Cada direccion que da el `.exe` se RESUELVE** contra el registro de
//! buferes de la casa (inicio, medida): una que no cae dentro de ninguno se
//! dice y no se lee. Leer a ciegas una direccion ajena seria el fallo de pagina
//! que nadie sabria explicar.
//!
//! **Lo que hace un Draw:** al ejecutarse la lista, lo PINTA (P3b3,
//! [`pintar`]): el sombreador de vertices una vez por vertice, los
//! triangulos por la trama de `bmo_proton_x` y el de pixeles en cada pixel,
//! sobre el render target. Y los primeros [`GUARDADOS`] quedan en
//! [`dibujos`] tal como los vio (P3b2): lo que el banco compara con X1.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use alloc::format;

use bmo_proton_x::dxil::{self, Etapa, Sombreador};
use bmo_proton_x::lote::{enlazar_con, Lote, NoDibuja, Topologia};
use bmo_proton_x::trama;
use bmo_proton_x::raiz::{self, Carga, Firma, Parametro, Rango};

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, E_OUTOFMEMORY, S_OK};
use crate::subrecursos::{Almacen, Forma};
use crate::{aviso, dir, plataforma};

// -- Constantes de D3D12 que se miran ---------------------------------------

const RS_VERSION_1: u32 = 1;
const DIMENSION_BUFFER: u32 = 1;
pub const FMT_D32_FLOAT: u32 = 40;
const FMT_R16_UINT: u32 = 57;
const FMT_R32_UINT: u32 = 42;
const APPEND_ALIGNED: u32 = 0xFFFF_FFFF;
pub const TRIANGLELIST: u32 = 4;

// -- Lectura de la memoria del .exe -----------------------------------------

/// Un `u32` de una estructura del `.exe`.
///
/// # Safety
/// `p + o` son 4 bytes legibles del `.exe`, como promete Windows.
unsafe fn u32_de(p: *const u8, o: usize) -> u32 {
    (p.add(o) as *const u32).read_unaligned()
}

unsafe fn u64_de(p: *const u8, o: usize) -> u64 {
    (p.add(o) as *const u64).read_unaligned()
}

unsafe fn f32_de(p: *const u8, o: usize) -> f32 {
    (p.add(o) as *const f32).read_unaligned()
}

unsafe fn cadena_c(p: *const u8) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < 256 {
        let c = p.add(i).read();
        if c == 0 {
            break;
        }
        s.push(c as char);
        i += 1;
    }
    s
}

// -- El blob ----------------------------------------------------------------

pub struct Blob {
    bytes: Vec<u8>,
}

pub(crate) fn blob(bytes: Vec<u8>) -> u64 {
    let vt = vtabla::<{ com::BLOB }>(&[(3, dir!(get_buffer_pointer)), (4, dir!(get_buffer_size))]);
    nuevo(com::BLOB, vt, Blob { bytes }) as u64
}

extern "win64" fn get_buffer_pointer(this: u64) -> *const u8 {
    // SAFETY: `this` es un Blob de la casa.
    unsafe { de::<Blob>(this).bytes.as_ptr() }
}

extern "win64" fn get_buffer_size(this: u64) -> usize {
    // SAFETY: como arriba.
    unsafe { de::<Blob>(this).bytes.len() }
}

// -- La root signature ------------------------------------------------------

pub struct RootSignature {
    pub firma: Firma,
}

/// `D3D12_ROOT_SIGNATURE_DESC` (40 B): NumParameters +0, pParameters +8,
/// NumStaticSamplers +16, pStaticSamplers +24, Flags +32. Cada
/// `D3D12_ROOT_PARAMETER` (32 B): tipo +0, la union +8, visibilidad +24.
unsafe fn firma_de(desc: *const u8) -> Result<Firma, &'static str> {
    let (n, pars, ns, samps, banderas) = (u32_de(desc, 0), u64_de(desc, 8) as *const u8, u32_de(desc, 16), u64_de(desc, 24) as *const u8, u32_de(desc, 32));
    let mut parametros = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let p = pars.add(32 * i);
        let tipo = u32_de(p, 0);
        let carga = match tipo {
            raiz::TABLA => {
                let (nr, rangos) = (u32_de(p, 8), u64_de(p, 16) as *const u8);
                // D3D12_DESCRIPTOR_RANGE: 5 u32.
                Carga::Tabla((0..nr as usize).map(|k| {
                    let r = rangos.add(20 * k);
                    Rango { tipo: u32_de(r, 0), cuantos: u32_de(r, 4), registro: u32_de(r, 8), espacio: u32_de(r, 12), desde: u32_de(r, 16) }
                }).collect())
            }
            raiz::CONSTANTES => Carga::Constantes { registro: u32_de(p, 8), espacio: u32_de(p, 12), cuantas: u32_de(p, 16) },
            raiz::CBV | raiz::SRV | raiz::UAV => Carga::Descriptor { registro: u32_de(p, 8), espacio: u32_de(p, 12) },
            _ => return Err("un parametro de un tipo que D3D12 no tiene"),
        };
        parametros.push(Parametro { tipo, visibilidad: u32_de(p, 24), carga });
    }
    let samplers = (0..ns as usize).map(|k| core::array::from_fn(|j| u32_de(samps, 52 * k + 4 * j))).collect();
    Ok(Firma { parametros, samplers, banderas })
}

/// `D3D12SerializeRootSignature(desc, version, ppBlob, ppError)`.
extern "win64" fn d3d12_serialize_root_signature(desc: *const u8, version: u32, pp: *mut u64, pp_error: *mut u64) -> i32 {
    if !pp_error.is_null() {
        // SAFETY: un puntero del `.exe` a donde dejar el blob de errores.
        unsafe { *pp_error = 0 };
    }
    if desc.is_null() || version != RS_VERSION_1 {
        aviso("D3D12SerializeRootSignature: solo la version 1.0, todavia");
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_ROOT_SIGNATURE_DESC del `.exe`, y lo que apunta.
    match unsafe { firma_de(desc) } {
        Ok(f) => dar(pp, blob(raiz::serializar(&f))),
        Err(m) => {
            aviso(m);
            E_INVALIDARG
        }
    }
}

/// **Una firma 1.1** (`D3D12_ROOT_SIGNATURE_DESC1`, la misma forma que la
/// 1.0): los rangos miden 24 B (con sus Flags en +16 y el desplazamiento en
/// +20) y los descriptores de la raiz llevan Flags en +16. Los Flags de 1.1
/// son pistas de rendimiento (DATA_STATIC, DESCRIPTORS_VOLATILE...): la casa
/// lee sus datos siempre al dibujar, asi que no cambian lo que se ve.
unsafe fn firma_de_1_1(desc: *const u8) -> Result<Firma, &'static str> {
    let (n, pars, ns, samps, banderas) = (u32_de(desc, 0), u64_de(desc, 8) as *const u8, u32_de(desc, 16), u64_de(desc, 24) as *const u8, u32_de(desc, 32));
    let mut parametros = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let p = pars.add(32 * i);
        let tipo = u32_de(p, 0);
        let carga = match tipo {
            raiz::TABLA => {
                let (nr, rangos) = (u32_de(p, 8), u64_de(p, 16) as *const u8);
                Carga::Tabla((0..nr as usize).map(|k| {
                    let r = rangos.add(24 * k);
                    Rango { tipo: u32_de(r, 0), cuantos: u32_de(r, 4), registro: u32_de(r, 8), espacio: u32_de(r, 12), desde: u32_de(r, 20) }
                }).collect())
            }
            raiz::CONSTANTES => Carga::Constantes { registro: u32_de(p, 8), espacio: u32_de(p, 12), cuantas: u32_de(p, 16) },
            raiz::CBV | raiz::SRV | raiz::UAV => Carga::Descriptor { registro: u32_de(p, 8), espacio: u32_de(p, 12) },
            _ => return Err("un parametro de un tipo que D3D12 no tiene"),
        };
        parametros.push(Parametro { tipo, visibilidad: u32_de(p, 24), carga });
    }
    let samplers = (0..ns as usize).map(|k| core::array::from_fn(|j| u32_de(samps, 52 * k + 4 * j))).collect();
    Ok(Firma { parametros, samplers, banderas })
}

/// `D3D12SerializeVersionedRootSignature(desc, ppBlob, ppError)`:
/// `D3D12_VERSIONED_ROOT_SIGNATURE_DESC` = Version +0 y la firma en +8
/// (1.0 o 1.1). Sale el mismo blob que la 1.0 (lo que la casa lee luego).
extern "win64" fn d3d12_serialize_versioned_root_signature(desc: *const u8, pp: *mut u64, pp_error: *mut u64) -> i32 {
    const VERSION_1_1: u32 = 2;
    if !pp_error.is_null() {
        // SAFETY: un puntero del `.exe` a donde dejar el blob de errores.
        unsafe { *pp_error = 0 };
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_VERSIONED_ROOT_SIGNATURE_DESC del `.exe`.
    let version = unsafe { u32_de(desc, 0) };
    // SAFETY: la firma de dentro, y lo que apunta.
    let f = unsafe {
        match version {
            RS_VERSION_1 => firma_de(desc.add(8)),
            VERSION_1_1 => firma_de_1_1(desc.add(8)),
            _ => Err("una version de root signature que no es 1.0 ni 1.1"),
        }
    };
    match f {
        Ok(f) => dar(pp, blob(raiz::serializar(&f))),
        Err(m) => {
            aviso(m);
            E_INVALIDARG
        }
    }
}

/// `CreateRootSignature(this, nodo, bytes, medida, riid, pp)`.
pub(crate) extern "win64" fn create_root_signature(_this: u64, _nodo: u32, bytes: *const u8, tam: usize, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::ROOTSIG) {
        return E_NOINTERFACE;
    }
    if bytes.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: `tam` bytes del `.exe`.
    let d = unsafe { core::slice::from_raw_parts(bytes, tam) };
    match raiz::leer(d) {
        Ok(firma) => {
            let vt = vtabla::<{ com::ROOTSIG }>(&[]);
            dar(pp, nuevo(com::ROOTSIG, vt, RootSignature { firma }) as u64)
        }
        Err(_) => {
            aviso("CreateRootSignature: esos bytes no son una root signature 1.0");
            E_INVALIDARG
        }
    }
}

// -- El PSO -------------------------------------------------------------------

/// Un elemento del input layout, ya leido (la forma neutra de `lote`).
pub use bmo_proton_x::lote::ElementoIa as EntradaIa;

pub struct Pso {
    pub raiz: u64,
    pub entradas: Vec<EntradaIa>,
    /// D3D12_CULL_MODE: 1 ninguna, 2 delante, 3 detras.
    pub descarte: u32,
    pub antihorario: bool,
    pub topologia: u32,
    /// Los formatos de sus render targets (`RTVFormats`, N5.8: hasta 8) y
    /// cuantos son (`NumRenderTargets`).
    pub formatos_rt: [u32; 8],
    pub n_rt: u32,
    /// Los dos sombreadores COMPILADOS y enlazados (P3b3), o por que no se
    /// pueden correr: entonces el PSO existe, y cada Draw lo dice. Con los
    /// nombres de sus funciones; compartido por los PSO con los mismos
    /// sombreadores y layout (`enlaces.rs`, 03-10: el `Sombreador` leido ya
    /// no se guarda).
    pub compilado: alloc::rc::Rc<crate::enlaces::Compilado>,
    /// N5.11: la mezcla de cada render target (con IndependentBlendEnable,
    /// la suya; sin el, la del 0 para todos), o por que no se sabe todavia
    /// (operacion logica, dos fuentes): entonces cada Draw lo dice.
    pub mezcla: Result<[bmo_proton_x::mezcla::Mezcla; 8], &'static str>,
    /// La prueba de profundidad (P3c4), si `DepthEnable`.
    pub profundidad: Option<trama::Profundidad>,
}


/// Los bytes de un D3D12_SHADER_BYTECODE, o `que` si no hay.
fn bytecode(p: *const u8, tam: usize, que: &'static str) -> Result<&'static [u8], &'static str> {
    if p.is_null() || tam == 0 {
        return Err(que);
    }
    // SAFETY: `tam` bytes del `.exe` (D3D12_SHADER_BYTECODE); se leen ahora.
    Ok(unsafe { core::slice::from_raw_parts(p, tam) })
}

fn sombreador(d: &[u8], etapa: Etapa) -> Result<Sombreador, &'static str> {
    let s = dxil::leer(d).map_err(|_| "CreateGraphicsPipelineState: un sombreador que no es DXIL ni SM5 (o no se lee)")?;
    if s.etapa != etapa {
        return Err("CreateGraphicsPipelineState: un sombreador de otra etapa en su hueco");
    }
    Ok(s)
}

/// Lee el `D3D12_GRAPHICS_PIPELINE_STATE_DESC` (656 B, desplazamientos
/// MEDIDOS con la cabecera de Windows: ver prueba/HACER.txt).
/// Con el PSO, si su enlace es NUEVO (no lo comparte con uno de antes).
unsafe fn pso_de(d: *const u8) -> Result<(Pso, bool), &'static str> {
    let raiz = u64_de(d, 0);
    if raiz == 0 {
        return Err("CreateGraphicsPipelineState sin root signature");
    }
    // D3D12_SHADER_BYTECODE de DS +40, HS +56, GS +72: su medida, +8.
    for o in [40usize, 56, 72] {
        if u64_de(d, o + 8) != 0 {
            return Err("CreateGraphicsPipelineState con dominio, casco o geometria: todavia no");
        }
    }
    let bytes_vs = bytecode(u64_de(d, 8) as *const u8, u64_de(d, 16) as usize, "CreateGraphicsPipelineState sin sombreador de vertices")?;
    // N5.12: sin sombreador de pixeles es un dibujo de solo profundidad.
    let bytes_ps = if u64_de(d, 24) == 0 || u64_de(d, 32) == 0 { None } else { Some(bytecode(u64_de(d, 24) as *const u8, u64_de(d, 32) as usize, "")?) };
    // El input layout, con los desplazamientos APPEND_ALIGNED resueltos.
    let (elems, n) = (u64_de(d, 552) as *const u8, u32_de(d, 560));
    let mut entradas: Vec<EntradaIa> = Vec::with_capacity(n as usize);
    let mut siguiente = [0u32; 16];
    for i in 0..n as usize {
        let e = elems.add(32 * i);
        let (formato, ranura, desde) = (u32_de(e, 12), u32_de(e, 16), u32_de(e, 20));
        // 03-10 (N3.1): cualquier formato de vertice (`formato_ia`); antes,
        // solo floats de 32 bits, y lo demas NEGABA el PSO entero.
        let Some(f) = bmo_proton_x::formato_ia::forma(formato) else {
            return Err("un input layout con un formato que no es de vertice");
        };
        let bytes = f.bytes;
        let r = (ranura as usize).min(15);
        let desde = if desde == APPEND_ALIGNED { siguiente[r] } else { desde };
        siguiente[r] = desde + bytes;
        entradas.push(EntradaIa { semantica: cadena_c(u64_de(e, 0) as *const u8), indice: u32_de(e, 8), formato, ranura, desde });
    }
    // N5.8 (03-10): hasta 8 render targets (el G-buffer); N5.12: ninguno es
    // un dibujo de solo profundidad (las sombras, el prepaso de Z).
    let n_rt = u32_de(d, 576);
    if n_rt > 8 {
        return Err("CreateGraphicsPipelineState con mas de 8 render targets: en Windows es un error");
    }
    let formatos_rt: [u32; 8] = core::array::from_fn(|i| u32_de(d, 580 + 4 * i));
    // DepthStencilState (+496): DepthEnable +0, DepthWriteMask +4 (1 ALL),
    // DepthFunc +8, StencilEnable +12.
    let profundidad = (u32_de(d, 496) != 0).then(|| trama::Profundidad { funcion: u32_de(d, 504), escribir: u32_de(d, 500) == 1 });
    if u32_de(d, 508) != 0 {
        aviso("CreateGraphicsPipelineState con stencil: se apunta, y no se usa todavia");
    }
    // BlendState (+120): AlphaToCoverageEnable +0, IndependentBlendEnable
    // +4, y RenderTarget[i] desde +8, de 40 bytes (`mezcla::Mezcla::de_desc`).
    // Sin IndependentBlendEnable, el 0 vale para todos.
    if u32_de(d, 120) != 0 {
        aviso("CreateGraphicsPipelineState con AlphaToCoverage: sin MSAA no cubre nada; se apunta, y no se usa");
    }
    let independiente = u32_de(d, 124) != 0;
    let mezcla = (0..8usize)
        .map(|i| {
            let rt = 128 + 40 * if independiente { i } else { 0 };
            bmo_proton_x::mezcla::Mezcla::de_desc(core::slice::from_raw_parts(d.add(rt), 40))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|v| core::array::from_fn(|i| v[i]));
    // Los sombreadores: leidos, comprobados y compilados UNA vez por (VS, PS,
    // layout); los demas PSO con lo mismo lo comparten (`enlaces.rs`).
    let (compilado, nuevo) = crate::enlaces::de(bytes_vs, bytes_ps.unwrap_or(&[]), &entradas, || {
        let vs = sombreador(bytes_vs, Etapa::Vertice)?;
        let ps = bytes_ps.map(|b| sombreador(b, Etapa::Pixel)).transpose()?;
        // Cada elemento del sombreador de vertices tiene que venir del
        // layout, MENOS los valores de sistema (SV_VertexID, SV_InstanceID):
        // esos los pone quien dibuja, y D3D12 no los pide al layout (03-10:
        // la casa negaba asi los triangulos de pantalla completa de Cyberpunk).
        for f in vs.entradas.iter().filter(|f| f.sistema == 0) {
            if !entradas.iter().any(|e| e.semantica.eq_ignore_ascii_case(&f.semantica) && e.indice == f.indice) {
                return Err("el sombreador de vertices lee una semantica que el input layout no da");
            }
        }
        let nombre = |s: &Sombreador| s.modulo.entrada().map(|f| f.nombre.clone()).unwrap_or_default();
        Ok(crate::enlaces::Compilado { nombres: (nombre(&vs), ps.as_ref().map(nombre).unwrap_or_default()), enlace: enlazar_con(&vs, ps.as_ref(), &entradas) })
    })?;
    if nuevo {
        if let Err(m) = &compilado.enlace {
            aviso(m);
        }
    }
    Ok((Pso {
        raiz,
        entradas,
        compilado,
        mezcla,
        profundidad,
        descarte: u32_de(d, 452 + 4),
        antihorario: u32_de(d, 452 + 8) != 0,
        topologia: u32_de(d, 572),
        formatos_rt,
        n_rt,
    }, nuevo))
}

pub(crate) extern "win64" fn create_graphics_pipeline_state(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::PSO) {
        return E_NOINTERFACE;
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    let empezo = (plataforma().ahora_ns)();
    // SAFETY: un D3D12_GRAPHICS_PIPELINE_STATE_DESC del `.exe`.
    let r = unsafe { pso_de(desc) };
    crate::pulso::contar(crate::pulso::Cosa::Pso, (plataforma().ahora_ns)().saturating_sub(empezo));
    match r {
        Ok((pso, estrenado)) => {
            let vt = vtabla::<{ com::PSO }>(&[(8, dir!(crate::d3d12_resto::get_cached_blob))]);
            let obj = nuevo(com::PSO, vt, pso) as u64;
            // P3b3b: sus sombreadores, traducidos a x86-64 una vez, aqui
            // (una por enlace: los PSO que lo comparten, tambien la
            // traduccion).
            // SAFETY: el Pso recien creado; vive lo que el proceso.
            if let (Ok(en), true) = (&unsafe { de::<Pso>(obj) }.compilado.enlace, estrenado) {
                crate::nativo::registrar(en);
            }
            dar(pp, obj)
        }
        Err(m) => {
            aviso(m);
            E_INVALIDARG
        }
    }
}

// -- Los buferes y su registro ----------------------------------------------

/// Un bufer de la casa: `bytes` desde `base`, alineado a 256 (lo que D3D12
/// pide a un bufer de constantes). La memoria es del PROCESO
/// (`memoria::pedir_bufer`), o la de su `ID3D12Heap` si es colocado; nunca
/// del monton del cargador (tanda 44). No se devuelve: ver `com::release`.
pub struct Bufer {
    base: u64,
    pub bytes: usize,
}

impl Bufer {
    pub fn base(&self) -> u64 {
        self.base
    }
}

struct Registro(UnsafeCell<Vec<(u64, usize)>>);
// SAFETY: un hilo (ver `Global` en lib.rs).
unsafe impl Sync for Registro {}
static BUFERES: Registro = Registro(UnsafeCell::new(Vec::new()));
static DIBUJOS: Dibujos = Dibujos(UnsafeCell::new(Vec::new()));

struct Dibujos(UnsafeCell<Vec<Dibujo>>);
// SAFETY: como arriba.
unsafe impl Sync for Dibujos {}

pub(crate) fn reiniciar() {
    // SAFETY: un hilo, antes de saltar al `.exe`.
    unsafe {
        (*BUFERES.0.get()).clear();
        (*DIBUJOS.0.get()).clear();
    }
}

/// **Resolver una direccion del `.exe`**: los `n` bytes desde `va`, si caen
/// ENTEROS dentro de un bufer de la casa.
fn resolver(va: u64, n: usize) -> Option<&'static [u8]> {
    // SAFETY: un hilo; el registro solo lo toca la casa.
    let r = unsafe { &*BUFERES.0.get() };
    r.iter().find(|&&(i, t)| va >= i && va - i + n as u64 <= t as u64)?;
    // SAFETY: [va, va + n) cae dentro de un bufer vivo de la casa (los
    // objetos de la casa no se liberan: ver com.rs).
    Some(unsafe { core::slice::from_raw_parts(va as *const u8, n) })
}

/// **Hasta `n` bytes desde `va`**, los que haya en el bufer de la casa que
/// la contiene (N5.2: un cbuffer mas corto de lo que se lee da 0 en lo que
/// falta, no deja de dibujar).
pub(crate) fn resolver_hasta(va: u64, n: usize) -> Option<&'static [u8]> {
    // SAFETY: un hilo; el registro solo lo toca la casa.
    let r = unsafe { &*BUFERES.0.get() };
    let &(i, t) = r.iter().find(|&&(i, t)| va >= i && va < i + t as u64)?;
    let n = n.min((i + t as u64 - va) as usize);
    // SAFETY: [va, va + n) cae dentro de un bufer vivo de la casa.
    Some(unsafe { core::slice::from_raw_parts(va as *const u8, n) })
}

/// Si `[va, va + n)` cae entero dentro de un bufer de la casa (tanda 48).
pub(crate) fn dentro_de_bufer(va: u64, n: usize) -> bool {
    resolver(va, n).is_some()
}

/// Una textura nueva en `pp`, o E_OUTOFMEMORY (antes, un panico del cargador).
fn dar_imagen(pp: *mut u64, forma: Forma, banderas: u32) -> i32 {
    match crate::d3d12::recurso_forma(forma, false, banderas) {
        Some(r) => dar(pp, r),
        None => {
            aviso("CreateCommittedResource: no hay memoria para la textura: E_OUTOFMEMORY");
            E_OUTOFMEMORY
        }
    }
}

/// `CreateCommittedResource(this, heap, banderas, desc, estado, clear, riid, pp)`.
/// `D3D12_RESOURCE_DESC` (56 B): Dimension +0, Width +16. Solo BUFFER.
pub(crate) extern "win64" fn create_committed_resource(_this: u64, heap: *const u8, _banderas: u32, desc: *const u8, _estado: u32, _clear: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    let r = crear_recurso(desc, riid, pp, None);
    crate::d3d12_resto::apuntar_monton(r, pp, heap);
    r
}

/// Un recurso: el de `CreateCommittedResource` (`memoria` = `None`: un bufer
/// con la suya), o uno colocado (`CreatePlacedResource`), con `memoria` la
/// direccion de su sitio en el monton, ya hecha.
pub(crate) fn crear_recurso(desc: *const u8, riid: *const Guid, pp: *mut u64, memoria: Option<u64>) -> i32 {
    if !pide(riid, com::RESOURCE) {
        return E_NOINTERFACE;
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_RESOURCE_DESC del `.exe`: Dimension +0, Width +16,
    // Format +32, SampleDesc.Count +36, Flags +48.
    let (dimension, ancho, formato, muestras, banderas) = unsafe { (u32_de(desc, 0), u64_de(desc, 16), u32_de(desc, 32), u32_de(desc, 36), u32_de(desc, 48)) };
    if dimension != DIMENSION_BUFFER {
        // ** 02-10: una TEXTURA de cualquier formato (los BC tal cual), con
        // sus mips y sus capas (arrays, cubos), 1D, 2D o 3D: ver
        // `subrecursos`. Una colocada (CreatePlacedResource) tambien va a
        // memoria suya: sus texeles no son los bytes del monton (como en una
        // GPU, que los guarda en su propio orden).
        // SAFETY: el mismo D3D12_RESOURCE_DESC.
        let Some(forma) = (unsafe { Forma::de(desc) }) else {
            aviso("CreateCommittedResource: una textura de medidas imposibles: E_INVALIDARG");
            return E_INVALIDARG;
        };
        if formato == 0 {
            aviso("CreateCommittedResource: una textura de formato UNKNOWN: E_INVALIDARG");
            return E_INVALIDARG;
        }
        if muestras > 1 {
            aviso("CreateCommittedResource: una textura MULTIMUESTRA (MSAA): se guarda con una muestra por texel");
        }
        return dar_imagen(pp, forma, banderas);
    }
    if ancho == 0 {
        return E_INVALIDARG;
    }
    let base = match memoria {
        Some(m) => m,
        None => match crate::memoria::pedir_bufer(ancho) {
            Some(m) => m,
            None => {
                aviso("CreateCommittedResource: no hay memoria para el bufer: E_OUTOFMEMORY");
                return E_OUTOFMEMORY;
            }
        },
    };
    let bytes = ancho as usize;
    let b = Bufer { base, bytes };
    // SAFETY: un hilo.
    unsafe { (*BUFERES.0.get()).push((b.base(), bytes)) };
    dar(pp, crate::d3d12::recurso_bufer(b))
}

pub(crate) extern "win64" fn map(this: u64, _sub: u32, _leer: *const u8, pp: *mut u64) -> i32 {
    let Some(base) = crate::d3d12::base_de_bufer(this) else {
        aviso("ID3D12Resource::Map sobre algo que no es un bufer: todavia no");
        return E_INVALIDARG;
    };
    if pp.is_null() {
        return S_OK;
    }
    dar(pp, base)
}

pub(crate) extern "win64" fn unmap(_this: u64, _sub: u32, _escrito: *const u8) {}

pub(crate) extern "win64" fn get_gpu_virtual_address(this: u64) -> u64 {
    crate::d3d12::base_de_bufer(this).unwrap_or(0)
}

// -- El estado de dibujo de una lista ---------------------------------------

#[derive(Clone, Copy, Default)]
pub struct Vista {
    pub va: u64,
    pub bytes: u32,
    /// El paso (vertices) o el formato (indices).
    pub paso_o_formato: u32,
}

#[derive(Clone, Default)]
pub struct Estado {
    pub pso: u64,
    pub raiz: u64,
    /// La direccion dada a cada parametro CBV de la raiz, por su indice.
    pub cbv: [u64; 16],
    /// Las constantes de 32 bits de la raiz (`SetGraphicsRoot32BitConstants`,
    /// N5.2), las de todos sus parametros una tras otra: ver `cbuffers`.
    pub raiz32: crate::cbuffers::Palabras,
    pub topologia: u32,
    pub vertices: Vista,
    pub indices: Vista,
    pub viewport: [f32; 6],
    pub tijera: [i32; 4],
    pub rtv: u64,
    /// N5.11: el factor de mezcla (`OMSetBlendFactor`); `None` es el de
    /// D3D12 sin poner, (1, 1, 1, 1).
    pub factor_mezcla: Option<[f32; 4]>,
    /// N5.8: los render targets 1..8 (recurso y subrecurso; 0, ninguno).
    pub rtv_otros: [(u64, u64); 7],
    /// El recurso de profundidad (OMSetRenderTargets), o 0.
    pub dsv: u64,
    /// 02-10: el subrecurso de cada vista (y su rebanada 3D << 32): ver
    /// `d3d12_vistas`. 0, el de siempre.
    pub rtv_sub: u64,
    pub dsv_sub: u64,
    /// El identificador de GPU dado a cada tabla de la raiz, por su indice
    /// (SetGraphicsRootDescriptorTable): la direccion de su primera ranura.
    pub tablas: [u64; 16],
}

/// **Lo que un dibujo ve**, ya leido de la memoria: lo que el banco compara.
#[derive(Debug, Clone, PartialEq)]
pub struct Dibujo {
    pub vs: String,
    pub ps: String,
    pub topologia: u32,
    /// Por vertice del bufer, cada elemento del input layout como floats.
    pub vertices: Vec<Vec<Vec<f32>>>,
    pub indices: Vec<u32>,
    /// Los primeros 256 bytes del bufer de constantes de b0 (raiz, CBV).
    pub constantes: Vec<u8>,
    pub viewport: [f32; 6],
    pub descarte: u32,
    pub antihorario: bool,
    /// El recurso donde se dibujaria.
    pub destino: u64,
    pub cuantos: u32,
    pub instancias: u32,
}

/// Cuantos dibujos se guardan (los primeros).
pub const GUARDADOS: usize = 64;

/// Los dibujos que se ejecutaron desde `empezar` (los [`GUARDADOS`] primeros).
pub fn dibujos() -> Vec<Dibujo> {
    // SAFETY: un hilo.
    unsafe { (*DIBUJOS.0.get()).clone() }
}

/// **Un dibujo, al ejecutarse**: todo lo que veria, leido y comprobado.
pub(crate) fn ejecutar_dibujo(e: &Estado, cuantos: u32, instancias: u32, primero: u32, base_vertice: i32, indexado: bool) {
    if e.pso == 0 {
        aviso("Draw sin PSO: no se dibuja nada");
        return;
    }
    if e.raiz == 0 {
        aviso("Draw sin SetGraphicsRootSignature: en Windows es un error, y no se dibuja");
        return;
    }
    // SAFETY: un Pso y dos RootSignature de la casa (los Set* solo guardan
    // de esos).
    let (pso, firma) = unsafe { (de::<Pso>(e.pso), &de::<RootSignature>(e.raiz).firma) };
    // SAFETY: como arriba.
    if firma != unsafe { &de::<RootSignature>(pso.raiz).firma } {
        aviso("Draw con una root signature distinta de la del PSO: en Windows es un error");
        return;
    }
    // N5.12: el de solo profundidad pinta en el DSV, y nada mas.
    if pso.n_rt == 0 && e.dsv == 0 {
        aviso("Draw de solo profundidad sin DSV en OMSetRenderTargets: no hay donde dibujar");
        return;
    }
    if pso.n_rt > 0 && e.rtv == 0 {
        aviso("Draw sin OMSetRenderTargets: no hay donde dibujar");
        return;
    }
    // SAFETY: el descriptor guarda un Recurso de la casa. El formato de la
    // vista puede no ser el del recurso (TYPELESS, SRGB): basta que se
    // guarden igual.
    if pso.n_rt > 0 && Almacen::de(unsafe { crate::d3d12::recurso_de(e.rtv) }.formato) != Almacen::de(pso.formatos_rt[0]) {
        aviso("Draw sobre un render target de otro formato que el del PSO");
        return;
    }
    // N5.8: los demas, igual (los que no estan puestos no se miran: lo que
    // va a ellos se pierde, como con un RTV nulo).
    for (k, &(r, _)) in e.rtv_otros.iter().enumerate().take((pso.n_rt as usize).saturating_sub(1)) {
        // SAFETY: como arriba.
        if r != 0 && Almacen::de(unsafe { crate::d3d12::recurso_de(r) }.formato) != Almacen::de(pso.formatos_rt[k + 1]) {
            aviso("Draw sobre un render target (de los 1..8) de otro formato que el del PSO");
            return;
        }
    }
    pintar(e, pso, cuantos, instancias, primero, base_vertice, indexado);
    // SAFETY: un hilo.
    let v = unsafe { &mut *DIBUJOS.0.get() };
    // Se guardan los primeros: en Ring 3 el monton solo avanza (ver
    // apps/proton-x/src/monton.rs), y un `.exe` que dibuje en cada fotograma
    // no puede gastarlo en capturas.
    if v.len() >= GUARDADOS {
        return;
    }
    let mut d = Dibujo {
        vs: pso.compilado.nombres.0.clone(),
        ps: pso.compilado.nombres.1.clone(),
        topologia: e.topologia,
        vertices: Vec::new(),
        indices: Vec::new(),
        constantes: Vec::new(),
        viewport: e.viewport,
        descarte: pso.descarte,
        antihorario: pso.antihorario,
        destino: e.rtv,
        cuantos,
        instancias,
    };
    // Los vertices: todo el bufer de la ranura 0, a traves del layout.
    let paso = e.vertices.paso_o_formato as usize;
    if paso > 0 {
        let Some(vb) = resolver(e.vertices.va, e.vertices.bytes as usize) else {
            aviso("IASetVertexBuffers: una direccion que no es de ningun bufer de la casa");
            return;
        };
        for v in vb.chunks_exact(paso) {
            let campos = pso
                .entradas
                .iter()
                .filter(|x| x.ranura == 0)
                .map(|x| {
                    // Con su formato (`formato_ia`, 03-10): los componentes que trae.
                    let n = bmo_proton_x::lote::componentes(x.formato);
                    let c = bmo_proton_x::formato_ia::leer(x.formato, v.get(x.desde as usize..).unwrap_or(&[]));
                    c[..n].to_vec()
                })
                .collect();
            d.vertices.push(campos);
        }
    }
    if indexado {
        let ancho = match e.indices.paso_o_formato {
            FMT_R16_UINT => 2,
            FMT_R32_UINT => 4,
            _ => {
                aviso("IASetIndexBuffer: un formato de indices que no es R16/R32_UINT");
                return;
            }
        };
        let Some(ib) = resolver(e.indices.va, e.indices.bytes as usize) else {
            aviso("IASetIndexBuffer: una direccion que no es de ningun bufer de la casa");
            return;
        };
        for i in 0..cuantos as usize {
            let o = (primero as usize + i) * ancho;
            let Some(b) = ib.get(o..o + ancho) else {
                aviso("DrawIndexedInstanced: pide mas indices de los que tiene el bufer");
                return;
            };
            let x = if ancho == 2 { u16::from_le_bytes([b[0], b[1]]) as u32 } else { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) };
            d.indices.push((x as i64 + base_vertice as i64) as u32);
        }
    }
    // b0: el parametro CBV de la root signature con registro 0, su direccion.
    if let Some(i) = firma.parametros.iter().position(|p| p.tipo == raiz::CBV && matches!(p.carga, Carga::Descriptor { registro: 0, espacio: 0 })) {
        match resolver(e.cbv.get(i).copied().unwrap_or(0), 256) {
            Some(c) => d.constantes = c.to_vec(),
            None => aviso("SetGraphicsRootConstantBufferView: una direccion que no es de ningun bufer de la casa"),
        }
    }
    v.push(d);
}


const TRIANGLESTRIP: u32 = 5;

/// **Pintar un Draw** (P3b3): el sombreador de vertices por cada vertice que
/// piden los indices (una vez cada uno), los triangulos por la trama, y el de
/// pixeles en cada pixel que cubren, sobre el render target. Lo que no sabe
/// hacer todavia lo dice y NO pinta: nunca un dibujo a medias callado.
fn pintar(e: &Estado, pso: &Pso, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool) {
    let en = match &pso.compilado.enlace {
        Ok(en) => en,
        Err(m) => {
            aviso(&format!("Draw no se dibuja: {m}"));
            return;
        }
    };
    let mezcla = match pso.mezcla {
        Ok(rt) => bmo_proton_x::mezcla::Mezclas { rt, factor: e.factor_mezcla.unwrap_or([1.0; 4]) },
        Err(m) => {
            aviso(&format!("Draw no se dibuja: {m}"));
            return;
        }
    };
    if instancias > 1 {
        aviso("Draw con varias instancias: se dibuja una (no hay datos por instancia todavia)");
    }
    if e.tijera == [0; 4] {
        aviso("Draw sin RSSetScissorRects: en D3D12 la tijera siempre corta, y vacia no deja pintar nada");
        return;
    }
    // N5.12: sin render target, el dibujo es de solo profundidad: sin
    // pixeles, y mide lo que su Z.
    let solo_z = pso.n_rt == 0 || e.rtv == 0;
    // SAFETY: el descriptor guarda un Recurso de la casa (Draw ya lo miro).
    let mut rt = (!solo_z).then(|| unsafe { de::<crate::d3d12::Recurso>(e.rtv) });
    // 02-10: todo lo que la casa guarda en 8 bits por canal (tambien un
    // RGBA16F o un R10G10B10A2: se pintan en 8 bits, como se guardan).
    let bgra = match rt.as_ref().map(|r| Almacen::de(r.formato)) {
        Some(Almacen::Bgra8) => true,
        Some(Almacen::Rgba8) | None => false,
        Some(_) => {
            aviso("Draw sobre un render target de floats (R32) o BC: todavia no");
            return;
        }
    };
    let (pixeles, ancho, alto): (&mut [u32], u32, u32) = if solo_z {
        match destino(e.dsv, e.dsv_sub) {
            Some((_, w, h)) => (&mut [], w, h),
            None => {
                aviso("Draw de solo profundidad sobre un subrecurso que la Z no tiene");
                return;
            }
        }
    } else {
        match destino(e.rtv, e.rtv_sub) {
            Some(d) => d,
            None => {
                aviso("Draw sobre una vista de un subrecurso que el render target no tiene");
                return;
            }
        }
    };
    // Los ids de vertice, en el orden en que llegan.
    let mut ids: Vec<u32> = Vec::with_capacity(cuantos as usize);
    if indexado {
        let ancho = if e.indices.paso_o_formato == FMT_R16_UINT { 2 } else { 4 };
        let Some(ib) = resolver(e.indices.va, e.indices.bytes as usize) else { return };
        for i in 0..cuantos as usize {
            let o = (primero as usize + i) * ancho;
            let Some(b) = ib.get(o..o + ancho) else { return };
            let x = if ancho == 2 { u16::from_le_bytes([b[0], b[1]]) as i64 } else { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64 };
            ids.push((x + base as i64) as u32);
        }
    } else {
        ids.extend(primero..primero + cuantos);
    }
    let topologia = match e.topologia {
        TRIANGLELIST => Topologia::Lista,
        TRIANGLESTRIP => Topologia::Tira,
        _ => {
            aviso("Draw con una topologia que no es de triangulos (lista o tira): todavia no");
            return;
        }
    };
    // Los vertices: el bufer de la ranura 0 entero.
    let paso = e.vertices.paso_o_formato as usize;
    let Some(vb) = (paso > 0).then(|| resolver(e.vertices.va, e.vertices.bytes as usize)).flatten() else {
        aviso("Draw sin un bufer de vertices de la casa en la ranura 0");
        return;
    };
    // Las CONSTANTES (N5.2): cada cbuffer que leen, de la raiz o de una
    // tabla, en su sitio del bloque (`cbuffers.rs`).
    // SAFETY: un RootSignature de la casa.
    let firma = unsafe { &de::<RootSignature>(e.raiz).firma };
    let cb = match crate::cbuffers::del_dibujo(firma, e, en) {
        Ok(c) => c,
        Err(m) => {
            aviso(&format!("Draw: {m}: no se dibuja"));
            return;
        }
    };
    // Hasta aqui, D3D12. Lo que sigue es un LOTE, y lo dibuja quien la
    // plataforma diga (hoy la CPU; luego VERRANO con la 3060).
    // ** Las TEXTURAS y los muestreadores (29-09): de las tablas de la raiz
    // (sus SRV y samplers, en las ranuras a las que apuntan) y de los
    // samplers estaticos de la firma. Por RANURA (03-10, N5.1): cada lugar
    // (espacio, registro, etapa) que leen, buscado en la firma.
    let (texturas, muestreadores, buferes) = recursos_del_dibujo(firma, &e.tablas, &en.ranuras);
    // N5.4 (05-10): las texturas de los arrays con el registro CALCULADO,
    // buscadas cuando un pixel las pide y GUARDADAS: una vez por textura
    // distinta del dibujo, no por pixel. Un millon de descriptores (el
    // monton de Cyberpunk) no se recorre: solo los que se leen.
    let guardadas: core::cell::RefCell<alloc::collections::BTreeMap<(u8, u32), Option<bmo_proton_x::textura::Textura<'static>>>> = Default::default();
    let buscar = |rango: u8, registro: u32| {
        if let Some(t) = guardadas.borrow().get(&(rango, registro)) {
            return *t;
        }
        let t = textura_dinamica(firma, &e.tablas, &en.ranuras, rango, registro);
        guardadas.borrow_mut().insert((rango, registro), t);
        t
    };
    // P3b4c: las limpiezas apuntadas de SU render target y de SU Z: las
    // hace quien dibuje este lote.
    let limpiar_z = if pso.profundidad.is_some() && e.dsv != 0 && e.dsv_sub == 0 { tomar_limpieza(e.dsv) } else { None };
    let limpiar_rt = if e.rtv_sub == 0 && !solo_z { tomar_limpieza(e.rtv) } else { None };
    let lote = Lote {
        recursos: bmo_proton_x::textura::Recursos { texturas: &texturas, muestreadores: &muestreadores, buferes: &buferes, dinamicas: Some(bmo_proton_x::textura::Dinamicas(&buscar)) },
        limpiar_z,
        limpiar_rt,
        enlace: en,
        entradas: &pso.entradas,
        vertices: vb,
        paso,
        ids: &ids,
        topologia,
        cb: &cb,
        reglas: trama::Reglas { viewport: e.viewport, tijera: e.tijera, descarte: pso.descarte, antihorario: pso.antihorario, profundidad: pso.profundidad, mezcla, z_del_sombreador: false },
    };
    // La profundidad: la del DSV, si el PSO la pide y mide lo mismo.
    let z = match (pso.profundidad, e.dsv) {
        (Some(_), 0) | (None, _) => None,
        // SAFETY: el descriptor DSV guarda un Recurso de la casa, distinto del RT.
        (Some(_), dsv) => match (Almacen::de(unsafe { de::<crate::d3d12::Recurso>(dsv) }.formato), destino(dsv, e.dsv_sub)) {
            (Almacen::Flotante, Some((z, w, h))) if (w, h) == (ancho, alto) => Some(z),
            _ => {
                aviso("Draw: la profundidad no es de floats (D32, D24S8...) o no mide lo que el render target: se dibuja sin ella");
                None
            }
        },
    };
    let cadena = rt.as_ref().is_some_and(|r| r.cadena) && e.rtv_sub == 0;
    // N5.8: los render targets 1..8 que el PSO escribe, con su limpieza
    // apuntada ya hecha (la del 0 la hace quien dibuje: `limpiar_rt`).
    let mut otros: Vec<trama::Otro> = Vec::new();
    let mut puestos = alloc::vec![e.rtv];
    for &(r, sub) in e.rtv_otros.iter().take((pso.n_rt as usize).saturating_sub(1)) {
        if r == 0 {
            otros.push(trama::Otro { pixeles: None, bgra: false });
            continue;
        }
        // Dos vistas del mismo recurso serian dos `&mut` a la misma memoria.
        if puestos.contains(&r) {
            aviso("Draw con el mismo recurso en dos render targets: en Windows es un error, y no se dibuja");
            return;
        }
        // SAFETY: el descriptor guarda un Recurso de la casa (Draw ya lo miro).
        let bgra = match Almacen::de(unsafe { de::<crate::d3d12::Recurso>(r) }.formato) {
            Almacen::Bgra8 => true,
            Almacen::Rgba8 => false,
            _ => {
                aviso("Draw sobre un render target (de los 1..8) de floats (R32) o BC: todavia no");
                return;
            }
        };
        if sub == 0 {
            aplicar_limpieza(r);
        }
        match destino(r, sub) {
            Some((p, w, h)) if (w, h) == (ancho, alto) => {
                puestos.push(r);
                otros.push(trama::Otro { pixeles: Some(p), bgra });
            }
            _ => {
                aviso("Draw: un render target (de los 1..8) que no mide lo que el 0, o un subrecurso que no tiene");
                return;
            }
        }
    }
    let mut destino = trama::Destino { pixeles, ancho, alto, bgra, z, cadena, otros: &mut otros };
    let r = (plataforma().dibujar)(&lote, &mut destino);
    // P3b4c.9 Z1: donde quedo este dibujo (la pantalla o la RAM) es donde
    // queda el fotograma: lo lee `Present`.
    if let (0, Some(rt)) = (e.rtv_sub, rt.as_mut()) {
        rt.en_pantalla = r.as_ref().is_ok_and(|c| c.en_pantalla);
    }
    match r {
        Ok(c) if c.sin_recortar > 0 => aviso("Draw: triangulos que cruzan el plano cercano o salen de la profundidad: sin recortar todavia, no se pintan"),
        Ok(_) => {}
        Err(NoDibuja::IndiceFuera(_)) => aviso("Draw: un indice que pasa del bufer de vertices"),
        Err(NoDibuja::SinVertices) => aviso("Draw sin vertices que leer"),
    }
}

/// **Las texturas y los muestreadores que ve un dibujo**, por RANURA del
/// enlace (03-10, N5.1): la posicion `i` es el lugar `ranuras.texturas[i]`.
///
/// Una tabla de la raiz es una direccion de ranura (`SetGraphicsRootDescriptorTable`)
/// y sus rangos dicen que hay en cada una; donde cae cada lugar (espacio,
/// registro, etapa) lo dice `bmo_proton_x::donde`. Cada ranura son 4
/// palabras: el recurso y la marca (`d3d12::DESC_SRV`, `DESC_MUESTREADOR`).
/// Lo que no esta -- ni en una tabla puesta ni en los samplers estaticos --
/// se lee como nulo, como en Windows con un descriptor nulo.
/// Lo que ve un dibujo, por ranura: cada SRV es una textura o un bufer.
pub(crate) type Vistos = (Vec<Option<bmo_proton_x::textura::Textura<'static>>>, Vec<Option<bmo_proton_x::textura::Muestreador>>, Vec<Option<bmo_proton_x::bufer::Bufer<'static>>>);

/// La ranura `i` de la tabla del parametro `k` (4 palabras), si el `.exe`
/// puso esa tabla.
pub(crate) fn descriptor_de(tablas: &[u64; 16], k: usize, i: u64) -> Option<&'static [u64]> {
    let base = *tablas.get(k)?;
    if base == 0 {
        return None;
    }
    // SAFETY: la ranura `i` de un monton de la casa (la tabla la puso el
    // `.exe` con un identificador de la casa; la firma dice que la ranura es
    // de ella).
    Some(unsafe { core::slice::from_raw_parts((base + i * DESCRIPTOR_BYTES) as *const u64, 4) })
}

/// **La textura del registro `registro` del rango dinamico `rango`** (N5.4):
/// el lugar del rango con ese registro, buscado en la firma como una ranura
/// fija; un registro que ninguna tabla tiene, o un SRV nulo o de bufer, se
/// lee como nulo (ceros).
fn textura_dinamica(firma: &Firma, tablas: &[u64; 16], ranuras: &bmo_proton_x::dxil::programa::Ranuras, rango: u8, registro: u32) -> Option<bmo_proton_x::textura::Textura<'static>> {
    use bmo_proton_x::donde::{self, RANGO_SRV};
    let l = bmo_proton_x::dxil::ranuras::Lugar { registro, ..*ranuras.dinamicas.get(rango as usize)? };
    let ranura = donde::en_tabla(firma, RANGO_SRV, l).and_then(|(k, i)| descriptor_de(tablas, k, i)).filter(|r| r[1] == crate::d3d12::DESC_SRV && r[0] != 0)?;
    if crate::d3d12_vistas::leer(ranura).0 .0 == crate::d3d12_vistas::SRV_BUFER {
        aviso("un array de texturas con un SRV de BUFER dentro: se lee como nulo");
        return None;
    }
    textura_de_srv(ranura).map_err(aviso).ok()
}

pub(crate) fn recursos_del_dibujo(firma: &Firma, tablas: &[u64; 16], ranuras: &bmo_proton_x::dxil::programa::Ranuras) -> Vistos {
    use bmo_proton_x::donde::{self, RANGO_MUESTREADOR, RANGO_SRV};
    use bmo_proton_x::textura::Muestreador;
    let descriptor = |k: usize, i: u64| descriptor_de(tablas, k, i);
    // N5.3: cada SRV, a su sitio: una textura, o un bufer en la misma ranura.
    let (mut tex, mut buf) = (Vec::with_capacity(ranuras.texturas.len()), Vec::with_capacity(ranuras.texturas.len()));
    for &l in &ranuras.texturas {
        let ranura = donde::en_tabla(firma, RANGO_SRV, l).and_then(|(k, i)| descriptor(k, i)).filter(|r| r[1] == crate::d3d12::DESC_SRV && r[0] != 0);
        let es_bufer = ranura.is_some_and(|r| crate::d3d12_vistas::leer(r).0 .0 == crate::d3d12_vistas::SRV_BUFER);
        tex.push(ranura.filter(|_| !es_bufer).and_then(|r| textura_de_srv(r).map_err(aviso).ok()));
        buf.push(ranura.filter(|_| es_bufer).and_then(|r| bufer_de_srv(r).map_err(aviso).ok()));
    }
    let mue = ranuras
        .muestreadores
        .iter()
        .map(|&l| {
            if let Some((k, i)) = donde::en_tabla(firma, RANGO_MUESTREADOR, l) {
                let ranura = descriptor(k, i)?;
                if ranura[1] != crate::d3d12::DESC_MUESTREADOR {
                    return None;
                }
                let (f, u, v, b) = (ranura[2] as u32, (ranura[2] >> 32) as u32, ranura[3] as u32, (ranura[3] >> 32) as u32);
                let borde = core::array::from_fn(|c| ((b >> (8 * c)) & 0xFF) as f32 / 255.0);
                return Muestreador::de_descriptor(f, u, v, borde, ranura[0] as u32).map_err(aviso).ok();
            }
            Muestreador::de_estatico(donde::estatico(firma, l)?).map_err(aviso).ok()
        })
        .collect();
    (tex, mue, buf)
}

/// **El bufer que lee un SRV de bufer** (N5.3): sus bytes desde el primer
/// elemento de la vista, hasta los que tenga el bufer de la casa.
fn bufer_de_srv(ranura: &[u64]) -> Result<bmo_proton_x::bufer::Bufer<'static>, &'static str> {
    let v = crate::d3d12_vistas::leer_bufer(ranura);
    let base = crate::d3d12::base_de_bufer(ranura[0]).ok_or("un SRV de bufer sobre algo que no es un bufer de la casa (se lee como nulo)")?;
    // Lo que mide un elemento: 4 bytes crudo, su paso estructurado, o su
    // formato con tipo.
    let medida = match (v.crudo, v.paso, bmo_proton_x::formato_ia::forma(v.formato)) {
        (true, _, _) => 4,
        (false, p, _) if p != 0 => p as u64,
        (false, _, Some(f)) => f.bytes as u64,
        _ => return Err("un SRV de bufer sin paso ni un formato que la casa sepa leer (se lee como nulo)"),
    };
    let bytes = resolver_hasta(base + v.primero * medida, v.elementos as usize * medida as usize).ok_or("un SRV de bufer fuera de su bufer (se lee como nulo)")?;
    let elementos = (bytes.len() as u64 / medida) as u32;
    let formato = if v.crudo || v.paso != 0 { 0 } else { v.formato };
    Ok(bmo_proton_x::bufer::Bufer { bytes, formato, paso: if v.crudo { 0 } else { v.paso }, elementos })
}

/// **La textura que lee un SRV** (02-10): TODA la textura (sus mips y sus
/// capas, como las guarda la casa), como se guarda (8 bits, floats o
/// bloques BC), y lo que mira la vista: su clase (plana, array, cubo, 3D),
/// su mip mas detallada y su primera capa o cara (ver `d3d12_vistas`), si
/// es sRGB y su mapeo.
fn textura_de_srv(ranura: &[u64]) -> Result<bmo_proton_x::textura::Textura<'static>, &'static str> {
    use bmo_proton_x::textura::{Clase, Como, Textura};
    let ((dimension, formato, mapeo), (sub, _)) = crate::d3d12_vistas::leer(ranura);
    aplicar_limpieza(ranura[0]);
    let Some(t) = crate::d3d12_vistas::tex(ranura[0]) else {
        return Err("un SRV de textura sobre un bufer: en Windows es un error (se lee como nulo)");
    };
    let clase = match dimension {
        2 | 4 | 6 => Clase::Plana,
        3 | 5 | 7 => Clase::Array,
        8 => Clase::Volumen,
        9 => Clase::Cubo,
        10 => Clase::CuboArray,
        _ => return Err("un SRV de bufer o de aceleracion sobre una textura: en Windows es un error (se lee como nulo)"),
    };
    let como = match t.almacen {
        Almacen::Rgba8 => Como::Rgba8,
        Almacen::Bgra8 => Como::Bgra8,
        Almacen::Flotante => Como::Flotante,
        Almacen::Bloques(b) => Como::Bloques(b),
    };
    let total: u64 = t.subs.iter().map(|x| x.bytes()).sum();
    // SAFETY: la memoria de una textura de la casa, que no se suelta.
    let texeles = unsafe { core::slice::from_raw_parts(t.datos as *const u32, (total / 4) as usize) };
    let f = t.forma;
    let (mip, capa) = f.sub(sub);
    let formato = if formato == 0 { f.formato } else { formato };
    let hondo = if f.dimension == crate::subrecursos::DIM_TEXTURA3D { f.hondo } else { 1 };
    Ok(Textura { texeles, ancho: f.ancho, alto: f.alto, como, srgb: crate::d3d12_vistas::es_srgb(formato), mapeo, mips: f.mips, capas: f.capas(), hondo, clase, mip, capa })
}

/// Lo que mide una ranura de un monton de descriptores de la casa.
const DESCRIPTOR_BYTES: u64 = 32;

/// **Las limpiezas APUNTADAS y aun sin hacer** (P3b4c): `(recurso, pixel)`,
/// una por recurso (la ultima gana). Las apunta `ExecuteCommandLists`; las
/// toma el dibujo que pinta en ese recurso (o su Z), o las aplica quien lea
/// los pixeles antes. Una tarea, hilos cooperativos: basta una celda.
struct Limpiezas(UnsafeCell<Vec<(u64, u32)>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Limpiezas {}
static LIMPIEZAS: Limpiezas = Limpiezas(UnsafeCell::new(Vec::new()));

fn limpiezas() -> &'static mut Vec<(u64, u32)> {
    // SAFETY: ver `Limpiezas`.
    unsafe { &mut *LIMPIEZAS.0.get() }
}

pub(crate) fn limpieza_pendiente(recurso: u64, pixel: u32) {
    let l = limpiezas();
    match l.iter_mut().find(|x| x.0 == recurso) {
        Some(x) => x.1 = pixel,
        None => l.push((recurso, pixel)),
    }
}

/// La limpieza pendiente de `recurso`, gastandola.
fn tomar_limpieza(recurso: u64) -> Option<u32> {
    let l = limpiezas();
    let i = l.iter().position(|x| x.0 == recurso)?;
    Some(l.swap_remove(i).1)
}

pub(crate) fn olvidar_limpieza(recurso: u64) {
    let _ = tomar_limpieza(recurso);
}

/// **El subrecurso `sub` de un recurso donde se dibuja o se limpia** (ver
/// `d3d12_vistas`: el subrecurso, y la rebanada 3D << 32): sus palabras y
/// sus medidas. El 0 es `pixeles` (la cadena incluida). `None`: no lo
/// tiene, o es de bloques.
pub(crate) fn destino(recurso: u64, sub: u64) -> Option<(&'static mut [u32], u32, u32)> {
    // SAFETY: lo que llega aqui son Recursos de la casa (una vista).
    let r = unsafe { de::<crate::d3d12::Recurso>(recurso) };
    if sub == 0 {
        return (!r.pixeles.is_empty()).then(|| {
            // SAFETY: los pixeles de un Recurso de la casa, que no se sueltan.
            (unsafe { core::slice::from_raw_parts_mut(r.pixeles.as_mut_ptr(), r.pixeles.len()) }, r.ancho, r.alto)
        });
    }
    let t = r.tex.as_ref()?;
    let s = *t.subs.get(sub as u32 as usize)?;
    let rebanada = (sub >> 32) as u32;
    if t.almacen.elemento() != (4, 1) || rebanada >= s.hondo {
        return None;
    }
    let n = s.ancho as usize * s.alto as usize;
    let p = (t.datos + s.desde + rebanada as u64 * s.fila * s.filas as u64) as *mut u32;
    // SAFETY: una rebanada de un subrecurso, dentro de la memoria de la textura.
    Some((unsafe { core::slice::from_raw_parts_mut(p, n) }, s.ancho, s.alto))
}

/// **Hacerla ya**, si la hay: alguien va a LEER los pixeles.
pub(crate) fn aplicar_limpieza(recurso: u64) {
    if let Some(p) = tomar_limpieza(recurso) {
        // SAFETY: las limpiezas son de Recursos de la casa.
        unsafe { de::<crate::d3d12::Recurso>(recurso) }.pixeles.fill(p);
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "D3D12SerializeRootSignature" => dir!(d3d12_serialize_root_signature),
        "D3D12SerializeVersionedRootSignature" => dir!(d3d12_serialize_versioned_root_signature),
        _ => return None,
    })
}

// La lectura de floats del `.exe` (viewport) la hace la lista: ver d3d12.rs.
pub(crate) unsafe fn viewport_de(p: *const u8) -> [f32; 6] {
    core::array::from_fn(|k| f32_de(p, 4 * k))
}
