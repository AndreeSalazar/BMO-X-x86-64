//! **Lo que `save` no decia y CABINA si** (2026-09-17).
//!
//! Eddi: *"Save tiene que decir todo en CABINA... mas organizado por completo"*.
//! Tres cosas que existian y solo se leian en F11 o en `cabina fallos`:
//!
//!   - **usb**: que controlador maneja este kernel, cuantos aparatos quedaron
//!     en el OTRO xHC sin que nadie los mire, y el libro del portero: que llego
//!     por cada puerto y que se hizo con ello. El movil del dueno enchufado al
//!     Ryzen fue lo que lo hizo visible: F11 callaba y `save` no sabia nada.
//!   - **prestamos**: las ventanas. Ofertas vivas, tomadas, huerfanas y las
//!     NEGADAS desde el arranque -- la negativa mas cara de esta casa.
//!   - **avisos**: los ultimos WARNING o peores del anillo, para no tener que
//!     acordarse de `cabina fallos` cuando algo no salio.
//!
//! Vive en su fichero porque `reports.rs` esta en el filo de L6a, y porque son
//! tres tablas que se leen juntas: "que hay enchufado, que se presto, que se
//! quejo".
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! [carril]  VERDE     pinta lo que el kernel contesta; no decide nada
//! [cuesta]  DATO      pregunta a la maquina (INFO y el anillo); una fila mal
//!                     leida engana al que mira, no a la maquina
//! [riesgo]  ESPEJO    desempaqueta bits que empaqueta el kernel; la forma esta
//!                     en `bmo-abi/syscalls/surface/informe.rs` y en un solo
//!                     sitio por campo
//! [consumo] NADA      solo corre cuando alguien escribe `save`

use bmo_userland as bmo;

use super::tabla::{fila, fila_cero, subregla};
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Cuantas fichas del portero se ensenan como mucho. El libro tiene 12.
const FICHAS: u64 = 12;
/// Cuantos avisos recientes se ensenan como mucho.
const AVISOS: u64 = 8;

