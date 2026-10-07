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

/// Cuantas trampas distintas puede haber en un proceso. 256 desde el 04-10:
/// las DLL del juego cargadas en vivo reciben una por cada importacion que
/// la casa no tiene (`tabla_o_trampa`).
pub(crate) const TRAMPAS: usize = 256;

struct Estado {
    /// `dll!funcion` de cada trampa dada, en orden.
    nombres: Vec<String>,
    /// Si ya se dijo que la llamaron.
    dichas: [bool; TRAMPAS],
}

struct Global(UnsafeCell<Estado>);
// SAFETY: la casa corre en un hilo a la vez (ver `Global` en lib.rs).
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
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

/// Las excepciones del cargador RETRASADO de MSVC (`/DELAYLOAD`):
/// `VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND)` y la de
/// `ERROR_PROC_NOT_FOUND`.
pub(crate) const RETRASADA_SIN_DLL: u32 = 0xC06D_007E;
pub(crate) const RETRASADA_SIN_FUNCION: u32 = 0xC06D_007F;

/// **Una importacion RETRASADA que no esta** (03-10, la sexta corrida). El
/// cargador retrasado del `.exe` (`__delayLoadHelper2`) pide la DLL y la
/// funcion a la casa; si no estan, lanza una de las dos excepciones de
/// arriba con su `DelayLoadInfo` y, SI ALGUIEN LA CONTINUA, salta a lo que
/// haya en `pfnCur` -- 0: el `.exe` llama a la direccion 0 (Cyberpunk, dos
/// veces en el mismo sitio, `call [rip+..]` desde `+0x1d4c6cf`).
///
/// Antes de despachar se pone en `pfnCur` la TRAMPA de `dll!funcion` y se
/// dice: si se continua, el `.exe` llama a la trampa (que lo dice y da 0),
/// no a la nada; si la coge un `__except`, no cambia nada.
///
/// `DelayLoadInfo` (x64): cb +0, pidd +8, ppfn +16, szDll +24, dlp
/// (fImportByName +32, nombre u ordinal +40), hmodCur +48, pfnCur +56.
pub(crate) fn retrasada(codigo: u32, parametros: &[u64]) {
    if codigo != RETRASADA_SIN_DLL && codigo != RETRASADA_SIN_FUNCION {
        return;
    }
    let Some(&dli) = parametros.first().filter(|&&p| p >= 0x1_0000) else { return };
    // SAFETY: el `DelayLoadInfo` que el cargador retrasado del `.exe` paso
    // como primer argumento de su excepcion (vive en su pila).
    let (dll, por_nombre, quien) = unsafe { ((dli as *const u64).add(3).read(), ((dli + 32) as *const u32).read(), ((dli + 40) as *const u64).read()) };
    let texto = |p: u64| if p >= 0x1_0000 { String::from_utf8_lossy(&crate::crt::cadena_c(p)).into_owned() } else { String::from("?") };
    let dll = texto(dll);
    let nombre = if por_nombre != 0 { texto(quien) } else { format!("#{}", quien as u16) };
    let t = trampa(&dll, &nombre).unwrap_or(0);
    // SAFETY: `pfnCur` del mismo `DelayLoadInfo`, que el `.exe` lee al volver.
    unsafe { ((dli + 56) as *mut u64).write(t) };
    let que = if codigo == RETRASADA_SIN_DLL { "la DLL no la tiene la casa" } else { "la casa no tiene esa funcion" };
    aviso(&format!("importacion RETRASADA {dll}!{nombre}: {que}; si el .exe sigue, llama a una TRAMPA (no a la direccion 0)"));
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
    64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95
    96 97 98 99 100 101 102 103 104 105 106 107 108 109 110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125 126 127
    128 129 130 131 132 133 134 135 136 137 138 139 140 141 142 143 144 145 146 147 148 149 150 151 152 153 154 155 156 157 158 159
    160 161 162 163 164 165 166 167 168 169 170 171 172 173 174 175 176 177 178 179 180 181 182 183 184 185 186 187 188 189 190 191
    192 193 194 195 196 197 198 199 200 201 202 203 204 205 206 207 208 209 210 211 212 213 214 215 216 217 218 219 220 221 222 223
    224 225 226 227 228 229 230 231 232 233 234 235 236 237 238 239 240 241 242 243 244 245 246 247 248 249 250 251 252 253 254 255
);

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un `DelayLoadInfo` como el del cargador retrasado de MSVC: la
    /// excepcion de "no esta" deja en `pfnCur` la trampa de `dll!funcion`
    /// (antes, 0: el `.exe` saltaba a la direccion 0), y la trampa da 0.
    #[test]
    fn una_retrasada_que_no_esta_deja_una_trampa_y_no_un_nulo() {
        let (dll, funcion) = (b"WTSAPI32.dll\0", b"WTSRegisterSessionNotification\0");
        let mut dli = [0u64; 9];
        dli[0] = 72;
        dli[3] = dll.as_ptr() as u64;
        dli[4] = 1; // fImportByName
        dli[5] = funcion.as_ptr() as u64;
        let p = dli.as_mut_ptr() as u64;
        retrasada(RETRASADA_SIN_DLL, &[p]);
        let t = dli[7];
        assert_ne!(t, 0, "pfnCur ya no es nulo");
        assert_eq!(Some(t), trampa("wtsapi32.dll", "WTSRegisterSessionNotification"), "la misma trampa que daria GetProcAddress");
        // SAFETY: una trampa de la casa: u64 f().
        assert_eq!(unsafe { core::mem::transmute::<u64, extern "win64" fn() -> u64>(t) }(), 0);
        // Otra excepcion cualquiera no toca nada.
        let mut otro = [0u64; 9];
        otro[3] = dll.as_ptr() as u64;
        retrasada(0xE06D_7363, &[otro.as_mut_ptr() as u64]);
        assert_eq!(otro[7], 0);
        // Por ordinal.
        let mut ord = [0u64; 9];
        ord[3] = dll.as_ptr() as u64;
        ord[5] = 7;
        retrasada(RETRASADA_SIN_FUNCION, &[ord.as_mut_ptr() as u64]);
        assert_eq!(Some(ord[7]), trampa("WTSAPI32.dll", "#7"));
    }
}
