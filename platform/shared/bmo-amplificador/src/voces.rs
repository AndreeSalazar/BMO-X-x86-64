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
    /// **Vuelve al principio al acabar** (2026-09-22). Lo pidio la musica de
    /// DOOM: una cancion entera en el banco que se toca en bucle. Un efecto no
    /// lo lleva. Es lo unico de los contratos de siempre (DirectSound, OpenAL)
    /// que entro: el TONO sigue fuera hasta que alguien lo pida.
    pub bucle: bool,
}

impl Sonido {
    /// El sonido de nada: lo que hay en una voz que no suena.
    pub const NADA: Sonido =
        Sonido { inicio: 0, muestras: 0, formato: Formato::U8, hz: 0, izq: 0, der: 0, pista: 0, bucle: false };
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

/// **Lo que tarda una voz en llegar al volumen que se le ajusto**: 240 tramas,
/// 5 ms a 48 kHz (2026-10-03).
///
/// Hasta hoy `ajustar` saltaba en la trama siguiente, y un salto de volumen
/// en mitad de una onda es un escalon: se oye como un CLIC. DOOM mueve el
/// paneo de cada voz en cada tic y nadie lo noto porque los disparos lo
/// tapan; una musica de fondo que se baja para dejar oir un aviso, no. 5 ms
/// es lo que tardan las mesas de mezcla en "suavizar" un fader: por debajo de
/// lo que el oido separa como movimiento, y bastante para no ser un escalon.
pub const RAMPA_TRAMAS: u32 = 240;

/// El paso por trama de la rampa, en la unidad interna (1/65536 de pleno).
const RAMPA_PASO: u32 = ((PLENO_VOZ as u32) << 8) / RAMPA_TRAMAS;

/// **Sin angulo**: la voz se paneo con `izq`/`der`, como siempre.
pub const SIN_ANGULO: i16 = i16::MIN;

/// Lo que el retardo entre oidos se mueve por trama, en Q8 muestras: 1/16 de
/// muestra. De lado a lado (31 muestras) son ~10 ms: un monstruo que cruza
/// delante no hace saltar el retardo, que seria un chasquido.
const RETARDO_PASO: i32 = 16;

/// Lo que el polo de la sombra se mueve por trama (Q16): de "sin filtro" a
/// la sombra mas honda en ~10 ms. Sin esto, una fuente que cruza por delante
/// cambia de golpe que oido es el lejano, y pasar de filtrado a sin filtrar
/// en una muestra es un escalon (la prueba lo vio: 920).
const POLO_PASO: i32 = 128;

#[derive(Clone, Copy, Debug)]
struct Voz {
    sonido: Sonido,
    /// Por donde va, en 16.16 sobre las muestras del sonido.
    pos: u64,
    /// Cuanto avanza por cada muestra de SALIDA, en 16.16.
    paso: u64,
    /// **El volumen que SUENA**, izquierdo y derecho, en 1/65536 de pleno: va
    /// hacia `sonido.izq`/`sonido.der` (el que se PIDIO) por la rampa. Cuando
    /// ya llego, la cuenta es la de siempre bit a bit: `(m * (v << 8)) >> 16`
    /// es `(m * v) >> 8`.
    vol: [u32; 2],
    activa: bool,
    /// **El 3D por voz** (S7, 2026-10-03): donde esta la fuente, en grados
    /// (+ derecha, 0 delante), o [`SIN_ANGULO`]. Con angulo, cada oido oye la
    /// voz con su retardo (el lejano lee el banco un poco ANTES: el mismo
    /// sonido, mas tarde) y su paso bajo (la sombra de la cabeza).
    angulo: i16,
    /// El retardo de cada oido, el que suena y al que va (Q8 muestras de
    /// salida), y el polo y el estado de su paso bajo.
    retardo: [i32; 2],
    retardo_meta: [i32; 2],
    polo: [i32; 2],
    polo_meta: [i32; 2],
    bajo: [i64; 2],
    /// La frecuencia de salida, para las tablas del 3D.
    hz: u32,
}

impl Voz {
    const CALLADA: Voz = Voz {
        sonido: Sonido::NADA,
        pos: 0,
        paso: 0,
        vol: [0, 0],
        activa: false,
        angulo: SIN_ANGULO,
        retardo: [0, 0],
        retardo_meta: [0, 0],
        polo: [65_536, 65_536],
        polo_meta: [65_536, 65_536],
        bajo: [0, 0],
        hz: 0,
    };
}

/// Un paso de la rampa: hacia `meta`, sin pasarse.
fn acercar(v: u32, meta: u32) -> u32 {
    if v < meta {
        (v + RAMPA_PASO).min(meta)
    } else {
        v.saturating_sub(RAMPA_PASO).max(meta)
    }
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
        // Un sonido NUEVO empieza ya a su volumen: el ataque de un disparo es
        // parte del disparo, y una rampa ahi se lo comeria.
        self.voz[canal] = Voz {
            sonido: s,
            pos: 0,
            paso: ((s.hz as u64) << 16) / hz_salida as u64,
            vol: [(s.izq as u32) << 8, (s.der as u32) << 8],
            activa: true,
            hz: hz_salida,
            ..Voz::CALLADA
        };
        Ok(())
    }

