//! **CARRIL ROJO** -- preguntar con la maquina ya rota.
//!
//! [carril]  ROJO      el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//! [consumo] NADA      solo corre cuando algo falla
//!
//! [cuesta]  MAQUINA -- por herencia, y es la misma herencia que `amarilla.rs`:
//!           esto camina la tabla de tareas y el mapa de marcos SIN CERROJO,
//!           con el kernel ya en el suelo. Colgarse aqui cambia un volcado
//!           legible por una maquina muda, que es peor que no preguntar.
//!
//! [riesgo]  AJENO SILENCIO
//!           AJENO    -- ni un solo numero de aqui lo escribe este fichero:
//!                       salen de `scheduler`, de `phys` y de una palabra que
//!                       hay en una pila que puede estar pisada. Lo unico que
//!                       se hace es COTEJARLOS.
//!           SILENCIO -- un testigo que contesta `None` cuando deberia contestar
//!                       exonera al culpable. Por eso todo lo de aqui devuelve
//!                       [`Dato`] o un veredicto con casos, y nunca un hueco.
//!
//! # *** LA PREGUNTA QUE EL KERNEL NO SABIA HACER (2026-09-20)
//!
//! La azul del 20-09 murio en `cabina::ring::record_fmt` leyendo un `&Location`
//! podrido. Ese puntero lo empuja el compilador como constante, y las 534
//! llamadas del binario lo empujan bien. O sea que la conclusion es directa:
//! **alguien escribio encima de la pila de tid=05**.
//!
//! Y a esa pregunta el kernel no tenia con que contestar. Sabia:
//!
//! ```text
//!    titular_de_pila(rsp)   de QUIEN es esta pila
//!    fue_de_quien(rsp)      de quien FUE, si ya no es de nadie  (30-08)
//! ```
//!
//! ** Las dos contestan sobre la PROPIEDAD, y ninguna sobre la INTEGRIDAD. La
//! segunda ademas solo habla cuando la pila no es de nadie vivo -- y la del
//! 20-09 era de tid=05, que estaba corriendo. O sea que el caso exacto que paso
//! es justo el que caia entre las dos.
//!
//! # Las tres preguntas de aqui, y por que son TRES y no una
//!
//! ```text
//!    1  el CENTINELA del fondo  -> esta pila la piso alguien?
//!    2  la MORGUE, pagina a pagina -> se entrego dos veces?
//!    3  el ASIGNADOR, pagina a pagina -> esta VIVA sobre marcos LIBRES?
//! ```
//!
//! Son tres porque separan tres culpables distintos, y confundirlos manda a
//! leer el fichero que no es:
//!
//! ```text
//!    1 rota y el `rsp` CERCA del fondo   -> la pila se desbordo SOLA
//!    1 rota y el `rsp` LEJOS del fondo   -> escribio OTRO
//!    2 contesta                          -> `reap` la solto y se re-entrego
//!    3 contesta                          -> el asignador la da por libre
//!                                           mientras alguien corre encima
//! ```
//!
//! ** Y el 2 y el 3 tienen nombre desde el 31-08: `reap` decide liberar mirando
//! el estado de la tarea y su propio `rsp`, y su comentario lista CUATRO
//! punteros publicados que no mira. Si el 2 contesta, ese comentario deja de
//! ser una sospecha.
//!
//! [!] Lo que esto NO dice: POR QUE. Dice si la pila esta entera y, cuando no,
//! **que hay escrito donde deberia estar el centinela** -- que es lo que nombro
//! al culpable el 04-09, cuando trece casillas de una tabla resultaron ser
//! `push r15; push r14; push r12`.

use super::verde::Dato;

/// **Que le pasa a la pila viva donde cayo el `rsp`.**
pub(in crate::ring0::plat::faults) enum Pila {
    /// El `rsp` no cae en ninguna pila de la tabla. No es un hallazgo de aqui:
    /// `amarilla.rs` ya tiene la morgue para ese caso.
    NoEsDeNadieVivo,
    /// El centinela del fondo esta donde tiene que estar.
    ///
    /// ** No dice de QUIEN. Este caso no se pinta --que algo este bien no es un
    /// renglon-- y un campo que nadie lee es la misma clase de adorno que la
    /// ley caza en otros sitios: se declara lo que se usa.
    Entera,
    /// La pila es de `tid` y **el centinela no esta**. `hay` es lo que hay en
    /// su sitio --la pista-- y `hueco` lo que le quedaba al `rsp` hasta el
    /// fondo, que es lo que separa un desbordamiento de una escritura ajena.
    Pisada { tid: u32, hay: u64, hueco: u64 },
}

