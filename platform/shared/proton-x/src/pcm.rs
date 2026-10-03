//! **El PCM de un juego, al del audifono** (03-10, N4.5): lo que WASAPI le
//! deja escribir al `.exe`, pasado a lo que come el tubo de BMO-X.
//!
//! [carril]  VERDE     cuentas sobre bytes; no toca la maquina
//! [cuesta]  DATO      una cuenta mal hecha se OYE: ruido, un lado mudo, o
//!                     la musica una octava mas aguda
//! [riesgo]  ESPEJO    el formato lo dice el `.exe` con un WAVEFORMATEX(TENSIBLE)
//!                     de Windows (mmreg.h, ksmedia.h); el banco lo prueba con
//!                     los bytes que escribe Windows
//! [consumo] NADA      solo cuando el juego suelta un trozo de sonido
//!
//! ```text
//!    entra   lo que pida el juego: float 32, o entero de 16, 24 o 32 bits;
//!            1 a 8 canales (con su mascara: 5.1, 7.1...); su frecuencia
//!    sale    lo del tubo: entero de 16 bits, ESTEREO, a la frecuencia del
//!            audifono (48.000 Hz en esta maquina)
//!    canales los de delante tal cual; el centro y los de atras/lado, al
//!            -3 dB a cada lado (lo que hace Windows al bajar a estereo); el
//!            subwoofer (LFE) no va a unos auriculares
//!    hz      interpolacion lineal entre dos muestras: basta para pasar de
//!            44.100 a 48.000 sin que se oiga, y no inventa nada
//! ```

use alloc::vec::Vec;

/// `WAVE_FORMAT_PCM`, `WAVE_FORMAT_IEEE_FLOAT` y `WAVE_FORMAT_EXTENSIBLE`.
pub const PCM: u16 = 1;
pub const FLOTANTE: u16 = 3;
pub const EXTENSIBLE: u16 = 0xFFFE;

/// Los SubFormat de KSDATAFORMAT: el tipo de antes (PCM o float) en los
/// cuatro primeros bytes, y el resto, el GUID de siempre.
const SUBFORMATO_COLA: [u8; 12] = [0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71];

/// `SPEAKER_*` de ksmedia.h: el bit de cada altavoz, en el orden en que
/// van los canales dentro de un fotograma.
pub const FL: u32 = 0x1;
pub const FR: u32 = 0x2;
pub const FC: u32 = 0x4;
pub const LFE: u32 = 0x8;
pub const BL: u32 = 0x10;
pub const BR: u32 = 0x20;
pub const FLC: u32 = 0x40;
pub const FRC: u32 = 0x80;
pub const BC: u32 = 0x100;
pub const SL: u32 = 0x200;
pub const SR: u32 = 0x400;

/// -3 dB: lo que pesa un canal que se reparte entre los dos lados.
const MEDIO: f32 = core::f32::consts::FRAC_1_SQRT_2;

/// **Un formato de sonido**, leido de un WAVEFORMATEX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formato {
    pub hz: u32,
    pub canales: u16,
    /// Bits por muestra (en un EXTENSIBLE, los del contenedor).
    pub bits: u16,
    pub flotante: bool,
    /// Bytes de un fotograma (todas las muestras de un instante).
    pub alinea: u16,
    /// A que altavoz va cada canal (`SPEAKER_*`); 0 = el de siempre para
    /// esos canales.
    pub mascara: u32,
}

/// La mascara de siempre para `n` canales (la que Windows supone sin
/// EXTENSIBLE).
pub fn mascara_de(n: u16) -> u32 {
    match n {
        1 => FC,
        2 => FL | FR,
        3 => FL | FR | FC,
        4 => FL | FR | BL | BR,
        5 => FL | FR | FC | BL | BR,
        6 => FL | FR | FC | LFE | BL | BR,
        7 => FL | FR | FC | LFE | BC | SL | SR,
        8 => FL | FR | FC | LFE | BL | BR | SL | SR,
        _ => 0,
    }
}

