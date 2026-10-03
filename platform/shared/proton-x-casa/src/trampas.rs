//! **Las trampas con nombre** (03-10): lo que GetProcAddress da cuando la
//! casa tiene la DLL pero no esa funcion.
//!
//! [carril]  VERDE     no toca la maquina: contesta, avisa y devuelve 0
//! [cuesta]  DATO      una trampa que devuelve 0 donde la funcion de verdad
//!                     devolvia un HRESULT dice S_OK sin haber hecho nada
//! [riesgo]  SILENCIO  justo eso: el `.exe` sigue con lo que no se hizo. Por
//!                     eso cada trampa se DICE la primera vez que se llama
//! [consumo] NADA      solo cuando el `.exe` llama a una
//!
//! # Por que
//!
//! En el metal (03-10, dos corridas iguales) Cyberpunk murio a los ~11 s
//! llamando a la DIRECCION 0 desde `Cyberpunk2077.exe+0x1d4c6cf`, con
//! `rcx = 0`. La casa le habia dado dos NULL por GetProcAddress
//! (`crypt32!CryptMsgClose`, `iphlpapi!if_nametoindex`); en Windows las dos
//! existen, asi que el juego no miro el NULL. Una llamada a 0 no pasa por el
//! diario ni por nada: la autopsia solo dice "puntero nulo".
//!
//! # La regla
//!
//! ```text
//!    del sistema   kernel32, kernelbase, ntdll, api-ms-*, ext-ms-*: sigue
//!                  dando NULL. Ahi preguntar "esta?" es lo normal (la `std`
//!                  de Rust mira asi lo de Windows 8 y 10) y el NULL ES la
//!                  respuesta
//!    lo demas      una TRAMPA: la primera vez que se llama dice
//!                  "dll!funcion, que la casa no tiene" y devuelve 0; cada
//!                  nombre tiene la suya (GetProcAddress dos veces da lo mismo)
//!    sin sitio     pasadas las [`TRAMPAS`], NULL, y se dice
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::aviso;

/// Cuantas trampas distintas puede haber en un proceso.
pub(crate) const TRAMPAS: usize = 64;

struct Estado {
    /// `dll!funcion` de cada trampa dada, en orden.
    nombres: Vec<String>,
    /// Si ya se dijo que la llamaron.
    dichas: [bool; TRAMPAS],
}

struct Global(UnsafeCell<Estado>);
// SAFETY: la casa corre en un hilo a la vez (ver `Global` en lib.rs).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { nombres: Vec::new(), dichas: [false; TRAMPAS] }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.nombres.clear();
    e.dichas = [false; TRAMPAS];
}

/// Si `dll` es del sistema: ahi un NULL es la respuesta que se espera.
pub(crate) fn es_del_sistema(dll: &str) -> bool {
    let d = dll.to_ascii_lowercase();
    d.starts_with("kernel32") || d.starts_with("kernelbase") || d.starts_with("ntdll") || d.starts_with("api-ms-") || d.starts_with("ext-ms-")
}

/// **La trampa de `dll!nombre`**: la misma si ya se dio; una nueva si no;
/// `None` sin sitio.
pub(crate) fn trampa(dll: &str, nombre: &str) -> Option<u64> {
    let e = estado();
    let clave = format!("{}!{nombre}", dll.to_ascii_lowercase());
    let i = match e.nombres.iter().position(|n| *n == clave) {
        Some(i) => i,
        None if e.nombres.len() < TRAMPAS => {
            e.nombres.push(clave);
            e.nombres.len() - 1
        }
        None => {
            aviso(&format!("GetProcAddress({dll}, \"{nombre}\"): sin sitio para otra trampa ({TRAMPAS}); el .exe recibe NULL"));
            return None;
        }
    };
    Some(DIRECCIONES[i] as usize as u64)
}

/// Lo que hace cada trampa al llamarla: decirlo (una vez) y devolver 0.
fn salto(i: usize) -> u64 {
    let e = estado();
    if i < TRAMPAS && !e.dichas[i] {
        e.dichas[i] = true;
        let n = e.nombres.get(i).cloned().unwrap_or_default();
        aviso(&format!("el .exe LLAMO a {n}, que la casa no tiene: devuelve 0 (si era un HRESULT, eso es S_OK sin hacer nada)"));
    }
    0
}

extern "win64" fn una<const I: usize>() -> u64 {
    salto(I)
}

macro_rules! direcciones {
    ($($i:literal)*) => { [$(una::<$i> as extern "win64" fn() -> u64),*] };
}

/// Una funcion por trampa: cada una sabe su numero, y por el, su nombre.
static DIRECCIONES: [extern "win64" fn() -> u64; TRAMPAS] = direcciones!(
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
    32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63
);
