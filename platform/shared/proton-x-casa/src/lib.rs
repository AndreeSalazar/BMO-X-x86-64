//! # PROTON-X -- las DLL de la casa (P1b, P1d, P2, P3 y P4)
//!
//! generacion: hijo -- da lo que un `.exe` importa; no sabe cargarlo ni que
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

pub mod carpetas;
pub mod com;
pub mod com_basico;
pub mod compilador;
pub mod crt;
pub mod crt_cadenas;
pub mod crt_entorno;
pub mod crt_ficheros;
pub mod crt_mates;
pub mod crt_numeros;
pub mod cxx;
pub mod cxx4;
pub mod com_objeto;
pub mod d3d12;
pub mod d3d12_resto;
pub mod dxgi_resto;
pub mod d3d12_capacidades;
pub mod d3d12_dispositivos;
pub mod d3d12_montones;
pub mod diario;
pub mod dll_chicas;
pub mod dxgi;
pub mod esperas;
pub mod excepciones;
pub mod ficheros;
pub mod gdi32;
pub mod hilos;
pub mod kernel32;
pub mod kernel32_a;
pub mod kernel32_hora;
pub mod kernel32_locale;
pub mod kernel32_mapeo;
pub mod kernel32_procesos;
pub mod advapi32;
pub mod aparatos;
pub mod advapi32_registro;
pub mod kernel32_pool;
pub mod kernel32_sistema;
pub mod memoria;
pub mod modulos;
pub mod msvcp_hilos;
pub mod msvcp_errores;
pub mod msvcp_locale;
pub mod msvcp_flujos;
pub mod msvcp_tiempo;
pub mod msvcp_accesos;
pub mod msvcp_ultimas;
pub mod nativo;
pub mod proceso;
pub mod red;
pub mod rtti;
pub mod red_puro;
/// El espia de Init y GetError de Galaxy, con el diario (01-10).
mod espia;
/// Los sockets de 127.0.0.1 dentro del proceso (01-10).
mod red_local;
/// La SSPI sin paquetes: secur32 y sspicli (01-10).
mod sspi;
/// Las imagenes PE de las DLL de la casa: un HMODULE que se puede leer (01-10).
mod imagenes;
pub mod cripto;
pub mod sistema;
pub mod texto;
pub mod tuberia;
pub mod user32;
pub mod user32_entrada;
pub mod user32_medidas;
pub mod user32_mensajes;
pub mod user32_portapapeles;
pub mod user32_dialogos;
pub mod user32_ventanas;
pub mod version_y_seguridad;

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
    /// La fecha de la placa, en segundos desde 1970, o `None` si no se sabe
    /// (P4f2: GetSystemTimeAsFileTime).
    pub fecha: fn() -> Option<u64>,
    /// Lo que hay en una carpeta del volumen (P4f3: FindFirstFileW), o
    /// `None` si no es una carpeta. `""` es la raiz.
    pub listar: fn(&[u8]) -> Option<Vec<bmo_proton_x::ficheros::Entrada>>,
    /// Crear carpetas, quitar y renombrar (ESTRATOS en BMO-X); `None`: no hay.
    pub carpetas: Option<Carpetas>,
    /// La RESERVA (P0.4c): una ventana de direcciones donde se hacen y
    /// deshacen paginas a peticion. `None`: solo las arenas de `memoria`.
    pub reserva: Option<Reserva>,
    /// Leer ficheros A LA CARTA (01-10): sin esto, todo fichero se trae
    /// entero con `leer_fichero`.
    pub trozos: Option<Trozos>,
}

/// **Leer un fichero a trozos** (01-10, Cyberpunk): su medida sin traerlo, y
/// un rango cualquiera. En BMO-X, `Archivo::reflejar` + `saltar` + `leer_en`.
#[derive(Clone, Copy)]
pub struct Trozos {
    /// La medida, o `None` si no esta (o es una carpeta).
    pub medida: fn(&[u8]) -> Option<u64>,
    /// `(ruta, desde, destino)`: los bytes traidos, o `None` si fallo.
    pub leer: fn(&[u8], u64, &mut [u8]) -> Option<usize>,
    /// Desde que medida un fichero de solo leer va a la carta (los chicos
    /// se traen enteros: leer de cuatro en cuatro bytes del disco no).
    pub umbral: u64,
}

