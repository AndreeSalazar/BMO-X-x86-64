//! **El VOLUMEN del audifono USB**, por control transfer.
//!
//! [carril]  AMARILLO  el volumen del audifono por control transfer
//! [consumo] NADA      el volumen: una transferencia cuando alguien la pide
//!
//! ## Por que esto llega antes que oir nada
//!
//! Reproducir muestras por USB pide transferencias **isocronas**, que
//! `bmo-xhci` no tiene: hoy sabe control e interrupt. Eso es la casilla 2.4 de
//! `docs/identidad/LIDERES.md` y es una pieza XL.
//!
//! El volumen no. Es un `SET_CUR` sobre el *Feature Unit* de la interfaz de
//! control del aparato, o sea **un control transfer** -- los mismos que enumeran
//! el teclado y el raton en cada arranque. Asi que BMO-X puede mandar sobre el
//! audifono **antes de reproducir una sola muestra**.
//!
//! Y en esta maquina eso importa mas de lo que parece: el altavoz del PC no
//! suena porque la placa no trae zumbador (`aparatos = 1` y silencio, visto en
//! el Ryzen el 2026-08-09). El aparato que el dueno usa de verdad es
//! `VID_1B3F&PID_2008`, un USB Audio Class 1.0. Este es el primer camino de
//! sonido que puede tener efecto audible en esta maquina.
//!
//! ## El reparto con `bmo-uaudio`
//!
//! Aqui se toca el hardware y **nada mas**: pedir el descriptor, mandar las
//! peticiones. Quien decide QUE mandar --como se lee un descriptor, como se
//! monta un `wIndex`, como se convierte un porcentaje en decibelios-- vive en
//! `platform/drivers/usb/uaudio`, que no depende de nada y **se prueba entero en
//! el anfitrion**: diez filas sin encender la maquina.
//!
//! Es el mismo reparto que hizo util al driver del raton: la decision separada
//! del registro.

use core::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, Ordering};

/// El slot xHCI del aparato de audio, o 0 si no se ha encontrado.
static SLOT: AtomicU8 = AtomicU8::new(0);
/// Datos del Feature Unit, ya localizados. Se guardan sueltos porque
/// `AudioControl` no es atomico y aqui no hay cerrojo que valga la pena.
static IFACE: AtomicU8 = AtomicU8::new(0);
static UNIT: AtomicU8 = AtomicU8::new(0);
/// Rango que declaro el aparato, en 1/256 dB. **No hay uno estandar**: se le
/// pregunta con `GET_MIN`/`GET_MAX` y suponerlo es el mismo error que suponer
/// el formato del informe HID.
static VOL_MIN: AtomicI16 = AtomicI16::new(0);
static VOL_MAX: AtomicI16 = AtomicI16::new(0);
/// Lo que el Feature Unit declaro. Se GUARDA en vez de suponerse en cada
/// llamada: mandar un mute a un aparato que no lo tiene es un STALL, y un
/// STALL contado como fallo esconde el volumen que si llego.
static TIENE_MUTE: AtomicBool = AtomicBool::new(false);
static CANALES: AtomicU8 = AtomicU8::new(0);

// -- EL VOLUMEN VA POR EL HILO DEL BUS (2026-09-21) -------------------------
//
// `AUDIO_OP_VOLUME` llegaba aqui desde un syscall y mandaba sus control
// transfers ahi mismo: pocos milisegundos, pero un segundo conductor del xHC
// fuera del hilo del bus, la misma clase que `buscar()` (244 ms) y que A0
// cerro para el escritorio. Ahora el syscall DEJA DICHO el porcentaje y el
// bombeo lo manda en su vuelta (`atender`, desde `pump_bus`). De paso, un
// volumen pedido ANTES de enchufar el audifono se aplica al reclamarlo, y
// al volver a enchufarlo se restaura el ultimo: es lo que hace cualquier
// sistema con un audifono, y aqui sale gratis.
//
/// Lo pedido desde Ring 3 y aun no mandado. `0xFF` = nada pendiente.
static VOL_PEDIDO: AtomicU8 = AtomicU8::new(0xFF);
/// El ultimo porcentaje MANDADO al aparato (`0xFF` = ninguno) y su valor en
/// 1/256 dB; lo que el aparato dijo tener al confirmar, y si coincidio.
static VOL_PCT: AtomicU8 = AtomicU8::new(0xFF);
static VOL_MANDADO: AtomicI16 = AtomicI16::new(0);
static VOL_TIENE: AtomicI16 = AtomicI16::new(0);
static VOL_CONFIRMADO: AtomicBool = AtomicBool::new(false);

