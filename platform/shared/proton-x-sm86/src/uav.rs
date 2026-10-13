//! **EL COMPUTO DE UNA APP EN LA 3060: los ids, la barrera y los UAV** (LI2f
//! de `docs/plan/PLAN_EL_LIBRETO.md`, 12-10) -- `Op::IdHilo`, `Op::Barrera`,
//! y `Op::LeeUav` / `Op::EscribeUav` de un bufer CRUDO (`ByteAddressBuffer`)
//! o ESTRUCTURADO (`StructuredBuffer<S>`), con las formas de
//! `bmo_sm86::computo` (su oro, `oro_computo.ptx`) y las de enteros de
//! `codifica` (las de E6c y E6d).
//!
//! ```text
//!    SV_GroupThreadID.c    S2R TID.c
//!    SV_GroupID.c          S2R CTAID.c
//!    SV_DispatchThreadID.c S2R CTAID.c; S2R TID.c; IMAD x = ctaid * hilos.c + tid
//!    SV_GroupIndex         tid.x + tid.y * hx + tid.z * hx * hy (IMAD)
//!    la barrera            BAR.SYNC, donde el control es UNIFORME (`uniforme.rs`)
//! ```
//!
//! # Un UAV, palabra a palabra, con la DIRECCION SUJETA
//!
//! Lo que el KERNEL pone de cada UAV (`Precarga::Uav`, fijo todo el programa:
//! el cuerpo no lo puede pisar, R7), y lo que dice:
//!
//! ```text
//!    Base       el par (lo, hi): la IOVA del UAV en la 3060, alineada
//!    Tope       la mayor direccion de palabra que se puede tocar, desde la
//!               base: lo prestado menos 4 (alineado). LA SEGURIDAD
//!    Limite     crudo: las palabras que valen empiezan antes (elementos * 4
//!               - 3, o 0 sin ninguna): x vale si x + 4 <= bytes
//!    Elementos  estructurado: los elementos (0 si el paso es 0)
//!    Paso       estructurado: los bytes de un elemento
//! ```
//!
//! Cada palabra `j` (la casa: `bufer::Bufer::cargar` y `Uav::escribir`):
//!
//! ```text
//!    crudo         x = i + desp + 4j (sin acarreo; con el, NO_VALE)
//!                  vale si x < Limite
//!    estructurado  x = i * paso + desp + 4j
//!                  vale si i < Elementos y desp + 4j + 4 <= paso (sin acarreo)
//!
//!    la direccion  LOP3       a = x & ~3            alineada
//!                  IMNMX.U32  a = min(a, Tope)      NUNCA fuera de lo prestado
//!                  IADD3      a.lo, P3 = Base.lo + a
//!                  IADD3.X    a.hi = Base.hi + RZ + P3
//!    leer          LDG v, [a]; SEL v = vale ? v : 0
//!    escribir      @vale STG [a], v
//! ```
//!
//! Lo que valga o no lo dice el predicado (P2), y eso es CORRECCION: un
//! cuerpo que se lo salte lee o escribe mal SU UAV, y nada mas. La
//! SEGURIDAD es la cadena de la direccion -- base mas lo sujeto al tope --,
//! siempre la misma y de forma fija, la que el juez del kernel reconoce
//! (como el LDC sujeto de E8f).
//!
//! [!] Una palabra NO alineada (`i + desp` de un crudo que no es multiplo de
//! 4): en D3D12 no esta definida; la casa lee los bytes de ahi, la 3060 la
//! palabra alineada de debajo. No son los mismos bits: no lo usa ningun
//! sombreador de los que hay (dxc pide el multiplo de 4).
//!
//! **Los atomicos** (`Op::Atomico`, los `Interlocked*` sobre un UAV) son la
//! palabra 0 con la misma cadena: `@vale ATOMG d, [a], v` (o ATOMG.CAS con
//! `igual`) y `SEL d = vale ? d : 0` -- fuera, la casa devuelve 0 y no
//! escribe --. **`GetDimensions`** (`Op::MedidasUav`) lee lo que el kernel
//! dice (`DelUav::Medida`: los elementos de la vista; en un crudo, sus bytes).
//!
//! Lo que NO va todavia (va por la CPU, con su indice): los UAV con tipo y
//! de textura, el contador, y la barrera donde el control no es uniforme.

