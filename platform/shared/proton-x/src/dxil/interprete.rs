//! **El interprete** (P3b3; partido de `programa.rs` el 03-10): un
//! [`Programa`] corrido sobre floats, una operacion tras otra -- el JUEZ de
//! lo que hagan la 3060 y el x86-64 traducido.
//!
//! [carril]  VERDE     cuentas sobre un vector de registros; no toca la maquina
//! [cuesta]  DATO      una operacion mal hecha pinta otro color
//! [riesgo]  ESPEJO    las reglas son las de D3D (NaN, enteros, saltos); el
//!                     banco las compara con las huellas de la 3060
//! [consumo] NADA      solo cuando se dibuja por la CPU

use alloc::vec::Vec;

use super::programa::{raiz, saturar, Lectura, Op, Programa, Reg, ANIDADO_MAXIMO};
use crate::bufer::Modo;

/// **Donde se paro un hilo** (N5.5, 05-10): en una barrera, para seguir
/// cuando lleguen los demas del grupo. Un pixel nunca se para.
#[derive(Clone, Copy)]
pub struct Pausa {
    pc: usize,
    bucles: [usize; ANIDADO_MAXIMO],
    hondo: usize,
    /// E2.5: la vuelta de cada bucle abierto (0 la primera): dos carriles
    /// en la misma operacion de ola van JUNTOS solo si van en la misma.
    vueltas: [u32; ANIDADO_MAXIMO],
    /// El rango y el registro del ultimo `EligeTextura` (N5.4): tras la
    /// barrera se vuelve a buscar la misma textura.
    elige: Option<(u8, u32)>,
}

impl Pausa {
    /// La de un hilo que empieza.
    pub const AL_EMPEZAR: Pausa = Pausa { pc: 0, bucles: [0; ANIDADO_MAXIMO], hondo: 0, vueltas: [0; ANIDADO_MAXIMO], elige: None };

    /// E2.5: la operacion en la que se paro (la de antes de `pc`).
    pub fn operacion(&self) -> usize {
        self.pc.wrapping_sub(1)
    }

    /// **E2.5: quien va antes** en el programa, como lo correria una ola
    /// (SIMT): de fuera a dentro, en el mismo bucle la vuelta menor; un
    /// carril dentro de un bucle va antes que uno parado DETRAS de el (y
    /// despues que uno parado delante); si no, el que esta mas arriba. Con
    /// `si` y bucles estructurados (`Programa::forma`), dos carriles en la
    /// misma operacion y `Equal` van por el mismo camino: los activos.
    pub fn orden(&self, otra: &Pausa) -> core::cmp::Ordering {
        use core::cmp::Ordering::{Greater, Less};
        let n = self.hondo.min(otra.hondo);
        for k in 0..n {
            if self.bucles[k] != otra.bucles[k] {
                return self.bucles[k].cmp(&otra.bucles[k]);
            }
            if self.vueltas[k] != otra.vueltas[k] {
                return self.vueltas[k].cmp(&otra.vueltas[k]);
            }
        }
        if self.hondo > n {
            return if otra.pc < self.bucles[n] { Greater } else { Less };
        }
        if otra.hondo > n {
            return if self.pc < otra.bucles[n] { Less } else { Greater };
        }
        self.pc.cmp(&otra.pc)
    }
}

/// **Por que se paro** un hilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paro {
    /// Acabo: si el pixel QUEDA (`false`, si un `Descarta` lo tiro).
    Fin(bool),
    /// Llego a una `Barrera`: sigue con [`Programa::correr_desde`].
    Barrera,
    /// E2.5: llego a una operacion de ola: la resuelve quien corre la ola
    /// (`dxil/carriles.rs`), y sigue con [`Programa::correr_desde`].
    Ola,
}

/// **Los ids de un hilo de computo** (N5.5): SV_DispatchThreadID,
/// SV_GroupID, SV_GroupThreadID y SV_GroupIndex.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ids {
    pub despacho: [u32; 3],
    pub grupo: [u32; 3],
    pub en_grupo: [u32; 3],
    pub indice: u32,
}

