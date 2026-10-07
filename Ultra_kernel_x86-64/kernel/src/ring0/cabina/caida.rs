//! **LA CAIDA** -- lo ultimo que dijo la maquina, en RAM que sobrevive al reinicio.
//!
//! [carril]  AMARILLO  es un instrumento: si miente no falla, CONVENCE. Un
//!                     fichero de caida con las lineas equivocadas manda la
//!                     investigacion al sitio que no es
//! [consumo] NADA      apunta o pinta cuando alguien lo llama
//! [cuesta]  NADA      un byte y un incremento por cada byte que sale a pantalla.
//!                     Ni disco, ni bloqueo, ni interrupcion
//! [riesgo]  AJENO     depende de que la PLACA conserve la DRAM en un reinicio
//!                     en caliente, y eso no lo decide este kernel: lo decide el
//!                     firmware. Es una hipotesis hasta que el Ryzen conteste
//!
//! # La pregunta del propietario (2026-09-11)
//!
//! > *"podemos hacer que el arranque escriba en tiempo real para saber?"*
//!
//! Y la pregunta de fondo, que es mejor: **hoy lo que se sabe es lo que `save`
//! alcanzo a guardar, y `save` necesita que la maquina siga viva.** Si se cuelga
//! en el minuto tres, lo del minuto tres no existe. `blackbox.rs` lo tiene
//! escrito: es la pieza que puede tener que correr *mientras lo que graba se
//! esta muriendo*, que es justo cuando el disco es lo menos fiable.
//!
//! # La idea, que no es de esta casa: `ramoops`
//!
//! Los kernels de verdad lo resuelven asi. **La DRAM no se borra en un reinicio
//! en caliente** --el reset no corta la alimentacion--, asi que una region fija
//! de RAM escrita durante la sesion sigue ahi cuando el kernel vuelve a
//! arrancar. Se lee ANTES de que nadie la pise, se guarda en el disco con calma
//! --ahora que el disco es fiable-- y se vuelve a preparar para esta sesion.
//!
//! ```text
//!    durante la sesion    cada byte que sale a pantalla -> tambien aqui
//!                         coste: un `mov` y un `inc`. Cero disco.
//!    la maquina muere     la RAM se queda como estaba
//!    reinicio             se lee la region, se escribe CAIDA.TXT, se limpia
//! ```
//!
//! *** Es "en tiempo real" en el unico sentido que importa: **nada de lo que
//! salio por pantalla se pierde**, y no cuesta nada mientras la maquina vive.
//!
//! # [!] Y ES UNA HIPOTESIS SOBRE LA PLACA, no una certeza -- LEY 24
//!
//! Dos cosas pueden borrarla y este kernel no manda sobre ninguna:
//!
//! ```text
//!    1. el firmware limpia la DRAM al reiniciar   (los AMI de AM4 no suelen)
//!    2. el firmware o el cargador ESCRIBEN encima  antes de que esto la lea
//! ```
//!
//! Contra la 2 se elige una direccion que ni las etapas ni el kernel tocan
//! --64 MiB, lejos de 0x100000/0x200000/0x400000-- y se comprueba al arrancar
//! que cae dentro de un tramo de RAM usable. Contra la 1 no hay defensa: solo
//! medirlo. **El primer arranque con esto dentro es el perfil.** Si tras un
//! reinicio en caliente aparece `CAIDA.TXT` con las lineas de la sesion
//! anterior, la placa lo conserva. Si no, se dice, y el plan B es un volcado
//! periodico al disco.
//!
//! # La forma en RAM
//!
//! ```text
//!    +0    magia        u64   "vale, esto es mio"
//!    +8    version      u32   por si la forma cambia
//!    +12   generacion   u32   cuantos arranques ha visto esta region
//!    +16   cursor       u64   bytes escritos en total; posicion = cursor % CAP
//!    +24   suma         u64   de magia|version|generacion. NO del cursor
//!    +32   texto        CAP bytes, anillo
//! ```
//!
//! ** La suma no cubre el cursor a proposito: recalcularla en cada byte seria
//! pagar una decena de operaciones por byte para proteger un numero que ya se
//! valida por rango. La cabecera fija se firma una vez; el cursor se comprueba
//! con `< 2^40`, que es lo que separa un cursor de basura.
//!
//! # Lo que NO hace
//!
//! No toma ningun cerrojo. Se escribe desde interrupciones y desde el camino de
//! la muerte, y un cerrojo ahi es la forma de que la ultima linea no se escriba
//! nunca. Una linea partida por otra en el fichero de caida es un precio; un
//! fichero vacio no lo es.

