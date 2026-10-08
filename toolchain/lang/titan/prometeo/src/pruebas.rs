//! El banco del escritor: cada regla con su prueba de NO. Con la 3060 DE
//! VERDAD (su emisor, su juez, su simulador), por el contrato: las leyes
//! L28-L31 son de ella. La tarjeta de juguete esta en `tests/juguete.rs`.

use super::*;
use bmo_proton_x_sm86::tarjeta::SM86;

/// El mismo codigo de la 3060 con las ESPERAS a cero (los bits 105..108 de
/// cada instruccion: 41..44 de su mitad alta, el byte 13): cada una leeria lo
/// de la anterior antes de que llegue.
fn sin_esperas(c: &mut Codigo) {
    for i in c.bytes.chunks_exact_mut(16) {
        i[8 + 5] &= !0x1E;
    }
}

fn module(src: &str) -> Module {
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    bmo_titan_front::lower_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

fn kernel(src: &str, name: &str) -> (Module, usize, Kernel<'static>) {
    let m = module(src);
    let f = m.functions.iter().position(|f| f.name == name).unwrap();
    let k = write(&m, f, &SM86).unwrap_or_else(|e| panic!("{:?}", e));
    (m, f, k)
}

fn f32s(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

const MEZCLA: &str = "mod main \"x\"\ngpu fn mezcla(a: f32, b: f32) -> f32\n    return (a + b) / 2.0\nfn main()\n    let xs: [f32; 2] = [1.0, 2.0]\n    let r = mezcla(xs, xs)\n    print(round(r[0], 1))\n";

/// ** La cadena entera: el Programa, el SASS, el juez que dice que si, y la
/// 3060 simulada que da los mismos bits que el f32 de Rust.
#[test]
fn a_gpu_fn_becomes_sass_the_judge_accepts_and_the_3060_agrees_bit_by_bit() {
    let (m, f, k) = kernel(MEZCLA, "mezcla");
    judge(&k).unwrap_or_else(|e| panic!("{}", e));
    let a = [1.0f32, 0.1, -3.5, 1e30, 7.25];
    let b = [3.0f32, 0.2, 2.0, 1e30, -0.0];
    let cells = [f32s(&a), f32s(&b)];
    let want: Vec<u32> = a.iter().zip(&b).map(|(x, y)| ((x + y) / 2.0).to_bits()).collect();
    assert_eq!(run(&k, &cells).unwrap(), want, "la 3060 simulada");
    assert_eq!(run_casa(&k, &cells), want, "la casa");
    assert!(verify(&m, f, &k).unwrap() >= 300, "la bateria entera");
    // Sin SPIR-V y sin division: `/ 2.0` es `* 0.5`.
    assert!(!k.programa.ops.iter().any(|o| matches!(o, Op::Div { .. })));
    assert!(!k.bytes().is_empty() && k.bytes().len() % 16 == 0);
}

/// ** Un `if` con sus `return`, en linea recta: los mismos resultados que
/// los saltos, en cada celda, y ni un `Si` en el Programa.
#[test]
fn an_if_becomes_an_elige_and_gives_the_same_cells() {
    let src = "mod main \"x\"\ngpu fn activa(x: f32) -> f32\n    if x > 0.0\n        return x\n    let y = x * 0.5\n    if y < -1.0\n        return -1.0\n    else\n        return y\nfn main()\n    let xs: [f32; 1] = [1.0]\n    let r = activa(xs)\n    print(round(r[0], 1))\n";
    let (m, f, k) = kernel(src, "activa");
    judge(&k).unwrap_or_else(|e| panic!("{}", e));
    let xs = [2.5f32, 0.0, -1.0, -2.0, -4.0, f32::NAN, -0.0];
    let rust = |x: f32| if x > 0.0 { x } else { let y = x * 0.5; if y < -1.0 { -1.0 } else { y } };
    let want: Vec<u32> = xs.iter().map(|x| rust(*x).to_bits()).collect();
    let got = run(&k, &[f32s(&xs)]).unwrap();
    for (g, w) in got.iter().zip(&want) {
        assert!(same_cell(*g, *w, Kind::F32), "{:?} {:?}", f32::from_bits(*g), f32::from_bits(*w));
    }
    verify(&m, f, &k).unwrap();
    assert!(!k.programa.ops.iter().any(|o| matches!(o, Op::Si { .. } | Op::Bucle)));
    assert!(k.programa.ops.iter().any(|o| matches!(o, Op::Elige { .. })));
}

#[test]
fn a_bool_travels_as_one_or_zero() {
    let src = "mod main \"x\"\ngpu fn grande(x: f32, flojo: bool) -> bool\n    return x > 10.0 and not flojo\nfn main()\n    let xs: [f32; 1] = [1.0]\n    print(1)\n";
    let (m, f, k) = kernel(src, "grande");
    judge(&k).unwrap_or_else(|e| panic!("{}", e));
    let cells = [f32s(&[20.0, 20.0, 5.0, f32::NAN]), vec![0, 1, 0, 0]];
    assert_eq!(run(&k, &cells).unwrap(), vec![1, 0, 0, 0]);
    assert_eq!(run_casa(&k, &cells), vec![1, 0, 0, 0]);
    verify(&m, f, &k).unwrap();
}

/// ** El signo de `-x` por sus BITS: -0 da +0, y +0 da -0 (con `0 - x`, +0
/// daria +0: la prueba del NO del atajo).
#[test]
fn the_sign_flips_by_its_bits() {
    let src = "mod main \"x\"\ngpu fn menos(x: f32) -> f32\n    return -x\nfn main()\n    print(1)\n";
    let (m, f, k) = kernel(src, "menos");
    judge(&k).unwrap();
    let got = run(&k, &[f32s(&[0.0, -0.0, 1.5, f32::INFINITY])]).unwrap();
    assert_eq!(got, f32s(&[-0.0, 0.0, -1.5, f32::NEG_INFINITY]));
    assert_ne!((0.0f32 - 0.0).to_bits(), (-0.0f32).to_bits(), "0 - x no es -x en el cero");
    verify(&m, f, &k).unwrap();
}

/// ** EL ORACULO COMO `Device`: el calculo no corre la gpu fn; la escribe,
/// la juzga y la corre la 3060 simulada -- y el programa escribe lo mismo.
#[test]
fn the_oracle_runs_the_gpu_fn_and_the_program_writes_the_same() {
    let src = "mod main \"x\"\ngpu fn mezcla(a: f32, b: f32) -> f32\n    return (a + b) * 0.25 - a\nfn main()\n    let xs: [f32; 3] = [1.0, 0.1, -7.5]\n    let ys: [f32; 3] = [2.0, 0.2, 1.25]\n    let r = mezcla(xs, ys)\n    for x in r\n        print(round(x, 9))\n";
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    let mut read = |p: &str| (p == "Titan.toml").then(|| toml.to_string());
    let plain = bmo_titan_front::lower_package("src/main.titan", src, &mut read).unwrap();
    let mut oracle = Oracle::new(&[&SM86]);
    let ran = bmo_titan_front::lower_package_with("src/main.titan", src, &mut read, Some(&mut oracle)).unwrap();
    assert_eq!(oracle.written(), 1, "the oracle wrote, judged and ran the gpu fn");
    assert_eq!(plain.flat, ran.flat);
}

/// ** LA DIVISION GENERAL, la prueba del NO: la de la 3060 no es la exacta,
/// y el NO dice la linea y la columna de la division, y por que.
#[test]
fn a_general_division_is_refused_where_it_is_written() {
    let src = "mod main \"x\"\ngpu fn tercio(a: f32) -> f32\n    return a / 3.0\nfn main()\n    print(1)\n";
    let m = module(src);
    let f = m.functions.iter().position(|f| f.name == "tercio").unwrap();
    let why = write(&m, f, &SM86).err().expect("la division entre 3 no se escribe").0;
    assert!(why.starts_with("src/main.titan, linea 3, columna 14 (gpu fn `tercio`)"), "{}", why);
    assert!(why.contains("LI2g"), "{}", why);
}

/// ** UN LIMITE ES EL NO DEL PROGRAMA (LB1 de PLAN_LAS_LIBRERIAS, 08-10): la
/// division general llega al calculo como SU NO, en la linea y la columna de
/// la division (no en la de la llamada), con su porque y su como -- nunca como
/// "un fallo del escritor ... avisa con este programa". Y `kernels()` dice lo
/// mismo de una gpu fn que nadie llama.
#[test]
fn a_limit_of_the_library_is_the_program_s_no_where_it_is_written() {
    let src = "mod main \"x\"\ngpu fn tercio(a: f32) -> f32\n    return a / 3.0\nfn main()\n    let xs: [f32; 1] = [1.0]\n    let r = tercio(xs)\n    print(round(r[0], 2))\n";
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    let mut oracle = Oracle::new(&[&SM86]);
    let no = bmo_titan_front::lower_package_with("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string()), Some(&mut oracle)).unwrap_err();
    assert_eq!((no.code, no.line, no.col), (bmo_titan_front::Code::GpuBody, 3, 14), "{:?}", no);
    assert!(no.why.contains("LI2g") && !no.why.contains("fallo"), "{}", no.why);
    assert!(no.how.contains("potencia de dos"), "{}", no.how);
    // Nadie la llama: el calculo no la ve, y kernels() la dice igual.
    let m = module("mod main \"x\"\ngpu fn tercio(a: f32) -> f32\n    return a / 3.0\nfn main()\n    print(1)\n");
    match kernels(&m, &[&SM86]) {
        Err(DeviceNo::Limit(said)) => assert_eq!((said.code, said.line, said.col), (bmo_titan_front::Code::GpuBody, 3, 14)),
        other => panic!("el limite, no {:?}", other.map(|k| k.len())),
    }
    // Y lo que no es un limite sigue siendo un fallo: el SASS sin sus esperas.
    let (_, _, mut k) = kernel(MEZCLA, "mezcla");
    sin_esperas(&mut k.oraculo);
    assert!(judge(&k).is_err());
}

/// ** EN UN PAQUETE (08-10): una gpu fn de un modulo HIJO dice su NO en SU
/// fichero y en SU linea. Antes el texto del escritor ponia el nombre del
/// fichero con la linea del paquete entero, que solo cuadra en la raiz.
#[test]
fn a_limit_in_a_child_module_is_said_in_its_file_and_its_line() {
    let main = "mod main \"x\"\nmod mates\nfn main()\n    let xs: [f32; 1] = [1.0]\n    let r = mates.tercio(xs)\n    print(round(r[0], 2))\n";
    let hijo = "mod mates \"cuentas de la 3060\"\n\npub gpu fn tercio(a: f32) -> f32\n    return a / 3.0\n";
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    let mut read = |p: &str| match p {
        "Titan.toml" => Some(toml.to_string()),
        "src/mates.titan" => Some(hijo.to_string()),
        _ => None,
    };
    let mut oracle = Oracle::new(&[&SM86]);
    let no = bmo_titan_front::lower_package_with("src/main.titan", main, &mut read, Some(&mut oracle)).unwrap_err();
    assert_eq!((no.file.as_deref(), no.line, no.col, no.code), (Some("src/mates.titan"), 4, 14, bmo_titan_front::Code::GpuBody), "{:?}", no);
    let m = bmo_titan_front::lower_package("src/main.titan", main, &mut read).unwrap();
    let f = m.functions.iter().position(|f| f.gpu).unwrap();
    let why = write(&m, f, &SM86).err().expect("la division entre 3 no se escribe").0;
    assert!(why.starts_with("src/mates.titan, linea 4, columna 14"), "{}", why);
}

/// ** Solo las potencias de dos tienen inverso EXACTO, y con ellas `x / c` y
/// `x * (1 / c)` dan los mismos bits en toda la bateria.
#[test]
fn only_a_power_of_two_has_an_exact_inverse() {
    for c in [2.0f32, 0.5, -4.0, 1024.0, 1.0, 2.0f32.powi(126)] {
        let inv = f32::from_bits(inverso_exacto(c.to_bits()).unwrap_or_else(|| panic!("{c}")));
        for x in BORDES {
            let (a, b) = (x / c, x * inv);
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{x} / {c}");
        }
    }
    for c in [3.0f32, 0.1, 0.0, -0.0, f32::INFINITY, f32::NAN, 1.0e-45, 2.0f32.powi(127)] {
        assert_eq!(inverso_exacto(c.to_bits()), None, "{c}");
    }
    // NO: 3 no es potencia de dos, y su inverso NO da los mismos bits.
    let x = 5.0f32;
    assert_ne!((x / 3.0).to_bits(), (x * (1.0 / 3.0)).to_bits());
}

/// ** SI ESTA BIEN: una cuenta que la 3060 y el calculo no dan igual se
/// ATRAPA (aqui, el SASS de una gpu fn medido contra el calculo de otra).
#[test]
fn a_disagreement_is_caught_with_its_input_and_the_answers() {
    assert_eq!(battery(&[Kind::F32, Kind::F32])[0].len(), BORDES.len() * BORDES.len());
    assert_eq!(battery(&[Kind::F32, Kind::Bool])[0].len(), BORDES.len() * 2);
    let src = "mod main \"x\"\ngpu fn suma(a: f32, b: f32) -> f32\n    return a + b\ngpu fn resta(a: f32, b: f32) -> f32\n    return a - b\nfn main()\n    print(1)\n";
    let m = module(src);
    let suma = m.functions.iter().position(|f| f.name == "suma").unwrap();
    let resta = m.functions.iter().position(|f| f.name == "resta").unwrap();
    let k = write(&m, suma, &SM86).unwrap();
    assert!(verify(&m, suma, &k).is_ok());
    let why = verify(&m, resta, &k).unwrap_err();
    assert!(why.contains("la 3060 da") && why.contains("el calculo") && why.contains("no hay .bex"), "{}", why);
}

/// ** EL JUEZ ES ESTRICTO, y se le pregunta de verdad: el mismo SASS con las
/// esperas a cero (cada instruccion leeria lo de la anterior antes de que
/// llegue) y el juez dice que no.
#[test]
fn the_judge_refuses_a_sass_without_its_waits() {
    let (_, _, mut k) = kernel(MEZCLA, "mezcla");
    judge(&k).unwrap();
    sin_esperas(&mut k.oraculo);
    let why = judge(&k).unwrap_err();
    // DONDE: en el .titan, la linea de la gpu fn; nunca "instruccion 3".
    assert!(why.starts_with("src/main.titan, linea 2 (gpu fn `mezcla`): el juez de la 3060 dijo que no"), "{}", why);
}