use alloc::vec::Vec;

use bmo_proton_x::bufer::Modo;
use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};
use bmo_sm86::computo::{self as g, Sr, SIEMPRE};

use super::planifica::Meta;
use super::{reg_de, Abi, Clase, Emisor, NoEmite, Precarga};

/// **Lo que el juez del kernel le deja a un cuerpo de computo emitido**: sus
/// ids y su barrera, y de cada UAV su base y su tope (los registros de sus
/// precargas), y los bytes de su compartida. `None`: mas UAV de los que ata
/// el kernel.
pub fn permisos_computo(e: &super::Emitido, p: &bmo_proton_x::dxil::programa::Programa) -> Option<bmo_gpu_ga10x::sass::juez::Permisos> {
    use bmo_gpu_ga10x::sass::juez_computo::{UavsDeApp, MAX_UAVS};
    let mut uavs = UavsDeApp::default();
    let de = |u: u8, q: DelUav| e.precargas.iter().find_map(|p| matches!(*p, Precarga::Uav { u: u2, que, .. } if (u2, que) == (u, q)).then(|| if let Precarga::Uav { reg, .. } = *p { reg } else { 0 }));
    for p in &e.precargas {
        if let Precarga::Uav { u, que: DelUav::Base, reg } = *p {
            let n = uavs.n as usize;
            if n == MAX_UAVS {
                return None;
            }
            uavs.base[n] = reg;
            uavs.tope[n] = de(u, DelUav::Tope)?;
            uavs.solo_lee |= ((u & SRV != 0) as u8) << n;
            uavs.n += 1;
        }
    }
    Some(bmo_gpu_ga10x::sass::juez::Permisos { computo: true, uavs, compartida: 4 * p.computo.compartida, ..Default::default() })
}

/// ** El bit de un SRV de bufer (`StructuredBuffer`, `ByteAddressBuffer`):
/// en `Precarga::Uav`, `u | SRV` es el tN, que se LEE con la misma cadena y
/// que el kernel presta solo para leer (el juez no deja escribirlo).
pub const SRV: u8 = 0x80;

/// Lo que el kernel pone de un UAV (ver el modulo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DelUav {
    Base = 0,
    Tope = 1,
    Limite = 2,
    Elementos = 3,
    Paso = 4,
    /// Lo que dice `GetDimensions`: los elementos de la vista (en un crudo,
    /// sus bytes), sin mirar lo prestado.
    Medida = 5,
}

impl DelUav {
    pub const TODOS: [DelUav; 6] = [DelUav::Base, DelUav::Tope, DelUav::Limite, DelUav::Elementos, DelUav::Paso, DelUav::Medida];

    /// El de un numero (el del .bsf vivo).
    pub fn de(n: u8) -> Option<DelUav> {
        DelUav::TODOS.into_iter().find(|q| *q as u8 == n)
    }

    /// **Lo que el KERNEL pone** en este registro (la base: el par entero)
    /// para un UAV de `bytes` prestados a la 3060 desde `base`, con su vista
    /// (`elementos`, en un crudo palabras; y `paso` si es estructurado): las
    /// reglas de la casa (`bufer::Uav`). Una vez, aqui: lo usan el kernel y
    /// las pruebas. `None`: no se presta (menos de una palabra, o la base sin
    /// alinear). [!] Un estructurado cuyo ultimo elemento no cabe entero en
    /// lo prestado lo pierde entero (la casa leeria sus palabras de dentro).
    pub fn valor(self, base: u64, bytes: u32, elementos: u32, paso: u32) -> Option<u64> {
        if bytes < 4 || base % 4 != 0 {
            return None;
        }
        Some(match self {
            DelUav::Base => base,
            DelUav::Tope => (bytes & !3) as u64 - 4,
            DelUav::Limite => (4 * elementos as u64).min(bytes as u64).saturating_sub(3),
            DelUav::Elementos if paso == 0 => 0,
            DelUav::Elementos => elementos.min(bytes / paso) as u64,
            DelUav::Paso => paso as u64,
            // La de la casa (`Bufer::medidas`): un crudo, en bytes.
            DelUav::Medida if paso == 0 => elementos.saturating_mul(4) as u64,
            DelUav::Medida => elementos as u64,
        })
    }
}

