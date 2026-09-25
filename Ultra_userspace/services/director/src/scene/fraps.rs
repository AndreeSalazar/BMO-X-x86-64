//! **LA CARA DE FRAPS-X**: el contador pintado. Cuando, donde y de quien lo
//! decide `desktop::fraps`; esto solo pinta lo que le dan, sobre lo que habia
//! debajo (`debajo`, guardado por quien lo pone).
//!
//! [consumo] NADA      pinta cuando `desktop::fraps` lo pone: una vez por
//!                     segundo como mucho (L6h)
//!
//! ```text
//!    +--------------------------------+
//!    |  68 FPS             FRAPS-X    |   el numero de FRAPS: grande y amarillo
//!    |  tid 7  14.7 ms  peor 22.0     |   a quien, el ultimo fotograma, el peor
//!    |  # BANCO  12 s  Ctrl+Shift+B   |   solo con el banco en marcha
//!    +--------------------------------+
//! ```
//!
//! El cristal es el de las ventanas: redondeado (`borde::dentro`), oscuro y
//! dejando ver lo de detras, con una orilla de 1 px del acento.

use bmo_userland as bmo;

use super::borde::dentro;
use super::globo::mezcla;

/// Lo que mide la caja.
pub(crate) const ANCHO: u32 = 250;
const ALTO_SIN_BANCO: u32 = 78;
const ALTO_CON_BANCO: u32 = 96;
/// Lo mas que puede tapar: es lo que tiene que poder guardar quien la pone.
pub(crate) const GUARDADO: usize = (ANCHO * ALTO_CON_BANCO) as usize;
const RADIO: u32 = 8;
/// El amarillo de FRAPS: el numero que todo el mundo reconoce.
const AMARILLO: u32 = 0x00FF_E600;
/// Lo que oscurece el cristal, sobre 256. Casi opaco: con 200 las letras de
/// una ventana de detras se leian a traves y ensuciaban el numero.
const CRISTAL: u32 = 236;

/// Lo alto de la caja, con o sin la linea del banco.
pub(crate) const fn alto(banco: bool) -> u32 {
    if banco { ALTO_CON_BANCO } else { ALTO_SIN_BANCO }
}

/// **Lo que se muestra.**
pub(crate) struct Vista<'a> {
    /// FPS en decimas; `None` hasta que se cierra el primer segundo.
    pub fps10: Option<u32>,
    /// A quien se mide: `tid 7`, `escritorio`.
    pub que: &'a [u8],
    pub ultimo_us: u32,
    pub peor_us: u32,
    /// Los segundos del banco, si hay uno en marcha.
    pub banco_s: Option<u32>,
    /// Los colores del tema: texto, tenue, el del banco y la orilla.
    pub tinta: (u32, u32, u32, u32),
}

/// **Pinta el contador** en `caja`, mezclando con `debajo` (sus pixeles,
/// fila a fila, tal como estaban).
pub(crate) fn pintar(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), debajo: &[u32], v: &Vista) {
    let (texto, tenue, mala, orilla) = v.tinta;
    p.marcar(x, y, w, h);
    let mut k = 0usize;
    for dy in 0..h {
        for dx in 0..w {
            let (px, py) = (x + dx, y + dy);
            let d = debajo.get(k).copied().unwrap_or(0);
            k += 1;
            if !dentro(px, py, x, y, w, h, RADIO) {
                continue;
            }
            let borde = !dentro(px, py, x + 1, y + 1, w - 2, h - 2, RADIO - 1);
            let c = if borde { mezcla(d, orilla, 170) } else { mezcla(d, 0x0006_0610, CRISTAL) };
            p.punto_ya_marcado(px, py, c);
        }
    }

    // El numero, grande y amarillo. `--` hasta que se cierra el primer segundo.
    let mut n = Renglon::new();
    match v.fps10 {
        Some(f) => n.num(((f + 5) / 10) as u64),
        None => n.pon(b"--"),
    }
    let num = core::str::from_utf8(n.bytes()).unwrap_or("--");
    let fin = p.texto_escala(x + 12, y + 8, num, AMARILLO, 3);
    p.texto_bytes(fin + 6, y + 8 + 2 * bmo::GLIFO_ALTO - 2, b"FPS", AMARILLO);
    p.texto_bytes(x + w - 7 * bmo::GLIFO_ANCHO - 12, y + 10, b"FRAPS-X", tenue);

    // Debajo: a quien, cuanto dura un fotograma, y el peor del segundo.
    let mut l = Renglon::new();
    l.pon(v.que);
    if v.fps10.is_some() {
        l.pon(b"  ");
        l.ms(v.ultimo_us);
        l.pon(b" ms  peor ");
        l.ms(v.peor_us);
    }
    let y2 = y + 8 + 3 * bmo::GLIFO_ALTO + 4;
    p.texto_bytes(x + 12, y2, l.bytes(), texto);

    if let Some(s) = v.banco_s {
        let mut l = Renglon::new();
        l.pon(b"BANCO  ");
        l.num(s as u64);
        l.pon(b" s  Ctrl+Shift+B");
        let y3 = y2 + bmo::GLIFO_ALTO + 2;
        p.rect(x + 12, y3 + 4, 8, 8, mala);
        p.texto_bytes(x + 26, y3, l.bytes(), mala);
    }
}

/// Un renglon sin memoria dinamica.
struct Renglon {
    b: [u8; 48],
    n: usize,
}

impl Renglon {
    fn new() -> Self {
        Renglon { b: [0; 48], n: 0 }
    }
    fn pon(&mut self, s: &[u8]) {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn num(&mut self, v: u64) {
        let mut d = [0u8; 20];
        let mut k = 0;
        let mut x = v;
        loop {
            d[k] = b'0' + (x % 10) as u8;
            k += 1;
            x /= 10;
            if x == 0 {
                break;
            }
        }
        while k > 0 {
            k -= 1;
            self.pon(&[d[k]]);
        }
    }
    /// Microsegundos como milisegundos con un decimal: `16.6`.
    fn ms(&mut self, us: u32) {
        self.num((us / 1000) as u64);
        self.pon(b".");
        self.pon(&[b'0' + (us / 100 % 10) as u8]);
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}