/// **Lo que cambia carpetas del volumen** (relevo 01-10, paso 4b): en
/// BMO-X, ESTRATOS (crear carpeta, quitar, renombrar en la misma carpeta).
/// `false` = no se pudo (la casa contesta lo de Windows). Sin esto (el banco,
/// un volumen sin ESTRATOS), la casa dice ACCESO DENEGADO como siempre.
#[derive(Clone, Copy)]
pub struct Carpetas {
    pub crear: fn(&[u8]) -> bool,
    pub quitar: fn(&[u8]) -> bool,
    /// `(ruta, nombre nuevo)`: el nombre, no la ruta; misma carpeta.
    pub renombrar: fn(&[u8], &[u8]) -> bool,
}

/// **La reserva de la plataforma** (P0.4c, 30-09): en BMO-X, la ventana de
/// `TASK_OP_RESERVA_*`. La casa elige las direcciones (reservar no cuesta
/// nada) y pide o devuelve paginas.
#[derive(Clone, Copy)]
pub struct Reserva {
    /// La ventana: `[base, base + bytes)`, a 64 KiB.
    pub base: u64,
    pub bytes: u64,
    /// Hacer las paginas que falten de `[va, va + bytes)`, a cero y R+W (lo
    /// grande lo parte la plataforma). `false` si no hay RAM.
    pub hacer: fn(u64, u64) -> bool,
    /// Devolverlas.
    pub deshacer: fn(u64, u64),
    /// La RAM de la maquina: (total, libre ahora), para GlobalMemoryStatus.
    pub ram: fn() -> (u64, u64),
}

/// Una clase registrada (`RegisterClassExW`).
struct Clase {
    nombre: Vec<u16>,
    atomo: u16,
    wndproc: u64,
    /// Lo demas del `WNDCLASSEXW` (tanda 6: GetClassLongPtr, GetClassInfoEx).
    datos: user32_ventanas::DatosClase,
}

/// Una ventana (`CreateWindowExW`).
struct Ventana {
    hwnd: u64,
    wndproc: u64,
    sup: Superficie,
    viva: bool,
    mostrada: bool,
    /// Estilos, titulo, USERDATA, bytes de mas... (tanda 6).
    datos: user32_ventanas::DatosVentana,
}

struct Estado {
    plataforma: Option<Plataforma>,
    clases: Vec<Clase>,
    ventanas: Vec<Ventana>,
    cola: Cola,
    avisos: u32,
    /// La huella de cada aviso ya dicho (02-10): uno repetido no gasta sitio.
    dichos: Vec<u64>,
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
    dichos: Vec::new(),
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
        e.dichos.clear();
    });
    hilos::reiniciar();
    tuberia::reiniciar();
    dxgi::reiniciar();
    nativo::reiniciar();
    ficheros::reiniciar();
    memoria::reiniciar();
    com_objeto::reiniciar();
    d3d12_resto::reiniciar();
    proceso::reiniciar();
    esperas::reiniciar();
    carpetas::reiniciar();
    kernel32::reiniciar();
    modulos::reiniciar();
    imagenes::reiniciar();
    crt::reiniciar();
    crt_cadenas::reiniciar();
    crt_entorno::reiniciar();
    crt_ficheros::reiniciar();
    excepciones::reiniciar();
    kernel32_pool::reiniciar();
    cxx::reiniciar();
    kernel32_mapeo::reiniciar();
    kernel32_procesos::reiniciar();
    advapi32::reiniciar();
    advapi32_registro::reiniciar();
    cripto::reiniciar();
    user32_medidas::reiniciar();
    user32_ventanas::reiniciar();
    user32_mensajes::reiniciar();
    user32_entrada::reiniciar();
    user32_portapapeles::reiniciar();
    version_y_seguridad::reiniciar();
    com_basico::reiniciar();
    user32_dialogos::reiniciar();
    // Lo del locale apunta al monton de antes: fuera (P0.4c).
    msvcp_locale::reiniciar();
    // Tanda 19: cerr, con la plataforma ya puesta (pide memoria).
    msvcp_flujos::reiniciar();
}

