//! **EL ESPACIO: el 3D en dos oidos (S7), para TODO lo que suena.**
//!
//! El propietario, el 2026-10-03: *"sigue con el 3D en dos oidos [...] se
//! puede aplicar global si ponemos un Modo 3D, no? pero PRO, EPICO [...] y
//! 4D?"*.
//!
//! # El problema que arregla
//!
//! Con un audifono, el estereo suena DENTRO de la cabeza: lo de la izquierda
//! entra solo por el oido izquierdo, cosa que en el mundo no pasa nunca. Un
//! altavoz a la izquierda llega a LOS DOS oidos: al lejano un poco despues
//! (hasta 0,66 ms) y con menos agudos (la cabeza los tapa). Ese retardo y esa
//! sombra son las dos pistas que el cerebro usa para situar un sonido a los
//! lados (`PLAN_EL_SONIDO.md` S7, 1 y 2), y son baratas: un bufer circular,
//! un paso bajo y una multiplicacion.
//!
//! ```text
//!    APAGADO   el estereo tal cual (un cable)
//!    CERCA     dos altavoces virtuales a +-30 grados: el sonido sale de la
//!              cabeza y se pone DELANTE. Cansa menos en horas de audifono
//!    SALA      lo mismo y una habitacion: ocho reflejos tempranos (5 a 20
//!              ms) que pegan en paredes de mentira. Es lo que hace que un
//!              sonido se oiga FUERA, a una distancia
//!    AMPLIO    los altavoces a +-60 grados, con sala: la escena se abre
//!    ORBITA    el "4D": la escena entera GIRA alrededor de la cabeza, una
//!              vuelta cada N segundos, con sala
//! ```
//!
//! # [!] Lo que NO hace, dicho
//!
//! **Arriba y abajo, y delante/detras con precision**, piden HRTF (S7.3): una
//! respuesta medida de una cabeza por cada direccion. No esta, y el plan ya
//! dice por que. Lo de detras aqui es una pista mas pobre --mas oscuro--, y
//! con dos pistas el cerebro a veces lo pone delante: es la confusion de
//! siempre de los sistemas sin HRTF, y se dice. Y **"4D" no es una dimension
//! del sonido**: lo que se vende como audio 4D u 8D es 3D que se MUEVE en el
//! tiempo. Eso si es esto: ORBITA.
//!
//! # Lo que lo hace profesional
//!
//! - **Apagado es un cable**, y el maestro no lo llama.
//! - **El mismo volumen**: cada modo lleva su ajuste para que una musica
//!   centrada suene igual de fuerte con y sin 3D (prueba
//!   `cada_modo_suena_igual_de_fuerte`): comparar modos no es comparar
//!   volumenes.
//! - **Sin clics al cambiar**: el paso de seco a 3D es un fundido de 20 ms,
//!   la sala entra por su propia rampa, y los altavoces virtuales GIRAN hasta
//!   su sitio (un grado cada 16 tramas) en vez de saltar: un retardo que salta
//!   es un clic.
//! - **Retardo fraccionario**: la orbita mueve el retardo poco a poco, y
//!   leerlo entre dos muestras evita el chasquido de saltar de una a otra.
//! - Sin coma flotante; tablas por grado a 44,1 y 48 kHz (`espacio_tablas.rs`).

use crate::espacio_tablas::*;

/// **El modo 3D.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    Apagado,
    Cerca,
    Sala,
    Amplio,
    Orbita,
}

impl Modo {
    pub fn de(n: u64) -> Modo {
        match n {
            1 => Modo::Cerca,
            2 => Modo::Sala,
            3 => Modo::Amplio,
            4 => Modo::Orbita,
            _ => Modo::Apagado,
        }
    }

