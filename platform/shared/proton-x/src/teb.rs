//! **El TEB y el PEB** (P1d, 27-09): lo que un `.exe` x64 encuentra en `gs:`.
//!
//! El compilador de Microsoft no llama a nadie para saber quien es: lee
//! `gs:[0x30]` (el propio TEB), `gs:[0x60]` (el PEB) y de ahi lo demas. El CRT
//! lo hace al arrancar; `GetLastError` es un `mov` desde el TEB. Asi que un
//! `.exe` de verdad no corre sin un TEB en su GS, y PROTON-X se lo da.
//!
//! Aqui solo va la FORMA: los desplazamientos de Windows x64 y los bytes que
//! se escriben. Quien carga (la app de Ring 3, o el banco del anfitrion) pone
//! la memoria y el GS. Se rellena lo que un `.exe` lee y se sabe que es
//! verdad; lo demas queda a cero, como en un TEB recien creado.
//!
//! ```text
//!    TEB (x64)                          PEB (x64)
//!    0x008  StackBase    (tope)         0x002  BeingDebugged       0
//!    0x010  StackLimit   (fondo)        0x010  ImageBaseAddress    la base
//!    0x030  Self         el TEB                                    del .exe
//!    0x040  ClientId.UniqueProcess
//!    0x048  ClientId.UniqueThread
//!    0x060  ProcessEnvironmentBlock
//!    0x068  LastErrorValue (u32)
//! ```
//!
//! Los desplazamientos son los de `winternl.h` / `ntddk` para x64, los mismos
//! que usa Wine: estan fijos desde Windows XP x64 porque el codigo compilado
//! los lleva dentro.

/// Lo que se reserva para cada uno. El TEB de x64 llega a ~0x1838 (las ranuras
/// de TLS en 0x1480 y 0x1780); el PEB a ~0x7C8. Dos paginas y una.
pub const TEB_BYTES: usize = 0x2000;
pub const PEB_BYTES: usize = 0x1000;

pub const TEB_STACK_BASE: usize = 0x08;
pub const TEB_STACK_LIMIT: usize = 0x10;
pub const TEB_SELF: usize = 0x30;
pub const TEB_PROCESS_ID: usize = 0x40;
pub const TEB_THREAD_ID: usize = 0x48;
pub const TEB_PEB: usize = 0x60;
pub const TEB_LAST_ERROR: usize = 0x68;

pub const PEB_BEING_DEBUGGED: usize = 0x02;
pub const PEB_IMAGE_BASE: usize = 0x10;

/// Lo que se sabe de ESTE hilo y de ESTE proceso al arrancar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hilo {
    /// Donde va a vivir el TEB (lo que se pone en el GS).
    pub teb: u64,
    /// Donde va a vivir el PEB.
    pub peb: u64,
    /// La pila del hilo: `pila_tope` es la direccion mas alta (StackBase) y
    /// `pila_fondo` la mas baja (StackLimit), como en Windows.
    pub pila_tope: u64,
    pub pila_fondo: u64,
    pub proceso: u64,
    pub hilo: u64,
    /// La base a la que se coloco el `.exe`: lo que `GetModuleHandle(NULL)`
    /// devuelve, leido del PEB.
    pub base_imagen: u64,
}

fn pon(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

/// **Escribir el TEB** en `teb` (que mide al menos [`TEB_BYTES`]; lo demas, a
/// cero). `LastErrorValue` empieza en 0.
pub fn escribir_teb(teb: &mut [u8], h: &Hilo) {
    teb[..TEB_BYTES].fill(0);
    pon(teb, TEB_STACK_BASE, h.pila_tope);
    pon(teb, TEB_STACK_LIMIT, h.pila_fondo);
    pon(teb, TEB_SELF, h.teb);
    pon(teb, TEB_PROCESS_ID, h.proceso);
    pon(teb, TEB_THREAD_ID, h.hilo);
    pon(teb, TEB_PEB, h.peb);
}

/// **Escribir el PEB** en `peb` (que mide al menos [`PEB_BYTES`]). Nadie
/// depura: `BeingDebugged` a 0.
pub fn escribir_peb(peb: &mut [u8], h: &Hilo) {
    peb[..PEB_BYTES].fill(0);
    pon(peb, PEB_IMAGE_BASE, h.base_imagen);
}
