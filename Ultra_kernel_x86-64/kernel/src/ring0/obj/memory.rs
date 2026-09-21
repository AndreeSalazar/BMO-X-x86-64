//! **`KIND_MEMORIA`** -- pedirle memoria al kernel, y que sea tuya.
//!
//! [carril]  ROJO      pedirle memoria al kernel, y que sea tuya
//! [consumo] NADA      corre cuando una tarea usa el objeto
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! === El hueco que tapa ===
//!
//! Hasta ahora un proceso recibia su imagen y 64 KiB de pila, **y no podia
//! pedir mas**. Eso bloqueaba dos cosas a la vez y por eso lleva tanto tiempo
//! en la hoja de ruta: cualquier lenguaje con recolector de basura, y cualquier
//! programa que no sepa de antemano cuanto va a necesitar.
//!
//! === * Por que NO es un `malloc` ===
//!
//! Aqui se entrega **un bloque grande, entero y contiguo**, y se acabo. No hay
//! listas de libres, ni troceado, ni fusion de huecos. Y no es una version
//! recortada de un asignador: es que **el asignador no es trabajo del kernel**.
//!
//! El caso que lo ensena es DOOM, y por eso es el que se uso para decidir:
//! pide ~8 MiB **una vez** al arrancar y se los administra el con su propio
//! `Z_Zone`. Un `malloc` general en el kernel habria sido escribir un asignador
//! que ese programa no usa, para que encima lo llame a traves de un syscall.
//!
//! El reparto queda asi, y es el mismo de siempre en este sistema:
//!
//! ```text
//!   el kernel  entrega paginas y dice donde estan
//!   el proceso decide que hace con ellas
//! ```
//!
//! Un `malloc` de C se escribe **encima** de esto en Ring 3, con la politica
//! que quiera cada lenguaje -- y COBOL, Ada o un GC futuro pueden traer otra
//! sin pedirle permiso al kernel.
//!
//! === Lo que NO hay, dicho entero ===
//!
//! - **SE DEVUELVE desde el 2026-09-20** ([`MEM_OP_SOLTAR`]). Aqui ponia *"no
//!   hay `liberar`"*, y Ring 3 estaba escrito como si lo hubiera:
//!   `fondo.rs` decia literalmente *"el fichero se suelta al acabar"* sobre un
//!   valor que solo se caia del alcance. Las dos mitades no se hablaban, y lo
//!   pago el escritorio -- cuatro peticiones gastadas antes de abrir nada, y el
//!   visor de imagenes sin cupo para las suyas.
//!   Lo que sigue sin haber es un `malloc`: se devuelve **el bloque entero**,
//!   el que se pidio, y no un trozo.
//! - **La VA no se reusa.** Soltar devuelve los marcos y la ranura; la
//!   direccion no. Ver el techo en `request`: gastar VA sale mas barato que
//!   razonar sobre un handle viejo que vuelve a resolver.
//! - **Contiguo en fisico.** Se piden marcos seguidos porque un bloque que el
//!   programa recorre como un array tiene que serlo. Si la RAM esta
//!   fragmentada y no hay hueco, se rechaza y **se dice** -- entregar memoria a
//!   trozos sin avisar seria peor.
//! - **Sin tocar los dos syscalls.** Esto es una capability mas y una
//!   operacion mas sobre `CURRENT_TASK`, como el framebuffer o la consola.

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Cuanto puede pedir un proceso de una vez.
///
/// 64 MiB: ocho veces lo que pide DOOM, y aun asi un numero que no se puede
/// pedir por accidente. Un tope alto y explicito es mejor que ninguno -- sin
/// el, un `request(-1)` mal calculado se lleva la maquina entera.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Cuantos bloques puede tener un proceso **A LA VEZ**.
///
/// *** ERAN CUATRO, Y LO QUE CONTABAN NO ERA ESTO (2026-09-20).
///
/// El numero llevaba escrito su propio motivo: *"no hay forma de devolver
/// memoria, asi que el numero de peticiones ES el numero de fugas posibles"*.
/// Cierto mientras no se pudiera devolver -- y por eso cuatro era **el
/// presupuesto de toda la vida del proceso**, no un limite de concurrencia.
///
/// ** Lo que costaba, medido en el escritorio: el doble bufer, el fichero del
/// fondo, los pixeles del fondo y la consola son CUATRO, y se gastan antes de
/// que el dueno abra nada. El visor de imagenes pide tres mas --fichero,
/// pixeles y el taller del inflate-- y los tres se estrellaban.
///
/// Con [`MEM_OP_SOLTAR`] el contador baja, asi que esto vuelve a ser lo que su
/// nombre dice: **cuantos a la vez**. Ocho porque es lo que cabe en una sesion
/// del escritorio con el visor abierto y todavia sobra, y porque dimensiona la
/// tabla `bloques` de cada ranura: 8 x 16 procesos x 24 B = 3 KiB, y ni una
/// asignacion.
///
/// [!] Subirlo YA NO es subir el numero de fugas. Lo era antes, y por eso no se
/// subio antes.
pub const MAX_PETICIONES: usize = 8;

/// **El techo de la VA de bloques.** Justo debajo de donde empiezan los
/// PRESTAMOS (`loan::PRESTAMO_VA_BASE`), que es el vecino de arriba.
///
/// ** Se escribe el numero y no se importa el de al lado a proposito: `loan`
/// esta en la misma familia y elige SU base diciendo *"lejos de
/// `MEMORIA_VA_BASE`"*. Las dos constantes se miran, y un guardian que las
/// compare es trabajo que todavia no existe; lo que si existe es que pasarse de
/// aqui se dice en voz alta en vez de mapear encima de lo ajeno.
const MEMORIA_VA_TOPE: u64 = 0x0000_0001_0000_0000;

/// Donde empieza el bloque. Espejo de `bmo_abi::...::MEM_OP_BASE`.
pub const MEM_OP_BASE: u64 = 0x01;
/// Cuantos bytes se le han entregado a este proceso.
pub const MEM_OP_BYTES: u64 = 0x02;

