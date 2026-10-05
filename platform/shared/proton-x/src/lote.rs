//! **Un lote: un dibujo de D3D12 SIN D3D12** (P4, 27-09) -- la costura entre
//! PROTON-X y quien dibuja de verdad.
//!
//! La casa (`proton-x-casa/src/tuberia.rs`) sabe de D3D12: punteros del
//! `.exe`, descriptores, root signatures, direcciones de GPU. Aqui no hay
//! nada de eso. Un [`Lote`] es solo lo que un dibujo NECESITA, ya resuelto a
//! bytes y numeros:
//!
//! ```text
//!    los dos sombreadores, compilados y cosidos      [`Enlace`]
//!    el input layout y los bytes de los vertices     [`ElementoIa`], `vertices`
//!    los vertices que se piden, en orden             `ids` + [`Topologia`]
//!    las constantes (b0)                             `cb`
//!    viewport, tijera, descarte                      `trama::Reglas`
//! ```
//!
//! y un [`Ejecutor`] es CUALQUIERA que sepa dibujarlo en un destino. Hoy hay
//! uno, [`en_cpu`] (los DXIL interpretados y la trama: la que dio las huellas
//! de la 3060). El siguiente es VERRANO con la 3060 (P3b4): el DXIL traducido
//! una vez a SASS, el mismo lote, el mismo destino -- y [`en_cpu`] como JUEZ
//! de lo que dibuje. La casa no cambia: la plataforma le da el ejecutor.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::dxil::programa::{self, Programa, Ranuras};
use crate::dxil::carriles::{Carril, Estado};
use crate::dxil::{Sombreador, Tiras};
use crate::trama;

pub use crate::formato_ia::{FMT_R32G32B32A32_FLOAT, FMT_R32G32B32_FLOAT, FMT_R32G32_FLOAT, FMT_R32_FLOAT};

/// SV_Position en una firma (valor de sistema 1).
const SV_POSITION: u32 = 1;
/// SV_VertexID y SV_InstanceID (`D3D_NAME` 6 y 8): no vienen del input layout,
/// los pone quien dibuja.
const SV_VERTEXID: u32 = 6;
const SV_INSTANCEID: u32 = 8;
/// SV_Target (`D3D_NAME_TARGET`): un render target.
const SV_TARGET: u32 = 64;
/// SV_Depth y sus variantes (`D3D_NAME_DEPTH`, `_GREATER_EQUAL`, `_LESS_EQUAL`).
const SV_DEPTH: u32 = 65;
const SV_DEPTH_MAYOR_IGUAL: u32 = 67;
const SV_DEPTH_MENOR_IGUAL: u32 = 68;

/// **De donde sale una entrada del sombreador de vertices** (03-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuente {
    /// Del elemento `i` del input layout.
    Ia(usize),
    /// SV_VertexID: el numero del vertice (el id, con el vertice base).
    Vertice,
    /// SV_InstanceID: el numero de la instancia, desde 0 (N5.13: sin
    /// StartInstanceLocation).
    Instancia,
}

/// Un elemento del input layout, ya leido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementoIa {
    pub semantica: String,
    pub indice: u32,
    pub formato: u32,
    pub ranura: u32,
    pub desde: u32,
    /// N5.13 (05-10): `None`, un elemento POR VERTICE; `Some(k)`, POR
    /// INSTANCIA (`D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA`): avanza
    /// cada `k` instancias (InstanceDataStepRate; con 0, todas leen el
    /// primero).
    pub por_instancia: Option<u32>,
}

/// **El bufer de vertices de otra ranura** (N5.13): sus bytes y su paso
/// (`D3D12_VERTEX_BUFFER_VIEW`). Sin bufer, `bytes` va vacio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flujo<'a> {
    pub bytes: &'a [u8],
    pub paso: usize,
}

/// Cuantos componentes trae un formato de vertice (1 si no es uno).
pub fn componentes(formato: u32) -> usize {
    crate::formato_ia::forma(formato).map_or(1, |f| f.componentes())
}

