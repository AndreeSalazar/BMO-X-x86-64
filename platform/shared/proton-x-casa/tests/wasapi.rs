//! **WASAPI de la casa, por sus puertas de Windows** (03-10, N4.5).
//!
//! Lo que hace un motor de sonido (Wwise en Cyberpunk) al arrancar, llamado
//! a mano como lo llamaria el `.exe`: `CoCreateInstance` del
//! `MMDeviceEnumerator`, el aparato por defecto, sus propiedades, un
//! `IAudioClient` por evento, `GetBuffer`/`ReleaseBuffer`, el relleno y el
//! reloj. El "audifono" es un anillo de prueba que suena cuando la prueba
//! dice, y la hora la mueve la prueba.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bmo_proton_x::pcm::Formato;
use bmo_proton_x::Funcion;
use bmo_proton_x_casa::com::{guid, Guid};
use bmo_proton_x_casa::{Plataforma, Sonido, Superficie};

static UNO_A_LA_VEZ: Mutex<()> = Mutex::new(());
static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static HORA: AtomicU64 = AtomicU64::new(0);

/// El anillo del audifono de prueba: lo escrito y aun no sonado, y lo que ya
/// sono (para mirarlo).
static ANILLO: Mutex<(Vec<u8>, Vec<u8>)> = Mutex::new((Vec::new(), Vec::new()));
const ANILLO_BYTES: usize = 19_200; // 100 ms de s16 estereo a 48 kHz

fn abrir() -> Option<u32> {
    Some(48_000)
}
fn escribir_sonido(b: &[u8]) -> usize {
    let mut a = ANILLO.lock().unwrap();
    let cabe = (ANILLO_BYTES - a.0.len()).min(b.len()) / 4 * 4;
    a.0.extend_from_slice(&b[..cabe]);
    cabe
}
fn pendientes() -> usize {
    ANILLO.lock().unwrap().0.len()
}
fn cerrar() {}
/// El aparato suena `n` fotogramas.
fn sonar(n: usize) {
    let mut a = ANILLO.lock().unwrap();
    let k = (4 * n).min(a.0.len());
    let sonado: Vec<u8> = a.0.drain(..k).collect();
    a.1.extend_from_slice(&sonado);
}

const AUDIFONO: Sonido = Sonido { abrir, escribir: escribir_sonido, pendientes, cerrar };

fn escribir(b: &[u8]) {
    DICHO.lock().unwrap().extend_from_slice(b);
}
fn salir(c: u32) -> ! {
    panic!("la casa quiso salir con {c:#x}: {}", String::from_utf8_lossy(&DICHO.lock().unwrap()))
}
fn superficie(_: u32, _: u32) -> Option<Superficie> {
    None
}
fn mostrar(_: &Superficie) -> bool {
    false
}
fn presentar(_: &Superficie) {}
fn evento(_: &Superficie) -> u64 {
    0
}
fn dormir() {}
fn poner_gs(_: u64) {}
fn sellar_codigo(_: &[u8]) -> Option<u64> {
    None
}
fn soltar_codigo(_: u64, _: usize) {}
fn leer_fichero(_: &[u8]) -> Option<Vec<u8>> {
    None
}
fn escribir_fichero(_: &[u8], _: &[u8]) -> bool {
    false
}
fn memoria(n: usize) -> Option<u64> {
    let forma = std::alloc::Layout::from_size_align(n.max(1), 4096).ok()?;
    // SAFETY: una forma que no mide cero; el bloque vive lo que la prueba.
    let p = unsafe { std::alloc::alloc_zeroed(forma) };
    (!p.is_null()).then_some(p as u64)
}
fn fecha() -> Option<u64> {
    None
}
fn listar(_: &[u8]) -> Option<Vec<bmo_proton_x::ficheros::Entrada>> {
    None
}
fn ahora_ns() -> u64 {
    HORA.load(Ordering::SeqCst)
}

fn empezar(sonido: Option<Sonido>) -> std::sync::MutexGuard<'static, ()> {
    let g = UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    DICHO.lock().unwrap().clear();
    *ANILLO.lock().unwrap() = (Vec::new(), Vec::new());
    HORA.store(1_000_000_000, Ordering::SeqCst);
    // SAFETY: ningun `.exe` corre; una prueba a la vez (el cerrojo).
    unsafe {
        bmo_proton_x_casa::empezar(Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x::lote::en_cpu, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar, carpetas: None, reserva: None, trozos: None, sonido, cuaderno: None })
    };
    g
}

fn funcion(dll: &str, n: &str) -> usize {
    bmo_proton_x_casa::tabla(dll, &Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n} no esta en la tabla")) as usize
}