/// **La direccion FISICA del bloque.** Pieza S2 del suelo de Ring 3.
///
/// # Para que sirve, y para que NO
///
/// Un driver de Ring 3 que quiera hablar por DMA tiene que escribir en un
/// descriptor la direccion **que ve la tarjeta**, y la tarjeta no pasa por la
/// MMU: ve fisicas. Sin esto no hay forma de construir un anillo de recepcion,
/// que es literalmente el paso 2 de `docs/maestro/RED_MAESTRO.md`.
///
/// ** Y no vale para nada mas. Una fisica es UN NUMERO: solo es peligrosa si
/// algo la acepta como orden, y en este sistema lo unico que lo haria es un
/// aparato haciendo DMA -- que es un problema que ya existe y no uno nuevo. Ver
/// la parte 4 de `docs/plan/PLAN_SUELO_RING3.md`.
///
/// # *** LAS DOS COSAS QUE HACEN QUE ESTE NUMERO NO SEA UNA MENTIRA
///
/// 1. **El bloque es CONTIGUO.** Sale de `alloc_frames_contig`, asi que la
///    fisica del primer marco mas un desplazamiento es la fisica de ese
///    desplazamiento. Si los marcos estuvieran sueltos, contestar "la fisica del
///    bloque" seria cierto para la primera pagina y falso para el resto -- y el
///    fallo lo pagaria la tarjeta escribiendo en memoria de otro.
///
/// 2. **No se mueve.** Hoy nada mueve un marco despues de asignarlo: no hay
///    intercambio a disco, ni compactacion, ni paginas grandes que se partan.
///    ** Y por eso justamente se escribe: la promesa cuesta CERO hoy, y el dia
///    que alguien anada cualquiera de esas tres, esta linea es lo que le dice
///    que hay un contrato que respetar. Una propiedad verdadera por accidente
///    deja de serlo sin que nadie lo note.
///
/// [!] Solo la contesta el DUENO del bloque, porque es una operacion sobre su
/// propia capability. Saber donde vive la memoria del vecino no le hace falta a
/// nadie.
pub const MEM_OP_FISICA: u64 = 0x04;

/// **Devolver el bloque.** Espejo de `bmo_abi::...::MEM_OP_SOLTAR`.
///
/// Contesta 1 si se devolvio y 0 si no se pudo --hoy el unico motivo es que
/// siga PRESTADO a otro--, y el motivo va a CABINA. **Un cero no es un fallo
/// del que llama**: es que hay alguien leyendo esa memoria ahora mismo.
pub const MEM_OP_SOLTAR: u64 = 0x05;

pub const ERROR_TOO_BIG: u32 = 0xE001;
pub const ERROR_NO_RAM: u32 = 0xE002;
pub const ERROR_TOO_MANY: u32 = 0xE003;
/// No queda ranura en la tabla de contabilidad: ya hay [`MAX_PROCS`] procesos
/// con memoria pedida. **Es un motivo distinto de `ERROR_TOO_MANY`** y por eso
/// tiene codigo propio -- uno dice "tu has pedido demasiado", el otro "el sistema
/// esta lleno", y confundirlos manda a buscar el fallo al programa equivocado.
pub const ERROR_NO_SLOT: u32 = 0xE004;

/// Cuantos procesos pueden tener memoria pedida **a la vez**.
///
/// * Y "a la vez" es la correccion entera. Esto era el tope de *pids* -- la
/// tabla se indexaba con `pid as usize` y se rechazaba `pid >= 16`. Pero el pid
/// es un **contador que solo sube y nunca se reutiliza** (`proc::next_pid`), asi
/// que a partir del programa numero 16 de un arranque **ningun proceso podia
/// volver a pedir memoria jamas**, y encima con el motivo equivocado:
/// `ERROR_TOO_MANY`, que quiere decir "has pedido demasiadas veces".
///
/// No era teorico: en la foto del 2026-07-30, `info` decia **17 lanzados** en
/// una sola sesion.
///
/// Es el **patron 17** otra vez --indexar algo VIVO con un contador HISTORICO--,
/// el mismo que dejo la maquina sin poder lanzar nada cuando `has_room()`
/// miraba una bitacora de ocho entries. Ahora la tabla es de ranuras, se busca
/// por pid y **se libera al morir el proceso**: el tope vuelve a ser lo que
/// dice ser, un limite de recursos concurrentes.
const MAX_PROCS: usize = 16;

/// **Un bloque entregado: donde lo ve el proceso y donde esta de verdad.**
///
/// === Por que se guarda la fisica, si nadie la pedia ===
///
/// Porque el kernel **acaba de reservarla** y tirarla es tirar una respuesta
/// segura. El caso que la pide es leer un fichero dentro de este bloque: el HBA
/// no sabe lo que es una direccion virtual, asi que sin este dato la unica
/// forma de escribir aqui es rebotar por una pagina del kernel y copiar.
///
/// La alternativa era **preguntarle a las tablas de pagina** donde vive el
/// bloque. `dev/disk.rs` ya recorrio ese camino y volvio: una lectura que sale
/// bien o mal segun donde pongas el destino no tiene el fallo en el disco, lo
/// tiene en la traduccion. Aqui no hay nada que traducir -- `alloc_frames_contig`
/// devolvio esta direccion hace tres lineas, y los marcos son **contiguos por
/// construccion**, que es justo lo que un PRDT de una entrada necesita.
#[derive(Clone, Copy)]
struct Bloque {
    /// La VA con la que se le entrego al proceso. `0` = ranura sin usar.
    base: u64,
    /// El primer marco fisico. Los `bytes` siguientes van seguidos.
    fisica: u64,
    bytes: u64,
    /// **La secuencia del bloque: cuantas veces VOLVIO un prestamo suyo.**
    ///
    /// Es lo que `WAIT` compara (`PLAN_LA_VIDA_UTIL` 7): `soltar` la devuelve
    /// cuando dice que no, y el dueno duerme hasta que se mueva. Sube en
    /// `loan.rs` cuando el prestatario suelta o muere. Nunca baja.
    devueltas: u64,
}

const SIN_BLOQUE: Bloque = Bloque { base: 0, fisica: 0, bytes: 0, devueltas: 0 };