    /// **Mover** una voz que suena: volumen y lado. No la reinicia, y no SALTA:
    /// llega en [`RAMPA_TRAMAS`].
    pub fn ajustar(&mut self, canal: usize, izq: u16, der: u16) {
        if let Some(v) = self.voz.get_mut(canal) {
            v.sonido.izq = izq.min(PLENO_VOZ);
            v.sonido.der = der.min(PLENO_VOZ);
        }
    }

    /// **Situar una voz en el espacio** (S7 por voz): `vol` 0..=256 y
    /// `angulo` en grados (+ derecha, 0 delante, 180 detras), o
    /// [`SIN_ANGULO`] para volver al paneo de siempre. El volumen de cada
    /// oido va por la rampa de siempre y el retardo por la suya: moverla no
    /// salta. Si la voz acaba de empezar, entra ya en su sitio.
    pub fn situar(&mut self, canal: usize, vol: u16, angulo: i16) {
        let Some(v) = self.voz.get_mut(canal) else { return };
        let vol = vol.min(PLENO_VOZ) as i32;
        let caminos = if angulo == SIN_ANGULO { None } else { crate::espacio::caminos(v.hz, angulo as i32) };
        let Some(c) = caminos else {
            v.angulo = SIN_ANGULO;
            v.sonido.izq = vol as u16;
            v.sonido.der = vol as u16;
            v.retardo_meta = [0, 0];
            v.polo_meta = [65_536, 65_536];
            return;
        };
        let nueva = v.angulo == SIN_ANGULO && v.pos == 0;
        v.angulo = angulo;
        v.sonido.izq = ((vol * c[0].0) >> 16) as u16;
        v.sonido.der = ((vol * c[1].0) >> 16) as u16;
        v.retardo_meta = [c[0].1, c[1].1];
        v.polo_meta = [c[0].2, c[1].2];
        if nueva {
            v.retardo = v.retardo_meta;
            v.polo = v.polo_meta;
            v.vol = [(v.sonido.izq as u32) << 8, (v.sonido.der as u32) << 8];
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
        self.mezclar_solo(banco, acc, canales, u32::MAX);
    }

    /// **Mezclar SOLO los canales de `mascara`** (bit `n` = canal `n`). Es lo
    /// que deja separar, en el mismo banco, lo que se agacha (la musica de
    /// fondo) de lo que no (el aviso que la hace agacharse).
    pub fn mezclar_solo(&mut self, banco: &[u8], acc: &mut [i32], canales: usize, mascara: u32) {
        let canales = canales.max(1);
        let tramas = acc.len() / canales;
        for (n, v) in self.voz.iter_mut().enumerate() {
            if !v.activa || mascara & (1 << n) == 0 {
                continue;
            }
            let s = v.sonido;
            for t in 0..tramas {
                let mut idx = (v.pos >> 16) as u32;
                if idx >= s.muestras {
                    if !s.bucle {
                        v.activa = false;
                        break;
                    }
                    // ** EL BUCLE: se vuelve al principio conservando la
                    // FRACCION, o cada vuelta dejaria caer un trozo de muestra
                    // y la cancion se iria adelantando.
                    v.pos %= (s.muestras as u64) << 16;
                    idx = (v.pos >> 16) as u32;
                }
                let Some(a) = muestra(banco, &s, idx) else {
                    v.activa = false;
                    break;
                };
                // La ultima va hacia el silencio: el sonido se acaba ahi. En
                // bucle va hacia la PRIMERA, que es la que suena despues.
                let b = if idx + 1 < s.muestras {
                    muestra(banco, &s, idx + 1).unwrap_or(0)
                } else if s.bucle {
                    muestra(banco, &s, 0).unwrap_or(0)
                } else {
                    0
                };
                let frac = (v.pos & 0xFFFF) as i64;
                let m = a + (((b - a) as i64 * frac) >> 16) as i32;
                v.vol[0] = acercar(v.vol[0], (s.izq as u32) << 8);
                v.vol[1] = acercar(v.vol[1], (s.der as u32) << 8);
                let (vi, vd) = (v.vol[0] as i64, v.vol[1] as i64);
                let base = t * canales;
                if v.angulo != SIN_ANGULO {
                    // ** EL 3D POR VOZ: cada oido con su retardo y su sombra.
                    let mut oye = [m; 2];
                    for (e, x) in oye.iter_mut().enumerate() {
                        let (r, meta) = (v.retardo[e], v.retardo_meta[e]);
                        let r = if r < meta { (r + RETARDO_PASO).min(meta) } else { (r - RETARDO_PASO).max(meta) };
                        v.retardo[e] = r;
                        if r > 0 {
                            // El retardo es en muestras de SALIDA; en el banco
                            // son `r * paso`. Antes del principio, silencio.
                            let atras = (r as u64 * v.paso) >> 8;
                            *x = if v.pos >= atras {
                                leer(banco, &s, v.pos - atras)
                            } else if s.bucle && s.muestras > 0 {
                                let largo = (s.muestras as u64) << 16;
                                leer(banco, &s, (v.pos + largo - atras % largo) % largo)
                            } else {
                                0
                            };
                        }
                        let (p, pm) = (v.polo[e], v.polo_meta[e]);
                        let p = if p < pm { (p + POLO_PASO).min(pm) } else { (p - POLO_PASO).max(pm) };
                        v.polo[e] = p;
                        // Siempre por el filtro: con el polo en 65536 deja
                        // pasar la muestra tal cual, y asi entrar y salir de
                        // la sombra es continuo.
                        v.bajo[e] += (p as i64 * (((*x as i64) << 16) - v.bajo[e]) + (1 << 15)) >> 16;
                        *x = ((v.bajo[e] + (1 << 15)) >> 16) as i32;
                    }
                    if canales >= 2 {
                        acc[base] += ((oye[0] as i64 * vi) >> 16) as i32;
                        acc[base + 1] += ((oye[1] as i64 * vd) >> 16) as i32;
                    } else {
                        acc[base] += (((oye[0] as i64 * vi) + (oye[1] as i64 * vd)) >> 17) as i32;
                    }
                } else if canales >= 2 {
                    acc[base] += ((m as i64 * vi) >> 16) as i32;
                    acc[base + 1] += ((m as i64 * vd) >> 16) as i32;
                } else {
                    acc[base] += ((m as i64 * ((vi + vd) / 2)) >> 16) as i32;
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

/// Una muestra de un sonido en una posicion 16.16, interpolada; 0 si no esta.
fn leer(banco: &[u8], s: &Sonido, p: u64) -> i32 {
    let idx = (p >> 16) as u32;
    let Some(a) = muestra(banco, s, idx) else { return 0 };
    let b = if idx + 1 < s.muestras {
        muestra(banco, s, idx + 1).unwrap_or(0)
    } else if s.bucle {
        muestra(banco, s, 0).unwrap_or(0)
    } else {
        0
    };
    a + (((b - a) as i64 * (p & 0xFFFF) as i64) >> 16) as i32
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
        Sonido { inicio: 0, muestras: 100, formato: Formato::U8, hz: 12_000, izq: 256, der: 256, pista: 3, bucle: false }
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
        // Y ajustar lo cambia sin reiniciar: por la rampa, asi que lo que se
        // mira es despues de ella (el sonido dura 50 muestras a 12 kHz, 200 a
        // la salida: se toca en bucle para que siga sonando).
        let mut s2 = s;
        s2.bucle = true;
        v.tocar(0, s2, 220, 48_000).unwrap();
        v.ajustar(0, 0, 256);
        let mut acc = [0i32; 2 * RAMPA_TRAMAS as usize];
        v.mezclar(&b, &mut acc, 2);
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
        let s = Sonido {
            inicio: 200,
            muestras: 10,
            formato: Formato::S16,
            hz: 48_000,
            izq: 256,
            der: 256,
            pista: 0,
            bucle: false,
        };
        v.tocar(0, s, 220, 48_000).unwrap();
        let mut acc = [0i32; 20];
        v.mezclar(&b, &mut acc, 2);
        assert_eq!(acc[2 * 3], 3000);
        assert_eq!(acc[2 * 9 + 1], 9000);
    }

    #[test]
    fn en_bucle_no_se_calla_y_vuelve_al_principio() {
        let mut v = Voces::nuevas();
        let b = banco();
        // 10 muestras S16 a 48 kHz: 0, 1000, ..., 9000, y otra vez.
        let s = Sonido {
            inicio: 200,
            muestras: 10,
            formato: Formato::S16,
            hz: 48_000,
            izq: 256,
            der: 256,
            pista: 0,
            bucle: true,
        };
        v.tocar(0, s, 220, 48_000).unwrap();
        let mut acc = [0i32; 50];
        v.mezclar(&b, &mut acc, 2);
        let izq: [i32; 25] = core::array::from_fn(|t| acc[2 * t]);
        // La trama 10 es otra vez la muestra 0, y la 13 la 3.
        assert_eq!(izq[9], 9000);
        assert_eq!(izq[10], 0);
        assert_eq!(izq[13], 3000);
        assert_eq!(izq[24], 4000);
        // Mil vueltas despues sigue sonando.
        for _ in 0..1000 {
            let mut acc = [0i32; 96];
            v.mezclar(&b, &mut acc, 2);
        }
        assert!(v.hay());
    }

    #[test]
    fn en_bucle_la_ultima_muestra_va_hacia_la_primera_y_no_al_silencio() {
        let mut v = Voces::nuevas();
        let b = banco();
        // Dos muestras (1000 y 2000... de la S16) a la MITAD de la salida:
        // cada muestra de entrada da dos de salida, y la de en medio es la
        // recta hacia la siguiente.
        let s = Sonido {
            inicio: 202,
            muestras: 2,
            formato: Formato::S16,
            hz: 24_000,
            izq: 256,
            der: 256,
            pista: 0,
            bucle: true,
        };
        v.tocar(0, s, 220, 48_000).unwrap();
        let mut acc = [0i32; 12];
        v.mezclar(&b, &mut acc, 2);
        let izq: [i32; 6] = core::array::from_fn(|t| acc[2 * t]);
        // 1000, 1500, 2000, 1500 (hacia la PRIMERA, no hacia 0), 1000...
        assert_eq!(izq, [1000, 1500, 2000, 1500, 1000, 1500]);
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

    /// Un banco S16 con una CONTINUA a 20.000: lo peor para un salto de
    /// volumen, porque todo el escalon se oye.
    fn continua() -> [u8; 200] {
        let mut b = [0u8; 200];
        for i in 0..100 {
            let [lo, hi] = 20_000i16.to_le_bytes();
            b[2 * i] = lo;
            b[2 * i + 1] = hi;
        }
        b
    }

    #[test]
    fn ajustar_no_salta_va_por_la_rampa() {
        let b = continua();
        let s = Sonido { inicio: 0, muestras: 100, formato: Formato::S16, hz: 48_000, izq: 256, der: 256, pista: 0, bucle: true };
        let mut v = Voces::nuevas();
        v.tocar(0, s, 200, 48_000).unwrap();
        let mut acc = [0i32; 96];
        v.mezclar(&b, &mut acc, 2);
        assert_eq!(acc[0], 20_000, "sin ajustar, el volumen pedido tal cual");
        // Bajar a cero de golpe: la salida baja por escalones de una rampa,
        // no de una vez.
        v.ajustar(0, 0, 0);
        let mut acc = [0i32; 2 * 300];
        v.mezclar(&b, &mut acc, 2);
        let mut antes = 20_000;
        for t in 0..300 {
            let x = acc[2 * t];
            assert!(antes - x <= 100, "trama {t}: de {antes} a {x} es un escalon");
            antes = x;
        }
        assert_eq!(acc[2 * 299], 0, "y al acabar la rampa, callada");
        // Un `tocar` nuevo NO lleva rampa: el ataque es del sonido.
        v.tocar(0, s, 200, 48_000).unwrap();
        let mut acc = [0i32; 2];
        v.mezclar(&b, &mut acc, 2);
        assert_eq!(acc[0], 20_000);
    }

    #[test]
    fn la_mascara_separa_lo_que_se_agacha_de_lo_que_no() {
        let b = continua();
        let s = Sonido { inicio: 0, muestras: 100, formato: Formato::S16, hz: 48_000, izq: 256, der: 256, pista: 0, bucle: true };
        let mut v = Voces::nuevas();
        v.tocar(0, s, 200, 48_000).unwrap();
        v.tocar(9, s, 200, 48_000).unwrap();
        let mut fondo = [0i32; 2];
        let mut aviso = [0i32; 2];
        v.mezclar_solo(&b, &mut fondo, 2, 0x00FF);
        v.mezclar_solo(&b, &mut aviso, 2, 0xFF00);
        assert_eq!((fondo[0], aviso[0]), (20_000, 20_000), "cada una en su cubo");
        // Y las dos mitades juntas son lo mismo que mezclar todo.
        let mut v2 = Voces::nuevas();
        v2.tocar(0, s, 200, 48_000).unwrap();
        v2.tocar(9, s, 200, 48_000).unwrap();
        let mut todo = [0i32; 2];
        v2.mezclar(&b, &mut todo, 2);
        assert_eq!(todo[0], fondo[0] + aviso[0]);
    }

    /// Un banco S16 con `n` muestras de lo que diga `f`.
    fn banco_de(n: usize, f: impl Fn(usize) -> i16) -> std::vec::Vec<u8> {
        let mut b = std::vec::Vec::new();
        for i in 0..n {
            b.extend_from_slice(&f(i).to_le_bytes());
        }
        b
    }

    extern crate std;

    fn voz(n: usize, bucle: bool) -> Sonido {
        Sonido { inicio: 0, muestras: n as u32, formato: Formato::S16, hz: 48_000, izq: 256, der: 256, pista: 0, bucle }
    }

    #[test]
    fn una_voz_a_la_derecha_llega_antes_y_mas_fuerte_al_oido_derecho() {
        // Un golpe y despues nada: se ve en que muestra llega a cada oido.
        let b = banco_de(200, |i| if i == 0 { 20_000 } else { 0 });
        let mut v = Voces::nuevas();
        v.tocar(0, voz(200, false), b.len() as u64, 48_000).unwrap();
        v.situar(0, 256, 90);
        let mut acc = [0i32; 400];
        v.mezclar(&b, &mut acc, 2);
        let izq: std::vec::Vec<i32> = (0..200).map(|t| acc[2 * t]).collect();
        let der: std::vec::Vec<i32> = (0..200).map(|t| acc[2 * t + 1]).collect();
        let primero = |x: &[i32]| x.iter().position(|&y| y.abs() > 30).unwrap();
        assert_eq!(primero(&der), 0, "el oido cercano, ya");
        let pi = primero(&izq);
        assert!((29..=33).contains(&pi), "el lejano, 0,66 ms despues: muestra {pi}");
        let pico = |x: &[i32]| x.iter().map(|y| y.abs()).max().unwrap();
        assert!(pico(&izq) < pico(&der) / 2);
    }

    #[test]
    fn sin_angulo_es_el_paneo_de_siempre() {
        // Volver a SIN_ANGULO pasa por la rampa del volumen; acabada, es el
        // paneo de siempre muestra a muestra.
        let b = banco_de(100, |i| (i as i16) * 100);
        let mut a = Voces::nuevas();
        let mut c = Voces::nuevas();
        a.tocar(0, voz(100, true), 200, 48_000).unwrap();
        c.tocar(0, voz(100, true), 200, 48_000).unwrap();
        c.situar(0, 256, 30);
        c.situar(0, 256, SIN_ANGULO);
        let (mut x, mut y) = ([0i32; 600], [0i32; 600]);
        a.mezclar(&b, &mut x, 2);
        c.mezclar(&b, &mut y, 2);
        let (mut x, mut y) = ([0i32; 200], [0i32; 200]);
        a.mezclar(&b, &mut x, 2);
        c.mezclar(&b, &mut y, 2);
        assert_eq!(x, y, "volver a SIN_ANGULO es el paneo de siempre");
    }

    #[test]
    fn un_monstruo_que_cruza_no_chasquea() {
        // Un seno de 300 Hz en bucle que cruza de -90 a +90 grados en medio
        // segundo, moviendolo cada 1/35 s como DOOM: sin escalones.
        let n = 160; // 300 Hz exactos a 48 kHz: 160 muestras por periodo
        let b = banco_de(n, |i| (12_000.0 * (2.0 * core::f64::consts::PI * i as f64 / n as f64).sin()) as i16);
        let mut v = Voces::nuevas();
        v.tocar(0, voz(n, true), b.len() as u64, 48_000).unwrap();
        v.situar(0, 256, -90);
        let mut antes = [0i32; 2];
        let mut peor = 0;
        for tic in 0..18 {
            v.situar(0, 256, (-90 + tic * 10) as i16);
            let mut acc = [0i32; 2 * 1371];
            v.mezclar(&b, &mut acc, 2);
            for t in 0..1371 {
                for e in 0..2 {
                    // El primer milisegundo es el ATAQUE del sonido (el
                    // filtro arranca de cero): lo que se mira es el cruce.
                    if tic > 0 || t > 48 {
                        peor = peor.max((acc[2 * t + e] - antes[e]).abs());
                    }
                    antes[e] = acc[2 * t + e];
                }
            }
        }
        // 300 Hz a 12.000 sube como mucho ~470 por muestra.
        assert!(peor < 700, "un salto de {peor}");
    }

    #[test]
    fn lo_de_detras_suena_mas_oscuro_que_lo_de_delante() {
        // Una cuadrada de 6 kHz: casi todo agudos.
        let b = banco_de(8, |i| if i < 4 { 12_000 } else { -12_000 });
        let fuerza = |g: i16| {
            let mut v = Voces::nuevas();
            v.tocar(0, voz(8, true), b.len() as u64, 48_000).unwrap();
            v.situar(0, 256, g);
            let mut acc = [0i32; 2 * 4_800];
            v.mezclar(&b, &mut acc, 2);
            acc[4_800..].iter().map(|&x| (x as f64).powi(2)).sum::<f64>()
        };
        assert!(fuerza(180) < fuerza(0) * 0.6, "detras no es mas oscuro");
    }
}