/// **Lo que ve un hilo de computo** ademas de lo de un pixel: sus ids, la
/// memoria compartida de su grupo y los UAV del despacho (por ranura).
pub struct Grupo<'a, 'b> {
    pub ids: Ids,
    pub compartida: &'a mut [u32],
    pub uavs: &'a mut [Option<crate::bufer::Uav<'b>>],
}

/// **Lo que emite un sombreador de GEOMETRIA** (E2.3b, 05-10): sus vertices
/// (cada uno, `salidas` elementos seguidos: las salidas al emitirlo) y donde
/// acaba cada tira. Lo que pase de `maximo` (`[maxvertexcount]`) se pierde,
/// como en D3D12.
#[derive(Debug, Default)]
pub struct Tiras {
    pub salidas: usize,
    pub maximo: usize,
    pub vertices: Vec<[f32; 4]>,
    /// Cuantos vertices iban emitidos al cortar cada tira.
    pub cortes: Vec<usize>,
}

impl Tiras {
    /// Los vertices emitidos.
    pub fn emitidos(&self) -> usize {
        self.vertices.len() / self.salidas.max(1)
    }

    /// Cortar la tira de ahora (si tiene algo).
    pub fn cortar(&mut self) {
        let n = self.emitidos();
        if self.cortes.last().copied().unwrap_or(0) != n {
            self.cortes.push(n);
        }
    }

    /// **Los triangulos de sus tiras**, como indices de vertice: cada uno
    /// nuevo con los dos de antes, los impares dados la vuelta (lo de una
    /// tira de D3D). Una tira de menos de tres no da ninguno.
    pub fn triangulos(&self) -> Vec<[u32; 3]> {
        let mut v = Vec::new();
        let mut desde = 0usize;
        for &hasta in self.cortes.iter().chain(core::iter::once(&self.emitidos())) {
            for i in desde..hasta.saturating_sub(2) {
                let i = i as u32;
                v.push(if (i as usize - desde) % 2 == 0 { [i, i + 1, i + 2] } else { [i + 1, i, i + 2] });
            }
            desde = desde.max(hasta);
        }
        v
    }
}

/// **Lo que ve un hilo ademas de sus entradas y sus salidas**, segun su
/// etapa.
pub enum Extra<'x, 'a, 'b> {
    /// Un vertice o un pixel: nada mas.
    Nada,
    /// N5.5: un hilo de computo, con su grupo.
    Grupo(&'x mut Grupo<'a, 'b>),
    /// E2.3b: un sombreador de geometria, con lo que lleva emitido y
    /// (05-10) los UAV del dibujo.
    Tiras(&'x mut Tiras, &'x mut [Option<crate::bufer::Uav<'b>>]),
    /// 05-10: un vertice o un pixel de un dibujo con UAV (`RWTexture2D`,
    /// `RWByteAddressBuffer`...): los del dibujo, por ranura.
    Uavs(&'x mut [Option<crate::bufer::Uav<'b>>]),
    /// E2.5: un pixel que va en una ola (`crate::cuadros`), con los UAV del
    /// dibujo (un AYUDANTE, ninguno: lo que escribe se pierde, como en D3D):
    /// se para en cada operacion de ola, como uno de computo.
    Ola(&'x mut [Option<crate::bufer::Uav<'b>>]),
}

impl<'b> Extra<'_, '_, 'b> {
    /// Los UAV que ve el hilo: los del Dispatch o los del dibujo.
    fn uavs(&mut self) -> &mut [Option<crate::bufer::Uav<'b>>] {
        match self {
            Extra::Grupo(g) => g.uavs,
            Extra::Uavs(u) | Extra::Ola(u) | Extra::Tiras(_, u) => u,
            _ => &mut [],
        }
    }
}

impl Programa {
    /// Desde el `Si` (o el `SiNo`) `i`: el indice tras su `SiNo` (si
    /// `hasta_sino`) o tras su `FinSi`.
    fn tras_si(&self, i: usize, hasta_sino: bool) -> usize {
        let mut hondo = 0usize;
        for (k, op) in self.ops.iter().enumerate().skip(i + 1) {
            match op {
                Op::Si { .. } => hondo += 1,
                Op::SiNo if hondo == 0 && hasta_sino => return k + 1,
                Op::FinSi if hondo == 0 => return k + 1,
                Op::FinSi => hondo -= 1,
                _ => {}
            }
        }
        self.ops.len()
    }