/// **Decir una linea por la consola**, sin el tope de `aviso` (quien llama
/// pone el suyo).
pub(crate) fn decir(texto: &str) {
    if let Some(p) = con(|e| e.plataforma) {
        (p.escribir)(b"PROTON-X: ");
        (p.escribir)(texto.as_bytes());
        (p.escribir)(b"\n");
    }
}

/// Cuantos avisos DISTINTOS se dicen.
const AVISOS: usize = 64;

/// **Decir algo que la casa no sabe hacer**, por la consola: cada aviso
/// distinto UNA vez, hasta [`AVISOS`] (un `.exe` que repita lo mismo en cada
/// fotograma no ahoga la consola).
///
/// Hasta el 02-10 eran los OCHO primeros, repetidos o no, y en Cyberpunk los
/// gastaban los `LoadLibrary` de GameServices*.dll antes de que el juego
/// tocara D3D12: lo que fallaba despues no salia en ningun sitio.
pub fn aviso(texto: &str) {
    // FNV-1a: para no repetir, no para nada mas.
    let huella = texto.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let (p, decir) = con(|e| {
        e.avisos += 1;
        let nuevo = !e.dichos.contains(&huella) && e.dichos.len() < AVISOS;
        if nuevo {
            e.dichos.push(huella);
        }
        (e.plataforma, nuevo)
    });
    if let Some(p) = p {
        if decir {
            (p.escribir)(b"PROTON-X: ");
            (p.escribir)(texto.as_bytes());
            (p.escribir)(b"\n");
        }
    }
}

/// **LA TABLA DE LA CASA**: la direccion de cada funcion que existe, tal
/// como la ve el `.exe` (el cargador y `GetProcAddress`): con el diario
/// encendido, su trampolin (`diario::envolver`).
pub fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    let d = tabla_casa(dll, f)?;
    // Lo que exporta una DLL propia puede ser un DATO: nunca se envuelve.
    if modulos::exportada(dll, f).is_some() {
        // Con el diario, Init y GetError de Galaxy van por el espia.
        return Some(espia::envolver(dll, &alloc::format!("{f}"), d));
    }
    // Tanda 18: los `id` de msvcp140 tambien son DATOS (y no van a su imagen:
    // un trampolin delante de un dato no es el dato).
    if matches!(f, Funcion::Nombre(n) if dll.eq_ignore_ascii_case("msvcp140.dll") && (msvcp_locale::es_dato(n) || msvcp_flujos::es_dato(n))) {
        return Some(d);
    }
    let r = diario::envolver(dll, &alloc::format!("{f}"), d);
    // Lo que se importa de una DLL de la casa va a su imagen (01-10), con su
    // trampolin del diario si lo hay: llamar por la imagen es llamar igual.
    if let Funcion::Nombre(n) = f {
        imagenes::apuntar(dll, n, r);
    }
    Some(r)
}

