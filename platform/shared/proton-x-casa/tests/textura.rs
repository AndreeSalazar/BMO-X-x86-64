//! **LA TEXTURA DE HELLOTEXTURE, por las puertas de Windows de la casa**
//! (29-09). El camino de `D3D12HelloTexture` (DirectX-Graphics-Samples de
//! Microsoft) llamado a mano, como lo llamaria el `.exe`, y dibujado en un
//! render target que se lee de vuelta:
//!
//! ```text
//!    D3D12SerializeVersionedRootSignature  una tabla (SRV t0) y un sampler
//!                                          estatico (PUNTO, BORDE), en 1.1
//!    CreateCommittedResource              la textura (8x8 RGBA) y el RT
//!    CopyTextureRegion bufer -> textura   lo que hace UpdateSubresources
//!    CreateShaderResourceView             en un monton SHADER_VISIBLE
//!    SetDescriptorHeaps, SetGraphicsRootDescriptorTable, DrawInstanced
//!    CopyTextureRegion RT -> bufer        y se lee con Map
//! ```
//!
//! Los sombreadores son los de `proton-x/prueba/textura.hlsl` (dxc): la
//! posicion tal cual y `imagen.Sample(muestreo, uv)`. La imagen esperada
//! sale de las reglas de D3D12 (el centro del pixel, `floor(uv * 8)`), no de
//! la casa.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::Mutex;

use bmo_proton_x::Funcion;
use bmo_proton_x_casa::com::{self, Guid};
use bmo_proton_x_casa::{Plataforma, Superficie};

const VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/textura_ps.dxil");

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
fn memoria(_: usize) -> Option<u64> {
    None
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

/// Una textura 8x8 de colores distintos (R = x, G = y, B = tablero).
fn texel(x: u32, y: u32) -> u32 {
    0xFF00_0000 | (x * 30) | (y * 30) << 8 | ((x + y) % 2 * 255) << 16
}

/// Lo que el PSO de HelloTexture es para la 3060 (P3b4c.8 T2b): sus
/// texturas en la receta, y si el cuerpo de pixel pasa el juez R7 con el asa
/// del kernel.
static PARA_LA_3060: Mutex<Vec<(Vec<(u8, u8)>, bool, usize)>> = Mutex::new(Vec::new());

/// El ejecutor: la CPU dibuja (el juez de la imagen), y de paso se mira lo
/// que la puerta de la 3060 haria con el PSO.
fn dibujar_y_su_tex(l: &bmo_proton_x::lote::Lote, d: &mut bmo_proton_x::trama::Destino) -> Result<bmo_proton_x::trama::Cuenta, bmo_proton_x::lote::NoDibuja> {
    use bmo_gpu_ga10x::pegamento::{asas, Carga};
    if let Ok(c) = bmo_proton_x_sm86::puerta::cuerpos(l.enlace, l.entradas) {
        let cuerpo: Vec<(u64, u64)> = c.ps.chunks(16).map(|w| (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap()))).collect();
        let juzgado = bmo_gpu_ga10x::sass::juez::juzgar_cuerpo_con_asas(&cuerpo, c.registros_ps, asas(&c.cargas_ps)).is_ok();
        let texs = cuerpo.iter().filter(|w| w.0 & 0xFFF == 0x361).count();
        assert!(c.cargas_ps.iter().any(|x| matches!(x, Carga::Asa { textura: 0, .. })));
        PARA_LA_3060.lock().unwrap().push((c.texturas.clone(), juzgado, texs));
    }
    bmo_proton_x::lote::en_cpu(l, d)
}