/// Donde vive. 64 MiB: lejos de las etapas (1, 2 y 4 MiB) y del kernel.
pub const BASE: u64 = 0x0400_0000;
/// Cuanto: 256 KiB de texto, que son unas cinco mil lineas.
pub const BYTES: u64 = 256 * 1024;
/// Lo que se reserva: el anillo y, detras, el tablero (`tablero.rs`).
pub const RESERVA: u64 = BYTES + super::tablero::BYTES;
/// **La pagina de 2 MiB entera** (V8): se reserva toda para que nada mas
/// viva en ella. Su tipo de memoria pasa a WT, y una pagina WT con RAM de
/// otro dentro seria memoria ajena lenta -- y un alias de tipos.
pub const PAGINA: u64 = 0x20_0000;
const _: () = assert!(BASE % PAGINA == 0 && RESERVA <= PAGINA);
const CAB: u64 = 32;
const CAP: u64 = BYTES - CAB;
/// Lo que esta sesion puede escribir ANTES de que el disco este montado sin
/// pisar lo recuperado. Un arranque hasta el montaje son unas decenas de KiB
/// de lineas; con 64 KiB de margen, lo recuperado se recorta a los ultimos
/// 192 KiB y el arranque nuevo escribe detras sin tocarlo.
const MARGEN: u64 = 64 * 1024;
const MAGIA: u64 = 0x4341_4944_4142_4D4F; // "CAIDABMO" al reves, como se lee en memoria
const VERSION: u32 = 1;

/// `true` desde que la region esta comprobada y preparada para esta sesion.
static mut LISTA: bool = false;

/// **Como llega a la RAM lo que se escribe aqui** (V8). Lo sabe quien abre
/// (`mm::phys`, que cambio la pagina con `vmm::escribir_directo`); CABINA no
/// toca tablas de paginas.
#[derive(Clone, Copy)]
pub enum Escritura {
    /// El tipo EFECTIVO de la pagina escribe directo (WT, UC): cada byte esta
    /// en la RAM al acabar la instruccion. Ni un `clflush` hace falta. El
    /// numero es el tipo (4 = WT).
    Directa(u8),
    /// Se pidio WT, pero el tipo efectivo no se sabe (los MTRR no lo dejan
    /// ver, `0xFF`) o no es directo: los dos cinturones, `clflush` como V7.
    Dudosa(u8),
    /// No se pudo cambiar la pagina (el porque, de `bmo-cache-juicio`):
    /// sigue en WB, con el `clflush` de V7.
    EnCache(u8),
}

/// `true` si la pagina escribe directo: `anotar` y `a_la_ram` no necesitan
/// sacar nada de la cache.
static mut DIRECTA: bool = false;
/// Lo que se dijo ANTES de que la region estuviera lista, para no perderlo.
const ANTES_MAX: usize = 4096;
static mut ANTES: [u8; ANTES_MAX] = [0; ANTES_MAX];
static mut ANTES_N: usize = 0;
/// Lo que se recupero del arranque anterior: `(bytes, generacion)`.
static mut RECUPERADO: usize = 0;
static mut GENERACION: u32 = 0;

/// Donde se ve `BASE` desde el kernel. **Lo pone quien abre** (`mm::phys::init`)
/// y no se pregunta a `mm`: CABINA es la familia mas baja que apunta (L8b), y
/// el dato sube como parametro en vez de importar la memoria (L7a).
static mut VIRT: u64 = 0;

#[inline(always)]
fn ptr(off: u64) -> *mut u8 {
    unsafe { (VIRT + off) as *mut u8 }
}

