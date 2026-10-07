//! **EL CENSO DEL ABI** (03-10): un objeto de cada interfaz COM de la casa
//! (D3D12, DXGI y WASAPI), creado por sus puertas de Windows como lo haria
//! un `.exe`, y de cada vtabla lo que es cada hueco -- lo HACE, es una FALLA
//! documentada (el HRESULT de Windows sin esa funcion, dicho) o FALTA (dice
//! cual y sale). Es la tabla de `docs/maestro/D3D12_MAESTRO.md`:
//!
//! ```text
//!    cargo test -p bmo-proton-x-casa --test abi -- --nocapture
//! ```
//!
//! La prueba no mira numeros que cambian con cada metodo nuevo: mira que se
//! creo cada interfaz, que IUnknown nunca falta, y que el censo cuadra con
//! el de la tabla del documento (la linea `TOTAL`).

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::Funcion;
use bmo_proton_x_casa::com::{self, guid, Guid, Hueco};
use bmo_proton_x_casa::{Plataforma, Sonido, Superficie};

const VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/textura_ps.dxil");

static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());

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
    1_000_000_000
}
fn abrir() -> Option<u32> {
    Some(48_000)
}
fn sonar(b: &[u8]) -> usize {
    b.len() / 4 * 4
}
fn pendientes() -> usize {
    0
}
fn cerrar() {}

fn funcion(dll: &str, n: &str) -> usize {
    bmo_proton_x_casa::tabla(dll, &Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n} no esta en la tabla")) as usize
}

/// El hueco `h` de la vtabla de un objeto de la casa, como funcion `F`.
fn hueco<F: Copy>(obj: u64, h: usize) -> F {
    // SAFETY: un objeto de la casa: su primer puntero es la vtabla; `F` es la
    // firma de ese hueco.
    unsafe {
        let d = (*(obj as *const *const usize)).add(h).read();
        core::mem::transmute_copy(&d)
    }
}

type Crear = extern "win64" fn(u64, *const u8, *const Guid, *mut u64) -> i32;

/// `D3D12_RESOURCE_DESC` (56 B) de un bufer.
fn bufer(n: u64) -> [u8; 56] {
    let mut d = [0u8; 56];
    d[0..4].copy_from_slice(&1u32.to_le_bytes());
    d[16..24].copy_from_slice(&n.to_le_bytes());
    d[24..28].copy_from_slice(&1u32.to_le_bytes());
    d[28..30].copy_from_slice(&1u16.to_le_bytes());
    d[30..32].copy_from_slice(&1u16.to_le_bytes());
    d[36..40].copy_from_slice(&1u32.to_le_bytes());
    d[48..52].copy_from_slice(&1u32.to_le_bytes()); // ROW_MAJOR
    d
}

const IID_ENUMERADOR: Guid = guid(0xA956_64D2, 0x9614, 0x4F35, [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6]);
const CLSID_ENUMERADOR: Guid = guid(0xBCDE_0395, 0xE52F, 0x467C, [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]);
const IID_PUNTA: Guid = guid(0x1BE0_9788, 0x6894, 0x4089, [0x85, 0x86, 0x9A, 0x2A, 0x6C, 0x26, 0x5A, 0xC5]);
const IID_CLIENTE: Guid = guid(0x1CB9_AD4C, 0xDBFA, 0x4C32, [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2]);
const SERVICIOS: [Guid; 6] = [
    guid(0xF294_ACFC, 0x3146, 0x4483, [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2]), // IAudioRenderClient
    guid(0xCD63_314F, 0x3FBA, 0x4A1B, [0x81, 0x2C, 0xEF, 0x96, 0x35, 0x87, 0x28, 0xE7]), // IAudioClock
    guid(0x87CE_5498, 0x68D6, 0x44E5, [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8]), // ISimpleAudioVolume
    guid(0xF4B1_A599, 0x7266, 0x4319, [0xA8, 0xCA, 0xE7, 0x0A, 0xCB, 0x11, 0xE8, 0xCD]), // IAudioSessionControl
    guid(0x9301_4887, 0x242D, 0x4068, [0x8A, 0x15, 0xCF, 0x5E, 0x93, 0xB9, 0x0F, 0xE3]), // IAudioStreamVolume
    guid(0x1C15_8861, 0xB533, 0x4B30, [0xB1, 0xCF, 0xE8, 0x53, 0xE5, 0x1C, 0x59, 0xB8]), // IChannelAudioVolume
];
const IID_CONSULTAS: Guid = guid(0x0d9658ae, 0xed45, 0x469e, [0xa6, 0x1d, 0x97, 0x0e, 0xc5, 0x83, 0xca, 0xb4]);
const IID_FIRMA: Guid = guid(0xc36a797c, 0xec80, 0x4f0a, [0x89, 0x85, 0xa7, 0xb2, 0x47, 0x50, 0x82, 0xd1]);

