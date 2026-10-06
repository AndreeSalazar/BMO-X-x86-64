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
//! Lo que NO es todavia, dicho: la parte de la CPU del paquete (el `.bex`:
//! el `Programa` y su x86-64 de `nativo`) se sigue haciendo en cada arranque;
//! y el de pixeles que MUESTREA se comprueba con las texturas a cero (lo que
//! lee una textura de verdad lo juzgan los jueces de `prueba/`).
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
    for k in 0..m {
        let x = &b[8 + 16 * n + 4 * k..][..4];
        precargas.push(match x[0] {
            0 => Precarga::Entrada { elemento: x[1], componente: x[2], reg: x[3] },
            1 => Precarga::Fila { fila: u16::from_le_bytes([x[1], x[2]]), reg: x[3] },
            2 => Precarga::Asa { textura: x[1], muestreador: x[2], reg: x[3] },
            _ => return None,
        });
    }
    Some(Emitido { codigo, registros, mufus: 0, ciclos: 0, precargas })
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
    }
    Ok(n)
}

/// **Los dos cuerpos de un PSO, vivos**: del recuerdo si hay un .bsf con
/// este mapa (y vuelve a pasar la comprobacion), o emitidos ahora,
/// comprobados y guardados. `emitir` es el emisor (lo pone la puerta).
pub fn cuerpos_vivos(
    en: &Enlace,
    recuerdo: Option<&mut dyn Recuerdo>,
    emitir: impl Fn(&Programa, &'static str) -> Result<Emitido, crate::pso::NoVa>,
) -> Result<(Emitido, Emitido, Origen), crate::pso::NoVa> {
    let comprobar_los_dos = |ev: &Emitido, ep: &Emitido| -> Result<(), crate::pso::NoVa> {
        comprobar(&en.vs, ev, "de vertice").and_then(|_| comprobar(&en.ps, ep, "de pixel")).map(|_| ()).map_err(|m| crate::pso::NoVa::Juez("la comprobacion contra la CPU", m))
    };
    let Some(r) = recuerdo else {
        let (ev, ep) = (emitir(&en.vs, "vertice")?, emitir(&en.ps, "pixel")?);
        comprobar_los_dos(&ev, &ep)?;
        return Ok((ev, ep, Origen::Traducido));
    };
    let m = mapa(en);
    let nombre = nombre(&m);
    if let Some(b) = r.leer(&nombre) {
        if let Ok((ev, ep)) = de_bsf(&b, &m) {
            if comprobar_los_dos(&ev, &ep).is_ok() {
                return Ok((ev, ep, Origen::Recordado));
            }
        }
    }
    let (ev, ep) = (emitir(&en.vs, "vertice")?, emitir(&en.ps, "pixel")?);
    comprobar_los_dos(&ev, &ep)?;
    r.guardar(&nombre, &a_bsf(&m, &ev, &ep));
    Ok((ev, ep, Origen::Traducido))
}
