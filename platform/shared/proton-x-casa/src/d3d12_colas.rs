//! **Las colas que ESPERAN en la GPU** (E2.1, 05-10): `ID3D12CommandQueue::
//! Wait`, y lo que se manda a una cola detras de el.
//!
//! [carril]  VERDE     solo lo que el `.exe` manda a sus colas
//! [cuesta]  DATO      una cola que corre antes de su valla lee lo que aun
//!                     no se ha escrito (y nadie lo dice: sale otro numero)
//! [riesgo]  ESPEJO    las reglas de Microsoft ("Synchronization and
//!                     Multi-Engine"): Wait hace que la COLA, no la CPU,
//!                     espere a que la valla llegue al valor; lo de detras
//!                     va en orden; una lista mandada es ya de la cola (se
//!                     puede reiniciar y volver a grabar enseguida)
//! [consumo] NADA      una copia de las ordenes de cada lista retenida
//!
//! La cola de la casa es SINCRONA: ExecuteCommandLists corre las ordenes en
//! el acto. Antes, Wait no hacia nada, y eso solo era verdad si la valla ya
//! habia llegado. Una cola de computo que espera a un valor que la directa da
//! DESPUES (o la CPU, o otro hilo) corria antes de tiempo. Ahora:
//!
//! ```text
//!    Wait(valla, x)       si la valla ya llego y la cola no espera nada, no
//!                         hace nada (lo de siempre). Si no, la cola queda
//!                         RETENIDA: lo que llegue detras se apunta en orden
//!    ExecuteCommandLists  en una cola retenida, se COPIAN sus ordenes (lo
//!                         mandado no cambia si la lista se reinicia)
//!    Signal(valla, x)     en una cola retenida, se apunta
//!    marcar una valla     (Signal de la CPU o de una cola) despierta a las
//!                         colas: cada una corre lo suyo hasta la siguiente
//!                         espera que no se cumpla, una y otra vez mientras
//!                         alguna avance (una marca suelta a otra cola)
//! ```
//!
//! Y lo de la CPU que espera a VARIAS vallas
//! (`SetEventOnMultipleFenceCompletion`, ALL y ANY): se mira en cada marca.
//!
//! **Los hilos cooperativos** (`hilos.rs`): una orden de D3D12 nunca cede el
//! turno, asi que una lista se graba y una cola corre de un tiron, y las
//! celdas de abajo no las toca nadie a la vez. Lo retenido lo corre QUIEN
//! marque la valla: puede ser otro hilo, en su pila (1 MiB por defecto: una
//! lista no la gasta, los dibujos van en el monton).
//!
//! **Lo que se rompe con H1 (Ring 3 en varios nucleos), dicho y NO arreglado
//! hoy:** estas celdas (`COLAS`) y todas las "una tarea" de la casa serian
//! carreras: el valor de una `Valla` y sus eventos (`d3d12::marcar`), las
//! consultas abiertas (`consultas::ABIERTAS`: con dos colas corriendo a la
//! vez se mezclarian sus cuentas; tendrian que ser de cada ejecucion), las
//! limpiezas apuntadas (`tuberia::LIMPIEZAS`), el registro de buferes que
//! crece mientras otro lo lee (`tuberia::BUFERES`), los contadores de COM
//! (no son atomicos) y el estado de una `Lista` (en D3D12 una lista es de un
//! hilo, pero su allocator y la cola no). Cuando H1 llegue, cada una con su
//! cerrojo o hecha de su hilo; y este juez (`prueba/multihilo.exe`) otra vez.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::d3d12::{self, Orden};
use crate::{aviso, hilos};

/// Lo que una cola retenida tiene apuntado, en orden.
enum Paso {
    /// Wait: hasta que la valla llegue al valor.
    Esperar(u64, u64),
    /// ExecuteCommandLists: las ordenes de cada lista, copiadas al mandarla.
    Correr(Vec<Vec<Orden>>),
    /// Signal de la cola.
    Marcar(u64, u64),
}

/// Una espera de la CPU a varias vallas: los pares (valla, valor), si son
/// TODAS (ALL) o alguna (ANY), y el evento.
struct Varias {
    pares: Vec<(u64, u64)>,
    todas: bool,
    evento: u64,
}

struct Estado {
    /// Las colas retenidas y lo suyo (una cola que se vacia, sale).
    retenidas: Vec<(u64, VecDeque<Paso>)>,
    varias: Vec<Varias>,
    /// Si ya se esta despertando (una marca DENTRO de despertar no vuelve a
    /// entrar: la vuelta de fuera la ve).
    despertando: bool,
}

