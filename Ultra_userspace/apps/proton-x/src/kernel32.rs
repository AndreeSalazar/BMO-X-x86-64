//! **El `kernel32` de la casa** (P1b, 27-09): la primera fila de LA TABLA.
//!
//! Lo que un `.exe` importa de `kernel32.dll`, escrito aqui en Rust como
//! `extern "win64"`: el compilador pone la convencion de Windows (rcx, rdx,
//! r8, r9 y 32 bytes de sombra) sin una linea de ensamblador. Y debajo, la
//! puerta de BMO-X: la consola y la salida del proceso.
//!
//! Hoy son las tres de `hola.exe`. Una funcion que no esta en [`buscar`] NO
//! se rellena con un stub: el cargador dice TODAS las que faltan, con su DLL,
//! y el `.exe` no arranca (PLAN_PROTON_X, seccion 5).

use bmo_proton_x::Funcion;
use bmo_userland as bmo;

/// `GetStdHandle(STD_OUTPUT_HANDLE)` y `(STD_ERROR_HANDLE)`: los dos van a la
/// consola de quien nos lanzo. Valores que no son punteros ni se confunden
/// con `INVALID_HANDLE_VALUE` (-1).
const SALIDA: u64 = 0x5A1D_0001;
const ERRORES: u64 = 0x5A1D_0002;
const NO_VALE: u64 = u64::MAX;

extern "win64" fn get_std_handle(n: u32) -> u64 {
    match n as i32 {
        -11 => SALIDA,
        -12 => ERRORES,
        _ => NO_VALE,
    }
}

/// `WriteFile` sobre la consola. Un `\r` delante de `\n` no se manda: la
/// consola de BMO-X es de lineas y el retorno de carro de Windows pintaria
/// un caracter de mas. Se cuenta como escrito: para el `.exe`, lo es.
extern "win64" fn write_file(h: u64, b: *const u8, n: u32, escritos: *mut u32, _solapado: u64) -> i32 {
    if h != SALIDA && h != ERRORES {
        return 0;
    }
    // SAFETY: el `.exe` promete `n` bytes legibles en `b`, como en Windows.
    let bytes = unsafe { core::slice::from_raw_parts(b, n as usize) };
    let mut desde = 0;
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            decir(&bytes[desde..i]);
            desde = i + 1;
        }
    }
    decir(&bytes[desde..]);
    if !escritos.is_null() {
        // SAFETY: como arriba, un DWORD suyo.
        unsafe { *escritos = n };
    }
    1
}

fn decir(b: &[u8]) {
    match core::str::from_utf8(b) {
        Ok(s) => bmo::consola(s),
        Err(_) => {
            for &c in b {
                let c = if c.is_ascii() { [c] } else { [b'?'] };
                bmo::consola(core::str::from_utf8(&c).unwrap_or("?"));
            }
        }
    }
}

/// `ExitProcess`: se dice con que salio y el proceso se va. No vuelve.
extern "win64" fn exit_process(codigo: u32) -> ! {
    super::fin_del_exe(codigo)
}

/// **La tabla de la casa**: la direccion de cada funcion que existe.
pub fn buscar(dll: &str, f: &Funcion) -> Option<u64> {
    if !dll.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    let Funcion::Nombre(n) = f else { return None };
    Some(match n.as_str() {
        "GetStdHandle" => get_std_handle as *const () as usize as u64,
        "WriteFile" => write_file as *const () as usize as u64,
        "ExitProcess" => exit_process as *const () as usize as u64,
        _ => return None,
    })
}
