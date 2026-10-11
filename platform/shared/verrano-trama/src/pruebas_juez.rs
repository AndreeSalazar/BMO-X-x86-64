//! Las pruebas del juez de los dibujos con programas (V3a, LI2a, LI2c, LI2d,
//! LI2f): eran de `verrano/src/cpu.rs` y vinieron con el juez (VC2 paso 2),
//! sin cambiar lo que comprueban -- ahora las dibuja LA trama de la casa --.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_cubo::referencia::{de_la_3060, huella, ALTO, ANCHO};
use bmo_cubo::FONDO_F;
use bmo_prometeo::programa::Programa;
use bmo_verrano::dibujo::{Buffer, Compute, Draw, DrawBackend, Pipeline, Shader};
use bmo_verrano::{Cull, Error, Image, Stats, Unorm8, Viewport};

use crate::juez::{Casa, Juez};


/// Un DXIL de dxc, como etapa de VERRANO (el Programa vive lo que la prueba).
fn programa(d: &[u8]) -> Casa<'static> {
    let p: Programa = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(d).unwrap()).unwrap();
    Casa(alloc::boxed::Box::leak(alloc::boxed::Box::new(p)))
}

fn los_dos() -> (Casa<'static>, Casa<'static>) {
    (programa(include_bytes!("../../proton-x/prueba/cubo_vs.dxil")), programa(include_bytes!("../../proton-x/prueba/cubo_ps.dxil")))
}

fn cbuffer(f: u32) -> Vec<u8> {
    let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), ANCHO as f32 / ALTO as f32);
    c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
}

fn atributos() -> Vec<[f32; 4]> {
    bmo_cubo::vertices().iter().flat_map(|v| [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color]).collect()
}

fn dibuja(vs: &dyn Shader, ps: &dyn Shader, cb: &[u8], indices: &[u32], cull: Cull) -> Result<(Vec<u32>, u32), Error> {
    let datos = atributos();
    let d = Draw {
        clear: Some(FONDO_F),
        viewport: Viewport { width: ANCHO, height: ALTO },
        pipeline: Pipeline::new(vs, ps, false, cull),
        vertices: Buffer { data: &datos, stride: 3 },
        indices: Some(indices),
        constants: cb,
        resources: bmo_verrano::dibujo::Resources::NONE,
        count: indices.len(),
    };
    let mut px = vec![0u32; (ANCHO * ALTO) as usize];
    let mut juez = Juez::NUEVO;
    let s = juez.draw_with(&d, &mut Image { pixels: &mut px, width: ANCHO, height: ALTO, depth: None })?;
    Ok((px, s.triangles))
}

fn indices() -> Vec<u32> {
    bmo_cubo::indices().iter().map(|&i| i as u32).collect()
}

#[test]
fn the_cube_of_bmox12_with_its_own_programs_gives_the_d3d12_print() {
    let (vs, ps) = los_dos();
    for f in [0, 30, 60] {
        let (px, tris) = dibuja(&vs, &ps, &cbuffer(f), &indices(), Cull::Back).unwrap();
        assert!((3..=8).contains(&tris), "fotograma {f}: {tris} triangulos de 12 miran");
        assert_eq!(Some(huella(&px)), de_la_3060(f), "fotograma {f}: la huella de D3D12");
    }
}

/// Sin descarte, las caras de detras se pintan encima (no hay
/// profundidad): la huella ya NO es la de D3D12. Y con la matriz de
/// otro fotograma, tampoco: lo que gira es la CONSTANTE.
#[test]
fn the_print_depends_on_the_cull_and_on_the_constants() {
    let (vs, ps) = los_dos();
    let (px, _) = dibuja(&vs, &ps, &cbuffer(30), &indices(), Cull::None).unwrap();
    assert_ne!(Some(huella(&px)), de_la_3060(30));
    let (px, _) = dibuja(&vs, &ps, &cbuffer(31), &indices(), Cull::Back).unwrap();
    assert_ne!(Some(huella(&px)), de_la_3060(30));
}

