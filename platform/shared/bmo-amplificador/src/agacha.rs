//! **EL AGACHE: la musica de fondo se aparta para dejar oir lo que avisa.**
//!
//! El propietario, el 2026-10-03: *"como si fuera fondo [...] integrado, que
//! relaja al usuario hasta que aparece una notificacion para avisar"*.
//!
//! Es lo que en una radio se llama *ducking* y en una mesa de mezcla, una
//! cadena lateral: cuando habla el locutor, la musica BAJA sola, y cuando
//! calla, VUELVE sola. Nadie mueve un fader. Aqui el "locutor" es un aviso (un
//! mensaje, un amigo que se conecta) o el juego que suena encima.
//!
//! ```text
//!    0 dB  ________                                 ____________
//!                  \  ATAQUE 30 ms                 /  RELAJO ~1,3 s
//!                   \_________ SOSTEN ____________/
//!   -15 dB              el aviso suena          ya no
//! ```
//!
//! # Por que asi y no de otra manera
//!
//! - **Baja deprisa y sube despacio.** Un aviso tiene que oirse desde su
//!   primera nota: 30 ms es bajar antes de que el oido lo separe del aviso. Y
//!   volver de golpe seria un segundo aviso que nadie pidio; mas de un segundo
//!   es "la musica vuelve", no "la musica salta".
//! - **En dB, no en factor.** Bajar 15 dB en linea recta de factor se oye como
//!   una caida al final; en dB se oye pareja, porque el oido mide asi.
//! - **Con sosten.** Dos avisos seguidos (un amigo que se conecta y te
//!   escribe) no hacen que la musica suba y baje entre los dos: se queda
//!   abajo [`SOSTEN_MS`] desde el ultimo.
//! - **En reposo es un cable**, bit a bit, como el maestro: sin aviso ni juego
//!   la musica pasa sin tocar.
//!
//! El factor se calcula UNA vez por bloque (con la misma tabla de dB que el
//! resto del crate) y se interpola trama a trama dentro del bloque: el
//! movimiento de 1 ms no tiene escalones.

use crate::{Ganancia, MilesimasDb, DB, MIN_DB};

/// **Lo que baja la musica bajo un aviso**: 15 dB. La musica sigue ahi --un
/// aviso no es un silencio--, a un tercio de lo que se oia.
pub const AGACHE_AVISO: MilesimasDb = -15 * DB;

/// **Lo que baja mientras el JUEGO suena**: 8 dB. Menos que un aviso: la
/// musica acompana la partida, no se va.
pub const AGACHE_JUEGO: MilesimasDb = -8 * DB;

/// Bajar: medio dB por milisegundo (15 dB en 30 ms).
pub const ATAQUE_DB_POR_MS: MilesimasDb = DB / 2;

/// Subir: 3/256 de dB por milisegundo (15 dB en ~1,3 s).
pub const RELAJO_DB_POR_MS: MilesimasDb = 3;

/// Lo que se queda abajo desde el ULTIMO aviso, aunque ya haya callado.
pub const SOSTEN_MS: u32 = 400;

/// Factor unidad en Q16.16.
const UNO: u32 = 1 << 16;

/// **El agache**: lo que esta bajado ahora, a donde va y cuanto aguanta.
#[derive(Clone, Copy, Debug)]
pub struct Agacha {
    hz: u32,
    /// Lo que suena ahora, en 1/256 dB (0 = nada bajado).
    actual: MilesimasDb,
    /// A donde va.
    objetivo: MilesimasDb,
    /// Tramas que el objetivo aguanta aunque el pedido suba.
    sosten: u32,
    /// El factor del final del bloque anterior: desde ahi se interpola.
    factor: u32,
}

impl Agacha {
    /// En reposo, a `hz` tramas por segundo.
    pub const fn nueva(hz: u32) -> Agacha {
        Agacha { hz: if hz == 0 { 48_000 } else { hz }, actual: 0, objetivo: 0, sosten: 0, factor: UNO }
    }

    /// Lo bajado ahora, en 1/256 dB (0 o negativo).
    pub fn actual(&self) -> MilesimasDb {
        self.actual
    }

    /// Sin nada bajado ni nada por bajar: la musica pasa sin tocar.
    pub fn en_reposo(&self) -> bool {
        self.actual == 0 && self.objetivo == 0 && self.factor == UNO
    }