    /// Desde dentro del bucle (en `i`): el indice tras su `FinBucle`.
    fn tras_bucle(&self, i: usize) -> usize {
        let mut hondo = 0usize;
        for (k, op) in self.ops.iter().enumerate().skip(i + 1) {
            match op {
                Op::Bucle => hondo += 1,
                Op::FinBucle if hondo == 0 => return k + 1,
                Op::FinBucle => hondo -= 1,
                _ => {}
            }
        }
        self.ops.len()
    }

    /// [`Programa::correr`] con las texturas y los muestreadores del dibujo.
    /// Una textura o un muestreador que no esta da (0, 0, 0, 0), como un SRV
    /// nulo en D3D12. Devuelve si el pixel QUEDA: `false` si un
    /// [`Op::Descarta`] lo tiro (N5.7); un programa sin ellos, siempre `true`.
    pub fn correr_con(&self, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut Vec<f32>) -> bool {
        self.correr_con_uavs(entradas, cb, rec, salidas, regs, &mut [])
    }

    /// **Lo mismo, con los UAV de un dibujo** (05-10): lo que escriba en ellos
    /// queda (un pixel tirado por un `discard` deja lo que escribio ANTES de
    /// el, y nada de despues: lo de D3D).
    pub fn correr_con_uavs(&self, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut Vec<f32>, uavs: &mut [Option<crate::bufer::Uav>]) -> bool {
        regs.clear();
        regs.extend_from_slice(&self.iniciales);
        let mut p = Pausa::AL_EMPEZAR;
        match self.correr_desde(&mut p, entradas, cb, rec, salidas, regs, Extra::Uavs(uavs)) {
            Paro::Fin(queda) => queda,
            // Una barrera fuera del computo no espera a nadie (y con
            // `Extra::Nada` una ola no para: es de un carril).
            Paro::Barrera | Paro::Ola => true,
        }
    }

    /// **Correr un sombreador de GEOMETRIA** (E2.3b) una vez, sobre una
    /// primitiva: `entradas` son sus vertices, cada uno [`Programa::entradas`]
    /// elementos seguidos; lo que emite queda en `tiras` (que se vacia antes),
    /// y lo que escribe en sus UAV (05-10), en `uavs`.
    #[allow(clippy::too_many_arguments)]
    pub fn correr_gs(&self, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut Vec<f32>, tiras: &mut Tiras, uavs: &mut [Option<crate::bufer::Uav>]) {
        regs.clear();
        regs.extend_from_slice(&self.iniciales);
        tiras.vertices.clear();
        tiras.cortes.clear();
        let mut p = Pausa::AL_EMPEZAR;
        self.correr_desde(&mut p, entradas, cb, rec, salidas, regs, Extra::Tiras(tiras, uavs));
    }

