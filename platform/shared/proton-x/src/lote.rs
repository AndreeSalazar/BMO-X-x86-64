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

use crate::dxil::programa::{self, Programa};
use crate::dxil::Sombreador;
use crate::trama;

pub use crate::formato_ia::{FMT_R32G32B32A32_FLOAT, FMT_R32G32B32_FLOAT, FMT_R32G32_FLOAT, FMT_R32_FLOAT};

/// SV_Position en una firma (valor de sistema 1).
const SV_POSITION: u32 = 1;
/// SV_VertexID y SV_InstanceID (`D3D_NAME` 6 y 8): no vienen del input layout,
/// los pone quien dibuja.
const SV_VERTEXID: u32 = 6;
const SV_INSTANCEID: u32 = 8;

/// **De donde sale una entrada del sombreador de vertices** (03-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuente {
    /// Del elemento `i` del input layout.
    Ia(usize),
    /// SV_VertexID: el numero del vertice (el id, con el vertice base).
    Vertice,
    /// SV_InstanceID: el numero de la instancia (el lote dibuja la 0).
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
    /// La salida del de vertices que es SV_Position.
    pub posicion: usize,
    /// Por elemento de entrada del de pixeles: la salida del de vertices que
    /// le llega (`None`: SV_Position, que hoy no lee).
    pub desde_vs: Vec<Option<usize>>,
}

/// **Coser** los dos sombreadores con el input layout. El texto dice por que
/// no, si no.
pub fn enlazar(vs: &Sombreador, ps: &Sombreador, entradas: &[ElementoIa]) -> Result<Enlace, String> {
    let pv = programa::compilar(vs).map_err(|e| format!("el sombreador de vertices no se sabe correr todavia: {e:?}"))?;
    let pp = programa::compilar(ps).map_err(|e| format!("el sombreador de pixeles no se sabe correr todavia: {e:?}"))?;
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
    let posicion = vs.salidas.iter().position(|f| f.sistema == SV_POSITION).ok_or_else(|| String::from("el sombreador de vertices no escribe SV_Position"))?;
    let mut desde_vs = Vec::with_capacity(ps.entradas.len());
    for (i, f) in ps.entradas.iter().enumerate() {
        if f.sistema == SV_POSITION {
            if pp.lee & (1 << i) != 0 {
                return Err(String::from("el sombreador de pixeles lee SV_Position: todavia no"));
            }
            desde_vs.push(None);
            continue;
        }
        let k = vs.salidas.iter().position(|o| o.semantica.eq_ignore_ascii_case(&f.semantica) && o.indice == f.indice);
        desde_vs.push(Some(k.ok_or_else(|| format!("el sombreador de pixeles lee {}{} y el de vertices no lo escribe", f.semantica, f.indice))?));
    }
    if pp.salidas != 1 {
        return Err(String::from("el sombreador de pixeles escribe mas de un render target: todavia no"));
    }
    Ok(Enlace { vs: pv, ps: pp, desde_ia, posicion, desde_vs })
}

/// Como se agrupan los ids en triangulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topologia {
    Lista,
    /// Cada vertice nuevo con los dos anteriores; los impares, dados la vuelta.
    Tira,
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
    /// Los bytes del cbuffer b0 que leen los sombreadores.
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
    }
}

/// Corre un sombreador: entradas -> salidas (el cbuffer lo lleva dentro).
pub type Corre<'a> = &'a mut dyn FnMut(&[[f32; 4]], &mut [[f32; 4]]);

/// **Una entrada del de vertices** para el vertice `id` (de bytes `v`): el
/// elemento del layout con su formato (`formato_ia`), o el numero de
/// vertice o de instancia, como ENTERO en los bits del registro.
pub fn entrada(l: &Lote, fuente: Fuente, id: u32, v: &[u8]) -> [f32; 4] {
    let entero = |n: u32| [f32::from_bits(n), 0.0, 0.0, 0.0];
    match fuente {
        Fuente::Vertice => entero(id),
        Fuente::Instancia => entero(0),
        Fuente::Ia(i) => {
            let el = &l.entradas[i];
            // Solo la ranura 0 llega al lote: lo de otra, como si no hubiera.
            if el.ranura != 0 {
                return [0.0, 0.0, 0.0, 1.0];
            }
            crate::formato_ia::leer(el.formato, v.get(el.desde as usize..).unwrap_or(&[]))
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
    en_cpu_con(l, destino, &mut |e, s| en.vs.correr_con(e, l.cb, &l.recursos, s, &mut rv), &mut |e, s| en.ps.correr_con(e, l.cb, &l.recursos, s, &mut rp))
}

/// **Lo mismo, con quien corre los sombreadores puesto desde fuera** (P3b3b:
/// el interprete, o su traduccion a x86-64). La trama y el orden no cambian:
/// lo unico que cambia es QUIEN hace las cuentas de cada sombreador.
pub fn en_cpu_con(l: &Lote, destino: &mut trama::Destino, vs: Corre, ps: Corre) -> Result<trama::Cuenta, NoDibuja> {
    // Las limpiezas que la casa dejo a quien dibuje: aqui, la CPU.
    if let Some(p) = l.limpiar_rt {
        destino.pixeles.fill(p);
    }
    if let (Some(b), Some(z)) = (l.limpiar_z, destino.z.as_mut()) {
        z.fill(b);
    }
    if l.paso == 0 {
        return Err(NoDibuja::SinVertices);
    }
    let en = l.enlace;
    let n_vertices = l.vertices.len() / l.paso;
    let mut hecho: Vec<Option<usize>> = vec![None; n_vertices];
    let mut sombreados: Vec<trama::Sombreado> = Vec::new();
    let mut ent = vec![[0.0f32, 0.0, 0.0, 1.0]; en.desde_ia.len().max(en.vs.entradas)];
    let mut sal = vec![[0.0f32; 4]; en.vs.salidas];
    let tris = triangulos(l.ids, l.topologia);
    let mut locales = Vec::with_capacity(tris.len());
    for t in &tris {
        let mut local = [0usize; 3];
        for (k, &id) in t.iter().enumerate() {
            let ranura = hecho.get_mut(id as usize).ok_or(NoDibuja::IndiceFuera(id))?;
            if let Some(i) = *ranura {
                local[k] = i;
                continue;
            }
            let v = &l.vertices[id as usize * l.paso..(id as usize + 1) * l.paso];
            for (x, &fuente) in ent.iter_mut().zip(&en.desde_ia) {
                *x = entrada(l, fuente, id, v);
            }
            vs(&ent, &mut sal);
            let atributos = en.desde_vs.iter().map(|o| o.and_then(|k| sal.get(k).copied()).unwrap_or([0.0; 4])).collect();
            sombreados.push(trama::Sombreado { pos: sal[en.posicion], atributos });
            *ranura = Some(sombreados.len() - 1);
            local[k] = sombreados.len() - 1;
        }
        locales.push(local);
    }
    let mut sal_ps = [[0.0f32; 4]; 1];
    Ok(trama::dibujar(&l.reglas, &sombreados, &locales, destino, |x| {
        ps(x, &mut sal_ps);
        sal_ps[0]
    }))
}
