//! LB5 de `docs/plan/PLAN_LAS_LIBRERIAS.md`: los BUCLES de una gpu fn, en
//! las DOS tarjetas que da `titan` -- la 3060 y la CPU, la reserva (L32) --,
//! contra la casa, el calculo y un espejo en Rust; el tope de una celda
//! contra el presupuesto del simulador de cada una; y las pruebas del NO.

use super::*;
use bmo_tarjeta_rtx3060_12g::RTX_3060_12G;
use bmo_tarjeta_cpu::CPU;

fn module(src: &str) -> Module {
    let toml = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";
    bmo_titan_front::lower_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| toml.to_string())).unwrap_or_else(|e| panic!("{:?}", e))
}

fn tarjetas() -> [&'static dyn Tarjeta; 2] {
    [&RTX_3060_12G, &CPU]
}

/// La gpu fn `name`, escrita, juzgada y pasada por su bateria en CADA
/// tarjeta: su indice y sus kernels.
fn en_las_dos(m: &Module, name: &str) -> (usize, Vec<Kernel<'static>>) {
    let f = m.functions.iter().position(|f| f.name == name).unwrap();
    let ks = tarjetas()
        .iter()
        .map(|t| {
            let k = write(m, f, *t).unwrap_or_else(|e| panic!("{}: {}", t.ficha().nombre, e.0));
            judge(&k).unwrap_or_else(|e| panic!("{}", e));
            verify(m, f, &k).unwrap_or_else(|e| panic!("{}", e));
            k
        })
        .collect();
    (f, ks)
}

