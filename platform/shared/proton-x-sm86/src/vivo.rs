//! **A9 (06-10): EL .BSF VIVO** -- la CPU entiende el MAPA de un PSO, genera
//! ahi mismo el codigo de la 3060, lo COMPRUEBA bit a bit contra su
//! interprete, y lo RECUERDA en un `.bsf` (en el banco, en la memoria; en
//! BMO-X, en ESTRATOS) para no volver a hacerlo.
//!
//! Lo pidio el propietario (06-10): *"que mi CPU entienda el mapa y guie al
//! GPU que genere en .bsf para generar datos precisos por completo"*, y que
//! se guarde *"en ESTRATOS para que no se olvide"*. Es la pieza A9 del
//! contador de `docs/plan/PLAN_LAS_TRES_GRANDES.md` (la cache de PSO) y el
//! primer paso de VC1 de `docs/plan/PLAN_VERRANO.md` (el paquete de un PSO).
//!
//! ```text
//!    el MAPA      lo que la CPU entiende del PSO: sus dos Programas (de
//!                 DXIL o de SM5), en un texto canonico con los bits de cada
//!                 constante, y la version del emisor. Su hash es el NOMBRE
//!                 del .bsf: el mismo mapa, el mismo fichero
//!    generar      el emisor (`emitir_con`, ABI de registros), como siempre
//!    COMPROBAR    el simulador de la 3060 (`simula`) corre lo emitido y el
//!                 interprete de la casa corre el Programa, con las mismas
//!                 entradas y el mismo cbuffer (cuatro tandas de numeros
//!                 de prueba): si UNA salida difiere en UN bit, ese PSO no va
//!                 a la 3060 (va por la CPU, y se dice) y no se guarda
//!    el .bsf      dos modulos ("vs", "ps") cuya fuente es el MAPA
//!                 (`bmo_bsf::MAPA_MAGIC`) y cuyo objetivo es el cuerpo de la
//!                 puerta (`abi::SM86_PUERTA_V1`): el sobre sella cada cosa
//!                 con su hash
//!    recordar     la vez siguiente (otro arranque del juego) se lee el
//!                 .bsf, se comprueba el sobre, que su mapa sea EXACTAMENTE
//!                 el de ahora, y se vuelve a comprobar bit a bit (barato:
//!                 unas pocas corridas); traducir, cero
//! ```
//!
//! **El JUEZ en tres niveles** (06-10, lo que pidio el propietario: "ese
//! mismo juez pueda aportar ... cuando ya ejecuta en tiempo real, que sea el
//! intermedio que esta en VERRANO"). Todo codigo de la 3060 pasa por la
//! PUERTA de VERRANO, y ahi lo juzgan los tres antes de que la tarjeta lo vea:
//!
//! ```text
//!    1  LA FORMA        el juez del SASS (`bmo_gpu_ga10x::sass::juez`): que
//!                       no cuelgue la 3060 (esperas, barreras, registros);
//!                       al pegar cada receta, en la puerta y en el kernel
//!    2  LOS BITS        [`comprobar`]: el simulador contra el interprete con
//!                       numeros de prueba, al GENERAR el .bsf (si no, no se
//!                       guarda)
//!    3  EL JUEGO        [`revisar`], el VIGIA (A9c, el modo dinamico): con
//!                       los vertices y las constantes DEL JUEGO, en el primer
//!                       lote de cada PSO y luego uno de cada
//!                       `puerta::VIGIA_CADA`; si no cuadra, ese PSO va por la
//!                       CPU desde ESE lote y queda marcado `.malo`
//! ```
//!
//! Lo que NO es todavia, dicho: la parte de la CPU del paquete (el `.bex`:
//! el `Programa` y su x86-64 de `nativo`) se sigue haciendo en cada arranque;
//! el de pixeles que MUESTREA se comprueba con las texturas a cero (lo que
//! lee una textura de verdad lo juzgan los jueces de `prueba/`); y la
//! LIBRETA de la GPU (que la 3060 misma apunte lo raro mientras dibuja)
//! tiene hecha la mitad de la app (9d, 06-10: [`crate::libreta`], el
//! termometro que esto tambien comprueba) y le falta la del kernel: hoy el
//! vigia es la CPU, con lotes de muestra.
//!
//! capa: puro -- bytes y cuentas; quien guarda y lee es un [`Recuerdo`]

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::lote::Enlace;

