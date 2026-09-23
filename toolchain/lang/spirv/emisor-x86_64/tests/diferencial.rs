//! **La prueba DIFERENCIAL de S4**: cada sombreador corre DOS veces con los
//! mismos datos -- en el oraculo (`bmo_spirv_front::Interpreter`) y emitido a
//! x86-64 y ejecutado en el emulador de `bmo-lower` -- y los buffers tienen que
//! quedar IGUALES, bit a bit. Si para, tiene que parar por lo mismo y en la
//! misma invocacion.
//!
//! El emulador no se fia de nada: una instruccion que no conoce le hace parar
//! con su codigo. Asi que "pasa aqui" quiere decir "esos bytes, ejecutados,
//! calculan lo mismo que la definicion".

use bmo_lower::emu::{run, Machine};
use bmo_spirv_front::{read, workspace_words, Buffer, Interpreter, Reason};
use bmo_spirv_x86_64::{emit, tables_words, trap_reason, Program, IDS_WORDS};

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSI: usize = 6;
const RDI: usize = 7;

const SUMA: &[u8] = include_bytes!("../../pruebas/suma.spv");
const SAXPY: &[u8] = include_bytes!("../../pruebas/saxpy.spv");
const MANDELBROT: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
const COLORES: &[u8] = include_bytes!("../../pruebas/colores.spv");
const DIVISION: &[u8] = include_bytes!("../../pruebas/division.spv");
const BUCLE: &[u8] = include_bytes!("../../pruebas/bucle.spv");
const COLLATZ: &[u8] = include_bytes!("../../pruebas/collatz.spv");
const TRIG: &[u8] = include_bytes!("../../pruebas/trig.spv");
const TRASCENDENTES: &[u8] = include_bytes!("../../pruebas/trascendentes.spv");

type Salida = Result<(), (Reason, [u32; 3])>;

/// El oraculo.
fn oraculo(spv: &[u8], groups: [u32; 3], datos: &mut [Vec<u32>], fuel: u64) -> Salida {
    let mut ids = vec![0u32; 1 << 16];
    let m = read(spv, &mut ids).unwrap();
    let mut ws = vec![0u32; workspace_words(&m)];
    let mut it = Interpreter::new(&m, &mut ws).unwrap();
    let mut bufs: Vec<Buffer> =
        datos.iter_mut().enumerate().map(|(k, d)| Buffer { set: 0, binding: k as u32, data: d }).collect();
    it.dispatch(groups, &mut bufs, fuel).map(|_| ()).map_err(|t| (t.reason, t.invocation))
}

fn emitir(spv: &[u8]) -> (Program, Vec<u8>) {
    let mut ids = vec![0u32; 1 << 16];
    let m = read(spv, &mut ids).unwrap();
    let mut tablas = vec![0u32; tables_words(&m)];
    let mut code = vec![0u8; 1 << 20];
    let p = emit(&m, &mut tablas, &mut code).unwrap_or_else(|e| panic!("emisor: {}", e));
    code.truncate(p.code_len);
    (p, code)
}

