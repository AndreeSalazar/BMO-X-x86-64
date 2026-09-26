//! # VERRANO -- la API de dibujo de BMO-X (V0)
//!
//! generacion: hija -- el fotograma, la imagen y los backends; no sabe que
//! GPU hay debajo
//! capa: puro -- ni un `unsafe`, ni un aparato: aritmetica sobre buferes
//!
//! [carril]  VERDE     el backend CPU es cuenta pura; el de la 3060 vive
//!                     donde se toca la tarjeta
//! [cuesta]  NADA      corre cuando alguien dibuja
//!
//! El nombre lo puso el propietario el 23-09 (`PLAN_EL_SOMBREADOR.md`). El
//! plan: `docs/plan/PLAN_VERRANO.md`. Lo que un juego o un motor llama va en
//! INGLES (la regla de la casa para las APIs: `Frame`, `Vertex`, `Backend`);
//! los comentarios, en castellano.
//!
//! # Lo que es V0
//!
//! ```text
//!    la app        un Frame: el color de limpieza y una lista de vertices
//!                  (posicion en coordenadas de recorte + color)
//!    VERRANO       Backend::draw(frame, imagen)
//!      +-- Cpu     el juez: las reglas de D3D10 y el modelo de la 3060
//!      +-- 3060    los DOS programas fijos del BSF (`kind` SM86) y los datos
//!                  en un buffer (`bmo_gpu_ga10x::tuberia`, en el escritorio)
//! ```
//!
//! **La GPU no compila nada.** Los programas son SIEMPRE los mismos y viajan
//! ya traducidos en el BSF (`platform/drivers/gpu/ga10x/sombreadores/cubo.bsf`,
//! con su SPIR-V de origen); de un fotograma a otro solo cambian los datos.
//! Es lo que el propietario pidio del BSF: "la GPU no pierde tiempo".
//!
//! # Lo que V0 NO tiene todavia (dicho antes)
//!
//! - Ni profundidad ni culling: dibuja lo que le dan, en orden (el cubo es
//!   convexo y la app le da solo las caras de delante).
//! - Un solo tipo de vertice y un solo par de programas: los del cubo.
//! - Sin buferes propios. Vallas, desde V1b solo por dentro: un backend
//!   puede dejar el fotograma EN VUELO (`Stats::in_flight`) y
//!   `Backend::finish` espera lo que quede.

#![no_std]
#![forbid(unsafe_code)]

pub mod cpu;

/// **Un vertice**, como lo lee `cubo.vert`: la posicion ya en coordenadas
/// de RECORTE (x, y, z, w) y el color (r, g, b, a). 32 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
}

/// El viewport de D3D: la imagen entera, profundidad 0..1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// **Como pasa el color a 8 bits** (la regla 4 del estudio D3D).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unorm8 {
    /// Saturar, por 255 y redondear: la cuenta ideal de D3D.
    Exact,
    /// Truncar a 12 bits y redondear a 8: lo que hace el ROP de la RTX 3060
    /// (VISTO el 25-09 17:18 con y sin Windows: es del silicio).
    Truncate12,
}

impl Unorm8 {
    pub fn convert(self, x: f32) -> u32 {
        let s = x.clamp(0.0, 1.0);
        match self {
            Unorm8::Exact => (s * 255.0 + 0.5) as u32,
            Unorm8::Truncate12 => {
                let t = (s * 4096.0) as u32;
                (t * 255 + 2048) / 4096
            }
        }
    }

    /// `[r, g, b, a]` -> `0xAARRGGBB`.
    pub fn pack(self, c: [f32; 4]) -> u32 {
        self.convert(c[3]) << 24 | self.convert(c[0]) << 16 | self.convert(c[1]) << 8 | self.convert(c[2])
    }
}

/// **Un fotograma**: con que se limpia y que triangulos se dibujan (cada
/// tres vertices, uno, en el orden en que llegan).
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub clear: [f32; 4],
    pub vertices: &'a [Vertex],
    pub viewport: Viewport,
}

/// **Un rectangulo de pixeles**: `x0..x1` por `y0..y1` (el maximo fuera).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: u32,
    pub x1: u32,
    pub y0: u32,
    pub y1: u32,
}

impl Rect {
    /// La imagen entera del viewport.
    pub fn full(v: Viewport) -> Rect {
        Rect { x0: 0, x1: v.width, y0: 0, y1: v.height }
    }

    /// El menor rectangulo que tiene a los dos.
    pub fn union(self, o: Rect) -> Rect {
        Rect { x0: self.x0.min(o.x0), x1: self.x1.max(o.x1), y0: self.y0.min(o.y0), y1: self.y1.max(o.y1) }
    }

