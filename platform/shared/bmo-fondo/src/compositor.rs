//! **EL COMPOSITOR: la partitura de una pieza, tocada muestra a muestra.**
//!
//! Es la `programar()` de la maqueta: dieciseis pasos por compas, ocho
//! compases por vuelta, y en cada paso el bombo, la caja, el plato, el bajo,
//! el arpegio y los acordes que dice el patron de la pieza.
//!
//! # El bucle SIN COSTURA
//!
//! La musica de fondo se toca en BUCLE (la voz del orquestador vuelve al
//! principio al acabar). Si se compusieran las ocho vueltas a secas, la
//! primera muestra empezaria en silencio y la ultima cortaria las notas que
//! aun sonaban: cada vuelta tendria un hueco y un corte, o sea un CLIC cada
//! veinte segundos.
//!
//! Aqui no: el compositor arranca UN COMPAS ANTES del cero ([`ANTES`] pasos)
//! y tira lo que suena ahi. Como el patron se repite, las notas de ese compas
//! son exactamente las del ultimo compas de la vuelta, asi que la muestra 0
//! ya lleva las colas de la vuelta anterior. La ultima muestra del bucle
//! sigue en la primera como si la cancion no se acabara nunca. La prueba
//! `el_bucle_no_tiene_costura` lo comprueba muestra a muestra.

use bmo_amplificador::{Ganancia, Limite, Medidor, MilesimasDb, DB};

use crate::neko::Maullido;
use crate::sintesis::{inc_midi, Forma, Nota, Timbre, HZ};
use crate::{Escala, Estilo, Mezcla, Pieza};

/// Pasos por vuelta: ocho compases de dieciseis. Los acordes se repiten cada
/// cuatro y el arpegio calla en el octavo, asi que la vuelta entera son ocho.
pub const PASOS: i64 = 128;

/// Pasos que se componen ANTES del cero y se tiran: un compas. La nota mas
/// larga (un acorde) dura quince pasos.
pub const ANTES: i64 = 16;

/// Notas que pueden sonar a la vez. Lo normal son menos de doce.
const POLIFONIA: usize = 24;

/// Volumenes de la maqueta, en 1/256 dB: 0,32 / 0,06 / 0,14 / 0,045.
const VOL_BAJO: MilesimasDb = -2534;
const VOL_ARPEGIO: MilesimasDb = -6256;
const VOL_ARPEGIO_SENO: MilesimasDb = -4372;
const VOL_ACORDE: MilesimasDb = -6896;

/// **El patron de una pieza**, sacado de su semilla como en la maqueta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patron {
    pub bajo: [Option<i32>; 8],
    pub arpegio: [Option<i32>; 16],
    pub bombo: [bool; 16],
    pub caja: [bool; 16],
    pub plato: [bool; 16],
    pub acordes: [i32; 4],
}

/// La `azar()` de la maqueta, bit a bit: un hash de 32 bits.
pub fn azar(n: u32) -> u32 {
    let mut n = (n ^ 61) ^ (n >> 16);
    n = n.wrapping_mul(9);
    n ^= n >> 4;
    n = n.wrapping_mul(0x27d4_eb2d);
    n ^= n >> 15;
    n
}

// "azar < p" de la maqueta, con p en doble precision: `x / 2^32 < p` es
// `x < techo(p * 2^32)`. Calculados con la fraccion exacta de cada doble.
const P55: u32 = 2_362_232_013;
const P62: u32 = 2_662_879_724;
const P30: u32 = 1_288_490_189;
const P10: u32 = 429_496_730;
const P25: u32 = 1_073_741_824;

impl Patron {
    pub fn de(p: &Pieza) -> Patron {
        let esc = p.escala.grados();
        let r = |i: u32| azar(p.semilla.wrapping_mul(7919).wrapping_add(i));
        let grado = |i: u32| esc[((r(i) as u64 * esc.len() as u64) >> 32) as usize];
        let mut pat = Patron {
            bajo: [None; 8],
            arpegio: [None; 16],
            bombo: [false; 16],
            caja: [false; 16],
            plato: [false; 16],
            acordes: [0, esc[4 % esc.len()] - 12, esc[3 % esc.len()] - 12, esc[esc.len() - 2] - 12],
        };
        for i in 0..8u32 {
            pat.bajo[i as usize] = if i % 4 == 0 {
                Some(0)
            } else if r(100 + i) < P55 {
                Some(grado(200 + i))
            } else {
                None
            };
        }
        for i in 0..16u32 {
            let k = i as usize;
            if r(300 + i) < P62 {
                pat.arpegio[k] = Some(grado(400 + i) + if r(500 + i) < P30 { 12 } else { 0 });
            }
            pat.bombo[k] = i % 4 == 0 || r(600 + i) < P10;
            pat.caja[k] = i % 8 == 4;
            pat.plato[k] = i % 2 == 0 || r(700 + i) < P25;
        }
        pat
    }
}

