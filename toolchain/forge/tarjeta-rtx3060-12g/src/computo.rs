//! ** LB9 (18-10): EL COMPUTO en la RTX 3060 12G de mentira -- una gpu fn de
//! TITAN++ que mira a sus vecinas, un hilo por celda.
//!
//! ```text
//!    la memoria   cada UAV `u` en su ventana: la IOVA `BASE + u * RANURA`;
//!                 fuera de las ventanas, un fallo del simulador (lo que el
//!                 kernel no presto, la 3060 no lo ve)
//!    el kernel    antes del cuerpo carga lo de cada UAV (su base, su tope,
//!                 sus elementos...) en los registros que dicen las
//!                 precargas del emisor (`Precarga::Uav`, `DelUav::valor`)
//!    el grupo     `correr_grupo`: sus hilos, su compartida (basura al
//!                 empezar, como en la 3060), su ctaid
//! ```
//!
//! El Programa llega con su codigo: se vuelve a emitir (el emisor es
//! determinista) para saber las precargas, y si no da los MISMOS bytes, no
//! es el codigo de ese Programa y se dice.

use bmo_gpu_ga10x::trabajos::computo_app::DelUav;
use bmo_prometeo::{Codigo, Programa};

use crate::isa::simula::Memoria;
use crate::isa::simula_grupo::correr_grupo;
use crate::isa::{emitir_con, Abi, Emitido, Precarga};
use crate::{a_bytes, NOMBRE, REGISTROS};

/// Donde vive el UAV 0 en la memoria de la 3060 de mentira, y lo que mide la
/// ventana de cada uno (el `u`, en `BASE + u * RANURA`).
const BASE: u64 = 0x1_2340_0000;
const RANURA: u64 = 0x100_0000;

/// Lo que el emisor escribe para `p`, si es `c`.
pub(crate) fn el_mismo(p: &Programa, c: &Codigo) -> Result<Emitido, String> {
    let e = emitir_con(p, REGISTROS, Abi::Registros).map_err(|e| format!("el emisor de {} no escribe este computo: {:?}", NOMBRE, e))?;
    if a_bytes(&e.codigo) != c.bytes {
        return Err(format!("el codigo no es el de este Programa: {} no lo escribio asi", NOMBRE));
    }
    Ok(e)
}

/// La memoria de la 3060: cada UAV en su ventana.
struct Ventanas<'u>(&'u mut [Vec<u32>]);

impl Ventanas<'_> {
    fn donde(&self, dir: u64) -> Option<(usize, usize)> {
        let u = (dir.checked_sub(BASE)? / RANURA) as usize;
        let o = dir - BASE - u as u64 * RANURA;
        (o % 4 == 0 && ((o / 4) as usize) < self.0.get(u)?.len()).then_some((u, (o / 4) as usize))
    }
}

impl Memoria for Ventanas<'_> {
    fn leer(&mut self, dir: u64) -> Option<u32> {
        let (u, k) = self.donde(dir)?;
        Some(self.0[u][k])
    }
    fn escribir(&mut self, dir: u64, v: u32) -> bool {
        match self.donde(dir) {
            Some((u, k)) => {
                self.0[u][k] = v;
                true
            }
            None => false,
        }
    }
}

/// **Corre el computo** `c` de `p` en `grupos` grupos, sobre `uavs`.
pub(crate) fn simular(p: &Programa, c: &Codigo, grupos: u32, uavs: &mut [Vec<u32>]) -> Result<(), String> {
    let e = el_mismo(p, c)?;
    if uavs.iter().any(|u| 4 * u.len() as u64 > RANURA) {
        return Err(format!("un UAV de mas de {} bytes no cabe en una ventana de la {} de mentira", RANURA, NOMBRE));
    }
    // Lo que el kernel carga: lo de cada UAV, por su vista (estructurada, de 4 bytes).
    let mut cargas: Vec<(u8, u32)> = Vec::new();
    for q in &e.precargas {
        match *q {
            Precarga::Uav { u, que, reg } => {
                let n = uavs.get(u as usize).ok_or_else(|| format!("el computo lee el UAV {}, y no se le dio", u))?.len() as u32;
                let x = que.valor(BASE + u as u64 * RANURA, 4 * n, n, 4).ok_or_else(|| format!("el UAV {} no se presta como {:?}", u, que))?;
                cargas.push((reg, x as u32));
                if que == DelUav::Base {
                    cargas.push((reg + 1, (x >> 32) as u32));
                }
            }
            otra => return Err(format!("un computo de TITAN++ sin entradas ni cbuffer, y el emisor pidio {:?}", otra)),
        }
    }
    let mut mem = Ventanas(uavs);
    for g in 0..grupos {
        // La compartida de la 3060 NO empieza a cero.
        let mut compartida = vec![0xA5u8; 4 * p.computo.compartida as usize];
        correr_grupo(&e.codigo, p.computo.hilos, [g, 0, 0], [&[]; 8], &mut compartida, &mut mem, |_, m| {
            for &(r, v) in &cargas {
                m.r[r as usize] = v;
            }
        })
        .map_err(|x| format!("el simulador de {}: {:?}", NOMBRE, x))?;
    }
    Ok(())
}
