//! **La tuberia de la casa, por sus puertas de Windows** (P3b2, 27-09).
//!
//! `corre.rs` corre `cubo.exe` entero: el camino bueno. Aqui se llama a las
//! mismas funciones `extern "win64"` a mano, como las llamaria un `.exe`, para
//! lo que un programa correcto no muestra:
//!
//! ```text
//!    D3D12SerializeRootSignature  la ESTRUCTURA de C leida -> los bytes de
//!                                 dxc (prueba/raiz.rts), por la vtabla del blob
//!    CreateGraphicsPipelineState  un input layout al que le falta una
//!                                 semantica del sombreador, y los sombreadores
//!                                 cambiados de hueco: E_INVALIDARG y el aviso
//! ```

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::Funcion;
use bmo_proton_x_casa::com::{self, Guid};
use bmo_proton_x_casa::{Plataforma, Superficie};

const RAIZ_RTS: &[u8] = include_bytes!("../../proton-x/prueba/raiz.rts");
const VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");

const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;

static UNO_A_LA_VEZ: Mutex<()> = Mutex::new(());
static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn escribir(b: &[u8]) {
    DICHO.lock().unwrap().extend_from_slice(b);
}
fn salir(c: u32) -> ! {
    panic!("la casa quiso salir con {c:#x}")
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
/// Bloques a cero de la plataforma: desde la tanda 19, `empezar` crea
/// `cerr` y el locale, y piden memoria (sin ella, la casa lo dice).
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
    0
}

fn empezar() -> std::sync::MutexGuard<'static, ()> {
    let g = UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    DICHO.lock().unwrap().clear();
    // SAFETY: ningun `.exe` corre; una prueba a la vez (el cerrojo).
    unsafe { bmo_proton_x_casa::empezar(Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x::lote::en_cpu, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar, carpetas: None, reserva: None, trozos: None, sonido: None }) };
    g
}

fn dicho() -> String {
    String::from_utf8_lossy(&DICHO.lock().unwrap()).into_owned()
}

fn funcion(dll: &str, n: &str) -> usize {
    bmo_proton_x_casa::tabla(dll, &Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n} no esta en la tabla")) as usize
}

/// El hueco `h` de la vtabla de un objeto de la casa.
fn hueco(obj: u64, h: usize) -> usize {
    // SAFETY: un objeto de la casa: su primer puntero es la vtabla.
    unsafe { (*(obj as *const *const usize)).add(h).read() }
}

/// `D3D12_ROOT_PARAMETER` y `D3D12_ROOT_SIGNATURE_DESC`, como en d3d12.h.
#[repr(C)]
struct Parametro {
    tipo: u32,
    registro: u32,
    espacio: u32,
    _resto_de_la_union: [u32; 3],
    visibilidad: u32,
    _relleno: u32,
}
#[repr(C)]
struct DescFirma {
    n: u32,
    parametros: *const Parametro,
    n_samplers: u32,
    samplers: *const u8,
    banderas: u32,
}

fn dispositivo() -> u64 {
    type Crear = extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32;
    // SAFETY: la direccion de `D3D12CreateDevice` de la casa.
    let crear: Crear = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12CreateDevice")) };
    let mut d = 0;
    assert_eq!(crear(0, 0xb000, &com::IID_DEVICE, &mut d), 0);
    d
}

/// La root signature del cubo (CBV b0, visible a todos, con input layout),
/// escrita como ESTRUCTURA y pasada por `D3D12SerializeRootSignature`.
fn serializar() -> Vec<u8> {
    let p = Parametro { tipo: 2, registro: 0, espacio: 0, _resto_de_la_union: [0; 3], visibilidad: 0, _relleno: 0 };
    let desc = DescFirma { n: 1, parametros: &p, n_samplers: 0, samplers: core::ptr::null(), banderas: 1 };
    type Serializar = extern "win64" fn(*const DescFirma, u32, *mut u64, *mut u64) -> i32;
    // SAFETY: la direccion de `D3D12SerializeRootSignature` de la casa.
    let f: Serializar = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12SerializeRootSignature")) };
    let (mut blob, mut error) = (0u64, 1u64);
    assert_eq!(f(&desc, 1, &mut blob, &mut error), 0);
    assert_eq!(error, 0, "sin blob de errores");
    type Puntero = extern "win64" fn(u64) -> *const u8;
    type Medida = extern "win64" fn(u64) -> usize;
    // SAFETY: los huecos 3 y 4 de un ID3DBlob de la casa.
    let (puntero, medida): (Puntero, Medida) = unsafe { (core::mem::transmute(hueco(blob, 3)), core::mem::transmute(hueco(blob, 4))) };
    // SAFETY: el blob da su puntero y su medida.
    unsafe { core::slice::from_raw_parts(puntero(blob), medida(blob)) }.to_vec()
}

#[test]
fn serialize_root_signature_lee_la_estructura_de_c_y_da_los_bytes_de_dxc() {
    let _uno = empezar();
    assert!(serializar() == RAIZ_RTS, "los mismos bytes que `dxc -T rootsig_1_0`, huella incluida");
    assert_eq!(dicho(), "");
}

/// Un elemento del input layout (32 B) y el PSO (656 B, los desplazamientos
/// de d3d12.h que `cubo.c` comprueba al compilar).
#[repr(C)]
struct Elemento {
    semantica: *const u8,
    indice: u32,
    formato: u32,
    ranura: u32,
    desde: u32,
    clase: u32,
    paso: u32,
}

fn pso(raiz: u64, vs: &[u8], ps: &[u8], layout: &[Elemento]) -> i32 {
    pso_y_objeto(raiz, vs, ps, layout).0
}