/// **Un objeto de cada interfaz**, por las puertas de Windows.
fn crear_de_todo() {
    // SAFETY: ningun `.exe` corre; la unica prueba de este fichero.
    unsafe { bmo_proton_x_casa::empezar(Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x::lote::en_cpu, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar, carpetas: None, reserva: None, trozos: None, sonido: Some(Sonido { abrir, escribir: sonar, pendientes, cerrar }), cuaderno: None }) };
    let ok = |hr: i32, que: &str| assert_eq!(hr, 0, "{que}: {}", String::from_utf8_lossy(&DICHO.lock().unwrap()));

    // -- D3D12 ---------------------------------------------------------------
    type CrearDisp = extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32;
    // SAFETY: la direccion de `D3D12CreateDevice` de la casa.
    let crear: CrearDisp = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12CreateDevice")) };
    let mut disp = 0;
    ok(crear(0, 0xb000, &com::IID_DEVICE, &mut disp), "D3D12CreateDevice");
    let cq: Crear = hueco(disp, 8);
    let mut cola = 0;
    ok(cq(disp, [0u8; 16].as_ptr(), &com::IID_QUEUE, &mut cola), "CreateCommandQueue");
    let ca: extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32 = hueco(disp, 9);
    let mut asig = 0;
    ok(ca(disp, 0, &com::IID_ALLOCATOR, &mut asig), "CreateCommandAllocator");
    let cl: extern "win64" fn(u64, u32, u32, u64, u64, *const Guid, *mut u64) -> i32 = hueco(disp, 12);
    let mut lista = 0;
    ok(cl(disp, 0, 0, asig, 0, &com::IID_LIST, &mut lista), "CreateCommandList");
    let mut dh = [0u8; 16];
    dh[4..8].copy_from_slice(&4u32.to_le_bytes());
    let cdh: Crear = hueco(disp, 14);
    let mut montonc = 0;
    ok(cdh(disp, dh.as_ptr(), &com::IID_HEAP, &mut montonc), "CreateDescriptorHeap");
    type Committed = extern "win64" fn(u64, *const u8, u32, *const u8, u32, *const u8, *const Guid, *mut u64) -> i32;
    let cc: Committed = hueco(disp, 27);
    let mut recurso = 0;
    ok(cc(disp, [0u8; 20].as_ptr(), 0, bufer(256).as_ptr(), 0, core::ptr::null(), &com::IID_RESOURCE, &mut recurso), "CreateCommittedResource");
    let cf: extern "win64" fn(u64, u64, u32, *const Guid, *mut u64) -> i32 = hueco(disp, 36);
    let mut valla = 0;
    ok(cf(disp, 0, 0, &com::IID_FENCE, &mut valla), "CreateFence");
    let mut hd = [0u8; 48];
    hd[0..8].copy_from_slice(&65_536u64.to_le_bytes());
    hd[8..12].copy_from_slice(&1u32.to_le_bytes());
    let ch: Crear = hueco(disp, 28);
    let mut monton = 0;
    ok(ch(disp, hd.as_ptr(), &com::IID_MEMORIA, &mut monton), "CreateHeap");
    let cqh: Crear = hueco(disp, 39);
    let mut consultas = 0;
    ok(cqh(disp, [0u32, 4, 0].as_ptr() as *const u8, &IID_CONSULTAS, &mut consultas), "CreateQueryHeap");
    // La firma raiz, vacia (y su blob).
    let mut firma = [0u8; 40];
    firma[32..36].copy_from_slice(&1u32.to_le_bytes());
    let (mut blob, mut error) = (0u64, 0u64);
    // SAFETY: la direccion de `D3D12SerializeRootSignature` de la casa.
    let s: extern "win64" fn(*const u8, u32, *mut u64, *mut u64) -> i32 = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12SerializeRootSignature")) };
    ok(s(firma.as_ptr(), 1, &mut blob, &mut error), "D3D12SerializeRootSignature");
    let (ptr, n): (extern "win64" fn(u64) -> *const u8, extern "win64" fn(u64) -> usize) = (hueco(blob, 3), hueco(blob, 4));
    let rs: extern "win64" fn(u64, u32, *const u8, usize, *const Guid, *mut u64) -> i32 = hueco(disp, 16);
    let mut raiz = 0;
    ok(rs(disp, 0, ptr(blob), n(blob), &com::IID_ROOTSIG, &mut raiz), "CreateRootSignature");
    // Una firma de ordenes indirectas: un DRAW.
    let argumento = [0u32; 4];
    let mut cs = [0u8; 24];
    cs[0..4].copy_from_slice(&16u32.to_le_bytes());
    cs[4..8].copy_from_slice(&1u32.to_le_bytes());
    cs[8..16].copy_from_slice(&(argumento.as_ptr() as u64).to_le_bytes());
    let ccs: extern "win64" fn(u64, *const u8, u64, *const Guid, *mut u64) -> i32 = hueco(disp, 41);
    let mut ordenes = 0;
    ok(ccs(disp, cs.as_ptr(), 0, &IID_FIRMA, &mut ordenes), "CreateCommandSignature");
    // Un PSO grafico (los sombreadores de textura.hlsl).
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
    let layout = [Elemento { semantica: b"POSITION\0".as_ptr(), indice: 0, formato: 6, ranura: 0, desde: 0, clase: 0, paso: 0 }, Elemento { semantica: b"TEXCOORD\0".as_ptr(), indice: 0, formato: 16, ranura: 0, desde: 12, clase: 0, paso: 0 }];
    let mut d = [0u8; 656];
    let mut pon = |o: usize, v: &[u8]| d[o..o + v.len()].copy_from_slice(v);
    pon(0, &raiz.to_le_bytes());
    pon(8, &(VS.as_ptr() as u64).to_le_bytes());
    pon(16, &(VS.len() as u64).to_le_bytes());
    pon(24, &(PS.as_ptr() as u64).to_le_bytes());
    pon(32, &(PS.len() as u64).to_le_bytes());
    pon(164, &[0x0F]);
    pon(448, &u32::MAX.to_le_bytes());
    pon(452, &3u32.to_le_bytes());
    pon(456, &1u32.to_le_bytes());
    pon(552, &(layout.as_ptr() as u64).to_le_bytes());
    pon(560, &(layout.len() as u32).to_le_bytes());
    pon(572, &3u32.to_le_bytes());
    pon(576, &1u32.to_le_bytes());
    pon(580, &28u32.to_le_bytes());
    pon(616, &1u32.to_le_bytes());
    let cpso: Crear = hueco(disp, 10);
    let mut pso = 0;
    ok(cpso(disp, d.as_ptr(), &com::IID_PSO, &mut pso), "CreateGraphicsPipelineState");

    // -- DXGI ----------------------------------------------------------------
    // SAFETY: la direccion de `CreateDXGIFactory2` de la casa.
    let cfab: extern "win64" fn(u32, *const Guid, *mut u64) -> i32 = unsafe { core::mem::transmute(funcion("dxgi.dll", "CreateDXGIFactory2")) };
    let mut fabrica = 0;
    ok(cfab(0, &com::IID_FACTORY2, &mut fabrica), "CreateDXGIFactory2");
    let ea: extern "win64" fn(u64, u32, *mut u64) -> i32 = hueco(fabrica, 12);
    let mut adaptador = 0;
    ok(ea(fabrica, 0, &mut adaptador), "EnumAdapters1");
    let eo: extern "win64" fn(u64, u32, *mut u64) -> i32 = hueco(adaptador, 7);
    let mut salida = 0;
    ok(eo(adaptador, 0, &mut salida), "EnumOutputs");

    // -- WASAPI --------------------------------------------------------------
    type CoCrear = extern "win64" fn(*const Guid, u64, u32, *const Guid, *mut u64) -> u32;
    // SAFETY: CoCreateInstance de la casa.
    let co: CoCrear = unsafe { core::mem::transmute(funcion("ole32.dll", "CoCreateInstance")) };
    let mut en = 0u64;
    ok(co(&CLSID_ENUMERADOR, 0, 1, &IID_ENUMERADOR, &mut en) as i32, "CoCreateInstance(MMDeviceEnumerator)");
    let enumerar: extern "win64" fn(u64, u32, u32, *mut u64) -> i32 = hueco(en, 3);
    let mut coleccion = 0u64;
    ok(enumerar(en, 0, 1, &mut coleccion), "EnumAudioEndpoints");
    let defecto: extern "win64" fn(u64, u32, u32, *mut u64) -> i32 = hueco(en, 4);
    let mut ap = 0u64;
    ok(defecto(en, 0, 0, &mut ap), "GetDefaultAudioEndpoint");
    let qi: extern "win64" fn(u64, *const Guid, *mut u64) -> i32 = hueco(ap, 0);
    let mut punta = 0u64;
    ok(qi(ap, &IID_PUNTA, &mut punta), "IMMEndpoint");
    let props: extern "win64" fn(u64, u32, *mut u64) -> i32 = hueco(ap, 4);
    let mut almacen = 0u64;
    ok(props(ap, 0, &mut almacen), "OpenPropertyStore");
    let activar: extern "win64" fn(u64, *const Guid, u32, u64, *mut u64) -> i32 = hueco(ap, 3);
    let mut cliente = 0u64;
    ok(activar(ap, &IID_CLIENTE, 1, 0, &mut cliente), "Activate(IAudioClient)");
    let mezcla: extern "win64" fn(u64, *mut u64) -> i32 = hueco(cliente, 8);
    let mut fmt = 0u64;
    ok(mezcla(cliente, &mut fmt), "GetMixFormat");
    let iniciar: extern "win64" fn(u64, u32, u32, i64, i64, u64, u64) -> i32 = hueco(cliente, 3);
    ok(iniciar(cliente, 0, 0, 200_000, 0, fmt, 0), "Initialize");
    let servicio: extern "win64" fn(u64, *const Guid, *mut u64) -> i32 = hueco(cliente, 14);
    for g in &SERVICIOS {
        let mut s = 0u64;
        ok(servicio(cliente, g, &mut s), "GetService");
    }
}

