//! **EL FOTOGRAMA: mirar, aprender, decidir.**
//!
//! ```text
//!    fotograma N dibujado con el mapa M(N)
//!       mirar(N)     las baldosas que fueron X1 son MAESTRAS: su leccion
//!                    (los rasgos que se usaron para decidir -> el error que
//!                    de verdad habria dado cada tasa); las demas, solo lo
//!                    que CAMBIO (la 4D)
//!       decidir      el mapa M(N+1), con las reglas
//!    fotograma N+1 dibujado con M(N+1)
//! ```
//!
//! **Las reglas que UMA no puede romper**, en este orden:
//!
//! ```text
//!    1 nunca vista            X1 (no hay rasgos)
//!    2 su turno de REFRESCO   X1: ninguna baldosa se queda gruesa mas de
//!                             `refresco` fotogramas. Rompe el lazo "se ve
//!                             lisa porque se dibujo gruesa" y es MAESTRA
//!    3 sin cambios            Cero, si quien dibuja lo garantiza (A2)
//!    4 interfaz               X1: el texto y el HUD, siempre enteros
//!    5 aprendiz               X1 hasta `aprendiz` lecciones del juego
//!    6 UMA                    la tasa mas gruesa cuyo error predicho cabe
//!                             en el umbral (con la 5D, si hay confianza)
//!    7 borde de geometria     como mucho 2 a LO LARGO del borde, nunca a
//!                             traves, nunca 2x2 ni 4x4
//! ```
//!
//! **El umbral se aprieta solo**: cada maestra dice tambien que habria
//! querido UMA y cuanto error habria dado. Si esa media se pasa del umbral,
//! la `exigencia` baja (mas detalle) hasta que vuelve a caber; si sobra,
//! sube despacio hasta el umbral del propietario.

use alloc::vec;
use alloc::vec::Vec;

use crate::maestra::{self, GRUESAS};
use crate::mano::{Prediccion, CONFIANZA_MIN};
use crate::pacha::{self, Extra, Imagen, Rasgos, CAMBIO, LADO, MOVIMIENTO, SALTO_Z, TRIANGULOS};
use crate::uma::Uma;
use crate::Tasa;

/// Lo que elige el propietario.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// El error que acepta (Weber, ver `maestra`).
    pub umbral: f32,
    /// La regla 4. Solo se apaga para la prueba del NO.
    pub regla_interfaz: bool,
    /// Fotogramas que una baldosa puede ir gruesa sin volver a X1.
    pub refresco: u32,
    /// Lecciones por fotograma como mucho (la CPU no se pasa).
    pub maestras_max: usize,
    /// Lecciones del juego antes de que UMA decida.
    pub aprendiz: u64,
}

impl Config {
    /// `ayni suave`: la diferencia casi no se ve.
    pub const SUAVE: Config = Config { umbral: 0.03, regla_interfaz: true, refresco: 30, maestras_max: 64, aprendiz: 64 };
    /// `ayni fuerte`: mas ahorro, se ve algo en lo liso.
    pub const FUERTE: Config = Config { umbral: 0.08, ..Config::SUAVE };
}

/// Lo que se cuenta de cada fotograma (para el DIARIO y FRAPS-X).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cuentas {
    /// Invocaciones del mapa decidido, y las de todo a X1.
    pub invocaciones: u64,
    pub llenas: u64,
    /// Lecciones de este fotograma.
    pub lecciones: u32,
    /// Error medio que habria dado lo que UMA queria en las maestras.
    pub error_maestras: f32,
    /// El factor del umbral (1 = el del propietario).
    pub exigencia: f32,
}

#[derive(Clone, Copy, Default)]
struct Memoria {
    vista: bool,
    rasgos: Rasgos,
    /// Fotogramas desde la ultima vez que fue X1.
    edad: u32,
    extra: Extra,
    /// Lo que UMA habria elegido (para medir las maestras).
    quiso: Option<Tasa>,
}