/// **La tabla, para la casa misma**: la funcion de verdad, sin trampolin
/// (lo que la casa se llama a si misma no va al diario).
pub(crate) fn tabla_casa(dll: &str, f: &Funcion) -> Option<u64> {
    // P5a: lo que exporta una DLL PROPIA ya cargada (tambien por ordinal).
    if let Some(d) = modulos::exportada(dll, f) {
        return Some(d);
    }
    let n = match f {
        Funcion::Nombre(n) => n,
        // Tanda 12: ws2_32 se importa por ordinal.
        Funcion::Ordinal(o) if dll.eq_ignore_ascii_case("ws2_32.dll") => return red::por_ordinal(*o),
        // Tanda 15: OLEAUT32 tambien (SysAllocString es el 2...).
        // Tanda 16: WLDAP32 tambien, y la casa no tiene LDAP.
        Funcion::Ordinal(o) if dll.eq_ignore_ascii_case("wldap32.dll") => return aparatos::ldap_por_ordinal(*o),
        Funcion::Ordinal(o) if dll.eq_ignore_ascii_case("oleaut32.dll") => return tabla_casa(dll, &Funcion::Nombre(com_basico::por_ordinal(*o)?.into())),
        _ => return None,
    };
    // P0.4b.7: los API set "downlevel" (`api-ms-win-downlevel-kernel32-l2-1-0`,
    // de dbghelp.dll) son el nombre de una DLL de verdad con otro traje: la
    // casa los resuelve como esa DLL (kernel32, advapi32, user32...).
    if let Some(de) = dll.get(..21).filter(|p| p.eq_ignore_ascii_case("api-ms-win-downlevel-")).and_then(|_| dll[21..].split('-').next()) {
        return tabla_casa(&alloc::format!("{de}.dll"), f);
    }
    // P4f2: los "API set" de Windows (`api-ms-win-core-synch-l1-2-0.dll`,
    // de donde la `std` de Rust importa WaitOnAddress) son nombres de
    // kernel32/kernelbase: Windows los resuelve ahi, y la casa tambien.
    let api_set = dll.len() > 16 && dll.as_bytes()[..16].eq_ignore_ascii_case(b"api-ms-win-core-");
    if dll.eq_ignore_ascii_case("kernel32.dll") || dll.eq_ignore_ascii_case("kernelbase.dll") || api_set {
        de_kernel32(n)
    } else if crt::es_del_crt(dll) {
        // P4f5: el CRT de MSVC (ucrtbase, vcruntime140 y sus API set).
        crt::buscar(n)
    } else if dll.eq_ignore_ascii_case("msvcp140.dll") {
        // Tanda 2 de Cyberpunk: la biblioteca de C++ de MSVC.
        // Tanda 18: su locale.
        msvcp_hilos::buscar(n).or_else(|| msvcp_errores::buscar(n)).or_else(|| msvcp_locale::buscar(n)).or_else(|| msvcp_flujos::buscar(n)).or_else(|| msvcp_tiempo::buscar(n)).or_else(|| msvcp_accesos::buscar(n)).or_else(|| msvcp_ultimas::buscar(n))
    } else if dll.eq_ignore_ascii_case("ntdll.dll") {
        // P4f4: NtReadFile/NtWriteFile de verdad; lo demas de ntdll, dicho.
        // P4c: __C_specific_handler y los Rtl* de las excepciones.
        sistema::buscar_ntdll(n).or_else(|| excepciones::buscar_ntdll(n)).or_else(|| dll_chicas::buscar_ntdll(n))
    } else if dll.eq_ignore_ascii_case("d3dcompiler_47.dll") {
        compilador::buscar(n)
    } else if dll.eq_ignore_ascii_case("oleaut32.dll") {
        sistema::buscar_oleaut32(n).or_else(|| com_basico::buscar(n))
    } else if dll.eq_ignore_ascii_case("ws2_32.dll") {
        red::buscar(n)
    } else if dll.eq_ignore_ascii_case("bcryptprimitives.dll") || dll.eq_ignore_ascii_case("userenv.dll") {
        sistema::buscar_otras(dll, n)
    } else if dll.eq_ignore_ascii_case("secur32.dll") || dll.eq_ignore_ascii_case("sspicli.dll") {
        sspi::buscar(n)
    } else if dll.eq_ignore_ascii_case("user32.dll") {
        user32::buscar(n).or_else(|| user32_medidas::buscar(n)).or_else(|| user32_ventanas::buscar(n)).or_else(|| user32_mensajes::buscar(n)).or_else(|| user32_entrada::buscar(n)).or_else(|| user32_portapapeles::buscar(n)).or_else(|| user32_dialogos::buscar(n))
    } else if dll.eq_ignore_ascii_case("crypt32.dll") {
        cripto::buscar_crypt32(n)
    } else if dll.eq_ignore_ascii_case("bcrypt.dll") {
        cripto::buscar_bcrypt(n)
    } else if dll.eq_ignore_ascii_case("advapi32.dll") || es_api_set_de(dll, &["api-ms-win-security-", "api-ms-win-eventing-"]) {
        // Tanda 11: lo suyo, y el registro y los tokens (de kernelbase).
        // Tanda 14a: sus API set (seguridad, ETW) y lo de ETW.
        advapi32::buscar(n).or_else(|| version_y_seguridad::buscar(n)).or_else(|| de_kernel32(n)).or_else(|| dll_chicas::buscar(n))
    } else if dll.eq_ignore_ascii_case("gdi32.dll") {
        gdi32::buscar(n).or_else(|| dll_chicas::buscar(n))
    } else if es_api_set_de(dll, &["api-ms-win-devices-config-"]) {
        // Tanda 20: el API set de CFGMGR32 (lo pide una DLL del juego).
        aparatos::buscar(n)
    } else if CHICAS.iter().any(|c| dll.eq_ignore_ascii_case(c)) {
        // Tanda 14a: las DLL de las que el juego pide una, dos o cuatro.
        dll_chicas::buscar(n).or_else(|| version_y_seguridad::buscar(n)).or_else(|| com_basico::buscar(n)).or_else(|| aparatos::buscar(n))
    } else if dll.eq_ignore_ascii_case("d3d12.dll") {
        d3d12::buscar(n).or_else(|| tuberia::buscar(n))
    } else if dll.eq_ignore_ascii_case("dxgi.dll") {
        dxgi::buscar(n)
    } else {
        None
    }
}

