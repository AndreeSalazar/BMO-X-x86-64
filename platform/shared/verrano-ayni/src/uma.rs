//! **UMA, la cabeza: una red diminuta, en la CPU.**
//!
//! ```text
//!    8 rasgos (PACHA) -> 16 neuronas (ReLU) -> 4 errores predichos
//!                                              (X2x1, X1x2, X2x2, X4x4)
//! ```
//!
//! 8x16 + 16x4 = 192 multiplicaciones por baldosa: 1920x1080 son 8160
//! baldosas, 1,6 millones por fotograma. Cero en la GPU.
//!
//! **No decide la tasa: predice el ERROR** de cada una. La decision
//! (`ayni`) toma la mas gruesa cuyo error predicho cabe en el umbral del
//! propietario. Asi la red aprende algo que se puede MEDIR (la leccion de
//! las maestras) y el umbral se cambia sin reentrenar.
//!
//! Sin `libm`: ReLU y sumas, nada de `exp` ni `sqrt`. Los pesos iniciales
//! salen de una semilla fija: la misma red en cada arranque, bit a bit.

use crate::pacha::ENTRADAS;

pub const OCULTAS: usize = 16;
pub const SALIDAS: usize = 4;
/// El error mas alto que se ensena: por encima, toda tasa gruesa sobra.
pub const TOPE: f32 = 1.0;
/// La semilla de los pesos.
pub const SEMILLA: u32 = 0x4159_4E49; // "AYNI"
/// El paso al aprender en vivo (pequeno: un fotograma raro no la tuerce).
pub const PASO_EN_VIVO: f32 = 0.01;

#[derive(Clone)]
pub struct Uma {
    w1: [[f32; ENTRADAS]; OCULTAS],
    b1: [f32; OCULTAS],
    w2: [[f32; OCULTAS]; SALIDAS],
    b2: [f32; SALIDAS],
    /// Lecciones del juego (las de `nacer` no cuentan).
    pub lecciones: u64,
    pub paso: f32,
}

/// Un generador de numeros de semilla fija (LCG de Numerical Recipes).
pub struct Dado(pub u32);

impl Dado {
    pub fn siguiente(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }

    /// En `[0, 1)`.
    pub fn real(&mut self) -> f32 {
        (self.siguiente() >> 8) as f32 / (1u32 << 24) as f32
    }
}

impl Uma {
    /// La red sin ensenar, con pesos de la semilla.
    pub fn en_blanco(semilla: u32) -> Self {
        let mut d = Dado(semilla);
        let mut u = Uma { w1: [[0.0; ENTRADAS]; OCULTAS], b1: [0.0; OCULTAS], w2: [[0.0; OCULTAS]; SALIDAS], b2: [0.0; SALIDAS], lecciones: 0, paso: PASO_EN_VIVO };
        // ~ 1/raiz(entradas): 8 -> 0,35; 16 -> 0,25.
        for fila in u.w1.iter_mut() {
            for w in fila.iter_mut() {
                *w = (d.real() * 2.0 - 1.0) * 0.35;
            }
        }
        for b in u.b1.iter_mut() {
            *b = 0.05;
        }
        for fila in u.w2.iter_mut() {
            for w in fila.iter_mut() {
                *w = (d.real() * 2.0 - 1.0) * 0.25;
            }
        }
        u
    }

    /// **UMA al nacer**: la red de la semilla, ensenada con baldosas de
    /// muestra (cielo, degradado, borde, textura, rayas, sombra). Sabe lo
    /// basico antes del primer fotograma; el juego la afina.
    pub fn nacer() -> Self {
        let mut u = Uma::en_blanco(SEMILLA);
        crate::semilla::ensenar(&mut u, crate::semilla::PASOS_DE_NACER);
        u.lecciones = 0;
        u.paso = PASO_EN_VIVO;
        u
    }

    fn ocultas(&self, x: &[f32; ENTRADAS]) -> [f32; OCULTAS] {
        let mut h = self.b1;
        for (j, hj) in h.iter_mut().enumerate() {
            for (k, xk) in x.iter().enumerate() {
                *hj += self.w1[j][k] * xk;
            }
            *hj = hj.max(0.0);
        }
        h
    }

    fn salida(&self, h: &[f32; OCULTAS]) -> [f32; SALIDAS] {
        let mut y = self.b2;
        for (i, yi) in y.iter_mut().enumerate() {
            for (j, hj) in h.iter().enumerate() {
                *yi += self.w2[i][j] * hj;
            }
        }
        y
    }

    /// **El error predicho** de cada tasa gruesa (`maestra::GRUESAS`).
    pub fn predecir(&self, x: &[f32; ENTRADAS]) -> [f32; SALIDAS] {
        self.salida(&self.ocultas(x)).map(|v| v.max(0.0))
    }

    /// **Una leccion**: un paso de descenso por el gradiente del error
    /// cuadratico. Devuelve el error cuadratico ANTES del paso.
    pub fn aprender(&mut self, x: &[f32; ENTRADAS], objetivo: &[f32; SALIDAS]) -> f32 {
        let h = self.ocultas(x);
        let y = self.salida(&h);
        let mut g = [0.0f32; SALIDAS];
        let mut perdida = 0.0;
        for i in 0..SALIDAS {
            let d = y[i] - objetivo[i].clamp(0.0, TOPE);
            g[i] = d;
            perdida += d * d;
        }
        // La capa oculta, con los pesos de ANTES del paso.
        let mut gh = [0.0f32; OCULTAS];
        for (j, ghj) in gh.iter_mut().enumerate() {
            if h[j] > 0.0 {
                for (i, gi) in g.iter().enumerate() {
                    *ghj += gi * self.w2[i][j];
                }
            }
        }
        let p = self.paso;
        for i in 0..SALIDAS {
            for j in 0..OCULTAS {
                self.w2[i][j] -= p * g[i] * h[j];
            }
            self.b2[i] -= p * g[i];
        }
        for j in 0..OCULTAS {
            for k in 0..ENTRADAS {
                self.w1[j][k] -= p * gh[j] * x[k];
            }
            self.b1[j] -= p * gh[j];
        }
        self.lecciones += 1;
        perdida
    }
}
