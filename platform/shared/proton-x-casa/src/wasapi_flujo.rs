//! **WASAPI: el flujo** (03-10, N4.5): `IAudioClient` y sus servicios -- donde
//! el juego escribe su sonido, y el reloj del que cuelgan sus videos.
//!
//! [carril]  AMARILLO  escribe en el anillo del sonido de la plataforma
//! [cuesta]  APARATO   un reloj mal contado y el juego se adelanta, se
//!                     atasca, o sus videos (Bink) se congelan esperandolo
//! [riesgo]  RELOJ     lo sonado sale del aparato (lo que el anillo ya no
//!                     tiene) o, sin aparato, de la hora; el banco mueve la
//!                     hora a mano y mira el relleno y el evento
//! [consumo] LATE      mientras un flujo corre, cada vuelta del planificador
//!                     (`latir`) empuja lo convertido al anillo y, una vez por
//!                     periodo, enciende el evento del juego
//!
//! ```text
//!    el juego             la casa                      la plataforma
//!    GetBuffer(n)     ->  la hoja (su formato)
//!    ReleaseBuffer(n) ->  convertida a s16 estereo  ->  el anillo del tubo
//!                         (`bmo_proton_x::pcm`) a       (lo que quepa; lo
//!                         la cola                       demas, al latir)
//!    GetCurrentPadding    lo escrito menos lo SONADO
//!    su evento            se enciende cuando cabe un periodo, y cada periodo
//!    IAudioClock          lo sonado, en bytes de su formato por segundo
//! ```
//!
//! Solo UN flujo suena por el aparato de verdad (el primero que hace Start:
//! el audifono es exclusivo); los demas corren con el reloj de la hora, para
//! que ningun hilo del juego se quede esperando para siempre.

use alloc::vec::Vec;

use bmo_proton_x::pcm::{Conversor, Formato};

use crate::com::{self, dar, de, guid, nuevo, vtabla, Guid, E_INVALIDARG, E_NOINTERFACE, E_POINTER, S_FALSE, S_OK};
use crate::wasapi::{estado, hz, texto_de_tarea};
use crate::{aviso, dir, plataforma};

pub const IID_CLIENTE: Guid = guid(0x1CB9_AD4C, 0xDBFA, 0x4C32, [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2]);
pub const IID_CLIENTE2: Guid = guid(0x7267_78CD, 0xF60A, 0x4EDA, [0x82, 0xDE, 0xE4, 0x76, 0x10, 0xCD, 0x78, 0xAA]);
pub const IID_CLIENTE3: Guid = guid(0x7ED4_EE07, 0x8E67, 0x4CD4, [0x8C, 0x1A, 0x2B, 0x7A, 0x59, 0x87, 0xAD, 0x42]);
pub const IID_RENDER: Guid = guid(0xF294_ACFC, 0x3146, 0x4483, [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2]);
pub const IID_RELOJ: Guid = guid(0xCD63_314F, 0x3FBA, 0x4A1B, [0x81, 0x2C, 0xEF, 0x96, 0x35, 0x87, 0x28, 0xE7]);
pub const IID_VOLUMEN: Guid = guid(0x87CE_5498, 0x68D6, 0x44E5, [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8]);
pub const IID_SESION: Guid = guid(0xF4B1_A599, 0x7266, 0x4319, [0xA8, 0xCA, 0xE7, 0x0A, 0xCB, 0x11, 0xE8, 0xCD]);
pub const IID_SESION2: Guid = guid(0xBFB7_FF88, 0x7239, 0x4FC9, [0x8F, 0xA2, 0x07, 0xC9, 0x50, 0xBE, 0x9C, 0x6D]);
pub const IID_VOLUMEN_FLUJO: Guid = guid(0x9301_4887, 0x242D, 0x4068, [0x8A, 0x15, 0xCF, 0x5E, 0x93, 0xB9, 0x0F, 0xE3]);
pub const IID_VOLUMEN_CANALES: Guid = guid(0x1C15_8861, 0xB533, 0x4B30, [0xB1, 0xCF, 0xE8, 0x53, 0xE5, 0x1C, 0x59, 0xB8]);

