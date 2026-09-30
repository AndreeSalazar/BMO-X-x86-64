//! **Lo demas que pide un runtime de Windows, de la casa** (P4f4, 27-09):
//! lo que hace falta de verdad, y lo que solo tiene que EXISTIR y contestar
//! lo que Windows contestaria en una maquina sin eso.
//!
//! ```text
//!    ntdll.dll    NtReadFile, NtWriteFile (la `std` de Rust lee y escribe
//!                 ficheros POR AQUI), RtlNtStatusToDosError; NtCreateFile,
//!                 NtOpenFile, NtCreateNamedPipeFile, NtSetInformationFile: NO
//!    bcryptprimitives.dll  ProcessPrng: el azar de HashMap, de RDRAND
//!    userenv.dll  GetUserProfileDirectoryW
//!    kernel32     FormatMessageW (y LocalFree), GetWindowsDirectoryW,
//!                 GetSystemDirectoryW, QueryDosDeviceW, GetOverlappedResult,
//!                 CancelIo, DeviceIoControl, ReadFileEx/WriteFileEx,
//!                 ReadConsoleW, CreateHardLinkW, CreateSymbolicLinkW
//!    procesos     GetCurrentProcess ya; GetProcessId, GetExitCodeProcess,
//!                 TerminateProcess del propio; CreateProcessW, CreatePipe y
//!                 las listas de atributos: NO, dicho
//! ```
//!
//! Lo que no es Windows, dicho: no hay procesos hijos ni tuberias (un
//! `.exe` es un proceso de BMO-X, y PROTON-X no lanza otro), no hay APC
//! (ReadFileEx/WriteFileEx), `C:\Windows` es un NOMBRE que se da y no una
//! carpeta que exista en el volumen, y si la CPU no diera RDRAND, ProcessPrng
//! mezclaria el reloj (el Ryzen lo da).

use alloc::vec::Vec;

use bmo_proton_x::mensajes::{self, STATUS_END_OF_FILE, STATUS_INVALID_HANDLE, STATUS_NOT_SUPPORTED, STATUS_SUCCESS};

use crate::{aviso, dir, ficheros, kernel32, memoria, plataforma};

const ERROR_INVALID_FUNCTION: u32 = 1;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NOT_SUPPORTED: u32 = 50;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const STILL_ACTIVE: u32 = 259;
const PSEUDO_PROCESO: u64 = u64::MAX;

// -- ntdll --------------------------------------------------------------------------

/// `IO_STATUS_BLOCK`: el NTSTATUS y los bytes.
fn cumplir(iosb: u64, status: u32, n: usize) -> u32 {
    if iosb != 0 {
        // SAFETY: un IO_STATUS_BLOCK del `.exe` (16 bytes).
        unsafe {
            (iosb as *mut u64).write_unaligned(status as u64);
            ((iosb + 8) as *mut u64).write_unaligned(n as u64);
        }
    }
    status
}

/// El desplazamiento de `NtReadFile`/`NtWriteFile`: NULL o
/// FILE_USE_FILE_POINTER_POSITION (-2, alto -1) es "desde el cursor".
fn desde(p: *const i64) -> Option<u64> {
    if p.is_null() {
        return None;
    }
    // SAFETY: un LARGE_INTEGER del `.exe`.
    let v = unsafe { p.read_unaligned() };
    (v >= 0).then_some(v as u64)
}

fn a_status(e: u32) -> u32 {
    match e {
        ERROR_INVALID_HANDLE => STATUS_INVALID_HANDLE,
        ERROR_INVALID_FUNCTION => 0xC000_0010, // STATUS_INVALID_DEVICE_REQUEST
        _ => mensajes::STATUS_INVALID_PARAMETER,
    }
}

/// `NtReadFile(h, evento, apc, ctx, *iosb, bufer, n, *desde, *clave)`: como
/// ReadFile, pero el final es STATUS_END_OF_FILE (y la `std` de Rust lo lee
/// como "0 bytes").
extern "win64" fn nt_read_file(h: u64, evento: u64, apc: u64, _ctx: u64, iosb: u64, b: *mut u8, n: u32, off: *const i64, _clave: u64) -> u32 {
    if evento != 0 || apc != 0 {
        aviso("NtReadFile con evento o APC: la casa lee sincrono, sin avisar a nadie");
    }
    // SAFETY: `n` bytes del `.exe`.
    let dst = if n == 0 { &mut [][..] } else { unsafe { core::slice::from_raw_parts_mut(b, n as usize) } };
    match ficheros::leer_de(h, dst, desde(off)) {
        Ok(0) if n > 0 => cumplir(iosb, STATUS_END_OF_FILE, 0),
        Ok(k) => cumplir(iosb, STATUS_SUCCESS, k),
        Err(e) => cumplir(iosb, a_status(e), 0),
    }
}