/// La contabilidad de un proceso que tiene memoria pedida.
#[derive(Clone, Copy)]
struct Count {
    /// `0` = ranura libre. El pid 0 no existe: `NEXT_PID` empieza en 1.
    pid: u32,
    /// Donde va el proximo bloque. Empieza en [`vmm::MEMORIA_VA_BASE`] y
    /// avanza; asi dos peticiones del mismo proceso no se pisan.
    cursor: u64,
    peticiones: usize,
    /// Bytes entregados. Para el panel: la memoria que un proceso pidio es la
    /// unica que el kernel no puede deducir mirando su imagen.
    entregados: u64,
    /// Los bloques vivos de este proceso. Son [`MAX_PETICIONES`] como mucho
    /// porque ese es el tope de peticiones, asi que la tabla no puede
    /// desbordarse por definicion.
    bloques: [Bloque; MAX_PETICIONES],
}

const FREE_SLOT: Count = Count {
    pid: 0,
    cursor: 0,
    peticiones: 0,
    entregados: 0,
    bloques: [SIN_BLOQUE; MAX_PETICIONES],
};

static mut CUENTAS: [Count; MAX_PROCS] = [FREE_SLOT; MAX_PROCS];

/// Total entregado desde el arranque, para `info`. **No baja al morir un
/// proceso**, y es a proposito: es "cuanta memoria ha pedido Ring 3 en esta
/// sesion", no "cuanta hay pedida ahora". Un contador historico dicho como tal
/// no engana a nadie; el problema es usarlo como indice.
static mut TOTAL: u64 = 0;

/// **De que proceso es este marco fisico.** `(pid, desplazamiento)`.
///
/// # Por que existe, y por que la pantalla de fallo la necesitaba
///
/// La azul sabia decir `marco OCUPADO` --el asignador cree que esta
/// entregado-- y ahi se paraba. Y `OCUPADO` significa **"se entrego dos
/// veces"**, que solo es accionable si se sabe la otra punta: sin el nombre,
/// el veredicto manda a mirar el arbol entero.
///
/// ** Es el mismo callejon del que ya salio `de NADIE VIVO` cuando gano la
/// morgue: *"decia que la pila no tiene duena y no decia quien la solto"*.
/// Aquello se arreglo mirando una tabla que ya existia. Esto tambien.
///
/// [!] SIN CERROJO, y decidido igual que `titular_de_pila`: lo llama la pantalla
/// de fallo con la maquina ya rota. Colgarse aqui cambia un volcado legible por
/// un silencio, y un dato de hace un tick sirve para un diagnostico.
///
/// [!] Y solo mira los bloques de `KIND_MEMORIA`. Un marco puede estar
/// entregado y no ser de aqui --una pila de kernel, un buffer de fichero, la
/// imagen de un proceso-- asi que `None` **no** significa "de nadie": significa
/// "de nadie DE AQUI". La pantalla pregunta a las tres tablas por turno.
pub fn titular_de_fisica(fisica: u64) -> Option<(u32, u64)> {
    let cuentas = unsafe { &*core::ptr::addr_of!(CUENTAS) };
    for c in cuentas.iter() {
        if c.pid == 0 {
            continue;
        }
        for b in c.bloques.iter() {
            if b.base == 0 || b.bytes == 0 {
                continue;
            }
            if fisica >= b.fisica && fisica < b.fisica + b.bytes {
                return Some((c.pid, fisica - b.fisica));
            }
        }
    }
    None
}


/// La ranura de este proceso, si tiene una.
/// **Cuantos bloques NACIERON con agujeros desde el arranque.**
///
/// *** POR QUE UN CONTADOR Y NO SOLO LA LINEA DE CABINA (2026-09-20)
///
/// La comprobacion de `request` avisa por CABINA, y CABINA **no llega al
/// KERNEL LOG** -- el panel que se fotografia. Lo escribi el 20-09 dando por
/// hecho que si, y no: CABINA va al anillo de eventos (`cabina`, F11, `save`).
/// Y llamar a `dashboard_log` desde aqui no vale: esto corre dentro de un
/// syscall con la pantalla cedida a Ring 3, y `dashboard.rs` avisa con todas
/// las letras de que un llamante de fondo **pinta encima de la app**.
///
/// Asi que el dato viaja hasta donde SI se pinta sin pisar a nadie: la
/// autopsia de Ring 3 lo lee y lo escribe en su propio renglon. Y con un solo
/// numero parte el caso: **cero significa que ningun bloque nacio roto, o sea
/// que la pagina se perdio DESPUES.**
static NACIERON_ROTOS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Lo lee la autopsia. Ver [`NACIERON_ROTOS`].
pub fn nacieron_rotos() -> u64 {
    NACIERON_ROTOS.load(core::sync::atomic::Ordering::Relaxed)
}

/// **DONDE CAE UNA DIRECCION RESPECTO DE LO QUE ESTE PID TIENE ENTREGADO.**
///
/// *** POR QUE EXISTE, Y ES UNA MENTIRA MENOS (2026-09-20)
///
/// La autopsia de Ring 3 contestaba a un `#PF` sin resolver asi:
///
/// ```text
///    *** SIN MAPEAR: puntero basura o indice fuera de rango
/// ```
///
/// ** Eso es una **O**, y las dos ramas mandan a sitios opuestos. *"Puntero
/// basura"* acusa al programa. *"Indice fuera de rango"* acusa al programa. Y
/// falta la tercera, que no acusa al programa en absoluto: **la direccion cae
/// DENTRO de un bloque que el kernel le entrego, y la pagina no esta**. Eso no
/// es un fallo de quien escribe: es una pagina que alguien le quito.
///
/// El 20-09 el escritorio murio escribiendo en `0xE0368948`, que con el modo de
/// ese panel cae a mitad de su doble bufer -- y la pantalla lo llamo puntero
/// basura. Esta tabla sabia la verdad y **nadie le preguntaba**.
///
/// [!] Se pregunta ANTES de `revoke_all`, y no es un detalle de orden: la
/// estacion 10 de ese desmontaje llama a [`process_died`], que pone la ranura
/// a `FREE_SLOT`. Quien pregunte despues recibe `SinCuenta` y no se entera de
/// que llega tarde. Por eso la captura de la autopsia es lo PRIMERO que corre.
#[derive(Clone, Copy, PartialEq)]
pub enum Caida {
    /// Este pid no tiene contabilidad: o no pidio nada, o ya se la cerraron.
    SinCuenta,
    /// Cae DENTRO del bloque `bloque`: `off` bytes de los `bytes` que mide.
    Dentro { bloque: usize, off: u64, bytes: u64 },
    /// Cae PASADO el final del bloque `bloque`, por `cuanto` bytes. Se da por
    /// suyo lo que cae a menos de un bloque de distancia por arriba: mas lejos
    /// no se puede decir de quien se paso.
    Pasado { bloque: usize, cuanto: u64 },
    /// Tiene bloques y no cae cerca de ninguno.
    Fuera,
}