unsafe fn lee64(off: u64) -> u64 { core::ptr::read_volatile(ptr(off) as *const u64) }
unsafe fn lee32(off: u64) -> u32 { core::ptr::read_volatile(ptr(off) as *const u32) }
unsafe fn pon64(off: u64, v: u64) { core::ptr::write_volatile(ptr(off) as *mut u64, v) }
unsafe fn pon32(off: u64, v: u32) { core::ptr::write_volatile(ptr(off) as *mut u32, v) }

fn suma(magia: u64, version: u32, generacion: u32) -> u64 {
    // Una mezcla barata, no criptografia: separa "cabecera de una sesion" de
    // "bytes cualesquiera", que es lo unico que hace falta.
    let mut s = magia ^ ((version as u64) << 32 | generacion as u64);
    s ^= s.rotate_left(29);
    s = s.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    s ^ s.rotate_left(17)
}

/// **Un byte que salio a pantalla.** Se llama desde `serial_write_byte`, que
/// es el embudo por el que pasa todo lo que la maquina dice.
#[inline]
pub fn anotar(b: u8) {
    unsafe {
        if !LISTA {
            // Todavia no hay region: se guarda aparte y se copia al abrir.
            if ANTES_N < ANTES_MAX {
                ANTES[ANTES_N] = b;
                ANTES_N += 1;
            }
            return;
        }
        let cursor = lee64(16);
        core::ptr::write_volatile(ptr(CAB + cursor % CAP), b);
        pon64(16, cursor + 1);
        // ** CADA LINEA, A LA RAM AL ACABARLA (07-10, la segunda caida con
        // `smp all`). El tick (`a_la_ram`) no basta: un camino con las
        // interrupciones cerradas -- despertar a los obreros, un syscall
        // largo -- no tiene tick, y si la maquina muere ahi dentro, sus
        // lineas solo estaban en la cache. Dos o tres `clflush` por linea.
        if b == b'\n' && !DIRECTA {
            let desde = LINEA.min(cursor);
            let mut t = desde & !63;
            while t <= cursor {
                a_la_ram_linea(ptr(CAB + t % CAP));
                t += 64;
            }
            a_la_ram_linea(ptr(CAB + cursor % CAP));
            a_la_ram_linea(ptr(0));
            LINEA = cursor + 1;
        }
    }
}

/// Donde empezo la linea que se esta escribiendo (para sacarla entera).
static mut LINEA: u64 = 0;

/// Hasta donde (el cursor) lo escrito ya esta en la RAM, no solo en la cache.
static mut EN_RAM: u64 = 0;

/// Escribir una linea de cache a la RAM (y sacarla de la cache).
#[inline]
pub(super) fn a_la_ram_linea(p: *const u8) {
    // SAFETY: CLFLUSH sobre memoria mapeada (el physmap de la reserva); no
    // cambia su contenido.
    unsafe { core::arch::asm!("clflush [{}]", in(reg) p, options(nostack, preserves_flags)) };
}

/// **Lo escrito, a la RAM de verdad** (07-10). Lo llama el tick.
///
/// *** POR QUE: las CUATRO caidas del 07-10 acababan en la MISMA linea del
/// arranque (la autopsia del booter de la 3060) aunque la maquina siguio
/// viva minutos despues. No era que nadie escribiera: el anillo y su CURSOR
/// se escriben en memoria con cache (WB), y el cursor es una linea que se
/// toca en cada byte -- no sale nunca de la cache. Un reinicio de golpe (un
/// triple fallo) BORRA la cache sin escribirla: la RAM se quedaba con el
/// cursor del ultimo `wbinvd`, que es justo el que hace el arranque de la
/// 3060 antes del booter. Todo lo de despues existia solo en la cache.
///
/// Ahora, en cada tick, si el cursor se movio: las lineas de texto nuevas y
/// la cabecera, con `clflush`. Sin nada nuevo, una lectura.
pub fn a_la_ram() {
    unsafe {
        // Con la pagina en WT (V8) ya esta todo en la RAM: nada que sacar.
        if !LISTA || DIRECTA {
            return;
        }
        let c = lee64(16);
        if c == EN_RAM {
            return;
        }
        let desde = if c.saturating_sub(EN_RAM) > CAP { c - CAP } else { EN_RAM.min(c) };
        let mut o = desde & !63;
        while o < c {
            a_la_ram_linea(ptr(CAB + o % CAP));
            o += 64;
        }
        // El texto empieza en +32: la ultima linea de cache puede quedar
        // detras del ultimo `o` (lo que se perdia: ~30 bytes de basura al
        // final del CAIDA del 07-10).
        a_la_ram_linea(ptr(CAB + (c - 1) % CAP));
        a_la_ram_linea(ptr(0));
        EN_RAM = c;
    }
}

