//! **LA RESERVA** -- la memoria que un juego pide EN MARCHA (`VirtualAlloc`,
//! sus montones: varios GiB), P0.4c de `PLAN_LAS_TRES_GRANDES` (30-09;
//! permiso del propietario: "si al kernel", y "completar TODO").
//!
//! [carril]  ROJO      entregar memoria a Ring 3
//! [consumo] NADA      corre cuando un proceso hace o deshace paginas
//!
//! generacion: nieto -- CADENA DE LLAMADAS: esta etiqueta dice cuanto SABE
//! esta pieza, no quien importa a quien (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: que es VirtualAlloc, ni que region reservo la app
//!
//! === El hueco que tapa ===
//!
//! Cyberpunk, en el metal, salto a su entrada y lo primero grande que hizo
//! fue `GlobalMemoryStatus` + `VirtualAlloc` de mas de 64 MiB. `KIND_MEMORIA`
//! da OCHO bloques de 64 MiB (`memory.rs`) y esos topes NO se suben: DOOM no
//! tiene que poder pedir 4 GiB por accidente. Esta es otra puerta, y es la
//! mas sencilla que cabe:
//!
//! ```text
//!    HACER(va, bytes)      en la VENTANA DE RESERVA del proceso, las paginas
//!                          de [va, va+bytes) que no estaban: cada una un
//!                          marco a cero, escribible y SIN ejecucion. Juzgado
//!                          (`bmo-imagen-juicio::reserva`) contra la RAM LIBRE
//!                          AHORA menos el margen del kernel: dinamico
//!    DESHACER(va, bytes)   las que estaban: desmapeadas, a cero, de vuelta
//!    al morir              NADA que hacer aqui: cada pagina va marcada como
//!                          del espacio (`PTE_NUESTRA`) y `destroy_address_space`
//!                          la pone a cero y la devuelve; solo se borra la cuenta
//! ```
//!
//! La cuenta de Windows (que esta reservado, que hecho, con que proteccion)
//! la lleva la casa en Ring 3: reservar es solo elegir DIRECCIONES dentro de
//! la ventana, y eso no le cuesta nada al kernel.
//!
//! === Lo que NO hay, dicho ===
//!
//! - **No mas de 64 MiB por llamada** (`MAX_POR_VEZ`): el kernel pone a cero
//!   cada pagina con las interrupciones cerradas; la casa parte lo grande.
//! - **Si el asignador se queda sin marcos A MITAD** (otro pidio entre medias
//!   del juicio y el mapeo), lo hecho hasta ahi SE QUEDA hecho y se dice
//!   cuanto: es memoria del proceso, marcada, que vuelve al deshacer o al
//!   morir. No es "todo o nada" como Windows; es contarlo bien.
//! - **Sin ejecucion, nunca**: el codigo que el juego genere en marcha (JIT)
//!   no sale de aqui. W^X no se negocia.
//! - Hoy corre en el BSP, el unico que corre tareas: deshacer no derriba la
//!   TLB de otros nucleos (la misma nota que `imagen.rs` y `memory::sellar`).

use crate::ring0::mm::{self, vmm};
use bmo_imagen_juicio::reserva::{self as juicio, LimitesReserva, NoReserva};

/// **La ventana de reserva**: 128 GiB desde `0x20_0000_0000`, detras de la
/// de imagenes (`0x10_0000_0000`, 16 GiB) y dentro de `PML4[0]`, que es lo
/// que `destroy_address_space` recorre al morir.
pub const VENTANA_BASE: u64 = 0x0000_0020_0000_0000;
pub const VENTANA_BYTES: u64 = 128 << 30;

/// Lo mas que se hace o deshace en una llamada.
pub const MAX_POR_VEZ: u64 = 64 << 20;

/// Cuantos procesos llevan cuenta a la vez (la cuenta es solo para CABINA:
/// lo que protege es el juez, pagina a pagina).
const MAX_CUENTAS: usize = 8;

// -- Los NO, con nombre (espejo de `bmo_abi::...::RESERVA_*`) --------------

/// 0 bytes, o sin alinear a pagina.
pub const RESERVA_RANGO_MALO: u32 = 1;
/// Fuera de la ventana de reserva.
pub const RESERVA_FUERA: u32 = 2;
/// Mas de [`MAX_POR_VEZ`] de una vez (el valor: lo mas, en bytes).
pub const RESERVA_DE_MAS: u32 = 3;
/// No hay RAM: el valor lleva lo que pide y lo que hay, en MiB
/// (`pide << 32 | hay`).
pub const RESERVA_SIN_RAM: u32 = 4;
/// El asignador se quedo sin marcos a mitad: el valor dice cuantos bytes SI
/// se hicieron (y se quedan hechos).
pub const RESERVA_SIN_MARCOS: u32 = 5;
/// Un mapeo fallo a mitad: el valor, cuantos bytes si se hicieron.
pub const RESERVA_NO_MAPEA: u32 = 6;

#[derive(Clone, Copy)]
struct Cuenta {
    pid: u32,
    bytes: u64,
}

static mut CUENTAS: [Cuenta; MAX_CUENTAS] = [Cuenta { pid: 0, bytes: 0 }; MAX_CUENTAS];

fn cuentas() -> &'static mut [Cuenta; MAX_CUENTAS] {
    // SAFETY: corre en syscalls (IF=0) y al morir un proceso, en el BSP: nadie
    // mas la toca a la vez, y nadie guarda la referencia.
    unsafe { &mut *core::ptr::addr_of_mut!(CUENTAS) }
}

