//! **P3b4c: EL LOTE A LA PUERTA DE LA 3060** -- lo que la app `proton-x`
//! manda al kernel por `IOMMU_OP_GPU_DIBUJAR`: la RECETA (VRN2) de un lote.
//!
//! capa: puro -- bytes; la llamada al kernel la hace la app
//!
//! ```text
//!    una vez por PSO   los CUERPOS emitidos (el emisor, ABI de registros),
//!                      sus cargas, el input layout y como van los genericos
//!    una vez por PSO   un PEGADO DE PRUEBA aqui, con el mismo pegamento y
//!    y paso            el mismo juez que el kernel: si no va a pasar, se
//!                      dice AQUI, con su porque, y no se llama a nadie
//!    cada lote         los DATOS (el cbuffer, los vertices TAL CUAL y los
//!                      indices), el dibujo (descarte, giro, Z) y el DESTINO:
//!                      el back buffer de la app
//! ```
//!
//! La CPU dirige: traduce una vez, copia bytes y llama. No dibuja.
//!
//! # La Z, coherente
//!
//! La profundidad de la 3060 vive en SU VRAM; la de la casa, en la RAM del
//! recurso D32. Mientras todos los dibujos de un fotograma van por la 3060,
//! la suya es LA profundidad. Si uno cae a la CPU, la de la 3060 deja de
//! tener lo ultimo: el siguiente con Z que no la limpie tambien va a la CPU
//! (y se dice). Lo lleva [`Puerta::z_viva`].

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_gpu_ga10x::destino::Destino;
/// Como guarda el destino cada pixel (la app la nombra por aqui: Q0a1).
pub use bmo_gpu_ga10x::destino::Flotante;
use bmo_gpu_ga10x::pegamento::{Canal, Carga, Elemento, Redondeo};
use bmo_gpu_ga10x::profundidad::Z;
use bmo_gpu_ga10x::texturas::{DeApp, Muestreo, MAX_TEXTURAS};
use bmo_proton_x::textura::{Clase, Como, Direccion, Filtro, Textura};
use bmo_gpu_ga10x::receta::{self, Receta, Taller, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
use bmo_gpu_ga10x::tuberia::{Descarte, Dibujo, DATOS_MAX};
use bmo_proton_x::lote::{ElementoIa, Enlace, Lote, Topologia};

use crate::pso::{cargas, elementos, NoVa};

/// Lo que contesta el kernel (`cubo::empaquetar`): `sano` = la 3060 pago el
/// dibujo entero; `desempaquetar` = `(us, triangulos, etapas, ..)`.
pub use bmo_gpu_ga10x::cubo::{a_pantalla, copia_us, desempaquetar, preparado, raro, sano};
use crate::vivo::{Origen, Recuerdo, Vivos};
use crate::{emitir_con, Abi};

/// Los registros que se le dan al emisor. ** LI2b (12-10): lo mas que deja
/// el pegamento -- `tuberia::MAX_REGISTROS` menos los `RESERVADOS` y los 8
/// del pegamento (a par) --, no los 64 de VERRANO: el censo de Cyberpunk
/// paraba lotes por "mas registros de los que da la tuberia". El emisor usa
/// los que necesita (de abajo arriba) y el kernel pide esos
/// (`tuberia::registros_para`): un programa que cabia en 64 sale igual.
pub const REGISTROS: u32 = bmo_gpu_ga10x::tuberia::MAX_REGISTROS - bmo_gpu_ga10x::sass::juez::RESERVADOS - 8;

/// **La Z por la 3060, en la SOMBRA del kernel** (P3b4c.6b). El metal
/// (28-09 20:31): `gpu verrano bmox12 30 z` dio Xid 69 (error de clase del
/// motor grafico) AL DIBUJAR -- el estado entero paso, los vertices tambien,
/// y el rasterizador rechazo la pareja ZT en bloque + color PITCH, y el canal
/// GR quedo muerto hasta reiniciar. Es del hardware: nouveau apaga la Z si
/// el color es lineal (`nvc0_validate_fb`) y NVK dibuja en una SOMBRA en
/// bloque y la copia (`nvk_rendering_linear`: "Depth and stencil are never
/// linear"). Desde el 29-09 el kernel hace lo mismo: con Z, el color va a su
/// sombra en VRAM (`bmo_gpu_ga10x::sombra`) y el motor de copia la lleva al
/// back buffer de la app. La receta con Z ya puede ir; apagar esto devuelve
/// cada lote con Z a la CPU (un kernel SIN sombra mataria el canal).
pub const Z_EN_LA_SOMBRA: bool = true;

/// **Lo de un PSO para la puerta**: los cuerpos y como se cargan.
#[derive(Debug, Clone)]
pub struct Cuerpos {
    pub vs: Vec<u8>,
    pub ps: Vec<u8>,
    pub registros_vs: u32,
    pub registros_ps: u32,
    pub salidas: u32,
    pub posicion: u32,
    pub filas: u32,
    pub elementos: Vec<Elemento>,
    pub cargas_vs: Vec<Carga>,
    pub cargas_ps: Vec<Carga>,
    pub genericos: Vec<Option<u8>>,
    /// P3b4c.8 T2b: las texturas del de pixel, en el orden de la receta: la
    /// k es la pareja (tN, sM) de su k-esima asa.
    pub texturas: Vec<(u8, u8)>,
    /// 9d: el registro del TERMOMETRO de cada cuerpo (con libreta).
    pub termometro_vs: Option<u8>,
    pub termometro_ps: Option<u8>,
    /// ** E8f: algun cuerpo lee su cbuffer con un LDC (`ConstantesEn`): la
    /// receta le pide al kernel el banco de la app (+88 bit 1).
    pub banco: bool,
}

fn bytes(codigo: &[(u64, u64)]) -> Vec<u8> {
    codigo.iter().flat_map(|&(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect()
}

/// **Los cuerpos de un PSO**: el emisor, con el ABI de registros (y A9,
/// comprobados bit a bit contra la CPU: [`crate::vivo`]).
/// **13-10: la ESCALERA, pedida por la app** (`--3060-escalera`): cada lote
/// va con un semaforo por escalon (estado, vertices, dibujo), para que un
/// lote que la 3060 no paga diga DONDE se paro. Mas lento: para
/// diagnosticar, no para jugar.
static ESCALERA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Pedir (o dejar de pedir) la escalera en los lotes siguientes.
pub fn pedir_escalera(si: bool) {
    ESCALERA.store(si, core::sync::atomic::Ordering::Relaxed);
}

fn escalera() -> bool {
    ESCALERA.load(core::sync::atomic::Ordering::Relaxed)
}

pub fn cuerpos(en: &Enlace, ia: &[ElementoIa]) -> Result<Cuerpos, NoVa> {
    cuerpos_con(en, ia, None).map(|(c, _)| c)
}

/// 9d (06-10): los cuerpos se emiten con la LIBRETA (su termometro, que el
/// pegamento del kernel apunta; `crate::libreta`).
pub const CON_LIBRETA: bool = true;

/// [`cuerpos`] con un [`Recuerdo`] (A9, 06-10): del .bsf de antes si lo hay,
/// o traducidos, comprobados y guardados en el. Dice de donde salieron.
pub fn cuerpos_con(en: &Enlace, ia: &[ElementoIa], recuerdo: Option<&mut dyn Recuerdo>) -> Result<(Cuerpos, Vivos), NoVa> {
    cuerpos_con_libreta(en, ia, recuerdo, CON_LIBRETA)
}

/// [`cuerpos_con`] diciendo si con la libreta. Sin ella, lo de antes de 9d
/// (lo pide la puerta si un PSO con libreta no cabe en el pegado: cinco
/// instrucciones mas por programa, y las del termometro).
pub fn cuerpos_con_libreta(en: &Enlace, ia: &[ElementoIa], recuerdo: Option<&mut dyn Recuerdo>, libreta: bool) -> Result<(Cuerpos, Vivos), NoVa> {
    // E2.3b: el sombreador de geometria corre en la CPU (la 3060, todavia no).
    if en.gs.is_some() {
        return Err(NoVa::Entrada("un sombreador de geometria"));
    }
    // ** LI2a (12-10): el de pixeles que lee SV_Position ya va a la 3060 si
    // lee su x y su y (`Carga::Posicion`); la z y la w, por la CPU todavia.
    if crate::pso::posicion_zw(en) {
        return Err(NoVa::Entrada("SV_Position.z o .w en el de pixeles"));
    }
    // LI2d (12-10): SV_Target0..n, hasta 4 (el G-buffer sin Z).
    objetivos_de(en).map_err(NoVa::Entrada)?;
    // Con libreta, y si su termometro no cabe en los registros, sin ella.
    let emitir = |p: &bmo_proton_x::dxil::programa::Programa, que: &'static str| {
        let sin = || emitir_con(p, REGISTROS, Abi::Registros).map_err(|e| NoVa::Emisor(que, e));
        if libreta {
            crate::emitir_libreta(p, REGISTROS, Abi::Registros, true).or_else(|_| sin())
        } else {
            sin()
        }
    };
    let vivos = crate::vivo::cuerpos_vivos(en, recuerdo, emitir)?;
    let (ev, ep) = (&vivos.vs, &vivos.ps);
    if ev.precargas.iter().any(|q| matches!(q, crate::Precarga::Asa { .. } | crate::Precarga::AsaPar { .. })) {
        return Err(NoVa::Emisor("vertice", crate::NoEmite::Operacion(0)));
    }
    let posicion = en.posicion as u32;
    let genericos = en.desde_vs.iter().map(|o| o.and_then(|o| bmo_gpu_ga10x::pegamento::generico(o as u32, posicion))).collect();
    let c = Cuerpos {
        vs: bytes(&ev.codigo),
        ps: bytes(&ep.codigo),
        registros_vs: ev.registros,
        registros_ps: ep.registros,
        salidas: en.vs.salidas as u32,
        posicion,
        filas: en.vs.filas_cb.max(en.ps.filas_cb) as u32,
        elementos: elementos(en, ia)?,
        cargas_vs: cargas(ev),
        cargas_ps: crate::pso::cargas_con_posicion(ep, en.pos_ps),
        genericos,
        texturas: crate::pso::texturas_de(ep),
        termometro_vs: ev.termometro,
        termometro_ps: ep.termometro,
        banco: ev.usa_banco() || ep.usa_banco(),
    };
    Ok((c, vivos))
}

/// El modo de la casa con su numero de D3D12.
const fn d3d(d: Direccion) -> u32 {
    match d {
        Direccion::Repetir => 1,
        Direccion::Espejo => 2,
        Direccion::Sujetar => 3,
        Direccion::Borde => 4,
        Direccion::EspejoUnaVez => 5,
    }
}

/// Lo que se sabe del destino (el render target de la casa).
#[derive(Clone, Copy, Debug)]
pub struct Blanco {
    /// La VA de sus pixeles en el proceso (tiene que empezar en pagina).
    pub va: u64,
    pub ancho: u32,
    pub alto: u32,
    /// `B8G8R8A8` (si no, `R8G8B8A8`).
    pub bgra: bool,
    /// P3b4c.9 Z1: es un back buffer de la cadena de intercambio (lo que se
    /// muestra en `Present`): el kernel puede ponerlo directo en la pantalla
    /// si el escritorio se la dio a esta app.
    pub cadena: bool,
    /// ** LI2d-HDR (12-10): un render target de FLOAT -- su formato DXGI
    /// (`trama::Destino::flotante`) y cuantas palabras guarda la casa por
    /// texel (4, o 1 en un R32_FLOAT) --, o `None`: 8 bits por canal.
    pub flotante: Option<(u32, u32)>,
    /// ** LI2d (12-10): los render targets 1..7 del lote (`trama::Otro`: el
    /// `k` es el SV_Target `k + 1`), de la medida del 0; `None`, un RTV nulo
    /// (o ninguno).
    pub mas: MasDelLote,
}

use bmo_gpu_ga10x::destino::OBJETIVOS;

/// **Un render target 1.. del lote** (LI2d): donde estan sus pixeles en la
/// app, su orden de bytes y, si es de float, como `Blanco::flotante`.
#[derive(Clone, Copy, Debug)]
pub struct Mas {
    pub va: u64,
    pub bgra: bool,
    pub flotante: Option<(u32, u32)>,
}

/// Los render targets 1..7 de un lote (los SV_Target 1..7 de D3D12).
pub type MasDelLote = [Option<Mas>; bmo_proton_x::trama::OBJETIVOS - 1];

/// Un lote de UN render target.
pub const SIN_MAS: MasDelLote = [None; bmo_proton_x::trama::OBJETIVOS - 1];

/// **Los render targets que escribe el de pixel** (LI2d): sus SV_Target, el
/// 0 el primero y los demas en orden (`gbuffer.hlsl` escribe 0, 1 y 3: el 2
/// se salta). La salida `k` (el emisor la deja en R4k) va a la RANURA `k` de
/// la 3060 (`destino::va_de`), con la memoria del SV_Target que escribe: en
/// la 3060 los huecos de D3D no existen. Hasta `destino::OBJETIVOS`.
pub fn objetivos_de(en: &bmo_proton_x::lote::Enlace) -> Result<usize, &'static str> {
    let n = en.objetivos.len();
    if en.objetivos.iter().any(|&o| o as usize >= bmo_proton_x::trama::OBJETIVOS) {
        return Err("SV_Depth o SV_StencilRef en el de pixel: la 3060 no los escribe todavia");
    }
    if en.objetivos.first() != Some(&0) || en.objetivos.windows(2).any(|w| w[0] >= w[1]) {
        return Err("un de pixel que no escribe el SV_Target0, o los escribe desordenados: la 3060 los lleva el 0 primero y en orden");
    }
    if n > OBJETIVOS {
        return Err("mas de 4 render targets: la 3060 tiene 4 ranuras de destino");
    }
    Ok(n)
}

/// Un render target de float de un formato que `formato_ia` no sabe leer (el
/// censo le pone su numero DXGI).
pub const FLOTANTE_DESCONOCIDO: &str = "un render target de float de un formato que la casa no conoce";

/// Uno que el juego ve con OTRO formato que el guardado (D2.7: un SNORM8
/// sobre un TYPELESS, el truco de los posprocesos): la CPU pasa los floats a
/// los bytes de la vista y de vuelta; la 3060 todavia no.
pub const FLOTANTE_OTRA_VISTA: &str = "un render target de float que el juego ve con OTRO formato (una vista sobre un TYPELESS): la CPU lo traduce, la 3060 todavia no";

/// **13-10: las parejas `(guardado, vista)` que pararon esa puerta**, para el
/// censo: la conversion que la 3060 tendria que hacer es de BITS (pasar lo
/// que escribe el de pixel en el formato de la vista a los bytes del
/// guardado), y es una por pareja. Antes de escribirla, saber cuales pide el
/// juego. Hasta [`MAX_VISTAS`] distintas; 0 = libre.
pub const MAX_VISTAS: usize = 8;
static VISTAS: [core::sync::atomic::AtomicU32; MAX_VISTAS] = [const { core::sync::atomic::AtomicU32::new(0) }; MAX_VISTAS];

fn apuntar_vista(guardado: u32, vista: u32) {
    use core::sync::atomic::Ordering;
    let x = (guardado & 0xFFFF) | (vista & 0xFFFF) << 16;
    for v in &VISTAS {
        match v.compare_exchange(0, x, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(y) if y == x => return,
            Err(_) => {}
        }
    }
}

/// Las parejas `(guardado, vista)` en DXGI vistas hasta ahora.
pub fn otras_vistas() -> impl Iterator<Item = (u32, u32)> {
    VISTAS.iter().map(|v| v.load(core::sync::atomic::Ordering::Acquire)).filter(|&x| x != 0).map(|x| (x & 0xFFFF, x >> 16))
}

/// **Un render target de float, como lo escribe la 3060** (LI2d-HDR, 12-10):
/// en floats de 32 (`destino::Flotante`), con la salida redondeada canal a
/// canal a lo que la CPU guarda (`formato_ia::cuantizar`: de 32 bits igual,
/// de 16 al half, el que no trae a 0 y el alfa que no trae a 1). Lo que la
/// 3060 no sabe redondear todavia (11 y 10 bits, UNORM, SNORM), su porque.
pub fn de_flotante(formato: u32, palabras: u32) -> Result<(Flotante, Redondeo), &'static str> {
    use bmo_proton_x::formato_ia::{forma, Clase};
    // ** Metal 12-10: la casa guarda un render target de float como CUATRO
    // floats con su marca (`bufer::CUATRO_FLOATS`) y, si se ve con otro
    // formato, el de la vista arriba (`bufer::con_vista`). Sin desempaquetar,
    // el censo decia "DXGI 32571392": era la marca, no un formato.
    let formato = if formato & bmo_proton_x::bufer::CUATRO_FLOATS != 0 {
        match bmo_proton_x::bufer::guardado_y_vista(formato) {
            (g, None) => g,
            (g, Some(v)) => {
                apuntar_vista(g, v);
                return Err(FLOTANTE_OTRA_VISTA);
            }
        }
    } else {
        formato
    };
    let Some(f) = forma(formato) else { return Err(FLOTANTE_DESCONOCIDO) };
    if f.clase != Clase::Float {
        return Err("un render target de UNORM o SNORM de mas de 8 bits (R10G10B10A2, RGBA16_UNORM...): la 3060 redondea floats de 32 y 16 todavia");
    }
    match palabras {
        1 if f.bits == [32, 0, 0, 0] => Ok((Flotante::R32, Redondeo::NADA)),
        4 => {
            let mut r = [Canal::Igual; 4];
            for (c, x) in r.iter_mut().enumerate() {
                *x = match f.bits[c] {
                    32 => Canal::Igual,
                    16 => Canal::Half,
                    0 if c == 3 => Canal::Uno,
                    0 => Canal::Cero,
                    _ => return Err("un render target de float de 11 o 10 bits (R11G11B10_FLOAT): la 3060 redondea a 32 y 16 todavia"),
                };
            }
            Ok((Flotante::Rgba32, Redondeo(r)))
        }
        _ => Err("un render target de float que la casa no guarda en 4 palabras por texel (ni es un R32_FLOAT)"),
    }
}

