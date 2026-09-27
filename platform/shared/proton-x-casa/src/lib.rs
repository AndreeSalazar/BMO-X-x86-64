//! # PROTON-X -- las DLL de la casa (P1b, P1d, P2, P3 y P4)
//!
//! generacion: hija -- da lo que un `.exe` importa; no sabe cargarlo ni que
//! maquina hay debajo
//!
//! Lo que un `.exe` de Windows importa de `kernel32.dll`, `user32.dll`,
//! `gdi32.dll`, `d3d12.dll` y `dxgi.dll` (y los objetos COM que estas dos
//! devuelven, `com.rs`), escrito en Rust como `extern "win64"`: el compilador pone la
//! convencion de Windows (rcx, rdx, r8, r9, 32 bytes de sombra y el resto en
//! la pila) sin una linea de ensamblador. [`tabla`] es LA TABLA DE LA CASA:
//! la direccion de cada funcion que existe, y ninguna mas.
//!
//! # La plataforma, y por que va por punteros
//!
//! Debajo de estas DLL no hay un kernel concreto: hay una [`Plataforma`] que
//! pone quien carga. En el Ryzen (`Ultra_userspace/apps/proton-x`) es la
//! puerta de BMO-X: la consola, las superficies del escritorio y su buzon. En
//! el banco del anfitrion (`tests/corre.rs`) es una pantalla de mentira en
//! memoria con las teclas escritas de antemano. **El codigo de Windows que se
//! prueba es el mismo que corre en el metal**: solo cambia lo de debajo.
//!
//! # Por que aqui hay `unsafe`
//!
//! Esta es la FRONTERA con el ABI de Windows: se leen estructuras de C por
//! puntero (`WNDCLASSEXW`, `MSG`, `PAINTSTRUCT`, `BITMAPINFOHEADER`) y se llama a
//! la `WndProc` del `.exe`. Lo que se puede decir sin punteros --la cola, que
//! mensaje es cada evento, como cae un DIB-- vive en `bmo_proton_x::ventanas`,
//! que es puro y lo prueba su banco.
//!
//! # [!] Ningun `float` POR VALOR, todavia
//!
//! En Ring 3 de BMO-X (`x86_64-unknown-none`) Rust compila los `float` por
//! SOFTWARE y los pasa en registros ENTEROS; Windows x64 los pasa en `xmm`. Un
//! `extern "win64" fn(x: f32)` de aqui leeria el `float` del sitio equivocado
//! (y el banco del anfitrion, con SSE, no lo veria). Hoy ninguna funcion de la
//! casa recibe ni devuelve un `float` por valor: los colores de
//! `ClearRenderTargetView` van por PUNTERO. La primera que lo necesite
//! (`OMSetBlendFactor` va por puntero; `SetGraphicsRoot32BitConstant` es un
//! `u32`) tendra que leer el `xmm` a mano.
//!
//! # Lo que NO hay, y se dice
//!
//! Una funcion que no esta en [`tabla`] no se rellena: el cargador dice cual
//! falta y el `.exe` no arranca. Y lo que una funcion que SI esta no sabe
//! hacer todavia (estirar un DIB, filtrar `GetMessage`) contesta el fallo de
//! Windows y lo dice por la consola con [`aviso`]: nunca un exito mentido.

#![no_std]

extern crate alloc;

pub mod com;
pub mod d3d12;
pub mod dxgi;
pub mod ficheros;
pub mod gdi32;
pub mod hilos;
pub mod kernel32;
pub mod memoria;
pub mod modulos;
pub mod nativo;
pub mod proceso;
pub mod texto;
pub mod tuberia;
pub mod user32;

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::ventanas::Cola;
use bmo_proton_x::Funcion;

/// Una superficie donde pintar: los pixeles (BGRA, `stride` por fila) y lo
/// que la plataforma necesite para encontrarla (`dato`).
#[derive(Debug, Clone, Copy)]
pub struct Superficie {
    pub pixeles: *mut u32,
    pub ancho: u32,
    pub alto: u32,
    pub stride: u32,
    pub dato: u64,
}

/// **Lo que las DLL de la casa piden al sistema**. Lo pone quien carga.
#[derive(Clone, Copy)]
pub struct Plataforma {
    /// La consola (lo que `WriteFile` escribe en la salida estandar).
    pub escribir: fn(&[u8]),
    /// El proceso se va con este codigo (`ExitProcess`).
    pub salir: fn(u32) -> !,
    /// Una superficie de `ancho` x `alto`, SIN mostrar todavia.
    pub superficie: fn(u32, u32) -> Option<Superficie>,
    /// Mostrarla en el escritorio (`ShowWindow`). `false` si no se pudo.
    pub mostrar: fn(&Superficie) -> bool,
    /// El dibujo esta entero: que se vea (`EndPaint`).
    pub presentar: fn(&Superficie),
    /// El siguiente evento de su buzon, crudo, o 0 si no hay.
    pub evento: fn(&Superficie) -> u64,
    /// Nada que hacer: dormir un poco (`GetMessage` sin mensajes).
    pub dormir: fn(),
    /// Poner el GS de este hilo de la plataforma (el TEB del hilo de Windows
    /// que va a correr): `TASK_OP_PON_GS` en BMO-X (P4).
    pub poner_gs: fn(u64),
    /// La hora, en nanosegundos desde cualquier origen fijo (P4: Sleep,
    /// los plazos, QueryPerformanceCounter).
    pub ahora_ns: fn() -> u64,
    /// Quien DIBUJA un lote de D3D12 (P4, la costura con VERRANO): hoy
    /// `bmo_proton_x::lote::en_cpu`; con la 3060, el ejecutor de VERRANO.
    pub dibujar: bmo_proton_x::lote::Ejecutor,
    /// Estos bytes, como CODIGO: un bloque nuevo, copiado y SELLADO (W^X);
    /// su direccion, o `None` si no se pudo (P3b3b: los sombreadores nativos).
    pub sellar_codigo: fn(&[u8]) -> Option<u64>,
    /// Soltar un bloque de `sellar_codigo` (direccion y medida).
    pub soltar_codigo: fn(u64, usize),
    /// Un fichero ENTERO, por su ruta del volumen; `None` si no esta (P4d).
    pub leer_fichero: fn(&[u8]) -> Option<Vec<u8>>,
    /// Escribir un fichero entero (crearlo o reemplazarlo). `false` si no salio.
    pub escribir_fichero: fn(&[u8], &[u8]) -> bool,
    /// Un bloque NUEVO de estos bytes, R+W y a ceros, que vive lo que el
    /// proceso: una arena del monton de Windows (P4e). `None` si no hay.
    pub memoria: fn(usize) -> Option<u64>,
}

