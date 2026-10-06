//! **EL RENGLON DE LOS GESTOS SOBRE ESTRATOS** -- crear, borrar, renombrar.
//!
//! [carril]  AMARILLO  crear, borrar y renombrar sobre ESTRATOS
//! [consumo] NADA      corre solo cuando una tarea cruza la puerta
//!
//! === Por que es un fichero, y por que se llama asi ===
//!
//! Por L6a: `syscall/mod.rs` esta en la linea base del censo y **no puede
//! crecer**. Pero el corte no es por medida -- es que este brazo del despacho
//! dejo de ser "crear un fichero" el dia que la maquina de abajo aprendio
//! cuatro verbos.
//!
//! Se llamaba `TASK_OP_ES_CREAR`, y un renglon que tambien BORRA no se puede
//! seguir llamando "crear": un nombre que miente es peor que uno feo, porque el
//! que lo lee deja de comprobar.
//!
//! === Un renglon, cuatro verbos ===
//!
//! Igual que abajo. `fsys::estratos::escribir::aplicar` es UNA maquina con un
//! `Gesto` de cuatro variantes; aqui hay UNA operacion con cuatro subordenes.
//! **La forma de la puerta es la forma del codigo que sirve**, y eso no es
//! estetica: dos puertas para una maquina es como una de las dos se queda sin
//! el arreglo que se le hizo a la otra.
//!
//! ```text
//!   LIMPIAR     vacia el renglon del contenido
//!   DATOS       mete ocho bytes en el (contenido, o el nombre nuevo)
//!   FICHERO     crea un fichero con lo acumulado dentro
//!   CARPETA     crea una carpeta vacia
//!   QUITAR      quita una entrada
//!   RENOMBRAR   le cambia el nombre a una entrada
//!   ES_RAMA_*   crear y cambiar de rama, y mezclar en dos fases (R4c-2b)
//! ```
//!
//! === ** LA RUTA LLEVA EL DESTINO ENTERO, y el kernel la parte ===
//!
//! El renglon de la ruta trae `datos/notas/x.txt` y aqui se corta en
//! `("datos/notas", "x.txt")`. La alternativa era un tercer renglon para el
//! nombre, y no hace falta: **una ruta ya contiene su ultimo tramo**.
//!
//! Ademas es lo que uno escribe. `borra datos/notas/x.txt` es una frase; una
//! carpeta por un canal y un nombre por otro es un formulario.
//!
//! [!] `RENOMBRAR` es el unico que necesita dos nombres, y el segundo va por el
//! renglon del CONTENIDO. No es un arreglo: ese renglon lleva una cuenta explicita
//! de bytes, asi que un nombre entra tal cual y sin ambiguedad.
//!
//! === Y CABINA no esta aqui ===
//!
//! A proposito. El aviso de que un gesto fallo vive en `escribir::aplicar`, que
//! es por donde pasan los cuatro: ponerlo en cada brazo de este `match` seria
//! cuatro sitios donde olvidarse del quinto. Aqui solo se anota QUIEN lo pidio,
//! que es lo unico que este lado sabe y el otro no.

use super::ops::*;
use super::{datos_limpiar, datos_meter, datos_tomar, ruta_tomar, ruta_tomar_cruda};
use crate::ring0::fsys::estratos::escribir::{self, Gesto};
use crate::ring0::fsys::estratos::ramas;
use crate::ring0::obj::cap;

/// Parte `a/b/c.txt` en `("a/b", "c.txt")`.
///
/// Sin tramo final devuelve `None`: `borra datos/` no dice que borrar, y
/// adivinarlo --tomar `datos` como el objetivo-- seria borrar la carpeta cuando
/// se pidio borrar algo de dentro.
fn partir(ruta: &str) -> Option<(&str, &str)> {
    let ruta = ruta.trim_end_matches(['/', '\\']);
    if ruta.is_empty() {
        return None;
    }
    match ruta.rfind(['/', '\\']) {
        Some(i) => {
            let nombre = &ruta[i + 1..];
            if nombre.is_empty() {
                None
            } else {
                Some((&ruta[..i], nombre))
            }
        }
        // Sin barras: esta en la raiz, y la raiz es la ruta vacia.
        None => Some(("", ruta)),
    }
}