impl Escala {
    fn grados(self) -> &'static [i32] {
        match self {
            Escala::Frigia => &[0, 1, 3, 5, 7, 8, 10],
            Escala::Menor => &[0, 2, 3, 5, 7, 8, 10],
            Escala::Mayor => &[0, 2, 4, 5, 7, 9, 11],
            Escala::Penta => &[0, 3, 5, 7, 10],
            Escala::Dorica => &[0, 2, 3, 5, 7, 9, 10],
        }
    }
}

/// **El patron del NEKO PHONK**, sacado de la semilla como el otro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatronPhonk {
    /// La frase del cencerro: dos compases de dieciseis pasos.
    pub cencerro: [Option<i32>; 32],
    /// Donde golpea el 808 en el compas.
    pub ocho: [bool; 16],
    /// La raiz de cada compas sobre la de la pieza: i, II bemol, i, VII bemol.
    pub raices: [i32; 4],
    /// El "nya" de cada compas impar (en semitonos), o nada.
    pub nya: [Option<i32>; 8],
}

impl PatronPhonk {
    pub fn de(p: &Pieza) -> PatronPhonk {
        let esc = p.escala.grados();
        let r = |i: u32| azar(p.semilla.wrapping_mul(7919).wrapping_add(1_000 + i));
        let grado = |i: u32| esc[((r(i) as u64 * esc.len() as u64) >> 32) as usize];
        let mut pat = PatronPhonk { cencerro: [None; 32], ocho: [false; 16], raices: [0, 1, 0, -2], nya: [None; 8] };
        for i in 0..32u32 {
            // A contratiempo casi siempre, en el tiempo a veces: el cencerro
            // del phonk baila ENTRE los golpes.
            let toca = if i % 2 == 1 { r(i) < P62 } else { i % 4 == 0 && r(i) < P30 };
            if toca {
                pat.cencerro[i as usize] = Some(grado(100 + i) + if r(200 + i) < P25 { 12 } else { 0 });
            }
        }
        pat.ocho[0] = true;
        pat.ocho[10] = true;
        pat.ocho[7] = r(300) < P55;
        pat.ocho[14] = r(301) < P30;
        for c in (1..8).step_by(2) {
            pat.nya[c] = Some(grado(400 + c as u32));
        }
        pat
    }
}

/// Volumenes del NEKO PHONK, en 1/256 dB.
const VOL_OCHO: MilesimasDb = -1_024;
const VOL_CENCERRO: MilesimasDb = -3_600;
const VOL_NYA: MilesimasDb = -2_800;
const VOL_ACORDE_SIERRA: MilesimasDb = -4_600;

/// **El bombeo**: lo que se bombea baja al 30 % con cada 808 y vuelve en
/// 125 ms. Es el latido del phonk (y del EDM entero).
const BOMBEO_VUELVE: u32 = 6_000;

/// La ganancia del bombeo (Q16) a las `t` muestras de un 808: del 30 % al
/// 100 % en linea recta en [`BOMBEO_VUELVE`].
pub fn ganancia_bombeo(t: u32) -> i32 {
    19_660 + (45_876 * t.min(BOMBEO_VUELVE) as i64 / BOMBEO_VUELVE as i64) as i32
}

/// **El compositor**: una pieza sonando, desde la muestra 0 de su bucle.
pub struct Compositor {
    pieza: Pieza,
    mezcla: Mezcla,
    patron: Patron,
    phonk: PatronPhonk,
    /// Muestras desde el ultimo 808: lo que lleva vuelto el bombeo.
    bombeo: u32,
    /// Donde empieza el proximo 808 (el bombeo arranca ahi).
    bombeo_en: Option<i64>,
    /// La muestra que sale ahora.
    s: i64,
    /// El siguiente paso por programar.
    k: i64,
    notas: [Nota; POLIFONIA],
    nivel: Ganancia,
    limite: Limite,
    medidor: Medidor,
    /// Notas que no cupieron y echaron a la mas vieja.
    robadas: u32,
}

