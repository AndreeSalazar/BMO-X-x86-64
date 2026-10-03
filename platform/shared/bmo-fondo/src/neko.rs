//! **EL GATO: las voces del NEKO PHONK** (S4h de `PLAN_EL_SONIDO.md`).
//!
//! El propietario, el 2026-10-03: *"inspirate en combinar Phonk [...] y
//! Geoxor [...] estudia como se hicieron, pero para tener todo ese sonido en
//! cat o neko sonido, para dar vida en todo mi BMO-X"*.
//!
//! Lo que se toma de cada uno, como TECNICA (nada se copia, todo se compone
//! aqui):
//!
//! ```text
//!    del PHONK     el CENCERRO del 808 (dos cuadradas en razon 1 : 1,48, un
//!                  golpe que cae enseguida y una cola corta), el 808 que cae
//!                  a su nota y se SATURA (el grunido), el BOMBEO de todo lo
//!                  demas con cada bombo, y la escala FRIGIA (la segunda
//!                  menor: la tension)
//!    de GEOXOR     lo tierno y lo bruto a la vez: brillante, rapido, con
//!                  "voces" que juegan
//!    del GATO      el MAULLIDO: una sierra por dos resonancias que se
//!                  MUEVEN (los formantes: la vocal i-a-u del "miau") con
//!                  su curva de tono; el RONRONEO, ruido grave que late a
//!                  26 Hz; y el BUFIDO, aire a 4 kHz, el gato enfadado
//! ```
//!
//! Sin coma flotante: las resonancias salen de una tabla (`neko_tablas.rs`)
//! y se mueven cada 16 muestras.

use bmo_amplificador::{MilesimasDb, DB};

use crate::neko_tablas::{FRECUENCIA, RESONANCIA, RONRONEO_POLO};
use crate::sintesis::{biquad, inc_hz, oscilar, seno_de, Forma, Nota, Timbre, HZ};

/// **Que maullido.** Cada uno es una curva: (ms, tono, primera y segunda
/// resonancia), en Hz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maullido {
    /// "nya": corto y agudo, el de llamar.
    Nya,
    /// "mi-a-u": entero.
    Miau,
    /// "mrrp": el trino de saludo, que sube.
    Mrrp,
    /// "mew?": sube al final, como una pregunta.
    Pregunta,
    /// El triste: cae.
    Triste,
}

type Curva = [(u32, u32, u32, u32); 4];

impl Maullido {
    fn curva(self) -> Curva {
        match self {
            Maullido::Nya => [(0, 620, 300, 2400), (60, 760, 700, 2200), (150, 820, 950, 1500), (280, 600, 650, 1100)],
            Maullido::Miau => [(0, 520, 350, 2500), (150, 700, 800, 1700), (350, 640, 900, 1300), (560, 460, 450, 800)],
            Maullido::Mrrp => [(0, 380, 400, 1200), (80, 470, 550, 1300), (160, 600, 650, 1600), (230, 720, 700, 1900)],
            Maullido::Pregunta => [(0, 560, 500, 2000), (150, 640, 700, 1900), (260, 760, 750, 2000), (360, 920, 800, 2200)],
            Maullido::Triste => [(0, 820, 850, 1700), (200, 720, 800, 1450), (420, 560, 600, 1100), (620, 420, 450, 850)],
        }
    }

    /// Lo que dura, en muestras.
    pub fn muestras(self) -> u32 {
        self.curva()[3].0 * HZ / 1000
    }
}

/// La resonancia de la tabla mas cercana a `hz`.
fn resonancia(hz: u32) -> [i64; 5] {
    let i = FRECUENCIA.partition_point(|&f| (f as u32) < hz).min(FRECUENCIA.len() - 1);
    let i = if i > 0 && hz.abs_diff(FRECUENCIA[i - 1] as u32) <= hz.abs_diff(FRECUENCIA[i] as u32) { i - 1 } else { i };
    let k = RESONANCIA[i];
    [k[0] as i64, k[1] as i64, k[2] as i64, k[3] as i64, k[4] as i64]
}

/// Interpola la curva a los `ms`: (tono, F1, F2).
fn en_la_curva(c: &Curva, ms: u32) -> (u32, u32, u32) {
    let mut r = (c[0].1, c[0].2, c[0].3);
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        if ms >= b.0 {
            r = (b.1, b.2, b.3);
        } else if ms >= a.0 {
            let f = (ms - a.0) * 1000 / (b.0 - a.0).max(1);
            let m = |x: u32, y: u32| (x as i64 + (y as i64 - x as i64) * f as i64 / 1000) as u32;
            r = (m(a.1, b.1), m(a.2, b.2), m(a.3, b.3));
        }
    }
    r
}

/// El 808 cae de la octava a su nota a la mitad cada 45 ms (Q30 por muestra).
const OCHO_CAE_Q30: u64 = 1_073_397_314;

/// Saturacion suave: `1,5 u - 0,5 u^3` con `u` en [-1, 1] (Q15), y +-1
/// fuera. Lo que hace grunir al 808 sin el chasquido de un recorte a pelo.
fn saturar(x: i64) -> i32 {
    let u = x.clamp(-32_768, 32_768);
    ((3 * u * 32_768 * 32_768 - u * u * u) / (2 * 32_768 * 32_768)) as i32
}

