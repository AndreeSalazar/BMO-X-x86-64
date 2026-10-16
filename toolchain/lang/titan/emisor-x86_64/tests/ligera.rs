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

/// ** B1 y B2 (16-10): un divisor que se DEMUESTRA 1 o mas no se mira al
/// correr, y `x % 2^k` de un `x` que se demuestra no negativo es un `and`.
/// Y donde NO se demuestra, el NO sigue en su linea.
#[test]
fn what_is_proven_is_not_checked_and_what_is_not_still_traps() {
    let programs: &[(&str, &str)] = &[
        // d empieza en 2 y solo sube: se demuestra, y da lo mismo
        ("fn main()\n    let mut c = 0\n    for k in range(2, 60)\n        let mut d = 2\n        while d * d <= k\n            if k % d == 0\n                c = c + 1\n            d = d + 1\n    print(c)\n", "69\n"),
        // d puede llegar a 0: se mira, y atrapa en su linea
        ("fn main()\n    let mut s = 0\n    let mut d = 3\n    while d > -2\n        s = s + 12 / d\n        d = d - 1\n    print(s)\n", "NO T0061 linea 7"),
        // d empieza en 0: no se demuestra
        ("fn main()\n    let mut s = 0\n    for d in range(0, 3)\n        s = s + 6 % d\n    print(s)\n", "NO T0061 linea 6"),
        // `% 256` de un negativo: no es un `and` (C trunca hacia cero)
        ("fn main()\n    let mut s = 0\n    let mut x = 5\n    for i in range(10)\n        x = x - 100\n        s = s + x % 256\n    print(s, \" \", x)\n", "-1354 -995\n"),
        // y de uno que se demuestra no negativo, si
        ("fn main()\n    let mut s = 0\n    for i in range(600)\n        s = s + (i * 7) % 256\n    print(s)\n", "75436\n"),
        // B3: el bucle girado mira su condicion LAS MISMAS veces (la llamada
        // de la condicion escribe cada vez)
        ("fn mira(x: int) -> int\n    print(\"m\", x)\n    return x\nfn main()\n    let mut i = 0\n    while mira(i) < 3\n        i = i + 1\n    print(\"fin \", i)\n", "m0\nm1\nm2\nm3\nfin 3\n"),
        // B4: `byte(x % 256)` de un x que puede ser negativo se sigue mirando
        ("fn main()\n    let mut px: [byte] = []\n    for i in range(10)\n        push(mut px, byte((i - 5) % 256))\n    print(len(px))\n", "NO T0060 linea 6"),
        // y el numero dentro del `imul` desborda igual
        ("fn main()\n    let mut x = 1\n    for i in range(10)\n        x = x * 100000\n    print(x)\n", "NO T0060 linea 6"),
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

/// ** B5 (16-10): la celda de una lista de enteros (o bytes) se lee y se
/// escribe DERECHO en `rax`, sin dejar su direccion en la pila. Lo mismo que
/// antes: el valor, y el NO fuera de ella en su linea.
#[test]
fn a_cell_read_and_written_in_the_register_is_the_same_cell() {
    let programs: &[(&str, &str)] = &[
        (
            "fn main()\n    let mut a: [int] = []\n    for i in range(6)\n        push(mut a, i * i)\n    a[2] = a[5] + a[1]\n    a[0] = a[a[1]] - 7\n    let mut s = 0\n    for x in a\n        s = s + x\n    print(a, \" \", s)\n",
            "[-6, 1, 26, 9, 16, 25] 71\n",
        ),
        (
            "fn main()\n    let mut b: [byte] = []\n    for i in range(5)\n        push(mut b, byte(i + 250))\n    b[0] = byte(b[4] - b[1])\n    print(b)\n",
            "[3, 251, 252, 253, 254]\n",
        ),
        // fuera, al leer y al escribir, por debajo y por encima
        ("fn main()\n    let a: [int] = [1, 2, 3]\n    let mut s = 0\n    for i in range(4)\n        let v = a[i]\n        s = s + v\n    print(s)\n", "NO T0072 linea 7"),
        ("fn main()\n    let a: [int] = [1, 2, 3]\n    let mut s = 0\n    for i in range(4)\n        if a[3 - i] > 0\n            s = s + 1\n    print(s)\n", "NO T0072 linea 7"),
        ("fn main()\n    let mut a: [int] = [1, 2, 3]\n    for i in range(4)\n        a[2 - i] = i\n    print(a)\n", "NO T0072 linea 6"),
        // un byte que no cabe, escrito derecho, sigue siendo un NO
        ("fn main()\n    let mut b: [byte] = [byte(1)]\n    for i in range(3)\n        b[0] = byte(b[0] * 200)\n    print(b)\n", "NO T0060 linea 6"),
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
