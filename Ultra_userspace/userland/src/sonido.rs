//! El SONIDO: `KIND_AUDIO` visto desde Ring 3.
//!
//! Lo que hay al otro lado es un CONTRATO, no un motor de audio: el derecho a
//! hacer ruido, exclusivo, que se recupera solo cuando su propietario muere. El
//! driver de HD Audio es otra cosa y todavia no existe -- ver
//! `docs/plan/PLAN_DOOM.md`, fase 5.
//!
//! Por eso lo unico que suena hoy es el altavoz del PC, y [`Sonido::aparatos`]
//! lo dice en vez de que haya que suponerlo. **Preguntar y no suponer** es todo
//! el motivo de que esa operacion exista: el dia que haya HDA, el mismo binario
//! se entera sin recompilarse.

use crate::*;

/// El sonido, cedido a este proceso.
///
/// Exclusivo como la pantalla: un solo proceso lo tiene a la vez. Dos propietarios
/// escribiendo en el mismo aparato no es mezclar, es ruido -- y mezclar es
/// trabajo de Ring 3, igual que componer ventanas.
pub struct Sonido {
    pub cap: u64,
}

impl Sonido {
    /// Reclamarlo. `None` si ya lo tiene otro proceso.
    pub fn claim() -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_AUDIO_CLAIM, 0, 0, 0).valor()?;
        Some(Self { cap })
    }

    /// Soltarlo y seguir vivo. Consume el `Sonido`, igual que `Pantalla::release`.
    ///
    /// Va desde el primer dia por lo que costo que faltara en la pantalla: sin
    /// esto, el primer programa que pite se queda el altavoz hasta que muera.
    ///
    /// Devuelve `false` si no era el propietario, en vez de fingir que lo solto.
    pub fn release(self) -> bool {
        invoke(CURRENT_TASK, OP_AUDIO_RELEASE, 0, 0, 0).valor().is_some()
    }

    /// Que aparatos hay: mascara de [`DEVICE_SPEAKER`] y [`DEVICE_HDA`].
    ///
    /// [!] Un bit puesto dice que **hay camino**, no que se vaya a oir algo. El
    /// puerto del altavoz existe en todo x86; el zumbador fisico, no -- muchas
    /// placas modernas traen el cabezal SPKR sin nada conectado, y desde aqui
    /// no hay forma de saberlo.
    pub fn aparatos(&self) -> u64 {
        invoke(self.cap, AUDIO_OP_DEVICES, 0, 0, 0).valor().unwrap_or(0)
    }

    /// Pitar. Devuelve los milisegundos que de verdad sono.
    ///
    /// [!] **BLOQUEA.** Mientras dura, este nucleo no hace otra cosa: el altavoz
    /// del PC no tiene interrupcion que avise de que el tono acabo. El kernel
    /// recorta a [`AUDIO_MAX_MS`], asi que pedir mas no cuelga la maquina --
    /// pero tampoco suena mas.
    pub fn pitar(&self, freq_hz: u32, ms: u32) -> u64 {
        invoke(self.cap, AUDIO_OP_BEEP, freq_hz as u64, ms as u64, 0)
            .valor()
            .unwrap_or(0)
    }

    /// Volumen de 0 a 100. Devuelve el que quedo puesto.
    ///
    /// En el altavoz del PC esto no es un fundido: son **dos escalones**, porque
    /// el volumen se consigue cambiando el modo del PIT --pulsos estrechos
    /// suenan mas flojo que una onda cuadrada al 50%-- y no hay mas modos.
    pub fn volumen(&self, v: u8) -> u64 {
        invoke(self.cap, AUDIO_OP_VOLUME, v as u64, 0, 0).valor().unwrap_or(0)
    }

    /// Callar ahora mismo.
    pub fn callar(&self) {
        let _ = invoke(self.cap, AUDIO_OP_SILENCE, 0, 0, 0);
    }
}

// ===================================================================
//  EL TUBO: lo unico que de verdad suena en esta maquina
// ===================================================================