/// **HelloTexture por la casa.** `version_1_1`: la firma como la manda
/// d3dx12 cuando el dispositivo dice 1.1 (la casa dice 1.0, y d3dx12 la
/// convierte; aqui se prueban los dos caminos).
fn hello_texture(version_1_1: bool) -> Vec<u32> {
    DICHO.lock().unwrap().clear();
    // SAFETY: ningun `.exe` corre; esta prueba no corre en paralelo con otra
    // que empiece la casa (es la unica de este fichero).
    unsafe { bmo_proton_x_casa::empezar(Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: dibujar_y_su_tex, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar }) };
    type CrearDisp = extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32;
    // SAFETY: la direccion de `D3D12CreateDevice` de la casa.
    let crear: CrearDisp = unsafe { core::mem::transmute(funcion("d3d12.dll", "D3D12CreateDevice")) };
    let mut disp = 0;
    assert_eq!(crear(0, 0xb000, &com::IID_DEVICE, &mut disp), 0);

    // CheckFeatureSupport(ROOT_SIGNATURE): la casa dice 1.0.
    let cfs: extern "win64" fn(u64, u32, *mut u8, u32) -> i32 = hueco(disp, 13);
    let mut version = 2u32;
    assert_eq!(cfs(disp, 12, &mut version as *mut u32 as *mut u8, 4), 0);
    assert_eq!(version, 1, "la casa contesta ROOT_SIGNATURE_VERSION_1_0");

    // La firma: una tabla con UN rango SRV t0 (visible al pixel) y el
    // sampler estatico de HelloTexture.
    let muestreador: [u32; 13] = [0, 4, 4, 4, 0, 0, 1, 0, 0, 0x7F7F_FFFF, 0, 0, 5];
    let rango_1_1: [u32; 6] = [0, 1, 0, 0, 8, 0]; // SRV, 1, t0, espacio 0, DATA_STATIC, desde 0
    let rango_1_0: [u32; 5] = [0, 1, 0, 0, 0];
    let mut param = [0u8; 32];
    param[8..12].copy_from_slice(&1u32.to_le_bytes());
    let rango_p = if version_1_1 { rango_1_1.as_ptr() as u64 } else { rango_1_0.as_ptr() as u64 };
    param[16..24].copy_from_slice(&rango_p.to_le_bytes());
    param[24..28].copy_from_slice(&5u32.to_le_bytes()); // PIXEL
    let mut firma = [0u8; 48];
    let (base, f) = if version_1_1 { (8, "D3D12SerializeVersionedRootSignature") } else { (0, "D3D12SerializeRootSignature") };
    firma[0..4].copy_from_slice(&(if version_1_1 { 2u32 } else { 1 }).to_le_bytes());
    firma[base..base + 4].copy_from_slice(&1u32.to_le_bytes());
    firma[base + 8..base + 16].copy_from_slice(&(param.as_ptr() as u64).to_le_bytes());
    firma[base + 16..base + 20].copy_from_slice(&1u32.to_le_bytes());
    firma[base + 24..base + 32].copy_from_slice(&(muestreador.as_ptr() as u64).to_le_bytes());
    firma[base + 32..base + 36].copy_from_slice(&1u32.to_le_bytes()); // ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT
    let (mut blob, mut error) = (0u64, 0u64);
    if version_1_1 {
        let s: extern "win64" fn(*const u8, *mut u64, *mut u64) -> i32 = unsafe { core::mem::transmute(funcion("d3d12.dll", f)) };
        assert_eq!(s(firma.as_ptr(), &mut blob, &mut error), 0);
    } else {
        let s: extern "win64" fn(*const u8, u32, *mut u64, *mut u64) -> i32 = unsafe { core::mem::transmute(funcion("d3d12.dll", f)) };
        assert_eq!(s(firma.as_ptr(), 1, &mut blob, &mut error), 0);
    }
    let (ptr, n): (extern "win64" fn(u64) -> *const u8, extern "win64" fn(u64) -> usize) = (hueco(blob, 3), hueco(blob, 4));
    let rs: extern "win64" fn(u64, u32, *const u8, usize, *const Guid, *mut u64) -> i32 = hueco(disp, 16);
    let mut raiz = 0;
    assert_eq!(rs(disp, 0, ptr(blob), n(blob), &com::IID_ROOTSIG, &mut raiz), 0);

    // El PSO: POSITION float3 y TEXCOORD float2, un RT RGBA8, sin descarte.
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
    // BlendState (+120).RenderTarget[0] (+8).RenderTargetWriteMask (+36): ALL.
    pon(164, &[0x0F]);
    pon(448, &u32::MAX.to_le_bytes());
    pon(452, &3u32.to_le_bytes());
    pon(456, &1u32.to_le_bytes()); // CULL_NONE
    pon(552, &(layout.as_ptr() as u64).to_le_bytes());
    pon(560, &(layout.len() as u32).to_le_bytes());
    pon(572, &3u32.to_le_bytes());
    pon(576, &1u32.to_le_bytes());
    pon(580, &28u32.to_le_bytes());
    pon(616, &1u32.to_le_bytes());
    let cpso: Crear = hueco(disp, 10);
    let mut pso = 0;
    assert_eq!(cpso(disp, d.as_ptr(), &com::IID_PSO, &mut pso), 0, "{}", String::from_utf8_lossy(&DICHO.lock().unwrap()));

    // La textura, y su subida (UpdateSubresources): un bufer con filas de
    // 256 B (D3D12_TEXTURE_DATA_PITCH_ALIGNMENT) y CopyTextureRegion.
    let tex = recurso(disp, &desc_recurso(3, 8, 8, 28));
    let subida = recurso(disp, &desc_recurso(1, 256 * 8, 1, 0));
    let p = mapear(subida);
    for y in 0..8u32 {
        for x in 0..8u32 {
            // SAFETY: dentro del bufer de la casa (256 * 8 bytes).
            unsafe { (p.add((256 * y + 4 * x) as usize) as *mut u32).write_unaligned(texel(x, y)) };
        }
    }
    // El SRV, en un monton CBV_SRV_UAV SHADER_VISIBLE.
    let (_srvs, srv_cpu, srv_gpu) = monton(disp, 0, 1, true);
    let csrv: extern "win64" fn(u64, u64, *const u8, u64) = hueco(disp, 18);
    let mut vista = [0u8; 40];
    vista[0..4].copy_from_slice(&28u32.to_le_bytes());
    vista[4..8].copy_from_slice(&4u32.to_le_bytes()); // TEXTURE2D
    vista[8..12].copy_from_slice(&0x1688u32.to_le_bytes());
    vista[20..24].copy_from_slice(&1u32.to_le_bytes()); // MipLevels
    csrv(disp, tex, vista.as_ptr(), srv_cpu);

    // El render target (64x64) y su RTV; los vertices (dos triangulos que
    // cubren todo) y el bufer de lectura.
    let rt = recurso(disp, &desc_recurso(3, 64, 64, 28));
    let (_rtvs, rtv, _) = monton(disp, 2, 1, false);
    let crtv: extern "win64" fn(u64, u64, *const u8, u64) = hueco(disp, 20);
    crtv(disp, rt, core::ptr::null(), rtv);
    let v: [[f32; 5]; 6] = [[-1.0, 1.0, 0.0, 0.0, 0.0], [1.0, 1.0, 0.0, 1.0, 0.0], [-1.0, -1.0, 0.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0, 0.0], [1.0, -1.0, 0.0, 1.0, 1.0], [-1.0, -1.0, 0.0, 0.0, 1.0]];
    let vb = recurso(disp, &desc_recurso(1, core::mem::size_of_val(&v) as u64, 1, 0));
    // SAFETY: el bufer mide lo mismo que `v`.
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr() as *const u8, mapear(vb), core::mem::size_of_val(&v)) };
    let va: extern "win64" fn(u64) -> u64 = hueco(vb, 11);
    let lectura = recurso(disp, &desc_recurso(1, 256 * 64, 1, 0));

    // La lista, como la graba HelloTexture.
    let cq: Crear = hueco(disp, 8);
    let mut cola = 0;
    assert_eq!(cq(disp, [0u8; 16].as_ptr(), &com::IID_QUEUE, &mut cola), 0);
    let ca: extern "win64" fn(u64, u32, *const Guid, *mut u64) -> i32 = hueco(disp, 9);
    let mut asig = 0;
    assert_eq!(ca(disp, 0, &com::IID_ALLOCATOR, &mut asig), 0);
    let cl: extern "win64" fn(u64, u32, u32, u64, u64, *const Guid, *mut u64) -> i32 = hueco(disp, 12);
    let mut l = 0;
    assert_eq!(cl(disp, 0, 0, asig, pso, &com::IID_LIST, &mut l), 0);
    let copia = |dst: &[u8; 48], src: &[u8; 48]| {
        let f: extern "win64" fn(u64, *const u8, u32, u32, u32, *const u8, *const u8) = hueco(l, 16);
        f(l, dst.as_ptr(), 0, 0, 0, src.as_ptr(), core::ptr::null());
    };
    let ubicacion = |r: u64, tipo: u32, huella: Option<(u32, u32, u32)>| {
        let mut x = [0u8; 48];
        x[0..8].copy_from_slice(&r.to_le_bytes());
        x[8..12].copy_from_slice(&tipo.to_le_bytes());
        if let Some((ancho, alto, paso)) = huella {
            x[24..28].copy_from_slice(&28u32.to_le_bytes());
            x[28..32].copy_from_slice(&ancho.to_le_bytes());
            x[32..36].copy_from_slice(&alto.to_le_bytes());
            x[36..40].copy_from_slice(&1u32.to_le_bytes());
            x[40..44].copy_from_slice(&paso.to_le_bytes());
        }
        x
    };
    copia(&ubicacion(tex, 0, None), &ubicacion(subida, 1, Some((8, 8, 256))));
    let set_raiz: extern "win64" fn(u64, u64) = hueco(l, 30);
    set_raiz(l, raiz);
    let set_montones: extern "win64" fn(u64, u32, *const u64) = hueco(l, 28);
    set_montones(l, 1, &_srvs);
    let set_tabla: extern "win64" fn(u64, u32, u64) = hueco(l, 32);
    set_tabla(l, 0, srv_gpu);
    let vp: [f32; 6] = [0.0, 0.0, 64.0, 64.0, 0.0, 1.0];
    let set_vp: extern "win64" fn(u64, u32, *const f32) = hueco(l, 21);
    set_vp(l, 1, vp.as_ptr());
    let tijera: [i32; 4] = [0, 0, 64, 64];
    let set_tijera: extern "win64" fn(u64, u32, *const i32) = hueco(l, 22);
    set_tijera(l, 1, tijera.as_ptr());
    let om: extern "win64" fn(u64, u32, *const u64, i32, *const u64) = hueco(l, 46);
    om(l, 1, &rtv, 0, core::ptr::null());
    let limpiar: extern "win64" fn(u64, u64, *const f32, u32, *const u8) = hueco(l, 48);
    limpiar(l, rtv, [0.0f32, 0.2, 0.4, 1.0].as_ptr(), 0, core::ptr::null());
    let topo: extern "win64" fn(u64, u32) = hueco(l, 20);
    topo(l, 4);
    let vista_vb: [u64; 2] = [va(vb), core::mem::size_of_val(&v) as u64 | 20 << 32];
    let set_vb: extern "win64" fn(u64, u32, u32, *const u64) = hueco(l, 44);
    set_vb(l, 0, 1, vista_vb.as_ptr());
    let draw: extern "win64" fn(u64, u32, u32, u32, u32) = hueco(l, 12);
    draw(l, 6, 1, 0, 0);
    copia(&ubicacion(lectura, 1, Some((64, 64, 256))), &ubicacion(rt, 0, None));
    let cerrar: extern "win64" fn(u64) -> i32 = hueco(l, 9);
    assert_eq!(cerrar(l), 0);
    let ejecutar: extern "win64" fn(u64, u32, *const u64) = hueco(cola, 10);
    ejecutar(cola, 1, &l);

    let q = mapear(lectura);
    (0..64 * 64).map(|i| {
        let (x, y) = (i % 64, i / 64);
        // SAFETY: dentro del bufer de lectura (256 * 64).
        unsafe { (q.add(256 * y + 4 * x) as *const u32).read_unaligned() }
    }).collect()
}

