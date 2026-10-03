//! **EL MAESTRO: la ultima etapa del sonido, pegada al cable** (S4c de
//! `docs/plan/PLAN_EL_SONIDO.md`).
//!
//! [carril]  AMARILLO  copia cada trama a un marco SUYO y le pasa la ganancia;
//!                     lo que el xHC lee sale de ese marco, no del de la app
//! [consumo] APARATO   solo con el tubo armado: lo llama `audio::latido`, en el
//!                     hilo del bus, una vez por trama
//!
//! [cuesta]  MAQUINA -- escribe por el physmap: lee el bloque de la app (el
//!           tramo ya lo juzgo `siguiente_trama`) y escribe en SU marco, en la
//!           ranura `RANURA x paso`, que `ranuras x paso <= PAGE` deja dentro.
//!           Una cuenta mal hecha ahi no suena mal: pisa memoria.
//!
//! [riesgo]  SILENCIO RELOJ
//!           SILENCIO: una ganancia mal puesta no falla, SUENA -- y lo peor que
//!           puede sonar es FUERTE. Por eso el mando solo lo mueve el
//!           escritorio, el techo es +24 dB (el del crate) y el limite de
//!           detras no deja salir ni una muestra de 16 bits. RELOJ: corre en el
//!           latido del bus, una vez por trama, y lo que tarde se lo quita al
//!           teclado y al raton.
//!
//! # Lo que cuesta, medido en trabajo
//!
//! Por trama (1 ms): copiar 192 bytes y 96 multiplicaciones con su
//! comparacion. Unos microsegundos por latido de 4 ms, contra un presupuesto
//! de 3.000 us del hilo del bus. **En reposo** --0 dB y sin mudo-- ni eso: el
//! xHC sigue leyendo del bufer de la app, como antes de que esto existiera, y
//! aqui solo se mira.
//!
//! # *** POR QUE ESTO VIVE EN EL KERNEL, SI EL PLAN DECIA LO CONTRARIO
//!
//! `PLAN_EL_SONIDO.md` decia *"nada de esto es del kernel: mezclar, amplificar
//! y situar es Ring 3"*, y `LIDERES.md` dibuja un `audio.bex` que reparte. El
//! 2026-09-22 el propietario, con DOOM sonando, pidio un control MAESTRO en el
//! escritorio, y la decision se le puso delante con sus tres caminos:
//!
//! ```text
//!    kernel, ultima etapa   vale para DOOM, musica y cualquiera SIN TOCARLOS
//!    el escritorio mezcla   el arbol entero, pero si el escritorio se atasca
//!                           (un save, abrir una imagen) se corta TODO
//!    solo el aparato        nada nuevo, y no sube de 0 dB: no amplifica
//! ```
//!
//! Eligio el primero. **Lo que sigue siendo de Ring 3** es todo lo demas:
//! mezclar varias fuentes, el arbol de pistas, situar en el espacio. El maestro
//! es la perilla de la HABITACION, no la de nadie en concreto, y por eso va
//! donde pasa todo lo que suena: detras del unico productor, antes del cable.
//!
//! # El reparto
//!
//! ```text
//!    bmo_amplificador::maestro   DECIDE   la rampa, el limite, el medidor
//!    este fichero                MUEVE    los bytes al marco y lo publica
//!    `op_aparato::audio_mando`   PERMITE  solo quien tiene la pantalla
//! ```
//!
//! # Por que se COPIA, si A4 se escribio para no copiar
//!
//! A4 (`audio.rs`) decia `MAL: el kernel copia 192 bytes a su anillo, mil
//! veces por segundo, mil cruces de puerta y mil copias`. Lo caro de aquella
//! frase eran **las mil puertas**: una llamada por trama. Esto no cruza
//! ninguna: la app sigue escribiendo en su bloque y diciendo hasta donde, y la
//! copia la hace el hilo del bus por dentro. Y hay un motivo para no modificar
//! las muestras en el sitio: **el bloque es de la app**. El kernel no escribe
//! en la memoria de otro, ni para mejorarla.
//!
//! El kernel LEE ese bloque por el *physmap*, no por la direccion de la app, y
//! por eso SMAP no tiene nada que decir: la pagina del physmap es del kernel.

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, Ordering};