/// Lo emitido, en el emulador. `datos[k]` es el buffer del binding `k`.
fn emulado(spv: &[u8], groups: [u32; 3], datos: &mut [Vec<u32>], fuel: u64) -> Salida {
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
fn diferencial(spv: &[u8], groups: [u32; 3], datos: Vec<Vec<u32>>, fuel: u64) -> Salida {
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
    ra
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn suma_igual_que_el_oraculo() {
    let a: Vec<f32> = (0..128).map(|i| i as f32 * 0.37 - 20.0).collect();
    let b: Vec<f32> = (0..128).map(|i| 1.0 / (i as f32 + 0.5)).collect();
    diferencial(SUMA, [2, 1, 1], vec![bits(&a), bits(&b), vec![0; 128]], 1_000_000).unwrap();
}

#[test]
fn saxpy_igual_que_el_oraculo() {
    let x: Vec<f32> = (0..128).map(|i| (i as f32).sqrt()).collect();
    let y: Vec<f32> = (0..128).map(|i| 100.0 - i as f32).collect();
    diferencial(SAXPY, [2, 1, 1], vec![vec![1.5f32.to_bits(), 100], bits(&x), bits(&y)], 1_000_000).unwrap();
}

#[test]
fn mandelbrot_igual_que_el_oraculo() {
    let (ancho, tope) = (16u32, 24u32);
    let ventana = vec![(-2.0f32).to_bits(), (-1.25f32).to_bits(), (2.5f32 / 16.0).to_bits(), (2.5f32 / 16.0).to_bits(), ancho, tope];
    diferencial(MANDELBROT, [2, 2, 1], vec![vec![0; 256], ventana], 1_000_000).unwrap();
}

#[test]
fn colores_igual_que_el_oraculo() {
    let v: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.0467).collect();
    diferencial(COLORES, [2, 1, 1], vec![bits(&v), vec![0; 128]], 1_000_000).unwrap();
}

#[test]
fn collatz_igual_que_el_oraculo() {
    diferencial(COLLATZ, [1, 1, 1], vec![vec![27, 97, 871, 1]], 1_000_000).unwrap();
}

#[test]
fn division_por_cero_para_igual() {
    let a: Vec<u32> = (0..64).map(|i| (i * 10) as u32).collect();
    let b: Vec<u32> = (0..64).map(|i| if i == 37 { 0 } else { (i % 5 + 1) as u32 }).collect();
    let r = diferencial(DIVISION, [1, 1, 1], vec![a, b, vec![0; 64]], 1_000_000);
    assert_eq!(r, Err((Reason::DivisionByZero, [37, 0, 0])));
}

#[test]
fn division_int_min_para_igual() {
    let r = diferencial(DIVISION, [1, 1, 1], vec![vec![i32::MIN as u32; 64], vec![u32::MAX; 64], vec![0; 64]], 1_000_000);
    assert_eq!(r, Err((Reason::DivisionOverflow, [0, 0, 0])));
}

#[test]
fn salirse_de_un_buffer_para_igual() {
    let r = diferencial(SUMA, [2, 1, 1], vec![vec![0; 128], vec![0; 128], vec![0; 100]], 1_000_000);
    assert_eq!(r, Err((Reason::OutOfBounds, [100, 0, 0])));
}

#[test]
fn un_bucle_sin_fin_para_en_los_dos() {
    // El combustible no se cuenta igual (el oraculo por instruccion, lo emitido
    // por salto hacia atras): se compara el MOTIVO, no la cuenta.
    let mut a = vec![vec![0u32, 0]];
    let mut b = a.clone();
    assert_eq!(oraculo(BUCLE, [1, 1, 1], &mut a, 5_000).unwrap_err().0, Reason::OutOfFuel);
    assert_eq!(emulado(BUCLE, [1, 1, 1], &mut b, 500).unwrap_err().0, Reason::OutOfFuel);
    diferencial(BUCLE, [1, 1, 1], vec![vec![1, 99]], 1_000_000).unwrap();
}

#[test]
fn trig_igual_que_el_oraculo() {
    // S4b: sin, cos, exp, log y pow emitidas en doble, como `math`.
    let v: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.173).collect();
    let datos = vec![bits(&v), vec![0; 64], vec![0; 64], vec![0; 64], vec![0; 64], vec![0; 64]];
    diferencial(TRIG, [1, 1, 1], datos, 1_000_000).unwrap();
}

#[test]
fn trascendentes_en_los_casos_raros() {
    // La entrada TAL CUAL: NaN, infinitos, negativos, ceros, subnormales,
    // angulos enormes (el cuadrante satura por encima de 2^63), y bits al azar.
    let mut v = vec![
        0.0f32, -0.0, 1.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::MIN,
        f32::MIN_POSITIVE, f32::from_bits(1), 1e20, -1e20, 3e38, 88.7, 88.8, -103.9, -104.0, 709.0,
        1.5707964, 3.1415927, -3.1415927, 1e6, 12345.678, 0.5, 2.0, 1e-30, -1e-30,
    ];
    let mut w = vec![2.0f32, 0.5, -1.0, 3.0, f32::NAN, 0.0, -0.0, 1e10, -1e10, 7.25];
    let mut x: u32 = 0x1234_5678;
    while v.len() < 128 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push(f32::from_bits(x));
        w.push(f32::from_bits(x.rotate_left(7) & 0x41FF_FFFF));
    }
    w.resize(128, 1.0);
    let datos = vec![bits(&v), bits(&w), vec![0; 5 * 128]];
    diferencial(TRASCENDENTES, [2, 1, 1], datos, 1_000_000).unwrap();
}

#[test]
fn poco_codigo_se_dice_con_cuanto_falta() {
    let mut ids = vec![0u32; 1 << 16];
    let m = read(SUMA, &mut ids).unwrap();
    let mut tablas = vec![0u32; tables_words(&m)];
    let mut code = vec![0u8; 16];
    let e = emit(&m, &mut tablas, &mut code).unwrap_err();
    assert!(matches!(e.reason, Reason::WorkspaceTooSmall { .. }), "{}", e);
}