pub const M_CLIENTE: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "Initialize", "GetBufferSize", "GetStreamLatency", "GetCurrentPadding",
    "IsFormatSupported", "GetMixFormat", "GetDevicePeriod", "Start", "Stop", "Reset", "SetEventHandle", "GetService",
    "IsOffloadCapable", "SetClientProperties", "GetBufferSizeLimits", "GetSharedModeEnginePeriod",
    "GetCurrentSharedModeEnginePeriod", "InitializeSharedAudioStream",
];
pub const M_RENDER: &[&str] = &["QueryInterface", "AddRef", "Release", "GetBuffer", "ReleaseBuffer"];
pub const M_RELOJ: &[&str] = &["QueryInterface", "AddRef", "Release", "GetFrequency", "GetPosition", "GetCharacteristics"];
pub const M_VOLUMEN: &[&str] = &["QueryInterface", "AddRef", "Release", "SetMasterVolume", "GetMasterVolume", "SetMute", "GetMute"];
pub const M_SESION: &[&str] = &[
    "QueryInterface", "AddRef", "Release", "GetState", "GetDisplayName", "SetDisplayName", "GetIconPath", "SetIconPath",
    "GetGroupingParam", "SetGroupingParam", "RegisterAudioSessionNotification", "UnregisterAudioSessionNotification",
    "GetSessionIdentifier", "GetSessionInstanceIdentifier", "GetProcessId", "IsSystemSoundsSession", "SetDuckingPreference",
];
/// IAudioStreamVolume e IChannelAudioVolume tienen los mismos huecos.
pub const M_VOLUMEN_CANALES: &[&str] = &["QueryInterface", "AddRef", "Release", "GetChannelCount", "SetChannelVolume", "GetChannelVolume", "SetAllVolumes", "GetAllVolumes"];

/// Los AUDCLNT_E_* de audioclient.h.
const NO_INICIADO: i32 = 0x8889_0001_u32 as i32;
const YA_INICIADO: i32 = 0x8889_0002_u32 as i32;
const NO_PARADO: i32 = 0x8889_0005_u32 as i32;
const DEMASIADO: i32 = 0x8889_0006_u32 as i32;
const FUERA_DE_ORDEN: i32 = 0x8889_0007_u32 as i32;
const FORMATO_NO: i32 = 0x8889_0008_u32 as i32;
const MEDIDA_MALA: i32 = 0x8889_0009_u32 as i32;
const EXCLUSIVO_NO: i32 = 0x8889_000E_u32 as i32;
const SIN_EVENTO: i32 = 0x8889_0014_u32 as i32;

/// AUDCLNT_STREAMFLAGS_EVENTCALLBACK, _AUTOCONVERTPCM y _SRC_DEFAULT_QUALITY.
const POR_EVENTO: u32 = 0x0004_0000;
const CONVERTIR: u32 = 0x8000_0000 | 0x0800_0000;
/// AUDCLNT_BUFFERFLAGS_SILENT: lo escrito no cuenta, es silencio.
const SILENCIO: u32 = 2;
/// El periodo del motor de Windows: 10 ms (en unidades de 100 ns), y el
/// minimo, 3 ms.
const PERIODO: i64 = 100_000;
const PERIODO_MIN: i64 = 30_000;
/// El "evento" de un flujo que no va por evento (no es un handle).
const SIN_EVENTO_HACE_FALTA: u64 = u64::MAX;
/// s16 estereo: lo que mide un fotograma en el anillo.
const BYTES_SALIDA: u64 = 4;

/// **Un flujo de WASAPI** (un IAudioClient).
pub struct Cliente {
    iniciado: bool,
    formato: Formato,
    conversor: Option<Conversor>,
    /// Los fotogramas de su bufer, y los de un periodo.
    marcos: u32,
    periodo: u32,
    evento: u64,
    corriendo: bool,
    /// Fotogramas soltados por el juego desde el principio (o el Reset).
    escritos: u64,
    /// Los que pidio el ultimo GetBuffer (0: ninguno pendiente).
    pedido: u32,
    /// La hoja donde escribe el juego (su formato).
    hoja: Vec<u8>,
    /// Lo convertido que aun no cupo en el anillo.
    cola: Vec<u8>,
    /// Bytes de salida dados al anillo, y los que no cuentan (antes del Reset).
    entregados: u64,
    ajuste: u64,
    /// El reloj sin aparato: lo sonado antes del ultimo Start, y su hora.
    reloj_antes: u64,
    reloj_desde: u64,
    /// La hora en que se encendio su evento por ultima vez.
    ultimo_evento: u64,
    volumen: f32,
    mudo: bool,
    /// Sus servicios, creados al pedirlos (uno de cada).
    servicios: [u64; 6],
}

