//! **Los ficheros del CRT, de la casa** (tanda 1 de Cyberpunk, 29-09): los
//! `FILE*` y los descriptores de `msvcrt.dll` y `api-ms-win-crt-stdio` /
//! `-filesystem`, encima de los ficheros de Windows de la casa
//! (`ficheros.rs`: CreateFileW y los suyos).
//!
//! ```text
//!    FILE*         fopen _wfopen _wfsopen fopen_s fclose fread fgetc getc
//!                  fgets fgetws ungetc fseek _fseeki64 ftell fgetpos fsetpos
//!                  feof ferror clearerr setvbuf _lock_file _unlock_file
//!                  _get_stream_buffer_pointers; y fwrite/fputs/fprintf, que
//!                  ya estaban para stdout, escriben tambien en estos
//!    descriptores  _open _close _read _write _lseeki64 _filelengthi64
//!                  _open_osfhandle
//!    mirar         _stat64 _fstat64 _access _waccess
//! ```
//!
//! Modo TEXTO como Windows: al leer, "\r\n" llega "\n"; al escribir, "\n"
//! sale "\r\n". D: sigue SOLO LECTURA: `CreateFileW` lo niega, y aqui llega
//! como EACCES. Lo que no, dicho: el bufer del FILE es el de Windows (cada
//! lectura va al fichero de la casa, que ya esta entero en memoria), asi que
//! `_get_stream_buffer_pointers` da un bufer VACIO y MSVCP140 lee por fgetc.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::crt;
use crate::crt_cadenas::{poner_errno, EACCES, EBADF, EEXIST, EINVAL, ENOENT};
use crate::{dir, ficheros, kernel32};

const O_WRONLY: i32 = 0x1;
const O_RDWR: i32 = 0x2;
const O_APPEND: i32 = 0x8;
const O_CREAT: i32 = 0x100;
const O_TRUNC: i32 = 0x200;
const O_EXCL: i32 = 0x400;
const O_TEXT: i32 = 0x4000;
const O_BINARY: i32 = 0x8000;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const NO_VALE: u64 = u64::MAX;

#[derive(Clone, Copy)]
struct Descriptor {
    h: u64,
    texto: bool,
    anexar: bool,
}

/// Un FILE de la casa. El `.exe` solo ve su direccion; `base`, `ptr` y
/// `cnt` son lo que da `_get_stream_buffer_pointers` (siempre vacio).
struct Flujo {
    fd: i32,
    eof: bool,
    err: bool,
    atras: Vec<u8>,
    base: u64,
    ptr: u64,
    cnt: i32,
}