/// **USB: el controlador y el libro del portero.**
pub(crate) fn report_usb(s: &mut Output) {
    subregla(s, b"usb -- que controlador se maneja, y que llego por cada puerto");
    let censo = bmo::info(bmo::INFO_USB_CENSO);
    let censados = censo & 0xFF;
    let vistos = (censo >> 8) & 0xFF;
    let huerfanos = (censo >> 16) & 0xFF;
    fila(s, b"xHC en la placa", censados, b"", b"controladores censados al arrancar");
    if censo >> 63 == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    ninguno elegido: este kernel no maneja USB en esta sesion\n");
        s.with_ink(INK_PLAIN);
    } else {
        let bdf = (censo >> 24) & 0xFF_FFFF;
        s.text(b"    elegido        ");
        s.hex((bdf >> 16) & 0xFF, 2);
        s.byte(b':');
        s.hex((bdf >> 8) & 0xFF, 2);
        s.byte(b'.');
        s.hex(bdf & 0xFF, 1);
        s.text(b"    bus:dev.func -- el que MAS aparatos veia; es el UNICO que se maneja\n");
        fila(s, b"ve", vistos, b"aparatos", b"en los puertos raiz del elegido");
    }
    // ** La fila que explica un movil mudo. Si no es cero, hay algo enchufado
    // que este kernel no va a mirar jamas, y la solucion esta en la nota.
    fila_cero(s, b"en el OTRO xHC", huerfanos,
              b"aparatos que NO se miran: cambiarlos a un puerto del elegido (S3b.2d)");

    let fichas = bmo::info(bmo::INFO_USB_FICHAS);
    let escritas = fichas & 0xFFFF;
    fila(s, b"fichas", escritas, b"", b"lo que el portero apunto: una por interfaz que llego");
    fila_cero(s, b"sin sitio", (fichas >> 16) & 0xFFFF, b"llegaron mas de las que caben en el libro");
    if escritas == 0 {
        return;
    }
    s.with_ink(INK_ECHO);
    s.text(b"      puerto  vid:pid    que es       que se hizo   (puerto como en F11: el 1 es el primero)\n");
    s.with_ink(INK_PLAIN);
    let mut txt = [0u8; 96];
    // ** Se recorren las `escritas`, sin cortar en un cero: una ficha de un
    // puerto 0 sin vid empaqueta exactamente 0, y el 17-09 el Ryzen enseno
    // `fichas 10` con la tabla VACIA por cortar en la primera.
    for i in 0..escritas.min(FICHAS) {
        let papeles = bmo::info(bmo::INFO_USB_FICHA | (i << 8));
        let empaquetado = bmo::info(bmo::INFO_USB_FICHA_VEREDICTO | (i << 8));
        let veredicto = empaquetado & 0xFF;
        let detalle = (empaquetado >> 8) & 0xFFFF;
        s.text(b"      ");
        s.dec_right(((papeles >> 24) & 0xFF) + 1, 4);
        s.text(b"    ");
        s.hex(papeles >> 48, 4);
        s.byte(b':');
        s.hex((papeles >> 32) & 0xFFFF, 4);
        s.text(b"  ");
        let n = bmo::info_texto(bmo::INFO_TXT_USB_QUE_ES | (i << 8), &mut txt);
        s.text(&txt[..n]);
        for _ in n..13 {
            s.byte(b' ');
        }
        // Teclado, raton, "configurado" y "reclamado" son lo bueno; el resto,
        // lo que falta.
        s.with_ink(if veredicto == 1 || veredicto == 2 || veredicto == 11 || veredicto == 12 { INK_GOOD } else { INK_ECHO });
        let n = bmo::info_texto(bmo::INFO_TXT_USB_MOTIVO | (i << 8), &mut txt);
        s.text(&txt[..n]);
        if veredicto == 7 {
            // "No se pudo preparar" con el numero que lo explica: el cc del
            // xHC (8 = no cabe en la agenda, 17 = un campo no vale, 254 = no
            // contesto). Ver `portero.rs`.
            s.text(b" (cc=");
            s.dec(detalle);
            s.byte(b')');
        }
        if veredicto == 10 {
            // "Sin descriptores" con el PASO en que se quedo (2026-09-21) y,
            // si llego a decirlo, cuanto declaro medir su configuracion. Es
            // lo que separa un aparato mudo de un audifono que no cabia.
            s.text(match detalle & 0xF {
                1 => b" (ni el descriptor del aparato)" as &[u8],
                2 => b" (sin cabecera de configuracion)",
                3 => b" (configuracion de menos de 9 B)",
                4 => b" (la configuracion NO CABE:",
                5 => b" (la configuracion vino corta:",
                _ => b"",
            });
            if detalle & 0xF >= 4 {
                s.byte(b' ');
                s.dec(detalle >> 4);
                s.text(b" B)");
            }
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
    report_latido(s);
}

/// **El peor retraso del latido del bus, y QUIEN** (2026-09-21).
///
/// `[!] usb el latido del bus llego TARDE 1266 ms` salio en dos saves seguidos
/// y era solo un numero. Esto es lo que lo explica, en el mismo informe.
fn report_latido(s: &mut Output) {
    let v = bmo::info(bmo::INFO_USB_LATIDO);
    let cuando = bmo::info(bmo::INFO_USB_LATIDO_CUANDO);
    let ms = v & 0xFFFF;
    if ms == 0 && cuando == 0 {
        fila_cero(s, b"latido tarde", 0, b"el bus nunca llego tarde por encima de 20 ms");
        return;
    }
    let tid = (v >> 16) & 0xFF;
    let suyo = (v >> 24) & 0xFFFF;
    let ticks = (v >> 40) & 0xFFFF;
    let vuelta = (v >> 56) & 0xFF;
    fila(s, b"latido tarde", ms, b"ms", b"el PEOR retraso del hilo del bus USB (peor caso)");
    fila(s, b"  en el tick", cuando, b"", b"cuando: ~1000 por segundo desde el arranque");
    fila(s, b"  el reloj dio", ticks, b"ticks", b"mientras tanto; 0 con retraso grande = interrupciones CERRADAS");
    fila(s, b"  el CPU lo tuvo", tid, b"tid", b"la tarea que corrio mientras el bus esperaba (4 = escritorio)");
    fila(s, b"  durante", suyo, b"ms", b"de esos ms, los que fueron de ese tid");
    fila(s, b"  la vuelta del bus", vuelta, b"ms", b"lo que costo la vuelta anterior; si es ~ el retraso, fue el BUS");
    // Y de los trabajos de la vuelta, el que MAS tardo desde el arranque: si
    // la vuelta fue el culpable, esto dice que parte de la vuelta.
    let ritmo = bmo::info(bmo::INFO_USB_RITMO);
    let mut txt = [0u8; 16];
    let k = bmo::info_texto(bmo::INFO_TXT_USB_TRABAJO, &mut txt);
    super::datos::anotar(b"peor trabajo", (ritmo >> 16) & 0xFFFF_FFFF, b"us");
    s.text(b"    peor trabajo   ");
    s.text(&txt[..k]);
    for _ in k..11 {
        s.byte(b' ');
    }
    s.dec((ritmo >> 16) & 0xFFFF_FFFF);
    s.with_ink(INK_ECHO);
    s.text(b" us   el trabajo de la vuelta del bus que mas tardo, desde el arranque\n");
    s.with_ink(INK_PLAIN);
}

/// **Los prestamos: las ventanas.**
pub(crate) fn report_prestamos(s: &mut Output) {
    subregla(s, b"prestamos -- la memoria que una app OFRECE y el escritorio TOMA (las ventanas)");
    let p = bmo::info(bmo::INFO_PRESTAMOS);
    fila(s, b"vivas", p & 0xFF, b"ofertas", b"bloques ofrecidos ahora mismo");
    fila(s, b"tomadas", (p >> 8) & 0xFF, b"", b"de esas, las que el escritorio ya compone");
    fila_cero(s, b"huerfanas", (p >> 16) & 0xFF, b"el dueno murio con la oferta viva");
    fila_cero(s, b"negadas", p >> 32,
              b"ofertas rechazadas desde el arranque: el motivo, en `cabina fallos`");
}

/// **El audio: el audifono reclamado, el volumen y el tubo** (2026-09-21).
///
/// Hasta hoy el `save` no tenia UNA fila de audio: lo unico que se veia
/// era "el dueno del sonido MURIO" tres veces en los avisos, sin poder decir
/// si habia audifono, si tenia tubo, o que volumen se le mando. Todo por
/// `fila`, para que DATOS.TXT lo lleve tambien.
pub(crate) fn report_audio(s: &mut Output) {
    subregla(s, b"audio -- el audifono USB reclamado, el volumen y el tubo");
    let a = bmo::info(bmo::INFO_AUDIO_APARATO);
    let ranura = a & 0xFF;
    fila(s, b"aparatos", (a >> 40) & 0xFF, b"mapa",
         b"bit 0 el altavoz del PC (hay puerto, no zumbador), bit 2 el audifono USB");
    fila(s, b"audifono", ranura, b"ranura",
         b"la ranura xHCI del reclamado; 0 = no hay ninguno (o no contesto sus papeles)");
    if ranura != 0 {
        fila(s, b"canales", (a >> 8) & 0xFF, b"", b"los que declara su Feature Unit");
        fila(s, b"mute", (a >> 16) & 1, b"", b"1 = el aparato tiene interruptor de mute");
        fila(s, b"reproduce", (a >> 17) & 1, b"", b"1 = declara una interfaz AudioStreaming de salida");
        let r = bmo::info(bmo::INFO_AUDIO_RANGO);
        fila_db(s, b"rango min", r & 0xFFFF, b"lo mas bajo que acepta, en dB (1/256 dB en DATOS.TXT)");
        fila_db(s, b"rango max", (r >> 16) & 0xFFFF, b"lo mas alto");
        let pct = (a >> 24) & 0xFF;
        if pct == 0xFF {
            fila_cero(s, b"volumen", 0, b"nadie ha puesto un volumen todavia");
        } else {
            fila(s, b"volumen", pct, b"%", b"el ultimo que se MANDO al aparato");
            fila_db(s, b"mandado", (r >> 32) & 0xFFFF, b"lo que ese % vale en su escala");
            fila_db(s, b"tiene", (r >> 48) & 0xFFFF, b"lo que el aparato dijo tener al confirmar");
            fila(s, b"confirmado", (a >> 18) & 1, b"", b"1 = tiene lo mandado; 0 = guardo OTRO, o no contesto");
        }
        let pedido = (a >> 32) & 0xFF;
        if pedido != 0xFF {
            fila(s, b"pedido", pedido, b"%", b"pedido por Ring 3 y aun no mandado: lo manda el hilo del bus");
        }
    }
    let d = bmo::info(bmo::INFO_AUDIO_DUENO);
    fila(s, b"dueno", d & 0xFFFF_FFFF, b"pid", b"el proceso que tiene el audio; 0 = nadie");
    let t = bmo::info(bmo::INFO_AUDIO_TUBO);
    fila(s, b"tubo", (t >> 56) & 1, b"", b"1 = el endpoint isocrono esta configurado y con su alt puesto");
    if (t >> 56) & 1 == 1 {
        fila(s, b"armado", (t >> 57) & 1, b"", b"1 = mandando silencio (tramas de ceros)");
        fila(s, b"frecuencia", t & 0xFF_FFFF, b"Hz", b"la elegida de las que el aparato acepta");
        fila(s, b"trama", (t >> 24) & 0xFFFF, b"B", b"bytes por milisegundo a esa frecuencia");
        fila(s, b"max packet", (t >> 40) & 0xFFFF, b"B", b"lo mas que el aparato acepta por intervalo");
        let c = bmo::info(bmo::INFO_AUDIO_TRAMAS);
        fila(s, b"encoladas", c & 0xFFFF_FFFF, b"", b"tramas isocronas desde el arranque");
        fila_cero(s, b"tarde", c >> 32, b"tramas que el xHC no llego a servir en su microtrama: se OYE");
        let h = bmo::info(bmo::INFO_AUDIO_HUECOS);
        fila_cero(s, b"huecos", h & 0xFFFF_FFFF, b"vueltas sin trama que mandar: el productor no llega");
        fila_cero(s, b"vetos DMA", h >> 32, b"tramos del bufer prestado que el juez nego (R-DMA)");
        fila(s, b"pendientes", d >> 32, b"B", b"lo escrito en el bufer prestado y aun no leido");
    }
}

/// Una fila en dB a partir de un `i16` en 1/256 dB (dos bytes de un INFO):
/// `-60.0 dB`. A DATOS.TXT va el crudo, con su unidad, porque el signo no
/// cabe en un `u64` y un decimal es tipografia.
fn fila_db(s: &mut Output, que: &[u8], crudo: u64, nota: &[u8]) {
    super::datos::anotar(que, crudo, b"1/256dB");
    let v = crudo as u16 as i16 as i32;
    let centesimas = v * 100 / 256;
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    // Ancho 9 como `fila`: signo, entero y un decimal, a la derecha.
    let ent = (centesimas / 100).unsigned_abs() as u64;
    let dec = ((centesimas % 100).unsigned_abs() / 10) as u64;
    let digitos = if ent >= 100 { 3 } else if ent >= 10 { 2 } else { 1 };
    let ancho = digitos + 2 + if centesimas < 0 { 1 } else { 0 };
    for _ in ancho..9 {
        s.byte(b' ');
    }
    if centesimas < 0 {
        s.byte(b'-');
    }
    s.dec(ent);
    s.byte(b'.');
    s.dec(dec);
    s.text(b" dB");
    if !nota.is_empty() {
        for _ in 2..8 {
            s.byte(b' ');
        }
        s.text(nota);
    }
    s.byte(b'\n');
}

/// **Los ultimos avisos del anillo**, WARNING o peor.
///
/// Es `cabina fallos` recortado a los ultimos [`AVISOS`]: lo justo para que
/// un `save` pegado en un mensaje traiga tambien lo que se quejo.
pub(crate) fn report_avisos(s: &mut Output) {
    subregla(s, b"avisos -- lo ultimo que se quejo (entero: `cabina fallos`)");
    let hay = bmo::cabina_disponibles();
    // Primera pasada: cuantos avisos hay, para quedarse con los ULTIMOS.
    let mut cuantos = 0u64;
    for n in 0..hay {
        let Some(sev) = bmo::cabina_campo(bmo::CABINA_SEVERIDAD, n) else {
            break;
        };
        if sev >= bmo::SEV_WARNING {
            cuantos += 1;
        }
    }
    if cuantos == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"    ni un aviso ni un fallo en todo el anillo\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let saltar = cuantos.saturating_sub(AVISOS);
    let mut vistos = 0u64;
    let mut modulo = [0u8; 24];
    let mut mensaje = [0u8; 96];
    for n in 0..hay {
        let Some(sev) = bmo::cabina_campo(bmo::CABINA_SEVERIDAD, n) else {
            break;
        };
        if sev < bmo::SEV_WARNING {
            continue;
        }
        vistos += 1;
        if vistos <= saltar {
            continue;
        }
        let valor = bmo::cabina_campo(bmo::CABINA_VALOR, n).unwrap_or(0);
        let nm = bmo::cabina_texto(n, bmo::CABINA_TXT_MODULO, &mut modulo);
        let nx = bmo::cabina_texto(n, bmo::CABINA_TXT_MENSAJE, &mut mensaje);
        s.with_ink(if sev >= bmo::SEV_FAULT { INK_ERR } else { INK_GOOD });
        s.text(if sev >= bmo::SEV_FAULT { b"    [X] " } else { b"    [!] " });
        s.text(&modulo[..nm]);
        for _ in nm..10 {
            s.byte(b' ');
        }
        s.text(&mensaje[..nx]);
        if valor != 0 {
            s.text(b" =");
            s.dec(valor);
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
    }
    if cuantos > AVISOS {
        s.with_ink(INK_ECHO);
        s.text(b"    ...y ");
        s.dec(cuantos - AVISOS);
        s.text(b" mas antes de estos\n");
        s.with_ink(INK_PLAIN);
    }
}
