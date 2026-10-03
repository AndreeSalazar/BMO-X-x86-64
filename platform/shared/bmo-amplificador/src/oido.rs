//! **EL OIDO: el perfil de quien escucha, aplicado a TODO lo que suena.**
//!
//! El propietario, el 2026-10-03: *"vamos a mejorar el audifono, eso es para
//! que se aplique en general, y control y mas cosas [...] audio
//! profesional"*. Y lo que esta casa ya sabia: su propietario es duro de
//! oido (ver la cabecera del crate), y su audifono tiene DOS transductores
//! (`PLAN_EL_SONIDO.md` 3.5): lo que se puede hacer por el, se hace en la
//! etapa que pasa todo, el MAESTRO.
//!
//! ```text
//!    MONO      los dos lados sumados: quien oye mejor por un oido no se
//!              pierde lo que la mezcla puso solo en el otro
//!    TONO      graves (100 Hz), medios (1 kHz) y AGUDOS (3,5 kHz), de -12
//!              a +12 dB: los agudos son lo primero que se pierde, y lo que
//!              hace entender una voz
//!    BALANCE   -100 (solo izquierda) .. +100 (solo derecha), sin saltos
//! ```
//!
//! # [!] Esto corrige el "no es un DAW" de `PLAN_EL_SONIDO.md` seccion 5
//!
//! Aquel "no hay ecualizador" sigue en pie para lo que decia: efectos de
//! estudio que se enchufan y se encadenan. Esto no es eso: son los TRES
//! mandos de tono de un amplificador de alta fidelidad, fijos, con su tabla,
//! y existen por la misma razon que el techo de +52 dB: el sonido de esta
//! casa es PERSONAL. Lo pidio el propietario, y la correccion se escribe alli.
//!
//! # Lo que lo hace profesional
//!
//! - **En plano es un cable.** Con el perfil plano no se toca ni un bit: el
//!   maestro de siempre, y sus pruebas lo fijan.
//! - **Sin coma flotante** y el mismo byte en todas las maquinas: los
//!   coeficientes salen de una tabla (`oido_tablas.rs`), uno por dB entero.
//! - **Sin el silbido de los filtros graves en coma fija.** Un estante a 100
//!   Hz tiene sus polos pegados a 1, y el redondeo de cada muestra se
//!   amplifica por ellos: en 16 bits sonaria como un soplido. Aqui el estado
//!   lleva 8 bits de mas y el resto de cada redondeo se devuelve a la muestra
//!   siguiente (realimentacion del error). La prueba
//!   `tras_un_golpe_el_silencio_es_silencio` lo fija.
//! - **El balance va por rampa**: moverlo no da un escalon.

use crate::oido_tablas::*;

/// **Un perfil de oido.** Lo que el escritorio pide y el kernel aplica.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perfil {
    /// -100 solo izquierda .. 0 centro .. +100 solo derecha.
    pub balance: i32,
    pub mono: bool,
    /// Los tres tonos, en dB enteros, -12..=12.
    pub graves: i32,
    pub medios: i32,
    pub agudos: i32,
}

impl Perfil {
    /// El perfil que no toca nada.
    pub const PLANO: Perfil = Perfil { balance: 0, mono: false, graves: 0, medios: 0, agudos: 0 };

    /// Recortado a lo que se acepta.
    pub fn recortado(self) -> Perfil {
        Perfil {
            balance: self.balance.clamp(-100, 100),
            mono: self.mono,
            graves: self.graves.clamp(-12, 12),
            medios: self.medios.clamp(-12, 12),
            agudos: self.agudos.clamp(-12, 12),
        }
    }

    pub fn es_plano(&self) -> bool {
        *self == Perfil::PLANO
    }

    /// **En un `u64`**, para pasar por un atomico (el escritorio escribe, el
    /// bus lee): `[0..8)` balance | `[8]` mono | `[16..24)` graves |
    /// `[24..32)` medios | `[32..40)` agudos, los cuatro como `i8`.
    pub fn empaquetar(&self) -> u64 {
        let b = |v: i32| (v as i8 as u8) as u64;
        b(self.balance) | ((self.mono as u64) << 8) | (b(self.graves) << 16) | (b(self.medios) << 24) | (b(self.agudos) << 32)
    }