/// Una clase registrada (`RegisterClassExW`).
struct Clase {
    nombre: Vec<u16>,
    atomo: u16,
    wndproc: u64,
}

/// Una ventana (`CreateWindowExW`).
struct Ventana {
    hwnd: u64,
    wndproc: u64,
    sup: Superficie,
    viva: bool,
    mostrada: bool,
}

struct Estado {
    plataforma: Option<Plataforma>,
    clases: Vec<Clase>,
    ventanas: Vec<Ventana>,
    cola: Cola,
    avisos: u32,
}

struct Global(UnsafeCell<Estado>);

// SAFETY: un `.exe` de P2 corre en UN hilo, y el banco corre los `.exe` de
// uno en uno (su cerrojo). Los hilos de Windows son P4.
unsafe impl Sync for Global {}

static ESTADO: Global = Global(UnsafeCell::new(Estado {
    plataforma: None,
    clases: Vec::new(),
    ventanas: Vec::new(),
    cola: Cola::nueva(),
    avisos: 0,
}));

/// **El estado, un momento.** Nunca se llama a la `WndProc` desde dentro:
/// ella vuelve a entrar en `user32`, y dos prestamos a la vez serian dos
/// `&mut` del mismo estado. Se copia lo que haga falta, se suelta, y despues
/// se llama.
fn con<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    // SAFETY: un hilo (ver `Global`) y ningun `con` anidado: las funciones de
    // aqui no llaman a la WndProc ni a otra funcion de la casa dentro de `f`.
    f(unsafe { &mut *ESTADO.0.get() })
}

fn plataforma() -> Plataforma {
    match con(|e| e.plataforma) {
        Some(p) => p,
        // Sin plataforma no hay a quien decirlo ni donde salir: esto es un
        // fallo de quien carga, no del `.exe`.
        None => panic!("PROTON-X: las DLL de la casa sin plataforma"),
    }
}

/// **Empezar un `.exe`**: la plataforma, y el estado de Win32 a cero.
///
/// # Safety
/// Antes de saltar a la entrada del `.exe` y con ningun otro `.exe` corriendo
/// (el estado es uno para el proceso).
pub unsafe fn empezar(p: Plataforma) {
    con(|e| {
        e.plataforma = Some(p);
        e.clases.clear();
        e.ventanas.clear();
        e.cola = Cola::nueva();
        e.avisos = 0;
    });
    hilos::reiniciar();
    tuberia::reiniciar();
    nativo::reiniciar();
    ficheros::reiniciar();
    memoria::reiniciar();
    proceso::reiniciar();
}

/// **Decir algo que la casa no sabe hacer**, por la consola. Los ocho primeros:
/// un `.exe` que repita lo mismo en cada fotograma no puede ahogar la consola.
pub fn aviso(texto: &str) {
    let (p, n) = con(|e| {
        e.avisos += 1;
        (e.plataforma, e.avisos)
    });
    if let Some(p) = p {
        if n <= 8 {
            (p.escribir)(b"PROTON-X: ");
            (p.escribir)(texto.as_bytes());
            (p.escribir)(b"\n");
        }
    }
}

/// **LA TABLA DE LA CASA**: la direccion de cada funcion que existe.
pub fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    let Funcion::Nombre(n) = f else { return None };
    if dll.eq_ignore_ascii_case("kernel32.dll") {
        kernel32::buscar(n).or_else(|| hilos::buscar(n)).or_else(|| ficheros::buscar(n)).or_else(|| memoria::buscar(n)).or_else(|| proceso::buscar(n)).or_else(|| texto::buscar(n)).or_else(|| modulos::buscar(n))
    } else if dll.eq_ignore_ascii_case("user32.dll") {
        user32::buscar(n)
    } else if dll.eq_ignore_ascii_case("gdi32.dll") {
        gdi32::buscar(n)
    } else if dll.eq_ignore_ascii_case("d3d12.dll") {
        d3d12::buscar(n).or_else(|| tuberia::buscar(n))
    } else if dll.eq_ignore_ascii_case("dxgi.dll") {
        dxgi::buscar(n)
    } else {
        None
    }
}

/// La direccion de una funcion, para la tabla.
macro_rules! dir {
    ($f:expr) => {
        $f as *const () as usize as u64
    };
}
pub(crate) use dir;