use crate::simula::{correr, Maquina};
use crate::{Emitido, Precarga};

/// La version del emisor que entra en el MAPA: se SUBE cada vez que el
/// emisor o su ABI de registros dan otro codigo para el mismo Programa. Un
/// .bsf de otra version tiene otro nombre (y ademas se vuelve a comprobar
/// bit a bit al cargarlo: una version que se olvido de subir no pasa).
pub const VERSION_EMISOR: u32 = 1;

/// **Quien guarda y lee los .bsf** (en BMO-X, ESTRATOS: `proton-x/<juego>/bsf/`;
/// en el banco, la memoria). `nombre` es el de [`nombre`].
pub trait Recuerdo {
    fn leer(&mut self, nombre: &str) -> Option<Vec<u8>>;
    fn guardar(&mut self, nombre: &str, bsf: &[u8]) -> bool;
}

/// De donde salio el codigo de un PSO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    /// Traducido ahora (y comprobado).
    Traducido,
    /// Leido de un .bsf de antes (y vuelto a comprobar).
    Recordado,
}

/// **El MAPA de un PSO**: lo que la CPU entiende de sus dos sombreadores.
/// Texto canonico (el `Debug` de cada `Programa`, sin sus constantes, y
/// detras sus constantes en bits: un `Debug` de float no dice la carga de
/// un NaN), con la cabecera [`bmo_bsf::MAPA_MAGIC`] y la version del
/// emisor; rellenado a 4 bytes (lo pide el sobre).
pub fn mapa(en: &Enlace) -> Vec<u8> {
    let mut s = format!("MAPA 1\nemisor {VERSION_EMISOR}\n");
    for (nombre, p) in [("vs", &en.vs), ("ps", &en.ps)] {
        let sin = Programa { iniciales: Vec::new(), ..p.clone() };
        s.push_str(&format!("{nombre} {sin:?}\n{nombre}.iniciales"));
        for x in &p.iniciales {
            s.push_str(&format!(" {:08x}", x.to_bits()));
        }
        s.push('\n');
    }
    let mut b = s.into_bytes();
    while b.len() % 4 != 0 {
        b.push(b'\n');
    }
    b
}

/// El nombre del .bsf de un mapa: su hash (BLAKE3, el de la casa) en
/// hexadecimal.
pub fn nombre(mapa: &[u8]) -> String {
    let mut s = String::with_capacity(68);
    for b in bmo_hash::hash(mapa) {
        s.push_str(&format!("{b:02x}"));
    }
    s.push_str(".bsf");
    s
}

/// El codigo de un cuerpo en el objetivo `SM86_PUERTA_V1` (ver
/// `bmo_bsf::abi`).
fn cuerpo(e: &Emitido) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + 16 * e.codigo.len() + 4 * e.precargas.len());
    b.extend_from_slice(&(e.codigo.len() as u32).to_le_bytes());
    b.extend_from_slice(&(e.precargas.len() as u32).to_le_bytes());
    for &(lo, hi) in &e.codigo {
        b.extend_from_slice(&lo.to_le_bytes());
        b.extend_from_slice(&hi.to_le_bytes());
    }
    for p in &e.precargas {
        b.extend_from_slice(&match *p {
            Precarga::Entrada { elemento, componente, reg } => [0, elemento, componente, reg],
            Precarga::Fila { fila, reg } => [1, fila as u8, (fila >> 8) as u8, reg],
            Precarga::Asa { textura, muestreador, reg } => [2, textura, muestreador, reg],
        });
    }
    // 9d: el termometro de la libreta, como una entrada mas (3): un .bsf de
    // antes no la lleva, y uno con ella no lo lee un lector de antes (lo
    // vuelve a emitir).
    if let Some(t) = e.termometro {
        b[4..8].copy_from_slice(&(e.precargas.len() as u32 + 1).to_le_bytes());
        b.extend_from_slice(&[3, 0, 0, t]);
    }
    b
}

