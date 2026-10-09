//! ** LB7a DE `docs/plan/PLAN_LAS_LIBRERIAS.md`, EN EL ANFITRION: el cubo de
//! `bmo_cubo` contado por gpu fn (`ejemplos/nivel11/cubo_gira`), BIT A BIT.
//!
//! - Sus cuentas (`num`): el seno, el coseno, la tangente, la raiz y los
//!   redondeos de `bmo_cubo::num`, con sus mismos bits -- tambien la raiz,
//!   cuya estimacion alli sale de los bits del f32 y aqui se cuenta con f32
//!   exactos --.
//! - La matriz de cada fotograma (`cubo.wvp`): las 16 entradas de la `wvp`
//!   de `constantes`, en los 360.
//! - La tanda de cada fotograma: cada vertice a recorte, su subpixel, si esta
//!   detras, la luz de cada cara; con eso, las caras que miran y su color --
//!   lo que `tanda::de_fotograma` le da a la 3060 --, en los 360.
//!
//! Cada cuenta, por CADA tarjeta que da `titan` (la RTX 3060 12G simulada y la
//! CPU), por la casa y por el calculo: los cuatro dan lo mismo, y es lo de
//! `bmo_cubo`. Lo hace posible la division EXACTA de la 3060 (DL10).

use bmo_cubo::{angulo_de_fotograma, constantes, indices, num, tanda, vertices};
use bmo_titan_front::ir::Module;
use bmo_titan_prometeo::Kernel;
use std::path::Path;