/// **Los dos sombreadores, listos y cosidos**: de donde sale cada entrada del
/// de vertices (el input layout) y cada entrada del de pixeles (las salidas
/// del de vertices).
#[derive(Debug, Clone, PartialEq)]
pub struct Enlace {
    pub vs: Programa,
    pub ps: Programa,
    /// Por elemento de entrada del de vertices: de donde sale (el input
    /// layout, o el numero de vertice o de instancia).
    pub desde_ia: Vec<Fuente>,
    /// La salida que es SV_Position: la del de vertices, o (E2.3b) la del
    /// de GEOMETRIA si lo hay (la ultima etapa antes de la trama).
    pub posicion: usize,
    /// Por elemento de entrada del de pixeles: la salida de la ultima etapa
    /// antes de la trama (el de vertices, o el GS) que le llega (`None`:
    /// SV_Position, que pone la trama).
    pub desde_vs: Vec<Option<usize>>,
    /// N5.9 (03-10): la entrada del de pixeles que es SV_Position, si la
    /// LEE: ahi la trama pone (x + 0.5, y + 0.5, z, w) del pixel.
    pub pos_ps: Option<usize>,
    /// N5.8 (03-10): por salida del de pixeles, a que render target va (su
    /// SV_Target: 0..8; `trama::PROFUNDIDAD` si es su SV_Depth). El
    /// G-buffer de Cyberpunk escribe cuatro o cinco.
    pub objetivos: Vec<u8>,
    /// 03-10: la salida del de pixeles que es SV_Depth (o sus variantes
    /// GreaterEqual/LessEqual), si la escribe.
    pub profundidad_ps: Option<usize>,
    /// Las texturas y los muestreadores de LOS DOS, en una tabla (03-10,
    /// N5.1): el `t` y el `s` de sus operaciones son posiciones aqui. Quien
    /// dibuja pone en cada posicion el descriptor de ese espacio y registro.
    pub ranuras: Ranuras,
    /// Los cbuffers de los dos, APLANADOS (03-10, N5.2): el de la ranura `i`
    /// (`ranuras.cbuffers[i]`) va en `constantes[i]`. Quien dibuja copia cada
    /// uno a su sitio de [`Lote::cb`].
    pub constantes: Vec<Bloque>,
    /// E2.3b (05-10): el sombreador de GEOMETRIA, si el PSO trae uno.
    pub gs: Option<EnlaceGs>,
}

/// **El sombreador de geometria de un enlace** (E2.3b, 05-10): corre UNA
/// vez por primitiva (puntos, lineas o triangulos) con las salidas del de
/// vertices de sus vertices, y lo que emite es lo que pinta la trama.
#[derive(Debug, Clone, PartialEq)]
pub struct EnlaceGs {
    pub programa: Programa,
    /// Por elemento de entrada del GS: la salida del de vertices que le
    /// llega (por su semantica).
    pub desde_vs: Vec<usize>,
    /// Su primitiva de entrada, su topologia de salida y cuantos emite.
    pub info: crate::dxil::recursos::Geometria,
}

/// **Un cbuffer en el bloque de las constantes**: desde que fila y cuantas
/// lee de el (de 16 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloque {
    pub fila: u16,
    pub filas: u16,
}

/// **Juntar las constantes de un dibujo**: el cbuffer de cada ranura (lo
/// que da `cada`, o nada) en su sitio del bloque. Lo que un cbuffer no
/// trae se lee como 0 (en D3D12, leer mas alla de la vista da 0).
pub fn juntar_constantes<'a>(constantes: &[Bloque], mut cada: impl FnMut(usize) -> Option<&'a [u8]>) -> Vec<u8> {
    let total = constantes.iter().map(|b| (b.fila as usize + b.filas as usize) * 16).max().unwrap_or(0);
    let mut v = vec![0u8; total];
    for (i, b) in constantes.iter().enumerate() {
        if let Some(c) = cada(i) {
            let (desde, n) = (b.fila as usize * 16, (b.filas as usize * 16).min(c.len()));
            v[desde..desde + n].copy_from_slice(&c[..n]);
        }
    }
    v
}

/// **Aplanar los cbuffers de todos** (el de vertices, el de pixeles y,
/// E2.3b, el de geometria): uno detras de otro, cada uno con las filas que
/// se leen de el. El interprete, la 3060 y el x86 ven UN cbuffer; solo
/// quien dibuja sabe que son varios.
fn aplanar(programas: &mut [&mut Programa], n: usize) -> Result<Vec<Bloque>, String> {
    let mut filas = vec![0u16; n];
    for p in programas.iter() {
        p.filas_por_cbuffer(&mut filas);
    }
    let mut bloques = Vec::with_capacity(filas.len());
    let mut fila = 0u32;
    for &f in &filas {
        bloques.push(Bloque { fila: fila as u16, filas: f });
        fila += f as u32;
    }
    if fila > u16::MAX as u32 {
        return Err(format!("los cbuffers de los dos sombreadores leen {fila} filas: mas de las que caben"));
    }
    let bases: Vec<u16> = bloques.iter().map(|b| b.fila).collect();
    for p in programas.iter_mut() {
        p.aplanar(&bases);
    }
    Ok(bloques)
}