/// **Ring 3 pide un volumen.** No toca el bus: lo deja dicho para el bombeo.
pub fn pedir_volumen(pct: u8) {
    VOL_PEDIDO.store(pct.min(100), Ordering::SeqCst);
}

/// **El bombeo manda lo pedido**, si hay audifono. Lo llama `pump_bus` en
/// cada vuelta, con el CR3 del kernel: fuera de un pedido es una lectura de
/// un atomico. Sin audifono lo pedido se queda esperando a que lo haya.
pub fn atender() {
    let pct = VOL_PEDIDO.load(Ordering::SeqCst);
    if pct == 0xFF || SLOT.load(Ordering::SeqCst) == 0 {
        return;
    }
    VOL_PEDIDO.store(0xFF, Ordering::SeqCst);
    set_volume(pct);
}

/// **El estado del audifono, empaquetado** para `INFO_AUDIO_APARATO`:
/// `[0..8)` ranura (0 = no hay) | `[8..16)` canales | bit 16 tiene mute |
/// bit 17 declara reproduccion | bit 18 el aparato CONFIRMO el volumen |
/// `[24..32)` pct mandado (0xFF = ninguno) | `[32..40)` pct pedido pendiente.
pub fn info_aparato() -> u64 {
    (SLOT.load(Ordering::SeqCst) as u64)
        | ((CANALES.load(Ordering::SeqCst) as u64) << 8)
        | ((TIENE_MUTE.load(Ordering::SeqCst) as u64) << 16)
        | ((HAY_REPRODUCCION.load(Ordering::SeqCst) as u64) << 17)
        | ((VOL_CONFIRMADO.load(Ordering::SeqCst) as u64) << 18)
        | ((VOL_PCT.load(Ordering::SeqCst) as u64) << 24)
        | ((VOL_PEDIDO.load(Ordering::SeqCst) as u64) << 32)
}

/// El rango y lo mandado, en 1/256 dB, para `INFO_AUDIO_RANGO`:
/// `[0..16)` min | `[16..32)` max | `[32..48)` mandado | `[48..64)` lo que
/// el aparato dijo tener. Cada uno es un `i16` en dos bytes.
pub fn info_rango() -> u64 {
    (VOL_MIN.load(Ordering::SeqCst) as u16 as u64)
        | ((VOL_MAX.load(Ordering::SeqCst) as u16 as u64) << 16)
        | ((VOL_MANDADO.load(Ordering::SeqCst) as u16 as u64) << 32)
        | ((VOL_TIENE.load(Ordering::SeqCst) as u16 as u64) << 48)
}

/// Hay un aparato de audio USB localizado y con volumen?
pub fn hay() -> bool {
    SLOT.load(Ordering::SeqCst) != 0
}