impl Nota {
    /// **Un golpe de cencerro** en la nota de paso `inc`.
    pub fn cencerro(inicio: i64, inc: u32, vol: MilesimasDb, extra: MilesimasDb) -> Nota {
        let ms = |m: u32| HZ * m / 1000;
        let mut n = Nota::con_envolvente(inicio, ms(400), Forma::Cencerro, ms(18), ms(380), vol + extra, vol - 9 * DB + extra, -60 * DB + extra);
        n.inc = (inc as u64) << 16;
        n.mueve = ((inc as u64 * 148) / 100) as u32;
        n.bombea = true;
        n
    }

    /// **Un 808**: arranca una octava arriba, cae a su nota, y suena `largo`
    /// muestras saturado.
    pub fn ochocientos(inicio: i64, inc: u32, largo: u32, vol: MilesimasDb, extra: MilesimasDb) -> Nota {
        let mut n = Nota::con_envolvente(inicio, largo, Forma::Ochocientos, HZ * 60 / 1000, largo, vol + extra, vol - 2 * DB + extra, -50 * DB + extra);
        n.inc = (inc as u64 * 2) << 16;
        n.mueve = inc;
        n
    }

    /// **Un maullido**, subido o bajado `semitonos` (enteros, -24..24): las
    /// resonancias no se mueven (la vocal es la misma), solo el tono.
    pub fn maullido(inicio: i64, m: Maullido, semitonos: i32, vol: MilesimasDb, extra: MilesimasDb) -> Nota {
        let largo = m.muestras();
        let mut n = Nota::con_envolvente(inicio, largo, Forma::Maullido(m), HZ * 35 / 1000, largo, vol - 30 * DB + extra, vol + extra, -50 * DB + extra);
        // 2^(s/12) en Q16, de la tabla de notas: la razon entre dos de ellas.
        let base = crate::sintesis::inc_midi(60) as u64;
        let otra = crate::sintesis::inc_midi(60 + semitonos.clamp(-24, 24)) as u64;
        n.mueve = ((otra << 16) / base) as u32;
        n.bombea = true;
        n
    }

    /// **Un ronroneo** de `largo` muestras: entra y sale como una respiracion.
    pub fn ronroneo(inicio: i64, largo: u32, vol: MilesimasDb) -> Nota {
        let mut n = Nota::con_envolvente(inicio, largo, Forma::Ronroneo, largo / 2, largo, vol - 24 * DB, vol, -40 * DB);
        n.mueve = inc_hz(26);
        n
    }

    /// **Un bufido** de `largo` muestras.
    pub fn bufido(inicio: i64, largo: u32, vol: MilesimasDb) -> Nota {
        let mut n = Nota::con_envolvente(inicio, largo, Forma::Bufido, HZ * 20 / 1000, largo, vol - 20 * DB, vol, -50 * DB);
        n.k1 = resonancia(4_200);
        n
    }

    pub(crate) fn cencerro_muestra(&mut self) -> i32 {
        let a = oscilar(Timbre::Cuadrada, self.fase, (self.inc >> 16) as u32);
        let b = oscilar(Timbre::Cuadrada, self.fase2, self.mueve);
        self.fase2 = self.fase2.wrapping_add(self.mueve);
        (a + b) / 2
    }

    pub(crate) fn ochocientos_muestra(&mut self) -> i32 {
        let o = seno_de(self.fase);
        let meta = (self.mueve as u64) << 16;
        if self.inc > meta {
            self.inc = (((self.inc as u128 * OCHO_CAE_Q30 as u128) >> 30) as u64).max(meta);
        }
        // Empujado tres veces a la saturacion: el grunido del 808.
        saturar(o as i64 * 3)
    }

    pub(crate) fn maullido_muestra(&mut self, m: Maullido) -> i32 {
        if self.t % 16 == 0 {
            let (f0, f1, f2) = en_la_curva(&m.curva(), self.t * 1000 / HZ);
            // El tono, transpuesto (`mueve` es la razon en Q16).
            self.inc = ((inc_hz(f0) as u64 * self.mueve as u64) >> 16) << 16;
            self.k1 = resonancia(f1);
            self.k2 = resonancia(f2);
        }
        let dt = (self.inc >> 16) as u32;
        let src = oscilar(Timbre::Sierra, self.fase, dt);
        let a = biquad(&self.k1, &mut self.x, &mut self.y, src);
        let b = biquad(&self.k2, &mut self.x2, &mut self.y2, src);
        // Las resonancias estrechas se llevan casi toda la energia: se
        // devuelve con ganancia, y la segunda un poco por debajo.
        (a as i64 * 3 + b as i64 * 2) as i32
    }

    pub(crate) fn ronroneo_muestra(&mut self) -> i32 {
        let r = self.blanco();
        // Dos pasos bajos seguidos (12 dB por octava): con uno solo, el ruido
        // blanco dejaba demasiados agudos y "ronroneaba" a 5 kHz.
        self.bajo += (RONRONEO_POLO as i64 * (((r as i64) << 16) - self.bajo) + (1 << 15)) >> 16;
        self.bajo2 += (RONRONEO_POLO as i64 * (self.bajo - self.bajo2) + (1 << 15)) >> 16;
        let grave = (self.bajo2 >> 16) as i32;
        // El latido: entre el 30 % y el 100 %, 26 veces por segundo.
        let late = 19_660 + ((seno_de(self.fase2) as i64 + 32_768) * 45_876 / 65_536) as i32;
        self.fase2 = self.fase2.wrapping_add(self.mueve);
        ((grave as i64 * 10 * late as i64) >> 16) as i32
    }

    pub(crate) fn bufido_muestra(&mut self) -> i32 {
        let r = self.blanco();
        let a = biquad(&self.k1, &mut self.x, &mut self.y, r);
        a * 2 + r / 4
    }
}