    /// **Correr un hilo desde `p`** (N5.5): hasta el final, o hasta una
    /// `Barrera` si es de computo (`Extra::Grupo`); `p` queda donde se paro.
    /// Los registros, `regs`, son los del hilo: quien llama los guarda entre
    /// una barrera y la siguiente. Un GS (E2.3b) emite en `Extra::Tiras`.
    #[allow(clippy::too_many_arguments)]
    pub fn correr_desde(&self, p: &mut Pausa, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut [f32], mut x: Extra) -> Paro {
        let bits = |regs: &[f32], r: Reg| regs[r as usize].to_bits();
        // Donde empieza cada bucle abierto (la forma ya se comprobo).
        let Pausa { mut pc, mut bucles, mut hondo, mut vueltas, mut elige } = *p;
        // N5.4: la textura que eligio el ultimo `EligeTextura` (la de antes
        // de la barrera, si el hilo viene de una).
        let mut elegida: Option<crate::textura::Textura<'static>> = elige.and_then(|(r, k)| rec.dinamica(r, k));
        while let Some(op) = self.ops.get(pc) {
            pc += 1;
            match *op {
                Op::Compara { d, a, b, como, entero } => {
                    let si = if entero { como.enteros(bits(regs, a) as i32, bits(regs, b) as i32) } else { como.floats(regs[a as usize], regs[b as usize]) };
                    regs[d as usize] = f32::from_bits(if si { u32::MAX } else { 0 });
                }
                Op::Elige { d, c, a, b } => regs[d as usize] = f32::from_bits(if bits(regs, c) != 0 { bits(regs, a) } else { bits(regs, b) }),
                Op::Copia { d, a } => regs[d as usize] = f32::from_bits(bits(regs, a)),
                Op::SumaEntera { d, a, b } => regs[d as usize] = f32::from_bits(bits(regs, a).wrapping_add(bits(regs, b))),
                Op::Entera { d, a, b, op } => regs[d as usize] = f32::from_bits(op.hacer(bits(regs, a), bits(regs, b))),
                Op::Convierte { d, a, como } => regs[d as usize] = f32::from_bits(como.hacer(bits(regs, a))),
                Op::Si { c } => {
                    if bits(regs, c) == 0 {
                        pc = self.tras_si(pc - 1, true);
                    }
                }
                // Se llega al SiNo corriendo la rama del si: la otra, no.
                Op::SiNo => pc = self.tras_si(pc - 1, false),
                Op::FinSi => {}
                Op::Bucle => {
                    bucles[hondo] = pc;
                    vueltas[hondo] = 0;
                    hondo += 1;
                }
                Op::FinBucle | Op::Continuar => {
                    pc = bucles[hondo - 1];
                    vueltas[hondo - 1] = vueltas[hondo - 1].wrapping_add(1);
                }
                Op::RomperSi { c, si_cero } => {
                    if (bits(regs, c) == 0) == si_cero {
                        pc = self.tras_bucle(pc - 1);
                        hondo -= 1;
                    }
                }
                Op::Romper => {
                    pc = self.tras_bucle(pc - 1);
                    hondo -= 1;
                }
                Op::Descarta { c } => {
                    if bits(regs, c) != 0 {
                        return Paro::Fin(false);
                    }
                }
                // N5.5: el computo.
                Op::IdHilo { d, que, c } => {
                    let v = match &x {
                        Extra::Grupo(g) => match que {
                            0 => g.ids.despacho[c as usize],
                            1 => g.ids.grupo[c as usize],
                            2 => g.ids.en_grupo[c as usize],
                            _ => g.ids.indice,
                        },
                        _ => 0,
                    };
                    regs[d as usize] = f32::from_bits(v);
                }
                Op::Barrera => {
                    if matches!(x, Extra::Grupo(_)) {
                        *p = Pausa { pc, bucles, hondo, vueltas, elige };
                        return Paro::Barrera;
                    }
                }
                // E2.5: en una ola (computo, o un pixel de `cuadros`), parar:
                // la resuelve `carriles.rs` con los demas. Si no (vertices,
                // GS), el hilo es una ola de 32 con UN carril activo, el 0.
                Op::Ola { d, a, b, que } => {
                    if matches!(x, Extra::Grupo(_) | Extra::Ola(_)) {
                        *p = Pausa { pc, bucles, hondo, vueltas, elige };
                        return Paro::Ola;
                    }
                    let (va, vb) = (bits(regs, a), bits(regs, b));
                    let r = super::olas::hacer(que, 0, 1, |_| va, vb);
                    for (k, v) in r.into_iter().enumerate().take(super::olas::anchura(que)) {
                        regs[d as usize + k] = f32::from_bits(v);
                    }
                }
                Op::LeeCompartida { d, base, n, i } => {
                    let k = bits(regs, i);
                    let v = match &x {
                        Extra::Grupo(g) if k < n => g.compartida.get((base + k) as usize).copied().unwrap_or(0),
                        _ => 0,
                    };
                    regs[d as usize] = f32::from_bits(v);
                }
                Op::EscribeCompartida { base, n, i, s } => {
                    let (k, v) = (bits(regs, i), bits(regs, s));
                    if let Extra::Grupo(g) = &mut x {
                        if let Some(w) = g.compartida.get_mut((base + k) as usize).filter(|_| k < n) {
                            *w = v;
                        }
                    }
                }
                // N5.5 y, desde el 05-10, en un dibujo (`Extra::Uavs`). 06-10:
                // en una funcion (`operar_uav`): la llama tambien el computo
                // traducido, y asi da los MISMOS bits.
                Op::EscribeUav { .. } | Op::Atomico { .. } | Op::MedidasUav { .. } | Op::LeeUav { .. } | Op::Contador { .. } => operar_uav(*op, regs, x.uavs()),
                Op::ConstantesEn { d, fila, filas, i, .. } => {
                    let k = bits(regs, i);
                    for c in 0..4 {
                        let o = (fila as usize + k as usize) * 16 + 4 * c;
                        regs[d as usize + c] = if k < filas as u32 { cb.get(o..o + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0.0) } else { 0.0 };
                    }
                }
                Op::LeeIndexado { d, base, n, i } => {
                    let k = bits(regs, i);
                    regs[d as usize] = if k < n as u32 { regs[base as usize + k as usize] } else { 0.0 };
                }
                Op::EscribeIndexado { base, n, i, s } => {
                    let k = bits(regs, i);
                    if k < n as u32 {
                        regs[base as usize + k as usize] = regs[s as usize];
                    }
                }
                Op::Entrada { d, elemento, componente } => {
                    regs[d as usize] = entradas.get(elemento as usize).map(|e| e[componente as usize & 3]).unwrap_or(0.0);
                }
                // E2.4: el contador del UAV (sin UAV, 0).
                // E2.3b: el GS lee el vertice `vertice` de su primitiva.
                Op::EntradaDe { d, vertice, elemento, componente } => {
                    let i = vertice as usize * self.entradas + elemento as usize;
                    regs[d as usize] = entradas.get(i).map(|e| e[componente as usize & 3]).unwrap_or(0.0);
                }
                Op::Emite { flujo } => {
                    if let Extra::Tiras(t, _) = &mut x {
                        if flujo == 0 && t.emitidos() < t.maximo {
                            let n = t.salidas;
                            t.vertices.extend((0..n).map(|k| salidas.get(k).copied().unwrap_or([0.0; 4])));
                        }
                    }
                }
                Op::Corta { flujo } => {
                    if let Extra::Tiras(t, _) = &mut x {
                        if flujo == 0 {
                            t.cortar();
                        }
                    }
                }
                Op::Salida { s, elemento, componente } => {
                    if let Some(e) = salidas.get_mut(elemento as usize) {
                        e[componente as usize & 3] = regs[s as usize];
                    }
                }
                Op::Constantes { d, fila, .. } => {
                    for k in 0..4 {
                        let o = fila as usize * 16 + 4 * k;
                        regs[d as usize + k] = cb.get(o..o + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0.0);
                    }
                }
                Op::Mul { d, a, b } => regs[d as usize] = regs[a as usize] * regs[b as usize],
                Op::Add { d, a, b } => regs[d as usize] = regs[a as usize] + regs[b as usize],
                Op::Sub { d, a, b } => regs[d as usize] = regs[a as usize] - regs[b as usize],
                Op::Div { d, a, b } => regs[d as usize] = regs[a as usize] / regs[b as usize],
                Op::Mad { d, a, b, c } => {
                    let p = regs[a as usize] * regs[b as usize];
                    regs[d as usize] = p + regs[c as usize];
                }
                Op::Dot { d, n, a, b } => {
                    let mut s = regs[a[0] as usize] * regs[b[0] as usize];
                    for k in 1..n as usize {
                        s = s + regs[a[k] as usize] * regs[b[k] as usize];
                    }
                    regs[d as usize] = s;
                }
                Op::Rsqrt { d, a } => regs[d as usize] = 1.0 / raiz(regs[a as usize]),
                Op::Sqrt { d, a } => regs[d as usize] = raiz(regs[a as usize]),
                Op::Saturate { d, a } => regs[d as usize] = saturar(regs[a as usize]),
                Op::Abs { d, a } => regs[d as usize] = f32::from_bits(regs[a as usize].to_bits() & 0x7FFF_FFFF),
                Op::Mate { d, a, f } => regs[d as usize] = f32::from_bits(f.aplicar(regs[a as usize].to_bits())),
                // FMin/FMax de D3D: si uno es NaN, el otro.
                Op::Min { d, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[d as usize] = if x.is_nan() || y < x { y } else { x };
                }
                Op::Max { d, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[d as usize] = if x.is_nan() || y > x { y } else { x };
                }
                Op::EligeTextura { i, rango } => {
                    elige = Some((rango, bits(regs, i)));
                    elegida = rec.dinamica(rango, bits(regs, i));
                }
                // X2 (05-10): la lectura, en una funcion: la llama tambien el
                // codigo traducido (`nativo_llamadas`), y asi da los MISMOS bits.
                Op::Muestra { .. } | Op::Lee { .. } => leer_textura(*op, regs, rec, elegida),
            }
        }
        *p = Pausa { pc, bucles, hondo, vueltas, elige };
        Paro::Fin(true)
    }
}