/// **El que enumera pregunta: es tuyo?** (2026-09-21). Lo llama
/// `XhciHal::reclamar` desde `bmo_uhid::instalar`, con la configuracion
/// entera del aparato en la mano y `SET_CONFIGURATION` ya mandado. Si es
/// USB Audio con volumen, se apunta su ranura --que a partir de aqui sigue
/// viva-- y se le pregunta su rango. `true` = me lo quedo.
///
/// *** ANTES ESTO ERA `buscar()`: recorrer las ranuras 1..8 pidiendo el
/// descriptor de configuracion, con transferencias BLOQUEANTES, desde
/// `AUDIO_OP_DEVICES` --o sea desde un syscall, con `IF=0` por `SFMASK`.
/// El `save` de las 13:52 lo midio: `latido tarde 244 ms`, `el reloj dio 3
/// ticks`, `el CPU lo tuvo tid 6` = `musica.ibx`. Un cuarto de segundo con
/// las interrupciones cerradas, y un segundo conductor del xHC fuera del
/// hilo del bus (la regla A0 lo prohibe al escritorio; a esto se le habia
/// pasado). Y encima no podia encontrar nada: desde el 17-09 un aparato sin
/// driver se configura y su ranura SE DEVUELVE, asi que `get_config_descriptor`
/// contestaba `no ep0 ring` en todas.
///
/// El que lee los descriptores es el que enumera. Que pregunte una vez.
pub fn reclamar(slot: u8, cfg: &[u8]) -> bool {
    let Some(ac) = bmo_uaudio::find_audio_control(cfg) else {
        return false;
    };
    if !ac.has_volume {
        // Existe y NO deja cambiar el volumen. Es un caso real, y la
        // respuesta correcta es decirlo, no fingir que se puso.
        crate::ring0::cabina::warn("uaudio", "aparato de audio SIN control de volumen", slot as u64);
        return false;
    }
    if SLOT.load(Ordering::SeqCst) != 0 {
        crate::ring0::cabina::warn("uaudio", "un SEGUNDO aparato de audio: solo se maneja uno, ranura", slot as u64);
        return false;
    }
    SLOT.store(slot, Ordering::SeqCst);
    IFACE.store(ac.interface, Ordering::SeqCst);
    UNIT.store(ac.feature_unit, Ordering::SeqCst);
    TIENE_MUTE.store(ac.has_mute, Ordering::SeqCst);
    CANALES.store(ac.channels, Ordering::SeqCst);
    // El rango son dos transferencias contra un aparato que ACABA de
    // contestar sus descriptores: microsegundos, en el hilo que enumera.
    leer_rango(&ac);
    crate::ring0::cabina::info("uaudio", "audifono USB con volumen, en la ranura", slot as u64);
    // El ultimo volumen que se mando (a este o al de antes) se restaura: un
    // audifono que se desenchufa y vuelve no tiene por que volver a cero.
    let ultimo = VOL_PCT.load(Ordering::SeqCst);
    if ultimo != 0xFF && VOL_PEDIDO.load(Ordering::SeqCst) == 0xFF {
        VOL_PEDIDO.store(ultimo, Ordering::SeqCst);
    }
    // Y su tubo de reproduccion, si lo declara: se guarda para `censar`, que
    // asi deja de leer descriptores desde un syscall.
    match bmo_uaudio::stream::find_playback(cfg) {
        Some(p) => {
            unsafe {
                REPRODUCCION = p;
            }
            HAY_REPRODUCCION.store(true, Ordering::SeqCst);
            // Y el tubo se pide ya: lo abre el hilo del bus en su vuelta,
            // asi que esta abierto antes de que un programa pregunte por el.
            crate::ring0::dev::usb::audio::pedir_tubo();
        }
        None => crate::ring0::cabina::info("uaudio", "  ...y sin interfaz de reproduccion", 0),
    }
    true
}

/// El aparato reclamado se fue: se olvida todo. Lo llama `XhciHal::soltado`
/// ANTES de que la ranura se devuelva.
pub fn soltado(slot: u8) {
    if SLOT.load(Ordering::SeqCst) != slot {
        return;
    }
    // El tubo primero, con la ranura todavia viva.
    crate::ring0::dev::usb::audio::cerrar(slot);
    SLOT.store(0, Ordering::SeqCst);
    HAY_REPRODUCCION.store(false, Ordering::SeqCst);
    VOL_CONFIRMADO.store(false, Ordering::SeqCst);
    crate::ring0::cabina::warn("uaudio", "el audifono se DESENCHUFO: se olvida su ranura", slot as u64);
}

/// La interfaz de reproduccion del audifono reclamado, con su ranura, si la
/// declaro. Es lo que `censar` abria antes leyendo descriptores por su
/// cuenta.
pub fn reproduccion() -> Option<(u8, bmo_uaudio::stream::Playback)> {
    if !HAY_REPRODUCCION.load(Ordering::SeqCst) {
        return None;
    }
    let slot = SLOT.load(Ordering::SeqCst);
    if slot == 0 {
        return None;
    }
    Some((slot, unsafe { REPRODUCCION }))
}