pub fn donde_cae(pid: u32, va: u64) -> Caida {
    let Some(slot) = slot(pid) else { return Caida::SinCuenta };
    let bloques = unsafe { (*core::ptr::addr_of!(CUENTAS))[slot].bloques };
    // Primero DENTRO, y en una pasada aparte: un bloque puede quedar justo
    // detras de otro --el cursor los pone seguidos-- y "dentro del segundo"
    // gana siempre a "pasado del primero". Mezclar las dos preguntas en un
    // solo bucle haria que el orden de la tabla decidiera el veredicto.
    for (i, b) in bloques.iter().enumerate() {
        if b.base != 0 && b.bytes != 0 && va >= b.base && va < b.base + b.bytes {
            return Caida::Dentro { bloque: i, off: va - b.base, bytes: b.bytes };
        }
    }
    let mut hay = false;
    for (i, b) in bloques.iter().enumerate() {
        if b.base == 0 || b.bytes == 0 {
            continue;
        }
        hay = true;
        let fin = b.base + b.bytes;
        if va >= fin && va - fin < b.bytes {
            return Caida::Pasado { bloque: i, cuanto: va - fin };
        }
    }
    if hay {
        Caida::Fuera
    } else {
        Caida::SinCuenta
    }
}

fn slot(pid: u32) -> Option<usize> {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().position(|c| c.pid == pid && pid != 0)
    }
}

/// La ranura de este proceso, tomando una libre si aun no tiene.
fn slot_or_new(pid: u32) -> Option<usize> {
    if pid == 0 {
        return None;
    }
    if let Some(i) = slot(pid) {
        return Some(i);
    }
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(CUENTAS);
        let i = t.iter().position(|c| c.pid == 0)?;
        t[i] = Count { pid, ..FREE_SLOT };
        Some(i)
    }
}

pub fn handed_over_by(pid: u32) -> u64 {
    match slot(pid) {
        Some(i) => unsafe { (*core::ptr::addr_of!(CUENTAS))[i].entregados },
        None => 0,
    }
}

/// Cuantos procesos tienen memoria pedida ahora mismo. Para el panel y para
/// que "no queda ranura" se pueda distinguir de "has pedido demasiadas veces".
pub fn processes_with_memory() -> usize {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().filter(|c| c.pid != 0).count()
    }
}

pub fn total_handed_over() -> u64 {
    unsafe { TOTAL }
}

/// **La ranura `n` de las ocupadas: `(pid, bytes, peticiones)`.**
///
/// `None` cuando ya no hay mas. `n` cuenta **solo las ocupadas**, no los huecos:
/// quien enumera pide 0, 1, 2... y para cuando le contestan `None`, sin tener
/// que saber que la tabla tiene agujeros dentro.
///
/// # Por que hacia falta, y es lo que pidio el dueno
///
/// Los datos ya estaban: `handed_over_by(pid)` contesta desde julio. Lo que no
/// habia era forma de preguntarlos **sin saber el pid de antemano** -- o sea que
/// se podia contestar *"cuanto come el proceso 4"* y no *"quien esta comiendo"*.
///
/// Y esa segunda es la que hace falta para una vista tipo administrador de
/// tareas: la gracia no es mirar a un sospechoso, es **descubrir cual lo es**.
///
/// * Se enumera aqui y no en Ring 3 con una lista de pids porque la tabla es del
/// kernel y cambia sola: un proceso puede morir entre la pregunta y la
/// respuesta. Contestando por indice, lo peor que pasa es que una fila salga
/// vacia un fotograma.
pub fn ranura(n: usize) -> Option<(u32, u64, usize)> {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter()
            .filter(|c| c.pid != 0)
            .nth(n)
            .map(|c| (c.pid, c.entregados, c.peticiones))
    }
}

