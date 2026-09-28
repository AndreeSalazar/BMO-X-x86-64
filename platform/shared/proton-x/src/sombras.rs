//! **`D3DCompile` pagando una vez** (P3c2, 28-09): el HLSL que un `.exe`
//! compila en marcha, compilado UNA vez en Windows y guardado.
//!
//! En BMO-X no hay compilador de HLSL (el de Microsoft es `d3dcompiler_47`,
//! cerrado). Lo honesto es lo que la casa ya hace con todo: pagar una vez.
//!
//! ```text
//!    BMO-X    D3DCompile(fuente, entrada, perfil, banderas, macros)
//!             -> la HUELLA de todo eso: 8 cifras hexadecimales (8.3)
//!             -> window/sombras/<huella>.cso esta?  el blob, S_OK
//!             -> no esta: deja <huella>.hls (la fuente) y <huella>.ent (lo
//!                demas) y contesta E_FAIL diciendo que falta
//!    Windows  sombras.exe A:\window\sombras   compila cada .hls pendiente con
//!             el d3dcompiler_47 de ESE Windows -- el mismo que usa el juego:
//!             los bytes son los que el juego tendria
//! ```
//!
//! Aqui va lo que se dice sin punteros: la huella (FNV-1a de 32 bits sobre
//! todo lo que cambia la salida) y el `.ent` (texto: la entrada, el perfil,
//! las dos banderas en hexadecimal y una macro `N=V` por linea).
//!
//! Lo que no es Windows, dicho: 32 bits de huella (dos compilaciones
//! distintas con la misma son improbables con los pocos sombreadores de un
//! juego, no imposibles: `sombras.exe` guarda la fuente al lado y se puede
//! comprobar), y los `#include` no viajan (se avisa).

use alloc::string::String;
use alloc::vec::Vec;

/// Lo que se le pide a `D3DCompile`, sin la fuente.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pedido {
    pub entrada: Vec<u8>,
    pub perfil: Vec<u8>,
    pub banderas1: u32,
    pub banderas2: u32,
    /// Las macros `D3D_SHADER_MACRO`, en su orden.
    pub macros: Vec<(Vec<u8>, Vec<u8>)>,
}

fn fnv(mut h: u32, b: &[u8]) -> u32 {
    for &c in b {
        h ^= c as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// **La huella**: FNV-1a sobre la fuente y el pedido, con un 0 entre cada
/// trozo (para que "ab"+"c" no sea "a"+"bc").
pub fn huella(fuente: &[u8], p: &Pedido) -> u32 {
    let mut h = 0x811C_9DC5u32;
    for trozo in [fuente, &p.entrada, &p.perfil, &p.banderas1.to_le_bytes(), &p.banderas2.to_le_bytes()] {
        h = fnv(h, trozo);
        h = fnv(h, &[0]);
    }
    for (n, v) in &p.macros {
        h = fnv(h, n);
        h = fnv(h, b"=");
        h = fnv(h, v);
        h = fnv(h, &[0]);
    }
    h
}

/// El nombre 8.3 de una huella, sin extension: `1a2b3c4d`.
pub fn nombre(h: u32) -> String {
    alloc::format!("{h:08x}")
}

/// **El `.ent`**: el pedido, en texto que `sombras.exe` lee.
pub fn escribir_ent(p: &Pedido) -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(&p.entrada);
    t.push(b'\n');
    t.extend_from_slice(&p.perfil);
    t.push(b'\n');
    t.extend_from_slice(alloc::format!("{:x}\n{:x}\n", p.banderas1, p.banderas2).as_bytes());
    for (n, v) in &p.macros {
        t.extend_from_slice(n);
        t.push(b'=');
        t.extend_from_slice(v);
        t.push(b'\n');
    }
    t
}

/// Leer un `.ent` (lo que escribe [`escribir_ent`]); `None` si no lo es.
pub fn leer_ent(t: &[u8]) -> Option<Pedido> {
    let mut l = t.split(|&c| c == b'\n').map(|x| x.strip_suffix(b"\r").unwrap_or(x));
    let entrada = l.next()?.to_vec();
    let perfil = l.next()?.to_vec();
    let hex = |x: &[u8]| u32::from_str_radix(core::str::from_utf8(x).ok()?, 16).ok();
    let banderas1 = hex(l.next()?)?;
    let banderas2 = hex(l.next()?)?;
    let mut macros = Vec::new();
    for x in l.filter(|x| !x.is_empty()) {
        let k = x.iter().position(|&c| c == b'=')?;
        macros.push((x[..k].to_vec(), x[k + 1..].to_vec()));
    }
    (!entrada.is_empty() && !perfil.is_empty()).then_some(Pedido { entrada, perfil, banderas1, banderas2, macros })
}