struct Colas(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna orden
// de D3D12 cede el turno (ver la cabecera, y lo que H1 rompe).
// [hilos] cerrojo -- las colas retenidas y las esperas de varias vallas: las toca quien mande, quien marque y quien despierte
unsafe impl Sync for Colas {}
static COLAS: Colas = Colas(UnsafeCell::new(Estado { retenidas: Vec::new(), varias: Vec::new(), despertando: false }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Colas`; nadie guarda la referencia mas alla de un paso
    // (lo que llama fuera -- correr, marcar -- la vuelve a pedir).
    unsafe { &mut *COLAS.0.get() }
}

pub(crate) fn reiniciar() {
    let e = estado();
    e.retenidas.clear();
    e.varias.clear();
    e.despertando = false;
}

/// Lo apuntado de `cola`, si esta retenida.
fn pasos(cola: u64) -> Option<&'static mut VecDeque<Paso>> {
    estado().retenidas.iter_mut().find(|(c, _)| *c == cola).map(|(_, p)| p)
}

fn apuntar(cola: u64, p: Paso) {
    match pasos(cola) {
        Some(v) => v.push_back(p),
        None => estado().retenidas.push((cola, VecDeque::from([p]))),
    }
}

/// `ExecuteCommandLists`: en el acto, o apuntado si la cola espera.
pub(crate) fn ejecutar(cola: u64, n: u32, listas: *const u64) {
    if pasos(cola).is_none() {
        d3d12::ejecutar_listas(n, listas);
        return;
    }
    let mut copias = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        // SAFETY: `n` punteros a listas de la casa.
        let l = unsafe { d3d12::lista(listas.add(i).read()) };
        if l.abierta {
            aviso("ExecuteCommandLists con una lista sin Close: en Windows es un error, y no se corre");
            continue;
        }
        copias.push(l.ordenes.clone());
    }
    apuntar(cola, Paso::Correr(copias));
}

/// `Signal` de una cola: ya, o cuando le toque.
pub(crate) fn signal(cola: u64, valla: u64, valor: u64) {
    if pasos(cola).is_some() {
        apuntar(cola, Paso::Marcar(valla, valor));
    } else {
        d3d12::marcar(valla, valor);
    }
}

/// `Wait` de una cola: nada si ya se cumple y no espera a nada mas.
pub(crate) fn wait(cola: u64, valla: u64, valor: u64) {
    if pasos(cola).is_some() || d3d12::valor_de_valla(valla) < valor {
        apuntar(cola, Paso::Esperar(valla, valor));
    }
}

/// `SetEventOnMultipleFenceCompletion` de una espera que aun no se cumple.
pub(crate) fn esperar_varias(pares: Vec<(u64, u64)>, todas: bool, evento: u64) {
    estado().varias.push(Varias { pares, todas, evento });
}

/// Si una espera a varias vallas ya se cumple.
pub(crate) fn cumplida(pares: &[(u64, u64)], todas: bool) -> bool {
    let llego = |&(v, x): &(u64, u64)| d3d12::valor_de_valla(v) >= x;
    if todas {
        pares.iter().all(llego)
    } else {
        pares.iter().any(llego)
    }
}

/// El siguiente paso de la cola `i` de las retenidas, si se puede dar (una
/// espera cumplida se gasta sola).
fn siguiente(i: usize) -> Option<Paso> {
    let v = &mut estado().retenidas[i].1;
    loop {
        match v.front()? {
            Paso::Esperar(valla, x) if d3d12::valor_de_valla(*valla) < *x => return None,
            Paso::Esperar(..) => {
                v.pop_front();
            }
            _ => return v.pop_front(),
        }
    }
}

/// **Despertar a las colas** tras marcar una valla (ver la cabecera).
pub(crate) fn despertar() {
    if estado().despertando {
        return;
    }
    estado().despertando = true;
    loop {
        let mut avanzo = false;
        let mut i = 0;
        while i < estado().retenidas.len() {
            match siguiente(i) {
                Some(Paso::Correr(listas)) => listas.iter().for_each(|o| d3d12::correr(o)),
                Some(Paso::Marcar(valla, x)) => d3d12::marcar(valla, x),
                Some(Paso::Esperar(..)) | None => {
                    i += 1;
                    continue;
                }
            }
            avanzo = true;
        }
        estado().retenidas.retain(|(_, p)| !p.is_empty());
        // Las esperas de la CPU a varias vallas que ya se cumplen.
        let e = estado();
        let mut k = 0;
        while k < e.varias.len() {
            if cumplida(&e.varias[k].pares, e.varias[k].todas) {
                hilos::encender_evento(e.varias.swap_remove(k).evento);
            } else {
                k += 1;
            }
        }
        if !avanzo {
            break;
        }
    }
    estado().despertando = false;
}
