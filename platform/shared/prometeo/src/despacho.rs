//! **El COMPUTO de D3D12 en la CPU** (N5.5, 05-10): un `Dispatch(x, y, z)`
//! de un [`Programa`] de computo, grupo a grupo, con sus barreras.
//!
//! [carril]  VERDE     cuentas sobre la memoria que le dan; no toca la maquina
//! [cuesta]  DATO      una barrera mal hecha deja leer lo que otro hilo aun
//!                     no escribio
//! [riesgo]  ESPEJO    las reglas de D3D12: los hilos de un grupo comparten
//!                     su memoria (`groupshared`) y se esperan en cada
//!                     barrera; los grupos no se ven entre si salvo por los
//!                     UAV
//! [consumo] DATO      todo por la CPU, hilo a hilo: el juez, no la velocidad
//!
//! ```text
//!    cada grupo (z, y, x)     su memoria compartida, a cero
//!      ronda                  cada hilo corre hasta una barrera o el final
//!      ...                    (`Programa::correr_desde`), en orden; cuando
//!                             todos llegaron, la ronda siguiente
//! ```
//!
//! Que todos los hilos crucen la misma barrera lo exige D3D12 (las barreras
//! van en flujo uniforme); un hilo que acaba antes que los demas deja de
//! correr, y los demas siguen sin el.
//!
//! E2.5 (05-10): los hilos van en OLAS de 32, los seguidos en el orden de
//! SV_GroupIndex (`carriles.rs`): cada ronda corre ola a ola, y dentro
//! de una, sus carriles se esperan en cada operacion de ola. Un CS sin olas
//! corre igual que antes: hilo a hilo, en orden.

//!
//! En PROMETEO desde el 08-10 (LB3b de `docs/plan/PLAN_LAS_LIBRERIAS.md`):
//! correr un Programa de computo es del Programa, no de quien lo tradujo.
//! Leer el CS de un PSO de D3D12 (`preparar`) sigue en PROTON-X
//! (`platform/shared/proton-x/src/dxil/computo.rs`).

use alloc::vec;
use alloc::vec::Vec;

use super::carriles::{Carril, Estado};
use super::interprete::{Extra, Grupo, Ids};
use super::olas::CARRILES;
use super::programa::Programa;

impl Programa {
    /// **`Dispatch(grupos)`**: cada hilo de cada grupo, con el cbuffer `cb`
    /// (aplanado), los SRV de `rec` y los UAV de `uavs` (por ranura).
    /// Devuelve los hilos que corrieron.
    pub fn despachar(&self, grupos: [u32; 3], cb: &[u8], rec: &crate::textura::Recursos, uavs: &mut [Option<crate::bufer::Uav>]) -> u64 {
        let [hx, hy, hz] = self.computo.hilos;
        let n = (hx * hy * hz) as usize;
        if n == 0 {
            return 0;
        }
        let mut compartida = vec![0u32; self.computo.compartida as usize];
        let mut hilos: Vec<Carril> = (0..n).map(|_| Carril::nuevo(self, false)).collect();
        let mut corridos = 0u64;
        for gz in 0..grupos[2] {
            for gy in 0..grupos[1] {
                for gx in 0..grupos[0] {
                    compartida.fill(0);
                    for h in hilos.iter_mut() {
                        h.reiniciar(self, false);
                    }
                    loop {
                        let mut alguno_espera = false;
                        for (w, ola) in hilos.chunks_mut(CARRILES as usize).enumerate() {
                            self.correr_ola(ola, |k, c| {
                                let t = (w * CARRILES as usize + k) as u32;
                                let en_grupo = [t % hx, (t / hx) % hy, t / (hx * hy)];
                                let ids = Ids {
                                    despacho: [gx * hx + en_grupo[0], gy * hy + en_grupo[1], gz * hz + en_grupo[2]],
                                    grupo: [gx, gy, gz],
                                    en_grupo,
                                    indice: t,
                                };
                                let mut g = Grupo { ids, compartida: &mut compartida, uavs };
                                self.correr_desde(&mut c.pausa, &[], cb, rec, &mut [], &mut c.regs, Extra::Grupo(&mut g))
                            });
                            // Los de la barrera, a la ronda siguiente.
                            for c in ola.iter_mut().filter(|c| c.estado == Estado::Barrera) {
                                c.estado = Estado::Corre;
                                alguno_espera = true;
                            }
                        }
                        if !alguno_espera {
                            break;
                        }
                    }
                    corridos += hilos.iter().filter(|c| matches!(c.estado, Estado::Fin(_))).count() as u64;
                }
            }
        }
        corridos
    }
}