use bmo_amplificador::maestro::Maestro;
use bmo_amplificador::espacio::{Ajuste, Espacio, Modo};
use bmo_amplificador::oido::Perfil;

use crate::ring0::cabina;

// ===================================================================
//  EL MANDO: lo escribe el escritorio, lo lee el bus
// ===================================================================

/// **El fader**, en 1/256 dB: lo que el escritorio pidio, entero.
static MANDO_DB: AtomicI32 = AtomicI32::new(0);
/// La parte que pone la etapa DIGITAL: el fader menos lo que da el aparato.
static DIGITAL_DB: AtomicI32 = AtomicI32::new(0);
/// La parte que se le pidio al APARATO. Solo vale con [`TOCADO`].
static APARATO_DB: AtomicI32 = AtomicI32::new(0);
static MUDO: AtomicBool = AtomicBool::new(false);
/// El escritorio ha movido el maestro alguna vez. Antes de eso el aparato
/// tiene el volumen que trajo (el de fabrica) y NO se le toca.
static TOCADO: AtomicBool = AtomicBool::new(false);

/// El techo del fader: +52 dB, el del MAESTRO en el crate (2026-09-22; era
/// +24). Por encima de ~+24 lo que sube es lo flojo y el limite sujeta la
/// punta: es compresion, y el panel lo dice. Ver `MAX_DB_MAESTRO`.
pub const TECHO_DB: i32 = bmo_amplificador::MAX_DB_MAESTRO;
/// El suelo: -96 dB, donde una muestra de 16 bits ya es cero.
pub const SUELO_DB: i32 = bmo_amplificador::MIN_DB;

/// **El escritorio mueve el fader.** Devuelve lo que quedo puesto.
///
/// La cuenta de las dos etapas vive aqui y no en el escritorio porque solo
/// aqui se sabe el rango del aparato en este instante:
///
/// ```text
///    fader dentro del rango del aparato    el APARATO lo da, limpio; digital 0
///    fader por encima de su tope           aparato a tope + digital el resto
///    fader por debajo de su suelo          aparato al suelo + digital el resto
/// ```
///
/// Sin aparato con volumen, todo es digital.
pub fn mover(db: i32) -> i32 {
    let db = db.clamp(SUELO_DB, TECHO_DB);
    let (aparato, digital) = match crate::ring0::dev::uaudio::rango_db() {
        Some((min, max)) => {
            let a = db.clamp(min as i32, max as i32);
            (Some(a), db - a)
        }
        None => (None, db),
    };
    if let Some(a) = aparato {
        // Solo si cambia: cada peticion son dos transferencias en el bus.
        if !TOCADO.load(Ordering::SeqCst) || APARATO_DB.load(Ordering::SeqCst) != a {
            crate::ring0::dev::uaudio::pedir_volumen_db(a as i16);
        }
        APARATO_DB.store(a, Ordering::SeqCst);
    }
    DIGITAL_DB.store(digital, Ordering::SeqCst);
    MANDO_DB.store(db, Ordering::SeqCst);
    TOCADO.store(true, Ordering::SeqCst);
    db
}

/// **El escritorio calla o descalla.** Es digital, con su rampa: el mute del
/// aparato es un corte seco y ademas no todos lo tienen.
pub fn callar(si: bool) {
    MUDO.store(si, Ordering::SeqCst);
}

/// **El OIDO** (03-10): el perfil de quien escucha, empaquetado
/// (`Perfil::empaquetar`). Lo escribe el escritorio, lo lee el bus en cada
/// trama. `0` es el plano.
static OIDO: AtomicU64 = AtomicU64::new(0);

/// **El escritorio mueve un mando del oido.** `que`: 3 balance (-100..100),
/// 4 mono (0/1), 5 graves, 6 medios, 7 agudos (dB, -12..12), 8 todo plano.
/// Devuelve el perfil que quedo, empaquetado.
pub fn oido(que: u64, valor: i64) -> u64 {
    let mut p = Perfil::desempaquetar(OIDO.load(Ordering::SeqCst));
    let v = valor.clamp(-1000, 1000) as i32;
    match que {
        3 => p.balance = v,
        4 => p.mono = v != 0,
        5 => p.graves = v,
        6 => p.medios = v,
        7 => p.agudos = v,
        8 => p = Perfil::PLANO,
        _ => {}
    }
    let x = p.recortado().empaquetar();
    OIDO.store(x, Ordering::SeqCst);
    x
}