/// El hueco `h` de la vtabla de un objeto de la casa.
fn hueco(obj: u64, h: usize) -> usize {
    // SAFETY: un objeto de la casa: su primer puntero es la vtabla.
    unsafe { (*(obj as *const *const usize)).add(h).read() }
}

const CLSID_ENUMERADOR: Guid = guid(0xBCDE_0395, 0xE52F, 0x467C, [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]);
const IID_ENUMERADOR: Guid = guid(0xA956_64D2, 0x9614, 0x4F35, [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6]);
const IID_PUNTA: Guid = guid(0x1BE0_9788, 0x6894, 0x4089, [0x85, 0x86, 0x9A, 0x2A, 0x6C, 0x26, 0x5A, 0xC5]);
const IID_CLIENTE: Guid = guid(0x1CB9_AD4C, 0xDBFA, 0x4C32, [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2]);
const IID_RENDER: Guid = guid(0xF294_ACFC, 0x3146, 0x4483, [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2]);
const IID_RELOJ: Guid = guid(0xCD63_314F, 0x3FBA, 0x4A1B, [0x81, 0x2C, 0xEF, 0x96, 0x35, 0x87, 0x28, 0xE7]);
/// PKEY_Device_FriendlyName.
const NOMBRE: (Guid, u32) = (guid(0xA45C_254E, 0xDF1C, 0x4EFD, [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0]), 14);

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTFOUND: i32 = 0x8007_0490_u32 as i32;
const SIN_EVENTO: i32 = 0x8889_0014_u32 as i32;
const FUERA_DE_ORDEN: i32 = 0x8889_0007_u32 as i32;
const DEMASIADO: i32 = 0x8889_0006_u32 as i32;
const POR_EVENTO: u32 = 0x0004_0000;

type F0 = extern "win64" fn(u64) -> i32;
type F1 = extern "win64" fn(u64, u64) -> i32;
type F2 = extern "win64" fn(u64, u64, u64) -> i32;
type F3 = extern "win64" fn(u64, u64, u64, u64) -> i32;
type F4 = extern "win64" fn(u64, u64, u64, u64, u64) -> i32;
type Iniciar = extern "win64" fn(u64, u32, u32, i64, i64, *const u8, *const Guid) -> i32;

fn m0(o: u64, h: usize) -> i32 {
    // SAFETY: el hueco `h` de un objeto de la casa, con esa forma.
    unsafe { core::mem::transmute::<usize, F0>(hueco(o, h))(o) }
}
fn m1(o: u64, h: usize, a: u64) -> i32 {
    // SAFETY: como `m0`.
    unsafe { core::mem::transmute::<usize, F1>(hueco(o, h))(o, a) }
}
fn m2(o: u64, h: usize, a: u64, b: u64) -> i32 {
    // SAFETY: como `m0`.
    unsafe { core::mem::transmute::<usize, F2>(hueco(o, h))(o, a, b) }
}
fn m3(o: u64, h: usize, a: u64, b: u64, c: u64) -> i32 {
    // SAFETY: como `m0`.
    unsafe { core::mem::transmute::<usize, F3>(hueco(o, h))(o, a, b, c) }
}
fn m4(o: u64, h: usize, a: u64, b: u64, c: u64, d: u64) -> i32 {
    // SAFETY: como `m0`.
    unsafe { core::mem::transmute::<usize, F4>(hueco(o, h))(o, a, b, c, d) }
}
fn p<T>(x: &mut T) -> u64 {
    x as *mut T as u64
}

/// Lo que hace el motor de sonido hasta tener un IAudioClient.
fn hasta_el_cliente() -> (u64, u64) {
    type Crear = extern "win64" fn(*const Guid, u64, u32, *const Guid, *mut u64) -> u32;
    // SAFETY: CoCreateInstance de la casa.
    let crear: Crear = unsafe { core::mem::transmute(funcion("ole32.dll", "CoCreateInstance")) };
    let mut en = 0u64;
    assert_eq!(crear(&CLSID_ENUMERADOR, 0, 1, &IID_ENUMERADOR, &mut en), 0, "el MMDeviceEnumerator existe");
    let mut ap = 0u64;
    assert_eq!(m3(en, 4, 1, 0, p(&mut ap)), E_NOTFOUND, "no hay microfono (eCapture)");
    assert_eq!(m3(en, 4, 0, 0, p(&mut ap)), S_OK, "la salida por defecto (eRender, eConsole)");
    let mut cliente = 0u64;
    assert_eq!(m4(ap, 3, &IID_CLIENTE as *const Guid as u64, 1, 0, p(&mut cliente)), S_OK, "Activate(IAudioClient)");
    (ap, cliente)
}