/// **Pide `bytes` de memoria.** Devuelve el handle de la capability; la
/// direccion se pregunta despues con `MEM_OP_BASE`.
///
/// `aspace` es el espacio de direcciones del llamante -- durante un syscall
/// desde Ring 3, CR3 **sigue siendo el suyo**: el cambio solo ocurre en un
/// cambio de contexto, y aqui todavia no ha habido ninguno. Es la misma nota
/// que lleva el framebuffer, y por el mismo motivo.
pub fn request(pid: u32, aspace: u64, bytes: u64) -> Result<u64, u32> {
    if bytes == 0 || bytes > MAX_BYTES {
        return Err(ERROR_TOO_BIG);
    }
    // La ranura se toma AQUI, despues de validar el tamano: una peticion
    // absurda no debe gastar una ranura de la tabla. Sin ranura libre el motivo
    // es otro y se dice con su nombre -- antes esto contestaba
    // `ERROR_TOO_MANY` a un proceso que no habia pedido nunca nada.
    let slot = match slot_or_new(pid) {
        Some(s) => s,
        None => {
            crate::ring0::cabina::warn(
                "mem",
                "no queda ranura de contabilidad para otro proceso",
                MAX_PROCS as u64,
            );
            return Err(ERROR_NO_SLOT);
        }
    };
    unsafe {
        if (*core::ptr::addr_of!(CUENTAS))[slot].peticiones >= MAX_PETICIONES {
            // *** EL UNICO NO DE ESTA FUNCION QUE NO SE DECIA (2026-09-20).
            //
            // Los otros dos --sin ranura, sin RAM contigua-- gritan en CABINA.
            // Este no, y **es el que se cobra de verdad**: la cabecera de
            // arriba promete que un programa que pide de mas falla "pronto y
            // DICIENDOLO", y hasta hoy fallaba pronto y callado.
            //
            // ** Lo que se ve cuando calla: el visor del escritorio pide TRES
            // bloques para una imagen, se queda sin cupo, y la pantalla dice
            // *"vacio, o no se pudo leer"*. Eso se lee como **el kernel no sabe
            // leer el fichero**, que es el sitio equivocado entero: el fichero
            // se lee perfectamente y lo que falta es una RANURA. Un limite que
            // no se nombra se disfraza del subsistema de al lado.
            //
            // [!] Va con los DOS numeros --lo que ya lleva y el tope-- porque
            // "te pasaste" sin decir de cuanto no dice si el programa pide uno
            // de mas o cien.
            crate::ring0::cabina::count(
                "mem",
                "SIN CUPO: este proceso ya gasto sus peticiones (tope 4)",
                (*core::ptr::addr_of!(CUENTAS))[slot].peticiones as u64,
            );
            return Err(ERROR_TOO_MANY);
        }
    }

    let paginas = (bytes + mm::PAGE - 1) / mm::PAGE;
    // Contiguo: ver la cabecera. Un bloque que el programa recorre como un
    // array no puede llegarle a trozos.
    let fisica = match mm::phys::alloc_frames_contig(paginas) {
        Some(f) => f,
        None => {
            crate::ring0::cabina::warn("mem", "sin RAM contigua para la peticion", bytes);
            return Err(ERROR_NO_RAM);
        }
    };

    let base = unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        if c.cursor == 0 {
            c.cursor = vmm::MEMORIA_VA_BASE;
        }
        c.cursor
    };
    // *** EL CURSOR NO VUELVE, NI SIQUIERA AL SOLTAR, y por eso hay techo.
    //
    // Soltar devuelve los MARCOS y la RANURA; la direccion no se reusa a
    // proposito: una VA que vuelve es una VA que un handle viejo podria volver
    // a resolver, y ese es justo el fallo que `loan::OP_SOLTAR` documenta y que
    // la generacion de la capability existe para impedir. Sale mas barato
    // gastar VA --hay 512 MiB aqui-- que razonar sobre alias.
    //
    // ** Pero gastar sin techo es caminar hacia la region de los PRESTAMOS
    // (`0x1_0000_0000`), y entonces un bloque nuevo se mapearia encima de lo
    // que otro proceso te presto **sin que nada fallara**. Con techo, el que
    // se pasa recibe un no con su motivo.
    if base < vmm::MEMORIA_VA_BASE
        || base.saturating_add(paginas * mm::PAGE) > MEMORIA_VA_TOPE
    {
        crate::ring0::cabina::warn(
            "mem", "SIN SITIO: esta sesion agoto los 512 MiB de VA de bloques", base);
        return Err(ERROR_NO_RAM);
    }

    let mut off = 0u64;
    while off < paginas * mm::PAGE {
        if vmm::map_page(aspace, base + off, fisica + off, true, true).is_err() {
            // Mapeo a medias = paginas sueltas en el espacio del usuario. Se
            // deshace lo hecho: quedarse con la mitad mapeada y sin handle es
            // peor que no tener nada. Mismo criterio que el framebuffer.
            let mut undo = 0u64;
            while undo < off {
                vmm::unmap_page(aspace, base + undo);
                undo += mm::PAGE;
            }
            return Err(ERROR_NO_RAM);
        }
        off += mm::PAGE;
    }

    // *** COMPROBAR LO QUE SE ACABA DE PROMETER (2026-09-20).
    //
    // ** `map_page` es todo-o-nada: o mapea la pagina o devuelve `Err`, y el
    // bucle de arriba deshace y se rinde. O sea que si este bucle LLEGO al
    // final, **las 2.025 paginas de un doble bufer estan mapeadas**. Eso es lo
    // que dice el codigo.
    //
    // Y el 20-09 el Ryzen dijo otra cosa: el escritorio murio escribiendo en
    // `0xE0368948`, que la contabilidad da por DENTRO de su bloque y la MMU da
    // por ausente. Las dos no pueden tener razon.
    //
    // *** ESTO PARTE EL CASO EN DOS, Y ES LA PARTICION ENTERA:
    //
    //    grita AQUI    -> el agujero nace con el bloque, y el fallo esta en
    //                     este fichero, en `map_page` o en el asignador
    //    NO grita      -> el bloque nacio entero y alguien le quito paginas
    //                     DESPUES, y entonces hay que buscar quien desmapea
    //
    // Sin esta linea las dos hipotesis se ven igual desde la pantalla azul, y
    // llevan a ficheros distintos.
    //
    // [!] Cuesta un paseo de cuatro niveles por pagina, UNA vez, cuando alguien
    // pide memoria. Un doble bufer son 2.025 paseos en el arranque; nadie pide
    // memoria en el bucle del escritorio. No corre en reposo.
    //
    // ** Y NO se rinde: el bloque ya esta entregado y quitarselo ahora seria
    // cambiar un hallazgo por una negativa. Se ACUSA y se sigue -- la misma
    // conducta que `caminable` en el otro extremo del kernel.
    {
        let mut faltan = 0u64;
        let mut primera = 0u64;
        let mut o = 0u64;
        while o < paginas * mm::PAGE {
            if vmm::translate(aspace, base + o).is_none() {
                if faltan == 0 {
                    primera = base + o;
                }
                faltan += 1;
            }
            o += mm::PAGE;
        }
        if faltan != 0 {
            crate::ring0::cabina::fault(
                "mem", "EL BLOQUE NACE CON AGUJEROS: paginas sin traduccion", faltan);
            crate::ring0::cabina::addr("mem", "la primera que falta", primera);
            NACIERON_ROTOS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
    }

    // ** RIGHT_WAIT (2026-09-21): un bloque es ESPERABLE. Su secuencia es
    // `devueltas`, y el brazo esta en `syscall/mod.rs::wait` -- el guardian
    // `esperable.py` exige que las dos mitades esten.
    let handle = match cap::grant(
        pid,
        cap::KIND_MEMORIA,
        cap::RIGHT_READ | cap::RIGHT_WRITE | cap::RIGHT_WAIT,
        base,
    ) {
        Some(h) => h,
        None => {
            let mut undo = 0u64;
            while undo < paginas * mm::PAGE {
                vmm::unmap_page(aspace, base + undo);
                undo += mm::PAGE;
            }
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };

    unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        c.cursor = base + paginas * mm::PAGE;
        // El bloque se apunta ANTES de subir el contador de peticiones, que es
        // lo que hace que el indice sea siempre uno libre.
        // *** POR RANURA LIBRE, Y NO POR EL CONTADOR (2026-09-20).
        //
        // Esto era `c.bloques[c.peticiones]`, que vale mientras el contador y
        // el numero de ranuras ocupadas sean el mismo numero -- o sea mientras
        // NO se pueda devolver. Con `MEM_OP_SOLTAR` dejan de serlo: se suelta
        // el bloque 1 de tres, el contador baja a 2, y la siguiente peticion
        // escribiria encima del bloque 2 **que sigue vivo**. El proceso se
        // quedaria con un bloque mapeado que la contabilidad ya no conoce: no
        // lo liberaria nadie al morir, y `donde_cae` diria "fuera" de algo que
        // es suyo. Se busca hueco, que es lo unico que sobrevive a soltar.
        if let Some(i) = c.bloques.iter().position(|b| b.base == 0) {
            c.bloques[i] = Bloque { base, fisica, bytes: paginas * mm::PAGE, devueltas: 0 };
        }
        c.peticiones += 1;
        c.entregados += paginas * mm::PAGE;
        TOTAL += paginas * mm::PAGE;
    }
    crate::ring0::cabina::info("mem", "bloque entregado a Ring 3", paginas * mm::PAGE);
    Ok(handle)
}

