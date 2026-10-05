//! **Lo que el emisor mira ANTES de emitir un programa que salta** (E6,
//! 02-10): que registro vive hasta donde, cual es una variable y que
//! comparacion va directa al predicado.
//!
//! ```text
//!    VARIABLE    un registro del Programa escrito mas de una vez, o escrito
//!                dentro de una rama y leido fuera de ella (o antes de
//!                escribirse: lo de la vuelta anterior). Tiene UN registro de
//!                la 3060 todo el programa, puesto al empezar a su valor
//!                inicial -- el que tiene en el interprete por un camino que
//!                no lo escribe --
//!    ULTIMO      su ultima lectura; si esta vivo en la cabeza de un bucle
//!                (definido antes, o variable), hasta el FinBucle: la vuelta
//!                de atras lo vuelve a leer
//!    FUNDIBLE    una Compara cuyo unico lector es la operacion de detras
//!                (un Si, un RomperSi o un Elige): su resultado no va a un
//!                registro, va a P0
//! ```
//!
//! Entrada, Constantes (y la precarga) no son variables nunca: lo que leen no
//! cambia en todo el programa.

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, Programa, Reg};

/// Lo que se sabe de un programa antes de emitirlo.
pub(crate) struct Analisis {
    /// El ultimo indice de operacion en que cada registro tiene que vivir.
    pub ultimo: Vec<usize>,
    pub variable: Vec<bool>,
    /// Que registros se sueltan tras cada operacion (los de `ultimo == i`).
    pub muere: Vec<Vec<Reg>>,
    /// Cuantas estructuras (si, bucle) hay abiertas al correr cada operacion.
    pub hondo: Vec<usize>,
    pub fundible: Vec<bool>,
}