/// Un render target de float con prueba de profundidad: la Z de la 3060 va con
/// la sombra en bloque, y la sombra es de 8 bits por canal todavia.
pub const Z_CON_FLOAT: &str = "un render target de float con la Z: la sombra en bloque de la 3060 es de 8 bits por canal todavia";

/// **La mezcla de un lote, como la lleva la 3060** (12-10): la del render
/// target 0 (la 3060 pinta UNO) y el factor de `OMSetBlendFactor`; o por que
/// no la sabe (MIN, MAX, `SRC_ALPHA_SAT`, dos fuentes).
pub fn mezcla_de(m: &bmo_proton_x::mezcla::Mezclas) -> Result<bmo_gpu_ga10x::mezcla::Mezcla, &'static str> {
    let r = m.rt[0];
    let x = bmo_gpu_ga10x::mezcla::Mezcla { encendida: r.encendida, origen: r.origen, destino: r.destino, op: r.op, origen_a: r.origen_a, destino_a: r.destino_a, op_a: r.op_a, mascara: r.mascara & 0xF, factor: m.factor.map(f32::to_bits) };
    x.valida()?;
    Ok(x)
}

/// La Z con mezcla: el color iria a la sombra en bloque, no al destino.
pub const MEZCLA_CON_Z: &str = "la mezcla con la Z: la 3060 la hace leyendo el destino de la app, y con Z el color va a la sombra en bloque";
pub const MEZCLA_CON_LIMPIEZA: &str = "la mezcla con la limpieza del render target en el mismo lote";