fn u16_en(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

impl Formato {
    /// **El de esta maquina** que da `GetMixFormat`: float de 32 bits,
    /// estereo, a `hz`. Es lo que Windows da en modo compartido, y lo que un
    /// motor de sonido (Wwise) espera.
    pub const fn mezcla(hz: u32) -> Formato {
        Formato { hz, canales: 2, bits: 32, flotante: true, alinea: 8, mascara: FL | FR }
    }

    /// **Leer un WAVEFORMATEX** (18 bytes y lo que diga `cbSize`): `None`
    /// si no es PCM ni float, o no cuadra.
    pub fn de_waveformatex(b: &[u8]) -> Option<Formato> {
        let (etiqueta, canales, hz) = (u16_en(b, 0)?, u16_en(b, 2)?, u32_en(b, 4)?);
        let (alinea, bits) = (u16_en(b, 12)?, u16_en(b, 14)?);
        let (flotante, mascara) = match etiqueta {
            PCM => (false, 0),
            FLOTANTE => (true, 0),
            EXTENSIBLE => {
                // cbSize +16 (>= 22), Samples +18, dwChannelMask +20,
                // SubFormat +24.
                if u16_en(b, 16)? < 22 {
                    return None;
                }
                let sub = b.get(24..40)?;
                if sub[4..] != SUBFORMATO_COLA || sub[2..4] != [0, 0] {
                    return None;
                }
                match u16::from_le_bytes([sub[0], sub[1]]) {
                    PCM => (false, u32_en(b, 20)?),
                    FLOTANTE => (true, u32_en(b, 20)?),
                    _ => return None,
                }
            }
            _ => return None,
        };
        let f = Formato { hz, canales, bits, flotante, alinea, mascara };
        f.valido().then_some(f)
    }

    fn valido(&self) -> bool {
        let bits_ok = if self.flotante { self.bits == 32 } else { matches!(self.bits, 8 | 16 | 24 | 32) };
        bits_ok && (1..=8).contains(&self.canales) && (8_000..=384_000).contains(&self.hz) && self.alinea as u32 == self.canales as u32 * self.bits as u32 / 8
    }

    /// **Escribir el WAVEFORMATEXTENSIBLE** (40 bytes) de este formato.
    pub fn a_waveformatextensible(&self) -> [u8; 40] {
        let mut b = [0u8; 40];
        let mut pon = |o: usize, v: &[u8]| b[o..o + v.len()].copy_from_slice(v);
        pon(0, &EXTENSIBLE.to_le_bytes());
        pon(2, &self.canales.to_le_bytes());
        pon(4, &self.hz.to_le_bytes());
        pon(8, &(self.hz * self.alinea as u32).to_le_bytes());
        pon(12, &self.alinea.to_le_bytes());
        pon(14, &self.bits.to_le_bytes());
        pon(16, &22u16.to_le_bytes());
        pon(18, &self.bits.to_le_bytes());
        pon(20, &self.mascara_real().to_le_bytes());
        pon(24, &(if self.flotante { FLOTANTE } else { PCM }).to_le_bytes());
        pon(28, &SUBFORMATO_COLA);
        b
    }

    /// La mascara que manda: la dicha, o la de siempre.
    pub fn mascara_real(&self) -> u32 {
        if self.mascara.count_ones() == self.canales as u32 {
            self.mascara
        } else {
            mascara_de(self.canales)
        }
    }

    /// La muestra `k` del fotograma `f`, de -1 a 1.
    fn muestra(&self, f: &[u8], k: usize) -> f32 {
        let n = self.bits as usize / 8;
        let Some(b) = f.get(k * n..k * n + n) else { return 0.0 };
        match (self.flotante, n) {
            (true, _) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            (false, 1) => (b[0] as f32 - 128.0) / 128.0,
            (false, 2) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
            (false, 3) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (false, _) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        }
    }

    /// **Un fotograma, a estereo** (izquierda, derecha).
    pub fn estereo(&self, f: &[u8]) -> [f32; 2] {
        if self.canales == 1 {
            let m = self.muestra(f, 0);
            return [m, m];
        }
        let mascara = self.mascara_real();
        let (mut i, mut d) = (0.0f32, 0.0f32);
        let mut k = 0;
        for bit in 0..18 {
            let altavoz = 1u32 << bit;
            if mascara & altavoz == 0 {
                continue;
            }
            let m = self.muestra(f, k);
            k += 1;
            match altavoz {
                FL | FLC => i += m,
                FR | FRC => d += m,
                FC | BC => {
                    i += MEDIO * m;
                    d += MEDIO * m;
                }
                BL | SL => i += MEDIO * m,
                BR | SR => d += MEDIO * m,
                // LFE y los de arriba: no van a unos auriculares.
                _ => {}
            }
            if k == self.canales as usize {
                break;
            }
        }
        [i, d]
    }
}

/// Una muestra de -1 a 1, a entero de 16 bits (recortada).
fn a_s16(x: f32) -> i16 {
    let y = x * 32767.0;
    if y >= 32767.0 {
        32767
    } else if y <= -32768.0 {
        -32768
    } else if y >= 0.0 {
        (y + 0.5) as i16
    } else {
        (y - 0.5) as i16
    }
}

/// **El conversor de un flujo**: guarda el ultimo fotograma y por donde va
/// entre dos, para que un trozo siga donde acabo el anterior.
#[derive(Debug, Clone)]
pub struct Conversor {
    pub de: Formato,
    pub a_hz: u32,
    /// El ultimo fotograma ya en estereo (el "anterior" de la interpolacion).
    previo: [f32; 2],
    /// Entre el previo y el siguiente: de 0 a 1, en pasos de `de.hz / a_hz`.
    fase: f64,
    empezado: bool,
}

impl Conversor {
    pub fn nuevo(de: Formato, a_hz: u32) -> Conversor {
        Conversor { de, a_hz, previo: [0.0; 2], fase: 0.0, empezado: false }
    }

    /// **Convertir** `entrada` (fotogramas enteros del formato del juego) y
    /// AGREGAR lo que sale a `salida` (s16 estereo, little-endian).
    pub fn convertir(&mut self, entrada: &[u8], salida: &mut Vec<u8>) {
        let al = self.de.alinea as usize;
        if al == 0 {
            return;
        }
        let paso = self.de.hz as f64 / self.a_hz.max(1) as f64;
        for f in entrada.chunks_exact(al) {
            let nuevo = self.de.estereo(f);
            if !self.empezado {
                // El primero solo arma el "anterior": cada fotograma sale
                // cuando llega el siguiente (un fotograma de retraso: a
                // 48 kHz, 21 microsegundos).
                self.previo = nuevo;
                self.empezado = true;
                continue;
            }
            // Mientras la fase cae entre el previo y este, sale una muestra.
            while self.fase < 1.0 {
                let t = self.fase as f32;
                for c in 0..2 {
                    let v = self.previo[c] + (nuevo[c] - self.previo[c]) * t;
                    salida.extend_from_slice(&a_s16(v).to_le_bytes());
                }
                self.fase += paso;
            }
            self.fase -= 1.0;
            self.previo = nuevo;
        }
    }

    /// Cuantos fotogramas de SALIDA dan `n` de entrada (para el reloj).
    pub fn salida_de(&self, n: u64) -> u64 {
        n * self.a_hz as u64 / self.de.hz.max(1) as u64
    }

    /// Y al reves: cuantos de entrada son `n` de salida.
    pub fn entrada_de(&self, n: u64) -> u64 {
        n * self.de.hz as u64 / self.a_hz.max(1) as u64
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un WAVEFORMATEXTENSIBLE como lo escribe Windows para su mezcla:
    /// float, estereo, 48 kHz (los 40 bytes de `GetMixFormat` en un PC).
    const MEZCLA_DE_WINDOWS: [u8; 40] = [
        0xFE, 0xFF, 0x02, 0x00, 0x80, 0xBB, 0x00, 0x00, 0x00, 0xDC, 0x05, 0x00, 0x08, 0x00, 0x20, 0x00, 0x16, 0x00, 0x20, 0x00, 0x03, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
    ];

    #[test]
    fn la_mezcla_se_escribe_como_windows_y_se_lee() {
        assert_eq!(Formato::mezcla(48_000).a_waveformatextensible(), MEZCLA_DE_WINDOWS);
        assert_eq!(Formato::de_waveformatex(&MEZCLA_DE_WINDOWS), Some(Formato::mezcla(48_000)));
        // Un PCM de 16 bits a secas (18 bytes, cbSize 0).
        let mut pcm = [0u8; 18];
        pcm[0] = 1;
        pcm[2] = 2;
        pcm[4..8].copy_from_slice(&44_100u32.to_le_bytes());
        pcm[12] = 4;
        pcm[14] = 16;
        let f = Formato::de_waveformatex(&pcm).unwrap();
        assert_eq!((f.hz, f.canales, f.bits, f.flotante, f.mascara_real()), (44_100, 2, 16, false, FL | FR));
        // Lo que no cuadra: un alinea que no es canales x bits.
        pcm[12] = 3;
        assert_eq!(Formato::de_waveformatex(&pcm), None);
    }

    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn s16s(b: &[u8]) -> Vec<i16> {
        b.chunks_exact(2).map(|x| i16::from_le_bytes([x[0], x[1]])).collect()
    }

    #[test]
    fn a_la_misma_frecuencia_sale_cada_fotograma_tal_cual() {
        let mut c = Conversor::nuevo(Formato::mezcla(48_000), 48_000);
        let mut s = Vec::new();
        c.convertir(&f32s(&[0.5, -0.5, 1.0, -1.0, 2.0, 0.0]), &mut s);
        assert_eq!(s16s(&s), [16384, -16384, 32767, -32767], "el ultimo espera al siguiente");
        c.convertir(&f32s(&[0.0, 0.0]), &mut s);
        assert_eq!(s16s(&s)[4..], [32767, 0], "y 2.0 se recorta");
    }

    #[test]
    fn el_5_1_baja_a_estereo_sin_el_subwoofer() {
        // FL FR FC LFE BL BR
        let f = Formato { hz: 48_000, canales: 6, bits: 32, flotante: true, alinea: 24, mascara: mascara_de(6) };
        let [i, d] = f.estereo(&f32s(&[0.1, 0.2, 0.4, 1.0, 0.3, 0.0]));
        assert!((i - (0.1 + MEDIO * 0.4 + MEDIO * 0.3)).abs() < 1e-6, "{i}");
        assert!((d - (0.2 + MEDIO * 0.4)).abs() < 1e-6, "{d}");
        // Mono: a los dos lados igual.
        let m = Formato { hz: 48_000, canales: 1, bits: 16, flotante: false, alinea: 2, mascara: 0 };
        assert_eq!(m.estereo(&16384i16.to_le_bytes()), [0.5, 0.5]);
    }

    #[test]
    fn de_44100_a_48000_salen_las_que_tocan_y_siguen_entre_trozos() {
        let f = Formato { hz: 44_100, canales: 2, bits: 16, flotante: false, alinea: 4, mascara: 0 };
        let mut c = Conversor::nuevo(f, 48_000);
        // Un segundo, en dos trozos que no caen en fotograma redondo de salida.
        let trozo: Vec<u8> = (0..22_050).flat_map(|_| [0u8, 0x40, 0, 0x40]).collect();
        let mut s = Vec::new();
        c.convertir(&trozo, &mut s);
        c.convertir(&trozo, &mut s);
        let n = s.len() / 4;
        assert!((47_995..=48_000).contains(&n), "{n} fotogramas por segundo");
        // Un tono constante sigue constante (la interpolacion no inventa).
        assert!(s16s(&s).iter().all(|&x| x == 16384));
        assert_eq!(c.salida_de(44_100), 48_000);
        assert_eq!(c.entrada_de(48_000), 44_100);
    }
}