/// Lo contrario de [`cuerpo`], con los registros que dice el objetivo.
fn de_cuerpo(b: &[u8], registros: u32) -> Option<Emitido> {
    let u32_ = |i: usize| b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    let (n, m) = (u32_(0)? as usize, u32_(4)? as usize);
    if b.len() != 8 + 16 * n + 4 * m {
        return None;
    }
    let u64_ = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
    let codigo = (0..n).map(|k| (u64_(8 + 16 * k), u64_(16 + 16 * k))).collect();
    let mut precargas = Vec::with_capacity(m);
    let mut termometro = None;
    for k in 0..m {
        let x = &b[8 + 16 * n + 4 * k..][..4];
        precargas.push(match x[0] {
            0 => Precarga::Entrada { elemento: x[1], componente: x[2], reg: x[3] },
            1 => Precarga::Fila { fila: u16::from_le_bytes([x[1], x[2]]), reg: x[3] },
            2 => Precarga::Asa { textura: x[1], muestreador: x[2], reg: x[3] },
            3 if termometro.is_none() => {
                termometro = Some(x[3]);
                continue;
            }
            _ => return None,
        });
    }
    Some(Emitido { codigo, registros, mufus: 0, ciclos: 0, precargas, termometro })
}

/// El objetivo de la puerta con `code` (ver `bmo_bsf::abi::SM86_PUERTA_V1`).
fn objetivo(code: &[u8], registros: u32) -> bmo_bsf::TargetIn<'_> {
    bmo_bsf::TargetIn { kind: bmo_bsf::kind::SM86, abi: bmo_bsf::abi::SM86_PUERTA_V1, requires: 0, code, init: 0, main: 0, frame_words: registros, slots: &[], emitter: b"proton-x-sm86" }
}

/// **El .bsf de un PSO**: sus dos cuerpos, con el mapa de fuente.
pub fn a_bsf(mapa: &[u8], vs: &Emitido, ps: &Emitido) -> Vec<u8> {
    let (cv, cp) = (cuerpo(vs), cuerpo(ps));
    let (tv, tp) = ([objetivo(&cv, vs.registros)], [objetivo(&cp, ps.registros)]);
    // ExecutionModel de SPIR-V: 0 Vertex, 4 Fragment.
    let modulo = |model: u8, name: &'static [u8], targets| bmo_bsf::ModuleIn { model, name, local_size: [0; 3], capabilities: 0, caps_high: 0, spirv: mapa, bindings: &[], targets };
    let modulos = [modulo(0, b"vs", &tv[..]), modulo(4, b"ps", &tp[..])];
    let n = bmo_bsf::size(&modulos).expect("dos modulos chicos caben en un BSF");
    let mut out = vec![0u8; n];
    bmo_bsf::write(&modulos, &mut out).expect("el escritor de BSF relee lo que escribe");
    out
}

/// **Leer un .bsf de antes**: el sobre entero (sus hashes), que su fuente
/// sea EXACTAMENTE `mapa` (el de ahora), y sus dos cuerpos. Lo que no
/// cuadre, `Err` con el motivo (y se vuelve a traducir).
pub fn de_bsf(bytes: &[u8], mapa: &[u8]) -> Result<(Emitido, Emitido), &'static str> {
    let b = bmo_bsf::Bsf::parse(bytes).map_err(|_| "el sobre no se sostiene")?;
    b.verify_all().map_err(|_| "un hash del sobre no cuadra")?;
    let uno = |nombre: &[u8]| -> Result<Emitido, &'static str> {
        let m = b.find(nombre).ok_or("le falta un modulo")?;
        if !m.es_mapa() || m.spirv().map_err(|_| "su fuente no es la del hash")? != mapa {
            return Err("su mapa no es el de ahora");
        }
        let t = m.target(bmo_bsf::kind::SM86, bmo_bsf::abi::SM86_PUERTA_V1, 0).ok_or("no trae el cuerpo de la puerta")?;
        de_cuerpo(t.code().map_err(|_| "su codigo no es el del hash")?, t.frame_words() as u32).ok_or("su cuerpo no se lee")
    };
    Ok((uno(b"vs")?, uno(b"ps")?))
}

