//! Entorno que PROTON-X prepara para el hilo principal de un `.exe` Windows.
//!
//! TEB/PEB y pila pertenecen juntos: el TEB describe tanto la pila inicial
//! como la pila mayor reservada desde la cabecera PE del ejecutable.

use super::{bmo, di, fin};
use alloc::format;
use bmo_proton_x::teb;

/// Lo menos y lo mas que se le da al hilo principal del `.exe` (lo que pide
/// su cabecera, entre estos dos): Windows reserva lo que pide y lo hace a
/// medida; aqui se hace entero, asi que se pone techo.
const PILA_EXE_MIN: u64 = 1 << 20;
const PILA_EXE_MAX: u64 = 32 << 20;

/// Pide la pila segun SizeOfStackReserve y actualiza StackBase/StackLimit.
pub(super) fn pila_del_exe(base: u64) -> u64 {
    // SAFETY: la cabecera PE de la imagen colocada en `base` (mapeada).
    let pide = unsafe {
        let nt = base + ((base + 0x3C) as *const u32).read_unaligned() as u64;
        ((nt + 0x18 + 0x48) as *const u64).read_unaligned()
    };
    let bytes = pide.clamp(PILA_EXE_MIN, PILA_EXE_MAX);
    let Some(fondo) = bmo_proton_x_casa::memoria::pila_principal(bytes) else {
        fin(&format!("no hay {} MiB para la pila del .exe", bytes >> 20));
    };
    let tope = fondo + bytes;
    let teb: u64;
    // SAFETY: leer gs:[0x30], el TEB que puso `poner_teb`.
    unsafe {
        core::arch::asm!("mov {}, gs:[0x30]", out(reg) teb, options(nostack, preserves_flags))
    };
    // SAFETY: el TEB de este hilo, nuestro y R+W.
    unsafe {
        ((teb + teb::TEB_STACK_BASE as u64) as *mut u64).write(tope);
        ((teb + teb::TEB_STACK_LIMIT as u64) as *mut u64).write(fondo);
    }
    di(&format!(
        "PROTON-X: la pila del .exe: {} MiB (pide {} KiB) en {fondo:#x}..{tope:#x}\n",
        bytes >> 20,
        pide >> 10
    ));
    tope
}

/// La pila de Ring 3 de BMO-X. Espejo de `USER_STACK_TOP` y `USER_STACK_SIZE`
/// (`Ultra_kernel_x86-64/kernel/src/ring0/mm/vmm/verde.rs`).
const PILA_TOPE: u64 = 0x8000_0000;
const PILA_BYTES: u64 = 0x1_0000;

/// Construye el TEB/PEB en el monton y hace que GS apunte al TEB.
pub(super) fn poner_teb(base: u64) {
    let rsp: u64;
    // SAFETY: leer un registro.
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags))
    };
    let fondo = PILA_TOPE - PILA_BYTES;
    if !(fondo < rsp && rsp <= PILA_TOPE) {
        fin(&format!("la pila no esta donde el kernel la pone ({rsp:#x} fuera de {fondo:#x}..{PILA_TOPE:#x}): el TEB mentiria"));
    }
    let bytes = teb::TEB_BYTES + teb::PEB_BYTES;
    let Ok(forma) = core::alloc::Layout::from_size_align(bytes, 4096) else {
        fin("la forma del TEB")
    };
    // SAFETY: `forma` no mide cero.
    let mem = unsafe { alloc::alloc::alloc_zeroed(forma) };
    if mem.is_null() {
        fin("sin monton para el TEB y el PEB");
    }
    let h = teb::Hilo {
        teb: mem as u64,
        peb: mem as u64 + teb::TEB_BYTES as u64,
        pila_tope: PILA_TOPE,
        pila_fondo: fondo,
        proceso: bmo::pid(),
        hilo: bmo::tid(),
        base_imagen: base,
    };
    // SAFETY: `bytes` recien pedidos al monton, nuestros y vivos para siempre.
    let t = unsafe { core::slice::from_raw_parts_mut(mem, bytes) };
    let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
    teb::escribir_teb(tb, &h);
    teb::escribir_peb(pb, &h);
    let ciclos = bmo::poner_gs(h.teb).unwrap_or_else(|c| {
        fin(&format!(
            "el kernel no pone el GS (codigo {c}): un .exe de Windows no encontraria su TEB"
        ))
    });
    let otra = bmo::poner_gs(h.teb).unwrap_or(u64::MAX);
    di(&format!(
        "PROTON-X: TEB en {:#x}, PEB en {:#x}; GS -> TEB: el wrmsr costo {} ciclos (el mismo otra vez: {}, no se toca)\n",
        h.teb, h.peb, ciclos, otra
    ));
}