/// **El tubo isocrono del audifono USB, con su bufer PRESTADO.**
///
/// *** ESTO FALTABA, Y ERA EL BLOQUEO (2026-09-22). El kernel expone el
/// contrato entero del productor desde A4 --ofrecer un bloque, decir hasta
/// donde se escribio, preguntar por donde va el aparato-- y `musica.inti` lo
/// usa. Pero desde Rust **no se podia**: el unico envoltorio era
/// [`crate::sys::audio_tubo`], que manda `(que, 0, 0)` y por tanto no puede
/// pasar el argumento que llevan `ofrecer` y `escrito`; y ademas reclamaba y
/// soltaba el aparato en cada llamada, o sea que no se podia sostener mientras
/// suena.
///
/// Consecuencia, dicha por su nombre: **ningun programa de Ring 3 escrito en
/// Rust podia hacer ruido**, y eso incluye al DIRECTOR. Por eso el
/// amplificador ([`bmo-amplificador`]) no tenia donde enchufarse: la pieza
/// estaba, el cable no.
///
/// # Los dos numeros que cruzan, y por que son dos
///
/// ```text
///    escrito   hasta donde ha llenado la APP        (solo crece)
///    leido     por donde va el APARATO              (lo dice el kernel)
/// ```
///
/// Con esos dos y un bloque en medio no hace falta una puerta por muestra: la
/// app escribe delante del aparato y el aparato come detras de la app. Cero
/// copias (el xHC lee la memoria de la app por DMA) y una puerta por vuelta,
/// no por trama.
pub struct Tubo<'a> {
    sonido: &'a Sonido,
}

/// Los campos de `AUDIO_OP_TUBO`. Se nombran para no escribir numeros sueltos
/// en el sitio de la llamada; el orden lo fija el kernel (`obj/audio.rs`).
const TUBO_ABIERTO: u64 = 0;
const TUBO_ARMAR: u64 = 1;
const TUBO_CALLAR: u64 = 2;
const TUBO_BYTES_TRAMA: u64 = 3;
const TUBO_FRECUENCIA: u64 = 4;
const TUBO_ENCOLADAS: u64 = 5;
const TUBO_TARDE: u64 = 6;
const TUBO_ARMADO: u64 = 7;
const TUBO_OFRECER: u64 = 8;
const TUBO_ESCRITO: u64 = 9;
const TUBO_LEIDO: u64 = 10;
const TUBO_PENDIENTES: u64 = 11;
const TUBO_HUECOS: u64 = 12;
const TUBO_SOLTAR: u64 = 13;
const TUBO_ANILLO: u64 = 14;

impl Sonido {
    /// El tubo de este aparato, mientras se tenga el sonido reclamado.
    pub fn tubo(&self) -> Tubo<'_> {
        Tubo { sonido: self }
    }
}

