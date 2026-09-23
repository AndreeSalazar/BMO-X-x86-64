//! El banco del ORACULO (S3 de PLAN_EL_SOMBREADOR).
//!
//! Los cuatro sombreadores del subconjunto se ejecutan y su salida se compara,
//! BIT A BIT, con el mismo calculo escrito a mano en Rust, en el mismo orden
//! de operaciones. Y cada forma de parar tiene su fila: division por cero,
//! bucle sin fin, salirse de un buffer, un buffer que falta, poca memoria de
//! trabajo, y un valor que la invocacion no definio.

use bmo_spirv_front::{read, workspace_words, Buffer, Interpreter, Reason, Stats, Trap};

const SUMA: &[u8] = include_bytes!("../pruebas/suma.spv");
const SAXPY: &[u8] = include_bytes!("../pruebas/saxpy.spv");
const MANDELBROT: &[u8] = include_bytes!("../pruebas/mandelbrot.spv");
const COLORES: &[u8] = include_bytes!("../pruebas/colores.spv");
const DIVISION: &[u8] = include_bytes!("../pruebas/division.spv");
const BUCLE: &[u8] = include_bytes!("../pruebas/bucle.spv");
const INDEFINIDO: &[u8] = include_bytes!("../pruebas/indefinido.spv");

const FUEL: u64 = 1_000_000;

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Lee, prepara y despacha.
fn run(spv: &[u8], groups: [u32; 3], buffers: &mut [Buffer], fuel: u64) -> Result<Stats, Trap> {
    let mut ids = vec![0u32; 4096];
    let m = read(spv, &mut ids).unwrap_or_else(|e| panic!("lector: {}", e));
    let mut ws = vec![0u32; workspace_words(&m)];
    let mut it = Interpreter::new(&m, &mut ws).unwrap_or_else(|e| panic!("oraculo: {}", e));
    it.dispatch(groups, buffers, fuel)
}

fn buf(binding: u32, data: &mut [u32]) -> Buffer<'_> {
    Buffer { set: 0, binding, data }
}

// ---- los cuatro, contra Rust -----------------------------------------------

#[test]
fn suma() {
    let n = 256;
    let a: Vec<f32> = (0..n).map(|i| i as f32 * 0.37 - 20.0).collect();
    let b: Vec<f32> = (0..n).map(|i| 1.0 / (i as f32 + 0.5)).collect();
    let (mut ba, mut bb, mut bc) = (bits(&a), bits(&b), vec![0u32; n]);
    let s = run(SUMA, [4, 1, 1], &mut [buf(0, &mut ba), buf(1, &mut bb), buf(2, &mut bc)], FUEL).unwrap();
    assert_eq!(s.invocations, 256);
    let esperado: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
    assert_eq!(bc, bits(&esperado));
}

#[test]
fn saxpy_respeta_el_limite_n() {
    let n = 256;
    let alfa = 1.5f32;
    let limite = 200u32;
    let x: Vec<f32> = (0..n).map(|i| (i as f32).sqrt()).collect();
    let y: Vec<f32> = (0..n).map(|i| 100.0 - i as f32).collect();
    // std140: `float alfa` en 0, `uint n` en 4.
    let mut params = vec![alfa.to_bits(), limite];
    let (mut bx, mut by) = (bits(&x), bits(&y));
    run(SAXPY, [4, 1, 1], &mut [buf(0, &mut params), buf(1, &mut bx), buf(2, &mut by)], FUEL).unwrap();
    let esperado: Vec<f32> =
        (0..n).map(|i| if (i as u32) < limite { alfa * x[i] + y[i] } else { y[i] }).collect();
    assert_eq!(by, bits(&esperado));
}

#[test]
fn mandelbrot_pixel_a_pixel() {
    let (ancho, tope) = (32u32, 48u32);
    let origen = [-2.0f32, -1.25];
    let paso = [2.5f32 / 32.0, 2.5 / 32.0];
    // std140: vec2 origen 0, vec2 paso 8, uint ancho 16, uint tope 20.
    let mut ventana = vec![origen[0].to_bits(), origen[1].to_bits(), paso[0].to_bits(), paso[1].to_bits(), ancho, tope];
    let mut salida = vec![0u32; (ancho * ancho) as usize];
    let s = run(MANDELBROT, [4, 4, 1], &mut [buf(0, &mut salida), buf(1, &mut ventana)], FUEL).unwrap();
    assert_eq!(s.invocations, 1024);
    for py in 0..ancho {
        for px in 0..ancho {
            // El MISMO orden que el GLSL: c = origen + vec2(p) * paso, etc.
            let c = [origen[0] + (px as f32) * paso[0], origen[1] + (py as f32) * paso[1]];
            let mut z = [0.0f32, 0.0];
            let mut k = 0u32;
            while k < tope && z[0] * z[0] + z[1] * z[1] <= 4.0 {
                z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
                k += 1;
            }
            assert_eq!(salida[(py * ancho + px) as usize], k, "pixel ({}, {})", px, py);
        }
    }
    // Y no es trivial: hay pixeles que salen pronto y otros que llegan al tope.
    assert!(salida.contains(&tope) && salida.iter().any(|&k| k < 4));
}