/// Los numeros de prueba de la tanda `t`: de -2 a 2, en cuartos (exactos),
/// distintos para cada elemento y componente.
fn prueba(t: usize, a: usize, b: usize) -> f32 {
    ((t * 7 + a * 3 + b * 5) % 17) as f32 * 0.25 - 2.0
}

/// **Comprobar un cuerpo contra el interprete**: cuatro tandas con las
/// mismas entradas y el mismo cbuffer en los dos; cada salida que el
/// programa escribe, los MISMOS bits (o NaN en los dos). Lo que muestrea,
/// con las texturas a cero en los dos. `Ok(n)`: cuantos valores se
/// compararon.
pub fn comprobar(p: &Programa, e: &Emitido, que: &str) -> Result<usize, String> {
    let escritas: Vec<(usize, usize)> = p.ops.iter().filter_map(|o| if let Op::Salida { elemento, componente, .. } = *o { Some((elemento as usize, componente as usize & 3)) } else { None }).collect();
    let cero = |_: u32, _: f32, _: f32| [0.0f32; 4];
    let (mut regs, mut n) = (Vec::new(), 0usize);
    for t in 0..4 {
        let ent: Vec<[f32; 4]> = (0..p.entradas.max(1)).map(|el| core::array::from_fn(|k| prueba(t, el, k))).collect();
        let cb: Vec<u8> = (0..16 * p.filas_cb as usize / 4).flat_map(|k| prueba(t + 1, k / 4, k % 4).to_bits().to_le_bytes()).collect();
        let mut m = Maquina::nueva([&[]; 8]);
        m.muestrear = Some(&cero);
        for (i, r) in m.r.iter_mut().enumerate() {
            *r = 0x7FC0_0000 | i as u32;
        }
        for &q in &e.precargas {
            match q {
                Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = ent.get(elemento as usize).map_or(0, |x| x[componente as usize & 3].to_bits()),
                Precarga::Fila { fila, reg } => {
                    for k in 0..4 {
                        let o = 16 * fila as usize + 4 * k;
                        m.r[reg as usize + k] = cb.get(o..o + 4).map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
                    }
                }
                // El asa: un numero cualquiera (`cero` no lo mira).
                Precarga::Asa { reg, .. } => m.r[reg as usize] = 0,
            }
        }
        correr(&e.codigo, &mut m).map_err(|x| format!("el {que}: el simulador de la 3060 no sabe correr lo emitido ({x:?})"))?;
        let mut casa = vec![[0f32; 4]; p.salidas.max(1)];
        p.correr(&ent, &cb, &mut casa, &mut regs);
        for &(el, k) in &escritas {
            let (g, c) = (m.r[4 * el + k], casa[el][k].to_bits());
            if g != c && !(f32::from_bits(g).is_nan() && f32::from_bits(c).is_nan()) {
                return Err(format!("el {que}: la salida {el}.{k} de la tanda {t} es {g:08x} en la 3060 y {c:08x} en la CPU"));
            }
            n += 1;
        }
        // 9d: con libreta, su termometro es el de la CPU sobre esas salidas
        // (cada registro escrito una vez, en su orden).
        if let Some(r) = e.termometro {
            let mut regs_salida: Vec<usize> = escritas.iter().map(|&(el, k)| 4 * el + k).collect();
            regs_salida.sort_unstable();
            regs_salida.dedup();
            let bits: Vec<u32> = regs_salida.iter().map(|&o| m.r[o]).collect();
            let (g, c) = (m.r[r as usize], crate::libreta::termometro(&bits));
            if g != c {
                return Err(format!("el {que}: el termometro de la libreta de la tanda {t} es {g:08x} en la 3060 y {c:08x} en la CPU"));
            }
            n += 1;
        }
    }
    Ok(n)
}