fn cliente<'a>(this: u64) -> &'a mut Cliente {
    // SAFETY: un IAudioClient de la casa (lo dice su vtabla).
    unsafe { de::<Cliente>(this) }
}

/// El IAudioClient de un servicio (render, reloj, volumen...): lo guarda dentro.
fn obj_de(servicio: u64) -> u64 {
    // SAFETY: un servicio de la casa guarda el cliente que lo dio.
    unsafe { *de::<u64>(servicio) }
}

fn cliente_de<'a>(servicio: u64) -> &'a mut Cliente {
    cliente(obj_de(servicio))
}

fn ahora() -> u64 {
    (plataforma().ahora_ns)()
}

/// **Un IAudioClient nuevo** (lo pide `IMMDevice::Activate`).
pub(crate) fn nuevo_cliente() -> u64 {
    let vt = vtabla::<{ com::CLIENTE }>(&[
        (3, dir!(initialize)),
        (4, dir!(get_buffer_size)),
        (5, dir!(get_stream_latency)),
        (6, dir!(get_current_padding)),
        (7, dir!(is_format_supported)),
        (8, dir!(get_mix_format)),
        (9, dir!(get_device_period)),
        (10, dir!(start)),
        (11, dir!(stop)),
        (12, dir!(reset)),
        (13, dir!(set_event_handle)),
        (14, dir!(get_service)),
        (15, dir!(is_offload_capable)),
        (16, dir!(set_client_properties)),
        (17, dir!(get_buffer_size_limits)),
        (18, dir!(get_shared_mode_engine_period)),
        (19, dir!(get_current_shared_mode_engine_period)),
        (20, dir!(initialize_shared_audio_stream)),
    ]);
    let c = Cliente {
        iniciado: false,
        formato: Formato::mezcla(hz()),
        conversor: None,
        marcos: 0,
        periodo: 0,
        evento: 0,
        corriendo: false,
        escritos: 0,
        pedido: 0,
        hoja: Vec::new(),
        cola: Vec::new(),
        entregados: 0,
        ajuste: 0,
        reloj_antes: 0,
        reloj_desde: 0,
        ultimo_evento: 0,
        volumen: 1.0,
        mudo: false,
        servicios: [0; 6],
    };
    let obj = nuevo(com::CLIENTE, vt, c) as u64;
    estado().flujos.push(obj);
    obj
}

/// Un WAVEFORMATEX del `.exe`: sus 18 bytes y los de `cbSize` (hasta 64).
fn leer_formato(p: *const u8) -> Option<Formato> {
    if p.is_null() {
        return None;
    }
    // SAFETY: un WAVEFORMATEX del `.exe`: 18 bytes, y `cbSize` mas.
    let extra = unsafe { (p.add(16) as *const u16).read_unaligned() } as usize;
    // SAFETY: como arriba.
    let b = unsafe { core::slice::from_raw_parts(p, 18 + extra.min(46)) };
    Formato::de_waveformatex(b)
}

/// Lo que acepta el modo compartido: la frecuencia y los canales del
/// aparato (los bits, cualquiera); con AUTOCONVERTPCM, todo lo que se lee.
fn acepta(f: &Formato, banderas: u32) -> bool {
    banderas & CONVERTIR != 0 || (f.hz == hz() && f.canales == 2)
}

