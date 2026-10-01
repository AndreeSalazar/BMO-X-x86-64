//! **LAS SOLAPAS DE EJECUTAR** (01-10). Pedido: *"el control + alt tiene que
//! tener solapas para facilitar procesos"*.
//!
//! [consumo] NADA      no corre solo: se toca al pulsar una solapa o un atajo
//!                     (L6h)
//!
//! La caja ya pintaba una solapa y un `+` desde el 25-09, de adorno. Ahora son
//! de verdad: hasta [`MAX`], cada una con SU salida (la rejilla entera, con su
//! historial y su busqueda) y SU linea (con su historial de ordenes).
//!
//! ```text
//!    la de delante   vive donde siempre: `dsk.out.grid` y `dsk.field`, asi
//!                    que nada de lo que ya escribe ahi se entera de esto
//!    las demas       esperan en `ALMACEN`, en .bss (~20 KiB cada una)
//!    cambiar         la de delante se copia a su sitio y la elegida se copia
//!                    delante: dos copias de memoria, sin pila (son grandes)
//! ```
//!
//! Atajos: **Ctrl+N** una nueva, **Ctrl+Tab** la siguiente, **Ctrl+Shift+W**
//! cierra la de delante; y con el raton, la solapa, su `x` y el `+`. La ultima
//! no se cierra: Ejecutar es la linea de ordenes de la maquina.
//!
//! [!] Lo que un programa lanzado escriba va a la solapa de DELANTE, sea cual
//! sea: hay una sola consola. Se dice aqui para que no sorprenda.

use core::mem::MaybeUninit;

use crate::desktop::{Desktop, Field};
use crate::scene::output::Output;

pub(crate) use crate::scene::solapas::{activa, lista, MAX};
use crate::scene::solapas::estado;

/// La salida y la linea de cada solapa que no esta delante. La de delante
/// tiene aqui su hueco sin usar.
static mut ALMACEN: MaybeUninit<[(Output, Field); MAX]> = MaybeUninit::uninit();

fn hueco(k: usize) -> *mut (Output, Field) {
    // SAFETY: `k < MAX`, dentro del arreglo; quien lo usa sabe si esta escrito.
    unsafe { (core::ptr::addr_of_mut!(ALMACEN) as *mut (Output, Field)).add(k) }
}

/// La de delante a su hueco.
fn guardar(dsk: &mut Desktop) {
    let a = estado().activa;
    // SAFETY: dos sitios distintos de memoria nuestra; los dos tipos son solo
    // arreglos y numeros (sin nada que soltar), asi que copiar es mover.
    unsafe {
        core::ptr::copy_nonoverlapping(&dsk.out.grid, core::ptr::addr_of_mut!((*hueco(a)).0), 1);
        core::ptr::copy_nonoverlapping(&dsk.field, core::ptr::addr_of_mut!((*hueco(a)).1), 1);
    }
}

/// La `k`, que esta en su hueco, delante.
fn cargar(dsk: &mut Desktop, k: usize) {
    // SAFETY: como en `guardar`; `k` esta escrita porque esta viva.
    unsafe {
        core::ptr::copy_nonoverlapping(core::ptr::addr_of!((*hueco(k)).0), &mut dsk.out.grid, 1);
        core::ptr::copy_nonoverlapping(core::ptr::addr_of!((*hueco(k)).1), &mut dsk.field, 1);
    }
    dsk.out.grid.dirty = true;
    dsk.tick.repaint_field = true;
}

/// **Poner delante la `k`.** `false` si ya lo esta o no existe.
pub(crate) fn ir(dsk: &mut Desktop, k: usize) -> bool {
    let e = estado();
    if k >= MAX || !e.vivas[k] || k == e.activa {
        return false;
    }
    guardar(dsk);
    cargar(dsk, k);
    e.activa = k;
    true
}

/// **La siguiente**, dando la vuelta.
pub(crate) fn siguiente(dsk: &mut Desktop) -> bool {
    let (l, n) = lista();
    let a = activa();
    let i = l[..n].iter().position(|&k| k == a).unwrap_or(0);
    ir(dsk, l[(i + 1) % n])
}

/// **Una nueva, delante.** Hereda el historial de ordenes y el portapapeles
/// de la de delante (lo que acabas de escribir, a mano); la salida empieza
/// limpia. `false` si ya hay [`MAX`].
pub(crate) fn nueva(dsk: &mut Desktop) -> bool {
    let e = estado();
    let Some(k) = (0..MAX).find(|&k| !e.vivas[k]) else { return false };
    guardar(dsk);
    // SAFETY: el hueco `k` es nuestro; se escribe entero antes de leerlo.
    unsafe {
        core::ptr::addr_of_mut!((*hueco(k)).0).write(Output::new());
        core::ptr::addr_of_mut!((*hueco(k)).1).write(Field::new());
        let f = &mut (*hueco(k)).1;
        core::ptr::copy_nonoverlapping(&dsk.field.history, &mut f.history, 1);
        f.history.cursor = f.history.n;
        f.clipboard = dsk.field.clipboard;
        f.clipboard_n = dsk.field.clipboard_n;
    }
    cargar(dsk, k);
    e.vivas[k] = true;
    e.activa = k;
    e.numero[k] = e.siguiente;
    e.siguiente = e.siguiente % 9 + 1;
    e.nombre_n[k] = 0;
    dsk.out.grid.text(b"  solapa nueva: Ctrl+Tab cambia, Ctrl+Shift+W la cierra\n");
    true
}

/// **Cerrar la `k`.** La ultima no se cierra. Si es la de delante, pasa
/// delante la de su izquierda (o la de su derecha).
pub(crate) fn cerrar(dsk: &mut Desktop, k: usize) -> bool {
    let (l, n) = lista();
    let e = estado();
    if n <= 1 || k >= MAX || !e.vivas[k] {
        return false;
    }
    if k == e.activa {
        let i = l[..n].iter().position(|&x| x == k).unwrap_or(0);
        let otra = if i > 0 { l[i - 1] } else { l[1] };
        ir(dsk, otra);
    }
    e.vivas[k] = false;
    e.nombre_n[k] = 0;
    true
}
