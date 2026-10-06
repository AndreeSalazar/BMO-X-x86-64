//! **LAS RAMAS Y LA MEZCLA, EN EL KERNEL** (`docs/plan/PLAN_LAS_RAMAS.md`,
//! R4c-2b): crear una rama, cambiar de rama y MEZCLAR otra en la de ahora.
//!
//! [carril]  ROJO      escribe en el volumen. Escribir ES commitear
//! [consumo] NADA      corre cuando una persona crea, cambia o mezcla
//!
//! [eje]     CORRECCION -- lo pide una persona y escribe en el almacen
//! [exige]   `ESTRATOS.md` 0.1.1 ("las ramas y la mezcla", "la tabla de
//!           ramas"): el formato no se decide aqui
//!
//! Nada de lo que decide vive aqui. La tabla es `bmo_estratos::ramas`, la
//! mezcla es `bmo_estratos::motor_mezcla` --el MISMO motor que
//! `estratos-mezcla` prueba sobre imagenes contra otras dos mezclas, al azar--
//! y el orden del commit es el de `escribir::volver`. Lo propio son tres
//! cosas:
//!
//! ```text
//!    la BASE         el antepasado comun, siguiendo los DOS padres (D2)
//!    las tablas      el motor no pide memoria: se la presta esto, de marcos
//!                    contiguos, solo mientras mezcla
//!    los choques     DOS FASES (D3): contar y apuntarlos, que una persona
//!                    elija cada uno por la puerta, y solo entonces mezclar
//! ```
//!
//! === ** POR QUE DOS FASES Y NO UNA PREGUNTA A MITAD ===
//!
//! Porque a mitad de una mezcla el kernel no puede esperar a nadie: tiene una
//! transaccion abierta. Asi que CONTAR no escribe nada y apunta los choques;
//! la persona los mira y elige con calma; y MEZCLAR vuelve a contar con esas
//! respuestas, reserva y escribe de una. Si entre medias el volumen cambio
//! --otra generacion--, lo contado ya no vale y se dice: no se mezcla un plan
//! hecho sobre otro volumen.

use bmo_estratos as es;
use bmo_estratos::motor_mezcla::{self as motor, Eleccion, Nivel, Taller, RUTA_MAX};
use bmo_estratos::objects::{BlockPtr, BLOQUE, NIVELES_MAX};
use bmo_estratos::ramas::Ramas;

use super::{copia_en_uso, identidad_ok, superbloque, walk::DelDisco, write_superblock, WriteError};
use crate::ring0::dev::disk;
use crate::ring0::mm::{self, phys};

/// Como se llama la rama de ahora cuando el volumen no tenia tabla.
const PRIMERA: &[u8] = b"principal";

/// Cuantos choques se apuntan para que una persona los elija.
pub const CHOQUES_MAX: usize = 64;

/// Cuantos antepasados se miran por lado para encontrar la base.
const ANTEPASADOS_MAX: usize = 512;

/// Niveles de carpetas que se intentan prestar: los mismos 16 que baja el
/// cursor (`HONDO_MAX`), y si no hay marcos contiguos para tanto, 8.
const NIVELES: [usize; 2] = [16, 8];

// -- La tabla de ramas -----------------------------------------------------------

// ** LA TABLA Y SU BLOQUE VIVEN EN `static`, no en la pila: son ~4 KiB cada
// uno, y en el marco de `cambiar` y `publicar_tabla` pasaban de los 40 KiB de
// la pila de un syscall (`pila` lo midio, 06-10). Un solo hilo escribe
// ESTRATOS (`walk::scratch_de_flujo`), asi que una copia basta.
static mut TABLA: Ramas = Ramas::VACIA;
static mut CODIFICADA: [u8; BLOQUE] = [0; BLOQUE];

fn tabla() -> &'static mut Ramas {
    // SAFETY: un solo hilo escribe ESTRATOS, y nadie guarda la referencia.
    unsafe { &mut *core::ptr::addr_of_mut!(TABLA) }
}