    pub fn desempaquetar(x: u64) -> Perfil {
        let b = |s: u32| ((x >> s) & 0xFF) as u8 as i8 as i32;
        Perfil { balance: b(0), mono: (x >> 8) & 1 != 0, graves: b(16), medios: b(24), agudos: b(32) }.recortado()
    }
}

/// Un biquad en forma directa I, con 8 bits de mas en el estado y
/// realimentacion del error de redondeo.
#[derive(Clone, Copy, Debug)]
struct Biquad {
    k: [i64; 5],
    x: [i64; 2],
    y: [i64; 2],
    resto: i64,
}

/// Bits de mas del estado.
const EXTRA: u32 = 8;

impl Biquad {
    const QUIETO: Biquad = Biquad { k: [1 << 28, 0, 0, 0, 0], x: [0; 2], y: [0; 2], resto: 0 };

    fn poner(&mut self, k: &[i32; 5]) {
        for (a, &b) in self.k.iter_mut().zip(k.iter()) {
            *a = b as i64;
        }
    }

    fn muestra(&mut self, entra: i32) -> i32 {
        let x0 = (entra as i64) << EXTRA;
        let acc = self.k[0] * x0 + self.k[1] * self.x[0] + self.k[2] * self.x[1] - self.k[3] * self.y[0] - self.k[4] * self.y[1]
            + self.resto;
        let y0 = acc >> 28;
        self.resto = acc - (y0 << 28);
        self.x = [x0, self.x[0]];
        self.y = [y0, self.y[0]];
        // Y de vuelta, redondeando al mas cercano.
        ((y0 + (1 << (EXTRA - 1))) >> EXTRA).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
}

/// Lo que da un paso el balance por trama (1/65536 de pleno): ~5 ms de punta
/// a punta a 48 kHz.
const BALANCE_PASO: i32 = 65_536 / 240;

/// **El oido en marcha**: el perfil y el estado de sus filtros.
#[derive(Clone, Copy, Debug)]
pub struct Oido {
    perfil: Perfil,
    hz: u32,
    /// Graves, medios y agudos, por lado.
    filtros: [[Biquad; 3]; 2],
    /// La ganancia de cada lado (Q16) que suena, y a la que va.
    lado: [i32; 2],
    lado_meta: [i32; 2],
}

impl Oido {
    pub const fn nuevo(hz: u32) -> Oido {
        Oido {
            perfil: Perfil::PLANO,
            hz,
            filtros: [[Biquad::QUIETO; 3]; 2],
            lado: [1 << 16; 2],
            lado_meta: [1 << 16; 2],
        }
    }

    /// La tabla de esta frecuencia, si la hay. Fuera de 44,1 y 48 kHz el TONO
    /// no se aplica (mono y balance si), y `tono_aplica` lo dice.
    fn tablas(&self) -> Option<[&'static [[i32; 5]; 25]; 3]> {
        match self.hz {
            44_100 => Some([&GRAVES_44100, &MEDIOS_44100, &AGUDOS_44100]),
            48_000 => Some([&GRAVES_48000, &MEDIOS_48000, &AGUDOS_48000]),
            _ => None,
        }
    }

    /// El tono sirve a esta frecuencia?
    pub fn tono_aplica(&self) -> bool {
        self.tablas().is_some()
    }

    pub fn perfil(&self) -> Perfil {
        self.perfil
    }