impl Tubo<'_> {
    fn pedir(&self, que: u64, dato: u64) -> u64 {
        invoke(self.sonido.cap, AUDIO_OP_TUBO, que, dato, 0).valor().unwrap_or(0)
    }

    /// **Hay tubo?** `false` = no hay audifono, o no dio sus papeles. Es la
    /// primera pregunta de cualquier productor: sin esto, lo demas es escribir
    /// en un bloque que nadie lee.
    pub fn abierto(&self) -> bool {
        self.pedir(TUBO_ABIERTO, 0) != 0
    }

    /// **Bytes por milisegundo**, que es lo que el aparato come de una vez.
    ///
    /// [!] De aqui salen los CANALES, y no hay otra forma de saberlos: a
    /// 48.000 Hz, `192 = 48 muestras x 2 canales x 2 bytes`. Suponer estereo
    /// es exactamente el error que este numero existe para evitar. Ver
    /// `docs/plan/PLAN_EL_SONIDO.md` 1.4.
    pub fn bytes_por_trama(&self) -> u64 {
        self.pedir(TUBO_BYTES_TRAMA, 0)
    }

    /// La frecuencia que el aparato acepto, en Hz. **No se supone.**
    pub fn frecuencia(&self) -> u64 {
        self.pedir(TUBO_FRECUENCIA, 0)
    }

    /// Empezar a empujar tramas (silencio si no hay nada que mandar).
    ///
    /// [!] Armar es TRAFICO: 250 latidos por segundo. Por eso no se enciende
    /// solo y hay que pedirlo, y por eso [`Tubo::callar`] existe.
    pub fn armar(&self) -> bool {
        self.pedir(TUBO_ARMAR, 0) != 0
    }

    /// Dejar de empujar.
    pub fn callar(&self) {
        let _ = self.pedir(TUBO_CALLAR, 0);
    }

    pub fn armado(&self) -> bool {
        self.pedir(TUBO_ARMADO, 0) != 0
    }

    /// **Prestar un bloque propio al aparato.** `va` es la direccion del
    /// bloque tal como lo devolvio la peticion de memoria; los bytes los mira
    /// el kernel en el bloque, no se los dice la app (una app no declara la
    /// medida de su propia memoria: se la preguntan al que la dio).
    ///
    /// `false` = esa memoria no es suya, o no hay tubo al que ofrecerla.
    pub fn ofrecer(&self, va: u64) -> bool {
        self.pedir(TUBO_OFRECER, va) != 0
    }

    /// **Hasta donde ha llenado la app**, en bytes desde el principio del
    /// bloque. Solo puede CRECER: un `escrito` que retrocede es la app pisando
    /// lo que el aparato no ha leido, y eso se oye.
    pub fn escrito(&self, hasta: u64) -> bool {
        self.pedir(TUBO_ESCRITO, hasta) != 0
    }

    /// Por donde va el APARATO. La distancia con `escrito` es lo que queda
    /// por sonar.
    pub fn leido(&self) -> u64 {
        self.pedir(TUBO_LEIDO, 0)
    }

    /// Lo escrito y aun no sonado, en bytes. **Es el mando del productor**: si
    /// baja de una trama, el aparato se queda sin nada y eso es un hueco.
    pub fn pendientes(&self) -> u64 {
        self.pedir(TUBO_PENDIENTES, 0)
    }

    /// Vueltas en las que no habia trama que mandar. **Tiene que ser cero**;
    /// si sube, el productor no llega.
    pub fn huecos(&self) -> u64 {
        self.pedir(TUBO_HUECOS, 0)
    }

    /// Tramas mandadas al bus desde el arranque. Tiene que SUBIR SOLA mientras
    /// algo suena: es la prueba de que las muestras salen de verdad.
    pub fn encoladas(&self) -> u64 {
        self.pedir(TUBO_ENCOLADAS, 0)
    }

    /// Tramas que el xHC no llego a servir en su microtrama. **Es la cifra que
    /// separa "suena bien" de "chasquea".**
    pub fn tarde(&self) -> u64 {
        self.pedir(TUBO_TARDE, 0)
    }

    /// **La medida del ANILLO**: los bytes del bloque redondeados hacia abajo
    /// a un numero entero de tramas. **Es donde hay que dar la vuelta.**
    ///
    /// *** SIN ESTE NUMERO EL ANILLO NO ERA UN ANILLO (2026-09-22). El kernel
    /// daba la vuelta en `bytes` y la app tenia que adivinarlo; y como
    /// `bytes` no suele ser multiplo de una trama, el corte caia en mitad de
    /// una muestra. Peor: `pendientes` se calculaba con una resta a secas, y
    /// **en cuanto la app daba la vuelta el tubo se quedaba sin nada que
    /// mandar** hasta que el aparato llegara al final -- lo que no pasaba
    /// nunca. Por eso todos los productores acababan volviendo a OFRECER el
    /// bloque para poner los indices a cero: un rodeo, no un esquema.
    pub fn anillo(&self) -> u64 {
        self.pedir(TUBO_ANILLO, 0)
    }

    /// Devolver el bloque. Se hace solo si el proceso muere, pero un programa
    /// que sigue vivo tiene que poder soltarlo sin morirse.
    pub fn soltar(&self) {
        let _ = self.pedir(TUBO_SOLTAR, 0);
    }
}
