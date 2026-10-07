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
use core::sync::atomic::{AtomicU64, Ordering};

use bmo_proton_x::lote::{self, Enlace, Lote, NoDibuja};
use bmo_proton_x::nativo_llamadas::{Llamadas, Muestras};
use bmo_proton_x::{nativo, trama};

use crate::{aviso, plataforma};

/// La firma de un sombreador de computo traducido (`nativo_computo`): el
/// ABI de System V. Vive aqui y no en `bmo-proton-x`, que es puro.
pub type FuncionComputo = unsafe extern "sysv64" fn(*mut f32, *mut bmo_proton_x::nativo_computo::Contexto, *const u8) -> u32;

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

/// **Lo que llama un vertice o un pixel traducido** (A10, 06-10): sus
/// `Muestras` (la textura ELEGIDA) y los UAV del lote. La celda se toma
/// SOLO durante cada operacion (`operar_uav`), como el interprete: entre
/// una y otra nadie la tiene, y quien rehace en el interprete la ve libre.
pub struct LlamadoDibujo<'a> {
    pub muestras: Muestras<'a>,
    pub uavs: Option<&'a lote::Uavs>,
}

/// **La llamada de un dibujo traducido** (A10, 06-10): la operacion `k` de
/// su programa -- una de UAV, con `operar_uav` del interprete sobre los UAV
/// del lote; una de textura, con sus `Muestras` --, los MISMOS bits que el
/// interprete (`correr_con_uavs`).
///
/// # Safety
/// `datos` apunta a un `LlamadoDibujo` vivo y `regs` a tantos registros
/// como `iniciales` tiene su programa, sin otra referencia a ellos mientras
/// dura: lo promete quien pone las `Llamadas` (`dibujar`).
pub unsafe extern "sysv64" fn dibujo_sysv(datos: *mut u8, regs: *mut f32, k: u32) {
    use bmo_proton_x::dxil::programa::Op;
    // SAFETY: lo de arriba.
    let l = unsafe { &mut *(datos as *mut LlamadoDibujo) };
    let regs = unsafe { core::slice::from_raw_parts_mut(regs, l.muestras.programa.iniciales.len()) };
    match l.muestras.programa.ops.get(k as usize) {
        Some(op @ (Op::LeeUav { .. } | Op::EscribeUav { .. } | Op::Atomico { .. } | Op::MedidasUav { .. } | Op::Contador { .. })) => match l.uavs {
            Some(u) => bmo_proton_x::dxil::operar_uav(*op, regs, &mut u.borrow_mut()),
            None => bmo_proton_x::dxil::operar_uav(*op, regs, &mut []),
        },
        _ => l.muestras.llamar(regs, k),
    }
}

/// Las `Llamadas` de un dibujo con su `LlamadoDibujo` (que tiene que vivir
/// lo que ellas: aqui solo se guarda el puntero).
pub fn llamadas_dibujo(l: *mut LlamadoDibujo, cb_bytes: usize) -> Llamadas {
    let t: unsafe extern "sysv64" fn(*mut u8, *mut f32, u32) = dibujo_sysv;
    Llamadas { textura: t as usize, datos: l as *mut u8, ..Llamadas::nuevas(cb_bytes) }
}

/// **Lo que llama un hilo de computo traducido** (06-10): sus `Muestras`
/// (la textura ELEGIDA es suya) y las ranuras de UAV que van por la llamada
/// (`nativo_computo::uavs_llamados`: las de textura, atomicos o medidas),
/// que son SOLO de la llamada -- el `Contexto` las ve nulas.
pub struct LlamadoComputo<'a> {
    pub muestras: Muestras<'a>,
    pub uavs: *mut Option<bmo_proton_x::bufer::Uav<'static>>,
    pub n_uavs: usize,
}

