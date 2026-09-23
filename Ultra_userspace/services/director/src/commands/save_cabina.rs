//! **Lo que `save` no decia y CABINA si** (2026-09-17).
//!
//! Eddi: *"Save tiene que decir todo en CABINA... mas organizado por completo"*.
//! Tres cosas que existian y solo se leian en F11 o en `cabina fallos`:
//!
//!   - **usb**: que controlador maneja este kernel, cuantos aparatos quedaron
//!     en el OTRO xHC sin que nadie los mire, y el libro del portero: que llego
//!     por cada puerto y que se hizo con ello. El movil del propietario enchufado al
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
//!                     leida burla al que mira, no a la maquina
//! [riesgo]  ESPEJO    desempaqueta bits que empaqueta el kernel; la forma esta
//!                     en `bmo-abi/syscalls/surface/informe.rs` y en un solo
//!                     sitio por campo
//! [consumo] NADA      solo corre cuando alguien escribe `save`

use bmo_userland as bmo;

use super::tabla::{fila, fila_cero, subregla};
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Cuantas fichas del portero se muestran como mucho. El libro tiene 12.
const FICHAS: u64 = 12;
/// Cuantos avisos recientes se muestran como mucho.
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
    // puerto 0 sin vid empaqueta exactamente 0, y el 17-09 el Ryzen mostro
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
        if veredicto == 9 || veredicto == 10 {
            // De un aparato sin papeles el bus solo sabe decir a que
            // VELOCIDAD va el puerto (2026-09-22): es lo que separa un
            // audifono Full Speed de un hub High Speed o un aparato USB 3.
            s.text(match papeles & 0xFF {
                1 => b" a Full Speed" as &[u8],
                2 => b" a Low Speed",
                3 => b" a High Speed",
                4 | 5 => b" a Super Speed",
                _ => b"",
            });
        }
        if veredicto == 9 {
            // "Sin direccion" con el PASO de los dos tiempos en que se quedo
            // (2026-09-22) y el cc con que el xHC dijo que no. Es lo que
            // separa "no hay nada" de "el SET_ADDRESS fallo con cc=19".
            s.text(match detalle & 0xF {
                1 => b" (en el reset del puerto" as &[u8],
                2 => b" (sin ranura",
                3 => b" (en la direccion 0, BSR=1",
                4 => b" (en el evaluate context",
                5 => b" (en el segundo reset",
                6 => b" (en el SET_ADDRESS",
                _ => b" (sin decir el paso",
            });
            s.text(b", cc=");
            s.dec(detalle >> 4);
            s.text(cc_en_palabras(detalle >> 4));
            s.byte(b')');
        }
        if veredicto != 7 && veredicto != 9 && veredicto != 10 && detalle != 0 {
            // Un aparato que SI entro: COMO (2026-09-22). Su paquete de EP0,
            // si hubo que decirselo al xHC (evaluate), y lo que costaron los
            // dos tiempos. Con esto se ve, aparato por aparato, cual entro
            // a la primera y cual necesito el paso 4.
            s.with_ink(INK_PLAIN);
            s.text(b"  [paquete ");
            s.dec(detalle & 0xFF);
            if detalle & 0x100 != 0 {
                s.text(b" evaluado");
            }
            s.text(b", ");
            s.dec((detalle >> 9) * 8);
            // El campo son 7 bits de octavos de ms: 1016 es su TECHO, no
            // una medida. Decirlo, o el que lo lea creera que midio.
            s.text(if (detalle >> 9) == 127 { b" ms o MAS]" as &[u8] } else { b" ms]" });
        }
        // Y para la maquina (DATOS.TXT): una clave por ficha, con el
        // veredicto y su detalle crudos.
        anotar_ficha(i, papeles, veredicto, detalle);
        if veredicto == 10 {
            // "Sin descriptores" con el PASO en que se quedo (2026-09-21) y,
            // si llego a decirlo, cuanto declaro medir su configuracion. Es
            // lo que separa un aparato mudo de un audifono que no cabia.
            s.text(match detalle & 0xF {
                1 => b" (ni el descriptor del aparato" as &[u8],
                2 => b" (sin cabecera de configuracion",
                3 => b" (configuracion de menos de 9 B)",
                4 => b" (la configuracion NO CABE:",
                5 => b" (la configuracion vino corta:",
                _ => b"",
            });
            if detalle & 0xF >= 4 {
                s.byte(b' ');
                s.dec(detalle >> 4);
                s.text(b" B)");
            } else if detalle & 0xF == 1 || detalle & 0xF == 2 {
                // El `cc` de la ultima respuesta: 3 = Babble (el paquete
                // era mas grande de lo que el xHC creia: el caso del
                // audifono), 4 = error de transaccion, 254 = no contesto,
                // 0 = el arranque, que no lo apunta.
                s.text(b", cc=");
                s.dec(detalle >> 4);
                s.text(match detalle >> 4 {
                    3 => b" babble)" as &[u8],
                    4 => b" error)",
                    254 => b" no contesto en 500 ms: NAK tras NAK, vivo pero sin datos)",
                    _ => b")",
                });
            }
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
    report_latido(s);
}