/// El proceso murio: **se devuelven sus marcos y su ranura queda libre.**
///
/// === La fuga, y como se encontro ===
///
/// Esta funcion soltaba la RANURA y nada mas, con un comentario que decia *"no
/// se desmapea nada -- el espacio de direcciones entero se destruye con el
/// proceso"*. **Esa frase describia una intencion que no implementaba ningun
/// codigo**: `vmm::destroy_address_space` solo se llama desde `vmm::self_test`,
/// y `scheduler::reap` libera unicamente la pila de kernel. O sea que los
/// marcos entregados a Ring 3 **no volvian jamas**.
///
/// Para DOOM eso son 12 MiB por cada vez que se lanza. Lo vio el dueno mirando
/// `mem` en el Ryzen el 2026-08-14 --*"vi que DOOM.bex esta comiendo RAM"*-- y
/// no lo vio ningun contador nuestro, porque **el que dice `fugas 0` cuenta
/// CAPABILITIES, no marcos**. La autopsia decia `recursos todo devuelto` y era
/// verdad: de los handles. La RAM no la miraba nadie.
///
/// === Por que se liberan SOLO estos bloques ===
///
/// La tentacion es recorrer las tablas de paginas y soltar todas las hojas.
/// Seria peor que la fuga: ahi dentro estan **el framebuffer** (que es MMIO, y
/// devolverlo al asignador de RAM es corrupcion) y **los marcos prestados**, que
/// por diseno sobreviven al que los presto. Lo que si es inequivocamente
/// nuestro es esto: bloques que salieron de `alloc_frames_contig` tres lineas
/// mas arriba, con su fisica apuntada, y de un solo dueno. El resto --imagen,
/// pila de usuario, tablas de paginas-- sigue sin devolverse y **eso es deuda
/// declarada**, no un descuido.
///
/// === Y SE PONEN A CERO, que no es opcional ===
///
/// `alloc_frames_contig` **no limpia**. Mientras los marcos no se reutilizaban
/// eso no tenia consecuencias; en cuanto se reutilizan, el siguiente programa
/// que pida memoria recibe **lo que habia dentro del anterior**. La fuga tapaba
/// un agujero de confidencialidad, asi que arreglar una sin la otra habria
/// cambiado un desperdicio por una filtracion. Se limpia aqui --al soltar, que
/// es cuando se sabe de quien era-- y no al pedir.
///
/// [!] Se libera sin desmapear, y se puede: esto corre dentro de
/// `cap::revoke_all`, o sea en el borde del syscall de un proceso que **no
/// vuelve a ejecutar ni una instruccion**. No hay reprogramacion entre el
/// `free_frame` y el cambio de contexto, asi que nadie puede tomar el marco
/// mientras su mapeo muerto sigue en pie.
///
/// [!] Y aqui **NO hace falta la danza de CR3** que si necesita
/// `fb::process_died`, aunque las dos corran en el mismo sitio. Aquella pinta
/// en el framebuffer, que vive a ~3,5 GiB en la mitad BAJA y no esta mapeado
/// bajo el CR3 del moribundo. `zero_frame` escribe por `phys_to_virt`, que es
/// `phys + HIGH_MEM_BASE`: el physmap vive en la mitad ALTA (indices 256..512
/// del PML4) y `vmm::new_address_space` la copia entera en **todo** espacio de
/// direcciones -- por eso los syscalls funcionan bajo el CR3 del usuario.
/// Comprobado antes de escribir esto, no supuesto: es la mina que ya costo dos
/// sesiones y no se pisa dos veces.
pub fn process_died(pid: u32) {
    let Some(slot) = slot(pid) else { return };
    let mut devueltos = 0u64;
    let mut retenidos = 0u64;
    unsafe {
        let bloques = (*core::ptr::addr_of!(CUENTAS))[slot].bloques;
        for b in bloques.iter() {
            if b.base == 0 || b.bytes == 0 {
                continue;
            }
            // ** LA PREGUNTA QUE EVITA LLEVARSE EL ESCRITORIO POR DELANTE.
            if crate::ring0::obj::loan::hay_prestado_en(pid, b.base, b.bytes) {
                retenidos += b.bytes;
                continue;
            }
            let paginas = b.bytes / mm::PAGE;
            for p in 0..paginas {
                let marco = b.fisica + p * mm::PAGE;
                mm::phys::zero_frame(marco);
                mm::phys::free_frame(marco);
            }
            devueltos += b.bytes;
        }
        (*core::ptr::addr_of_mut!(CUENTAS))[slot] = FREE_SLOT;
    }
    if devueltos > 0 {
        // ** DECIA "marcos" Y CONTABA BYTES, Y ADEMAS EN HEXADECIMAL.
        //
        // En el Ryzen del 25-08 salio `marcos devueltos al morir el pid
        // =870000`. Ochocientos setenta mil marcos serian 3,4 GiB; son
        // 0x870000 BYTES, o sea 8,4 MiB, que es lo que gasta la calculadora.
        // La palabra decia una unidad y el numero traia otra, en una base que
        // no se anuncia. `bytes` es el mismo dato dicho como lo que es.
        crate::ring0::cabina::bytes("mem", "memoria devuelta al morir el pid", devueltos);
    }
    // ** Y si algo NO se pudo devolver, se dice. Un bloque retenido es correcto
    // --lo sostiene el que lo tomo prestado-- pero es RAM que sigue fuera, y una
    // retencion que nadie suelta se ve como una fuga tres arranques despues.
    if retenidos > 0 {
        crate::ring0::cabina::warn("mem", "no devuelto: sigue PRESTADO a otro", retenidos);
    }
}