fn f32s(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Cada tarjeta y la casa dan, celda a celda, los bits del espejo en Rust.
fn como_rust(ks: &[Kernel], cells: &[Vec<u32>], want: &[u32]) {
    for k in ks {
        for (que, got) in [("su simulador", run(k, cells).unwrap()), ("la casa", run_casa(k, cells))] {
            for (i, (g, w)) in got.iter().zip(want).enumerate() {
                assert!(same_cell(*g, *w, k.ret), "{} ({}), celda {}: {:?} y Rust {:?}", k.tarjeta.ficha().nombre, que, i, f32::from_bits(*g), f32::from_bits(*w));
            }
        }
    }
}

const XS: [f32; 12] = [0.0, -0.0, 1.0, -1.0, 0.5, 2.0, -1.5, 3.0, 0.1, 1.0e-20, f32::MAX, f32::NAN];

/// ** UNA POTENCIA: `for i in range(5)`, un `Bucle` de verdad en las dos
/// tarjetas, con los bits de Rust; y en lo alto de la gpu fn, sin `Si`.
#[test]
fn a_power_loops_on_both_cards_with_the_bits_of_rust() {
    let m = module("mod main \"x\"\ngpu fn potencia(x: f32) -> f32\n    let mut r = 1.0\n    for i in range(5)\n        r = r * x\n    return r\nfn main()\n    print(1)\n");
    let (_, ks) = en_las_dos(&m, "potencia");
    let want: Vec<u32> = XS.iter().map(|&x| (0..5).fold(1.0f32, |r, _| r * x).to_bits()).collect();
    como_rust(&ks, &[f32s(&XS)], &want);
    let ops = &ks[0].programa.ops;
    assert!(ops.iter().any(|o| matches!(o, Op::Bucle)) && ops.iter().any(|o| matches!(o, Op::RomperSi { si_cero: true, .. })) && ops.iter().any(|o| matches!(o, Op::FinBucle)));
    assert!(!ops.iter().any(|o| matches!(o, Op::Si { .. })), "en lo alto, el bucle va sin su `Si`");
}

/// ** UN NOMBRE QUE LEE LO VIEJO de otro que cambia en la vuelta (09-10):
/// `let viejo = s`, despues `s = s + 1.0` -- en la vuelta y dentro de un
/// `if` -- y `viejo` sigue siendo lo de antes. La casa de `s` se pisaba en
/// su sitio y `viejo` la leia: el calculo daba 3 y las tarjetas 6 (el
/// oraculo lo paraba y no habia .bex).
#[test]
fn a_name_keeps_the_old_value_of_one_that_changes_in_the_turn() {
    let m = module("mod main \"x\"\ngpu fn suma(x: f32) -> f32\n    let mut s = x\n    let mut r = 0.0\n    for i in range(3)\n        let viejo = s\n        s = s + 1.0\n        if x > 0.0\n            let otro = s\n            s = s * 2.0\n            r = r + otro\n        r = r + viejo\n    return r\nfn main()\n    print(1)\n");
    let (_, ks) = en_las_dos(&m, "suma");
    let rust = |x: f32| {
        let (mut s, mut r) = (x, 0.0f32);
        for _ in 0..3 {
            let viejo = s;
            s = s + 1.0;
            if x > 0.0 {
                let otro = s;
                s = s * 2.0;
                r = r + otro;
            }
            r = r + viejo;
        }
        r
    };
    let want: Vec<u32> = XS.iter().map(|&x| rust(x).to_bits()).collect();
    como_rust(&ks, &[f32s(&XS)], &want);
}

/// ** UNA SERIE con sus coeficientes ESCRITOS (todos 1.0: la de 1 / (1 - x))
/// y `e` por cuadrados, `(1 + x / 1024)^1024` -- sin dividir: `/ 1024` es
/// por su inverso, exacto.
#[test]
fn a_series_with_its_coefficients_written_and_e_by_squares() {
    let src = "mod main \"x\"\ngpu fn serie(x: f32) -> f32\n    let mut s = 0.0\n    let mut t = 1.0\n    for k in range(8)\n        s = s + t\n        t = t * x\n    return s\ngpu fn e(x: f32) -> f32\n    let mut y = 1.0 + x / 1024.0\n    for i in range(10)\n        y = y * y\n    return y\nfn main()\n    print(1)\n";
    let m = module(src);
    let (_, ks) = en_las_dos(&m, "serie");
    let want: Vec<u32> = XS
        .iter()
        .map(|&x| {
            let (mut s, mut t) = (0.0f32, 1.0f32);
            for _ in 0..8 {
                s += t;
                t *= x;
            }
            s.to_bits()
        })
        .collect();
    como_rust(&ks, &[f32s(&XS)], &want);
    let (_, ks) = en_las_dos(&m, "e");
    let want: Vec<u32> = XS.iter().map(|&x| (0..10).fold(1.0f32 + x * (1.0 / 1024.0), |y, _| y * y).to_bits()).collect();
    como_rust(&ks, &[f32s(&XS)], &want);
    // (1 + 1/1024)^1024 = 2.71695572..., y sus diez cuadrados en f32 se
    // quedan en 2.7169437: el redondeo de cada uno, a la vista.
    let e = f32::from_bits(run(&ks[0], &[f32s(&[1.0])]).unwrap()[0]);
    assert!((e - 2.7169557).abs() < 2.0e-5 && e != 2.7169557, "e, a f32: {}", e);
}

/// ** `continue`, `break` y `return` dentro de un bucle, un bucle dentro de
/// otro, y todo dentro de un `if` (su `Si`): en las dos tarjetas, con los bits
/// de Rust en cada celda.
#[test]
fn break_continue_and_return_inside_loops_inside_an_if() {
    let src = "mod main \"x\"\ngpu fn f(x: f32, flojo: bool) -> f32\n    let mut r = 0.0\n    let mut n = 0.0\n    if not flojo\n        for i in range(10)\n            if i == 2.0\n                continue\n            n = n + 1.0\n            for j in range(3)\n                r = r + x * j\n                if r > 100.0\n                    return r\n            if r < -50.0\n                break\n    return r + n\nfn main()\n    print(1)\n";
    let m = module(src);
    let (_, ks) = en_las_dos(&m, "f");
    let espejo = |x: f32, flojo: bool| -> f32 {
        let (mut r, mut n) = (0.0f32, 0.0f32);
        if !flojo {
            for i in 0..10 {
                if i as f32 == 2.0 {
                    continue;
                }
                n += 1.0;
                for j in 0..3 {
                    r += x * j as f32;
                    if r > 100.0 {
                        return r;
                    }
                }
                if r < -50.0 {
                    break;
                }
            }
        }
        r + n
    };
    let mut xs = Vec::new();
    let mut flojos = Vec::new();
    for &x in XS.iter().chain(&[5.0, -5.0, 2.5, -3.25, 100.0]) {
        for flojo in [false, true] {
            xs.push(x);
            flojos.push(flojo as u32);
        }
    }
    let want: Vec<u32> = xs.iter().zip(&flojos).map(|(x, b)| espejo(*x, *b != 0).to_bits()).collect();
    como_rust(&ks, &[f32s(&xs), flojos], &want);
    let ops = &ks[0].programa.ops;
    assert_eq!(ops.iter().filter(|o| matches!(o, Op::Bucle)).count(), 2);
    assert!(ops.iter().any(|o| matches!(o, Op::Si { .. })), "el bucle de dentro de un `if`, en su `Si`");
}

/// ** UNA DIVISION GENERAL DENTRO DE UN BUCLE (DL10, 09-10): hasta hoy era
/// el limite de la 3060 (LI2g), dicho en su linea y su columna, y solo la CPU
/// la hacia (`divss`). Ahora las DOS tarjetas dan los bits de Rust, vuelta a
/// vuelta -- tambien cuando el cociente se hace subnormal y cuando llega al
/// cero --.
#[test]
fn a_division_inside_a_loop_is_exact_on_both_cards() {
    let m = module("mod main \"x\"\ngpu fn tercios(x: f32) -> f32\n    let mut r = x\n    for i in range(3)\n        r = r / 3.0\n    return r\nfn main()\n    print(1)\n");
    let (_, ks) = en_las_dos(&m, "tercios");
    let xs: Vec<f32> = XS.iter().copied().chain([1.0e-40, 3.0e-45, -1.0e-38, 7.0, 1.0e38]).collect();
    let want: Vec<u32> = xs.iter().map(|&x| ((x / 3.0) / 3.0 / 3.0).to_bits()).collect();
    como_rust(&ks, &[f32s(&xs)], &want);
    assert!(ks[0].programa.ops.iter().any(|o| matches!(o, Op::Div { .. })));
}

/// ** SIN FIN: un Programa que no sale (escrito a mano: TITAN++ no sabe
/// escribirlo) es `SinFin` en el simulador de la 3060 y "no vuelve" en la
/// CPU, que ni lo emite -- nunca un cuelgue.
#[test]
fn an_endless_programa_is_said_by_each_simulator_never_a_hang() {
    let p = Programa {
        ops: vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Copia { d: 1, a: 0 },
            Op::Bucle,
            Op::Add { d: 2, a: 1, b: 0 },
            Op::Copia { d: 1, a: 2 },
            Op::FinBucle,
            Op::Salida { s: 1, elemento: 0, componente: 0 },
        ],
        iniciales: vec![0.0; 3],
        entradas: 1,
        salidas: 1,
        lee: 1,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    };
    let t = std::time::Instant::now();
    let sm86 = RTX_3060_12G.emitir(&p, Para::Oraculo).unwrap_or_else(|e| panic!("{:?}", e));
    let no = RTX_3060_12G.simular(&sm86, &[[1.0f32.to_bits(), 0, 0, 0]], 1).unwrap_err();
    assert!(no.contains("SinFin"), "{}", no);
    // La CPU lo dice antes: al emitir mide sus instrucciones corriendo una
    // celda, y ahi ya no vuelve.
    match CPU.emitir(&p, Para::Oraculo) {
        Err(NoEmite::Fallo { por_que, .. }) => assert!(por_que.contains("no vuelve"), "{}", por_que),
        other => panic!("la CPU lo tenia que decir: {:?}", other.map(|c| c.instrucciones)),
    }
    assert!(t.elapsed().as_secs() < 120, "acotado: {:?}", t.elapsed());
}

