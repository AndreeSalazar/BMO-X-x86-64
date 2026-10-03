//! **LOS AVISOS: los sonidos del sistema, cortos y cada uno con su forma.**
//!
//! La `sonidoBMO()` de la maqueta. Son los que hacen que la musica de fondo
//! se AGACHE (`bmo_amplificador::agacha`): un aviso suena encima de la musica
//! y la musica se aparta para dejarlo oir.

use bmo_amplificador::{Ganancia, Limite, MilesimasDb};

use crate::sintesis::{inc_hz, inc_midi, Forma, Nota, Timbre, HZ};

/// **Que avisa.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aviso {
    /// La ciudad de neon se enciende: cinco notas que suben.
    Arranque,
    /// Un amigo te zumba: cinco golpes graves y cuadrados.
    Zumbido,
    /// Llega un mensaje: dos notas, como un toque en el hombro.
    Mensaje,
    /// Un amigo se conecta: un acorde que sube.
    Conecta,
    /// El juez borra algo: baja y se rompe.
    Juez,
    /// Una captura: el obturador.
    Captura,
    /// **La voz de BMO-X** (03-10): algo NO salio. Dos notas graves que
    /// caen, cerca y delante.
    Error,
    /// Ojo, pero sigue: una nota que cae un poco, delante a la derecha.
    Advertencia,
    /// Te pide un si o un no: sube, como una pregunta.
    Pregunta,
    /// Salio DE VERDAD: un acorde que se abre. Nunca antes de que salga.
    Hecho,
    /// Llega algo de fuera: sube, desde detras a la izquierda.
    Llega,
    /// Algo se va (la red): baja, hacia atras.
    SeVa,
}

/// Todos, en orden: lo que el banco guarda despues de la musica.
pub const AVISOS: [Aviso; 12] = [
    Aviso::Arranque,
    Aviso::Zumbido,
    Aviso::Mensaje,
    Aviso::Conecta,
    Aviso::Juez,
    Aviso::Captura,
    Aviso::Error,
    Aviso::Advertencia,
    Aviso::Pregunta,
    Aviso::Hecho,
    Aviso::Llega,
    Aviso::SeVa,
];

/// **Donde suena cada aviso** (grados, + derecha, 0 delante, 180 detras): la
/// pieza que lo dice tiene su sitio alrededor de la cabeza. Lo urgente,
/// delante; HERMES, a la izquierda (donde vive F3); lo que llega, de detras.
/// El orquestador lo situa con el 3D por voz (`AUDIO_FONDO_SITUAR`).
pub fn angulo(a: Aviso) -> i16 {
    match a {
        Aviso::Mensaje | Aviso::Zumbido | Aviso::Conecta => -50,
        Aviso::Advertencia => 30,
        Aviso::Llega => -140,
        Aviso::SeVa => 160,
        _ => 0,
    }
}

/// **El nivel de cada aviso**, para que TODOS lleguen a -6 dBFS de pico: el
/// zumbido son 92 Hz cuadrados y el mensaje dos senos agudos, y a la misma
/// ganancia uno sonaria la mitad que el otro. Medido por la prueba
/// `los_avisos_se_oyen_y_no_se_pasan`.
fn nivel(a: Aviso) -> MilesimasDb {
    match a {
        Aviso::Arranque => 998,
        Aviso::Zumbido => 2534,
        Aviso::Mensaje => 1613,
        Aviso::Conecta => 1690,
        Aviso::Juez => 2688,
        Aviso::Captura => 2611,
        Aviso::Error => 3010,
        Aviso::Advertencia => 2135,
        Aviso::Pregunta => 1855,
        Aviso::Hecho => 1710,
        Aviso::Llega => 2517,
        Aviso::SeVa => 2517,
    }
}

/// Cuantas notas lleva el aviso mas grande.
const MAX_NOTAS: usize = 6;

fn ms(m: u32) -> i64 {
    (HZ * m / 1000) as i64
}