/// **Coser** los dos sombreadores con el input layout. El texto dice por que
/// no, si no.
pub fn enlazar(vs: &Sombreador, ps: &Sombreador, entradas: &[ElementoIa]) -> Result<Enlace, String> {
    enlazar_con(vs, Some(ps), entradas)
}

/// **Coser** con o sin sombreador de pixeles (N5.12: sin el, el dibujo es
/// solo profundidad -- las sombras, el prepaso de Z --, y el de pixeles es
/// [`Programa::vacio`]).
pub fn enlazar_con(vs: &Sombreador, ps: Option<&Sombreador>, entradas: &[ElementoIa]) -> Result<Enlace, String> {
    enlazar_con_gs(vs, None, ps, entradas)
}

/// **Coser** con un sombreador de GEOMETRIA en medio, o sin el (E2.3b,
/// 05-10): el GS lee las salidas del de vertices, y el de pixeles las del GS.
pub fn enlazar_con_gs(vs: &Sombreador, gs: Option<&Sombreador>, ps: Option<&Sombreador>, entradas: &[ElementoIa]) -> Result<Enlace, String> {
    let mut pv = programa::compilar(vs).map_err(|e| format!("el sombreador de vertices no se sabe correr todavia: {e:?}"))?;
    let mut pg = gs.map(|g| programa::compilar(g).map_err(|e| format!("el sombreador de geometria no se sabe correr todavia: {e:?}"))).transpose()?;
    let mut pp = match ps {
        Some(ps) => programa::compilar(ps).map_err(|e| format!("el sombreador de pixeles no se sabe correr todavia: {e:?}"))?,
        None => Programa::vacio(),
    };
    let (ps_entradas, ps_salidas) = ps.map_or((&[][..], &[][..]), |p| (&p.entradas[..], &p.salidas[..]));
    // Una tabla para todos: las del de vertices se quedan donde estan y las
    // de los demas se renumeran a la suya (la misma si los dos la leen).
    // Cada lugar, con su etapa: el t0 de uno no es el t0 del otro si la root
    // signature les da tablas distintas (`donde::en_tabla`).
    let mut ranuras = pv.ranuras.clone().de_la_etapa(crate::donde::VISTA_VERTICES);
    if let Some(g) = pg.as_mut() {
        let mapa = ranuras.unir(&g.ranuras.clone().de_la_etapa(crate::donde::VISTA_GEOMETRIA)).map_err(|e| format!("los recursos de los sombreadores: {e:?}"))?;
        g.renumerar(&mapa);
    }
    let mapa = ranuras.unir(&pp.ranuras.clone().de_la_etapa(crate::donde::VISTA_PIXELES)).map_err(|e| format!("los recursos de los dos sombreadores: {e:?}"))?;
    pp.renumerar(&mapa);
    pp.ranuras = ranuras.clone();
    pv.ranuras = ranuras.clone();
    if let Some(g) = pg.as_mut() {
        g.ranuras = ranuras.clone();
    }
    let constantes = match pg.as_mut() {
        Some(g) => aplanar(&mut [&mut pv, g, &mut pp], ranuras.cbuffers.len())?,
        None => aplanar(&mut [&mut pv, &mut pp], ranuras.cbuffers.len())?,
    };
    let mut desde_ia = Vec::with_capacity(vs.entradas.len());
    for f in &vs.entradas {
        match f.sistema {
            0 => {}
            SV_VERTEXID => {
                desde_ia.push(Fuente::Vertice);
                continue;
            }
            SV_INSTANCEID => {
                desde_ia.push(Fuente::Instancia);
                continue;
            }
            s => return Err(format!("el sombreador de vertices lee el valor de sistema {s} ({}): todavia no", f.semantica)),
        }
        let i = entradas.iter().position(|e| e.semantica.eq_ignore_ascii_case(&f.semantica) && e.indice == f.indice);
        desde_ia.push(Fuente::Ia(i.ok_or_else(|| format!("el sombreador de vertices lee {}{} y el input layout no lo da", f.semantica, f.indice))?));
    }
    // E2.3b: con un GS, lo que llega a la trama es lo que el emite; y el lee
    // del de vertices, por semantica.
    let (ultima, etapa) = match gs {
        Some(g) => (&g.salidas[..], "geometria"),
        None => (&vs.salidas[..], "vertices"),
    };
    let posicion = ultima.iter().position(|f| f.sistema == SV_POSITION).ok_or_else(|| format!("el sombreador de {etapa} no escribe SV_Position"))?;
    let gs = match (gs, pg) {
        (Some(g), Some(programa)) => {
            let Some(info) = g.geometria else {
                return Err(String::from("un sombreador de geometria sin su primitiva ni su topologia (sin PSV0: el SM5 todavia no)"));
            };
            if info.salida != 5 {
                return Err(format!("un GS que emite {} (topologia {}): la trama solo pinta triangulos todavia", if info.salida == 1 { "puntos" } else { "lineas" }, info.salida));
            }
            if posicion >= programa.salidas {
                return Err(String::from("el GS no escribe SV_Position"));
            }
            let mut desde = Vec::with_capacity(g.entradas.len());
            for f in &g.entradas {
                let k = vs.salidas.iter().position(|o| o.semantica.eq_ignore_ascii_case(&f.semantica) && o.indice == f.indice);
                desde.push(k.ok_or_else(|| format!("el GS lee {}{} y el de vertices no lo escribe", f.semantica, f.indice))?);
            }
            Some(EnlaceGs { programa, desde_vs: desde, info })
        }
        _ => None,
    };
    let mut desde_vs = Vec::with_capacity(ps_entradas.len());
    let mut pos_ps = None;
    for (i, f) in ps_entradas.iter().enumerate() {
        if f.sistema == SV_POSITION {
            if pp.lee & (1 << i) != 0 {
                pos_ps = Some(i);
            }
            desde_vs.push(None);
            continue;
        }
        let k = ultima.iter().position(|o| o.semantica.eq_ignore_ascii_case(&f.semantica) && o.indice == f.indice);
        desde_vs.push(Some(k.ok_or_else(|| format!("el sombreador de pixeles lee {}{} y el de {etapa} no lo escribe", f.semantica, f.indice))?));
    }
    let mut objetivos = Vec::with_capacity(ps_salidas.len());
    let mut profundidad_ps = None;
    for (k, f) in ps_salidas.iter().enumerate() {
        if matches!(f.sistema, SV_DEPTH | SV_DEPTH_MAYOR_IGUAL | SV_DEPTH_MENOR_IGUAL) {
            profundidad_ps = Some(k);
            objetivos.push(trama::PROFUNDIDAD as u8);
            continue;
        }
        if f.sistema != SV_TARGET && !f.semantica.eq_ignore_ascii_case("SV_Target") {
            return Err(format!("el sombreador de pixeles escribe {}{} (valor de sistema {}): todavia no", f.semantica, f.indice, f.sistema));
        }
        if f.indice as usize >= trama::OBJETIVOS {
            return Err(format!("el sombreador de pixeles escribe SV_Target{}: D3D12 tiene 8", f.indice));
        }
        objetivos.push(f.indice as u8);
    }
    Ok(Enlace { vs: pv, ps: pp, desde_ia, posicion, desde_vs, pos_ps, objetivos, profundidad_ps, ranuras, constantes, gs })
}