/// **Si la mezcla de este lote va a la 3060**, o por que no (lo mira la
/// puerta y lo cuenta el censo).
pub fn mezcla_puede(l: &Lote) -> Result<(), &'static str> {
    let m = mezcla_de(&l.reglas.mezcla)?;
    if m.trivial() {
        return Ok(());
    }
    if l.reglas.profundidad.is_some() {
        return Err(MEZCLA_CON_Z);
    }
    if l.limpiar_rt.is_some() {
        return Err(MEZCLA_CON_LIMPIEZA);
    }
    crate::solape::sin_solape(l)
}

/// **La medida de VERRANO** (1280x720): la de la Z y la sombra en bloque de
/// la 3060 (`Destino::es_la_de_verrano`) y la de la pantalla directa (Z1).
/// PROTON-X, con `--3060`, se la da al juego como su pantalla (LI2j, 12-10):
/// el juego elige de lo que ve. Sin Z, la puerta dibuja OTRAS medidas
/// ([`medida`], Q0a1).
pub const MEDIDA: (u32, u32) = (bmo_gpu_ga10x::cubo::ANCHO, bmo_gpu_ga10x::cubo::ALTO);

/// **Por que un destino no es de la medida que la puerta dibuja** (Q0a1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoMedida {
    /// Sus filas (la casa las guarda juntas: `bytes * ancho`) no van de 128
    /// en 128 B, lo que escribe el destino de color de la 3060.
    Filas,
    /// No cabe en el mapa del destino (`destino::MAX_BYTES`), o un lado pasa
    /// de `destino::MAX_LADO`.
    Grande,
    /// Sus pixeles no empiezan en pagina (la IOMMU presta paginas).
    Pagina,
    /// Con la Z: la Z y la sombra en bloque miden 1280x720.
    ConZ,
}

