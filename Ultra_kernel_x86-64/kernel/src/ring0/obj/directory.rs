//! `KIND_DIRECTORIO` -- **preguntar que hay**, como capability.
//!
//! [carril]  AMARILLO  preguntar que hay
//! [consumo] NADA      corre cuando una tarea usa el objeto
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! Hasta ahora Ring 3 podia LANZAR un programa pero no MIRAR el disco: habia
//! que saberse la ruta de memoria y teclearla entera. Sin esto no hay `ls`, no
//! hay autocompletado y no hay iconos de carpeta -- no por falta de dibujo,
//! sino porque no existia la pregunta.
//!
//! ## Esto NO es como abrir un archivo por su nombre
//!
//! Y ahi esta la diferencia con un sistema de ficheros clasico, que merece
//! estar escrita porque es el modelo entero:
//!
//! - En Unix, **una ruta es un NAME**. Cualquiera puede escribir `/etc/passwd`
//!   y el kernel decide despues si le deja. El nombre siempre es nombrable.
//! - Aqui una ruta abierta es un **HANDLE que a alguien le concedieron**. Lo
//!   que no te han dado no existe para tu proceso: no es que te lo nieguen, es
//!   que no tienes con que preguntar.
//!
//! Por eso `open` es una operacion sobre `CURRENT_TASK` --lo que uno pide por
//! ser quien es-- y el listado es una operacion sobre el handle resultante. El
//! dia que haya varios usuarios, quien puede abrir que se decide en `open` y
//! el resto del sistema no se entera.
//!
//! ## Sin cursor en el driver
//!
//! El handle guarda `(cluster, indice)` y el driver contesta "dame la entrada
//! n". Es O(n) por llamada, o sea O(n^2) por listado -- irrelevante con
//! directorios de decenas de entries, y a cambio **dos listados a la vez no se
//! pisan** y una entrada que desaparece no deja un cursor apuntando al vacio.
//!
//! ## Los nombres salen en 8.3 crudo
//!
//! `COBOL   BEX` con sus espacios, tal cual esta en el disco. Convertirlo a
//! `COBOL.BEX` es presentacion, y la presentacion es de Ring 3 -- la misma linea
//! que deja el cursor del raton fuera del kernel.

use crate::ring0::obj::cap;

/// Cuantos directorios pueden estar abiertos a la vez.
pub const MAX_ABIERTOS: usize = 8;

pub const NO_OWNER: u32 = u32::MAX;

/// No quedan ranuras de directorio abierto.
// ** EL NOMBRE DICE DE QUE PUERTA ES (2026-09-12). Se llamaban
// `ERROR_NOT_THERE` y `ERROR_NO_FREE_SLOT` -- los mismos nombres que usaban
// `launch`, `file` y `console` con OTROS numeros. El numero no cambia: cambia
// que ahora se sabe cual es cual sin abrir tres ficheros. Ver L6j.
pub const ERROR_DIR_SIN_HUECO: u32 = 25;
/// La ruta no existe, o no es un directorio.
pub const ERROR_DIR_NO_ESTA: u32 = 26;

/// Avanza a la siguiente entrada y devuelve lo que se sabe de ella:
/// `(hay << 63) | (es_dir << 62) | medida`. `hay == 0` = se acabo el
/// directorio.
///
/// El NAME no viaja aqui: son 11 bytes y no caben con los demas campos.
/// Se pide aparte con `DIR_OP_NOMBRE`, que es la misma decision que ya se tomo
/// en la consola -- un contador honesto vale mas que un byte apretado.
pub const DIR_OP_SIGUIENTE: u64 = 0x01;
/// Los 11 bytes del nombre 8.3 de la entrada ACTUAL, de 7 en 7.
/// `arg0` = desplazamiento (0 o 7). Devuelve `(n << 56) | bytes_LE`.
pub const DIR_OP_NOMBRE: u64 = 0x02;
/// **Cerrar. Devuelve la ranura.**
///
/// === Por que faltaba, y lo que costo ===
///
/// No existia, asi que la UNICA forma de liberar una ranura era
/// [`process_died`] -- o sea, que el proceso se muriera. Y el cliente de esto
/// es **el compositor, que no muere nunca**: es el escritorio.
///
/// Resultado: cada `ls` se quedaba una ranura para siempre, y al noveno la
/// tabla estaba llena. A partir de ahi `ls` contestaba **"no puedo abrir esa
/// carpeta"** -- un mensaje falso, porque la carpeta estaba perfectamente ahi.
/// Lo que no habia era sitio para abrirla.
///
/// `KIND_ARCHIVO` si tenia su `ARCH_OP_CERRAR` desde el principio. Esta es la
/// misma clase de recurso y se quedo sin el; la asimetria no se ve leyendo
/// ninguno de los dos archivos por separado.
///
/// Es el **patron 17** de nuevo --una tabla de recursos vivos que solo se libera
/// con un evento que no ocurre-- y ya se pago hoy en `KIND_MEMORIA`, donde el
/// contador se indexaba por un pid que solo sube. La pregunta que lo caza en
/// los dos casos es la misma: **quien devuelve esto, y ocurre alguna vez?**
pub const DIR_OP_CERRAR: u64 = 0x03;

