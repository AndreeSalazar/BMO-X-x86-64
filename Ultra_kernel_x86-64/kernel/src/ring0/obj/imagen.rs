//! **LA DECLARACION DE IMAGEN** -- un proceso que va a cargar un `.exe` de
//! Windows y sus DLL declara TODAS sus partes de una vez, y el kernel la
//! juzga UNA vez (P0.4b de `PLAN_LAS_TRES_GRANDES`, 30-09; permiso del
//! propietario: "si al kernel").
//!
//! [carril]  ROJO      entregar memoria a Ring 3 y sellar codigo
//! [consumo] NADA      corre cuando un proceso declara, sella o muere
//!
//! generacion: nieto -- CADENA DE LLAMADAS: esta etiqueta dice cuanto SABE
//! esta pieza, no quien importa a quien (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: que es un PE, ni que hay dentro de cada parte
//!
//! === El hueco que tapa ===
//!
//! `KIND_MEMORIA` da, como mucho, OCHO bloques vivos de 64 MiB en una
//! ventana de 512 MiB (`memory.rs`). Cyberpunk son unas 55 partes (el `.exe`
//! y sus 26 DLL, codigo y datos de cada una), una de 70 MiB. Los topes de
//! `memory.rs` NO se suben: DOOM no tiene que poder pedir 500 MiB por
//! accidente. Lo que hay es otra puerta, para quien SABE lo que necesita:
//!
//! ```text
//!    DECLARAR   la tabla de partes (en un bloque propio: por la puerta no
//!               viajan punteros), juzgada por `bmo-imagen-juicio` contra la
//!               RAM LIBRE AHORA menos el margen del kernel -- dinamico, no un
//!               tope escrito --; si se concede, cada pagina se mapea (a cero,
//!               escribible, SIN ejecucion) en la ventana de imagenes
//!    PARTE(i)   donde quedo la parte `i`
//!    SELLAR(i)  una parte de CODIGO pasa a R+X sin W, irreversible (W^X, lo
//!               mismo que `MEM_OP_SELLAR`)
//!    al morir   cada pagina se desmapea, se pone a cero y vuelve al asignador
//! ```
//!
//! La burocracia se paga al DECLARAR, una vez: despues el proceso escribe y
//! ejecuta en lo suyo con `mov` y `call`, sin volver a preguntar.
//!
//! === Lo que NO hay, dicho ===
//!
//! - **Una declaracion por proceso.** Una segunda se niega: lo concedido no
//!   crece ni se amplia.
//! - **Sin fisica contigua**: ninguna parte la ve un aparato (no hay DMA ni
//!   `MEM_OP_FISICA` aqui), asi que 400 MiB no dependen de un hueco seguido.
//!   Y por lo mismo, el kernel NO escribe en estas paginas en nombre de nadie
//!   (`LEER_EN` va a un bloque de `KIND_MEMORIA`; la app copia de alli).
//! - **Ni prestar ni soltar a trozos**: la imagen vive lo que vive el proceso.
//! - Hoy corre en el BSP, el unico que corre tareas: sellar no derriba la TLB
//!   de otros nucleos. El dia que corran en varios, esto lo pide (dicho aqui
//!   para que ese dia se encuentre, como en `memory::sellar`).

use crate::ring0::mm::{self, vmm};
use bmo_imagen_juicio::{self as juicio, Limites, Motivo, Parte, Veredicto};

/// **La ventana de imagenes**: 16 GiB desde `0x10_0000_0000`, lejos de todo
/// lo de Ring 3 que esta por debajo de 4 GiB -- la imagen del `.bex`, la
/// pila, los canales, el framebuffer, los bloques (`0xE000_0000`) -- y de los
/// prestamos (`0x1_0000_0000` a `0x1_4000_0000`).
pub const VENTANA_BASE: u64 = 0x0000_0010_0000_0000;
const VENTANA_BYTES: u64 = 16 << 30;