/// **Lo vivo de un PSO**: sus dos cuerpos, de donde salieron y el nombre
/// de su `.bsf` (con recuerdo).
#[derive(Debug, Clone)]
pub struct Vivos {
    pub vs: Emitido,
    pub ps: Emitido,
    pub origen: Origen,
    pub nombre: Option<String>,
}

/// El nombre de la MARCA de un `.bsf` malo (A9c): el mismo, `.malo`.
pub fn malo(nombre: &str) -> String {
    format!("{}.malo", nombre.trim_end_matches(".bsf"))
}

/// **Los dos cuerpos de un PSO, vivos**. Con recuerdo: si una revision con
/// datos reales lo marco MALO (A9c), no va a la 3060; si hay un `.bsf` de
/// este mapa con su sobre sano, se usa TAL CUAL (el MODO ESTATICO: lo que ya
/// se sabe, se dibuja; el vigia lo revisa con los datos del juego en su
/// primer dibujo); si no, se emite, se comprueba con los numeros de prueba
/// y se guarda. `emitir` es el emisor (lo pone la puerta).
pub fn cuerpos_vivos(en: &Enlace, recuerdo: Option<&mut dyn Recuerdo>, emitir: impl Fn(&Programa, &'static str) -> Result<Emitido, crate::pso::NoVa>) -> Result<Vivos, crate::pso::NoVa> {
    let comprobar_los_dos = |ev: &Emitido, ep: &Emitido| -> Result<(), crate::pso::NoVa> {
        comprobar(&en.vs, ev, "de vertice").and_then(|_| comprobar(&en.ps, ep, "de pixel")).map(|_| ()).map_err(|m| crate::pso::NoVa::Juez("la comprobacion contra la CPU", m))
    };
    let Some(r) = recuerdo else {
        let (vs, ps) = (emitir(&en.vs, "vertice")?, emitir(&en.ps, "pixel")?);
        comprobar_los_dos(&vs, &ps)?;
        return Ok(Vivos { vs, ps, origen: Origen::Traducido, nombre: None });
    };
    let m = mapa(en);
    let nombre = nombre(&m);
    if let Some(motivo) = r.leer(&malo(&nombre)) {
        return Err(crate::pso::NoVa::Juez("una revision con los datos del juego lo marco malo", String::from_utf8_lossy(&motivo).into_owned()));
    }
    if let Some(b) = r.leer(&nombre) {
        if let Ok((vs, ps)) = de_bsf(&b, &m) {
            return Ok(Vivos { vs, ps, origen: Origen::Recordado, nombre: Some(nombre) });
        }
    }
    let (vs, ps) = (emitir(&en.vs, "vertice")?, emitir(&en.ps, "pixel")?);
    comprobar_los_dos(&vs, &ps)?;
    r.guardar(&nombre, &a_bsf(&m, &vs, &ps));
    Ok(Vivos { vs, ps, origen: Origen::Traducido, nombre: Some(nombre) })
}

/// Las salidas que un programa escribe: `(elemento, componente)`.
fn escritas(p: &Programa) -> Vec<(usize, usize)> {
    p.ops.iter().filter_map(|o| if let Op::Salida { elemento, componente, .. } = *o { Some((elemento as usize, componente as usize & 3)) } else { None }).collect()
}

/// Cuantos vertices de un lote revisa el vigia.
pub const VERTICES_REVISADOS: usize = 8;

/// **EL VIGIA (A9c, 06-10, el MODO DINAMICO): un lote de VERDAD, revisado.**
/// Hasta [`VERTICES_REVISADOS`] vertices del lote: el cuerpo de vertice en
/// el simulador de la 3060 con lo que el pegamento cargaria de los DATOS
/// (el bufer de vertices y el cbuffer del juego TAL CUAL, `els` dice donde
/// va cada elemento) contra el interprete de la CPU con su lectura del
/// input layout; y el de pixel con las salidas de ese vertice (un pixel
/// justo en el), contra la CPU. Cada salida escrita, los mismos bits (o
/// NaN en los dos). Lo que muestrea, con las texturas a cero en los dos.
/// `Ok(n)`: cuantos valores; `Err`, el primero que no cuadra.
pub fn revisar(en: &Enlace, els: &[bmo_gpu_ga10x::pegamento::Elemento], vs: &Emitido, ps: &Emitido, l: &bmo_proton_x::lote::Lote) -> Result<usize, String> {
    let palabra = |b: &[u8], o: usize| b.get(o..o + 4).map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    let cero = |_: u32, _: f32, _: f32| [0.0f32; 4];
    let mut ids: Vec<u32> = l.ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(VERTICES_REVISADOS);
    let (ev, ep) = (escritas(&en.vs), escritas(&en.ps));
    let (mut regs, mut n) = (Vec::new(), 0usize);
    // Una maquina con lo que cargan sus precargas: `entrada(elemento,
    // componente)` y las filas del cbuffer del juego.
    let maquina = |e: &Emitido, entrada: &dyn Fn(usize, usize) -> u32| {
        let mut m = Maquina::nueva([&[]; 8]);
        for (i, r) in m.r.iter_mut().enumerate() {
            *r = 0x7FC0_0000 | i as u32;
        }
        for &q in &e.precargas {
            match q {
                Precarga::Entrada { elemento, componente, reg } => m.r[reg as usize] = entrada(elemento as usize, componente as usize & 3),
                Precarga::Fila { fila, reg } => {
                    for k in 0..4 {
                        m.r[reg as usize + k] = palabra(l.cb, 16 * fila as usize + 4 * k);
                    }
                }
                Precarga::Asa { reg, .. } => m.r[reg as usize] = 0,
            }
        }
        m
    };
    let comparar = |m: &Maquina, casa: &[[f32; 4]], escritas: &[(usize, usize)], que: &str, id: u32| -> Result<usize, String> {
        for &(el, k) in escritas {
            let (g, c) = (m.r[4 * el + k], casa[el][k].to_bits());
            if g != c && !(f32::from_bits(g).is_nan() && f32::from_bits(c).is_nan()) {
                return Err(format!("el de {que}, con el vertice {id} del juego, salida {el}.{k}: {g:08x} en la 3060 y {c:08x} en la CPU"));
            }
        }
        Ok(escritas.len())
    };
    for id in ids {
        let v = id as usize * l.paso;
        let leer_vertice = |el: usize, k: usize| match els.get(el) {
            Some(x) if k < x.componentes as usize => palabra(l.vertices, v + x.desde as usize + 4 * k),
            Some(_) if k == 3 => 1.0f32.to_bits(),
            _ => 0,
        };
        let mut m = maquina(vs, &leer_vertice);
        m.muestrear = Some(&cero);
        correr(&vs.codigo, &mut m).map_err(|x| format!("el de vertice: el simulador no sabe correrlo ({x:?})"))?;
        let ent: Vec<[f32; 4]> = en.desde_ia.iter().map(|&f| bmo_proton_x::lote::entrada(l, f, id, 0)).collect();
        let mut casa = vec![[0f32; 4]; en.vs.salidas.max(1)];
        en.vs.correr(&ent, l.cb, &mut casa, &mut regs);
        n += comparar(&m, &casa, &ev, "vertice", id)?;
        // El de pixel en ese vertice: sus entradas, las salidas de la CPU.
        let ent_ps: Vec<[f32; 4]> = en.desde_vs.iter().map(|o| o.and_then(|k| casa.get(k).copied()).unwrap_or([0.0; 4])).collect();
        let leer_pixel = |el: usize, k: usize| ent_ps.get(el).map_or(0, |x| x[k].to_bits());
        let mut m = maquina(ps, &leer_pixel);
        m.muestrear = Some(&cero);
        correr(&ps.codigo, &mut m).map_err(|x| format!("el de pixel: el simulador no sabe correrlo ({x:?})"))?;
        let mut casa_ps = vec![[0f32; 4]; en.ps.salidas.max(1)];
        en.ps.correr(&ent_ps, l.cb, &mut casa_ps, &mut regs);
        n += comparar(&m, &casa_ps, &ep, "pixel", id)?;
    }
    Ok(n)
}
