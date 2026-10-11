//! **EL JUEZ DE LOS DIBUJOS CON PROGRAMAS** (VC2 paso 2, 11-10): un `Draw` y
//! un `Dispatch` de VERRANO, dibujados por la TRAMA de la casa -- una sola,
//! la que ya dibuja D3D12 en la CPU --. Antes VERRANO tenia la suya en
//! `verrano/src/cpu.rs`: la misma cuenta escrita dos veces.
//!
//! [carril]  VERDE     cuentas; no toca la maquina
//! [riesgo]  ESPEJO    las reglas son las de la trama (D3D12), no unas
//!                     parecidas: un `Draw` de VERRANO y un `Lote` de
//!                     PROTON-X con los mismos datos dan los mismos bits
//!
//! ```text
//!    VERRANO              la trama
//!    Pipeline.cull        Reglas.descarte (None 1, Back 3); delante es
//!                         horario en la pantalla, lo de D3D
//!    Pipeline.depth       Profundidad MENOR que, y se escribe
//!    Pipeline.blend       Mezclas (la de cada render target y el factor)
//!    salida k del de      entrada k del de pixeles (la 0, la posicion: si
//!    vertice              la lee, es SV_Position, que pone la trama)
//!    Image (0xAARRGGBB)   Destino con `bgra`; la profundidad, sus bits
//! ```
//!
//! El Programa de la casa entra por [`Casa`] (un `Shader` y un `Compute`
//! de VERRANO): las dos son de fuera de este crate, y Rust pide el envoltorio.

use alloc::vec;
use alloc::vec::Vec;

use bmo_prometeo::programa::Programa;
use bmo_prometeo::profundidad::Profundidad;
use bmo_prometeo::textura::{Direccion, Filtro, Lod, Muestreador, Recursos, Textura};
use bmo_verrano::dibujo::{check_dispatch, check_draw, Address, BlendOp, Compute, ComputeBackend, Dispatch, DrawBackend, Factor, Filter, ReadBuffer, Resources, RwBuffer, Shader, MAX_IO};
use bmo_verrano::{Cull, Draw, Error, Image, Stats};

use crate::mezcla::{self, Mezcla, Mezclas};
use crate::trama::{self, Destino, Otro, Reglas, Sombreado, SALIDAS};

/// **El Programa de la casa como etapa de VERRANO**: el que sale de DXIL, de
/// SM5 y de TITAN++, corrido por su interprete (y, si es de computo, por su
/// despacho: grupos, memoria compartida, barreras, olas).
#[derive(Clone, Copy, Debug)]
pub struct Casa<'a>(pub &'a Programa);

impl Shader for Casa<'_> {
    fn inputs(&self) -> usize {
        self.0.entradas
    }
    fn outputs(&self) -> usize {
        self.0.salidas
    }
    fn reads(&self, k: usize) -> bool {
        k < 32 && self.0.lee & 1 << k != 0
    }
    fn run(&self, inputs: &[[f32; 4]], constants: &[u8], outputs: &mut [[f32; 4]]) -> bool {
        let mut regs = Vec::new();
        self.0.correr(inputs, constants, outputs, &mut regs)
    }
    fn run_with(&self, inputs: &[[f32; 4]], constants: &[u8], resources: &Resources, outputs: &mut [[f32; 4]]) -> bool {
        if resources.textures.is_empty() && resources.samplers.is_empty() {
            return self.run(inputs, constants, outputs);
        }
        let (texturas, muestreadores) = de_verrano(resources);
        let rec = Recursos { texturas: &texturas, muestreadores: &muestreadores, buferes: &[], dinamicas: None };
        let mut regs = Vec::new();
        self.0.correr_con(inputs, constants, &rec, outputs, &mut regs)
    }
}

