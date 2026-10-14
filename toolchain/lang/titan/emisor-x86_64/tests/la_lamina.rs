//! ** A4b DE `docs/plan/EL_FOCO.md` (14-10): LA LAMINA de TITAN++ -- RAM que
//! el programa le pide al kernel por su puerta (`e1/ram.rs`), con UN
//! propietario, que se presta (`mut`), se entrega (`take`) y se suelta sola
//! al morir quien la tiene (`e1/lamina.rs`). El emulador hace de kernel:
//! cuenta los bloques que se piden y los que vuelven (`memoria_soltada`), y
//! deja tener ocho a la vez, como el kernel.
//!
//! (No confundir con `tests/lamina.rs`: esa es la lamina de VERRANO, la que
//! publica vertices para que la 3060 los dibuje.)

use bmo_abi::syscalls::surface::{CURRENT_TASK, MEM_OP_SOLTAR, TASK_OP_MEMORIA_PEDIR};
use bmo_lower::emu::{cargar_bex, run, Machine};

fn corre(cuerpo: &str) -> Machine {
    let src = format!("mod main \"x\"\n\n{cuerpo}");
    let bex = bmo_titan_x86_64::build_package("main.titan", &src, &mut |_| None).unwrap_or_else(|e| panic!("{:?}\n{}", e, src));
    let m = run(cargar_bex(&bex).unwrap(), 50_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

/// El NO del compilador: su codigo y su linea.
fn no(cuerpo: &str) -> (u16, usize) {
    let src = format!("mod main \"x\"\n\n{cuerpo}");
    match bmo_titan_x86_64::build_package("main.titan", &src, &mut |_| None) {
        Err(bmo_titan_x86_64::Failure::Source(m)) => (m.code.number(), m.line),
        Ok(_) => panic!("compilo y tenia que decir NO:\n{}", src),
        Err(e) => panic!("{:?}", e),
    }
}

/// ** Prestada, entregada y soltada: las dos laminas vuelven al kernel, y
/// cada una sale por la puerta de la RAM.
#[test]
fn lent_given_and_let_go() {
    let m = corre(
        "fn llena(mut l: lamina, v: int)\n    for i in range(len(l))\n        l[i] = byte(v)\n\nfn suma(take l: lamina) -> int\n    let mut s = 0\n    for i in range(len(l))\n        s = s + l[i]\n    return s\n\nfn main()\n    let mut a = lamina(100)\n    let mut b = lamina(50)\n    llena(mut a, 3)\n    llena(mut b, 7)\n    print(len(a), \" \", len(b), \" \", a[0], \" \", b[49])\n    a[5] = byte(200)\n    print(a[5])\n    print(suma(take a))\n    print(suma(take b))\n",
    );
    assert_eq!(m.console, "100 50 3 7\n200\n497\n350\n");
    assert_eq!(m.memoria_soltada(), 2, "las dos, una vez cada una: la entregada no se suelta dos veces");
    // Y ni una LLAMADA de mas: quien la entrego no intenta soltarla tambien.
    let sueltas = m.syscalls.iter().filter(|c| c.capability != CURRENT_TASK && c.operation == MEM_OP_SOLTAR).count();
    assert_eq!(sueltas, 2, "cada lamina se suelta UNA vez");
    let p: Vec<_> = m.syscalls.iter().filter(|c| c.capability == CURRENT_TASK && c.operation == TASK_OP_MEMORIA_PEDIR).map(|c| (c.arg0, c.rip)).collect();
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), vec![100, 50]);
    assert_eq!(p[0].1, p[1].1, "por la misma puerta");
}

/// ** Se suelta SOLA al morir su propietario, por cualquier camino: al
/// acabar la fn, al tomar otro valor, al cerrar cada vuelta, con `break`,
/// con `continue` y volviendo pronto. Veinte en un bucle con un tope de ocho
/// a la vez solo caben si cada una vuelve.
#[test]
fn let_go_by_itself_on_every_path() {
    let casos: [(&str, &str, u64); 6] = [
        ("fn main()\n    let c = lamina(10)\n    print(len(c))\n", "10\n", 1),
        ("fn main()\n    let mut a = lamina(10)\n    a = lamina(20)\n    print(len(a))\n", "20\n", 2),
        ("fn main()\n    let mut n = 0\n    for i in range(20)\n        let mut x = lamina(4096)\n        x[4095] = byte(i)\n        n = n + x[4095]\n    print(n)\n", "190\n", 20),
        ("fn main()\n    for i in range(5)\n        let x = lamina(10)\n        if i == 2\n            break\n    print(1)\n", "1\n", 3),
        ("fn main()\n    let mut i = 0\n    while i < 30\n        let x = lamina(10)\n        i = i + 1\n        if i % 2 == 0\n            continue\n    print(i)\n", "30\n", 30),
        ("fn f(n: int) -> int\n    let x = lamina(10)\n    if n > 0\n        return 1\n    return len(x)\n\nfn main()\n    print(f(1), \" \", f(0))\n", "1 10\n", 2),
    ];
    for (src, sale, soltadas) in casos {
        let m = corre(src);
        assert_eq!((m.console.as_str(), m.memoria_soltada()), (sale, soltadas), "{}", src);
    }
}

