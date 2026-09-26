//! **Del BSF al silicio, sin traducir.** El codigo sale del fichero, se
//! comprueba (capas 1 a 3, y el hash del codigo al tomarlo), se sella (W^X con `VirtualProtect`, como
//! `MEM_OP_SELLAR` en BMO-X) y se llama con la tabla que arma el FORMATO. El
//! Ryzen del anfitrion tiene que dar los mismos bits que el oraculo.

#![cfg(all(windows, target_arch = "x86_64"))]

use core::ffi::c_void;

use bmo_bsf::*;
use bmo_bsf_x86_64::*;
use bmo_spirv_front::{read, Buffer, Interpreter};
use bmo_spirv_x86_64::{emit, tables_words, IDS_WORDS};

const MANDELBROT: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
const SAXPY: &[u8] = include_bytes!("../../pruebas/saxpy.spv");

#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    fn VirtualProtect(addr: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    fn VirtualFree(addr: *mut c_void, size: usize, kind: u32) -> i32;
}

type Init = extern "sysv64" fn(*mut u32, *const u64);
type Main = extern "sysv64" fn(*mut u32, *const u64, *const u32, u64) -> u64;

fn fabricar(nombre: &str, spv: &[u8]) -> Vec<u8> {
    let mut ids = vec![0u32; 1 << 12];
    let m = read(spv, &mut ids).unwrap();
    let h = facts(&m).unwrap();
    let mut tablas = vec![0u32; tables_words(&m)];
    let mut code = vec![0u8; 1 << 20];
    let p = emit(&m, &mut tablas, &mut code).unwrap();
    let mut ranuras = [0xFF; MAX_BINDINGS];
    let t = [x86_64_target(&p, &code, h.bindings(), &mut ranuras).unwrap()];
    let modulo = ModuleIn {
        model: h.model,
        name: nombre.as_bytes(),
        local_size: h.local_size,
        capabilities: h.capabilities,
        caps_high: h.caps_high,
        spirv: spv,
        bindings: h.bindings(),
        targets: &t,
    };
    let mut out = vec![0u8; size(&[modulo]).unwrap()];
    write(&[modulo], &mut out).unwrap();
    out
}

/// Despacha el objetivo x86-64 de `m` sobre `datos[binding]`, en el silicio.
fn despachar(m: &ModuleView, groups: [u32; 3], datos: &mut [Vec<u32>]) {
    let t = m.target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2).expect("hay codigo para esta maquina");
    let code = t.code().unwrap();
    let base = unsafe {
        let b = VirtualAlloc(core::ptr::null_mut(), code.len(), 0x3000, 0x04);
        assert!(!b.is_null());
        core::ptr::copy_nonoverlapping(code.as_ptr(), b as *mut u8, code.len());
        let mut viejo = 0;
        assert!(VirtualProtect(b, code.len(), 0x20, &mut viejo) != 0);
        b
    };
    let init: Init = unsafe { core::mem::transmute(base as usize + t.init()) };
    let main: Main = unsafe { core::mem::transmute(base as usize + t.main()) };

    // Quien llama da los buffers en SU orden; el formato los comprueba y los
    // pone en el del codigo.
    let given: Vec<Given> = m
        .bindings()
        .map(|b| {
            let d = &mut datos[b.binding as usize];
            Given { set: b.set, binding: b.binding, addr: d.as_mut_ptr() as u64, bytes: (d.len() * 4) as u64, writable: true }
        })
        .collect();
    let mut tabla = [0u64; 2 * MAX_BINDINGS + 2];
    t.table(m, &given, &mut tabla).unwrap();
    let mut marco = vec![0u32; t.frame_words() + 1];
    init(marco.as_mut_ptr(), tabla.as_ptr());
    let ls = m.local_size();
    for wy in 0..groups[1] {
        for wx in 0..groups[0] {
            for ly in 0..ls[1] {
                for lx in 0..ls[0] {
                    let mut ids = [0u32; IDS_WORDS];
                    ids[..12].copy_from_slice(&[wx * ls[0] + lx, wy * ls[1] + ly, 0, lx, ly, 0, wx, wy, 0, groups[0], groups[1], 1]);
                    ids[12] = ly * ls[0] + lx;
                    assert_eq!(main(marco.as_mut_ptr(), tabla.as_ptr(), ids.as_ptr(), 1 << 20) as u32, 0);
                }
            }
        }
    }
    unsafe {
        VirtualFree(base, 0, 0x8000);
    }
}

fn oraculo(m: &ModuleView, groups: [u32; 3], datos: &mut [Vec<u32>]) {
    let mut ids = vec![0u32; 1 << 12];
    let modulo = read(m.spirv().unwrap(), &mut ids).unwrap();
    let mut ws = vec![0u32; bmo_spirv_front::workspace_words(&modulo)];
    let mut it = Interpreter::new(&modulo, &mut ws).unwrap();
    let mut bufs: Vec<Buffer> = datos.iter_mut().enumerate().map(|(k, d)| Buffer { set: 0, binding: k as u32, data: d }).collect();
    it.dispatch(groups, &mut bufs, 1 << 26).unwrap();
}

#[test]
fn mandelbrot_desde_el_bsf_da_los_bits_del_oraculo() {
    let bytes = fabricar("mandelbrot", MANDELBROT);
    let bsf = Bsf::parse(&bytes).unwrap();
    let m = bsf.find(b"mandelbrot").unwrap();
    let ventana = vec![(-2.0f32).to_bits(), (-1.25f32).to_bits(), (2.5f32 / 64.0).to_bits(), (2.5f32 / 64.0).to_bits(), 64, 64];
    let mut a = vec![vec![0u32; 64 * 64], ventana.clone()];
    let mut b = a.clone();
    despachar(&m, [8, 8, 1], &mut a);
    oraculo(&m, [8, 8, 1], &mut b);
    assert_eq!(a, b);
    assert!(a[0].iter().any(|&k| k == 64) && a[0].iter().any(|&k| k < 4), "hay dentro y fuera del conjunto");
}

#[test]
fn saxpy_desde_el_bsf_con_las_ranuras_cruzadas() {
    // saxpy: el emisor pide sus buffers en otro orden que (set, binding) --
    // las ranuras no son la identidad, y aun asi cada uno llega a su sitio.
    let bytes = fabricar("saxpy", SAXPY);
    let bsf = Bsf::parse(&bytes).unwrap();
    let m = bsf.module(0);
    let t = m.target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2).unwrap();
    assert_ne!(t.slots(), &[0, 1, 2], "si fueran la identidad esta prueba no probaria nada");
    let x: Vec<u32> = (0..256).map(|i| (i as f32 * 0.25 - 7.0).to_bits()).collect();
    let y: Vec<u32> = (0..256).map(|i| (1.0 / (i as f32 + 1.0)).to_bits()).collect();
    let mut a = vec![vec![2.5f32.to_bits(), 256], x, y];
    let mut b = a.clone();
    despachar(&m, [4, 1, 1], &mut a);
    oraculo(&m, [4, 1, 1], &mut b);
    assert_eq!(a, b);
}
