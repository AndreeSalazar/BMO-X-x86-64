//! **La tuberia de D3D12 de la casa** (P3b2, 27-09): root signature, PSO,
//! buferes, y lo que cada `Draw` ve.
//!
//! ```text
//!    D3D12SerializeRootSignature  la estructura del .exe -> RTS0 (los MISMOS
//!                                 bytes que Microsoft: bmo_proton_x::raiz)
//!    CreateRootSignature          RTS0 -> la firma, leida
//!    CreateGraphicsPipelineState  los dos DXIL leidos (bmo_proton_x::dxil),
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
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use alloc::format;

use bmo_proton_x::dxil::{self, Etapa, Sombreador};
use bmo_proton_x::lote::{enlazar, Enlace, Lote, NoDibuja, Topologia};
use bmo_proton_x::trama;
use bmo_proton_x::raiz::{self, Carga, Firma, Parametro, Rango};

use crate::com::{self, dar, de, nuevo, pide, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, S_OK};
use crate::{aviso, dir, plataforma};

// -- Constantes de D3D12 que se miran ---------------------------------------

const RS_VERSION_1: u32 = 1;
const DIMENSION_BUFFER: u32 = 1;
const FMT_R32G32B32A32_FLOAT: u32 = 2;
const FMT_R32G32B32_FLOAT: u32 = 6;
const FMT_R32G32_FLOAT: u32 = 16;
const FMT_R32_FLOAT: u32 = 41;
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

fn blob(bytes: Vec<u8>) -> u64 {
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
    pub vs: Sombreador,
    pub ps: Sombreador,
    pub entradas: Vec<EntradaIa>,
    /// D3D12_CULL_MODE: 1 ninguna, 2 delante, 3 detras.
    pub descarte: u32,
    pub antihorario: bool,
    pub topologia: u32,
    pub formato_rt: u32,
    /// Los dos sombreadores COMPILADOS y enlazados (P3b3), o por que no se
    /// pueden correr: entonces el PSO existe, y cada Draw lo dice.
    pub enlace: Result<Enlace, String>,
    /// Mezcla encendida o una mascara de escritura que no es RGBA: todavia no.
    pub mezcla: bool,
}


fn sombreador(bytecode: *const u8, tam: usize, etapa: Etapa, que: &'static str) -> Result<Sombreador, &'static str> {
    if bytecode.is_null() || tam == 0 {
        return Err(que);
    }
    // SAFETY: `tam` bytes del `.exe` (D3D12_SHADER_BYTECODE).
    let d = unsafe { core::slice::from_raw_parts(bytecode, tam) };
    let s = dxil::leer(d).map_err(|_| "CreateGraphicsPipelineState: un sombreador que no es DXIL (o no se lee)")?;
    if s.etapa != etapa {
        return Err("CreateGraphicsPipelineState: un sombreador de otra etapa en su hueco");
    }
    Ok(s)
}

/// Lee el `D3D12_GRAPHICS_PIPELINE_STATE_DESC` (656 B, desplazamientos
/// MEDIDOS con la cabecera de Windows: ver prueba/HACER.txt).
unsafe fn pso_de(d: *const u8) -> Result<Pso, &'static str> {
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
    let vs = sombreador(u64_de(d, 8) as *const u8, u64_de(d, 16) as usize, Etapa::Vertice, "CreateGraphicsPipelineState sin sombreador de vertices")?;
    let ps = sombreador(u64_de(d, 24) as *const u8, u64_de(d, 32) as usize, Etapa::Pixel, "CreateGraphicsPipelineState sin sombreador de pixeles")?;
    // El input layout, con los desplazamientos APPEND_ALIGNED resueltos.
    let (elems, n) = (u64_de(d, 552) as *const u8, u32_de(d, 560));
    let mut entradas: Vec<EntradaIa> = Vec::with_capacity(n as usize);
    let mut siguiente = [0u32; 16];
    for i in 0..n as usize {
        let e = elems.add(32 * i);
        let (formato, ranura, desde) = (u32_de(e, 12), u32_de(e, 16), u32_de(e, 20));
        let bytes = match formato {
            FMT_R32G32B32A32_FLOAT => 16,
            FMT_R32G32B32_FLOAT => 12,
            FMT_R32G32_FLOAT => 8,
            FMT_R32_FLOAT => 4,
            _ => return Err("un input layout con un formato que no es float de 32 bits: todavia no"),
        };
        let r = (ranura as usize).min(15);
        let desde = if desde == APPEND_ALIGNED { siguiente[r] } else { desde };
        siguiente[r] = desde + bytes;
        entradas.push(EntradaIa { semantica: cadena_c(u64_de(e, 0) as *const u8), indice: u32_de(e, 8), formato, ranura, desde });
    }
    // Cada elemento del sombreador de vertices tiene que venir del layout.
    for f in &vs.entradas {
        if !entradas.iter().any(|e| e.semantica.eq_ignore_ascii_case(&f.semantica) && e.indice == f.indice) {
            return Err("el sombreador de vertices lee una semantica que el input layout no da");
        }
    }
    let (n_rt, formato_rt) = (u32_de(d, 576), u32_de(d, 580));
    if n_rt != 1 {
        return Err("CreateGraphicsPipelineState con mas de un render target: todavia no");
    }
    if u32_de(d, 496) != 0 {
        aviso("CreateGraphicsPipelineState con profundidad: se apunta, y no se usa todavia");
    }
    // RenderTarget[0] de BlendState (+120): BlendEnable +8, LogicOpEnable
    // +12, la mascara de escritura +44.
    let mezcla = u32_de(d, 128) != 0 || u32_de(d, 132) != 0 || (d.add(164).read() & 0xF) != 0xF;
    let enlace = enlazar(&vs, &ps, &entradas);
    if let Err(m) = &enlace {
        aviso(m);
    }
    Ok(Pso {
        raiz,
        vs,
        ps,
        entradas,
        enlace,
        mezcla,
        descarte: u32_de(d, 452 + 4),
        antihorario: u32_de(d, 452 + 8) != 0,
        topologia: u32_de(d, 572),
        formato_rt,
    })
}