extern "win64" fn nt_write_file(h: u64, evento: u64, apc: u64, _ctx: u64, iosb: u64, b: *const u8, n: u32, off: *const i64, _clave: u64) -> u32 {
    if evento != 0 || apc != 0 {
        aviso("NtWriteFile con evento o APC: la casa escribe sincrono, sin avisar a nadie");
    }
    if kernel32::es_consola(h) {
        let mut escritos = 0u32;
        kernel32::write_file(h, b, n, &mut escritos, 0);
        return cumplir(iosb, STATUS_SUCCESS, escritos as usize);
    }
    // SAFETY: `n` bytes del `.exe`.
    let src = if n == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(b, n as usize) } };
    match ficheros::escribir_en(h, src, desde(off)) {
        Ok(k) => cumplir(iosb, STATUS_SUCCESS, k),
        Err(e) => cumplir(iosb, a_status(e), 0),
    }
}

extern "win64" fn rtl_nt_status_to_dos_error(s: u32) -> u32 {
    mensajes::dos_de_status(s)
}

/// Las de ntdll que abren por su camino (rutas NT, `\??\C:\...`) o cambian
/// lo que el FAT32 de BMO-X no cambia: no, dicho. La `std` de Rust las usa
/// para `remove_dir_all` y las tuberias de `Command`.
extern "win64" fn nt_no(_a: u64, _b: u64, _c: u64, _d: u64) -> u32 {
    aviso("NtCreateFile/NtOpenFile/NtCreateNamedPipeFile/NtSetInformationFile: la casa abre por CreateFileW, y no hay tuberias");
    STATUS_NOT_SUPPORTED
}

// -- El azar ------------------------------------------------------------------------

/// Un numero de RDRAND (hasta diez intentos, lo que pide Intel/AMD).
fn rdrand() -> Option<u64> {
    for _ in 0..10 {
        let (v, ok): (u64, u8);
        // SAFETY: RDRAND solo escribe el registro y CF.
        unsafe { core::arch::asm!("rdrand {v}", "setc {ok}", v = out(reg) v, ok = out(reg_byte) ok, options(nomem, nostack)) };
        if ok != 0 {
            return Some(v);
        }
    }
    None
}

/// `ProcessPrng(bufer, n)`: siempre TRUE, como Windows.
pub(crate) extern "win64" fn process_prng(b: *mut u8, n: usize) -> i32 {
    let mut mezcla = (plataforma().ahora_ns)() ^ 0x9E37_79B9_7F4A_7C15;
    let mut i = 0;
    while i < n {
        let v = rdrand().unwrap_or_else(|| {
            // Sin RDRAND: splitmix64 del reloj. No es para cifrar.
            mezcla = mezcla.wrapping_add(0x9E37_79B9_7F4A_7C15) ^ (plataforma().ahora_ns)();
            let mut z = mezcla;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        });
        let k = (n - i).min(8);
        // SAFETY: `n` bytes del `.exe`.
        unsafe { core::ptr::copy_nonoverlapping(v.to_le_bytes().as_ptr(), b.add(i), k) };
        i += k;
    }
    1
}

// -- Los mensajes ----------------------------------------------------------------------

const FORMAT_MESSAGE_ALLOCATE_BUFFER: u32 = 0x100;
const FORMAT_MESSAGE_FROM_STRING: u32 = 0x400;
const FORMAT_MESSAGE_FROM_HMODULE: u32 = 0x800;
const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x1000;

