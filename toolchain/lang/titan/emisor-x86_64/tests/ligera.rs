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

/// ** E2 (B5, 16-10): las fn ligeras de enteros las emite E2, con registros
/// virtuales repartidos entre los de verdad. Lo mismo que E1: el valor, y
/// cada NO en su linea.
#[test]
fn e2_gives_the_same_numbers_and_the_same_noes() {
    let programs: &[(&str, &str)] = &[
        // los valores CRUZADOS (un movimiento paralelo) y una vuelta de tres
        (
            "fn g(a: int, b: int) -> int\n    return a * 10 + b\nfn cruza(a: int, b: int) -> int\n    return g(b, a)\nfn rota(a: int, b: int, c: int) -> int\n    return tres(c, a, b)\nfn tres(a: int, b: int, c: int) -> int\n    return a * 100 + b * 10 + c\nfn main()\n    print(cruza(1, 2), \" \", rota(1, 2, 3))\n",
            "21 312\n",
        ),
        // divisiones: por un local, por -1, el resto, y lo que no es exacta
        (
            "fn d(a: int, b: int) -> int\n    return a / b + a % b\nfn main()\n    print(d(12, 4), \" \", d(-12, -1), \" \", d(7, 7), \" \", d(-9, 3))\n",
            "3 12 1 -3\n",
        ),
        ("fn d(a: int, b: int) -> int\n    return a / b\nfn main()\n    print(d(7, 2))\n", "NO T0062 linea 4"),
        ("fn d(a: int, b: int) -> int\n    return a % b\nfn main()\n    print(d(7, 0))\n", "NO T0061 linea 4"),
        ("fn d(a: int, b: int) -> int\n    return a / b\nfn main()\n    print(d(0 - 9223372036854775807 - 1, 0 - 1))\n", "NO T0060 linea 4"),
        // bucles con `and` y `or`, una resta al reves y numeros grandes
        (
            "fn cuenta(n: int) -> int\n    let mut i = 0\n    let mut c = 0\n    while i < n and (i % 3 != 0 or i < 5)\n        c = 100 - c\n        i = i + 1\n    return c + 5000000000 - 5000000000 * 1\nfn main()\n    print(cuenta(20), \" \", cuenta(4), \" \", cuenta(5))\n",
            "0 0 100\n",
        ),
        // muchos valores vivos a la vez: no caben, la emite E1 (y da lo mismo)
        (
            "fn muchos(a: int) -> int\n    let b = a + 1\n    let c = b + 1\n    let d = c + 1\n    let e = d + 1\n    let f = e + 1\n    let g = f + 1\n    let h = g + 1\n    let i = h + 1\n    return a + b + c + d + e + f + g + h + i\nfn main()\n    print(muchos(1))\n",
            "45\n",
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

/// ** B5c (16-10): un BUCLE de cualquier fn lo emite E2 en registros si sabe
/// hacerlo entero: lo que entra se carga al entrar, lo que cambia vuelve a
/// su sitio al salir, y las listas se leen `[base + i*8]` con lo que miden
/// cargado una vez. Lo mismo que E0, y cada NO en su linea.
#[test]
fn a_loop_in_registers_gives_the_same_and_leaves_everything_in_its_place() {
    let programs: &[(&str, &str)] = &[
        // insercion: un while con `and`, celdas leidas y escritas
        (
            "fn main()\n    let mut a: [int] = []\n    for i in range(30)\n        push(mut a, (i * 7919) % 100)\n    let n = len(a)\n    for i in range(1, n)\n        let v = a[i]\n        let mut j = i - 1\n        while j >= 0 and a[j] > v\n            a[j + 1] = a[j]\n            j = j - 1\n        a[j + 1] = v\n    print(a[0], \" \", a[29], \" \", a[15])\n",
            "0 99 51\n",
        ),
        // locales que viven en registros de E1 y que el bucle no toca: siguen
        (
            "fn main()\n    let mut s = 0\n    let mut t = 0\n    let mut u = 0\n    for i in range(50)\n        s = s + i\n        t = t + 2 * i\n        u = u + 3\n    let mut b: [byte] = [byte(1), byte(2), byte(3)]\n    for k in range(3)\n        b[k] = byte(b[k] * 10)\n    for i in range(50)\n        s = s + 1\n    print(s, \" \", t, \" \", u, \" \", b)\n",
            "1275 2450 150 [10, 20, 30]\n",
        ),
        // un bucle con muchos valores vivos (toma prestados registros de E1)
        // mientras otros locales viven en ellos sin que el bucle los toque
        (
            "fn main()\n    let mut p = 0\n    let mut q = 0\n    for i in range(10)\n        for k in range(10)\n            p = p + i + k\n            q = q + p % 7\n            p = p + q % 3\n    let mut s = 0\n    for i in range(20)\n        let x1 = i + 1\n        let x2 = i + 2\n        let x3 = i + 3\n        let x4 = i + 4\n        let x5 = i + 5\n        s = s + x1 * x2 + x3 * x4 + x5 + x1 + x2 + x3 + x4\n    print(p, \" \", q, \" \", s)\n",
            "997 327 8370\n",
        ),
        // `break`, y una lista prestada (`mut`): su base esta detras de un puntero
        (
            "fn llena(mut a: [int], n: int)\n    for i in range(len(a))\n        if i == n\n            break\n        a[i] = i * i\nfn main()\n    let mut a: [int] = [0, 0, 0, 0, 0]\n    llena(mut a, 3)\n    print(a)\n",
            "[0, 1, 4, 0, 0]\n",
        ),
        // fuera de la lista dentro del bucle: el NO en su linea
        ("fn main()\n    let a: [int] = [1, 2, 3]\n    let mut s = 0\n    for i in range(5)\n        s = s + a[i]\n    print(s)\n", "NO T0072 linea 7"),
        ("fn main()\n    let mut a: [int] = [1, 2, 3]\n    for i in range(5)\n        a[4 - i] = i\n    print(a)\n", "NO T0072 linea 6"),
        // un byte que no cabe, dentro del bucle
        ("fn main()\n    let mut b: [byte] = [byte(100)]\n    for i in range(3)\n        b[0] = byte(b[0] + 100)\n    print(b)\n", "NO T0060 linea 6"),
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

/// ** B6 (16-10): lo que ya se miro o se leyo no se repite... salvo que
/// algo lo cambie en medio: la celda (por cualquier indice), el indice o la
/// lista. Cada caso, con su numero.
#[test]
fn what_was_read_is_reused_only_while_nothing_changed() {
    let programs: &[(&str, &str)] = &[
        (
            "fn main()\n    let mut a: [int] = [1, 2, 3, 4, 5]\n    let mut s = 0\n    for k in range(3)\n        let i = k + 1\n        let x = a[i]\n        a[i] = x * 10\n        let y = a[i]\n        let j = i\n        a[j] = 99\n        let u = a[i]\n        s = s + x + y + u\n    print(s, \" \", a)\n",
            "396 [1, 99, 99, 99, 5]\n",
        ),
        (
            "fn main()\n    let mut b: [byte] = [byte(250), byte(5)]\n    let mut s = 0\n    for k in range(2)\n        let x = b[k]\n        b[k] = byte((x + 10) % 256)\n        s = s + b[k] + x\n    print(s, \" \", b)\n",
            "274 [4, 15]\n",
        ),
    ];
    for (k, (body, want)) in programs.iter().enumerate() {
        match same(body) {
            Ok(out) => assert_eq!(out, *want, "programa {k}"),
            Err(e) => panic!("programa {k}: {e}"),
        }
    }
}

/// ** M2 (16-10): `push` dentro de un bucle de E2, con lo que mide la lista
/// en un registro; si no cabe, crece por el camino de E1. Crecer muchas
/// veces, leer lo que mide y sus celdas mientras crece, una lista prestada,
/// bytes: lo mismo que E0.
#[test]
fn a_list_grows_inside_a_loop_in_registers() {
    let programs: &[(&str, &str)] = &[
        (
            "fn main()\n    let mut a: [int] = []\n    let mut s = 0\n    for i in range(1000)\n        push(mut a, i * 3)\n        s = s + len(a) + a[(i - i % 2) / 2]\n    print(len(a), \" \", a[999], \" \", s)\n",
            "1000 2997 1249000\n",
        ),
        (
            "fn llena(mut b: [byte], n: int)\n    for i in range(n)\n        push(mut b, byte(i % 251))\nfn main()\n    let mut b: [byte] = [byte(7)]\n    llena(mut b, 300)\n    llena(mut b, 5)\n    let mut s = 0\n    for x in b\n        s = s + x\n    print(len(b), \" \", b[0], \" \", b[305], \" \", s)\n",
            "306 7 4 32568\n",
        ),
        // un byte que no cabe, empujado dentro del bucle: el NO en su linea
        ("fn main()\n    let mut b: [byte] = []\n    for i in range(300)\n        push(mut b, byte(i))\n    print(len(b))\n", "NO T0060 linea 6"),
    ];
    for (k, (body, want)) in programs.iter().enumerate() {
        match same(body) {
            Ok(out) => assert_eq!(out, *want, "programa {k}"),
            Err(e) => panic!("programa {k}: {e}"),
        }
    }
}

/// ** (16-10) Lo que se DEMUESTRA de un local (que vale 1 o mas) solo quita
/// el cero de una division: una suma con el sigue mirando su desborde. Y
/// `i + 1` de un indice ya mirado contra una lista no desborda (i < largo).
#[test]
fn a_proven_positive_still_overflows_where_it_overflows() {
    let programs: &[(&str, &str)] = &[
        ("fn main()\n    let mut x = 9223372036854775000\n    let mut d = 2\n    for i in range(2000)\n        d = d + 1\n        x = x + d\n    print(x)\n", "NO T0060 linea 8"),
        ("fn main()\n    let mut d = 9223372036854775000\n    for i in range(2000)\n        d = d + 1\n    print(d)\n", "NO T0060 linea 6"),
        (
            "fn main()\n    let a: [int] = [5, 6, 7, 8]\n    let mut s = 0\n    for i in range(3)\n        let v = a[i]\n        s = s + a[i + 1] - v\n    print(s)\n",
            "3\n",
        ),
    ];
    for (k, (body, want)) in programs.iter().enumerate() {
        match same(body) {
            Ok(out) => assert_eq!(out, *want, "programa {k}"),
            Err(e) => panic!("programa {k}: {e}"),
        }
    }
}