    /// **Poner un perfil.** Si no cambio, no se toca nada (el kernel lo
    /// llama en cada trama). Los filtros conservan su estado: cambiar un dB
    /// no reinicia la onda.
    pub fn poner(&mut self, p: Perfil) {
        let p = p.recortado();
        if p == self.perfil {
            return;
        }
        // Una banda que se ENCIENDE empieza de cero: su memoria es de cuando
        // estaba apagada, y arrancar con ella seria un golpe.
        let antes = [self.perfil.graves, self.perfil.medios, self.perfil.agudos];
        let ahora = [p.graves, p.medios, p.agudos];
        for b in 0..3 {
            if antes[b] == 0 && ahora[b] != 0 {
                for lado in self.filtros.iter_mut() {
                    lado[b] = Biquad::QUIETO;
                }
            }
        }
        self.perfil = p;
        if let Some(t) = self.tablas() {
            for lado in self.filtros.iter_mut() {
                lado[0].poner(&t[0][(p.graves + 12) as usize]);
                lado[1].poner(&t[1][(p.medios + 12) as usize]);
                lado[2].poner(&t[2][(p.agudos + 12) as usize]);
            }
        }
        let b = p.balance;
        self.lado_meta = [
            if b > 0 { (100 - b) * 65_536 / 100 } else { 1 << 16 },
            if b < 0 { (100 + b) * 65_536 / 100 } else { 1 << 16 },
        ];
    }

    /// En plano y sin rampa por delante: el maestro puede no llamarlo.
    pub fn en_reposo(&self) -> bool {
        self.perfil.es_plano() && self.lado == self.lado_meta && self.lado[0] == 1 << 16
    }

    /// **Una trama** (las muestras de un instante, una por canal), en su
    /// sitio. Con mas de dos canales, el oido toca los dos primeros.
    pub fn trama(&mut self, t: &mut [i32]) {
        if t.len() >= 2 && self.perfil.mono {
            let m = ((t[0] as i64 + t[1] as i64) / 2) as i32;
            t[0] = m;
            t[1] = m;
        }
        let tono = self.tablas().is_some();
        let p = self.perfil;
        let bandas = [p.graves != 0, p.medios != 0, p.agudos != 0];
        for (c, x) in t.iter_mut().take(2).enumerate() {
            if tono {
                for (b, &activa) in bandas.iter().enumerate() {
                    if activa {
                        *x = self.filtros[c][b].muestra(*x);
                    }
                }
            }
            let (g, meta) = (self.lado[c], self.lado_meta[c]);
            let g = if g < meta { (g + BALANCE_PASO).min(meta) } else { (g - BALANCE_PASO).max(meta) };
            self.lado[c] = g;
            if g != 1 << 16 {
                *x = ((*x as i64 * g as i64) >> 16) as i32;
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    #[test]
    fn las_tablas_del_oido_cuadran() {
        // El estante alto a +12 dB: en 16 kHz tiene que dar casi +12, y en
        // 100 Hz casi nada. Se mide con la respuesta del biquad, en dobles.
        let respuesta = |k: &[i32; 5], f: f64, fs: f64| {
            let w = 2.0 * core::f64::consts::PI * f / fs;
            let q = |v: i32| v as f64 / (1u64 << 28) as f64;
            let (c1, s1, c2, s2) = (w.cos(), -w.sin(), (2.0 * w).cos(), -(2.0 * w).sin());
            let num = ((q(k[0]) + q(k[1]) * c1 + q(k[2]) * c2), (q(k[1]) * s1 + q(k[2]) * s2));
            let den = ((1.0 + q(k[3]) * c1 + q(k[4]) * c2), (q(k[3]) * s1 + q(k[4]) * s2));
            20.0 * ((num.0 * num.0 + num.1 * num.1) / (den.0 * den.0 + den.1 * den.1)).sqrt().log10()
        };
        for (tabla, fs) in [(&AGUDOS_48000, 48_000.0), (&AGUDOS_44100, 44_100.0)] {
            let alto = respuesta(&tabla[24], 16_000.0, fs);
            let bajo = respuesta(&tabla[24], 100.0, fs);
            assert!((alto - 12.0).abs() < 1.0, "agudos +12 en 16 kHz: {alto:.2}");
            assert!(bajo.abs() < 0.5, "agudos +12 en 100 Hz: {bajo:.2}");
            assert!(respuesta(&tabla[12], 5_000.0, fs).abs() < 0.01, "0 dB es plano");
        }
        let medio = respuesta(&MEDIOS_48000[18], 1_000.0, 48_000.0);
        assert!((medio - 6.0).abs() < 0.05, "medios +6 en 1 kHz: {medio:.2}");
        let grave = respuesta(&GRAVES_48000[0], 30.0, 48_000.0);
        assert!((grave + 12.0).abs() < 1.0, "graves -12 en 30 Hz: {grave:.2}");
    }

    /// Un seno de `f` Hz por el oido; la fuerza de salida entre la de entrada,
    /// en dB, del lado izquierdo, despues de 0,2 s de asentarse.
    fn ganancia(o: &mut Oido, f: f64) -> f64 {
        let n = 48_000usize;
        let (mut e, mut s) = (0f64, 0f64);
        for i in 0..n {
            let x = (8_000.0 * (2.0 * core::f64::consts::PI * f * i as f64 / 48_000.0).sin()) as i32;
            let mut t = [x, x];
            o.trama(&mut t);
            if i > n / 5 {
                e += (x as f64).powi(2);
                s += (t[0] as f64).powi(2);
            }
        }
        10.0 * (s / e).log10()
    }

    #[test]
    fn el_tono_hace_lo_que_dice() {
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil { agudos: 9, ..Perfil::PLANO });
        let g = ganancia(&mut o, 12_000.0);
        assert!((g - 9.0).abs() < 1.0, "agudos +9 a 12 kHz: {g:.2}");
        let g = ganancia(&mut o, 200.0);
        assert!(g.abs() < 0.5, "y a 200 Hz no: {g:.2}");
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil { graves: 6, ..Perfil::PLANO });
        let g = ganancia(&mut o, 40.0);
        assert!((g - 6.0).abs() < 1.0, "graves +6 a 40 Hz: {g:.2}");
    }

    #[test]
    fn tras_un_golpe_el_silencio_es_silencio() {
        // Lo peor para la coma fija: graves a tope. Un golpe, y despues
        // silencio: tiene que volver a CERO, sin soplido ni ciclo limite.
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil { graves: 12, medios: -12, agudos: 12, ..Perfil::PLANO });
        for i in 0..4_800 {
            let x = if i < 2_400 { 30_000 } else { -30_000 };
            o.trama(&mut [x, x]);
        }
        let mut cola = Vec::new();
        for _ in 0..96_000 {
            let mut t = [0, 0];
            o.trama(&mut t);
            cola.push(t[0]);
        }
        let ultimo_segundo = &cola[48_000..];
        assert!(ultimo_segundo.iter().all(|&x| x.abs() <= 1), "el silencio sopla: {:?}", &ultimo_segundo[..8]);
    }

