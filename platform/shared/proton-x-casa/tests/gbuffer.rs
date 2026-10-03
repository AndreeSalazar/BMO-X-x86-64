//! **EL G-BUFFER, por las puertas de Windows de la casa** (03-10, N5.8):
//! un PSO con CUATRO render targets (el 2 sin vista, el 3 en BGRA), puestos
//! con `OMSetRenderTargets` de descriptores CONSECUTIVOS, un Draw, y cada
//! render target leido de vuelta. Los sombreadores son los de
//! `proton-x/prueba/textura.hlsl` (vertices) y `gbuffer.hlsl` (pixeles).

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::Funcion;
use bmo_proton_x_casa::com::{self, Guid};
use bmo_proton_x_casa::{Plataforma, Superficie};

const VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/gbuffer.dxil");

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

fn funcion(dll: &str, n: &str) -> usize {
    bmo_proton_x_casa::tabla(dll, &Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n} no esta en la tabla")) as usize
}

/// El hueco `h` de la vtabla de un objeto de la casa, como funcion `F`.
fn hueco<F: Copy>(obj: u64, h: usize) -> F {
    // SAFETY: un objeto de la casa: su primer puntero es la vtabla; `F` es
    // la firma de ese hueco (la pone quien llama).
    unsafe {
        let d = (*(obj as *const *const usize)).add(h).read();
        core::mem::transmute_copy(&d)
    }
}

type Crear = extern "win64" fn(u64, *const u8, *const Guid, *mut u64) -> i32;


/// `D3D12_RESOURCE_DESC` (56 B).
fn desc_recurso(dimension: u32, ancho: u64, alto: u32, formato: u32) -> [u8; 56] {
    let mut d = [0u8; 56];
    d[0..4].copy_from_slice(&dimension.to_le_bytes());
    d[16..24].copy_from_slice(&ancho.to_le_bytes());
    d[24..28].copy_from_slice(&alto.to_le_bytes());
    d[28..30].copy_from_slice(&1u16.to_le_bytes());
    d[30..32].copy_from_slice(&1u16.to_le_bytes());
    d[32..36].copy_from_slice(&formato.to_le_bytes());
    d[36..40].copy_from_slice(&1u32.to_le_bytes());
    d
}

fn recurso(disp: u64, desc: &[u8; 56]) -> u64 {
    type Committed = extern "win64" fn(u64, *const u8, u32, *const u8, u32, *const u8, *const Guid, *mut u64) -> i32;
    let f: Committed = hueco(disp, 27);
    let mut r = 0;
    assert_eq!(f(disp, [0u8; 20].as_ptr(), 0, desc.as_ptr(), 0, core::ptr::null(), &com::IID_RESOURCE, &mut r), 0);
    r
}

fn mapear(r: u64) -> *mut u8 {
    let f: extern "win64" fn(u64, u32, *const u8, *mut u64) -> i32 = hueco(r, 8);
    let mut p = 0;
    assert_eq!(f(r, 0, core::ptr::null(), &mut p), 0);
    p as *mut u8
}

fn monton(disp: u64, tipo: u32, n: u32, visible: bool) -> (u64, u64, u64) {
    let f: Crear = hueco(disp, 14);
    let mut d = [0u8; 16];
    d[0..4].copy_from_slice(&tipo.to_le_bytes());
    d[4..8].copy_from_slice(&n.to_le_bytes());
    d[8..12].copy_from_slice(&(visible as u32).to_le_bytes());
    let mut m = 0;
    assert_eq!(f(disp, d.as_ptr(), &com::IID_HEAP, &mut m), 0);
    let inicio = |h: usize| {
        let g: extern "win64" fn(u64, *mut u64) -> *mut u64 = hueco(m, h);
        let mut x = 0;
        g(m, &mut x);
        x
    };
    (m, inicio(9), inicio(10))
}


const RGBA8: u32 = 28;
const BGRA8: u32 = 87;