#[test]
fn colores_con_su_funcion_auxiliar() {
    let n = 256;
    let v: Vec<f32> = (0..n).map(|i| (i as f32 - 128.0) * 0.0234).collect();
    let mut bv = bits(&v);
    let mut rgba = vec![0u32; n];
    run(COLORES, [4, 1, 1], &mut [buf(0, &mut bv), buf(1, &mut rgba)], FUEL).unwrap();
    let canal = |x: f32| (x.max(0.0).min(1.0) * 255.0 + 0.5) as u32;
    let mezcla = |x: [f32; 3], y: [f32; 3], a: f32| [0, 1, 2].map(|k| x[k] * (1.0 - a) + y[k] * a);
    for i in 0..n {
        let a = v[i].abs();
        let t = a - a.floor();
        let col = if t < 0.5 {
            mezcla([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], t * 2.0)
        } else {
            mezcla([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], t * 2.0 - 1.0)
        };
        let alfa = if v[i] < 0.0 { 128u32 } else { 255 };
        let esperado = canal(col[0]) | (canal(col[1]) << 8) | (canal(col[2]) << 16) | (alfa << 24);
        assert_eq!(rgba[i], esperado, "i = {} (v = {})", i, v[i]);
    }
}

// ---- las formas de parar -----------------------------------------------------

#[test]
fn division_por_cero_para_en_su_invocacion() {
    let mut a: Vec<u32> = (0..64).map(|i| (i * 10) as u32).collect();
    let mut b: Vec<u32> = (0..64).map(|i| if i == 37 { 0 } else { (i % 5 + 1) as u32 }).collect();
    let mut q = vec![0u32; 64];
    let t = run(DIVISION, [1, 1, 1], &mut [buf(0, &mut a), buf(1, &mut b), buf(2, &mut q)], FUEL).unwrap_err();
    assert_eq!(t.reason, Reason::DivisionByZero);
    assert_eq!(t.invocation, [37, 0, 0]);
    // Las anteriores SI se hicieron.
    assert_eq!(q[36] as i32, 360 / (36 % 5 + 1));
}

#[test]
fn division_int_min_entre_menos_uno() {
    let mut a = vec![i32::MIN as u32; 64];
    let mut b = vec![(-1i32) as u32; 64];
    let mut q = vec![0u32; 64];
    let t = run(DIVISION, [1, 1, 1], &mut [buf(0, &mut a), buf(1, &mut b), buf(2, &mut q)], FUEL).unwrap_err();
    assert_eq!(t.reason, Reason::DivisionOverflow);
}

#[test]
fn un_bucle_sin_fin_para_por_combustible() {
    let mut bandera = vec![0u32, 0];
    let t = run(BUCLE, [1, 1, 1], &mut [buf(0, &mut bandera)], 10_000).unwrap_err();
    assert_eq!(t.reason, Reason::OutOfFuel);
    // Con la bandera puesta termina, y dice cuantas vueltas dio: ninguna.
    let mut bandera = vec![1u32, 99];
    run(BUCLE, [1, 1, 1], &mut [buf(0, &mut bandera)], 10_000).unwrap();
    assert_eq!(bandera[1], 0);
}

#[test]
fn salirse_de_un_buffer_para() {
    let mut a = vec![0u32; 256];
    let mut b = vec![0u32; 256];
    let mut c = vec![0u32; 100]; // cabe la mitad
    let t = run(SUMA, [4, 1, 1], &mut [buf(0, &mut a), buf(1, &mut b), buf(2, &mut c)], FUEL).unwrap_err();
    assert_eq!(t.reason, Reason::OutOfBounds);
    assert_eq!(t.invocation, [100, 0, 0]);
}

#[test]
fn un_buffer_que_falta_se_nombra() {
    let mut a = vec![0u32; 256];
    let mut b = vec![0u32; 256];
    let t = run(SUMA, [4, 1, 1], &mut [buf(0, &mut a), buf(1, &mut b)], FUEL).unwrap_err();
    assert_eq!(t.reason, Reason::NoBuffer { set: 0, binding: 2 });
}

#[test]
fn poca_memoria_de_trabajo_se_dice() {
    let mut ids = vec![0u32; 4096];
    let m = read(SUMA, &mut ids).unwrap();
    let need = workspace_words(&m);
    let mut ws = vec![0u32; need - 1];
    let e = Interpreter::new(&m, &mut ws).err().expect("tiene que decir que no");
    assert_eq!(e.reason, Reason::WorkspaceTooSmall { need, have: need - 1 });
}

#[test]
fn un_valor_que_no_domina_para_en_la_invocacion_que_no_lo_definio() {
    // La invocacion 0 entra en `then` y define %v; la 1 no, y lo lee igual.
    let mut salida = vec![0u32; 2];
    let t = run(INDEFINIDO, [1, 1, 1], &mut [buf(0, &mut salida)], FUEL).unwrap_err();
    assert!(matches!(t.reason, Reason::UndefinedValue { .. }), "{:?}", t.reason);
    assert_eq!(t.invocation, [1, 0, 0]);
    // La 0 si escribio: (0 + 7) + 7.
    assert_eq!(salida[0], 14);
}

const COLLATZ: &[u8] = include_bytes!("../pruebas/collatz.spv");

#[test]
fn collatz_con_los_numeros_conocidos() {
    // 27 -> 111 pasos, 97 -> 118, 871 -> 178, 1 -> 0.
    let mut n = vec![27u32, 97, 871, 1];
    run(COLLATZ, [1, 1, 1], &mut [buf(0, &mut n)], FUEL).unwrap();
    assert_eq!(n, vec![111, 118, 178, 0]);
    // Y desde 0 no llega nunca: el oraculo para por combustible, no se cuelga.
    // Es lo que le paso al collatz del banco de Naga con los buffers a cero.
    let mut n = vec![0u32, 1, 1, 1];
    let t = run(COLLATZ, [1, 1, 1], &mut [buf(0, &mut n)], 50_000).unwrap_err();
    assert_eq!((t.reason, t.invocation), (Reason::OutOfFuel, [0, 0, 0]));
}
