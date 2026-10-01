//! **SECUR32 DE LA CASA** -- la interfaz SSPI, sin paquetes (01-10).
//!
//! Por Cyberpunk. Con la red local ya cosida, Galaxy llega mas lejos: en un
//! hilo suyo pregunta la version de Windows, mira la carpeta del sistema y
//! carga `C:\Windows\System32\secur32.dll`. Es el arranque de libcurl
//! (`Curl_sspi_global_init`): carga la DLL, pide `InitSecurityInterfaceW` y
//! exige una TABLA de funciones; sin DLL, curl no arranca, Galaxy lanza una
//! excepcion que nadie atrapa y el juego aborta (exit 3).
//!
//! Esto es **un Windows sin paquetes de seguridad**, no un Schannel
//! inventado: la tabla existe y cada entrada contesta lo que contestaria un
//! Windows al que le faltan Kerberos, NTLM y Schannel:
//!
//! ```text
//!    InitSecurityInterfaceW/A   la tabla (version 4, la de Windows 10)
//!    EnumerateSecurityPackages  0 paquetes, SEC_E_OK
//!    QuerySecurityPackageInfo   SEC_E_SECPKG_NOT_FOUND, *info = NULL
//!    AcquireCredentialsHandle   SEC_E_SECPKG_NOT_FOUND (nota en el diario)
//!    FreeContextBuffer          SEC_E_OK (la casa nunca da buffers)
//!    lo que pide un handle      SEC_E_INVALID_HANDLE: nunca hubo ninguno
//!    lo demas                   SEC_E_UNSUPPORTED_FUNCTION
//! ```
//!
//! La disposicion de la tabla es la de `SecurityFunctionTableW` de `sspi.h`
//! (la ABI publica): un `dwVersion` y 31 punteros, los "Reserved" a NULL.
//! secur32 y sspicli exportan lo mismo (en Windows una reenvia a la otra).

use crate::{diario, dir};

const SEC_E_OK: u32 = 0;
const SEC_E_INVALID_HANDLE: u32 = 0x8009_0301;
const SEC_E_UNSUPPORTED_FUNCTION: u32 = 0x8009_0302;
const SEC_E_SECPKG_NOT_FOUND: u32 = 0x8009_0305;

/// La version de la tabla: hasta ChangeAccountPasswordW, como Windows 10.
const VERSION: u32 = 4;

type F = extern "win64" fn() -> u32;

/// `SecurityFunctionTableW` (y la A, que tiene la misma forma).
#[repr(C)]
pub(crate) struct Tabla {
    version: u32,
    f: [Option<F>; 31],
}

fn no_esta() -> u32 {
    SEC_E_SECPKG_NOT_FOUND
}

extern "win64" fn sin_handle() -> u32 {
    SEC_E_INVALID_HANDLE
}

extern "win64" fn no_soportada() -> u32 {
    SEC_E_UNSUPPORTED_FUNCTION
}

extern "win64" fn bien() -> u32 {
    SEC_E_OK
}

extern "win64" fn enumerar(cuantos: *mut u32, info: *mut u64) -> u32 {
    // SAFETY: punteros del llamante (si los da).
    unsafe {
        if !cuantos.is_null() {
            *cuantos = 0;
        }
        if !info.is_null() {
            *info = 0;
        }
    }
    SEC_E_OK
}

extern "win64" fn info_de_paquete(_nombre: *const u8, info: *mut u64) -> u32 {
    if !info.is_null() {
        // SAFETY: puntero del llamante.
        unsafe { *info = 0 };
    }
    no_esta()
}

extern "win64" fn credenciales_w(_principal: *const u16, paquete: *const u16) -> u32 {
    if diario::encendido() && !paquete.is_null() {
        let mut v = alloc::vec::Vec::new();
        // SAFETY: cadena del llamante terminada en 0.
        unsafe {
            while v.len() < 64 && *paquete.add(v.len()) != 0 {
                v.push(*paquete.add(v.len()));
            }
        }
        diario::nota(&alloc::format!("sspi: AcquireCredentialsHandle(\"{}\"): sin paquetes", alloc::string::String::from_utf16_lossy(&v)));
    }
    no_esta()
}