/// **Donde esta DE VERDAD el rango `[va, va + len)` de un bloque de `pid`.**
///
/// `None` si ese rango no cae entero dentro de un solo bloque entregado a ese
/// proceso -- y entonces quien pregunta hace lo de siempre, que es correcto y
/// mas lento. Nunca se contesta "casi": media respuesta aqui es el HBA
/// escribiendo en memoria de otro.
///
/// Lo usa `syscall.rs` para que **el disco escriba dentro del bloque del
/// programa sin escala**: es el escalon 3 de `docs/identidad/LA_RAM.md` aplicado a leer
/// ficheros, y la razon por la que [`Bloque`] guarda la fisica.
/// **Cuantos bytes mide ESTE bloque**, el que empieza en `base`. `None` si ese
/// proceso no tiene ninguno que empiece ahi.
///
/// # *** POR QUE EXISTE, Y ES UN FALLO REAL QUE ESTABA VIVO (2026-08-24)
///
/// El limite de `ARCH_OP_LEER_EN` y `ARCH_OP_ESCRIBIR_DE` se comprobaba contra
/// [`handed_over_by`], que es **la suma de todos los bloques del proceso**:
///
/// ```text
///    bloque A   4 KiB      entregados = 8192
///    bloque B   4 KiB
///
///    leer_en(handle_de_A, desde=0, cuantos=8192)   ->  8192 <= 8192, PASA
///    y el kernel escribe 8 KiB empezando en la base de A
/// ```
///
/// ** Cuatro mil noventa y seis bytes fuera del bloque que el handle autoriza.
/// El camino rapido no se traga esto --[`fisica_de`] valida bloque a bloque--
/// pero el de respaldo escribe en la VA del proceso, y ahi no habia limite.
///
/// [!] Y lo peor no es que el proceso se corrompa a si mismo: si el rango cae en
/// VA **sin mapear**, quien falla es el KERNEL, porque durante una syscall el
/// kernel escribe con las tablas del llamante. Una app sin privilegios podia
/// tumbar la maquina con dos numeros.
///
/// *** Es exactamente lo que la doctrina de `syscall/ops.rs` queria impedir --
/// *"aqui no hay `copy_from_user`"*-- y se colo igual, no por un puntero sino
/// por una **pareja desplazamiento+largo** comprobada contra el total
/// equivocado. Un limite es tan bueno como el numero con el que se compara.
pub fn bytes_de_bloque(pid: u32, base: u64) -> Option<u64> {
    let slot = slot(pid)?;
    unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        for b in c.bloques.iter() {
            if b.base != 0 && b.base == base {
                return Some(b.bytes);
            }
        }
    }
    None
}

pub fn fisica_de(pid: u32, va: u64, len: u64) -> Option<u64> {
    let slot = slot(pid)?;
    let fin = va.checked_add(len)?;
    unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        for b in c.bloques.iter() {
            if b.base != 0 && va >= b.base && fin <= b.base + b.bytes {
                return Some(b.fisica + (va - b.base));
            }
        }
    }
    None
}