fn tabla(censo: &[(&str, Vec<(&str, Hueco)>)]) -> String {
    let mut t = String::new();
    let (mut hace, mut falla, mut falta) = (0, 0, 0);
    for (nombre, huecos) in censo {
        let n = |e: Hueco| huecos.iter().filter(|x| x.1 == e).count();
        t.push_str(&format!("\n### {nombre} -- {} metodos: {} hace, {} falla documentada, {} falta\n\n", huecos.len(), n(Hueco::Hace), n(Hueco::Falla), n(Hueco::Falta)));
        hace += n(Hueco::Hace);
        falla += n(Hueco::Falla);
        falta += n(Hueco::Falta);
        for (e, que) in [(Hueco::Falla, "falla documentada"), (Hueco::Falta, "FALTA")] {
            let v: Vec<String> = huecos.iter().enumerate().filter(|x| x.1 .1 == e).map(|(h, x)| format!("{h} {}", x.0)).collect();
            if !v.is_empty() {
                t.push_str(&format!("- {que}: {}\n", v.join(", ")));
            }
        }
    }
    t.push_str(&format!("\nTOTAL: {} interfaces, {} huecos: {hace} hace, {falla} falla documentada, {falta} falta\n", censo.len(), hace + falla + falta));
    t
}

/// *** El censo: las 28 interfaces que la casa sabe dar (todas menos la
/// cadena de intercambio, que pide una ventana), IUnknown en todas, y la
/// cuenta del documento al dia.
#[test]
fn el_censo_del_abi_cuadra_con_el_documento() {
    crear_de_todo();
    let censo = com::censo();
    let t = tabla(&censo);
    println!("{t}");
    let nombres: Vec<&str> = censo.iter().map(|c| c.0).collect();
    assert_eq!(censo.len(), 28, "{nombres:?}");
    for (nombre, huecos) in &censo {
        assert!(huecos.iter().take(3).all(|x| x.1 == Hueco::Hace), "{nombre}: IUnknown");
    }
    let total = t.lines().find(|l| l.starts_with("TOTAL")).unwrap().to_string();
    let doc = include_str!("../../../../docs/maestro/D3D12_MAESTRO.md");
    assert!(doc.contains(&total), "el documento no tiene `{total}`: correr esta prueba con --nocapture y poner la tabla nueva");
}