extern "win64" fn credenciales_a() -> u32 {
    diario::nota("sspi: AcquireCredentialsHandleA: sin paquetes");
    no_esta()
}

/// Lo de una tabla, con las funciones del juego W o A en sus sitios.
const fn tabla(enumera: F, info: F, credenciales: F) -> Tabla {
    let s = Some(sin_handle as F);
    let n = Some(no_soportada as F);
    Tabla {
        version: VERSION,
        f: [
            Some(enumera), // EnumerateSecurityPackages
            s,             // QueryCredentialsAttributes
            Some(credenciales),
            s,             // FreeCredentialsHandle
            None,          // Reserved2
            s,             // InitializeSecurityContext
            s,             // AcceptSecurityContext
            s,             // CompleteAuthToken
            s,             // DeleteSecurityContext
            s,             // ApplyControlToken
            s,             // QueryContextAttributes
            s,             // ImpersonateSecurityContext
            s,             // RevertSecurityContext
            s,             // MakeSignature
            s,             // VerifySignature
            Some(bien as F), // FreeContextBuffer
            Some(info),    // QuerySecurityPackageInfo
            None,          // Reserved3
            None,          // Reserved4
            s,             // ExportSecurityContext
            n,             // ImportSecurityContext
            s,             // AddCredentials
            None,          // Reserved8
            s,             // QuerySecurityContextToken
            s,             // EncryptMessage
            s,             // DecryptMessage
            s,             // SetContextAttributes
            s,             // SetCredentialsAttributes
            n,             // ChangeAccountPassword
            s,             // QueryContextAttributesEx
            s,             // QueryCredentialsAttributesEx
        ],
    }
}

// En win64 quien llama limpia la pila: una entrada que no lee sus
// argumentos no los necesita declarar, y una que si los lee entra en la
// tabla con su firma de verdad borrada a `F`.
static TABLA_W: Tabla = tabla(
    // SAFETY: misma convencion; el llamante pasa la firma de Windows.
    unsafe { core::mem::transmute::<extern "win64" fn(*mut u32, *mut u64) -> u32, F>(enumerar) },
    unsafe { core::mem::transmute::<extern "win64" fn(*const u8, *mut u64) -> u32, F>(info_de_paquete) },
    unsafe { core::mem::transmute::<extern "win64" fn(*const u16, *const u16) -> u32, F>(credenciales_w) },
);
static TABLA_A: Tabla = tabla(
    unsafe { core::mem::transmute::<extern "win64" fn(*mut u32, *mut u64) -> u32, F>(enumerar) },
    unsafe { core::mem::transmute::<extern "win64" fn(*const u8, *mut u64) -> u32, F>(info_de_paquete) },
    credenciales_a,
);

extern "win64" fn init_security_interface_w() -> *const Tabla {
    diario::nota("sspi: InitSecurityInterfaceW: la tabla, sin paquetes");
    &TABLA_W
}

extern "win64" fn init_security_interface_a() -> *const Tabla {
    &TABLA_A
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "InitSecurityInterfaceW" => dir!(init_security_interface_w),
        "InitSecurityInterfaceA" => dir!(init_security_interface_a),
        "EnumerateSecurityPackagesW" | "EnumerateSecurityPackagesA" => dir!(enumerar),
        "QuerySecurityPackageInfoW" | "QuerySecurityPackageInfoA" => dir!(info_de_paquete),
        "AcquireCredentialsHandleW" => dir!(credenciales_w),
        "AcquireCredentialsHandleA" => dir!(credenciales_a),
        "FreeContextBuffer" => dir!(bien),
        "ImportSecurityContextW" | "ImportSecurityContextA" | "ChangeAccountPasswordW" | "ChangeAccountPasswordA" => dir!(no_soportada),
        "FreeCredentialsHandle" | "InitializeSecurityContextW" | "InitializeSecurityContextA" | "AcceptSecurityContext" | "CompleteAuthToken"
        | "DeleteSecurityContext" | "ApplyControlToken" | "QueryContextAttributesW" | "QueryContextAttributesA" | "MakeSignature" | "VerifySignature"
        | "EncryptMessage" | "DecryptMessage" | "QueryCredentialsAttributesW" | "QueryCredentialsAttributesA" => dir!(sin_handle),
        _ => return None,
    })
}