/// `FormatMessageW(banderas, fuente, id, idioma, bufer, n, args)`: los
/// mensajes de SISTEMA (bmo_proton_x::mensajes), con su "\r\n".
extern "win64" fn format_message_w(banderas: u32, _fuente: u64, id: u32, _idioma: u32, buf: *mut u16, n: u32, _args: u64) -> u32 {
    if banderas & (FORMAT_MESSAGE_FROM_STRING | FORMAT_MESSAGE_FROM_HMODULE) != 0 || banderas & FORMAT_MESSAGE_FROM_SYSTEM == 0 {
        aviso("FormatMessageW: la casa solo tiene los mensajes de sistema");
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let Some(t) = mensajes::texto(id) else {
        kernel32::poner_error(mensajes::ERROR_MR_MID_NOT_FOUND);
        return 0;
    };
    let w: Vec<u16> = t.encode_utf16().chain("\r\n".encode_utf16()).collect();
    if banderas & FORMAT_MESSAGE_ALLOCATE_BUFFER != 0 {
        // `buf` es un LPWSTR*: el bloque sale del monton del proceso (y se
        // suelta con LocalFree), al menos `n` caracteres.
        let medida = w.len().max(n as usize) + 1;
        let Some(p) = memoria::pedir_del_proceso(2 * medida as u64) else {
            kernel32::poner_error(8);
            return 0;
        };
        // SAFETY: el bloque recien pedido, y el LPWSTR* del `.exe`.
        unsafe {
            core::ptr::copy_nonoverlapping(w.as_ptr(), p as *mut u16, w.len());
            *(p as *mut u16).add(w.len()) = 0;
            *(buf as *mut u64) = p;
        }
        return w.len() as u32;
    }
    if (n as usize) <= w.len() || buf.is_null() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: el `.exe` da `n` > w.len() caracteres.
    unsafe {
        core::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len());
        *buf.add(w.len()) = 0;
    }
    w.len() as u32
}

/// `LocalFree`: lo que dio FormatMessageW(ALLOCATE_BUFFER). NULL si salio.
extern "win64" fn local_free(p: u64) -> u64 {
    if p == 0 || memoria::soltar_del_proceso(p) {
        return 0;
    }
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    p
}

// -- Nombres del sistema ------------------------------------------------------------------

fn dar(s: &str, buf: *mut u16, n: u32) -> u32 {
    let w: Vec<u16> = s.encode_utf16().collect();
    if (n as usize) <= w.len() || buf.is_null() {
        return w.len() as u32 + 1;
    }
    // SAFETY: el `.exe` da `n` > w.len() caracteres.
    unsafe {
        core::ptr::copy_nonoverlapping(w.as_ptr(), buf, w.len());
        *buf.add(w.len()) = 0;
    }
    w.len() as u32
}

/// `C:\Windows`: el nombre que un programa espera para construir rutas. NO
/// existe en el volumen de BMO-X (y una DLL de sistema pedida con esa ruta
/// la resuelve la casa por su nombre).
extern "win64" fn get_windows_directory_w(buf: *mut u16, n: u32) -> u32 {
    dar("C:\\Windows", buf, n)
}

extern "win64" fn get_system_directory_w(buf: *mut u16, n: u32) -> u32 {
    dar("C:\\Windows\\System32", buf, n)
}

/// `GetUserProfileDirectoryW(token, bufer, *n)`: el perfil es la carpeta del
/// `.exe` (lo mismo que USERPROFILE del entorno).
extern "win64" fn get_user_profile_directory_w(_token: u64, buf: *mut u16, n: *mut u32) -> i32 {
    if n.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let perfil: Vec<u16> = crate::proceso::variable("USERPROFILE").unwrap_or_else(|| "C:\\".encode_utf16().collect());
    // SAFETY: un DWORD del `.exe`: lo que cabe, y a la vuelta lo que hace falta.
    let cabe = unsafe { *n } as usize;
    unsafe { *n = perfil.len() as u32 + 1 };
    if cabe <= perfil.len() || buf.is_null() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: el `.exe` da `cabe` > perfil.len() caracteres.
    unsafe {
        core::ptr::copy_nonoverlapping(perfil.as_ptr(), buf, perfil.len());
        *buf.add(perfil.len()) = 0;
    }
    1
}