/// **Una operacion de UAV** (`EscribeUav`, `Atomico`, `MedidasUav`,
/// `LeeUav` o `Contador`) sobre los registros y los UAV de quien corre. 06-10:
/// fuera de [`Programa::correr_desde`] para que el computo traducido la
/// LLAME (`nativo_computo`: las ranuras con atomicos, medidas o texturas)
/// en vez de copiarla: el mismo Rust, los mismos bits. Otra operacion, nada.
pub fn operar_uav(op: Op, regs: &mut [f32], uavs: &mut [Option<crate::bufer::Uav>]) {
    let bits = |regs: &[f32], r: Reg| regs[r as usize].to_bits();
    match op {
        Op::EscribeUav { u, modo, i, desp, z, v, mascara } => {
            let (k, o, v) = (bits(regs, i), bits(regs, desp), v.map(|r| bits(regs, r)));
            if let Some(Some(w)) = uavs.get_mut(u as usize) {
                // 06-10: en una textura, su rebanada z (3D o array).
                match modo {
                    Modo::Textura => {
                        if let Some(mut r) = w.rebanada(bits(regs, z)) {
                            r.escribir(modo, k, o, v, mascara);
                        }
                    }
                    _ => w.escribir(modo, k, o, v, mascara),
                }
            }
        }
        Op::Atomico { d, u, modo, i, desp, z, como, v, igual } => {
            let (k, o, v, igual) = (bits(regs, i), bits(regs, desp), bits(regs, v), bits(regs, igual));
            let antes = match uavs.get_mut(u as usize) {
                Some(Some(w)) if modo == Modo::Textura => w.rebanada(bits(regs, z)).map_or(0, |mut r| r.atomico(modo, k, o, como, v, igual)),
                Some(Some(w)) => w.atomico(modo, k, o, como, v, igual),
                _ => 0,
            };
            regs[d as usize] = f32::from_bits(antes);
        }
        Op::MedidasUav { d, u, modo } => {
            let v = match uavs.get(u as usize) {
                Some(Some(w)) if modo == Modo::Textura => w.medidas_textura(),
                Some(Some(w)) => crate::bufer::Bufer { bytes: w.bytes, formato: w.formato, paso: w.paso, elementos: w.elementos }.medidas(modo),
                _ => [0; 4],
            };
            for (j, w) in v.into_iter().enumerate() {
                regs[d as usize + j] = f32::from_bits(w);
            }
        }
        Op::LeeUav { d, u, modo, i, desp, z } => {
            let (k, o) = (bits(regs, i), bits(regs, desp));
            let v = match uavs.get_mut(u as usize) {
                Some(Some(w)) if modo == Modo::Textura => w.rebanada(bits(regs, z)).map_or([0; 4], |r| r.cargar(modo, k, o)),
                Some(Some(w)) => w.cargar(modo, k, o),
                _ => [0; 4],
            };
            for (j, w) in v.into_iter().enumerate() {
                regs[d as usize + j] = f32::from_bits(w);
            }
        }
        Op::Contador { d, u, inc } => {
            let v = match uavs.get_mut(u as usize) {
                Some(Some(w)) => w.contar(inc),
                _ => 0,
            };
            regs[d as usize] = f32::from_bits(v);
        }
        _ => {}
    }
}