/// El `cc` del xHC en palabras, para el que lee el save sin la tabla 6.4.2
/// delante.
fn cc_en_palabras(cc: u64) -> &'static [u8] {
    match cc {
        0 => b" sin comando",
        1 => b" bien",
        3 => b" babble",
        4 => b" error de transaccion",
        5 => b" el TRB no vale",
        6 => b" stall",
        9 => b" sin ranuras",
        11 => b" la ranura no estaba habilitada",
        17 => b" un campo del contexto no vale",
        19 => b" la ranura no estaba en ese estado",
        254 => b" no contesto",
        _ => b"",
    }
}

/// La ficha para DATOS.TXT: `usb.ficha.N = puerto:vid:pid:veredicto:detalle`
/// no cabe en un numero, asi que van dos claves por ficha: los papeles
/// (puerto en los bits 24..32, vid en 48..64, pid en 32..48, como
/// `INFO_USB_FICHA`) y el veredicto con su detalle (`veredicto | detalle << 8`).
fn anotar_ficha(i: u64, papeles: u64, veredicto: u64, detalle: u64) {
    let mut clave = [0u8; 20];
    let n = clave_ficha(&mut clave, i, b"papeles");
    super::datos::anotar(&clave[..n], papeles, b"");
    let n = clave_ficha(&mut clave, i, b"veredicto");
    super::datos::anotar(&clave[..n], veredicto | (detalle << 8), b"");
}

fn clave_ficha(buf: &mut [u8; 20], i: u64, que: &[u8]) -> usize {
    let mut n = 0;
    for &b in b"ficha" {
        buf[n] = b;
        n += 1;
    }
    // Dos cifras bastan: el libro tiene menos de cien fichas.
    buf[n] = b'0' + ((i / 10) % 10) as u8;
    buf[n + 1] = b'0' + (i % 10) as u8;
    buf[n + 2] = b' ';
    n += 3;
    for &b in que {
        if n < buf.len() {
            buf[n] = b;
            n += 1;
        }
    }
    n
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
    // ** El nombre, PREGUNTADO. Aqui ponia "(4 = escritorio)" escrito a mano,
    // y el 23-09 (06:57) el tid 4 era EL PROPIO hilo del bus: los hilos de
    // kernel nuevos (el del disco, el enterrador) corrieron los numeros. Una nota fija sobre un numero que
    // cambia es un instrumento que miente con toda la seguridad del mundo.
    let mut quien = [0u8; 64];
    let k = nombre_del_tid(tid, &mut quien);
    fila(s, b"  el CPU lo tuvo", tid, b"tid", &quien[..k]);
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
    serie(s);
}

/// **El puerto serie: si escribir en el le cuesta algo a quien habla.** Va
/// debajo del latido porque fue el latido quien lo destapo (23-09, 06:57).
fn serie(s: &mut Output) {
    let v = bmo::info(bmo::INFO_SERIE);
    if v & bmo::SERIE_COLA == 0 {
        fila(s, b"serie", v & bmo::SERIE_APUNTADOS_MASK, b"B",
             b"DIRECTO: cada byte lo paga quien escribe, ~87 us a 115200 baudios");
        return;
    }
    fila(s, b"serie", v & bmo::SERIE_APUNTADOS_MASK, b"B",
         b"apuntados en la cola del puerto serie; los saca la IDLE y nadie espera");
    fila(s, b"  en cola, pico", (v >> bmo::SERIE_PICO_SHIFT) & bmo::SERIE_PICO_MASK, b"B",
         b"lo mas que llego a esperar; la cola son 16384");
    fila_cero(s, b"  perdidos", (v >> bmo::SERIE_PERDIDOS_SHIFT) & bmo::SERIE_PERDIDOS_MASK,
              b"no cupieron en la cola; la caja negra en RAM los tiene igual");
}

