//! **El juez del computo traducido a x86-64** (E2.3b, 05-10): el interprete.
//!
//! `bmo_proton_x::nativo_computo` traduce un programa con saltos, enteros,
//! memoria compartida, buferes y barreras; aqui se pone en memoria
//! ejecutable (escribir, y despues R+X sin W, como `MEM_OP_SELLAR`) y se
//! corre contra el interprete, BIT A BIT: el CS de `computo.dxil`, el de
//! nBodyGravity (Microsoft, MIT), los programas de saltos de `ejemplos` y
//! uno hecho a mano con cada operacion entera, cada comparacion y cada
//! conversion sobre los valores que muerden.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use bmo_proton_x::bufer::{Bufer, Uav};
use bmo_proton_x::dxil::programa::{Comparacion, Conversion, Op, OpEntera, Programa};
use bmo_proton_x::dxil::{self, ejemplos};
use bmo_proton_x::nativo_computo::{self, Contexto, Funcion, Vista, ACABO, VISTAS};
use bmo_proton_x::textura::Recursos;

const COMPUTO: &[u8] = include_bytes!("../../proton-x/prueba/computo.dxil");
const NBODY_CS: &[u8] = include_bytes!("../../proton-x/prueba/muestras/nbody/nBodyGravityCS.cso");

unsafe fn syscall6(n: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let r: u64;
    core::arch::asm!("syscall", inlateout("rax") n => r, in("rdi") a, in("rsi") b, in("rdx") c,
        in("r10") d, in("r8") e, in("r9") f, lateout("rcx") _, lateout("r11") _, options(nostack));
    r
}

/// Los bytes, en memoria nueva, y despues R+X (sin W).
fn sellar(codigo: &[u8]) -> Funcion {
    let n = codigo.len().div_ceil(4096) as u64 * 4096;
    let base = unsafe { syscall6(9, 0, n, 3, 0x22, u64::MAX, 0) };
    assert!(base < (-4096i64) as u64);
    unsafe {
        core::ptr::copy_nonoverlapping(codigo.as_ptr(), base as *mut u8, codigo.len());
        assert_eq!(syscall6(10, base, n, 5, 0, 0, 0), 0);
        core::mem::transmute::<u64, Funcion>(base)
    }
}

