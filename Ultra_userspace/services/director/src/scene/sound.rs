//! **EL SONIDO: el MAESTRO en el escritorio** -- F10, o el indicador de la barra.
//!
//! [consumo] LATE      con sonido, el medidor se mira 20 veces por segundo y
//!                     solo se repinta la parte que CAMBIO; en silencio, 4 veces
//!                     y nada que pintar. El indicador de la barra, igual (L6h)
//!
//! === *** POR QUE ESTA VENTANA YA NO RECLAMA EL SONIDO (2026-09-22) ===
//!
//! Nacio el 2026-08-09 como el control del altavoz del PC: reclamaba
//! `KIND_AUDIO` al abrirse y lo soltaba al cerrarse, con un teclado de siete
//! notas para comprobar que sonaba. Y aquello era correcto entonces --el
//! aparato es exclusivo, y quedarselo al arrancar habria dejado mudos a todos
//! los programas-- pero tenia una consecuencia que salio con DOOM:
//!
//! ```text
//!    DOOM suena  ->  DOOM tiene el sonido  ->  esta ventana: "lo tiene OTRO"
//!                                          ->  `audio volumen`: 0
//! ```
//!
//! **Con un juego sonando, el escritorio no podia tocar ni el volumen.**
//! Producir y mandar eran el mismo permiso. El propietario lo dijo asi:
//! *"funcionan pero no tengo control de audio... un control que viva en mi
//! escritorio, no como app"*.
//!
//! Ahora hay dos permisos. El que PRODUCE sigue siendo exclusivo y lo tiene la
//! app. El que MANDA es `OP_AUDIO_MANDO`: no reclama nada, convive con quien
//! suene, y el kernel solo se lo da a quien tiene la pantalla. Esta ventana es
//! la cara de ese mando. El teclado de siete notas se fue con el altavoz: en
//! esta placa no suena (no trae zumbador) y lo que ahora dice si algo suena es
//! el MEDIDOR, que mide lo que sale al cable.
//!
//! === Lo que se toma de Premiere, y lo que no ===
//!
//! ```text
//!    se toma   el fader VERTICAL en dB con su escala, que tiene mas recorrido
//!              cerca de 0 que abajo; el medidor por LADO con verde, ambar y
//!              rojo, la marca de pico que se queda un momento y la luz de
//!              RECORTE; y los numeros al lado, porque ahi se lee lo que el
//!              ojo no distingue en una barra
//!    no        pistas, panoramas, automatizacion: eso es LA MESA, y es otra
//!              casilla. Esto es el maestro, la perilla de la habitacion
//! ```
//!
//! === Por que el fader es UNO y por debajo son DOS etapas ===
//!
//! El audifono del propietario da de -45 a 0 dB y no mas. Por debajo de su tope,
//! el fader le pide el volumen AL APARATO, que lo da limpio; por encima, el
//! aparato se queda a tope y el resto lo pone la etapa digital del kernel, con
//! su limite detras. La mano mueve una sola cosa, y el panel dice cual de las
//! dos esta trabajando: un control que esconde donde actua es un control del
//! que no te puedes fiar cuando suena raro.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

// Proporcion de la pantalla y no una medida fija, como las demas: ver
// `docs/identidad/LIDERES.md`. Los minimos son los de la columna de numeros:
// por debajo, "aparato  -45.0 dB (su tope)" no cabe entero.
const SND_PCT_W: u32 = 30;
const SND_PCT_H: u32 = 52;
const SND_MIN_W: u32 = 470;
const SND_MIN_H: u32 = 430;

// El ambar es de esta ventana igual que el azul es del kernel y el verde de
// ESTRATOS: el color dice cual es antes de leer el titulo.
const SND_BG: u32 = 0x0018_1105;
pub(crate) const SND_TITLE_BG: u32 = 0x0026_1A08;
const SND_EDGE: u32 = 0x004A_3418;
const SND_TITLE: u32 = 0x00F0_A860;
const SND_BAR_GAP: u32 = 0x0032_2410;
/// El tramo del fader por encima de 0 dB: ahi trabaja la etapa DIGITAL.
const SND_DIGITAL: u32 = 0x0050_3410;

// Las tres zonas del medidor, las de cualquier mesa: verde hasta -12 dBFS,
// ambar hasta -3, rojo arriba. Y apagado, para que se vea el recorrido entero.
const VERDE: u32 = 0x0022_C55E;
const AMBAR: u32 = 0x00EA_B308;
const ROJO: u32 = 0x00EF_4444;
const APAGADO: u32 = 0x0010_0C06;
const LUZ_APAGADA: u32 = 0x0030_1410;
const RMS_MARCA: u32 = 0x00F0_ECE0;

const DB: i32 = 256;
/// El silencio del todo, como lo dice el kernel: -96 dB (el `MIN_DB` del
/// amplificador, que el escritorio no enlaza).
const NADA: i32 = -96 * DB;