/// ** LAS FECHAS Y LOS ATRIBUTOS de la entrada ACTUAL (01-10): `arg0` 0, 1 y
/// 2 son creado, escrito y leido (FILETIME: 100 ns desde 1601); 3, los
/// atributos de Windows. Solo los da el disco Personal (`d:`, NTFS); en FAT32
/// y ESTRATOS, 0 (= no se sabe).
///
/// Por que hace falta (y por que en Ring 0): Cyberpunk mira su
/// `final.redscripts` SOLO con GetFileAttributesExW, y con las fechas a 0 (un
/// fichero de 1601) dice "corrupted or missing scripts file". Las fechas
/// estan en la clave del indice NTFS que el kernel ya lee para listar; este
/// es el unico sitio que las tiene.
pub const DIR_OP_FECHAS: u64 = 0x04;

/// `[creado, escrito, leido, atributos]` de la entrada actual de cada ranura.
static mut FECHAS: [[u64; 4]; MAX_ABIERTOS] = [[0; 4]; MAX_ABIERTOS];

static mut CLUSTER: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// ** De que VOLUMEN es el cluster: `true` = la particion de arranque (`efi:`),
/// en solo lectura. Un cluster sin su volumen es un numero que apunta a dos
/// sitios distintos -- el 7 de la ESP no es el 7 de DATOS.
static mut ARRANQUE: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
static mut INDICE: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut NAME: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut OWNER: [u32; MAX_ABIERTOS] = [NO_OWNER; MAX_ABIERTOS];
/// ** N1b (29-09): la carpeta es del disco Personal (`d:`): su registro del
/// MFT; `u64::MAX` = no, es de FAT. Su nombre ya no es un 8.3 de 11 bytes:
/// va entero en UTF-8 en `LARGO` y lo cuenta `NLEN`.
static mut PERSONAL: [u64; MAX_ABIERTOS] = [u64::MAX; MAX_ABIERTOS];
static mut LARGO: [[u8; 256]; MAX_ABIERTOS] = [[0; 256]; MAX_ABIERTOS];
static mut NLEN: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// ** ESTRATOS (01-10): la carpeta en ESTRATOS, si la hay. Una ruta puede
/// estar en los DOS volumenes (la raiz siempre): se listan las de FAT32 y
/// despues las de ESTRATOS, como una sola carpeta. `FAT[i]` dice si quedan
/// de FAT32; `DESDE[i]`, en que numero empezaron las de ESTRATOS.
static mut ESTRATOS: [Option<bmo_estratos::objects::BlockPtr>; MAX_ABIERTOS] = [None; MAX_ABIERTOS];
static mut FAT: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
static mut DESDE: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// La entrada de ahora es de ESTRATOS (nombre largo en `LARGO`).
static mut DE_ESTRATOS: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];

/// El bit de `SIGUIENTE` que dice "nombre ENTERO en UTF-8" para una entrada
/// de ESTRATOS (su medida va en los 61 de abajo). Las de FAT32 no lo llevan:
/// quien solo sabe de 8.3 (`next` del userland) las salta.
pub const DIR_NOMBRE_LARGO: u64 = 1 << 61;

