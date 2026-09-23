//! **Administrar el disco desde Ring 3.** Las dos ordenes que ACTUAN.
//!
//! === Por que esto no vive en `sys.rs` con las demas ===
//!
//! Porque no es la misma clase de llamada. `info`, el klog, CABINA y el cursor
//! de ESTRATOS **contestan**; esto **manda**, y sobre el aparato donde vive el
//! trabajo del propietario. Un fichero propio hace que se vea al abrir la carpeta, y
//! esa es toda la ambicion del corte.
//!
//! === Lo que no se puede pedir desde aqui, y es a proposito ===
//!
//! Un LBA. Ninguna de las dos ordenes acepta una direccion: el rango de un
//! recorte lo calcula el kernel a partir de `log_head` --el puntero que solo
//! avanza-- y lo vuelve a comprobar contra la ventana de escritura. Una
//! `trim(lba, n)` desde Ring 3 seria un borrado apuntable a cualquier sector del
//! disco, incluida la particion de arranque.
//!
//! === Y por que el motivo vuelve empaquetado ===
//!
//! Por la puerta cabe **un** numero. Un `0` a secas obligaria a adivinar cual de
//! las cinco puertas dijo que no, y son cinco conversaciones distintas -- una es
//! del aparato, otra es un estado que se puede ganar, otra es un bug del que
//! llama. El byte alto lleva el motivo y el resto los sectores.

use crate::*;

/// Devolverle al disco la cola libre del volumen ESTRATOS.
pub const DISCO_OP_TRIM_LIBRE: u64 = 0x01;
/// `FLUSH CACHE` a mano.
pub const DISCO_OP_BARRERA: u64 = 0x02;
/// **El metro**: leer y cronometrar. Solo lee.
pub const DISCO_OP_BANDA: u64 = 0x03;

/// Medido: los bits bajos son los MB/s.
pub const DISCO_BANDA_HECHA: u64 = 0;
pub const DISCO_BANDA_SIN_DISCO: u64 = 1;
/// No hay particion de datos reconocida.
pub const DISCO_BANDA_SIN_VOLUMEN: u64 = 2;
pub const DISCO_BANDA_SIN_MEMORIA: u64 = 3;
/// Una lectura fallo a medias; lo leido SI queda medido.
pub const DISCO_BANDA_FALLO: u64 = 4;
pub const DISCO_BANDA_SIN_RELOJ: u64 = 5;

/// Se hizo. Los sectores dicen cuantos.
pub const DISCO_TRIM_HECHO: u64 = 0;
pub const DISCO_TRIM_SIN_DISCO: u64 = 1;
/// El disco **no declara TRIM** (palabra 169).
pub const DISCO_TRIM_NO_SOPORTADO: u64 = 2;
/// El gate de identidad o la ventana de escritura dijeron que no.
pub const DISCO_TRIM_SIN_PERMISO: u64 = 3;
/// No hay volumen ESTRATOS montado, o su cola libre esta vacia.
pub const DISCO_TRIM_SIN_VOLUMEN: u64 = 4;
pub const DISCO_TRIM_RANGO: u64 = 5;
/// El disco rechazo la orden. **Los sectores llevan lo que SI se recorto.**
pub const DISCO_TRIM_FALLO: u64 = 6;

pub const DISCO_TRIM_MOTIVO_SHIFT: u64 = 56;
pub const DISCO_TRIM_SECTORES_MASK: u64 = (1 << 56) - 1;

/// **Recorta la cola libre del volumen.** Devuelve `(motivo, sectores)`.
///
/// Con `motivo == DISCO_TRIM_HECHO` los sectores son los que se devolvieron. Con
/// `DISCO_TRIM_FALLO` son los que se devolvieron **antes** de romperse: un
/// recorte a medias no se deshace, y quien lo pinte tiene que poder decirlo.
///
/// [!] Tarda lo que tarde el disco: la cola de un volumen grande son cientos de
/// ordenes. Quien la llame deberia avisar en pantalla ANTES, como hace `smp`.
pub fn trim_libre() -> (u64, u64) {
    let v = invoke(CURRENT_TASK, OP_DISCO, DISCO_OP_TRIM_LIBRE, 0, 0).value;
    (v >> DISCO_TRIM_MOTIVO_SHIFT, v & DISCO_TRIM_SECTORES_MASK)
}

/// **La barrera, a mano.** `true` si el disco confirmo.
///
/// Este disco declara `SOLO_BARRERA`: no tiene condensadores, asi que esto es
/// literalmente lo unico que separa *"el disco se quedo los bytes"* de *"los
/// bytes sobreviven a un corte"*. Poder pedirla desde donde se trabaja es lo que
/// hace comprobable esa frase.
pub fn barrera() -> bool {
    invoke(CURRENT_TASK, OP_DISCO, DISCO_OP_BARRERA, 0, 0).value != 0
}