/// Como se agrupan los ids en triangulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topologia {
    Lista,
    /// Cada vertice nuevo con los dos anteriores; los impares, dados la vuelta.
    Tira,
    /// E2.3b (05-10): puntos, lineas y tiras de lineas. La trama pinta
    /// triangulos: estas solo se dibujan con un GS que los haga.
    Puntos,
    Lineas,
    TiraDeLineas,
}

/// **Las primitivas de un lote** (E2.3b), como ids: de `n` vertices cada
/// una (lo que lee el GS), o `None` si la topologia no da de esas.
pub fn primitivas(ids: &[u32], t: Topologia, n: usize) -> Option<Vec<Vec<u32>>> {
    Some(match (t, n) {
        (Topologia::Puntos, 1) => ids.iter().map(|&i| vec![i]).collect(),
        (Topologia::Lineas, 2) => ids.chunks_exact(2).map(<[u32]>::to_vec).collect(),
        (Topologia::TiraDeLineas, 2) => ids.windows(2).map(<[u32]>::to_vec).collect(),
        (Topologia::Lista | Topologia::Tira, 3) => triangulos(ids, t).into_iter().map(|x| x.to_vec()).collect(),
        _ => return None,
    })
}

/// **Un dibujo, sin D3D12.**
pub struct Lote<'a> {
    pub enlace: &'a Enlace,
    pub entradas: &'a [ElementoIa],
    /// Los bytes del bufer de vertices de la ranura 0, y su paso.
    pub vertices: &'a [u8],
    pub paso: usize,
    /// Los vertices que se piden, en orden (ya con el vertice base sumado).
    pub ids: &'a [u32],
    pub topologia: Topologia,
    /// Los bytes de las constantes que leen los sombreadores: los cbuffers
    /// uno detras de otro, como dice [`Enlace::constantes`] (con uno solo,
    /// el b0 tal cual).
    pub cb: &'a [u8],
    pub reglas: trama::Reglas,
    /// P3b4c: la profundidad se LIMPIO desde el ultimo dibujo en ella
    /// (`ClearDepthStencilView`), con estos bits (un f32). La CPU ya la limpio
    /// en su bufer; quien dibuja con OTRA Z (la de la 3060, en VRAM) lo
    /// necesita saber para limpiar la suya.
    pub limpiar_z: Option<u32>,
    /// P3b4c: el render target se LIMPIO (`ClearRenderTargetView`) y la casa
    /// NO lo hizo todavia: lo hace quien dibuje, con este pixel tal como va
    /// en memoria. La 3060 lo limpia ella; la CPU, al empezar
    /// ([`en_cpu_con`]). Igual con `limpiar_z`: la CPU la aplica a su bufer.
    pub limpiar_rt: Option<u32>,
    /// Las texturas y los muestreadores que ven sus sombreadores.
    pub recursos: crate::textura::Recursos<'a>,
    /// E2.7 (05-10): hay una consulta de OCLUSION abierta: quien dibuje
    /// tiene que contar los pixeles que pasan ([`trama::Cuenta::pasan`]).
    /// La trama de la CPU los cuenta siempre; la 3060, aun no.
    pub oclusion: bool,
    /// N5.13 (05-10): los buferes de vertices de las ranuras 1..16
    /// (`otros[r - 1]` es la ranura `r`); la 0 es `vertices` y `paso`.
    pub otros: &'a [Flujo<'a>],
    /// N5.13: cuantas instancias se dibujan (1, lo de siempre) y la primera
    /// (StartInstanceLocation): solo mueve lo que leen los elementos POR
    /// INSTANCIA; SV_InstanceID cuenta desde 0, como en D3D12.
    pub instancias: u32,
    pub primera_instancia: u32,
    /// 05-10: los UAV que ven sus sombreadores de vertices y de pixeles,
    /// por ranura (`enlace.ranuras.uavs`): lo que escriben QUEDA. `None`, un
    /// dibujo sin UAV. En una celda: el lote se da prestado (`&Lote`) y los
    /// sombreadores escriben, uno detras de otro, nunca a la vez.
    pub uavs: Option<&'a Uavs>,
}