/// Carga en [`TABLA`] la tabla del volumen. `false`: el volumen no tiene.
#[inline(never)]
fn cargar(sb: &es::Superblock) -> Result<bool, WriteError> {
    if sb.ramas.es_nulo() {
        return Ok(false);
    }
    let d = super::seguir(&sb.ramas, 0).ok_or(WriteError::NoSeLeeLaRaiz)?;
    *tabla() = Ramas::decode(d).map_err(|_| WriteError::NoSeLeeLaRaiz)?;
    Ok(true)
}

/// La tabla en su bloque, en [`CODIFICADA`].
#[inline(never)]
fn codificar() -> &'static [u8; BLOQUE] {
    // SAFETY: como `tabla`.
    unsafe {
        let b = &mut *core::ptr::addr_of_mut!(CODIFICADA);
        *b = tabla().encode();
        b
    }
}

/// Publica [`TABLA`] --y, si `sigue` trae una, la punta que el superbloque
/// pasa a seguir-- con el orden de siempre.
///
/// ** LA SUBIDA A v2 ESCRIBE LAS DOS COPIAS del superbloque (`ESTRATOS.md`, "la
/// tabla de ramas"): con una sola, un kernel v1 montaria la copia vieja y
/// escribiria encima de la nueva, perdiendo la tabla. Con las dos, un kernel
/// v1 no monta el volumen -- y no puede borrarla.
fn publicar_tabla(sb: &es::Superblock, sigue: Option<BlockPtr>) -> Result<u64, WriteError> {
    let cual = copia_en_uso();
    let mut tr = es::escritura::Transaccion::open(sb, cual, identidad_ok()).map_err(WriteError::Rechazada)?;
    let base = tr.reserve(1).map_err(WriteError::Rechazada)?;
    let bytes = codificar();
    super::escribir::poner(base, bytes)?;
    let p = BlockPtr::nuevo(base, 0, bytes);

    tr.cerrar_datos().map_err(WriteError::Rechazada)?;
    if !disk::flush() {
        tr.abandonar();
        return Err(WriteError::SinBarrera);
    }
    tr.barrera_hecha().map_err(WriteError::Rechazada)?;
    let (destino, nuevo) = tr.commit(sigue.unwrap_or(sb.estrato)).map_err(WriteError::Rechazada)?;
    let nuevo = nuevo.con_ramas(p);
    let sector = nuevo.encode();
    if !write_superblock(destino, &sector) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }
    // ** LA SUBIDA: la primera copia ya es v2, valida y la mas nueva, asi que
    // un corte aqui no pierde nada. Despues de esto, ninguna copia es v1.
    if sb.version != es::VERSION_RAMAS {
        if !write_superblock(cual, &sector) || !disk::flush() {
            crate::ring0::cabina::fault("estratos", "la subida a v2 no escribio la otra copia", cual);
            super::fijar_superbloque(nuevo);
            return Err(WriteError::NoEscribio);
        }
        crate::ring0::cabina::info("estratos", "volumen subido a v2: las dos copias llevan la tabla", nuevo.generation);
    }
    super::fijar_superbloque(nuevo);
    if sigue.is_some() {
        super::refrescar_fecha();
    }
    Ok(nuevo.generation)
}

/// **UNA RAMA NUEVA** que empieza en la punta de ahora. No cambia de rama.
///
/// Si el volumen no tenia ramas, la de ahora pasa a llamarse `principal`: la
/// tabla nace con DOS filas, la de siempre y la nueva.
#[inline(never)]
pub fn crear(nombre: &str) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    if !cargar(&sb)? {
        *tabla() = Ramas::nueva(PRIMERA).map_err(WriteError::Rama)?;
    }
    tabla().crear(nombre.as_bytes(), sb.estrato).map_err(WriteError::Rama)?;
    let g = publicar_tabla(&sb, None)?;
    crate::ring0::cabina::info("estratos", "rama nueva en la punta de ahora", g);
    Ok(g)
}

/// **CAMBIAR A LA RAMA `nombre`**: el superbloque sigue su punta, y la de
/// ahora se queda guardada en la tabla.
///
/// ** No publica estrato: nadie se vuelve antepasado de nadie. Por eso las
/// ramas no se mezclan solas al ir y venir -- que era el agujero de D1 con
/// `volver` (`PLAN_LAS_RAMAS.md`, D5).
#[inline(never)]
pub fn cambiar(nombre: &str) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    if !cargar(&sb)? {
        return Err(WriteError::Rama(es::ramas::RamaError::NoEsta));
    }
    let sigue = tabla().cambiar(nombre.as_bytes(), sb.estrato).map_err(WriteError::Rama)?;
    let g = publicar_tabla(&sb, Some(sigue))?;
    crate::ring0::cabina::info("estratos", "cambio de rama", g);
    Ok(g)
}