/// Cada render target leido de vuelta (8x8), o `None` si no se creo.
fn g_buffer(mezcla: bool) -> [Option<Vec<u32>>; 4] {
    // SAFETY: ningun `.exe` corre; la unica prueba de este fichero.
    unsafe { bmo_proton_x_casa::empezar(Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: bmo_proton_x::lote::en_cpu, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar, carpetas: None, reserva: None, trozos: None, sonido: None }) };
    type CrearDisp = extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32;
    // SAFETY: la direccion de `D3D12CreateDevice` de la casa.
    let crear: CrearDisp = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12CreateDevice")) };
    let mut disp = 0;
    assert_eq!(crear(0, 0xb000, &com::IID_DEVICE, &mut disp), 0);

    // Una firma vacia (solo ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT), en 1.0.
    let mut firma = [0u8; 40];
    firma[32..36].copy_from_slice(&1u32.to_le_bytes());
    let (mut blob, mut error) = (0u64, 0u64);
    // SAFETY: la direccion de `D3D12SerializeRootSignature` de la casa.
    let s: extern "win64" fn(*const u8, u32, *mut u64, *mut u64) -> i32 = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12SerializeRootSignature")) };
    assert_eq!(s(firma.as_ptr(), 1, &mut blob, &mut error), 0);
    let (ptr, n): (extern "win64" fn(u64) -> *const u8, extern "win64" fn(u64) -> usize) = (hueco(blob, 3), hueco(blob, 4));
    let rs: extern "win64" fn(u64, u32, *const u8, usize, *const Guid, *mut u64) -> i32 = hueco(disp, 16);
    let mut raiz = 0;
    assert_eq!(rs(disp, 0, ptr(blob), n(blob), &com::IID_ROOTSIG, &mut raiz), 0);

    // El PSO: POSITION float3 y TEXCOORD float2; cuatro render targets.
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
    let formatos = [RGBA8, RGBA8, RGBA8, BGRA8];
    let mut d = [0u8; 656];
    let mut pon = |o: usize, v: &[u8]| d[o..o + v.len()].copy_from_slice(v);
    pon(0, &raiz.to_le_bytes());
    pon(8, &(VS.as_ptr() as u64).to_le_bytes());
    pon(16, &(VS.len() as u64).to_le_bytes());
    pon(24, &(PS.as_ptr() as u64).to_le_bytes());
    pon(32, &(PS.len() as u64).to_le_bytes());
    // BlendState (+120).RenderTarget[0] (+8).RenderTargetWriteMask (+36):
    // ALL (sin IndependentBlendEnable vale para los cuatro).
    pon(164, &[0x0F]);
    if mezcla {
        // N5.11: IndependentBlendEnable, y en el 0: BLEND_FACTOR, ZERO, ADD
        // (color y alfa); los demas, apagados y RGBA.
        pon(124, &1u32.to_le_bytes());
        for (k, v) in [1u32, 0, 14, 1, 1, 14, 1, 1].into_iter().enumerate() {
            pon(128 + 4 * k, &v.to_le_bytes());
        }
        for i in 1..4 {
            pon(128 + 40 * i + 36, &[0x0F]);
        }
    }
    pon(448, &u32::MAX.to_le_bytes());
    pon(452, &3u32.to_le_bytes());
    pon(456, &1u32.to_le_bytes()); // CULL_NONE
    pon(552, &(layout.as_ptr() as u64).to_le_bytes());
    pon(560, &(layout.len() as u32).to_le_bytes());
    pon(572, &3u32.to_le_bytes());
    pon(576, &4u32.to_le_bytes());
    for (i, f) in formatos.iter().enumerate() {
        pon(580 + 4 * i, &f.to_le_bytes());
    }
    pon(616, &1u32.to_le_bytes());
    let cpso: Crear = hueco(disp, 10);
    let mut pso = 0;
    assert_eq!(cpso(disp, d.as_ptr(), &com::IID_PSO, &mut pso), 0, "{}", String::from_utf8_lossy(&DICHO.lock().unwrap()));

    // Los render targets 0, 1 y 3 (8x8) y sus RTV, CONSECUTIVOS en un monton
    // de 4: la ranura 2 queda vacia (un RTV nulo).
    let (_rtvs, rtv, _) = monton(disp, 2, 4, false);
    let paso: extern "win64" fn(u64, u32) -> u32 = hueco(disp, 15);
    let incremento = paso(disp, 2) as u64;
    let crtv: extern "win64" fn(u64, u64, *const u8, u64) = hueco(disp, 20);
    let rts: Vec<Option<u64>> = (0..4)
        .map(|i| {
            (i != 2).then(|| {
                let r = recurso(disp, &desc_recurso(3, 8, 8, formatos[i]));
                crtv(disp, r, core::ptr::null(), rtv + i as u64 * incremento);
                r
            })
        })
        .collect();
    // Dos triangulos que cubren todo.
    let v: [[f32; 5]; 6] = [[-1.0, 1.0, 0.0, 0.0, 0.0], [1.0, 1.0, 0.0, 1.0, 0.0], [-1.0, -1.0, 0.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0, 0.0], [1.0, -1.0, 0.0, 1.0, 1.0], [-1.0, -1.0, 0.0, 0.0, 1.0]];
    let vb = recurso(disp, &desc_recurso(1, core::mem::size_of_val(&v) as u64, 1, 0));
    // SAFETY: el bufer mide lo mismo que `v`.
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr() as *const u8, mapear(vb), core::mem::size_of_val(&v)) };
    let va: extern "win64" fn(u64) -> u64 = hueco(vb, 11);
    let lecturas: Vec<u64> = (0..4).map(|_| recurso(disp, &desc_recurso(1, 256 * 8, 1, 0))).collect();

    let cq: Crear = hueco(disp, 8);
    let mut cola = 0;
    assert_eq!(cq(disp, [0u8; 16].as_ptr(), &com::IID_QUEUE, &mut cola), 0);
    let ca: extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32 = hueco(disp, 9);
    let mut asig = 0;
    assert_eq!(ca(disp, 0, &com::IID_ALLOCATOR, &mut asig), 0);
    let cl: extern "win64" fn(u64, u32, u32, u64, u64, *const Guid, *mut u64) -> i32 = hueco(disp, 12);
    let mut l = 0;
    assert_eq!(cl(disp, 0, 0, asig, pso, &com::IID_LIST, &mut l), 0);
    let ubicacion = |r: u64, tipo: u32, formato: u32| {
        let mut x = [0u8; 48];
        x[0..8].copy_from_slice(&r.to_le_bytes());
        x[8..12].copy_from_slice(&tipo.to_le_bytes());
        if tipo == 1 {
            x[24..28].copy_from_slice(&formato.to_le_bytes());
            x[28..32].copy_from_slice(&8u32.to_le_bytes());
            x[32..36].copy_from_slice(&8u32.to_le_bytes());
            x[36..40].copy_from_slice(&1u32.to_le_bytes());
            x[40..44].copy_from_slice(&256u32.to_le_bytes());
        }
        x
    };
    let set_raiz: extern "win64" fn(u64, u64) = hueco(l, 30);
    set_raiz(l, raiz);
    let vp: [f32; 6] = [0.0, 0.0, 8.0, 8.0, 0.0, 1.0];
    let set_vp: extern "win64" fn(u64, u32, *const f32) = hueco(l, 21);
    set_vp(l, 1, vp.as_ptr());
    let tijera: [i32; 4] = [0, 0, 8, 8];
    let set_tijera: extern "win64" fn(u64, u32, *const i32) = hueco(l, 22);
    set_tijera(l, 1, tijera.as_ptr());
    // Cuatro descriptores CONSECUTIVOS desde `rtv` (RTsSingleHandleToDescriptorRange).
    let om: extern "win64" fn(u64, u32, *const u64, i32, *const u64) = hueco(l, 46);
    om(l, 4, &rtv, 1, core::ptr::null());
    if mezcla {
        let factor: extern "win64" fn(u64, *const f32) = hueco(l, 23);
        factor(l, [0.5f32, 0.25, 1.0, 0.5].as_ptr());
    }
    // El 1 se limpia antes: el dibujo lo pisa entero igual.
    let limpiar: extern "win64" fn(u64, u64, *const f32, u32, *const u8) = hueco(l, 48);
    limpiar(l, rtv + incremento, [0.5f32, 0.5, 0.5, 0.5].as_ptr(), 0, core::ptr::null());
    let topo: extern "win64" fn(u64, u32) = hueco(l, 20);
    topo(l, 4);
    let vista_vb: [u64; 2] = [va(vb), core::mem::size_of_val(&v) as u64 | 20 << 32];
    let set_vb: extern "win64" fn(u64, u32, u32, *const u64) = hueco(l, 44);
    set_vb(l, 0, 1, vista_vb.as_ptr());
    let draw: extern "win64" fn(u64, u32, u32, u32, u32) = hueco(l, 12);
    draw(l, 6, 1, 0, 0);
    let copia: extern "win64" fn(u64, *const u8, u32, u32, u32, *const u8, *const u8) = hueco(l, 16);
    for (i, r) in rts.iter().enumerate() {
        if let Some(r) = *r {
            copia(l, ubicacion(lecturas[i], 1, formatos[i]).as_ptr(), 0, 0, 0, ubicacion(r, 0, 0).as_ptr(), core::ptr::null());
        }
    }
    let cerrar: extern "win64" fn(u64) -> i32 = hueco(l, 9);
    assert_eq!(cerrar(l), 0);
    let ejecutar: extern "win64" fn(u64, u32, *const u64) = hueco(cola, 10);
    ejecutar(cola, 1, &l);
    core::array::from_fn(|i| {
        rts[i].map(|_| {
            let q = mapear(lecturas[i]);
            // SAFETY: dentro del bufer de lectura (256 * 8).
            (0..64).map(|k| unsafe { (q.add(256 * (k / 8) + 4 * (k % 8)) as *const u32).read_unaligned() }).collect()
        })
    })
}