/// El recorrido del fader. El kernel acepta de -96 a +52 (el techo del MAESTRO
/// subio de +24 a +52 el 2026-09-22); por debajo de -60 no queda nada que oir,
/// y el MUDO es el boton para el silencio de verdad.
///
/// [!] Por encima de ~+24 el aparato ya esta en su tope y lo que sube es lo
/// FLOJO: el limite sujeta la punta. Se oye mas denso, no mas alto en el pico,
/// y la fila `aplasta` lo dice en rojo.
pub(crate) const FADER_MIN: i32 = -60 * DB;
pub(crate) const FADER_MAX: i32 = 52 * DB;
/// El fader se mueve de medio en medio dB: cada paso que cae en el rango del
/// aparato son dos transferencias en el bus, y la oreja no distingue menos.
pub(crate) const FADER_GRANO: i32 = DB / 2;

/// **La escala del fader**: `(dB, milesimas del alto desde ARRIBA)`. Mas
/// recorrido cerca de 0 que abajo, como en una mesa: ahi es donde se ajusta.
const ESCALA_FADER: [(i32, u32); 9] = [
    (52, 0),
    (36, 130),
    (24, 250),
    (12, 390),
    (0, 560),
    (-12, 700),
    (-24, 810),
    (-45, 930),
    (-60, 1000),
];
/// Las marcas que se escriben junto al fader.
const MARCAS_FADER: [i32; 9] = [52, 36, 24, 12, 0, -12, -24, -45, -60];
/// **La escala del medidor**, en dBFS.
const ESCALA_MEDIDOR: [(i32, u32); 8] =
    [(0, 0), (-6, 120), (-12, 250), (-18, 380), (-24, 500), (-36, 700), (-48, 850), (-60, 1000)];
const MARCAS_MEDIDOR: [i32; 8] = [0, -6, -12, -18, -24, -36, -48, -60];

/// La ventana del sonido. **Movible**, como todas las del escritorio.
pub(crate) struct SoundWindow {
    pub(crate) chrome: Chrome,
}

impl SoundWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(p, SND_PCT_W, SND_PCT_H, SND_MIN_W, SND_MIN_H),
        }
    }

    /// **Al abrirla desde el panel, al lado del indicador**: pegada al panel y
    /// con el pie a la altura del vol, que es de donde se pidio. Con F10 se
    /// queda donde estuviera.
    pub(crate) fn junto_a_la_barra(&mut self, p: &bmo::Pantalla) {
        let pie = match barra_caja(p.alto) {
            Some((_, by, _, bh)) => by + bh,
            None => p.alto.saturating_sub(8),
        };
        self.chrome.x = super::lateral::margen() + super::chrome::HUECO;
        self.chrome.y = pie.saturating_sub(self.chrome.height).max(super::chrome::HUECO);
    }
}

// ===================================================================
//  LO QUE SE LEE DEL KERNEL
// ===================================================================

/// **Todo lo que el panel muestra, leido de una vez** por `OP_INFO`. Sin
/// handle: preguntar que hay no es tener derecho a usarlo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lectura {
    pub hay_aparato: bool,
    pub frecuencia: u32,
    pub canales: u8,
    /// El fader que puso el escritorio, en 1/256 dB.
    pub fader: i32,
    /// La parte que se le pidio al aparato.
    pub aparato: i32,
    /// La ganancia digital que hay puesta AHORA, por donde va la rampa.
    pub digital: i32,
    pub mudo: bool,
    /// El escritorio ha movido el maestro alguna vez.
    pub tocado: bool,
    /// `0` sin tubo, `1` en marcha, `2..4` el maestro no actua (ver el kernel).
    pub estado: u8,
    pub pico: [i32; 2],
    pub rms: [i32; 2],
    pub dobladas: u64,
    pub reduccion: i32,
    pub ventanas: u16,
    /// El volumen con el que vino el aparato, si se leyo.
    pub fabrica: Option<i32>,
    /// El tope del aparato (normalmente 0 dB).
    pub tope: i32,
}

fn i16_en(v: u64, bit: u32) -> i32 {
    ((v >> bit) & 0xFFFF) as u16 as i16 as i32
}

pub(crate) fn leer() -> Lectura {
    let ap = bmo::info(bmo::INFO_AUDIO_APARATO);
    let rango = bmo::info(bmo::INFO_AUDIO_RANGO);
    let tubo = bmo::info(bmo::INFO_AUDIO_TUBO);
    let m = bmo::info(bmo::INFO_AUDIO_MAESTRO);
    let med = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let lim = bmo::info(bmo::INFO_AUDIO_LIMITE);
    let fab = bmo::info(bmo::INFO_AUDIO_FABRICA);
    // Los canales de lo que SUENA: los del formato elegido, no los del mando
    // de volumen (que el `save` muestra aparte porque son otra cosa).
    let formatos = bmo::info(bmo::INFO_AUDIO_FORMATOS);
    let elegido = (formatos >> 8) & 0xFF;
    let formato = if formatos & 0xFF > 0 { bmo::info(bmo::INFO_AUDIO_FORMATO | (elegido << 8)) } else { 0 };
    Lectura {
        hay_aparato: ap & 0xFF != 0,
        frecuencia: if tubo != 0 { (tubo & 0xFF_FFFF) as u32 } else { 0 },
        canales: ((formato >> 8) & 0xFF) as u8,
        fader: i16_en(m, 0),
        aparato: i16_en(m, 16),
        digital: i16_en(m, 32),
        mudo: m & (1 << 48) != 0,
        tocado: m & (1 << 49) != 0,
        estado: (m >> 56) as u8,
        pico: [i16_en(med, 0), i16_en(med, 16)],
        rms: [i16_en(med, 32), i16_en(med, 48)],
        dobladas: lim & 0xFFFF_FFFF,
        reduccion: i16_en(lim, 32),
        ventanas: (lim >> 48) as u16,
        fabrica: if fab & (1 << 16) != 0 { Some(i16_en(fab, 0)) } else { None },
        tope: i16_en(rango, 16),
    }
}