impl Compositor {
    /// Lista para dar la muestra 0. Compone el compas de antes y lo tira.
    pub fn nuevo(pieza: &Pieza, mezcla: Mezcla) -> Compositor {
        let mut c = Compositor {
            pieza: *pieza,
            mezcla,
            patron: Patron::de(pieza),
            phonk: PatronPhonk::de(pieza),
            bombeo: BOMBEO_VUELVE,
            bombeo_en: None,
            s: 0,
            k: -ANTES,
            notas: [Nota::NADA; POLIFONIA],
            nivel: Ganancia::db(pieza.nivel + mezcla.maestro),
            limite: Limite::inmediato(HZ).con_relajo_ms(HZ, 250),
            medidor: Medidor::nuevo(),
            robadas: 0,
        };
        c.s = c.inicio(-ANTES);
        while c.s < 0 {
            let _ = c.siguiente();
        }
        c.medidor.olvidar();
        c
    }

    /// Muestras de una vuelta entera: lo que mide el bucle en el banco.
    pub fn muestras_del_bucle(&self) -> usize {
        self.paso_suelto(PASOS) as usize
    }

    /// Donde empieza el paso `k` de la primera vuelta (0..=128), redondeado.
    fn paso_suelto(&self, k: i64) -> i64 {
        let bpm = self.pieza.bpm as i64;
        (k * 720_000 * 2 + bpm) / (2 * bpm)
    }

    /// Donde empieza el paso `k`, de cualquier vuelta (tambien negativa).
    fn inicio(&self, k: i64) -> i64 {
        let vuelta = k.div_euclid(PASOS);
        vuelta * self.paso_suelto(PASOS) + self.paso_suelto(k.rem_euclid(PASOS))
    }

    /// Una duracion de `decimas` de paso, en muestras.
    fn pasos(&self, decimas: u32) -> u32 {
        decimas * 72_000 / self.pieza.bpm
    }

    fn meter(&mut self, n: Nota) {
        if let Some(sitio) = self.notas.iter_mut().find(|x| !x.viva()) {
            *sitio = n;
            return;
        }
        self.robadas += 1;
        if let Some(vieja) = self.notas.iter_mut().max_by_key(|x| x.t) {
            *vieja = n;
        }
    }

    /// **El paso `k` del NEKO PHONK.**
    fn programar_phonk(&mut self, k: i64) {
        let kk = k.rem_euclid(PASOS) as usize;
        let (i, compas) = (kk % 16, kk / 16);
        let t = self.inicio(k);
        let medio = (self.inicio(k + 1) - t) / 2;
        let (m, p) = (self.mezcla, self.phonk);
        let raiz = self.pieza.raiz + p.raices[compas % 4];
        // El 808 (y un golpe encima para el ataque): suena hasta el siguiente.
        if p.ocho[i] {
            let hasta = (i + 1..16).find(|&j| p.ocho[j]).unwrap_or(16);
            let largo = self.pasos(((hasta - i).min(8) * 10) as u32);
            if m.suena(m.bajo) {
                self.meter(Nota::ochocientos(t, inc_midi(raiz - 24), largo, VOL_OCHO, m.bajo));
            }
            if m.suena(m.bombo) {
                self.meter(Nota::golpe(t, Forma::Bombo, m.bombo - 4 * DB));
            }
            self.bombeo_en = Some(t);
        }
        if i == 8 && m.suena(m.caja) {
            self.meter(Nota::golpe(t, Forma::Caja, m.caja));
        }
        // El plato a corcheas, y redobles a fusas al final de cada cuatro.
        if m.suena(m.plato) {
            let redoble = compas % 4 == 3 && i >= 12;
            if i % 2 == 0 || redoble {
                self.meter(Nota::golpe(t, Forma::Plato, m.plato - 2 * DB));
            }
            if redoble {
                self.meter(Nota::golpe(t + medio, Forma::Plato, m.plato - 5 * DB));
            }
        }
        // El cencerro: la frase de dos compases; el octavo compas respira.
        if let Some(g) = p.cencerro[(compas % 2) * 16 + i] {
            if compas % 8 != 7 && m.suena(m.arpegio) {
                self.meter(Nota::cencerro(t, inc_midi(raiz + 12 + g), VOL_CENCERRO, m.arpegio));
            }
        }
        // Lo de Geoxor: un acorde de sierra corto y brillante en la segunda
        // mitad, en el uno de cada compas.
        if i == 0 && compas >= 4 && m.suena(m.acordes) {
            for x in [0, 3, 7] {
                let mut n = Nota::tono(t, inc_midi(raiz + 12 + x), self.pasos(30), Timbre::Sierra, VOL_ACORDE_SIERRA, m.acordes);
                n.bombea = true;
                self.meter(n);
            }
        }
        // El gato: "nya" al final de cada compas impar; "miau" al final de todo.
        if m.suena(m.acordes) {
            if i == 14 {
                if let Some(s) = p.nya[compas] {
                    self.meter(Nota::maullido(t, Maullido::Nya, s, VOL_NYA, m.acordes));
                }
            }
            if compas == 7 && i == 10 {
                self.meter(Nota::maullido(t, Maullido::Miau, 0, VOL_NYA, m.acordes));
            }
        }
    }