/// Un Dispatch por los dos caminos, con los mismos datos: lo que escribe
/// cada uno en su UAV.
fn los_dos(cs: &[u8], grupos: [u32; 3], cb: &[u8], srv: &[u8], paso_srv: u32, uav: &[u8], paso_uav: u32) -> (Vec<u8>, Vec<u8>) {
    let p = dxil::computo::preparar(cs).unwrap().programa;
    let f = sellar(&nativo_computo::compilar(&p).expect("se traduce entero"));
    let elementos_srv = srv.len() as u32 / paso_srv;
    let buf = [Some(Bufer { bytes: srv, formato: 0, paso: paso_srv, elementos: elementos_srv })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &buf, dinamicas: None };
    let (mut a, mut b) = (uav.to_vec(), uav.to_vec());
    let elementos = uav.len() as u32 / paso_uav;
    let t = std::time::Instant::now();
    {
        let mut u = [Some(Uav { bytes: &mut a, formato: 0, paso: paso_uav, elementos })];
        p.despachar(grupos, cb, &rec, &mut u);
    }
    let interpretado = t.elapsed();
    let t = std::time::Instant::now();
    {
        let mut u = [Some(Uav { bytes: &mut b, formato: 0, paso: paso_uav, elementos })];
        // SAFETY: `f` es la traduccion de `p`, sellada y viva.
        unsafe { nativo_computo::despachar(&p, f, grupos, cb, &buf, &mut u) };
    }
    eprintln!("interpretado {interpretado:?}, traducido {:?}", t.elapsed());
    (a, b)
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// *** `computo.dxil` (64 hilos, memoria compartida, una barrera): los dos
/// caminos escriben los MISMOS bytes, con n = 250 (el ultimo grupo escribe
/// 58 de 64) y con un grupo mas de los que hay datos (lee fuera: 0).
#[test]
fn el_cs_de_computo_traducido_da_los_bits_del_interprete() {
    let entrada: Vec<f32> = (0..256u32).flat_map(|k| [k as f32, k as f32 * 0.25, -(k as f32) * 3.0 - 1.0, (k % 7) as f32]).collect();
    let mut cb = vec![0u8; 16];
    cb[..4].copy_from_slice(&250u32.to_le_bytes());
    cb[4..8].copy_from_slice(&2.0f32.to_le_bytes());
    let antes = floats(&vec![-1.0f32; 256 * 4]);
    let (a, b) = los_dos(COMPUTO, [4, 1, 1], &cb, &floats(&entrada), 16, &antes, 16);
    assert_ne!(a, antes, "el interprete escribio");
    assert_eq!(a, b, "los mismos bytes");
    // Cinco grupos sobre 256 datos: el quinto lee fuera del SRV.
    cb[..4].copy_from_slice(&320u32.to_le_bytes());
    let antes = floats(&vec![-1.0f32; 320 * 4]);
    let (a, b) = los_dos(COMPUTO, [5, 1, 1], &cb, &floats(&entrada), 16, &antes, 16);
    assert_eq!(a, b);
}

/// *** El CS de nBodyGravity (128 hilos, la barrera DENTRO de un bucle,
/// 384 lecturas de la compartida con indice constante, 129 raices y
/// divisiones por interaccion): 512 particulas (4 grupos), los mismos bits
/// por los dos caminos. Y lo que tarda cada uno.
#[test]
fn el_cs_de_nbody_traducido_da_los_bits_del_interprete() {
    let n = 512u32;
    // (pos, velo): una espiral, con las velocidades a cero menos la w.
    let datos: Vec<f32> = (0..n)
        .flat_map(|k| {
            let t = k as f32 * 0.37;
            [t.cos() * (10.0 + k as f32), t.sin() * (10.0 + k as f32), (k % 17) as f32 - 8.0, 1.0, 0.0, 0.0, 0.0, 0.0]
        })
        .collect();
    let mut cb = vec![0u8; 32];
    cb[..4].copy_from_slice(&n.to_le_bytes());
    cb[4..8].copy_from_slice(&n.div_ceil(128).to_le_bytes());
    cb[16..20].copy_from_slice(&0.1f32.to_le_bytes());
    cb[20..24].copy_from_slice(&1.0f32.to_le_bytes());
    let antes = vec![0u8; n as usize * 32];
    let t = std::time::Instant::now();
    let (a, b) = los_dos(NBODY_CS, [n.div_ceil(128), 1, 1], &cb, &floats(&datos), 32, &antes, 32);
    eprintln!("nbody, {n} particulas, los dos caminos: {:?}", t.elapsed());
    assert_ne!(a, antes);
    assert_eq!(a, b, "los mismos bytes");
}

/// Correr un programa con entradas y salidas (los de `ejemplos`) por el
/// codigo traducido: un hilo, con su contexto.
fn correr_nativo(p: &Programa, f: Funcion, ent: &[[f32; 4]], salidas: usize) -> Vec<[f32; 4]> {
    let mut s = vec![[0.0f32; 4]; salidas];
    let mut regs = p.iniciales.clone();
    let mut c = Contexto {
        ids: [0; 10],
        reanudar: 0,
        n_compartida: 0,
        compartida: core::ptr::null_mut(),
        entradas: ent.as_ptr(),
        salidas: s.as_mut_ptr(),
        srv: [Vista::NULA; VISTAS],
        uav: [Vista::NULA; VISTAS],
    };
    let cb = [0u8; 16];
    // SAFETY: `f` es la traduccion de `p`; el contexto y lo que apunta, de aqui.
    assert_eq!(unsafe { f(regs.as_mut_ptr(), &mut c, cb.as_ptr()) }, ACABO);
    s
}

fn igual(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()))
}

/// Los valores que muerden: ceros con signo, unos, NaN, infinitos, lo que no
/// cabe en un entero, los bordes de i32 y u32 como bits.
fn raros() -> Vec<f32> {
    let mut v = vec![
        0.0f32, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, -2.5, 3.0, 7.75, 100.0, -100.0, 1e-40, -1e-40, f32::MIN_POSITIVE, f32::MAX, f32::MIN, f32::INFINITY,
        f32::NEG_INFINITY, f32::NAN, -f32::NAN, 2147483520.0, 2147483648.0, -2147483648.0, -2147483904.0, 4294967040.0, 4294967296.0, 1e10, -1e10, 0.999_999_9,
    ];
    for b in [0u32, 1, 2, 3, 31, 32, 33, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 0x0001_0000, 12345, 0x00FF_00FF] {
        v.push(f32::from_bits(b));
    }
    v
}