impl NoMedida {
    /// Para el censo (fijo: agrupa lotes).
    pub const fn motivo(self) -> &'static str {
        match self {
            NoMedida::Filas => "un render target cuyas filas no van de 128 en 128 B (la casa las guarda juntas; la 3060 escribe filas de 128 B)",
            NoMedida::Grande => "un render target que no cabe en el mapa del destino de la 3060 (16 MiB, 4096 por lado)",
            NoMedida::Pagina => "un render target que no empieza a 128 B (el color de la 3060 empieza a 128 B; Q0a1b)",
            NoMedida::ConZ => "un render target de otra medida que 1280x720 con la Z (la Z y la sombra de la 3060 miden 1280x720)",
        }
    }
}

/// **El destino de un blanco, si la puerta sabe dibujarlo** (Q0a1, 12-10).
/// Hasta hoy, solo 1280x720; el driver dibuja CUALQUIER medida que quepa
/// (`Destino::valido`: el viewport y el recorte salen de ella, el banco lo
/// probo con 640x360 igual a la CPU) y el metal del 12-10 dijo que lo que
/// paraba a Cyberpunk eran sus render targets propios (256x1, 256x256).
/// Con la Z, sigue en 1280x720 (`tuberia::cabe`).
pub fn medida(b: &Blanco, flotante: Flotante, con_z: bool) -> Result<Destino, NoMedida> {
    use bmo_gpu_ga10x::destino::{MAX_BYTES, MAX_LADO};
    let fila = flotante.bytes() as u64 * b.ancho as u64;
    if b.ancho == 0 || b.alto == 0 || b.ancho > MAX_LADO || b.alto > MAX_LADO || fila * b.alto as u64 > MAX_BYTES {
        return Err(NoMedida::Grande);
    }
    let dst = Destino { fila: fila as u32, ancho: b.ancho, alto: b.alto, flotante, rgb: !b.bgra };
    if dst.fila % 128 != 0 {
        return Err(NoMedida::Filas);
    }
    // Q0a1b (12-10): a mitad de pagina vale, si empieza a 128 B; lo que se
    // mete en la pagina cuenta en el mapa.
    if b.va % bmo_gpu_ga10x::destino::ALINEA != 0 {
        return Err(NoMedida::Pagina);
    }
    if b.va % 4096 + dst.bytes() > MAX_BYTES {
        return Err(NoMedida::Grande);
    }
    if con_z && !dst.es_la_de_verrano() {
        return Err(NoMedida::ConZ);
    }
    debug_assert!(dst.valido_en(b.va));
    Ok(dst)
}

/// **Escribir la receta de un lote** en `caja` (con `datos` de apoyo).
/// `Err` = por que este lote NO va a la 3060 (va a la CPU, y se dice).
/// **LA TEXTURA NULA** (12-10, el censo de Cyberpunk: "t0 sin atar"): lo
/// que lee la 3060 donde el lote no ata textura o muestreador. La CPU da
/// CEROS ahi (`Recursos::muestrear`, el SRV nulo de D3D12); la 3060 lee 8x1
/// texeles a cero (una fila de 32 B, lo que pide su TIC) con un muestreador
/// PROPIO de punto y repetir -- nunca el del lote: su color de borde no daria
/// ceros --. `Sample`, `SampleLevel` y `Load` dan (0, 0, 0, 0) en todo punto.
/// Vive en el monton de la app (la memoria que el kernel sabe prestar).
#[repr(C, align(32))]
pub struct Nula([u32; 8]);

impl Nula {
    pub fn nueva() -> Box<Nula> {
        Box::new(Nula([0; 8]))
    }

    pub fn texeles(&self) -> &[u32; 8] {
        &self.0
    }

    /// Como viaja en la receta.
    pub fn de_app(&self) -> DeApp {
        DeApp { va: self.0.as_ptr() as u64, ancho: 8, alto: 1, fila: 32, bgra: false, muestreo: Muestreo { lineal: false, u: 1, v: 1, borde: [0.0; 4] } }
    }
}