/// Lo que `find_playback` saco del descriptor, guardado en el instante de
/// reclamar. Se escribe SOLO desde `reclamar` (el hilo que enumera) y se lee
/// con `HAY_REPRODUCCION` delante; el valor de arranque es un relleno que
/// nadie lee.
static mut REPRODUCCION: bmo_uaudio::stream::Playback = bmo_uaudio::stream::Playback::VACIA;
static HAY_REPRODUCCION: AtomicBool = AtomicBool::new(false);

fn leer_rango(ac: &bmo_uaudio::AudioControl) {
    let min = leer(ac, bmo_uaudio::GET_MIN);
    let max = leer(ac, bmo_uaudio::GET_MAX);
    if let (Some(a), Some(b)) = (min, max) {
        // La validacion vive en `bmo-uaudio` y no aqui: un rango del reves y un
        // minimo que en realidad es el marcador de silencio son decisiones, y
        // las decisiones se prueban en el anfitrion.
        if let Some((a, b)) = bmo_uaudio::rango(a, b) {
            VOL_MIN.store(a, Ordering::SeqCst);
            VOL_MAX.store(b, Ordering::SeqCst);
            return;
        }
    }
    // -60 dB a 0 dB, que es el rango tipico de un aparato de estos.
    VOL_MIN.store(-15360, Ordering::SeqCst);
    VOL_MAX.store(0, Ordering::SeqCst);
    crate::ring0::cabina::warn("uaudio", "el aparato no dijo su rango: se supone", 0);
}

fn leer(ac: &bmo_uaudio::AudioControl, cual: u8) -> Option<i16> {
    let r = bmo_uaudio::get_volume(ac, bmo_uaudio::CHANNEL_MASTER, cual);
    let mut buf = [0u8; 2];
    let n = unsafe {
        bmo_xhci::control_transfer(
            SLOT.load(Ordering::SeqCst),
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut buf,
            true,
        )
    };
    if n < 2 {
        return None;
    }
    Some(i16::from_le_bytes(buf))
}

/// Pone el volumen del audifono, de 0 a 100. Devuelve si el aparato lo acepto.
///
/// [!] El porcentaje NO se manda tal cual: se convierte a decibelios con la
/// curva de `bmo_uaudio::percent_to_volume` y se recorta al rango que declaro el
/// aparato. El campo va en 1/256 dB con signo, y **el 0% no es el valor 0** --
/// ese es 0 dB, o sea el maximo. Confundirlos pone el audifono a tope creyendo
/// que se apaga, con los cascos puestos.
/// Manda `pct` al aparato AHORA. Solo desde `atender` (el hilo del bus):
/// desde Ring 3 se pide con `pedir_volumen`.
fn set_volume(pct: u8) -> bool {
    let slot = SLOT.load(Ordering::SeqCst);
    if slot == 0 {
        return false;
    }
    let ac = actual();
    let plan = bmo_uaudio::plan(
        pct,
        VOL_MIN.load(Ordering::SeqCst),
        VOL_MAX.load(Ordering::SeqCst),
        ac.has_mute,
    );
    match plan {
        bmo_uaudio::Plan::Callar => mandar_mute(slot, &ac, true),
        bmo_uaudio::Plan::Poner { valor, quitar_mute } => {
            // El mute va DELANTE del volumen: si el aparato se quedo callado
            // de la vez anterior, mandar solo el volumen deja un aparato que
            // acepta el numero y no suena -- que parece el camino roto entero.
            if quitar_mute {
                mandar_mute(slot, &ac, false);
            }
            mandar_volumen(slot, &ac, pct, valor)
        }
    }
}

/// El Feature Unit tal como lo declaro el aparato. **Se lee de lo guardado**, y
/// no se inventa: la version anterior construia esta struct con
/// `channels: 2, has_mute: true` a pelo en cada llamada, o sea que le mandaba
/// un mute a un aparato que podia no tenerlo.
fn actual() -> bmo_uaudio::AudioControl {
    bmo_uaudio::AudioControl {
        interface: IFACE.load(Ordering::SeqCst),
        feature_unit: UNIT.load(Ordering::SeqCst),
        channels: CANALES.load(Ordering::SeqCst),
        has_volume: true,
        has_mute: TIENE_MUTE.load(Ordering::SeqCst),
    }
}

