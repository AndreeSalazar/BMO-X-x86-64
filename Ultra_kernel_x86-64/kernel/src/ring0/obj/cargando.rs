//! **EL ARCHIVO QUE SE ESTA TRAYENDO** -- la carga por trozos de una ranura.
//!
//! [carril]  AMARILLO  la carga por trozos de una ranura
//! [consumo] NADA      corre cuando una tarea usa el objeto
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! === Por que es un fichero y no un trozo de `file.rs` ===
//!
//! Por L6a: `obj/file.rs` estaba en 1.026 lineas, en la linea base del censo, y
//! **no podia crecer ni una**. Antes de anadirle la puerta de ESTRATOS habia
//! que hacerle sitio.
//!
//! Pero el corte no se eligio por medida. Se eligio porque esto es un
//! **ciclo de vida** --un archivo que todavia no ha llegado entero-- y no un
//! camino de datos: sus dos `static` no los mira nadie mas, tenia ya su propio
//! comentario de seccion, y ninguna de sus funciones se llama desde fuera del
//! modulo. Nombres libres, que es como se eligen los cortes en esta casa.
//!
//! === Que resuelve ===
//!
//! `open` lee el fichero ENTERO antes de devolver el handle. Para un `.txt` no
//! se nota; para un `.bex` de 813 KB, el que lo pidio no existe durante toda la
//! lectura -- y si el que lo pidio es el escritorio, el escritorio no pinta.
//!
//! `abrir_asinc` --que se queda en `file.rs`, porque lo suyo es repartir
//! ranuras-- devuelve el handle **en cuanto sabe que el archivo existe**, y los
//! bytes van llegando en trozos por aqui.
//!
//! === Por que el avance lo empuja el que PREGUNTA ===
//!
//! La alternativa era seguir la cadena desde el manejador de interrupcion. No:
//! seguir una cadena FAT es leer mas disco --la propia tabla-- asi que seria
//! pedir E/S desde dentro de la interrupcion de E/S. Empujando desde la
//! pregunta, el trabajo ocurre en el turno de quien lo quiere, que es de quien
//! es.
//!
//! [!] Esto es FAT32 y solo FAT32, y esta bien que lo sea: un fichero de
//! ESTRATOS no se trae por trozos de una cadena, se reconstruye por su arbol de
//! atributos. Cuando ESTRATOS entre por la puerta de `file.rs` no pasara por
//! aqui.

use super::file::{buf, LARGO, MAX_ABIERTOS};

/// Cluster por el que va la carga. `0` = no hay carga en curso.
pub(super) static mut LOAD_CLUSTER: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Lo que mide el archivo entero, mientras se trae.
///
/// * Aqui NO hay un contador de trozos. Se escribio, para que el `wait` supiera
/// si habia habido progreso desde la ultima mirada -- y como ese `wait` no
/// llego a existir (ver `syscall.rs`), el contador se quedaba subiendo para
/// nadie. Lo que hace falta saber de fuera --cuanto ha llegado-- ya lo dice
/// `LARGO`, y lo contesta `ARCH_OP_LISTO`.
pub(super) static mut LOAD_TOTAL: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];

/// Cuanto se trae de una vez.
///
/// 128 KiB: bastante para que un archivo normal llegue en una o dos vueltas, y
/// poco para que el kernel no se quede dentro mas de lo que dura un turno. El
/// numero correcto se sabra midiendo en metal; este es el que no estorba.
const TROZO: usize = 128 * 1024;

/// Se esta trayendo todavia?
pub(super) fn hay(i: usize) -> bool {
    unsafe { i < MAX_ABIERTOS && LOAD_CLUSTER[i] != 0 }
}