impl Lectura {
    /// **Donde esta el fader DE VERDAD.** Si el escritorio no lo ha tocado
    /// nunca, el aparato tiene su volumen de fabrica y la etapa digital esta a
    /// 0: el fader tiene que salir ahi, y no en un 0 dB que no es cierto.
    pub(crate) fn fader_efectivo(&self) -> i32 {
        if self.tocado {
            self.fader
        } else {
            self.fabrica.unwrap_or(0)
        }
    }

    /// Hay algo que medir: la ultima ventana tuvo onda.
    pub(crate) fn suena(&self) -> bool {
        self.pico[0] > NADA || self.pico[1] > NADA
    }
}

// ===================================================================
//  EL ESTADO DEL PANEL: lo que la mano y el ojo necesitan
// ===================================================================

/// **Lo que el panel recuerda y el kernel no sabe**: la marca de pico que se
/// queda un momento, la luz de RECORTE encendida un rato, el arrastre del
/// fader y lo ultimo que se pinto. Vive en el escritorio (`SoundState`), no
/// aqui: un modulo que pinta y ademas recuerda acaba con dos verdades.
pub(crate) struct Panel {
    /// El raton tiene el fader cogido.
    pub arrastrando: bool,
    /// Lo ultimo que se mando al kernel: arrastrando llegan cien movimientos
    /// por segundo, y el mismo valor dos veces son dos puertas para nada.
    pub enviado: Option<i32>,
    /// El siguiente refresco, en ciclos del TSC.
    pub proximo: u64,
    /// La ultima lectura tenia onda: el medidor se mira a 20 Hz y no a 4.
    pub activo: bool,
    retenido: [i32; 2],
    retenido_hasta: [u64; 2],
    recorte_hasta: u64,
    /// Lo doblegado la ultima vez que se miro. `None` = aun no se ha mirado.
    dobladas_vistas: Option<u64>,
    luz: bool,
    visto_fader: Option<(i32, bool)>,
    visto_medidor: Option<[u32; 7]>,
    visto_numeros: Option<Lectura>,
    /// **Lo que se esta ESCRIBIENDO**: los dB tecleados, `-` incluido, antes
    /// del Enter. Tres letras bastan: `-60` y `52` son los extremos.
    pub(crate) escrito: [u8; 3],
    pub(crate) escrito_n: usize,
}

/// Lo que dura la marca de pico: un segundo y medio, lo de una mesa.
const RETENER_MS: u64 = 1500;
/// Lo que dura encendida la luz de RECORTE tras la ultima muestra doblegada.
const RECORTE_MS: u64 = 2000;

impl Panel {
    pub(crate) const fn nuevo() -> Self {
        Panel {
            arrastrando: false,
            enviado: None,
            proximo: 0,
            activo: false,
            retenido: [NADA; 2],
            retenido_hasta: [0; 2],
            recorte_hasta: 0,
            dobladas_vistas: None,
            luz: false,
            visto_fader: None,
            visto_medidor: None,
            visto_numeros: None,
            escrito: [0; 3],
            escrito_n: 0,
        }
    }

    /// Se dejo de escribir: la linea del aviso vuelve a decir lo suyo en el
    /// refresco siguiente.
    pub(crate) fn olvidar_cabecera(&mut self) {
        self.visto_numeros = None;
    }

    /// **Se apunta lo que dijo el kernel**: la marca de pico y la luz.
    pub(crate) fn observar(&mut self, l: &Lectura, ahora: u64, hz: u64) {
        let ms = |n: u64| n * hz / 1000;
        for i in 0..2 {
            if l.pico[i] >= self.retenido[i] || ahora >= self.retenido_hasta[i] {
                self.retenido[i] = l.pico[i];
                self.retenido_hasta[i] = ahora + ms(RETENER_MS);
            }
        }
        // La primera lectura no enciende nada: lo doblegado ANTES de mirar ya
        // lo dice el numero, y encender la luz por el pasado es mentir.
        if let Some(antes) = self.dobladas_vistas {
            if l.dobladas > antes {
                self.recorte_hasta = ahora + ms(RECORTE_MS);
            }
        }
        self.dobladas_vistas = Some(l.dobladas);
        self.activo = l.suena() || self.retenido.iter().any(|&r| r > NADA);
        self.luz = ahora < self.recorte_hasta;
    }