/// Abre un directorio y entrega su handle a `pid`. Ruta vacia = la raiz de
/// DATOS; `efi:` delante = la particion de arranque, solo para mirar (ver
/// `fs::sin_volumen`). Listar no escribe, asi que aqui no hace falta mas.
pub fn open(pid: u32, ruta: &str) -> Result<u64, u32> {
    // ** `d:` es el disco Personal, en NTFS y solo para leer (N1b).
    let mut personal = u64::MAX;
    let mut estratos = None;
    let mut fat = false;
    let (arranque, cluster) = if let Some(resto) = crate::ring0::dev::disk::ajeno::ruta_personal(ruta) {
        match crate::ring0::dev::disk::ajeno::abrir(resto) {
            Some(Ok(n)) if n.carpeta => personal = n.registro,
            _ => return Err(ERROR_DIR_NO_ESTA),
        }
        (false, 0)
    } else {
        // FAT32 y ESTRATOS: la carpeta puede estar en uno, en otro o en los dos.
        let f = crate::ring0::fsys::fs::dir_de(ruta);
        let es = crate::ring0::fsys::estratos::is_mounted().then(|| crate::ring0::fsys::estratos::abrir_carpeta(ruta)).flatten();
        if f.is_none() && es.is_none() {
            return Err(ERROR_DIR_NO_ESTA);
        }
        fat = f.is_some();
        estratos = es;
        f.unwrap_or((false, 0))
    };
    unsafe {
        let libre = (0..MAX_ABIERTOS).find(|&i| OWNER[i] == NO_OWNER);
        let i = match libre {
            Some(i) => i,
            None => return Err(ERROR_DIR_SIN_HUECO),
        };
        CLUSTER[i] = cluster;
        ARRANQUE[i] = arranque;
        // * Empieza en usize::MAX para que el PRIMER `SIGUIENTE` caiga en la
        // entrada 0. Si empezara en 0, la primera llamada devolveria la
        // segunda entrada y la primera no la veria nadie -- el clasico error
        // de un cursor que ya apunta a algo antes de que le pidan avanzar.
        INDICE[i] = usize::MAX;
        NAME[i] = [b' '; 11];
        PERSONAL[i] = personal;
        NLEN[i] = 0;
        ESTRATOS[i] = estratos;
        FAT[i] = fat;
        DESDE[i] = 0;
        DE_ESTRATOS[i] = false;
        OWNER[i] = pid;
        match cap::grant(pid, cap::KIND_DIRECTORIO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("dir", "directorio abierto para Ring 3", pid as u64);
                Ok(h)
            }
            None => {
                OWNER[i] = NO_OWNER;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn next(i: usize) -> u64 {
    unsafe {
        let n = INDICE[i].wrapping_add(1);
        if PERSONAL[i] != u64::MAX {
            let nombre = &mut *core::ptr::addr_of_mut!(LARGO[i]);
            return match crate::ring0::dev::disk::ajeno::entrada(PERSONAL[i], n, nombre) {
                // La medida en 62 bits: un fichero de Cyberpunk pasa de 4 GiB.
                Some((largo, carpeta, medida, f, atributos)) => {
                    INDICE[i] = n;
                    NLEN[i] = largo;
                    FECHAS[i] = [f[0], f[1], f[2], atributos as u64];
                    (1u64 << 63) | ((carpeta as u64) << 62) | (medida & ((1 << 62) - 1))
                }
                None => 0,
            };
        }
        if FAT[i] {
            match crate::ring0::fsys::fs::entrada_de(ARRANQUE[i], CLUSTER[i], n) {
                Some((name, es_dir, tam)) => {
                    INDICE[i] = n;
                    NAME[i] = name;
                    DE_ESTRATOS[i] = false;
                    return (1u64 << 63) | ((es_dir as u64) << 62) | tam as u64;
                }
                None => {
                    FAT[i] = false;
                    DESDE[i] = n;
                }
            }
        }
        let Some(p) = ESTRATOS[i] else { return 0 };
        let nombre = &mut *core::ptr::addr_of_mut!(LARGO[i]);
        match crate::ring0::fsys::estratos::entrada_n(&p, n - DESDE[i], nombre) {
            Some((largo, carpeta, medida)) => {
                INDICE[i] = n;
                NLEN[i] = largo;
                DE_ESTRATOS[i] = true;
                (1u64 << 63) | ((carpeta as u64) << 62) | DIR_NOMBRE_LARGO | (medida & (DIR_NOMBRE_LARGO - 1))
            }
            None => 0,
        }
    }
}

fn name(i: usize, desde: usize) -> u64 {
    unsafe {
        let n: &[u8] = if PERSONAL[i] != u64::MAX || DE_ESTRATOS[i] { &LARGO[i][..NLEN[i]] } else { &NAME[i] };
        let mut w = [0u8; 8];
        let mut k = 0usize;
        while k < 7 && desde + k < n.len() {
            w[k] = n[desde + k];
            k += 1;
        }
        ((k as u64) << 56) | u64::from_le_bytes(w)
    }
}

pub fn operation(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    match op {
        DIR_OP_SIGUIENTE => Some(next(i)),
        DIR_OP_NOMBRE => Some(name(i, arg0 as usize)),
        // SAFETY: el indice ya se comprobo; un solo hilo toca la tabla.
        DIR_OP_FECHAS => Some(unsafe { FECHAS[i][(arg0 as usize).min(3)] }),
        DIR_OP_CERRAR => {
            unsafe {
                OWNER[i] = NO_OWNER;
                CLUSTER[i] = 0;
                INDICE[i] = usize::MAX;
                PERSONAL[i] = u64::MAX;
                FECHAS[i] = [0; 4];
                ESTRATOS[i] = None;
                FAT[i] = false;
                DE_ESTRATOS[i] = false;
            }
            Some(1)
        }
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`: los directorios que tuviera abiertos se cierran.
/// Cuantas ranuras siguen siendo de `pid`. **Despues de `process_died` tiene
/// que ser CERO**, y quien lo comprueba es la autopsia: el escalon 1 de
/// `docs/plan/PLAN_AUTOCURACION.md`.
///
/// Existe porque `process_died` hace su trabajo y **nadie miraba si funciono**.
/// Una fuga de ranuras no da error: da un sistema que un dia no puede abrir un
/// directorio mas, sin nada que lo relacione con el proceso que murio hace una
/// hora.
pub fn pending_of(pid: u32) -> u32 {
    let mut n = 0;
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                n += 1;
            }
        }
    }
    n
}

pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                OWNER[i] = NO_OWNER;
                CLUSTER[i] = 0;
                INDICE[i] = usize::MAX;
                PERSONAL[i] = u64::MAX;
                ESTRATOS[i] = None;
                FAT[i] = false;
                DE_ESTRATOS[i] = false;
            }
        }
    }
}