/// **Quien es un tid**, preguntado: un hilo de kernel con compas (su nombre
/// en `INFO_TXT_COMPAS_NOMBRE`) o un programa lanzado (`INFO_PROG_QUIEN`).
fn nombre_del_tid(tid: u64, out: &mut [u8; 64]) -> usize {
    let mut n = 0usize;
    let mut pon = |b: &[u8], n: &mut usize| {
        for &c in b {
            if *n < out.len() {
                out[*n] = c;
                *n += 1;
            }
        }
    };
    pon(b"la tarea que corrio mientras el bus esperaba: ", &mut n);
    let mut txt = [0u8; 32];
    let mut i = 0u64;
    loop {
        let v = bmo::info(bmo::INFO_COMPAS | (i << 8));
        if v == 0 {
            break;
        }
        if v & 0xFF == tid {
            let k = bmo::info_texto(bmo::INFO_TXT_COMPAS_NOMBRE | (i << 8), &mut txt);
            pon(b"el hilo ", &mut n);
            pon(&txt[..k], &mut n);
            return n;
        }
        i += 1;
    }
    let mut j = 0u64;
    while j < 64 {
        let q = bmo::info(bmo::INFO_PROG_QUIEN | (j << 8));
        if q == 0 {
            break;
        }
        if (q >> 16) & 0xFFFF == tid {
            let k = bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (j << 8), &mut txt);
            pon(&txt[..k], &mut n);
            return n;
        }
        j += 1;
    }
    pon(b"un hilo de kernel sin compas", &mut n);
    n
}

/// **Los prestamos: las ventanas.**
pub(crate) fn report_prestamos(s: &mut Output) {
    subregla(s, b"prestamos -- la memoria que una app OFRECE y el escritorio TOMA (las ventanas)");
    let p = bmo::info(bmo::INFO_PRESTAMOS);
    fila(s, b"vivas", p & 0xFF, b"ofertas", b"bloques ofrecidos ahora mismo");
    fila(s, b"tomadas", (p >> 8) & 0xFF, b"", b"de esas, las que el escritorio ya compone");
    fila_cero(s, b"huerfanas", (p >> 16) & 0xFF, b"el propietario murio con la oferta viva");
    fila_cero(s, b"negadas", p >> 32,
              b"ofertas rechazadas desde el arranque: el motivo, en `cabina fallos`");
}

