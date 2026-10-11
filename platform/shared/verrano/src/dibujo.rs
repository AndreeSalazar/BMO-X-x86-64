//! **V3a (11-10): VERRANO POR CATEGORIAS -- el dibujo con PROGRAMAS.**
//!
//! El propietario, el 11-10: *"que VERRANO se aislen de elementos para poder
//! aplicar todo eso y asi crecer por categoria [...] TITAN++ llama a VERRANO
//! [...] tienen librerias propias que ya sabe hablar con la GPU"*; y sobre
//! quien hace que: *"PROTON-X [...] solo le emula a la ABI MUY PRECISAS [...]
//! pero VERRANO DEVORA DirectX para poder tener FPS NATIVOS"*.
//!
//! Un [`Frame`](crate::Frame) trae los vertices YA en recorte: la CPU los
//! conto. Un [`Draw`] trae lo que trae cualquier API de verdad -- D3D12,
//! Vulkan, OpenGL --, partido en sus CATEGORIAS, y cada una crece sola:
//!
//! ```text
//!    RECURSOS     Buffer      los vertices SIN transformar (float4 por
//!                             atributo), y los indices
//!                 constants   los bytes del cbuffer (la matriz, la luz)
//!    PROGRAMAS    Shader      una etapa: de vertice o de pixeles. VERRANO no
//!                             sabe que lengua es: el de la casa es el
//!                             Programa de PROMETEO (la feature `prometeo`),
//!                             que ya sale de DXIL, de SM5 y de TITAN++
//!    TUBERIA      Pipeline    las dos etapas, la profundidad y el descarte
//!    DIBUJO       Draw        todo junto, con la limpieza y la vista
//! ```
//!
//! Lo de FUERA se traduce a estas piezas y nada mas -- las llamadas de
//! D3D12 o de Vulkan a un `Draw`, sus sombreadores al Programa -- y lo de
//! DENTRO (TITAN++, MUNDO) las usa directo. Cada backend las ejecuta: el
//! juez de la CPU aqui (`cpu.rs`), la 3060 donde se toca la tarjeta (su
//! emisor, su juez y su `.bsf`, en `proton-x-sm86`).
//!
//! # Las reglas del juez con programas
//!
//! ```text
//!    1  cada vertice: su programa de vertice con sus atributos y el cbuffer;
//!       la salida 0 es la posicion en recorte (SV_Position)
//!    2  el triangulo, como un Frame: recorte de cerca, descarte, cobertura
//!       y profundidad (las reglas de cpu.rs)
//!    3  el programa de pixeles lee lo que el de vertice dejo, salida k en
//!       la entrada k, del VERTICE 0 del triangulo (plano, la regla 4 del
//!       juez: un color por cara). Interpolar llega con las texturas (M3)
//!    4  su salida 0 es el color; si tira el pixel, no se pinta ni toca la
//!       profundidad
//! ```
//!
//! [!] Un programa de pixeles que lee su POSICION (SV_Position) no se dibuja
//! todavia: [`Error::Shader`]. Se dice; no se inventa.

use crate::{Cull, Error, Image, Stats, Viewport};

/// Lo mas que entra o sale de una etapa (D3D12: 32; aqui sobra para hoy).
pub const MAX_IO: usize = 16;

/// **Una etapa de la tuberia**: un programa de vertice o de pixeles, en la
/// lengua que sea. Los valores van por su id en la firma, de cuatro en cuatro.
pub trait Shader {
    /// Cuantos elementos lee y cuantos escribe.
    fn inputs(&self) -> usize;
    fn outputs(&self) -> usize;
    /// Si de verdad LEE el elemento de entrada `k`.
    fn reads(&self, k: usize) -> bool;
    /// Una vez: `false` si tira (un pixel con `discard`).
    fn run(&self, inputs: &[[f32; 4]], constants: &[u8], outputs: &mut [[f32; 4]]) -> bool;
}

/// **La tuberia**: las dos etapas y como se dibuja.
#[derive(Clone, Copy)]
pub struct Pipeline<'a> {
    pub vertex: &'a dyn Shader,
    pub pixel: &'a dyn Shader,
    pub depth: bool,
    pub cull: Cull,
}

/// **Un bufer de vertices**: `stride` float4 por vertice, uno por atributo.
#[derive(Clone, Copy, Debug)]
pub struct Buffer<'a> {
    pub data: &'a [[f32; 4]],
    pub stride: usize,
}

/// **Un dibujo**: `count` vertices -- o indices, si los hay -- en triangulos.
#[derive(Clone, Copy)]
pub struct Draw<'a> {
    pub clear: [f32; 4],
    pub viewport: Viewport,
    pub pipeline: Pipeline<'a>,
    pub vertices: Buffer<'a>,
    pub indices: Option<&'a [u32]>,
    pub constants: &'a [u8],
    pub count: usize,
}

impl Draw<'_> {
    /// El vertice que va en el lugar `k` del dibujo.
    pub fn vertex_at(&self, k: usize) -> usize {
        match self.indices {
            Some(i) => i[k] as usize,
            None => k,
        }
    }
}

/// **Un backend que dibuja con programas.**
pub trait DrawBackend {
    fn draw_with(&mut self, d: &Draw, out: &mut Image) -> Result<Stats, Error>;
}

/// Lo que se comprueba antes de nada, igual para todos los backends.
pub fn check_draw(d: &Draw, out: &Image, max_vertices: usize) -> Result<(), Error> {
    let p = &d.pipeline;
    if d.count == 0 || d.count % 3 != 0 || d.count > max_vertices {
        return Err(Error::Vertices);
    }
    let vertices = d.vertices.data.len().checked_div(d.vertices.stride).ok_or(Error::Vertices)?;
    match d.indices {
        Some(i) if i.len() < d.count || i[..d.count].iter().any(|&k| k as usize >= vertices) => return Err(Error::Vertices),
        None if d.count > vertices => return Err(Error::Vertices),
        _ => {}
    }
    if d.vertices.stride < p.vertex.inputs() || p.vertex.outputs() == 0 || p.vertex.outputs() > MAX_IO || p.pixel.outputs() == 0 || p.pixel.outputs() > MAX_IO {
        return Err(Error::Shader);
    }
    // la de pixeles lee lo que la de vertice deja, y no su posicion (aun)
    if p.pixel.inputs() > p.vertex.outputs() || p.pixel.reads(0) {
        return Err(Error::Shader);
    }
    let (w, h) = (d.viewport.width, d.viewport.height);
    if w == 0 || h == 0 || w != out.width || h != out.height || out.pixels.len() < (w * h) as usize {
        return Err(Error::Image);
    }
    Ok(())
}

/// **El Programa de la casa es una etapa** (la feature `prometeo`): el que
/// sale de DXIL, de SM5 y de TITAN++, corrido por su interprete -- el juez
/// de lo que hagan las tarjetas --.
#[cfg(feature = "prometeo")]
impl Shader for bmo_prometeo::programa::Programa {
    fn inputs(&self) -> usize {
        self.entradas
    }
    fn outputs(&self) -> usize {
        self.salidas
    }
    fn reads(&self, k: usize) -> bool {
        k < 32 && self.lee & 1 << k != 0
    }
    fn run(&self, inputs: &[[f32; 4]], constants: &[u8], outputs: &mut [[f32; 4]]) -> bool {
        let mut regs = alloc::vec::Vec::new();
        self.correr(inputs, constants, outputs, &mut regs)
    }
}