pub struct Ayni {
    pub uma: Uma,
    pub cfg: Config,
    columnas: usize,
    filas: usize,
    memoria: Vec<Memoria>,
    mapa: Vec<Tasa>,
    /// Por donde empiezan las lecciones de este fotograma (rotan).
    turno: usize,
    exigencia: f32,
    pub cuentas: Cuentas,
}

/// Lo mas que la exigencia aprieta (un cuarto del umbral).
const EXIGENCIA_MIN: f32 = 0.25;

impl Ayni {
    pub fn nuevo(ancho: usize, alto: usize, cfg: Config) -> Self {
        Ayni::con_uma(ancho, alto, cfg, Uma::nacer())
    }

    /// Con una UMA ya hecha (el banco nace una vez y la clona).
    pub fn con_uma(ancho: usize, alto: usize, cfg: Config, uma: Uma) -> Self {
        let (columnas, filas) = (ancho.div_ceil(LADO), alto.div_ceil(LADO));
        let n = columnas * filas;
        Ayni { uma, cfg, columnas, filas, memoria: vec![Memoria::default(); n], mapa: vec![Tasa::X1; n], turno: 0, exigencia: 1.0, cuentas: Cuentas::default() }
    }

    pub fn columnas(&self) -> usize {
        self.columnas
    }

    pub fn filas(&self) -> usize {
        self.filas
    }

    /// El mapa del fotograma que se va a dibujar.
    pub fn mapa(&self) -> &[Tasa] {
        &self.mapa
    }

    /// **Mirar el fotograma dibujado** con el mapa de ahora. `antes`: el
    /// fotograma anterior (para lo que cambio). `extras`: uno por baldosa, o
    /// ninguno.
    pub fn mirar(&mut self, ahora: &Imagen, antes: Option<&Imagen>, extras: &[Extra]) {
        let n = self.memoria.len();
        let mut lecciones = 0u32;
        let (mut suma_error, mut medidas) = (0.0f32, 0u32);
        for k in 0..n {
            // Las lecciones empiezan en `turno` y dan la vuelta: con el
            // tope, ninguna baldosa se queda sin mostrar.
            let i = (self.turno + k) % n;
            let (bx, by) = (i % self.columnas, i / self.columnas);
            let b = ahora.baldosa(bx, by);
            let previa = antes.map(|a| a.baldosa(bx, by));
            let extra = extras.get(i).copied().unwrap_or_default();
            let m = &mut self.memoria[i];
            if self.mapa[i] == Tasa::X1 {
                if m.vista && (lecciones as usize) < self.cfg.maestras_max {
                    self.uma.aprender(&m.rasgos.entrada, &maestra::errores(&b));
                    lecciones += 1;
                    if let Some(t) = m.quiso {
                        suma_error += maestra::error_de(&b, t);
                        medidas += 1;
                    }
                }
                m.rasgos = pacha::rasgos(&b, previa.as_ref(), &extra);
                m.vista = true;
                m.edad = 0;
            } else {
                // Gruesa (o Cero): sus rasgos de pixel son de la ultima vez
                // que fue entera. Lo que cambio y lo de quien dibuja, de ahora.
                if let Some(p) = previa.as_ref() {
                    m.rasgos.entrada[CAMBIO] = pacha::cambio(&b, p).clamp(0.0, 1.0);
                }
                m.rasgos.entrada[SALTO_Z] = extra.salto_z.clamp(0.0, 1.0);
                m.rasgos.entrada[MOVIMIENTO] = extra.movimiento.clamp(0.0, 1.0);
                m.rasgos.entrada[TRIANGULOS] = extra.triangulos.clamp(0.0, 1.0);
                m.edad = m.edad.saturating_add(1);
            }
            m.extra = extra;
        }
        self.turno = (self.turno + lecciones as usize) % n.max(1);
        let error_maestras = if medidas > 0 { suma_error / medidas as f32 } else { 0.0 };
        if medidas > 0 {
            if error_maestras > self.cfg.umbral {
                self.exigencia = (self.exigencia * 0.9).max(EXIGENCIA_MIN);
            } else {
                self.exigencia = (self.exigencia * 1.02).min(1.0);
            }
        }
        self.cuentas.lecciones = lecciones;
        self.cuentas.error_maestras = error_maestras;
        self.cuentas.exigencia = self.exigencia;
    }