    pub fn numero(self) -> u64 {
        self as u64
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Modo::Apagado => "apagado",
            Modo::Cerca => "cerca",
            Modo::Sala => "sala",
            Modo::Amplio => "amplio",
            Modo::Orbita => "orbita",
        }
    }

    /// Los grados de cada altavoz virtual a un lado del centro.
    fn abertura(self) -> i32 {
        match self {
            Modo::Amplio => 60,
            _ => 30,
        }
    }

    fn con_sala(self) -> bool {
        matches!(self, Modo::Sala | Modo::Amplio | Modo::Orbita)
    }

    /// **El ajuste de volumen del modo** (Q16), medido por la prueba
    /// `cada_modo_suena_igual_de_fuerte` con una musica centrada.
    fn ajuste(self) -> i32 {
        match self {
            Modo::Apagado => 65_536,
            Modo::Cerca => 50_674,
            Modo::Sala => 53_359,
            Modo::Amplio => 52_839,
            Modo::Orbita => 42_924,
        }
    }
}

/// **El ajuste**: el modo y lo que tarda una vuelta de la orbita.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ajuste {
    pub modo: Modo,
    /// Segundos por vuelta, 2..=60.
    pub vuelta_s: u32,
}

impl Ajuste {
    pub const APAGADO: Ajuste = Ajuste { modo: Modo::Apagado, vuelta_s: 8 };

    pub fn recortado(self) -> Ajuste {
        Ajuste { modo: self.modo, vuelta_s: self.vuelta_s.clamp(2, 60) }
    }

    /// `[0..8)` modo | `[8..16)` segundos por vuelta (0 = 8).
    pub fn empaquetar(&self) -> u64 {
        self.modo.numero() | ((self.vuelta_s.min(255) as u64) << 8)
    }

    pub fn desempaquetar(x: u64) -> Ajuste {
        let s = ((x >> 8) & 0xFF) as u32;
        Ajuste { modo: Modo::de(x & 0xFF), vuelta_s: if s == 0 { 8 } else { s } }.recortado()
    }
}

/// La linea del sonido directo (el retardo entre oidos cabe de sobra).
const LINEA: usize = 64;
/// La linea de la sala: 21 ms a 48 kHz.
const SALA_N: usize = 1024;
/// Cada cuantas tramas se recalculan los caminos.
const PASO: u32 = 16;
/// El fundido de seco a 3D: 20 ms a 48 kHz.
const FUNDIDO: i32 = 65_536 / 960;

/// Los reflejos de la sala: (milesimas de segundo x10, ganancia Q16, de que
/// altavoz, a que oido). Primeros reflejos de una habitacion chica (el
/// suelo, las paredes, el techo y sus segundos rebotes, de -9 a -18 dB),
/// **en parejas espejo**: lo que llega a un oido llega al otro con la misma
/// fuerza y unas decimas de ms despues. Asi la sala no empuja la escena a un
/// lado (una musica centrada sigue en el centro) y los dos oidos no oyen lo
/// mismo, que es lo que la saca de la cabeza.
const REFLEJOS: [(u32, i32, usize, usize); 8] = [
    (53, 23_270, 0, 0),
    (59, 23_270, 1, 1),
    (91, 16_462, 0, 1),
    (97, 16_462, 1, 0),
    (143, 11_653, 0, 0),
    (151, 11_653, 1, 1),
    (179, 8_250, 0, 1),
    (197, 8_250, 1, 0),
];

/// Un camino de un altavoz virtual a un oido.
#[derive(Clone, Copy, Debug)]
struct Camino {
    ganancia: i32,
    /// En muestras, Q8.
    retardo: i32,
    /// El polo del paso bajo (Q16); 65536 = sin filtro.
    a: i32,
    estado: i64,
}

impl Camino {
    const NADA: Camino = Camino { ganancia: 0, retardo: 0, a: 65_536, estado: 0 };
}

