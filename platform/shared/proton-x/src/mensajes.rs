//! **Los codigos de Windows y lo que dicen** (P4f4, 27-09).
//!
//! ```text
//!    dos_de_status   RtlNtStatusToDosError: un NTSTATUS de ntdll al error
//!                    de Win32 que ve GetLastError
//!    texto           FormatMessageW(FROM_SYSTEM): el mensaje de sistema de un
//!                    error, en castellano y acabado en "\r\n" como Windows
//! ```
//!
//! La `std` de Rust los usa para `io::Error`: su `Display` es el texto de
//! FormatMessageW, y sus lecturas por NtReadFile devuelven NTSTATUS.
//!
//! Lo que no es Windows, dicho: los textos son los de Windows en castellano
//! escritos aqui A MANO (no hay un Windows de donde leerlos): la idea es la
//! misma, la letra puede no ser identica. Un codigo que no esta en la tabla
//! contesta como Windows cuando no tiene el mensaje (ERROR_MR_MID_NOT_FOUND).

pub const STATUS_SUCCESS: u32 = 0;
pub const STATUS_END_OF_FILE: u32 = 0xC000_0011;
pub const STATUS_INVALID_HANDLE: u32 = 0xC000_0008;
pub const STATUS_INVALID_PARAMETER: u32 = 0xC000_000D;
pub const STATUS_ACCESS_DENIED: u32 = 0xC000_0022;
pub const STATUS_OBJECT_NAME_NOT_FOUND: u32 = 0xC000_0034;
pub const STATUS_OBJECT_NAME_COLLISION: u32 = 0xC000_0035;
pub const STATUS_OBJECT_PATH_NOT_FOUND: u32 = 0xC000_003A;
pub const STATUS_NOT_SUPPORTED: u32 = 0xC000_00BB;
pub const STATUS_NOT_IMPLEMENTED: u32 = 0xC000_0002;

/// No hay mensaje para ese codigo (lo que da RtlNtStatusToDosError y
/// FormatMessageW cuando no saben).
pub const ERROR_MR_MID_NOT_FOUND: u32 = 317;

/// **`RtlNtStatusToDosError`**.
pub fn dos_de_status(s: u32) -> u32 {
    match s {
        STATUS_SUCCESS => 0,
        STATUS_END_OF_FILE => 38,
        STATUS_INVALID_HANDLE => 6,
        STATUS_INVALID_PARAMETER => 87,
        STATUS_ACCESS_DENIED => 5,
        STATUS_OBJECT_NAME_NOT_FOUND => 2,
        STATUS_OBJECT_NAME_COLLISION => 183,
        STATUS_OBJECT_PATH_NOT_FOUND => 3,
        STATUS_NOT_SUPPORTED => 50,
        STATUS_NOT_IMPLEMENTED => 1,
        // STATUS_PENDING y STATUS_TIMEOUT tienen su error de Win32.
        0x0000_0103 => 997,
        0x0000_0102 => 1460,
        _ => ERROR_MR_MID_NOT_FOUND,
    }
}

/// **El mensaje de sistema** de un error de Win32, sin el `\r\n` del final
/// (quien lo da lo pone, como Windows).
pub fn texto(error: u32) -> Option<&'static str> {
    Some(match error {
        0 => "La operaci\u{f3}n se complet\u{f3} correctamente.",
        1 => "Funci\u{f3}n incorrecta.",
        2 => "El sistema no puede encontrar el archivo especificado.",
        3 => "El sistema no puede encontrar la ruta especificada.",
        5 => "Acceso denegado.",
        6 => "Controlador no v\u{e1}lido.",
        8 => "No hay suficiente espacio de almacenamiento para procesar este comando.",
        18 => "No hay m\u{e1}s archivos.",
        38 => "Se ha alcanzado el final del archivo.",
        50 => "No se admite la solicitud.",
        80 => "El archivo existe.",
        87 => "El par\u{e1}metro no es correcto.",
        122 => "El \u{e1}rea de datos transferida a una llamada del sistema es demasiado peque\u{f1}a.",
        126 => "No se puede encontrar el m\u{f3}dulo especificado.",
        127 => "No se encontr\u{f3} el proceso especificado.",
        183 => "No se puede crear un archivo que ya existe.",
        203 => "El sistema no pudo encontrar la opci\u{f3}n de entorno especificada.",
        267 => "El nombre del directorio no es v\u{e1}lido.",
        1113 => "No existe asignaci\u{f3}n para el car\u{e1}cter Unicode en la p\u{e1}gina de c\u{f3}digos multibyte de destino.",
        1460 => "Esta operaci\u{f3}n ha vuelto porque el tiempo de espera expir\u{f3}.",
        _ => return None,
    })
}