/// Un programa de vertice de prueba: deja su entrada 0 tal cual (la
/// posicion ya en recorte). Uno que no es el Programa de la casa:
/// cualquier etapa entra por `Shader`.
struct Paso;
impl Shader for Paso {
    fn inputs(&self) -> usize {
        1
    }
    fn outputs(&self) -> usize {
        1
    }
    fn reads(&self, k: usize) -> bool {
        k == 0
    }
    fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
        o[0] = i[0];
        true
    }
}

/// Uno de pixeles de prueba: la w de su SV_Position / 4, en rojo.
struct LaW;
impl Shader for LaW {
    fn inputs(&self) -> usize {
        1
    }
    fn outputs(&self) -> usize {
        1
    }
    fn reads(&self, k: usize) -> bool {
        k == 0
    }
    fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
        o[0] = [i[0][3] / 4.0, 0.0, 0.0, 1.0];
        true
    }
}

fn de_8x8(vs: &dyn Shader, ps: &dyn Shader, datos: &[[f32; 4]], indices: &[u32]) -> Vec<u32> {
    let d = Draw {
        clear: Some([0.0; 4]),
        viewport: Viewport { width: 8, height: 8 },
        pipeline: Pipeline::new(vs, ps, false, Cull::None),
        vertices: Buffer { data: datos, stride: 1 },
        indices: Some(indices),
        constants: &[],
        resources: bmo_verrano::dibujo::Resources::NONE,
        count: indices.len(),
    };
    let mut px = vec![0u32; 64];
    let mut juez = Juez::NUEVO;
    juez.draw_with(&d, &mut Image { pixels: &mut px, width: 8, height: 8, depth: None }).unwrap();
    px
}