/// Los UAV de un lote (ver [`Lote::uavs`]).
pub type Uavs = core::cell::RefCell<Vec<Option<crate::bufer::Uav<'static>>>>;

/// `f` con los UAV del lote (sin ellos, ninguno).
fn con_uavs<R>(l: &Lote, f: impl FnOnce(&mut [Option<crate::bufer::Uav<'static>>]) -> R) -> R {
    match l.uavs {
        Some(u) => f(&mut u.borrow_mut()),
        None => f(&mut []),
    }
}

impl Lote<'_> {
    /// El bufer de la ranura `r` (0..16): sus bytes y su paso.
    pub fn flujo(&self, r: u32) -> Flujo<'_> {
        if r == 0 {
            Flujo { bytes: self.vertices, paso: self.paso }
        } else {
            self.otros.get(r as usize - 1).copied().unwrap_or_default()
        }
    }

    /// Lo que el lote no puede dibujar antes de empezar (N5.13): un elemento
    /// que lee una ranura SIN bufer, o un id que pasa de un bufer que se lee
    /// por vertice. Si no, cuantos vertices distintos puede haber (el mayor
    /// id + 1).
    fn comprobar(&self) -> Result<usize, NoDibuja> {
        let mut tope = usize::MAX;
        for &f in &self.enlace.desde_ia {
            let Fuente::Ia(i) = f else { continue };
            let el = &self.entradas[i];
            let fl = self.flujo(el.ranura);
            if fl.bytes.is_empty() {
                return Err(NoDibuja::SinVertices);
            }
            if el.por_instancia.is_none() && fl.paso > 0 {
                tope = tope.min(fl.bytes.len() / fl.paso);
            }
        }
        let n = self.ids.iter().max().map_or(0, |&m| m as usize + 1);
        match self.ids.iter().find(|&&id| id as usize >= tope) {
            Some(&id) => Err(NoDibuja::IndiceFuera(id)),
            None => Ok(n),
        }
    }
}

/// Por que un ejecutor no dibujo un lote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDibuja {
    /// Un id pasa del bufer de vertices.
    IndiceFuera(u32),
    /// Sin paso: no hay vertices que leer.
    SinVertices,
}

/// **Quien dibuja un lote.** Lo pone la plataforma de la casa.
pub type Ejecutor = fn(&Lote, &mut trama::Destino) -> Result<trama::Cuenta, NoDibuja>;