pub fn escribir(c: &Cuerpos, l: &Lote, b: Blanco, limpiar_z: Option<u32>, nula: &Nula, datos: &mut Vec<u8>, caja: &mut [u8]) -> Result<usize, String> {
    // ** 12-10: la TIRA, como LISTA -- los triangulos de la CPU
    // (`lote::triangulos`: el impar se da la vuelta, para que su cara sea la
    // misma), con los mismos ids --: la 3060 dibuja listas.
    let lista: Vec<u32>;
    let ids: &[u32] = match l.topologia {
        Topologia::Lista => l.ids,
        Topologia::Tira => {
            lista = bmo_proton_x::lote::triangulos(l.ids, Topologia::Tira).concat();
            &lista
        }
        _ => return Err(String::from("puntos, lineas o adyacencia: la 3060 dibuja triangulos")),
    };
    let n = ids.len();
    if n == 0 || n % 3 != 0 {
        return Err(format!("{n} indices: no son triangulos enteros"));
    }
    let (flotante, redondeo) = match b.flotante {
        None => (Flotante::No, Redondeo::NADA),
        Some((formato, palabras)) => de_flotante(formato, palabras).map_err(String::from)?,
    };
    if flotante != Flotante::No {
        // LI2d-HDR: lo que un destino de float NO lleva todavia
        // (`tuberia::cabe`).
        if l.reglas.profundidad.is_some() {
            return Err(String::from(Z_CON_FLOAT));
        }
        if l.limpiar_rt.is_some() || b.cadena {
            return Err(String::from("un render target de float que se limpia en el lote o se muestra: la 3060 lo dibuja sin limpiar y fuera de la pantalla todavia"));
        }
    }
    // ** Q0a1 (12-10): cualquier medida que el destino de la 3060 sepa.
    let dst = medida(&b, flotante, l.reglas.profundidad.is_some()).map_err(|m| format!("el render target de {}x{} (VA {:#x}): {}", b.ancho, b.alto, b.va, m.motivo()))?;
    // ** LI2d (12-10): los render targets 1.. -- cada uno de la medida del
    // 0, con su VA, su orden de bytes y su float; un float que pide
    // redondeo (half) no: el pegamento redondea solo el 0. Y sin mezcla ni
    // mascara (la del sombreador es la del 0). Sin Z: la sombra es una.
    let objetivos = objetivos_de(l.enlace).map_err(String::from)?;
    let mut mas = [(0u64, bmo_gpu_ga10x::destino::Destino::default()); OBJETIVOS - 1];
    for k in 1..objetivos {
        // La salida k escribe el SV_Target `s`: su memoria va a la ranura k.
        let s = l.enlace.objetivos[k] as usize;
        let Some(m) = b.mas[s - 1] else {
            return Err(format!("el render target {s} es un RTV nulo (lo que escribe se pierde): la 3060 escribe en todos"));
        };
        if l.reglas.profundidad.is_some() {
            return Err(String::from("varios render targets con la Z: la sombra en bloque de la 3060 es UNA todavia"));
        }
        if l.limpiar_rt.is_some() || b.cadena {
            return Err(String::from("varios render targets con la limpieza en el lote, o en la cadena: la 3060 no lo lleva todavia"));
        }
        if !l.reglas.mezcla.rt[s].trivial() {
            return Err(format!("el render target {s} mezcla o enmascara: la 3060 mezcla solo el 0 todavia"));
        }
        let (f, red) = match m.flotante {
            None => (Flotante::No, Redondeo::NADA),
            Some((formato, palabras)) => de_flotante(formato, palabras).map_err(|x| format!("el render target {s}: {x}"))?,
        };
        if red != Redondeo::NADA {
            return Err(format!("el render target {s} es de un float que se redondea (half, R11G11B10): el pegamento redondea solo el 0 todavia"));
        }
        let bk = Blanco { va: m.va, bgra: m.bgra, cadena: false, flotante: m.flotante, mas: SIN_MAS, ..b };
        let dk = medida(&bk, f, false).map_err(|x| format!("el render target {s} (VA {:#x}): {}", m.va, x.motivo()))?;
        mas[k - 1] = (m.va, dk);
    }
    let r = &l.reglas;
    if r.viewport != [0.0, 0.0, b.ancho as f32, b.alto as f32, 0.0, 1.0] {
        return Err(format!("un viewport que no es el back buffer entero ({:?})", r.viewport));
    }
    if r.tijera[0] > 0 || r.tijera[1] > 0 || r.tijera[2] < b.ancho as i32 || r.tijera[3] < b.alto as i32 {
        return Err(format!("una tijera que corta ({:?}): la 3060 no la pone todavia", r.tijera));
    }
    let descarte = match r.descarte {
        1 => Descarte::Ninguna,
        2 => Descarte::Delanteras,
        3 => Descarte::Traseras,
        // N5.16b: la receta de la 3060 recorta siempre en z.
        d if d & bmo_proton_x::trama::SIN_RECORTE_Z != 0 => return Err(String::from("DepthClipEnable = FALSE: la receta de la 3060 recorta en z siempre (no lo apaga todavia)")),
        d => return Err(format!("un modo de descarte que no es de D3D12 ({d})")),
    };
    let z = r.profundidad.map(|p| Z { funcion: p.funcion, escribir: p.escribir, limpiar: limpiar_z });
    if z.is_some_and(|z| !z.valida()) {
        return Err(String::from("una regla de profundidad fuera de D3D12"));
    }
    // Los DATOS: el cbuffer (a ceros lo que no traiga), los vertices que se
    // piden TAL CUAL (hasta el mayor indice) y detras los indices, en u32.
    // LI2e (12-10): la 3060 lee el vertice `vid` y su SV_VertexID ES `vid`.
    // Por eso los indices van SIN el vertice base y los vertices se copian
    // DESDE el base: la direccion de cada vertice no cambia, y el `vid` es el
    // SV_VertexID de la CPU (`lote::entrada`: el id menos el base). Sin paso,
    // solo valores de sistema (un cuadro de pantalla completa).
    let paso = l.paso;
    let de_memoria = c.elementos.iter().any(|e| e.sistema == bmo_gpu_ga10x::pegamento::Sistema::No);
    if paso % 4 != 0 || (paso == 0 && de_memoria) {
        return Err(format!("un paso de vertice de {paso} bytes"));
    }
    let base = l.base_vertice as usize;
    if ids.iter().any(|&id| (id as usize) < base) {
        return Err(format!("un indice por debajo del vertice base ({base}): un BaseVertexLocation negativo"));
    }
    let mayor = *ids.iter().max().unwrap_or(&(base as u32)) as usize;
    if paso != 0 {
        let hay = l.vertices.len() / paso;
        if mayor >= hay {
            return Err(format!("el indice {mayor} pasa de los {hay} vertices del bufer"));
        }
    }
    let cb = 16 * c.filas as usize;
    let vertices = mayor + 1 - base;
    let desde = (cb + vertices * paso).next_multiple_of(4);
    let total = (desde + 4 * n).next_multiple_of(16);
    if total > DATOS_MAX {
        return Err(format!("{total} bytes de DATOS: no caben en los {DATOS_MAX} de la 3060"));
    }
    datos.clear();
    datos.resize(total, 0);
    let k = l.cb.len().min(cb);
    datos[..k].copy_from_slice(&l.cb[..k]);
    datos[cb..cb + vertices * paso].copy_from_slice(&l.vertices[base * paso..(base + vertices) * paso]);
    for (i, &id) in ids.iter().enumerate() {
        datos[desde + 4 * i..desde + 4 * i + 4].copy_from_slice(&(id - base as u32).to_le_bytes());
    }
    if c.elementos.len() > MAX_ELEMENTOS || c.cargas_vs.len() > MAX_CARGAS || c.cargas_ps.len() > MAX_CARGAS || c.genericos.len() > MAX_GENERICOS {
        return Err(String::from("el PSO trae mas entradas o cargas de las que caben en una receta"));
    }
    // P3b4c.8 T2b: cada textura del de pixel -- la imagen tN con el
    // muestreador sM de este lote --, tal como esta en la RAM de la app: el
    // kernel la presta SOLO LECTURA mientras dibuja.
    if c.texturas.len() > MAX_TEXTURAS {
        return Err(format!("{} texturas: la receta lleva {MAX_TEXTURAS}", c.texturas.len()));
    }
    let mut texturas = [DeApp::NINGUNA; MAX_TEXTURAS];
    for (k, &(tn, sm)) in c.texturas.iter().enumerate() {
        // Sin TEXTURA atada: ceros, como la CPU (con `Sample` y con `Load`).
        // Con textura y sin muestreador, no: `Load` la leeria (no usa
        // muestreador) y aqui no se sabe cual de las dos es esta pareja.
        let Some(Some(t)) = l.recursos.texturas.get(tn as usize) else {
            texturas[k] = nula.de_app();
            continue;
        };
        let Some(Some(m)) = l.recursos.muestreadores.get(sm as usize) else {
            return Err(format!("el sombreador muestrea t{tn} con s{sm} y el lote no tiene s{sm} atado"));
        };
        // 02-10: la 3060 lee aqui RGBA/BGRA de 8 bits, lineales y con el
        // mapeo de siempre; un BC, un float, un sRGB o un mapeo, en la CPU
        // (la casa los decodifica con SUS tablas: la 3060, con las suyas, no
        // se promete igual hasta medirlo -- DL10 --).
        if !matches!(t.como, Como::Rgba8 | Como::Bgra8) || t.srgb || t.mapeo & 0xFFF != Textura::MAPEO & 0xFFF || t.clase != Clase::Plana {
            return Err(format!("la textura t{tn} es BC, float, sRGB, con otro mapeo o no es una 2D: la 3060 todavia lee solo RGBA/BGRA de 8 bits"));
        }
        // ** 12-10: la vista de OTRA mip de una 2D (el censo de Cyberpunk):
        // el plano que ella mira, como lo lee la CPU (`plano_visto`), es una
        // 2D mas -- su direccion y sus medidas --; con una sola mip en la
        // vista (eso ya lo mira `mip_importa`, abajo).
        let Some((plano, ancho, alto)) = t.plano_visto() else {
            return Err(format!("la textura t{tn}: su mip {} no esta en la memoria que dio la casa", t.mip));
        };
        let muestreo = Muestreo { lineal: m.filtro == Filtro::Lineal, u: d3d(m.u), v: d3d(m.v), borde: m.borde };
        let d = DeApp { va: plano.as_ptr() as u64, ancho, alto, fila: 4 * ancho, bgra: t.bgra(), muestreo };
        if plano.len() < (ancho * alto) as usize || !d.valida() {
            return Err(format!(
                "la textura t{tn} ({ancho}x{alto}, VA {:#x}): la 3060 la lee con filas de 32 B, desde 32 B y en {} MiB; esta no",
                d.va,
                bmo_gpu_ga10x::texturas::RANURA >> 20
            ));
        }
        texturas[k] = d;
    }
    let mut rec = Receta {
        n,
        vs: &c.vs,
        ps: &c.ps,
        registros_vs: c.registros_vs,
        registros_ps: c.registros_ps,
        salidas: c.salidas,
        posicion: c.posicion,
        filas: c.filas,
        paso: paso as u32,
        elementos: [NINGUNO; MAX_ELEMENTOS],
        n_elementos: c.elementos.len(),
        cargas_vs: [NINGUNA; MAX_CARGAS],
        n_cargas_vs: c.cargas_vs.len(),
        cargas_ps: [NINGUNA; MAX_CARGAS],
        n_cargas_ps: c.cargas_ps.len(),
        genericos: [None; MAX_GENERICOS],
        n_genericos: c.genericos.len(),
        datos,
        dibujo: Dibujo { indices: Some(desde as u32), vertices: vertices as u32, descarte, antihorario: r.antihorario, destino: Some((b.va, dst)), z, color: l.limpiar_rt, texturas: c.texturas.len() as u8, cadena: b.cadena, escalera: escalera(), pantalla: false, banco: if c.banco { 16 * c.filas } else { 0 }, redondeo, mezcla: mezcla_de(&l.reglas.mezcla).map_err(String::from)?, registros: 0, mas, n_mas: objetivos as u8 - 1 },
        texturas,
        termometro_vs: c.termometro_vs,
        termometro_ps: c.termometro_ps,
    };
    rec.elementos[..c.elementos.len()].copy_from_slice(&c.elementos);
    rec.cargas_vs[..c.cargas_vs.len()].copy_from_slice(&c.cargas_vs);
    rec.cargas_ps[..c.cargas_ps.len()].copy_from_slice(&c.cargas_ps);
    rec.genericos[..c.genericos.len()].copy_from_slice(&c.genericos);
    receta::escribir(caja, &rec).ok_or_else(|| String::from("la receta no se sostiene (lo que lee el pegamento no cae en los DATOS)"))
}

