//! **Lo que el BSF ahorra, medido en el anfitrion** (el mismo Ryzen 5 5600X).
//!
//! Abrir un sombreador por el JIT de S5 es leer + juzgar + emitir. Abrirlo por
//! el BSF es comprobar las capas 1 a 3 (forma, indice, disposicion) y TOMAR el
//! codigo que se va a sellar (su hash, la capa 4 de ese blob).
//! Las capas 5 (releer y re-emitir) se miden aparte: son las que paga quien
//! fabrica, no quien abre.
//!
//!     cargo run --release -p bmo-bsf-x86-64 --example medir

use std::time::Instant;

use bmo_bsf::*;
use bmo_bsf_x86_64::*;
use bmo_spirv_front::read;
use bmo_spirv_x86_64::{emit, tables_words};

const N: u32 = 2000;

fn main() {
    for (nombre, spv) in [
        ("mandelbrot", &include_bytes!("../../pruebas/mandelbrot.spv")[..]),
        ("trascendentes", &include_bytes!("../../pruebas/trascendentes.spv")[..]),
    ] {
        let mut ids = vec![0u32; 1 << 12];
        let mut tablas = vec![0u32; 1 << 13];
        let mut code = vec![0u8; 1 << 20];

        // El JIT: leer + juzgar + emitir.
        let t = Instant::now();
        for _ in 0..N {
            let m = read(spv, &mut ids).unwrap();
            let k = tables_words(&m);
            std::hint::black_box(emit(&m, &mut tablas[..k], &mut code).unwrap());
        }
        let jit = t.elapsed() / N;

        // Fabricar el BSF.
        let m = read(spv, &mut ids).unwrap();
        let h = facts(&m).unwrap();
        let k = tables_words(&m);
        let p = emit(&m, &mut tablas[..k], &mut code).unwrap();
        let mut ranuras = [0xFF; MAX_BINDINGS];
        let objetivo = [x86_64_target(&p, &code, h.bindings(), &mut ranuras).unwrap()];
        let modulo = ModuleIn {
            model: h.model,
            name: nombre.as_bytes(),
            local_size: h.local_size,
            capabilities: h.capabilities,
            caps_high: h.caps_high,
            spirv: spv,
            bindings: h.bindings(),
            targets: &objetivo,
        };
        let mut bsf = vec![0u8; size(&[modulo]).unwrap()];
        write(&[modulo], &mut bsf).unwrap();

        // Abrir por el BSF: capas 1 a 4 y elegir el objetivo.
        let t = Instant::now();
        for _ in 0..N {
            let b = Bsf::parse(&bsf).unwrap();
            let t = b.module(0).target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2).unwrap();
            std::hint::black_box(t.code().unwrap());
        }
        let abrir = t.elapsed() / N;

        let b = Bsf::parse(&bsf).unwrap();
        let t = Instant::now();
        for _ in 0..N {
            b.deep(0, &mut ids).unwrap();
        }
        let hondo = t.elapsed() / N;
        let mut code2 = vec![0u8; 1 << 20];
        let t = Instant::now();
        for _ in 0..N {
            reproducir(&b, 0, &mut ids, &mut tablas, &mut code2).unwrap();
        }
        let repro = t.elapsed() / N;

        println!("{} -- SPIR-V {} B, codigo {} B, BSF {} B", nombre, spv.len(), p.code_len, bsf.len());
        println!("  JIT (leer + juzgar + emitir)     {:>8.1} us", jit.as_secs_f64() * 1e6);
        println!("  BSF: capas 1-3 + tomar codigo    {:>8.1} us   ({:.0}x menos)", abrir.as_secs_f64() * 1e6, jit.as_secs_f64() / abrir.as_secs_f64());
        println!("  capa 5a: releer y comparar       {:>8.1} us", hondo.as_secs_f64() * 1e6);
        println!("  capa 5b: re-emitir y comparar    {:>8.1} us", repro.as_secs_f64() * 1e6);
    }
}
