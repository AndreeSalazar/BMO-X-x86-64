//! **LO QUE UNA APP LE PIDE AL ESCRITORIO** (01-10) -- el JUGAR de la LUDOTECA.
//!
//! [consumo] NADA      mira los bytes que el escritorio YA drena de la
//!                     consola de sus hijos; no corre solo (L6h)
//!
//! Un juego lanzado por la LUDOTECA seria su hijo, y su ventana se ofreceria
//! a ELLA, no al escritorio que compone. Asi que la app no lanza: PIDE, con
//! una linea de su consola que empieza por el byte 0x1E (el separador de
//! registros: nadie lo teclea, y en la salida no se pinta):
//!
//! ```text
//!    0x1E personal diario <ruta en D:>   PROTON-X con su diario, como tecleado
//!    0x1E apps/<algo>.bex [args]          un programa de apps/ o sys/
//!    0x1E sys/<algo>.bex [args]
//! ```
//!
//! Lista BLANCA: cualquier otra cosa se dice en la salida y no se hace. Una
//! app ya podia lanzar programas ella misma (`bmo::ejecutar`), asi que esto
//! no le da un poder nuevo: le da el sitio correcto para la ventana.

use bmo_userland as bmo;

use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_PLAIN};
use crate::PATH_MAX;

const RS: u8 = 0x1E;

struct Estado {
    /// El ultimo byte que se dejo pasar fue un fin de linea (o no hubo ninguno).
    al_principio: bool,
    /// Se esta guardando una peticion.
    guardando: bool,
    linea: [u8; PATH_MAX],
    n: usize,
    /// Una peticion entera, esperando a que el bucle la atienda.
    lista: Option<([u8; PATH_MAX], usize)>,
}

static mut ESTADO: Estado = Estado { al_principio: true, guardando: false, linea: [0; PATH_MAX], n: 0, lista: None };

fn estado() -> &'static mut Estado {
    // SAFETY: solo el hilo del escritorio, al drenar y al atender.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// **Filtra lo drenado**: lo normal sale por `pasa`; una linea que empieza por
/// 0x1E se guarda y no se pinta.
pub(crate) fn filtrar(bytes: &[u8], mut pasa: impl FnMut(&[u8])) {
    let e = estado();
    // Desde donde va el trozo normal que todavia no se ha dejado pasar.
    let mut desde = 0;
    for (k, &b) in bytes.iter().enumerate() {
        if e.guardando {
            if b == b'\n' {
                e.guardando = false;
                e.al_principio = true;
                e.lista = Some((e.linea, e.n));
                desde = k + 1;
            } else if e.n < PATH_MAX {
                e.linea[e.n] = b;
                e.n += 1;
            }
        } else if b == RS && e.al_principio {
            if k > desde {
                pasa(&bytes[desde..k]);
            }
            e.guardando = true;
            e.n = 0;
        } else {
            e.al_principio = b == b'\n';
        }
    }
    if !e.guardando && desde < bytes.len() {
        pasa(&bytes[desde..]);
    }
}

/// Que hacer con una peticion.
pub(crate) enum Que<'a> {
    /// `personal diario <ruta>`.
    Diario(&'a [u8]),
    /// Un `.bex` de `apps/` o `sys/`, con lo que lleve detras.
    Programa(&'a [u8]),
    /// Fuera de la lista blanca.
    No,
}

pub(crate) fn juzgar(l: &[u8]) -> Que<'_> {
    if let Some(r) = l.strip_prefix(b"personal diario ") {
        return if r.is_empty() { Que::No } else { Que::Diario(r) };
    }
    let programa = l.split(|&c| c == b' ').next().unwrap_or(l);
    let de_casa = programa.starts_with(b"apps/") || programa.starts_with(b"sys/");
    if de_casa && programa.ends_with(b".bex") && !programa.windows(2).any(|w| w == b"..") {
        return Que::Programa(l);
    }
    Que::No
}

/// La peticion que llego, UNA vez.
pub(crate) fn tomar() -> Option<([u8; PATH_MAX], usize)> {
    estado().lista.take()
}

/// **Atiende la peticion que haya**: la dice en la salida y la pasa por la
/// misma puerta que un `run` tecleado (`scene::abrir`).
pub(crate) fn atender(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let Some((linea, n)) = tomar() else { return };
    let l = &linea[..n];
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ECHO);
    g.text(b"  una app pide: ");
    g.text(l);
    g.text(b"\n");
    g.with_ink(INK_PLAIN);
    match juzgar(l) {
        Que::Diario(ruta) => {
            let mut buf = [0u8; PATH_MAX];
            match crate::commands::files::linea_censo(ruta, true, &mut buf) {
                Ok(k) => {
                    crate::desktop::keys::editor::antes_de_proton_x(dsk, p, &buf[..k]);
                    crate::scene::abrir::pedir(&[&buf[..k]]);
                }
                Err(frase) => {
                    let g = &mut dsk.out.grid;
                    g.with_ink(INK_ERR);
                    g.text(b"  ");
                    g.text(frase);
                    g.text(b"\n");
                    g.with_ink(INK_PLAIN);
                }
            }
        }
        Que::Programa(l) => {
            crate::scene::abrir::pedir(&[l]);
        }
        Que::No => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            g.text(b"  no: solo `personal diario <ruta>` o un .bex de apps/ o sys/\n");
            g.with_ink(INK_PLAIN);
        }
    }
    dsk.out.grid.dirty = true;
}