/// La cadena de kernel32 (kernelbase y sus API set): ningun nombre esta en
/// dos modulos (PX5).
/// Las DLL chicas del censo (tanda 14a): todo lo suyo esta en `dll_chicas`.
const CHICAS: &[&str] = &[
    "winmm.dll",
    "shlwapi.dll",
    "shell32.dll",
    "powrprof.dll",
    "wininet.dll",
    "normaliz.dll",
    "iphlpapi.dll",
    "mswsock.dll",
    "xinput9_1_0.dll",
    "xinput1_3.dll",
    "xinput1_4.dll",
    "rpcrt4.dll",
    "ole32.dll",
    "version.dll",
    "hid.dll",
    "setupapi.dll",
    "cfgmgr32.dll",
    "wldap32.dll",
];

/// Si `dll` es un API set que empieza por alguno de `prefijos`.
fn es_api_set_de(dll: &str, prefijos: &[&str]) -> bool {
    prefijos.iter().any(|p| dll.len() > p.len() && dll.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes()))
}

fn de_kernel32(n: &str) -> Option<u64> {
    kernel32::buscar(n).or_else(|| hilos::buscar(n)).or_else(|| ficheros::buscar(n)).or_else(|| memoria::buscar(n)).or_else(|| proceso::buscar(n)).or_else(|| texto::buscar(n)).or_else(|| modulos::buscar(n)).or_else(|| esperas::buscar(n)).or_else(|| carpetas::buscar(n)).or_else(|| sistema::buscar(n)).or_else(|| excepciones::buscar(n)).or_else(|| kernel32_hora::buscar(n)).or_else(|| kernel32_sistema::buscar(n)).or_else(|| kernel32_a::buscar(n)).or_else(|| kernel32_pool::buscar(n)).or_else(|| kernel32_mapeo::buscar(n)).or_else(|| kernel32_locale::buscar(n)).or_else(|| kernel32_procesos::buscar(n)).or_else(|| advapi32_registro::buscar(n))
}

/// Si `f` (un nombre de fichero en minusculas) es un API set o una DLL del
/// CRT: tambien son de la casa (P5a: no se buscan en el disco).
pub(crate) fn es_api_set_o_crt(f: &str) -> bool {
    // Tanda 20: TODOS los api-ms-win-* (core, crt, security, eventing,
    // devices-config...): en Windows ninguno es un fichero de verdad, y uno
    // que la casa no conozca carga igual (lo que falte avisa al llamarse).
    es_api_set_de(f, &["api-ms-win-"]) || crt::es_del_crt(f)
}

/// La direccion de una funcion, para la tabla.
macro_rules! dir {
    ($f:expr) => {
        $f as *const () as usize as u64
    };
}
pub(crate) use dir;