fn sumar(pid: u32, bytes: u64) {
    let c = cuentas();
    if let Some(x) = c.iter_mut().find(|x| x.pid == pid) {
        x.bytes = x.bytes.saturating_add(bytes);
    } else if let Some(x) = c.iter_mut().find(|x| x.pid == 0) {
        *x = Cuenta { pid, bytes };
    }
}

fn restar(pid: u32, bytes: u64) {
    if let Some(x) = cuentas().iter_mut().find(|x| x.pid == pid) {
        x.bytes = x.bytes.saturating_sub(bytes);
    }
}

/// Los limites que el juez recibe: la RAM libre de AHORA.
fn limites() -> LimitesReserva {
    let (_, libres) = mm::phys::stats();
    LimitesReserva {
        ventana_base: VENTANA_BASE,
        ventana_bytes: VENTANA_BYTES,
        pagina: mm::PAGE,
        max_por_vez: MAX_POR_VEZ,
        libre: libres.saturating_mul(mm::PAGE),
        margen: crate::ring0::task::admitir::MARGEN_DEL_KERNEL,
    }
}

fn mib(b: u64) -> u64 {
    (b >> 20).min(0xFFFF_FFFF)
}

/// El NO del juez, como (motivo, valor).
fn motivo(m: NoReserva) -> (u32, u64) {
    match m {
        NoReserva::Vacia | NoReserva::Desalineada | NoReserva::LimitesMalos => (RESERVA_RANGO_MALO, 0),
        NoReserva::FueraDeVentana => (RESERVA_FUERA, 0),
        NoReserva::DeMasDeUnaVez { max, .. } => (RESERVA_DE_MAS, max),
        NoReserva::SinRam { pide, hay } => (RESERVA_SIN_RAM, mib(pide) << 32 | mib(hay)),
    }
}

/// **HACER**: las paginas de `[va, va + bytes)` que faltan. `Ok(bytes
/// nuevos)` o `Err((motivo, valor))`. `aspace` es el del llamante: durante
/// el syscall CR3 sigue siendo el suyo (la misma nota que `memory::request`).
pub fn hacer(pid: u32, aspace: u64, va: u64, bytes: u64) -> Result<u64, (u32, u64)> {
    let l = limites();
    let paginas = juicio::rango(va, bytes, &l).map_err(motivo)?;
    // Lo que falta: las que ya estan no se tocan (ni se ponen a cero: son
    // del proceso y tienen lo suyo).
    let nuevas = (0..paginas).filter(|k| vmm::translate(aspace, va + k * mm::PAGE).is_none()).count() as u64;
    if let Err(m) = juicio::ram(nuevas, &l) {
        let (mot, val) = motivo(m);
        crate::ring0::cabina::warn("reserva", "HACER negado: no hay RAM (MiB: pide << 32 | hay)", val);
        return Err((mot, val));
    }
    let mut hechas = 0u64;
    for k in 0..paginas {
        let p = va + k * mm::PAGE;
        if vmm::translate(aspace, p).is_some() {
            continue;
        }
        let Some(marco) = mm::phys::alloc_frame() else {
            sumar(pid, hechas * mm::PAGE);
            crate::ring0::cabina::warn("reserva", "HACER: sin marcos a mitad; lo hecho se queda (bytes)", hechas * mm::PAGE);
            return Err((RESERVA_SIN_MARCOS, hechas * mm::PAGE));
        };
        // A CERO: el asignador no limpia al entregar (ver `memory::process_died`).
        mm::phys::zero_frame(marco);
        // PROPIA: el espacio la devuelve (a cero) al morir, sin cuenta aparte.
        if vmm::map_page_propia(aspace, p, marco, true, true).is_err() {
            mm::phys::free_frame(marco);
            sumar(pid, hechas * mm::PAGE);
            crate::ring0::cabina::fault("reserva", "HACER: un mapeo fallo a mitad", p);
            return Err((RESERVA_NO_MAPEA, hechas * mm::PAGE));
        }
        hechas += 1;
    }
    sumar(pid, hechas * mm::PAGE);
    Ok(hechas * mm::PAGE)
}

/// **DESHACER**: las paginas de `[va, va + bytes)` que estaban, fuera, a
/// cero y de vuelta al asignador. `Ok(bytes devueltos)`.
pub fn deshacer(pid: u32, aspace: u64, va: u64, bytes: u64) -> Result<u64, (u32, u64)> {
    let paginas = juicio::rango(va, bytes, &limites()).map_err(motivo)?;
    let mut devueltos = 0u64;
    for k in 0..paginas {
        if let Some(marco) = vmm::unmap_page(aspace, va + k * mm::PAGE) {
            mm::phys::zero_frame(marco);
            mm::phys::free_frame(marco);
            devueltos += mm::PAGE;
        }
    }
    restar(pid, devueltos);
    Ok(devueltos)
}

/// Lo que tiene hecho `pid` ahora (para CABINA y el informe).
pub fn hechos(pid: u32) -> u64 {
    cuentas().iter().find(|x| x.pid == pid).map_or(0, |x| x.bytes)
}

/// El proceso murio: sus paginas las devuelve `destroy_address_space` (van
/// marcadas `PTE_NUESTRA`); aqui solo se borra su cuenta.
pub fn process_died(pid: u32) {
    if let Some(x) = cuentas().iter_mut().find(|x| x.pid == pid) {
        if x.bytes > 0 {
            crate::ring0::cabina::bytes("reserva", "al morir, lo hecho vuelve con su espacio", x.bytes);
        }
        *x = Cuenta { pid: 0, bytes: 0 };
    }
}
