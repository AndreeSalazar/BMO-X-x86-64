//! **LA SINTESIS: una nota, muestra a muestra, en enteros.**
//!
//! Lo que en la maqueta hacia WebAudio (`OscillatorNode`, `BiquadFilterNode`,
//! `exponentialRampToValueAtTime`), escrito aqui para que suene IGUAL en el
//! Ryzen y sea el mismo byte en todas las maquinas.
//!
//! # Lo que la hace sonar a estudio y no a juguete
//!
//! ```text
//!    OSCILADORES SIN ALIASING   la cuadrada y la sierra ingenuas tienen un
//!                               escalon por periodo, y un escalon tiene
//!                               armonicos hasta el infinito: los que pasan de
//!                               24 kHz vuelven DOBLADOS como un silbido que no
//!                               es ninguna nota. PolyBLEP redondea el escalon
//!                               en las dos muestras que lo rodean y el
//!                               silbido se va. Es lo que hacen los sintes
//!                               virtuales desde 2007 (Valimaki)
//!    ENVOLVENTES EN dB          una rampa exponencial ES una recta en dB: se
//!                               lleva en dB, se pasa a factor cada 16
//!                               muestras con la tabla del amplificador, y se
//!                               interpola dentro: sin escalones de cremallera
//!    FILTROS DE VERDAD          la caja y el plato son ruido por un biquad
//!                               (paso banda a 1,7 kHz y paso alto a 7,5
//!                               kHz), con los coeficientes de la norma de
//!                               WebAudio, en Q28
//! ```

use bmo_amplificador::{Ganancia, MilesimasDb, DB};

use crate::tablas::{INC, SENO};

/// La frecuencia a la que se compone: la del tubo del auricular.
pub const HZ: u32 = 48_000;

/// El "casi nada" de las rampas de WebAudio (0,0001): -80 dB.
pub(crate) const CASI_NADA: MilesimasDb = -80 * DB;

/// Cada cuantas muestras se recalcula el factor de la envolvente.
const TROZO: u32 = 16;

/// **El color de una nota.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timbre {
    Seno,
    Cuadrada,
    Sierra,
    Triangulo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Forma {
    Tono(Timbre),
    /// Un seno que cae de 150 a 42 Hz en 140 ms.
    Bombo,
    /// Ruido por un paso banda.
    Caja,
    /// Ruido por un paso alto.
    Plato,
    /// **El cencerro del 808** (`neko.rs`): dos cuadradas en razon 1,48.
    Cencerro,
    /// **El 808**: un seno que cae a su nota y se SATURA.
    Ochocientos,
    /// **Un maullido**: una sierra por dos resonancias que se mueven.
    Maullido(crate::neko::Maullido),
    /// El ronroneo: ruido grave que late a 26 Hz.
    Ronroneo,
    /// El bufido: aire a 4 kHz. El gato enfadado.
    Bufido,
}

/// **Una envolvente**: recta en dB de `a` a `b` hasta `t1`, de `b` a `c`
/// hasta `t2`, y `c` despues.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Envolvente {
    t1: u32,
    t2: u32,
    a: MilesimasDb,
    b: MilesimasDb,
    c: MilesimasDb,
}

impl Envolvente {
    fn db_en(&self, t: u32) -> MilesimasDb {
        let Envolvente { t1, t2, a, b, c } = *self;
        if t <= t1 {
            a + ((b - a) as i64 * t as i64 / t1.max(1) as i64) as i32
        } else if t <= t2 {
            b + ((c - b) as i64 * (t - t1) as i64 / (t2 - t1).max(1) as i64) as i32
        } else {
            c
        }
    }
}

/// Factor lineal de unos dB, en Q16.16, con la tabla del amplificador.
fn factor(db: MilesimasDb) -> i32 {
    Ganancia::db(db).factor_q16() as i32
}

// Los biquads de la caja y el plato, Q28: b0, b1, b2, a1, a2. Salen de la
// norma de WebAudio a 48 kHz (`BiquadFilterNode`, Q = 1).
const PASO_BANDA_1700: [i64; 5] = [26_677_662, 0, -26_677_662, -471_593_215, 215_080_131];
const PASO_ALTO_7500: [i64; 5] = [152_339_614, -304_679_228, 152_339_614, -217_631_716, 123_291_283];

/// El bombo cae de 150 a 42 Hz en 140 ms (6.720 muestras): la fase avanza
/// multiplicada por esto en cada muestra (Q30).
const BOMBO_CAIDA_Q30: u64 = 1_073_538_445;
const BOMBO_INC_150: u64 = 13_421_773;
const BOMBO_INC_42: u64 = 3_758_096;
const BOMBO_CAE: u32 = 6_720;