fn unorm8(x: f32) -> u32 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32
}

/// *** N5.8: un Draw sobre cuatro render targets: el 0 rojo, el 1 su
/// SV_Position, el 2 (sin vista) nada, el 3 la normal en BGRA. Antes el
/// PSO se negaba, y `OMSetRenderTargets` tomaba solo el primero.
#[test]
fn un_draw_pinta_el_g_buffer_entero() {
    let [albedo, sitio, nada, normal] = g_buffer(false);
    let dicho = String::from_utf8_lossy(&DICHO.lock().unwrap()).into_owned();
    assert!(!dicho.contains("no se dibuja") && !dicho.contains("render target"), "{dicho}");
    assert!(nada.is_none());
    let (albedo, sitio, normal) = (albedo.unwrap(), sitio.unwrap(), normal.unwrap());
    assert!(albedo.iter().all(|&p| p == 0xFF00_00FF), "rojo en RGBA: {:08x}", albedo[0]);
    assert!(normal.iter().all(|&p| p == 0x8000_00FF), "azul con alfa 0.5 en BGRA: {:08x}", normal[0]);
    for (k, &p) in sitio.iter().enumerate() {
        let (x, y) = ((k % 8) as f32 + 0.5, (k / 8) as f32 + 0.5);
        assert_eq!(p, 0xFF00_0000 | unorm8(y / 8.0) << 8 | unorm8(x / 8.0), "({x}, {y})");
    }
}

/// *** N5.11: el mismo Draw con MEZCLA en el render target 0 (BLEND_FACTOR,
/// ZERO, con `OMSetBlendFactor`): el rojo por el factor; los demas, sin
/// mezcla (IndependentBlendEnable), igual que antes. Antes ese Draw se
/// decia y no se pintaba.
#[test]
fn la_mezcla_y_el_factor_de_mezcla_llegan_al_draw() {
    let [albedo, sitio, _, normal] = g_buffer(true);
    let dicho = String::from_utf8_lossy(&DICHO.lock().unwrap()).into_owned();
    assert!(!dicho.contains("no se dibuja"), "{dicho}");
    // (1, 0, 0, 1) * (0.5, 0.25, 1, 0.5) = (0.5, 0, 0, 0.5).
    assert!(albedo.unwrap().iter().all(|&p| p == 0x8000_0080), "el 0, mezclado");
    assert!(normal.unwrap().iter().all(|&p| p == 0x8000_00FF), "el 3, sin mezcla");
    assert_eq!(sitio.unwrap()[0], 0xFF00_0000 | unorm8(0.5 / 8.0) << 8 | unorm8(0.5 / 8.0));
}
