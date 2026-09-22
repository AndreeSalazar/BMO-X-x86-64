//! **LAS VOCES DEL ORQUESTADOR: la app DECLARA sus sonidos, el orquestador
//! los toca.**
//!
//! El propietario, el 2026-09-22, despues de tres arreglos seguidos a los
//! tirones de DOOM: *"mejora mas, pero mas elegante... por completo, por algo
//! es BMO: Bare Metal ORQUESTADOR. Orquesta, para facilitar"*.
//!
//! # Lo que estaba mal no era DOOM, era el REPARTO
//!
//! Hasta hoy una app que queria sonar mandaba PCM ya mezclado a un anillo y
//! tenia que ir SIEMPRE 100 ms por delante del aparato. Si se atascaba --DOOM
//! derritiendo la pantalla, cargando un mapa--, el aparato se quedaba sin nada
//! y sonaba un corte. Cada app tenia que acordarse de llegar a tiempo, y la que
//! no llegara, cortaba. Eso es MULTIPLEXAR: el sistema espera a que la app
//! llegue.
//!
//! **Orquestar es al reves: el que marca el tiempo es el orquestador.** La app
//! dice *"toca este sonido, a este volumen, a este lado"* y se olvida. El
//! kernel mezcla las voces cada milisegundo, en el hilo del bus, justo antes
//! del maestro. Es lo que hacian las tarjetas con voces propias (la Gravis
//! UltraSound, la AWE32 -- que DOOM ya nombra en su lista de aparatos).
//!
//! ```text
//!                          PCM (el anillo)          VOCES (esto)
//!    quien marca el paso   la app                   el orquestador
//!    si la app se atasca   CORTE                    sigue sonando
//!    del disparo al ruido  hasta 100 ms             lo que tarde la trama: 1-8 ms
//!    lo que la app hace    mezclar, remuestrear,    tocar, ajustar, callar
//!                          llevar la cuenta
//! ```
//!
//! El anillo PCM no se va: es para lo que es un FLUJO (una cancion, un MP3,
//! la antena). Las voces son para lo que es un SONIDO: empieza, dura y acaba.
//!
//! # El banco, y por que las muestras no viajan
//!
//! La app presta UN bloque suyo --el banco-- con todas sus muestras dentro, y
//! cada voz dice *"de aqui a aqui del banco"*. Asi tocar un disparo es una
//! puerta con seis numeros, no una copia del disparo. Y el banco se juzga
//! UNA VEZ al prestarlo: cada voz que se pida se comprueba contra su medida
//! ANTES de sonar, y al mezclar cada muestra se lee con `get` -- un banco que
//! encogiera por debajo de una voz la calla, no la desborda.
//!
//! # Lo que cada voz lleva, y por que la PISTA
//!
//! `pista` es un numero que la mezcla de hoy no usa y LA MESA si
//! (`PLAN_LA_MESA.md`, M3): es por donde un juego EXPONE sus subcategorias
//! --armas, pasos, voces-- sin que el orquestador tenga que adivinarlas. Va
//! desde el primer dia en el contrato para que el dia que la mesa llegue no
//! haya que cambiar la puerta.

/// Cuantas voces suenan a la vez. DOOM usa ocho; el doble deja sitio a quien
/// venga despues sin que el mezclador se vuelva un bucle largo (16 voces x 48
/// muestras por trama = 768 sumas por milisegundo).
pub const MAX_VOCES: usize = 16;

/// La unidad del volumen de una voz: `256` es tal cual.
pub const PLENO_VOZ: u16 = 256;

/// Como estan guardadas las muestras de una voz en el banco. Siempre MONO: el
/// lado lo pone el paneo de la voz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    /// 8 bits SIN signo, `128` es el silencio. Lo que traen los WAD de DOOM.
    U8,
    /// 16 bits con signo, little endian.
    S16,
}

impl Formato {
    /// Desde el numero de la puerta: `0` U8, `1` S16.
    pub fn de(n: u64) -> Option<Formato> {
        match n {
            0 => Some(Formato::U8),
            1 => Some(Formato::S16),
            _ => None,
        }
    }

    fn bytes(self) -> u64 {
        match self {
            Formato::U8 => 1,
            Formato::S16 => 2,
        }
    }
}

/// **Un sonido**: de donde sale en el banco y como tiene que sonar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sonido {
    /// Donde empieza, en BYTES desde el principio del banco.
    pub inicio: u32,
    /// Cuantas MUESTRAS tiene (no bytes).
    pub muestras: u32,
    pub formato: Formato,
    /// La frecuencia a la que se grabo: 11.025 los de DOOM.
    pub hz: u32,
    /// Volumen de cada lado, `0..=256`. El paneo es la diferencia.
    pub izq: u16,
    pub der: u16,
    /// A que pista de LA MESA pertenece. Hoy no cambia la mezcla.
    pub pista: u8,
}