/// **Una nota que suena** (o que esta por sonar).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Nota {
    /// En que muestra empieza, en el reloj de quien la toca.
    pub inicio: i64,
    /// Cuantas muestras dura.
    pub largo: u32,
    forma: Forma,
    env: Envolvente,
    /// La muestra de la nota por la que va.
    pub t: u32,
    pub(crate) fase: u32,
    /// El paso de fase, en Q16 sobre el `u32`: el bombo lo va cambiando.
    pub(crate) inc: u64,
    /// El factor de la envolvente ahora, y lo que sube por muestra.
    g: i32,
    dg: i32,
    pub(crate) ruido: u32,
    pub(crate) x: [i32; 2],
    pub(crate) y: [i32; 2],
    /// Lo que piden las voces del gato (`neko.rs`): un segundo oscilador,
    /// un segundo filtro, y la transposicion (Q16, 65536 = tal cual).
    pub(crate) fase2: u32,
    pub(crate) x2: [i32; 2],
    pub(crate) y2: [i32; 2],
    pub(crate) k1: [i64; 5],
    pub(crate) k2: [i64; 5],
    pub(crate) mueve: u32,
    pub(crate) bajo: i64,
    pub(crate) bajo2: i64,
    /// A que bus va: los golpes, o lo que se BOMBEA con el bombo (S4h).
    pub(crate) bombea: bool,
}

impl Nota {
    pub const NADA: Nota = Nota {
        inicio: 0,
        largo: 0,
        forma: Forma::Tono(Timbre::Seno),
        env: Envolvente { t1: 0, t2: 0, a: CASI_NADA, b: CASI_NADA, c: CASI_NADA },
        t: 0,
        fase: 0,
        inc: 0,
        g: 0,
        dg: 0,
        ruido: 0,
        x: [0, 0],
        y: [0, 0],
        fase2: 0,
        x2: [0, 0],
        y2: [0, 0],
        k1: [0; 5],
        k2: [0; 5],
        mueve: 65_536,
        bajo: 0,
        bajo2: 0,
        bombea: false,
    };

    /// Sigue sonando?
    pub fn viva(&self) -> bool {
        self.t < self.largo
    }

    /// **Un tono**: la `tono()` de la maqueta. Sube de casi nada a `vol` en 12
    /// ms y cae a casi nada al final de `largo`; `extra` es la ganancia de su
    /// parte en la mezcla.
    pub fn tono(inicio: i64, inc: u32, largo: u32, timbre: Timbre, vol: MilesimasDb, extra: MilesimasDb) -> Nota {
        let ataque = HZ * 12 / 1000;
        Nota {
            inicio,
            largo,
            forma: Forma::Tono(timbre),
            env: Envolvente { t1: ataque, t2: largo.max(ataque + 1), a: CASI_NADA + extra, b: vol + extra, c: CASI_NADA + extra },
            inc: (inc as u64) << 16,
            ..Nota::NADA
        }
    }

    /// **Un golpe**: la `golpe()` de la maqueta.
    pub fn golpe(inicio: i64, forma: Forma, extra: MilesimasDb) -> Nota {
        // (volumen de salida, ms hasta 0,001, ms que suena)
        let (vol, cae_ms, dura_ms) = match forma {
            Forma::Bombo => (-361, 260, 300),
            Forma::Caja => (-2152, 160, 200),
            _ => (-4715, 50, 200),
        };
        let cae = HZ * cae_ms / 1000;
        Nota {
            inicio,
            largo: HZ * dura_ms / 1000,
            forma,
            env: Envolvente { t1: 1, t2: cae, a: vol + extra, b: vol + extra, c: -60 * DB + extra },
            inc: BOMBO_INC_150 << 16,
            ruido: 0x9E37_79B9,
            ..Nota::NADA
        }
    }

    /// La siguiente muestra, en Q15 (32768 = pleno). Una nota que no suena da 0.
    pub fn muestra(&mut self) -> i32 {
        if !self.viva() {
            return 0;
        }
        if self.t % TROZO == 0 {
            let g0 = factor(self.env.db_en(self.t));
            let g1 = factor(self.env.db_en(self.t + TROZO));
            self.g = g0;
            self.dg = (g1 - g0) / TROZO as i32;
        }
        let o = match self.forma {
            Forma::Tono(timbre) => oscilar(timbre, self.fase, (self.inc >> 16) as u32),
            Forma::Bombo => {
                let o = seno(self.fase);
                self.inc = if self.t < BOMBO_CAE { ((self.inc as u128 * BOMBO_CAIDA_Q30 as u128) >> 30) as u64 } else { BOMBO_INC_42 << 16 };
                o
            }
            Forma::Caja => self.filtrar(PASO_BANDA_1700),
            Forma::Plato => self.filtrar(PASO_ALTO_7500),
            Forma::Cencerro => self.cencerro_muestra(),
            Forma::Ochocientos => self.ochocientos_muestra(),
            Forma::Maullido(m) => self.maullido_muestra(m),
            Forma::Ronroneo => self.ronroneo_muestra(),
            Forma::Bufido => self.bufido_muestra(),
        };
        self.fase = self.fase.wrapping_add((self.inc >> 16) as u32);
        let s = ((o as i64 * self.g as i64) >> 16) as i32;
        self.g += self.dg;
        self.t += 1;
        s
    }

