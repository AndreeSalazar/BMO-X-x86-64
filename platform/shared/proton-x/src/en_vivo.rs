//! **Las DLL que el juego carga EN VIVO** (04-10, N4.4): de las que hay
//! junto al `.exe`, cuales NOMBRA el juego en sus datos.
//!
//! [carril]  VERDE     lee bytes; no toca la maquina
//! [cuesta]  DATO      una DLL de mas corre su DllMain sin que nadie la
//!                     pida; una de menos es un LoadLibrary que da NULL
//! [riesgo]  ESPEJO    en Windows, `LoadLibrary("x")` abre `x.dll` de la
//!                     carpeta del `.exe`; aqui se carga ANTES, al arrancar,
//!                     porque la imagen se declara al kernel de una vez
//! [consumo] NADA      una vez al arrancar, sobre las secciones de datos
//!
//! El salto a 0 de diez corridas era esto: Cyberpunk hace
//! `LoadLibraryA("amd_fidelityfx_dx12")` (FSR 3.1), guarda
//! `GetProcAddress(h, "ffxDispatch")` en una variable, y la llama SIN mirar
//! si es nula -- en Windows esa DLL siempre esta junto al `.exe`. PROTON-X
//! solo cargaba las DLL de la tabla de importaciones, y su LoadLibrary daba
//! NULL a todas las demas.
//!
//! La regla: una DLL de la carpeta del `.exe` se carga si su nombre (con o
//! sin `.dll`) aparece en los datos del juego como palabra entera, en ASCII
//! o en UTF-16. Es lo que puede pedir un LoadLibrary; lo que no se nombra no
//! se carga.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

fn vale(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.'
}

/// El nombre sin `.dll` y en minuscula: `"AMD_FidelityFX_DX12.dll"` ->
/// `"amd_fidelityfx_dx12"`.
pub fn tallo(dll: &str) -> String {
    let d = dll.to_ascii_lowercase();
    match d.strip_suffix(".dll") {
        Some(t) => String::from(t),
        None => d,
    }
}

/// Cada cuantos bytes se llama a `ceder` (el turno al escritorio).
const CEDER_CADA: usize = 1 << 20;

/// **Cuales de `candidatos` nombra `datos`**: un `bool` por candidato, en su
/// orden. `ceder` se llama cada MiB mirado.
pub fn nombrados(datos: &[u8], candidatos: &[String], ceder: &mut dyn FnMut()) -> Vec<bool> {
    let mut tallos: Vec<(String, usize)> = candidatos.iter().enumerate().map(|(i, c)| (tallo(c), i)).collect();
    tallos.sort();
    let mut si = vec![false; candidatos.len()];
    let mirar = |palabra: &[u8], si: &mut Vec<bool>| {
        if palabra.len() < 3 || palabra.len() > 80 {
            return;
        }
        let p = String::from_utf8_lossy(palabra).to_ascii_lowercase();
        let t = p.strip_suffix(".dll").unwrap_or(&p);
        if let Ok(k) = tallos.binary_search_by(|(x, _)| x.as_str().cmp(t)) {
            si[tallos[k].1] = true;
        }
    };
    // ASCII: tiradas de letras de nombre.
    let mut a = 0;
    for i in 0..=datos.len() {
        if i % CEDER_CADA == 0 && i > 0 {
            ceder();
        }
        if i < datos.len() && vale(datos[i]) {
            continue;
        }
        if i > a {
            mirar(&datos[a..i], &mut si);
        }
        a = i + 1;
    }
    // UTF-16 (alineado a 2): tiradas de (letra, 0).
    let mut palabra: Vec<u8> = Vec::new();
    for i in (0..datos.len().saturating_sub(1)).step_by(2) {
        if i % CEDER_CADA == 0 && i > 0 {
            ceder();
        }
        if datos[i + 1] == 0 && vale(datos[i]) {
            palabra.push(datos[i]);
            continue;
        }
        mirar(&palabra, &mut si);
        palabra.clear();
    }
    mirar(&palabra, &mut si);
    si
}
