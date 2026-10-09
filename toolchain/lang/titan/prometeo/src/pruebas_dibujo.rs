//! El banco de LB6: la gpu fn que DIBUJA, de vertice y de pixel, hecha el
//! Programa de la casa por elementos y componentes, en LAS DOS tarjetas que
//! da `titan` (la RTX 3060 12G y la CPU), contra la casa y el calculo -- y a
//! mano, que tres que se ponen de acuerdo tambien pueden estar mal los tres.

use super::*;
use bmo_tarjeta_cpu::CPU;
use bmo_tarjeta_rtx3060_12g::RTX_3060_12G;

fn module(src: &str) -> Module {
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    bmo_titan_front::lower_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

fn pos(m: &Module, name: &str) -> usize {
    m.functions.iter().position(|f| f.name == name).unwrap()
}

/// Un elemento de cuatro f32, en bits.
fn e4(x: [f32; 4]) -> [u32; 4] {
    x.map(f32::to_bits)
}

/// Las dos tarjetas, en su orden.
fn tarjetas() -> [&'static dyn Tarjeta; 2] {
    [&RTX_3060_12G, &CPU]
}

/// Los (elemento, componente) de las entradas o de las salidas del Programa.
fn lee(p: &Programa) -> Vec<(u8, u8)> {
    p.ops.iter().filter_map(|o| if let Op::Entrada { elemento, componente, .. } = o { Some((*elemento, *componente)) } else { None }).collect()
}

fn deja(p: &Programa) -> Vec<(u8, u8)> {
    p.ops.iter().filter_map(|o| if let Op::Salida { elemento, componente, .. } = o { Some((*elemento, *componente)) } else { None }).collect()
}

const TIPOS: &str = "mod main \"x\"\n\ntype Cuatro\n    x: f32\n    y: f32\n    z: f32\n    w: f32\n\n";

/// Las dos del cubo de V0: la de vertice deja lo que le dan; la de pixel, el color.
const CUBO: &str = "type Vertice\n    posicion: Cuatro\n    color: Cuatro\n\ngpu fn vertice(v: Vertice) -> Vertice\n    return v\n\ngpu fn pixel(v: Vertice) -> Cuatro\n    return v.color\n\nfn main()\n    print(1)\n";

/// ** LAS DOS DEL CUBO, por su firma: la de vertice lee sus dos elementos
/// (cuatro componentes cada uno) y los deja; la de pixel no lee su posicion
/// -- ni una `Entrada` del elemento 0 -- y deja su color en el elemento 0. En
/// las dos tarjetas: escritas, juzgadas y con la bateria entera pasada; y a
/// mano, en la 3060: lo que entra sale.
#[test]
fn the_cube_s_two_gpu_fns_are_programs_by_elements_on_both_cards() {
    let m = module(&format!("{}{}", TIPOS, CUBO));
    let (fv, fp) = (pos(&m, "vertice"), pos(&m, "pixel"));
    assert_eq!((forma(&m, fv), forma(&m, fp)), (Forma::Vertice { posicion: 0 }, Forma::Pixel { posicion: 0 }));
    let (pv, _) = programa(&m, fv).unwrap();
    let todo: Vec<(u8, u8)> = (0..2).flat_map(|e| (0..4).map(move |c| (e, c))).collect();
    assert_eq!((lee(&pv), deja(&pv)), (todo.clone(), todo));
    assert_eq!((pv.entradas, pv.salidas, pv.lee), (2, 2, 0b11));
    let (pp, _) = programa(&m, fp).unwrap();
    assert_eq!(lee(&pp), (0..4).map(|c| (1, c)).collect::<Vec<_>>(), "la posicion no se lee");
    assert_eq!(deja(&pp), (0..4).map(|c| (0, c)).collect::<Vec<_>>());
    assert_eq!((pp.entradas, pp.salidas, pp.lee), (2, 1, 0b10));
    for t in tarjetas() {
        for f in [fv, fp] {
            let k = write(&m, f, t).unwrap_or_else(|e| panic!("{}: {:?}", t.ficha().nombre, e));
            judge(&k).unwrap_or_else(|e| panic!("{}", e));
            assert_eq!(verify(&m, f, &k).unwrap(), 4096, "{} en {}: la bateria entera", k.name, t.ficha().nombre);
        }
    }
    let celda = vec![e4([1.0, -2.5, 0.1, 1.0]), e4([0.25, 0.5, 0.75, 1.0])];
    let k = write(&m, fv, &RTX_3060_12G).unwrap();
    let d = k.dibujo.clone().unwrap();
    assert_eq!((d.entra.as_str(), d.sale.as_str(), d.entradas.clone(), d.salidas.clone(), d.posicion()), ("Vertice", "Vertice", vec![4, 4], vec![4, 4], 0));
    assert_eq!(dibuja::run(&k, &d, &[celda.clone()]).unwrap(), vec![celda.clone()]);
    let k = write(&m, fp, &RTX_3060_12G).unwrap();
    let d = k.dibujo.clone().unwrap();
    assert_eq!(dibuja::run(&k, &d, &[celda.clone()]).unwrap(), vec![vec![celda[1]]]);
}

/// Uno que trabaja: un campo de un f32 (y sus otros tres componentes), un
/// campo que cambia (`w.posicion.x = ...`), un `if` que cambia un registro
/// entero, un bucle con su `break` que llama a otra gpu fn (EN LINEA), y un
/// pixel que vuelve desde dentro de su bucle.
const TRABAJA: &str = "type Vertice\n    posicion: Cuatro\n    color: Cuatro\n    brillo: f32\n\ngpu fn medio(a: f32, b: f32) -> f32\n    return (a + b) / 2.0\n\ngpu fn vertice(v: Vertice) -> Vertice\n    let mut w = v\n    w.posicion.x = v.posicion.x * 0.5\n    if v.brillo > 1.0\n        w.color = Cuatro { x: 1.0, y: 1.0, z: 1.0, w: 1.0 }\n    let mut s = 0.0\n    for i in range(4)\n        s = s + medio(v.color.x, v.color.y)\n        if s > 3.0\n            break\n    w.brillo = s\n    return w\n\ngpu fn pixel(v: Vertice) -> Cuatro\n    let mut c = v.color\n    for i in range(3)\n        if v.brillo > 2.0\n            return Cuatro { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }\n        c.x = c.x * v.brillo\n    return c\n\nfn main()\n    print(1)\n";

/// ** A MANO, en las dos tarjetas y en la casa: lo que deja cada una,
/// componente a componente -- tambien los ceros de un campo de un f32 -- y la
/// bateria entera contra el calculo.
#[test]
fn a_gpu_fn_that_works_draws_the_bits_worked_out_by_hand() {
    let m = module(&format!("{}{}", TIPOS, TRABAJA));
    let (fv, fp) = (pos(&m, "vertice"), pos(&m, "pixel"));
    assert_eq!((forma(&m, fv), forma(&m, fp)), (Forma::Vertice { posicion: 0 }, Forma::Pixel { posicion: 0 }));
    let posicion = e4([2.0, 3.0, 4.0, 1.0]);
    let color = e4([1.0, 2.0, 0.5, 1.0]);
    let brillo = |b: f32| [b.to_bits(), 0, 0, 0];
    // brillo 0.5: el color se queda; s = 1.5, 3.0, 4.5 -> break.
    let tenue = vec![posicion, color, brillo(0.5)];
    let tenue_v = vec![e4([1.0, 3.0, 4.0, 1.0]), color, brillo(4.5)];
    // brillo 2.0: el color pasa a blanco.
    let fuerte = vec![posicion, color, brillo(2.0)];
    let fuerte_v = vec![e4([1.0, 3.0, 4.0, 1.0]), e4([1.0; 4]), brillo(4.5)];
    // El de pixel: tres vueltas de c.x * 0.5; con brillo 3.0, vuelve negro en la primera.
    let tenue_p = vec![e4([0.125, 2.0, 0.5, 1.0])];
    let oscuro = vec![posicion, color, brillo(3.0)];
    let oscuro_p = vec![e4([0.0, 0.0, 0.0, 1.0])];
    for t in tarjetas() {
        let k = write(&m, fv, t).unwrap_or_else(|e| panic!("{}: {:?}", t.ficha().nombre, e));
        judge(&k).unwrap_or_else(|e| panic!("{}", e));
        let d = k.dibujo.clone().unwrap();
        assert_eq!(d.salidas, vec![4, 4, 1]);
        let celdas = [tenue.clone(), fuerte.clone()];
        assert_eq!(dibuja::run(&k, &d, &celdas).unwrap(), vec![tenue_v.clone(), fuerte_v.clone()], "{}", t.ficha().nombre);
        assert_eq!(dibuja::run_casa(&k, &d, &celdas), vec![tenue_v.clone(), fuerte_v.clone()], "la casa");
        assert_eq!(verify(&m, fv, &k).unwrap(), 4096);
        let k = write(&m, fp, t).unwrap_or_else(|e| panic!("{}: {:?}", t.ficha().nombre, e));
        judge(&k).unwrap_or_else(|e| panic!("{}", e));
        let d = k.dibujo.clone().unwrap();
        let celdas = [tenue.clone(), oscuro.clone()];
        assert_eq!(dibuja::run(&k, &d, &celdas).unwrap(), vec![tenue_p.clone(), oscuro_p.clone()], "{}", t.ficha().nombre);
        assert_eq!(dibuja::run_casa(&k, &d, &celdas), vec![tenue_p.clone(), oscuro_p.clone()], "la casa");
        assert!(verify(&m, fp, &k).unwrap() >= 18);
    }
    // Y el calculo, lo mismo.
    assert_eq!(bmo_titan_front::calc::run_gpu_dibujo(&m, fv, &[tenue, fuerte]).unwrap(), vec![tenue_v, fuerte_v]);
}

/// ** LA POSICION DEL DE PIXEL ES DE LA TARJETA: lo que haya en ella no
/// cambia lo que pinta -- ni un NaN.
#[test]
fn the_pixel_never_reads_its_position() {
    let m = module(&format!("{}{}", TIPOS, TRABAJA));
    let fp = pos(&m, "pixel");
    let color = e4([1.0, 2.0, 0.5, 1.0]);
    let celdas = [vec![e4([0.0; 4]), color, [0.5f32.to_bits(), 0, 0, 0]], vec![e4([f32::NAN, f32::INFINITY, -0.0, 7.0]), color, [0.5f32.to_bits(), 0, 0, 0]]];
    for t in tarjetas() {
        let k = write(&m, fp, t).unwrap();
        let d = k.dibujo.clone().unwrap();
        let r = dibuja::run(&k, &d, &celdas).unwrap();
        assert_eq!(r[0], r[1], "{}", t.ficha().nombre);
        assert!(!lee(&k.programa).iter().any(|(e, _)| *e == 0));
    }
}

/// ** Una cuenta, varias respuestas: si la casa no dice lo que dice la
/// tarjeta, no hay `.bex`, y el NO dice con que entrada y las tres
/// respuestas.
#[test]
fn a_disagreement_in_what_draws_is_caught_with_its_input() {
    let m = module(&format!("{}{}", TIPOS, CUBO));
    let fv = pos(&m, "vertice");
    let mut k = write(&m, fv, &RTX_3060_12G).unwrap();
    // La casa deja el color en el sitio de la posicion.
    let s4 = k.programa.ops.iter().find_map(|o| if let Op::Salida { s, elemento: 1, componente: 0, .. } = o { Some(*s) } else { None }).unwrap();
    for o in k.programa.ops.iter_mut() {
        if let Op::Salida { s, elemento: 0, componente: 0, .. } = o {
            *s = s4;
        }
    }
    let no = verify(&m, fv, &k).unwrap_err();
    assert!(no.contains("main.titan, linea 13 (gpu fn `vertice`)") && no.contains("la casa") && no.contains("no hay .bex"), "{}", no);
}

/// ** Lo que dibuja no se llama: si llegara al oraculo del calculo, la red lo
/// dice (el calculo ya lo dijo antes, `gpu_call`).
#[test]
fn a_gpu_fn_that_draws_is_never_run_as_cells() {
    let m = module(&format!("{}{}", TIPOS, CUBO));
    let mut o = Oracle::new(&[&RTX_3060_12G]);
    let no = bmo_titan_front::calc::Device::run(&mut o, &m, pos(&m, "vertice"), vec![vec![0]]).unwrap_err();
    assert!(matches!(&no, DeviceNo::Failure(why) if why.contains("dibuja: no se llama")), "{:?}", no);
}
