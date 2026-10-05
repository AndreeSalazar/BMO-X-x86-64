//! **Las ranuras de un programa** (03-10, N5.1 y N5.2): cada textura,
//! muestreador y cbuffer que lee, por su lugar (espacio, registro, etapa), y
//! lo que hace el enlace con ellas (unir las de dos etapas, renumerar,
//! aplanar los cbuffers). Aparte de `programa.rs` porque es la costura con
//! la root signature, no la compilacion.
//!
//! [carril]  VERDE     numeros; no toca la maquina
//! [cuesta]  DATO      una ranura mal renumerada lee el recurso de otro
//! [consumo] NADA      una vez por PSO, al enlazar

use alloc::vec::Vec;

use super::programa::{NoPrograma, Op, Programa};

/// **Donde vive un recurso**: su espacio y su registro (`t40, space1`), y
/// la etapa que lo lee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lugar {
    pub espacio: u32,
    pub registro: u32,
    /// `D3D12_SHADER_VISIBILITY` de quien lo lee (1 vertices, 5 pixeles; 0
    /// sin decir): el t0 del de vertices y el del de pixeles pueden venir de
    /// tablas distintas. La pone el enlace ([`Ranuras::de_la_etapa`]).
    pub vista: u32,
}

/// **Las ranuras de un programa** (03-10, N5.1): cada textura y cada
/// muestreador que lee, sin repetir, en el orden en que aparecen. Hasta hoy
/// el `t` de una operacion ERA el registro, y solo cabian t0..t31 y s0..s15
/// del espacio 0: el primer sombreador de Cyberpunk con un recurso mas alla
/// no corria.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ranuras {
    pub texturas: Vec<Lugar>,
    pub muestreadores: Vec<Lugar>,
    /// Los cbuffers (N5.2): b0 ya no es el unico.
    pub cbuffers: Vec<Lugar>,
    /// N5.4 (05-10): los rangos de texturas que se leen con el registro
    /// CALCULADO (`g_txMats[i]`, bindless): su espacio y su PRIMER registro.
    /// No ocupan ranura de textura: la textura se busca al correr
    /// (`Op::EligeTextura`, `textura::Dinamicas`).
    pub dinamicas: Vec<Lugar>,
    /// N5.5 (05-10): los UAV de bufer que lee o escribe el computo.
    pub uavs: Vec<Lugar>,
    /// N5.3b (05-10): el PASO de cada bufer estructurado que declara, de sus
    /// metadatos: `(uav, espacio, registro, paso)`. Lo necesita quien lo da
    /// desde la RAIZ (esa vista no lo lleva). Ver [`Ranuras::paso`].
    pub pasos: Vec<(bool, u32, u32, u32)>,
}

/// **Lo que [`Ranuras::unir`] devuelve**: por ranura de las otras, su
/// ranura en la union; lo que pide [`Programa::renumerar`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mapa {
    pub texturas: Vec<u8>,
    pub muestreadores: Vec<u8>,
    pub cbuffers: Vec<u8>,
    pub dinamicas: Vec<u8>,
    pub uavs: Vec<u8>,
}

impl Ranuras {
    fn de(v: &mut Vec<Lugar>, l: Lugar) -> Result<u8, NoPrograma> {
        if let Some(i) = v.iter().position(|&x| x == l) {
            return Ok(i as u8);
        }
        // 255 se reserva: es la textura ELEGIDA al correr (`DINAMICA`).
        if v.len() >= 255 {
            return Err(NoPrograma::Forma("un sombreador con mas de 255 texturas o muestreadores distintos"));
        }
        v.push(l);
        Ok((v.len() - 1) as u8)
    }

    /// La ranura de la textura de `espacio` y `registro` (nueva si no estaba).
    pub fn textura(&mut self, espacio: u32, registro: u32) -> Result<u8, NoPrograma> {
        Self::de(&mut self.texturas, Lugar { espacio, registro, vista: 0 })
    }

    /// La del muestreador.
    pub fn muestreador(&mut self, espacio: u32, registro: u32) -> Result<u8, NoPrograma> {
        Self::de(&mut self.muestreadores, Lugar { espacio, registro, vista: 0 })
    }

    /// La del cbuffer.
    pub fn cbuffer(&mut self, espacio: u32, registro: u32) -> Result<u8, NoPrograma> {
        Self::de(&mut self.cbuffers, Lugar { espacio, registro, vista: 0 })
    }

    /// N5.5: la del UAV.
    pub fn uav(&mut self, espacio: u32, registro: u32) -> Result<u8, NoPrograma> {
        Self::de(&mut self.uavs, Lugar { espacio, registro, vista: 0 })
    }

    /// N5.4: el rango dinamico que empieza en `registro` de `espacio`.
    pub fn dinamica(&mut self, espacio: u32, registro: u32) -> Result<u8, NoPrograma> {
        Self::de(&mut self.dinamicas, Lugar { espacio, registro, vista: 0 })
    }