/// A9c: de cada cuantos lotes de un PSO revisa uno el vigia (el primero,
/// siempre). Revisar son unos pocos vertices en el simulador: en 256 lotes,
/// menos que nada.
pub const VIGIA_CADA: u64 = 256;

/// **La puerta de la app**: los cuerpos por PSO, lo probado por PSO y paso,
/// la caja de la receta y la Z.
pub struct Puerta {
    /// Por PSO: sus cuerpos y lo vivo (A9c: lo que revisa el vigia), y
    /// cuantos lotes lleva.
    cuerpos: Vec<(usize, Result<(Cuerpos, Vivos), NoVa>, u64)>,
    probados: Vec<(usize, usize, Result<(), String>)>,
    /// La caja de la receta: en el MONTON de la app (un bloque suyo), que es
    /// lo que el kernel exige.
    pub caja: Vec<u8>,
    datos: Vec<u8>,
    taller: Box<Taller>,
    /// La textura nula ([`Nula`]): lo que lee la 3060 donde no hay nada atado.
    nula: Box<Nula>,
    /// La Z de la 3060 tiene lo ultimo (ver la cabecera).
    pub z_viva: bool,
    /// Mandar lotes con Z a la 3060 ([`Z_EN_LA_SOMBRA`]): el kernel los
    /// dibuja en su sombra en bloque y los copia al back buffer.
    pub z_a_la_3060: bool,
    /// A9 (06-10): donde se recuerdan los .bsf de cada PSO (en BMO-X,
    /// ESTRATOS); sin el, se traduce cada arranque.
    pub recuerdo: Option<Box<dyn Recuerdo + Send>>,
    /// A9: los PSO traducidos (y comprobados) y los que salieron del
    /// recuerdo, en la vida de esta puerta.
    pub traducidos: usize,
    pub recordados: usize,
    /// A9c (06-10): EL VIGIA, el modo dinamico -- el PRIMER lote de cada PSO
    /// y luego uno de cada `vigia_cada` se revisan con los datos del juego
    /// ([`crate::vivo::revisar`]); uno que no cuadra se dibuja por la CPU
    /// desde ese lote, y se marca malo en el recuerdo.
    pub vigia_cada: u64,
    pub revisados: usize,
    pub corregidos: usize,
    /// 9d (06-10): los dibujos en los que la LIBRETA de la 3060 apunto algo
    /// raro ([`Puerta::apunto`]), en la vida de esta puerta; y los PSO que no
    /// cabian con ella en el pegado y van sin libreta.
    pub apuntados: usize,
    pub sin_libreta: usize,
}