    /// **El mapa del siguiente fotograma.** `mano`: la 5D, si la hay.
    pub fn decidir(&mut self, mano: Option<&Prediccion>) -> &[Tasa] {
        let diagonal = raiz_de((self.columnas * self.columnas + self.filas * self.filas) as f32) * LADO as f32;
        let mut invocaciones = 0u64;
        for i in 0..self.memoria.len() {
            let m = self.memoria[i];
            let (cx, cy) = (((i % self.columnas) * LADO + LADO / 2) as f32, ((i / self.columnas) * LADO + LADO / 2) as f32);
            let quiso = if m.vista { Some(self.de_uma(&m, relajo(mano, cx, cy, diagonal))) } else { None };
            let t = if !m.vista || m.edad >= self.cfg.refresco {
                Tasa::X1
            } else if m.extra.sin_cambios {
                Tasa::Cero
            } else if m.extra.interfaz && self.cfg.regla_interfaz {
                Tasa::X1
            } else if self.uma.lecciones < self.cfg.aprendiz {
                Tasa::X1
            } else {
                quiso.unwrap_or(Tasa::X1)
            };
            self.memoria[i].quiso = quiso;
            self.mapa[i] = t;
            invocaciones += t.invocaciones() as u64;
        }
        self.cuentas.invocaciones = invocaciones;
        self.cuentas.llenas = self.mapa.len() as u64 * Tasa::X1.invocaciones() as u64;
        &self.mapa
    }

    /// Reglas 6 y 7: lo que UMA elige, con el borde de geometria encima.
    fn de_uma(&self, m: &Memoria, relajo: f32) -> Tasa {
        let umbral = self.cfg.umbral * self.exigencia * relajo;
        let errores = self.uma.predecir(&m.rasgos.entrada);
        let mut mejor = Tasa::X1;
        let mut mejor_error = 0.0;
        for (t, e) in GRUESAS.iter().zip(errores) {
            if e <= umbral {
                let menos = t.invocaciones() < mejor.invocaciones();
                let igual_y_mejor = t.invocaciones() == mejor.invocaciones() && e < mejor_error;
                if menos || igual_y_mejor {
                    mejor = *t;
                    mejor_error = e;
                }
            }
        }
        if m.extra.borde_geometria {
            // A lo largo del borde: si la luz cambia a lo ancho, el borde es
            // vertical y solo se engruesa a lo alto.
            let a_lo_largo = if m.rasgos.cambia_a_lo_ancho { Tasa::X1x2 } else { Tasa::X2x1 };
            mejor = match mejor {
                Tasa::X1 => Tasa::X1,
                t if t == a_lo_largo => t,
                Tasa::X2x2 | Tasa::X4x4 => a_lo_largo,
                _ => Tasa::X1,
            };
        }
        mejor
    }
}

/// **La 5D en el umbral**: con confianza, mas detalle donde va la mano (la
/// mitad del umbral) y, si la mano corre, menos lejos de ella. Sin
/// confianza, nada cambia.
fn relajo(mano: Option<&Prediccion>, cx: f32, cy: f32, diagonal: f32) -> f32 {
    match mano {
        Some(p) if p.confianza >= CONFIANZA_MIN => {
            let (dx, dy) = (cx - p.x, cy - p.y);
            let d = raiz_de(dx * dx + dy * dy) / diagonal;
            if d < FOCO {
                0.5
            } else {
                1.0 + 2.0 * p.rapidez
            }
        }
        _ => 1.0,
    }
}

/// El foco de la mano: una decima de la diagonal.
const FOCO: f32 = 0.1;

fn raiz_de(v: f32) -> f32 {
    crate::mano::raiz(v)
}