/// Los registros del Programa que lee una operacion.
pub(crate) fn leidos(op: &Op) -> [Option<Reg>; 8] {
    let mut v = [None; 8];
    match *op {
        Op::Salida { s, .. } => v[0] = Some(s),
        Op::Mul { a, b, .. } | Op::Add { a, b, .. } | Op::Sub { a, b, .. } | Op::Div { a, b, .. } | Op::Min { a, b, .. } | Op::Max { a, b, .. } | Op::Compara { a, b, .. } | Op::SumaEntera { a, b, .. } | Op::Entera { a, b, .. } => {
            v[0] = Some(a);
            v[1] = Some(b);
        }
        Op::Mad { a, b, c, .. } => {
            v[0] = Some(a);
            v[1] = Some(b);
            v[2] = Some(c);
        }
        Op::Elige { c, a, b, .. } => {
            v[0] = Some(c);
            v[1] = Some(a);
            v[2] = Some(b);
        }
        Op::Dot { n, a, b, .. } => {
            for j in 0..n as usize {
                v[2 * j] = Some(a[j]);
                v[2 * j + 1] = Some(b[j]);
            }
        }
        Op::Rsqrt { a, .. } | Op::Sqrt { a, .. } | Op::Saturate { a, .. } | Op::Abs { a, .. } | Op::Mate { a, .. } | Op::Copia { a, .. } | Op::Convierte { a, .. } => v[0] = Some(a),
        Op::Muestra { u, v: vv, .. } => {
            v[0] = Some(u);
            v[1] = Some(vv);
        }
        Op::Lee { c, nivel, .. } => {
            for (k, r) in c.into_iter().enumerate() {
                v[k] = Some(r);
            }
            v[4] = Some(nivel);
        }
        Op::Si { c } | Op::RomperSi { c, .. } | Op::Descarta { c } => v[0] = Some(c),
        Op::LeeIndexado { i, .. } | Op::ConstantesEn { i, .. } | Op::EligeTextura { i, .. } => v[0] = Some(i),
        Op::EscribeIndexado { i, s, .. } => {
            v[0] = Some(i);
            v[1] = Some(s);
        }
        Op::Entrada { .. } | Op::Constantes { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::Romper | Op::Continuar | Op::FinBucle => {}
    }
    v
}

/// Los registros del Programa que escribe una operacion (`d`, o `d..d+4`).
pub(crate) fn escritos(op: &Op) -> ([Option<Reg>; 4], bool) {
    let uno = |d: Reg| ([Some(d), None, None, None], false);
    match *op {
        // Lo que lee algo que no cambia: nunca variable.
        Op::Entrada { d, .. } => ([Some(d), None, None, None], true),
        Op::Constantes { d, .. } => ([Some(d), Some(d + 1), Some(d + 2), Some(d + 3)], true),
        Op::ConstantesEn { d, .. } => ([Some(d), Some(d + 1), Some(d + 2), Some(d + 3)], false),
        Op::Muestra { d, .. } | Op::Lee { d, .. } => ([Some(d), Some(d + 1), Some(d + 2), Some(d + 3)], false),
        Op::Mul { d, .. }
        | Op::Add { d, .. }
        | Op::Sub { d, .. }
        | Op::Div { d, .. }
        | Op::Mad { d, .. }
        | Op::Dot { d, .. }
        | Op::Rsqrt { d, .. }
        | Op::Sqrt { d, .. }
        | Op::Saturate { d, .. }
        | Op::Abs { d, .. }
        | Op::Mate { d, .. }
        | Op::Min { d, .. }
        | Op::Max { d, .. }
        | Op::Compara { d, .. }
        | Op::Elige { d, .. }
        | Op::Copia { d, .. }
        | Op::SumaEntera { d, .. }
        | Op::Entera { d, .. }
        | Op::Convierte { d, .. }
        | Op::LeeIndexado { d, .. } => uno(d),
        // EligeTextura no escribe registros: escoge la textura de la lectura de detras.
        Op::Salida { .. } | Op::Descarta { .. } | Op::EscribeIndexado { .. } | Op::EligeTextura { .. } | Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle => ([None; 4], false),
    }
}

/// **Analizar** `p` (con su forma ya comprobada).
pub(crate) fn analizar(p: &Programa) -> Analisis {
    let n = p.iniciales.len();
    let m = p.ops.len();
    // Las regiones: la rama del si, la del SiNo, el cuerpo del bucle; y la
    // mas interna de cada operacion. Y los bucles (cabeza, fin).
    let mut regiones: Vec<(usize, usize)> = Vec::new();
    let mut region_de = vec![None::<usize>; m];
    let mut bucles: Vec<(usize, usize)> = Vec::new();
    let mut pila: Vec<(usize, bool)> = Vec::new();
    let mut hondo = vec![0usize; m];
    for (i, op) in p.ops.iter().enumerate() {
        region_de[i] = pila.last().map(|x| x.0);
        hondo[i] = pila.len();
        match op {
            Op::Si { .. } => {
                regiones.push((i, m));
                pila.push((regiones.len() - 1, false));
            }
            Op::Bucle => {
                regiones.push((i, m));
                pila.push((regiones.len() - 1, true));
            }
            Op::SiNo => {
                if let Some((r, _)) = pila.pop() {
                    regiones[r].1 = i;
                }
                regiones.push((i, m));
                pila.push((regiones.len() - 1, false));
            }
            Op::FinSi | Op::FinBucle => {
                if let Some((r, es_bucle)) = pila.pop() {
                    regiones[r].1 = i;
                    if es_bucle {
                        bucles.push(regiones[r]);
                    }
                }
            }
            _ => {}
        }
    }
    let mut lecturas: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut escrituras: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut inmutable = vec![false; n];
    for (i, op) in p.ops.iter().enumerate() {
        for &r in leidos(op).iter().flatten() {
            if let Some(l) = lecturas.get_mut(r as usize) {
                l.push(i);
            }
        }
        let (w, fija) = escritos(op);
        for &r in w.iter().flatten() {
            if let Some(e) = escrituras.get_mut(r as usize) {
                e.push(i);
                inmutable[r as usize] |= fija;
            }
        }
    }
    let mut variable = vec![false; n];
    for r in 0..n {
        let (l, e) = (&lecturas[r], &escrituras[r]);
        variable[r] = !inmutable[r]
            && match e.as_slice() {
                [] => false,
                [w] => {
                    let w = *w;
                    l.iter().any(|&x| x < w)
                        || region_de[w].is_some_and(|g| {
                            let fin = regiones[g].1;
                            l.iter().any(|&x| x > fin)
                        })
                }
                _ => true,
            };
    }
    let mut ultimo = vec![0usize; n];
    for r in 0..n {
        ultimo[r] = lecturas[r].iter().copied().max().unwrap_or(0);
        if variable[r] {
            ultimo[r] = ultimo[r].max(escrituras[r].iter().copied().max().unwrap_or(0));
        }
    }
    // Vivo en la cabeza de un bucle -> hasta su fin. Hasta que no cambie
    // nada (un bucle dentro de otro alarga al de fuera).
    let mut cambia = true;
    while cambia {
        cambia = false;
        for &(b, f) in &bucles {
            for r in 0..n {
                let dentro = |x: usize| x > b && x < f;
                let usado = lecturas[r].iter().any(|&x| dentro(x)) || dentro(ultimo[r]);
                // ** Lo que lee una Entrada o una fila del cbuffer esta desde
                // el PRINCIPIO (con el ABI de registros es una precarga, hecha
                // una vez): aunque `dxc` repita su `cbufferLoadLegacy` dentro
                // del bucle, vive hasta su fin (E6b, 02-10: `anidado.hlsl`
                // perdia `k.x` en la segunda vuelta).
                let antes = inmutable[r] || escrituras[r].first().is_none_or(|&w| w < b);
                if usado && (antes || variable[r]) && ultimo[r] < f {
                    ultimo[r] = f;
                    cambia = true;
                }
            }
        }
    }
    let mut muere = vec![Vec::new(); m];
    for r in 0..n {
        if (!lecturas[r].is_empty() || variable[r]) && ultimo[r] < m {
            muere[ultimo[r]].push(r as Reg);
        }
    }
    let mut fundible = vec![false; m];
    for i in 0..m.saturating_sub(1) {
        if let Op::Compara { d, .. } = p.ops[i] {
            let lector = matches!(p.ops[i + 1], Op::Si { c } | Op::RomperSi { c, .. } | Op::Elige { c, .. } if c == d);
            let d = d as usize;
            fundible[i] = lector && !variable[d] && lecturas[d].len() == 1 && escrituras[d].len() == 1;
        }
    }
    Analisis { ultimo, variable, muere, hondo, fundible }
}