struct Estado {
    /// Los descriptores: 0, 1 y 2 son los estandar.
    fds: Vec<Option<Descriptor>>,
    /// Los FILE vivos (sus direcciones).
    flujos: Vec<u64>,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { fds: Vec::new(), flujos: Vec::new() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`; nadie guarda la referencia de un turno a otro.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.fds.clear();
    e.flujos.clear();
}

fn estandar_si_falta() {
    let e = estado();
    if e.fds.is_empty() {
        for n in [-10, -11, -12] {
            e.fds.push(Some(Descriptor { h: kernel32::estandar(n), texto: true, anexar: false }));
        }
    }
}

fn descriptor(fd: i32) -> Option<Descriptor> {
    estandar_si_falta();
    estado().fds.get(usize::try_from(fd).ok()?).copied().flatten()
}

fn nuevo_fd(d: Descriptor) -> i32 {
    estandar_si_falta();
    let v = &mut estado().fds;
    if let Some(i) = v.iter().skip(3).position(Option::is_none) {
        v[i + 3] = Some(d);
        return (i + 3) as i32;
    }
    v.push(Some(d));
    (v.len() - 1) as i32
}

/// El errno de un LastError de CreateFileW.
fn errno_de(e: u32) -> i32 {
    match e {
        2 | 3 => ENOENT,
        5 => EACCES,
        80 | 183 => EEXIST,
        _ => EINVAL,
    }
}

/// `_open` sobre una ruta ya ancha (con su 0).
fn abrir_w(ruta: &[u16], flags: i32) -> i32 {
    let acceso = match flags & 3 {
        O_WRONLY => GENERIC_WRITE,
        O_RDWR => GENERIC_READ | GENERIC_WRITE,
        _ => GENERIC_READ,
    };
    let disposicion = match (flags & O_CREAT != 0, flags & O_EXCL != 0, flags & O_TRUNC != 0) {
        (true, true, _) => 1,      // CREATE_NEW
        (true, false, true) => 2,  // CREATE_ALWAYS
        (true, false, false) => 4, // OPEN_ALWAYS
        (false, _, true) => 5,     // TRUNCATE_EXISTING
        _ => 3,                    // OPEN_EXISTING
    };
    let h = ficheros::create_file_w(ruta.as_ptr(), acceso, 3, 0, disposicion, 0x80, 0);
    if h == NO_VALE {
        poner_errno(errno_de(kernel32::ultimo_error()));
        return -1;
    }
    nuevo_fd(Descriptor { h, texto: flags & O_BINARY == 0, anexar: flags & O_APPEND != 0 })
}

fn ancha(p: *const u8) -> Vec<u16> {
    let t = crt::cadena_c(p as u64);
    let mut w: Vec<u16> = alloc::string::String::from_utf8_lossy(&t).encode_utf16().collect();
    w.push(0);
    w
}

fn ancha_w(p: *const u16) -> Vec<u16> {
    let mut w = crt::cadena_w(p as u64);
    w.push(0);
    w
}

extern "win64" fn open(ruta: *const u8, flags: i32, _permisos: i32) -> i32 {
    if ruta.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    abrir_w(&ancha(ruta), flags)
}

extern "win64" fn close(fd: i32) -> i32 {
    let Some(d) = descriptor(fd) else {
        poner_errno(EBADF);
        return -1;
    };
    estado().fds[fd as usize] = None;
    if fd > 2 && ficheros::cerrar(d.h) == 0 {
        poner_errno(EBADF);
        return -1;
    }
    0
}

/// Leer crudo del descriptor.
fn leer_crudo(d: &Descriptor, dst: &mut [u8]) -> Result<usize, i32> {
    if !ficheros::es_fichero(d.h) {
        // stdin de la consola: no hay teclado de consola en la casa.
        return Ok(0);
    }
    ficheros::leer_de(d.h, dst, None).map_err(|_| EBADF)
}

/// Leer con el modo del descriptor: en texto, "\r\n" -> "\n".
fn leer(d: &Descriptor, dst: &mut [u8]) -> Result<usize, i32> {
    let n = leer_crudo(d, dst)?;
    if !d.texto || n == 0 {
        return Ok(n);
    }
    let mut k = 0;
    let mut i = 0;
    while i < n {
        if dst[i] == b'\r' {
            let siguiente = if i + 1 < n {
                Some(dst[i + 1])
            } else {
                // El '\r' era el ultimo: mirar el que viene, y devolverlo.
                let mut uno = [0u8];
                match leer_crudo(d, &mut uno)? {
                    0 => None,
                    _ => {
                        if uno[0] != b'\n' {
                            if let Some(a) = ficheros::abierto(d.h) {
                                a.pos -= 1;
                            }
                        }
                        Some(uno[0])
                    }
                }
            };
            if siguiente == Some(b'\n') {
                dst[k] = b'\n';
                k += 1;
                i += 2;
                continue;
            }
        }
        dst[k] = dst[i];
        k += 1;
        i += 1;
    }
    Ok(k)
}

fn escribir(d: &Descriptor, fd: i32, src: &[u8]) -> Result<usize, i32> {
    if fd == 1 || fd == 2 {
        return match crt::a_stream(crt::acrt_iob_func(fd as u32), src) {
            r if r < 0 => Err(EBADF),
            _ => Ok(src.len()),
        };
    }
    if d.anexar {
        if let Some(a) = ficheros::abierto(d.h) {
            a.pos = a.medida();
        }
    }
    let t: Vec<u8>;
    let bytes = if d.texto {
        t = src.iter().flat_map(|&c| if c == b'\n' { [b'\r', b'\n'].to_vec() } else { [c].to_vec() }).collect();
        &t[..]
    } else {
        src
    };
    ficheros::escribir_en(d.h, bytes, None).map_err(|_| EBADF)?;
    Ok(src.len())
}

extern "win64" fn read(fd: i32, buf: *mut u8, n: u32) -> i32 {
    let Some(d) = descriptor(fd) else {
        poner_errno(EBADF);
        return -1;
    };
    // SAFETY: `n` bytes del `.exe` donde leer.
    let dst = unsafe { core::slice::from_raw_parts_mut(buf, n as usize) };
    match leer(&d, dst) {
        Ok(k) => k as i32,
        Err(e) => {
            poner_errno(e);
            -1
        }
    }
}

extern "win64" fn write(fd: i32, buf: *const u8, n: u32) -> i32 {
    let Some(d) = descriptor(fd) else {
        poner_errno(EBADF);
        return -1;
    };
    // SAFETY: `n` bytes del `.exe`.
    let src = unsafe { core::slice::from_raw_parts(buf, n as usize) };
    match escribir(&d, fd, src) {
        Ok(k) => k as i32,
        Err(e) => {
            poner_errno(e);
            -1
        }
    }
}

extern "win64" fn lseeki64(fd: i32, off: i64, desde: i32) -> i64 {
    let a = descriptor(fd).and_then(|d| ficheros::abierto(d.h));
    match a.and_then(|a| a.mover(off, desde as u32)) {
        Some(p) => p as i64,
        None => {
            poner_errno(if descriptor(fd).is_none() { EBADF } else { EINVAL });
            -1
        }
    }
}

extern "win64" fn filelengthi64(fd: i32) -> i64 {
    match descriptor(fd).and_then(|d| ficheros::abierto(d.h)) {
        Some(a) => a.medida() as i64,
        None => {
            poner_errno(EBADF);
            -1
        }
    }
}

extern "win64" fn open_osfhandle(h: u64, flags: i32) -> i32 {
    if h == 0 || h == NO_VALE {
        poner_errno(EBADF);
        return -1;
    }
    nuevo_fd(Descriptor { h, texto: flags & O_TEXT != 0, anexar: flags & O_APPEND != 0 })
}

// -- stat y access ---------------------------------------------------------------------------

const S_IFDIR: u16 = 0x4000;
const S_IFREG: u16 = 0x8000;
const S_IREAD: u16 = 0x100;
const S_IWRITE: u16 = 0x80;
const S_IEXEC: u16 = 0x40;

/// El `struct _stat64` de MSVC x64 (56 bytes).
fn poner_stat(st: *mut u8, carpeta: bool, medida: u64, solo_lectura: bool, exe: bool) {
    let mut m = if carpeta { S_IFDIR | S_IEXEC } else { S_IFREG } | S_IREAD;
    if !solo_lectura {
        m |= S_IWRITE;
    }
    if exe {
        m |= S_IEXEC;
    }
    // El CRT copia los permisos del propietario al grupo y a los demas.
    m |= (m & 0x1C0) >> 3 | (m & 0x1C0) >> 6;
    let t = crate::crt_entorno::unix_ahora();
    // SAFETY: los 56 bytes del `_stat64` del `.exe`.
    unsafe {
        core::ptr::write_bytes(st, 0, 56);
        (st.add(6) as *mut u16).write_unaligned(m);
        (st.add(8) as *mut i16).write_unaligned(1);
        (st.add(24) as *mut i64).write_unaligned(medida as i64);
        for off in [32, 40, 48] {
            (st.add(off) as *mut i64).write_unaligned(t);
        }
    }
}

fn es_exe(ruta: &[u16]) -> bool {
    let s = alloc::string::String::from_utf16_lossy(ruta).to_ascii_lowercase();
    [".exe\0", ".bat\0", ".cmd\0", ".com\0"].iter().any(|e| s.ends_with(e))
}

fn stat_w(ruta: &[u16], st: *mut u8) -> i32 {
    let at = ficheros::get_file_attributes_w(ruta.as_ptr());
    if at == u32::MAX {
        poner_errno(ENOENT);
        return -1;
    }
    let carpeta = at & 0x10 != 0;
    let mut medida = 0;
    if !carpeta {
        // La medida: la del fichero, sin leerlo entero (ver
        // `carpetas::medida_real`: la del listado puede ser vieja).
        if let Ok(r) = ficheros::ruta_de(ruta.as_ptr()) {
            medida = crate::carpetas::entrada(&r).map_or(0, |e| crate::carpetas::medida_real(&r, &e));
        }
    }
    poner_stat(st, carpeta, medida, at & 1 != 0, es_exe(ruta));
    0
}

extern "win64" fn stat64(ruta: *const u8, st: *mut u8) -> i32 {
    if ruta.is_null() || st.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    stat_w(&ancha(ruta), st)
}

extern "win64" fn fstat64(fd: i32, st: *mut u8) -> i32 {
    let Some(d) = descriptor(fd) else {
        poner_errno(EBADF);
        return -1;
    };
    if st.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    match ficheros::abierto(d.h) {
        Some(a) => poner_stat(st, a.carpeta, a.medida(), !a.escribe, false),
        // La consola: un dispositivo de caracteres.
        None => {
            poner_stat(st, false, 0, false, false);
            // SAFETY: el `st_mode` del `_stat64`.
            unsafe { (st.add(6) as *mut u16).write_unaligned(0x2000 | S_IREAD | S_IWRITE) };
        }
    }
    0
}

fn access_w(ruta: &[u16], modo: i32) -> i32 {
    let at = ficheros::get_file_attributes_w(ruta.as_ptr());
    if at == u32::MAX {
        poner_errno(ENOENT);
        return -1;
    }
    // Pedir escribir en algo de solo lectura (y D: lo es entero).
    let en_d = ficheros::ruta_de(ruta.as_ptr()).is_ok_and(|r| bmo_proton_x::ficheros::en_personal(&r).is_some());
    if modo & 2 != 0 && (at & 1 != 0 || en_d) {
        poner_errno(EACCES);
        return -1;
    }
    0
}

extern "win64" fn access(ruta: *const u8, modo: i32) -> i32 {
    if ruta.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    access_w(&ancha(ruta), modo)
}

extern "win64" fn waccess(ruta: *const u16, modo: i32) -> i32 {
    if ruta.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    access_w(&ancha_w(ruta), modo)
}

// -- FILE* -----------------------------------------------------------------------------------

fn flujo(f: u64) -> Option<&'static mut Flujo> {
    if !estado().flujos.contains(&f) {
        return None;
    }
    // SAFETY: una direccion que dio `fopen` y sigue viva.
    Some(unsafe { &mut *(f as *mut Flujo) })
}

/// Si `f` es un FILE de la casa (no uno de los estandar).
pub(crate) fn es_flujo(f: u64) -> bool {
    estado().flujos.contains(&f)
}

/// Escribir en un FILE de la casa (fwrite, fputs, fprintf...). Los bytes, o -1.
pub(crate) fn escribir_flujo(f: u64, b: &[u8]) -> i32 {
    let Some(fl) = flujo(f) else { return -1 };
    let Some(d) = descriptor(fl.fd) else { return -1 };
    fl.atras.clear();
    match escribir(&d, fl.fd, b) {
        Ok(n) => n as i32,
        Err(e) => {
            fl.err = true;
            poner_errno(e);
            -1
        }
    }
}

/// Las banderas de `_open` de un modo de `fopen` ("rb", "w+", "a+t, ccs=UTF-8").
fn banderas_de(modo: &[u8]) -> Option<i32> {
    let mut f = match modo.first()? {
        b'r' => 0,
        b'w' => O_WRONLY | O_CREAT | O_TRUNC,
        b'a' => O_WRONLY | O_CREAT | O_APPEND,
        _ => return None,
    };
    for &c in &modo[1..] {
        match c {
            b'+' => f = (f & !3) | O_RDWR,
            b'b' => f |= O_BINARY,
            b't' => f &= !O_BINARY,
            b',' => break,
            _ => {}
        }
    }
    Some(f)
}

fn abrir_flujo(ruta: &[u16], modo: &[u8]) -> u64 {
    let Some(b) = banderas_de(modo) else {
        poner_errno(EINVAL);
        return 0;
    };
    let fd = abrir_w(ruta, b);
    if fd < 0 {
        return 0;
    }
    let p = Box::into_raw(Box::new(Flujo { fd, eof: false, err: false, atras: Vec::new(), base: 0, ptr: 0, cnt: 0 })) as u64;
    estado().flujos.push(p);
    p
}

pub(crate) extern "win64" fn fopen(ruta: *const u8, modo: *const u8) -> u64 {
    if ruta.is_null() || modo.is_null() {
        poner_errno(EINVAL);
        return 0;
    }
    abrir_flujo(&ancha(ruta), &crt::cadena_c(modo as u64))
}

extern "win64" fn wfopen(ruta: *const u16, modo: *const u16) -> u64 {
    if ruta.is_null() || modo.is_null() {
        poner_errno(EINVAL);
        return 0;
    }
    let m: Vec<u8> = crt::cadena_w(modo as u64).iter().map(|&c| c as u8).collect();
    abrir_flujo(&ancha_w(ruta), &m)
}

extern "win64" fn wfsopen(ruta: *const u16, modo: *const u16, _compartir: i32) -> u64 {
    wfopen(ruta, modo)
}

extern "win64" fn fopen_s(f: *mut u64, ruta: *const u8, modo: *const u8) -> i32 {
    if f.is_null() {
        return EINVAL;
    }
    let p = fopen(ruta, modo);
    // SAFETY: el FILE* del `.exe`.
    unsafe { *f = p };
    if p == 0 {
        let e = crate::crt_cadenas::errno_actual();
        return if e == 0 { EINVAL } else { e };
    }
    0
}

pub(crate) extern "win64" fn fclose(f: u64) -> i32 {
    if crt::cual(f).is_some() {
        return 0;
    }
    let Some(fl) = flujo(f) else {
        poner_errno(EINVAL);
        return -1;
    };
    let r = close(fl.fd);
    estado().flujos.retain(|&x| x != f);
    // SAFETY: lo creo `abrir_flujo` con Box::into_raw y ya no esta en la lista.
    drop(unsafe { Box::from_raw(f as *mut Flujo) });
    if r < 0 {
        -1
    } else {
        0
    }
}

/// Leer de un FILE: primero lo devuelto con ungetc, luego el fichero.
fn leer_flujo(f: u64, dst: &mut [u8]) -> usize {
    let Some(fl) = flujo(f) else {
        // stdin (o algo que no es un FILE): al final.
        return 0;
    };
    let mut k = 0;
    while k < dst.len() {
        let Some(c) = fl.atras.pop() else { break };
        dst[k] = c;
        k += 1;
    }
    if k < dst.len() {
        match descriptor(fl.fd).map(|d| leer(&d, &mut dst[k..])) {
            Some(Ok(n)) => k += n,
            _ => fl.err = true,
        }
    }
    if k < dst.len() && !fl.err {
        fl.eof = true;
    }
    k
}

extern "win64" fn fread(p: *mut u8, tam: usize, n: usize, f: u64) -> usize {
    let Some(total) = tam.checked_mul(n) else { return 0 };
    if total == 0 {
        return 0;
    }
    // SAFETY: `tam * n` bytes del `.exe`.
    let dst = unsafe { core::slice::from_raw_parts_mut(p, total) };
    leer_flujo(f, dst) / tam
}

extern "win64" fn fgetc(f: u64) -> i32 {
    let mut c = [0u8];
    if leer_flujo(f, &mut c) == 1 {
        c[0] as i32
    } else {
        -1
    }
}

extern "win64" fn fgets(buf: *mut u8, n: i32, f: u64) -> *mut u8 {
    if buf.is_null() || n <= 0 {
        return core::ptr::null_mut();
    }
    let mut k = 0usize;
    while k + 1 < n as usize {
        let c = fgetc(f);
        if c < 0 {
            break;
        }
        // SAFETY: `n` bytes en `buf`; `k + 1 < n`.
        unsafe { *buf.add(k) = c as u8 };
        k += 1;
        if c == b'\n' as i32 {
            break;
        }
    }
    if k == 0 && n > 1 {
        return core::ptr::null_mut();
    }
    // SAFETY: `k < n`.
    unsafe { *buf.add(k) = 0 };
    buf
}

extern "win64" fn fgetws(buf: *mut u16, n: i32, f: u64) -> *mut u16 {
    if buf.is_null() || n <= 0 {
        return core::ptr::null_mut();
    }
    let mut k = 0usize;
    while k + 1 < n as usize {
        let c = fgetc(f);
        if c < 0 {
            break;
        }
        // SAFETY: como `fgets`, en `wchar_t`.
        unsafe { *buf.add(k) = c as u16 };
        k += 1;
        if c == b'\n' as i32 {
            break;
        }
    }
    if k == 0 && n > 1 {
        return core::ptr::null_mut();
    }
    // SAFETY: `k < n`.
    unsafe { *buf.add(k) = 0 };
    buf
}

extern "win64" fn ungetc(c: i32, f: u64) -> i32 {
    let Some(fl) = flujo(f) else { return -1 };
    if c < 0 {
        return -1;
    }
    fl.atras.push(c as u8);
    fl.eof = false;
    c & 0xFF
}

fn buscar_en_flujo(f: u64, off: i64, desde: i32) -> i32 {
    let Some(fl) = flujo(f) else {
        poner_errno(EINVAL);
        return -1;
    };
    // SEEK_CUR cuenta desde donde el `.exe` cree estar: sin lo devuelto.
    let off = if desde == 1 { off - fl.atras.len() as i64 } else { off };
    fl.atras.clear();
    if lseeki64(fl.fd, off, desde) < 0 {
        return -1;
    }
    fl.eof = false;
    0
}

extern "win64" fn fseek(f: u64, off: i32, desde: i32) -> i32 {
    buscar_en_flujo(f, off as i64, desde)
}

pub(crate) extern "win64" fn fseeki64(f: u64, off: i64, desde: i32) -> i32 {
    buscar_en_flujo(f, off, desde)
}

fn posicion(f: u64) -> i64 {
    let Some(fl) = flujo(f) else {
        poner_errno(EINVAL);
        return -1;
    };
    match descriptor(fl.fd).and_then(|d| ficheros::abierto(d.h)) {
        Some(a) => a.pos as i64 - fl.atras.len() as i64,
        None => -1,
    }
}

extern "win64" fn ftell(f: u64) -> i32 {
    let p = posicion(f);
    if p > i32::MAX as i64 {
        poner_errno(crate::crt_cadenas::ERANGE);
        return -1;
    }
    p as i32
}

extern "win64" fn fgetpos(f: u64, pos: *mut i64) -> i32 {
    let p = posicion(f);
    if p < 0 || pos.is_null() {
        return -1;
    }
    // SAFETY: el fpos_t del `.exe`.
    unsafe { pos.write_unaligned(p) };
    0
}

extern "win64" fn fsetpos(f: u64, pos: *const i64) -> i32 {
    if pos.is_null() {
        poner_errno(EINVAL);
        return -1;
    }
    // SAFETY: el fpos_t del `.exe`.
    buscar_en_flujo(f, unsafe { pos.read_unaligned() }, 0)
}

extern "win64" fn feof(f: u64) -> i32 {
    if crt::cual(f) == Some(0) {
        return 1;
    }
    flujo(f).is_some_and(|fl| fl.eof) as i32
}

extern "win64" fn ferror(f: u64) -> i32 {
    flujo(f).is_some_and(|fl| fl.err) as i32
}

extern "win64" fn clearerr(f: u64) {
    if let Some(fl) = flujo(f) {
        fl.eof = false;
        fl.err = false;
    }
}

extern "win64" fn setvbuf(_f: u64, _buf: u64, _modo: i32, _n: usize) -> i32 {
    0
}

extern "win64" fn nada(_f: u64) {}

extern "win64" fn stream_buffer_pointers(f: u64, base: *mut u64, ptr: *mut u64, cnt: *mut u64) -> i32 {
    let Some(fl) = flujo(f) else { return EINVAL };
    // SAFETY: tres punteros del `.exe` donde dejar las direcciones.
    unsafe {
        if !base.is_null() {
            *base = &mut fl.base as *mut u64 as u64;
        }
        if !ptr.is_null() {
            *ptr = &mut fl.ptr as *mut u64 as u64;
        }
        if !cnt.is_null() {
            *cnt = &mut fl.cnt as *mut i32 as u64;
        }
    }
    0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "_open" => dir!(open),
        "_close" => dir!(close),
        "_read" => dir!(read),
        "_write" => dir!(write),
        "_lseeki64" => dir!(lseeki64),
        "_filelengthi64" => dir!(filelengthi64),
        "_open_osfhandle" => dir!(open_osfhandle),
        "_stat64" => dir!(stat64),
        "_fstat64" => dir!(fstat64),
        "_access" => dir!(access),
        "_waccess" => dir!(waccess),
        "fopen" => dir!(fopen),
        "_wfopen" => dir!(wfopen),
        "_wfsopen" => dir!(wfsopen),
        "fopen_s" => dir!(fopen_s),
        "fclose" => dir!(fclose),
        "fread" => dir!(fread),
        "fgetc" | "getc" => dir!(fgetc),
        "fgets" => dir!(fgets),
        "fgetws" => dir!(fgetws),
        "ungetc" => dir!(ungetc),
        "fseek" => dir!(fseek),
        "_fseeki64" => dir!(fseeki64),
        "ftell" => dir!(ftell),
        "fgetpos" => dir!(fgetpos),
        "fsetpos" => dir!(fsetpos),
        "feof" => dir!(feof),
        "ferror" => dir!(ferror),
        "clearerr" => dir!(clearerr),
        "setvbuf" => dir!(setvbuf),
        "_lock_file" | "_unlock_file" => dir!(nada),
        "_get_stream_buffer_pointers" => dir!(stream_buffer_pointers),
        _ => return None,
    })
}