impl Sonido {
    /// El sonido de nada: lo que hay en una voz que no suena.
    pub const NADA: Sonido =
        Sonido { inicio: 0, muestras: 0, formato: Formato::U8, hz: 0, izq: 0, der: 0, pista: 0 };
}

/// **Por que el orquestador dijo que no.** Se dice con nombre: un sonido que
/// no suena sin motivo es un sonido que parece del aparato.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rechazo {
    /// El canal no existe (hay [`MAX_VOCES`]).
    Canal,
    /// Cero muestras: nada que tocar.
    Vacio,
    /// La frecuencia no tiene sentido (fuera de 1 kHz .. 192 kHz).
    Frecuencia,
    /// El sonido se sale del banco.
    FueraDelBanco,
}

#[derive(Clone, Copy, Debug)]
struct Voz {
    sonido: Sonido,
    /// Por donde va, en 16.16 sobre las muestras del sonido.
    pos: u64,
    /// Cuanto avanza por cada muestra de SALIDA, en 16.16.
    paso: u64,
    activa: bool,
}

impl Voz {
    const CALLADA: Voz = Voz { sonido: Sonido::NADA, pos: 0, paso: 0, activa: false };
}

/// **Las voces**: el estado entero del mezclador. Sin asignador, sin nada
/// fuera: dieciseis voces y sus cuentas.
#[derive(Clone, Copy, Debug)]
pub struct Voces {
    voz: [Voz; MAX_VOCES],
}

impl Default for Voces {
    fn default() -> Self {
        Voces::nuevas()
    }
}

impl Voces {
    /// Todas calladas. `const` para que el kernel la tenga en un `static`.
    pub const fn nuevas() -> Voces {
        Voces { voz: [Voz::CALLADA; MAX_VOCES] }
    }

    /// **Tocar** `s` en `canal`, desde el principio. Si el canal ya sonaba, el
    /// sonido nuevo lo sustituye: es lo que DOOM espera de un canal.
    ///
    /// `bytes_banco` es la medida del banco AHORA, y `hz_salida` la del
    /// aparato: la cuenta del remuestreo sale de las dos.
    pub fn tocar(&mut self, canal: usize, s: Sonido, bytes_banco: u64, hz_salida: u32) -> Result<(), Rechazo> {
        comprobar(canal, &s, bytes_banco, hz_salida)?;
        let mut s = s;
        s.izq = s.izq.min(PLENO_VOZ);
        s.der = s.der.min(PLENO_VOZ);
        self.voz[canal] = Voz {
            sonido: s,
            pos: 0,
            paso: ((s.hz as u64) << 16) / hz_salida as u64,
            activa: true,
        };
        Ok(())
    }

    /// **Mover** una voz que suena: volumen y lado. No la reinicia.
    pub fn ajustar(&mut self, canal: usize, izq: u16, der: u16) {
        if let Some(v) = self.voz.get_mut(canal) {
            v.sonido.izq = izq.min(PLENO_VOZ);
            v.sonido.der = der.min(PLENO_VOZ);
        }
    }

    /// **Callar** una voz.
    pub fn callar(&mut self, canal: usize) {
        if let Some(v) = self.voz.get_mut(canal) {
            v.activa = false;
        }
    }

    /// Callar todas: el banco cambio o su propietario se fue.
    pub fn callar_todas(&mut self) {
        for v in self.voz.iter_mut() {
            v.activa = false;
        }
    }

    /// Las que suenan, en un mapa de bits (bit `n` = canal `n`).
    pub fn activas(&self) -> u32 {
        let mut m = 0u32;
        for (i, v) in self.voz.iter().enumerate() {
            if v.activa {
                m |= 1 << i;
            }
        }
        m
    }

    /// Suena alguna?
    pub fn hay(&self) -> bool {
        self.voz.iter().any(|v| v.activa)
    }

    /// La pista de un canal, para quien quiera contar por pistas.
    pub fn pista(&self, canal: usize) -> Option<u8> {
        self.voz.get(canal).filter(|v| v.activa).map(|v| v.sonido.pista)
    }

