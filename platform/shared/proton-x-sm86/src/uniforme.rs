//! **LO QUE TODO EL GRUPO CORRE A LA VEZ** (LI2f, 12-10) -- la uniformidad,
//! para la BARRERA: un BAR.SYNC al que no llegan todos los hilos del grupo
//! deja a la 3060 esperando (lo corta el vigilante del kernel, E7, y el
//! canal de GR queda muerto). D3D12 pide las barreras en flujo uniforme;
//! aqui se DEMUESTRA, y la que no se demuestra va por la CPU.
//!
//! ```text
//!    un registro es UNIFORME   si vale lo mismo en todos los hilos del grupo:
//!                              una constante, el cbuffer (Constantes, y
//!                              ConstantesEn con indice uniforme), el id del
//!                              GRUPO (SV_GroupID), GetDimensions, y lo que
//!                              se calcula solo con uniformes, fuera de todo
//!                              control que no lo sea
//!    NO lo es                  los ids del hilo, todo lo que lee memoria
//!                              (UAV, SRV, compartida, texturas), los
//!                              atomicos, las olas, las entradas, y lo que se
//!                              escribe bajo un control que no es uniforme
//!    un CONTROL es uniforme    si cada `si` de alrededor tiene su condicion
//!                              uniforme, y cada bucle de alrededor sale solo
//!                              por condiciones uniformes (y bajo control
//!                              uniforme): todos dan las mismas vueltas
//! ```
//!
//! Un punto fijo: se empieza con todo uniforme y se va quitando, hasta que
//! nada cambia. Lo que queda es conservador: puede decir que no a una
//! barrera que si era uniforme, nunca al reves.

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, Programa};

use crate::saltos::{escritos, leidos};

/// Si `op` da, aunque todo lo que lee sea uniforme, algo de cada hilo.
fn de_cada_hilo(op: &Op) -> bool {
    match *op {
        Op::IdHilo { que, .. } => que != 1,
        Op::LeeUav { .. } | Op::Lee { .. } | Op::Muestra { .. } | Op::LeeCompartida { .. } | Op::Atomico { .. } | Op::AtomicoCompartido { .. } | Op::Contador { .. } | Op::Ola { .. } | Op::Entrada { .. } | Op::EntradaDe { .. } | Op::LeeIndexado { .. } => true,
        _ => false,
    }
}

/// **Para cada operacion: si su control es uniforme** (la corre todo el
/// grupo a la vez, o nadie).
pub(crate) fn control_uniforme(p: &Programa) -> Vec<bool> {
    let m = p.ops.len();
    let mut uni = vec![true; p.iniciales.len()];
    let mut bucle_uni = vec![true; m];
    let mut control = vec![true; m];
    loop {
        let mut cambio = false;
        // La pila: (si es bucle, donde empieza, si es uniforme).
        let mut pila: Vec<(bool, usize, bool)> = Vec::new();
        for (i, op) in p.ops.iter().enumerate() {
            match *op {
                Op::FinSi | Op::FinBucle => {
                    pila.pop();
                }
                _ => {}
            }
            let ctl = pila.iter().all(|x| x.2);
            control[i] = ctl;
            let lee_uni = leidos(op).iter().flatten().all(|&r| uni.get(r as usize).copied().unwrap_or(true));
            // Una salida de bucle: la del bucle mas interno.
            let sale = match *op {
                Op::RomperSi { c, .. } => Some(uni.get(c as usize).copied().unwrap_or(true)),
                Op::Romper | Op::Continuar => Some(true),
                _ => None,
            };
            if let Some(cond) = sale {
                if let Some(&(_, b, _)) = pila.iter().rev().find(|x| x.0) {
                    if bucle_uni[b] && !(cond && ctl) {
                        bucle_uni[b] = false;
                        cambio = true;
                    }
                }
            }
            // Lo que escribe.
            let distinto = !ctl || !lee_uni || de_cada_hilo(op);
            if distinto {
                for r in escritos(op).0.into_iter().flatten() {
                    if let Some(u) = uni.get_mut(r as usize) {
                        if *u {
                            *u = false;
                            cambio = true;
                        }
                    }
                }
            }
            match *op {
                Op::Si { c } => pila.push((false, i, uni.get(c as usize).copied().unwrap_or(true))),
                Op::Bucle => pila.push((true, i, bucle_uni[i])),
                _ => {}
            }
        }
        if !cambio {
            return control;
        }
    }
}