/// Sirve una suborden del renglon. Devuelve la generacion nueva, o `0`.
///
/// ** El `0` es "no se hizo" y no trae el motivo, igual que antes: el motivo va
/// a CABINA, que es donde caben las frases. Ring 3 no puede hacer nada distinto
/// con "no cabe" que con "esa ruta no existe" salvo ensenarselo a una persona,
/// y para eso esta F11.
pub(super) fn servir(pid: u32, arg0: u64, arg1: u64) -> u64 {
    // ** MARCAR, VOLVER y LAS RAMAS no llevan ruta y tocan el volumen ENTERO
    // (H3): a un proceso encerrado, no. Los renglones se vacian igual, como en
    // `hacer`.
    if matches!(arg0 & 0xFF, ES_GESTO_MARCAR | ES_GESTO_VOLVER | ES_RAMA_CREAR..=ES_RAMA_MEZCLAR)
        && crate::ring0::task::raiz::encerrado(pid)
    {
        ruta_tomar_cruda(pid);
        datos_tomar(pid);
        crate::ring0::cabina::warn("raiz", "un proceso encerrado quiso tocar las versiones o las ramas del volumen", pid as u64);
        return 0;
    }
    match arg0 & 0xFF {
        ES_GESTO_LIMPIAR => {
            datos_limpiar(pid);
            0
        }
        // `arg1` son los ocho bytes y los bits altos de `arg0` CUANTOS valen. La
        // ruta se corta en el primer cero porque en una ruta un cero no puede
        // aparecer; en un contenido SI, asi que aqui la cuenta es explicita o se
        // entregaria la mitad de un fichero.
        ES_GESTO_DATOS => datos_meter(pid, arg1, arg0 >> 8) as u64,
        ES_GESTO_FICHERO => hacer(pid, "fichero nuevo", |ruta, datos| {
            let (dir, nombre) = partir(ruta)?;
            Some(Gesto::Fichero { nombre, datos })
                .map(|g| escribir::aplicar(dir, g))
        }),
        ES_GESTO_CARPETA => hacer(pid, "carpeta nueva", |ruta, _| {
            let (dir, nombre) = partir(ruta)?;
            Some(escribir::aplicar(dir, Gesto::Carpeta { nombre }))
        }),
        ES_GESTO_QUITAR => hacer(pid, "quitar una entrada", |ruta, _| {
            let (dir, nombre) = partir(ruta)?;
            Some(escribir::aplicar(dir, Gesto::Quitar { nombre }))
        }),
        ES_GESTO_COPIA => hacer(pid, "copiar un fichero de FAT32", |ruta, datos| {
            let (dir, nombre) = partir(ruta)?;
            // El ORIGEN viene por el renglon del contenido, igual que el nombre
            // nuevo de `renombrar`. Son dos nombres y ningun byte de fichero.
            let origen = core::str::from_utf8(datos).ok()?;
            if origen.is_empty() {
                return None;
            }
            // ** El TIPO del origen no se construye aqui. `copiar_fichero` lo
            // pone, y asi `Origen` se queda dentro de `fsys::estratos` -- que
            // es de quien es. El borde resuelve capabilities y parte rutas; de
            // como se llama por dentro el sitio de donde salen los bytes no
            // tiene por que enterarse.
            Some(escribir::copiar_fichero(dir, nombre, origen))
        }),
        // ** ANOTAR DE DONDE SALE EL CONTENIDO. No lee un byte.
        ES_GESTO_ORIGEN => origen_poner(pid, arg1, arg0 >> 8),
        // ** Y EJECUTARLO. `arg1` son los bytes a tomar.
        //
        // El origen se toma --y el renglon se vacia-- ANTES de mirar la ruta, y
        // por el mismo motivo que `hacer` vacia los otros dos salga bien o mal:
        // un gesto que falla no puede dejarle el origen puesto al siguiente, y
        // el siguiente puede ser otro fichero.
        ES_GESTO_FICHERO_DE => {
            let origen = origen_tomar(pid, arg1);
            hacer(pid, "fichero desde un bloque propio", |ruta, _| {
                let (dir, nombre) = partir(ruta)?;
                let (base, size) = origen?;
                Some(unsafe { escribir::crear_desde(dir, nombre, base, size) })
            })
        }
        // ** EL QUINTO VERBO. Mismo renglon, misma puerta, y lo unico que
        // cambia es que este NO se queja si el nombre ya estaba: publica su
        // version nueva. Ver `ES_GESTO_GUARDAR` en el ABI.
        ES_GESTO_GUARDAR => {
            let origen = origen_tomar(pid, arg1);
            hacer(pid, "guardar una version nueva", |ruta, _| {
                let (dir, nombre) = partir(ruta)?;
                let (base, size) = origen?;
                Some(unsafe { escribir::guardar_desde(dir, nombre, base, size) })
            })
        }
        // ** El unico que NO parte la ruta: aqui no hay destino, hay un
        // NOMBRE. Partirlo por la ultima barra convertiria `copia de ayer` en
        // otra cosa el dia que alguien use una barra en un nombre.
        ES_GESTO_MARCAR => {
            let nombre = ruta_tomar_cruda(pid);
            datos_tomar(pid);
            crate::ring0::cabina::info("estratos", "marcar la version", pid as u64);
            escribir::marcar(nombre).unwrap_or(0)
        }
        // El unico que no necesita ningun renglon: el numero cabe en `arg1`.
        // Pedir una ruta para esto habria sido inventar un texto donde ya hay
        // un entero.
        ES_GESTO_VOLVER => {
            crate::ring0::cabina::info("estratos", "volver a una version", pid as u64);
            escribir::volver(arg1 as usize).unwrap_or(0)
        }
        // ** LAS RAMAS (R4c-2b). La ruta lleva un NOMBRE, no un destino, y no
        // se parte: igual que en MARCAR.
        ES_RAMA_CREAR | ES_RAMA_CAMBIAR | ES_RAMA_CONTAR => {
            let nombre = ruta_tomar_cruda(pid);
            datos_tomar(pid);
            if nombre.is_empty() {
                crate::ring0::cabina::warn("estratos", "un gesto de ramas sin nombre", pid as u64);
                return 0;
            }
            let r = match arg0 & 0xFF {
                ES_RAMA_CREAR => ramas::crear(nombre),
                ES_RAMA_CAMBIAR => ramas::cambiar(nombre),
                // `(choques << 32) | bloques`: los bloques cuentan el estrato,
                // asi que un conteo que sale bien nunca es cero.
                _ => ramas::contar(pid, nombre).map(|(c, b)| (c as u64) << 32 | b.min(0xFFFF_FFFF)),
            };
            dicho(pid, r)
        }
        // Lo contado se lee y se elige por numero: no hay ruta que mandar.
        ES_RAMA_CHOQUE => ramas::choque(pid, arg1 as usize, (arg0 >> 8) as usize),
        ES_RAMA_ELEGIR => ramas::elegir(pid, arg1 as usize, u8::try_from(arg0 >> 8).unwrap_or(0)) as u64,
        ES_RAMA_MEZCLAR => {
            crate::ring0::cabina::info("estratos", "mezclar una rama", pid as u64);
            dicho(pid, ramas::mezclar(pid))
        }
        ES_GESTO_RENOMBRAR => hacer(pid, "renombrar una entrada", |ruta, datos| {
            let (dir, viejo) = partir(ruta)?;
            // El nombre nuevo viene por el renglon del contenido.
            let nuevo = core::str::from_utf8(datos).ok()?;
            if nuevo.is_empty() {
                return None;
            }
            Some(escribir::aplicar(dir, Gesto::Renombrar { viejo, nuevo }))
        }),
        // Una suborden que no existe contesta cero y no un fallo: quien la mande
        // se entera igual, y un `unsupported` obligaria al que llama a
        // distinguir dos formas de "no paso nada".
        _ => 0,
    }
}

