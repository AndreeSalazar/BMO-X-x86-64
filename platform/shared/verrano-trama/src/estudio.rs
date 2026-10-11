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
//! [cuesta]  CPU       rayos * rebotes; un pixel no mira a otro: se reparte
//!                     entre todos los nucleos por filas (`pixel`)
//! [riesgo]  NINGUNO   determinista: la misma semilla, la misma imagen
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
fn menos(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn por(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn mul(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}
fn punto(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cruz(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unidad(a: V3) -> V3 {
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
}

/// **Lo que hay en la escena.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Forma {
    Esfera { centro: V3, radio: f32 },
    /// Un rectangulo: una esquina y sus dos lados (su normal, `u x v`).
    Rect { esquina: V3, u: V3, v: V3 },
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
/// `Material::Luz`) y la camara.
#[derive(Clone, Copy, Debug)]
pub struct Escena<'a> {
    pub objetos: &'a [Objeto],
    pub luz: usize,
    pub camara: Camara,
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
        por(emite, cos_p * cos_l * area / (d2 * core::f32::consts::PI))
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
        for rebote in 0..12 {
            let Some(g) = self.golpe(o, d, f32::MAX) else { break };
            match self.objetos[g.k].material {
                Material::Luz { emite } => {
                    // la luz se ve directa solo desde la camara o un espejo:
                    // desde lo mate ya se conto (`directa`)
                    if especular && g.fuera {
                        let mut c = mul(peso, emite);
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
                }
                Material::Metal { color, aspereza } => {
                    let r = menos(d, por(g.n, 2.0 * punto(d, g.n)));
                    d = unidad(mas(r, por(en_esfera(az), aspereza)));
                    if punto(d, g.n) <= 0.0 {
                        break;
                    }
                    peso = mul(peso, color);
                    especular = true;
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
                }
            }
            o = g.p;
            // la ruleta: despues de 3 rebotes, sin sesgo
            if rebote >= 3 {
                let q = peso[0].max(peso[1]).max(peso[2]).min(0.95);
                if az.uno() > q {
                    break;
                }
                peso = por(peso, 1.0 / q);
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn escena(objetos: &[Objeto]) -> Escena<'_> {
        let (_, luz, camara) = caja_de_cornell();
        Escena { objetos, luz, camara }
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