/// Los pasos de la 3060 simulada en una celda.
fn pasos_3060(c: &Codigo, entradas: &[[u32; 4]]) -> usize {
    let codigo: Vec<(u64, u64)> = c.bytes.chunks_exact(16).map(|i| (u64::from_le_bytes(i[..8].try_into().unwrap()), u64::from_le_bytes(i[8..].try_into().unwrap()))).collect();
    let banco: Vec<u8> = entradas.iter().flat_map(|e| e.iter().flat_map(|x| x.to_le_bytes())).collect();
    let mut m = bmo_tarjeta_rtx3060_12g::isa::simula::Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
    bmo_tarjeta_rtx3060_12g::isa::simula::correr(&codigo, &mut m).unwrap()
}

/// ** EL TOPE DE UNA CELDA (65536 de obra, `gpu.rs`) cabe en el presupuesto
/// del simulador de cada tarjeta, con margen: la 3060 (4M instrucciones) por
/// ocho, la CPU (4M) por cuatro, en cada patron que mas cuesta -- `if`
/// anidados con una condicion que no cuesta nada, `break`, `return`, `not`.
/// Medido el 08-10: la 3060, hasta ~3.5 instrucciones por unidad; la CPU,
/// hasta ~12.3. 09-10 (DL10): y la division general, que pesa
/// `OBRA_DIVISION` (24) porque en la 3060 es una cuenta entera: con un
/// cociente SUBNORMAL en cada vuelta -- su camino mas largo --, la 3060 hace
/// ~3.6 por unidad.
#[test]
fn the_top_of_a_cell_fits_each_card_s_budget() {
    let tope = bmo_titan_front::gpu::OBRA_MAXIMA;
    let division = bmo_titan_front::gpu::OBRA_DIVISION;
    // La celda: 0.5; las divisiones, un subnormal (1e-40), que las lleva a
    // lo lento.
    let (x, sub) = (0.5f32, 1.0e-40f32);
    let patrones = [
        ("aritmetica", "    let mut r = 0.0\n", "        r = r * x + 1.0\n", "    return r\n", 3u64, x),
        ("si", "    let mut r = 0.0\n", "        if x > r\n            r = r + 1.0\n        else\n            r = r - 0.5\n", "    return r\n", 6, x),
        ("comparaciones", "    let mut b = false\n    let r = x\n", "        b = x > r and r < 1.0 or not (x == r)\n", "    if b\n        return 1.0\n    return r\n", 7, x),
        ("variables", "    let mut r = 0.0\n    let mut a = 0.0\n    let mut c = 0.0\n", "        a = c\n        c = r\n        r = a + 1.0\n", "    return r + a + c\n", 4, x),
        ("break", "    let mut r = 0.0\n", "        if r > 1000000.0\n            break\n        r = r + x\n", "    return r\n", 5, x),
        ("si_nombre", "    let mut r = 0.0\n    let q = x > 0.0\n", "        if q\n            r = r + 1.0\n", "    return r\n", 3, x),
        ("si_anidado", "    let mut r = 0.0\n    let q = x > 0.0\n", "        if q\n            if q\n                if q\n                    r = 1.0\n", "    return r\n", 4, x),
        ("vuelve", "    let mut r = 0.0\n", "        if r > 1000000.0\n            return r\n        r = r + x\n", "    return r\n", 5, x),
        ("no", "    let mut q = x > 0.0\n", "        q = not not not q\n", "    if q\n        return 1.0\n    return 0.0\n", 4, x),
        ("muchos_break", "    let mut r = 0.0\n    let q = x > 2.0\n", "        if q\n            break\n        if q\n            break\n        if q\n            break\n        r = r + 1.0\n", "    return r\n", 8, x),
        ("signo", "    let mut r = x\n", "        r = -r\n", "    return r\n", 2, x),
        ("division", "    let mut r = 0.0\n", "        r = x / 3.0\n", "    return r\n", 1 + division, sub),
        ("divisiones", "    let mut r = 0.0\n", "        r = x / 3.0 / 3.0 / 3.0 / 3.0\n", "    return r\n", 1 + 4 * division, sub),
    ];
    for (nombre, antes, cuerpo, fin, coste, celda) in patrones {
        let vueltas = (tope - 40) / (coste + 2);
        let src = format!("mod main \"x\"\ngpu fn f(x: f32) -> f32\n{}    for i in range({})\n{}{}fn main()\n    print(1)\n", antes, vueltas, cuerpo, fin);
        let m = module(&src);
        let func = m.functions.iter().position(|f| f.name == "f").unwrap();
        let obra = m.functions[func].obra;
        assert!(obra <= tope && obra > tope - 200, "{}: obra {}", nombre, obra);
        let entradas = [[celda.to_bits(), 0, 0, 0]];
        let k3 = write(&m, func, &RTX_3060_12G).unwrap_or_else(|e| panic!("{}: {}", nombre, e.0));
        let p3 = pasos_3060(&k3.oraculo, &entradas);
        assert!(p3 * 8 <= bmo_tarjeta_rtx3060_12g::isa::simula::PASOS_MAXIMOS, "{}: la 3060, {} pasos", nombre, p3);
        let kc = write(&m, func, &CPU).unwrap_or_else(|e| panic!("{}: {}", nombre, e.0));
        let pc = bmo_tarjeta_cpu::correr(&kc.oraculo, &entradas, 1).unwrap_or_else(|e| panic!("{}: {}", nombre, e)).pasos;
        assert!(pc * 4 <= bmo_tarjeta_cpu::PASOS as u64, "{}: la CPU, {} pasos", nombre, pc);
    }
}