/// *** Los cuatro de `ejemplos` (un si, un bucle que sale por su RomperSi, un
/// contador entero con Elige, bucles anidados con un Romper dentro de un
/// si): con cada par de valores raros, la salida del traducido es la del
/// interprete.
#[test]
fn los_saltos_traducidos_dan_lo_del_interprete() {
    for (nombre, p) in [("si_sino", ejemplos::si_sino()), ("bucle_geometrico", ejemplos::bucle_geometrico()), ("bucle_entero", ejemplos::bucle_entero()), ("anidado", ejemplos::anidado())] {
        let f = sellar(&nativo_computo::compilar(&p).unwrap_or_else(|| panic!("{nombre}: se traduce")));
        // Los pares que ACABAN (los bucles con otros no acaban, en ninguno de
        // los dos): el si y el contador, todos los raros; la suma
        // geometrica, x por debajo de 2y; el anidado, j*y > x en pocas
        // vueltas.
        let vals = raros();
        let pares: Vec<(f32, f32)> = match nombre {
            "bucle_geometrico" => [0.25f32, 0.5, 1.0, 2.0, 3.0, 100.0, 1e-3].iter().flat_map(|&y| [-1e10f32, -1.0, -0.0, 0.0, 0.5 * y, y, 1.5 * y, 1.9 * y].map(|x| (x, y))).collect(),
            "anidado" => [0.125f32, 0.5, 1.0, 2.0, 3.0].iter().flat_map(|&y| [-1e10f32, -1.0, 0.0, 0.5, 2.0, 7.25, 100.0].map(|x| (x, y))).collect(),
            _ => vals.iter().flat_map(|&x| vals.iter().map(move |&y| (x, y))).collect(),
        };
        let mut casos = 0;
        for (x, y) in pares {
            {
                let ent = [[x, y, 0.0, 0.0]];
                let mut s1 = vec![[0.0f32; 4]; p.salidas];
                let mut regs = Vec::new();
                p.correr(&ent, &[0u8; 16], &mut s1, &mut regs);
                let s2 = correr_nativo(&p, f, &ent, p.salidas);
                assert!(igual(s1[0], s2[0]), "{nombre}({x:e}, {y:e}): interprete {:?}, traducido {:?}", s1[0], s2[0]);
                casos += 1;
            }
        }
        assert!(casos >= 35, "{nombre}: {casos} casos");
    }
}