#[test]
fn el_juego_encuentra_el_aparato_y_lo_que_dice_de_si() {
    let _uno = empezar(Some(AUDIFONO));
    let (ap, cliente) = hasta_el_cliente();
    let mut estado = 0u32;
    assert_eq!(m1(ap, 6, p(&mut estado)), S_OK);
    assert_eq!(estado, 1, "DEVICE_STATE_ACTIVE");
    // Su nombre, por el almacen de propiedades.
    let mut alm = 0u64;
    assert_eq!(m2(ap, 4, 0, p(&mut alm)), S_OK);
    let mut clave = [0u8; 20];
    clave[..16].copy_from_slice(&NOMBRE.0);
    clave[16..].copy_from_slice(&NOMBRE.1.to_le_bytes());
    let mut pv = [0u8; 24];
    assert_eq!(m2(alm, 5, clave.as_ptr() as u64, pv.as_mut_ptr() as u64), S_OK);
    assert_eq!(u16::from_le_bytes([pv[0], pv[1]]), 31, "VT_LPWSTR");
    let texto = u64::from_le_bytes(pv[8..16].try_into().unwrap()) as *const u16;
    // SAFETY: un texto de la casa con su cero.
    let nombre: String = char::decode_utf16((0..).map(|k| unsafe { texto.add(k).read() }).take_while(|&c| c != 0)).map(|c| c.unwrap()).collect();
    assert_eq!(nombre, "Audifono USB (BMO-X)");
    // IMMEndpoint: otro objeto, y dice SALIDA.
    let mut punta = 0u64;
    assert_eq!(m2(ap, 0, &IID_PUNTA as *const Guid as u64, p(&mut punta)), S_OK);
    assert_ne!(punta, ap);
    let mut flujo = 9u32;
    assert_eq!(m1(punta, 3, p(&mut flujo)), S_OK);
    assert_eq!(flujo, 0, "eRender");
    // La mezcla: float, estereo, 48 kHz -- la de Windows en un PC.
    let mut fmt = 0u64;
    assert_eq!(m1(cliente, 8, p(&mut fmt)), S_OK);
    // SAFETY: 40 bytes de la casa.
    let mezcla = unsafe { core::slice::from_raw_parts(fmt as *const u8, 40) };
    assert_eq!(Formato::de_waveformatex(mezcla), Some(Formato::mezcla(48_000)));
    // 44,1 kHz en modo compartido: S_FALSE y el mas parecido (la mezcla).
    let mut otro = Formato::mezcla(44_100).a_waveformatextensible();
    otro[8..12].copy_from_slice(&(44_100u32 * 8).to_le_bytes());
    let mut cerca = 0u64;
    assert_eq!(m3(cliente, 7, 0, otro.as_ptr() as u64, p(&mut cerca)), S_FALSE);
    assert_ne!(cerca, 0);
}