// -- La memoria prestada ---------------------------------------------------------

/// Marcos contiguos, del kernel, **solo mientras dura la mezcla**: se
/// devuelven al soltarlo, salga bien o mal.
///
/// ** Por que no `static`: las tablas del motor son ~38 KiB por nivel de
/// carpetas, 600 KiB para 16 niveles. Tenerlas en `.bss` para siempre por un
/// gesto que se hace a mano seria pagar todo el rato por algo de un momento.
struct Prestado {
    fisica: u64,
    marcos: u64,
}

impl Drop for Prestado {
    fn drop(&mut self) {
        for i in 0..self.marcos {
            phys::free_frame_de(self.fisica + i * mm::PAGE, phys::Titular::Kernel);
        }
    }
}

/// Lo que se presta, cortado en sus tablas.
struct Mesa<'p> {
    taller: Taller<'p>,
    de_a: &'p mut [BlockPtr],
    de_b: &'p mut [BlockPtr],
}

const fn paginas(bytes: usize) -> u64 {
    bytes.div_ceil(mm::PAGE as usize) as u64
}

/// Pide los marcos y los reparte. `None`: no hubo tramo contiguo ni para 8
/// niveles, y se dice.
fn prestar() -> Option<(Prestado, usize)> {
    let fijo = paginas((NIVELES_MAX + NIVELES_MAX + 1 + 1) * BLOQUE)
        + paginas(RUTA_MAX)
        + 2 * paginas(ANTEPASADOS_MAX * core::mem::size_of::<BlockPtr>());
    NIVELES.iter().find_map(|&n| {
        let marcos = fijo + paginas(n * core::mem::size_of::<Nivel>());
        let fisica = phys::alloc_frames_contig_de(marcos, phys::Titular::Kernel)?;
        // ** TODO A CERO ES `Nivel::VACIO`, un `BlockPtr::NULO` y un bloque en
        // blanco: cada campo es un entero o bytes. Es lo que hace valido mirar
        // estos marcos como esas tablas.
        unsafe { core::ptr::write_bytes(mm::phys_to_virt(fisica) as *mut u8, 0, (marcos * mm::PAGE) as usize) };
        Some((Prestado { fisica, marcos }, n))
    })
}

impl Prestado {
    /// Las tablas, dentro de los marcos. Cada una empieza en su pagina.
    fn mesa(&mut self, niveles: usize) -> Mesa<'_> {
        let mut p = mm::phys_to_virt(self.fisica) as *mut u8;
        // SAFETY: los marcos son de este `Prestado`, contiguos y a cero
        // (`prestar`), y cada tabla cae en un tramo propio, alineado a pagina.
        unsafe {
            let mut cortar = |bytes: usize| {
                let aqui = p;
                p = p.add((paginas(bytes) * mm::PAGE) as usize);
                aqui
            };
            let indice = core::slice::from_raw_parts_mut(cortar(NIVELES_MAX * BLOQUE) as *mut [u8; BLOQUE], NIVELES_MAX);
            let scratch = core::slice::from_raw_parts_mut(cortar((NIVELES_MAX + 1) * BLOQUE) as *mut [u8; BLOQUE], NIVELES_MAX + 1);
            let bloque = &mut *(cortar(BLOQUE) as *mut [u8; BLOQUE]);
            let ruta = &mut *(cortar(RUTA_MAX) as *mut [u8; RUTA_MAX]);
            let lado = ANTEPASADOS_MAX * core::mem::size_of::<BlockPtr>();
            let de_a = core::slice::from_raw_parts_mut(cortar(lado) as *mut BlockPtr, ANTEPASADOS_MAX);
            let de_b = core::slice::from_raw_parts_mut(cortar(lado) as *mut BlockPtr, ANTEPASADOS_MAX);
            let tablas = core::slice::from_raw_parts_mut(cortar(niveles * core::mem::size_of::<Nivel>()) as *mut Nivel, niveles);
            Mesa { taller: Taller { niveles: tablas, scratch, indice, bloque, ruta }, de_a, de_b }
        }
    }
}