/// **EL ESPACIO** (03-10, S7): el modo 3D, empaquetado (`Ajuste`). Lo escribe
/// el escritorio, lo lee el bus.
static ESPACIO_AJUSTE: AtomicU64 = AtomicU64::new(0);
/// El espacio en marcha. En un `static` y no dentro de la etapa: sus lineas
/// de sala son ~4 KiB y la etapa se crea en la pila del bus.
static mut ESPACIO: Espacio = Espacio::nuevo(48_000); // [escribe] bombeo

/// **El escritorio mueve el 3D.** `que`: 9 el modo (0 apagado, 1 cerca,
/// 2 sala, 3 amplio, 4 orbita), 10 los segundos por vuelta de la orbita.
/// Devuelve el ajuste que quedo, empaquetado.
pub fn espacio(que: u64, valor: i64) -> u64 {
    let mut a = Ajuste::desempaquetar(ESPACIO_AJUSTE.load(Ordering::SeqCst));
    match que {
        9 => a.modo = Modo::de(valor.clamp(0, 255) as u64),
        10 => a.vuelta_s = valor.clamp(2, 60) as u32,
        _ => {}
    }
    let x = a.recortado().empaquetar();
    ESPACIO_AJUSTE.store(x, Ordering::SeqCst);
    x
}

/// `INFO_AUDIO_ESPACIO`: `[0..8)` el modo | `[8..16)` segundos por vuelta |
/// bit 48: se aplica a la frecuencia del tubo (44,1 o 48 kHz).
pub fn info_espacio() -> u64 {
    let hz = super::audio::tubo().map(|t| t.frecuencia).unwrap_or(0);
    ESPACIO_AJUSTE.load(Ordering::SeqCst) | (((hz == 44_100 || hz == 48_000) as u64) << 48)
}

/// El espacio de esta trama, al dia con su ajuste y la frecuencia del tubo.
///
/// # Safety
/// Desde el hilo del bus: es su unico escritor.
unsafe fn espacio_al_dia(hz: u32) -> &'static mut Espacio {
    let e = &mut *core::ptr::addr_of_mut!(ESPACIO);
    e.frecuencia(hz);
    e.poner(Ajuste::desempaquetar(ESPACIO_AJUSTE.load(Ordering::SeqCst)));
    e
}

/// `INFO_AUDIO_OIDO`: `[0..40)` el perfil empaquetado | bit 48: el TONO se
/// aplica a la frecuencia del tubo (44,1 o 48 kHz; mono y balance siempre).
pub fn info_oido() -> u64 {
    let hz = super::audio::tubo().map(|t| t.frecuencia).unwrap_or(0);
    OIDO.load(Ordering::SeqCst) | (((hz == 44_100 || hz == 48_000) as u64) << 48)
}

// ===================================================================
//  LA ETAPA: la corre el hilo del bus
// ===================================================================

/// La etapa, con la frecuencia para la que se creo.
static mut ETAPA: Option<(u32, u8, Maestro)> = None; // [escribe] bombeo
/// El marco fisico donde se copian las tramas. `0` = aun no se pidio.
static mut REBOTE: u64 = 0; // [escribe] bombeo
/// La siguiente ranura del rebote.
static mut RANURA: u32 = 0; // [escribe] bombeo
/// Tramas desde que se cerro la ultima ventana del medidor.
static mut EN_VENTANA: u32 = 0; // [escribe] bombeo

/// **Cada cuantas tramas se cierra la ventana del medidor**: 50 ms. El
/// escritorio lee 20 veces por segundo, asi que cada lectura ve una ventana
/// entera y ninguna a medias.
const TRAMAS_POR_VENTANA: u32 = 50;

/// **Ranuras minimas del rebote.** Una ranura no se puede volver a escribir
/// mientras el xHC aun no la ha leido, y en vuelo hay como mucho
/// `TRAMAS_EN_VUELO` (8). El doble deja el margen de un latido que el xHC
/// tarde en servir.
const RANURAS_MINIMAS: u32 = 16;