impl Compute for Casa<'_> {
    fn threads(&self) -> [u32; 3] {
        self.0.computo.hilos
    }
    fn dispatch(&self, groups: [u32; 3], constants: &[u8], reads: &[ReadBuffer], writes: &mut [RwBuffer]) -> u64 {
        use bmo_prometeo::bufer::{Bufer, Rebanadas, Uav};
        use bmo_verrano::dibujo::elements;
        let srv: Vec<Option<Bufer>> = reads.iter().map(|r| Some(Bufer { bytes: r.bytes, formato: 0, paso: r.stride, elementos: elements(r.bytes.len(), r.stride) })).collect();
        let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &srv, dinamicas: None };
        let mut uavs: Vec<Option<Uav>> = writes
            .iter_mut()
            .map(|w| {
                let elementos = elements(w.bytes.len(), w.stride);
                Some(Uav { bytes: &mut *w.bytes, formato: 0, paso: w.stride, elementos, contador: None, rebanadas: Rebanadas::PLANA })
            })
            .collect();
        self.0.despachar(groups, constants, &rec, &mut uavs)
    }
}

/// Las texturas y los muestreadores de VERRANO, como los lee PROMETEO.
fn de_verrano<'a>(r: &Resources<'a>) -> (Vec<Option<Textura<'a>>>, Vec<Option<Muestreador>>) {
    let texturas = r.textures.iter().map(|t| Some(Textura::rgba(t.texels, t.width, t.height, t.bgra))).collect();
    let muestreadores = r
        .samplers
        .iter()
        .map(|m| {
            let filtro = match m.filter {
                Filter::Point => Filtro::Punto,
                Filter::Linear => Filtro::Lineal,
            };
            let d = match m.address {
                Address::Wrap => Direccion::Repetir,
                Address::Mirror => Direccion::Espejo,
                Address::Clamp => Direccion::Sujetar,
                Address::Border => Direccion::Borde,
            };
            Some(Muestreador { filtro, u: d, v: d, borde: m.border, comparacion: 0, lod: Lod { mip: filtro, ..Lod::DE_SIEMPRE } })
        })
        .collect();
    (texturas, muestreadores)
}

/// El factor de VERRANO, con el numero de D3D12 que lleva la mezcla.
pub(crate) fn factor(f: Factor) -> u8 {
    match f {
        Factor::Zero => mezcla::CERO,
        Factor::One => mezcla::UNO,
        Factor::SrcColor => mezcla::ORIGEN_COLOR,
        Factor::InvSrcColor => mezcla::INV_ORIGEN_COLOR,
        Factor::SrcAlpha => mezcla::ORIGEN_ALFA,
        Factor::InvSrcAlpha => mezcla::INV_ORIGEN_ALFA,
        Factor::DestAlpha => mezcla::DESTINO_ALFA,
        Factor::InvDestAlpha => mezcla::INV_DESTINO_ALFA,
        Factor::DestColor => mezcla::DESTINO_COLOR,
        Factor::InvDestColor => mezcla::INV_DESTINO_COLOR,
        Factor::SrcAlphaSat => mezcla::ORIGEN_ALFA_SAT,
        Factor::BlendFactor => mezcla::FACTOR,
        Factor::InvBlendFactor => mezcla::INV_FACTOR,
    }
}

pub(crate) fn operacion(o: BlendOp) -> u8 {
    match o {
        BlendOp::Add => mezcla::SUMAR,
        BlendOp::Subtract => mezcla::RESTAR,
        BlendOp::RevSubtract => mezcla::RESTAR_AL_REVES,
        BlendOp::Min => mezcla::MIN,
        BlendOp::Max => mezcla::MAX,
    }
}

/// **El juez de los dibujos con programas**: la trama de la casa, con el
/// techo de vertices que se le diga.
#[derive(Clone, Copy, Debug)]
pub struct Juez {
    pub max_vertices: usize,
}

impl Juez {
    pub const NUEVO: Juez = Juez { max_vertices: 1 << 20 };
}