// -- La base ---------------------------------------------------------------------

/// Las tres raices de una mezcla: la BASE, la de ahora (A) y la que entra (B).
/// El recorrido es `bmo_estratos::raices`, el mismo que corre el anfitrion.
fn raices(m: &mut Mesa, ahora: BlockPtr, otra: BlockPtr) -> Result<[BlockPtr; 3], WriteError> {
    es::raices::raices(&mut DelDisco, ahora, otra, m.de_a, m.de_b, m.taller.bloque).map_err(|e| match e {
        es::raices::NoHay::NadaQueMezclar => WriteError::NadaQueMezclar,
        es::raices::NoHay::SinBase => WriteError::SinBase,
        es::raices::NoHay::SegundoPadre => {
            crate::ring0::cabina::fault("estratos", "el segundo padre no es el estrato que se apunto", ahora.lba);
            WriteError::NoSeLeeLaRaiz
        }
        es::raices::NoHay::Formato(_) => WriteError::NoSeLeeLaRaiz,
    })
}

// -- Los choques, entre las dos fases ---------------------------------------------

#[derive(Clone, Copy)]
struct Choque {
    ruta: [u8; RUTA_MAX],
    largo: u8,
    /// bit 0: A lo tiene; bit 1: B lo tiene.
    lados: u8,
    /// 0 = sin elegir; 1 = A, 2 = B, 3 = quitar.
    eleccion: u8,
}

impl Choque {
    const VACIO: Choque = Choque { ruta: [0; RUTA_MAX], largo: 0, lados: 0, eleccion: 0 };

    fn ruta(&self) -> &[u8] {
        &self.ruta[..self.largo as usize]
    }
}

/// Lo contado, esperando a que una persona elija.
#[derive(Clone, Copy)]
struct Pendiente {
    pid: u32,
    generacion: u64,
    otra: BlockPtr,
    choques: usize,
}

static mut CHOQUES: [Choque; CHOQUES_MAX] = [Choque::VACIO; CHOQUES_MAX];
static mut PENDIENTE: Option<Pendiente> = None;

fn eleccion_de(n: u8) -> Option<Eleccion> {
    match n {
        1 => Some(Eleccion::A),
        2 => Some(Eleccion::B),
        3 => Some(Eleccion::Quitar),
        _ => None,
    }
}

fn motor_dice(f: motor::Fallo) -> WriteError {
    WriteError::Motor(f)
}

/// **FASE 1: CONTAR la mezcla de la rama `nombre` en la de ahora.** No escribe
/// nada: apunta los choques para que una persona elija.
///
/// Devuelve `(choques, bloques)`. Los bloques cuentan el estrato y suponen que
/// cada choque se queda con lo de A: lo que de verdad cuesta se vuelve a contar
/// al mezclar, con lo elegido.
#[inline(never)]
pub fn contar(pid: u32, nombre: &str) -> Result<(u32, u64), WriteError> {
    let (sb, otra) = punta_de(nombre)?;
    if otra == sb.estrato {
        return Err(WriteError::NadaQueMezclar);
    }
    let (mut prestado, niveles) = prestar().ok_or(WriteError::SinMemoria)?;
    let mut m = prestado.mesa(niveles);
    let [rb, ra, rx] = raices(&mut m, sb.estrato, otra)?;

    // SAFETY: ESTRATOS se escribe desde un solo hilo (`walk::scratch_de_flujo`).
    let choques = unsafe { &mut *core::ptr::addr_of_mut!(CHOQUES) };
    let mut n = 0usize;
    let cuenta = motor::contar(&mut DelDisco, &mut m.taller, rb, ra, rx, &mut |ruta, a, b| {
        if let Some(c) = choques.get_mut(n) {
            *c = Choque::VACIO;
            c.ruta[..ruta.len()].copy_from_slice(ruta);
            c.largo = ruta.len() as u8;
            c.lados = a.is_some() as u8 | (b.is_some() as u8) << 1;
        }
        n += 1;
        Eleccion::A
    })
    .map_err(motor_dice)?;
    unsafe { PENDIENTE = Some(Pendiente { pid, generacion: sb.generation, otra, choques: n }) };
    if n > CHOQUES_MAX {
        crate::ring0::cabina::warn("estratos", "mas choques de los que caben para elegir", n as u64);
    }
    Ok((n as u32, cuenta.bloques + 1))
}

