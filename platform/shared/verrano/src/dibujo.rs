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
//!                 Texture     una textura 2D (RGBA8, un nivel por ahora) y su
//!                 Sampler     muestreador: el material, el G-buffer de la
//!                             pasada anterior
//!    TUBERIA      Pipeline    las dos etapas, la profundidad, el descarte,
//!                             cuantos render targets y la MEZCLA de cada uno
//!    COMPUTO      Compute     un programa de computo y su `Dispatch`: grupos
//!                             de hilos con su memoria compartida y sus
//!                             barreras, que leen `ReadBuffer` y escriben
//!                             `RwBuffer` (los SRV y los UAV de D3D12). Ahi
//!                             calcula Cyberpunk su luz, sus sombras y su
//!                             posproceso
//!    DIBUJO       Draw        todo junto, con la limpieza (o sin ella: lo
//!                             que ya habia se queda, para mezclar encima)
//!                             y la vista
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
//!       la entrada k, INTERPOLADO CON PERSPECTIVA en el centro de cada
//!       pixel (la cuenta de la casa, `bmo_proton_x::trama`); lo que vale lo
//!       mismo en los tres vertices queda EXACTO (las caras del cubo: un
//!       color por cara, sin un bit de mas). Recortar en el plano de cerca
//!       recorta tambien lo que el vertice deja
//!    4  su salida 0 es el color; si tira el pixel, no se pinta ni toca la
//!       profundidad
//!    5  si lee su entrada 0, es SV_POSITION y corre en CADA pixel: el
//!       centro (x + 0.5, y + 0.5), la z lineal en la pantalla y la w de
//!       recorte con perspectiva -- lo de la casa (`bmo_proton_x::trama`,
//!       N5.9) y lo de D3D --. Cyberpunk la lee para su G-buffer y su ruido
//!    6  VARIOS RENDER TARGETS (el G-buffer, N5.8 de la casa): su salida k
//!       va al render target k, hasta 8
//!    7  LA MEZCLA de cada uno (N5.11 de la casa, `D3D12_BLEND_DESC`): en
//!       floats, sobre el que ya estaba (sus 8 bits / 255), y a 8 bits otra
//!       vez; la mascara deja los canales que no estan. La luz que se SUMA,
//!       el humo, el cristal, la interfaz
//! ```

use crate::{Cull, Error, Image, Stats, Viewport};

/// Lo mas que entra o sale de una etapa (D3D12: 32; aqui sobra para hoy).
pub const MAX_IO: usize = 16;

/// **Como se filtra** al muestrear (min, mag y mip, los tres).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Point,
    Linear,
}

/// **Que pasa fuera de 0..1** (`D3D12_TEXTURE_ADDRESS_MODE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Wrap,
    Mirror,
    Clamp,
    Border,
}

/// **Un muestreador** (`D3D12_SAMPLER_DESC`, lo de hoy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampler {
    pub filter: Filter,
    pub address: Address,
    /// El color del borde (R, G, B, A), para [`Address::Border`].
    pub border: [f32; 4],
}

/// **Una textura 2D** de 8 bits por canal y un nivel: `texels` fila a fila,
/// en el orden de bytes R, G, B, A (o B, G, R, A con `bgra`).
#[derive(Clone, Copy, Debug)]
pub struct Texture<'a> {
    pub texels: &'a [u32],
    pub width: u32,
    pub height: u32,
    pub bgra: bool,
}

/// **Lo que un programa puede leer**: sus texturas (`t0`, `t1`...) y sus
/// muestreadores (`s0`, `s1`...).
#[derive(Clone, Copy, Debug)]
pub struct Resources<'a> {
    pub textures: &'a [Texture<'a>],
    pub samplers: &'a [Sampler],
}

impl Resources<'_> {
    pub const NONE: Resources<'static> = Resources { textures: &[], samplers: &[] };
}

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
    /// Una vez, pudiendo leer `resources` (las texturas). El que no lee
    /// ninguna no tiene que escribirlo.
    fn run_with(&self, inputs: &[[f32; 4]], constants: &[u8], resources: &Resources, outputs: &mut [[f32; 4]]) -> bool {
        let _ = resources;
        self.run(inputs, constants, outputs)
    }
}