/// Cuantas partes caben en una declaracion. Cyberpunk son ~55.
pub const MAX_PARTES: usize = 128;

/// Cuantos procesos pueden tener una imagen declarada a la vez. Un juego a
/// la vez es lo que este sistema se cree; cuatro deja sitio a un segundo
/// `.exe` y a los que mueren mientras otro declara.
const MAX_DECLARACIONES: usize = 4;

/// Donde empieza cada PE: 64 KiB, la granularidad de Windows.
const ALINEACION: u64 = 64 << 10;

/// Una parte en la tabla que la app escribe: 16 bytes (`bytes` u64, `pe`
/// u16, `codigo` u8 a 1 o 0, el resto a cero). Espejo de
/// `bmo_abi::...::IMAGEN_PARTE_BYTES`.
pub const PARTE_BYTES: u64 = 16;

// -- Los NO, con nombre (espejo de `bmo_abi::...::IMAGEN_*`) --------------

/// Este proceso ya declaro su imagen: lo concedido no crece.
pub const IMAGEN_YA_DECLARADA: u32 = 1;
/// La tabla no esta entera dentro de un bloque de este proceso.
pub const IMAGEN_SIN_TABLA: u32 = 2;
/// Mas partes de las que caben ([`MAX_PARTES`]), o ninguna.
pub const IMAGEN_DE_MAS: u32 = 3;
/// Una parte de 0 bytes (el valor dice cual).
pub const IMAGEN_PARTE_VACIA: u32 = 4;
/// Partes fuera de orden: PE 0, 1, 2... y de cada uno su codigo antes que
/// sus datos (el valor dice cual).
pub const IMAGEN_DESORDENADA: u32 = 5;
/// No hay RAM: el valor lleva lo que pide y lo que hay, en MiB
/// (`pide << 32 | hay`).
pub const IMAGEN_SIN_RAM: u32 = 6;
/// No cabe en la ventana de imagenes (el valor: `pide << 32 | hay`, MiB).
pub const IMAGEN_SIN_VENTANA: u32 = 7;
/// Ya hay [`MAX_DECLARACIONES`] procesos con imagen.
pub const IMAGEN_SIN_RANURA: u32 = 8;
/// El juez dijo que si y el asignador se quedo sin marcos a mitad (otro
/// pidio entre medias): se deshizo todo.
pub const IMAGEN_SIN_MARCOS: u32 = 9;
/// Un mapeo fallo a mitad: se deshizo todo.
pub const IMAGEN_NO_MAPEA: u32 = 10;
/// SELLAR: esa parte no existe (o no hay declaracion).
pub const IMAGEN_NO_HAY_PARTE: u32 = 11;
/// SELLAR: esa parte es de DATOS; los datos no se ejecutan nunca.
pub const IMAGEN_NO_ES_CODIGO: u32 = 12;
/// SELLAR: ya estaba sellada.
pub const IMAGEN_YA_SELLADA: u32 = 13;
/// SELLAR: `EFER.NXE` apagado; sin NX, sellar no garantiza nada.
pub const IMAGEN_SIN_NX: u32 = 14;
/// SELLAR: el remapeo fallo a mitad; la parte queda desmapeada.
pub const IMAGEN_NO_REMAPEA: u32 = 15;

#[derive(Clone, Copy)]
struct Declaracion {
    /// `0` = ranura libre.
    pid: u32,
    n: usize,
    partes: [Parte; MAX_PARTES],
    /// Un bit por parte: sellada.
    selladas: [u64; MAX_PARTES / 64],
}

const SIN_PARTE: Parte = Parte { pe: 0, codigo: false, bytes: 0 };
const LIBRE: Declaracion = Declaracion { pid: 0, n: 0, partes: [SIN_PARTE; MAX_PARTES], selladas: [0; MAX_PARTES / 64] };