/// El formato de la mezcla, en `CoTaskMemAlloc` (lo suelta el `.exe`).
fn mezcla_de_tarea() -> u64 {
    let f = Formato::mezcla(hz()).a_waveformatextensible();
    let p = crate::dll_chicas::co_task_mem_alloc(f.len() as u64);
    if p != 0 {
        // SAFETY: 40 bytes recien pedidos.
        unsafe { core::ptr::copy_nonoverlapping(f.as_ptr(), p as *mut u8, f.len()) };
    }
    p
}

fn escribir<T>(p: *mut T, v: T) -> i32 {
    if p.is_null() {
        return E_POINTER;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { p.write_unaligned(v) };
    S_OK
}

impl Cliente {
    fn es_propietario(&self, this: u64) -> bool {
        estado().propietario == this
    }

    /// **Los fotogramas ya sonados** (del formato del juego).
    fn sonados(&self, this: u64) -> u64 {
        if !self.iniciado {
            return 0;
        }
        let total = match (&self.conversor, self.es_propietario(this), plataforma().sonido) {
            (Some(c), true, Some(s)) => {
                let en_anillo = ((s.pendientes)() as u64).min(self.entregados);
                c.entrada_de((self.entregados - en_anillo).saturating_sub(self.ajuste) / BYTES_SALIDA)
            }
            _ => {
                let corre = if self.corriendo { ahora().saturating_sub(self.reloj_desde) as u128 * self.formato.hz as u128 / 1_000_000_000 } else { 0 };
                self.reloj_antes + corre as u64
            }
        };
        total.min(self.escritos)
    }

    /// Lo escrito y aun no sonado, en fotogramas.
    fn relleno(&self, this: u64) -> u32 {
        (self.escritos - self.sonados(this)).min(self.marcos as u64) as u32
    }

    /// **Empujar** lo convertido al anillo, lo que quepa.
    fn bombear(&mut self, this: u64) {
        if !self.es_propietario(this) || self.cola.is_empty() {
            return;
        }
        let Some(s) = plataforma().sonido else { return };
        let n = (s.escribir)(&self.cola).min(self.cola.len());
        self.cola.drain(..n);
        self.entregados += n as u64;
    }

    fn iniciar(&mut self, banderas: u32, duracion: i64, f: Formato) -> i32 {
        if self.iniciado {
            return YA_INICIADO;
        }
        if !acepta(&f, banderas) {
            return FORMATO_NO;
        }
        // El bufer: lo pedido, y no menos de dos periodos (como Windows,
        // que redondea hacia arriba).
        let periodo = (PERIODO as u64 * f.hz as u64 / 10_000_000) as u32;
        let pedido = (duracion.max(0) as u64 * f.hz as u64 / 10_000_000) as u32;
        self.marcos = pedido.max(2 * periodo);
        self.periodo = periodo;
        self.formato = f;
        self.conversor = Some(Conversor::nuevo(f, hz()));
        self.evento = 0;
        self.iniciado = true;
        self.pedido = 0;
        self.hoja = Vec::new();
        // Sin AUDCLNT_STREAMFLAGS_EVENTCALLBACK no hace falta evento: se
        // marca con un valor que no es un handle, y Start no lo pide.
        if banderas & POR_EVENTO == 0 {
            self.evento = SIN_EVENTO_HACE_FALTA;
        }
        S_OK
    }
}

// -- IAudioClient -----------------------------------------------------------

extern "win64" fn initialize(this: u64, modo: u32, banderas: u32, duracion: i64, _periodicidad: i64, fmt: *const u8, _sesion: *const Guid) -> i32 {
    if modo != 0 {
        return EXCLUSIVO_NO;
    }
    let Some(f) = leer_formato(fmt) else {
        aviso("IAudioClient::Initialize con un formato que no es PCM ni float: AUDCLNT_E_UNSUPPORTED_FORMAT");
        return FORMATO_NO;
    };
    cliente(this).iniciar(banderas, duracion, f)
}

extern "win64" fn get_buffer_size(this: u64, sale: *mut u32) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    escribir(sale, c.marcos)
}

extern "win64" fn get_stream_latency(this: u64, sale: *mut i64) -> i32 {
    if !cliente(this).iniciado {
        return NO_INICIADO;
    }
    escribir(sale, PERIODO)
}

extern "win64" fn get_current_padding(this: u64, sale: *mut u32) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    c.bombear(this);
    escribir(sale, c.relleno(this))
}