/// **La llamada del computo traducido** (06-10): la operacion `k` de su
/// programa -- una de UAV, con `operar_uav` del interprete sobre las
/// ranuras llamadas; una de textura, con las `Muestras` del hilo --, los
/// MISMOS bits que el interprete.
///
/// # Safety
/// `datos` apunta a un `LlamadoComputo` vivo, sus `uavs` a `n_uavs` ranuras
/// que nadie mas toca mientras dura, y `regs` a tantos registros como
/// `iniciales` tiene su programa: lo promete quien despacha.
pub unsafe extern "sysv64" fn computo_sysv(datos: *mut u8, regs: *mut f32, k: u32) {
    use bmo_proton_x::dxil::programa::Op;
    // SAFETY: lo de arriba.
    let l = unsafe { &mut *(datos as *mut LlamadoComputo) };
    let regs = unsafe { core::slice::from_raw_parts_mut(regs, l.muestras.programa.iniciales.len()) };
    match l.muestras.programa.ops.get(k as usize) {
        Some(op @ (Op::LeeUav { .. } | Op::EscribeUav { .. } | Op::Atomico { .. } | Op::MedidasUav { .. } | Op::Contador { .. })) => {
            // SAFETY: lo de arriba (las ranuras llamadas, solo de aqui).
            let uavs = unsafe { core::slice::from_raw_parts_mut(l.uavs, l.n_uavs) };
            bmo_proton_x::dxil::operar_uav(*op, regs, uavs);
        }
        _ => l.muestras.llamar(regs, k),
    }
}

/// Las `Llamadas` de un hilo de computo con su `LlamadoComputo` (que tiene
/// que vivir lo que ellas: aqui solo se guarda el puntero).
pub fn llamadas_computo(l: *mut LlamadoComputo, cb_bytes: usize) -> Llamadas {
    let t: unsafe extern "sysv64" fn(*mut u8, *mut f32, u32) = computo_sysv;
    Llamadas { textura: t as usize, datos: l as *mut u8, ..Llamadas::nuevas(cb_bytes) }
}