/// `QueryDosDeviceW("C:")`: el nombre NT de la unidad, en una lista de
/// cadenas (acaba en dos ceros).
extern "win64" fn query_dos_device_w(nombre: *const u16, buf: *mut u16, n: u32) -> u32 {
    let mut w = Vec::new();
    if !nombre.is_null() {
        // SAFETY: una cadena del `.exe`.
        unsafe {
            while w.len() < 8 && *nombre.add(w.len()) != 0 {
                w.push(*nombre.add(w.len()));
            }
        }
    }
    if !(w.len() == 2 && (w[0] | 0x20) == b'c' as u16 && w[1] == b':' as u16) {
        kernel32::poner_error(ERROR_FILE_NOT_FOUND);
        return 0;
    }
    let t: Vec<u16> = "\\Device\\BmoDatos\0\0".encode_utf16().collect();
    if (n as usize) < t.len() || buf.is_null() {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    // SAFETY: el `.exe` da `n` >= t.len() caracteres.
    unsafe { core::ptr::copy_nonoverlapping(t.as_ptr(), buf, t.len()) };
    t.len() as u32
}

// -- La E/S que no hay que esperar ---------------------------------------------------------

/// `GetOverlappedResult`: la casa acaba cada E/S antes de volver, asi que el
/// resultado ya esta en el OVERLAPPED (Internal e InternalHigh).
extern "win64" fn get_overlapped_result(_h: u64, ov: u64, n: *mut u32, _esperar: i32) -> i32 {
    if ov == 0 {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    // SAFETY: un OVERLAPPED del `.exe`.
    let (status, k) = unsafe { ((ov as *const u64).read_unaligned() as u32, ((ov + 8) as *const u64).read_unaligned()) };
    if !n.is_null() {
        // SAFETY: un DWORD del `.exe`.
        unsafe { *n = k as u32 };
    }
    if status != 0 {
        kernel32::poner_error(mensajes::dos_de_status(status));
        return 0;
    }
    1
}

extern "win64" fn cancel_io(h: u64) -> i32 {
    if ficheros::es_fichero(h) || kernel32::es_consola(h) {
        // Nada pendiente que cancelar: cada E/S acabo al volver.
        return 1;
    }
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    0
}

extern "win64" fn device_io_control(_h: u64, codigo: u32, _a: u64, _b: u32, _c: u64, _d: u32, _e: u64, _f: u64) -> i32 {
    aviso(&alloc::format!("DeviceIoControl({codigo:#x}): la casa no habla con dispositivos"));
    kernel32::poner_error(ERROR_INVALID_FUNCTION);
    0
}

extern "win64" fn read_file_ex(_h: u64, _b: u64, _n: u32, _ov: u64, _rutina: u64) -> i32 {
    aviso("ReadFileEx/WriteFileEx: sin APC en la casa (y sin handles FILE_FLAG_OVERLAPPED)");
    kernel32::poner_error(ERROR_INVALID_PARAMETER);
    0
}

extern "win64" fn read_console_w(_h: u64, _b: u64, _n: u32, _leidos: u64, _ctl: u64) -> i32 {
    // No hay entrada de consola: GetStdHandle(STD_INPUT_HANDLE) no da nada.
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    0
}

/// Enlaces duros y simbolicos: el FAT32 no los tiene (lo mismo que dice
/// Windows en un FAT32).
extern "win64" fn create_link_w(_a: u64, _b: u64, _c: u64) -> i32 {
    kernel32::poner_error(ERROR_INVALID_FUNCTION);
    0
}

// -- Procesos -------------------------------------------------------------------------------

extern "win64" fn get_process_id(h: u64) -> u32 {
    if h == PSEUDO_PROCESO {
        return kernel32::id_del_proceso();
    }
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    0
}

extern "win64" fn get_exit_code_process(h: u64, codigo: *mut u32) -> i32 {
    if h != PSEUDO_PROCESO || codigo.is_null() {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    // SAFETY: un DWORD del `.exe`.
    unsafe { *codigo = STILL_ACTIVE };
    1
}

/// `TerminateProcess` del PROPIO proceso: salir con ese codigo, como Windows.
extern "win64" fn terminate_process(h: u64, codigo: u32) -> i32 {
    if h == PSEUDO_PROCESO {
        (plataforma().salir)(codigo);
    }
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    0
}

/// `CreateProcessW`: si el `.exe` que se nombra no esta, lo que Windows
/// dice; si esta, que la casa no lanza procesos.
extern "win64" fn create_process_w(aplicacion: *const u16, linea: *const u16, _a: u64, _b: u64, _c: i32, _d: u32, _e: u64, _f: u64, _g: u64, _h: u64) -> i32 {
    let mut w = Vec::new();
    let p = if aplicacion.is_null() { linea } else { aplicacion };
    if !p.is_null() {
        // SAFETY: una cadena del `.exe`.
        unsafe {
            while w.len() < 1024 && *p.add(w.len()) != 0 {
                w.push(*p.add(w.len()));
            }
        }
    }
    // El programa: hasta el primer espacio, o entre comillas.
    let nombre: Vec<u16> = if w.first() == Some(&(b'"' as u16)) {
        w[1..].iter().copied().take_while(|&c| c != b'"' as u16).collect()
    } else {
        w.iter().copied().take_while(|&c| c != b' ' as u16).collect()
    };
    let existe = bmo_proton_x::ficheros::ruta(&nombre, &ficheros::directorio()).ok().and_then(|r| crate::carpetas::entrada(&r)).is_some();
    if !existe {
        kernel32::poner_error(ERROR_FILE_NOT_FOUND);
        return 0;
    }
    aviso("CreateProcessW: PROTON-X no lanza otro proceso todavia");
    kernel32::poner_error(ERROR_NOT_SUPPORTED);
    0
}

extern "win64" fn create_pipe(_r: u64, _w: u64, _attr: u64, _n: u32) -> i32 {
    aviso("CreatePipe: no hay tuberias en la casa (no hay procesos hijos que las usen)");
    kernel32::poner_error(ERROR_NOT_SUPPORTED);
    0
}

/// `InitializeProcThreadAttributeList(lista, n, 0, *medida)`: con lista NULL
/// dice cuanto mide (como Windows, con ERROR_INSUFFICIENT_BUFFER).
extern "win64" fn initialize_proc_thread_attribute_list(lista: u64, n: u32, _banderas: u32, medida: *mut usize) -> i32 {
    if medida.is_null() {
        kernel32::poner_error(ERROR_INVALID_PARAMETER);
        return 0;
    }
    let hace_falta = 48 + 24 * n as usize;
    // SAFETY: un SIZE_T del `.exe`.
    let hay = unsafe { *medida };
    unsafe { *medida = hace_falta };
    if lista == 0 || hay < hace_falta {
        kernel32::poner_error(ERROR_INSUFFICIENT_BUFFER);
        return 0;
    }
    1
}

extern "win64" fn update_proc_thread_attribute(_l: u64, _f: u32, _a: usize, _v: u64, _n: usize, _p: u64, _r: u64) -> i32 {
    1
}

extern "win64" fn delete_proc_thread_attribute_list(_l: u64) {}

// -- Los errores COM (P3c1): lo que el crate `windows` pide para un HRESULT --

/// `GetErrorInfo(0, **info)`: no hay informacion de error: S_FALSE y NULL.
extern "win64" fn get_error_info(_r: u32, info: *mut u64) -> i32 {
    if !info.is_null() {
        // SAFETY: un IErrorInfo** del `.exe`.
        unsafe { *info = 0 };
    }
    1
}

/// Un BSTR: la medida en bytes (u32) va 4 bytes antes del texto.
extern "win64" fn sys_string_len(b: u64) -> u32 {
    if b == 0 {
        return 0;
    }
    // SAFETY: un BSTR del `.exe`.
    unsafe { ((b - 4) as *const u32).read_unaligned() / 2 }
}

extern "win64" fn sys_free_string(b: u64) {
    if b != 0 && !memoria::soltar_del_proceso(b - 4) {
        aviso("SysFreeString de un BSTR que no dio la casa");
    }
}

/// `RoOriginateErrorW`: nadie escucha errores de WinRT: FALSE, "no se guardo".
extern "win64" fn ro_originate_error_w(_hr: i32, _n: u32, _texto: u64) -> i32 {
    0
}

// -- P3c: lo que el arranque del CRT estatico de MSVC pide (BMOX-12) --------

/// `InitializeSListHead`: una lista enlazada atomica vacia (16 bytes a cero).
extern "win64" fn initialize_slist_head(cabeza: *mut u8) {
    if !cabeza.is_null() {
        // SAFETY: un SLIST_HEADER del `.exe` (16 bytes).
        unsafe { core::ptr::write_bytes(cabeza, 0, 16) };
    }
}

/// `IsDebuggerPresent`: en BMO-X no hay depurador de Ring 3.
extern "win64" fn is_debugger_present() -> i32 {
    0
}

const PF_XMMI_INSTRUCTIONS_AVAILABLE: u32 = 6;
const PF_XMMI64_INSTRUCTIONS_AVAILABLE: u32 = 10;
const PF_NX_ENABLED: u32 = 12;

/// `IsProcessorFeaturePresent`: lo que TODO x86-64 tiene (SSE, SSE2, NX), y
/// nada mas. PF_FASTFAIL_AVAILABLE (23) es NO a proposito: `__fastfail` es
/// `int 0x29`, y el kernel de BMO-X no la sirve; con NO, el CRT termina por
/// su otro camino (TerminateProcess). Decir NO a lo demas solo le hace tomar
/// caminos mas lentos, nunca uno roto.
extern "win64" fn is_processor_feature_present(que: u32) -> i32 {
    matches!(que, PF_XMMI_INSTRUCTIONS_AVAILABLE | PF_XMMI64_INSTRUCTIONS_AVAILABLE | PF_NX_ENABLED) as i32
}

const EXCEPTION_EXECUTE_HANDLER: i32 = 1;

/// `UnhandledExceptionFilter`: lo llama el CRT cuando algo no tiene arreglo
/// (`__report_gsfailure`, una excepcion que llega a su `__except` de arriba).
/// Se dice, y EXECUTE_HANDLER: quien llama termina el proceso. Lo que no es
/// Windows, dicho: no llama al filtro de SetUnhandledExceptionFilter ni
/// ofrece depurar.
extern "win64" fn unhandled_exception_filter(_punteros: u64) -> i32 {
    aviso("UnhandledExceptionFilter: una excepcion sin arreglo, el proceso termina");
    EXCEPTION_EXECUTE_HANDLER
}

/// Las de oleaut32.dll.
pub(crate) fn buscar_oleaut32(n: &str) -> Option<u64> {
    Some(match n {
        "GetErrorInfo" => dir!(get_error_info),
        "SysStringLen" => dir!(sys_string_len),
        "SysFreeString" => dir!(sys_free_string),
        _ => return None,
    })
}

/// Las de kernel32 (y kernelbase y los API set).
pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "FormatMessageW" => dir!(format_message_w),
        "RoOriginateErrorW" => dir!(ro_originate_error_w),
        "LocalFree" => dir!(local_free),
        "GetWindowsDirectoryW" => dir!(get_windows_directory_w),
        "GetSystemDirectoryW" => dir!(get_system_directory_w),
        "QueryDosDeviceW" => dir!(query_dos_device_w),
        "GetOverlappedResult" => dir!(get_overlapped_result),
        "CancelIo" => dir!(cancel_io),
        "DeviceIoControl" => dir!(device_io_control),
        "ReadFileEx" | "WriteFileEx" => dir!(read_file_ex),
        "ReadConsoleW" => dir!(read_console_w),
        "CreateHardLinkW" | "CreateSymbolicLinkW" => dir!(create_link_w),
        "GetProcessId" => dir!(get_process_id),
        "GetExitCodeProcess" => dir!(get_exit_code_process),
        "TerminateProcess" => dir!(terminate_process),
        "CreateProcessW" => dir!(create_process_w),
        "CreatePipe" => dir!(create_pipe),
        "InitializeProcThreadAttributeList" => dir!(initialize_proc_thread_attribute_list),
        "UpdateProcThreadAttribute" => dir!(update_proc_thread_attribute),
        "DeleteProcThreadAttributeList" => dir!(delete_proc_thread_attribute_list),
        "InitializeSListHead" => dir!(initialize_slist_head),
        "IsDebuggerPresent" => dir!(is_debugger_present),
        "IsProcessorFeaturePresent" => dir!(is_processor_feature_present),
        "UnhandledExceptionFilter" => dir!(unhandled_exception_filter),
        _ => return None,
    })
}

/// Las de `ntdll.dll`.
pub(crate) fn buscar_ntdll(n: &str) -> Option<u64> {
    Some(match n {
        "NtReadFile" => dir!(nt_read_file),
        "NtWriteFile" => dir!(nt_write_file),
        "RtlNtStatusToDosError" => dir!(rtl_nt_status_to_dos_error),
        "NtCreateFile" | "NtOpenFile" | "NtCreateNamedPipeFile" | "NtSetInformationFile" => dir!(nt_no),
        _ => return None,
    })
}

/// `bcryptprimitives.dll` y `userenv.dll`.
pub(crate) fn buscar_otras(dll: &str, n: &str) -> Option<u64> {
    if dll.eq_ignore_ascii_case("bcryptprimitives.dll") && n == "ProcessPrng" {
        return Some(dir!(process_prng));
    }
    if dll.eq_ignore_ascii_case("userenv.dll") && n == "GetUserProfileDirectoryW" {
        return Some(dir!(get_user_profile_directory_w));
    }
    None
}