/// Los predicados de la guarda: el acarreo, si la palabra vale, y el
/// acarreo de la direccion. P0 es del `si` fundido: no se toca.
const P_ACARREO: u8 = 1;
const P_VALE: u8 = 2;
const P_DIR: u8 = 3;
/// Lo que marca una palabra que no vale (ningun UAV llega a 4 GiB).
const NO_VALE: u32 = u32::MAX;
/// La direccion, alineada a su palabra.
const PALABRA: u32 = !3;

/// Lo de un UAV que comparten sus cuatro palabras.
struct Guia {
    base: u8,
    tope: u8,
    como: Como,
}

enum Como {
    /// `s` = i + desp (o NO_VALE), y el limite.
    Crudo { s: u8, limite: u8 },
    /// `q` = i * paso + desp, `m` = desp (o NO_VALE si `i` no vale), y el paso.
    Estructurado { q: u8, m: u8, paso: u8 },
}

impl Emisor<'_> {
    /// El registro de lo que el kernel pone del UAV `u` (la primera vez se
    /// pide, fijo todo el programa). Se piden ANTES del cuerpo.
    fn precarga_uav(&mut self, u: u8, que: DelUav) -> Result<u8, NoEmite> {
        for &p in &self.precargas {
            if let Precarga::Uav { u: u2, que: q2, reg } = p {
                if (u2, q2) == (u, que) {
                    return Ok(reg);
                }
            }
        }
        let reg = self.bloque(if que == DelUav::Base { 2 } else { 1 }, true)?;
        self.precargas.push(Precarga::Uav { u, que, reg });
        Ok(reg)
    }

    /// Lo que pide un UAV de modo `modo` (la pasada de antes del cuerpo).
    pub(super) fn precargas_uav(&mut self, u: u8, modo: Modo, k: usize) -> Result<(), NoEmite> {
        self.hilos(k)?;
        let resto: &[DelUav] = match modo {
            Modo::Crudo => &[DelUav::Limite],
            Modo::Estructurado => &[DelUav::Elementos, DelUav::Paso],
            _ => return Err(NoEmite::Operacion(k)),
        };
        for &q in [DelUav::Base, DelUav::Tope].iter().chain(resto) {
            self.precarga_uav(u, q)?;
        }
        Ok(())
    }

    /// Lo que pide un `GetDimensions` del UAV `u` (la pasada de antes del
    /// cuerpo).
    pub(super) fn precarga_medida(&mut self, u: u8, modo: Modo, k: usize) -> Result<(), NoEmite> {
        self.hilos(k)?;
        if !matches!(modo, Modo::Crudo | Modo::Estructurado) {
            return Err(NoEmite::Operacion(k));
        }
        self.precarga_uav(u, DelUav::Medida).map(|_| ())
    }

    /// **`d` = `GetDimensions` del UAV `u`** (`d+1..d+4`, 0).
    pub(super) fn medidas_uav(&mut self, d: Reg, u: u8, k: usize) -> Result<(), NoEmite> {
        let Some(m) = self.precargas.iter().find_map(|p| match *p {
            Precarga::Uav { u: u2, que: DelUav::Medida, reg } if u2 == u => Some(reg),
            _ => None,
        }) else {
            return Err(NoEmite::Operacion(k));
        };
        let x = self.destino(d, k)?;
        self.poner(c::mov(x, c::r(m), 0), Clase::Alu, Some(x), [Some(m), None, None]);
        for j in 1..4 {
            let r = d as usize + j;
            if self.ultimo.get(r).copied().unwrap_or(0) > k || self.variable.get(r).copied().unwrap_or(false) {
                let y = self.destino(r as Reg, k)?;
                self.poner(c::mov(y, Fuente::Imm(0), 0), Clase::Alu, Some(y), [None; 3]);
            }
        }
        Ok(())
    }

    /// Un S2R (desacoplado: con su barrera, como un MUFU).
    fn s2r(&mut self, x: u8, sr: Sr) {
        self.poner(g::s2r(x, sr, 0), Clase::Mufu, Some(x), [None; 3]);
    }

    /// Los hilos de un grupo, si el programa es de computo.
    pub(super) fn hilos(&self, k: usize) -> Result<[u32; 3], NoEmite> {
        let h = self.p.computo.hilos;
        if self.abi != Abi::Registros || h.contains(&0) {
            return Err(NoEmite::Operacion(k));
        }
        Ok(h)
    }

    /// **`d` = un id del hilo** (`que`: 0 SV_DispatchThreadID, 1 SV_GroupID,
    /// 2 SV_GroupThreadID, su componente `comp`; 3 SV_GroupIndex).
    pub(super) fn id_hilo(&mut self, d: Reg, que: u8, comp: u8, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let h = self.hilos(k)?;
        if comp > 2 || que > 3 {
            return Err(NoEmite::Operacion(k));
        }
        let co = comp as usize;
        let x = self.destino(d, k)?;
        match que {
            0 => {
                let t = self.pedir()?;
                paso.push(t);
                self.s2r(t, Sr::CTAID[co]);
                self.s2r(x, Sr::TID[co]);
                self.poner(c::imad(x, t, Fuente::Imm(h[co]), x, 0), Clase::Fma, Some(x), [Some(t), Some(x), None]);
            }
            1 => self.s2r(x, Sr::CTAID[co]),
            2 => self.s2r(x, Sr::TID[co]),
            _ => self.indice(x, h)?,
        }
        Ok(())
    }

    /// `x` = SV_GroupIndex: tid.x + tid.y * hx + tid.z * hx * hy.
    pub(super) fn indice(&mut self, x: u8, h: [u32; 3]) -> Result<(), NoEmite> {
        self.s2r(x, Sr::TidX);
        // Con una sola fila (o capa), su id es 0: no suma nada.
        for (sr, ancho, hay) in [(Sr::TidY, h[0], h[1] > 1), (Sr::TidZ, h[0] * h[1], h[2] > 1)] {
            if hay {
                let t = self.pedir()?;
                self.s2r(t, sr);
                self.poner(c::imad(x, t, Fuente::Imm(ancho), x, 0), Clase::Fma, Some(x), [Some(t), Some(x), None]);
                self.soltar(t);
            }
        }
        Ok(())
    }

    /// **La barrera del grupo**: BAR.SYNC, que espera antes todo lo que este
    /// en vuelo (como un salto). Solo donde el control es UNIFORME
    /// (`uniforme.rs`: cada `si` y cada salida de bucle de alrededor dependen
    /// de lo que vale igual en todo el grupo): ahi llegan todos. Donde no se
    /// demuestra, por la CPU -- si un hilo no llegara, la 3060 se quedaria
    /// esperando (lo cortaria el vigilante del kernel, E7) --.
    pub(super) fn barrera(&mut self, k: usize) -> Result<(), NoEmite> {
        self.hilos(k)?;
        if !self.uniforme.get(k).copied().unwrap_or(false) {
            return Err(NoEmite::Operacion(k));
        }
        self.poner_meta(g::bar_sync(0), Meta { salto: true, ..Meta::de(Clase::Nada, None, [None; 3]) });
        Ok(())
    }

    /// Lo comun a las palabras de un acceso al UAV `u`.
    fn guia(&mut self, u: u8, modo: Modo, i: Reg, desp: Reg, k: usize, paso: &mut Vec<u8>) -> Result<Guia, NoEmite> {
        self.hilos(k)?;
        let una = |e: &Self, q: DelUav| {
            e.precargas.iter().find_map(|p| match *p {
                Precarga::Uav { u: u2, que, reg } if (u2, que) == (u, q) => Some(reg),
                _ => None,
            })
        };
        let (Some(base), Some(tope)) = (una(self, DelUav::Base), una(self, DelUav::Tope)) else {
            return Err(NoEmite::Operacion(k));
        };
        let ri = self.registro(i, paso)?;
        let como = match (modo, una(self, DelUav::Limite), una(self, DelUav::Elementos), una(self, DelUav::Paso)) {
            (Modo::Crudo, Some(limite), _, _) => {
                let fd = self.fuente(desp);
                let s = self.pedir()?;
                paso.push(s);
                self.acarreo(s, ri, fd);
                Como::Crudo { s, limite }
            }
            (Modo::Estructurado, _, Some(elementos), Some(pa)) => {
                let rd = self.registro(desp, paso)?;
                let (q, m) = (self.pedir()?, self.pedir()?);
                paso.extend([q, m]);
                self.poner(c::imad(q, ri, c::r(pa), rd, 0), Clase::Fma, Some(q), [Some(ri), Some(pa), Some(rd)]);
                self.poner_meta(c::isetp(P_VALE, Cmp::Lt, ri, c::r(elementos), true, 0), Meta { escribe_p: Some(P_VALE), ..Meta::de(Clase::Alu, None, [Some(ri), Some(elementos), None]) });
                self.poner_meta(c::sel(m, rd, Fuente::Imm(NO_VALE), P_VALE, false, 0), Meta { lee_p: Some((P_VALE, false)), ..Meta::de(Clase::Alu, Some(m), [Some(rd), None, None]) });
                Como::Estructurado { q, m, paso: pa }
            }
            _ => return Err(NoEmite::Operacion(k)),
        };
        Ok(Guia { base, tope, como })
    }

    /// `x = a + b`, o NO_VALE si se paso de 32 bits.
    fn acarreo(&mut self, x: u8, a: u8, b: Fuente) {
        self.poner_meta(c::iadd3_acarreo(x, P_ACARREO, a, b, 0), Meta { escribe_p: Some(P_ACARREO), ..Meta::de(Clase::Alu, Some(x), [Some(a), reg_de(b), None]) });
        self.poner_meta(c::sel(x, x, Fuente::Imm(NO_VALE), P_ACARREO, true, 0), Meta { lee_p: Some((P_ACARREO, false)), ..Meta::de(Clase::Alu, Some(x), [Some(x), None, None]) });
    }

    /// La palabra `j`: P_VALE si vale, y su direccion SUJETA en el par `a`.
    fn palabra(&mut self, gu: &Guia, j: u32, a: u8, x: u8) {
        let o = 4 * j;
        let desde = match gu.como {
            Como::Crudo { s, limite } => {
                let x = if j == 0 {
                    s
                } else {
                    self.acarreo(x, s, Fuente::Imm(o));
                    x
                };
                self.poner_meta(c::isetp(P_VALE, Cmp::Lt, x, c::r(limite), true, 0), Meta { escribe_p: Some(P_VALE), ..Meta::de(Clase::Alu, None, [Some(x), Some(limite), None]) });
                x
            }
            Como::Estructurado { q, m, paso } => {
                self.acarreo(x, m, Fuente::Imm(o + 4));
                self.poner_meta(c::isetp(P_VALE, Cmp::Le, x, c::r(paso), true, 0), Meta { escribe_p: Some(P_VALE), ..Meta::de(Clase::Alu, None, [Some(x), Some(paso), None]) });
                if j == 0 {
                    q
                } else {
                    self.poner(c::iadd3(x, q, Fuente::Imm(o), 0), Clase::Alu, Some(x), [Some(q), None, None]);
                    x
                }
            }
        };
        // La cadena que reconoce el juez: alinear, sujetar, sumar a la base.
        self.poner(c::lop3(a, desde, Fuente::Imm(PALABRA), c::Y, 0), Clase::Alu, Some(a), [Some(desde), None, None]);
        self.poner(c::imnmx(a, a, c::r(gu.tope), false, false, 0), Clase::Alu, Some(a), [Some(a), Some(gu.tope), None]);
        self.poner_meta(c::iadd3_acarreo(a, P_DIR, gu.base, c::r(a), 0), Meta { escribe_p: Some(P_DIR), ..Meta::de(Clase::Alu, Some(a), [Some(gu.base), Some(a), None]) });
        self.poner_meta(c::iadd3_x(a + 1, gu.base + 1, c::r(RZ), P_DIR, 0), Meta { lee_p: Some((P_DIR, false)), ..Meta::de(Clase::Alu, Some(a + 1), [Some(gu.base + 1), None, None]) });
    }

    /// El par de la direccion y un temporal, de paso.
    fn par_y_temporal(&mut self, paso: &mut Vec<u8>) -> Result<(u8, u8), NoEmite> {
        let a = self.bloque(2, false)?;
        let x = self.pedir()?;
        paso.extend([a, a + 1, x]);
        Ok((a, x))
    }

    /// **`d..d+4` = el UAV `u`** (`bufferLoad`): solo las palabras que se leen.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lee_uav(&mut self, d: Reg, u: u8, modo: Modo, i: Reg, desp: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let gu = self.guia(u, modo, i, desp, k, paso)?;
        let (a, x) = self.par_y_temporal(paso)?;
        for j in 0..4u32 {
            let r = d as usize + j as usize;
            if !self.variable.get(r).copied().unwrap_or(false) && self.ultimo.get(r).copied().unwrap_or(0) <= k {
                continue;
            }
            self.palabra(&gu, j, a, x);
            let v = self.destino(r as Reg, k)?;
            self.poner_meta(g::ldg(SIEMPRE, v, a, 0, 0), Meta { lee_tarde: true, ..Meta::de(Clase::Tex, Some(v), [Some(a), Some(a + 1), None]) });
            self.poner_meta(c::sel(v, v, c::r(RZ), P_VALE, false, 0), Meta { lee_p: Some((P_VALE, false)), ..Meta::de(Clase::Alu, Some(v), [Some(v), None, None]) });
        }
        Ok(())
    }

    /// **`d` = lo de antes; la palabra 0 del elemento = `como(antes, v)`**
    /// (`Interlocked*`; en el CAS, solo si lo de antes es `igual`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn atomico_uav(&mut self, d: Reg, u: u8, modo: Modo, i: Reg, desp: Reg, como: bmo_proton_x::bufer::Atomo, v: Reg, igual: Reg, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let rv = self.registro(v, paso)?;
        let ri = if como == bmo_proton_x::bufer::Atomo::CambiaSiIgual { Some(self.registro(igual, paso)?) } else { None };
        let gu = self.guia(u, modo, i, desp, k, paso)?;
        let (a, x) = self.par_y_temporal(paso)?;
        self.palabra(&gu, 0, a, x);
        let rd = self.destino(d, k)?;
        let mut w = match (super::compartida::atomo(como), ri) {
            (Some((op, signo)), _) => g::atomg(op, signo, rd, a, rv, 0),
            (None, Some(ig)) => g::atomg_cas(rd, a, ig, rv, 0),
            (None, None) => return Err(NoEmite::Operacion(k)),
        };
        w.0 = w.0 & !(0xF << 12) | (P_VALE as u64) << 12;
        let lee_mas = [ri, None, None];
        self.poner_meta(w, Meta { lee_p: Some((P_VALE, true)), lee_tarde: true, lee_mas, ..Meta::de(Clase::Tex, Some(rd), [Some(a), Some(a + 1), Some(rv)]) });
        self.poner_meta(c::sel(rd, rd, c::r(RZ), P_VALE, false, 0), Meta { lee_p: Some((P_VALE, false)), ..Meta::de(Clase::Alu, Some(rd), [Some(rd), None, None]) });
        Ok(())
    }

    /// **El UAV `u` = los canales de `v` que dice `mascara`** (`bufferStore`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn escribe_uav(&mut self, u: u8, modo: Modo, i: Reg, desp: Reg, v: [Reg; 4], mascara: u8, k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let gu = self.guia(u, modo, i, desp, k, paso)?;
        let (a, x) = self.par_y_temporal(paso)?;
        for j in 0..4u32 {
            if mascara & 1 << j == 0 {
                continue;
            }
            let rv = self.registro(v[j as usize], paso)?;
            self.palabra(&gu, j, a, x);
            self.poner_meta(g::stg(P_VALE, a, 0, rv, 0), Meta { lee_p: Some((P_VALE, true)), lee_tarde: true, ..Meta::de(Clase::Tex, None, [Some(a), Some(a + 1), Some(rv)]) });
        }
        Ok(())
    }
}
