//! **Los sombreadores NATIVOS: traducidos una vez, sellados y llamados**
//! (P3b3b, 27-09).
//!
//! `bmo_proton_x::nativo` da los bytes (x86-64 con SSE, con las reglas de
//! INTI); aqui se ponen donde se pueden ejecutar y se llaman. El sitio es UN
//! bloque sellado (`MEM_OP_SELLAR`: W^X, de datos a codigo, irreversible):
//!
//! ```text
//!    CreateGraphicsPipelineState   se traducen sus dos sombreadores y se
//!                                  agregan al codigo de TODOS los anteriores
//!    un bloque nuevo               con todo el codigo, sellado; el anterior
//!                                  se suelta (MEM_OP_SOLTAR): vivo, solo uno
//!    Draw                          el ejecutor de la casa ([`dibujar`]) llama
//!                                  al codigo de su PSO; si no hay (no quedo
//!                                  bloque), el interprete, que da lo mismo
//! ```
//!
//! **Por que cambiar de bloque es seguro:** los hilos de la casa son
//! cooperativos y un Draw no cede el turno; mientras se crea un PSO nadie esta
//! dentro de un sombreador. Y el codigo no depende de donde cae (solo usa sus
//! punteros), asi que se copia tal cual.
//!
//! **Los que MUESTREAN** (X2, 05-10): el traducido llama, por las
//! `Llamadas` que se le pasan, a [`textura_sysv`] (aqui: sigue los punteros)
//! y esta al muestreo del interprete (`nativo_llamadas::Muestras`): los
//! mismos bits, y el PSO con texturas ya no se interpreta.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::lote::{self, Enlace, Lote, NoDibuja};
use bmo_proton_x::nativo_llamadas::{Llamadas, Muestras};
use bmo_proton_x::{nativo, trama};

use crate::{aviso, plataforma};

/// La firma de un sombreador de computo traducido (`nativo_computo`): el
/// ABI de System V. Vive aqui y no en `bmo-proton-x`, que es puro.
pub(crate) type FuncionComputo = unsafe extern "sysv64" fn(*mut f32, *mut bmo_proton_x::nativo_computo::Contexto, *const u8) -> u32;

/// `fn(registros, entradas, cbuffer, salidas, llamadas) -> QUEDA o
/// DESCARTADO`: solo punteros, el ABI entero que el Rust soft-float de Ring
/// 3 sabe llamar. Las `Llamadas` (X2) las lee solo el que las usa.
pub type Sombreador = unsafe extern "sysv64" fn(*mut f32, *const [f32; 4], *const u8, *mut [f32; 4], *const Llamadas) -> u32;

/// **La llamada de texturas del codigo traducido** (X2, 05-10): `datos` son
/// unas [`Muestras`] y `regs` los registros de su programa; corre su
/// operacion `k` como el interprete. Publica: el banco la usa tal cual.
///
/// # Safety
/// `datos` apunta a unas `Muestras` vivas y `regs` a tantos registros como
/// `iniciales` tiene su programa, sin otra referencia a ellos mientras dura:
/// lo promete quien pone las `Llamadas` (el traducido solo la llama con los
/// suyos, y no los toca hasta que vuelve).
pub unsafe extern "sysv64" fn textura_sysv(datos: *mut u8, regs: *mut f32, k: u32) {
    // SAFETY: lo de arriba.
    let m = unsafe { &mut *(datos as *mut Muestras) };
    let regs = unsafe { core::slice::from_raw_parts_mut(regs, m.programa.iniciales.len()) };
    m.llamar(regs, k);
}

/// **Las `Llamadas` de un sombreador** con sus `Muestras` (que tienen que
/// vivir lo que ellas: aqui solo se guarda el puntero, el mismo por el que
/// quien llama la toca entre llamada y llamada) y un cbuffer de `cb_bytes`.
pub fn llamadas(m: *mut Muestras, cb_bytes: usize) -> Llamadas {
    let t: unsafe extern "sysv64" fn(*mut u8, *mut f32, u32) = textura_sysv;
    Llamadas { textura: t as usize, datos: m as *mut u8, ..Llamadas::nuevas(cb_bytes) }
}

struct Traducido {
    /// El `Enlace` de su PSO (los PSO no se liberan: su direccion vale).
    enlace: usize,
    vs: usize,
    ps: usize,
}

