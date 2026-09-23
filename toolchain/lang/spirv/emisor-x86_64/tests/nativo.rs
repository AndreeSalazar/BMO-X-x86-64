//! **LO EMITIDO EN EL SILICIO DE VERDAD**: el Ryzen del anfitrion, sin emulador.
//!
//! El emulador de `bmo-lower` juzga los bytes, pero es un modelo: el 23-09 se
//! le cazo calculando las banderas a 64 bits. Asi que aqui el codigo emitido se
//! ejecuta en el procesador DE VERDAD --el mismo Ryzen 5 5600X donde arranca
//! BMO-X, con Windows encima-- y se compara con el oraculo bit a bit.
//!
//! Y con el mismo patron W^X que S5 usara en BMO-X: la memoria se pide de
//! lectura y escritura, se escribe el codigo, se pasa a lectura y EJECUCION
//! (sin escritura) y solo entonces se llama. En Windows es `VirtualAlloc` +
//! `VirtualProtect`; en BMO-X sera `MEM_OP_SELLAR`.
//!
//! La llamada es `extern "sysv64"`: el convenio del emisor (rdi, rsi, rdx, rcx;
//! xmm6/xmm7 los toca, y en el convenio de Windows serian del que llama). Rust
//! sabe llamar con el de System V tambien en Windows. Devuelve `u128`: en
//! System V eso es rdx:rax, o sea el codigo de trampa Y la palabra.

#![cfg(all(windows, target_arch = "x86_64"))]

mod comun;

use core::ffi::c_void;

use bmo_spirv_front::Reason;
use bmo_spirv_x86_64::{trap_reason, IDS_WORDS};
use comun::*;

const MEM_COMMIT_RESERVE: u32 = 0x3000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_EXECUTE_READ: u32 = 0x20;