    /// **Mezclar**: SUMA todas las voces en `acc` (intercalado, `canales` por
    /// trama), sin pisar lo que ya hubiera -- asi el PCM del anillo y las
    /// voces suenan juntos. Remuestrea con la RECTA entre dos muestras (un
    /// escalon por muestra son agudos que el sonido no tenia).
    ///
    /// Una voz que se acaba se calla sola; una muestra que no esta en el
    /// banco la calla tambien, en vez de leer fuera.
    pub fn mezclar(&mut self, banco: &[u8], acc: &mut [i32], canales: usize) {
        let canales = canales.max(1);
        let tramas = acc.len() / canales;
        for v in self.voz.iter_mut() {
            if !v.activa {
                continue;
            }
            let s = v.sonido;
            for t in 0..tramas {
                let idx = (v.pos >> 16) as u32;
                if idx >= s.muestras {
                    v.activa = false;
                    break;
                }
                let Some(a) = muestra(banco, &s, idx) else {
                    v.activa = false;
                    break;
                };
                // La ultima va hacia el silencio: el sonido se acaba ahi.
                let b = if idx + 1 < s.muestras { muestra(banco, &s, idx + 1).unwrap_or(0) } else { 0 };
                let frac = (v.pos & 0xFFFF) as i64;
                let m = a + (((b - a) as i64 * frac) >> 16) as i32;
                let base = t * canales;
                if canales >= 2 {
                    acc[base] += (m * s.izq as i32) >> 8;
                    acc[base + 1] += (m * s.der as i32) >> 8;
                } else {
                    acc[base] += (m * ((s.izq as i32 + s.der as i32) / 2)) >> 8;
                }
                v.pos += v.paso;
            }
        }
    }
}

/// **EL JUEZ de una voz**, aparte de tocarla: el kernel lo usa DOS veces --en
/// el syscall, para contestar que no en el acto y con motivo, y en el hilo del
/// bus, al aplicarla-- y tiene que ser el mismo juez las dos veces. Dos
/// comprobaciones escritas por separado acaban discrepando.
pub fn comprobar(canal: usize, s: &Sonido, bytes_banco: u64, hz_salida: u32) -> Result<(), Rechazo> {
    if canal >= MAX_VOCES {
        return Err(Rechazo::Canal);
    }
    if s.muestras == 0 {
        return Err(Rechazo::Vacio);
    }
    if s.hz < 1_000 || s.hz > 192_000 || hz_salida == 0 {
        return Err(Rechazo::Frecuencia);
    }
    let fin = s.inicio as u64 + s.muestras as u64 * s.formato.bytes();
    if fin > bytes_banco {
        return Err(Rechazo::FueraDelBanco);
    }
    Ok(())
}

