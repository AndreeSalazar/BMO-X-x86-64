//! ** LO QUE GASTA LA PILA (MC1 de `docs/plan/PLAN_MUNDO.md`, 11-10). Los
//! marcos de E1 caben en 48 KiB; dos cosas los inflaban sin motivo:
//!
//! ```text
//!    un `mut` prestado     es un PUNTERO al del que llama: 8 bytes, no la
//!                          tabla entera otra vez en cada marco
//!    let t = [x; n]        cada celda, derecha a su local: sin una tabla
//!                          temporal del mismo tamanio al lado
//! ```
//!
//! La vara, la de siempre: E0 y E1 escriben lo mismo. Antes, los dos
//! programas de aqui eran T0066 en E1 (la pila no daba) y no en E0.

use bmo_lower::emu::{cargar_bex, run};

fn console(bex: Vec<u8>) -> (String, bool) {
    let m = run(cargar_bex(&bex).unwrap(), 200_000_000);
    (m.console.clone(), m.exited)
}

/// E0 y E1, sobre el mismo programa: lo mismo.
fn same(body: &str) -> String {
    let src = format!("mod main \"pila\"\n\n{body}");
    let e1 = console(bmo_titan_x86_64::build_package_e1("pila.titan", &src, &mut |_| None).unwrap_or_else(|e| panic!("E1 no lo emite: {e:?}")));
    let e0 = console(bmo_titan_x86_64::build_package("pila.titan", &src, &mut |_| None).unwrap_or_else(|e| panic!("E0: {e:?}")));
    assert_eq!(e0, e1, "{src}");
    e1.0
}

/// Una tabla de 2000 (16 KiB) prestada tres veces hacia dentro: con la tabla
/// en cada marco eran 64 KiB.
#[test]
fn a_lent_table_is_a_pointer_in_every_frame() {
    let out = same(
        "fn hondo(mut t: [int; 2000], n: int)\n    t[n] = t[n] + n\n    if n > 0\n        hondo(mut t, n - 1)\n\nfn main()\n    let mut t: [int; 2000] = [1; 2000]\n    hondo(mut t, 3)\n    let mut s = 0\n    for x in t\n        s = s + x\n    print(s, \" \", t[3])\n",
    );
    assert_eq!(out, "2006 4\n");
}

/// Dos tablas de 2500 (20 KiB cada una) hechas con `[x; n]`, y una llamada:
/// con el temporal al lado eran 60 KiB.
#[test]
fn a_repeated_table_goes_straight_to_its_local() {
    let out = same(
        "fn suma(t: [int; 3]) -> int\n    return t[0] + t[1] + t[2]\n\nfn main()\n    let x = 2\n    let a: [int; 2500] = [x; 2500]\n    let b = [x + 1; 2500]\n    print(a[2499] + b[0], \" \", suma([a[0], b[1], 4]))\n",
    );
    assert_eq!(out, "5 9\n");
}