fn paquete() -> Module {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join("nivel11").join("cubo_gira");
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("el ejemplo del banco");
    bmo_titan_front::lower_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// Las gpu fn del paquete, escritas, juzgadas y con su bateria pasada en
/// cada tarjeta.
struct Cubo {
    m: Module,
    ks: Vec<Kernel<'static>>,
}

impl Cubo {
    fn nuevo() -> Cubo {
        let m = paquete();
        let ks = bmo_titan_prometeo::kernels(&m, &bmo_titan_x86_64::TARJETAS).unwrap_or_else(|e| panic!("{}", e));
        Cubo { m, ks }
    }

    /// **La gpu fn `nombre` sobre estas celdas** (una columna por valor), por
    /// cada tarjeta, la casa y el calculo: lo mismo en todos, o no hay nada.
    fn corre(&self, nombre: &str, celdas: &[Vec<u32>]) -> Vec<u32> {
        let func = self.m.functions.iter().position(|f| f.name == nombre).unwrap_or_else(|| panic!("no hay `{}`", nombre));
        let calc = bmo_titan_front::calc::run_gpu(&self.m, func, celdas).unwrap_or_else(|e| panic!("{}: el calculo: {:?}", nombre, e));
        // Cada tarjeta la escribio: si no, no se compara nada.
        let suyas: Vec<_> = self.ks.iter().filter(|k| k.name == nombre).collect();
        assert_eq!(suyas.len(), bmo_titan_x86_64::TARJETAS.len(), "`{}`: no la escribieron todas las tarjetas", nombre);
        for k in suyas {
            let suya = bmo_titan_prometeo::run(k, celdas).unwrap_or_else(|e| panic!("{}: {}", nombre, e));
            let casa = bmo_titan_prometeo::run_casa(k, celdas);
            for i in 0..calc.len() {
                let mismo = |a: u32, b: u32| a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan());
                assert!(mismo(suya[i], calc[i]) && mismo(casa[i], calc[i]), "{} en {}, celda {} ({:?}): la tarjeta {:#x}, la casa {:#x}, el calculo {:#x}", nombre, k.tarjeta.ficha().nombre, i, celdas.iter().map(|c| f32::from_bits(c[i])).collect::<Vec<_>>(), suya[i], casa[i], calc[i]);
            }
        }
        calc
    }
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn igual(nombre: &str, x: f32, da: u32, quiere: f32) {
    assert!(da == quiere.to_bits() || (f32::from_bits(da).is_nan() && quiere.is_nan()), "{}({:e} = {:#x}): {:#x} y bmo_cubo {:#x}", nombre, x, x.to_bits(), da, quiere.to_bits());
}

/// Los valores que se le dan a las cuentas: los angulos del cubo, un barrido
/// de -20 a 20, potencias de dos con sus vecinos, y los bordes.
fn valores() -> Vec<f32> {
    let mut v: Vec<f32> = (0..360).flat_map(|f| [angulo_de_fotograma(f), angulo_de_fotograma(f) * 0.7]).collect();
    v.extend((-2000..2000).map(|k| k as f32 * 0.01));
    for e in -40..40 {
        let p = 2.0f32.powi(e);
        v.extend([p, -p, f32::from_bits(p.to_bits() + 1), f32::from_bits(p.to_bits() - 1), p * 1.5, p * 0.75 + 0.25]);
    }
    v.extend([0.0, -0.0, 0.5, -0.5, 1.5, 2.5, -2.5, 3.5, 1.0e-45, 1.0e-40, f32::MIN_POSITIVE, 1.0e30, -1.0e30, f32::MAX, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 3.1415927, -3.1415927, 1.5707964, 6.2831855, 100.0, 1000.5]);
    v
}

/// ** SUS CUENTAS (`num`), las de `bmo_cubo::num`: los mismos bits.
#[test]
fn the_numbers_of_bmo_cubo_counted_by_gpu_fn_give_its_bits() {
    let c = Cubo::nuevo();
    let xs = valores();
    let col = [bits(&xs)];
    for (nombre, f) in [("num.seno", num::seno as fn(f32) -> f32), ("num.coseno", num::coseno), ("num.tangente", num::tangente)] {
        for (x, da) in xs.iter().zip(c.corre(nombre, &col)) {
            igual(nombre, *x, da, f(*x));
        }
    }
    for (x, da) in xs.iter().zip(c.corre("num.redondear", &col)) {
        igual("num.redondear", *x, da, num::redondear(*x) as f32);
    }
    for (x, da) in xs.iter().zip(c.corre("num.saturar", &col)) {
        igual("num.saturar", *x, da, num::saturar(*x));
    }
    // redondear_par: alli un i64 que se sale con |x| >= 2^63; aqui, lo de una
    // pantalla por 256 y mucho mas.
    let pares: Vec<f32> = xs.iter().copied().filter(|x| x.abs() < 1.0e18 || x.is_nan()).chain((0..4000).map(|k| k as f32 * 0.25 - 500.0)).collect();
    for (x, da) in pares.iter().zip(c.corre("num.redondear_par", &[bits(&pares)])) {
        igual("num.redondear_par", *x, da, num::redondear_par(*x) as f32);
    }
    // La raiz: lo que da la estimacion por bits de `bmo_cubo`, tambien con
    // subnormales, enormes, ceros, negativos, NaN e infinitos.
    let mut raices: Vec<f32> = xs.clone();
    let mut z: u32 = 0x5eed;
    for _ in 0..4000 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        raices.push(f32::from_bits(z & 0x7FFF_FFFF));
    }
    for (x, da) in raices.iter().zip(c.corre("num.raiz", &[bits(&raices)])) {
        igual("num.raiz", *x, da, num::raiz(*x));
    }
}

/// La `wvp` de los 360 fotogramas, por gpu fn: 16 entradas por fotograma.
fn matrices(c: &Cubo) -> Vec<[f32; 16]> {
    let (fs, ks): (Vec<f32>, Vec<f32>) = (0..360).flat_map(|f| (0..16).map(move |k| (f as f32, k as f32))).unzip();
    let w = c.corre("cubo.wvp", &[bits(&fs), bits(&ks)]);
    w.chunks(16).map(|e| core::array::from_fn(|k| f32::from_bits(e[k]))).collect()
}

/// ** LA MATRIZ DE CADA FOTOGRAMA: la `wvp` de `constantes`, en los 360.
#[test]
fn the_matrix_of_each_frame_is_the_one_of_bmo_cubo() {
    let c = Cubo::nuevo();
    for (f, w) in matrices(&c).iter().enumerate() {
        let quiere = constantes(angulo_de_fotograma(f as u32), 1280.0 / 720.0).wvp;
        for k in 0..16 {
            assert_eq!(w[k].to_bits(), quiere[k].to_bits(), "fotograma {}, entrada {}: {} y bmo_cubo {}", f, k, w[k], quiere[k]);
        }
    }
}