// ** Lo que se publica para `OP_INFO`. Lo escribe el bus y lo lee quien sea:
// atomicos y no `static mut`, porque aqui SI hay dos hilos.
static ESTADO: AtomicU8 = AtomicU8::new(ESTADO_SIN_TUBO);
static ACTUAL: AtomicI32 = AtomicI32::new(0);
/// Cuatro `i16`: pico izq, pico der, rms izq, rms der, en 1/256 dBFS.
///
/// ** NACE EN SILENCIO, no en cero (2026-09-22). Era `new(0)`, y un cero en
/// dBFS es **0 dB: el maximo**. Hasta que pasaba la primera muestra por el tubo
/// cualquiera que leyera el medidor veia la saturacion entera: la captura del
/// metal de las 22:59 lo mostro en rojo en el panel y en la ventana del sonido,
/// con el panel diciendo "aun no ha pasado sonido por el tubo" al lado. El
/// silencio del amplificador es `MIN_DB`, en los cuatro.
static MEDIDA: AtomicU64 = AtomicU64::new(SILENCIO);
const SILENCIO: u64 = {
    let s = (bmo_amplificador::MIN_DB as i16 as u16) as u64;
    s | (s << 16) | (s << 32) | (s << 48)
};
/// `[0..32)` dobladas | `[32..48)` reduccion del limite (i16) | `[48..64)` ventanas.
static LIMITE: AtomicU64 = AtomicU64::new(0);
static VENTANAS: AtomicU64 = AtomicU64::new(0);

/// Aun no ha pasado ninguna trama por aqui.
pub const ESTADO_SIN_TUBO: u8 = 0;
/// En marcha: las tramas pasan por la etapa.
pub const ESTADO_EN_MARCHA: u8 = 1;
/// El tubo no es PCM de 16 bits: se manda sin tocar y **el maestro no hace
/// nada**. Se dice, porque un fader que no mueve nada es un mando que miente.
pub const ESTADO_NO_ES_16: u8 = 2;
/// Una trama no cabe en las ranuras del rebote (un aparato de 768 B por ms):
/// se manda sin tocar.
pub const ESTADO_NO_CABE: u8 = 3;
/// No hubo marco para el rebote: se manda sin tocar.
pub const ESTADO_SIN_MARCO: u8 = 4;

/// La etapa para `hz`, creada o recreada si el tubo cambio de frecuencia.
/// La etapa para `hz` y `canales`, creada o recreada si el tubo cambio.
///
/// ** Con los CANALES, desde el 22-09: el limite ve las muestras intercaladas,
/// y creado con `hz` a secas sus tiempos eran la mitad de lo que decian.
unsafe fn etapa(hz: u32, canales: u8) -> &'static mut Maestro {
    let hace_falta = !matches!(ETAPA, Some((f, c, _)) if f == hz && c == canales);
    if hace_falta {
        ETAPA = Some((hz, canales, Maestro::nuevo(hz, canales.max(1) as u32)));
    }
    let (_, _, m) = ETAPA.as_mut().unwrap();
    m.pedir(DIGITAL_DB.load(Ordering::SeqCst), MUDO.load(Ordering::SeqCst));
    m.oido(Perfil::desempaquetar(OIDO.load(Ordering::SeqCst)));
    m
}

/// **Una trama de la app por la etapa.** Devuelve la fisica que hay que
/// encolar: la del rebote, o la de la app tal cual si la etapa no puede o no
/// tiene nada que hacer.
///
/// # Safety
/// Desde el hilo del bus, con `desde` ya juzgada (`siguiente_trama`).
pub unsafe fn pasar(desde: u64, n: u16, t: &super::audio::Tubo) -> u64 {
    if t.bits != 16 {
        ESTADO.store(ESTADO_NO_ES_16, Ordering::SeqCst);
        return desde;
    }
    let canales = t.canales.max(1) as usize;
    let m = etapa(t.frecuencia, t.canales);
    let muestras = n as usize / 2;
    let origen = crate::ring0::mm::phys_to_virt(desde) as *const i16;

    let esp = espacio_al_dia(t.frecuencia);
    if m.en_reposo() && esp.en_reposo() {
        // ** EL CABLE: el xHC lee del bloque de la app, como el primer dia.
        let s = core::slice::from_raw_parts(origen, muestras);
        m.mirar(s, canales);
        ventana(m);
        ESTADO.store(ESTADO_EN_MARCHA, Ordering::SeqCst);
        return desde;
    }

    let Some(destino) = ranura(n) else { return desde };
    let dst = crate::ring0::mm::phys_to_virt(destino) as *mut i16;
    core::ptr::copy_nonoverlapping(origen, dst, muestras);
    m.pasar_con(Some(esp), core::slice::from_raw_parts_mut(dst, muestras), canales);
    ventana(m);
    ESTADO.store(ESTADO_EN_MARCHA, Ordering::SeqCst);
    destino
}