/// Un paso bajo de un polo, en Q16 con el estado en Q16 extra.
fn paso_bajo(estado: &mut i64, a: i32, x: i32) -> i32 {
    if a >= 65_536 {
        *estado = (x as i64) << 16;
        return x;
    }
    // Redondeando al mas cercano, en el paso y en la salida: con el
    // desplazamiento a secas un negativo se queda pegado en -1 para siempre
    // (redondea hacia menos infinito), y el silencio nunca vuelve a ser cero.
    *estado += (a as i64 * (((x as i64) << 16) - *estado) + (1 << 15)) >> 16;
    ((*estado + (1 << 15)) >> 16) as i32
}

/// **El espacio en marcha.** Grande (las lineas de la sala): el kernel lo
/// tiene en un `static`, no en la pila. [`Espacio::nuevo`] es `const`.
pub struct Espacio {
    ajuste: Ajuste,
    hz: u32,
    linea: [[i32; LINEA]; 2],
    sala: [[i16; SALA_N]; 2],
    pos: usize,
    /// [altavoz][oido].
    caminos: [[Camino; 2]; 2],
    /// Lo de detras, mas oscuro: un paso bajo por altavoz.
    detras: [Camino; 2],
    sala_lp: [i64; 2],
    /// Donde esta cada altavoz virtual AHORA (grados, + derecha), y la
    /// fase de la orbita en milesimas de grado.
    angulo: [i32; 2],
    fase: u64,
    cuenta: u32,
    /// Cuanto 3D suena (0..65536) y cuanta sala, que van por rampa.
    mezcla: i32,
    sala_cuanta: i32,
    ultimo_ajuste: i32,
}

/// Seno de un angulo en grados enteros, en Q15.
fn seno(g: i32) -> i32 {
    let g = g.rem_euclid(360);
    match g {
        0..=90 => SENO[g as usize],
        91..=180 => SENO[(180 - g) as usize],
        181..=270 => -SENO[(g - 180) as usize],
        _ => -SENO[(360 - g) as usize],
    }
}

impl Espacio {
    pub const fn nuevo(hz: u32) -> Espacio {
        Espacio {
            ajuste: Ajuste::APAGADO,
            hz,
            linea: [[0; LINEA]; 2],
            sala: [[0; SALA_N]; 2],
            pos: 0,
            caminos: [[Camino::NADA; 2]; 2],
            detras: [Camino::NADA; 2],
            sala_lp: [0; 2],
            angulo: [-30, 30],
            fase: 0,
            cuenta: 0,
            mezcla: 0,
            sala_cuanta: 0,
            ultimo_ajuste: 65_536,
        }
    }

    pub fn ajuste(&self) -> Ajuste {
        self.ajuste
    }

    /// Hay tablas para esta frecuencia? (44,1 y 48 kHz.)
    pub fn aplica(&self) -> bool {
        matches!(self.hz, 44_100 | 48_000)
    }

    /// La frecuencia del tubo cambio: se empieza de cero.
    pub fn frecuencia(&mut self, hz: u32) {
        if hz != self.hz {
            let a = self.ajuste;
            *self = Espacio::nuevo(hz);
            self.ajuste = a;
        }
    }

    /// **Poner un ajuste.** No salta: el fundido, la sala y los angulos van
    /// por sus rampas desde aqui.
    pub fn poner(&mut self, a: Ajuste) {
        self.ajuste = a.recortado();
    }

    /// Apagado y sin nada por fundir: el maestro no tiene que llamarlo.
    pub fn en_reposo(&self) -> bool {
        (self.ajuste.modo == Modo::Apagado || !self.aplica()) && self.mezcla == 0
    }

    fn tablas(&self) -> (&'static [i32; 91], &'static [i32; 91], i32, i32) {
        if self.hz == 44_100 {
            (&ITD_44100, &SOMBRA_44100, DETRAS_44100, SALA_44100)
        } else {
            (&ITD_48000, &SOMBRA_48000, DETRAS_48000, SALA_48000)
        }
    }

