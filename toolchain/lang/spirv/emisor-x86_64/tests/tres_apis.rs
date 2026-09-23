//! **LAS TRES PUERTAS: Vulkan, OpenGL y DirectX llegan al MISMO x86-64.**
//!
//! El mismo sombreador escrito dos veces --GLSL (`pruebas/*.comp`) y HLSL
//! (`pruebas/hlsl/*.hlsl`)-- y compilado tres: `glslc` para Vulkan, `glslc
//! --target-env=opengl` para OpenGL, y `dxc -spirv` (el compilador de
//! Microsoft) para DirectX. Los tres dan SPIR-V, y desde ahi el camino es UNO:
//! lector, juez, oraculo, emisor.
//!
//! Cada version se ejecuta en el oraculo y emitida en el emulador (tienen que
//! coincidir bit a bit, como en `diferencial.rs`), y ademas **las tres tienen
//! que dar los mismos buffers entre si**: es la prueba de que un juego de
//! DirectX y uno de Vulkan, con el mismo sombreador, calculan lo mismo en
//! BMO-X.

mod comun;

use comun::*;

const VK_SUMA: &[u8] = include_bytes!("../../pruebas/suma.spv");
const GL_SUMA: &[u8] = include_bytes!("../../pruebas/opengl/suma.spv");
const DX_SUMA: &[u8] = include_bytes!("../../pruebas/hlsl/suma.spv");
const VK_SAXPY: &[u8] = include_bytes!("../../pruebas/saxpy.spv");
const GL_SAXPY: &[u8] = include_bytes!("../../pruebas/opengl/saxpy.spv");
const DX_SAXPY: &[u8] = include_bytes!("../../pruebas/hlsl/saxpy.spv");
const VK_MANDELBROT: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
const GL_MANDELBROT: &[u8] = include_bytes!("../../pruebas/opengl/mandelbrot.spv");
const DX_MANDELBROT: &[u8] = include_bytes!("../../pruebas/hlsl/mandelbrot.spv");
const VK_TRASC: &[u8] = include_bytes!("../../pruebas/trascendentes.spv");
const GL_TRASC: &[u8] = include_bytes!("../../pruebas/opengl/trascendentes.spv");
const DX_TRASC: &[u8] = include_bytes!("../../pruebas/hlsl/trascendentes.spv");

/// Las tres puertas con los mismos datos: cada una igual a su oraculo, y las
/// tres iguales entre si.
fn tres(nombre: &str, spvs: [(&str, &[u8]); 3], groups: [u32; 3], datos: Vec<Vec<u32>>) {
    let mut salidas = Vec::new();
    for (api, spv) in spvs {
        let (r, buffers) = diferencial_con(spv, groups, datos.clone(), 1_000_000);
        assert_eq!(r, Ok(()), "{} por {}", nombre, api);
        salidas.push((api, buffers));
    }
    let (base, primero) = &salidas[0];
    // Que no pase en vacio: el sombreador tiene que haber ESCRITO algo.
    assert!(primero != &datos, "{}: ningun buffer cambio", nombre);
    for (api, otro) in &salidas[1..] {
        for (k, (x, y)) in primero.iter().zip(otro).enumerate() {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                assert_eq!(p, q, "{}: {} y {} difieren en el buffer {} palabra {}", nombre, base, api, k, i);
            }
        }
    }
}

#[test]
fn suma_por_las_tres_puertas() {
    let a: Vec<f32> = (0..128).map(|i| i as f32 * 0.37 - 20.0).collect();
    let b: Vec<f32> = (0..128).map(|i| 1.0 / (i as f32 + 0.5)).collect();
    tres(
        "suma",
        [("Vulkan", VK_SUMA), ("OpenGL", GL_SUMA), ("DirectX", DX_SUMA)],
        [2, 1, 1],
        vec![bits(&a), bits(&b), vec![0; 128]],
    );
}

#[test]
fn saxpy_por_las_tres_puertas() {
    let x: Vec<f32> = (0..128).map(|i| (i as f32).sqrt()).collect();
    let y: Vec<f32> = (0..128).map(|i| 100.0 - i as f32).collect();
    tres(
        "saxpy",
        [("Vulkan", VK_SAXPY), ("OpenGL", GL_SAXPY), ("DirectX", DX_SAXPY)],
        [2, 1, 1],
        vec![vec![1.5f32.to_bits(), 100], bits(&x), bits(&y)],
    );
}

#[test]
fn mandelbrot_por_las_tres_puertas() {
    let ventana = vec![(-2.0f32).to_bits(), (-1.25f32).to_bits(), (2.5f32 / 16.0).to_bits(), (2.5f32 / 16.0).to_bits(), 16, 24];
    tres(
        "mandelbrot",
        [("Vulkan", VK_MANDELBROT), ("OpenGL", GL_MANDELBROT), ("DirectX", DX_MANDELBROT)],
        [2, 2, 1],
        vec![vec![0; 256], ventana],
    );
}

#[test]
fn trascendentes_por_las_tres_puertas() {
    let v: Vec<f32> = (0..64).map(|i| (i as f32 - 20.0) * 0.731).chain([f32::NAN, 1e20, -0.0, 3e38]).collect();
    let w: Vec<f32> = (0..68).map(|i| (i % 7) as f32 * 0.5 - 1.0).collect();
    let mut v = v;
    v.resize(128, 2.5);
    let mut w = w;
    w.resize(128, 1.5);
    tres(
        "trascendentes",
        [("Vulkan", VK_TRASC), ("OpenGL", GL_TRASC), ("DirectX", DX_TRASC)],
        [2, 1, 1],
        vec![bits(&v), bits(&w), vec![0; 5 * 128]],
    );
}
