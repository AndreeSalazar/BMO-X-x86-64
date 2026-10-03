//! **P3b4a: UN PSO PARA LA 3060, PAGANDO UNA VEZ** -- lo que la casa enlazo
//! (`lote::Enlace`: los dos sombreadores y como se cosen) y el input layout
//! del juego, a los dos programas que sube el kernel (SPH y codigo), con el
//! pegamento de VERRANO y JUZGADOS. El [`Almacen`] los guarda por PSO y paso
//! de vertice: la segunda vez no se traduce ni se juzga nada.
//!
//! ```text
//!    Enlace.vs/ps  --emitir_con(Abi::Registros)-->  cuerpo + precargas
//!    input layout  --------------------------->     Datos (desde, componentes)
//!    Enlace.desde_vs, posicion  -------------->     el generico de cada entrada
//!                  --pegamento::vertice/pixel-->  SPH + codigo
//!                  --juez::juzgar_programa----->  PERFECTO, o no se sube
//! ```
//!
//! Lo que la CPU hace en cada dibujo es COPIAR: el cbuffer y el bufer de
//! vertices del juego tal cual (`Datos`). Dirige; no dibuja.

use alloc::string::String;
use alloc::vec::Vec;

use bmo_gpu_ga10x::pegamento::{self, Carga, Datos, Elemento, NoPega};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::tuberia;
use bmo_proton_x::lote::{componentes, ElementoIa, Enlace, Fuente};

use crate::{emitir_con, Abi, Emitido, NoEmite, Precarga};