struct Estado {
    codigo: Vec<u8>,
    traducidos: Vec<Traducido>,
    /// El bloque sellado de ahora: (direccion, medida).
    bloque: Option<(u64, usize)>,
    sin_bloque: bool,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos (ver hilos.rs).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { codigo: Vec::new(), traducidos: Vec::new(), bloque: None, sin_bloque: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    if let Some((b, n)) = e.bloque.take() {
        (plataforma().soltar_codigo)(b, n);
    }
    e.codigo.clear();
    e.traducidos.clear();
    e.sin_bloque = false;
}

/// Agregar `c` al codigo (a 16 bytes, con int3 en medio: entre funciones no
/// se cae en nada); donde empieza.
fn agregar(e: &mut Estado, c: &[u8]) -> usize {
    while e.codigo.len() % 16 != 0 {
        e.codigo.push(0xCC);
    }
    let desde = e.codigo.len();
    e.codigo.extend_from_slice(c);
    desde
}

/// **Traducir los sombreadores de un PSO** y rehacer el bloque sellado.
pub(crate) fn registrar(en: &Enlace) {
    let e = estado();
    // X2 (05-10): los que MUESTREAN ya se traducen (llaman al muestreo del
    // interprete); lo que no, el PSO entero va por el interprete.
    let (Some(cv), Some(cp)) = (nativo::compilar(&en.vs), nativo::compilar(&en.ps)) else {
        // N5.13 (05-10): el motivo de VERDAD.
        match nativo::por_que_no(&en.vs).or_else(|| nativo::por_que_no(&en.ps)) {
            Some(m) => aviso(&alloc::format!("un PSO cuyo sombreador {m}: sus sombreadores se interpretan (el codigo nativo aun no lo sabe)")),
            None => aviso("un PSO cuyos sombreadores el codigo nativo no sabe traducir: se interpretan"),
        }
        return;
    };
    let vs = agregar(e, &cv);
    let ps = agregar(e, &cp);
    e.traducidos.push(Traducido { enlace: en as *const Enlace as usize, vs, ps });
    sellar(e);
}

/// **Traducir el CS de un PSO de computo** (E2.3b, 05-10) y rehacer el
/// bloque sellado: donde empieza en el codigo, o `None` si no se traduce
/// (sus Dispatch van por el interprete, que da lo mismo).
pub(crate) fn registrar_computo(p: &bmo_proton_x::dxil::programa::Programa) -> Option<usize> {
    let c = bmo_proton_x::nativo_computo::compilar(p)?;
    let e = estado();
    let desde = agregar(e, &c);
    sellar(e);
    Some(desde)
}

/// **La funcion de computo** que empieza en `desde`, en el bloque de ahora
/// (`None` si no quedo bloque).
pub(crate) fn computo(desde: usize) -> Option<FuncionComputo> {
    let (base, n) = estado().bloque?;
    // SAFETY: `base + desde` es el principio de una funcion traducida por
    // `nativo_computo::compilar`, dentro del bloque sellado vivo (que mide
    // `n`); su firma es esa.
    (desde < n).then(|| unsafe { core::mem::transmute::<usize, FuncionComputo>(base as usize + desde) })
}

/// Rehacer el bloque sellado con TODO el codigo de ahora.
fn sellar(e: &mut Estado) {
    let p = plataforma();
    match (p.sellar_codigo)(&e.codigo) {
        Some(b) => {
            if let Some((viejo, n)) = e.bloque.replace((b, e.codigo.len())) {
                (p.soltar_codigo)(viejo, n);
            }
        }
        None => {
            if !e.sin_bloque {
                aviso("sin bloque sellado para el codigo nativo: los sombreadores se interpretan (dan lo mismo, mas despacio)");
                e.sin_bloque = true;
            }
        }
    }
}

/// La funcion nativa de `desde` en el bloque de ahora.
fn funcion(base: u64, desde: usize) -> Sombreador {
    // SAFETY: `base + desde` es el principio de una funcion traducida por
    // `nativo::compilar`, dentro del bloque sellado vivo; su firma es esa
    // (llamarla es `unsafe`: sus punteros los pone `dibujar`).
    unsafe { core::mem::transmute::<usize, Sombreador>(base as usize + desde) }
}

/// **El ejecutor de la casa** (`Plataforma::dibujar`): el lote con los
/// sombreadores nativos de su PSO, o interpretados si no los hay.
pub fn dibujar(l: &Lote, destino: &mut trama::Destino) -> Result<trama::Cuenta, NoDibuja> {
    let e = estado();
    let yo = l.enlace as *const Enlace as usize;
    let (Some((base, _)), Some(t)) = (e.bloque, e.traducidos.iter().find(|t| t.enlace == yo)) else {
        return lote::en_cpu(l, destino);
    };
    let en = l.enlace;
    // D4.4: lo traducido corre pixel a pixel, sin el cuadro de 2x2 que da las
    // derivadas: muestrea con gradientes 0 (la mip mas detallada, el filtro
    // de cerca). Solo vale si la mip no cambia nada; si la cambia (una vista
    // de varias mips, o MIN y MAG distintos), por el interprete, en cuadros.
    // Las del indice dinamico (el bindless) no se ven de antemano (cada
    // pixel calcula la suya): esas, siempre por el interprete.
    let mip_por_derivadas = en.ps.mip_por_derivadas() || en.vs.mip_por_derivadas();
    if mip_por_derivadas && l.recursos.mip_importa() {
        aviso("un dibujo muestrea con la mip de sus derivadas una textura de varias mips (o con MIN y MAG distintos): va por el interprete, en cuadros de 2x2 (el codigo nativo no los tiene)");
        return lote::en_cpu(l, destino);
    }
    if mip_por_derivadas && l.recursos.dinamicas.is_some() && (en.ps.elige_texturas() || en.vs.elige_texturas()) {
        aviso("un dibujo muestrea con la mip de sus derivadas texturas del indice dinamico (bindless): va por el interprete, en cuadros de 2x2 (sus mips no se ven de antemano)");
        return lote::en_cpu(l, destino);
    }
    let (fv, fp) = (funcion(base, t.vs), funcion(base, t.ps));
    // El cbuffer, con lo que lean los dos: lo que falte, a cero (como el
    // interprete).
    let filas = en.vs.filas_cb.max(en.ps.filas_cb) as usize * 16;
    let mut relleno = Vec::new();
    let cb: &[u8] = if l.cb.len() >= filas {
        l.cb
    } else {
        relleno.extend_from_slice(l.cb);
        relleno.resize(filas, 0);
        &relleno
    };
    let cbp = if cb.is_empty() { [0u8; 16].as_ptr() } else { cb.as_ptr() };
    // X2 (05-10): a quien llaman (las texturas del dibujo, la matematica) y
    // la medida del cbuffer, una vez por dibujo; la elegida, a `None` en cada
    // vertice o pixel (como el interprete).
    let (mut mv, mut mp) = (Muestras { programa: &en.vs, recursos: &l.recursos, elegida: None }, Muestras { programa: &en.ps, recursos: &l.recursos, elegida: None });
    let (pmv, pmp): (*mut Muestras, *mut Muestras) = (&mut mv, &mut mp);
    let (lv, lp) = (llamadas(pmv, cb.len()), llamadas(pmp, cb.len()));
    let (mut rv, mut rp) = (Vec::new(), Vec::new());
    let mut vs = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rv.clear();
        rv.extend_from_slice(&en.vs.iniciales);
        if ent.len() >= en.vs.entradas && sal.len() >= en.vs.salidas {
            // SAFETY: `fv` es la traduccion de `en.vs`; los registros, las
            // entradas, el cbuffer, las salidas y las llamadas, de aqui (las
            // de `textura_sysv`: `mv` y los `iniciales` de su programa).
            unsafe {
                (*pmv).elegida = None;
                fv(rv.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr(), &lv);
            }
        } else {
            en.vs.correr_con(ent, l.cb, &l.recursos, sal, &mut rv);
        }
    };
    let mut ps = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rp.clear();
        rp.extend_from_slice(&en.ps.iniciales);
        if ent.len() >= en.ps.entradas && sal.len() >= en.ps.salidas {
            // La VELOCIDAD (05-10): lo traducido ya puede tener `discard`
            // (por el cuerpo del computo), y lo dice al volver.
            // SAFETY: como el de vertices, con `en.ps` y `mp`.
            unsafe {
                (*pmp).elegida = None;
                fp(rp.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr(), &lp) != nativo::DESCARTADO
            }
        } else {
            en.ps.correr_con(ent, l.cb, &l.recursos, sal, &mut rp)
        }
    };
    lote::en_cpu_con(l, destino, &mut vs, &mut ps)
}