/// **Abrir la region: recuperar lo del arranque anterior y preparar esta.**
///
/// Se llama desde `mm::phys::init`, justo despues de reservar el rango y con
/// el physmap ya en pie. `dentro_de_ram` lo comprueba quien llama, que es
/// quien tiene el mapa: si la region no cae en RAM usable, esto no se abre y
/// se dice. `base_virtual` es donde el physmap muestra `BASE`.
pub fn abrir(dentro_de_ram: bool, base_virtual: u64, escritura: Escritura) {
    unsafe {
        VIRT = base_virtual;
        DIRECTA = dentro_de_ram && matches!(escritura, Escritura::Directa(_));
    }
    if !dentro_de_ram {
        crate::ring0::cabina::warn(
            "caida",
            "la region de 64 MiB no cae en RAM usable: sin caja negra en RAM",
            BASE,
        );
        return;
    }
    let habia_magia;
    unsafe {
        let magia = lee64(0);
        habia_magia = magia == MAGIA;
        let version = lee32(8);
        let generacion = lee32(12);
        let cursor = lee64(16);
        let firma = lee64(24);
        let cabecera_ok = magia == MAGIA
            && version == VERSION
            && firma == suma(magia, version, generacion)
            && cursor < (1u64 << 40);
        if cabecera_ok && cursor > 0 {
            // *** HAY RASTRO DEL ARRANQUE ANTERIOR. No se mueve de sitio: se
            // apunta cuanto es y se deja donde esta hasta que el disco pueda
            // recibirlo. `volcar` lo lee de aqui.
            let total = if cursor > CAP { CAP } else { cursor };
            GENERACION = generacion.wrapping_add(1);
            // Lo recuperado tiene que sobrevivir a esta apertura: los bytes
            // nuevos de esta sesion empiezan DESPUES de reservarlo. Se
            // empaqueta al principio del anillo en orden cronologico, para que
            // `volcar` lo lea de un tiron y esta sesion escriba detras.
            enderezar(cursor);
            // ** Y SE RECORTA A LOS ULTIMOS `CAP - MARGEN`. Si se guardara
            // todo el anillo lleno, el cursor de esta sesion arrancaria en
            // CAP, o sea en 0: la primera linea del arranque nuevo pisaria
            // la primera linea recuperada, y el disco no esta montado hasta
            // bastante despues. Lo mas viejo es lo que menos vale.
            let k = if total > CAP - MARGEN { CAP - MARGEN } else { total };
            if k < total {
                core::ptr::copy(ptr(CAB + (total - k)), ptr(CAB), k as usize);
            }
            RECUPERADO = k as usize;
            pon64(16, k);
        } else {
            RECUPERADO = 0;
            GENERACION = if cabecera_ok { generacion.wrapping_add(1) } else { 1 };
            pon64(16, 0);
        }
        // EL TABLERO (07-10): lo que cada nucleo estaba haciendo, al final de
        // lo recuperado (detras del anillo, en la misma reserva).
        super::tablero::abrir(VIRT + BYTES, &mut |linea: &str| {
            let n = RECUPERADO as u64;
            let l = linea.len() as u64 + 1;
            if n + l < CAP - 1024 {
                core::ptr::copy_nonoverlapping(linea.as_ptr(), ptr(CAB + n), linea.len());
                *ptr(CAB + n + l - 1) = b'\n';
                RECUPERADO = (n + l) as usize;
                pon64(16, n + l);
            }
        });
        pon64(0, MAGIA);
        pon32(8, VERSION);
        pon32(12, GENERACION);
        pon64(24, suma(MAGIA, VERSION, GENERACION));
        EN_RAM = 0;
        LINEA = lee64(16);
        LISTA = true;
        // Y lo que se dijo antes de abrir, ahora dentro.
        let n = ANTES_N;
        let mut i = 0;
        while i < n {
            anotar(ANTES[i]);
            i += 1;
        }
        ANTES_N = 0;
    }
    // *** TRES RESPUESTAS DISTINTAS, porque mandan a sitios distintos. "No hay
    // rastro" y "hay rastro pisado" se leerian igual con un solo mensaje, y son
    // las dos hipotesis de la cabecera: la placa BORRA, o alguien ESCRIBE encima.
    match (unsafe { RECUPERADO }, habia_magia) {
        (n, _) if n > 0 => crate::ring0::cabina::info(
            "caida",
            "la RAM CONSERVO lo ultimo que dijo el arranque anterior: bytes",
            n as u64,
        ),
        (_, true) => crate::ring0::cabina::info(
            "caida",
            "la RAM conservo la cabecera y nada dentro: la sesion anterior no escribio (o es esta misma)",
            unsafe { GENERACION } as u64,
        ),
        (_, false) => crate::ring0::cabina::info(
            "caida",
            "la RAM no traia rastro: primer arranque, o la placa la borra al reiniciar",
            unsafe { GENERACION } as u64,
        ),
    }
    // ** V8: Y COMO ESCRIBE ESTA SESION. Es lo primero que hay que mirar en
    // un CAIDA corto: con "directo" la caja no pierde nada por la cache; con
    // "en cache" un final cortado puede ser la cache, no la maquina.
    match escritura {
        Escritura::Directa(t) => crate::ring0::cabina::info(
            "caida",
            "escribe DIRECTO a la RAM, sin quedarse en la cache (tipo efectivo, 4 = WT)",
            t as u64,
        ),
        Escritura::Dudosa(t) => crate::ring0::cabina::warn(
            "caida",
            "pidio WT pero el tipo efectivo no es directo o no se sabe (0xFF): sigue el clflush de V7",
            t as u64,
        ),
        Escritura::EnCache(motivo) => crate::ring0::cabina::warn(
            "caida",
            "la pagina sigue en WB (1 no presente, 2 no es de 2 MiB, 3 PAT sin WT): sigue el clflush de V7",
            motivo as u64,
        ),
    }
}