/// **Trae el siguiente trozo.** `true` si el archivo ya esta entero.
///
/// Lo llama cualquier operacion sobre el handle: preguntar por el archivo ES lo
/// que lo hace avanzar. Si no habia carga en curso, contesta que si -- un
/// archivo que ya estaba entero lo esta igual despues de preguntar.
pub(super) fn avanzar(i: usize) -> bool {
    // Si el hilo tiene un trozo en el aparato, se cierra antes: los dos
    // caminos no pueden contar el mismo trozo dos veces.
    cerrar_vuelo();
    if !hay(i) {
        return true;
    }
    unsafe {
        let cluster = LOAD_CLUSTER[i];
        let total = LOAD_TOTAL[i];
        let ya = LARGO[i];
        let dst = buf(i);
        let (leidos, siguiente) =
            crate::ring0::fsys::fs::leer_trozo(cluster, ya, total as u32, dst, TROZO);
        LARGO[i] = ya + leidos;
        // `siguiente == 0` es fin -- de la cadena o del archivo. Y `leidos == 0`
        // tambien corta: un tramo que no avanza dos veces seguidas seria un
        // bucle infinito en el que pregunta, y prefiero un archivo corto que se
        // nota a una maquina que no vuelve.
        if siguiente == 0 || leidos == 0 {
            LOAD_CLUSTER[i] = 0;
            crate::ring0::cabina::info("arch", "archivo completo", LARGO[i] as u64);
            return true;
        }
        LOAD_CLUSTER[i] = siguiente;
        false
    }
}


// == *** LA CARGA LA MUEVE EL HILO DEL DISCO (paso D1, 2026-09-23) ==========
//
// Arriba, `avanzar` trae un trozo EN EL TURNO DE QUIEN PREGUNTA, girando
// dentro de su syscall. Aqui el trozo lo trae el hilo del disco: planea el
// tramo (la FAT, que casi siempre sale de su cache), manda UNA orden directa
// al bufer del fichero y duerme hasta la IRQ. Al terminar sube la secuencia
// de la ranura y despierta a quien la espere con `WAIT` sobre su handle.
//
// ** El `avanzar` de arriba SE QUEDA: es el camino de una ranura que no lleva
// el hilo (no arranco, o se abrio antes), y el de quien pide bytes que todavia
// no han llegado. Antes de trabajar, cierra el vuelo si era de su ranura: los
// dos caminos no pueden contar el mismo trozo dos veces.

/// La ranura la lleva el hilo: solo entonces se concede esperar sobre ella.
pub(super) static mut POR_HILO: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];

/// El trozo que esta en el aparato: `(ranura, bytes utiles, cluster siguiente)`.
static mut EN_VUELO: Option<(usize, usize, u32)> = None;
/// Por donde va el reparto entre ranuras: una vuelta cada una, sin favoritos.
static mut ULTIMA: usize = 0;

/// Cuanto se pide al aparato de una vez. Una orden de 1 MiB son ~2 ms en este
/// SATA, y cada una despierta a quien espera: mas chico seria despertarlo por
/// nada; mas grande, que tarde en ver los primeros bytes.
const TROZO_HILO: usize = 1024 * 1024;

/// La llave sobre la que duerme quien espera la ranura `i`.
pub(crate) fn llave(i: usize) -> u64 {
    0xD15C_A000_0000_0000 | i as u64
}

/// **La secuencia de la ranura**: lo mismo que contesta `ARCH_OP_LISTO` -- los
/// bytes que ya llegaron, y el bit 63 cuando esta entero. `WAIT` duerme
/// mientras no cambie.
pub(crate) fn secuencia(i: usize) -> u64 {
    if i >= MAX_ABIERTOS {
        return 1 << 63;
    }
    let entero = if hay(i) { 0 } else { 1u64 << 63 };
    entero | unsafe { LARGO[i] } as u64
}

/// La lleva el hilo?
pub(crate) fn por_hilo(i: usize) -> bool {
    i < MAX_ABIERTOS && unsafe { POR_HILO[i] }
}

/// **Lo que el trozo en vuelo trajo, apuntado en su ranura.** Y quien espere,
/// despierto: con los bytes o con el aviso de que no van a llegar.
unsafe fn aplicar(i: usize, movidos: Option<u16>, bytes: usize, siguiente: u32) {
    let pedidos = bytes.div_ceil(512);
    match movidos {
        Some(n) if n as usize >= pedidos => {
            LARGO[i] += bytes;
            if siguiente == 0 || LARGO[i] >= LOAD_TOTAL[i] {
                LOAD_CLUSTER[i] = 0;
                crate::ring0::cabina::info("arch", "archivo completo (hilo del disco)", LARGO[i] as u64);
            } else {
                LOAD_CLUSTER[i] = siguiente;
            }
        }
        // Corto o fallido: se para AQUI. Un archivo corto se nota; volver a
        // pedir lo mismo seria un bucle con el disco dentro.
        _ => {
            LOAD_CLUSTER[i] = 0;
            crate::ring0::cabina::warn("arch", "el disco no entrego el trozo: archivo corto", LARGO[i] as u64);
        }
    }
    crate::ring0::task::scheduler::wake_by_key(llave(i));
}