pub(crate) extern "win64" fn create_graphics_pipeline_state(_this: u64, desc: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::PSO) {
        return E_NOINTERFACE;
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_GRAPHICS_PIPELINE_STATE_DESC del `.exe`.
    match unsafe { pso_de(desc) } {
        Ok(pso) => {
            let vt = vtabla::<{ com::PSO }>(&[]);
            let obj = nuevo(com::PSO, vt, pso) as u64;
            // P3b3b: sus sombreadores, traducidos a x86-64 una vez, aqui.
            // SAFETY: el Pso recien creado; vive lo que el proceso.
            if let Ok(en) = &unsafe { de::<Pso>(obj) }.enlace {
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

/// 256 bytes alineados a 256: la unidad de un bufer de la casa (la alineacion
/// que D3D12 pide a un bufer de constantes).
#[repr(C, align(256))]
#[derive(Clone, Copy)]
struct Trozo([u8; 256]);

pub struct Bufer {
    trozos: Vec<Trozo>,
    pub bytes: usize,
}

impl Bufer {
    pub fn base(&self) -> u64 {
        self.trozos.as_ptr() as u64
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

/// `CreateCommittedResource(this, heap, banderas, desc, estado, clear, riid, pp)`.
/// `D3D12_RESOURCE_DESC` (56 B): Dimension +0, Width +16. Solo BUFFER.
pub(crate) extern "win64" fn create_committed_resource(_this: u64, _heap: *const u8, _banderas: u32, desc: *const u8, _estado: u32, _clear: *const u8, riid: *const Guid, pp: *mut u64) -> i32 {
    if !pide(riid, com::RESOURCE) {
        return E_NOINTERFACE;
    }
    if desc.is_null() {
        return E_INVALIDARG;
    }
    // SAFETY: un D3D12_RESOURCE_DESC del `.exe`.
    let (dimension, ancho) = unsafe { (u32_de(desc, 0), u64_de(desc, 16)) };
    if dimension != DIMENSION_BUFFER {
        aviso("CreateCommittedResource: solo buferes todavia (texturas, con los sombreadores que las lean)");
        return E_INVALIDARG;
    }
    let bytes = ancho as usize;
    let b = Bufer { trozos: vec![Trozo([0; 256]); bytes.div_ceil(256).max(1)], bytes };
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
    pub topologia: u32,
    pub vertices: Vista,
    pub indices: Vista,
    pub viewport: [f32; 6],
    pub tijera: [i32; 4],
    pub rtv: u64,
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
    if e.rtv == 0 {
        aviso("Draw sin OMSetRenderTargets: no hay donde dibujar");
        return;
    }
    // SAFETY: el descriptor guarda un Recurso de la casa.
    if unsafe { crate::d3d12::recurso_de(e.rtv) }.formato != pso.formato_rt {
        aviso("Draw sobre un render target de otro formato que el del PSO");
        return;
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
    let entrada = |s: &Sombreador| s.modulo.entrada().map(|f| f.nombre.clone()).unwrap_or_default();
    let mut d = Dibujo {
        vs: entrada(&pso.vs),
        ps: entrada(&pso.ps),
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
                    let n = match x.formato {
                        FMT_R32G32B32A32_FLOAT => 4,
                        FMT_R32G32B32_FLOAT => 3,
                        FMT_R32G32_FLOAT => 2,
                        _ => 1,
                    };
                    (0..n).map(|k| {
                        let o = x.desde as usize + 4 * k;
                        v.get(o..o + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0.0)
                    }).collect()
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
const FMT_B8G8R8A8_UNORM: u32 = 87;
const FMT_R8G8B8A8_UNORM: u32 = 28;

/// **Pintar un Draw** (P3b3): el sombreador de vertices por cada vertice que
/// piden los indices (una vez cada uno), los triangulos por la trama, y el de
/// pixeles en cada pixel que cubren, sobre el render target. Lo que no sabe
/// hacer todavia lo dice y NO pinta: nunca un dibujo a medias callado.
fn pintar(e: &Estado, pso: &Pso, cuantos: u32, instancias: u32, primero: u32, base: i32, indexado: bool) {
    let en = match &pso.enlace {
        Ok(en) => en,
        Err(m) => {
            aviso(&format!("Draw no se dibuja: {m}"));
            return;
        }
    };
    if pso.mezcla {
        aviso("Draw no se dibuja: mezcla, operacion logica o mascara de escritura parcial, todavia no");
        return;
    }
    if instancias > 1 {
        aviso("Draw con varias instancias: se dibuja una (no hay datos por instancia todavia)");
    }
    if e.tijera == [0; 4] {
        aviso("Draw sin RSSetScissorRects: en D3D12 la tijera siempre corta, y vacia no deja pintar nada");
        return;
    }
    // SAFETY: el descriptor guarda un Recurso de la casa (Draw ya lo miro).
    let rt = unsafe { de::<crate::d3d12::Recurso>(e.rtv) };
    let bgra = match rt.formato {
        FMT_B8G8R8A8_UNORM => true,
        FMT_R8G8B8A8_UNORM => false,
        _ => {
            aviso("Draw sobre un render target que no es RGBA/BGRA de 8 bits: todavia no");
            return;
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
    // El cbuffer b0 (lo que los dos sombreadores leen de el).
    // SAFETY: un RootSignature de la casa.
    let firma = unsafe { &de::<RootSignature>(e.raiz).firma };
    let filas = en.vs.filas_cb.max(en.ps.filas_cb) as usize;
    let cb: &[u8] = match firma.parametros.iter().position(|p| p.tipo == raiz::CBV && matches!(p.carga, Carga::Descriptor { registro: 0, espacio: 0 })) {
        _ if filas == 0 => &[],
        Some(i) => match resolver(e.cbv.get(i).copied().unwrap_or(0), filas * 16) {
            Some(c) => c,
            None => {
                aviso("Draw: el cbuffer b0 no es un bufer de la casa (o es mas corto de lo que se lee)");
                return;
            }
        },
        None => {
            aviso("Draw: los sombreadores leen b0 y la root signature no tiene un CBV b0 en la raiz: todavia no");
            return;
        }
    };
    // Hasta aqui, D3D12. Lo que sigue es un LOTE, y lo dibuja quien la
    // plataforma diga (hoy la CPU; luego VERRANO con la 3060).
    let lote = Lote {
        enlace: en,
        entradas: &pso.entradas,
        vertices: vb,
        paso,
        ids: &ids,
        topologia,
        cb,
        reglas: trama::Reglas { viewport: e.viewport, tijera: e.tijera, descarte: pso.descarte, antihorario: pso.antihorario },
    };
    let (ancho, alto) = (rt.ancho, rt.alto);
    let mut destino = trama::Destino { pixeles: &mut rt.pixeles, ancho, alto, bgra };
    match (plataforma().dibujar)(&lote, &mut destino) {
        Ok(c) if c.sin_recortar > 0 => aviso("Draw: triangulos que cruzan el plano cercano o salen de la profundidad: sin recortar todavia, no se pintan"),
        Ok(_) => {}
        Err(NoDibuja::IndiceFuera(_)) => aviso("Draw: un indice que pasa del bufer de vertices"),
        Err(NoDibuja::SinVertices) => aviso("Draw sin vertices que leer"),
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "D3D12SerializeRootSignature" => dir!(d3d12_serialize_root_signature),
        _ => return None,
    })
}

// La lectura de floats del `.exe` (viewport) la hace la lista: ver d3d12.rs.
pub(crate) unsafe fn viewport_de(p: *const u8) -> [f32; 6] {
    core::array::from_fn(|k| f32_de(p, 4 * k))
}
