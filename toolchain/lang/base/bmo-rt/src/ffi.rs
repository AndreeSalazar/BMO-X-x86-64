//! **Los nombres de C**: lo que un `.bex` enlaza de `bmo-rt`.
//!
//! [!] Solo existen en el `.bex` (`cfg_attr(all(not(test), feature = "libc"), no_mangle)`). En las
//! pruebas del anfitrion no se exportan, y es a proposito: hasta el 03-10 el
//! `malloc` de aqui sustituia al del anfitrion dentro del binario de pruebas
//! --que no tiene `KIND_MEMORIA`--, y `cargo test -p bmo-rt` moria con
//! "memory allocation of 48 bytes failed" antes de probar nada. Las pruebas
//! llaman a lo de Rust; los nombres de C son de BMO-X.
//!
//! Un programa de C es de UN hilo: la tabla de `FILE` no lleva cerrojo.

#![allow(non_upper_case_globals, clippy::missing_safety_doc)]

use crate::fichero::{Fichero, Kernel, Tabla, EOF};
use crate::fmt::{cadena, formatear, Args, Salida};
use crate::heap;
use core::ptr::{self, addr_of_mut};

static mut TABLA: Tabla = Tabla::new();

fn tabla() -> &'static mut Tabla {
    unsafe { &mut *addr_of_mut!(TABLA) }
}

fn de(f: *mut Fichero) -> Option<&'static mut Fichero> {
    tabla().de(f)
}

/// Lo que C llama `stdin`: leerlo da fin de fichero (la entrada de BMO-X es
/// `KIND_INPUT`).
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub static mut stdin: *mut Fichero = unsafe { addr_of_mut!(TABLA.f[0]) };
/// `stdout`: la consola.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub static mut stdout: *mut Fichero = unsafe { addr_of_mut!(TABLA.f[1]) };
/// `stderr`: la consola tambien.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub static mut stderr: *mut Fichero = unsafe { addr_of_mut!(TABLA.f[2]) };

/// Cerrar todos los `FILE`: lo escrito llega al disco. Lo llama `exit`.
pub fn cerrar_todos() {
    tabla().cerrar_todos(&Kernel);
}

// -- El monton -------------------------------------------------------------

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    heap::malloc(size)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    heap::free(ptr)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    heap::calloc(nmemb, size)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    heap::realloc(ptr, new_size)
}

// -- Los ficheros ------------------------------------------------------------

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fopen(ruta: *const u8, modo: *const u8) -> *mut Fichero {
    tabla().abrir(&Kernel, cadena(ruta), cadena(modo))
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fclose(f: *mut Fichero) -> i32 {
    if tabla().cerrar(&Kernel, f) {
        0
    } else {
        EOF
    }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fread(dst: *mut u8, size: usize, n: usize, f: *mut Fichero) -> usize {
    let (Some(f), Some(total)) = (de(f), size.checked_mul(n)) else { return 0 };
    if size == 0 {
        return 0;
    }
    f.leer(&Kernel, dst, total) / size
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fwrite(src: *const u8, size: usize, n: usize, f: *mut Fichero) -> usize {
    let (Some(f), Some(total)) = (de(f), size.checked_mul(n)) else { return 0 };
    if size == 0 {
        return 0;
    }
    f.escribir(&Kernel, src, total) / size
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fgetc(f: *mut Fichero) -> i32 {
    de(f).map_or(EOF, |f| f.byte(&Kernel))
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn getc(f: *mut Fichero) -> i32 {
    fgetc(f)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn getchar() -> i32 {
    fgetc(stdin)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn ungetc(c: i32, f: *mut Fichero) -> i32 {
    de(f).map_or(EOF, |f| f.devolver(c))
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fgets(s: *mut u8, n: i32, f: *mut Fichero) -> *mut u8 {
    let leido = n > 0 && de(f).is_some_and(|f| f.linea(&Kernel, s, n as usize));
    if leido {
        s
    } else {
        ptr::null_mut()
    }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fputc(c: i32, f: *mut Fichero) -> i32 {
    let b = c as u8;
    if de(f).is_some_and(|f| f.escribir(&Kernel, &b, 1) == 1) {
        b as i32
    } else {
        EOF
    }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn putc(c: i32, f: *mut Fichero) -> i32 {
    fputc(c, f)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn putchar(c: i32) -> i32 {
    fputc(c, stdout)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fputs(s: *const u8, f: *mut Fichero) -> i32 {
    let c = cadena(s);
    if de(f).is_some_and(|f| f.escribir(&Kernel, c.as_ptr(), c.len()) == c.len()) {
        0
    } else {
        EOF
    }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn puts(s: *const u8) -> i32 {
    if fputs(s, stdout) == EOF {
        return EOF;
    }
    fputc(b'\n' as i32, stdout)
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fseek(f: *mut Fichero, desp: i64, desde: i32) -> i32 {
    de(f).map_or(-1, |f| f.saltar(&Kernel, desp, desde))
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn ftell(f: *mut Fichero) -> i64 {
    de(f).map_or(-1, |f| f.donde())
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn rewind(f: *mut Fichero) {
    if let Some(f) = de(f) {
        f.saltar(&Kernel, 0, 0);
        f.limpiar();
    }
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn feof(f: *mut Fichero) -> i32 {
    de(f).is_some_and(|f| f.al_final()) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn ferror(f: *mut Fichero) -> i32 {
    de(f).is_some_and(|f| f.con_error()) as i32
}

#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn clearerr(f: *mut Fichero) {
    if let Some(f) = de(f) {
        f.limpiar();
    }
}

/// `fflush(NULL)` baja todos.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn fflush(f: *mut Fichero) -> i32 {
    if f.is_null() {
        let t = tabla();
        let todos = t.f.iter_mut().fold(true, |b, f| f.bajar(&Kernel) & b);
        return if todos { 0 } else { EOF };
    }
    if de(f).is_some_and(|f| f.bajar(&Kernel)) {
        0
    } else {
        EOF
    }
}

// -- printf ------------------------------------------------------------------

/// Formatear a un `FILE`, de 256 en 256.
struct AFichero {
    f: &'static mut Fichero,
    t: [u8; 256],
    k: usize,
    total: usize,
}

impl Salida for AFichero {
    fn poner(&mut self, b: u8) {
        self.t[self.k] = b;
        self.k += 1;
        self.total += 1;
        if self.k == self.t.len() {
            self.soltar();
        }
    }
}

impl AFichero {
    fn soltar(&mut self) {
        self.f.escribir(&Kernel, self.t.as_ptr(), self.k);
        self.k = 0;
    }
}

/// `fprintf(f, fmt, ...)` con la convencion de BMO: `n` argumentos en un
/// arreglo de palabras (ver `fmt`). Devuelve los bytes escritos.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn bmo_fprintf(f: *mut Fichero, fmt: *const u8, n: u64, args: *const u64) -> i32 {
    let Some(f) = de(f) else { return -1 };
    let mut s = AFichero { f, t: [0; 256], k: 0, total: 0 };
    formatear(&mut s, cadena(fmt), &mut Args::new(args, n as usize));
    s.soltar();
    s.total as i32
}

/// `printf(fmt, ...)`: a `stdout`, sin tope de largo.
#[cfg_attr(all(not(test), feature = "libc"), no_mangle)]
pub unsafe extern "C" fn bmo_printf(fmt: *const u8, n: u64, args: *const u64) -> i32 {
    bmo_fprintf(stdout, fmt, n, args)
}