/// **El metro de lectura.** `mib` = cuanto (0 = 64; el kernel pone techo).
/// Devuelve `(motivo, MB/s)`; el detalle queda en `INFO_DISCO_BANDA*`.
///
/// [!] Tiene el disco para si mientras mide y la llamada no vuelve hasta
/// acabar: quien la llame avisa en pantalla ANTES, como `trim ya`.
pub fn banda(mib: u64) -> (u64, u64) {
    let v = invoke(CURRENT_TASK, OP_DISCO, DISCO_OP_BANDA, mib, 0).value;
    (v >> DISCO_TRIM_MOTIVO_SHIFT, v & DISCO_TRIM_SECTORES_MASK)
}

/// El motivo del metro en palabras.
pub fn banda_en_palabras(motivo: u64) -> &'static [u8] {
    match motivo {
        DISCO_BANDA_HECHA => b"medido",
        DISCO_BANDA_SIN_DISCO => b"no hay disco listo",
        DISCO_BANDA_SIN_VOLUMEN => b"no hay particion de datos: no se mide fuera de lo de BMO-X",
        DISCO_BANDA_SIN_MEMORIA => b"no hubo memoria contigua para el bufer",
        DISCO_BANDA_FALLO => b"una lectura FALLO a medias (CABINA dice el LBA)",
        DISCO_BANDA_SIN_RELOJ => b"el reloj no esta calibrado: sin reloj no hay metro",
        _ => b"motivo desconocido",
    }
}

// -- ** POR QUE FALLO, cuando el motivo es `DISCO_TRIM_FALLO` ---------------
//
// Las cinco maneras de fallar del driver, que mandan a mirar sitios DISTINTOS:
// `SIN_TIEMPO` acusa al presupuesto de espera, `APARATO` acusa al disco, y
// `PETICION` acusa a quien armo el payload. Aplanarlas en "rechazo" --que es lo
// que se hizo la primera vez-- pierde la unica pista que hay.

pub const DISCO_FALLO_NINGUNO: u64 = 0;
pub const DISCO_FALLO_NO_LISTO: u64 = 1;
pub const DISCO_FALLO_OCUPADO: u64 = 2;
/// No termino a tiempo. **No dijo que no: no contesto.**
pub const DISCO_FALLO_SIN_TIEMPO: u64 = 3;
/// El disco contesto con error. El `PxTFD` dice cual.
pub const DISCO_FALLO_APARATO: u64 = 4;
pub const DISCO_FALLO_PETICION: u64 = 5;
pub const DISCO_FALLO_CLASE_SHIFT: u64 = 32;
pub const DISCO_FALLO_TFD_MASK: u64 = 0xFFFF_FFFF;

/// La clase de fallo en palabras, y **cada una manda a mirar otro sitio**.
pub fn fallo_en_palabras(clase: u64) -> &'static [u8] {
    match clase {
        DISCO_FALLO_NINGUNO => b"sin fallo",
        DISCO_FALLO_NO_LISTO => b"el puerto no estaba preparado",
        DISCO_FALLO_OCUPADO => b"el disco no solto BSY/DRQ: ni se le pudo dar",
        DISCO_FALLO_SIN_TIEMPO => b"NO CONTESTO A TIEMPO (no es que dijera que no)",
        DISCO_FALLO_APARATO => b"el disco CONTESTO CON ERROR",
        DISCO_FALLO_PETICION => b"la peticion era imposible antes de salir",
        _ => b"clase desconocida",
    }
}

/// El motivo en palabras, para pintarlo sin una segunda tabla en cada llamante.
pub fn motivo_en_palabras(motivo: u64) -> &'static [u8] {
    match motivo {
        DISCO_TRIM_HECHO => b"hecho",
        DISCO_TRIM_SIN_DISCO => b"no hay disco listo",
        DISCO_TRIM_NO_SOPORTADO => b"este disco NO declara TRIM (palabra 169)",
        DISCO_TRIM_SIN_PERMISO => b"sin permiso: gate de identidad o ventana (F11 dice cual)",
        DISCO_TRIM_SIN_VOLUMEN => b"no hay volumen ESTRATOS montado, o su cola esta vacia",
        DISCO_TRIM_RANGO => b"el rango no es representable",
        DISCO_TRIM_FALLO => b"el disco RECHAZO la orden a mitad",
        _ => b"motivo desconocido",
    }
}