/// Los triangulos de un lote, como ids.
pub fn triangulos(ids: &[u32], t: Topologia) -> Vec<[u32; 3]> {
    match t {
        Topologia::Lista => ids.chunks_exact(3).map(|x| [x[0], x[1], x[2]]).collect(),
        Topologia::Tira => ids.windows(3).enumerate().map(|(i, x)| if i % 2 == 0 { [x[0], x[1], x[2]] } else { [x[1], x[0], x[2]] }).collect(),
        // Sin GS, puntos y lineas no dan triangulos.
        Topologia::Puntos | Topologia::Lineas | Topologia::TiraDeLineas => Vec::new(),
    }
}

/// Corre un sombreador: entradas -> salidas (el cbuffer lo lleva dentro).
pub type Corre<'a> = &'a mut dyn FnMut(&[[f32; 4]], &mut [[f32; 4]]);

/// El de pixeles, igual, diciendo si el pixel QUEDA (`false`: un `discard`
/// lo tiro, N5.7).
pub type CorrePs<'a> = &'a mut dyn FnMut(&[[f32; 4]], &mut [[f32; 4]]) -> bool;

/// **Una entrada del de vertices** para el vertice `id` de la instancia
/// `instancia`: el elemento del layout con su formato (`formato_ia`), de SU
/// ranura (N5.13) y en el elemento que le toca (por vertice o por
/// instancia), o el numero de vertice o de instancia, como ENTERO en los
/// bits del registro. Lo que cae fuera del bufer se lee como ceros, como en
/// D3D12.
pub fn entrada(l: &Lote, fuente: Fuente, id: u32, instancia: u32) -> [f32; 4] {
    let entero = |n: u32| [f32::from_bits(n), 0.0, 0.0, 0.0];
    match fuente {
        Fuente::Vertice => entero(id),
        Fuente::Instancia => entero(instancia),
        Fuente::Ia(i) => {
            let el = &l.entradas[i];
            let f = l.flujo(el.ranura);
            let n = match el.por_instancia {
                None => id as usize,
                Some(0) => l.primera_instancia as usize,
                Some(k) => l.primera_instancia as usize + (instancia / k) as usize,
            };
            let desde = n * f.paso + el.desde as usize;
            let mide = crate::formato_ia::forma(el.formato).map_or(0, |x| x.bytes as usize);
            match f.bytes.get(desde..desde + mide) {
                Some(v) => crate::formato_ia::leer(el.formato, v),
                None => [0.0; 4],
            }
        }
    }
}

/// **El ejecutor de la CPU** (P3b3): el sombreador de vertices UNA vez por
/// vertice distinto, los triangulos por la trama, el de pixeles en cada pixel
/// que cubren -- con los sombreadores INTERPRETADOS. Es el que dio las huellas
/// de D3D12 en la 3060: el juez de los que vengan.
pub fn en_cpu(l: &Lote, destino: &mut trama::Destino) -> Result<trama::Cuenta, NoDibuja> {
    let (mut rv, mut rp) = (Vec::new(), Vec::new());
    let en = l.enlace;
    // 05-10: los dos, con los UAV del lote.
    let mut vs = |e: &[[f32; 4]], s: &mut [[f32; 4]]| {
        con_uavs(l, |u| en.vs.correr_con_uavs(e, l.cb, &l.recursos, s, &mut rv, u));
    };
    if !en.ps.usa_olas() {
        return en_cpu_con(l, destino, &mut vs, &mut |e, s| con_uavs(l, |u| en.ps.correr_con_uavs(e, l.cb, &l.recursos, s, &mut rp, u)));
    }
    // E2.5: un de pixeles con OLAS va en cuadros y olas (`cuadros`).
    let mut ola = olas_de(&en.ps, l.cb, &l.recursos, &en.objetivos, l.uavs);
    en_cpu_olas(l, destino, &mut vs, &mut |_, _| false, Some(&mut ola))
}

/// **Quien corre las olas de pixeles** de `ps` (E2.5, 05-10): los carriles
/// de cada una, juntos (`dxil::carriles`), con los UAV del lote (los
/// ayudantes, sin ellos), y la salida `k` de cada uno a su render target
/// `objetivos[k]`, como la de un pixel solo.
pub fn olas_de<'a, 'r: 'a>(ps: &'a Programa, cb: &'a [u8], rec: &'a crate::textura::Recursos<'r>, objetivos: &'a [u8], uavs: Option<&'a Uavs>) -> impl FnMut(&mut [crate::cuadros::Carril]) + 'a {
    let mut carriles: Vec<Carril> = Vec::new();
    let mut salidas: Vec<Vec<[f32; 4]>> = Vec::new();
    let n_sal = ps.salidas.max(objetivos.len());
    move |ola: &mut [crate::cuadros::Carril]| {
        carriles.resize_with(ola.len(), || Carril::nuevo(ps, false));
        salidas.resize_with(ola.len(), Vec::new);
        for ((c, s), o) in carriles.iter_mut().zip(salidas.iter_mut()).zip(ola.iter()) {
            c.reiniciar(ps, o.ayudante);
            s.clear();
            s.resize(n_sal, [0.0; 4]);
        }
        let entradas: Vec<&[[f32; 4]]> = ola.iter().map(|o| o.entrada.as_slice()).collect();
        match uavs {
            Some(u) => ps.correr_pixeles(&mut carriles[..ola.len()], &entradas, cb, rec, &mut salidas, &mut u.borrow_mut()),
            None => ps.correr_pixeles(&mut carriles[..ola.len()], &entradas, cb, rec, &mut salidas, &mut []),
        }
        for (k, o) in ola.iter_mut().enumerate() {
            o.queda = carriles[k].estado == Estado::Fin(true);
            for (j, &t) in objetivos.iter().enumerate() {
                o.colores[t as usize] = salidas[k][j];
            }
        }
    }
}

