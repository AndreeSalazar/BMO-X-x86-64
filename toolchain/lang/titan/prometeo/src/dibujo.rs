//! **LA gpu fn QUE DIBUJA** (LB6 de `docs/plan/PLAN_LAS_LIBRERIAS.md`) -- la
//! de VERTICE y la de PIXEL, dichas por su FIRMA (`bmo_titan_front::gpu::
//! Forma`, DL6 tomada como recomienda el plan), hechas el Programa de la
//! casa: el mismo que sale de los DXIL de PROTON-X, por elementos y
//! componentes, el que pega cada tarjeta en su tuberia.
//!
//! ```text
//!    lo que recibe   UN registro: cada campo, un ELEMENTO de entrada; cada
//!                    f32 de un campo, un COMPONENTE (de 1 a 4)
//!                    el de pixel no lee su `posicion` (SV_Position: con ella
//!                    la tarjeta sabe que pixel pinta); su sitio, un cero
//!    lo que deja     el de vertice: cada campo, un elemento de salida -- uno
//!                    es su `posicion` --; el de pixel: su COLOR, el elemento 0
//!                    un componente que el campo no tiene sale 0: cada salida
//!                    dice sus cuatro, siempre los mismos bits
//! ```
//!
//! El cuerpo lo escribe el escritor de siempre (`escribe.rs`), hoja a hoja:
//! un registro de dibujo es un `Valor` de varias hojas, con sus `if`, sus
//! bucles y sus casas como cualquier f32.
//!
//! ** TRES RESPUESTAS, como en una celda: cada elemento que deja, por el
//! simulador de cada tarjeta (sobre su codigo), por el interprete de la casa
//! (sobre el Programa) y por el calculo de TITAN++ (`calc::run_gpu_dibujo`,
//! sobre la IR). Si dos no dan los mismos bits en un componente, no hay
//! `.bex`.

use bmo_prometeo::programa::Op;
use bmo_prometeo::Programa;
use bmo_titan_front::gpu::{campos, Forma};
use bmo_titan_front::ir::Module;

use crate::escribe::{straight, valor_de, Env, Writer};
use crate::{battery_de, same_cell, Failure, Kernel, Kind};

/// ** LO QUE DIBUJA UNA gpu fn, por elementos: lo que la tarjeta necesita
/// para ponerla en su tuberia (el pegamento de la RTX 3060 12G, E5) y el
/// oraculo para correrla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dibujo {
    /// De vertice o de pixel, y donde va su `posicion`.
    pub forma: Forma,
    /// El tipo que recibe y el que deja (sus nombres en el `.titan`): el de
    /// pixel de un par recibe lo que deja el de vertice.
    pub entra: String,
    pub sale: String,
    /// Los componentes de cada elemento que recibe, por su numero (uno por
    /// campo); y los de cada uno que deja.
    pub entradas: Vec<usize>,
    pub salidas: Vec<usize>,
}

impl Dibujo {
    /// El elemento de su `posicion`: el que deja la de vertice, el que recibe
    /// (y no lee) la de pixel.
    pub fn posicion(&self) -> usize {
        match self.forma {
            Forma::Vertice { posicion } | Forma::Pixel { posicion } => posicion,
            Forma::Celda => usize::MAX,
        }
    }
}

/// **La gpu fn que dibuja `func` de `m`, hecha el Programa de la casa**: el
/// Programa, el sitio del `.titan` de cada operacion, y lo que dibuja.
pub(crate) fn programa(m: &Module, func: usize, forma: Forma) -> Result<(Programa, Vec<(usize, usize)>, Dibujo), Failure> {
    let f = &m.functions[func];
    let mal = |que: &str| Failure::writer(format!("`gpu fn {}` dibuja {}: gpu.rs tenia que haberlo dicho", f.name, que));
    let [(local, entra)] = &f.params[..] else { return Err(mal("sin su registro")) };
    let sale = f.ret.as_ref().ok_or_else(|| mal("sin devolver nada"))?;
    let (Some(ce), Some(cs)) = (campos(&m.types, entra), campos(&m.types, sale)) else { return Err(mal("con algo que no es un registro de dibujo")) };
    let no_lee = match forma {
        Forma::Pixel { posicion } => Some(posicion),
        Forma::Vertice { .. } => None,
        Forma::Celda => return Err(mal("y es una celda")),
    };
    let mut w = Writer::nuevo(m, func);
    // -- lo que recibe: cada campo un elemento, cada f32 un componente -----------------
    let mut hojas = Vec::new();
    for (e, (_, k)) in ce.iter().enumerate() {
        for c in 0..*k {
            if Some(e) == no_lee {
                // La posicion del de pixel es de la tarjeta: no se lee.
                hojas.push(w.bits(0)?);
            } else {
                let d = w.reg()?;
                w.op(Op::Entrada { d, elemento: e as u8, componente: c as u8 });
                hojas.push(d);
            }
        }
    }
    let mut env = Env::new();
    env.insert(*local, valor_de(m, entra, hojas)?);
    let r = straight(&mut w, f, env)?;
    // -- lo que deja: cada elemento con sus cuatro componentes ------------------------
    w.aqui = (f.line, 1);
    let salidas: Vec<usize> = match forma {
        Forma::Pixel { .. } => vec![r.len()],
        _ => cs.iter().map(|(_, k)| *k).collect(),
    };
    let mut hoja = r.iter();
    for (e, k) in salidas.iter().enumerate() {
        for c in 0..4u8 {
            let s = match (c as usize) < *k {
                true => *hoja.next().ok_or_else(|| mal("con menos hojas de las que deja"))?,
                false => w.bits(0)?,
            };
            w.op(Op::Salida { s, elemento: e as u8, componente: c });
        }
    }
    let lee = (0..ce.len()).filter(|e| Some(*e) != no_lee).fold(0u32, |m, e| m | 1 << e);
    let p = Programa { ops: w.ops, iniciales: w.iniciales, entradas: ce.len(), salidas: salidas.len(), lee, filas_cb: 0, ranuras: Default::default(), computo: Default::default() };
    let dibujo = Dibujo { forma, entra: entra.name(), sale: sale.name(), entradas: ce.iter().map(|(_, k)| *k).collect(), salidas };
    Ok((p, w.donde, dibujo))
}