/// El superbloque y la punta de la rama `nombre`.
///
/// ** Aparte y sin `inline` A PROPOSITO: lo que lee la tabla no puede quedarse
/// en el marco de `contar`, que esta en la pila durante TODA la mezcla, debajo
/// del motor. `pila` lo midio: 48 KiB en una pila de 40.
#[inline(never)]
fn punta_de(nombre: &str) -> Result<(es::Superblock, BlockPtr), WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    if !cargar(&sb)? {
        return Err(WriteError::Rama(es::ramas::RamaError::NoEsta));
    }
    let otra = tabla().punta(nombre.as_bytes(), sb.estrato).map_err(WriteError::Rama)?;
    Ok((sb, otra))
}

fn pendiente_de(pid: u32) -> Option<Pendiente> {
    unsafe { PENDIENTE }.filter(|p| p.pid == pid)
}

/// Un trozo del choque `i` de lo contado por `pid`. `0`: no hay.
///
/// `trozo` 0 es la cabeza --`largo | lados << 8 | eleccion << 16`--, y del 1 en
/// adelante, ocho bytes de la ruta cada uno.
#[inline(never)]
pub fn choque(pid: u32, i: usize, trozo: usize) -> u64 {
    let Some(p) = pendiente_de(pid) else { return 0 };
    if i >= p.choques.min(CHOQUES_MAX) {
        return 0;
    }
    let c = unsafe { (*core::ptr::addr_of!(CHOQUES))[i] };
    if trozo == 0 {
        return c.largo as u64 | (c.lados as u64) << 8 | (c.eleccion as u64) << 16;
    }
    let desde = (trozo - 1) * 8;
    let mut b = [0u8; 8];
    for (k, x) in b.iter_mut().enumerate() {
        *x = c.ruta().get(desde + k).copied().unwrap_or(0);
    }
    u64::from_le_bytes(b)
}

/// **EL CANDADO de lo contado por `pid`**:
/// `estado | choques << 8 | elegidos << 24`.
///
/// Estado `0`: no hay nada contado. `1`, CERRADO: contado y esperando a que se
/// elija, sin haber escrito nada -- irse y volver no pierde nada. `2`, ROTO:
/// el volumen cambio desde que se conto, y hay que contar otra vez.
#[inline(never)]
pub fn candado(pid: u32) -> u64 {
    let Some(p) = pendiente_de(pid) else { return 0 };
    let vale = superbloque().is_some_and(|sb| sb.generation == p.generacion);
    let choques = unsafe { &*core::ptr::addr_of!(CHOQUES) };
    let elegidos = choques[..p.choques.min(CHOQUES_MAX)].iter().filter(|c| c.eleccion != 0).count() as u64;
    (if vale { 1 } else { 2 }) | (p.choques.min(0xFFFF) as u64) << 8 | elegidos << 24
}

/// **El nombre de la rama `i`**, de ocho en ocho. El trozo `0` es la cabeza,
/// `largo | actual << 8`; `0` si esa rama no esta (o el volumen no tiene).
///
/// Lee la tabla cada vez: UN bloque, y quien pregunta es un panel que la lee
/// al abrirse y despues de un gesto, no al repintar.
#[inline(never)]
pub fn nombre(i: usize, trozo: usize) -> u64 {
    let Some(sb) = superbloque() else { return 0 };
    if !matches!(cargar(&sb), Ok(true)) {
        return 0;
    }
    let Some((n, actual)) = tabla().rama(i) else { return 0 };
    if trozo == 0 {
        return n.len() as u64 | (actual as u64) << 8;
    }
    let mut b = [0u8; 8];
    for (k, x) in b.iter_mut().enumerate() {
        *x = n.get((trozo - 1) * 8 + k).copied().unwrap_or(0);
    }
    u64::from_le_bytes(b)
}