    #[test]
    fn mono_y_balance() {
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil { mono: true, ..Perfil::PLANO });
        let mut t = [10_000, 0];
        o.trama(&mut t);
        assert_eq!(t, [5_000, 5_000], "lo de un lado, a los dos");
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil { balance: 100, ..Perfil::PLANO });
        let mut ultimo = [0, 0];
        let mut antes = 10_000;
        for _ in 0..400 {
            let mut t = [10_000, 10_000];
            o.trama(&mut t);
            assert!(antes - t[0] <= 50, "el balance da un escalon");
            antes = t[0];
            ultimo = t;
        }
        assert_eq!(ultimo, [0, 10_000], "todo a la derecha, por la rampa");
    }

    #[test]
    fn en_plano_no_toca_nada() {
        let mut o = Oido::nuevo(48_000);
        o.poner(Perfil::PLANO);
        assert!(o.en_reposo());
        let mut t = [12_345, -6_789];
        o.trama(&mut t);
        assert_eq!(t, [12_345, -6_789]);
        assert_eq!(Perfil::desempaquetar(Perfil { balance: -40, mono: true, graves: -3, medios: 5, agudos: 12 }.empaquetar()).agudos, 12);
        let p = Perfil { balance: -40, mono: true, graves: -3, medios: 5, agudos: 12 };
        assert_eq!(Perfil::desempaquetar(p.empaquetar()), p);
        assert_eq!(Perfil::desempaquetar(Perfil { agudos: 99, ..Perfil::PLANO }.empaquetar()).agudos, 12, "recortado");
    }
}
