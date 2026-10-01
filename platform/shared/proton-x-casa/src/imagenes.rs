//! **LAS IMAGENES DE LA CASA** -- un HMODULE que es una imagen PE de verdad
//! (01-10, Cyberpunk).
//!
//! En Windows un HMODULE ES la direccion de la imagen cargada, y hay codigo
//! que la LEE: `sl.interposer.dll` (NVIDIA Streamline, en Cyberpunk) pide
//! `dxgi.dll` y mira su cabecera -- `handle + 0x3C`, el `e_lfanew` -- para
//! recorrer sus exportaciones. Los handles de la casa eran numeros sin nada
//! detras (`0x5A1DD...`): leer ahi era un fallo de pagina y el juego caia.
//!
//! Ahora cada DLL de la casa tiene su imagen, todas en UN bloque sellado
//! (leer y ejecutar, W^X), hecho la primera vez que alguien pide un modulo:
//!
//! ```text
//!    +0x000   cabecera DOS ("MZ", e_lfanew = 0x80)
//!    +0x080   "PE\0\0", FileHeader (x64, DLL) y OptionalHeader64
//!    +0x188   una seccion .text
//!    +0x1000  un trampolin por funcion: jmp [rip+0] y la direccion de la casa
//!    detras   el directorio de exportaciones: nombres EN ORDEN, ordinales y
//!             RVA de cada trampolin, y el nombre de la DLL
//! ```
//!
//! Que exporta cada una: lo que el `.exe` y sus DLL IMPORTARON de ella (se
//! apunta al resolver, `apuntar`) y una lista corta de lo que se busca a mano
//! (dxgi y d3d12). Sin bloque sellado, los handles de siempre: nada se rompe.
//!
//! [!] `ImageBase` dice la base preferida de una DLL de x64 (0x180000000) y no
//! la real: el bloque se sella con los bytes ya escritos. Windows la corrige
//! al cargar; quien la lea aqui vera la preferida.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::modulos::DLL;

/// Cada imagen empieza en un multiplo de esto.
const ALINEA: usize = 0x1000;
const CABECERAS: usize = 0x1000;
const TRAMPOLIN: usize = 16;

/// Lo que se busca con GetProcAddress sin importarlo (o recorriendo la
/// imagen, como Streamline): siempre en su imagen si la casa lo tiene.
const A_MANO: &[(&str, &[&str])] = &[
    ("dxgi.dll", &["CreateDXGIFactory", "CreateDXGIFactory1", "CreateDXGIFactory2", "DXGIGetDebugInterface1", "DXGIDeclareAdapterRemovalSupport"]),
    (
        "d3d12.dll",
        &[
            "D3D12CreateDevice",
            "D3D12GetDebugInterface",
            "D3D12SerializeRootSignature",
            "D3D12SerializeVersionedRootSignature",
            "D3D12CreateRootSignatureDeserializer",
            "D3D12CreateVersionedRootSignatureDeserializer",
            "D3D12EnableExperimentalFeatures",
            "D3D12GetInterface",
        ],
    ),
];

