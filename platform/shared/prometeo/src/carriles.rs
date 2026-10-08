//! **Una OLA, corrida en la CPU** (E2.5, 05-10): los carriles de una ola
//! de un [`Programa`] corren uno detras de otro, cada uno hasta su proxima
//! operacion de ola ([`Op::Ola`]); ahi se esperan y se resuelve con los que
//! llegaron por el MISMO camino.
//!
//! [carril]  VERDE     cuentas sobre los registros de cada carril
//! [cuesta]  DATO      una ola mal agrupada da otra suma, otra papeleta
//! [riesgo]  ESPEJO    las reglas de D3D: las operaciones de ola miran los
//!                     carriles ACTIVOS (los ayudantes no); las de cuadro
//!                     leen el vecino, ayudante o no
//! [consumo] DATO      por la CPU, carril a carril: el juez, no la velocidad
//!
//! ```text
//!    ronda      cada carril que corre, hasta una ola, una barrera o el fin
//!    la ola     de los parados en una ola, el que va ANTES (`Pausa::orden`:
//!               lo que haria una GPU, SIMT) y los que van con el (`Equal`):
//!               esos son los activos; su operacion, resuelta (`olas::hacer`)
//!               y siguen. Hasta que ninguno espera en una ola.
//! ```
//!
//! Un carril que acaba (o que un `discard` tira) deja de estar activo; uno
//! en una barrera espera a la ronda siguiente del grupo (`computo.rs`).

use alloc::vec::Vec;

use super::interprete::{Extra, Paro, Pausa};
use super::olas::{anchura, hacer, CARRILES};
use super::programa::{Op, Programa};

/// Como esta un carril.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Corre,
    Ola,
    Barrera,
    /// Acabo; si el pixel queda.
    Fin(bool),
}

/// **Un carril**: sus registros, donde va y si es AYUDANTE (un pixel de un
/// cuadro que el triangulo no cubre: corre para sus vecinos, no cuenta).
pub struct Carril {
    pub regs: Vec<f32>,
    pub pausa: Pausa,
    pub estado: Estado,
    pub ayudante: bool,
}

impl Carril {
    /// Uno que empieza, con los registros iniciales de `p`.
    pub fn nuevo(p: &Programa, ayudante: bool) -> Carril {
        Carril { regs: p.iniciales.clone(), pausa: Pausa::AL_EMPEZAR, estado: Estado::Corre, ayudante }
    }

    /// Volver a empezar (sin pedir memoria).
    pub fn reiniciar(&mut self, p: &Programa, ayudante: bool) {
        self.regs.clear();
        self.regs.extend_from_slice(&p.iniciales);
        (self.pausa, self.estado, self.ayudante) = (Pausa::AL_EMPEZAR, Estado::Corre, ayudante);
    }
}

impl Programa {
    /// **Correr una ola** (hasta [`CARRILES`]: el carril `k` es el `k` de la
    /// ola) hasta que cada uno acaba o espera en una barrera. `correr(k,
    /// carril)` corre el carril `k` desde su pausa (con su `Extra`, que para
    /// en las olas: `Grupo` u `Ola`).
    pub fn correr_ola(&self, ola: &mut [Carril], mut correr: impl FnMut(usize, &mut Carril) -> Paro) {
        let n = ola.len().min(CARRILES as usize);
        let ola = &mut ola[..n];
        loop {
            for (k, c) in ola.iter_mut().enumerate() {
                if c.estado == Estado::Corre {
                    c.estado = match correr(k, c) {
                        Paro::Fin(queda) => Estado::Fin(queda),
                        Paro::Barrera => Estado::Barrera,
                        Paro::Ola => Estado::Ola,
                    };
                }
            }
            // El que va antes de los que esperan en una ola, y los suyos.
            let mut primero: Option<usize> = None;
            for (k, c) in ola.iter().enumerate() {
                if c.estado == Estado::Ola && primero.is_none_or(|p| c.pausa.orden(&ola[p].pausa).is_lt()) {
                    primero = Some(k);
                }
            }
            let Some(p) = primero else {
                return;
            };
            let junto = |c: &Carril| c.estado == Estado::Ola && c.pausa.orden(&ola[p].pausa).is_eq();
            let juntos = ola.iter().enumerate().filter(|(_, c)| junto(c)).fold(0u32, |m, (k, _)| m | 1 << k);
            let activos = ola.iter().enumerate().filter(|(_, c)| junto(c) && !c.ayudante).fold(0u32, |m, (k, _)| m | 1 << k);
            let Some(&Op::Ola { d, a, b, que }) = self.ops.get(ola[p].pausa.operacion()) else {
                // No puede ser (solo `Op::Ola` da `Paro::Ola`): que siga.
                ola.iter_mut().filter(|c| c.estado == Estado::Ola).for_each(|c| c.estado = Estado::Corre);
                continue;
            };
            // Todos los resultados antes de escribir ninguno (un carril lee
            // el registro `a` de otro). Uno que no existe lee el suyo.
            let mut r = [[0u32; 4]; CARRILES as usize];
            for k in (0..ola.len()).filter(|&k| juntos >> k & 1 != 0) {
                let de = |j: usize| ola.get(j).unwrap_or(&ola[k]).regs[a as usize].to_bits();
                r[k] = hacer(que, k, activos, de, ola[k].regs[b as usize].to_bits());
            }
            for (k, c) in ola.iter_mut().enumerate().filter(|(k, _)| juntos >> k & 1 != 0) {
                for (j, v) in r[k].into_iter().enumerate().take(anchura(que)) {
                    c.regs[d as usize + j] = f32::from_bits(v);
                }
                c.estado = Estado::Corre;
            }
        }
    }

    /// **Una ola de PIXELES** (`cuadros`, en PROTON-X): cada carril con sus
    /// entradas (y si es ayudante), y sus salidas por el id de su firma; al
    /// acabar, si cada pixel QUEDA (un `discard` lo tira). Los UAV del
    /// dibujo (N5.3d) los ven los pixeles de verdad; un AYUDANTE no escribe
    /// ninguno (lee 0), como en D3D.
    #[allow(clippy::too_many_arguments)]
    pub fn correr_pixeles(&self, ola: &mut [Carril], entradas: &[&[[f32; 4]]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [Vec<[f32; 4]>], uavs: &mut [Option<crate::bufer::Uav>]) {
        self.correr_ola(ola, |k, c| match (entradas.get(k), salidas.get_mut(k)) {
            (Some(e), Some(s)) => {
                let u: &mut [Option<crate::bufer::Uav>] = if c.ayudante { &mut [] } else { &mut *uavs };
                self.correr_desde(&mut c.pausa, e, cb, rec, s, &mut c.regs, Extra::Ola(u))
            }
            _ => Paro::Fin(false),
        });
    }
}