/// Deja los ultimos `min(cursor, CAP)` bytes del anillo en orden cronologico
/// desde el offset 0 del texto. Con un anillo que dio la vuelta hay que rotar;
/// sin vuelta ya estan en orden.
unsafe fn enderezar(cursor: u64) {
    if cursor <= CAP {
        return;
    }
    // Rotacion in situ por inversiones: tres `reverse` sin memoria extra.
    let inicio = (cursor % CAP) as usize;
    let texto = core::slice::from_raw_parts_mut(ptr(CAB), CAP as usize);
    texto[..inicio].reverse();
    texto[inicio..].reverse();
    texto.reverse();
}

/// **Lo que se recupero del arranque anterior**, en orden cronologico. Vacio si
/// no habia nada.
///
/// ** Aqui vivia `volcar`, que lo escribia en `CAIDA.TXT`. Escribir en el disco
/// es de quien LEE a todos, no del registro que todos llaman: salio al
/// `mirador` (L8b, 2026-09-13), y CABINA solo entrega los bytes.
pub fn texto_recuperado() -> &'static [u8] {
    let n = unsafe { RECUPERADO };
    if n == 0 {
        return &[];
    }
    unsafe { core::slice::from_raw_parts(ptr(CAB) as *const u8, n) }
}

/// Cuantos bytes se recuperaron del arranque anterior (para el panel).
pub fn recuperado() -> u64 {
    unsafe { RECUPERADO as u64 }
}

/// La generacion de esta region: cuantos arranques ha visto.
pub fn generacion() -> u64 {
    unsafe { GENERACION as u64 }
}
