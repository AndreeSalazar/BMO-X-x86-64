//! **VERRANO ESTUDIO** (11-10): el render de CALIDAD, al estilo RenderMan --
//! la luz de verdad, calculada por la CPU --, al lado de la trama (la de
//! tiempo real).
//!
//! El propietario, el 11-10: *"mi verrano puede renderizar como el estilo
//! renderman? en vez que mi GPU las tipicas rtx haga Ray tracing de Nvidia
//! haga el renderman exclusivo en verrano [...] pero que la CPU calcule
//! magistral"*. RenderMan (Pixar) no usa los nucleos RT: sus peliculas salen
//! sobre todo de la CPU, por TRAZADO DE CAMINOS. Esto es eso, en chico y de
//! la casa:
//!
//! ```text
//!    cada pixel     muchos rayos (muestras), cada uno con su sitio dentro
//!                   del pixel (el antialias sale solo)
//!    cada rayo      rebota por la escena: en lo mate, hacia cualquier lado
//!                   (con el coseno); en el espejo, como un espejo; en el
//!                   vidrio, se refleja o se refracta (Fresnel, de Schlick)
//!    la luz         una de AREA (no un punto): en cada rebote mate se le
//!                   pregunta directo (la sombra SUAVE sale de su tamanio) y
//!                   lo que rebota trae la luz INDIRECTA: el rojo de una
//!                   pared tine el suelo (el "color bleeding")
//!    la ruleta      despues de 3 rebotes, un camino que trae poco se corta
//!                   al azar y el que sigue vale mas: sin sesgo
//!    el techo       las causticas (de lo mate, por el vidrio, a la luz)
//!                   llevan un techo, como `clampIndirect` de RenderMan: sin
//!                   "luciernagas" (`TECHO_INDIRECTO`)
//!    la pelicula    lo de cada pixel es LUZ (sin techo); se lleva a la
//!                   pantalla con la curva ACES (la del cine) y el gamma
//! ```
//!
//! [carril]  VERDE     cuentas puras; ni un `unsafe`, ni un aparato
//! [cuesta]  NADA      una cuenta mal da una foto mal, nada mas
//!
//! Lo que cuesta en la CPU: rayos * rebotes; un pixel no mira a otro, y se
//! reparte entre todos los nucleos por filas (`pixel`). Determinista: la
//! misma semilla, la misma imagen.
//!
//! Lo que es de la casa: la raiz es la de PROMETEO (`programa::raiz`, la
//! exacta); el log2, el de `mates`; el azar, un xorshift con semilla por
//! pixel. Lo que NO es todavia: texturas, mallas de triangulos, la 3060
//! (el mismo trazado en su computo es el paso siguiente: un rayo por hilo).

use bmo_prometeo::mates;
use bmo_prometeo::programa::raiz;

/// Un vector de tres.
pub type V3 = [f32; 3];

