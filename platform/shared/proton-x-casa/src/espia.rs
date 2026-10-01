//! **EL ESPIA DE GALAXY** -- por que sale Cyberpunk, dicho por Galaxy (01-10).
//!
//! Con el diario encendido, dos exportaciones de `REDGalaxy64.dll` que el
//! `.exe` importa se le dan ENVUELTAS: `redgalaxy::api::Init` (se apunta que
//! volvio) y `redgalaxy::api::GetError` (si devuelve un error, se apunta su
//! nombre, su mensaje y su tipo). La IA local vio que el juego llama a Init y
//! luego a GetError, y que un GetError no nulo lo hace salir con
//! `quick_exit(0)`: esto convierte "sale con 0" en una frase de Galaxy.
//!
//! `IError` (SDK de Galaxy): `~IError`, `GetName`, `GetMsg` y `GetType`, en ese
//! orden en la vtable de MSVC. Sin el diario, nada se envuelve.

use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::{diario, dir};

static INIT: AtomicU64 = AtomicU64::new(0);
static GET_ERROR: AtomicU64 = AtomicU64::new(0);

/// Lo que el `.exe` recibe para la exportacion `nombre` de la DLL propia
/// `dll` (direccion `d`): la misma, o el espia.
pub(crate) fn envolver(dll: &str, nombre: &str, d: u64) -> u64 {
    if !diario::encendido() || !bmo_proton_x::dll::fichero(dll).eq_ignore_ascii_case("redgalaxy64.dll") {
        return d;
    }
    if nombre.contains("Init@api@redgalaxy@@") {
        INIT.store(d, Ordering::Relaxed);
        return dir!(init);
    }
    if nombre.contains("GetError@api@redgalaxy@@") {
        GET_ERROR.store(d, Ordering::Relaxed);
        return dir!(get_error);
    }
    d
}

extern "win64" fn init(opciones: u64) {
    // SAFETY: la exportacion de verdad, `void Init(const InitOptions&)`.
    let f: extern "win64" fn(u64) = unsafe { core::mem::transmute(INIT.load(Ordering::Relaxed)) };
    diario::nota("galaxy: el .exe llama a redgalaxy::api::Init");
    f(opciones);
    diario::nota("galaxy: redgalaxy::api::Init volvio");
}

/// Una cadena C de Galaxy, hasta 160 bytes.
fn cadena(p: u64) -> String {
    if p == 0 {
        return String::from("(nula)");
    }
    let b = crate::crt::cadena_c(p);
    String::from_utf8_lossy(&b[..b.len().min(160)]).into_owned()
}

extern "win64" fn get_error() -> u64 {
    // SAFETY: la exportacion de verdad, `const IError* GetError()`.
    let f: extern "win64" fn() -> u64 = unsafe { core::mem::transmute(GET_ERROR.load(Ordering::Relaxed)) };
    let e = f();
    if e == 0 {
        diario::nota("galaxy: GetError -> ninguno");
        return e;
    }
    // SAFETY: un IError vivo de Galaxy: su vtable de MSVC, slots 1..3.
    let (nombre, msg, tipo) = unsafe {
        let vt = *(e as *const u64);
        let m = |k: u64| *((vt + 8 * k) as *const u64);
        let texto: extern "win64" fn(u64) -> u64 = core::mem::transmute(m(1));
        let msg: extern "win64" fn(u64) -> u64 = core::mem::transmute(m(2));
        let tipo: extern "win64" fn(u64) -> u32 = core::mem::transmute(m(3));
        (cadena(texto(e)), cadena(msg(e)), tipo(e))
    };
    diario::nota(&alloc::format!("galaxy: GetError -> {nombre}: \"{msg}\" (tipo {tipo})"));
    e
}
