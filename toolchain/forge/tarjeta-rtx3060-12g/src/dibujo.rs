//! **LA RTX 3060 12G DIBUJA** (LB6 de `docs/plan/PLAN_LAS_LIBRERIAS.md`) --
//! una gpu fn de VERTICE o de PIXEL, ya hecha el Programa de la casa (por
//! PROMETEO, que no nombra a nadie), PEGADA a la tuberia de VERRANO y JUZGADA
//! como la juzga la puerta del kernel; y las de un paquete, en su SOBRE.
//!
//! ```text
//!    el Programa   --emitir_con(Abi::Registros)-->  el cuerpo y sus precargas
//!                  --vivo::comprobar----------->    LOS BITS: su simulador
//!                                                   contra el interprete de
//!                                                   la casa, o no se pega
//!                  --pegamento::vertice/pixel-->    SPH y codigo, como viaja
//!                  --juez::juzgar_programa----->    LA FORMA: PERFECTO, o no
//!                                                   hay sobre
//!    el SOBRE      un BSF (`kind` SM86, ABI `SM86_V1`): un modulo por gpu
//!                  fn, con su nombre; su fuente, el MAPA de la casa
//!                  (`vivo::mapa_de`, `MAPA_MAGIC`)
//! ```
//!
//! Lo mismo que `pegados()` del banco de BMOX-12
//! (`ga10x/tests/bmox12_sm86.rs`), lo que ya dibujo en el metal (E5, 28-09):
//! el emisor, el pegamento y el juez, SIN libreta -- la libreta la apunta el
//! pegamento del kernel en la puerta de PROTON-X (`pso::traducir`); un sobre
//! de `SM86_V1` sube tal cual.
//!
//! ** LOS DATOS de la de vertice, los de E5 (`pegamento::Datos::float4`): cada
//! elemento de entrada, un float4 seguido del anterior -- 16 bytes por campo
//! del registro que recibe --, desde el byte 0 del bufer de la ranura 0. El
//! vertice de VERRANO V0 (la posicion y el color, 32 bytes) es eso mismo: el
//! cubo de TITAN++ lee el bufer que hoy lee el SASS a mano de `cubo.bsf`.
//!
//! ** LA DE PIXEL recibe las salidas de la de vertice por sus genericos (la
//! `posicion`, no: es de la tarjeta), interpoladas EN PANTALLA (ScreenLinear,
//! el IPA de T2a): exacto cuando los tres vertices de un triangulo dan lo
//! mismo, como las caras del cubo; con perspectiva es otro paso, y se dice.

use bmo_bsf::{abi, kind, Binding, ModuleIn, TargetIn, READS};
use bmo_gpu_ga10x::pegamento::{self, Carga, Datos};
use bmo_gpu_ga10x::raster::SPH;
use bmo_gpu_ga10x::sass::juez::{self, Contexto};
use bmo_gpu_ga10x::tuberia;
use bmo_prometeo::Programa;

use crate::isa::{emitir_con, vivo, Abi, Precarga};
use crate::NOMBRE;

/// ** LA ETAPA de una gpu fn que dibuja, y donde va su `posicion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etapa {
    /// La de VERTICE: lee sus elementos de cada vertice y deja sus salidas;
    /// la `posicion`-esima es la posicion (SV_Position), las demas van a los
    /// genericos 0, 1... en orden.
    Vertice { posicion: usize },
    /// La de PIXEL: recibe las salidas de una de vertice -- la
    /// `posicion`-esima no, es de la tarjeta -- y deja su color en R0..R3.
    Pixel { posicion: usize },
}

/// ** Una gpu fn que dibuja, PEGADA y juzgada: lo que viaja.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pegada {
    pub nombre: String,
    pub etapa: Etapa,
    /// La cabecera SPH y el codigo, tal como viajan: lo que guarda el sobre
    /// y lo que sube el kernel.
    pub bytes: Vec<u8>,
    /// Las instrucciones, con las del pegamento.
    pub instrucciones: usize,
    /// Bytes de un vertice (la de vertice: 16 por elemento); 0 la de pixel.
    pub paso: u32,
    /// Lo que dijo su juez, en sus palabras.
    pub veredicto: String,
}

