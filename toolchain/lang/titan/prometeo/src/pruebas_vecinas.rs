//! LB9 de `docs/plan/PLAN_LAS_LIBRERIAS.md`: la gpu fn que mira a sus
//! VECINAS, un COMPUTO, en las DOS tarjetas que da `titan` -- la 3060 y la
//! CPU, la reserva (L32) --, contra la casa, el calculo y un espejo en Rust;
//! y lo de fuera de la lista, 0 (DL9).

use super::*;
use bmo_prometeo::programa::Op;
use bmo_tarjeta_cpu::CPU;
use bmo_tarjeta_rtx3060_12g::RTX_3060_12G;

fn module(src: &str) -> Module {
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    bmo_titan_front::lower_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

/// La gpu fn `name`, escrita, juzgada y pasada por su bateria de vecinas en
/// CADA tarjeta.
fn en_las_dos(m: &Module, name: &str) -> Vec<Kernel<'static>> {
    let f = m.functions.iter().position(|f| f.name == name).unwrap();
    let tarjetas: [&'static dyn Tarjeta; 2] = [&RTX_3060_12G, &CPU];
    tarjetas
        .iter()
        .map(|t| {
            let k = write(m, f, *t).unwrap_or_else(|e| panic!("{}: {}", t.ficha().nombre, e.0));
            judge(&k).unwrap_or_else(|e| panic!("{}", e));
            verify(m, f, &k).unwrap_or_else(|e| panic!("{}", e));
            k
        })
        .collect()
}

fn f32s(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

const SUAVIZA: &str = "mod main \"x\"\ngpu fn suaviza(x: f32, campo: [f32]) -> f32\n    let i = celda()\n    return (campo[i - 1.0] + 2.0 * x + campo[i + 1.0]) * 0.25\nfn main()\n    print(1)\n";

/// ** EL DESENFOQUE: cada celda con sus dos vecinas, en las dos tarjetas y
/// en la casa con los bits de Rust -- la de antes de la primera y la de
/// despues de la ultima, 0 --, y mas de un grupo de hilos.
#[test]
fn a_blur_reads_its_neighbours_on_both_cards_and_outside_is_zero() {
    let m = module(SUAVIZA);
    let ks = en_las_dos(&m, "suaviza");
    let xs: Vec<f32> = (0..150).map(|k| (k * k) as f32 * 0.5 - 7.0).collect();
    let at = |i: i64| if i < 0 || i >= xs.len() as i64 { 0.0 } else { xs[i as usize] };
    let want: Vec<u32> = (0..xs.len() as i64).map(|i| ((at(i - 1) + 2.0 * at(i) + at(i + 1)) * 0.25).to_bits()).collect();
    let cells = [f32s(&xs)];
    let enteras = [f32s(&xs)];
    for k in &ks {
        assert_eq!(k.vecinas, Some(1));
        assert!(k.programa.ops.iter().any(|o| matches!(o, Op::IdHilo { .. })), "un computo: {:?}", k.programa.ops);
        for (que, got) in [("su simulador", run_vecinas(k, &cells, &enteras).unwrap()), ("la casa", run_casa_vecinas(k, &cells, &enteras))] {
            assert_eq!(got.len(), want.len(), "{} ({})", k.tarjeta.ficha().nombre, que);
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                assert_eq!(g, w, "{} ({}), celda {}: {:?} y Rust {:?}", k.tarjeta.ficha().nombre, que, i, f32::from_bits(*g), f32::from_bits(*w));
            }
        }
    }
}

/// ** EL CALCULO como `Device` (lo que hace `titan build`): el oraculo da las
/// celdas del programa, las mismas en las dos tarjetas.
#[test]
fn the_oracle_runs_a_neighbour_fn_for_the_calculation() {
    let src = "mod main \"x\"\ngpu fn suaviza(x: f32, campo: [f32]) -> f32\n    let i = celda()\n    return (campo[i - 1.0] + 2.0 * x + campo[i + 1.0]) * 0.25\nfn main()\n    let mut xs: [f32] = []\n    for k in range(8)\n        let v: f32 = k * k\n        push(mut xs, v)\n    let ys = suaviza(xs, xs)\n    print(len(ys), \" \", round(ys[0], 2), \" \", round(ys[3], 2), \" \", round(ys[7], 2))\n";
    let m = module(src);
    let mut o = Oracle::new(&[&RTX_3060_12G, &CPU]);
    let folded = bmo_titan_front::calc::fold_with(&m, Some(&mut o)).unwrap_or_else(|e| panic!("{:?}", e));
    let shown = format!("{:?}", folded.flat);
    for want in ["\"8\"", "\"0.25\"", "\"9.50\"", "\"33.50\""] {
        assert!(shown.contains(want), "{want} in {shown}");
    }
    assert_eq!(o.written(), 1);
}

/// ** UN BOOL que llega celda a celda, una vecina lejana y un indice que no
/// es entero (`f32 as i32`, hacia el cero): las tres respuestas, iguales.
#[test]
fn a_bool_a_far_neighbour_and_a_fraction_agree() {
    let m = module("mod main \"x\"\ngpu fn mira(x: f32, b: bool, campo: [f32], otro: [f32]) -> f32\n    let i = celda()\n    if b\n        return campo[x] + otro[i * 2.0]\n    return campo[i + 0.75] - otro[0.0 - i - 0.5]\nfn main()\n    print(1)\n");
    let ks = en_las_dos(&m, "mira");
    assert_eq!(ks[0].vecinas, Some(2));
    assert_eq!(ks[0].params, vec![Kind::F32, Kind::Bool]);
}

/// ** EL NO DE R7: el codigo del computo, juzgado como el cuerpo de una
/// celda (sin los permisos del computo), no pasa en la 3060 -- lee y escribe
/// memoria, y un cuerpo de celdas no la ve --; con ellos, si. Y una tarjeta
/// que no sabe juzgar un computo lo DICE, no lo deja pasar.
#[test]
fn r7_still_says_no_to_memory_in_a_cell_body() {
    let m = module(SUAVIZA);
    let ks = en_las_dos(&m, "suaviza");
    let k = ks.iter().find(|k| k.tarjeta.ficha().nombre == RTX_3060_12G.ficha().nombre).unwrap();
    assert!(RTX_3060_12G.juzgar(&k.viaje, Para::Viaje).is_err(), "R7 deja un cuerpo de celdas que lee memoria");
    assert!(RTX_3060_12G.juzgar_computo(&k.programa, &k.viaje, Para::Viaje).is_ok());
    struct Ciega;
    impl Tarjeta for Ciega {
        fn ficha(&self) -> bmo_prometeo::Ficha {
            RTX_3060_12G.ficha()
        }
        fn emitir(&self, p: &bmo_prometeo::Programa, para: Para) -> Result<bmo_prometeo::Codigo, bmo_prometeo::NoEmite> {
            RTX_3060_12G.emitir(p, para)
        }
        fn juzgar(&self, c: &bmo_prometeo::Codigo, para: Para) -> Result<(), String> {
            RTX_3060_12G.juzgar(c, para)
        }
        fn simular(&self, c: &bmo_prometeo::Codigo, e: &[[u32; 4]], s: usize) -> Result<Vec<[u32; 4]>, String> {
            RTX_3060_12G.simular(c, e, s)
        }
    }
    static CIEGA: Ciega = Ciega;
    let f = m.functions.iter().position(|f| f.name == "suaviza").unwrap();
    let k = write(&m, f, &CIEGA).unwrap_or_else(|e| panic!("{}", e.0));
    assert!(judge(&k).unwrap_err().contains("todavia no juzga un computo"));
}

/// ** IL2: `piso(x)`, los bits de `f32::floor` en las dos tarjetas y en la
/// casa: los bordes (los dos ceros, los subnormales, los que no caben en el
/// entero, el infinito y el NaN) y un barrido alrededor de los enteros.
#[test]
fn piso_is_the_floor_of_rust_on_both_cards() {
    let m = module("mod main \"x\"\ngpu fn p(x: f32) -> f32\n    return piso(x)\nfn main()\n    print(1)\n");
    let f = m.functions.iter().position(|f| f.name == "p").unwrap();
    let mut xs: Vec<f32> = vec![0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5, 2.999_999_8, -2.999_999_8, 1.0e-45, -1.0e-45, f32::MIN_POSITIVE, -f32::MIN_POSITIVE];
    xs.extend([8388607.5, -8388607.5, 8388608.0, -8388608.0, 8388609.0, 1.0e10, -1.0e10, 2147483648.0, -2147483904.0, f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -f32::NAN]);
    for k in -40..40 {
        let b = (k as f32 * 0.37).to_bits();
        xs.extend([f32::from_bits(b), f32::from_bits(b.wrapping_add(1)), f32::from_bits(b.wrapping_sub(1))]);
    }
    let cells = [f32s(&xs)];
    for t in [&RTX_3060_12G as &'static dyn Tarjeta, &CPU] {
        let k = write(&m, f, t).unwrap_or_else(|e| panic!("{}", e.0));
        judge(&k).unwrap();
        verify(&m, f, &k).unwrap();
        for (que, got) in [("su simulador", run(&k, &cells).unwrap()), ("la casa", run_casa(&k, &cells))] {
            for (x, g) in xs.iter().zip(&got) {
                assert_eq!(*g, x.floor().to_bits(), "{} ({}): piso({:?}) = {:?}, Rust {:?}", t.ficha().nombre, que, x, f32::from_bits(*g), x.floor());
            }
        }
    }
}