extern "win64" fn is_format_supported(_this: u64, modo: u32, fmt: *const u8, cerca: *mut u64) -> i32 {
    if !cerca.is_null() {
        // SAFETY: un puntero del `.exe`.
        unsafe { cerca.write(0) };
    }
    let Some(f) = leer_formato(fmt) else { return FORMATO_NO };
    if modo != 0 {
        return FORMATO_NO;
    }
    if acepta(&f, 0) {
        return S_OK;
    }
    // Lo de Windows: S_FALSE y el mas parecido (la mezcla), si hay donde.
    if cerca.is_null() {
        return FORMATO_NO;
    }
    // SAFETY: un puntero del `.exe`.
    unsafe { cerca.write(mezcla_de_tarea()) };
    S_FALSE
}

extern "win64" fn get_mix_format(_this: u64, sale: *mut u64) -> i32 {
    escribir(sale, mezcla_de_tarea())
}

extern "win64" fn get_device_period(_this: u64, defecto: *mut i64, minimo: *mut i64) -> i32 {
    if !defecto.is_null() {
        escribir(defecto, PERIODO);
    }
    if !minimo.is_null() {
        escribir(minimo, PERIODO_MIN);
    }
    S_OK
}

extern "win64" fn start(this: u64) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    if c.corriendo {
        return NO_PARADO;
    }
    if c.evento == 0 {
        return SIN_EVENTO;
    }
    // El primero en arrancar se queda el aparato de verdad (si lo hay).
    let e = estado();
    if e.propietario == 0 && crate::wasapi::hay_aparato() {
        e.propietario = this;
    }
    c.corriendo = true;
    c.reloj_desde = ahora();
    c.bombear(this);
    S_OK
}

extern "win64" fn stop(this: u64) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    if !c.corriendo {
        return S_FALSE;
    }
    c.reloj_antes = c.sonados(this);
    c.corriendo = false;
    S_OK
}

extern "win64" fn reset(this: u64) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    if c.corriendo {
        return NO_PARADO;
    }
    // Lo que el anillo aun tiene se deja sonar; desde aqui, todo cuenta de cero.
    c.ajuste = c.entregados;
    c.escritos = 0;
    c.reloj_antes = 0;
    c.cola.clear();
    if let Some(conv) = &mut c.conversor {
        *conv = Conversor::nuevo(conv.de, conv.a_hz);
    }
    S_OK
}

extern "win64" fn set_event_handle(this: u64, h: u64) -> i32 {
    let c = cliente(this);
    if !c.iniciado {
        return NO_INICIADO;
    }
    if h == 0 {
        return E_INVALIDARG;
    }
    c.evento = h;
    S_OK
}

/// Los servicios: (hueco en `servicios`, interfaz).
fn servicio(this: u64, k: usize, interfaz: usize) -> u64 {
    let c = cliente(this);
    if c.servicios[k] == 0 {
        let vt = match interfaz {
            com::RENDER => vtabla::<{ com::RENDER }>(&[(3, dir!(get_buffer)), (4, dir!(release_buffer))]),
            com::RELOJ => vtabla::<{ com::RELOJ }>(&[(3, dir!(get_frequency)), (4, dir!(get_position)), (5, dir!(get_characteristics))]),
            com::VOLUMEN => vtabla::<{ com::VOLUMEN }>(&[(3, dir!(set_master_volume)), (4, dir!(get_master_volume)), (5, dir!(set_mute)), (6, dir!(get_mute))]),
            com::SESION => vtabla::<{ com::SESION }>(&[
                (3, dir!(sesion_get_state)),
                (4, dir!(sesion_texto)),
                (5, dir!(sesion_poner)),
                (6, dir!(sesion_texto)),
                (7, dir!(sesion_poner)),
                (8, dir!(sesion_get_grouping)),
                (9, dir!(sesion_poner)),
                (10, dir!(sesion_poner)),
                (11, dir!(sesion_poner)),
                (12, dir!(sesion_texto)),
                (13, dir!(sesion_texto)),
                (14, dir!(sesion_get_process_id)),
                (15, dir!(sesion_es_del_sistema)),
                (16, dir!(sesion_poner)),
            ]),
            com::VOLUMEN_FLUJO => vtabla::<{ com::VOLUMEN_FLUJO }>(&[
                (3, dir!(get_channel_count)),
                (4, dir!(set_channel_volume)),
                (5, dir!(get_channel_volume)),
                (6, dir!(set_all_volumes)),
                (7, dir!(get_all_volumes)),
            ]),
            // IChannelAudioVolume: los mismos, con el GUID del contexto detras.
            _ => vtabla::<{ com::VOLUMEN_CANALES }>(&[
                (3, dir!(get_channel_count)),
                (4, dir!(set_channel_volume_ctx)),
                (5, dir!(get_channel_volume)),
                (6, dir!(set_all_volumes_ctx)),
                (7, dir!(get_all_volumes)),
            ]),
        };
        c.servicios[k] = nuevo(interfaz, vt, this) as u64;
    }
    com::add_ref_de(c.servicios[k]);
    c.servicios[k]
}