#[test]
fn el_juego_escribe_suena_por_el_anillo_y_el_relleno_baja() {
    let _uno = empezar(Some(AUDIFONO));
    let (_, cliente) = hasta_el_cliente();
    let mezcla = Formato::mezcla(48_000).a_waveformatextensible();
    // SAFETY: Initialize de la casa.
    let iniciar: Iniciar = unsafe { core::mem::transmute(hueco(cliente, 3)) };
    assert_eq!(iniciar(cliente, 0, POR_EVENTO, 200_000, 0, mezcla.as_ptr(), core::ptr::null()), S_OK, "20 ms, por evento");
    let mut marcos = 0u32;
    assert_eq!(m1(cliente, 4, p(&mut marcos)), S_OK);
    assert_eq!(marcos, 960, "20 ms a 48 kHz");
    assert_eq!(m0(cliente, 10), SIN_EVENTO, "Start sin SetEventHandle");
    type CrearEvento = extern "win64" fn(u64, i32, i32, u64) -> u64;
    // SAFETY: CreateEventW de la casa.
    let crear_evento: CrearEvento = unsafe { core::mem::transmute(funcion("kernel32.dll", "CreateEventW")) };
    let ev = crear_evento(0, 0, 0, 0);
    assert_eq!(m1(cliente, 13, ev), S_OK);
    let mut render = 0u64;
    assert_eq!(m2(cliente, 14, &IID_RENDER as *const Guid as u64, p(&mut render)), S_OK);
    // Antes de Start, el juego llena medio bufer: 480 fotogramas a (0.5, -0.25).
    let mut hoja = 0u64;
    assert_eq!(m2(render, 3, 480, p(&mut hoja)), S_OK);
    assert_eq!(m2(render, 3, 1, p(&mut hoja)), FUERA_DE_ORDEN, "dos GetBuffer sin soltar");
    for k in 0..480 {
        // SAFETY: 480 fotogramas de 8 bytes que dio GetBuffer.
        unsafe {
            (hoja as *mut f32).add(2 * k).write(0.5);
            (hoja as *mut f32).add(2 * k + 1).write(-0.25);
        }
    }
    assert_eq!(m2(render, 4, 480, 0), S_OK);
    let mut relleno = 0u32;
    assert_eq!(m1(cliente, 6, p(&mut relleno)), S_OK);
    assert_eq!(relleno, 480, "nada sono todavia");
    assert_eq!(m2(render, 3, 481, p(&mut hoja)), DEMASIADO, "solo caben 480 mas");
    assert_eq!(m0(cliente, 10), S_OK, "Start");
    // Lo convertido ya esta en el anillo: s16 estereo (un fotograma espera al
    // siguiente: la interpolacion).
    assert_eq!(pendientes(), 479 * 4);
    // El aparato suena 240: el relleno baja a 240, y el reloj lo dice.
    sonar(240);
    assert_eq!(m1(cliente, 6, p(&mut relleno)), S_OK);
    assert_eq!(relleno, 240);
    let mut reloj = 0u64;
    assert_eq!(m2(cliente, 14, &IID_RELOJ as *const Guid as u64, p(&mut reloj)), S_OK);
    let (mut frec, mut pos, mut qpc) = (0u64, 0u64, 0u64);
    assert_eq!(m1(reloj, 3, p(&mut frec)), S_OK);
    assert_eq!(frec, 48_000 * 8, "bytes de su formato por segundo");
    assert_eq!(m2(reloj, 4, p(&mut pos), p(&mut qpc)), S_OK);
    assert_eq!(pos, 240 * 8);
    let sonado = ANILLO.lock().unwrap().1.clone();
    let muestras: Vec<i16> = sonado.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    assert!(muestras.chunks_exact(2).all(|m| m == [16384, -8192]), "(0.5, -0.25) en s16");
    // El latido: cabe un periodo (480 de 960) -> el evento se enciende.
    HORA.fetch_add(20_000_000, Ordering::SeqCst);
    bmo_proton_x_casa::wasapi_flujo::latir();
    type Esperar = extern "win64" fn(u64, u32) -> u32;
    // SAFETY: WaitForSingleObject de la casa.
    let esperar: Esperar = unsafe { core::mem::transmute(funcion("kernel32.dll", "WaitForSingleObject")) };
    assert_eq!(esperar(ev, 0), 0, "WAIT_OBJECT_0: el evento del juego, encendido");
    assert_eq!(esperar(ev, 0), 0x102, "y era de reinicio automatico: WAIT_TIMEOUT");
}

#[test]
fn sin_aparato_el_reloj_corre_con_la_hora_y_no_suena_nada() {
    let _uno = empezar(None);
    let (_, cliente) = hasta_el_cliente();
    let mezcla = Formato::mezcla(48_000).a_waveformatextensible();
    // SAFETY: Initialize de la casa.
    let iniciar: Iniciar = unsafe { core::mem::transmute(hueco(cliente, 3)) };
    assert_eq!(iniciar(cliente, 0, 0, 1_000_000, 0, mezcla.as_ptr(), core::ptr::null()), S_OK, "100 ms, sin evento");
    let mut render = 0u64;
    assert_eq!(m2(cliente, 14, &IID_RENDER as *const Guid as u64, p(&mut render)), S_OK);
    let mut hoja = 0u64;
    assert_eq!(m2(render, 3, 4800, p(&mut hoja)), S_OK);
    assert_eq!(m2(render, 4, 4800, 2), S_OK, "AUDCLNT_BUFFERFLAGS_SILENT");
    assert_eq!(m0(cliente, 10), S_OK, "Start sin evento: no hace falta");
    let mut relleno = 0u32;
    HORA.fetch_add(25_000_000, Ordering::SeqCst);
    assert_eq!(m1(cliente, 6, p(&mut relleno)), S_OK);
    assert_eq!(relleno, 4800 - 1200, "25 ms de reloj = 1200 fotogramas");
    assert_eq!(m0(cliente, 11), S_OK, "Stop");
    HORA.fetch_add(25_000_000, Ordering::SeqCst);
    assert_eq!(m1(cliente, 6, p(&mut relleno)), S_OK);
    assert_eq!(relleno, 3600, "parado, el reloj no corre");
    assert!(ANILLO.lock().unwrap().0.is_empty(), "nada llego a un aparato");
    assert!(String::from_utf8_lossy(&DICHO.lock().unwrap()).contains("reloj que no suena"));
}