fn mandar_mute(slot: u8, ac: &bmo_uaudio::AudioControl, callar: bool) -> bool {
    let r = bmo_uaudio::set_mute(ac, bmo_uaudio::CHANNEL_MASTER, callar);
    let mut datos = [if callar { 1u8 } else { 0u8 }];
    let n = unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut datos,
            false,
        )
    };
    if n == 0 {
        crate::ring0::cabina::warn("uaudio", "el aparato rechazo el mute", callar as u64);
        return false;
    }
    true
}

/// Manda el volumen, y si el canal maestro no vale, **prueba canal por canal**.
///
/// # Por que la segunda vuelta
///
/// El canal 0 (maestro) es opcional. Hay aparatos --sobre todo los que separan
/// izquierdo y derecho-- que **solo** aceptan el volumen por canal y contestan
/// STALL al maestro. Sin esta vuelta, ese aparato queda para siempre como "el
/// aparato rechazo el volumen": un aparato con volumen perfectamente
/// controlable al que le estabamos hablando por el canal que no era.
fn mandar_volumen(slot: u8, ac: &bmo_uaudio::AudioControl, pct: u8, valor: i16) -> bool {
    VOL_PCT.store(pct, Ordering::SeqCst);
    VOL_MANDADO.store(valor, Ordering::SeqCst);
    VOL_CONFIRMADO.store(false, Ordering::SeqCst);
    if escribir_volumen(slot, ac, bmo_uaudio::CHANNEL_MASTER, valor) {
        confirmar(slot, ac, bmo_uaudio::CHANNEL_MASTER, pct, valor);
        return true;
    }
    let mut alguno = false;
    for canal in 1..=ac.channels {
        if escribir_volumen(slot, ac, canal, valor) {
            alguno = true;
        }
    }
    if alguno {
        crate::ring0::cabina::info(
            "uaudio",
            "el maestro no valia: el volumen va por canal",
            ac.channels as u64,
        );
        return true;
    }
    crate::ring0::cabina::warn("uaudio", "el aparato rechazo el volumen", pct as u64);
    false
}

fn escribir_volumen(slot: u8, ac: &bmo_uaudio::AudioControl, canal: u8, valor: i16) -> bool {
    let r = bmo_uaudio::set_volume(ac, canal, valor);
    let mut datos = valor.to_le_bytes();
    // Un STALL devuelve 0 bytes. Se dice: un volumen que no llego y se cuenta
    // como puesto es un control que miente, y esos se descubren girando la
    // rueda sin que pase nada.
    unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut datos,
            false,
        ) != 0
    }
}

/// Le vuelve a preguntar al aparato **que volumen tiene puesto**, y lo dice.
///
/// # Por que no basta con que el `SET_CUR` no diera STALL
///
/// Que la peticion se acepte prueba que llego, no que hiciera algo. Un aparato
/// puede aceptar el valor y recortarlo, redondearlo a su paso (`GET_RES`) o
/// ignorarlo. Con el `GET_CUR` de vuelta, el arranque deja escrito **el numero
/// que el aparato dice tener** al lado del que se le mando: si son distintos,
/// no hay que adivinar por que la oreja no nota el cambio.
///
/// Solo se dice cuando NO coinciden. Una linea por cada pulsacion de flecha
/// llenaria CABINA de ruido y taparia justo lo que hay que ver.
fn confirmar(slot: u8, ac: &bmo_uaudio::AudioControl, canal: u8, pct: u8, mandado: i16) {
    let r = bmo_uaudio::get_volume(ac, canal, bmo_uaudio::GET_CUR);
    let mut buf = [0u8; 2];
    let n = unsafe {
        bmo_xhci::control_transfer(
            slot,
            r.bm_request_type,
            r.b_request,
            r.w_value,
            r.w_index,
            &mut buf,
            true,
        )
    };
    if n < 2 {
        return;
    }
    let tiene = i16::from_le_bytes(buf);
    VOL_TIENE.store(tiene, Ordering::SeqCst);
    VOL_CONFIRMADO.store(tiene == mandado, Ordering::SeqCst);
    if tiene != mandado {
        crate::ring0::cabina::info("uaudio", "el aparato guardo OTRO volumen", pct as u64);
    }
}