static mut TABLA: [Declaracion; MAX_DECLARACIONES] = [LIBRE; MAX_DECLARACIONES];

fn tabla() -> &'static mut [Declaracion; MAX_DECLARACIONES] {
    // SAFETY: corre en syscalls (IF=0) y al morir un proceso, en el BSP: nadie
    // mas la toca a la vez, y nadie guarda la referencia.
    unsafe { &mut *core::ptr::addr_of_mut!(TABLA) }
}

fn ranura(pid: u32) -> Option<usize> {
    tabla().iter().position(|d| d.pid == pid && pid != 0)
}

/// Los limites que el juez recibe: la RAM libre de AHORA.
fn limites() -> Limites {
    let (_, libres) = mm::phys::stats();
    Limites {
        max_partes: MAX_PARTES,
        libre: libres.saturating_mul(mm::PAGE),
        margen: crate::ring0::task::admitir::MARGEN_DEL_KERNEL,
        ventana_base: VENTANA_BASE,
        ventana_bytes: VENTANA_BYTES,
        pagina: mm::PAGE,
        alineacion: ALINEACION,
    }
}

fn mib(b: u64) -> u64 {
    (b >> 20).min(0xFFFF_FFFF)
}

/// El NO del juez, como (motivo, valor).
fn motivo(m: Motivo) -> (u32, u64) {
    match m {
        Motivo::Vacia | Motivo::DeMas { .. } | Motivo::LimitesMalos => (IMAGEN_DE_MAS, 0),
        Motivo::ParteVacia { i } => (IMAGEN_PARTE_VACIA, i as u64),
        Motivo::Desordenada { i } => (IMAGEN_DESORDENADA, i as u64),
        Motivo::SinRam { pide, hay } => (IMAGEN_SIN_RAM, mib(pide) << 32 | mib(hay)),
        Motivo::SinVentana { pide, hay } => (IMAGEN_SIN_VENTANA, mib(pide) << 32 | mib(hay)),
    }
}

/// Desmapea (y devuelve a cero) las paginas de las partes `0..hasta` de
/// esta declaracion. Lo usan deshacer una declaracion a medias y morir.
fn devolver(aspace: u64, partes: &[Parte], hasta: usize) -> u64 {
    let l = limites_fijos();
    let mut devueltos = 0u64;
    for i in 0..hasta {
        let Some((va, n)) = juicio::donde(partes, &l, i) else { continue };
        let mut off = 0u64;
        while off < n {
            if let Some(marco) = vmm::unmap_page(aspace, va + off) {
                mm::phys::zero_frame(marco);
                mm::phys::free_frame(marco);
                devueltos += mm::PAGE;
            }
            off += mm::PAGE;
        }
    }
    devueltos
}

/// Los limites que NO dependen de la RAM de ahora: los que deciden el sitio
/// de cada parte (la RAM solo decide si se concede; con ella al maximo,
/// `juicio::donde` contesta lo mismo que cuando se concedio).
fn limites_fijos() -> Limites {
    Limites { libre: u64::MAX, margen: 0, ..limites() }
}