/// **Una lectura de textura** ([`Op::Muestra`] o [`Op::Lee`]) sobre los
/// registros, con la textura `elegida` por el ultimo `EligeTextura` (N5.4).
/// X2 (05-10): fuera de [`Programa::correr_desde`] para que el codigo
/// traducido la LLAME (`nativo_llamadas::textura`) en vez de copiarla: el
/// mismo Rust, los mismos bits. Otra operacion, nada.
pub fn leer_textura(op: Op, regs: &mut [f32], rec: &crate::textura::Recursos, elegida: Option<crate::textura::Textura<'static>>) {
    match op {
        Op::Muestra { d, t, s, u, v, g } => {
            let (unica, solo);
            let (rec, t) = if t == super::programa::DINAMICA {
                unica = [elegida];
                solo = crate::textura::Recursos { texturas: &unica, muestreadores: rec.muestreadores, buferes: &[], dinamicas: None };
                (&solo, 0)
            } else {
                (rec, t)
            };
            // D4.4: la mip de sus gradientes (las derivadas de antes; en el
            // codigo traducido, 0: la casa solo lo usa si la mip no importa).
            let c = match g {
                Some(g) => rec.muestrear_grad(t, s, [regs[u as usize], regs[v as usize], 0.0, 0.0], core::array::from_fn(|k| regs[g as usize + k]), [0.0; 2], [0; 3]),
                None => rec.muestrear(t, s, regs[u as usize], regs[v as usize]),
            };
            regs[d as usize..d as usize + 4].copy_from_slice(&c);
        }
        Op::Lee { d, t, s, como, c, nivel, desp } => {
            // La ELEGIDA, en la ranura 0 de unos recursos de una.
            let (unica, solo);
            let (rec, t) = if t == super::programa::DINAMICA {
                unica = [elegida];
                solo = crate::textura::Recursos { texturas: &unica, muestreadores: rec.muestreadores, buferes: &[], dinamicas: None };
                (&solo, 0)
            } else {
                (rec, t)
            };
            let f = c.map(|r| regs[r as usize]);
            let b = |r: Reg| regs[r as usize].to_bits();
            let x = match como {
                Lectura::Muestra => rec.muestrear_en(t, s, f, None, desp).map(f32::to_bits),
                Lectura::Nivel => rec.muestrear_en(t, s, f, Some(regs[nivel as usize]), desp).map(f32::to_bits),
                Lectura::Carga { enteros } => rec.cargar(t, [b(c[0]) as i32, b(c[1]) as i32, b(c[2]) as i32], b(nivel) as i32, desp, enteros),
                Lectura::Medidas => rec.medidas(t, b(nivel)),
                Lectura::Bufer(modo) => rec.cargar_bufer(t, modo, b(c[0]), b(c[1])),
                Lectura::MedidasBufer(modo) => rec.medidas_bufer(t, modo),
                Lectura::Junta { canal } => rec.juntar(t, s, f, canal as usize, desp).map(f32::to_bits),
                Lectura::Compara => [rec.comparar(t, s, f, regs[nivel as usize], desp).to_bits(); 4],
                // D4.4: el bloque de `nivel` (ver `Lectura::Gradientes`).
                Lectura::Gradientes { compara } => {
                    let n = |k: usize| regs[nivel as usize + k];
                    let (g, sesgo_clamp) = ([n(0), n(1), n(2), n(3)], [n(4), n(5)]);
                    if compara {
                        [rec.comparar_grad(t, s, f, n(6), g, sesgo_clamp, desp).to_bits(); 4]
                    } else {
                        rec.muestrear_grad(t, s, f, g, sesgo_clamp, desp).map(f32::to_bits)
                    }
                }
                Lectura::Lod { sujeta } => [rec.lod(t, s, core::array::from_fn(|k| regs[nivel as usize + k]), sujeta).to_bits(), 0, 0, 0],
                Lectura::JuntaCompara { canal } => {
                    // Cada texel contra la referencia, con la funcion del muestreador.
                    let g = rec.juntar(t, s, f, canal as usize, desp);
                    let m = rec.muestreadores.get(s as usize).copied().flatten();
                    let fun = m.map_or(4, |m| if m.comparacion == 0 { 4 } else { m.comparacion });
                    g.map(|x| if (crate::trama::Profundidad { funcion: fun, escribir: false }).pasa(regs[nivel as usize], x) { 1.0f32.to_bits() } else { 0 })
                }
            };
            for (k, v) in x.into_iter().enumerate() {
                regs[d as usize + k] = f32::from_bits(v);
            }
        }
        _ => {}
    }
}