/// **Lo mismo, con quien corre los sombreadores puesto desde fuera** (P3b3b:
/// el interprete, o su traduccion a x86-64). La trama y el orden no cambian:
/// lo unico que cambia es QUIEN hace las cuentas de cada sombreador.
pub fn en_cpu_con(l: &Lote, destino: &mut trama::Destino, vs: Corre, ps: CorrePs) -> Result<trama::Cuenta, NoDibuja> {
    en_cpu_olas(l, destino, vs, ps, None)
}

/// E2.5: [`en_cpu_con`] con quien corre las OLAS de pixeles (`Some`: el de
/// pixeles usa las olas, y `ps` no se llama).
fn en_cpu_olas(l: &Lote, destino: &mut trama::Destino, vs: Corre, ps: CorrePs, olas: Option<crate::cuadros::Olas>) -> Result<trama::Cuenta, NoDibuja> {
    // Las limpiezas que la casa dejo a quien dibuje: aqui, la CPU.
    if let Some(p) = l.limpiar_rt {
        destino.pixeles.fill(p);
    }
    if let (Some(b), Some(z)) = (l.limpiar_z, destino.z.as_mut()) {
        z.fill(b);
    }
    let n_vertices = l.comprobar()?;
    let en = l.enlace;
    if let Some(g) = &en.gs {
        return en_cpu_gs(l, destino, vs, ps, g, n_vertices, olas);
    }
    let mut sombreados: Vec<trama::Sombreado> = Vec::new();
    let mut ent = vec![[0.0f32, 0.0, 0.0, 1.0]; en.desde_ia.len().max(en.vs.entradas)];
    let mut sal = vec![[0.0f32; 4]; en.vs.salidas];
    let tris = triangulos(l.ids, l.topologia);
    let mut locales = Vec::with_capacity(tris.len() * l.instancias.max(1) as usize);
    // N5.13: instancia a instancia, en orden (D3D12 las pinta asi): el de
    // vertices una vez por vertice distinto DE CADA UNA.
    for inst in 0..l.instancias {
        let mut hecho: Vec<Option<usize>> = vec![None; n_vertices];
        for t in &tris {
            let mut local = [0usize; 3];
            for (k, &id) in t.iter().enumerate() {
                let ranura = &mut hecho[id as usize];
                if let Some(i) = *ranura {
                    local[k] = i;
                    continue;
                }
                for (x, &fuente) in ent.iter_mut().zip(&en.desde_ia) {
                    *x = entrada(l, fuente, id, inst);
                }
                vs(&ent, &mut sal);
                let atributos = en.desde_vs.iter().map(|o| o.and_then(|k| sal.get(k).copied()).unwrap_or([0.0; 4])).collect();
                sombreados.push(trama::Sombreado { pos: sal[en.posicion], atributos });
                *ranura = Some(sombreados.len() - 1);
                local[k] = sombreados.len() - 1;
            }
            locales.push(local);
        }
    }
    // N5.8: cada salida del de pixeles, a su render target.
    let mut sal_ps = vec![[0.0f32; 4]; en.ps.salidas.max(en.objetivos.len())];
    // Con SV_Depth, la prueba de profundidad va despues del de pixeles.
    let reglas = trama::Reglas { z_del_sombreador: en.profundidad_ps.is_some(), ..l.reglas };
    if let Some(o) = olas {
        return Ok(trama::dibujar_en_olas(&reglas, efectos(en), &sombreados, &locales, destino, en.pos_ps, o));
    }
    Ok(trama::dibujar_con(&reglas, efectos(en), &sombreados, &locales, destino, en.pos_ps, |x, colores| {
        let queda = ps(x, &mut sal_ps);
        for (k, &t) in en.objetivos.iter().enumerate() {
            colores[t as usize] = sal_ps[k];
        }
        queda
    }))
}