struct Estado {
    /// (indice en DLL, nombre, direccion) de cada funcion resuelta.
    apuntadas: Vec<(usize, String, u64)>,
    /// La base de la imagen de cada DLL, ya hechas; vacio si todavia no.
    bases: Vec<u64>,
    /// (indice, nombre, trampolin): lo que GetProcAddress contesta, igual que
    /// quien recorre la tabla (como en Windows).
    trampolines: Vec<(usize, String, u64)>,
    /// Ya se intento (y quiza no hubo bloque: entonces, los handles viejos).
    intentado: bool,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { apuntadas: Vec::new(), bases: Vec::new(), trampolines: Vec::new(), intentado: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Global`.
    unsafe { &mut *ESTADO.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.apuntadas.clear();
    e.bases.clear();
    e.trampolines.clear();
    e.intentado = false;
}

/// El indice en `DLL` de una DLL de la casa (por su fichero, sin ruta).
fn indice(dll: &str) -> Option<usize> {
    let f = bmo_proton_x::dll::fichero(dll);
    DLL.iter().position(|d| d.eq_ignore_ascii_case(&f))
}

/// **El resolver resolvio `nombre` de `dll` en `dir`**: va a su imagen.
pub(crate) fn apuntar(dll: &str, nombre: &str, dir: u64) {
    let e = estado();
    if e.intentado {
        return;
    }
    if let Some(i) = indice(dll) {
        if !e.apuntadas.iter().any(|(j, n, _)| *j == i && n == nombre) {
            e.apuntadas.push((i, String::from(nombre), dir));
        }
    }
}

fn pon16(v: &mut [u8], en: usize, x: u16) {
    v[en..en + 2].copy_from_slice(&x.to_le_bytes());
}
fn pon32(v: &mut [u8], en: usize, x: u32) {
    v[en..en + 4].copy_from_slice(&x.to_le_bytes());
}
fn pon64(v: &mut [u8], en: usize, x: u64) {
    v[en..en + 8].copy_from_slice(&x.to_le_bytes());
}

/// Una imagen: sus bytes, para `nombre` y sus funciones (ya en orden).
fn imagen(nombre: &str, funciones: &[(String, u64)]) -> Vec<u8> {
    let n = funciones.len();
    let trampolines = CABECERAS;
    let dir_exp = trampolines + n * TRAMPOLIN;
    let funcs = dir_exp + 40;
    let nombres = funcs + 4 * n;
    let ordinales = nombres + 4 * n;
    let mut cadenas = ordinales + 2 * n;
    let largo_cadenas: usize = nombre.len() + 1 + funciones.iter().map(|(f, _)| f.len() + 1).sum::<usize>();
    let fin = (cadenas + largo_cadenas).next_multiple_of(ALINEA);
    let mut v = alloc::vec![0u8; fin];

    // -- DOS y NT --
    v[0] = b'M';
    v[1] = b'Z';
    pon32(&mut v, 0x3C, 0x80);
    v[0x80..0x84].copy_from_slice(b"PE\0\0");
    let fh = 0x84;
    pon16(&mut v, fh, 0x8664); // Machine: x64
    pon16(&mut v, fh + 2, 1); // una seccion
    pon16(&mut v, fh + 16, 0xF0); // SizeOfOptionalHeader
    pon16(&mut v, fh + 18, 0x2022); // EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE | DLL
    let oh = fh + 20;
    pon16(&mut v, oh, 0x20B); // PE32+
    v[oh + 2] = 14; // el enlazador de MSVC 14
    pon32(&mut v, oh + 4, (fin - CABECERAS) as u32); // SizeOfCode
    pon32(&mut v, oh + 20, CABECERAS as u32); // BaseOfCode
    pon64(&mut v, oh + 24, 0x1_8000_0000); // ImageBase (la preferida: ver cabecera)
    pon32(&mut v, oh + 32, ALINEA as u32); // SectionAlignment
    pon32(&mut v, oh + 36, 0x200); // FileAlignment
    pon16(&mut v, oh + 40, 10); // MajorOperatingSystemVersion
    pon16(&mut v, oh + 48, 10); // MajorSubsystemVersion
    pon32(&mut v, oh + 56, fin as u32); // SizeOfImage
    pon32(&mut v, oh + 60, 0x400); // SizeOfHeaders
    pon16(&mut v, oh + 68, 3); // Subsystem: consola
    pon16(&mut v, oh + 70, 0x0160); // DYNAMIC_BASE | NX_COMPAT | HIGH_ENTROPY_VA
    pon64(&mut v, oh + 72, 0x40000); // SizeOfStackReserve
    pon64(&mut v, oh + 80, 0x1000);
    pon64(&mut v, oh + 88, 0x100000);
    pon64(&mut v, oh + 96, 0x1000);
    pon32(&mut v, oh + 108, 16); // NumberOfRvaAndSizes
    pon32(&mut v, oh + 112, dir_exp as u32); // DataDirectory[EXPORT]
    pon32(&mut v, oh + 116, (fin - dir_exp) as u32);
    // -- la seccion --
    let sh = oh + 0xF0;
    v[sh..sh + 5].copy_from_slice(b".text");
    pon32(&mut v, sh + 8, (fin - CABECERAS) as u32); // VirtualSize
    pon32(&mut v, sh + 12, CABECERAS as u32); // VirtualAddress
    pon32(&mut v, sh + 16, (fin - CABECERAS) as u32); // SizeOfRawData
    pon32(&mut v, sh + 20, 0x400); // PointerToRawData
    pon32(&mut v, sh + 36, 0x6000_0020); // CODE | EXECUTE | READ

    // -- el directorio de exportaciones --
    pon32(&mut v, dir_exp + 12, cadenas as u32); // Name
    pon32(&mut v, dir_exp + 16, 1); // Base de los ordinales
    pon32(&mut v, dir_exp + 20, n as u32); // NumberOfFunctions
    pon32(&mut v, dir_exp + 24, n as u32); // NumberOfNames
    pon32(&mut v, dir_exp + 28, funcs as u32);
    pon32(&mut v, dir_exp + 32, nombres as u32);
    pon32(&mut v, dir_exp + 36, ordinales as u32);
    v[cadenas..cadenas + nombre.len()].copy_from_slice(nombre.as_bytes());
    cadenas += nombre.len() + 1;
    for (k, (f, dir)) in funciones.iter().enumerate() {
        // El trampolin: jmp qword [rip+0], y detras la direccion.
        let t = trampolines + k * TRAMPOLIN;
        v[t..t + 6].copy_from_slice(&[0xFF, 0x25, 0, 0, 0, 0]);
        pon64(&mut v, t + 6, *dir);
        v[t + 14] = 0xCC;
        v[t + 15] = 0xCC;
        pon32(&mut v, funcs + 4 * k, t as u32);
        pon32(&mut v, nombres + 4 * k, cadenas as u32);
        pon16(&mut v, ordinales + 2 * k, k as u16);
        v[cadenas..cadenas + f.len()].copy_from_slice(f.as_bytes());
        cadenas += f.len() + 1;
    }
    v
}

/// Hace todas las imagenes en un bloque. `false` si no hubo bloque.
fn hacer() -> bool {
    let e = estado();
    e.intentado = true;
    let apuntadas = core::mem::take(&mut e.apuntadas);
    let mut todo: Vec<u8> = Vec::new();
    let mut desde = Vec::with_capacity(DLL.len());
    for (i, dll) in DLL.iter().enumerate() {
        let mut f: Vec<(String, u64)> = apuntadas.iter().filter(|(j, _, _)| *j == i).map(|(_, n, d)| (n.clone(), *d)).collect();
        for (d, nombres) in A_MANO {
            if dll.eq_ignore_ascii_case(d) {
                for n in nombres.iter() {
                    if f.iter().all(|(x, _)| x != n) {
                        if let Some(dir) = crate::tabla(dll, &bmo_proton_x::Funcion::Nombre((*n).into())) {
                            f.push((String::from(*n), dir));
                        }
                    }
                }
            }
        }
        // Windows (y quien recorre la tabla) busca por biseccion: en orden.
        f.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        desde.push((todo.len(), f.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()));
        todo.extend_from_slice(&imagen(dll, &f));
    }
    // `tabla` (arriba) ya no apunta: `intentado` esta puesto.
    let e = estado();
    e.apuntadas.clear();
    let Some(base) = (crate::plataforma().sellar_codigo)(&todo) else {
        return false;
    };
    for (i, (o, nombres)) in desde.into_iter().enumerate() {
        let b = base + o as u64;
        e.bases.push(b);
        for (k, n) in nombres.into_iter().enumerate() {
            e.trampolines.push((i, n, b + (CABECERAS + k * TRAMPOLIN) as u64));
        }
    }
    true
}

/// **La base de la imagen de la DLL `i`**, o `None` si no se pudo hacer.
pub(crate) fn base(i: usize) -> Option<u64> {
    let e = estado();
    if !e.intentado && !hacer() {
        return None;
    }
    estado().bases.get(i).copied()
}

/// La DLL cuya imagen empieza en `h`.
pub(crate) fn de_base(h: u64) -> Option<usize> {
    estado().bases.iter().position(|&b| b == h)
}

/// **Lo que exporta la imagen de la DLL `i` con ese nombre**: su trampolin.
pub(crate) fn exportada(i: usize, nombre: &str) -> Option<u64> {
    estado().trampolines.iter().find(|(j, n, _)| *j == i && n == nombre).map(|t| t.2)
}