    pub fn area(self) -> u64 {
        (self.x1.saturating_sub(self.x0)) as u64 * (self.y1.saturating_sub(self.y0)) as u64
    }
}

impl Frame<'_> {
    /// **Donde PUEDEN pintar sus triangulos** (V1c): cada vertice por el
    /// viewport de D3D (`x * w/2 + w/2`, `y * -h/2 + h/2`), y 2 pixeles de
    /// margen a cada lado -- un triangulo no sale de la caja de sus
    /// vertices. Fuera de ella el fotograma es su color de limpieza. `None`
    /// si algun vertice no se deja proyectar (w <= 0, o no finito) o no hay
    /// vertices: entonces, la imagen entera.
    ///
    /// Es lo que la CPU sabe y la tarjeta no: con la caja de este fotograma
    /// y la del anterior, un backend limpia SOLO eso (`coopera`).
    pub fn cover(&self) -> Option<Rect> {
        let (w, h) = (self.viewport.width, self.viewport.height);
        let (mw, mh) = ((w / 2) as f32, (h / 2) as f32);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for v in self.vertices {
            let [x, y, _, q] = v.position;
            if !(q > 0.0 && q.is_finite() && x.is_finite() && y.is_finite()) {
                return None;
            }
            let (sx, sy) = (x / q * mw + mw, -y / q * mh + mh);
            x0 = x0.min(sx);
            x1 = x1.max(sx);
            y0 = y0.min(sy);
            y1 = y1.max(sy);
        }
        let a = |f: f32, tope: u32| f.clamp(0.0, tope as f32) as u32;
        let r = Rect { x0: a(x0 - 2.0, w), x1: a(x1 + 3.0, w), y0: a(y0 - 2.0, h), y1: a(y1 + 3.0, h) };
        (!self.vertices.is_empty() && r.x0 < r.x1 && r.y0 < r.y1).then_some(r)
    }
}

/// **Donde se dibuja**: `width * height` pixeles `0xAARRGGBB`, fila 0 arriba.
pub struct Image<'a> {
    pub pixels: &'a mut [u32],
    pub width: u32,
    pub height: u32,
}

/// Lo que cuenta un backend de su fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub triangles: u32,
    /// Lo que tardo el aparato (0 si no lo sabe medir).
    pub device_us: u32,
    /// Lo que costo dejarle el fotograma listo (subir los datos, las
    /// ordenes) antes de que empezara; 0 si no lo sabe medir.
    pub prepare_us: u32,
    /// Si se reuso lo fijo del fotograma anterior (programas, ordenes) y
    /// solo se subio lo que cambio. Lo decide el backend, no quien llama.
    pub warm: bool,
    /// El fotograma quedo EN VUELO (V1b): enviado sin esperar a que el
    /// aparato lo acabe -- la CPU orquesta, no espera. Entonces `device_us`
    /// es lo que la CPU espero a que hubiera sitio para el (0 si el aparato
    /// va por delante), no lo que tardo el aparato. Lo que quede en vuelo
    /// lo espera [`Backend::finish`].
    pub in_flight: bool,
}

/// **Por que un fotograma no se dibujo.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Los vertices no son un multiplo de 3, o pasan de lo que cabe.
    Vertices,
    /// La imagen no mide lo que dice el viewport.
    Image,
    /// El aparato lo nego o no contesto (con su motivo).
    Device(u32),
}

/// **Un backend de VERRANO**: dibuja un fotograma en una imagen.
pub trait Backend {
    fn draw(&mut self, frame: &Frame, out: &mut Image) -> Result<Stats, Error>;

    /// **Esperar lo que quedo en vuelo**: vuelve cuando el aparato acabo
    /// todos los fotogramas que `draw` envio. `Ok(us esperados)`. Un
    /// backend que no deja nada en vuelo (la CPU) no espera nada.
    fn finish(&mut self) -> Result<u32, Error> {
        Ok(0)
    }
}

/// Lo que la app comprueba antes de pedir nada (igual para todos los backends).
pub fn check(frame: &Frame, out: &Image, max_vertices: usize) -> Result<(), Error> {
    let n = frame.vertices.len();
    if n == 0 || n % 3 != 0 || n > max_vertices {
        return Err(Error::Vertices);
    }
    let (w, h) = (frame.viewport.width, frame.viewport.height);
    if w == 0 || h == 0 || w != out.width || h != out.height || out.pixels.len() < (w * h) as usize {
        return Err(Error::Image);
    }
    Ok(())
}