    /// Lo pintado se da por perdido: la ventana se repinto entera.
    pub(crate) fn olvidar(&mut self) {
        self.visto_fader = None;
        self.visto_medidor = None;
        self.visto_numeros = None;
    }

    /// La luz de RECORTE. La deja calculada `observar`, para que pintar no
    /// necesite el reloj.
    fn luz_encendida(&self) -> bool {
        self.luz
    }
}

// ===================================================================
//  LA GEOMETRIA: un sitio para pintar y para el raton
// ===================================================================

/// Donde va cada cosa, calculado del marco. El raton y el pintor leen ESTO:
/// si cada uno hiciera su cuenta, el fader se pintaria en un sitio y se
/// cogeria en otro.
pub(crate) struct Sitio {
    tx: u32,
    y_aparato: u32,
    y_aviso: u32,
    pub(crate) arriba: u32,
    pub(crate) abajo: u32,
    pub(crate) fader_x: u32,
    pub(crate) fader_w: u32,
    medidor_x: [u32; 2],
    medidor_w: u32,
    luz_y: u32,
    escala_medidor_x: u32,
    num_x: u32,
    num_w: u32,
    pub(crate) mudo: (u32, u32, u32, u32),
    pie_y: u32,
}

const LINEA: u32 = bmo::GLIFO_ALTO + 4;
const LINEAS_NUMEROS: u32 = 7;

pub(crate) fn sitio(c: &SoundWindow) -> Sitio {
    let ch = &c.chrome;
    let tx = ch.x + 16;
    let y_aparato = ch.y + TITLE_H + 10;
    let y_aviso = y_aparato + LINEA;
    let arriba = y_aviso + LINEA + 20;
    let pie_y = (ch.y + ch.height).saturating_sub(2 * LINEA + 8);
    let abajo = pie_y.saturating_sub(16).max(arriba + 60);
    let fader_x = tx + 36;
    let fader_w = 32;
    let m0 = fader_x + fader_w + 20;
    let medidor_w = 14;
    let m1 = m0 + medidor_w + 4;
    let escala_medidor_x = m1 + medidor_w + 6;
    let num_x = escala_medidor_x + 3 * bmo::GLIFO_ANCHO + 16;
    let num_w = (ch.x + ch.width).saturating_sub(16 + num_x);
    let mudo_y = arriba + LINEAS_NUMEROS * LINEA + 10;
    Sitio {
        tx,
        y_aparato,
        y_aviso,
        arriba,
        abajo,
        fader_x,
        fader_w,
        medidor_x: [m0, m1],
        medidor_w,
        luz_y: arriba.saturating_sub(14),
        escala_medidor_x,
        num_x,
        num_w,
        mudo: (num_x, mudo_y, 14 * bmo::GLIFO_ANCHO, bmo::GLIFO_ALTO + 10),
        pie_y,
    }
}

fn a_milesimas(db: i32, escala: &[(i32, u32)]) -> u32 {
    let n = escala.len();
    if db >= escala[0].0 * DB {
        return escala[0].1;
    }
    if db <= escala[n - 1].0 * DB {
        return escala[n - 1].1;
    }
    for w in escala.windows(2) {
        let (a, ma) = (w[0].0 * DB, w[0].1);
        let (b, mb) = (w[1].0 * DB, w[1].1);
        if db <= a && db >= b {
            return ma + ((a - db) as i64 * (mb - ma) as i64 / (a - b).max(1) as i64) as u32;
        }
    }
    escala[n - 1].1
}

fn de_milesimas(m: u32, escala: &[(i32, u32)]) -> i32 {
    let n = escala.len();
    if m <= escala[0].1 {
        return escala[0].0 * DB;
    }
    for w in escala.windows(2) {
        let (a, ma) = (w[0].0 * DB, w[0].1);
        let (b, mb) = (w[1].0 * DB, w[1].1);
        if m >= ma && m <= mb {
            return a - ((m - ma) as i64 * (a - b) as i64 / (mb - ma).max(1) as i64) as i32;
        }
    }
    escala[n - 1].0 * DB
}

impl Sitio {
    fn y_fader(&self, db: i32) -> u32 {
        self.arriba + (self.abajo - self.arriba) * a_milesimas(db, &ESCALA_FADER) / 1000
    }

    fn y_medidor(&self, db: i32) -> u32 {
        self.arriba + (self.abajo - self.arriba) * a_milesimas(db, &ESCALA_MEDIDOR) / 1000
    }

    /// **El dB que hay bajo el puntero**, redondeado al grano del fader. Lo
    /// usa el raton: la misma escala que pinta es la que lee.
    pub(crate) fn db_en(&self, y: u32) -> i32 {
        let y = y.clamp(self.arriba, self.abajo);
        let m = (y - self.arriba) * 1000 / (self.abajo - self.arriba).max(1);
        redondear(de_milesimas(m, &ESCALA_FADER))
    }

    /// El puntero esta en la columna del fader?
    pub(crate) fn en_el_fader(&self, x: u32, y: u32) -> bool {
        x >= self.fader_x
            && x < self.fader_x + self.fader_w
            && y + 8 >= self.arriba
            && y <= self.abajo + 8
    }