/// ** Lo que solo se sabe al correr: fuera de ella (T0072), pedir cero bytes
/// o que el kernel diga que no -- la novena a la vez -- (T0060), en su linea.
#[test]
fn what_only_running_says() {
    let casos = [
        ("fn main()\n    let mut a = lamina(10)\n    a[10] = byte(1)\n", "NO T0072 al correr, linea 5: una celda fuera de la lamina\n"),
        ("fn main()\n    let a = lamina(10)\n    print(a[-1])\n", "NO T0072 al correr, linea 5: una celda fuera de la lamina\n"),
        ("fn main()\n    let n = 0\n    let a = lamina(n)\n    print(len(a))\n", "NO T0060 al correr, linea 5: lamina(n) pide al menos un byte\n"),
        (
            "fn main()\n    let a = lamina(1)\n    let b = lamina(1)\n    let c = lamina(1)\n    let d = lamina(1)\n    let e = lamina(1)\n    let f = lamina(1)\n    let g = lamina(1)\n    let h = lamina(1)\n    let i = lamina(1)\n    print(len(i))\n",
            "NO T0060 al correr, linea 12: el kernel no dio la lamina (sin RAM, o ya hay todas las que se pueden tener a la vez)\n",
        ),
    ];
    for (src, sale) in casos {
        assert_eq!(corre(src).console, sale, "{}", src);
    }
}

/// ** Los NO del compilador: una lamina no se COPIA (T0077) -- ni con un
/// `let`, ni imprimiendola, ni en una tabla, ni como parametro sin modo, ni
/// devuelta, ni dentro de un tipo --, no crece, y lo demas es lo de siempre.
#[test]
fn what_the_compiler_says_no_to() {
    let casos: [(&str, (u16, usize)); 14] = [
        ("fn main()\n    let a = lamina(10)\n    let b = a\n", (77, 5)),
        ("fn main()\n    let a = lamina(10)\n    print(a)\n", (77, 5)),
        ("fn main()\n    let a = lamina(10)\n    let t = [a]\n", (77, 5)),
        ("fn f(l: lamina) -> int\n    return len(l)\nfn main()\n    let a = lamina(10)\n    print(f(a))\n", (77, 3)),
        ("fn f() -> lamina\n    let l = lamina(4)\n    return l\nfn main()\n    print(1)\n", (77, 3)),
        ("type T\n    l: lamina\nfn main()\n    print(1)\n", (77, 4)),
        ("fn main()\n    let t: [lamina] = []\n    print(len(t))\n", (77, 4)),
        ("fn main()\n    let mut a = lamina(10)\n    push(mut a, byte(1))\n", (77, 5)),
        ("fn main()\n    let mut a = lamina(10)\n    let b = lamina(5)\n    a = b\n", (77, 6)),
        // la entregada ya no es tuya
        ("fn f(take l: lamina) -> int\n    return len(l)\nfn main()\n    let a = lamina(10)\n    print(f(take a))\n    print(len(a))\n", (75, 8)),
        // prestarla para cambiarla pide `let mut`
        ("fn f(mut l: lamina)\n    l[0] = byte(1)\nfn main()\n    let a = lamina(10)\n    f(mut a)\n", (56, 7)),
        // sus celdas son bytes: un int no entra callado
        ("fn main()\n    let mut a = lamina(10)\n    a[0] = 7\n", (71, 5)),
        // pedirla y no guardarla
        ("fn main()\n    lamina(10)\n", (69, 4)),
        ("fn main()\n    let a = lamina(\"x\")\n", (71, 4)),
    ];
    for (src, want) in casos {
        assert_eq!(no(src), want, "{}", src);
    }
}