/// Lo que el pegamento carga antes del cuerpo: lo que el emisor dijo que
/// esperaba en registros. Una gpu fn no muestrea texturas (todavia).
fn cargas(nombre: &str, precargas: &[Precarga]) -> Result<Vec<Carga>, String> {
    precargas
        .iter()
        .map(|p| match *p {
            Precarga::Entrada { elemento, componente, reg } => Ok(Carga::Entrada { elemento, componente, reg }),
            Precarga::Fila { fila, reg } => Ok(Carga::Fila { fila, reg }),
            Precarga::Asa { .. } => Err(format!("`{}` muestrea una textura: {} no la pega todavia en una gpu fn", nombre, NOMBRE)),
        })
        .collect()
}

/// **Pega la gpu fn `nombre`** (su Programa `p`) a la tuberia de VERRANO en
/// su `etapa`: el cuerpo con las entradas en registros, sus bits comprobados
/// contra la casa, el pegamento de E5, y el juez -- con los registros que da
/// la tuberia (`tuberia::REGISTROS`) y la cabecera que lleva. Un NO dice
/// cual y por que, en las palabras de quien lo dijo.
pub fn pegar(nombre: &str, p: &Programa, etapa: Etapa) -> Result<Pegada, String> {
    let que = match etapa {
        Etapa::Vertice { .. } => "de vertice",
        Etapa::Pixel { .. } => "de pixel",
    };
    let no = |por: String| format!("`gpu fn {}` ({}): {}", nombre, que, por);
    let e = emitir_con(p, tuberia::REGISTROS, Abi::Registros).map_err(|x| no(format!("el emisor de {} dijo que no: {:?}", NOMBRE, x)))?;
    // LOS BITS: su simulador sobre el cuerpo, contra el interprete de la casa.
    vivo::comprobar(p, &e, &format!("{} `{}`", que, nombre)).map_err(&no)?;
    let cargas = cargas(nombre, &e.precargas).map_err(&no)?;
    if p.entradas > 8 || p.salidas > 8 {
        return Err(no(format!("{} elementos de entrada y {} de salida: la tuberia de VERRANO lleva 8 de cada uno", p.entradas, p.salidas)));
    }
    let datos = Datos::float4(p.filas_cb as u32, p.entradas as u32);
    let pegamento = |x: pegamento::NoPega| no(format!("el pegamento de VERRANO dijo que no: {:?}", x));
    let (g, paso) = match etapa {
        Etapa::Vertice { posicion } => (pegamento::vertice(&e.codigo, e.registros, &cargas, datos, p.salidas as u32, posicion as u32).map_err(pegamento)?, datos.paso),
        Etapa::Pixel { posicion } => {
            let genericos: Vec<Option<u8>> = (0..p.entradas as u32).map(|k| pegamento::generico(k, posicion as u32)).collect();
            (pegamento::pixel(&e.codigo, e.registros, &cargas, datos, &genericos).map_err(pegamento)?, 0)
        }
    };
    let mut bytes = vec![0u8; tuberia::HUECO];
    let n = g.bytes(&mut bytes);
    bytes.truncate(n);
    // LA FORMA: el juez con su cabecera, y como viaja (lo que hara la puerta).
    let bodrio = |b: juez::Bodrio| no(format!("{}\n{}", b, juez::REMATE));
    let veredicto = juez::juzgar(g.codigo(), &Contexto { registros: tuberia::REGISTROS, sph: Some(&g.sph) }).map_err(bodrio)?;
    juez::juzgar_programa(&bytes, tuberia::REGISTROS).map_err(bodrio)?;
    Ok(Pegada { nombre: nombre.to_string(), etapa, bytes, instrucciones: g.n, paso, veredicto: veredicto.to_string() })
}