/// ** LA TANDA DE LOS 360 FOTOGRAMAS: cada vertice a recorte, su subpixel y
/// si esta detras; las caras que miran (la misma cuenta que
/// `tanda::de_fotograma`, con los subpixeles de las gpu fn) y la luz de cada
/// una. Lo que sale es la tanda de `bmo_cubo`, bit a bit.
#[test]
fn the_360_frames_counted_by_gpu_fn_are_the_tanda_of_bmo_cubo() {
    let c = Cubo::nuevo();
    let ws = matrices(&c);
    let vs = vertices();
    let is = indices();
    // Cada vertice de cada fotograma, a recorte: una celda por (fotograma,
    // vertice, componente), con su fila de la matriz.
    let mut col: [Vec<f32>; 7] = Default::default();
    for w in &ws {
        for v in &vs {
            for fila in 0..4 {
                for (k, x) in [w[fila], w[4 + fila], w[8 + fila], w[12 + fila], v.pos[0], v.pos[1], v.pos[2]].into_iter().enumerate() {
                    col[k].push(x);
                }
            }
        }
    }
    let recorte: Vec<f32> = c.corre("cubo.recorte", &col.iter().map(|x| bits(x)).collect::<Vec<_>>()).into_iter().map(f32::from_bits).collect();
    let clip = |f: usize, i: usize| -> [f32; 4] { core::array::from_fn(|k| recorte[(f * 24 + i) * 4 + k]) };
    // Su subpixel y si esta detras.
    let (mut xs, mut ys, mut wz) = (Vec::new(), Vec::new(), Vec::new());
    for f in 0..360 {
        for i in 0..24 {
            let p = clip(f, i);
            xs.push(p[0]);
            ys.push(p[1]);
            wz.push(p[3]);
        }
    }
    let sx = c.corre("cubo.pantalla_x", &[bits(&xs), bits(&wz)]);
    let sy = c.corre("cubo.pantalla_y", &[bits(&ys), bits(&wz)]);
    let detras = c.corre("cubo.detras", &[bits(&wz)]);
    // La luz de la cara de cada triangulo, en cada fotograma: un canal por celda.
    let (mut ff, mut cc, mut n0, mut n1, mut n2) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for f in 0..360 {
        for k in 0..12 {
            let v = &vs[is[3 * k] as usize];
            for canal in 0..3 {
                ff.push(f as f32);
                cc.push(v.color[canal]);
                n0.push(v.normal[0]);
                n1.push(v.normal[1]);
                n2.push(v.normal[2]);
            }
        }
    }
    let color = c.corre("cubo.color", &[bits(&ff), bits(&cc), bits(&n0), bits(&n1), bits(&n2)]);
    let mut caras = 0;
    for f in 0..360 {
        let quiere = tanda::de_fotograma(f as u32, 1280, 720).expect("la tanda");
        let s = |i: usize| (f32::from_bits(sx[f * 24 + i]) as i64, f32::from_bits(sy[f * 24 + i]) as i64);
        let mut n = 0;
        for k in 0..12 {
            let i = [is[3 * k] as usize, is[3 * k + 1] as usize, is[3 * k + 2] as usize];
            if i.iter().any(|&j| detras[f * 24 + j] != 0) {
                continue;
            }
            let ((ax, ay), (bx, by), (px, py)) = (s(i[0]), s(i[1]), s(i[2]));
            if (bx - ax) * (py - ay) - (by - ay) * (px - ax) <= 0 {
                continue;
            }
            let t = &quiere.tris[n];
            assert!(n < quiere.n && quiere.caras[n] == k, "fotograma {}: el triangulo {} mira, y en bmo_cubo no", f, k);
            for (j, &v) in i.iter().enumerate() {
                assert_eq!(clip(f, v).map(f32::to_bits), t.clip[j].map(f32::to_bits), "fotograma {}, triangulo {}, vertice {}", f, k, j);
            }
            for canal in 0..3 {
                assert_eq!(color[(f * 12 + k) * 3 + canal], t.color[canal].to_bits(), "fotograma {}, triangulo {}, canal {}", f, k, canal);
            }
            assert_eq!(vs[i[0]].color[3].to_bits(), t.color[3].to_bits());
            n += 1;
        }
        assert_eq!(n, quiere.n, "fotograma {}: las caras que miran", f);
        caras += n;
    }
    // En una vuelta entera, el cubo muestra de dos a tres caras: entre 4 y 6
    // triangulos por fotograma.
    assert!(caras > 360 * 4 && caras <= 360 * 6, "{} triangulos", caras);
}