/// **DEVOLVER UN BLOQUE: los marcos, la ranura y el handle.**
///
/// `Some(1)` devuelto, `Some(0)` no se pudo y el motivo esta en CABINA,
/// `None` ese bloque no es de este proceso.
///
/// # *** EL ORDEN DE LOS TRES PASOS, QUE NO ES LIBRE
///
/// ```text
///    1  DESMAPEAR   quitarle las PTE al proceso
///    2  liberar     devolver los marcos al asignador
///    3  revocar     invalidar el handle
/// ```
///
/// ** Uno antes que dos, y a vida o muerte. Al reves, el marco vuelve al
/// asignador **mientras el proceso todavia lo tiene mapeado**: se lo entregan a
/// otro, los dos escriben encima del mismo sitio y nada falla hasta tres
/// arranques despues. Es exactamente la familia que `caminable` y
/// `esta_libre` existen para cazar en el otro extremo del kernel.
///
/// ** Y tres, porque un handle que sobrevive a su bloque es lo que
/// `loan::OP_SOLTAR` ya documenta: la generacion de la capability esta para
/// esto y basta con dejarla trabajar.
///
/// # Lo unico que puede decir que NO
///
/// Que el bloque siga **prestado** a otro proceso. `memory::process_died` ya
/// hace esa misma pregunta al morir --y por el mismo motivo-- asi que aqui no
/// hay una politica nueva: hay la misma, aplicada tambien al caso vivo.
///
/// [!] `TOTAL` no baja, y es a proposito: significa *"cuanta memoria ha pedido
/// Ring 3 en esta sesion"*, no cuanta tiene. `entregados` SI baja, porque ese
/// es el numero que contesta `MEM_OP_BYTES` y ahi la pregunta es *"cuanta
/// tengo"*.
///
/// [!] Se desmapea en `read_cr3()`. Durante un syscall desde Ring 3 el CR3
/// sigue siendo el del llamante, que es de quien es este bloque. Es la misma
/// nota que llevan `memory::request` y `loan::OP_SOLTAR`.
fn soltar(pid: u32, base: u64) -> Option<u64> {
    let slot = slot(pid)?;
    let (i, b) = unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        let i = c.bloques.iter().position(|x| x.base == base && x.bytes != 0)?;
        (i, c.bloques[i])
    };
    if crate::ring0::obj::loan::hay_prestado_en(pid, b.base, b.bytes) {
        crate::ring0::cabina::warn(
            "mem", "NO se suelta: ese bloque sigue PRESTADO a otro", b.base);
        // *** EL NO TRAE LA SECUENCIA QUE VIO (2026-09-21, PLAN_LA_VIDA_UTIL 7).
        //
        // Un 0 a secas dejaba al dueno sin paso siguiente: reintentar cuando?
        // Girar? Ahora contesta `devueltas << 1` (par, nunca 1): la secuencia
        // del bloque EN ESTE INSTANTE. El bucle correcto en Ring 3 es
        //
        //    soltar -> par  -> WAIT(bloque, ese par >> 1) -> soltar -> 1
        //
        // y `wait_current_checked` compara bajo el cerrojo del planificador:
        // si el prestatario solto entre este renglon y el WAIT, la secuencia
        // ya no es la vista y el WAIT vuelve en el acto. WAIT dice CUANDO
        // volver a preguntar; quien dice SI sigue siendo `hay_prestado_en`.
        return Some(b.devueltas << 1);
    }
    let aspace = vmm::read_cr3();
    let paginas = b.bytes / mm::PAGE;
    // 1: quitarselo de delante ANTES de que vuelva al asignador.
    for p in 0..paginas {
        vmm::unmap_page(aspace, b.base + p * mm::PAGE);
    }
    // 2: y limpiarlos al devolverlos, por lo mismo que `process_died`: el
    // asignador no limpia al entregar, asi que el siguiente leeria lo de este.
    for p in 0..paginas {
        let marco = b.fisica + p * mm::PAGE;
        mm::phys::zero_frame(marco);
        mm::phys::free_frame(marco);
    }
    unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        c.bloques[i] = SIN_BLOQUE;
        c.peticiones = c.peticiones.saturating_sub(1);
        c.entregados = c.entregados.saturating_sub(b.bytes);
    }
    // 3.
    if let Some(h) = cap::find(pid, cap::KIND_MEMORIA, base) {
        cap::revoke(pid, h);
    }
    crate::ring0::cabina::bytes("mem", "bloque DEVUELTO por su dueno", b.bytes);
    Some(1)
}

/// Las operaciones sobre el handle. `base` es la VA con la que se concedio.
/// **La llave de espera de un bloque**: unica por `(pid, base)`, en un
/// espacio que no pisa a las otras llaves (`latido::LLAVE`, `puerta::LLAVE`,
/// las paginas fisicas de los canales). El pid cabe en 16 bits (MAX_TASKS) y
/// la base en pagina en 20: el resto es la marca `MEM`.
pub fn llave_de(pid: u32, base: u64) -> u64 {
    0x4D45_4D00_0000_0000 | ((pid as u64 & 0xFFFF) << 24) | ((base >> 12) & 0xF_FFFF)
}

/// **La secuencia de un bloque**: cuantas veces volvio un prestamo suyo.
/// `0` si no hay tal bloque -- y WAIT sobre un bloque que no existe no
/// bloquea: la `cap` ya no resolveria.
pub fn secuencia_de(pid: u32, base: u64) -> u64 {
    let Some(slot) = slot(pid) else { return 0 };
    unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        for b in c.bloques.iter() {
            if b.base != 0 && b.base == base {
                return b.devueltas;
            }
        }
    }
    0
}

/// **Un prestamo salido de `origen` VOLVIO.** Lo llama `loan.rs` cuando el
/// prestatario suelta o muere: sube la secuencia del bloque del dueno y
/// despierta a quien la este esperando. Si el dueno ya no tiene cuenta (murio
/// antes: prestamo huerfano), no hay a quien avisar y no pasa nada.
pub fn devuelto(owner: u32, origen: u64) {
    let Some(slot) = slot(owner) else { return };
    let mut llave = 0u64;
    unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        for b in c.bloques.iter_mut() {
            if b.base != 0 && origen >= b.base && origen < b.base + b.bytes {
                b.devueltas = b.devueltas.wrapping_add(1);
                llave = llave_de(owner, b.base);
                break;
            }
        }
    }
    if llave != 0 {
        crate::ring0::task::scheduler::wake_by_key(llave);
    }
}

pub fn operation(base: u64, operation: u64, pid: u32) -> Option<u64> {
    match operation {
        MEM_OP_BASE => Some(base),
        MEM_OP_BYTES => Some(handed_over_by(pid)),
        // ** `None` y no `Some(0)` si no se encuentra el bloque: cero es una
        // fisica valida --el primer marco de la maquina-- asi que devolverlo
        // como "no lo se" seria mandar a un driver a programar un DMA contra el
        // vector de interrupciones. `None` sale por la puerta como "esa
        // operacion no existe aqui", que es lo que de verdad pasa.
        // ** SE REUSA `fisica_de`, que ya existia para el prestamo del audio.
        //
        // Y trae de regalo lo unico que hacia falta y no se ve: comprueba que
        // `[va, va+len)` **cae dentro del bloque**. Una version escrita a mano
        // aqui habria devuelto la fisica del primer marco sin mirar el largo, y
        // eso es correcto para una pagina y falso para un anillo de DMA.
        //
        // `len = 1` porque lo que se pregunta es "donde empieza": el largo del
        // bloque ya lo contesta `MEM_OP_BYTES`, y pedir dos numeros por dos
        // caminos distintos es como se acaban desacoplando.
        MEM_OP_FISICA => fisica_de(pid, base, 1),
        MEM_OP_SOLTAR => soltar(pid, base),
        _ => None,
    }
}
