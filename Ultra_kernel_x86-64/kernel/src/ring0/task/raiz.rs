//! **LA RAIZ DE CADA PROCESO** -- una carpeta como capacidad. H3 de
//! `docs/plan/PLAN_HERMES.md`.
//!
//! [carril]  ROJO      lo que un proceso puede nombrar del disco
//! [consumo] NADA      corre cuando se lanza, se toma una ruta o muere un proceso
//!
//! # Lo que habia, y por que no bastaba
//!
//! Hasta el 03-10 cualquier proceso abria cualquier ruta: `sys/director.bex`, el
//! disco `d:` entero, lo que escribiera. La autoridad (`autoridad.rs`) decia
//! si un proceso podia REINICIAR o LANZAR, pero el disco no tenia propietario. Para
//! la app de F3 eso no vale: lee lo que manda otra maquina, y si un fichero
//! hecho a mala idea la toma, lo que toma tiene que ser su carpeta.
//!
//! # Como se cierra, en tres sitios y ninguno mas
//!
//! ```text
//!    al NACER    `nacer(pid)`, llamado por `proc.rs` justo despues de dar el
//!                pid y ANTES de crear la tarea: no hay un instante en que el
//!                proceso exista sin su raiz
//!    al NOMBRAR  `resolver`, desde `ruta_tomar`, que es el UNICO sitio por
//!                donde una ruta de Ring 3 entra al kernel. La ruta se escribe
//!                DENTRO de la raiz (`bmo-raiz-juicio`): no se compara, se
//!                construye, y ningun tramo puede subir
//!    lo que no   el cursor de ESTRATOS, sellar, el disco y las versiones no
//!    lleva ruta  llevan ruta y ven el volumen entero: un proceso encerrado
//!                recibe NO (`encerrado`, en `syscall/mod.rs` y `gesto.rs`)
//! ```
//!
//! # De donde sale la raiz de un hijo
//!
//! La pone QUIEN LANZA, como la consola: `TASK_OP_RAIZ_HIJO` dice "el
//! siguiente hijo que lance vive en esta carpeta", y la carpeta se lee DENTRO
//! de la raiz del que lanza. Un hijo de un proceso encerrado nace, como poco,
//! igual de encerrado: si su padre no pide nada, hereda la raiz entera.
//!
//! ** No es una capability y por la misma razon que la autoridad: un handle
//! se pasa, y esto no se puede pasar. La raiz se da al nacer, no se ensancha
//! nunca, y se olvida al morir (`olvidar`, desde `cap::revoke_all`, junto a la
//! autoridad: un pid reutilizado no hereda la carpeta del muerto).
//!
//! [!] Lo que NO cubre todavia: un handle que el padre le PASE al hijo (una
//! consola, un bloque) sigue siendo suyo, como siempre. La raiz cierra lo que
//! el hijo puede NOMBRAR, no lo que le dan.

use bmo_raiz_juicio::{NoRuta, Raiz, RUTA_MAX};

use crate::ring0::plat::spin::SpinLock;

/// Tantas plazas como procesos vivos caben (`cap::MAX_PROCS`).
const PLAZAS: usize = crate::ring0::obj::cap::MAX_PROCS;

static LOCK: SpinLock = SpinLock::new("raiz");

/// `(pid, raiz)` de cada proceso encerrado. Pid 0 = plaza libre.
static mut ENCERRADOS: [(u32, Option<Raiz>); PLAZAS] = [(0, None); PLAZAS];
/// `(pid del que lanza, raiz de su SIGUIENTE hijo)`, pedida con RAIZ_HIJO.
static mut PARA_HIJO: [(u32, Option<Raiz>); PLAZAS] = [(0, None); PLAZAS];
/// La raiz del lanzamiento en curso: la pone `preparar` y la toma `nacer`.
static mut PENDIENTE: Option<Raiz> = None;
/// Donde se escribe la ruta ya resuelta. Mismo trato que `RUTA_BUF` en
/// `syscall/mod.rs`: vive lo que vive la llamada que la pidio.
static mut RESUELTA: [u8; RUTA_MAX] = [0; RUTA_MAX];

fn buscar(tabla: &[(u32, Option<Raiz>); PLAZAS], pid: u32) -> Option<usize> {
    if pid == 0 {
        return None;
    }
    tabla.iter().position(|(p, _)| *p == pid)
}

/// **La raiz de `pid`**, o `None` si ve el disco entero.
pub fn de(pid: u32) -> Option<Raiz> {
    let _g = LOCK.lock();
    // SAFETY: las tablas solo se tocan con LOCK cogido.
    let t = unsafe { &*core::ptr::addr_of!(ENCERRADOS) };
    buscar(t, pid).and_then(|i| t[i].1)
}

/// **Esta encerrado?** Lo preguntan las operaciones que no llevan ruta.
pub fn encerrado(pid: u32) -> bool {
    de(pid).is_some()
}