impl DrawBackend for Juez {
    fn draw_targets(&mut self, d: &Draw, out: &mut Image, others: &mut [&mut [u32]]) -> Result<Stats, Error> {
        check_draw(d, out, others, self.max_vertices)?;
        let p = d.pipeline;
        let (w, h) = (out.width, out.height);
        let n = (w * h) as usize;
        // la limpieza: los render targets con su color, la profundidad a 1.0
        if let Some(c) = d.clear {
            let fondo = trama::empaquetar(c, true);
            out.pixels[..n].fill(fondo);
            for o in others[..p.targets - 1].iter_mut() {
                o[..n].fill(fondo);
            }
        }
        let mut z: Option<Vec<u32>> = None;
        if p.depth {
            let Some(zb) = out.depth.as_deref_mut().filter(|z| z.len() >= n) else { return Err(Error::Image) };
            if d.clear.is_some() {
                zb[..n].fill(1.0);
            }
            z = Some(zb[..n].iter().map(|f| f.to_bits()).collect());
        }
        // cada vertice, por su programa (una vez cada uno)
        let stride = d.vertices.stride;
        let cuantos = d.vertices.data.len() / stride;
        let entradas = p.pixel.inputs();
        let mut sombreados: Vec<Option<Sombreado>> = vec![None; cuantos];
        let mut tris = Vec::with_capacity(d.count / 3);
        for t in 0..d.count / 3 {
            let mut tri = [0usize; 3];
            for (j, slot) in tri.iter_mut().enumerate() {
                let v = d.vertex_at(3 * t + j);
                if sombreados[v].is_none() {
                    let mut s = [[0f32; 4]; MAX_IO];
                    p.vertex.run_with(&d.vertices.data[v * stride..(v + 1) * stride], d.constants, &d.resources, &mut s[..p.vertex.outputs()]);
                    sombreados[v] = Some(Sombreado { pos: s[0], atributos: s[..entradas].to_vec() });
                }
                *slot = v;
            }
            tris.push(tri);
        }
        // la trama pide los vertices seguidos: los que no se usan van vacios
        let vertices: Vec<Sombreado> = sombreados.into_iter().map(|s| s.unwrap_or(Sombreado { pos: [0.0, 0.0, 0.0, 1.0], atributos: vec![[0.0; 4]; entradas] })).collect();
        let mut rt = [Mezcla::NINGUNA; 8];
        for (k, b) in p.blend.iter().enumerate() {
            rt[k] = Mezcla { encendida: b.enable, origen: factor(b.src), destino: factor(b.dst), op: operacion(b.op), origen_a: factor(b.src_alpha), destino_a: factor(b.dst_alpha), op_a: operacion(b.op_alpha), mascara: b.write_mask };
        }
        let reglas = Reglas {
            viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0],
            tijera: [0, 0, w as i32, h as i32],
            descarte: if p.cull == Cull::Back { 3 } else { 1 },
            antihorario: false,
            profundidad: p.depth.then_some(Profundidad { funcion: 2, escribir: true }),
            mezcla: Mezclas { rt, factor: p.blend_factor },
            z_del_sombreador: false,
            stencil: None,
        };
        let mut otros: Vec<Otro> = others[..p.targets - 1].iter_mut().map(|o| Otro { pixeles: Some(&mut o[..n]), bgra: true, flotante: None }).collect();
        let posicion = p.pixel.reads(0).then_some(0);
        let cuenta = {
            let mut destino = Destino { pixeles: &mut out.pixels[..n], ancho: w, alto: h, bgra: true, z: z.as_deref_mut(), cadena: false, otros: &mut otros, flotante: None, stencil: None };
            let (ps, cb, res) = (p.pixel, d.constants, &d.resources);
            let salidas = p.pixel.outputs().min(SALIDAS);
            trama::dibujar(&reglas, &vertices, &tris, &mut destino, posicion, |ent, colores| ps.run_with(ent, cb, res, &mut colores[..salidas]))
        };
        if let (Some(z), Some(zb)) = (z, out.depth.as_deref_mut()) {
            for (f, b) in zb.iter_mut().zip(z) {
                *f = f32::from_bits(b);
            }
        }
        Ok(Stats { triangles: cuenta.dibujados, device_us: 0, prepare_us: 0, warm: false, in_flight: false, wait_us: 0 })
    }
}

impl ComputeBackend for Juez {
    fn dispatch(&mut self, d: &Dispatch, reads: &[ReadBuffer], writes: &mut [RwBuffer]) -> Result<u64, Error> {
        check_dispatch(d)?;
        Ok(d.program.dispatch(d.groups, d.constants, reads, writes))
    }
}