/// **Cierra el vuelo, esperando si hace falta.** Para quien no puede seguir sin
/// el: traer por el camino de arriba, o soltar el bufer que es su destino.
pub(super) fn cerrar_vuelo() {
    unsafe {
        let Some((i, bytes, sig)) = EN_VUELO else { return };
        match crate::ring0::dev::disk::esperar_vuelo() {
            crate::ring0::dev::disk::EstadoVuelo::Termino(r) => {
                EN_VUELO = None;
                aplicar(i, r, bytes, sig);
            }
            // Sin orden en el aparato y con una apuntada aqui: se perdio. Se
            // trata como fallo, que es lo que no deja nada a medias.
            _ => {
                EN_VUELO = None;
                aplicar(i, None, bytes, sig);
            }
        }
    }
}

/// **La ranura se suelta.** Si su bufer es el destino de la orden en vuelo, se
/// espera a que acabe ANTES: soltar marcos con un DMA dentro es R-DMA-3.
///
/// Bajo el cerrojo del paso del hilo (`sin_el_hilo`): la purga de Ring 3 llega
/// aqui desde el hilo del bus, con las interrupciones abiertas.
pub(super) fn soltar(i: usize) {
    crate::ring0::dev::disk::sin_el_hilo(|| unsafe {
        if matches!(EN_VUELO, Some((j, _, _)) if j == i) {
            cerrar_vuelo();
        }
        POR_HILO[i] = false;
        LOAD_CLUSTER[i] = 0;
    })
}

/// **UNA VUELTA DEL HILO DEL DISCO.** Corre con las interrupciones cerradas.
pub fn paso() -> crate::ring0::dev::disk::Paso {
    use crate::ring0::dev::disk::{self, EstadoVuelo, Paso};
    unsafe {
        // 1. Lo que estaba en el aparato.
        if let Some((i, bytes, sig)) = EN_VUELO {
            match disk::mirar_vuelo() {
                EstadoVuelo::EnCurso => return Paso::Esperando,
                EstadoVuelo::Termino(r) => {
                    EN_VUELO = None;
                    aplicar(i, r, bytes, sig);
                }
                EstadoVuelo::Libre => {
                    EN_VUELO = None;
                    aplicar(i, None, bytes, sig);
                }
            }
        }
        // 2. La siguiente ranura con carga, en turno.
        for k in 1..=MAX_ABIERTOS {
            let i = (ULTIMA + k) % MAX_ABIERTOS;
            if !POR_HILO[i] || !hay(i) {
                continue;
            }
            ULTIMA = i;
            let ya = LARGO[i];
            let Some(t) = crate::ring0::fsys::fs::planear_trozo(LOAD_CLUSTER[i], ya, LOAD_TOTAL[i] as u32, TROZO_HILO) else {
                LOAD_CLUSTER[i] = 0;
                crate::ring0::task::scheduler::wake_by_key(llave(i));
                return Paso::Otra;
            };
            // ** El bufer es CONTIGUO (`reserve`) y va por paginas, asi que el
            // rabo del ultimo sector cabe. Se comprueba igual: un tramo que no
            // cabe es el disco escribiendo en memoria de otro.
            let fin = ya + t.sectores as usize * 512;
            if fin > super::file::capacidad(i) {
                // No cabe directo: este trozo por el camino de siempre.
                avanzar(i);
                crate::ring0::task::scheduler::wake_by_key(llave(i));
                return Paso::Otra;
            }
            let destino = super::file::fisica(i) + ya as u64;
            if disk::emitir_vuelo(t.lba, t.sectores, destino) {
                EN_VUELO = Some((i, t.bytes, t.siguiente));
                return Paso::Esperando;
            }
            aplicar(i, None, t.bytes, t.siguiente);
            return Paso::Otra;
        }
        Paso::Nada
    }
}