    /// El puntero esta en el boton de MUDO?
    pub(crate) fn en_el_mudo(&self, x: u32, y: u32) -> bool {
        let (mx, my, mw, mh) = self.mudo;
        x >= mx && x < mx + mw && y >= my && y < my + mh
    }
}

/// Al grano del fader, y dentro de su recorrido.
pub(crate) fn redondear(db: i32) -> i32 {
    let g = FADER_GRANO;
    let r = if db >= 0 { (db + g / 2) / g * g } else { -((-db + g / 2) / g * g) };
    r.clamp(FADER_MIN, FADER_MAX)
}

// ===================================================================
//  LOS NUMEROS: dB con un decimal
// ===================================================================

/// `+4.0`, `-12.5`, `0.0`, o `-inf` para el silencio del todo.
fn db_texto(db: i32, dst: &mut [u8], n: &mut usize) {
    if db <= NADA {
        pon(b"-inf", dst, n);
        return;
    }
    // A decimas, redondeando lejos de cero: 1/256 dB no se escribe.
    let d = if db >= 0 { (db * 10 + DB / 2) / DB } else { -((-db * 10 + DB / 2) / DB) };
    pon(if d > 0 { b"+" } else if d < 0 { b"-" } else { b"" }, dst, n);
    let a = d.unsigned_abs() as u64;
    let mut b = [0u8; 10];
    let k = decimal(a / 10, &mut b);
    pon(&b[..k], dst, n);
    pon(b".", dst, n);
    pon(&[b'0' + (a % 10) as u8], dst, n);
}

fn pon(s: &[u8], dst: &mut [u8], n: &mut usize) {
    for &b in s {
        if *n < dst.len() {
            dst[*n] = b;
            *n += 1;
        }
    }
}

fn entero(v: u64, dst: &mut [u8], n: &mut usize) {
    // Diez cifras: lo que cabe en `decimal`, y las cuentas de aqui son de 32 bits.
    let mut d = [0u8; 10];
    let k = decimal(v, &mut d);
    pon(&d[..k], dst, n);
}

/// El color de un nivel en dBFS, por su zona.
fn color_de(db: i32) -> u32 {
    if db >= -3 * DB {
        ROJO
    } else if db >= -12 * DB {
        AMBAR
    } else {
        VERDE
    }
}

// ===================================================================
//  PINTAR: la ventana entera, y las tres partes que se mueven
// ===================================================================

/// **Pinta la ventana entera.** La llaman los que la abren, la mueven o la
/// destapan; el refresco de 20 Hz pinta solo las partes vivas ([`vivo`]).
pub(crate) fn paint(p: &bmo::Pantalla, c: &SoundWindow, panel: &Panel) {
    if c.chrome.minimized {
        return;
    }
    // El cromo lo pinta el Marco: sombra, esquinas, barra de titulo y los tres
    // botones. Escrito a mano aqui un dia no casaria con el de las otras.
    c.chrome.paint_chrome(p, SND_EDGE, SND_BG, SND_TITLE_BG, SND_TITLE);
    c.chrome.paint_buttons(p, SND_TITLE_BG);

    let s = sitio(c);
    let l = leer();
    p.rect(s.tx, c.chrome.y + 9, 8, 8, SND_TITLE);
    let px = p.texto(s.tx + 16, c.chrome.y + 8, "Sonido", INK);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.chrome.y + 8, "maestro", INK_DIM);

    cabecera(p, &s, &l);

    // Las escalas: fijas, se pintan aqui y no en cada refresco.
    for &m in MARCAS_FADER.iter() {
        let y = s.y_fader(m * DB);
        let mut t = [0u8; 4];
        let mut n = 0;
        pon(if m > 0 { b"+" } else { b"" }, &mut t, &mut n);
        if m < 0 {
            pon(b"-", &mut t, &mut n);
        }
        entero(m.unsigned_abs() as u64, &mut t, &mut n);
        let ancho = n as u32 * bmo::GLIFO_ANCHO;
        let color = if m == 0 { INK } else { INK_DIM };
        p.texto_bytes((s.tx + 32).saturating_sub(ancho), y.saturating_sub(bmo::GLIFO_ALTO / 2), &t[..n], color);
    }
    for &m in MARCAS_MEDIDOR.iter() {
        let y = s.y_medidor(m * DB);
        let mut t = [0u8; 4];
        let mut n = 0;
        if m < 0 {
            pon(b"-", &mut t, &mut n);
        }
        entero(m.unsigned_abs() as u64, &mut t, &mut n);
        p.texto_bytes(s.escala_medidor_x, y.saturating_sub(bmo::GLIFO_ALTO / 2), &t[..n], INK_DIM);
    }
    p.texto(s.medidor_x[0] + 3, s.abajo + 4, "I", INK_DIM);
    p.texto(s.medidor_x[1] + 3, s.abajo + 4, "D", INK_DIM);

    fader(p, &s, &l);
    medidores(p, &s, &l, panel);
    numeros(p, &s, &l, panel);

    p.texto(
        s.tx,
        s.pie_y,
        "escribe los dB (40, -12) y Enter   flechas 1 dB   M mudo",
        INK_DIM,
    );
    p.texto(s.tx, s.pie_y + LINEA, "RePag/AvPag 6 dB   arrastra el fader   ESC o F10 cierra", INK_DIM);
    if panel.escrito_n > 0 {
        escrito(p, c, panel);
    }
}