extern "win64" fn get_service(this: u64, iid: *const Guid, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    if !cliente(this).iniciado {
        // SAFETY: un puntero del `.exe`.
        unsafe { sale.write(0) };
        return NO_INICIADO;
    }
    let (k, i) = match () {
        _ if com::pide(iid, com::RENDER) => (0, com::RENDER),
        _ if com::pide(iid, com::RELOJ) => (1, com::RELOJ),
        _ if com::pide(iid, com::VOLUMEN) => (2, com::VOLUMEN),
        _ if com::pide(iid, com::SESION) => (3, com::SESION),
        _ if com::pide(iid, com::VOLUMEN_FLUJO) => (4, com::VOLUMEN_FLUJO),
        _ if com::pide(iid, com::VOLUMEN_CANALES) => (5, com::VOLUMEN_CANALES),
        _ => {
            aviso(&alloc::format!("IAudioClient::GetService {}: la casa no lo tiene", crate::com_basico::clsid_texto(iid as *const u8)));
            // SAFETY: un puntero del `.exe`.
            unsafe { sale.write(0) };
            return E_NOINTERFACE;
        }
    };
    dar(sale, servicio(this, k, i))
}

extern "win64" fn is_offload_capable(_this: u64, _categoria: u32, sale: *mut i32) -> i32 {
    escribir(sale, 0)
}

extern "win64" fn set_client_properties(_this: u64, _p: *const u8) -> i32 {
    S_OK
}

extern "win64" fn get_buffer_size_limits(_this: u64, _fmt: *const u8, _evento: i32, minimo: *mut i64, maximo: *mut i64) -> i32 {
    escribir(minimo, 2 * PERIODO);
    escribir(maximo, 20_000_000)
}

extern "win64" fn get_shared_mode_engine_period(_this: u64, fmt: *const u8, defecto: *mut u32, fundamental: *mut u32, minimo: *mut u32, maximo: *mut u32) -> i32 {
    let f = leer_formato(fmt).map_or(hz(), |f| f.hz);
    let p = (PERIODO as u64 * f as u64 / 10_000_000) as u32;
    for x in [defecto, fundamental, minimo, maximo] {
        if !x.is_null() {
            escribir(x, p);
        }
    }
    S_OK
}

extern "win64" fn get_current_shared_mode_engine_period(_this: u64, fmt: *mut u64, periodo: *mut u32) -> i32 {
    if !fmt.is_null() {
        escribir(fmt, mezcla_de_tarea());
    }
    escribir(periodo, (PERIODO as u64 * hz() as u64 / 10_000_000) as u32)
}

extern "win64" fn initialize_shared_audio_stream(this: u64, banderas: u32, periodo: u32, fmt: *const u8, _sesion: *const Guid) -> i32 {
    let Some(f) = leer_formato(fmt) else { return FORMATO_NO };
    let duracion = periodo as i64 * 10_000_000 / f.hz.max(1) as i64;
    cliente(this).iniciar(banderas, duracion, f)
}

// -- IAudioRenderClient -----------------------------------------------------

