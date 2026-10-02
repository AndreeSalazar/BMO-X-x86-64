//! **Lo que escribe el programa en marcha, sin repetir** (02-10).
//!
//! Cada byte que el programa lanzado desde Ejecutar escribe pasa por aqui
//! (lo apunta `watch.rs` al drenarlo) y queda en el [`bmo_registro::Registro`]
//! de la sesion: cada linea distinta UNA vez, con cuantas veces vino. Lo
//! leen CABINA (la vista `P`, en vivo) y `save` (el capitulo 8). La consola
//! de Ejecutar no cambia: sigue mostrando todo, tal como llega.
//!
//! [consumo] NADA      solo cuando un programa escribe
//!
//! Un programa nuevo empieza un registro nuevo: lo de antes ya quedo en su
//! `.txt` y en el `save` que se hiciera.

use bmo_registro::Registro;

/// Cuantas lineas DISTINTAS caben (unos 100 KiB de ceros en el `.bex`).
pub(crate) const DISTINTAS: usize = 512;

static mut REGISTRO: Registro<DISTINTAS> = Registro::nuevo();
/// El nombre de lo que se lanzo (lo que se tecleo), para el rotulo.
static mut NOMBRE: [u8; 64] = [0; 64];
static mut LARGO_NOMBRE: usize = 0;

/// El registro de la sesion (un hilo: el bucle del escritorio).
pub(crate) fn registro() -> &'static mut Registro<DISTINTAS> {
    // SAFETY: el DIRECTOR es un hilo; nadie guarda la referencia de una
    // vuelta a otra.
    unsafe { &mut *core::ptr::addr_of_mut!(REGISTRO) }
}

/// **Un programa nuevo**: registro a cero y su nombre.
pub(crate) fn empezar(nombre: &[u8]) {
    registro().vaciar();
    // SAFETY: como arriba.
    unsafe {
        let n = nombre.len().min(64);
        (&mut *core::ptr::addr_of_mut!(NOMBRE))[..n].copy_from_slice(&nombre[..n]);
        LARGO_NOMBRE = n;
    }
}

/// Lo que el programa escribio, tal como llega.
pub(crate) fn escribir(bytes: &[u8]) {
    registro().escribir(bytes);
}

/// El programa acabo: lo que quedo sin `\n` es una linea.
pub(crate) fn terminar() {
    registro().terminar();
}

/// El nombre de lo que se lanzo (vacio si nada).
pub(crate) fn nombre() -> &'static [u8] {
    // SAFETY: como arriba.
    unsafe { &(&*core::ptr::addr_of!(NOMBRE))[..LARGO_NOMBRE] }
}