/// **Lo que se esta escribiendo**, en la linea del aviso: se ve cada tecla, y
/// dice que hacer con ella. Mientras se escribe, esa linea es de la mano.
pub(crate) fn escrito(p: &bmo::Pantalla, c: &SoundWindow, panel: &Panel) {
    if c.chrome.minimized {
        return;
    }
    let s = sitio(c);
    p.rect(s.tx, s.y_aviso, s.num_x + s.num_w - s.tx, LINEA, SND_BG);
    let x = p.texto(s.tx, s.y_aviso, "maestro a ", INK);
    let x = p.texto_bytes(x, s.y_aviso, &panel.escrito[..panel.escrito_n], AMBAR);
    let x = p.texto(x, s.y_aviso, "_ dB", AMBAR);
    p.texto(x + 2 * bmo::GLIFO_ANCHO, s.y_aviso, "Enter pone   Retroceso borra   ESC deja", INK_DIM);
}

/// La linea del aparato y la del aviso.
fn cabecera(p: &bmo::Pantalla, s: &Sitio, l: &Lectura) {
    let mut t = [0u8; 64];
    let mut n = 0;
    if l.hay_aparato {
        pon(b"audifono USB", &mut t, &mut n);
        if l.frecuencia > 0 {
            pon(b"   ", &mut t, &mut n);
            entero(l.frecuencia as u64, &mut t, &mut n);
            pon(b" Hz", &mut t, &mut n);
        }
        if l.canales > 0 {
            pon(b"   ", &mut t, &mut n);
            entero(l.canales as u64, &mut t, &mut n);
            pon(if l.canales == 1 { b" canal" } else { b" canales" }, &mut t, &mut n);
        }
        p.texto_bytes(s.tx, s.y_aparato, &t[..n], INK);
    } else {
        p.texto(s.tx, s.y_aparato, "sin audifono USB: el maestro espera a que haya uno", INK_DIM);
    }
    // ** EL AVISO DICE SI EL MAESTRO ACTUA. Un fader que se mueve y no hace
    // nada es el peor control posible; si no actua, se escribe por que.
    let (aviso, color): (&str, u32) = match l.estado {
        2 => ("[!] el tubo no es PCM de 16 bits: el maestro NO actua", ROJO),
        3 => ("[!] la trama no cabe en la etapa: el maestro NO actua", ROJO),
        4 => ("[!] sin memoria para la etapa: el maestro NO actua", ROJO),
        0 => ("aun no ha pasado sonido por el tubo", INK_DIM),
        // ** EL TECHO FISICO, dicho. Por encima de 0 dBFS el aparato no da mas:
        // lo que el fader sube de ahi el limite lo tiene que bajar, y lo que se
        // oye no es mas fuerte, es APLASTADO (y el fondo sube con el).
        _ if l.reduccion < -6 * DB => ("[!] aplasta mas de 6 dB: subir ya no da mas fuerza", ROJO),
        _ if !l.tocado => ("sin tocar: el aparato tiene su volumen de fabrica", INK_DIM),
        _ => ("el maestro manda sobre todo lo que suena", INK_DIM),
    };
    p.texto(s.tx, s.y_aviso, aviso, color);
}

/// La columna del fader: la guia, el tramo digital, la raya de 0 y el mando.
fn fader(p: &bmo::Pantalla, s: &Sitio, l: &Lectura) {
    let alto = s.abajo - s.arriba;
    p.rect(s.fader_x, s.arriba.saturating_sub(6), s.fader_w, alto + 12, SND_BG);
    let cx = s.fader_x + s.fader_w / 2 - 2;
    // El tope del aparato: por encima de el, la etapa digital.
    let y_tope = s.y_fader(l.tope);
    p.rect(cx, s.arriba, 4, y_tope - s.arriba, SND_DIGITAL);
    p.rect(cx, y_tope, 4, s.abajo - y_tope, SND_BAR_GAP);
    for &m in MARCAS_FADER.iter() {
        let y = s.y_fader(m * DB);
        p.rect(s.fader_x + 4, y, 6, 1, INK_DIM);
    }
    p.rect(s.fader_x, y_tope, s.fader_w, 1, SND_TITLE);
    let f = l.fader_efectivo();
    let y = s.y_fader(f);
    let color = if l.mudo {
        INK_DIM
    } else if f > l.tope {
        AMBAR
    } else {
        INK
    };
    p.rect(s.fader_x + 2, y.saturating_sub(5), s.fader_w - 4, 10, color);
    p.rect(s.fader_x + 6, y, s.fader_w - 12, 1, SND_BG);
}

/// Las alturas que dibujan un medidor, para saber si hay que repintarlo.
fn medidas(s: &Sitio, l: &Lectura, panel: &Panel) -> [u32; 7] {
    [
        s.y_medidor(l.pico[0]),
        s.y_medidor(l.pico[1]),
        s.y_medidor(panel.retenido[0]),
        s.y_medidor(panel.retenido[1]),
        s.y_medidor(l.rms[0]),
        s.y_medidor(l.rms[1]),
        panel.luz_encendida() as u32,
    ]
}

