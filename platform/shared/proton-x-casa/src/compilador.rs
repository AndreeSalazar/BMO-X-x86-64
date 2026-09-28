//! **`d3dcompiler_47.dll` de la casa** (P3c2, 28-09): `D3DCompile` sin
//! compilador, pagando una vez (`bmo_proton_x::sombras`).
//!
//! ```text
//!    D3DCompile   la huella del pedido -> <dir del .exe>/sombras/<huella>.cso
//!                 esta: el ID3DBlob con esos bytes, S_OK
//!                 no esta: deja <huella>.hls y <huella>.ent al lado, y
//!                 contesta E_FAIL con un blob de errores que dice que falta y
//!                 como hacerlo (el juego suele imprimirlo)
//! ```
//!
//! La carpeta `sombras` la crea el build (`ejemplos.ps1`): el FAT32 de BMO-X
//! no crea carpetas desde Ring 3. `sombras.exe` (en Windows) compila lo
//! pendiente con el `d3dcompiler_47` de ese Windows.
//!
//! Lo que no es Windows, dicho: los `#include` no viajan (se avisa si se pide
//! un ID3DInclude propio), y el blob de errores de un fallo de verdad del
//! HLSL lo da `sombras.exe` en Windows, no aqui.

use alloc::vec::Vec;

use bmo_proton_x::sombras::{self, Pedido};

use crate::{aviso, dir, ficheros, plataforma, tuberia};

const E_FAIL: i32 = 0x8000_4005_u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
/// `D3D_COMPILE_STANDARD_FILE_INCLUDE`: ((ID3DInclude*)1).
const INCLUDE_ESTANDAR: u64 = 1;

/// Una cadena C del `.exe` (sin el 0), o vacia si es NULL.
fn cadena(p: *const u8) -> Vec<u8> {
    let mut v = Vec::new();
    if p.is_null() {
        return v;
    }
    // SAFETY: una cadena del `.exe` acabada en 0.
    unsafe {
        while v.len() < 4096 && *p.add(v.len()) != 0 {
            v.push(*p.add(v.len()));
        }
    }
    v
}

/// Las `D3D_SHADER_MACRO` (dos punteros cada una) hasta la de NULL.
fn macros(p: *const u64) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut v = Vec::new();
    if p.is_null() {
        return v;
    }
    let mut i = 0;
    loop {
        // SAFETY: el arreglo del `.exe`, acabado en {NULL, NULL}.
        let (n, d) = unsafe { (p.add(2 * i).read(), p.add(2 * i + 1).read()) };
        if n == 0 || i > 256 {
            return v;
        }
        v.push((cadena(n as *const u8), cadena(d as *const u8)));
        i += 1;
    }
}

fn dar(pp: *mut u64, v: u64) {
    if !pp.is_null() {
        // SAFETY: un puntero del `.exe` donde dejar un ID3DBlob*.
        unsafe { *pp = v };
    }
}

/// `D3DCompile(fuente, n, nombre, macros, include, entrada, perfil, f1, f2, **codigo, **errores)`.
extern "win64" fn d3d_compile(
    fuente: *const u8,
    n: usize,
    _nombre: *const u8,
    defs: *const u64,
    include: u64,
    entrada: *const u8,
    perfil: *const u8,
    f1: u32,
    f2: u32,
    codigo: *mut u64,
    errores: *mut u64,
) -> i32 {
    dar(codigo, 0);
    dar(errores, 0);
    if fuente.is_null() || n == 0 || codigo.is_null() {
        return E_INVALIDARG;
    }
    if include != 0 && include != INCLUDE_ESTANDAR {
        aviso("D3DCompile con un ID3DInclude propio: los #include no viajan a Windows");
    }
    // SAFETY: `n` bytes de fuente del `.exe`.
    let src = unsafe { core::slice::from_raw_parts(fuente, n) };
    let p = Pedido { entrada: cadena(entrada), perfil: cadena(perfil), banderas1: f1, banderas2: f2, macros: macros(defs) };
    let h = sombras::nombre(sombras::huella(src, &p));
    let dir = ficheros::directorio();
    let base = if dir.is_empty() { alloc::format!("sombras/{h}") } else { alloc::format!("{dir}/sombras/{h}") };
    let pl = plataforma();
    if let Some(b) = (pl.leer_fichero)(alloc::format!("{base}.cso").as_bytes()) {
        dar(codigo, tuberia::blob(b));
        return 0;
    }
    // Pendiente: la fuente y el pedido, para sombras.exe.
    let dejado = (pl.escribir_fichero)(alloc::format!("{base}.hls").as_bytes(), src) && (pl.escribir_fichero)(alloc::format!("{base}.ent").as_bytes(), &sombras::escribir_ent(&p));
    let que = alloc::format!(
        "PROTON-X: D3DCompile({}, {}) sin compilar todavia: {} -- en Windows, `sombras.exe A:\\{}\\sombras` y otra vez aqui",
        core::str::from_utf8(&p.entrada).unwrap_or("?"),
        core::str::from_utf8(&p.perfil).unwrap_or("?"),
        if dejado { alloc::format!("dejado en {base}.hls") } else { alloc::format!("y NO se pudo dejar en {base}.hls (falta la carpeta sombras?)") },
        if dir.is_empty() { "" } else { dir.as_str() },
    );
    aviso(que.trim_start_matches("PROTON-X: "));
    dar(errores, tuberia::blob(que.into_bytes()));
    E_FAIL
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "D3DCompile" => dir!(d3d_compile),
        _ => return None,
    })
}