/// **El audio: el audifono reclamado, el volumen y el tubo** (2026-09-21).
///
/// Hasta hoy el `save` no tenia UNA fila de audio: lo unico que se veia
/// era "el propietario del sonido MURIO" tres veces en los avisos, sin poder decir
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
        fila(s, b"canales de volumen", (a >> 8) & 0xFF, b"",
             b"los del Feature Unit (el mando), NO los que se reproducen: esos van abajo");
        fila(s, b"mute", (a >> 16) & 1, b"", b"1 = el aparato tiene interruptor de mute");
        fila(s, b"reproduce", (a >> 17) & 1, b"", b"1 = declara una interfaz AudioStreaming de salida");
        let r = bmo::info(bmo::INFO_AUDIO_RANGO);
        fila_db(s, b"rango min", r & 0xFFFF, b"lo mas bajo que acepta, en dB (1/256 dB en DATOS.TXT)");
        fila_db(s, b"rango max", (r >> 16) & 0xFFFF, b"lo mas alto");
        let pct = (a >> 24) & 0xFF;
        if pct == 0xFF {
            fila_cero(s, b"volumen", 0, b"nadie ha puesto un volumen todavia");
        } else if pct == 0xFE {
            // 0xFE = lo ultimo llego en dB, del MAESTRO: el porcentaje no dice
            // nada, y escribir "254 %" seria un numero sin procedencia.
            fila_db(s, b"mandado", (r >> 32) & 0xFFFF, b"lo puso el MAESTRO, en dB: ver su tabla abajo");
            fila_db(s, b"tiene", (r >> 48) & 0xFFFF, b"lo que el aparato dijo tener al confirmar");
            fila(s, b"confirmado", (a >> 18) & 1, b"", b"1 = tiene lo mandado; 0 = guardo OTRO, o no contesto");
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
    let d = bmo::info(bmo::INFO_AUDIO_PROPIETARIO);
    fila(s, b"propietario", d & 0xFFFF_FFFF, b"pid", b"el proceso que tiene el audio; 0 = nadie");
    let t = bmo::info(bmo::INFO_AUDIO_TUBO);
    fila(s, b"tubo", (t >> 56) & 1, b"", b"1 = el endpoint isocrono esta configurado y con su alt puesto");
    if (t >> 56) & 1 == 1 {
        fila(s, b"armado", (t >> 57) & 1, b"", b"1 = mandando silencio (tramas de ceros)");
        fila(s, b"frecuencia", t & 0xFF_FFFF, b"Hz", b"la elegida de las que el aparato acepta");
        let bytes_ms = (t >> 24) & 0xFFFF;
        fila(s, b"bytes por ms", bytes_ms, b"B", b"lo que el aparato come de una vez (el latido del bus es 1 ms)");
        fila(s, b"max packet", (t >> 40) & 0xFFFF, b"B", b"lo mas que el aparato acepta por intervalo");
        let c = bmo::info(bmo::INFO_AUDIO_TRAMAS);
        fila(s, b"encoladas", c & 0xFFFF_FFFF, b"", b"tramas isocronas desde el arranque");
        fila_cero(s, b"tarde", c >> 32, b"tramas que el xHC no llego a servir en su microtrama: se OYE");
        let h = bmo::info(bmo::INFO_AUDIO_HUECOS);
        fila(s, b"huecos", h & 0xFFFF_FFFF, b"",
             b"tramas en silencio con bufer prestado, CONTANDO el arranque de la app");
        // ** Los que se OYEN: con el productor ya en marcha (22-09).
        let tr = bmo::info(bmo::INFO_AUDIO_TIRONES);
        fila_cero(s, b"en marcha", tr & 0xFFFF_FFFF, b"de esos, los que faltaron YA SONANDO: se oyen");
        fila_cero(s, b"tirones", (tr >> 32) & 0xFFFF, b"cortes: rachas de silencio seguidas mientras sonaba");
        fila_cero(s, b"el mas largo", tr >> 48, b"ms que duro el peor corte");
        fila_cero(s, b"vetos DMA", h >> 32, b"tramos del bufer prestado que el juez nego (R-DMA)");
        fila(s, b"pendientes", d >> 32, b"B", b"lo escrito en el bufer prestado y aun no leido");
    }
    // ** LAS VOCES DEL ORQUESTADOR (22-09): la app declara, el kernel toca.
    // Van FUERA del `if tubo`: un banco prestado sin tubo tambien se cuenta.
    {
        let v = bmo::info(bmo::INFO_AUDIO_VOCES);
        let c = bmo::info(bmo::INFO_AUDIO_VOCES_CUENTA);
        fila(s, b"voces sonando", (v & 0xFFFF).count_ones() as u64, b"",
             b"cuantas toca AHORA el orquestador (la app declara, el kernel mezcla)");
        fila(s, b"banco", (v >> 16) & 0xFFFF_FFFF, b"B", b"lo que la app presto con sus muestras; 0 = nadie");
        fila(s, b"banco de", v >> 48, b"pid", b"de quien es el banco");
        fila(s, b"tocadas", c & 0xFFFF_FFFF, b"", b"sonidos que se pidieron desde el arranque");
        fila_cero(s, b"rechazadas", (c >> 32) & 0xFFFF, b"las que el juez nego (el motivo, en `cabina fallos`)");
        fila_cero(s, b"perdidas", c >> 48, b"ordenes que no cupieron en la cola");
    }
    if ranura != 0 {
        tabla_de_formatos(s);
    }
    report_maestro(s);
}