pub(crate) fn mas(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn menos(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn por(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn mul(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}
pub(crate) fn punto(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cruz(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn unidad(a: V3) -> V3 {
    por(a, 1.0 / raiz(punto(a, a)))
}

/// 2^x para x en [-126, 127]: la parte entera en el exponente, la fraccion
/// con su polinomio (un error de ~1e-7: para la pantalla, de sobra).
fn exp2(x: f32) -> f32 {
    let x = x.clamp(-126.0, 127.0);
    let i = mates::suelo(x);
    let f = x - i;
    let p = 1.0 + f * (0.693_147_2 + f * (0.240_226_5 + f * (0.055_504_11 + f * (0.009_618_129 + f * (0.001_333_355 + f * 0.000_154_035_3)))));
    p * f32::from_bits(((i as i32 + 127) as u32) << 23)
}

/// x^p para x > 0.
fn potencia(x: f32, p: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else {
        exp2(p * mates::log2(x))
    }
}

/// **El azar**: un xorshift de 32 bits, con su semilla.
#[derive(Clone, Copy, Debug)]
pub struct Azar(u32);

impl Azar {
    pub fn nuevo(semilla: u32) -> Azar {
        // mezclar la semilla para que pixeles vecinos no empiecen parecido
        let mut s = semilla.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        s ^= s >> 16;
        Azar(s | 1)
    }
    /// En [0, 1).
    pub fn uno(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// **Un material.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Material {
    /// Mate (Lambert): la luz sale hacia cualquier lado, con el coseno.
    Mate { color: V3 },
    /// Metal: un espejo de `color`, borroso por `aspereza` (0, perfecto).
    Metal { color: V3, aspereza: f32 },
    /// Vidrio: refracta con su indice (1,5) y refleja segun Fresnel.
    Vidrio { indice: f32 },
    /// Lo que da luz.
    Luz { emite: V3 },
    /// **LA SUPERFICIE** (VE4a): el material por capas de los render de
    /// pelicula -- el `standard_surface` de Arnold, el OpenPBR --, ver
    /// [`Superficie`].
    Superficie(Superficie),
    /// **LA PIEL** (VE4b): lo que deja entrar la luz y la saca por otro
    /// sitio -- la piel, el jade, el marmol, la cera --. `color`: lo que se
    /// ve de lejos (el albedo de todo el paseo); `radio`: cuanto anda la luz
    /// por dentro de cada color antes de chocar (el rojo, mas: por eso la
    /// oreja a contraluz es roja); `ior`: el brillo de su piel de fuera.
    Piel { color: V3, radio: V3, ior: f32 },
}

/// **LA SUPERFICIE** (VE4a, 11-10): un material por CAPAS, como el
/// `standard_surface` de Arnold y el OpenPBR (las dos son especificaciones
/// abiertas: se toma la IDEA, no su codigo). De abajo arriba:
///
/// ```text
///    la base      lo mate de debajo, de `base` (Lambert), lo que el brillo
///                 deja pasar (1 - Fresnel, al entrar y al salir)
///    el brillo    microfacetas GGX de `aspereza` (la de Trowbridge-Reitz,
///                 la de todos: Arnold, RenderMan, Cycles, Unreal), con la
///                 sombra de Smith y Fresnel de Schlick. Dielectrico: blanco,
///                 con su `ior`; metal: del color de la `base`
///    el metal     de 0 a 1: cuanto es metal (sin base mate, brillo de color)
///    la capa      el barniz de encima (`capa`, de 0 a 1): otro brillo GGX,
///                 transparente y de ior 1,5, con su `capa_aspereza` -- la
///                 pintura de un coche, la madera barnizada --; lo de debajo
///                 recibe lo que el barniz no refleja
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Superficie {
    pub base: V3,
    pub metal: f32,
    pub aspereza: f32,
    pub ior: f32,
    pub capa: f32,
    pub capa_aspereza: f32,
}

impl Superficie {
    /// Un plastico (o una ceramica): base de color y brillo blanco.
    pub const fn plastico(base: V3, aspereza: f32) -> Superficie {
        Superficie { base, metal: 0.0, aspereza, ior: 1.5, capa: 0.0, capa_aspereza: 0.0 }
    }
    /// Un metal: el brillo del color de la base.
    pub const fn metal(base: V3, aspereza: f32) -> Superficie {
        Superficie { base, metal: 1.0, aspereza, ior: 1.5, capa: 0.0, capa_aspereza: 0.0 }
    }
}

/// Lo mas liso que se deja a GGX (un espejo PERFECTO es otro material: con
/// aspereza 0 la cuenta divide por cero).
const ASPEREZA_MIN: f32 = 0.02;

/// Fresnel de Schlick: lo que refleja con `f0` de frente, a `coseno`.
fn schlick(f0: f32, coseno: f32) -> f32 {
    let x = (1.0 - coseno).clamp(0.0, 1.0);
    let x2 = x * x;
    f0 + (1.0 - f0) * x2 * x2 * x
}

/// La distribucion GGX (Trowbridge-Reitz) de las microfacetas, con `a` =
/// aspereza^2 (la convencion de Disney, la que siguen todos).
fn ggx_d(a: f32, nh: f32) -> f32 {
    let a2 = a * a;
    // su minimo es a^2 (nh = 1); el redondeo de un nh de 1,0000001 lo
    // llevaba a 0 -- un infinito, y un NaN al multiplicarlo por una capa
    // que pesa 0: pixeles negros en la foto --
    let t = (nh * nh * (a2 - 1.0) + 1.0).max(a2);
    a2 / (core::f32::consts::PI * t * t)
}

/// La sombra de Smith de GGX en una direccion (G1).
fn ggx_g1(a: f32, nv: f32) -> f32 {
    let a2 = a * a;
    2.0 * nv / (nv + raiz(a2 + (1.0 - a2) * nv * nv))
}

/// Lo que pesa cada capa al MUESTREAR (no al evaluar): cuanto refleja cada
/// una vista desde `cos_o`. Suman 1.
fn reparto(m: &Superficie, cos_o: f32) -> (f32, f32, f32) {
    let f0d = ((m.ior - 1.0) / (m.ior + 1.0)) * ((m.ior - 1.0) / (m.ior + 1.0));
    let fc = m.capa * schlick(0.04, cos_o);
    let f0 = luminancia(mezcla_v(&[f0d; 3], &m.base, m.metal));
    let ws = (1.0 - fc) * schlick(f0, cos_o);
    let wd = (1.0 - fc) * (1.0 - m.metal) * luminancia(m.base) * (1.0 - schlick(f0d, cos_o));
    let t = fc + ws + wd;
    if t <= 0.0 {
        (0.0, 0.0, 1.0)
    } else {
        (fc / t, ws / t, wd / t)
    }
}

fn luminancia(v: V3) -> f32 {
    0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2]
}

fn mezcla_v(a: &V3, b: &V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

impl Superficie {
    fn asperezas(&self) -> (f32, f32) {
        let a = self.aspereza.max(ASPEREZA_MIN);
        let c = self.capa_aspereza.max(ASPEREZA_MIN);
        (a * a, c * c)
    }

    /// **Lo que refleja** (la BSDF, sin el coseno): de la luz que llega por
    /// `wi`, cuanta sale hacia `wo` (los dos, desde el punto hacia fuera).
    pub fn refleja(&self, n: V3, wo: V3, wi: V3) -> V3 {
        let (cos_o, cos_i) = (punto(n, wo), punto(n, wi));
        if cos_o <= 0.0 || cos_i <= 0.0 {
            return [0.0; 3];
        }
        let h = unidad(mas(wo, wi));
        let (nh, oh) = (punto(n, h).max(0.0), punto(wo, h).max(0.0));
        let (a, ac) = self.asperezas();
        let f0d = ((self.ior - 1.0) / (self.ior + 1.0)) * ((self.ior - 1.0) / (self.ior + 1.0));
        // el brillo: D G F / (4 cos cos)
        let dg = ggx_d(a, nh) * ggx_g1(a, cos_o) * ggx_g1(a, cos_i) / (4.0 * cos_o * cos_i);
        let f0 = mezcla_v(&[f0d; 3], &self.base, self.metal);
        let brillo = f0.map(|f| schlick(f, oh) * dg);
        // la base: lo que el brillo deja entrar y salir
        let pasa = (1.0 - schlick(f0d, cos_o)) * (1.0 - schlick(f0d, cos_i));
        let mate = por(self.base, (1.0 - self.metal) * pasa / core::f32::consts::PI);
        let debajo = mas(brillo, mate);
        if self.capa <= 0.0 {
            return debajo;
        }
        // el barniz: su brillo, y lo de debajo con lo que el deja pasar
        let barniz = self.capa * schlick(0.04, oh) * ggx_d(ac, nh) * ggx_g1(ac, cos_o) * ggx_g1(ac, cos_i) / (4.0 * cos_o * cos_i);
        let deja = (1.0 - self.capa * schlick(0.04, cos_o)) * (1.0 - self.capa * schlick(0.04, cos_i));
        mas(por(debajo, deja), [barniz; 3])
    }

    /// **La probabilidad** (por angulo solido) de que [`muestrea`](Self::muestrea)
    /// saque `wi` viendo desde `wo`: la de cada capa, por lo que pesa.
    pub fn probabilidad(&self, n: V3, wo: V3, wi: V3) -> f32 {
        let (cos_o, cos_i) = (punto(n, wo), punto(n, wi));
        if cos_o <= 0.0 || cos_i <= 0.0 {
            return 0.0;
        }
        let h = unidad(mas(wo, wi));
        let (nh, oh) = (punto(n, h).max(0.0), punto(wo, h).max(1e-6));
        let (a, ac) = self.asperezas();
        let (pc, ps, pd) = reparto(self, cos_o);
        // la capa que no pesa no se cuenta (ni su infinito, si lo hubiera)
        let ggx = |p: f32, a: f32| if p > 0.0 { p * ggx_d(a, nh) * nh / (4.0 * oh) } else { 0.0 };
        ggx(pc, ac) + ggx(ps, a) + pd * cos_i / core::f32::consts::PI
    }

    /// **Una direccion de salida** al azar viendo desde `wo`: se elige una
    /// capa por lo que pesa y se saca de ella (la normal de una microfaceta
    /// de GGX y su reflejo, o el coseno de la base). `None` si cae por
    /// debajo de la superficie (la luz se pierde ahi: la sombra de Smith).
    pub fn muestrea(&self, n: V3, wo: V3, az: &mut Azar) -> Option<V3> {
        let cos_o = punto(n, wo);
        let (pc, ps, _) = reparto(self, cos_o);
        let (a, ac) = self.asperezas();
        let u = az.uno();
        let wi = if u < pc + ps {
            let a = if u < pc { ac } else { a };
            // la microfaceta: tan^2 = a^2 e / (1 - e), y un giro al azar
            // (un punto del circulo, por rechazo: sin seno ni coseno)
            let e = az.uno();
            let tan = a * raiz(e / (1.0 - e).max(1e-7));
            let (x, y) = loop {
                let (x, y) = (2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0);
                let r2 = x * x + y * y;
                if r2 > 1e-6 && r2 < 1.0 {
                    let r = raiz(r2);
                    break (x / r, y / r);
                }
            };
            let (t, b) = base_de(n);
            let h = unidad(mas(n, por(mas(por(t, x), por(b, y)), tan)));
            menos(por(h, 2.0 * punto(wo, h)), wo)
        } else {
            coseno(n, az)
        };
        (punto(n, wi) > 0.0).then_some(wi)
    }
}

/// Dos ejes perpendiculares a `n` (y entre si).
fn base_de(n: V3) -> (V3, V3) {
    let t = unidad(if n[0].abs() > 0.5 { cruz(n, [0.0, 1.0, 0.0]) } else { cruz(n, [1.0, 0.0, 0.0]) });
    (t, cruz(n, t))
}

/// **MIS** (Veach, 1997; lo usan Arnold, RenderMan y Cycles): la misma luz
/// se puede encontrar de DOS maneras -- preguntando a la luz o rebotando y
/// dando con ella --; cada una es buena donde la otra es mala (la luz
/// grande en lo aspero; la luz chica en lo pulido). Se usan las dos, cada
/// una con su peso (el de la potencia: p^2 / (p^2 + q^2)), y sin sesgo.
fn potencia_mis(p: f32, q: f32) -> f32 {
    let (p2, q2) = (p * p, q * q);
    if p2 + q2 <= 0.0 {
        0.0
    } else {
        p2 / (p2 + q2)
    }
}

/// **Lo que hay en la escena.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Forma {
    Esfera { centro: V3, radio: f32 },
    /// Un rectangulo: una esquina y sus dos lados (su normal, `u x v`).
    Rect { esquina: V3, u: V3, v: V3 },
    /// Una MALLA de triangulos (VE3): la `indice` de las de la escena.
    Malla { indice: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Objeto {
    pub forma: Forma,
    pub material: Material,
}

/// **La camara**: desde donde, hacia donde, y su apertura vertical (la
/// tangente de la mitad del angulo).
#[derive(Clone, Copy, Debug)]
pub struct Camara {
    pub ojo: V3,
    pub mira: V3,
    pub arriba: V3,
    pub tan_medio: f32,
}

/// **Una escena**: sus objetos, cual es la luz de area (un `Rect` de
/// `Material::Luz`), la camara y sus mallas (las que nombran los objetos de
/// `Forma::Malla`; ninguna: `&[]`).
#[derive(Clone, Copy, Debug)]
pub struct Escena<'a> {
    pub objetos: &'a [Objeto],
    pub luz: usize,
    pub camara: Camara,
    pub mallas: &'a [crate::malla::Malla],
    /// La NIEBLA (VE4b), si la hay.
    pub niebla: Option<Niebla>,
}

/// **LA NIEBLA** (VE4b): un medio que llena una caja -- el humo, la bruma,
/// el polvo en el aire --. La luz que la cruza se apaga con la distancia
/// (Beer: `e^(-densidad d)`) y una parte se DESVIA hacia el ojo: los rayos
/// de luz que se ven en el aire.
///
/// ```text
///    densidad   cuanto choca por unidad de camino
///    albedo     de lo que choca, cuanto se desvia (el resto se apaga)
///    g          hacia donde se desvia (Henyey-Greenstein): 0 a todos
///               lados, > 0 hacia delante (la bruma, el humo)
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Niebla {
    pub densidad: f32,
    pub albedo: f32,
    pub g: f32,
    pub min: V3,
    pub max: V3,
}

impl Niebla {
    /// El tramo del rayo `o + t d` (t en [0, lim]) dentro de la caja.
    fn tramo(&self, o: V3, d: V3, lim: f32) -> Option<(f32, f32)> {
        let (mut t0, mut t1) = (0.0f32, lim);
        for k in 0..3 {
            let inv = 1.0 / d[k];
            let (a, b) = ((self.min[k] - o[k]) * inv, (self.max[k] - o[k]) * inv);
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return None;
            }
        }
        Some((t0, t1))
    }

    /// Lo que llega de la luz tras `dist` por `d` desde `o`.
    fn transmite(&self, o: V3, d: V3, dist: f32) -> f32 {
        self.tramo(o, d, dist).map_or(1.0, |(t0, t1)| exp(-self.densidad * (t1 - t0)))
    }
}

/// e^x (por `exp2`).
fn exp(x: f32) -> f32 {
    exp2(x * core::f32::consts::LOG2_E)
}

/// ln x (por el log2 de PROMETEO).
fn ln(x: f32) -> f32 {
    mates::log2(x) * core::f32::consts::LN_2
}

/// Henyey-Greenstein: cuanto se desvia hacia un angulo de coseno `c` (por
/// angulo solido; integra 1).
fn hg(g: f32, c: f32) -> f32 {
    let x = 1.0 + g * g - 2.0 * g * c;
    (1.0 - g * g) / (4.0 * core::f32::consts::PI * x * raiz(x))
}

/// Una direccion desviada de `d` por Henyey-Greenstein (exacta: su
/// probabilidad es `hg`).
fn muestrea_hg(g: f32, d: V3, az: &mut Azar) -> V3 {
    let e = az.uno();
    let c = if g.abs() < 1e-3 {
        1.0 - 2.0 * e
    } else {
        let q = (1.0 - g * g) / (1.0 - g + 2.0 * g * e);
        ((1.0 + g * g - q * q) / (2.0 * g)).clamp(-1.0, 1.0)
    };
    let s = raiz((1.0 - c * c).max(0.0));
    let (x, y) = loop {
        let (x, y) = (2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0);
        let r2 = x * x + y * y;
        if r2 > 1e-6 && r2 < 1.0 {
            let r = raiz(r2);
            break (x / r, y / r);
        }
    };
    let (t, b) = base_de(d);
    unidad(mas(por(d, c), por(mas(por(t, x), por(b, y)), s)))
}

/// **El albedo de cada choque** dentro de LA PIEL para que el paseo entero
/// devuelva `a` (la inversion de van de Hulst, la de Cycles y de Arnold para
/// el paseo al azar con entrada de Lambert).
pub fn albedo_de_paseo(a: f32) -> f32 {
    let a = a.clamp(0.0, 0.999);
    let s = 4.09712 + 4.20863 * a - raiz(9.59217 + 41.6808 * a + 17.7126 * a * a);
    1.0 - s * s
}

/// **EL PASEO** (VE4b): la luz que entro en `o` por `d` anda por dentro --
/// choca, se desvia a cualquier lado, se apaga un poco en cada choque --
/// hasta que SALE. `sale(p, d)` da cuanto falta hasta el borde y la normal
/// de fuera alli (`None`: por ahi no hay borde, como el fondo de una losa). Cada paso elige un color al azar para la distancia (el
/// "color heroe") y pesa los tres por lo que les tocaba (sin sesgo).
/// `Some((donde sale, su normal de fuera, el peso de cada color))`, o `None`
/// si la luz se quedo dentro.
pub fn paseo(o: V3, d: V3, color: V3, radio: V3, az: &mut Azar, sale: impl Fn(V3, V3) -> Option<(f32, V3)>) -> Option<(V3, V3, V3)> {
    let sigma = radio.map(|r| 1.0 / r.max(1e-4));
    let alfa = color.map(albedo_de_paseo);
    let (mut p, mut d, mut w) = (o, d, [1.0f32; 3]);
    for _ in 0..256 {
        let c = ((az.uno() * 3.0) as usize).min(2);
        let t = -ln(1.0 - az.uno()) / sigma[c];
        let (borde, n) = sale(p, d).unwrap_or((f32::INFINITY, [0.0; 3]));
        if t >= borde {
            let tr = sigma.map(|s| exp(-s * borde));
            let q = (tr[0] + tr[1] + tr[2]) / 3.0;
            w = [w[0] * tr[0] / q, w[1] * tr[1] / q, w[2] * tr[2] / q];
            return Some((mas(p, por(d, borde)), n, w));
        }
        let tr = [sigma[0] * exp(-sigma[0] * t), sigma[1] * exp(-sigma[1] * t), sigma[2] * exp(-sigma[2] * t)];
        let q = (tr[0] + tr[1] + tr[2]) / 3.0;
        w = [w[0] * alfa[0] * tr[0] / q, w[1] * alfa[1] * tr[1] / q, w[2] * alfa[2] * tr[2] / q];
        // casi nada: se queda dentro (y el peso no explota)
        if w[0].max(w[1]).max(w[2]) < 1e-4 {
            return None;
        }
        p = mas(p, por(d, t));
        d = unidad(en_esfera(az));
    }
    None
}

struct Golpe {
    p: V3,
    n: V3,
    fuera: bool,
    k: usize,
}

const EPS: f32 = 1e-3;

/// **El techo de la luz INDIRECTA** (el `clampIndirect` de RenderMan): una
/// caustica -- de lo mate, por el vidrio o el metal, a la luz -- es un
/// camino raro y fortisimo; sin techo, salen "luciernagas" (pixeles blancos
/// sueltos) hasta con miles de muestras. Con el, un poco menos de caustica
/// (un sesgo pequenio y dicho) y una imagen limpia. Lo directo no se toca.
pub const TECHO_INDIRECTO: f32 = 2.0;

fn golpea(f: &Forma, o: V3, d: V3, t_max: f32) -> Option<(f32, V3)> {
    match *f {
        Forma::Esfera { centro, radio } => {
            let oc = menos(o, centro);
            let b = punto(oc, d);
            let c = punto(oc, oc) - radio * radio;
            let disc = b * b - c;
            if disc < 0.0 {
                return None;
            }
            let r = raiz(disc);
            for t in [-b - r, -b + r] {
                if t > EPS && t < t_max {
                    return Some((t, por(menos(mas(o, por(d, t)), centro), 1.0 / radio)));
                }
            }
            None
        }
        // las mallas las mira la escena (`Escena::golpe`): son suyas
        Forma::Malla { .. } => None,
        Forma::Rect { esquina, u, v } => {
            let n = cruz(u, v);
            let den = punto(n, d);
            if den.abs() < 1e-9 {
                return None;
            }
            let t = punto(n, menos(esquina, o)) / den;
            if t <= EPS || t >= t_max {
                return None;
            }
            let q = menos(mas(o, por(d, t)), esquina);
            let (a, b) = (punto(q, u) / punto(u, u), punto(q, v) / punto(v, v));
            if (0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b) {
                Some((t, unidad(n)))
            } else {
                None
            }
        }
    }
}

impl Escena<'_> {
    fn golpe(&self, o: V3, d: V3, t_max: f32) -> Option<Golpe> {
        let mut mejor: Option<Golpe> = None;
        let mut lim = t_max;
        for (k, ob) in self.objetos.iter().enumerate() {
            if let Forma::Malla { indice } = ob.forma {
                let Some(g) = self.mallas.get(indice as usize).and_then(|m| m.golpea(o, d, lim)) else { continue };
                lim = g.t;
                // de que lado, por el triangulo; la normal suave, hacia el
                // rayo (si la mezcla la tuerce de mas, la del triangulo)
                let fuera = punto(d, g.geo) < 0.0;
                let n = if fuera { g.n } else { por(g.n, -1.0) };
                let n = if punto(d, n) < 0.0 { n } else if fuera { g.geo } else { por(g.geo, -1.0) };
                mejor = Some(Golpe { p: mas(o, por(d, g.t)), n, fuera, k });
                continue;
            }
            if let Some((t, n)) = golpea(&ob.forma, o, d, lim) {
                lim = t;
                let fuera = punto(d, n) < 0.0;
                mejor = Some(Golpe { p: mas(o, por(d, t)), n: if fuera { n } else { por(n, -1.0) }, fuera, k });
            }
        }
        mejor
    }

    /// La luz DIRECTA en `p` (normal `n`, mate): un punto al azar de la luz
    /// de area, y si se ve (la sombra), lo que llega -- por eso la sombra es
    /// SUAVE: cada muestra ve un trozo distinto de la luz --.
    fn directa(&self, p: V3, n: V3, az: &mut Azar) -> V3 {
        let ob = &self.objetos[self.luz];
        let (Forma::Rect { esquina, u, v }, Material::Luz { emite }) = (ob.forma, ob.material) else { return [0.0; 3] };
        let q = mas(esquina, mas(por(u, az.uno()), por(v, az.uno())));
        let hacia = menos(q, p);
        let d2 = punto(hacia, hacia);
        let dist = raiz(d2);
        let l = por(hacia, 1.0 / dist);
        let cos_p = punto(n, l);
        let nl = unidad(cruz(u, v));
        let cos_l = punto(nl, l).abs();
        if cos_p <= 0.0 {
            return [0.0; 3];
        }
        // la sombra: algo entre p y la luz
        if let Some(g) = self.golpe(p, l, dist - EPS) {
            if g.k != self.luz {
                return [0.0; 3];
            }
        }
        let area = raiz(punto(cruz(u, v), cruz(u, v)));
        let niebla = self.niebla.map_or(1.0, |nb| nb.transmite(p, l, dist));
        por(emite, niebla * cos_p * cos_l * area / (d2 * core::f32::consts::PI))
    }

    /// **La luz DIRECTA en una SUPERFICIE** (VE4a): un punto al azar de la
    /// luz, por lo que refleja hacia `wo`, con el peso de MIS.
    fn directa_superficie(&self, p: V3, n: V3, wo: V3, m: &Superficie, az: &mut Azar) -> V3 {
        let ob = &self.objetos[self.luz];
        let (Forma::Rect { esquina, u, v }, Material::Luz { emite }) = (ob.forma, ob.material) else { return [0.0; 3] };
        let q = mas(esquina, mas(por(u, az.uno()), por(v, az.uno())));
        let hacia = menos(q, p);
        let d2 = punto(hacia, hacia);
        let dist = raiz(d2);
        let l = por(hacia, 1.0 / dist);
        let cos_p = punto(n, l);
        // la luz alumbra por su cara de delante (la de `u x v`)
        let cos_l = -punto(unidad(cruz(u, v)), l);
        if cos_p <= 0.0 || cos_l <= 0.0 {
            return [0.0; 3];
        }
        if let Some(g) = self.golpe(p, l, dist - EPS) {
            if g.k != self.luz {
                return [0.0; 3];
            }
        }
        let area = raiz(punto(cruz(u, v), cruz(u, v)));
        let p_luz = d2 / (cos_l * area);
        let w = potencia_mis(p_luz, m.probabilidad(n, wo, l));
        let niebla = self.niebla.map_or(1.0, |nb| nb.transmite(p, l, dist));
        mul(emite, por(m.refleja(n, wo, l), niebla * cos_p * w / p_luz))
    }

    /// **La luz directa en un punto de la NIEBLA** (VE4b): un punto al azar
    /// de la luz, por lo que se desvia hacia `d` (Henyey-Greenstein) y lo que
    /// la niebla deja llegar.
    fn directa_niebla(&self, p: V3, d: V3, nb: &Niebla, az: &mut Azar) -> V3 {
        let ob = &self.objetos[self.luz];
        let (Forma::Rect { esquina, u, v }, Material::Luz { emite }) = (ob.forma, ob.material) else { return [0.0; 3] };
        let q = mas(esquina, mas(por(u, az.uno()), por(v, az.uno())));
        let hacia = menos(q, p);
        let d2 = punto(hacia, hacia);
        let dist = raiz(d2);
        let l = por(hacia, 1.0 / dist);
        let cos_l = -punto(unidad(cruz(u, v)), l);
        if cos_l <= 0.0 {
            return [0.0; 3];
        }
        if let Some(g) = self.golpe(p, l, dist - EPS) {
            if g.k != self.luz {
                return [0.0; 3];
            }
        }
        let area = raiz(punto(cruz(u, v), cruz(u, v)));
        por(emite, hg(nb.g, punto(d, l)) * nb.transmite(p, l, dist) * cos_l * area / d2)
    }

    /// La probabilidad (por angulo solido) de que [`directa_superficie`]
    /// saque la luz que se ve a `t` por `d`, golpeada con normal `n`.
    fn probabilidad_luz(&self, d: V3, n: V3, t: f32) -> f32 {
        let Forma::Rect { u, v, .. } = self.objetos[self.luz].forma else { return 0.0 };
        let area = raiz(punto(cruz(u, v), cruz(u, v)));
        let cos_l = -punto(d, n);
        if cos_l <= 0.0 {
            0.0
        } else {
            t * t / (cos_l * area)
        }
    }

    /// **La luz que llega por un rayo** (el camino entero).
    pub fn camino(&self, o: V3, d: V3, az: &mut Azar) -> V3 {
        self.camino_con(o, d, az, None, None)
    }

    /// **El camino con la MEMORIA DE LA LUZ** (ESTUDIO VIVO, `vivo.rs`): en
    /// un rebote mate que no es el primero, si la memoria ya sabe la luz que
    /// sale de ese sitio, se usa y el camino ACABA ahi (mas corto, y sin el
    /// ruido de lo que falta); y lo que este camino aprende de cada sitio
    /// mate se APUNTA (la luz que sale de el), para educar la memoria. El
    /// primer golpe siempre se calcula entero: las sombras directas, nitidas.
    pub fn camino_con(&self, mut o: V3, mut d: V3, az: &mut Azar, memoria: Option<&crate::vivo::Memoria>, mut apuntes: Option<&mut dyn FnMut(crate::vivo::Apunte)>) -> V3 {
        let mut luz = [0.0f32; 3];
        let mut peso = [1.0f32; 3];
        // los sitios mate de este camino: (punto, normal, luz y peso al llegar)
        let mut vistos: [(V3, V3, V3, V3); 4] = [([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]); 4];
        let mut n_vistos = 0;
        let mut especular = true;
        // si el camino ya paso por algo mate: lo que llegue de la luz por un
        // espejo o un vidrio despues es una CAUSTICA, y se le pone techo
        let mut vio_mate = false;
        // si el ultimo rebote fue una SUPERFICIE, con que probabilidad saco
        // su direccion (para el peso de MIS si da con la luz)
        let mut ultima: Option<f32> = None;
        for rebote in 0..12 {
            let golpe = self.golpe(o, d, f32::MAX);
            // ** LA NIEBLA (VE4b): antes del golpe, el rayo puede CHOCAR en
            // el aire (a una distancia al azar, la de Beer); si choca, se
            // desvia ahi, con su luz directa
            if let Some(nb) = self.niebla {
                let lim = golpe.as_ref().map_or(f32::MAX, |g| raiz(punto(menos(g.p, o), menos(g.p, o))));
                if let Some((t0, t1)) = nb.tramo(o, d, lim) {
                    let t = t0 - ln(1.0 - az.uno()) / nb.densidad;
                    if t < t1 {
                        let p = mas(o, por(d, t));
                        peso = por(peso, nb.albedo);
                        luz = mas(luz, mul(peso, self.directa_niebla(p, d, &nb, az)));
                        d = muestrea_hg(nb.g, d, az);
                        o = p;
                        (especular, ultima, vio_mate) = (false, None, true);
                        if !ruleta(rebote, &mut peso, az) {
                            break;
                        }
                        continue;
                    }
                }
            }
            let Some(g) = golpe else { break };
            // de donde sale el rayo siguiente (LA PIEL lo saca por otro sitio)
            let mut desde = g.p;
            match self.objetos[g.k].material {
                Material::Luz { emite } => {
                    // la luz se ve directa solo desde la camara o un espejo:
                    // desde lo mate ya se conto (`directa`)
                    let w = match ultima {
                        _ if !g.fuera => 0.0,
                        // por un rebote de SUPERFICIE: MIS contra preguntar a la luz
                        Some(pb) => {
                            let t = raiz(punto(menos(g.p, o), menos(g.p, o)));
                            potencia_mis(pb, self.probabilidad_luz(d, g.n, t))
                        }
                        None if especular => 1.0,
                        None => 0.0,
                    };
                    if w > 0.0 {
                        let mut c = por(mul(peso, emite), w);
                        if vio_mate {
                            c = c.map(|x| x.min(TECHO_INDIRECTO));
                        }
                        luz = mas(luz, c);
                    }
                    break;
                }
                Material::Mate { color } => {
                    if rebote > 0 {
                        if let Some(l) = memoria.and_then(|m| m.mirar(g.p, g.n)) {
                            luz = mas(luz, mul(peso, l));
                            break;
                        }
                    }
                    if n_vistos < vistos.len() {
                        vistos[n_vistos] = (g.p, g.n, luz, peso);
                        n_vistos += 1;
                    }
                    luz = mas(luz, mul(peso, mul(color, self.directa(g.p, g.n, az))));
                    peso = mul(peso, color);
                    d = coseno(g.n, az);
                    especular = false;
                    vio_mate = true;
                    ultima = None;
                }
                Material::Superficie(m) => {
                    let wo = por(d, -1.0);
                    luz = mas(luz, mul(peso, self.directa_superficie(g.p, g.n, wo, &m, az)));
                    let Some(wi) = m.muestrea(g.n, wo, az) else { break };
                    let pb = m.probabilidad(g.n, wo, wi);
                    if pb <= 0.0 {
                        break;
                    }
                    peso = mul(peso, por(m.refleja(g.n, wo, wi), punto(g.n, wi) / pb));
                    d = wi;
                    especular = false;
                    ultima = Some(pb);
                }
                Material::Metal { color, aspereza } => {
                    let r = menos(d, por(g.n, 2.0 * punto(d, g.n)));
                    d = unidad(mas(r, por(en_esfera(az), aspereza)));
                    if punto(d, g.n) <= 0.0 {
                        break;
                    }
                    peso = mul(peso, color);
                    especular = true;
                    ultima = None;
                }
                Material::Vidrio { indice } => {
                    let eta = if g.fuera { 1.0 / indice } else { indice };
                    let cos_i = -punto(d, g.n);
                    let sin2 = eta * eta * (1.0 - cos_i * cos_i);
                    let r0 = ((1.0 - indice) / (1.0 + indice)) * ((1.0 - indice) / (1.0 + indice));
                    let x = 1.0 - cos_i;
                    let fresnel = r0 + (1.0 - r0) * x * x * x * x * x;
                    d = if sin2 > 1.0 || az.uno() < fresnel {
                        menos(d, por(g.n, 2.0 * punto(d, g.n)))
                    } else {
                        unidad(mas(por(d, eta), por(g.n, eta * cos_i - raiz(1.0 - sin2))))
                    };
                    especular = true;
                    ultima = None;
                }
                Material::Piel { color, radio, ior } => {
                    if !g.fuera {
                        break;
                    }
                    // su piel de fuera: refleja como un espejo lo que dice
                    // Fresnel; lo demas ENTRA (con el coseno) y pasea
                    let r0 = ((1.0 - ior) / (1.0 + ior)) * ((1.0 - ior) / (1.0 + ior));
                    if az.uno() < schlick(r0, -punto(d, g.n)) {
                        d = menos(d, por(g.n, 2.0 * punto(d, g.n)));
                        especular = true;
                        ultima = None;
                    } else {
                        let k = g.k;
                        let sale = |p: V3, d: V3| {
                            let s = self.golpe(p, d, f32::MAX)?;
                            // el borde, visto desde dentro: su normal de fuera
                            let n = if s.k == k && !s.fuera { por(s.n, -1.0) } else { s.n };
                            Some((raiz(punto(menos(s.p, p), menos(s.p, p))), n))
                        };
                        let Some((p, n, w)) = paseo(g.p, coseno(por(g.n, -1.0), az), color, radio, az, sale) else { break };
                        // sale como de algo mate: su luz directa y un rebote
                        peso = mul(peso, w);
                        luz = mas(luz, mul(peso, self.directa(p, n, az)));
                        d = coseno(n, az);
                        desde = p;
                        (especular, ultima, vio_mate) = (false, None, true);
                    }
                }
            }
            o = desde;
            if !ruleta(rebote, &mut peso, az) {
                break;
            }
        }
        // lo aprendido: la luz que salio de cada sitio mate, por lo que el
        // camino junto DESPUES de el (dividido por lo que pesaba al llegar)
        if let Some(a) = apuntes.as_deref_mut() {
            for &(p, n, antes, w) in &vistos[..n_vistos] {
                if w.iter().all(|&x| x > 1e-6) {
                    let l = menos(luz, antes);
                    a(crate::vivo::Apunte { p, n, l: [l[0] / w[0], l[1] / w[1], l[2] / w[2]] });
                }
            }
        }
        luz
    }

    /// **La GUIA de un rayo** (VE5, para completar): lo que ve primero --
    /// su normal, a que distancia y el color de su material -- sin luz y sin
    /// azar. `None` si no golpea nada.
    pub fn guia(&self, o: V3, d: V3) -> Option<(V3, f32, V3)> {
        let g = self.golpe(o, d, f32::MAX)?;
        let albedo = match self.objetos[g.k].material {
            Material::Mate { color } | Material::Metal { color, .. } => color,
            Material::Superficie(m) => m.base,
            Material::Piel { color, .. } => color,
            Material::Vidrio { .. } | Material::Luz { .. } => [1.0; 3],
        };
        let dist = raiz(punto(menos(g.p, o), menos(g.p, o)));
        Some((g.n, dist, albedo))
    }

    /// El rayo de la camara por el CENTRO del pixel (x, y).
    pub fn rayo_centro(&self, x: u32, y: u32, ancho: u32, alto: u32) -> (V3, V3) {
        let c = self.camara;
        let w = unidad(menos(c.mira, c.ojo));
        let u = unidad(cruz(w, c.arriba));
        let v = cruz(u, w);
        let aspecto = ancho as f32 / alto as f32;
        let sx = (2.0 * (x as f32 + 0.5) / ancho as f32 - 1.0) * c.tan_medio * aspecto;
        let sy = (1.0 - 2.0 * (y as f32 + 0.5) / alto as f32) * c.tan_medio;
        (c.ojo, unidad(mas(w, mas(por(u, sx), por(v, sy)))))
    }

    /// El rayo de la camara por un sitio al azar del pixel (x, y).
    pub fn rayo(&self, x: u32, y: u32, ancho: u32, alto: u32, az: &mut Azar) -> (V3, V3) {
        let c = self.camara;
        let w = unidad(menos(c.mira, c.ojo));
        let u = unidad(cruz(w, c.arriba));
        let v = cruz(u, w);
        let aspecto = ancho as f32 / alto as f32;
        let sx = (2.0 * (x as f32 + az.uno()) / ancho as f32 - 1.0) * c.tan_medio * aspecto;
        let sy = (1.0 - 2.0 * (y as f32 + az.uno()) / alto as f32) * c.tan_medio;
        (c.ojo, unidad(mas(w, mas(por(u, sx), por(v, sy)))))
    }

    /// **Un pixel**: `muestras` rayos por el, cada uno en su sitio dentro
    /// del pixel, de media. La luz (sin techo), en lineal.
    pub fn pixel(&self, x: u32, y: u32, ancho: u32, alto: u32, muestras: u32) -> V3 {
        let mut az = Azar::nuevo(y * ancho + x + 1);
        let mut suma = [0.0f32; 3];
        for _ in 0..muestras {
            let (o, d) = self.rayo(x, y, ancho, alto, &mut az);
            suma = mas(suma, self.camino(o, d, &mut az));
        }
        por(suma, 1.0 / muestras as f32)
    }
}

/// **La ruleta**: despues de 3 rebotes, un camino que trae poco se corta al
/// azar y el que sigue vale mas: sin sesgo. `false`: se corto.
fn ruleta(rebote: u32, peso: &mut V3, az: &mut Azar) -> bool {
    if rebote >= 3 {
        let q = peso[0].max(peso[1]).max(peso[2]).min(0.95);
        if az.uno() > q {
            return false;
        }
        *peso = por(*peso, 1.0 / q);
    }
    true
}

/// Un punto al azar dentro de la esfera unidad (por rechazo: sin seno ni
/// coseno).
fn en_esfera(az: &mut Azar) -> V3 {
    loop {
        let p = [2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0];
        if punto(p, p) < 1.0 {
            return p;
        }
    }
}

/// Una direccion alrededor de `n` con el COSENO (lo de un mate): un punto
/// del disco unidad, levantado a la semiesfera (Malley).
fn coseno(n: V3, az: &mut Azar) -> V3 {
    let (a, b) = loop {
        let (a, b) = (2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0);
        if a * a + b * b < 1.0 {
            break (a, b);
        }
    };
    let z = raiz((1.0 - a * a - b * b).max(0.0));
    let t = unidad(if n[0].abs() > 0.5 { cruz(n, [0.0, 1.0, 0.0]) } else { cruz(n, [1.0, 0.0, 0.0]) });
    let s = cruz(n, t);
    unidad(mas(mas(por(t, a), por(s, b)), por(n, z)))
}

/// **De la luz a la pantalla**: la curva ACES (la del cine, el ajuste de
/// Narkowicz) y el gamma de sRGB (aprox. 1/2.2), a 8 bits: `0xRRGGBB`.
pub fn pelicula(l: V3, exposicion: f32) -> u32 {
    let c = |x: f32| {
        let x = x * exposicion;
        let t = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
        (potencia(t, 1.0 / 2.2) * 255.0 + 0.5) as u32
    };
    c(l[0]) << 16 | c(l[1]) << 8 | c(l[2])
}

/// **La caja de Cornell, al estilo de la casa**: paredes roja y verde, techo
/// y suelo blancos, una luz de area calida en el techo, y tres esferas -- una
/// de arcilla, una de ORO y una de VIDRIO --. La escena con la que se mide un
/// trazador de caminos desde 1984.
pub fn caja_de_cornell() -> ([Objeto; 9], usize, Camara) {
    let blanco = Material::Mate { color: [0.73, 0.73, 0.73] };
    let rojo = Material::Mate { color: [0.65, 0.05, 0.05] };
    let verde = Material::Mate { color: [0.12, 0.45, 0.15] };
    let rect = |esquina: V3, u: V3, v: V3, material| Objeto { forma: Forma::Rect { esquina, u, v }, material };
    let objetos = [
        // suelo (normal +y), techo (-y), fondo (-z)
        rect([-1.0, 0.0, -1.0], [0.0, 0.0, 2.0], [2.0, 0.0, 0.0], blanco),
        rect([-1.0, 2.0, -1.0], [2.0, 0.0, 0.0], [0.0, 0.0, 2.0], blanco),
        rect([-1.0, 0.0, -1.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], blanco),
        // izquierda roja (+x), derecha verde (-x)
        rect([-1.0, 0.0, -1.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0], rojo),
        rect([1.0, 0.0, -1.0], [0.0, 0.0, 2.0], [0.0, 2.0, 0.0], verde),
        // la luz: un rectangulo justo bajo el techo, mirando abajo
        rect([-0.3, 1.998, -0.3], [0.6, 0.0, 0.0], [0.0, 0.0, 0.6], Material::Luz { emite: [17.0, 13.5, 9.0] }),
        Objeto { forma: Forma::Esfera { centro: [-0.45, 0.35, -0.35], radio: 0.35 }, material: Material::Mate { color: [0.78, 0.62, 0.48] } },
        Objeto { forma: Forma::Esfera { centro: [0.45, 0.3, -0.5], radio: 0.3 }, material: Material::Metal { color: [1.0, 0.78, 0.34], aspereza: 0.04 } },
        Objeto { forma: Forma::Esfera { centro: [0.1, 0.25, 0.25], radio: 0.25 }, material: Material::Vidrio { indice: 1.5 } },
    ];
    let camara = Camara { ojo: [0.0, 1.0, 3.4], mira: [0.0, 0.95, 0.0], arriba: [0.0, 1.0, 0.0], tan_medio: 0.36 };
    (objetos, 5, camara)
}

/// **LA SALA DE MATERIALES** (VE4a): la caja de Cornell con CINCO esferas
/// de LA SUPERFICIE en fila, de izquierda a derecha -- plastico rojo,
/// oro cepillado, la pintura de un coche (azul metalizado con barniz),
/// cobre pulido y ceramica blanca -- y, delante, la de vidrio. Para ver lo
/// que hace cada capa con la misma luz.
pub fn sala_de_materiales() -> ([Objeto; 12], usize, Camara) {
    let (c, luz, _) = caja_de_cornell();
    let esfera = |x: f32, m: Superficie| Objeto { forma: Forma::Esfera { centro: [x, 0.17, -0.35], radio: 0.17 }, material: Material::Superficie(m) };
    let objetos = [
        c[0],
        c[1],
        c[2],
        c[3],
        c[4],
        c[5],
        esfera(-0.76, Superficie::plastico([0.72, 0.06, 0.05], 0.25)),
        esfera(-0.38, Superficie::metal([1.0, 0.78, 0.34], 0.35)),
        esfera(0.0, Superficie { base: [0.05, 0.16, 0.6], metal: 0.6, aspereza: 0.45, ior: 1.5, capa: 1.0, capa_aspereza: 0.03 }),
        esfera(0.38, Superficie::metal([0.95, 0.64, 0.54], 0.06)),
        esfera(0.76, Superficie::plastico([0.85, 0.85, 0.82], 0.08)),
        Objeto { forma: Forma::Esfera { centro: [0.0, 0.16, 0.35], radio: 0.16 }, material: Material::Vidrio { indice: 1.5 } },
    ];
    let camara = Camara { ojo: [0.0, 0.85, 3.4], mira: [0.0, 0.6, 0.0], arriba: [0.0, 1.0, 0.0], tan_medio: 0.36 };
    (objetos, luz, camara)
}

/// **LA SALA DE MALLAS** (VE3): la caja de Cornell con TRIANGULOS -- una
/// esfera de oro de 5120 (suave), una dona de pintura de coche de 36864
/// (suave) tumbada en el suelo, y un icosaedro de plastico rojo de 80 (con
/// sus facetas) --: unos 42 000 triangulos que cada rayo cruza mirando unas
/// pocas cajas del arbol. Devuelve tambien las mallas (la escena las presta).
pub fn sala_de_mallas() -> (alloc::vec::Vec<Objeto>, usize, Camara, alloc::vec::Vec<crate::malla::Malla>) {
    use crate::malla::{dona, icosfera, Malla};
    let (c, luz, _) = caja_de_cornell();
    let mut objetos: alloc::vec::Vec<Objeto> = c[..6].to_vec();
    let (v, k) = icosfera([-0.45, 0.32, -0.45], 0.32, 4);
    let esfera = Malla::nueva(&v, &k, true);
    let (v, k) = dona([0.42, 0.11, -0.3], 0.3, 0.11, 192, 96);
    let rosca = Malla::nueva(&v, &k, true);
    let (v, k) = icosfera([0.0, 0.2, 0.3], 0.2, 1);
    let facetas = Malla::nueva(&v, &k, false);
    let mallas = alloc::vec![esfera, rosca, facetas];
    objetos.push(Objeto { forma: Forma::Malla { indice: 0 }, material: Material::Superficie(Superficie::metal([1.0, 0.78, 0.34], 0.2)) });
    objetos.push(Objeto { forma: Forma::Malla { indice: 1 }, material: Material::Superficie(Superficie { base: [0.05, 0.16, 0.6], metal: 0.6, aspereza: 0.45, ior: 1.5, capa: 1.0, capa_aspereza: 0.03 }) });
    objetos.push(Objeto { forma: Forma::Malla { indice: 2 }, material: Material::Superficie(Superficie::plastico([0.72, 0.06, 0.05], 0.2)) });
    let camara = Camara { ojo: [0.0, 0.95, 3.4], mira: [0.0, 0.55, 0.0], arriba: [0.0, 1.0, 0.0], tan_medio: 0.36 };
    (objetos, luz, camara, mallas)
}

/// **LA SALA DE PIEL Y NIEBLA** (VE4b): la caja de Cornell con BRUMA (la
/// luz del techo se ve en el aire) y tres esferas que dejan entrar la luz:
/// PIEL (el rojo anda mas por dentro), JADE y MARMOL.
pub fn sala_de_piel() -> ([Objeto; 9], usize, Camara, Niebla) {
    let (c, luz, camara) = caja_de_cornell();
    let esfera = |centro: V3, radio: f32, material| Objeto { forma: Forma::Esfera { centro, radio }, material };
    let objetos = [
        c[0],
        c[1],
        c[2],
        c[3],
        c[4],
        c[5],
        esfera([-0.45, 0.32, -0.3], 0.32, Material::Piel { color: [0.86, 0.58, 0.47], radio: [0.12, 0.045, 0.025], ior: 1.4 }),
        esfera([0.45, 0.28, -0.45], 0.28, Material::Piel { color: [0.35, 0.75, 0.48], radio: [0.03, 0.12, 0.05], ior: 1.6 }),
        esfera([0.05, 0.2, 0.25], 0.2, Material::Piel { color: [0.9, 0.89, 0.86], radio: [0.09, 0.09, 0.08], ior: 1.5 }),
    ];
    let niebla = Niebla { densidad: 0.25, albedo: 0.9, g: 0.4, min: [-1.0, 0.0, -1.0], max: [1.0, 2.0, 1.0] };
    (objetos, luz, camara, niebla)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena(objetos: &[Objeto]) -> Escena<'_> {
        let (_, luz, camara) = caja_de_cornell();
        Escena { objetos, luz, camara, mallas: &[], niebla: None }
    }

    /// La misma semilla, la misma luz: el render es determinista.
    #[test]
    fn the_same_pixel_twice_is_the_same_light() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        assert_eq!(e.pixel(20, 30, 64, 48, 8), e.pixel(20, 30, 64, 48, 8));
    }

    /// ** La luz INDIRECTA: el suelo blanco junto a la pared roja sale
    /// ROJIZO (el color que sangra), y junto a la verde, VERDOSO -- y con la
    /// luz apagada, todo negro --.
    #[test]
    fn the_red_wall_bleeds_onto_the_white_floor() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        let media = |x0: u32, y0: u32| {
            let mut s = [0.0f32; 3];
            for dy in 0..4 {
                for dx in 0..4 {
                    s = mas(s, e.pixel(x0 + dx, y0 + dy, 64, 48, 64));
                }
            }
            s
        };
        // el suelo, cerca de cada pared (abajo a la izquierda y a la derecha)
        let (izq, der) = (media(6, 42), media(54, 42));
        assert!(izq[0] / izq[1] > der[0] / der[1], "izquierda {izq:?}, derecha {der:?}");
        assert!(der[1] / der[0] > izq[1] / izq[0]);
        let mut oscuro = o;
        oscuro[5].material = Material::Luz { emite: [0.0; 3] };
        let e = escena(&oscuro);
        assert_eq!(e.pixel(32, 40, 64, 48, 16), [0.0; 3]);
    }

    /// Lo que refleja en total (el albedo) visto desde `wo`, CONTADO sin
    /// azar: con la normal arriba, la direccion `(x, raiz(1-x^2-z^2), z)`
    /// por cada punto del disco unidad, y `f cos dw = f dx dz` (el angulo
    /// solido proyectado: sin seno ni coseno).
    fn albedo_contado(m: &Superficie, wo: V3, lado: usize) -> f32 {
        let n = [0.0, 1.0, 0.0];
        let paso = 2.0 / lado as f32;
        let mut s = 0.0f64;
        for i in 0..lado {
            for j in 0..lado {
                let (x, z) = (-1.0 + (i as f32 + 0.5) * paso, -1.0 + (j as f32 + 0.5) * paso);
                let y2 = 1.0 - x * x - z * z;
                if y2 > 0.0 {
                    s += luminancia(m.refleja(n, wo, [x, raiz(y2), z])) as f64;
                }
            }
        }
        (s * (paso * paso) as f64) as f32
    }

    fn materiales() -> [Superficie; 5] {
        [
            Superficie::plastico([0.7, 0.1, 0.08], 0.3),
            Superficie::plastico([0.8, 0.8, 0.8], 0.2),
            Superficie::metal([1.0, 0.78, 0.34], 0.4),
            Superficie::metal([0.95, 0.64, 0.54], 0.2),
            Superficie { base: [0.1, 0.2, 0.65], metal: 0.5, aspereza: 0.4, ior: 1.5, capa: 1.0, capa_aspereza: 0.2 },
        ]
    }

    /// Pulidas: un lobulo tan fino que lo uniforme casi nunca lo encuentra.
    fn pulidos() -> [Superficie; 3] {
        [
            Superficie::plastico([0.8, 0.8, 0.8], 0.02),
            Superficie::metal([0.95, 0.95, 0.95], 0.02),
            Superficie { base: [0.9, 0.9, 0.9], metal: 0.0, aspereza: 0.3, ior: 1.5, capa: 1.0, capa_aspereza: 0.02 },
        ]
    }

    /// *** LA SUPERFICIE se muestrea con la probabilidad que DICE: lo que
    /// refleja en total (el albedo), con sus muestras y CONTADO sin azar,
    /// sale igual -- si `probabilidad` mintiera, la imagen saldria mas clara
    /// u oscura que la luz de verdad --; y nunca devuelve mas luz de la que
    /// le llega.
    #[test]
    fn the_surface_samples_with_the_probability_it_says() {
        let n = [0.0, 1.0, 0.0];
        for (k, m) in materiales().iter().enumerate() {
            for wo in [unidad([0.0, 1.0, 0.2]), unidad([0.6, 0.5, 0.0]), unidad([0.0, 0.25, -1.0])] {
                let mut az = Azar::nuevo(7 + k as u32);
                let mut propia = [0.0f32; 3];
                let muestras = 30_000;
                for _ in 0..muestras {
                    if let Some(wi) = m.muestrea(n, wo, &mut az) {
                        let pb = m.probabilidad(n, wo, wi);
                        if pb > 0.0 {
                            propia = mas(propia, por(m.refleja(n, wo, wi), punto(n, wi) / pb));
                        }
                    }
                }
                let (a, b) = (luminancia(propia) / muestras as f32, albedo_contado(m, wo, 400));
                assert!((a - b).abs() < 0.02 * b.max(0.05), "material {k}, wo {wo:?}: con sus muestras {a}, contado {b}");
                assert!(a <= 1.02, "material {k}: devuelve {a} de cada 1");
            }
        }
        // lo pulido y blanco: ni crea luz, ni la pierde de mas
        for (k, m) in pulidos().iter().enumerate() {
            for wo in [unidad([0.0, 1.0, 0.2]), unidad([0.0, 0.25, -1.0])] {
                let mut az = Azar::nuevo(31 + k as u32);
                let mut propia = [0.0f32; 3];
                for _ in 0..100_000 {
                    if let Some(wi) = m.muestrea(n, wo, &mut az) {
                        let pb = m.probabilidad(n, wo, wi);
                        if pb > 0.0 {
                            propia = mas(propia, por(m.refleja(n, wo, wi), punto(n, wi) / pb));
                        }
                    }
                }
                let a = luminancia(propia) / 100_000.0;
                assert!(a <= 1.02 && a > 0.5, "pulido {k}, wo {wo:?}: devuelve {a} de cada 1");
            }
        }
    }

    /// Lo PULIDO no da NaN: una microfaceta justo en la normal (nh de
    /// 1,0000001 por el redondeo) dejaba la GGX en infinito, y por una capa
    /// que pesa 0, NaN -- en la foto de la sala, puntos negros por todas
    /// partes --. Con muchas vistas al azar, la probabilidad y lo que
    /// refleja salen siempre finitos.
    #[test]
    fn polished_surfaces_never_give_nan() {
        assert!(ggx_d(0.0004, 1.000_000_1).is_finite());
        let mut az = Azar::nuevo(3);
        for m in pulidos() {
            for _ in 0..100_000 {
                let n = unidad(en_esfera(&mut az));
                let wo = unidad(en_esfera(&mut az));
                if punto(n, wo) <= 0.0 {
                    continue;
                }
                if let Some(wi) = m.muestrea(n, wo, &mut az) {
                    let pb = m.probabilidad(n, wo, wi);
                    let f = m.refleja(n, wo, wi);
                    assert!(pb.is_finite() && f.iter().all(|x| x.is_finite()), "n {n:?} wo {wo:?} wi {wi:?}: pb {pb}, f {f:?}");
                }
            }
        }
    }

    /// *** MIS no tiene sesgo: la luz directa en un punto del suelo de
    /// SUPERFICIE, preguntando SOLO a la luz, SOLO rebotando, o con las dos
    /// y MIS, da lo mismo. Si los pesos no sumaran 1 (la luz contada dos
    /// veces, o ninguna), saldria distinta.
    #[test]
    fn mis_is_unbiased() {
        let (o, luz, camara) = caja_de_cornell();
        let e = Escena { objetos: &o[..6], luz, camara, mallas: &[], niebla: None };
        let (p, n) = ([0.1, 0.0, 0.3], [0.0, 1.0, 0.0]);
        let wo = unidad([0.0, 1.0, 1.2]);
        let Material::Luz { emite } = o[luz].material else { panic!() };
        let Forma::Rect { esquina, u, v } = o[luz].forma else { panic!() };
        let area = raiz(punto(cruz(u, v), cruz(u, v)));
        for m in [Superficie::plastico([0.7, 0.1, 0.08], 0.3), Superficie::metal([1.0, 0.78, 0.34], 0.15)] {
            let muestras = 400_000;
            let mut az = Azar::nuevo(99);
            let (mut solo_luz, mut solo_rebote, mut con_mis) = (0.0f32, 0.0f32, 0.0f32);
            for _ in 0..muestras {
                // solo la luz, sin MIS
                let q = mas(esquina, mas(por(u, az.uno()), por(v, az.uno())));
                let hacia = menos(q, p);
                let d2 = punto(hacia, hacia);
                let l = por(hacia, 1.0 / raiz(d2));
                let cos_l = -punto(unidad(cruz(u, v)), l);
                if punto(n, l) > 0.0 && cos_l > 0.0 {
                    solo_luz += luminancia(mul(emite, por(m.refleja(n, wo, l), punto(n, l) * cos_l * area / d2)));
                }
                // solo rebotar, y lo que da con la luz
                let rebote = |az: &mut Azar| -> (f32, f32) {
                    let Some(wi) = m.muestrea(n, wo, az) else { return (0.0, 0.0) };
                    let pb = m.probabilidad(n, wo, wi);
                    match e.golpe(p, wi, f32::MAX) {
                        Some(g) if g.k == luz && g.fuera && pb > 0.0 => {
                            let t = raiz(punto(menos(g.p, p), menos(g.p, p)));
                            let c = luminancia(mul(emite, por(m.refleja(n, wo, wi), punto(n, wi) / pb)));
                            (c, c * potencia_mis(pb, e.probabilidad_luz(wi, g.n, t)))
                        }
                        _ => (0.0, 0.0),
                    }
                };
                let (sin_peso, con_peso) = rebote(&mut az);
                solo_rebote += sin_peso;
                con_mis += con_peso + luminancia(e.directa_superficie(p, n, wo, &m, &mut az));
            }
            let (a, b, c) = (solo_luz / muestras as f32, solo_rebote / muestras as f32, con_mis / muestras as f32);
            assert!((a - c).abs() < 0.03 * a && (b - c).abs() < 0.05 * a, "solo la luz {a}, solo rebotar {b}, con MIS {c}");
        }
    }

    /// La sala de materiales: el oro sale AMARILLO (su brillo es de su
    /// color) y el plastico rojo, ROJO; y las esferas pulidas reflejan la
    /// luz del techo (un punto mucho mas claro que su media).
    #[test]
    fn the_materials_room_shows_each_layer() {
        let (o, luz, camara) = sala_de_materiales();
        let e = Escena { objetos: &o, luz, camara, mallas: &[], niebla: None };
        let (w, h) = (96, 72);
        // el centro de cada esfera en la pantalla, a ojo de la camara
        let en = |x: f32| {
            let d = unidad(menos([x, 0.17, -0.35], camara.ojo));
            // de vuelta al pixel: el que tiene el rayo de centro mas parecido
            let mut mejor = (0, 0, -1.0);
            for py in 0..h {
                for px in 0..w {
                    let (_, r) = e.rayo_centro(px, py, w, h);
                    let c = punto(r, d);
                    if c > mejor.2 {
                        mejor = (px, py, c);
                    }
                }
            }
            (mejor.0, mejor.1)
        };
        let media = |(x, y): (u32, u32)| {
            let mut s = [0.0f32; 3];
            for dy in 0..2 {
                for dx in 0..2 {
                    s = mas(s, e.pixel(x + dx, y + dy, w, h, 64));
                }
            }
            s
        };
        let (rojo, oro) = (media(en(-0.76)), media(en(-0.38)));
        assert!(rojo[0] > 3.0 * rojo[1], "rojo {rojo:?}");
        assert!(oro[0] > oro[1] && oro[1] > 1.5 * oro[2], "oro {oro:?}");
    }

    /// La sala de mallas: la esfera de triangulos se ve donde la pondria
    /// la de verdad (el pixel de su centro es ORO: amarillo), y el suelo
    /// bajo la dona esta en SOMBRA (mas oscuro que el suelo libre).
    #[test]
    fn the_meshes_room_draws_its_triangles() {
        let (o, luz, camara, mallas) = sala_de_mallas();
        let e = Escena { objetos: &o, luz, camara, mallas: &mallas, niebla: None };
        let (w, h) = (96, 72);
        let pixel_de = |p: V3| {
            let d = unidad(menos(p, camara.ojo));
            let mut mejor = (0, 0, -1.0);
            for py in 0..h {
                for px in 0..w {
                    let c = punto(e.rayo_centro(px, py, w, h).1, d);
                    if c > mejor.2 {
                        mejor = (px, py, c);
                    }
                }
            }
            (mejor.0, mejor.1)
        };
        let (x, y) = pixel_de([-0.45, 0.32, -0.45]);
        let oro = e.pixel(x, y, w, h, 64);
        assert!(oro[0] > oro[1] && oro[1] > 1.5 * oro[2], "oro {oro:?}");
        // el rayo del centro de ese pixel da en la malla 0
        let (ojo, d) = e.rayo_centro(x, y, w, h);
        let g = e.golpe(ojo, d, f32::MAX).unwrap();
        assert_eq!(o[g.k].forma, Forma::Malla { indice: 0 });
    }

    /// *** EL PASEO DEVUELVE EL COLOR QUE SE PIDE: en una losa sin fondo
    /// (lo de debajo de z = 0), la luz que entra con el coseno y pasea sale
    /// en la proporcion `color` (la inversion de van de Hulst esta hecha
    /// para eso). Saboteado el albedo de cada choque, sale otra cosa.
    #[test]
    fn the_walk_gives_back_the_color_asked() {
        let mut az = Azar::nuevo(17);
        let color = [0.8, 0.5, 0.2];
        let radio = [0.1, 0.1, 0.1];
        let sale = |p: V3, d: V3| if d[2] > 0.0 { Some((-p[2] / d[2], [0.0, 0.0, 1.0])) } else { None };
        let n = 40_000;
        let mut suma = [0.0f32; 3];
        for _ in 0..n {
            let d = coseno([0.0, 0.0, -1.0], &mut az);
            if let Some((p, _, w)) = paseo([0.0; 3], d, color, radio, &mut az, sale) {
                assert!(p[2].abs() < 1e-3, "sale por el borde: {p:?}");
                suma = mas(suma, w);
            }
        }
        for k in 0..3 {
            let r = suma[k] / n as f32;
            assert!((r - color[k]).abs() < 0.05, "color {k}: pedido {}, sale {r}", color[k]);
        }
    }

    /// *** LA NIEBLA APAGA COMO DICE BEER: con albedo 0 (solo apaga), la luz
    /// del techo vista desde dentro de la caja llega por `e^(-densidad d)`,
    /// d lo que el rayo anda DENTRO de la caja.
    #[test]
    fn the_fog_dims_the_light_as_beer_says() {
        let (o, luz, camara) = caja_de_cornell();
        let niebla = Niebla { densidad: 0.8, albedo: 0.0, g: 0.0, min: [-1.0, 0.0, -1.0], max: [1.0, 2.0, 1.0] };
        let solo_luz = [o[luz]];
        let (desde, hasta) = ([0.0, 0.5, 0.0], [0.0, 1.998, 0.0]);
        let d = unidad(menos(hasta, desde));
        let media = |e: &Escena| {
            let mut az = Azar::nuevo(3);
            let n = 20_000;
            (0..n).map(|_| e.camino(desde, d, &mut az)[0]).sum::<f32>() / n as f32
        };
        let limpia = media(&Escena { objetos: &solo_luz, luz: 0, camara, mallas: &[], niebla: None });
        let con = media(&Escena { objetos: &solo_luz, luz: 0, camara, mallas: &[], niebla: Some(niebla) });
        let beer = exp(-0.8 * 1.498);
        assert!((con / limpia - beer).abs() < 0.02, "con niebla {con}, sin {limpia}: {} y Beer dice {beer}", con / limpia);
    }

    /// LA NIEBLA QUE DESVIA da luz donde no la habia: un rayo desde dentro
    /// de la caja hacia su boca abierta (la noche, negra) no trae NADA sin
    /// bruma, y con bruma trae la luz que el aire desvio hacia el; y la sala
    /// de piel y niebla no da NaN.
    #[test]
    fn the_fog_that_scatters_lights_the_air() {
        let (o, luz, camara, niebla) = sala_de_piel();
        let (desde, d) = ([0.0, 1.0, 0.5], [0.0, 0.0, 1.0]);
        let media = |e: &Escena| {
            let mut az = Azar::nuevo(9);
            let n = 4000;
            (0..n).map(|_| luminancia(e.camino(desde, d, &mut az))).sum::<f32>() / n as f32
        };
        let sin = media(&Escena { objetos: &o, luz, camara, mallas: &[], niebla: None });
        let con = media(&Escena { objetos: &o, luz, camara, mallas: &[], niebla: Some(niebla) });
        assert_eq!(sin, 0.0, "sin bruma, la noche es negra");
        assert!(con > 0.01, "con bruma, el aire trae luz: {con}");
        let e = Escena { objetos: &o, luz, camara, mallas: &[], niebla: Some(niebla) };
        let (w, h) = (48, 36);
        for y in (0..h).step_by(5) {
            for x in (0..w).step_by(5) {
                assert!(e.pixel(x, y, w, h, 4).iter().all(|v| v.is_finite()), "({x}, {y})");
            }
        }
    }

    /// LA PIEL deja pasar la luz: la esfera de piel, en su lado de SOMBRA
    /// (abajo, contra el suelo), sale mas clara que la misma esfera mate; y
    /// del color que es (rojiza).
    #[test]
    fn the_skin_lets_the_light_through() {
        let (o, luz, camara, _) = sala_de_piel();
        let mut mate = o;
        mate[6].material = Material::Mate { color: [0.86, 0.58, 0.47] };
        let (w, h) = (64, 48);
        // abajo de la esfera de piel, en la pantalla
        let zona = |e: &Escena| {
            let mut s = [0.0f32; 3];
            for y in 33..36 {
                for x in 17..21 {
                    s = mas(s, e.pixel(x, y, w, h, 64));
                }
            }
            s
        };
        let piel = zona(&Escena { objetos: &o, luz, camara, mallas: &[], niebla: None });
        let solo = zona(&Escena { objetos: &mate, luz, camara, mallas: &[], niebla: None });
        assert!(luminancia(piel) > luminancia(solo), "piel {piel:?}, mate {solo:?}");
        assert!(piel[0] > piel[2], "rojiza: {piel:?}");
    }

    /// La curva de la pelicula: el negro es negro, la luz de sobra no pasa
    /// de blanco, y sube sin bajar.
    #[test]
    fn the_film_curve_is_monotone_and_bounded() {
        assert_eq!(pelicula([0.0; 3], 1.0), 0);
        assert_eq!(pelicula([1e6; 3], 1.0), 0xFF_FFFF);
        let mut antes = 0;
        for k in 0..200 {
            let g = pelicula([k as f32 * 0.05; 3], 1.0) & 0xFF;
            assert!(g >= antes);
            antes = g;
        }
    }
}
