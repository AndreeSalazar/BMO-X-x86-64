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
use crate::dxil::Sombreador;
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
    /// le llega (`None`: SV_Position, que pone la trama).
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

/// **Aplanar los cbuffers de los dos**: uno detras de otro, cada uno con las
/// filas que se leen de el. El interprete, la 3060 y el x86 ven UN cbuffer;
/// solo quien dibuja sabe que son varios.
fn aplanar(vs: &mut Programa, ps: &mut Programa, n: usize) -> Result<Vec<Bloque>, String> {
    let mut filas = vec![0u16; n];
    vs.filas_por_cbuffer(&mut filas);
    ps.filas_por_cbuffer(&mut filas);
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
    vs.aplanar(&bases);
    ps.aplanar(&bases);
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
    let mut pv = programa::compilar(vs).map_err(|e| format!("el sombreador de vertices no se sabe correr todavia: {e:?}"))?;
    let mut pp = match ps {
        Some(ps) => programa::compilar(ps).map_err(|e| format!("el sombreador de pixeles no se sabe correr todavia: {e:?}"))?,
        None => Programa::vacio(),
    };
    let (ps_entradas, ps_salidas) = ps.map_or((&[][..], &[][..]), |p| (&p.entradas[..], &p.salidas[..]));
    // Una tabla para los dos: las del de vertices se quedan donde estan y las
    // del de pixeles se renumeran a la suya (la misma si los dos la leen).
    // Cada lugar, con su etapa: el t0 de uno no es el t0 del otro si la root
    // signature les da tablas distintas (`donde::en_tabla`).
    let mut ranuras = pv.ranuras.clone().de_la_etapa(crate::donde::VISTA_VERTICES);
    let mapa = ranuras.unir(&pp.ranuras.clone().de_la_etapa(crate::donde::VISTA_PIXELES)).map_err(|e| format!("los recursos de los dos sombreadores: {e:?}"))?;
    pp.renumerar(&mapa);
    pp.ranuras = ranuras.clone();
    pv.ranuras = ranuras.clone();
    let constantes = aplanar(&mut pv, &mut pp, ranuras.cbuffers.len())?;
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
        let k = vs.salidas.iter().position(|o| o.semantica.eq_ignore_ascii_case(&f.semantica) && o.indice == f.indice);
        desde_vs.push(Some(k.ok_or_else(|| format!("el sombreador de pixeles lee {}{} y el de vertices no lo escribe", f.semantica, f.indice))?));
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
    Ok(Enlace { vs: pv, ps: pp, desde_ia, posicion, desde_vs, pos_ps, objetivos, profundidad_ps, ranuras, constantes })
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

/// El de pixeles, igual, diciendo si el pixel QUEDA (`false`: un `discard`
/// lo tiro, N5.7).
pub type CorrePs<'a> = &'a mut dyn FnMut(&[[f32; 4]], &mut [[f32; 4]]) -> bool;

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
    let mut vs = |e: &[[f32; 4]], s: &mut [[f32; 4]]| {
        en.vs.correr_con(e, l.cb, &l.recursos, s, &mut rv);
    };
    en_cpu_con(l, destino, &mut vs, &mut |e, s| en.ps.correr_con(e, l.cb, &l.recursos, s, &mut rp))
}

/// **Lo mismo, con quien corre los sombreadores puesto desde fuera** (P3b3b:
/// el interprete, o su traduccion a x86-64). La trama y el orden no cambian:
/// lo unico que cambia es QUIEN hace las cuentas de cada sombreador.
pub fn en_cpu_con(l: &Lote, destino: &mut trama::Destino, vs: Corre, ps: CorrePs) -> Result<trama::Cuenta, NoDibuja> {
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
    // N5.8: cada salida del de pixeles, a su render target.
    let mut sal_ps = vec![[0.0f32; 4]; en.ps.salidas.max(en.objetivos.len())];
    // Con SV_Depth, la prueba de profundidad va despues del de pixeles.
    let reglas = trama::Reglas { z_del_sombreador: en.profundidad_ps.is_some(), ..l.reglas };
    Ok(trama::dibujar(&reglas, &sombreados, &locales, destino, en.pos_ps, |x, colores| {
        let queda = ps(x, &mut sal_ps);
        for (k, &t) in en.objetivos.iter().enumerate() {
            colores[t as usize] = sal_ps[k];
        }
        queda
    }))
}
