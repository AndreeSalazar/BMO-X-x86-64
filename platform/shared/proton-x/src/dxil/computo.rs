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

use alloc::vec;
use alloc::vec::Vec;

use super::interprete::{Grupo, Ids, Paro, Pausa};
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
        let mut hilos: Vec<(Vec<f32>, Pausa, bool)> = (0..n).map(|_| (Vec::new(), Pausa::AL_EMPEZAR, false)).collect();
        let mut corridos = 0u64;
        for gz in 0..grupos[2] {
            for gy in 0..grupos[1] {
                for gx in 0..grupos[0] {
                    compartida.fill(0);
                    for h in hilos.iter_mut() {
                        h.0.clear();
                        h.0.extend_from_slice(&self.iniciales);
                        (h.1, h.2) = (Pausa::AL_EMPEZAR, false);
                    }
                    loop {
                        let mut alguno_espera = false;
                        for (t, (regs, pausa, acabo)) in hilos.iter_mut().enumerate() {
                            if *acabo {
                                continue;
                            }
                            let t = t as u32;
                            let en_grupo = [t % hx, (t / hx) % hy, t / (hx * hy)];
                            let ids = Ids {
                                despacho: [gx * hx + en_grupo[0], gy * hy + en_grupo[1], gz * hz + en_grupo[2]],
                                grupo: [gx, gy, gz],
                                en_grupo,
                                indice: t,
                            };
                            let mut g = Grupo { ids, compartida: &mut compartida, uavs };
                            match self.correr_desde(pausa, &[], cb, rec, &mut [], regs, Some(&mut g)) {
                                Paro::Barrera => alguno_espera = true,
                                Paro::Fin(_) => {
                                    *acabo = true;
                                    corridos += 1;
                                }
                            }
                        }
                        if !alguno_espera {
                            break;
                        }
                    }
                }
            }
        }
        corridos
    }
}

/// **Un sombreador de computo listo para despachar**: su programa (con los
/// cbuffers ya APLANADOS en un bloque, como los de un dibujo) y donde va
/// cada cbuffer en el (`lote::Bloque`, por su ranura).
#[derive(Debug, Clone, PartialEq)]
pub struct DeComputo {
    pub programa: Programa,
    pub constantes: Vec<crate::lote::Bloque>,
}

/// **Preparar el CS de un PSO de computo**: leerlo, compilarlo y aplanar sus
/// cbuffers. El texto dice por que no, si no.
pub fn preparar(cs: &[u8]) -> Result<DeComputo, alloc::string::String> {
    use alloc::format;
    let s = super::leer(cs).map_err(|e| format!("el CS no se lee: {e:?}"))?;
    if s.etapa != super::Etapa::Computo {
        return Err(format!("el sombreador de un PSO de computo es de otra etapa ({:?})", s.etapa));
    }
    if s.hilos.contains(&0) {
        return Err(alloc::string::String::from("un CS sin numthreads (su PSV0 no lo dice)"));
    }
    // D3D12: hasta 1024 hilos por grupo (x e y hasta 1024, z hasta 64).
    let [x, y, z] = s.hilos;
    if x > 1024 || y > 1024 || z > 64 || x as u64 * y as u64 * z as u64 > 1024 {
        return Err(format!("numthreads({x}, {y}, {z}): mas de lo que deja D3D12 (1024 hilos por grupo)"));
    }
    let mut programa = super::programa::compilar(&s).map_err(|e| format!("el CS no se sabe correr todavia: {e:?}"))?;
    let mut filas = vec![0u16; programa.ranuras.cbuffers.len()];
    programa.filas_por_cbuffer(&mut filas);
    let mut constantes = Vec::with_capacity(filas.len());
    let mut fila = 0u32;
    for &f in &filas {
        constantes.push(crate::lote::Bloque { fila: fila as u16, filas: f });
        fila += f as u32;
    }
    if fila > u16::MAX as u32 {
        return Err(format!("los cbuffers del CS leen {fila} filas: mas de las que caben"));
    }
    let bases: Vec<u16> = constantes.iter().map(|b| b.fila).collect();
    programa.aplanar(&bases);
    Ok(DeComputo { programa, constantes })
}