/// **La siguiente ranura del rebote**, alineada a 64 (lo que el xHC pide a un
/// bufer de datos). `None` si la trama no cabe o no hubo marco, y el ESTADO lo
/// dice.
unsafe fn ranura(n: u16) -> Option<u64> {
    let paso = (n as u64 + 63) & !63;
    let ranuras = (crate::ring0::mm::PAGE / paso.max(1)) as u32;
    if ranuras < RANURAS_MINIMAS {
        ESTADO.store(ESTADO_NO_CABE, Ordering::SeqCst);
        return None;
    }
    if REBOTE == 0 {
        match crate::ring0::mm::phys::alloc_frame() {
            Some(f) => {
                crate::ring0::mm::phys::zero_frame(f);
                REBOTE = f;
                cabina::count("audio", "el MAESTRO tiene su marco: ranuras de la etapa", ranuras as u64);
            }
            None => {
                ESTADO.store(ESTADO_SIN_MARCO, Ordering::SeqCst);
                return None;
            }
        }
    }
    let destino = REBOTE + (RANURA % ranuras) as u64 * paso;
    RANURA = (RANURA + 1) % ranuras;
    Some(destino)
}

/// Muestras de una trama que caben en la suma: 512 son 5,3 ms de estereo a
/// 48 kHz, y una trama es 1 ms. Un aparato con tramas mayores sigue sonando
/// por el camino del anillo, sin voces, y el ESTADO lo dice.
const SUMA_MAX: usize = 512;
/// La suma de la trama, en 32 bits: el anillo mas las voces.
static mut SUMA: [i32; SUMA_MAX] = [0; SUMA_MAX]; // [escribe] bombeo

/// **COMPONER LA TRAMA**: lo que el anillo trae (`pcm`) MAS las voces del
/// orquestador, por el maestro, al cable. Devuelve la fisica y los bytes que
/// hay que encolar, o `None` si no hay nada que mandar (y entonces van los
/// ceros de siempre).
///
/// ```text
///    sin voces   el camino de siempre: la trama del anillo por `pasar`
///                (y en reposo, sin copia), o silencio
///    con voces   anillo + voces de la app + el FONDO agachado + sus
///                avisos, sumados en 32 bits -> maestro -> rebote
/// ```
///
/// # Safety
/// Desde el hilo del bus, con `pcm` ya juzgada (`siguiente_trama`).
pub unsafe fn componer(pcm: Option<(u64, u16)>, largo: u16, t: &super::audio::Tubo) -> Option<(u64, u16)> {
    super::voces::atender(t.frecuencia);
    if !super::voces::hay() {
        return match pcm {
            Some((desde, n)) => Some((pasar(desde, n, t), n)),
            None => {
                silencio(largo, t);
                None
            }
        };
    }
    if t.bits != 16 {
        ESTADO.store(ESTADO_NO_ES_16, Ordering::SeqCst);
        return pcm;
    }
    let n = pcm.map(|p| p.1).unwrap_or(largo);
    let muestras = n as usize / 2;
    if muestras > SUMA_MAX {
        ESTADO.store(ESTADO_NO_CABE, Ordering::SeqCst);
        return pcm;
    }
    let Some(destino) = ranura(n) else { return pcm };
    let canales = t.canales.max(1) as usize;
    let suma = &mut (&mut *core::ptr::addr_of_mut!(SUMA))[..muestras];
    match pcm {
        Some((desde, _)) => {
            let origen = core::slice::from_raw_parts(
                crate::ring0::mm::phys_to_virt(desde) as *const i16,
                muestras,
            );
            for (s, &o) in suma.iter_mut().zip(origen.iter()) {
                *s = o as i32;
            }
        }
        None => suma.fill(0),
    }
    super::voces::APP.mezclar(suma, canales, u32::MAX);
    // ** EL FONDO (2026-10-03): la musica del escritorio, agachada si suena
    // un aviso o la app, y los avisos encima. "Suena la app" es que lo que
    // trae ESTA trama pasa de -40 dBFS: el sosten del agache (400 ms) tapa
    // las tramas flojas entre dos golpes.
    if super::voces::FONDO.hay() {
        let app_suena = suma.iter().any(|&x| !(-328..=328).contains(&x));
        super::voces::mezclar_fondo(suma, canales, t.frecuencia, app_suena);
    }
    let m = etapa(t.frecuencia, t.canales);
    let dst = core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(destino) as *mut i16, muestras);
    m.pasar_acumulador_con(Some(espacio_al_dia(t.frecuencia)), suma, dst, canales);
    ventana(m);
    ESTADO.store(ESTADO_EN_MARCHA, Ordering::SeqCst);
    Some((destino, n))
}