/// Lo que su sombreador de pixeles hace ademas del color (05-10): con UAV,
/// cada pixel corre, en orden, y la Z se prueba despues (si no la pide antes).
fn efectos(en: &Enlace) -> trama::Efectos {
    trama::Efectos { uav: en.ps.toca_uav(), temprana: en.ps.computo.temprana }
}

/// **El dibujo con un sombreador de GEOMETRIA** (E2.3b, 05-10): el de
/// vertices una vez por vertice distinto (todas sus salidas guardadas), el
/// GS una vez por primitiva (punto, linea o triangulo, de la topologia del
/// lote), y sus tiras de triangulos a la trama, con el de pixeles. El GS va
/// siempre por el interprete.
fn en_cpu_gs(l: &Lote, destino: &mut trama::Destino, vs: Corre, ps: CorrePs, g: &EnlaceGs, n_vertices: usize, olas: Option<crate::cuadros::Olas>) -> Result<trama::Cuenta, NoDibuja> {
    let en = l.enlace;
    let por_vs = en.vs.salidas.max(1);
    let mut salidas_vs: Vec<[f32; 4]> = Vec::new();
    let mut ent = vec![[0.0f32, 0.0, 0.0, 1.0]; en.desde_ia.len().max(en.vs.entradas)];
    let mut sal = vec![[0.0f32; 4]; por_vs];
    // La topologia del lote tiene que dar lo que el GS lee (la casa lo mira
    // antes: en D3D12 es un error).
    let Some(prims) = primitivas(l.ids, l.topologia, g.info.vertices()) else {
        return Ok(trama::Cuenta::default());
    };
    let mut hecho: Vec<Option<usize>> = vec![None; n_vertices];
    let paso_gs = g.programa.entradas;
    let mut ent_gs = vec![[0.0f32; 4]; paso_gs * g.info.vertices()];
    let mut sal_gs = vec![[0.0f32; 4]; g.programa.salidas];
    let mut tiras = Tiras { salidas: g.programa.salidas, maximo: g.info.maximo as usize, ..Tiras::default() };
    let mut regs = Vec::new();
    let mut sombreados: Vec<trama::Sombreado> = Vec::new();
    let mut locales: Vec<[usize; 3]> = Vec::new();
    // N5.13: instancia a instancia, como sin GS.
    for inst in 0..l.instancias {
        hecho.fill(None);
        for prim in &prims {
                for (k, &id) in prim.iter().enumerate() {
                    let ranura = &mut hecho[id as usize];
                    let base = match *ranura {
                        Some(b) => b,
                        None => {
                            for (x, &fuente) in ent.iter_mut().zip(&en.desde_ia) {
                                *x = entrada(l, fuente, id, inst);
                            }
                            vs(&ent, &mut sal);
                            let b = salidas_vs.len();
                            salidas_vs.extend_from_slice(&sal);
                            *ranura = Some(b);
                            b
                        }
                    };
                    for (j, &o) in g.desde_vs.iter().enumerate().take(paso_gs) {
                        ent_gs[k * paso_gs + j] = salidas_vs.get(base + o).copied().unwrap_or([0.0; 4]);
                    }
                }
                sal_gs.fill([0.0; 4]);
                g.programa.correr_gs(&ent_gs, l.cb, &l.recursos, &mut sal_gs, &mut regs, &mut tiras);
                let primero = sombreados.len();
                for v in tiras.vertices.chunks_exact(tiras.salidas.max(1)) {
                    let atributos = en.desde_vs.iter().map(|o| o.and_then(|k| v.get(k).copied()).unwrap_or([0.0; 4])).collect();
                    sombreados.push(trama::Sombreado { pos: v[en.posicion], atributos });
                }
                locales.extend(tiras.triangulos().into_iter().map(|t| t.map(|i| primero + i as usize)));
        }
    }
    let mut sal_ps = vec![[0.0f32; 4]; en.ps.salidas.max(en.objetivos.len())];
    let reglas = trama::Reglas { z_del_sombreador: en.profundidad_ps.is_some(), ..l.reglas };
    if let Some(o) = olas {
        return Ok(trama::dibujar_en_olas(&reglas, efectos(en), &sombreados, &locales, destino, en.pos_ps, o));
    }
    Ok(trama::dibujar_con(&reglas, efectos(en), &sombreados, &locales, destino, en.pos_ps, |x, colores| {
        let queda = ps(x, &mut sal_ps);
        for (k, &t) in en.objetivos.iter().enumerate() {
            colores[t as usize] = sal_ps[k];
        }
        queda
    }))
}