/// ** LAS LLAMADAS ENTRE gpu fn, EN LINEA (LB5): una que limita, una que
/// suaviza llamando a la otra, una que llama dentro de su bucle, y una que
/// vuelve desde dentro de su bucle con una llamada en la pregunta. En las dos
/// tarjetas, con los bits de Rust; y el Programa no tiene ni una llamada:
/// todo esta escrito dentro.
#[test]
fn a_call_between_gpu_fns_is_written_in_line_on_both_cards() {
    let src = "mod main \"x\"\ngpu fn cuadrado(x: f32) -> f32\n    return x * x\ngpu fn limita(x: f32, a: f32, b: f32) -> f32\n    if x < a\n        return a\n    if x > b\n        return b\n    return x\ngpu fn suave(x: f32) -> f32\n    let t = limita(x, 0.0, 1.0)\n    return t * t * (3.0 - 2.0 * t)\ngpu fn a_la_ocho(x: f32) -> f32\n    let mut r = x\n    for i in range(3)\n        r = cuadrado(r)\n    return r\ngpu fn primero_mayor(x: f32) -> f32\n    for i in range(10)\n        if cuadrado(i) > x\n            return i\n    return -1.0\ngpu fn suma_de_ochos(x: f32) -> f32\n    let mut s = 0.0\n    for k in range(4)\n        s = s + a_la_ocho(x + k)\n    return s\nfn main()\n    print(1)\n";
    let m = module(src);
    let limita = |x: f32, a: f32, b: f32| if x < a { a } else if x > b { b } else { x };
    let suave = |x: f32| {
        let t = limita(x, 0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let ocho = |x: f32| (0..3).fold(x, |r, _| r * r);
    let primero = |x: f32| (0..10).map(|i| i as f32).find(|i| i * i > x).unwrap_or(-1.0);
    let sumas = |x: f32| (0..4).fold(0.0f32, |s, k| s + ocho(x + k as f32));
    let xs: Vec<f32> = XS.iter().copied().chain([0.25, 0.75, 5.0, -2.0, 50.0]).collect();
    let espejos: [(&str, &dyn Fn(f32) -> f32); 4] = [("suave", &suave), ("a_la_ocho", &ocho), ("primero_mayor", &primero), ("suma_de_ochos", &sumas)];
    for (name, espejo) in espejos {
        let (_, ks) = en_las_dos(&m, name);
        let want: Vec<u32> = xs.iter().map(|x| espejo(*x).to_bits()).collect();
        como_rust(&ks, &[f32s(&xs)], &want);
    }
    // `suma_de_ochos`: el bucle de `a_la_ocho` dentro del suyo, escrito en linea.
    let (_, ks) = en_las_dos(&m, "suma_de_ochos");
    assert_eq!(ks[0].programa.ops.iter().filter(|o| matches!(o, Op::Bucle)).count(), 2);
}

/// ** UNA DIVISION DENTRO DE UNA LLAMADA (DL10, 09-10): hasta hoy, el limite
/// de la 3060 dicho en la linea de la division, no en la de la llamada (eso
/// lo prueba ahora la tarjeta de juguete, `tests/juguete.rs`). Ahora la
/// gpu fn llamada se escribe en linea en las dos tarjetas, con los bits de
/// Rust.
#[test]
fn a_division_inside_a_called_gpu_fn_is_exact_on_both_cards() {
    let m = module("mod main \"x\"\ngpu fn tercio(x: f32) -> f32\n    return x / 3.0\ngpu fn f(x: f32) -> f32\n    return tercio(x) + 1.0\nfn main()\n    print(1)\n");
    let (_, ks) = en_las_dos(&m, "f");
    let want: Vec<u32> = XS.iter().map(|&x| (x / 3.0 + 1.0).to_bits()).collect();
    como_rust(&ks, &[f32s(&XS)], &want);
}