/// **Un Dispatch con el computo TRADUCIDO** (`f`, de
/// `nativo_computo::compilar` sobre `p`), 06-10: las ranuras de UAV que van
/// por la llamada (`nativo_computo::uavs_llamados`: texturas, atomicos,
/// medidas) salen de `uavs` y son SOLO de ella; cada hilo del grupo, sus
/// `Muestras` (su textura elegida) y sus `Llamadas`. Lo demas, como el
/// interprete (`Programa::despachar`): su juez, bit a bit. Publica: el banco
/// la mide contra el.
pub fn despachar_computo(p: &bmo_proton_x::dxil::programa::Programa, f: FuncionComputo, grupos: [u32; 3], cb: &[u8], rec: &bmo_proton_x::textura::Recursos, buferes: &[Option<bmo_proton_x::bufer::Bufer>], uavs: &mut [Option<bmo_proton_x::bufer::Uav<'static>>]) -> u64 {
    let llamados = bmo_proton_x::nativo_computo::uavs_llamados(p);
    let mut suyas: Vec<Option<bmo_proton_x::bufer::Uav<'static>>> = uavs.iter_mut().enumerate().map(|(k, u)| if llamados.get(k) == Some(&true) { u.take() } else { None }).collect();
    let [hx, hy, hz] = p.computo.hilos;
    let n = (hx * hy * hz) as usize;
    let (pu, nu) = (suyas.as_mut_ptr(), suyas.len());
    let mut hilos: Vec<LlamadoComputo> = (0..n).map(|_| LlamadoComputo { muestras: Muestras { programa: p, recursos: rec, elegida: None }, uavs: pu, n_uavs: nu }).collect();
    let mut por_hilo: Vec<Llamadas> = hilos.iter_mut().map(|h| llamadas_computo(h, cb.len())).collect();
    // SAFETY: `f` es la traduccion de `p`, en el bloque sellado de quien
    // llama (que no cambia mientras corre: un Dispatch no cede el turno). Sus
    // llamadas apuntan a `hilos` y a `suyas`, que viven hasta el final.
    let mut llamar = |r: *mut f32, c: *mut bmo_proton_x::nativo_computo::Contexto, b: *const u8| unsafe { f(r, c, b) };
    let corridos = bmo_proton_x::nativo_computo::despachar(p, &mut llamar, grupos, cb, buferes, uavs, &mut por_hilo);
    // Las ranuras llamadas, de vuelta a su sitio (quien llamo las ve igual).
    drop(por_hilo);
    drop(hilos);
    for (k, s) in suyas.into_iter().enumerate() {
        if s.is_some() {
            uavs[k] = s;
        }
    }
    corridos
}

struct Traducido {
    /// El `Enlace` de su PSO (los PSO no se liberan: su direccion vale).
    enlace: usize,
    vs: usize,
    ps: usize,
    /// X3 (06-10): el de pixeles DERIVA (`ddx`, la mip de un `Sample`...):
    /// `ps` es su cuerpo sin entrada, que corre en cuadros de 2x2
    /// ([`en_cuadros`]); si no, un [`Sombreador`] de un pixel.
    cuadros: bool,
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
    // interprete); lo que no, el PSO entero va por el interprete. X3 (06-10):
    // el de pixeles que DERIVA, en cuadros de 2x2.
    let cuadros = en.ps.usa_olas();
    let ps = if cuadros { nativo::compilar_cuadros(&en.ps) } else { nativo::compilar(&en.ps) };
    let (Some(cv), Some(cp)) = (nativo::compilar(&en.vs), ps) else {
        // N5.13 (05-10): el motivo de VERDAD.
        match nativo::por_que_no(&en.vs).or_else(|| nativo::por_que_no(&en.ps)) {
            Some(m) => aviso(&alloc::format!("un PSO cuyo sombreador {m}: sus sombreadores se interpretan (el codigo nativo aun no lo sabe)")),
            None => aviso("un PSO cuyos sombreadores el codigo nativo no sabe traducir: se interpretan"),
        }
        return;
    };
    let vs = agregar(e, &cv);
    let ps = agregar(e, &cp);
    e.traducidos.push(Traducido { enlace: en as *const Enlace as usize, vs, ps, cuadros });
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
    let fv = funcion(base, t.vs);
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
    // vertice o pixel (como el interprete). A10 (06-10): y los UAV del lote.
    let llamado = |p| LlamadoDibujo { muestras: Muestras { programa: p, recursos: &l.recursos, elegida: None }, uavs: l.uavs };
    let (mut mv, mut mp) = (llamado(&en.vs), llamado(&en.ps));
    let (pmv, pmp): (*mut LlamadoDibujo, *mut LlamadoDibujo) = (&mut mv, &mut mp);
    let (lv, lp) = (llamadas_dibujo(pmv, cb.len()), llamadas_dibujo(pmp, cb.len()));
    let (mut rv, mut rp) = (Vec::new(), Vec::new());
    // Lo que no cabe en lo traducido, por el interprete con los mismos UAV.
    let interpretar = |p: &bmo_proton_x::dxil::programa::Programa, ent: &[[f32; 4]], sal: &mut [[f32; 4]], r: &mut Vec<f32>| match l.uavs {
        Some(u) => p.correr_con_uavs(ent, l.cb, &l.recursos, sal, r, &mut u.borrow_mut()),
        None => p.correr_con(ent, l.cb, &l.recursos, sal, r),
    };
    let mut vs = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rv.clear();
        rv.extend_from_slice(&en.vs.iniciales);
        if ent.len() >= en.vs.entradas && sal.len() >= en.vs.salidas {
            // SAFETY: `fv` es la traduccion de `en.vs`; los registros, las
            // entradas, el cbuffer, las salidas y las llamadas, de aqui (las
            // de `dibujo_sysv`: `mv` y los `iniciales` de su programa).
            unsafe {
                (*pmv).muestras.elegida = None;
                fv(rv.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr(), &lv);
            }
        } else {
            interpretar(&en.vs, ent, sal, &mut rv);
        }
    };
    if t.cuadros {
        // SAFETY: `base + t.ps` es el principio de un cuerpo traducido por
        // `nativo::compilar_cuadros`, dentro del bloque sellado vivo; su firma
        // es la de `FuncionComputo`.
        let fc = unsafe { core::mem::transmute::<usize, FuncionComputo>(base as usize + t.ps) };
        let mut cuadros = en_cuadros(&en.ps, &en.objetivos, &l.recursos, cb, fc);
        return lote::en_cpu_en_olas(l, destino, &mut vs, &mut cuadros);
    }
    let fp = funcion(base, t.ps);
    let mut ps = |ent: &[[f32; 4]], sal: &mut [[f32; 4]]| {
        rp.clear();
        rp.extend_from_slice(&en.ps.iniciales);
        if ent.len() >= en.ps.entradas && sal.len() >= en.ps.salidas {
            // La VELOCIDAD (05-10): lo traducido ya puede tener `discard`
            // (por el cuerpo del computo), y lo dice al volver.
            // SAFETY: como el de vertices, con `en.ps` y `mp`.
            unsafe {
                (*pmp).muestras.elegida = None;
                fp(rp.as_mut_ptr(), ent.as_ptr(), cbp, sal.as_mut_ptr(), &lp) != nativo::DESCARTADO
            }
        } else {
            interpretar(&en.ps, ent, sal, &mut rp)
        }
    };
    lote::en_cpu_con(l, destino, &mut vs, &mut ps)
}