/// Lo mas de render targets a la vez (D3D12: 8).
pub const MAX_TARGETS: usize = 8;

/// **Un factor de mezcla** (`D3D12_BLEND`, los que usan los juegos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Factor {
    Zero,
    One,
    SrcColor,
    InvSrcColor,
    SrcAlpha,
    InvSrcAlpha,
    DestAlpha,
    InvDestAlpha,
    DestColor,
    InvDestColor,
    /// min(o.a, 1 - d.a) en el color; 1 en el alfa.
    SrcAlphaSat,
    /// El de `OMSetBlendFactor` ([`Pipeline::blend_factor`]).
    BlendFactor,
    InvBlendFactor,
}

/// **Una operacion de mezcla** (`D3D12_BLEND_OP`). Min y Max no miran los
/// factores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendOp {
    Add,
    Subtract,
    RevSubtract,
    Min,
    Max,
}

/// **La mezcla de un render target** (`D3D12_RENDER_TARGET_BLEND_DESC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blend {
    pub enable: bool,
    pub src: Factor,
    pub dst: Factor,
    pub op: BlendOp,
    pub src_alpha: Factor,
    pub dst_alpha: Factor,
    pub op_alpha: BlendOp,
    /// R 1, G 2, B 4, A 8.
    pub write_mask: u8,
}

impl Blend {
    /// Sin mezcla y los cuatro canales: el pixel nuevo, tal cual.
    pub const NONE: Blend = Blend { enable: false, src: Factor::One, dst: Factor::Zero, op: BlendOp::Add, src_alpha: Factor::One, dst_alpha: Factor::Zero, op_alpha: BlendOp::Add, write_mask: 0xF };
    /// La luz que se SUMA (One, One).
    pub const ADD: Blend = Blend { enable: true, src: Factor::One, dst: Factor::One, op: BlendOp::Add, src_alpha: Factor::One, dst_alpha: Factor::One, op_alpha: BlendOp::Add, write_mask: 0xF };
    /// La transparencia de siempre (SrcAlpha, InvSrcAlpha).
    pub const ALPHA: Blend = Blend { enable: true, src: Factor::SrcAlpha, dst: Factor::InvSrcAlpha, op: BlendOp::Add, src_alpha: Factor::One, dst_alpha: Factor::InvSrcAlpha, op_alpha: BlendOp::Add, write_mask: 0xF };

    /// Si no cambia nada: el pixel nuevo se escribe tal cual.
    pub fn trivial(&self) -> bool {
        !self.enable && self.write_mask == 0xF
    }

    /// **El color que queda**: el nuevo `o` sobre el que estaba `d`, con el
    /// factor `k` (la cuenta de la casa: `bmo_proton_x::mezcla`, N5.11).
    pub fn apply(&self, o: [f32; 4], d: [f32; 4], k: [f32; 4]) -> [f32; 4] {
        let mezclado = if self.enable {
            core::array::from_fn(|c| {
                let (fo, fd, op) = if c < 3 { (self.src, self.dst, self.op) } else { (self.src_alpha, self.dst_alpha, self.op_alpha) };
                let (a, b) = (o[c] * factor(fo, c, o, d, k), d[c] * factor(fd, c, o, d, k));
                match op {
                    BlendOp::Add => a + b,
                    BlendOp::Subtract => a - b,
                    BlendOp::RevSubtract => b - a,
                    BlendOp::Min => o[c].min(d[c]),
                    BlendOp::Max => o[c].max(d[c]),
                }
            })
        } else {
            o
        };
        core::array::from_fn(|c| if self.write_mask & (1 << c) != 0 { mezclado[c] } else { d[c] })
    }
}

