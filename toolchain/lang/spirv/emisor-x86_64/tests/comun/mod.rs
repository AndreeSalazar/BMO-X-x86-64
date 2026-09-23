//! El arnes de las pruebas del emisor: el oraculo, lo emitido en el emulador,
//! y la comparacion bit a bit. Lo usan `diferencial.rs` y `tres_apis.rs`.
#![allow(dead_code)]

use bmo_lower::emu::{run, Machine};
use bmo_spirv_front::{read, workspace_words, Buffer, Interpreter, Reason};
use bmo_spirv_x86_64::{emit, tables_words, trap_reason, Program, IDS_WORDS};

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSI: usize = 6;
const RDI: usize = 7;

pub type Salida = Result<(), (Reason, [u32; 3])>;

/// El oraculo.
pub fn oraculo(spv: &[u8], groups: [u32; 3], datos: &mut [Vec<u32>], fuel: u64) -> Salida {
    let mut ids = vec![0u32; 1 << 16];
    let m = read(spv, &mut ids).unwrap();
    let mut ws = vec![0u32; workspace_words(&m)];
    let mut it = Interpreter::new(&m, &mut ws).unwrap();
    let mut bufs: Vec<Buffer> =
        datos.iter_mut().enumerate().map(|(k, d)| Buffer { set: 0, binding: k as u32, data: d }).collect();
    it.dispatch(groups, &mut bufs, fuel).map(|_| ()).map_err(|t| (t.reason, t.invocation))
}

pub fn emitir(spv: &[u8]) -> (Program, Vec<u8>) {
    let mut ids = vec![0u32; 1 << 16];
    let m = read(spv, &mut ids).unwrap();
    let mut tablas = vec![0u32; tables_words(&m)];
    let mut code = vec![0u8; 1 << 20];
    let p = emit(&m, &mut tablas, &mut code).unwrap_or_else(|e| panic!("emisor: {}", e));
    code.truncate(p.code_len);
    (p, code)
}

/// Lo emitido, en el emulador. `datos[k]` es el buffer del binding `k`.
pub fn emulado(spv: &[u8], groups: [u32; 3], datos: &mut [Vec<u32>], fuel: u64) -> Salida {
    let (p, code) = emitir(spv);
    let mut m = Machine::new(code);
    let marco = m.load_data(&vec![0u8; p.frame_words * 4 + 4]);
    let mut dirs = Vec::new();
    let mut tabla = Vec::new();
    for &(_, binding) in &p.buffers[..p.n_buffers] {
        let d = &datos[binding as usize];
        let bytes: Vec<u8> = d.iter().flat_map(|x| x.to_le_bytes()).collect();
        let addr = m.load_data(&bytes);
        dirs.push((binding as usize, addr));
        tabla.extend_from_slice(&addr.to_le_bytes());
        tabla.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    }
    tabla.extend_from_slice(&[0u8; 16]);
    let t = m.load_data(&tabla);
    let ids = m.load_data(&[0u8; IDS_WORDS * 4]);

    m.regs[RDI] = marco;
    m.regs[RSI] = t;
    m.llamar(p.init);
    m = run(m, 50_000_000);

    let ls = p.local_size;
    let mut salida = Ok(());
    'todo: for wz in 0..groups[2] {
        for wy in 0..groups[1] {
            for wx in 0..groups[0] {
                for lz in 0..ls[2] {
                    for ly in 0..ls[1] {
                        for lx in 0..ls[0] {
                            let global = [wx * ls[0] + lx, wy * ls[1] + ly, wz * ls[2] + lz];
                            let index = lz * ls[0] * ls[1] + ly * ls[0] + lx;
                            let bloque = [global, [lx, ly, lz], [wx, wy, wz], groups]
                                .concat()
                                .into_iter()
                                .chain([index])
                                .flat_map(|x| x.to_le_bytes())
                                .collect::<Vec<u8>>();
                            m.escribir(ids, &bloque);
                            m.regs[RDI] = marco;
                            m.regs[RSI] = t;
                            m.regs[RDX] = ids;
                            m.regs[RCX] = fuel;
                            m.llamar(p.main);
                            m = run(m, 50_000_000);
                            let codigo = m.regs[RAX] as u32;
                            if codigo != 0 {
                                let r = trap_reason(codigo).unwrap_or_else(|| panic!("codigo de trampa {}", codigo));
                                salida = Err((r, global));
                                break 'todo;
                            }
                        }
                    }
                }
            }
        }
    }
    // Los buffers, de vuelta.
    for (binding, addr) in dirs {
        let d = &mut datos[binding];
        for (i, x) in d.iter_mut().enumerate() {
            *x = (m.read_u64(addr + 4 * i as u64) & 0xFFFF_FFFF) as u32;
        }
    }
    salida
}

/// **La comparacion**: mismos datos, mismo resultado.
pub fn diferencial(spv: &[u8], groups: [u32; 3], datos: Vec<Vec<u32>>, fuel: u64) -> Salida {
    diferencial_con(spv, groups, datos, fuel).0
}

/// Como `diferencial`, y devuelve tambien los buffers como quedaron.
pub fn diferencial_con(spv: &[u8], groups: [u32; 3], datos: Vec<Vec<u32>>, fuel: u64) -> (Salida, Vec<Vec<u32>>) {
    let mut a = datos.clone();
    let mut b = datos;
    let ra = oraculo(spv, groups, &mut a, fuel);
    let rb = emulado(spv, groups, &mut b, fuel);
    assert_eq!(ra, rb, "el oraculo y lo emitido paran distinto");
    for (k, (x, y)) in a.iter().zip(&b).enumerate() {
        for (i, (p, q)) in x.iter().zip(y).enumerate() {
            assert_eq!(p, q, "buffer {} palabra {}: oraculo 0x{:08x}, emitido 0x{:08x}", k, i, p, q);
        }
    }
    (ra, a)
}

pub fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