    /// **El paso del bufer estructurado** (SRV, o UAV con `uav`) de `l`, si
    /// lo declara.
    pub fn paso(&self, uav: bool, l: Lugar) -> Option<u32> {
        self.pasos.iter().find(|p| (p.0, p.1, p.2) == (uav, l.espacio, l.registro)).map(|p| p.3)
    }

    /// **Las de la etapa `vista`**: todas pasan a ser de ella.
    pub fn de_la_etapa(mut self, vista: u32) -> Ranuras {
        for l in self.texturas.iter_mut().chain(self.muestreadores.iter_mut()).chain(self.cbuffers.iter_mut()).chain(self.dinamicas.iter_mut()).chain(self.uavs.iter_mut()) {
            l.vista = vista;
        }
        self
    }

    /// **Sumar las de `otras`** (las del de pixeles a las del de vertices):
    /// lo que ya estaba guarda su ranura, lo nuevo va detras. Devuelve, por
    /// ranura de `otras`, su ranura aqui: lo que pide [`Programa::renumerar`].
    pub fn unir(&mut self, otras: &Ranuras) -> Result<Mapa, NoPrograma> {
        let sumar = |v: &mut Vec<Lugar>, de: &[Lugar]| de.iter().map(|&l| Self::de(v, l)).collect::<Result<Vec<u8>, _>>();
        for p in &otras.pasos {
            if !self.pasos.contains(p) {
                self.pasos.push(*p);
            }
        }
        Ok(Mapa {
            texturas: sumar(&mut self.texturas, &otras.texturas)?,
            muestreadores: sumar(&mut self.muestreadores, &otras.muestreadores)?,
            cbuffers: sumar(&mut self.cbuffers, &otras.cbuffers)?,
            dinamicas: sumar(&mut self.dinamicas, &otras.dinamicas)?,
            uavs: sumar(&mut self.uavs, &otras.uavs)?,
        })
    }
}

impl Programa {
    /// **Renumerar sus texturas y muestreadores** (la ranura `i` pasa a
    /// `texturas[i]` y `muestreadores[i]`, y lo mismo los cbuffers): lo que
    /// hace el enlace para que el de vertices y el de pixeles compartan UNA
    /// tabla.
    pub fn renumerar(&mut self, m: &Mapa) {
        let a = |v: &[u8], x: u8| v.get(x as usize).copied().unwrap_or(x);
        for op in &mut self.ops {
            match op {
                Op::Muestra { t, s, .. } | Op::Lee { t, s, .. } => {
                    // La ELEGIDA no es una ranura: la dice el `EligeTextura` de antes.
                    if *t != super::programa::DINAMICA {
                        *t = a(&m.texturas, *t);
                    }
                    *s = a(&m.muestreadores, *s);
                }
                Op::EligeTextura { rango, .. } => *rango = a(&m.dinamicas, *rango),
                Op::EscribeUav { u, .. } | Op::LeeUav { u, .. } | Op::MedidasUav { u, .. } | Op::Contador { u, .. } => *u = a(&m.uavs, *u),
                Op::Constantes { cb, .. } | Op::ConstantesEn { cb, .. } => *cb = a(&m.cbuffers, *cb),
                _ => {}
            }
        }
    }

    /// **Las filas que lee de cada cbuffer** (la mayor mas uno), por ranura,
    /// sumadas a `filas` (que crece si hace falta).
    pub fn filas_por_cbuffer(&self, filas: &mut Vec<u16>) {
        for op in &self.ops {
            let (hasta, cb) = match *op {
                Op::Constantes { fila, cb, .. } => (fila + 1, cb),
                Op::ConstantesEn { fila, filas: n, cb, .. } => (fila + n, cb),
                _ => continue,
            };
            if filas.len() <= cb as usize {
                filas.resize(cb as usize + 1, 0);
            }
            filas[cb as usize] = filas[cb as usize].max(hasta);
        }
    }

    /// **Aplanar**: cada fila pasa a su sitio en el bloque de todas las
    /// constantes (`fila + bases[cb]`), y `filas_cb` a la mayor que lee mas
    /// uno. Desde aqui, quien corre el programa ve UN cbuffer.
    pub fn aplanar(&mut self, bases: &[u16]) {
        let mut filas = 0;
        for op in &mut self.ops {
            match op {
                Op::Constantes { fila, cb, .. } => {
                    *fila += bases.get(*cb as usize).copied().unwrap_or(0);
                    filas = filas.max(*fila + 1);
                }
                Op::ConstantesEn { fila, filas: n, cb, .. } => {
                    *fila += bases.get(*cb as usize).copied().unwrap_or(0);
                    filas = filas.max(fila.saturating_add(*n));
                }
                _ => {}
            }
        }
        self.filas_cb = filas;
    }
}