    /// Los caminos de un altavoz virtual en `g` grados a los dos oidos.
    fn situar(&mut self, s: usize, g: i32) {
        let (itd, sombra, detras, _) = self.tablas();
        let g = (g + 180).rem_euclid(360) - 180;
        let lado = g.abs();
        let lat = if lado <= 90 { lado } else { 180 - lado } as usize;
        let sn = seno(lat as i32);
        for oido in 0..2 {
            let cerca = if oido == 1 { g >= 0 } else { g <= 0 };
            let c = &mut self.caminos[s][oido];
            if cerca {
                c.ganancia = 65_536;
                c.retardo = 0;
                c.a = 65_536;
            } else {
                // El oido lejano: despues, mas flojo y con la sombra.
                c.ganancia = 65_536 - (sn * 2 / 3);
                c.retardo = itd[lat];
                c.a = sombra[lat];
            }
        }
        // Detras: mas oscuro cuanto mas atras.
        self.detras[s].a = if lado <= 90 { 65_536 } else { 65_536 - (65_536 - detras) * (lado - 90) / 90 };
    }

    /// Cada [`PASO`] tramas: mueve la orbita y lleva los angulos a su sitio.
    fn mover(&mut self) {
        let a = self.ajuste;
        let w = a.modo.abertura();
        let centro = if a.modo == Modo::Orbita {
            // 360 grados por vuelta: avanza PASO tramas cada vez.
            let por_paso = 360_000u64 * PASO as u64 / (a.vuelta_s as u64 * self.hz.max(1) as u64);
            self.fase = (self.fase + por_paso.max(1)) % 360_000;
            (self.fase / 1000) as i32
        } else {
            // Al salir de la orbita vuelve al frente por el camino corto.
            0
        };
        for (s, meta) in [(0usize, centro - w), (1, centro + w)] {
            let ahora = self.angulo[s];
            let d = (meta - ahora + 540).rem_euclid(360) - 180;
            self.angulo[s] = ahora + d.signum() * d.abs().min(if a.modo == Modo::Orbita { 3 } else { 1 });
            let g = self.angulo[s];
            self.situar(s, g);
        }
    }

    /// Lee la linea de `s` con un retardo fraccionario (Q8 muestras).
    fn leer(&self, s: usize, retardo: i32) -> i32 {
        let d = (retardo >> 8) as usize;
        let f = (retardo & 255) as i64;
        let a = self.linea[s][(self.pos + LINEA - d) % LINEA] as i64;
        let b = self.linea[s][(self.pos + LINEA - d - 1) % LINEA] as i64;
        (a + (((b - a) * f) >> 8)) as i32
    }