/// **DECLARAR**: la tabla de `n` partes que la app escribio en su bloque, en
/// `tabla_va`. `Ok(VENTANA_BASE)` o `Err((motivo, valor))`.
///
/// `aspace` es el del llamante: durante el syscall CR3 sigue siendo el suyo
/// (la misma nota que `memory::request`).
pub fn declarar(pid: u32, aspace: u64, tabla_va: u64, n: u64) -> Result<u64, (u32, u64)> {
    if pid == 0 {
        return Err((IMAGEN_SIN_RANURA, 0));
    }
    if ranura(pid).is_some() {
        crate::ring0::cabina::warn("imagen", "NO: este proceso ya declaro su imagen", pid as u64);
        return Err((IMAGEN_YA_DECLARADA, 0));
    }
    if n == 0 || n > MAX_PARTES as u64 {
        return Err((IMAGEN_DE_MAS, n));
    }
    let n = n as usize;
    // La ranura se toma YA (sin pid: no cuenta hasta el final) y sirve de
    // bufer: 2 KiB de partes no van a la pila de 32 KiB del syscall.
    let Some(r) = tabla().iter().position(|d| d.pid == 0 && d.n == 0) else {
        crate::ring0::cabina::warn("imagen", "NO: ya hay imagenes declaradas en todas las ranuras", MAX_DECLARACIONES as u64);
        return Err((IMAGEN_SIN_RANURA, 0));
    };
    // La tabla, leida de un bloque SUYO por su fisica (contiguo: ver
    // `memory::fisica_de`). Por la puerta no viajan punteros de Ring 3.
    let Some(fisica) = crate::ring0::obj::memory::fisica_de(pid, tabla_va, n as u64 * PARTE_BYTES) else {
        return Err((IMAGEN_SIN_TABLA, 0));
    };
    for i in 0..n {
        let v = mm::phys_to_virt(fisica + i as u64 * PARTE_BYTES);
        // SAFETY: `n * 16` bytes de un bloque de este proceso, mapeados en el
        // physmap del kernel (comprobado por `fisica_de`).
        let (bytes, pe, codigo) = unsafe { ((v as *const u64).read_unaligned(), ((v + 8) as *const u16).read_unaligned(), ((v + 10) as *const u8).read()) };
        tabla()[r].partes[i] = Parte { pe, codigo: codigo != 0, bytes };
    }
    let partes_n = &tabla()[r].partes[..n];
    let l = limites();
    let (ram, va) = match juicio::juzgar(partes_n, &l) {
        Veredicto::Concedida { ram, va } => (ram, va),
        Veredicto::Negada(m) => {
            let (mot, val) = motivo(m);
            crate::ring0::cabina::warn("imagen", "declaracion NEGADA (motivo; el valor, abajo)", mot as u64);
            crate::ring0::cabina::count("imagen", "  el valor del NO (MiB: pide << 32 | hay)", val);
            return Err((mot, val));
        }
    };
    // Mapear: cada pagina, un marco a cero, escribible y SIN ejecucion.
    let lf = limites_fijos();
    for i in 0..n {
        let Some((base, bytes)) = juicio::donde(partes_n, &lf, i) else { continue };
        let mut off = 0u64;
        while off < bytes {
            let Some(marco) = mm::phys::alloc_frame() else {
                devolver_parcial(aspace, partes_n, i, base, off);
                crate::ring0::cabina::warn("imagen", "NO: el asignador se quedo sin marcos a mitad; deshecho", ram);
                return Err((IMAGEN_SIN_MARCOS, 0));
            };
            // A CERO: el asignador no limpia al entregar (ver
            // `memory::process_died`), y esto es memoria de un proceso.
            mm::phys::zero_frame(marco);
            if vmm::map_page(aspace, base + off, marco, true, true).is_err() {
                mm::phys::free_frame(marco);
                devolver_parcial(aspace, partes_n, i, base, off);
                crate::ring0::cabina::fault("imagen", "un mapeo fallo a mitad; deshecho", base + off);
                return Err((IMAGEN_NO_MAPEA, 0));
            }
            off += mm::PAGE;
        }
    }
    let d = &mut tabla()[r];
    d.pid = pid;
    d.n = n;
    d.selladas = [0; MAX_PARTES / 64];
    crate::ring0::cabina::count("imagen", "imagen DECLARADA y concedida: partes", n as u64);
    crate::ring0::cabina::bytes("imagen", "  RAM entregada", ram);
    crate::ring0::cabina::bytes("imagen", "  VA que ocupa", va);
    Ok(VENTANA_BASE)
}

