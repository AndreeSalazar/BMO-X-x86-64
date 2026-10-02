//! **La FALLA DOCUMENTADA** (tanda 48, 02-10): la tercera categoria.
//!
//! Hasta hoy un hueco de vtabla era una de dos cosas: lo que la casa HACE, o
//! un `falta` que dice cual es y SALE (`0xC0DE....`). La investigacion de
//! como Proton domo a Cyberpunk (`reports/`, la leccion 4) enseno que hay
//! una tercera, y que el juego depende de ella: metodos donde FALLAR con un
//! HRESULT es lo que Windows hace en una maquina sin esa funcion, y el juego
//! lo sabe llevar. Cyberpunk llama a `OpenExistingHeapFromFileMapping` sin
//! mirar antes `D3D12_FEATURE_EXISTING_HEAPS`, y vkd3d-proton tuvo que
//! "fallar correctamente" para que arrancara (commit 9df19c91, 2026).
//!
//! ```text
//!    hace              la casa lo hace, como Windows
//!    falla documentada  devuelve el HRESULT de Windows sin esa funcion, deja
//!                      a NULL el puntero de salida, y lo DICE (una linea,
//!                      la primera vez): nunca un stub callado
//!    falta             dice cual es y sale: lo que la casa todavia no sabe
//!                      si se puede fallar
//! ```
//!
//! La regla de la casa sigue en pie: "nunca un stub callado". Una falla
//! documentada no finge que hizo nada: dice que no, con el numero de Windows,
//! y queda escrita. Cada fila de [`FALLAS`] lleva su POR QUE: la funcion que
//! la casa anuncia que NO tiene (CheckFeatureSupport lo dice igual), o el
//! comportamiento de Windows que se copia.
//!
//! Los metodos `void` que entran aqui no hacen nada y lo dicen: son los de
//! funciones que la casa anuncia como ausentes (rayos, malla, tiles), que un
//! juego honesto no llama.

use crate::com::{self, INTERFACES};

pub(crate) const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
pub(crate) const E_NOTIMPL: i32 = 0x8000_4001_u32 as i32;
const DXGI_ERROR_NOT_CURRENTLY_AVAILABLE: i32 = 0x887A_0022_u32 as i32;
/// Un metodo `void`: no hay HRESULT que devolver.
const VOID: i32 = 0;

/// Una falla documentada: la interfaz, el hueco, lo que devuelve, que
/// argumento es el puntero de salida a poner a NULL (0 = ninguno; `this` es
/// el 0), y por que.
pub(crate) struct Falla {
    pub interfaz: usize,
    pub hueco: usize,
    pub hr: i32,
    pub salida: usize,
    pub por_que: &'static str,
}

const fn f(interfaz: usize, hueco: usize, hr: i32, salida: usize, por_que: &'static str) -> Falla {
    Falla { interfaz, hueco, hr, salida, por_que }
}

const RESERVADOS: &str = "recursos reservados (tiled): la casa anuncia TiledResourcesTier NOT_SUPPORTED";
const COMPARTIDOS: &str = "compartir entre procesos o adaptadores: la casa no tiene";
const PROTEGIDOS: &str = "contenido protegido: la casa anuncia ProtectedResourceSessionSupport NONE";
const META: &str = "meta-ordenes: EnumerateMetaCommands dice cero";
const RAYOS: &str = "rayos: la casa anuncia RaytracingTier NOT_SUPPORTED";
const MALLA: &str = "sombreadores de malla: la casa anuncia MeshShaderTier NOT_SUPPORTED";
const PANTALLA: &str = "la gamma y la superficie del monitor solo existen a pantalla completa exclusiva: en ventana, Windows dice NOT_CURRENTLY_AVAILABLE";