#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    fn VirtualProtect(addr: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    fn VirtualFree(addr: *mut c_void, size: usize, kind: u32) -> i32;
    fn FlushInstructionCache(process: *mut c_void, addr: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

type Init = extern "sysv64" fn(*mut u32, *const u64);
type Main = extern "sysv64" fn(*mut u32, *const u64, *const u32, u64) -> u128;

/// El codigo, en una pagina que primero se escribe y despues SOLO se ejecuta.
struct Sellado {
    base: *mut c_void,
    len: usize,
}

impl Sellado {
    fn new(code: &[u8]) -> Self {
        unsafe {
            let base = VirtualAlloc(core::ptr::null_mut(), code.len(), MEM_COMMIT_RESERVE, PAGE_READWRITE);
            assert!(!base.is_null(), "VirtualAlloc");
            core::ptr::copy_nonoverlapping(code.as_ptr(), base as *mut u8, code.len());
            let mut viejo = 0u32;
            // W^X: desde aqui se ejecuta y NO se escribe. Es SELLAR.
            assert!(VirtualProtect(base, code.len(), PAGE_EXECUTE_READ, &mut viejo) != 0, "VirtualProtect");
            FlushInstructionCache(GetCurrentProcess(), base, code.len());
            Sellado { base, len: code.len() }
        }
    }

    fn at<T>(&self, off: usize) -> T {
        assert!(off < self.len);
        unsafe { core::mem::transmute_copy(&(self.base as usize + off)) }
    }
}

impl Drop for Sellado {
    fn drop(&mut self) {
        unsafe {
            VirtualFree(self.base, 0, MEM_RELEASE);
        }
    }
}

/// Lo emitido, en el silicio. Mismo contrato que `comun::emulado`.
fn nativo(spv: &[u8], groups: [u32; 3], datos: &mut [Vec<u32>], fuel: u64) -> Salida {
    let (p, code) = emitir(spv);
    let codigo = Sellado::new(&code);
    let init: Init = codigo.at(p.init);
    let main: Main = codigo.at(p.main);
    let mut marco = vec![0u32; p.frame_words + 1];
    let mut tabla = Vec::new();
    for &(_, binding) in &p.buffers[..p.n_buffers] {
        let d = &mut datos[binding as usize];
        tabla.push(d.as_mut_ptr() as u64);
        tabla.push((d.len() * 4) as u64);
    }
    tabla.extend_from_slice(&[0, 0]);
    init(marco.as_mut_ptr(), tabla.as_ptr());
    let ls = p.local_size;
    for wz in 0..groups[2] {
        for wy in 0..groups[1] {
            for wx in 0..groups[0] {
                for lz in 0..ls[2] {
                    for ly in 0..ls[1] {
                        for lx in 0..ls[0] {
                            let global = [wx * ls[0] + lx, wy * ls[1] + ly, wz * ls[2] + lz];
                            let index = lz * ls[0] * ls[1] + ly * ls[0] + lx;
                            let mut ids = [0u32; IDS_WORDS];
                            ids[..12].copy_from_slice(&[global, [lx, ly, lz], [wx, wy, wz], groups].concat());
                            ids[12] = index;
                            let r = main(marco.as_mut_ptr(), tabla.as_ptr(), ids.as_ptr(), fuel);
                            let codigo = r as u32;
                            if codigo != 0 {
                                let reason = trap_reason(codigo).unwrap_or_else(|| panic!("codigo de trampa {}", codigo));
                                return Err((reason, global));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// El oraculo contra el silicio: mismos buffers, misma parada.
fn contra_el_silicio(spv: &[u8], groups: [u32; 3], datos: Vec<Vec<u32>>) -> Salida {
    let mut a = datos.clone();
    let mut b = datos;
    let ra = oraculo(spv, groups, &mut a, 1_000_000);
    let rb = nativo(spv, groups, &mut b, 1_000_000);
    assert_eq!(ra, rb, "el oraculo y el silicio paran distinto");
    for (k, (x, y)) in a.iter().zip(&b).enumerate() {
        for (i, (p, q)) in x.iter().zip(y).enumerate() {
            assert_eq!(p, q, "buffer {} palabra {}: oraculo 0x{:08x}, silicio 0x{:08x}", k, i, p, q);
        }
    }
    ra
}

const SUMA: &[u8] = include_bytes!("../../pruebas/suma.spv");
const MANDELBROT: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
const COLORES: &[u8] = include_bytes!("../../pruebas/colores.spv");
const COLLATZ: &[u8] = include_bytes!("../../pruebas/collatz.spv");
const DIVISION: &[u8] = include_bytes!("../../pruebas/division.spv");
const TRASCENDENTES: &[u8] = include_bytes!("../../pruebas/trascendentes.spv");
const DX_MANDELBROT: &[u8] = include_bytes!("../../pruebas/hlsl/mandelbrot.spv");

#[test]
fn suma_en_el_silicio() {
    let a: Vec<f32> = (0..256).map(|i| i as f32 * 0.37 - 20.0).collect();
    let b: Vec<f32> = (0..256).map(|i| 1.0 / (i as f32 + 0.5)).collect();
    contra_el_silicio(SUMA, [4, 1, 1], vec![bits(&a), bits(&b), vec![0; 256]]).unwrap();
}

#[test]
fn mandelbrot_en_el_silicio_por_vulkan_y_por_directx() {
    // 64x64 pixeles y 64 vueltas: aqui ya no es el emulador, va rapido.
    let ventana = vec![(-2.0f32).to_bits(), (-1.25f32).to_bits(), (2.5f32 / 64.0).to_bits(), (2.5f32 / 64.0).to_bits(), 64, 64];
    for spv in [MANDELBROT, DX_MANDELBROT] {
        contra_el_silicio(spv, [8, 8, 1], vec![vec![0; 64 * 64], ventana.clone()]).unwrap();
    }
}

#[test]
fn colores_y_collatz_en_el_silicio() {
    let v: Vec<f32> = (0..256).map(|i| (i as f32 - 128.0) * 0.0234).collect();
    contra_el_silicio(COLORES, [4, 1, 1], vec![bits(&v), vec![0; 256]]).unwrap();
    contra_el_silicio(COLLATZ, [1, 1, 1], vec![vec![27, 97, 871, 1]]).unwrap();
}

#[test]
fn las_trampas_paran_igual_en_el_silicio() {
    let a: Vec<u32> = (0..64).map(|i| (i * 10) as u32).collect();
    let b: Vec<u32> = (0..64).map(|i| if i == 37 { 0 } else { (i % 5 + 1) as u32 }).collect();
    let r = contra_el_silicio(DIVISION, [1, 1, 1], vec![a, b, vec![0; 64]]);
    assert_eq!(r, Err((Reason::DivisionByZero, [37, 0, 0])));
    let r = contra_el_silicio(SUMA, [4, 1, 1], vec![vec![0; 256], vec![0; 256], vec![0; 100]]);
    assert_eq!(r, Err((Reason::OutOfBounds, [100, 0, 0])));
}

#[test]
fn trascendentes_en_el_silicio_con_bits_al_azar() {
    let mut v = vec![0.0f32, -0.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e20, 3e38, f32::from_bits(1)];
    let mut w = vec![2.0f32; 8];
    let mut x: u32 = 0x0BAD_F00D;
    while v.len() < 1024 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push(f32::from_bits(x));
        w.push(f32::from_bits(x.rotate_left(11) & 0x41FF_FFFF));
    }
    contra_el_silicio(TRASCENDENTES, [16, 1, 1], vec![bits(&v), bits(&w), vec![0; 5 * 1024]]).unwrap();
}

#[test]
fn el_sello_no_deja_escribir() {
    // W^X de verdad: la pagina sellada no admite escritura. Se comprueba
    // preguntando al sistema, no rompiendo el proceso.
    let (_, code) = emitir(SUMA);
    let s = Sellado::new(&code);
    #[repr(C)]
    struct Info {
        base: *mut c_void,
        alloc_base: *mut c_void,
        alloc_protect: u32,
        region: usize,
        state: u32,
        protect: u32,
        kind: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn VirtualQuery(addr: *const c_void, info: *mut Info, len: usize) -> usize;
    }
    let mut info: Info = unsafe { core::mem::zeroed() };
    let n = unsafe { VirtualQuery(s.base, &mut info, core::mem::size_of::<Info>()) };
    assert!(n > 0);
    assert_eq!(info.protect, PAGE_EXECUTE_READ, "la pagina sellada es R+X, sin W");
}
