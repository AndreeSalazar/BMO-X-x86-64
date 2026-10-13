//! **LOS MATERIALES DE ESTUDIO** (13-10, pieza propia): como devuelve la luz
//! cada superficie -- Lambert, el metal y el plastico de GGX, el vidrio, la
//! piel. Salieron de `estudio.rs` enteros: con ellos pasaba de las 1.000
//! lineas de codigo (L6a). El trazador (`estudio.rs`) pregunta; esto contesta.

use bmo_prometeo::programa::raiz;

use crate::estudio::*;

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
pub(crate) fn schlick(f0: f32, coseno: f32) -> f32 {
    let x = (1.0 - coseno).clamp(0.0, 1.0);
    let x2 = x * x;
    f0 + (1.0 - f0) * x2 * x2 * x
}

/// La distribucion GGX (Trowbridge-Reitz) de las microfacetas, con `a` =
/// aspereza^2 (la convencion de Disney, la que siguen todos).
pub(crate) fn ggx_d(a: f32, nh: f32) -> f32 {
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

pub(crate) fn luminancia(v: V3) -> f32 {
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
pub(crate) fn base_de(n: V3) -> (V3, V3) {
    let t = unidad(if n[0].abs() > 0.5 { cruz(n, [0.0, 1.0, 0.0]) } else { cruz(n, [1.0, 0.0, 0.0]) });
    (t, cruz(n, t))
}

/// **MIS** (Veach, 1997; lo usan Arnold, RenderMan y Cycles): la misma luz
/// se puede encontrar de DOS maneras -- preguntando a la luz o rebotando y
/// dando con ella --; cada una es buena donde la otra es mala (la luz
/// grande en lo aspero; la luz chica en lo pulido). Se usan las dos, cada
/// una con su peso (el de la potencia: p^2 / (p^2 + q^2)), y sin sesgo.
pub(crate) fn potencia_mis(p: f32, q: f32) -> f32 {
    let (p2, q2) = (p * p, q * q);
    if p2 + q2 <= 0.0 {
        0.0
    } else {
        p2 / (p2 + q2)
    }
}