/// Los dos medidores, izquierdo y derecho, con su luz de RECORTE encima.
fn medidores(p: &bmo::Pantalla, s: &Sitio, l: &Lectura, panel: &Panel) {
    let alto = s.abajo - s.arriba;
    let luz = if panel.luz_encendida() { ROJO } else { LUZ_APAGADA };
    for i in 0..2 {
        let x = s.medidor_x[i];
        let w = s.medidor_w;
        p.rect(x, s.luz_y, w, 8, luz);
        p.rect(x, s.arriba, w, alto, APAGADO);
        let y_pico = s.y_medidor(l.pico[i]);
        // Encendido desde abajo hasta el pico, cada zona con su color.
        for (desde_db, hasta_db, color) in [
            (-60 * DB, -12 * DB, VERDE),
            (-12 * DB, -3 * DB, AMBAR),
            (-3 * DB, 0, ROJO),
        ] {
            let y_baja = s.y_medidor(desde_db);
            let y_alta = s.y_medidor(hasta_db).max(y_pico);
            if y_alta < y_baja {
                p.rect(x, y_alta, w, y_baja - y_alta, color);
            }
        }
        // La marca de pico que se queda, y el RMS: la fuerza que se oye.
        let r = panel.retenido[i];
        if r > FADER_MIN {
            p.rect(x, s.y_medidor(r), w, 2, color_de(r));
        }
        if l.rms[i] > -60 * DB {
            p.rect(x, s.y_medidor(l.rms[i]), w, 2, RMS_MARCA);
        }
    }
}

/// La columna de numeros y el boton de MUDO.
fn numeros(p: &bmo::Pantalla, s: &Sitio, l: &Lectura, panel: &Panel) {
    p.rect(s.num_x, s.arriba.saturating_sub(2), s.num_w, LINEAS_NUMEROS * LINEA + 4, SND_BG);
    let mut y = s.arriba;
    let mut fila = |etiqueta: &[u8], cuerpo: &[u8], color: u32| {
        let x = p.texto_bytes(s.num_x, y, etiqueta, INK_DIM);
        p.texto_bytes(x, y, cuerpo, color);
        y += LINEA;
    };
    let f = l.fader_efectivo();

    let mut t = [0u8; 32];
    let mut n = 0;
    db_texto(f, &mut t, &mut n);
    pon(b" dB", &mut t, &mut n);
    fila(b"maestro  ", &t[..n], if f > l.tope { AMBAR } else { INK });

    let mut t = [0u8; 40];
    let mut n = 0;
    let aparato = if l.tocado { Some(l.aparato) } else { l.fabrica };
    match aparato {
        _ if !l.hay_aparato => pon(b"sin aparato", &mut t, &mut n),
        Some(a) => {
            db_texto(a, &mut t, &mut n);
            pon(b" dB", &mut t, &mut n);
            if !l.tocado {
                pon(b" de fabrica", &mut t, &mut n);
            } else if a >= l.tope {
                pon(b" (su tope)", &mut t, &mut n);
            }
        }
        None => pon(b"el de fabrica, sin leer", &mut t, &mut n),
    }
    fila(b"aparato  ", &t[..n], INK);

    let mut t = [0u8; 32];
    let mut n = 0;
    db_texto(l.digital, &mut t, &mut n);
    pon(b" dB", &mut t, &mut n);
    fila(b"digital  ", &t[..n], if l.digital > 0 { AMBAR } else { INK });

    for (nombre, v) in [(b"pico     ", l.pico), (b"rms      ", l.rms)] {
        let mut t = [0u8; 32];
        let mut n = 0;
        db_texto(v[0], &mut t, &mut n);
        pon(b"  ", &mut t, &mut n);
        db_texto(v[1], &mut t, &mut n);
        fila(nombre, &t[..n], INK);
    }

    let mut t = [0u8; 32];
    let mut n = 0;
    if l.reduccion < 0 {
        db_texto(l.reduccion, &mut t, &mut n);
        pon(b" dB", &mut t, &mut n);
    } else {
        pon(b"nada", &mut t, &mut n);
    }
    let color_limite = if l.reduccion < -6 * DB {
        ROJO
    } else if l.reduccion < 0 {
        AMBAR
    } else {
        INK
    };
    fila(b"aplasta  ", &t[..n], color_limite);

    let mut t = [0u8; 32];
    let mut n = 0;
    entero(l.dobladas, &mut t, &mut n);
    fila(b"recorte  ", &t[..n], if panel.luz_encendida() { ROJO } else { INK });

    let (mx, my, mw, mh) = s.mudo;
    p.rect(mx, my, mw, mh, if l.mudo { ROJO } else { SND_BAR_GAP });
    p.texto(mx + 8, my + 5, if l.mudo { "MUDO  (M)" } else { "mudo  (M)" }, if l.mudo { INK } else { INK_DIM });
}

