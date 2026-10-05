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
//! cuatro punteros), asi que se copia tal cual.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::lote::{self, Enlace, Lote, NoDibuja};
use bmo_proton_x::{nativo, trama};

use crate::{aviso, plataforma};

/// `fn(registros, entradas, cbuffer, salidas)`: solo punteros, el ABI entero
/// que el Rust soft-float de Ring 3 sabe llamar.
type Sombreador = extern "sysv64" fn(*mut f32, *const [f32; 4], *const u8, *mut [f32; 4]);

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
    // Un sombreador que MUESTREA una textura no se traduce todavia: el PSO
    // entero va por el interprete (el muestreo, `bmo_proton_x::textura`).
    let (Some(cv), Some(cp)) = (nativo::compilar(&en.vs), nativo::compilar(&en.ps)) else {
        aviso("un PSO con texturas: sus sombreadores se interpretan (el codigo nativo aun no muestrea)");
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
pub(crate) fn computo(desde: usize) -> Option<bmo_proton_x::nativo_computo::Funcion> {
    let (base, n) = estado().bloque?;
    // SAFETY: `base + desde` es el principio de una funcion traducida por
    // `nativo_computo::compilar`, dentro del bloque sellado vivo (que mide
    // `n`); su firma es esa.
    (desde < n).then(|| unsafe { core::mem::transmute::<usize, bmo_proton_x::nativo_computo::Funcion>(base as usize + desde) })
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
    // `nativo::compilar`, dentro del bloque sellado vivo; su firma es esa.
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
    let (mut rv, mut rp) = (Vec::new(), Vec::new());
    let mut vs = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rv.clear();
        rv.extend_from_slice(&en.vs.iniciales);
        if ent.len() >= en.vs.entradas && sal.len() >= en.vs.salidas {
            fv(rv.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr());
        } else {
            en.vs.correr(ent, cb, sal, &mut rv);
        }
    };
    let mut ps = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rp.clear();
        rp.extend_from_slice(&en.ps.iniciales);
        if ent.len() >= en.ps.entradas && sal.len() >= en.ps.salidas {
            // Lo traducido no tiene `Op::Descarta` (`nativo` no lo traduce):
            // el pixel siempre queda.
            fp(rp.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr());
            true
        } else {
            en.ps.correr(ent, cb, sal, &mut rp)
        }
    };
    lote::en_cpu_con(l, destino, &mut vs, &mut ps)
}