/// El fondo de la pila de `rsp`, cotejado con su centinela.
///
/// ** Se lee con `read_volatile` a proposito: el compilador no tiene forma de
/// saber que ese sitio se escribio desde otro contexto, y una lectura que el
/// optimizador resuelve en compilacion es un instrumento que contesta sin
/// mirar.
pub(in crate::ring0::plat::faults) fn pila_viva(rsp: u64) -> Pila {
    let (tid, fisica, _paginas) = match crate::ring0::task::scheduler::rango_de_pila(rsp) {
        Some(x) => x,
        None => return Pila::NoEsDeNadieVivo,
    };
    let fondo = crate::ring0::mm::phys_to_virt(fisica);
    let hay = unsafe { (fondo as *const u64).read_volatile() };
    if hay == crate::ring0::task::scheduler::CENTINELA {
        return Pila::Entera;
    }
    Pila::Pisada {
        tid,
        hay,
        hueco: rsp.saturating_sub(fondo),
    }
}

/// **Alguna pagina de esta pila VIVA paso por la morgue o esta libre AHORA.**
///
/// Devuelve `(pagina, fisica_de_esa_pagina, que)` de la PRIMERA que conteste,
/// con `que`:
///
/// ```text
///    1  el asignador la da por LIBRE, y hay alguien corriendo encima
///    2  la morgue la tiene: se libero y se volvio a entregar
///    3  se devolvio DOS VECES
/// ```
///
/// [!] Se para en la primera. Cuatro paginas rotas no dicen mas que una --dicen
/// lo mismo cuatro veces-- y la pantalla azul tiene 112 bytes por renglon: la
/// leccion del cepo del 30-08 es que un instrumento que repite tapa su propio
/// mensaje.
pub(in crate::ring0::plat::faults) fn pagina_sospechosa(rsp: u64) -> Option<(u64, u64, u8)> {
    let (_tid, fisica, paginas) = crate::ring0::task::scheduler::rango_de_pila(rsp)?;
    for p in 0..paginas {
        let f = fisica + p * crate::ring0::mm::PAGE;
        if let Some(true) = crate::ring0::mm::phys::esta_libre(f) {
            return Some((p, f, 1));
        }
        if crate::ring0::mm::phys::se_devolvio_dos_veces(f).is_some() {
            return Some((p, f, 3));
        }
        if crate::ring0::task::scheduler::fue_de_quien(crate::ring0::mm::phys_to_virt(f)).is_some()
        {
            return Some((p, f, 2));
        }
    }
    None
}

/// **Los dos limites de `.text`, del enlazador.** Vive aqui y no en el renglon
/// que lo usa porque es exactamente lo mismo que el resto de este fichero:
/// preguntarle un hecho a otro, con la maquina rota.
///
/// [!] Sin `unsafe`: TOMAR la direccion de un `static` externo es seguro -- lo
/// que no lo seria es leerlo. Aqui no se lee ni un byte: los dos simbolos no
/// tienen contenido, su valor ES su direccion.
pub(in crate::ring0::plat::faults) fn texto_del_kernel() -> (u64, u64) {
    extern "C" {
        static __text_start: u8;
        static __text_end: u8;
    }
    (
        core::ptr::addr_of!(__text_start) as u64,
        core::ptr::addr_of!(__text_end) as u64,
    )
}

/// **Cuanto hay del principio de `.text` hasta el `rip`** -- el numero que come
/// `simbolo.py` y que pone el nombre de la funcion sin tener la maquina
/// delante.
///
/// *** ESTE ES EL RENGLON QUE SALIO VACIO EL 20-09, y por eso el tipo empieza
/// aqui. La foto del Ryzen decia literalmente:
///
/// ```text
///    en .text del kernel, +0x
/// ```
///
/// ** Aquello fue un `hex(v, 0)` que no escribia un solo digito, y ya esta
/// arreglado. Pero el arreglo de aquel dia no impedia el de luego: un
/// desplazamiento sigue siendo un `u64` en el que **cero significa dos cosas**
/// --*"el rip es el primer byte de `.text`"* y *"no hay desplazamiento que
/// dar"*--. Con [`Dato`] el segundo caso tiene que escribir su motivo, y la
/// unica forma de que el renglon salga mudo es que alguien lo borre a mano.
pub(in crate::ring0::plat::faults) fn desplazamiento_en_texto(rip: u64) -> Dato {
    let (ini, fin) = texto_del_kernel();
    if ini == 0 || fin <= ini {
        return Dato::NoSabido("el enlazador no publico .text");
    }
    if rip < ini || rip >= fin {
        return Dato::NoSabido("el rip NO cae en .text");
    }
    Dato::Sabido(rip - ini)
}

/// Cuantas veces llego a CABINA un `Location` que no vive en `.rodata`, y el
/// ultimo de esos punteros. `None` = **ninguno**, que exonera a toda esa
/// familia de golpe.
///
/// ** El `None` va aqui y no en quien pinta, y es el mismo movimiento que
/// `desmontaje::donde()`: quien sabe si hay algo que decir es el que pregunta.
/// Dejar que el renglon lo decidiera obligaria a `amarilla.rs` a saberse que
/// cero es especial -- o sea a repetir aqui la regla que ya esta alla.
pub(in crate::ring0::plat::faults) fn sitios_podridos() -> Option<(Dato, Dato)> {
    let (n, ultimo) = crate::ring0::cabina::sitios_podridos();
    if n == 0 {
        return None;
    }
    Some((Dato::Sabido(n), Dato::Sabido(ultimo)))
}