    /// La `programar()` de la maqueta, para el paso `k`.
    fn programar(&mut self, k: i64) {
        if self.pieza.estilo == Estilo::NekoPhonk {
            self.programar_phonk(k);
            return;
        }
        let kk = k.rem_euclid(PASOS) as usize;
        let (i, compas) = (kk % 16, kk / 16);
        let ac = self.patron.acordes[compas % 4];
        let t = self.inicio(k);
        let (m, raiz, timbre) = (self.mezcla, self.pieza.raiz, self.pieza.timbre);
        if self.patron.bombo[i] && m.suena(m.bombo) {
            self.meter(Nota::golpe(t, Forma::Bombo, m.bombo));
        }
        if self.patron.caja[i] && m.suena(m.caja) {
            self.meter(Nota::golpe(t, Forma::Caja, m.caja));
        }
        if self.patron.plato[i] && m.suena(m.plato) {
            self.meter(Nota::golpe(t, Forma::Plato, m.plato));
        }
        if i % 2 == 0 {
            if let Some(b) = self.patron.bajo[i / 2] {
                let n = Nota::tono(t, inc_midi(raiz - 12 + ac + b), self.pasos(18), Timbre::Triangulo, VOL_BAJO, m.bajo);
                self.meter(n);
            }
        }
        if let Some(a) = self.patron.arpegio[i] {
            if compas % 8 != 7 {
                let vol = if timbre == Timbre::Seno { VOL_ARPEGIO_SENO } else { VOL_ARPEGIO };
                let n = Nota::tono(t, inc_midi(raiz + 12 + ac + a), self.pasos(9), timbre, vol, m.arpegio);
                self.meter(n);
            }
        }
        if i == 0 {
            for x in [0, 3, 7] {
                let n = Nota::tono(t, inc_midi(raiz + ac + x), self.pasos(150), Timbre::Seno, VOL_ACORDE, m.acordes);
                self.meter(n);
            }
        }
    }

    /// La siguiente muestra, ya por el nivel y el limite.
    fn siguiente(&mut self) -> i16 {
        while self.inicio(self.k) <= self.s {
            self.programar(self.k);
            self.k += 1;
        }
        // El bombeo empieza en la muestra del 808, no en la del paso.
        if self.bombeo_en == Some(self.s) {
            self.bombeo = 0;
            self.bombeo_en = None;
        }
        let mut golpes: i32 = 0;
        let mut bombean: i32 = 0;
        for n in self.notas.iter_mut() {
            if n.viva() && n.inicio <= self.s {
                let x = n.muestra();
                if n.bombea {
                    bombean = bombean.saturating_add(x);
                } else {
                    golpes = golpes.saturating_add(x);
                }
            }
        }
        // ** EL BOMBEO: lo que se bombea, al 30 % con el 808 y de vuelta en
        // 125 ms. En las piezas de ambiente no hay nada que bombee.
        let g = ganancia_bombeo(self.bombeo);
        self.bombeo = self.bombeo.saturating_add(1);
        let suma = golpes.saturating_add(((bombean as i64 * g as i64) >> 16) as i32);
        self.s += 1;
        let x = self.limite.muestra(self.nivel.aplicar(suma));
        self.medidor.mirar_uno(x as i32);
        x
    }

    /// **Llena `fuera`** con lo que sigue. Se puede llamar a trozos: la
    /// musica sigue donde se quedo, y pasada la vuelta empieza la siguiente.
    pub fn llenar(&mut self, fuera: &mut [i16]) {
        for x in fuera.iter_mut() {
            *x = self.siguiente();
        }
    }

    /// Lo que salio hasta ahora: pico y fuerza (RMS) en dBFS.
    pub fn medida(&self) -> (MilesimasDb, MilesimasDb) {
        (self.medidor.pico_dbfs(), self.medidor.rms_dbfs())
    }

    /// Muestras que el limite tuvo que sujetar: si sube, el nivel esta mal.
    pub fn sujetadas(&self) -> u64 {
        self.limite.sujetadas()
    }

    /// Notas que no cupieron.
    pub fn robadas(&self) -> u32 {
        self.robadas
    }
}
