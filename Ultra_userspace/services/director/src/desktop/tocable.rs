//! **LO QUE SE TOCA EN LA SALIDA** (2026-09-29). Pedido del propietario:
//! *"control + click en el comando ... para que escriba por completo = da
//! flojera escribir"*, y *"que la escritura lo complete TODO porque se limita
//! con lo que esta dentro de la caja"*.
//!
//! [consumo] NADA      corre cuando se escribe una fila tocable o el raton
//!                     pasa o pulsa sobre la salida; en reposo, nada (L6h)
//!
//! # La idea
//!
//! Cada fila de la salida que merece tocarse --el eco de una orden, una
//! entrada de `ls` o de `personal ls`-- deja apuntada aqui la ORDEN ENTERA que
//! escribiria:
//!
//! ```text
//!    . personal ls Cyberpunk 2077     ->  la misma orden (repetirla)
//!      r6                  <DIR>      ->  personal ls Cyberpunk 2077/r6/
//!      REDprelauncher.exe  1 MiB      ->  personal lee Cyberpunk 2077/REDprelauncher.exe
//! ```
//!
//! **CLIC** la escribe en el campo; **CTRL+CLIC** la escribe y la corre. Es la
//! misma regla que la linea de sugerencias, a proposito: dos gestos iguales no
//! pueden significar cosas distintas en dos sitios de la misma ventana.
//!
//! # Por que se apunta y no se lee de la pantalla
//!
//! La rejilla tiene 88 columnas y un nombre NTFS hasta 255 letras: lo que se
//! ve puede estar CORTADO (o envuelto en dos filas), y lo que no es ASCII sale
//! como `?`. Leer la pantalla escribiria un nombre roto. Lo apuntado es el
//! nombre entero, en UTF-8, tal cual lo dio el disco.
//!
//! La fila se reconoce por su numero de linea ESCRITA (`Output::written`), que
//! solo sube: el historial se desplaza y el numero sigue siendo el mismo.

use crate::desktop::Desktop;
use crate::scene::{OUT_COLS, OUT_HIST, PATH_MAX};

/// Cuantas filas tocables se recuerdan (las mas recientes).
const MAX: usize = 256;

#[derive(Clone, Copy)]
struct Tocable {
    /// La linea escrita (`Output::mark`) de la fila. `usize::MAX` = vacia.
    marca: usize,
    n: usize,
    orden: [u8; PATH_MAX],
}

const VACIA: Tocable = Tocable { marca: usize::MAX, n: 0, orden: [0; PATH_MAX] };

struct Estado {
    t: [Tocable; MAX],
    sig: usize,
}

static mut ESTADO: Estado = Estado { t: [VACIA; MAX], sig: 0 };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// **Apunta** que la fila `marca` escribe `orden`. Una orden que no cabe en
/// el campo no se apunta: escribirla cortada seria peor que no escribir.
pub(crate) fn apuntar(marca: usize, orden: &[u8]) {
    if orden.is_empty() || orden.len() > PATH_MAX {
        return;
    }
    let e = estado();
    let k = e.sig;
    e.t[k].marca = marca;
    e.t[k].n = orden.len();
    e.t[k].orden[..orden.len()].copy_from_slice(orden);
    e.sig = (k + 1) % MAX;
}

/// **Arma y apunta** `prefijo` + `ruta` [+ `/` + `nombre`] [+ `/`]: la orden
/// de una entrada listada. `false` si no cabe.
pub(crate) fn apuntar_entrada(marca: usize, prefijo: &[u8], ruta: &[u8], nombre: &[u8], carpeta: bool) -> bool {
    let mut b = [0u8; PATH_MAX];
    let mut n = 0usize;
    let mut poner = |s: &[u8], n: &mut usize| -> bool {
        if *n + s.len() > PATH_MAX {
            return false;
        }
        b[*n..*n + s.len()].copy_from_slice(s);
        *n += s.len();
        true
    };
    let barra = !ruta.is_empty() && !matches!(ruta.last(), Some(b'/' | b'\\'));
    let ok = poner(prefijo, &mut n)
        && poner(ruta, &mut n)
        && (!barra || poner(b"/", &mut n))
        && poner(nombre, &mut n)
        && (!carpeta || poner(b"/", &mut n));
    if ok {
        apuntar(marca, &b[..n]);
    }
    ok
}

fn buscar(marca: usize) -> Option<&'static Tocable> {
    let e = estado();
    // La mas reciente primero: una marca no se repite, pero por si acaso.
    (0..MAX).map(|i| &e.t[(e.sig + MAX - 1 - i) % MAX]).find(|t| t.marca == marca)
}

/// **La fila tocable bajo el raton**, si la hay: su marca.
pub(crate) fn bajo(dsk: &Desktop, x: u32, y: u32) -> Option<usize> {
    let c = &dsk.run_box;
    let s = &dsk.out.grid;
    let filas = c.out_rows();
    let ancho = OUT_COLS as u32 * bmo_userland::GLIFO_ANCHO;
    if x < c.out_x || x >= c.out_x + ancho || y < c.out_y || y >= c.out_y + c.out_h() {
        return None;
    }
    let f = ((y - c.out_y) / bmo_userland::GLIFO_ALTO) as usize;
    if f >= filas || OUT_HIST < filas + s.view {
        return None;
    }
    let h = OUT_HIST - filas - s.view + f;
    if h > s.row {
        return None;
    }
    let marca = s.written.checked_sub(s.row - h)?;
    buscar(marca).map(|_| marca)
}

/// **Un clic sobre la salida.** Escribe la orden de la fila tocada y, con
/// Ctrl, la corre. `true` si habia una.
pub(crate) fn clic(dsk: &mut Desktop, x: u32, y: u32, ctrl: bool) -> bool {
    let Some(marca) = bajo(dsk, x, y) else { return false };
    let Some(t) = buscar(marca) else { return false };
    let n = t.n.min(PATH_MAX);
    dsk.field.path[..n].copy_from_slice(&t.orden[..n]);
    dsk.field.n = n;
    dsk.field.cur = n;
    dsk.field.sug_base_n = 0;
    dsk.tick.repaint_field = true;
    if ctrl && dsk.field.ni < dsk.field.injected.len() {
        dsk.field.injected[dsk.field.ni] = b'\n';
        dsk.field.ni += 1;
    }
    true
}