/// **Una trama de silencio** (no habia muestras): el medidor cae y la rampa
/// anda, aunque por el cable vayan los ceros de siempre.
///
/// # Safety
/// Desde el hilo del bus.
pub unsafe fn silencio(n: u16, t: &super::audio::Tubo) {
    if t.bits != 16 {
        return;
    }
    let m = etapa(t.frecuencia, t.canales);
    m.silencio(n as usize / 2, t.canales.max(1) as usize);
    ventana(m);
}

unsafe fn ventana(m: &mut Maestro) {
    EN_VENTANA += 1;
    ACTUAL.store(m.actual(), Ordering::SeqCst);
    if EN_VENTANA < TRAMAS_POR_VENTANA {
        return;
    }
    EN_VENTANA = 0;
    let l = m.lectura();
    let d = |v: i32| (v.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16) as u64;
    MEDIDA.store(
        d(l.pico[0]) | (d(l.pico[1]) << 16) | (d(l.rms[0]) << 32) | (d(l.rms[1]) << 48),
        Ordering::SeqCst,
    );
    let v = VENTANAS.fetch_add(1, Ordering::SeqCst) + 1;
    LIMITE.store(
        l.dobladas.min(0xFFFF_FFFF) | (d(l.reduccion) << 32) | ((v & 0xFFFF) << 48),
        Ordering::SeqCst,
    );
}

// ===================================================================
//  LO QUE SE LEE: `OP_INFO`, sin handle
// ===================================================================

/// `INFO_AUDIO_MAESTRO`: `[0..16)` fader | `[16..32)` la parte del aparato |
/// `[32..48)` la ganancia digital que hay puesta ahora (tras la rampa), los
/// tres `i16` en 1/256 dB | bit 48 mudo | bit 49 el escritorio lo toco |
/// `[56..64)` el estado ([`ESTADO_EN_MARCHA`] y los demas).
pub fn info() -> u64 {
    let d = |v: i32| (v.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16) as u64;
    d(MANDO_DB.load(Ordering::SeqCst))
        | (d(APARATO_DB.load(Ordering::SeqCst)) << 16)
        | (d(ACTUAL.load(Ordering::SeqCst)) << 32)
        | ((MUDO.load(Ordering::SeqCst) as u64) << 48)
        | ((TOCADO.load(Ordering::SeqCst) as u64) << 49)
        | ((ESTADO.load(Ordering::SeqCst) as u64) << 56)
}

/// `INFO_AUDIO_MEDIDOR`: pico izq, pico der, rms izq, rms der (`i16`, 1/256 dBFS).
pub fn info_medidor() -> u64 {
    MEDIDA.load(Ordering::SeqCst)
}

/// `INFO_AUDIO_LIMITE`: `[0..32)` muestras doblegadas desde el principio |
/// `[32..48)` **lo MAS que el limite bajo en la ultima ventana** (`i16`; por
/// encima de 6 dB el fader ya no sube el volumen, lo aplasta) | `[48..64)`
/// ventanas cerradas (da la vuelta): si no sube, el medidor no se mueve.
pub fn info_limite() -> u64 {
    LIMITE.load(Ordering::SeqCst)
}
