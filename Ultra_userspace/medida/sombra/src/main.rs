//! **S5 DEL SOMBREADOR: el JIT, en el Ryzen, con SELLAR.**
//!
//! Un sombreador de computo (`mandelbrot.spv`, el del banco) viaja dentro de
//! este `.bex` y se traduce AQUI, en Ring 3 de BMO-X: el lector, el juez y el
//! emisor son los mismos crates `no_std` sin `alloc` que se prueban en el
//! anfitrion. El codigo se escribe en un bloque de `KIND_MEMORIA`, se SELLA
//! (`MEM_OP_SELLAR`: R+X, sin W) y se llama.
//!
//! Y se mide contra dos testigos, los tres sobre los mismos pixeles:
//!
//! ```text
//!   JIT       lo emitido, sellado, llamado invocacion a invocacion
//!   oraculo   `bmo_spirv_front::Interpreter`, la DEFINICION
//!   Rust      el mismo calculo escrito a mano, compilado por rustc
//! ```
//!
//! Tienen que dar los MISMOS pixeles (el JIT y el oraculo bit a bit ya lo
//! dice el banco del anfitrion; esto es decirlo EN EL METAL). Y los tres
//! numeros que pide el plan: lo que tarda traducir, lo que tarda una pasada
//! del JIT, y la misma pasada escrita a mano. Sin esos numeros no se toca S7.
//!
//! Se lanza con `sys/sombra.bex`.

#![no_std]
#![no_main]

use bmo_spirv_front::{read, workspace_words, Buffer, Interpreter};
use bmo_spirv_x86_64::{emit, tables_words, trap_reason, IDS_WORDS};
use bmo_userland as bmo;

static SPV: &[u8] = include_bytes!("../../../../toolchain/lang/spirv/pruebas/mandelbrot.spv");

/// La imagen: `ANCHO x ANCHO` pixeles, `TOPE` vueltas como mucho.
const ANCHO: u32 = 128;
const TOPE: u32 = 64;
const PIXELES: usize = (ANCHO * ANCHO) as usize;
/// Combustible por invocacion (saltos hacia atras): de sobra para `TOPE`.
const FUEL: u64 = 1 << 20;

// ============================ LA SALIDA ============================

/// Una linea de consola sobre pila. Sin `alloc` no hay `format!`.
struct Linea {
    buf: [u8; 160],
    n: usize,
}

impl Linea {
    const fn nueva() -> Self {
        Self { buf: [0; 160], n: 0 }
    }
    fn soltar(&mut self) {
        if let Ok(s) = core::str::from_utf8(&self.buf[..self.n]) {
            bmo::consola(s);
        }
        self.n = 0;
    }
}

impl core::fmt::Write for Linea {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.as_bytes() {
            if self.n < self.buf.len() {
                self.buf[self.n] = *b;
                self.n += 1;
            }
        }
        Ok(())
    }
}

macro_rules! di {
    ($l:expr, $($t:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($l, $($t)*);
        $l.soltar();
    }};
}

fn fin(l: &mut Linea, motivo: &str) -> ! {
    di!(l, "SOMBRA: {}\n", motivo);
    bmo::salir();
}

/// Reparte un bloque de palabras en trozos, sin `alloc`.
struct Reparto<'a> {
    resto: &'a mut [u32],
}

impl<'a> Reparto<'a> {
    fn toma(&mut self, n: usize) -> Option<&'a mut [u32]> {
        let resto = core::mem::take(&mut self.resto);
        if resto.len() < n {
            return None;
        }
        let (a, b) = resto.split_at_mut(n);
        self.resto = b;
        a.fill(0);
        Some(a)
    }
}

/// El mismo mandelbrot, a mano, en el MISMO orden de operaciones que el GLSL.
///
/// [!] OJO a lo que mide este testigo: el target de Ring 3
/// (`x86_64-unknown-none`) compila los `float` de Rust POR SOFTWARE
/// (`__mulsf3`, `__addsf3`: `+soft-float` manda aunque se pida `sse2` con
/// `#[target_feature]`, se probo el 23-09). O sea que esto es "Rust de Ring 3
/// tal como hoy compila en BMO-X", no "Rust con SSE". La comparacion con Rust
/// CON SSE se hace en el anfitrion, en el mismo Ryzen
/// (`emisor-x86_64/examples/medir.rs`).
fn mandelbrot_rust(salida: &mut [u32], ventana: &[u32; 6]) {
    let f = f32::from_bits;
    let (ox, oy, px, py, ancho, tope) = (f(ventana[0]), f(ventana[1]), f(ventana[2]), f(ventana[3]), ventana[4], ventana[5]);
    for y in 0..ancho {
        for x in 0..ancho {
            let c = [ox + (x as f32) * px, oy + (y as f32) * py];
            let mut z = [0.0f32, 0.0];
            let mut k = 0u32;
            while k < tope && z[0] * z[0] + z[1] * z[1] <= 4.0 {
                z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
                k += 1;
            }
            salida[(y * ancho + x) as usize] = k;
        }
    }
}

