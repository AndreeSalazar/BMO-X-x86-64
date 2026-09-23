//! **Los tres numeros de S5, medidos en el anfitrion** -- el mismo Ryzen 5
//! 5600X donde arranca BMO-X, con Windows encima.
//!
//! En BMO-X (`sys/sombra.bex`) el testigo de Rust sale con los `float` por
//! SOFTWARE (el target de Ring 3 es `+soft-float`), asi que la comparacion
//! justa con "la misma pasada escrita a mano" se hace aqui: Rust nativo en
//! release, con SSE, contra el JIT sellado (W^X con `VirtualProtect`).
//!
//!     cargo run --release -p bmo-spirv-x86-64 --example medir

#[cfg(all(windows, target_arch = "x86_64"))]
fn main() {
    use bmo_spirv_front::{read, workspace_words, Buffer, Interpreter};
    use bmo_spirv_x86_64::{emit, tables_words, IDS_WORDS};
    use core::ffi::c_void;
    use std::time::Instant;

    #[link(name = "kernel32")]
    extern "system" {
        fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
        fn VirtualProtect(addr: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    }
    type Init = extern "sysv64" fn(*mut u32, *const u64);
    type Main = extern "sysv64" fn(*mut u32, *const u64, *const u32, u64) -> u64;

    const SPV: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
    const ANCHO: u32 = 128;
    const TOPE: u32 = 64;
    let pixeles = (ANCHO * ANCHO) as usize;
    let ventana: [u32; 6] =
        [(-2.0f32).to_bits(), (-1.25f32).to_bits(), (2.5f32 / 128.0).to_bits(), (2.5f32 / 128.0).to_bits(), ANCHO, TOPE];

    // 1. Traducir.
    let t = Instant::now();
    let mut ids = vec![0u32; 4096];
    let m = read(SPV, &mut ids).unwrap();
    let mut tablas = vec![0u32; tables_words(&m)];
    let mut code = vec![0u8; 256 << 10];
    let p = emit(&m, &mut tablas, &mut code).unwrap();
    let t_traducir = t.elapsed();

    // Sellar.
    let base = unsafe {
        let b = VirtualAlloc(core::ptr::null_mut(), p.code_len, 0x3000, 0x04);
        core::ptr::copy_nonoverlapping(code.as_ptr(), b as *mut u8, p.code_len);
        let mut viejo = 0;
        VirtualProtect(b, p.code_len, 0x20, &mut viejo);
        b as usize
    };
    let init: Init = unsafe { core::mem::transmute(base + p.init) };
    let main: Main = unsafe { core::mem::transmute(base + p.main) };

    // 2. El JIT.
    let mut s_jit = vec![0u32; pixeles];
    let mut v = ventana.to_vec();
    let mut marco = vec![0u32; p.frame_words + 1];
    let mut tabla = Vec::new();
    for &(_, binding) in &p.buffers[..p.n_buffers] {
        let (d, n) = if binding == 0 { (s_jit.as_mut_ptr(), pixeles) } else { (v.as_mut_ptr(), 6) };
        tabla.push(d as u64);
        tabla.push((n * 4) as u64);
    }
    let ls = p.local_size;
    let grupos = [ANCHO / ls[0], ANCHO / ls[1], 1];
    let t = Instant::now();
    init(marco.as_mut_ptr(), tabla.as_ptr());
    for wy in 0..grupos[1] {
        for wx in 0..grupos[0] {
            for ly in 0..ls[1] {
                for lx in 0..ls[0] {
                    let mut idv = [0u32; IDS_WORDS];
                    idv[..12].copy_from_slice(&[wx * ls[0] + lx, wy * ls[1] + ly, 0, lx, ly, 0, wx, wy, 0, grupos[0], grupos[1], 1]);
                    idv[12] = ly * ls[0] + lx;
                    assert_eq!(main(marco.as_mut_ptr(), tabla.as_ptr(), idv.as_ptr(), 1 << 20) as u32, 0);
                }
            }
        }
    }
    let t_jit = t.elapsed();

    // 3. Rust a mano, nativo con SSE.
    let mut s_rust = vec![0u32; pixeles];
    let t = Instant::now();
    let f = f32::from_bits;
    let (ox, oy, px, py) = (f(ventana[0]), f(ventana[1]), f(ventana[2]), f(ventana[3]));
    for y in 0..ANCHO {
        for x in 0..ANCHO {
            let c = [ox + (x as f32) * px, oy + (y as f32) * py];
            let mut z = [0.0f32, 0.0];
            let mut k = 0u32;
            while k < TOPE && z[0] * z[0] + z[1] * z[1] <= 4.0 {
                z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
                k += 1;
            }
            s_rust[(y * ANCHO + x) as usize] = k;
        }
    }
    let t_rust = t.elapsed();
    std::hint::black_box(&s_rust);

    // 4. El oraculo.
    let mut s_oraculo = vec![0u32; pixeles];
    let mut v2 = ventana.to_vec();
    let mut ws = vec![0u32; workspace_words(&m)];
    let t = Instant::now();
    let mut it = Interpreter::new(&m, &mut ws).unwrap();
    let mut bufs = [Buffer { set: 0, binding: 0, data: &mut s_oraculo }, Buffer { set: 0, binding: 1, data: &mut v2 }];
    it.dispatch(grupos, &mut bufs, 1 << 24).unwrap();
    let t_oraculo = t.elapsed();

    let distintos = (0..pixeles).filter(|&i| s_jit[i] != s_rust[i] || s_jit[i] != s_oraculo[i]).count();
    println!("mandelbrot {}x{}, tope {} -- {} bytes de codigo emitido", ANCHO, ANCHO, TOPE, p.code_len);
    println!("  traducir (leer + juzgar + emitir)  {:>9.1} us", t_traducir.as_secs_f64() * 1e6);
    println!("  JIT sellado                        {:>9.1} us", t_jit.as_secs_f64() * 1e6);
    println!("  Rust a mano (release, SSE)         {:>9.1} us", t_rust.as_secs_f64() * 1e6);
    println!("  oraculo (interprete)               {:>9.1} us", t_oraculo.as_secs_f64() * 1e6);
    println!("  JIT / Rust = {:.2}x   oraculo / JIT = {:.1}x", t_jit.as_secs_f64() / t_rust.as_secs_f64(), t_oraculo.as_secs_f64() / t_jit.as_secs_f64());
    println!("  pixeles distintos entre los tres: {} (tiene que ser 0)", distintos);
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
fn main() {
    println!("medir: solo en Windows x86-64 (usa VirtualAlloc/VirtualProtect para sellar)");
}