/// X3: los cuadros que corrio lo traducido, y los que se rehicieron en el
/// interprete (se separaron). Los mira el banco (que lo juzgado no sea solo
/// el interprete) y quien quiera saber cuanto se rehace.
static NATIVOS: AtomicU64 = AtomicU64::new(0);
static REHECHOS: AtomicU64 = AtomicU64::new(0);

/// (cuadros por lo traducido, cuadros rehechos en el interprete), desde que
/// empezo el proceso.
pub fn cuadros_contados() -> (u64, u64) {
    (NATIVOS.load(Ordering::Relaxed), REHECHOS.load(Ordering::Relaxed))
}

/// **X3 (06-10): quien corre los pixeles de un dibujo que DERIVA, en
/// cuadros de 2x2, con lo traducido** (`f`, de `nativo::compilar_cuadros`).
/// Cada cuadro, sus cuatro carriles (ayudantes incluidos) con su `Contexto`
/// y sus `Muestras` (la textura ELEGIDA es de cada uno: una parada cae entre
/// `EligeTextura` y su muestreo). Cada vez que los cuatro vuelven con `OLA`
/// en el MISMO punto, sus derivadas se hacen como en el interprete
/// (`olas::hacer`, con los registros de los cuatro) y siguen. Si se separan
/// (uno acaba o se tira y otro se para, o se paran en sitios distintos), el
/// cuadro entero se rehace en el interprete: un pixel que deriva y toca UAV
/// no se traduce (`nativo::compilar_cuadros`), asi que correrlo otra vez no
/// deja nada, y sale lo del interprete.
///
/// `objetivos`: a que render target va cada salida (los del enlace); `cb`,
/// el cbuffer YA con lo que leen sus filas (ver `dibujar`). Publica: el
/// banco la juzga contra el interprete en olas, bit a bit.
pub fn en_cuadros<'a>(ps: &'a bmo_proton_x::dxil::programa::Programa, objetivos: &'a [u8], rec: &'a bmo_proton_x::textura::Recursos<'a>, cb: &'a [u8], f: FuncionComputo) -> impl FnMut(&mut [bmo_proton_x::cuadros::Carril]) + 'a {
    use bmo_proton_x::dxil::olas;
    use bmo_proton_x::dxil::programa::Op;
    use bmo_proton_x::nativo_computo::{self as nc, Contexto, Vista, VISTAS};
    static CERO: [u8; 16] = [0; 16];
    let cbp = if cb.is_empty() { CERO.as_ptr() } else { cb.as_ptr() };
    let paradas = nc::paradas(ps, true);
    let n_sal = ps.salidas.max(objetivos.len()).max(1);
    // Un pixel que DERIVA no se traduce si toca UAV
    // (`nativo::compilar_cuadros`): sin ellos.
    let mut interprete = lote::olas_de(ps, cb, rec, objetivos, None);
    // Las cuatro `Muestras` no se mueven mas: sus `Llamadas` guardan su
    // direccion (y solo se tocan por ella).
    let mut muestras: alloc::boxed::Box<[Muestras<'a>; 4]> = alloc::boxed::Box::new(core::array::from_fn(|_| Muestras { programa: ps, recursos: rec, elegida: None }));
    let pm: *mut Muestras<'a> = muestras.as_mut_ptr();
    // SAFETY: `pm.add(q)`, q < 4, dentro de la caja.
    let ll: [Llamadas; 4] = core::array::from_fn(|q| llamadas(unsafe { pm.add(q) }, cb.len()));
    let mut regs: [Vec<f32>; 4] = Default::default();
    let mut sal: [Vec<[f32; 4]>; 4] = Default::default();
    move |ola: &mut [bmo_proton_x::cuadros::Carril]| {
        let _ = &muestras;
        for cuadro in ola.chunks_mut(4) {
            let hecho = 'cuadro: {
                if cuadro.len() < 4 || cuadro.iter().any(|c| c.entrada.len() < ps.entradas) {
                    break 'cuadro false;
                }
                let mut ctx: [Contexto; 4] = core::array::from_fn(|q| {
                    regs[q].clear();
                    regs[q].extend_from_slice(&ps.iniciales);
                    sal[q].clear();
                    sal[q].resize(n_sal, [0.0; 4]);
                    // SAFETY: como arriba; nadie mas la toca ahora.
                    unsafe { (*pm.add(q)).elegida = None };
                    Contexto {
                        ids: [0; 10],
                        reanudar: 0,
                        n_compartida: 0,
                        compartida: core::ptr::null_mut(),
                        entradas: cuadro[q].entrada.as_ptr(),
                        salidas: sal[q].as_mut_ptr(),
                        llamadas: &ll[q],
                        srv: [Vista::NULA; VISTAS],
                        uav: [Vista::NULA; VISTAS],
                    }
                });
                // Lo de cada carril: `None` corre; `Some(queda)` acabo.
                let mut fin = [None::<bool>; 4];
                loop {
                    let mut punto = [0u32; 4];
                    for q in 0..4 {
                        if fin[q].is_some() {
                            continue;
                        }
                        // SAFETY: `f` es la traduccion de `ps`; sus registros
                        // (`iniciales` de largo), su Contexto (entradas, salidas
                        // y llamadas de aqui, vivas) y el cbuffer (`cbp`, con
                        // lo que leen sus filas, ver `dibujar`).
                        match unsafe { f(regs[q].as_mut_ptr(), &mut ctx[q], cbp) } {
                            nc::ACABO => fin[q] = Some(true),
                            nc::DESCARTADO => fin[q] = Some(false),
                            nc::OLA => punto[q] = ctx[q].reanudar,
                            _ => break 'cuadro false,
                        }
                    }
                    if fin.iter().all(Option::is_some) {
                        break;
                    }
                    let k = punto[0];
                    if k == 0 || fin.iter().any(Option::is_some) || punto.iter().any(|&p| p != k) {
                        break 'cuadro false;
                    }
                    for &i in paradas.get(k as usize - 1).map_or(&[][..], |g| &g[..]) {
                        let Some(&Op::Ola { d, a, b, que }) = ps.ops.get(i) else {
                            break 'cuadro false;
                        };
                        // Todos antes de escribir ninguno (uno lee el `a` de otro).
                        let r: [u32; 4] = core::array::from_fn(|q| olas::hacer(que, q, 0b1111, |j| regs[j & 3][a as usize].to_bits(), regs[q][b as usize].to_bits())[0]);
                        for q in 0..4 {
                            regs[q][d as usize] = f32::from_bits(r[q]);
                        }
                    }
                }
                for (q, c) in cuadro.iter_mut().enumerate() {
                    c.queda = fin[q] == Some(true);
                    for (j, &t) in objetivos.iter().enumerate() {
                        c.colores[t as usize] = sal[q][j];
                    }
                }
                true
            };
            if hecho {
                NATIVOS.fetch_add(1, Ordering::Relaxed);
            } else {
                REHECHOS.fetch_add(1, Ordering::Relaxed);
                interprete(cuadro);
            }
        }
    }
}

