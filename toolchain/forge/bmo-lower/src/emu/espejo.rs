//! ** EL ESPEJO DE LA MEMORIA NO SE VUELVE A SEPARAR (14-10, A4 de
//! `docs/plan/EL_FOCO.md`). El emulador copia del kernel cuanto puede pedir
//! un proceso y cuantas veces; el kernel no enlaza este crate, asi que se
//! copia el numero. El 14-10 la copia decia cuatro y el kernel ocho, y nadie
//! lo vio hasta que la puerta de la RAM de TITAN++ pidio cinco. Ahora se lee
//! el fuente del kernel y se compara.

use super::{MEMORIA_MAX_BYTES, MEMORIA_MAX_PETICIONES};

const KERNEL: &str = include_str!("../../../../../Ultra_kernel_x86-64/kernel/src/ring0/obj/memory.rs");

/// El valor de `pub const <nombre>: <tipo> = <expr>;` del fuente del
/// kernel, con `*` y `_` (lo que usan estas dos).
fn constante(nombre: &str) -> u64 {
    let pre = format!("pub const {}:", nombre);
    let linea = KERNEL.lines().find(|l| l.trim_start().starts_with(&pre)).unwrap_or_else(|| panic!("el kernel ya no define {nombre}"));
    let expr = linea.split('=').nth(1).unwrap().trim().trim_end_matches(';');
    expr.split('*').map(|f| f.trim().replace('_', "").parse::<u64>().unwrap_or_else(|_| panic!("{nombre}: no se lee `{expr}`"))).product()
}

#[test]
fn el_emulador_pide_lo_que_el_kernel_da() {
    assert_eq!(MEMORIA_MAX_PETICIONES as u64, constante("MAX_PETICIONES"), "cuantas veces");
    assert_eq!(MEMORIA_MAX_BYTES, constante("MAX_BYTES"), "cuanto de una vez");
}