/// Las notas de un aviso, y cuantas son. Volumenes de la maqueta en 1/256 dB
/// (0,2 / 0,16 / 0,25 / 0,22 / 0,18 / 0,1).
fn notas(a: Aviso) -> ([Nota; MAX_NOTAS], usize) {
    let mut n = [Nota::NADA; MAX_NOTAS];
    let mut k = 0;
    let mut poner = |x: Nota| {
        n[k] = x;
        k += 1;
    };
    match a {
        Aviso::Arranque => {
            for (j, x) in [0, 4, 7, 12, 16].into_iter().enumerate() {
                let dura = (1100 - j as u32 * 100) * HZ / 1000;
                poner(Nota::tono(ms(90) * j as i64, inc_midi(60 + x), dura, Timbre::Triangulo, -3579, 0));
            }
        }
        Aviso::Zumbido => {
            for j in 0..5u32 {
                poner(Nota::tono(ms(100) * j as i64, inc_hz(92 - j * 4), HZ * 80 / 1000, Timbre::Cuadrada, -4075, 0));
            }
        }
        Aviso::Mensaje => {
            poner(Nota::tono(0, inc_midi(76), HZ * 180 / 1000, Timbre::Seno, -3083, 0));
            poner(Nota::tono(ms(120), inc_midi(83), HZ * 300 / 1000, Timbre::Seno, -3367, 0));
        }
        Aviso::Conecta => {
            for (j, x) in [0, 4, 7].into_iter().enumerate() {
                poner(Nota::tono(ms(70) * j as i64, inc_midi(67 + x), HZ / 2, Timbre::Seno, -3813, 0));
            }
        }
        Aviso::Juez => {
            for (j, x) in [12, 7, 3, 0].into_iter().enumerate() {
                poner(Nota::tono(ms(70) * j as i64, inc_midi(60 + x), HZ * 120 / 1000, Timbre::Cuadrada, -5120, 0));
            }
            poner(Nota::golpe(ms(300), Forma::Caja, 0));
        }
        Aviso::Captura => {
            poner(Nota::golpe(0, Forma::Plato, 0));
            poner(Nota::golpe(ms(60), Forma::Caja, 0));
        }
        // La voz de BMO-X, igual que en la maqueta (`VOZ`): 0,13 / 0,2 / 0,22 / 0,18.
        Aviso::Error => {
            poner(Nota::tono(0, inc_midi(52), HZ * 160 / 1000, Timbre::Cuadrada, -4536, 0));
            poner(Nota::tono(ms(170), inc_midi(46), HZ * 280 / 1000, Timbre::Cuadrada, -4536, 0));
        }
        Aviso::Advertencia => {
            poner(Nota::tono(0, inc_midi(69), HZ * 120 / 1000, Timbre::Triangulo, -3579, 0));
            poner(Nota::tono(ms(100), inc_midi(67), HZ * 250 / 1000, Timbre::Triangulo, -3813, 0));
        }
        Aviso::Pregunta => {
            poner(Nota::tono(0, inc_midi(67), HZ * 140 / 1000, Timbre::Seno, -3367, 0));
            poner(Nota::tono(ms(130), inc_midi(72), HZ * 240 / 1000, Timbre::Seno, -3367, 0));
        }
        Aviso::Hecho => {
            for (j, x) in [0, 4, 7, 12].into_iter().enumerate() {
                poner(Nota::tono(ms(60) * j as i64, inc_midi(67 + x), (450 - j as u32 * 50) * HZ / 1000, Timbre::Seno, -3813, 0));
            }
        }
        Aviso::Llega => {
            for (j, x) in [55, 60, 67, 79].into_iter().enumerate() {
                poner(Nota::tono(ms(90) * j as i64, inc_midi(x), HZ * 200 / 1000, Timbre::Triangulo, -4075, 0));
            }
        }
        Aviso::SeVa => {
            for (j, x) in [72, 67, 64, 60].into_iter().enumerate() {
                poner(Nota::tono(ms(110) * j as i64, inc_midi(x), HZ * 260 / 1000, Timbre::Triangulo, -4075, 0));
            }
        }
    }
    (n, k)
}

/// Cuantas muestras mide un aviso: hasta que calla su ultima nota.
pub fn muestras(a: Aviso) -> usize {
    let (n, k) = notas(a);
    n[..k].iter().map(|x| x.inicio + x.largo as i64).max().unwrap_or(0) as usize
}

/// **Compone el aviso en `fuera`** desde el principio. Devuelve las muestras
/// escritas: el aviso entero, o lo que quepa.
pub fn componer(a: Aviso, fuera: &mut [i16]) -> usize {
    let (mut n, k) = notas(a);
    let largo = muestras(a).min(fuera.len());
    let nivel = Ganancia::db(nivel(a));
    let mut limite = Limite::inmediato(HZ);
    for (s, x) in fuera[..largo].iter_mut().enumerate() {
        let mut suma = 0i32;
        for nota in n[..k].iter_mut() {
            if nota.inicio <= s as i64 {
                suma = suma.saturating_add(nota.muestra());
            }
        }
        *x = limite.muestra(nivel.aplicar(suma));
    }
    largo
}