/// **El siguiente hijo de `padre` vivira en `pedida`**, leida dentro de la raiz
/// del padre. Se usa en el siguiente `EJECUTAR` y se gasta ahi.
pub fn pedir_para_hijo(padre: u32, pedida: &str) -> Result<(), NoRuta> {
    let propia = de(padre);
    let r = Raiz::para_hijo(propia.as_ref(), pedida)?;
    let _g = LOCK.lock();
    // SAFETY: con LOCK cogido.
    let t = unsafe { &mut *core::ptr::addr_of_mut!(PARA_HIJO) };
    let i = match buscar(t, padre) {
        Some(i) => i,
        None => t.iter().position(|(p, _)| *p == 0).ok_or(NoRuta::Raiz)?,
    };
    t[i] = (padre, Some(r));
    Ok(())
}

/// **Antes de lanzar**: la raiz que tendra el hijo de `padre`. La pedida con
/// RAIZ_HIJO si la hay (y se gasta); si no, la del padre entera. `None` para
/// los lanzamientos de Ring 0, que ven el disco entero como siempre.
pub fn preparar(padre: Option<u32>) {
    let heredada = padre.and_then(de);
    let _g = LOCK.lock();
    // SAFETY: con LOCK cogido.
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(PARA_HIJO);
        let pedida = padre.and_then(|p| buscar(t, p)).and_then(|i| core::mem::replace(&mut t[i], (0, None)).1);
        *core::ptr::addr_of_mut!(PENDIENTE) = pedida.or(heredada);
    }
}

/// **Despues de lanzar**, salga bien o mal: que la raiz de este lanzamiento no
/// se la lleve el siguiente.
pub fn soltar_pendiente() {
    let _g = LOCK.lock();
    // SAFETY: con LOCK cogido.
    unsafe { *core::ptr::addr_of_mut!(PENDIENTE) = None };
}

/// **Al nacer `pid`**, antes de que exista su tarea. Toma la raiz pendiente.
///
/// Devuelve `false` si el proceso tenia que nacer encerrado y no queda plaza
/// donde apuntarlo: entonces NO nace. La otra opcion --nacer suelto-- es la
/// que este fichero existe para no tener.
pub fn nacer(pid: u32) -> bool {
    let _g = LOCK.lock();
    // SAFETY: con LOCK cogido.
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(ENCERRADOS);
        if let Some(i) = buscar(t, pid) {
            t[i] = (0, None);
        }
        let Some(r) = (*core::ptr::addr_of_mut!(PENDIENTE)).take() else {
            return true;
        };
        match t.iter().position(|(p, _)| *p == 0) {
            Some(i) => {
                t[i] = (pid, Some(r));
                true
            }
            None => false,
        }
    }
}

/// **Al morir `pid`**: ni su raiz ni la que pidio para un hijo sobreviven.
pub fn olvidar(pid: u32) {
    let _g = LOCK.lock();
    // SAFETY: con LOCK cogido.
    unsafe {
        for t in [&mut *core::ptr::addr_of_mut!(ENCERRADOS), &mut *core::ptr::addr_of_mut!(PARA_HIJO)] {
            if let Some(i) = buscar(t, pid) {
                t[i] = (0, None);
            }
        }
    }
}

/// **La ruta que de verdad se abre** cuando `pid` escribe `cruda`. Sin raiz,
/// la misma. Con raiz, escrita dentro de ella, o el motivo por el que no.
pub fn resolver(pid: u32, cruda: &'static str) -> Result<&'static str, NoRuta> {
    let Some(r) = de(pid) else {
        return Ok(cruda);
    };
    // SAFETY: el renglon vive lo que la llamada que lo pidio, como RUTA_BUF.
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(RESUELTA);
        let n = r.resolver(cruda, buf)?;
        core::str::from_utf8(&buf[..n]).map_err(|_| NoRuta::Control)
    }
}

/// **La linea de EJECUTAR** de `pid`: la RUTA dentro de su raiz y los
/// argumentos tal cual detras. Solo la ruta pasa por el juez: un argumento
/// (`-iwad apps/doom2.wad`) es texto para el hijo, no un sitio del disco.
pub fn linea_de_lanzar(pid: u32, cruda: &'static str) -> Result<&'static str, NoRuta> {
    let Some(r) = de(pid) else {
        return Ok(cruda);
    };
    let (ruta, resto) = crate::ring0::task::argumentos::partir(cruda);
    // SAFETY: el renglon vive lo que la llamada que lo pidio, como RUTA_BUF.
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(RESUELTA);
        let mut n = r.resolver(ruta, buf)?;
        if !resto.is_empty() {
            let fin = n + 1 + resto.len();
            if fin > RUTA_MAX {
                return Err(NoRuta::Larga);
            }
            buf[n] = b' ';
            buf[n + 1..fin].copy_from_slice(resto.as_bytes());
            n = fin;
        }
        core::str::from_utf8(&buf[..n]).map_err(|_| NoRuta::Control)
    }
}

/// El motivo, en palabras de CABINA.
pub fn motivo(n: NoRuta) -> &'static str {
    match n {
        NoRuta::Subir => "fuera de su raiz: un tramo de puntos sube",
        NoRuta::Borde => "fuera de su raiz: un tramo con blancos en el borde",
        NoRuta::Letra => "fuera de su raiz: una letra de unidad no existe para el",
        NoRuta::Control => "fuera de su raiz: un caracter de control",
        NoRuta::Larga => "la ruta, dentro de su raiz, no cabe",
        NoRuta::Raiz => "la raiz pedida no vale o no queda plaza",
    }
}