/// Por que un PSO no va a la 3060 (y se dibuja en la CPU, y se dice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoVa {
    /// El emisor no sabe (una operacion, o no caben los registros).
    Emisor(&'static str, NoEmite),
    /// El pegamento no puede (una carga, las salidas, no cabe).
    Pegamento(&'static str, NoPega),
    /// Un elemento de entrada que no viene de la ranura 0 (hoy solo esa).
    Ranura(u32),
    /// Una entrada que la 3060 no lee todavia (03-10): va por la CPU.
    Entrada(&'static str),
    /// El juez dijo BODRIO: nunca llega a la 3060.
    Juez(&'static str, String),
}

/// Los dos programas, listos para el paquete, y como van los DATOS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParaLa3060 {
    pub vs: Vec<u8>,
    pub ps: Vec<u8>,
    /// Filas del cbuffer delante de los vertices en los DATOS.
    pub filas: u32,
    /// Bytes de un vertice (el del juego).
    pub paso: u32,
    /// Instrucciones que el juez miro, las dos.
    pub instrucciones: usize,
}

impl ParaLa3060 {
    /// Los DATOS de un dibujo en `out`: `filas` filas del cbuffer (a ceros lo
    /// que no traiga) y detras los vertices TAL CUAL. Cuantos bytes, o `None`
    /// si no caben o los vertices no son enteros.
    pub fn datos(&self, cb: &[u8], vertices: &[u8], out: &mut [u8]) -> Option<usize> {
        let c = 16 * self.filas as usize;
        let n = c + vertices.len();
        if self.paso == 0 || vertices.len() % self.paso as usize != 0 || n > out.len() {
            return None;
        }
        let k = cb.len().min(c);
        out[..k].copy_from_slice(&cb[..k]);
        out[k..c].fill(0);
        out[c..n].copy_from_slice(vertices);
        Some(n)
    }
}

pub(crate) fn cargas(e: &Emitido) -> Vec<Carga> {
    let pares = texturas_de(e);
    e.precargas
        .iter()
        .map(|p| match *p {
            Precarga::Entrada { elemento, componente, reg } => Carga::Entrada { elemento, componente, reg },
            Precarga::Fila { fila, reg } => Carga::Fila { fila, reg },
            // La pareja (tN, sM) es la textura k de la receta: su puesto.
            Precarga::Asa { textura, muestreador, reg } => Carga::Asa { textura: pares.iter().position(|&x| x == (textura, muestreador)).unwrap_or(0) as u8, reg },
        })
        .collect()
}

/// **Las texturas de un programa emitido**, en el orden de la receta: la
/// textura k es la pareja `(tN, sM)` de su k-esima asa.
pub fn texturas_de(e: &Emitido) -> Vec<(u8, u8)> {
    e.precargas
        .iter()
        .filter_map(|p| match *p {
            Precarga::Asa { textura, muestreador, .. } => Some((textura, muestreador)),
            _ => None,
        })
        .collect()
}

fn bytes(p: &pegamento::Pegado) -> Vec<u8> {
    let mut b = alloc::vec![0u8; tuberia::HUECO];
    let n = p.bytes(&mut b);
    b.truncate(n);
    b
}

/// Como estan los elementos de entrada del de vertice en un vertice del
/// juego (del input layout, por `Enlace::desde_ia`).
pub fn elementos(en: &Enlace, ia: &[ElementoIa]) -> Result<Vec<Elemento>, NoVa> {
    en.desde_ia
        .iter()
        .map(|&f| {
            // 03-10: el numero de vertice o de instancia, y los formatos que
            // no son floats de 32 bits, los lee la CPU (`lote::entrada`):
            // aqui se dice que no, y el dibujo va por ella.
            let Fuente::Ia(i) = f else { return Err(NoVa::Entrada("un valor de sistema (SV_VertexID o SV_InstanceID)")) };
            let e = &ia[i];
            if e.ranura != 0 {
                return Err(NoVa::Ranura(e.ranura));
            }
            if !bmo_proton_x::formato_ia::es_float32(e.formato) {
                return Err(NoVa::Entrada("un formato de vertice que no es float de 32 bits"));
            }
            Ok(Elemento { desde: e.desde, componentes: componentes(e.formato) as u8 })
        })
        .collect()
}

/// **Traducir un PSO** para un paso de vertice. Emite, pega y JUZGA.
pub fn traducir(en: &Enlace, ia: &[ElementoIa], paso: u32) -> Result<ParaLa3060, NoVa> {
    let ev = emitir_con(&en.vs, 64, Abi::Registros).map_err(|e| NoVa::Emisor("vertice", e))?;
    let ep = emitir_con(&en.ps, 64, Abi::Registros).map_err(|e| NoVa::Emisor("pixel", e))?;
    let els = elementos(en, ia)?;
    let filas = en.vs.filas_cb.max(en.ps.filas_cb) as u32;
    let datos = Datos { filas, paso, elementos: &els };
    // N5.9: el de pixeles que lee SV_Position, por la CPU todavia (la 3060
    // la da en un atributo de sistema que el pegamento no pone).
    if en.pos_ps.is_some() {
        return Err(NoVa::Entrada("SV_Position en el de pixeles"));
    }
    let posicion = en.posicion as u32;
    let v = pegamento::vertice(&ev.codigo, ev.registros, &cargas(&ev), datos, en.vs.salidas as u32, posicion).map_err(|e| NoVa::Pegamento("vertice", e))?;
    let genericos: Vec<Option<u8>> = en.desde_vs.iter().map(|o| o.and_then(|o| pegamento::generico(o as u32, posicion))).collect();
    let p = pegamento::pixel(&ep.codigo, ep.registros, &cargas(&ep), datos, &genericos).map_err(|e| NoVa::Pegamento("pixel", e))?;
    let (vs, ps) = (bytes(&v), bytes(&p));
    let juzgar = |cual: &'static str, b: &[u8]| juez::juzgar_programa(b, tuberia::REGISTROS).map_err(|x| NoVa::Juez(cual, alloc::format!("{x}")));
    let instrucciones = juzgar("vertice", &vs)?.instrucciones + juzgar("pixel", &ps)?.instrucciones;
    Ok(ParaLa3060 { vs, ps, filas, paso, instrucciones })
}

/// **El almacen**: cada PSO (su `Enlace`, que vive lo que el proceso) y paso,
/// traducido UNA vez -- bien o con su motivo, que tampoco se repite.
#[derive(Default)]
pub struct Almacen {
    guardados: Vec<(usize, u32, Result<ParaLa3060, NoVa>)>,
    /// Cuantas traducciones se hicieron de verdad (las demas, del almacen).
    pub traducciones: usize,
}

impl Almacen {
    pub const fn nuevo() -> Self {
        Almacen { guardados: Vec::new(), traducciones: 0 }
    }

    /// Lo de este PSO y paso: del almacen, o traducido ahora y guardado.
    pub fn dar(&mut self, en: &Enlace, ia: &[ElementoIa], paso: u32) -> &Result<ParaLa3060, NoVa> {
        let clave = en as *const Enlace as usize;
        let i = match self.guardados.iter().position(|g| g.0 == clave && g.1 == paso) {
            Some(i) => i,
            None => {
                self.traducciones += 1;
                self.guardados.push((clave, paso, traducir(en, ia, paso)));
                self.guardados.len() - 1
            }
        };
        &self.guardados[i].2
    }
}
