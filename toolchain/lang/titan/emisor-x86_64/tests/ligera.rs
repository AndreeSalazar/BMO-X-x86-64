//! ** AF7 de `docs/plan/PLAN_TITAN_AFINADO.md` (16-10): LA FN LIGERA. Una fn
//! de 1 a 4 `int` copiados que da un `int` (o nada) recibe sus valores en
//! registros y devuelve el resultado en `rax` (`e1/llamada.rs`).
//!
//! La vara, la de siempre: el calculo (E0) y la maquina (E1) escriben LO
//! MISMO, o el mismo NO en la misma linea.

use bmo_lower::emu::{cargar_bex, run};

fn console(bex: Vec<u8>) -> (String, bool) {
    let m = run(cargar_bex(&bex).unwrap(), 50_000_000);
    (m.console.clone(), m.exited)
}

/// E0 y E1, sobre el mismo programa: lo mismo, o el mismo NO.
fn same(body: &str) -> Result<String, String> {
    let src = format!("mod main \"ligera\"\n\n{body}");
    let e1 = match bmo_titan_x86_64::build_package_e1("ligera.titan", &src, &mut |_| None) {
        Ok(bex) => console(bex),
        Err(e) => return Err(format!("E1 no lo emite: {e:?}\n{src}")),
    };
    match bmo_titan_x86_64::build_package("ligera.titan", &src, &mut |_| None) {
        Ok(bex) => {
            let e0 = console(bex);
            if e0 == e1 {
                Ok(e1.0)
            } else {
                Err(format!("{src}\n  E0 {e0:?}\n  E1 {e1:?}"))
            }
        }
        Err(bmo_titan_x86_64::Failure::Source(m)) => {
            let no = format!("NO T{:04} al correr, linea {}:", m.code.number(), m.line);
            if e1.0.contains(&no) {
                Ok(format!("NO T{:04} linea {}", m.code.number(), m.line))
            } else {
                Err(format!("{src}\n  E0 dice {no}\n  E1 {e1:?}"))
            }
        }
        Err(other) => Err(format!("{other:?}")),
    }
}

#[test]
fn a_light_fn_runs_as_the_calculation_says() {
    let programs: &[(&str, &str)] = &[
        // cuatro valores: escritos, un local, y LLAMADAS dentro de los
        // valores (la segunda pisa los registros de la primera)
        (
            "fn mezcla(a: int, b: int, c: int, d: int) -> int\n    return a * 1000 + b * 100 + c * 10 + d\nfn sube(n: int) -> int\n    return n + 1\nfn main()\n    let x = 5\n    print(mezcla(1, x, sube(2), mezcla(0, 0, sube(0), sube(3))))\n",
            "1544\n",
        ),
        // sin resultado, llamada en su linea
        ("fn dice(n: int)\n    print(n * 2)\nfn main()\n    for i in range(3)\n        dice(i)\n", "0\n2\n4\n"),
        // lo que muere al volver (una lista) se suelta con el resultado aparte
        (
            "fn cuenta(n: int) -> int\n    let mut l: [int] = []\n    for i in range(n)\n        push(mut l, i * i)\n    return len(l) + l[n - 1]\nfn main()\n    print(cuenta(4), \" \", cuenta(1))\n",
            "13 1\n",
        ),
        // recursiva, con sus dos llamadas en la misma cuenta
        ("fn fib(n: int) -> int\n    if n < 2\n        return n\n    return fib(n - 1) + fib(n - 2)\nfn main()\n    print(fib(15))\n", "610\n"),
        // un NO dentro de la fn ligera, en SU linea
        ("fn dobla(n: int) -> int\n    return n * 2\nfn main()\n    let mut x = 1\n    for i in range(70)\n        x = dobla(x)\n    print(x)\n", "NO T0060 linea 4"),
        // cinco valores: la de siempre, y las dos se llaman entre si
        (
            "fn cinco(a: int, b: int, c: int, d: int, e: int) -> int\n    return dos(a, b) + dos(c, d) + e\nfn dos(a: int, b: int) -> int\n    return a * 10 + b\nfn main()\n    print(cinco(1, 2, 3, 4, 5))\n",
            "51\n",
        ),
    ];
    let mut wrong = Vec::new();
    for (k, (body, want)) in programs.iter().enumerate() {
        match same(body) {
            Ok(out) if out == *want => {}
            Ok(out) => wrong.push(format!("programa {k}: escribe {out:?}, y se esperaba {want:?}")),
            Err(e) => wrong.push(format!("programa {k}: {e}")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}