/// *** Cada operacion entera, cada comparacion (float y entera) y cada
/// conversion, sobre cada par de valores raros: un programa hecho a mano,
/// una salida por operacion.
#[test]
fn los_enteros_las_comparaciones_y_las_conversiones_dan_lo_del_interprete() {
    let enteras = [
        OpEntera::Resta,
        OpEntera::Mul,
        OpEntera::Shl,
        OpEntera::ShrL,
        OpEntera::ShrA,
        OpEntera::Y,
        OpEntera::O,
        OpEntera::OX,
        OpEntera::MinS,
        OpEntera::MaxS,
        OpEntera::MinU,
        OpEntera::MaxU,
        OpEntera::DivU,
        OpEntera::RemU,
        OpEntera::DivS,
        OpEntera::RemS,
    ];
    let comparaciones = [
        Comparacion::Menor,
        Comparacion::MenorIgual,
        Comparacion::Mayor,
        Comparacion::MayorIgual,
        Comparacion::Igual,
        Comparacion::Distinto,
        Comparacion::MenorSinSigno,
        Comparacion::MenorIgualSinSigno,
        Comparacion::MayorSinSigno,
        Comparacion::MayorIgualSinSigno,
    ];
    let conversiones = [Conversion::EnteroAFloat, Conversion::SinSignoAFloat, Conversion::FloatAEntero, Conversion::FloatASinSigno];
    let mut ops = vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }];
    let mut d = 2u16;
    let mut salidas = Vec::new();
    let mut salir = |ops: &mut Vec<Op>, r: u16| {
        let k = salidas.len();
        salidas.push(r);
        ops.push(Op::Salida { s: r, elemento: (k / 4) as u8, componente: (k % 4) as u8 });
    };
    for &o in &enteras {
        ops.push(Op::Entera { d, a: 0, b: 1, op: o });
        salir(&mut ops, d);
        d += 1;
    }
    for &c in &comparaciones {
        for entero in [false, true] {
            ops.push(Op::Compara { d, a: 0, b: 1, como: c, entero });
            salir(&mut ops, d);
            d += 1;
        }
    }
    for &c in &conversiones {
        ops.push(Op::Convierte { d, a: 0, como: c });
        salir(&mut ops, d);
        d += 1;
    }
    // Y lo demas que no tiene `ejemplos`: suma entera, elegir, valor
    // absoluto, minimo y maximo de D3D, saturar, raiz y su inversa.
    for op in [
        Op::SumaEntera { d, a: 0, b: 1 },
        Op::Elige { d: d + 1, c: 0, a: 1, b: 0 },
        Op::Abs { d: d + 2, a: 0 },
        Op::Min { d: d + 3, a: 0, b: 1 },
        Op::Max { d: d + 4, a: 0, b: 1 },
        Op::Saturate { d: d + 5, a: 0 },
        Op::Sqrt { d: d + 6, a: 0 },
        Op::Rsqrt { d: d + 7, a: 0 },
        Op::Mad { d: d + 8, a: 0, b: 1, c: 0 },
        Op::Div { d: d + 9, a: 0, b: 1 },
    ] {
        ops.push(op);
    }
    for k in 0..10 {
        salir(&mut ops, d + k);
    }
    d += 10;
    let n_salidas = salidas.len().div_ceil(4);
    let p = Programa { ops, iniciales: vec![0.0; d as usize], entradas: 1, salidas: n_salidas, lee: 1, filas_cb: 0, ranuras: Default::default(), computo: Default::default() };
    let f = sellar(&nativo_computo::compilar(&p).unwrap());
    let vals = raros();
    let mut malos = Vec::new();
    for &x in &vals {
        for &y in &vals {
            let ent = [[x, y, 0.0, 0.0]];
            let mut s1 = vec![[0.0f32; 4]; n_salidas];
            let mut regs = Vec::new();
            p.correr(&ent, &[0u8; 16], &mut s1, &mut regs);
            let s2 = correr_nativo(&p, f, &ent, n_salidas);
            for (k, (a, b)) in s1.iter().zip(&s2).enumerate() {
                if !igual(*a, *b) {
                    malos.push(format!("({:#010x}, {:#010x}) salida {k}: interprete {:?}, traducido {:?}", x.to_bits(), y.to_bits(), a.map(f32::to_bits), b.map(f32::to_bits)));
                }
            }
        }
    }
    assert!(malos.is_empty(), "{} distintos; los primeros:\n{}", malos.len(), malos.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// El `rand` del UCRT (`x = x * 214013 + 2531011`, los bits 16..30).
struct RandDeMsvc(u32);

impl RandDeMsvc {
    fn siguiente(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        ((self.0 >> 16) & 0x7FFF) as i32
    }
}

/// Las particulas de nBodyGravity como las pone la muestra
/// (`LoadParticles`: `srand(0)` en cada mitad, un punto al azar de la bola
/// de radio 400 con `RandomPercent`, y las dos mitades a +-200 en x con
/// velocidad -20 y 20 en z), `n` / 2 de cada una. En floats, como su C++.
fn particulas_de_nbody(n: usize) -> Vec<f32> {
    let spread = 400.0f32;
    let mut v = Vec::with_capacity(n * 8);
    for (cx, vz) in [(spread * 0.5, -20.0f32), (-spread * 0.5, 20.0)] {
        let mut r = RandDeMsvc(0);
        for _ in 0..n / 2 {
            let mut d = [spread; 3];
            while (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2] > spread * spread {
                for x in d.iter_mut() {
                    *x = ((r.siguiente() % 10000) - 5000) as f32 / 5000.0 * spread;
                }
            }
            v.extend_from_slice(&[cx + d[0], d[1], d[2], 1e8, 0.0, 0.0, vz, 1e-8]);
        }
    }
    v
}

/// *** El CS de nBodyGravity TRADUCIDO, sobre 2048 particulas como las de la
/// muestra, da la FISICA: cada aceleracion, velocidad y posicion contra la
/// cuenta en f64 (la suma de cada par, sin las fantasmas). El margen (R3):
/// floats sumados en otro orden, asi que la cota es 1e-3 de la suma de los
/// valores absolutos de lo sumado (lo que puede perder una suma) mas lo de
/// las fantasmas que el CS suma y resta. Probado que dice NO: con un paso
/// de 0.1001 en vez de 0.1 (una milesima), las posiciones se salen.
#[test]
fn el_cs_de_nbody_traducido_da_la_fisica() {
    let n = 2048usize;
    let datos = particulas_de_nbody(n);
    let p = dxil::computo::preparar(NBODY_CS).unwrap().programa;
    let f = sellar(&nativo_computo::compilar(&p).unwrap());
    let grupos = n.div_ceil(128) as u32;
    let mut cb = vec![0u8; 32];
    cb[..4].copy_from_slice(&(n as u32).to_le_bytes());
    cb[4..8].copy_from_slice(&grupos.to_le_bytes());
    cb[16..20].copy_from_slice(&0.1f32.to_le_bytes());
    cb[20..24].copy_from_slice(&1.0f32.to_le_bytes());
    let srv_bytes = floats(&datos);
    let buf = [Some(Bufer { bytes: &srv_bytes, formato: 0, paso: 32, elementos: n as u32 })];
    let mut salida = vec![0u8; n * 32];
    {
        let mut u = [Some(Uav { bytes: &mut salida, formato: 0, paso: 32, elementos: n as u32 })];
        // SAFETY: `f` es la traduccion de `p`, sellada y viva.
        unsafe { nativo_computo::despachar(&p, f, [grupos, 1, 1], &cb, &buf, &mut u) };
    }
    let leer = |i: usize, k: usize| f32::from_le_bytes(salida[i * 32 + 4 * k..i * 32 + 4 * k + 4].try_into().unwrap()) as f64;
    let (masa, suave, dt) = (6.673e-11f64 * 1e4 * 1e4 * 1e4, 0.00125f64 * 0.00125, 0.1f64);
    let fantasmas = (grupos as usize * 128 - n) as f64;
    let mut peor = 0.0f64;
    for i in 0..n {
        let pi = [datos[i * 8] as f64, datos[i * 8 + 1] as f64, datos[i * 8 + 2] as f64];
        let (mut a, mut cota) = ([0.0f64; 3], [0.0f64; 3]);
        for j in 0..n {
            let r = [datos[j * 8] as f64 - pi[0], datos[j * 8 + 1] as f64 - pi[1], datos[j * 8 + 2] as f64 - pi[2]];
            let d2 = r[0] * r[0] + r[1] * r[1] + r[2] * r[2] + suave;
            let s = masa / (d2 * d2.sqrt());
            for k in 0..3 {
                a[k] += r[k] * s;
                cota[k] += (r[k] * s).abs();
            }
        }
        let d2 = pi[0] * pi[0] + pi[1] * pi[1] + pi[2] * pi[2] + suave;
        let fantasma = 2.0 * fantasmas * masa / d2;
        let v = [datos[i * 8 + 4] as f64 + a[0] * dt, datos[i * 8 + 5] as f64 + a[1] * dt, datos[i * 8 + 6] as f64 + a[2] * dt];
        for k in 0..3 {
            let margen = 1e-3 * (cota[k] + fantasma) + 1e-6;
            let (vc, pc) = (leer(i, 4 + k), leer(i, k));
            assert!((vc - v[k]).abs() <= margen * dt + 1e-6, "particula {i}, v{k}: {vc} y la fisica {} (margen {})", v[k], margen * dt);
            assert!((pc - (pi[k] + v[k] * dt)).abs() <= margen * dt * dt + 1e-4 * pi[k].abs().max(1.0), "particula {i}, p{k}");
            peor = peor.max((vc - v[k]).abs() / (margen * dt + 1e-6));
        }
        let modulo = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        assert!((leer(i, 7) - modulo).abs() <= 1e-3 * (cota[0] + cota[1] + cota[2] + fantasma) + 1e-6, "particula {i}: |a|");
    }
    eprintln!("nbody: lo peor, {:.3} del margen", peor);
}