impl Default for Puerta {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Puerta {
    pub fn nueva() -> Self {
        Puerta { cuerpos: Vec::new(), probados: Vec::new(), caja: alloc::vec![0; receta::MAX_RECETA], datos: Vec::new(), taller: Box::new(Taller::nuevo()), nula: Nula::nueva(), z_viva: false, z_a_la_3060: Z_EN_LA_SOMBRA, recuerdo: None, traducidos: 0, recordados: 0, vigia_cada: VIGIA_CADA, revisados: 0, corregidos: 0, apuntados: 0, sin_libreta: 0 }
    }

    /// **La receta de este lote**, en `self.caja[..n]`: `Ok(n)`, o por que
    /// no va a la 3060.
    pub fn preparar(&mut self, l: &Lote, b: Blanco) -> Result<usize, String> {
        // LI2d-HDR: un render target de float con Z, ANTES que las cuentas de
        // la Z de la 3060 (que dirian otro porque).
        if b.flotante.is_some() && l.reglas.profundidad.is_some() {
            return Err(String::from(Z_CON_FLOAT));
        }
        // E2.7: con una consulta de oclusion abierta hay que CONTAR los
        // pixeles que pasan, y la 3060 aun no los cuenta: por la CPU.
        if l.oclusion {
            return Err(String::from("hay una consulta de oclusion abierta: la 3060 no cuenta los pixeles que pasan todavia"));
        }
        // 05-10: lo que un dibujo escribe en sus UAV queda en la memoria de la
        // CPU; la receta no lleva UAV (ni la 3060 emite sus operaciones).
        if l.uavs.is_some() {
            return Err(String::from("sus sombreadores leen o escriben UAV (RWTexture, RWBuffer, Interlocked): la 3060 no los lleva todavia"));
        }
        // E2.5 (05-10): las olas (Wave*, Quad*) tampoco: el emisor no las
        // sabe (`vote`, `shfl`); dicho con su nombre, no con un numero.
        if l.enlace.vs.olas_propias() || l.enlace.ps.olas_propias() {
            return Err(String::from("sus sombreadores usan las olas (Wave*, Quad*): la 3060 no las lleva todavia"));
        }
        // D4.4 (05-10): las derivadas (ddx, ddy, fwidth) tampoco; y la mip de
        // un `Sample` la saca su TEX, pero de la mip 0 de una textura de una
        // mip: si la vista tiene mas, o el muestreador filtra distinto de
        // lejos, la CPU (que elige la mip) daria otra cosa.
        if l.enlace.vs.deriva() || l.enlace.ps.deriva() {
            return Err(String::from("sus sombreadores usan las derivadas (ddx, ddy, fwidth): la 3060 no las lleva todavia"));
        }
        if l.enlace.ps.mip_por_derivadas() && l.recursos.mip_importa() {
            return Err(String::from("muestrea con la mip de sus derivadas una textura de varias mips (o con MIN y MAG distintos): la 3060 lee la mip 0 todavia"));
        }
        // ** E8g (10-10): SampleLevel y Load leen la mip que les dicen, y la
        // 3060 tiene en su TIC (PITCH) la mip 0 de cada textura: con una
        // vista de varias mips, o un muestreador que filtra distinto de lejos,
        // daria otra cosa. Hasta el TIC de bloques (Ring 0), por la CPU.
        if l.enlace.ps.lee_con_nivel() && l.recursos.mip_importa() {
            return Err(String::from("lee con SampleLevel o Load una textura de varias mips (o con MIN y MAG distintos): la 3060 tiene la mip 0 todavia"));
        }
        // N5.13: la receta lleva UNA instancia y los elementos por vertice.
        if l.instancias != 1 || l.entradas.iter().any(|e| e.por_instancia.is_some()) {
            return Err(String::from("el lote dibuja varias instancias o lee datos por instancia: la 3060 no lo sabe todavia"));
        }
        // N5.11 / 12-10: la mezcla del render target 0, en el sombreador
        // (`bmo_gpu_ga10x::mezcla`), con los bits de la CPU: si la 3060 la
        // sabe, sin la Z, sin limpieza en el lote y sin solape (`solape`).
        if let Err(m) = mezcla_puede(l) {
            return Err(String::from(m));
        }
        // 05-10: el stencil vive en la RAM de la casa (un byte por texel del
        // DSV) y la receta no lo lleva: ese lote, por la CPU (y `despues`
        // da por muerta la Z de la 3060 si el lote tambien la usa).
        if l.reglas.stencil.is_some() {
            return Err(String::from("el lote usa STENCIL: la 3060 no lo prueba ni lo escribe todavia (su plano vive en la RAM de la casa)"));
        }
        if l.reglas.profundidad.is_some() && !self.z_a_la_3060 {
            return Err(String::from(
                "el lote usa Z y la 3060 no dibuja con Z sobre un color PITCH (el back buffer en tu RAM): sin la sombra en bloque del kernel fue Xid 69 y el canal GR MUERTO hasta reiniciar",
            ));
        }
        if l.reglas.profundidad.is_some() && l.limpiar_z.is_none() && !self.z_viva {
            return Err(String::from("la Z de la 3060 no tiene lo ultimo (un dibujo anterior fue por la CPU) y este no la limpia"));
        }
        let clave = l.enlace as *const Enlace as usize;
        let i = match self.cuerpos.iter().position(|x| x.0 == clave) {
            Some(i) => i,
            None => {
                // A9: vivos -- del .bsf de antes, o traducidos, comprobados
                // contra la CPU y guardados.
                let r = cuerpos_con(l.enlace, l.entradas, self.recuerdo.as_mut().map(|r| r.as_mut() as &mut dyn Recuerdo));
                if let Ok((_, v)) = &r {
                    match v.origen {
                        Origen::Traducido => self.traducidos += 1,
                        Origen::Recordado => self.recordados += 1,
                    }
                }
                self.cuerpos.push((clave, r, 0));
                self.cuerpos.len() - 1
            }
        };
        // A9c: EL VIGIA -- el primer lote de este PSO, y uno de cada
        // `vigia_cada`, con los datos del juego. Si no cuadra: por la CPU
        // desde YA (este lote no llega mal a la pantalla), marcado malo en el
        // recuerdo (el arranque siguiente ni lo intenta), y se dice.
        let (_, r, lotes) = &mut self.cuerpos[i];
        *lotes += 1;
        if let Ok((c, v)) = r {
            if (*lotes - 1) % self.vigia_cada.max(1) == 0 {
                self.revisados += 1;
                if let Err(m) = crate::vivo::revisar(l.enlace, &c.elementos, &v.vs, &v.ps, l) {
                    if let (Some(rec), Some(nombre)) = (self.recuerdo.as_mut(), &v.nombre) {
                        rec.guardar(&crate::vivo::malo(nombre), m.as_bytes());
                    }
                    self.corregidos += 1;
                    *r = Err(NoVa::Juez("el vigia, con los datos del juego", m));
                }
            }
        }
        let c = match &self.cuerpos[i].1 {
            Ok((c, _)) => c,
            Err(e) => return Err(format!("el PSO no va a la 3060: {e:?}")),
        };
        let mut n = escribir(c, l, b, l.limpiar_z, &self.nula, &mut self.datos, &mut self.caja)?;
        let con_libreta = c.termometro_vs.is_some() || c.termometro_ps.is_some();
        // UNA vez por PSO y paso: el pegado de prueba, como lo hara el kernel.
        if !self.probados.iter().any(|p| p.0 == clave && p.1 == l.paso) {
            let mut prueba = self.pegar_de_prueba(n);
            // 9d: si con la libreta no cabe (cinco instrucciones mas por
            // programa, y las del termometro), el PSO sigue SIN ella: lo de
            // antes, que si cabia. Sin recuerdo: es un caso raro.
            if prueba.is_err() && con_libreta {
                if let Ok((c2, v2)) = cuerpos_con_libreta(l.enlace, l.entradas, None, false) {
                    self.sin_libreta += 1;
                    self.cuerpos[i].1 = Ok((c2, v2));
                    if let Ok((c2, _)) = &self.cuerpos[i].1 {
                        n = escribir(c2, l, b, l.limpiar_z, &self.nula, &mut self.datos, &mut self.caja)?;
                        prueba = self.pegar_de_prueba(n);
                    }
                }
            }
            self.probados.push((clave, l.paso, prueba));
        }
        match &self.probados.iter().find(|p| p.0 == clave && p.1 == l.paso).expect("recien puesto").2 {
            Ok(()) => Ok(n),
            Err(e) => Err(e.clone()),
        }
    }