/// La generacion, o `0` con el motivo en CABINA: las ramas no pasan por
/// `escribir::aplicar`, que es quien lo dice para los otros gestos.
fn dicho(pid: u32, r: Result<u64, crate::ring0::fsys::estratos::WriteError>) -> u64 {
    match r {
        Ok(g) => g,
        Err(e) => {
            crate::ring0::cabina::warn("estratos", e.name(), pid as u64);
            0
        }
    }
}

/// El molde de los cuatro verbos: anotar quien lo pide, vaciar los renglones y
/// llamar.
///
/// * Los renglones se vacian SIEMPRE, salga bien o mal. Si no, un gesto que
/// falla le dejaria la ruta puesta al siguiente -- y el siguiente puede ser un
/// `quitar`.
fn hacer(
    pid: u32,
    que: &'static str,
    construir: impl FnOnce(&str, &[u8]) -> Option<Result<u64, crate::ring0::fsys::estratos::WriteError>>,
) -> u64 {
    let ruta = ruta_tomar(pid);
    let datos = datos_tomar(pid);
    // Fuera de la raiz (H3): no se construye nada, y los dos renglones ya se
    // vaciaron arriba.
    let Ok(ruta) = ruta else {
        crate::ring0::cabina::warn("raiz", "un gesto de ESTRATOS fuera de la raiz", pid as u64);
        return 0;
    };
    // Lo unico que este lado sabe y el otro no: QUIEN lo ha pedido. El resto de
    // la historia --que paso y por que-- lo cuenta `escribir::aplicar`.
    crate::ring0::cabina::info("estratos", que, pid as u64);
    match construir(ruta, datos) {
        Some(Ok(g)) => g,
        Some(Err(_)) => 0,
        // La ruta no daba para un gesto: sin tramo final, o un nombre nuevo que
        // no es texto. Se dice aqui porque `aplicar` no llega a verlo.
        None => {
            crate::ring0::cabina::warn("estratos", "la ruta del gesto no vale", pid as u64);
            0
        }
    }
}