/// El factor `f` para el canal `c` (3 es el alfa).
fn factor(f: Factor, c: usize, o: [f32; 4], d: [f32; 4], k: [f32; 4]) -> f32 {
    match f {
        Factor::Zero => 0.0,
        Factor::One => 1.0,
        Factor::SrcColor => o[c],
        Factor::InvSrcColor => 1.0 - o[c],
        Factor::SrcAlpha => o[3],
        Factor::InvSrcAlpha => 1.0 - o[3],
        Factor::DestAlpha => d[3],
        Factor::InvDestAlpha => 1.0 - d[3],
        Factor::DestColor => d[c],
        Factor::InvDestColor => 1.0 - d[c],
        Factor::SrcAlphaSat if c < 3 => o[3].min(1.0 - d[3]),
        Factor::SrcAlphaSat => 1.0,
        Factor::BlendFactor => k[c],
        Factor::InvBlendFactor => 1.0 - k[c],
    }
}

/// **La tuberia**: las dos etapas y como se dibuja.
#[derive(Clone, Copy)]
pub struct Pipeline<'a> {
    pub vertex: &'a dyn Shader,
    pub pixel: &'a dyn Shader,
    pub depth: bool,
    pub cull: Cull,
    /// Cuantos render targets escribe (1..=8): la salida k del de pixeles
    /// va al k.
    pub targets: usize,
    /// La mezcla de cada uno.
    pub blend: [Blend; MAX_TARGETS],
    /// `OMSetBlendFactor`.
    pub blend_factor: [f32; 4],
}

impl<'a> Pipeline<'a> {
    /// Un render target, sin mezcla.
    pub fn new(vertex: &'a dyn Shader, pixel: &'a dyn Shader, depth: bool, cull: Cull) -> Pipeline<'a> {
        Pipeline { vertex, pixel, depth, cull, targets: 1, blend: [Blend::NONE; MAX_TARGETS], blend_factor: [1.0; 4] }
    }
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
    /// Con que se limpian los render targets (y la profundidad, a 1.0) antes
    /// de dibujar; `None`: lo que habia se queda (una pasada que mezcla
    /// encima de otra).
    pub clear: Option<[f32; 4]>,
    pub viewport: Viewport,
    pub pipeline: Pipeline<'a>,
    pub vertices: Buffer<'a>,
    pub indices: Option<&'a [u32]>,
    pub constants: &'a [u8],
    /// Las texturas y los muestreadores que leen sus programas.
    pub resources: Resources<'a>,
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

/// **Un backend que dibuja con programas.** `out` es el render target 0 (y
/// la profundidad); `others`, los de detras (1, 2...), de su misma medida.
pub trait DrawBackend {
    fn draw_targets(&mut self, d: &Draw, out: &mut Image, others: &mut [&mut [u32]]) -> Result<Stats, Error>;

    /// Un render target.
    fn draw_with(&mut self, d: &Draw, out: &mut Image) -> Result<Stats, Error> {
        self.draw_targets(d, out, &mut [])
    }
}

/// Lo que se comprueba antes de nada, igual para todos los backends.
pub fn check_draw(d: &Draw, out: &Image, others: &[&mut [u32]], max_vertices: usize) -> Result<(), Error> {
    let p = &d.pipeline;
    // tantos render targets como dice, cada uno con su salida
    if p.targets == 0 || p.targets > MAX_TARGETS || p.targets > p.pixel.outputs() {
        return Err(Error::Shader);
    }
    let n = (out.width * out.height) as usize;
    if others.len() + 1 < p.targets || others[..p.targets - 1].iter().any(|o| o.len() < n) {
        return Err(Error::Image);
    }
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
    // la de pixeles lee lo que la de vertice deja (su entrada 0, la
    // posicion: la pone la trama, regla 5)
    if p.pixel.inputs() > p.vertex.outputs() {
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
    fn run_with(&self, inputs: &[[f32; 4]], constants: &[u8], resources: &Resources, outputs: &mut [[f32; 4]]) -> bool {
        use bmo_prometeo::textura::{Direccion, Filtro, Lod, Muestreador, Recursos, Textura};
        if resources.textures.is_empty() && resources.samplers.is_empty() {
            return self.run(inputs, constants, outputs);
        }
        let texturas: alloc::vec::Vec<Option<Textura>> = resources.textures.iter().map(|t| Some(Textura::rgba(t.texels, t.width, t.height, t.bgra))).collect();
        let muestreadores: alloc::vec::Vec<Option<Muestreador>> = resources
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
        let rec = Recursos { texturas: &texturas, muestreadores: &muestreadores, buferes: &[], dinamicas: None };
        let mut regs = alloc::vec::Vec::new();
        self.correr_con(inputs, constants, &rec, outputs, &mut regs)
    }
}

/// **Un bufer que el computo LEE** (un SRV estructurado: `stride` bytes por
/// elemento; con `stride` 0, crudo: palabras de 4 bytes).
#[derive(Clone, Copy, Debug)]
pub struct ReadBuffer<'a> {
    pub bytes: &'a [u8],
    pub stride: u32,
}

