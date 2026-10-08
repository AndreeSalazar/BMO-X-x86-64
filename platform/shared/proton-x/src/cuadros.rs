//! **Los pixeles en CUADROS de 2x2, y los cuadros en OLAS** (E2.5, 05-10):
//! lo que la trama hace con un sombreador de pixeles que usa las olas
//! (`WaveActiveSum`, `QuadReadAcrossX`...), para que vea a sus vecinos.
//!
//! [carril]  VERDE     cuentas sobre la trama; no toca la maquina
//! [cuesta]  DATO      un cuadro mal hecho lee el vecino equivocado
//! [riesgo]  ESPEJO    las reglas de D3D: un cuadro empieza en x e y PARES;
//!                     sus carriles 0 1 arriba, 2 3 abajo; un pixel del
//!                     cuadro que no llega al sombreador (fuera del
//!                     triangulo, o que una prueba de antes ya tiro) corre
//!                     igual, de AYUDANTE: sus vecinos lo leen, las olas no
//!                     lo cuentan, no escribe UAV, no se pinta ni cuenta
//! [consumo] DATO      solo con olas; los demas, pixel a pixel como siempre
//!
//! ```text
//!    el triangulo     los pixeles que LLEGAN al sombreador (los decide el
//!                     bucle de `trama::dibujar_todo`, con sus pruebas de
//!                     antes); sus cuadros, fila a fila de cuadros, enteros
//!                     (4 carriles: los que faltan, ayudantes)
//!    la ola           8 cuadros (32 carriles), o los que queden: el
//!                     sombreador los corre juntos (`dxil::carriles`)
//!    lo que sale      el color (o el descarte) de cada pixel que llego, que
//!                     la trama pone como siempre (`trama::poner_pixel`)
//! ```
//!
//! Una GPU junta cuadros de varios triangulos en una ola; aqui, de uno
//! (D3D no dice cuales van juntos: cualquier reparto vale, y este es el
//! que se puede repetir).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::dxil::olas::CARRILES;
use crate::trama::{Tri, SALIDAS};

/// **Un carril de una ola de pixeles**: lo que entra, si es ayudante, y lo
/// que sale (el color de cada render target, y si el pixel queda).
pub struct Carril {
    pub entrada: Vec<[f32; 4]>,
    pub ayudante: bool,
    pub colores: [[f32; 4]; SALIDAS],
    pub queda: bool,
}

/// Quien corre una ola de pixeles: el sombreador sobre sus carriles.
pub type Olas<'a> = &'a mut dyn FnMut(&mut [Carril]);

/// **Sombrear en olas los pixeles de un triangulo** que llegan al
/// sombreador: cada uno `(x, y, entrada, ayudante)` -- ayudante, uno que
/// corre solo para saber si lo tira (ver `trama`). Los de sus cuadros que
/// no estan, AYUDANTES con su entrada de [`Tri::entrada`] (los mismos pesos,
/// extrapolados). Lo que dio cada uno, en su orden: sus colores, o `None`
/// si lo tiro.
pub fn sombrear(tri: &Tri, pixeles: &[(i64, i64, &[[f32; 4]], bool)], posicion: Option<usize>, olas: &mut dyn FnMut(&mut [Carril])) -> Vec<Option<[[f32; 4]; SALIDAS]>> {
    let mut hechos = alloc::vec![None; pixeles.len()];
    // Los cuadros (fila de cuadros, columna), con el pixel de cada carril.
    let mut cuadros: BTreeMap<(i64, i64), [Option<usize>; 4]> = BTreeMap::new();
    for (j, &(px, py, ..)) in pixeles.iter().enumerate() {
        cuadros.entry((py & !1, px & !1)).or_insert([None; 4])[((py & 1) * 2 + (px & 1)) as usize] = Some(j);
    }
    let mut ola: Vec<Carril> = Vec::with_capacity(CARRILES as usize);
    // De cada carril: el pixel que llego, si es uno.
    let mut donde: Vec<Option<usize>> = Vec::with_capacity(CARRILES as usize);
    let mut correr = |ola: &mut Vec<Carril>, donde: &mut Vec<Option<usize>>, hechos: &mut Vec<Option<[[f32; 4]; SALIDAS]>>| {
        olas(ola);
        for (c, d) in ola.iter().zip(donde.iter()) {
            if let Some(j) = *d {
                hechos[j] = c.queda.then_some(c.colores);
            }
        }
        ola.clear();
        donde.clear();
    };
    for (&(qy, qx), carriles) in &cuadros {
        for (k, &j) in carriles.iter().enumerate() {
            let (entrada, ayudante) = match j {
                Some(j) => (pixeles[j].2.to_vec(), pixeles[j].3),
                None => {
                    let (px, py) = (qx + (k & 1) as i64, qy + (k >> 1) as i64);
                    let mut e = tri.de_partida().to_vec();
                    tri.entrada(&tri.aristas(px, py), px, py, posicion, &mut e);
                    (e, true)
                }
            };
            ola.push(Carril { entrada, ayudante, colores: [[0.0; 4]; SALIDAS], queda: false });
            donde.push(j);
        }
        if ola.len() >= CARRILES as usize {
            correr(&mut ola, &mut donde, &mut hechos);
        }
    }
    if !ola.is_empty() {
        correr(&mut ola, &mut donde, &mut hechos);
    }
    hechos
}