// -- ** EL RENGLON DEL ORIGEN, y por que la capability se resuelve AQUI -------
//
// Los otros dos renglones --la ruta y el contenido-- viven en `syscall/mod.rs`.
// Este no, por dos razones que apuntan al mismo sitio:
//
//   1. `syscall/mod.rs` esta en la linea base de L6a con 1.363 lineas y **no
//      puede crecer**. Un renglon mas ahi lo rechaza el censo, y con razon.
//   2. Este renglon no guarda bytes: guarda una capability YA RESUELTA. Su sitio
//      es donde se resuelve, y eso es el borde del syscall -- que es este
//      fichero tanto como `mod.rs`.
//
// ** Lo que NO cambia es la regla que `mod.rs` escribio al despachar
// `ARCH_OP_LEER_EN`: *hace falta resolver una SEGUNDA capability y las
// capabilities viven en este borde*. Se resuelve en el borde. Lo que baja a
// `fsys::estratos` es una direccion ya comprobada, no un handle.
static mut ORIGEN_BASE: u64 = 0;
static mut ORIGEN_DESDE: u64 = 0;
static mut ORIGEN_PID: u32 = u32::MAX;

/// Anota el bloque `handle` con su desplazamiento. `1` si vale, `0` si no.
///
/// ** Se pide `RIGHT_READ` y no `RIGHT_WRITE`: el kernel LEE el bloque para
/// llevarselo al disco, no escribe dentro. Exigir mas autoridad de la que la
/// operacion usa es lo que un sistema de capabilities no debe hacer -- y es la
/// misma distincion que separa `ARCH_OP_LEER_EN` de `ARCH_OP_ESCRIBIR_DE`.
fn origen_poner(pid: u32, handle: u64, desde: u64) -> u64 {
    let bloque = match cap::resolve(pid, handle, cap::RIGHT_READ) {
        Ok(b) if b.kind == cap::KIND_MEMORIA => b,
        _ => {
            crate::ring0::cabina::warn("estratos", "ese handle no es un bloque propio", handle);
            return 0;
        }
    };
    unsafe {
        ORIGEN_BASE = bloque.object;
        ORIGEN_DESDE = desde;
        ORIGEN_PID = pid;
    }
    1
}

/// La direccion y el medida, **y vacia el renglon**. `None` si no cuadra.
///
/// La comprobacion del rango es contra lo que el kernel entrego, y esa es la
/// idea entera: no hay que validar un puntero de Ring 3 porque no hay ningun
/// puntero de Ring 3 -- hay un bloque que dimos nosotros.
///
/// *** LA DIRECCION QUE SALE ES LA DEL ESPEJO DEL KERNEL, NO LA DEL PROCESO
/// (2026-09-29). Hasta hoy salia `bloque.object` + `desde`, que es la VA de
/// Ring 3, y `copiar::traer` la leia tal cual. Con CR4.SMAP encendido eso es un
/// #PF de Ring 0 -- *"proteccion leyendo desde el KERNEL"*, dentro de `memcpy`,
/// con cr2 en la primera pagina del bloque --, y la maquina entera se paraba.
/// Lo piso el primero que guardo en ESTRATOS desde un bloque despues de que
/// SMAP se encendiera: F1 sembrando su biblioteca (`PLAN_TALLER` 8.6).
///
/// Y el limite se comparaba contra `handed_over_by` -- **la suma de todos los
/// bloques del proceso** --, que es exactamente el fallo que
/// `memory::bytes_de_bloque` dejo escrito el 2026-08-24 para `LEER_EN` y
/// `ESCRIBIR_DE`. Aquellos dos se corrigieron; este renglon, que es el mismo
/// camino con otro verbo, se quedo atras. Ahora hace lo mismo que
/// `ESCRIBIR_DE`: `fisica_de` exige que `[desde, desde+cuantos)` caiga dentro
/// de ESTE bloque, y los marcos de un bloque son contiguos por construccion,
/// asi que el espejo cubre el rango entero.
fn origen_tomar(pid: u32, cuantos: u64) -> Option<(u64, u32)> {
    let (base, desde) = unsafe {
        if ORIGEN_PID != pid {
            return None;
        }
        ORIGEN_PID = u32::MAX;
        (ORIGEN_BASE, ORIGEN_DESDE)
    };
    // Un fichero de cero bytes no tiene arbol que construir. Se dice aqui, que
    // es donde el numero llega, en vez de dejar que lo descubra `plan_de`.
    if cuantos == 0 || cuantos > u32::MAX as u64 {
        return None;
    }
    let fisica = base
        .checked_add(desde)
        .and_then(|va| crate::ring0::obj::memory::fisica_de(pid, va, cuantos));
    let Some(fisica) = fisica else {
        crate::ring0::cabina::warn("estratos", "ese rango no cae dentro del bloque", cuantos);
        return None;
    };
    Some((crate::ring0::mm::phys_to_virt(fisica), cuantos as u32))
}
