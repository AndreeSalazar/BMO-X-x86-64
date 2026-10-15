//! ** C4 DE `docs/plan/EL_FOCO.md` (LB5): un programa de INTI se CUENTA a si
//! mismo con los contadores del Zen 3, por la puerta (`op_contar`), sin una
//! linea nueva en el compilador: los numeros viven en `modulos.toml` y
//! `tests/espejo_del_kernel.rs` los compara con el fuente del kernel. En el
//! anfitrion el emulador hace de kernel: su contador de instrucciones
//! retiradas es el metro (`pasos`).

use bmo_lower::emu::{run, Machine};

fn corre(f: &str) -> Machine {
    let arbol = bmo_inti_front::armar(f);
    assert!(!arbol.hay_errores(), "no se lee: {}", arbol.pintar("prueba.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(&arbol.valor, bmo_inti_front::disposicion::Medidas::cargar(&raices));
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    run(Machine::new(bmo_inti_x86_64::emitir(&ir).codigo), 5_000_000)
}

/// Las instrucciones de un bucle de `n` vueltas, contadas por el programa.
fn cuenta(n: u64) -> u64 {
    let f = format!(
        "perfil llano\nusa bmo\n\nfuncion principal devuelve natural64\n    invoca(mi_tarea, op_contar, contar_empieza, 0, 0)\n    cambiante a es natural64 = invoca_valor(mi_tarea, op_contar, contar_lee, 1, 0)\n    cambiante s es natural64 = 0\n    cambiante i es natural64 = 0\n    repite mientras i < {n}\n        s = s + i\n        i = i + 1\n    cambiante b es natural64 = invoca_valor(mi_tarea, op_contar, contar_lee, 1, 0)\n    devuelve b - a\n"
    );
    let m = corre(&f);
    assert!(m.exited, "el programa tiene que terminar");
    m.syscalls.last().unwrap().arg0
}

/// Lineal en las vueltas, como en TITAN++: lo de fuera se cancela.
#[test]
fn inti_counts_itself_and_the_count_is_linear_in_the_loop() {
    let (c0, c1, c2) = (cuenta(0), cuenta(1000), cuenta(2000));
    let por_vuelta = (c1 - c0) / 1000;
    assert!(por_vuelta >= 3, "una vuelta son varias instrucciones: {}", por_vuelta);
    assert_eq!(c1 - c0, 1000 * por_vuelta);
    assert_eq!(c2 - c1, c1 - c0);
}