/// **Todas las fallas documentadas de la casa.** Lo que no esta aqui ni lo
/// hace la casa, sale con su `0xC0DE....`.
pub(crate) static FALLAS: &[Falla] = &[
    // ID3D12Device
    f(com::DEVICE, 30, E_INVALIDARG, 5, RESERVADOS),
    f(com::DEVICE, 31, E_NOTIMPL, 5, COMPARTIDOS),
    f(com::DEVICE, 32, E_INVALIDARG, 3, COMPARTIDOS),
    f(com::DEVICE, 33, E_NOTIMPL, 3, COMPARTIDOS),
    f(com::DEVICE, 49, E_NOTIMPL, 3, "un monton desde un FileMapping: como vkd3d-proton fuera de Win32 (Cyberpunk lo llama sin mirar EXISTING_HEAPS y sigue)"),
    f(com::DEVICE, 52, E_NOTIMPL, 3, PROTEGIDOS),
    f(com::DEVICE, 55, E_INVALIDARG, 6, RESERVADOS),
    f(com::DEVICE, 57, E_NOTIMPL, 3, "rastreadores de vida (lifetime trackers): la casa no tiene"),
    f(com::DEVICE, 60, E_INVALIDARG, 0, META),
    f(com::DEVICE, 61, E_INVALIDARG, 6, META),
    f(com::DEVICE, 62, E_NOTIMPL, 3, RAYOS),
    f(com::DEVICE, 66, E_NOTIMPL, 4, RAYOS),
    f(com::DEVICE, 67, E_NOTIMPL, 3, PROTEGIDOS),
    f(com::DEVICE, 71, VOID, 0, "sampler feedback: la casa anuncia SamplerFeedbackTier NOT_SUPPORTED"),
    f(com::DEVICE, 78, E_INVALIDARG, 8, RESERVADOS),
    // ID3D12CommandQueue
    f(com::QUEUE, 8, VOID, 0, RESERVADOS),
    f(com::QUEUE, 9, VOID, 0, RESERVADOS),
    // ID3D12GraphicsCommandList
    f(com::LIST, 18, VOID, 0, RESERVADOS),
    f(com::LIST, 70, VOID, 0, META),
    f(com::LIST, 71, VOID, 0, META),
    f(com::LIST, 72, VOID, 0, RAYOS),
    f(com::LIST, 73, VOID, 0, RAYOS),
    f(com::LIST, 74, VOID, 0, RAYOS),
    f(com::LIST, 75, VOID, 0, RAYOS),
    f(com::LIST, 76, VOID, 0, RAYOS),
    f(com::LIST, 79, VOID, 0, MALLA),
    f(com::LIST, 84, VOID, 0, "work graphs: la casa no los anuncia"),
    f(com::LIST, 85, VOID, 0, "work graphs: la casa no los anuncia"),
    // ID3D12Resource1, ID3D12Heap1
    f(com::RESOURCE, 15, E_INVALIDARG, 2, PROTEGIDOS),
    f(com::MEMORIA, 9, E_INVALIDARG, 2, PROTEGIDOS),
    // IDXGIFactory
    f(com::FACTORY, 17, E_INVALIDARG, 2, COMPARTIDOS),
    // IDXGIOutput
    f(com::OUTPUT, 13, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
    f(com::OUTPUT, 14, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
    f(com::OUTPUT, 15, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
    f(com::OUTPUT, 16, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
    f(com::OUTPUT, 17, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
    f(com::OUTPUT, 21, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, 0, PANTALLA),
];

fn buscar(i: usize, s: usize) -> Option<&'static Falla> {
    FALLAS.iter().find(|x| x.interfaz == i && x.hueco == s)
}

/// **El hueco de una falla documentada.** Recibe hasta nueve argumentos (en
/// Windows x64, los que el `.exe` no paso son memoria de su marco: se leen y
/// no se tocan); solo escribe en el que su fila dice que es el de salida.
#[allow(clippy::too_many_arguments)]
extern "win64" fn falla<const I: usize, const S: usize>(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, a7: u64, a8: u64) -> i32 {
    let Some(x) = buscar(I, S) else { return E_NOTIMPL };
    let i = &INTERFACES[I];
    let hr = if x.hr == VOID { alloc::string::String::from("no hace nada") } else { alloc::format!("HRESULT {:#010X}", x.hr as u32) };
    crate::aviso(&alloc::format!("{}::{} (hueco {}): falla documentada, {} -- {}", i.nombre, i.metodos.get(S).copied().unwrap_or("?"), S, hr, x.por_que));
    let args = [a0, a1, a2, a3, a4, a5, a6, a7, a8];
    if let Some(&p) = args.get(x.salida).filter(|_| x.salida != 0) {
        if p != 0 {
            // SAFETY: el puntero de salida que el `.exe` paso (un `void **`,
            // un HANDLE* o un LUID*: ocho bytes).
            unsafe { (p as *mut u64).write_unaligned(0) };
        }
    }
    x.hr
}

macro_rules! fallas {
    ($i:ident; $($s:literal)*) => { [$(falla::<$i, $s> as *const () as usize as u64),*] };
}

/// **Poner las fallas documentadas de `I`** en su vtabla (antes de los
/// metodos que la casa hace, que mandan).
pub(crate) fn aplicar<const I: usize>(v: &mut [u64]) {
    let todas: [u64; com::HUECOS] = fallas!(I; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59
        60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87);
    let n = v.len();
    for x in FALLAS.iter().filter(|x| x.interfaz == I && x.hueco < n) {
        v[x.hueco] = todas[x.hueco];
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_falla_es_de_un_hueco_que_existe_y_no_se_repite() {
        for (k, x) in FALLAS.iter().enumerate() {
            assert!(x.hueco < INTERFACES[x.interfaz].metodos.len(), "{}: hueco {}", INTERFACES[x.interfaz].nombre, x.hueco);
            assert!(x.hueco > 2, "IUnknown no falla nunca");
            assert!(!FALLAS[..k].iter().any(|y| y.interfaz == x.interfaz && y.hueco == x.hueco));
            assert!(x.salida < 9);
        }
    }
}