/// **Lo que una persona elige** para el choque `i` (1 = A, 2 = B, 3 = quitar).
#[inline(never)]
pub fn elegir(pid: u32, i: usize, eleccion: u8) -> bool {
    let Some(p) = pendiente_de(pid) else { return false };
    if i >= p.choques.min(CHOQUES_MAX) || eleccion_de(eleccion).is_none() {
        return false;
    }
    unsafe { (*core::ptr::addr_of_mut!(CHOQUES))[i].eleccion = eleccion };
    true
}

/// **FASE 2: MEZCLAR** lo contado por `pid`, con lo elegido. Publica UN
/// estrato de DOS padres (D2): la punta de ahora y la de la otra rama.
///
/// Dice que no, sin tocar un sector, si: no hay nada contado, el volumen
/// cambio desde entonces, hay choques sin elegir (D3), o al volver a contar
/// sale un choque que no se conto.
#[inline(never)]
pub fn mezclar(pid: u32) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    let p = pendiente_de(pid).ok_or(WriteError::Caducada)?;
    if p.generacion != sb.generation {
        unsafe { PENDIENTE = None };
        return Err(WriteError::Caducada);
    }
    let choques = unsafe { &*core::ptr::addr_of!(CHOQUES) };
    if p.choques > CHOQUES_MAX || choques[..p.choques].iter().any(|c| c.eleccion == 0) {
        return Err(WriteError::SinElegir);
    }
    let (mut prestado, niveles) = prestar().ok_or(WriteError::SinMemoria)?;
    let mut m = prestado.mesa(niveles);
    let [rb, ra, rx] = raices(&mut m, sb.estrato, p.otra)?;

    // Lo elegido, por la ruta. Un choque que no se conto no se adivina.
    let no_contado = core::cell::Cell::new(false);
    let mut responde = |ruta: &[u8], _: Option<&BlockPtr>, _: Option<&BlockPtr>| {
        match choques[..p.choques].iter().find(|c| c.ruta() == ruta).and_then(|c| eleccion_de(c.eleccion)) {
            Some(e) => e,
            None => {
                no_contado.set(true);
                Eleccion::Quitar
            }
        }
    };
    let cuenta = motor::contar(&mut DelDisco, &mut m.taller, rb, ra, rx, &mut responde).map_err(motor_dice)?;
    if no_contado.get() || cuenta.choques as usize != p.choques {
        return Err(WriteError::Caducada);
    }

    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok()).map_err(WriteError::Rechazada)?;
    let desde = t.reserve(cuenta.bloques + 1).map_err(WriteError::Rechazada)?;
    let hecho = motor::escribir(&mut DelDisco, &mut m.taller, rb, ra, rx, &mut responde, desde, &mut |lba, d| {
        super::escribir::poner(lba, d).is_ok()
    });
    let (raiz, cursor) = match hecho {
        Ok(r) if r.1 == desde + cuenta.bloques => r,
        Ok(_) => {
            t.abandonar();
            crate::ring0::cabina::fault("estratos", "la mezcla no escribio lo que conto", desde);
            return Err(WriteError::NoEscribio);
        }
        Err(f) => {
            t.abandonar();
            return Err(motor_dice(f));
        }
    };

    let cuando = crate::ring0::dev::clock::ahora();
    let estrato = es::Estrato::mezcla(raiz, sb.estrato, &p.otra, cuando, es::Autor::Proceso(pid), "");
    let bytes = estrato.encode();
    super::escribir::poner(cursor, &bytes)?;
    let p_estrato = BlockPtr::nuevo(cursor, 0, &bytes);

    // El mismo orden de siempre, que es el unico que no pierde datos.
    t.cerrar_datos().map_err(WriteError::Rechazada)?;
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;
    let (destino, nuevo) = t.commit(p_estrato).map_err(WriteError::Rechazada)?;
    if !write_superblock(destino, &nuevo.encode()) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }
    super::fijar_superbloque(nuevo);
    unsafe {
        super::ESTRATO_FECHA = cuando;
        PENDIENTE = None;
    }
    crate::ring0::cabina::info("estratos", "MEZCLA: un estrato de dos padres", nuevo.generation);
    Ok(nuevo.generation)
}