/// *** HelloTexture por la casa: cada pixel del RT es el texel de
/// `floor(uv * 8)` en su centro (PUNTO); con la firma en 1.0 y en 1.1.
#[test]
fn hellotexture_dibuja_su_textura() {
    for version_1_1 in [false, true] {
        let img = hello_texture(version_1_1);
        let dicho = String::from_utf8_lossy(&DICHO.lock().unwrap()).into_owned();
        // Lo UNICO que dice: que ese PSO se interpreta (el JIT aun no muestrea).
        assert_eq!(dicho, "PROTON-X: un PSO con texturas: sus sombreadores se interpretan (el codigo nativo aun no muestrea)\n");
        let mut malos = 0;
        for y in 0..64u32 {
            for x in 0..64u32 {
                let esperado = texel(x / 8, y / 8);
                if img[(64 * y + x) as usize] != esperado {
                    malos += 1;
                    assert!(malos > 5, "({x}, {y}): {:08x} y D3D12 dice {esperado:08x} (firma 1.1: {version_1_1})", img[(64 * y + x) as usize]);
                }
            }
        }
        assert_eq!(malos, 0, "firma 1.1: {version_1_1}");
    }
    // P3b4c.8 T2b: su PSO, para la 3060, es UN TEX de t0 con s0 (la
    // textura 0 de la receta), con el asa que pone el kernel.
    let vistos = PARA_LA_3060.lock().unwrap();
    assert!(!vistos.is_empty(), "el PSO de HelloTexture tiene cuerpos para la 3060");
    for (texturas, juzgado, texs) in vistos.iter() {
        assert_eq!((texturas.as_slice(), *juzgado, *texs), (&[(0u8, 0u8)][..], true, 1));
    }
}