/// **Un bufer que el computo LEE Y ESCRIBE** (un UAV: `RWStructuredBuffer`
/// con `stride`, o `RWByteAddressBuffer` con `stride` 0).
#[derive(Debug)]
pub struct RwBuffer<'a> {
    pub bytes: &'a mut [u8],
    pub stride: u32,
}

/// Los elementos de un bufer: de `stride` bytes, o palabras si es crudo.
pub fn elements(bytes: usize, stride: u32) -> u32 {
    (bytes / if stride == 0 { 4 } else { stride as usize }) as u32
}

/// **Un programa de computo**, en la lengua que sea.
pub trait Compute {
    /// Los hilos de un grupo (`[numthreads(x, y, z)]`).
    fn threads(&self) -> [u32; 3];
    /// `Dispatch(groups)`: cada hilo de cada grupo, con su memoria compartida
    /// y sus barreras. Devuelve los hilos que corrieron.
    fn dispatch(&self, groups: [u32; 3], constants: &[u8], reads: &[ReadBuffer], writes: &mut [RwBuffer]) -> u64;
}

/// **Un despacho**: el programa, cuantos grupos y su cbuffer.
#[derive(Clone, Copy)]
pub struct Dispatch<'a> {
    pub program: &'a dyn Compute,
    pub groups: [u32; 3],
    pub constants: &'a [u8],
}

/// **Un backend que computa.**
pub trait ComputeBackend {
    fn dispatch(&mut self, d: &Dispatch, reads: &[ReadBuffer], writes: &mut [RwBuffer]) -> Result<u64, Error>;
}

/// Lo que se comprueba antes de despachar (los techos de D3D12): de 1 a
/// 65535 grupos por eje, y de 1 a 1024 hilos por grupo (z hasta 64).
pub fn check_dispatch(d: &Dispatch) -> Result<(), Error> {
    let [x, y, z] = d.program.threads();
    if d.groups.iter().any(|&g| g == 0 || g > 65535) || x == 0 || y == 0 || z == 0 || z > 64 || x as u64 * y as u64 * z as u64 > 1024 {
        return Err(Error::Groups);
    }
    Ok(())
}

/// **El Programa de la casa es un programa de computo** (la feature
/// `prometeo`): su `despachar` (`prometeo/src/despacho.rs`, N5.5), con los
/// buferes como SRV y UAV estructurados (o crudos).
#[cfg(feature = "prometeo")]
impl Compute for bmo_prometeo::programa::Programa {
    fn threads(&self) -> [u32; 3] {
        self.computo.hilos
    }
    fn dispatch(&self, groups: [u32; 3], constants: &[u8], reads: &[ReadBuffer], writes: &mut [RwBuffer]) -> u64 {
        use bmo_prometeo::bufer::{Bufer, Rebanadas, Uav};
        let srv: alloc::vec::Vec<Option<Bufer>> = reads.iter().map(|r| Some(Bufer { bytes: r.bytes, formato: 0, paso: r.stride, elementos: elements(r.bytes.len(), r.stride) })).collect();
        let rec = bmo_prometeo::textura::Recursos { texturas: &[], muestreadores: &[], buferes: &srv, dinamicas: None };
        let mut uavs: alloc::vec::Vec<Option<Uav>> = writes
            .iter_mut()
            .map(|w| {
                let elementos = elements(w.bytes.len(), w.stride);
                Some(Uav { bytes: &mut *w.bytes, formato: 0, paso: w.stride, elementos, contador: None, rebanadas: Rebanadas::PLANA })
            })
            .collect();
        self.despachar(groups, constants, &rec, &mut uavs)
    }
}