    /// **Un bloque**: `pedido` es lo que manda ESTE bloque ([`AGACHE_AVISO`]
    /// si suena un aviso, [`AGACHE_JUEGO`] si suena el juego, `0` si nada), y
    /// `acc` la musica de fondo, intercalada con `canales` por trama.
    pub fn bloque(&mut self, pedido: MilesimasDb, acc: &mut [i32], canales: usize) {
        let canales = canales.max(1);
        let tramas = (acc.len() / canales) as u32;
        let pedido = pedido.clamp(MIN_DB, 0);
        let sosten = (SOSTEN_MS as u64 * self.hz as u64 / 1000) as u32;
        // Mas abajo, o igual de abajo: se toma ya y el sosten vuelve a contar.
        if pedido <= self.objetivo {
            self.objetivo = pedido;
            if pedido < 0 {
                self.sosten = sosten;
            }
        } else if self.sosten > tramas {
            self.sosten -= tramas;
        } else {
            self.sosten = 0;
            self.objetivo = pedido;
        }
        let paso = |por_ms: MilesimasDb| ((por_ms as i64 * tramas as i64 * 1000) / self.hz as i64).max(1) as i32;
        if self.actual > self.objetivo {
            self.actual = (self.actual - paso(ATAQUE_DB_POR_MS)).max(self.objetivo);
        } else if self.actual < self.objetivo {
            self.actual = (self.actual + paso(RELAJO_DB_POR_MS)).min(self.objetivo);
        }
        let nuevo = if self.actual == 0 { UNO } else { Ganancia::db(self.actual).factor_q16() };
        let previo = self.factor;
        self.factor = nuevo;
        if previo == UNO && nuevo == UNO {
            return;
        }
        if tramas == 0 {
            return;
        }
        let (p, n) = (previo as i64, nuevo as i64);
        for t in 0..tramas as usize {
            let f = p + (n - p) * (t as i64 + 1) / tramas as i64;
            for x in acc[t * canales..(t + 1) * canales].iter_mut() {
                *x = ((*x as i64 * f) >> 16) as i32;
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una continua a 10.000 en estereo, `ms` milisegundos a 48 kHz, pasada
    /// por el agache en bloques de 1 ms con el pedido que diga `pedido(ms)`.
    /// Devuelve el canal izquierdo, trama a trama.
    fn pasar(a: &mut Agacha, ms: usize, pedido: impl Fn(usize) -> MilesimasDb) -> std::vec::Vec<i32> {
        let mut fuera = std::vec::Vec::new();
        for m in 0..ms {
            let mut acc = [10_000i32; 96];
            a.bloque(pedido(m), &mut acc, 2);
            for t in 0..48 {
                fuera.push(acc[2 * t]);
            }
        }
        fuera
    }

    extern crate std;

    #[test]
    fn en_reposo_es_un_cable() {
        let mut a = Agacha::nueva(48_000);
        let s = pasar(&mut a, 50, |_| 0);
        assert!(s.iter().all(|&x| x == 10_000));
        assert!(a.en_reposo());
    }

    #[test]
    fn un_aviso_la_baja_en_30_ms_y_vuelve_sola() {
        let mut a = Agacha::nueva(48_000);
        // Un aviso de 200 ms y despues nada.
        let s = pasar(&mut a, 3000, |m| if m < 200 { AGACHE_AVISO } else { 0 });
        // A los 31 ms ya esta abajo del todo: -15 dB es ~0,178 de 10.000.
        let abajo = s[31 * 48];
        assert!((1700..=1850).contains(&abajo), "a los 31 ms: {abajo}");
        // Durante el aviso y el sosten (hasta 600 ms) se queda abajo.
        assert!(s[590 * 48] <= 1850, "en el sosten: {}", s[590 * 48]);
        // Y a los 3 s ha vuelto entera, sin pasarse.
        assert_eq!(*s.last().unwrap(), 10_000);
        assert!(s.iter().all(|&x| x <= 10_000));
        assert!(a.en_reposo());
        // Sin escalones: de una trama a la siguiente, menos de 1%.
        for w in s.windows(2) {
            assert!((w[0] - w[1]).abs() <= 100, "escalon de {} a {}", w[0], w[1]);
        }
    }

    #[test]
    fn dos_avisos_seguidos_no_la_hacen_bombear() {
        let mut a = Agacha::nueva(48_000);
        // Dos avisos de 100 ms separados por 250 ms de nada.
        let s = pasar(&mut a, 600, |m| if m < 100 || (350..450).contains(&m) { AGACHE_AVISO } else { 0 });
        // Entre los dos, el sosten la mantiene abajo: no sube.
        let entre = s[300 * 48];
        assert!(entre <= 1850, "entre los dos avisos subio a {entre}");
    }

    #[test]
    fn el_juego_la_baja_menos_que_un_aviso() {
        let mut a = Agacha::nueva(48_000);
        let s = pasar(&mut a, 100, |_| AGACHE_JUEGO);
        // -8 dB es ~0,398.
        let x = *s.last().unwrap();
        assert!((3900..=4050).contains(&x), "-8 dB: {x}");
        // Y si mientras tanto llega un aviso, baja mas.
        let s = pasar(&mut a, 100, |_| AGACHE_AVISO);
        assert!(*s.last().unwrap() < 1850);
    }
}