    /// Ruido blanco (xorshift32, el mismo en cada golpe: la maqueta reutiliza
    /// un solo bufer de ruido) por un biquad en forma directa I.
    fn filtrar(&mut self, k: [i64; 5]) -> i32 {
        let x0 = self.blanco();
        let y0 = (k[0] * x0 as i64 + k[1] * self.x[0] as i64 + k[2] * self.x[1] as i64
            - k[3] * self.y[0] as i64
            - k[4] * self.y[1] as i64)
            >> 28;
        let y0 = y0.clamp(i32::MIN as i64 / 4, i32::MAX as i64 / 4) as i32;
        self.x = [x0, self.x[0]];
        self.y = [y0, self.y[0]];
        y0
    }

    /// Ruido blanco en Q15 (xorshift32).
    pub(crate) fn blanco(&mut self) -> i32 {
        let mut r = self.ruido;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        self.ruido = r;
        (r as i32) >> 16
    }

    /// Una nota de cualquier forma con su envolvente de tres tramos en dB:
    /// `a` al empezar, `b` a los `t1`, `c` a los `t2` (muestras).
    pub(crate) fn con_envolvente(inicio: i64, largo: u32, forma: Forma, t1: u32, t2: u32, a: MilesimasDb, b: MilesimasDb, c: MilesimasDb) -> Nota {
        Nota { inicio, largo, forma, env: Envolvente { t1, t2, a, b, c }, ruido: 0x2545_F491, ..Nota::NADA }
    }
}

/// Un biquad en forma directa I sobre un estado, para quien lleve dos.
pub(crate) fn biquad(k: &[i64; 5], x: &mut [i32; 2], y: &mut [i32; 2], x0: i32) -> i32 {
    let y0 = (k[0] * x0 as i64 + k[1] * x[0] as i64 + k[2] * x[1] as i64 - k[3] * y[0] as i64 - k[4] * y[1] as i64) >> 28;
    let y0 = y0.clamp(i32::MIN as i64 / 4, i32::MAX as i64 / 4) as i32;
    *x = [x0, x[0]];
    *y = [y0, y[0]];
    y0
}

/// El seno de la tabla, para las voces del gato.
pub(crate) fn seno_de(fase: u32) -> i32 {
    seno(fase)
}

/// El paso de fase de una nota MIDI (0..127).
pub(crate) fn inc_midi(m: i32) -> u32 {
    INC[m.clamp(0, 127) as usize]
}

/// El paso de fase de una frecuencia en Hz enteros (el ZUMBIDO las usa).
pub(crate) fn inc_hz(hz: u32) -> u32 {
    (((hz as u64) << 32) / HZ as u64) as u32
}

/// El seno de la tabla, interpolado: 9 bits de indice y 16 de fraccion.
fn seno(fase: u32) -> i32 {
    let i = (fase >> 23) as usize;
    let f = ((fase >> 7) & 0xFFFF) as i32;
    let a = SENO[i] as i32;
    let b = SENO[i + 1] as i32;
    a + (((b - a) * f) >> 16)
}

/// **PolyBLEP**: el escalon de una onda, redondeado en la muestra de antes y
/// la de despues. `t` es la fase, `dt` lo que avanza por muestra. Q15.
fn blep(t: u32, dt: u32) -> i32 {
    if dt == 0 {
        return 0;
    }
    if t < dt {
        let x = ((t as i64) << 15) / dt as i64;
        (2 * x - ((x * x) >> 15) - 32_768) as i32
    } else if t > u32::MAX - dt {
        let x = ((t as i64 - (1i64 << 32)) << 15) / dt as i64;
        (((x * x) >> 15) + 2 * x + 32_768) as i32
    } else {
        0
    }
}

/// Una muestra de un oscilador, en Q15.
pub(crate) fn oscilar(timbre: Timbre, t: u32, dt: u32) -> i32 {
    match timbre {
        Timbre::Seno => seno(t),
        Timbre::Sierra => ((t >> 16) as i32 - 32_768) - blep(t, dt),
        Timbre::Cuadrada => {
            let ingenua = if t < 1 << 31 { 32_767 } else { -32_768 };
            ingenua + blep(t, dt) - blep(t.wrapping_add(1 << 31), dt)
        }
        Timbre::Triangulo => {
            let u = (t >> 16) as i32;
            if u < 16_384 {
                2 * u
            } else if u < 49_152 {
                65_536 - 2 * u
            } else {
                2 * u - 131_072
            }
        }
    }
}