extern "win64" fn get_buffer(this: u64, n: u32, sale: *mut u64) -> i32 {
    if sale.is_null() {
        return E_POINTER;
    }
    let obj = obj_de(this);
    let c = cliente_de(this);
    if c.pedido != 0 {
        return FUERA_DE_ORDEN;
    }
    if n > c.marcos - c.relleno(obj) {
        return DEMASIADO;
    }
    let bytes = n as usize * c.formato.alinea as usize;
    if c.hoja.len() < bytes {
        c.hoja.resize(bytes, 0);
    }
    c.pedido = n;
    // SAFETY: un puntero del `.exe`.
    unsafe { sale.write(if n == 0 { 0 } else { c.hoja.as_mut_ptr() as u64 }) };
    S_OK
}

extern "win64" fn release_buffer(this: u64, n: u32, banderas: u32) -> i32 {
    let obj = obj_de(this);
    let c = cliente_de(this);
    if n > c.pedido {
        return MEDIDA_MALA;
    }
    c.pedido = 0;
    if n == 0 {
        return S_OK;
    }
    let bytes = n as usize * c.formato.alinea as usize;
    if banderas & SILENCIO != 0 {
        c.hoja[..bytes].fill(if c.formato.bits == 8 { 0x80 } else { 0 });
    }
    let desde = c.cola.len();
    if let Some(conv) = &mut c.conversor {
        conv.convertir(&c.hoja[..bytes], &mut c.cola);
    }
    // El volumen de la sesion (y el mudo), sobre lo recien convertido.
    if c.mudo || c.volumen < 1.0 {
        let v = if c.mudo { 0.0 } else { c.volumen.max(0.0) };
        for m in c.cola[desde..].chunks_exact_mut(2) {
            let x = (i16::from_le_bytes([m[0], m[1]]) as f32 * v) as i16;
            m.copy_from_slice(&x.to_le_bytes());
        }
    }
    c.escritos += n as u64;
    c.bombear(obj);
    S_OK
}

// -- IAudioClock ------------------------------------------------------------

extern "win64" fn get_frequency(this: u64, sale: *mut u64) -> i32 {
    let c = cliente_de(this);
    // Lo de Windows en modo compartido: bytes de su formato por segundo.
    escribir(sale, c.formato.hz as u64 * c.formato.alinea as u64)
}

extern "win64" fn get_position(this: u64, pos: *mut u64, qpc: *mut u64) -> i32 {
    let obj = obj_de(this);
    let c = cliente_de(this);
    if !qpc.is_null() {
        // En unidades de 100 ns, como QueryPerformanceCounter pasado a 10 MHz.
        escribir(qpc, ahora() / 100);
    }
    escribir(pos, c.sonados(obj) * c.formato.alinea as u64)
}

extern "win64" fn get_characteristics(_this: u64, sale: *mut u32) -> i32 {
    // AUDIOCLOCK_CHARACTERISTIC_FIXED_FREQ.
    escribir(sale, 1)
}

// -- ISimpleAudioVolume -----------------------------------------------------

extern "win64" fn set_master_volume(this: u64, nivel: f32, _ctx: *const Guid) -> i32 {
    if !(0.0..=1.0).contains(&nivel) {
        return E_INVALIDARG;
    }
    cliente_de(this).volumen = nivel;
    S_OK
}

extern "win64" fn get_master_volume(this: u64, sale: *mut f32) -> i32 {
    escribir(sale, cliente_de(this).volumen)
}

extern "win64" fn set_mute(this: u64, mudo: i32, _ctx: *const Guid) -> i32 {
    cliente_de(this).mudo = mudo != 0;
    S_OK
}

extern "win64" fn get_mute(this: u64, sale: *mut i32) -> i32 {
    escribir(sale, cliente_de(this).mudo as i32)
}

// -- IAudioSessionControl2 --------------------------------------------------

extern "win64" fn sesion_get_state(this: u64, sale: *mut u32) -> i32 {
    // AudioSessionStateActive (1) mientras corre; Inactive (0) si no.
    escribir(sale, cliente_de(this).corriendo as u32)
}

