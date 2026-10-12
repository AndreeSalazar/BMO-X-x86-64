//! **Las opciones de delante de la ruta**, en cualquier orden. Salio de
//! `_start` (`main.rs`) por L6a moviendo texto: el bucle es el mismo.

use crate::{la3060, obreros};

/// Quita las opciones de delante de `todo` y devuelve lo que queda y si va
/// con diario.
pub fn leer(todo: &[u8]) -> (&[u8], bool) {
    // ** EL DIARIO (P0.3, 30-09): `--diario <ruta>` ejecuta como siempre, y
    // cada funcion de Windows que el `.exe` llama por primera vez va, en
    // orden, a `informe/diario.txt` (ver `bmo_proton_x_casa::diario`).
    // [!] Se ENCIENDE mas abajo, con el monton ya puesto: el diario guarda su
    // ruta en memoria dinamica, y encenderlo aqui era un panico de 18 bytes
    // (`informe/diario.txt`) con el monton a 0 -- el primer contacto del
    // metal (30-09) no paso de aqui. El banco no lo veia: alli el monton
    // existe siempre.
    // ** `--sin-obreros` (07-10): los sub-directores APAGADOS -- todo el
    // dibujo en el nucleo de la casa, aunque haya `smp all`. Para comparar
    // en el metal sin recompilar. En cualquier orden con `--diario`.
    let (mut todo, mut con_diario) = (todo, false);
    loop {
        if let Some(r) = todo.strip_prefix(b"--diario ") {
            (todo, con_diario) = (r, true);
        } else if let Some(r) = todo.strip_prefix(b"--3060 ") {
            // ** EL MODO 3060 (12-10): el diario, y la 3060 contada --
            // si esta lista al arrancar, el censo (LI0) cada 10 s y lo que
            // dibujo ella. `personal 3060 <ruta>` en el escritorio.
            (todo, con_diario) = (r, true);
            la3060::modo_3060();
        } else if let Some(r) = todo.strip_prefix(b"--sin-obreros ") {
            todo = r;
            obreros::apagar();
        } else {
            break;
        }
    }
    (todo, con_diario)
}