/// **EL MAESTRO: el fader del escritorio y la etapa del kernel** (2026-09-22).
///
/// Cada numero que el panel muestra --fader, las dos partes, el medidor, el
/// limite-- sale aqui tambien, para que un `save` diga lo mismo que la pantalla
/// y DATOS.TXT lo lleve a quien lee con una maquina. Un panel cuyos numeros no
/// estan en el informe es un panel que no se puede comprobar despues.
fn report_maestro(s: &mut Output) {
    subregla(s, b"el MAESTRO -- el fader del escritorio, la etapa del kernel y su medidor");
    let m = bmo::info(bmo::INFO_AUDIO_MAESTRO);
    let estado = m >> 56;
    fila(s, b"estado", estado, b"",
         b"1 en marcha; 0 sin tubo; 2 no es 16 bits, 3 no cabe, 4 sin marco: en esos NO actua");
    let tocado = (m >> 49) & 1;
    fila(s, b"tocado", tocado, b"", b"1 = el escritorio lo movio; 0 = el aparato sigue con su volumen de fabrica");
    let f = bmo::info(bmo::INFO_AUDIO_FABRICA);
    if (f >> 16) & 1 == 1 {
        fila_db(s, b"de fabrica", f & 0xFFFF, b"el que TRAIA el aparato al reclamarlo (GET_CUR)");
    } else {
        fila_cero(s, b"de fabrica", 0, b"no se leyo: no hay aparato o no contesto su GET_CUR");
    }
    fila_db(s, b"fader", m & 0xFFFF, b"lo que pidio el escritorio");
    fila_db(s, b"parte aparato", (m >> 16) & 0xFFFF, b"la que se le pidio AL APARATO: sale limpia");
    fila_db(s, b"digital", (m >> 32) & 0xFFFF, b"la de la etapa del kernel, por donde va su rampa");
    fila(s, b"mudo", (m >> 48) & 1, b"", b"1 = callado por el maestro (con rampa, no en seco)");
    let med = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    fila_db(s, b"pico izq", med & 0xFFFF, b"la ultima ventana de 50 ms de lo que SALE al cable; -96 = nada");
    fila_db(s, b"pico der", (med >> 16) & 0xFFFF, b"");
    fila_db(s, b"rms izq", (med >> 32) & 0xFFFF, b"la fuerza que se OYE, no la punta");
    fila_db(s, b"rms der", (med >> 48) & 0xFFFF, b"");
    let lim = bmo::info(bmo::INFO_AUDIO_LIMITE);
    fila_cero(s, b"dobladas", lim & 0xFFFF_FFFF, b"muestras que el limite corto a pelo: es la luz de RECORTE");
    fila_db(s, b"limite", (lim >> 32) & 0xFFFF,
            b"lo MAS que bajo en la ultima ventana; pasado de -6 dB, subir el fader APLASTA");
    fila(s, b"ventanas", lim >> 48, b"", b"del medidor, cerradas (da la vuelta en 65536): si no sube, no mide");
}