fn pso_y_objeto(raiz: u64, vs: &[u8], ps: &[u8], layout: &[Elemento]) -> (i32, u64) {
    let mut d = [0u8; 656];
    let mut pon = |o: usize, v: &[u8]| d[o..o + v.len()].copy_from_slice(v);
    pon(0, &raiz.to_le_bytes());
    pon(8, &(vs.as_ptr() as u64).to_le_bytes());
    pon(16, &(vs.len() as u64).to_le_bytes());
    pon(24, &(ps.as_ptr() as u64).to_le_bytes());
    pon(32, &(ps.len() as u64).to_le_bytes());
    pon(448, &u32::MAX.to_le_bytes());
    pon(452, &3u32.to_le_bytes());
    pon(456, &3u32.to_le_bytes());
    pon(552, &(layout.as_ptr() as u64).to_le_bytes());
    pon(560, &(layout.len() as u32).to_le_bytes());
    pon(572, &3u32.to_le_bytes());
    pon(576, &1u32.to_le_bytes());
    pon(580, &28u32.to_le_bytes());
    pon(616, &1u32.to_le_bytes());
    let disp = dispositivo();
    type Crear = extern "win64" fn(u64, *const u8, *const Guid, *mut u64) -> i32;
    // SAFETY: el hueco 10 del dispositivo, CreateGraphicsPipelineState.
    let crear: Crear = unsafe { core::mem::transmute(hueco(disp, 10)) };
    let mut p = 0;
    (crear(disp, d.as_ptr(), &com::IID_PSO, &mut p), p)
}

fn raiz() -> u64 {
    let bytes = serializar();
    let disp = dispositivo();
    type Crear = extern "win64" fn(u64, u32, *const u8, usize, *const Guid, *mut u64) -> i32;
    // SAFETY: el hueco 16 del dispositivo, CreateRootSignature.
    let crear: Crear = unsafe { core::mem::transmute(hueco(disp, 16)) };
    let mut r = 0;
    assert_eq!(crear(disp, 0, bytes.as_ptr(), bytes.len(), &com::IID_ROOTSIG, &mut r), 0);
    r
}

fn layout(con_color: bool) -> Vec<Elemento> {
    let e = |s: &'static [u8], f, o| Elemento { semantica: s.as_ptr(), indice: 0, formato: f, ranura: 0, desde: o, clase: 0, paso: 0 };
    let mut v = vec![e(b"POSITION\0", 6, 0), e(b"NORMAL\0", 6, 0xFFFF_FFFF)];
    if con_color {
        v.push(e(b"COLOR\0", 2, 0xFFFF_FFFF));
    }
    v
}

#[test]
fn create_graphics_pipeline_state_cruza_el_layout_con_el_sombreador() {
    let _uno = empezar();
    let r = raiz();
    // Con APPEND_ALIGNED en NORMAL y COLOR: 12 y 24, como el de cubo.c.
    assert_eq!(pso(r, VS, PS, &layout(true)), 0);
    // Esta plataforma no sabe sellar codigo: la casa lo dice UNA vez, y los
    // sombreadores se interpretan (dan lo mismo).
    assert_eq!(dicho(), "PROTON-X: sin bloque sellado para el codigo nativo: los sombreadores se interpretan (dan lo mismo, mas despacio)\n");
    DICHO.lock().unwrap().clear();

    assert_eq!(pso(r, VS, PS, &layout(false)), E_INVALIDARG, "COLOR lo lee el sombreador y el layout no lo da");
    assert_eq!(dicho(), "PROTON-X: el sombreador de vertices lee una semantica que el input layout no da\n");

    DICHO.lock().unwrap().clear();
    assert_eq!(pso(r, PS, VS, &layout(true)), E_INVALIDARG);
    assert_eq!(dicho(), "PROTON-X: CreateGraphicsPipelineState: un sombreador de otra etapa en su hueco\n");

    DICHO.lock().unwrap().clear();
    assert_eq!(pso(0, VS, PS, &layout(true)), E_INVALIDARG);
    assert_eq!(dicho(), "PROTON-X: CreateGraphicsPipelineState sin root signature\n");
}

/// 03-10: dos PSO con los mismos sombreadores y layout COMPARTEN lo
/// compilado (`enlaces.rs`): el segundo no lee el DXIL ni lo compila otra
/// vez, y el PSO ya no guarda los `Sombreador` leidos (en el metal llenaban
/// el monton: 64 MiB con 176 PSO). Otro layout, otro enlace.
#[test]
fn los_pso_con_los_mismos_sombreadores_comparten_lo_compilado() {
    let _uno = empezar();
    let r = raiz();
    let (a, pa) = pso_y_objeto(r, VS, PS, &layout(true));
    let (b, pb) = pso_y_objeto(r, VS, PS, &layout(true));
    assert_eq!((a, b), (0, 0));
    assert_ne!(pa, pb, "dos PSO");
    // SAFETY: dos PSO de la casa.
    let (ca, cb) = unsafe { (&com::de::<bmo_proton_x_casa::tuberia::Pso>(pa).compilado, &com::de::<bmo_proton_x_casa::tuberia::Pso>(pb).compilado) };
    assert!(std::rc::Rc::ptr_eq(ca, cb), "lo compilado es uno");
    assert_eq!((ca.nombres.0.as_str(), ca.nombres.1.as_str()), ("vertice", "pixel"));
    assert!(ca.enlace.is_ok());
    // Con otro layout (COLOR en otro sitio) es otro.
    let mut otro = layout(true);
    otro[2].desde = 40;
    let (c, pc) = pso_y_objeto(r, VS, PS, &otro);
    assert_eq!(c, 0);
    // SAFETY: un PSO de la casa.
    assert!(!std::rc::Rc::ptr_eq(ca, unsafe { &com::de::<bmo_proton_x_casa::tuberia::Pso>(pc).compilado }));
}