    /// **Una trama estereo**, en su sitio. Con un solo canal no hay nada que
    /// situar; con mas de dos, se usan los dos primeros.
    pub fn trama(&mut self, t: &mut [i32]) {
        if t.len() < 2 || !self.aplica() {
            return;
        }
        let modo = self.ajuste.modo;
        let meta = if modo == Modo::Apagado { 0 } else { 65_536 };
        self.mezcla = if self.mezcla < meta { (self.mezcla + FUNDIDO).min(meta) } else { (self.mezcla - FUNDIDO).max(meta) };
        let sala_meta = if modo.con_sala() { 65_536 } else { 0 };
        self.sala_cuanta =
            if self.sala_cuanta < sala_meta { (self.sala_cuanta + FUNDIDO).min(sala_meta) } else { (self.sala_cuanta - FUNDIDO).max(sala_meta) };
        if self.mezcla == 0 {
            return;
        }
        if self.cuenta % PASO == 0 {
            self.mover();
        }
        self.cuenta = self.cuenta.wrapping_add(1);
        let seco = [t[0], t[1]];
        self.pos = (self.pos + 1) % LINEA;
        let sp = self.cuenta as usize % SALA_N;
        for s in 0..2 {
            let a = self.detras[s].a;
            let x = paso_bajo(&mut self.detras[s].estado, a, seco[s]);
            self.linea[s][self.pos] = x;
            self.sala[s][sp] = x.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
        let (_, _, _, sala_a) = self.tablas();
        let escala = self.hz as u64;
        // Al apagar, el fundido sale con el ajuste del modo que se iba.
        if modo != Modo::Apagado {
            self.ultimo_ajuste = modo.ajuste();
        }
        let ajuste = self.ultimo_ajuste as i64;
        for oido in 0..2 {
            let mut acc: i64 = 0;
            for s in 0..2 {
                let c = self.caminos[s][oido];
                let v = self.leer(s, c.retardo);
                let v = paso_bajo(&mut self.caminos[s][oido].estado, c.a, v);
                acc += (v as i64 * c.ganancia as i64) >> 16;
            }
            if self.sala_cuanta > 0 {
                let mut r: i64 = 0;
                for &(dms, g, de, a) in REFLEJOS.iter() {
                    if a != oido {
                        continue;
                    }
                    let n = (dms as u64 * escala / 10_000) as usize;
                    let x = self.sala[de][(sp + SALA_N - n.min(SALA_N - 1)) % SALA_N] as i64;
                    r += (x * g as i64) >> 16;
                }
                let r = paso_bajo(&mut self.sala_lp[oido], sala_a, r as i32) as i64;
                acc += (r * self.sala_cuanta as i64) >> 16;
            }
            let mojado = (acc * ajuste) >> 16;
            let d = seco[oido] as i64;
            t[oido] = (d + (((mojado - d) * self.mezcla as i64) >> 16)).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::boxed::Box;
    use std::vec::Vec;

    fn nuevo(m: Modo) -> Box<Espacio> {
        let mut e = Box::new(Espacio::nuevo(48_000));
        e.poner(Ajuste { modo: m, vuelta_s: 4 });
        e
    }

    #[test]
    fn las_tablas_del_espacio_cuadran() {
        for g in [0usize, 30, 45, 60, 90] {
            let r = (g as f64).to_radians();
            assert!((SENO[g] as f64 - 32767.0 * r.sin()).abs() <= 1.0);
            let itd = 48_000.0 * 0.0875 / 343.0 * (r + r.sin());
            assert!((ITD_48000[g] as f64 / 256.0 - itd).abs() < 0.01, "ITD de {g}");
        }
        assert!(ITD_48000[90] / 256 == 31, "0,66 ms a 90 grados");
        assert!(SOMBRA_48000[0] > SOMBRA_48000[90], "de lado, mas sombra");
    }

    #[test]
    fn apagado_es_un_cable() {
        let mut e = nuevo(Modo::Apagado);
        assert!(e.en_reposo());
        let mut t = [1234, -5678];
        e.trama(&mut t);
        assert_eq!(t, [1234, -5678]);
    }

    #[test]
    fn lo_de_la_izquierda_llega_al_oido_derecho_despues_y_mas_flojo() {
        let mut e = nuevo(Modo::Cerca);
        // Que pase el fundido y los angulos lleguen.
        for _ in 0..2_000 {
            e.trama(&mut [0, 0]);
        }
        let mut izq = Vec::new();
        let mut der = Vec::new();
        for i in 0..200 {
            let mut t = [if i == 0 { 20_000 } else { 0 }, 0];
            e.trama(&mut t);
            izq.push(t[0]);
            der.push(t[1]);
        }
        let primero = |v: &Vec<i32>| v.iter().position(|&x| x.abs() > 50).unwrap();
        let (pi, pd) = (primero(&izq), primero(&der));
        // 30 grados: 12,5 muestras de retardo (0,26 ms).
        assert_eq!(pi, 0, "el cercano, ya");
        assert!((11..=14).contains(&pd), "el lejano llega en la muestra {pd}");
        let pico = |v: &Vec<i32>| v.iter().map(|x| x.abs()).max().unwrap();
        assert!(pico(&der) < pico(&izq) / 2, "y mas flojo y apagado: {} contra {}", pico(&der), pico(&izq));
    }

    /// Una musica centrada (cuatro senos iguales en los dos lados): su
    /// fuerza en dB despues del modo, contra antes.
    fn fuerza(m: Modo) -> f64 {
        let mut e = nuevo(m);
        let (mut a, mut b) = (0f64, 0f64);
        for i in 0..96_000usize {
            let ts = i as f64 / 48_000.0;
            let x = [110.0, 440.0, 1_760.0, 5_000.0]
                .iter()
                .map(|f| 2_000.0 * (2.0 * core::f64::consts::PI * f * ts).sin())
                .sum::<f64>() as i32;
            let mut t = [x, x];
            e.trama(&mut t);
            if i > 48_000 {
                a += 2.0 * (x as f64).powi(2);
                b += (t[0] as f64).powi(2) + (t[1] as f64).powi(2);
            }
        }
        10.0 * (b / a).log10()
    }

    #[test]
    fn cada_modo_suena_igual_de_fuerte() {
        let medidas: Vec<(Modo, f64)> = [Modo::Cerca, Modo::Sala, Modo::Amplio, Modo::Orbita].iter().map(|&m| (m, fuerza(m))).collect();
        for &(m, db) in &medidas {
            std::println!("{:?}: {db:+.2} dB (ajuste {})", m, m.ajuste());
        }
        for (m, db) in medidas {
            assert!(db.abs() < 0.5, "{:?} cambia el volumen {db:+.2} dB", m);
        }
    }

    #[test]
    fn la_orbita_da_la_vuelta_a_la_cabeza() {
        // Un sonido centrado: con la orbita, en un momento manda el oido
        // izquierdo y en otro el derecho.
        let mut e = nuevo(Modo::Orbita);
        let mut balances = Vec::new();
        for trozo in 0..40 {
            let (mut l, mut r) = (0f64, 0f64);
            for i in 0..4_800usize {
                let ts = (trozo * 4_800 + i) as f64 / 48_000.0;
                let x = (8_000.0 * (2.0 * core::f64::consts::PI * 700.0 * ts).sin()) as i32;
                let mut t = [x, x];
                e.trama(&mut t);
                l += (t[0] as f64).powi(2);
                r += (t[1] as f64).powi(2);
            }
            balances.push(10.0 * (l / r).log10());
        }
        let max = balances.iter().cloned().fold(f64::MIN, f64::max);
        let min = balances.iter().cloned().fold(f64::MAX, f64::min);
        assert!(max > 2.0 && min < -2.0, "la escena no gira: de {min:.1} a {max:.1} dB");
    }

    #[test]
    fn cambiar_de_modo_no_da_golpes() {
        let mut e = nuevo(Modo::Apagado);
        let mut antes = 0i32;
        let mut peor = 0i32;
        for i in 0..20_000usize {
            if i == 5_000 {
                e.poner(Ajuste { modo: Modo::Sala, vuelta_s: 8 });
            }
            if i == 12_000 {
                e.poner(Ajuste { modo: Modo::Amplio, vuelta_s: 8 });
            }
            let x = (10_000.0 * (2.0 * core::f64::consts::PI * 200.0 * i as f64 / 48_000.0).sin()) as i32;
            let mut t = [x, x];
            e.trama(&mut t);
            if i > 0 {
                peor = peor.max((t[0] - antes).abs());
            }
            antes = t[0];
        }
        // Un seno de 200 Hz a 10.000 sube como mucho ~262 por muestra; un
        // golpe se veria como miles.
        assert!(peor < 900, "un salto de {peor}");
    }

    #[test]
    fn tras_el_sonido_el_silencio_es_silencio() {
        let mut e = nuevo(Modo::Orbita);
        for i in 0..4_800 {
            let x = if i % 100 < 50 { 20_000 } else { -20_000 };
            e.trama(&mut [x, x]);
        }
        let mut ultimo = [0, 0];
        for _ in 0..48_000 {
            let mut t = [0, 0];
            e.trama(&mut t);
            ultimo = t;
        }
        assert!(ultimo[0].abs() <= 1 && ultimo[1].abs() <= 1, "queda {ultimo:?}");
    }
}
