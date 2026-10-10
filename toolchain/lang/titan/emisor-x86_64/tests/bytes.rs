//! ** TA1 de `docs/plan/PLAN_LA_TINTA.md` (10-10): EL BYTE de TITAN++.
//!
//! ```text
//!    byte        0 a 255; cuenta como un int (sumar, comparar, indexar)
//!    byte(x)     un int a byte: si no cabe, NO T0060 -- al compilar o al correr
//!    [byte]      UNA celda, UN byte: los pixeles de una pagina caben
//! ```
//!
//! La vara, la de siempre: el calculo (E0) y la maquina (E1) escriben LO MISMO,
//! o el mismo NO en la misma linea. Y la medida que justifica el tipo: una
//! `[byte]` de tres millones de celdas pide a la memoria una octava parte de
//! lo que pide la misma `[int]`.

use bmo_lower::emu::{cargar_bex, run};

fn console(bex: Vec<u8>) -> (String, bool) {
    let m = run(cargar_bex(&bex).unwrap(), 50_000_000);
    (m.console.clone(), m.exited)
}

/// E0 y E1, sobre el mismo programa: lo mismo, o el mismo NO.
fn same(body: &str) -> Result<String, String> {
    let src = format!("mod main \"bytes\"\n\n{body}");
    let e1 = match bmo_titan_x86_64::build_package_e1("bytes.titan", &src, &mut |_| None) {
        Ok(bex) => console(bex),
        Err(e) => return Err(format!("E1 no lo emite: {e:?}\n{src}")),
    };
    match bmo_titan_x86_64::build_package("bytes.titan", &src, &mut |_| None) {
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
fn bytes_run_in_the_machine_as_the_calculation_says() {
    let programs: &[(&str, &str)] = &[
        // push, index, len, for, a cell changed, print, pop, equality
        (
            "fn main()\n    let mut px: [byte] = []\n    for i in range(6)\n        push(mut px, byte(i * 50))\n    px[1] = byte(7)\n    let mut s = 0\n    for b in px\n        s = s + b\n    print(px, \" \", len(px), \" \", px[5], \" \", s)\n    match pop(mut px)\n        Hay(b)\n            print(\"saco \", b, \" \", b + 1000)\n        NoHay\n            print(\"nada\")\n    print(px == [byte(0), byte(7), byte(100), byte(150), byte(200)], \" \", px == [byte(0)])\n",
            "[0, 7, 100, 150, 200, 250] 6 250 707\nsaco 250 1250\ntrue false\n",
        ),
        // a byte counts as an int: arithmetic, comparisons, an index, a range
        (
            "fn main()\n    let t: [int; 4] = [10, 20, 30, 40]\n    let b = byte(3)\n    let c: byte = byte(200)\n    print(b * c, \" \", c > b, \" \", c == 200, \" \", t[b], \" \", -b, \" \", c % 7)\n    let mut n = 0\n    for i in range(b)\n        n = n + i\n    print(n)\n",
            "600 true true 40 -3 4\n3\n",
        ),
        // through fns, records, maps and Opcion: a byte alone is a word
        (
            "type Pixel\n    r: byte\n    g: byte\nfn mezcla(a: byte, b: byte) -> byte\n    return byte((a + b) / 2)\nfn main()\n    let p = Pixel { r: byte(255), g: mezcla(byte(10), byte(30)) }\n    let mut m: {text: byte} = {}\n    put(mut m, \"rojo\", p.r)\n    print(p.r, \" \", p.g, \" \", get(m, \"rojo\"), \" \", get(m, \"azul\"))\n",
            "255 20 Hay(255) NoHay\n",
        ),
        // a table written into a list; a copy is another list; a list of lists
        (
            "fn main()\n    let a: [byte] = [byte(1), byte(2), byte(3)]\n    let mut b = a\n    b[0] = byte(9)\n    let mut filas: [[byte]] = []\n    push(mut filas, a)\n    push(mut filas, b)\n    print(a, \" \", b, \" \", filas, \" \", filas[1][0])\n",
            "[1, 2, 3] [9, 2, 3] [[1, 2, 3], [9, 2, 3]] 9\n",
        ),
        // growth: thousands of one-byte cells, read back
        (
            "fn main()\n    let mut px: [byte] = []\n    for i in range(5000)\n        push(mut px, byte(i % 256))\n    let mut s = 0\n    for b in px\n        s = s + b\n    print(len(px), \" \", s, \" \", px[4999], \" \", px[256])\n",
            "5000 629340 135 0\n",
        ),
        // what does not fit is a NO, never a cut: written
        ("fn main()\n    let b = byte(256)\n    print(b)\n", "NO T0060 linea 4"),
        ("fn main()\n    let b = byte(0 - 1)\n    print(b)\n", "NO T0060 linea 4"),
        // and computed
        ("fn main()\n    let mut px: [byte] = []\n    for i in range(300)\n        push(mut px, byte(i))\n    print(len(px))\n", "NO T0060 linea 6"),
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

/// Lo que el calculo dice NO al compilar, sin correr nada.
#[test]
fn an_int_becomes_a_byte_only_through_byte() {
    let no = |body: &str| match bmo_titan_x86_64::build_package("bytes.titan", &format!("mod main \"bytes\"\n\n{body}"), &mut |_| None) {
        Err(bmo_titan_x86_64::Failure::Source(m)) => format!("T{:04}", m.code.number()),
        other => format!("{other:?}"),
    };
    // un int no entra callado donde va un byte
    assert_eq!(no("fn main()\n    let b: byte = 7\n    print(b)\n"), "T0071");
    assert_eq!(no("fn main()\n    let mut px: [byte] = []\n    push(mut px, 7)\n    print(px)\n"), "T0071");
    // ni una [byte] es una [int]: otra lista, no otra mirada
    assert_eq!(no("fn main()\n    let a: [byte] = [byte(1)]\n    let b: [int] = a\n    print(b)\n"), "T0071");
    // byte pide UN valor, y un numero entero
    assert_eq!(no("fn main()\n    let b = byte(1, 2)\n    print(b)\n"), "T0068");
    assert_eq!(no("fn main()\n    let b = byte(\"7\")\n    print(b)\n"), "T0071");
}

/// ** LA MEDIDA DEL TIPO: tres millones de celdas -- una pagina de 1600 x
/// 1875 en grises -- en `[byte]` y en `[int]`. La de bytes cabe en DOS trozos
/// del monton (sus bloques crecen al doble y llevan cabecera: el de 3 MB vive
/// en uno de 8); la de ints no cabe en el monton que el emulador da, como el
/// kernel (o pide al menos tres veces mas).
#[test]
fn a_byte_list_takes_one_byte_per_cell() {
    let pide = |cell: &str, valor: &str| {
        let src = format!("mod main \"medida\"\n\nfn main()\n    let r = lee()\n    let mut px: [{cell}] = []\n    for i in range(3000000)\n        push(mut px, {valor})\n    print(r, len(px))\n");
        let bex = bmo_titan_x86_64::build_package_e1("medida.titan", &src, &mut |_| None).unwrap();
        let mut m = cargar_bex(&bex).unwrap();
        m.poner_entrada("n\n");
        let m = run(m, 400_000_000);
        (m.console.clone(), m.memoria_entregada())
    };
    let (cb, b) = pide("byte", "byte(i % 256)");
    assert_eq!(cb, "n3000000\n");
    assert!(b <= 32 << 20, "[byte] pidio {} MiB", b >> 20);
    // La misma pagina en [int]: o no cabe en el monton (el tope que el
    // emulador modela, como el kernel), o pide al menos tres veces mas.
    let (ci, i) = pide("int", "i % 256");
    let sin_sitio = ci.contains("el monton (la memoria de las listas y los mapas) se acabo");
    assert!(sin_sitio || (ci == "n3000000\n" && i >= 3 * b), "[int]: {ci:?}, {} MiB contra {} MiB", i >> 20, b >> 20);
    eprintln!("tres millones de celdas: [byte] {} MiB; [int] {}", b >> 20, if sin_sitio { "NO CABE".to_string() } else { format!("{} MiB", i >> 20) });
}