/// *** Cyberpunk lee SV_Position en el de pixeles (su G-buffer, su
/// ruido): era lo primero que la 3060 le nego. `posicion.hlsl` de la
/// casa (dxc) en un destino de 8x8: cada pixel ve SU centro, la z del
/// viewport y la w de recorte -- lo mismo que la prueba N5.9 de
/// `bmo_proton_x` --.
#[test]
fn the_pixel_program_reads_its_sv_position() {
    let ps = programa(include_bytes!("../../proton-x/prueba/posicion.dxil"));
    assert!(ps.reads(0), "posicion.hlsl lee su SV_Position");
    let (z, w) = (0.5f32, 2.0f32);
    let cuadro: Vec<[f32; 4]> = [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|&(x, y)| [x * w, y * w, z * w, w]).collect();
    let px = de_8x8(&Paso, &ps, &cuadro, &[0, 1, 2, 2, 1, 3]);
    for y in 0..8 {
        for x in 0..8 {
            let esperado = Unorm8::Exact.pack([(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.5, 0.5]);
            assert_eq!(px[y * 8 + x], esperado, "pixel ({x}, {y})");
        }
    }
}

/// La w va con PERSPECTIVA: con w distinta por vertice, la del centro
/// de un pixel es la inversa de la media de las 1/w (el triangulo de
/// la prueba de la casa, `la_w_de_sv_position_va_con_perspectiva`).
#[test]
fn the_w_of_sv_position_goes_with_perspective() {
    let tri = [[-1.0, 1.0, 0.0, 1.0], [12.0, 4.0, 0.0, 4.0], [-4.0, -12.0, 0.0, 4.0]];
    let px = de_8x8(&Paso, &LaW, &tri, &[0, 1, 2]);
    let pesos = |x: f32, y: f32| {
        let (b1, b2) = (x / 16.0, y / 16.0);
        1.0 / ((1.0 - b1 - b2) / 1.0 + b1 / 4.0 + b2 / 4.0)
    };
    for (x, y) in [(0usize, 0usize), (7, 7), (3, 5)] {
        let esperado = Unorm8::Exact.pack([pesos(x as f32 + 0.5, y as f32 + 0.5) / 4.0, 0.0, 0.0, 1.0]);
        assert_eq!(px[y * 8 + x], esperado, "pixel ({x}, {y})");
    }
    // y no es plana: la esquina de w = 1 y la del fondo, distintas
    assert_ne!(px[0], px[63]);
}

/// Uno de pixeles de prueba: los mismos colores siempre (uno por
/// render target).
struct Fijo([[f32; 4]; 3]);
impl Shader for Fijo {
    fn inputs(&self) -> usize {
        0
    }
    fn outputs(&self) -> usize {
        3
    }
    fn reads(&self, _: usize) -> bool {
        false
    }
    fn run(&self, _: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
        o.copy_from_slice(&self.0[..o.len()]);
        true
    }
}

/// El cuadro que llena 8x8, en recorte (w = 1).
fn lleno() -> Vec<[f32; 4]> {
    [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|&(x, y)| [x, y, 0.5, 1.0]).collect()
}

/// Un dibujo del cuadro en 8x8 sobre lo que ya hay en `px` (y en
/// `otros`), con `p` como tuberia.
fn encima(p: Pipeline, clear: Option<[f32; 4]>, px: &mut [u32], otros: &mut [&mut [u32]]) -> Result<Stats, Error> {
    let datos = lleno();
    let d = Draw { clear, viewport: Viewport { width: 8, height: 8 }, pipeline: p, vertices: Buffer { data: &datos, stride: 1 }, indices: Some(&[0, 1, 2, 2, 1, 3]), constants: &[], resources: bmo_verrano::dibujo::Resources::NONE, count: 6 };
    let mut juez = Juez::NUEVO;
    juez.draw_targets(&d, &mut Image { pixels: px, width: 8, height: 8, depth: None }, otros)
}

/// *** LA MEZCLA ES LA DE LA CASA (N5.11): desde VC2 no hay otra -- la
/// cuenta es la de `mezcla.rs` --; lo que se prueba es la TRADUCCION: cada
/// factor y cada operacion de VERRANO al numero de D3D12 que lleva la
/// mezcla, y que dibujar con una mezcla da lo que dice esa cuenta.
#[test]
fn blending_is_the_house_blending() {
    use crate::mezcla as m;
    use bmo_verrano::dibujo::{Blend, BlendOp, Factor};
    let factores = [
        (Factor::Zero, m::CERO),
        (Factor::One, m::UNO),
        (Factor::SrcColor, m::ORIGEN_COLOR),
        (Factor::InvSrcColor, m::INV_ORIGEN_COLOR),
        (Factor::SrcAlpha, m::ORIGEN_ALFA),
        (Factor::InvSrcAlpha, m::INV_ORIGEN_ALFA),
        (Factor::DestAlpha, m::DESTINO_ALFA),
        (Factor::InvDestAlpha, m::INV_DESTINO_ALFA),
        (Factor::DestColor, m::DESTINO_COLOR),
        (Factor::InvDestColor, m::INV_DESTINO_COLOR),
        (Factor::SrcAlphaSat, m::ORIGEN_ALFA_SAT),
        (Factor::BlendFactor, m::FACTOR),
        (Factor::InvBlendFactor, m::INV_FACTOR),
    ];
    for (f, n) in factores {
        assert_eq!(crate::juez::factor(f), n, "{f:?}");
    }
    for (o, n) in [(BlendOp::Add, m::SUMAR), (BlendOp::Subtract, m::RESTAR), (BlendOp::RevSubtract, m::RESTAR_AL_REVES), (BlendOp::Min, m::MIN), (BlendOp::Max, m::MAX)] {
        assert_eq!(crate::juez::operacion(o), n, "{o:?}");
    }
    // y dibujado: rojo a 0,8 con alfa 0,4 sobre un fondo de 0,2 (SrcAlpha,
    // InvSrcAlpha): 0,8 * 0,4 + d * 0,6, d el fondo guardado a 8 bits
    let mut px = vec![0u32; 64];
    encima(Pipeline::new(&Paso, &Fijo([[0.2, 0.2, 0.2, 1.0]; 3]), false, Cull::None), Some([0.0; 4]), &mut px, &mut []).unwrap();
    let mut p = Pipeline::new(&Paso, &Fijo([[0.8, 0.0, 0.0, 0.4]; 3]), false, Cull::None);
    p.blend[0] = Blend::ALPHA;
    encima(p, None, &mut px, &mut []).unwrap();
    let d = Unorm8::Exact.convert(0.2) as f32 / 255.0;
    let casa = m::Mezcla { encendida: true, origen: m::ORIGEN_ALFA, destino: m::INV_ORIGEN_ALFA, op: m::SUMAR, origen_a: m::UNO, destino_a: m::INV_ORIGEN_ALFA, op_a: m::SUMAR, mascara: 0xF }.aplicar([0.8, 0.0, 0.0, 0.4], [d, d, d, 1.0], [1.0; 4]);
    assert_eq!(px[9], Unorm8::Exact.pack(casa));
}

/// ** LA LUZ QUE SE SUMA: una pasada que limpia y pinta 0,25, y otra
/// SIN limpiar que suma 0,5 encima (One, One): lo guardado mas 0,5.
/// Y la transparencia (SrcAlpha, InvSrcAlpha) de rojo a la mitad sobre
/// azul; y la mascara, que deja lo que no esta.
#[test]
fn light_adds_up_over_two_passes() {
    use bmo_verrano::dibujo::Blend;
    let mut px = vec![0u32; 64];
    let base = Fijo([[0.25, 0.25, 0.25, 1.0]; 3]);
    encima(Pipeline::new(&Paso, &base, false, Cull::None), Some([0.0, 0.0, 0.0, 1.0]), &mut px, &mut []).unwrap();
    let luz = Fijo([[0.5, 0.5, 0.5, 0.0]; 3]);
    let mut p = Pipeline::new(&Paso, &luz, false, Cull::None);
    p.blend[0] = Blend::ADD;
    encima(p, None, &mut px, &mut []).unwrap();
    // el destino es de 8 bits: guardo 0,25 como 64/255, y sobre ESO se
    // suma (lo de D3D con un render target UNORM8): 192, no 191
    let guardado = Unorm8::Exact.convert(0.25) as f32 / 255.0;
    let esperado = Unorm8::Exact.pack([guardado + 0.5, guardado + 0.5, guardado + 0.5, 1.0]);
    assert_eq!(esperado & 0xFF, 192);
    assert!(px.iter().all(|&c| c == esperado), "{:#x} contra {esperado:#x}", px[0]);

    let mut px = vec![0u32; 64];
    encima(Pipeline::new(&Paso, &Fijo([[0.0, 0.0, 1.0, 1.0]; 3]), false, Cull::None), Some([0.0; 4]), &mut px, &mut []).unwrap();
    let mut p = Pipeline::new(&Paso, &Fijo([[1.0, 0.0, 0.0, 0.5]; 3]), false, Cull::None);
    p.blend[0] = Blend::ALPHA;
    encima(p, None, &mut px, &mut []).unwrap();
    assert_eq!(px[27], Unorm8::Exact.pack([0.5, 0.0, 0.5, 1.0]), "rojo a la mitad sobre azul");

    let mut p = Pipeline::new(&Paso, &Fijo([[1.0, 1.0, 1.0, 1.0]; 3]), false, Cull::None);
    p.blend[0] = Blend { write_mask: 0b0001, ..Blend::NONE };
    encima(p, None, &mut px, &mut []).unwrap();
    assert_eq!(px[27], Unorm8::Exact.pack([1.0, 0.0, 0.5, 1.0]), "solo el rojo cambia");
}

/// *** EL G-BUFFER: `gbuffer.hlsl` de la casa (dxc) escribe tres
/// render targets a la vez -- el albedo, el SITIO (de su SV_Position)
/// y la normal --, como Cyberpunk antes de su luz. Cada salida en el
/// suyo, pixel a pixel.
#[test]
fn the_gbuffer_writes_three_targets_at_once() {
    let ps = programa(include_bytes!("../../proton-x/prueba/gbuffer.dxil"));
    assert_eq!((ps.outputs(), ps.reads(0)), (3, true));
    // uno de vertice que deja la posicion y un TEXCOORD a cero
    struct DosSalidas;
    impl Shader for DosSalidas {
        fn inputs(&self) -> usize {
            1
        }
        fn outputs(&self) -> usize {
            2
        }
        fn reads(&self, k: usize) -> bool {
            k == 0
        }
        fn run(&self, i: &[[f32; 4]], _: &[u8], o: &mut [[f32; 4]]) -> bool {
            o[0] = i[0];
            o[1] = [0.0; 4];
            true
        }
    }
    let (mut a, mut b, mut c) = (vec![0u32; 64], vec![0u32; 64], vec![0u32; 64]);
    let mut p = Pipeline::new(&DosSalidas, &ps, false, Cull::None);
    p.targets = 3;
    encima(p, Some([0.0; 4]), &mut a, &mut [&mut b, &mut c]).unwrap();
    let u = Unorm8::Exact;
    for y in 0..8 {
        for x in 0..8 {
            let i = y * 8 + x;
            assert_eq!(a[i], u.pack([1.0, 0.0, 0.0, 1.0]), "albedo ({x}, {y})");
            assert_eq!(b[i], u.pack([(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.0, 1.0]), "sitio ({x}, {y})");
            assert_eq!(c[i], u.pack([0.0, 0.0, 1.0, 0.5]), "normal ({x}, {y})");
        }
    }
    // mas render targets que salidas, o sin su imagen: se dice
    let mut p = Pipeline::new(&DosSalidas, &ps, false, Cull::None);
    p.targets = 4;
    assert_eq!(encima(p, None, &mut a, &mut [&mut b, &mut c]).err(), Some(Error::Shader));
    p.targets = 3;
    assert_eq!(encima(p, None, &mut a, &mut [&mut b]).err(), Some(Error::Image));
}

fn computo(d: &[u8]) -> Casa<'static> {
    let p: Programa = bmo_proton_x::dxil::computo::preparar(d).unwrap().programa;
    Casa(alloc::boxed::Box::leak(alloc::boxed::Box::new(p)))
}

fn f32s(b: &[u8], k: usize) -> [f32; 4] {
    core::array::from_fn(|c| f32::from_le_bytes(b[16 * k + 4 * c..16 * k + 4 * c + 4].try_into().unwrap()))
}

fn u32s(b: &[u8], k: usize) -> [u32; 4] {
    core::array::from_fn(|c| u32::from_le_bytes(b[16 * k + 4 * c..16 * k + 4 * c + 4].try_into().unwrap()))
}

/// *** EL COMPUTO POR VERRANO: `computo.hlsl` de la casa (dxc), dos
/// grupos de 64 hilos. Cada hilo deja su elemento en la memoria
/// COMPARTIDA, espera a los demas (la barrera) y escribe el de su
/// ESPEJO por la escala, con su grupo y su indice en la w: sin la
/// barrera o sin memoria compartida de verdad, sale otra cosa.
#[test]
fn compute_runs_with_shared_memory_and_a_barrier() {
    use bmo_verrano::dibujo::{ComputeBackend, Dispatch, ReadBuffer, RwBuffer};
    let cs = computo(include_bytes!("../../proton-x/prueba/computo.dxil"));
    assert_eq!(cs.threads(), [64, 1, 1]);
    let entrada: Vec<u8> = (0..128u32).flat_map(|k| [k as f32, 0.5, -1.0, 2.0]).flat_map(f32::to_le_bytes).collect();
    let mut salida = vec![0u8; 128 * 16];
    let mut cb = [0u8; 16];
    cb[..4].copy_from_slice(&128u32.to_le_bytes());
    cb[4..8].copy_from_slice(&3.0f32.to_le_bytes());
    let d = Dispatch { program: &cs, groups: [2, 1, 1], constants: &cb };
    let mut juez = Juez::NUEVO;
    let hilos = juez.dispatch(&d, &[ReadBuffer { bytes: &entrada, stride: 16 }], &mut [RwBuffer { bytes: &mut salida, stride: 16 }]).unwrap();
    assert_eq!(hilos, 128);
    for k in 0..128usize {
        let (g, gi) = (k / 64, k % 64);
        let espejo = (64 * g + 63 - gi) as f32;
        assert_eq!(f32s(&salida, k), [espejo * 3.0, 1.5, -3.0, (g * 1000 + gi) as f32], "hilo {k}");
    }
}

/// Lo que pidieron dos CS de Cyberpunk en el metal (06-10): los
/// `Interlocked*` sobre la memoria COMPARTIDA y `mad` de enteros
/// (`compartido.hlsl`). Lo que se juzga no depende del orden de los
/// hilos: lo que quedo, que los "antes" de la suma son distintos, y
/// que UNO solo vio el cambio sin hacer.
#[test]
fn compute_atomics_on_shared_memory() {
    use bmo_verrano::dibujo::{ComputeBackend, Dispatch, RwBuffer};
    let cs = computo(include_bytes!("../../proton-x/prueba/compartido.dxil"));
    let mut salida = vec![0u8; 65 * 16];
    let d = Dispatch { program: &cs, groups: [1, 1, 1], constants: &[] };
    let mut juez = Juez::NUEVO;
    juez.dispatch(&d, &[], &mut [RwBuffer { bytes: &mut salida, stride: 16 }]).unwrap();
    // w = 5 gi + 1; v = 200 - 3 gi
    let suma: u32 = (0..64).map(|gi| 5 * gi + 1).sum();
    assert_eq!(u32s(&salida, 64), [suma, 200 - 3 * 63, 33, u32::MAX], "la suma, el menor, el mayor y los bits");
    let mut antes: Vec<u32> = (0..64).map(|k| u32s(&salida, k)[2]).collect();
    antes.sort();
    antes.dedup();
    assert_eq!(antes.len(), 64, "cada hilo vio la suma de ANTES, distinta");
    assert_eq!((0..64).filter(|&k| u32s(&salida, k)[3] == 1).count(), 1, "uno solo vio el contador sin cambiar");
}

/// Los techos de D3D12 se dicen: sin grupos, o mas de 65535 por eje.
#[test]
fn a_dispatch_out_of_the_limits_is_refused() {
    use bmo_verrano::dibujo::{ComputeBackend, Dispatch};
    let cs = computo(include_bytes!("../../proton-x/prueba/compartido.dxil"));
    let mut juez = Juez::NUEVO;
    for groups in [[0, 1, 1], [65536, 1, 1], [1, 0, 1]] {
        assert_eq!(juez.dispatch(&Dispatch { program: &cs, groups, constants: &[] }, &[], &mut []).err(), Some(Error::Groups), "{groups:?}");
    }
}

/// *** LA TEXTURA, EN PERSPECTIVA: `textura.hlsl` de la casa (dxc,
/// `imagen.Sample(muestreo, uv)`) en un cuadro de 16x16 cuyos dos
/// vertices de arriba estan TRES veces mas lejos (w = 3): la uv va
/// con perspectiva (no lineal en la pantalla), la textura de 4x4 se
/// filtra y se repite. VERRANO da, pixel a pixel, lo MISMO que la casa
/// (`bmo_proton_x::lote::en_cpu`, el camino de D3D12 en la CPU) con
/// los mismos datos -- con filtro lineal y repetir, y con punto y
/// borde --.
#[test]
fn a_textured_quad_in_perspective_is_the_house_one() {
    use bmo_verrano::dibujo::{Address, Filter, Resources, Sampler, Texture};
    use bmo_proton_x::lote::{self, ElementoIa, Lote, Topologia};
    use bmo_proton_x::{textura as tx, trama};
    let (vs_d, ps_d): (&[u8], &[u8]) = (include_bytes!("../../proton-x/prueba/textura_vs.dxil"), include_bytes!("../../proton-x/prueba/textura_ps.dxil"));
    let texeles: Vec<u32> = (0..16u32).map(|i| 0xFF00_0000 | (i * 16) | (255 - i * 16) << 8 | (i % 4 * 60) << 16).collect();
    // abajo w = 1, arriba w = 3; la uv llega a 2 (la textura, dos veces)
    let esquinas = [(-1.0f32, -1.0f32, 1.0f32, 0.0f32, 2.0f32), (1.0, -1.0, 1.0, 2.0, 2.0), (-1.0, 1.0, 3.0, 0.0, 0.0), (1.0, 1.0, 3.0, 2.0, 0.0)];
    let datos: Vec<[f32; 4]> = esquinas.iter().flat_map(|&(x, y, w, u, v)| [[x * w, y * w, 0.25 * w, w], [u, v, 0.0, 0.0]]).collect();
    let bytes: Vec<u8> = datos.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
    let ids = [0u32, 2, 1, 1, 2, 3];
    let (vs, ps) = (programa(vs_d), programa(ps_d));
    for (filter, address) in [(Filter::Linear, Address::Wrap), (Filter::Point, Address::Border)] {
        // VERRANO
        let textura = [Texture { texels: &texeles, width: 4, height: 4, bgra: false }];
        let muestreo = [Sampler { filter, address, border: [0.25, 0.5, 0.75, 1.0] }];
        let d = Draw {
            clear: Some([0.0; 4]),
            viewport: Viewport { width: 16, height: 16 },
            pipeline: Pipeline::new(&vs, &ps, false, Cull::None),
            vertices: Buffer { data: &datos, stride: 2 },
            indices: Some(&ids),
            constants: &[],
            resources: Resources { textures: &textura, samplers: &muestreo },
            count: 6,
        };
        let mut px = vec![0u32; 256];
        let mut juez = Juez::NUEVO;
        juez.draw_with(&d, &mut Image { pixels: &mut px, width: 16, height: 16, depth: None }).unwrap();
        // la casa
        let ia = [
            ElementoIa { semantica: "POSITION".into(), indice: 0, formato: 2, ranura: 0, desde: 0, por_instancia: None },
            ElementoIa { semantica: "TEXCOORD".into(), indice: 0, formato: 2, ranura: 0, desde: 16, por_instancia: None },
        ];
        let en = lote::enlazar(&bmo_proton_x::dxil::leer(vs_d).unwrap(), &bmo_proton_x::dxil::leer(ps_d).unwrap(), &ia).unwrap();
        let filtro = if filter == Filter::Linear { tx::Filtro::Lineal } else { tx::Filtro::Punto };
        let dir = if address == Address::Wrap { tx::Direccion::Repetir } else { tx::Direccion::Borde };
        let t = [Some(tx::Textura::rgba(&texeles, 4, 4, false))];
        let m = [Some(tx::Muestreador { filtro, u: dir, v: dir, borde: muestreo[0].border, comparacion: 0, lod: tx::Lod { mip: filtro, ..tx::Lod::DE_SIEMPRE } })];
        let reglas = trama::Reglas { viewport: [0.0, 0.0, 16.0, 16.0, 0.0, 1.0], tijera: [0, 0, 16, 16], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
        let l = Lote { enlace: &en, entradas: &ia, vertices: &bytes, paso: 32, ids: &ids, topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: tx::Recursos { texturas: &t, muestreadores: &m, buferes: &[], dinamicas: None }, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
        let mut casa = vec![0u32; 256];
        let mut dst = trama::Destino { pixeles: &mut casa, ancho: 16, alto: 16, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
        lote::en_cpu(&l, &mut dst).unwrap();
        // la casa guarda R en el byte bajo; VERRANO, B
        let a_verrano = |p: u32| p & 0xFF00_FF00 | (p & 0xFF) << 16 | (p >> 16) & 0xFF;
        let distintos = (0..256).filter(|&i| px[i] != a_verrano(casa[i])).count();
        assert_eq!(distintos, 0, "{filter:?} {address:?}: el primero en {:?}", (0..256).find(|&i| px[i] != a_verrano(casa[i])));
        // y no es plano: la textura se ve (mas de 8 colores)
        let mut colores = px.clone();
        colores.sort();
        colores.dedup();
        assert!(colores.len() > 8, "{filter:?}: {} colores", colores.len());
    }
}

/// Lo que no encaja se dice: un indice fuera, una de pixeles que lee
/// mas de lo que deja la de vertice.
#[test]
fn what_does_not_fit_is_refused() {
    let (vs, ps) = los_dos();
    let mut fuera = indices();
    fuera[5] = 24;
    assert_eq!(dibuja(&vs, &ps, &cbuffer(0), &fuera, Cull::Back).err(), Some(Error::Vertices));
    // la de pixeles como de vertice: lee 3, y la de pixeles deja 1
    assert_eq!(dibuja(&ps, &ps, &cbuffer(0), &indices(), Cull::Back).err(), Some(Error::Shader));
}
