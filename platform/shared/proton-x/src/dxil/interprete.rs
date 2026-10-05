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

/// **Donde se paro un hilo** (N5.5, 05-10): en una barrera, para seguir
/// cuando lleguen los demas del grupo. Un pixel nunca se para.
#[derive(Clone, Copy)]
pub struct Pausa {
    pc: usize,
    bucles: [usize; ANIDADO_MAXIMO],
    hondo: usize,
    /// El rango y el registro del ultimo `EligeTextura` (N5.4): tras la
    /// barrera se vuelve a buscar la misma textura.
    elige: Option<(u8, u32)>,
}

impl Pausa {
    /// La de un hilo que empieza.
    pub const AL_EMPEZAR: Pausa = Pausa { pc: 0, bucles: [0; ANIDADO_MAXIMO], hondo: 0, elige: None };
}

/// **Por que se paro** un hilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paro {
    /// Acabo: si el pixel QUEDA (`false`, si un `Descarta` lo tiro).
    Fin(bool),
    /// Llego a una `Barrera`: sigue con [`Programa::correr_desde`].
    Barrera,
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
    /// E2.3b: un sombreador de geometria, con lo que lleva emitido.
    Tiras(&'x mut Tiras),
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
        regs.clear();
        regs.extend_from_slice(&self.iniciales);
        let mut p = Pausa::AL_EMPEZAR;
        match self.correr_desde(&mut p, entradas, cb, rec, salidas, regs, Extra::Nada) {
            Paro::Fin(queda) => queda,
            // Una barrera fuera del computo no espera a nadie.
            Paro::Barrera => true,
        }
    }

    /// **Correr un sombreador de GEOMETRIA** (E2.3b) una vez, sobre una
    /// primitiva: `entradas` son sus vertices, cada uno [`Programa::entradas`]
    /// elementos seguidos; lo que emite queda en `tiras` (que se vacia antes).
    #[allow(clippy::too_many_arguments)]
    pub fn correr_gs(&self, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut Vec<f32>, tiras: &mut Tiras) {
        regs.clear();
        regs.extend_from_slice(&self.iniciales);
        tiras.vertices.clear();
        tiras.cortes.clear();
        let mut p = Pausa::AL_EMPEZAR;
        self.correr_desde(&mut p, entradas, cb, rec, salidas, regs, Extra::Tiras(tiras));
    }

    /// **Correr un hilo desde `p`** (N5.5): hasta el final, o hasta una
    /// `Barrera` si es de computo (`Extra::Grupo`); `p` queda donde se paro.
    /// Los registros, `regs`, son los del hilo: quien llama los guarda entre
    /// una barrera y la siguiente. Un GS (E2.3b) emite en `Extra::Tiras`.
    #[allow(clippy::too_many_arguments)]
    pub fn correr_desde(&self, p: &mut Pausa, entradas: &[[f32; 4]], cb: &[u8], rec: &crate::textura::Recursos, salidas: &mut [[f32; 4]], regs: &mut [f32], mut x: Extra) -> Paro {
        let bits = |regs: &[f32], r: Reg| regs[r as usize].to_bits();
        // Donde empieza cada bucle abierto (la forma ya se comprobo).
        let Pausa { mut pc, mut bucles, mut hondo, mut elige } = *p;
        // N5.4: la textura que eligio el ultimo `EligeTextura` (la de antes
        // de la barrera, si el hilo viene de una).
        let mut elegida: Option<crate::textura::Textura> = elige.and_then(|(r, k)| rec.dinamica(r, k));
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
                    hondo += 1;
                }
                Op::FinBucle => pc = bucles[hondo - 1],
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
                Op::Continuar => pc = bucles[hondo - 1],
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
                        *p = Pausa { pc, bucles, hondo, elige };
                        return Paro::Barrera;
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
                Op::EscribeUav { u, modo, i, desp, v, mascara } => {
                    let (k, o, v) = (bits(regs, i), bits(regs, desp), v.map(|r| bits(regs, r)));
                    if let Extra::Grupo(g) = &mut x {
                        if let Some(Some(w)) = g.uavs.get_mut(u as usize) {
                            w.escribir(modo, k, o, v, mascara);
                        }
                    }
                }
                Op::LeeUav { d, u, modo, i, desp } => {
                    let (k, o) = (bits(regs, i), bits(regs, desp));
                    let v = match &x {
                        Extra::Grupo(g) => match g.uavs.get(u as usize) {
                            Some(Some(w)) => w.cargar(modo, k, o),
                            _ => [0; 4],
                        },
                        _ => [0; 4],
                    };
                    for (j, w) in v.into_iter().enumerate() {
                        regs[d as usize + j] = f32::from_bits(w);
                    }
                }
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
                // E2.4: el contador del UAV (fuera del computo, 0).
                Op::Contador { d, u, inc } => {
                    let v = match &mut x {
                        Extra::Grupo(g) => match g.uavs.get_mut(u as usize) {
                            Some(Some(w)) => w.contar(inc),
                            _ => 0,
                        },
                        _ => 0,
                    };
                    regs[d as usize] = f32::from_bits(v);
                }
                // E2.3b: el GS lee el vertice `vertice` de su primitiva.
                Op::EntradaDe { d, vertice, elemento, componente } => {
                    let i = vertice as usize * self.entradas + elemento as usize;
                    regs[d as usize] = entradas.get(i).map(|e| e[componente as usize & 3]).unwrap_or(0.0);
                }
                Op::Emite { flujo } => {
                    if let Extra::Tiras(t) = &mut x {
                        if flujo == 0 && t.emitidos() < t.maximo {
                            let n = t.salidas;
                            t.vertices.extend((0..n).map(|k| salidas.get(k).copied().unwrap_or([0.0; 4])));
                        }
                    }
                }
                Op::Corta { flujo } => {
                    if let Extra::Tiras(t) = &mut x {
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
                Op::Muestra { d, t, s, u, v } => {
                    let (unica, solo);
                    let (rec, t) = if t == super::programa::DINAMICA {
                        unica = [elegida];
                        solo = crate::textura::Recursos { texturas: &unica, muestreadores: rec.muestreadores, buferes: &[], dinamicas: None };
                        (&solo, 0)
                    } else {
                        (rec, t)
                    };
                    let c = rec.muestrear(t, s, regs[u as usize], regs[v as usize]);
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
            }
        }
        *p = Pausa { pc, bucles, hondo, elige };
        Paro::Fin(true)
    }
}