    /// El pegado de prueba de la receta de `self.caja[..n]`, como el kernel.
    fn pegar_de_prueba(&mut self, n: usize) -> Result<(), String> {
        match receta::leer(&self.caja[..n]) {
            None => Err(String::from("la receta no se relee")),
            Some(r) => receta::pegar(&r, &mut self.taller).map_err(|e| format!("el kernel no la pegaria: {e:?}")),
        }
    }

    /// **LA LIBRETA DIJO RARO en el dibujo de `l`** (9d, 06-10): el kernel lo
    /// dice en el `Ok` de SU receta (`cubo::raro`). El siguiente lote de ese
    /// PSO lo revisa el vigia con los datos del juego YA, sin esperar a su
    /// turno de `vigia_cada`: la 3060 dice donde mirar, y la CPU decide.
    /// `false` si el PSO no es de esta puerta (o ya va por la CPU).
    pub fn apunto(&mut self, l: &Lote) -> bool {
        let clave = l.enlace as *const Enlace as usize;
        match self.cuerpos.iter_mut().find(|x| x.0 == clave) {
            Some((_, Ok(_), lotes)) => {
                *lotes = 0;
                self.apuntados += 1;
                true
            }
            _ => false,
        }
    }

    /// Lo que paso con el lote: `a_la_3060` = lo dibujo la 3060. La Z de la
    /// 3060 queda viva si la uso; muerta si el lote se dibujo en la CPU.
    pub fn despues(&mut self, l: &Lote, a_la_3060: bool) {
        if l.reglas.profundidad.is_some() {
            self.z_viva = a_la_3060;
        }
    }
}