/// Una muestra de un sonido, ya en 16 bits con signo. `None` si no esta en el
/// banco (un banco que encogio): quien la pide calla la voz.
fn muestra(banco: &[u8], s: &Sonido, idx: u32) -> Option<i32> {
    match s.formato {
        Formato::U8 => {
            let b = *banco.get(s.inicio as usize + idx as usize)?;
            Some((b as i32 - 128) << 8)
        }
        Formato::S16 => {
            let i = s.inicio as usize + idx as usize * 2;
            let lo = *banco.get(i)?;
            let hi = *banco.get(i + 1)?;
            Some(i16::from_le_bytes([lo, hi]) as i32)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un banco con un diente de sierra U8 de 100 muestras en 0 y un S16 de
    /// 10 muestras en 200.
    fn banco() -> [u8; 220] {
        let mut b = [128u8; 220];
        for i in 0..100 {
            b[i] = (128 + i) as u8;
        }
        for i in 0..10 {
            let v = (i as i16) * 1000;
            let [lo, hi] = v.to_le_bytes();
            b[200 + 2 * i] = lo;
            b[200 + 2 * i + 1] = hi;
        }
        b
    }

    fn sierra() -> Sonido {
        Sonido { inicio: 0, muestras: 100, formato: Formato::U8, hz: 12_000, izq: 256, der: 256, pista: 3 }
    }

    #[test]
    fn lo_que_se_sale_del_banco_no_suena() {
        let mut v = Voces::nuevas();
        let mut s = sierra();
        s.inicio = 150;
        assert_eq!(v.tocar(0, s, 220, 48_000), Err(Rechazo::FueraDelBanco));
        assert_eq!(v.tocar(MAX_VOCES, sierra(), 220, 48_000), Err(Rechazo::Canal));
        let mut s = sierra();
        s.muestras = 0;
        assert_eq!(v.tocar(0, s, 220, 48_000), Err(Rechazo::Vacio));
        let mut s = sierra();
        s.hz = 5;
        assert_eq!(v.tocar(0, s, 220, 48_000), Err(Rechazo::Frecuencia));
        assert!(!v.hay());
    }

    #[test]
    fn una_voz_dura_lo_que_dura_y_se_calla_sola() {
        let mut v = Voces::nuevas();
        let b = banco();
        v.tocar(0, sierra(), 220, 48_000).unwrap();
        // 100 muestras a 12 kHz sobre 48 kHz son 400 tramas de salida.
        let mut tramas = 0;
        while v.hay() {
            let mut acc = [0i32; 96];
            v.mezclar(&b, &mut acc, 2);
            tramas += 48;
            assert!(tramas < 1000, "no se calla");
        }
        assert!((400..=448).contains(&tramas), "{} tramas", tramas);
    }

    #[test]
    fn la_recta_sube_sin_escalones() {
        let mut v = Voces::nuevas();
        let b = banco();
        v.tocar(0, sierra(), 220, 48_000).unwrap();
        let mut acc = [0i32; 96];
        v.mezclar(&b, &mut acc, 2);
        // La sierra sube: cada trama igual o por encima de la anterior, y NO
        // cuatro tramas iguales seguidas (eso era el escalon).
        let izq: [i32; 48] = core::array::from_fn(|t| acc[2 * t]);
        for t in 1..48 {
            assert!(izq[t] >= izq[t - 1]);
        }
        assert!(izq[1] > izq[0] && izq[2] > izq[1] && izq[3] > izq[2]);
    }

    #[test]
    fn el_paneo_manda_cada_lado() {
        let mut v = Voces::nuevas();
        let b = banco();
        let mut s = sierra();
        s.izq = 256;
        s.der = 0;
        s.inicio = 50;
        s.muestras = 50;
        v.tocar(0, s, 220, 48_000).unwrap();
        let mut acc = [0i32; 96];
        v.mezclar(&b, &mut acc, 2);
        assert!(acc[0] != 0);
        assert!((0..48).all(|t| acc[2 * t + 1] == 0));
        // Y ajustar lo cambia sin reiniciar.
        v.ajustar(0, 0, 256);
        let mut acc = [0i32; 96];
        v.mezclar(&b, &mut acc, 2);
        assert!((0..48).all(|t| acc[2 * t] == 0));
        assert!(acc[1] != 0);
    }

    #[test]
    fn dos_voces_se_suman_y_lo_de_antes_no_se_pisa() {
        let mut v = Voces::nuevas();
        let b = banco();
        let mut s = sierra();
        s.inicio = 99; // una sola muestra, 227 -> (99 << 8)
        s.muestras = 1;
        s.hz = 48_000;
        v.tocar(0, s, 220, 48_000).unwrap();
        v.tocar(1, s, 220, 48_000).unwrap();
        let mut acc = [1000i32; 2];
        v.mezclar(&b, &mut acc, 2);
        // 1000 que habia + dos veces 99*256.
        assert_eq!(acc[0], 1000 + 2 * (99 << 8));
        // Una muestra y fuera: en la trama siguiente ya no queda nada.
        let mut acc = [0i32; 2];
        v.mezclar(&b, &mut acc, 2);
        assert_eq!(acc, [0, 0]);
        assert!(!v.hay());
    }

    #[test]
    fn callar_calla_y_activas_lo_dice() {
        let mut v = Voces::nuevas();
        v.tocar(2, sierra(), 220, 48_000).unwrap();
        v.tocar(5, sierra(), 220, 48_000).unwrap();
        assert_eq!(v.activas(), (1 << 2) | (1 << 5));
        assert_eq!(v.pista(2), Some(3));
        v.callar(2);
        assert_eq!(v.activas(), 1 << 5);
        v.callar_todas();
        assert!(!v.hay());
    }

    #[test]
    fn el_formato_de_16_bits_se_lee_con_signo() {
        let mut v = Voces::nuevas();
        let b = banco();
        let s = Sonido { inicio: 200, muestras: 10, formato: Formato::S16, hz: 48_000, izq: 256, der: 256, pista: 0 };
        v.tocar(0, s, 220, 48_000).unwrap();
        let mut acc = [0i32; 20];
        v.mezclar(&b, &mut acc, 2);
        assert_eq!(acc[2 * 3], 3000);
        assert_eq!(acc[2 * 9 + 1], 9000);
    }

    #[test]
    fn un_banco_que_encoge_calla_la_voz_y_no_lee_fuera() {
        let mut v = Voces::nuevas();
        v.tocar(0, sierra(), 220, 48_000).unwrap();
        // El banco de verdad es mas chico de lo que se juzgo: 10 bytes.
        let chico = [128u8; 10];
        let mut acc = [0i32; 96];
        v.mezclar(&chico, &mut acc, 2);
        assert!(!v.hay());
    }
}