// ---- el oraculo de lo que dibuja ------------------------------------------------------

/// Una celda de lo que dibuja: un `[u32; 4]` por elemento (sus bits).
pub type Celda = Vec<[u32; 4]>;

/// **La bateria de lo que dibuja**: cada componente de cada elemento recorre
/// los bordes a su propio paso (la de las celdas, con un valor por
/// componente), en tantas celdas como deje la obra.
pub fn bateria(d: &Dibujo, obra: u64) -> Vec<Celda> {
    let hojas: usize = d.entradas.iter().sum();
    let columnas = battery_de(&vec![Kind::F32; hojas], obra);
    let n = columnas.first().map(|c| c.len()).unwrap_or(0);
    (0..n)
        .map(|i| {
            let mut j = 0;
            d.entradas
                .iter()
                .map(|k| {
                    let mut e = [0u32; 4];
                    for x in e.iter_mut().take(*k) {
                        *x = columnas[j][i];
                        j += 1;
                    }
                    e
                })
                .collect()
        })
        .collect()
}

/// **El ORACULO de lo que dibuja**: el codigo de la tarjeta, corrido por SU
/// simulador, un hilo por celda; lo que deja, por elementos.
pub fn run(k: &Kernel, d: &Dibujo, celdas: &[Celda]) -> Result<Vec<Celda>, String> {
    celdas.iter().map(|e| k.tarjeta.simular(&k.oraculo, e, d.salidas.len())).collect()
}

/// Lo mismo por el INTERPRETE de la casa, sobre el Programa.
pub fn run_casa(k: &Kernel, d: &Dibujo, celdas: &[Celda]) -> Vec<Celda> {
    let mut regs = Vec::new();
    celdas
        .iter()
        .map(|e| {
            let ent: Vec<[f32; 4]> = e.iter().map(|c| c.map(f32::from_bits)).collect();
            let mut s = vec![[0.0f32; 4]; d.salidas.len()];
            k.programa.correr(&ent, &[], &mut s, &mut regs);
            s.iter().map(|c| c.map(f32::to_bits)).collect()
        })
        .collect()
}

/// **La comparacion de cada build**, componente a componente: la tarjeta, la
/// casa y el calculo; la primera que no da lo mismo, dicha con su entrada.
pub(crate) fn compare(m: &Module, func: usize, k: &Kernel, d: &Dibujo, celdas: &[Celda], what: &str) -> Result<Vec<Celda>, String> {
    let suya = run(k, d, celdas)?;
    let casa = run_casa(k, d, celdas);
    let calc = bmo_titan_front::calc::run_gpu_dibujo(m, func, celdas).map_err(|e| format!("el calculo: {}", e.what))?;
    let ver = |c: &Celda| c.iter().map(|e| format!("({})", e.iter().map(|b| format!("{:?}", f32::from_bits(*b))).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join(" ");
    for i in 0..celdas.len() {
        let (a, b, c) = (&suya[i], &casa[i], calc.get(i).ok_or_else(|| format!("el calculo no dio la celda {}", i))?);
        let igual = |x: &Celda, y: &Celda| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.iter().zip(q).all(|(u, v)| same_cell(*u, *v, Kind::F32)));
        if !igual(a, c) || !igual(b, c) {
            return Err(format!(
                "{}, linea {} (gpu fn `{}`): con {} {} {} deja {}, la casa {} y el calculo {} -- una cuenta, varias respuestas: no hay .bex",
                k.file, k.line, k.name, what, ver(&celdas[i]), k.tarjeta.ficha().nombre, ver(a), ver(b), ver(c)
            ));
        }
    }
    Ok(suya)
}

/// **La bateria de bordes** de una gpu fn que dibuja, por los tres.
pub(crate) fn verify(m: &Module, func: usize, k: &Kernel, d: &Dibujo) -> Result<usize, String> {
    let celdas = bateria(d, m.functions[func].obra);
    compare(m, func, k, d, &celdas, "la bateria de bordes")?;
    Ok(celdas.len())
}