/// Quien escribio el codigo de un sobre de TITAN++ (`emitter` del BSF: ASCII
/// sin espacios, 16 como mucho).
pub const EMISOR: &[u8] = b"titan-prometeo";

/// **EL SOBRE** de unas gpu fn que dibujan, ya pegadas (y el Programa de cada
/// una): un BSF con un modulo por cada una -- su nombre, su etapa (`model`:
/// 0 vertice, 4 pixel, los de SPIR-V), y su codigo en `kind` SM86, ABI
/// `SM86_V1` --; la de vertice dice el bufer que lee (set 0, binding 0, de
/// `paso` bytes por vertice). Su fuente es el MAPA de la casa de todas.
/// Determinista: las mismas pegadas, los mismos bytes.
pub fn sobre(pegadas: &[(&Pegada, &Programa)]) -> Result<Vec<u8>, String> {
    if pegadas.is_empty() {
        return Err("un sobre sin ninguna gpu fn que dibuje".into());
    }
    for (g, _) in pegadas {
        if g.nombre.is_empty() || g.nombre.len() > bmo_bsf::MAX_NAME || !g.nombre.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(format!("`{}`: el nombre de un modulo del sobre es ASCII sin espacios, de 1 a {} letras", g.nombre, bmo_bsf::MAX_NAME));
        }
    }
    let nombres: Vec<(&str, &Programa)> = pegadas.iter().map(|(g, p)| (g.nombre.as_str(), *p)).collect();
    let mapa = vivo::mapa_de(&nombres);
    let buferes: Vec<[Binding; 1]> = pegadas.iter().map(|(g, p)| [Binding { set: 0, binding: 0, storage: true, access: READS, base_bytes: 16 * p.filas_cb as u32, stride: g.paso }]).collect();
    let objetivos: Vec<[TargetIn; 1]> = pegadas
        .iter()
        .map(|(g, _)| {
            let slots: &[u8] = match g.etapa {
                Etapa::Vertice { .. } => &[0],
                Etapa::Pixel { .. } => &[],
            };
            [TargetIn { kind: kind::SM86, abi: abi::SM86_V1, requires: 0, code: &g.bytes, init: 0, main: 4 * SPH as u32, frame_words: 0, slots, emitter: EMISOR }]
        })
        .collect();
    let modulos: Vec<ModuleIn> = pegadas
        .iter()
        .enumerate()
        .map(|(i, (g, _))| {
            let (model, bindings): (u8, &[Binding]) = match g.etapa {
                Etapa::Vertice { .. } => (0, &buferes[i]),
                Etapa::Pixel { .. } => (4, &[]),
            };
            ModuleIn { model, name: g.nombre.as_bytes(), local_size: [0; 3], capabilities: 0, caps_high: 0, spirv: &mapa, bindings, targets: &objetivos[i] }
        })
        .collect();
    let n = bmo_bsf::size(&modulos).map_err(|f| format!("el sobre no cabe en un BSF: {:?}", f))?;
    let mut out = vec![0u8; n];
    let escrito = bmo_bsf::write(&modulos, &mut out).map_err(|f| format!("el escritor de BSF no relee lo que escribe: {:?}", f))?;
    out.truncate(escrito);
    Ok(out)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_prometeo::programa::Op;

    /// Un Programa a mano: lee `lee` (elemento, componente) en registros
    /// seguidos y deja cada (elemento, componente) de `deja` del registro dicho.
    fn programa(lee: &[(u8, u8)], deja: &[(u8, u8, u16)], entradas: usize, salidas: usize) -> Programa {
        let mut ops: Vec<Op> = lee.iter().enumerate().map(|(d, &(elemento, componente))| Op::Entrada { d: d as u16, elemento, componente }).collect();
        ops.extend(deja.iter().map(|&(elemento, componente, s)| Op::Salida { s, elemento, componente }));
        let lee_mascara = lee.iter().fold(0u32, |m, (e, _)| m | 1 << e);
        Programa { ops, iniciales: vec![0.0; lee.len().max(1)], entradas, salidas, lee: lee_mascara, filas_cb: 0, ranuras: Default::default(), computo: Default::default() }
    }

    /// El par del cubo de V0, a mano: la de vertice deja sus dos elementos;
    /// la de pixel, el elemento 1 (el color) en su salida 0.
    fn par() -> (Programa, Programa) {
        let todo: Vec<(u8, u8)> = (0..2).flat_map(|e| (0..4).map(move |c| (e, c))).collect();
        let vs = programa(&todo, &todo.iter().enumerate().map(|(r, &(e, c))| (e, c, r as u16)).collect::<Vec<_>>(), 2, 2);
        let color: Vec<(u8, u8)> = (0..4).map(|c| (1, c)).collect();
        let ps = programa(&color, &(0..4).map(|c| (0, c, c as u16)).collect::<Vec<_>>(), 2, 1);
        (vs, ps)
    }

    /// ** EL PAR SE PEGA, SU JUEZ DICE PERFECTO, Y EL SOBRE SE LEE: un modulo
    /// por cada una, con su nombre, su etapa, su bufer (la de vertice: 32
    /// bytes por vertice, el `Vertex` de VERRANO) y su codigo tal cual se pego.
    #[test]
    fn a_pair_is_glued_judged_and_put_in_its_sobre() {
        let (vs, ps) = par();
        let v = pegar("cubo_vertice", &vs, Etapa::Vertice { posicion: 0 }).unwrap_or_else(|e| panic!("{}", e));
        let p = pegar("cubo_pixel", &ps, Etapa::Pixel { posicion: 0 }).unwrap_or_else(|e| panic!("{}", e));
        assert!(v.veredicto.contains("PERFECTO Y PRECISO") && p.veredicto.contains("PERFECTO Y PRECISO"));
        assert_eq!((v.paso, p.paso), (32, 0));
        assert_eq!(v.bytes.len(), 4 * SPH + 16 * v.instrucciones);
        let s = sobre(&[(&v, &vs), (&p, &ps)]).unwrap();
        assert_eq!(s, sobre(&[(&v, &vs), (&p, &ps)]).unwrap(), "determinista");
        let b = bmo_bsf::Bsf::parse(&s).unwrap();
        b.verify_all().unwrap();
        let m = b.find(b"cubo_vertice").unwrap();
        assert_eq!((m.model(), m.binding_count(), m.binding(0).stride), (0, 1, 32));
        assert!(m.es_mapa() && m.spirv().unwrap().starts_with(b"MAPA 1\nemisor"));
        assert_eq!(m.target(kind::SM86, abi::SM86_V1, 0).unwrap().code().unwrap(), &v.bytes[..]);
        let m = b.find(b"cubo_pixel").unwrap();
        assert_eq!((m.model(), m.binding_count()), (4, 0));
        assert_eq!(m.target(kind::SM86, abi::SM86_V1, 0).unwrap().code().unwrap(), &p.bytes[..]);
    }

    /// ** LO QUE NO SE PEGA SE DICE, con quien lo dijo: una posicion que no
    /// es una de sus salidas (el pegamento), y un nombre que no cabe en el
    /// sobre.
    #[test]
    fn what_does_not_glue_is_said_with_who_said_it() {
        let (vs, _) = par();
        let no = pegar("v", &vs, Etapa::Vertice { posicion: 5 }).unwrap_err();
        assert!(no.contains("`gpu fn v` (de vertice)") && no.contains("pegamento") && no.contains("Salidas"), "{}", no);
        let v = pegar("con espacio", &vs, Etapa::Vertice { posicion: 0 }).unwrap();
        let no = sobre(&[(&v, &vs)]).unwrap_err();
        assert!(no.contains("ASCII sin espacios"), "{}", no);
        assert!(sobre(&[]).is_err());
    }
}