/// Deshace una declaracion a medias: las partes `0..i` enteras, y de la
/// `i`, sus primeros `off` bytes.
fn devolver_parcial(aspace: u64, partes: &[Parte], i: usize, base: u64, off: u64) {
    devolver(aspace, partes, i);
    let mut o = 0u64;
    while o < off {
        if let Some(marco) = vmm::unmap_page(aspace, base + o) {
            mm::phys::zero_frame(marco);
            mm::phys::free_frame(marco);
        }
        o += mm::PAGE;
    }
}

/// **PARTE(i)**: donde quedo la parte `i` de la imagen de `pid`.
pub fn parte(pid: u32, i: u64) -> Option<u64> {
    let d = &tabla()[ranura(pid)?];
    let i = i as usize;
    if i >= d.n {
        return None;
    }
    juicio::donde(&d.partes[..d.n], &limites_fijos(), i).map(|(va, _)| va)
}

/// **SELLAR(i)**: la parte de CODIGO `i` pasa a R+X sin W. Ver
/// `memory::sellar`: el mismo W^X, la misma irreversibilidad.
pub fn sellar(pid: u32, aspace: u64, i: u64) -> u32 {
    let Some(r) = ranura(pid) else { return IMAGEN_NO_HAY_PARTE };
    let d = &tabla()[r];
    let i = i as usize;
    if i >= d.n {
        return IMAGEN_NO_HAY_PARTE;
    }
    if !d.partes[i].codigo {
        return IMAGEN_NO_ES_CODIGO;
    }
    if d.selladas[i / 64] & (1 << (i % 64)) != 0 {
        return IMAGEN_YA_SELLADA;
    }
    if !vmm::nx_disponible() {
        crate::ring0::cabina::warn("imagen", "NO se sella: EFER.NXE apagado, W^X no se sostiene", i as u64);
        return IMAGEN_SIN_NX;
    }
    let Some((base, bytes)) = juicio::donde(&d.partes[..d.n], &limites_fijos(), i) else { return IMAGEN_NO_HAY_PARTE };
    // Primero se apunta: si el remapeo fallara a mitad, queda marcada.
    tabla()[r].selladas[i / 64] |= 1 << (i % 64);
    let mut off = 0u64;
    while off < bytes {
        let va = base + off;
        let fallo = match vmm::unmap_page(aspace, va) {
            Some(marco) => vmm::map_page_sellada(aspace, va, marco).is_err().then_some(marco),
            None => None,
        };
        if let Some(marco) = fallo {
            // A medias: el marco que no se remapeo vuelve ya; el resto de la
            // parte se desmapea (y vuelve al morir, que desmapea lo que haya).
            mm::phys::zero_frame(marco);
            mm::phys::free_frame(marco);
            let mut o = 0u64;
            while o < bytes {
                if let Some(m) = vmm::unmap_page(aspace, base + o) {
                    mm::phys::zero_frame(m);
                    mm::phys::free_frame(m);
                }
                o += mm::PAGE;
            }
            crate::ring0::cabina::fault("imagen", "sellar: el remapeo fallo; parte desmapeada", va);
            return IMAGEN_NO_REMAPEA;
        }
        off += mm::PAGE;
    }
    crate::ring0::cabina::bytes("imagen", "parte SELLADA: ahora es codigo (R+X, sin escritura)", bytes);
    0
}

/// El proceso murio: cada pagina de su imagen, desmapeada, a cero y de
/// vuelta al asignador; su ranura, libre. `aspace` es el SUYO (el que muere
/// puede no ser el que llama: ver `cap::revoke_all`).
pub fn process_died(pid: u32, aspace: u64) {
    let Some(r) = ranura(pid) else { return };
    let d = &mut tabla()[r];
    let devueltos = devolver(aspace, &d.partes[..d.n], d.n);
    d.pid = 0;
    d.n = 0;
    d.selladas = [0; MAX_PARTES / 64];
    crate::ring0::cabina::bytes("imagen", "imagen DEVUELTA al morir su proceso", devueltos);
}