type Init = extern "sysv64" fn(*mut u32, *const u64);
type Main = extern "sysv64" fn(*mut u32, *const u64, *const u32, u64) -> u64;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut l = Linea::nueva();
    di!(l, "SOMBRA: S5 -- mandelbrot.spv traducido EN BMO-X, sellado y ejecutado\n");
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let us = |ticks: u64| ticks * 1_000_000 / hz;

    // -- La memoria: dos bloques. Uno de datos; el otro, el CODIGO, se sella.
    let Some(datos) = bmo::Memoria::request(4 << 20) else { fin(&mut l, "no hay bloque de datos") };
    let Some(codigo) = bmo::Memoria::request(256 << 10) else { fin(&mut l, "no hay bloque de codigo") };
    let palabras = unsafe { core::slice::from_raw_parts_mut(datos.base() as *mut u32, (4 << 20) / 4) };
    let mut r = Reparto { resto: palabras };
    let bound = u32::from_le_bytes([SPV[12], SPV[13], SPV[14], SPV[15]]) as usize;
    let Some(ids) = r.toma(bound) else { fin(&mut l, "sin sitio para los ids") };

    // -- 1. TRADUCIR: leer, juzgar y emitir, aqui dentro.
    let t0 = bmo::ciclos();
    let m = match read(SPV, ids) {
        Ok(m) => m,
        Err(e) => {
            di!(l, "SOMBRA: el lector dice NO: {}\n", e);
            bmo::salir();
        }
    };
    let Some(tablas) = r.toma(tables_words(&m)) else { fin(&mut l, "sin sitio para las tablas") };
    let bytes_codigo = unsafe { core::slice::from_raw_parts_mut(codigo.base(), 256 << 10) };
    let p = match emit(&m, tablas, bytes_codigo) {
        Ok(p) => p,
        Err(e) => {
            di!(l, "SOMBRA: el emisor dice NO: {}\n", e);
            bmo::salir();
        }
    };
    let t_traducir = bmo::ciclos() - t0;
    di!(l, "SOMBRA: traducido: {} bytes de x86-64, marco de {} palabras, {} us\n", p.code_len, p.frame_words, us(t_traducir));

    // -- 2. SELLAR: de datos a codigo (W^X). Sin esto, llamar seria un #PF por NX.
    if let Err(motivo) = codigo.sellar() {
        di!(l, "SOMBRA: SELLAR dice NO (motivo {})\n", motivo);
        bmo::salir();
    }
    let otra = codigo.sellar();
    di!(l, "SOMBRA: sellado (R+X, sin W); sellar otra vez contesta {:?}\n", otra);

    // -- 3. Los datos, y los tres calculos.
    let ventana: [u32; 6] = [
        (-2.0f32).to_bits(),
        (-1.25f32).to_bits(),
        (2.5f32 / ANCHO as f32).to_bits(),
        (2.5f32 / ANCHO as f32).to_bits(),
        ANCHO,
        TOPE,
    ];
    let Some(v_jit) = r.toma(6) else { fin(&mut l, "sin sitio") };
    v_jit.copy_from_slice(&ventana);
    let Some(s_jit) = r.toma(PIXELES) else { fin(&mut l, "sin sitio") };
    let Some(s_oraculo) = r.toma(PIXELES) else { fin(&mut l, "sin sitio") };
    let Some(v_oraculo) = r.toma(6) else { fin(&mut l, "sin sitio") };
    v_oraculo.copy_from_slice(&ventana);
    let Some(s_rust) = r.toma(PIXELES) else { fin(&mut l, "sin sitio") };
    let Some(marco) = r.toma(p.frame_words + 1) else { fin(&mut l, "sin sitio para el marco") };

    // La tabla de buffers en el orden que dice el programa emitido.
    let mut tabla = [0u64; 2 * (bmo_spirv_x86_64::MAX_BUFFERS + 1)];
    for (k, &(_, binding)) in p.buffers[..p.n_buffers].iter().enumerate() {
        let (dir, len) = if binding == 0 { (s_jit.as_mut_ptr(), PIXELES) } else { (v_jit.as_mut_ptr(), 6) };
        tabla[2 * k] = dir as u64;
        tabla[2 * k + 1] = (len * 4) as u64;
    }

    // JIT
    let base = codigo.base() as usize;
    let init: Init = unsafe { core::mem::transmute(base + p.init) };
    let main: Main = unsafe { core::mem::transmute(base + p.main) };
    let grupos = [ANCHO / p.local_size[0], ANCHO / p.local_size[1], 1];
    let t0 = bmo::ciclos();
    init(marco.as_mut_ptr(), tabla.as_ptr());
    let ls = p.local_size;
    let mut trampa = 0u32;
    'todo: for wy in 0..grupos[1] {
        for wx in 0..grupos[0] {
            for ly in 0..ls[1] {
                for lx in 0..ls[0] {
                    let global = [wx * ls[0] + lx, wy * ls[1] + ly, 0];
                    let index = ly * ls[0] + lx;
                    let mut idv = [0u32; IDS_WORDS];
                    idv[..12].copy_from_slice(&[
                        global[0], global[1], 0, lx, ly, 0, wx, wy, 0, grupos[0], grupos[1], 1,
                    ]);
                    idv[12] = index;
                    let c = main(marco.as_mut_ptr(), tabla.as_ptr(), idv.as_ptr(), FUEL) as u32;
                    if c != 0 {
                        trampa = c;
                        break 'todo;
                    }
                }
            }
        }
    }
    let t_jit = bmo::ciclos() - t0;
    if trampa != 0 {
        di!(l, "SOMBRA: el JIT PARO: {:?}\n", trap_reason(trampa));
    }

    // Oraculo
    let t0 = bmo::ciclos();
    let Some(ws) = r.toma(workspace_words(&m)) else { fin(&mut l, "sin sitio para el oraculo") };
    let oraculo = match Interpreter::new(&m, ws) {
        Ok(mut it) => {
            let mut bufs = [
                Buffer { set: 0, binding: 0, data: s_oraculo },
                Buffer { set: 0, binding: 1, data: v_oraculo },
            ];
            it.dispatch(grupos, &mut bufs, 1 << 24).map(|_| ())
        }
        Err(_) => Ok(()),
    };
    let t_oraculo = bmo::ciclos() - t0;
    if let Err(t) = oraculo {
        di!(l, "SOMBRA: el oraculo PARO: {}\n", t.reason.name());
    }

    // Rust
    let t0 = bmo::ciclos();
    mandelbrot_rust(s_rust, &ventana);
    let t_rust = bmo::ciclos() - t0;

    // -- 4. El veredicto.
    let mut distintos_oraculo = 0usize;
    let mut distintos_rust = 0usize;
    let mut fuera = 0usize;
    for i in 0..PIXELES {
        if s_jit[i] != s_oraculo[i] {
            distintos_oraculo += 1;
        }
        if s_jit[i] != s_rust[i] {
            distintos_rust += 1;
        }
        if s_jit[i] < TOPE {
            fuera += 1;
        }
    }
    di!(l, "SOMBRA: {}x{} pixeles, {} escapan antes de {} vueltas\n", ANCHO, ANCHO, fuera, TOPE);
    di!(l, "SOMBRA: JIT contra oraculo: {} pixeles distintos (tiene que ser 0)\n", distintos_oraculo);
    di!(l, "SOMBRA: JIT contra Rust:    {} pixeles distintos (tiene que ser 0)\n", distintos_rust);
    di!(l, "SOMBRA: traducir {} us | JIT {} us | oraculo {} us | Rust (float por software) {} us\n", us(t_traducir), us(t_jit), us(t_oraculo), us(t_rust));
    if t_rust > 0 {
        di!(l, "SOMBRA: el JIT tarda {}.{:02}x lo de Rust; el oraculo {}x\n", t_jit / t_rust, (t_jit * 100 / t_rust) % 100, t_oraculo / t_rust);
    }
    di!(l, "SOMBRA: hecho\n");
    bmo::salir();
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en sombra\n");
    let mut l = Linea::nueva();
    {
        use core::fmt::Write;
        let _ = write!(l, "{}\n", info.message());
    }
    l.soltar();
    bmo::salir();
}
