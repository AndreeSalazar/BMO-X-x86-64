//! **Las opciones de delante de la ruta**, en cualquier orden: `--diario`,
//! `--3060`, `--3060-escalera`, `--3060-solo`, `--pantalla WxH` (LI2j) y `--sin-obreros`. Salio de
//! `_start` (`main.rs`) por L6a moviendo texto: el bucle es el mismo.

use crate::{bmo, la3060, obreros};

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
        } else if let Some(r) = todo.strip_prefix(b"--3060-escalera ") {
            // ** LA ESCALERA (13-10): `--3060` y ademas cada lote con un
            // semaforo por escalon (estado, vertices, dibujo). Mas lento: es
            // para DIAGNOSTICAR -- un lote que la 3060 no paga dice en que
            // escalon se paro, y el parte del canal muerto lo completa.
            (todo, con_diario) = (r, true);
            la3060::modo_3060();
            bmo_proton_x_sm86::puerta::pedir_escalera(true);
            crate::di("PROTON-X --3060-escalera: cada lote de la 3060 con su escalera (estado, vertices, dibujo): mas lento, para saber DONDE se para\n");
        } else if let Some(r) = todo.strip_prefix(b"--3060-solo ") {
            // ** SOLO LA GPU (14-10): `--3060` y lo que la 3060 no sabe NO lo
            // hace la CPU: se salta (`la3060::solo_gpu`). La imagen, incompleta.
            (todo, con_diario) = (r, true);
            la3060::solo_gpu();
            crate::di("PROTON-X --3060-solo: SOLO LA GPU -- lo que la 3060 no sabe dibujar o correr se SALTA (la CPU no pinta): la imagen saldra incompleta, y el censo dice cuanto se salto\n");
        } else if let Some(r) = todo.strip_prefix(b"--3060 ") {
            // ** EL MODO 3060 (12-10): el diario, y la 3060 contada --
            // si esta lista al arrancar, el censo (LI0) cada 10 s y lo que
            // dibujo ella. `personal 3060 <ruta>` en el escritorio.
            (todo, con_diario) = (r, true);
            la3060::modo_3060();
        } else if let Some(r) = todo.strip_prefix(b"--pantalla ") {
            // ** LA PANTALLA DEL PROPIETARIO (LI2j, 12-10): `--pantalla 1920x1080`
            // -- la que ve el juego, la elige el propietario (sin ella: 1920x1080,
            // o con `--3060` la que dibuja la puerta). Sin barra de tareas si
            // es la de la 3060; si no, la de Windows (40).
            let k = r.iter().position(|&c| c == b' ').unwrap_or(r.len());
            let Some((ancho, alto)) = bmo_proton_x_casa::pantalla::leer(&r[..k]) else {
                crate::di("PROTON-X: NO -- `--pantalla` quiere ancho x alto (`--pantalla 1920x1080`)\n");
                bmo::salir();
            };
            let barra = if (ancho, alto) == bmo_proton_x_sm86::puerta::MEDIDA { 0 } else { 40 };
            if let Err(m) = bmo_proton_x_casa::pantalla::poner(bmo_proton_x_casa::pantalla::Pantalla { ancho, alto, barra }, true) {
                crate::di("PROTON-X: NO -- `--pantalla`: ");
                crate::di(m);
                crate::di("\n");
                bmo::salir();
            }
            todo = r.get(k + 1..).unwrap_or(&[]);
        } else if let Some(r) = todo.strip_prefix(b"--sin-obreros ") {
            todo = r;
            obreros::apagar();
        } else {
            break;
        }
    }
    (todo, con_diario)
}