/// **El refresco de 20 Hz**: solo las partes que cambiaron. La ventana
/// entera son ~200.000 pixeles; el medidor, unos 7.000.
pub(crate) fn vivo(p: &bmo::Pantalla, c: &SoundWindow, l: &Lectura, panel: &mut Panel) {
    if c.chrome.minimized {
        return;
    }
    let s = sitio(c);
    let f = (l.fader_efectivo(), l.mudo);
    if panel.visto_fader != Some(f) {
        fader(p, &s, l);
        panel.visto_fader = Some(f);
    }
    let m = medidas(&s, l, panel);
    if panel.visto_medidor != Some(m) {
        medidores(p, &s, l, panel);
        panel.visto_medidor = Some(m);
    }
    // Los numeros, sin las ventanas (que suben siempre): si no, se repintarian
    // en cada refresco aunque no cambiara nada que se lea.
    let mut sin = *l;
    sin.ventanas = 0;
    if panel.visto_numeros != Some(sin) {
        cabecera_si_cambia(p, &s, l, panel);
        numeros(p, &s, l, panel);
        panel.visto_numeros = Some(sin);
        // Si la cabecera se repinto a media escritura, lo escrito vuelve.
        if panel.escrito_n > 0 {
            escrito(p, c, panel);
        }
    }
}

fn cabecera_si_cambia(p: &bmo::Pantalla, s: &Sitio, l: &Lectura, panel: &Panel) {
    let cambia = match panel.visto_numeros {
        Some(v) => {
            v.estado != l.estado || v.tocado != l.tocado || v.hay_aparato != l.hay_aparato
                || v.frecuencia != l.frecuencia || v.canales != l.canales
        }
        None => true,
    };
    if cambia {
        p.rect(s.tx, s.y_aparato, s.num_x + s.num_w - s.tx, 2 * LINEA, SND_BG);
        cabecera(p, s, l);
    }
}

// ===================================================================
//  EL INDICADOR DE LA BARRA
// ===================================================================

/// Letras del texto del indicador: `vol +24.0` son nueve.
const BARRA_LETRAS: u32 = 9;

/// `(x, y, ancho, alto)` del indicador: al pie del panel de la izquierda
/// (2026-09-22; iba a la derecha de la barra de arriba, que se fundio en el
/// panel). `None` con el panel escondido: entonces no hay indicador, y el
/// maestro sigue en F10.
pub(crate) fn barra_caja(alto: u32) -> Option<(u32, u32, u32, u32)> {
    super::lateral::caja_vol(alto)
}

/// El puntero esta sobre el indicador?
pub(crate) fn en_la_barra(x: u32, y: u32, alto: u32) -> bool {
    match barra_caja(alto) {
        Some((bx, by, bw, bh)) => x >= bx && x < bx + bw && y >= by && y < by + bh,
        None => false,
    }
}

static mut BARRA_VISTA: Option<([u8; 12], usize, [u32; 2])> = None;

/// La barra se repinto debajo: lo pintado se da por perdido.
pub(crate) fn olvidar_barra() {
    unsafe { *core::ptr::addr_of_mut!(BARRA_VISTA) = None };
}

/// **Pinta el indicador de la barra**, si cambio lo que dice.
pub(crate) fn barra(p: &bmo::Pantalla, l: &Lectura) {
    let Some((bx, by, bw, bh)) = barra_caja(p.alto) else {
        return;
    };
    let mut t = [0u8; 12];
    let mut n = 0;
    pon(b"vol ", &mut t, &mut n);
    if l.mudo {
        pon(b"MUDO", &mut t, &mut n);
    } else if !l.hay_aparato {
        pon(b"--", &mut t, &mut n);
    } else {
        db_texto(l.fader_efectivo(), &mut t, &mut n);
    }
    let alto = bh.saturating_sub(12);
    let nivel = |db: i32| alto * (1000 - a_milesimas(db, &ESCALA_MEDIDOR)) / 1000;
    let barras = [nivel(l.pico[0]), nivel(l.pico[1])];
    let vista = unsafe { &mut *core::ptr::addr_of_mut!(BARRA_VISTA) };
    if let Some((vt, vn, vb)) = vista {
        if *vn == n && vt[..n] == t[..n] && *vb == barras {
            return;
        }
    }
    *vista = Some((t, n, barras));

    p.rect(bx, by, bw, bh, super::estilo::estilo().barra_fondo);
    let ty = by + (bh.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    let x = p.texto_bytes(bx + 2, ty, &t[..4], INK_DIM);
    let color = if l.mudo {
        ROJO
    } else if l.fader_efectivo() > l.tope {
        AMBAR
    } else {
        INK
    };
    p.texto_bytes(x, ty, &t[4..n], color);
    let mx = bx + BARRA_LETRAS * bmo::GLIFO_ANCHO + 8;
    for (i, &h) in barras.iter().enumerate() {
        let x = mx + i as u32 * 7;
        let base = by + 6 + alto;
        p.rect(x, by + 6, 5, alto, APAGADO);
        if h > 0 {
            let db = if i == 0 { l.pico[0] } else { l.pico[1] };
            p.rect(x, base - h, 5, h, color_de(db));
        }
    }
}