/// **QUE FORMATOS DECLARA EL APARATO, Y CUAL SE COGIO** (2026-09-22).
///
/// El propietario: *"el save en audio total con tablas por completo, y que
/// versiones agarran para eso, que EXPONGA que es"*. Hasta hoy el informe
/// decia `frecuencia 48000` y `192 B` y ahi se acababa: **no habia forma de
/// saber si el aparato ofrecia otra cosa**, porque el driver se quedaba con el
/// primer formato y no miraba mas. Ahora se guardan todos y se muestran con la
/// cuenta hecha, que es lo que contesta la pregunta de verdad: *este audifono
/// "7.1", puede llevar 5.1 por el cable, o su 7.1 es de mentira?*
fn tabla_de_formatos(s: &mut Output) {
    let f = bmo::info(bmo::INFO_AUDIO_FORMATOS);
    let cuantos = f & 0xFF;
    subregla(s, b"formatos que el aparato DECLARA, y la cuenta de cada uno");
    if cuantos == 0 {
        fila_cero(s, b"formatos", 0, b"no declara ninguna interfaz de reproduccion");
        return;
    }
    fila(s, b"formatos", cuantos, b"", b"alternate settings con endpoint isocrono de salida");
    s.with_ink(INK_ECHO);
    s.text(b"      alt  canales   bits  B/ms  max pkt  sinc    frecuencias
");
    s.with_ink(INK_PLAIN);
    for i in 0..cuantos.min(8) {
        let p = bmo::info(bmo::INFO_AUDIO_FORMATO | (i << 8));
        if p == 0 {
            continue;
        }
        let canales = (p >> 8) & 0xFF;
        let bits = (p >> 16) & 0xFF;
        let sub = (p >> 24) & 0xFF;
        let maxpkt = (p >> 32) & 0xFFFF;
        let n_hz = (p >> 48) & 0xFF;
        let cabe = (p >> 56) & 1 == 1;
        let elegido = (p >> 57) & 1 == 1;
        // *** LA FRECUENCIA CON LA QUE SE HACE LA CUENTA, Y EL METAL LA
        // CAZO. La primera version usaba la PRIMERA que el aparato declara, y
        // el save del 22-09 a las 08:23 salio con `B/ms 176` para un formato
        // que estaba sonando a 48.000 (192): el audifono declara `44100/48000`
        // en ese orden. Un numero con la unidad correcta y la entrada
        // equivocada es peor que no ponerlo. Ahora: del elegido se toma la que
        // el tubo tiene PUESTA, y de los demas la mas alta que declaren.
        let mut hz = 0u64;
        for k in 0..n_hz.min(6) {
            let r = bmo::info(bmo::INFO_AUDIO_FRECUENCIA | (i << 8) | (k << 12));
            if r > hz {
                hz = r;
            }
        }
        if elegido {
            let en_uso = bmo::info(bmo::INFO_AUDIO_TUBO) & 0xFF_FFFF;
            if en_uso != 0 {
                hz = en_uso;
            }
        }
        s.with_ink(if elegido { INK_GOOD } else { INK_PLAIN });
        s.text(b"      ");
        s.dec_right(p & 0xFF, 3);
        s.dec_right(canales, 9);
        s.dec_right(bits, 7);
        // Los bytes por milisegundo A ESA frecuencia: la cuenta que decide si
        // cabe, hecha aqui para que nadie tenga que hacerla.
        s.dec_right((hz / 1000) * canales * sub, 6);
        s.dec_right(maxpkt, 9);
        s.text(match (p >> 58) & 3 {
            1 => b"  async" as &[u8],
            2 => b"  adapt",
            3 => b"  sincr",
            _ => b"  --   ",
        });
        s.text(b"  ");
        for k in 0..n_hz.min(6) {
            if k > 0 {
                s.byte(b'/');
            }
            s.dec(bmo::info(bmo::INFO_AUDIO_FRECUENCIA | (i << 8) | (k << 12)));
        }
        if elegido {
            s.text(b"  <- ELEGIDO");
        } else if !cabe {
            s.text(b"  (no cabe en 1 ms)");
        }
        s.with_ink(INK_PLAIN);
        s.byte(10);
        // Y a DATOS.TXT, crudo, para el que lee con una maquina.
        anotar_formato(i, p, hz);
    }
    s.with_ink(INK_ECHO);
    s.text(b"      la cuenta: B/ms = (Hz / 1000) x canales x bytes por muestra, a la que el
");
    s.text(b"      tubo tiene PUESTA (el elegido) o a la mas alta que declare (los demas).
");
    s.text(b"      Si pasa de `max pkt`, ese formato NO cabe en el milisegundo del bus.
");
    s.text(b"      192 B/ms a 48 kHz son 48 x 2 x 2: ESTEREO. Un 5.1 pediria 576, un 7.1 768.
");
    s.text(b"      Una sola fila de 2 canales = este aparato NO lleva multicanal por el cable.
");
    s.with_ink(INK_PLAIN);
}

fn anotar_formato(i: u64, p: u64, hz: u64) {
    let mut clave = [0u8; 20];
    let n = clave_ficha(&mut clave, i, b"formato");
    super::datos::anotar(&clave[..n], p, b"");
    let n = clave_ficha(&mut clave, i, b"hz");
    super::datos::anotar(&clave[..n], hz, b"Hz");
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