/// Los textos de la sesion (nombre, icono, identificadores): vacios, o el
/// de la casa, en `CoTaskMemAlloc`.
extern "win64" fn sesion_texto(_this: u64, sale: *mut u64) -> i32 {
    escribir(sale, texto_de_tarea(""))
}

/// Lo que la sesion acepta y no cambia nada (nombre, icono, avisos...).
extern "win64" fn sesion_poner(_this: u64, _a: u64, _b: u64) -> i32 {
    S_OK
}

extern "win64" fn sesion_get_grouping(_this: u64, sale: *mut Guid) -> i32 {
    escribir(sale, [0u8; 16])
}

extern "win64" fn sesion_get_process_id(_this: u64, sale: *mut u32) -> i32 {
    escribir(sale, crate::kernel32::id_del_proceso())
}

extern "win64" fn sesion_es_del_sistema(_this: u64) -> i32 {
    S_FALSE
}

// -- IAudioStreamVolume e IChannelAudioVolume --------------------------------

extern "win64" fn get_channel_count(this: u64, sale: *mut u32) -> i32 {
    escribir(sale, cliente_de(this).formato.canales as u32)
}

extern "win64" fn set_channel_volume(this: u64, i: u32, nivel: f32) -> i32 {
    if i >= cliente_de(this).formato.canales as u32 || !(0.0..=1.0).contains(&nivel) {
        return E_INVALIDARG;
    }
    S_OK
}

extern "win64" fn set_channel_volume_ctx(this: u64, i: u32, nivel: f32, _ctx: *const Guid) -> i32 {
    set_channel_volume(this, i, nivel)
}

extern "win64" fn get_channel_volume(this: u64, i: u32, sale: *mut f32) -> i32 {
    if i >= cliente_de(this).formato.canales as u32 {
        return E_INVALIDARG;
    }
    escribir(sale, 1.0)
}

extern "win64" fn set_all_volumes(this: u64, n: u32, _v: *const f32) -> i32 {
    if n != cliente_de(this).formato.canales as u32 {
        return E_INVALIDARG;
    }
    S_OK
}

extern "win64" fn set_all_volumes_ctx(this: u64, n: u32, v: *const f32, _ctx: *const Guid) -> i32 {
    set_all_volumes(this, n, v)
}

extern "win64" fn get_all_volumes(this: u64, n: u32, v: *mut f32) -> i32 {
    if n != cliente_de(this).formato.canales as u32 || v.is_null() {
        return E_INVALIDARG;
    }
    for k in 0..n as usize {
        // SAFETY: `n` floats del `.exe`.
        unsafe { v.add(k).write_unaligned(1.0) };
    }
    S_OK
}

// -- El latido ----------------------------------------------------------------

/// **El latido** (lo llama el planificador en cada vuelta): cada flujo que
/// corre empuja lo suyo al anillo, y si cabe un periodo, su evento se
/// enciende -- como mucho una vez por periodo, o en el acto si se quedo sin
/// nada que sonar.
pub fn latir() {
    let t = ahora();
    // Por indice, sin copiar la lista: esto corre en cada vuelta.
    for k in 0..estado().flujos.len() {
        let obj = estado().flujos[k];
        let c = cliente(obj);
        if !c.corriendo {
            continue;
        }
        c.bombear(obj);
        if c.evento == 0 || c.evento == SIN_EVENTO_HACE_FALTA {
            continue;
        }
        let relleno = c.relleno(obj);
        let periodo_ns = c.periodo as u64 * 1_000_000_000 / c.formato.hz.max(1) as u64;
        let toca = t.saturating_sub(c.ultimo_evento) >= periodo_ns || relleno == 0;
        if relleno + c.periodo <= c.marcos && toca {
            c.ultimo_evento = t;
            crate::hilos::encender_evento(c.evento);
        }
    }
}

/// Si algun flujo corre por evento: entonces "todos esperan" no es un bloqueo
/// mutuo -- el latido encendera su evento cuando suene lo que tiene.
pub(crate) fn alguno_corre() -> bool {
    estado().flujos.iter().any(|&o| {
        let c = cliente(o);
        c.corriendo && c.evento != 0 && c.evento != SIN_EVENTO_HACE_FALTA
    })
}
