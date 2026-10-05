//! **Las consultas de OCLUSION y la PREDICACION de D3D12** (E2.7, 05-10):
//! cuantos pixeles de unos dibujos pasan la prueba de profundidad, y saltarse
//! ordenes segun un numero que esta en un bufer.
//!
//! [carril]  VERDE     solo lo que la lista apunto, al ejecutarla
//! [cuesta]  DATO      una consulta mal contada hace que el motor NO dibuje
//!                     lo que se ve (o que dibuje lo tapado: solo cuesta)
//! [riesgo]  ESPEJO    las reglas de Microsoft ("Predication" y "Queries"):
//!                     una consulta de oclusion empieza y acaba en la MISMA
//!                     lista; toda lista empieza sin predicacion; el numero
//!                     se lee cuando se EJECUTA SetPredication ("snaps the
//!                     value"), no al apuntarlo
//! [consumo] NADA      un contador por consulta abierta
//!
//! ```text
//!    BeginQuery(OCLUSION | BINARIA)   se abre: desde aqui cada dibujo suma
//!                                     sus pixeles que PASAN (los cuenta la
//!                                     trama: `Cuenta::pasan`)
//!    EndQuery                         se cierra: OCLUSION el numero, BINARIA
//!                                     0 o 1; ResolveQueryData lo copia
//!    SetPredication(bufer, op)        lee su u64: EQUAL_ZERO salta si es 0,
//!                                     NOT_EQUAL_ZERO si no lo es; hasta el
//!                                     siguiente SetPredication o el fin de
//!                                     la lista
//! ```
//!
//! Lo que se SALTA es lo que dice Microsoft: los Draw, los Dispatch,
//! ExecuteIndirect, las copias (CopyBufferRegion, CopyTextureRegion,
//! CopyResource) y las limpiezas. No: las consultas, ResolveQueryData,
//! WriteBufferImmediate ni las copias atomicas. Un bundle no la pone, pero la
//! hereda (sus ordenes van dentro de la lista: `execute_bundle`).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::aviso;
use crate::d3d12::Orden;

/// D3D12_QUERY_TYPE_OCCLUSION y D3D12_QUERY_TYPE_BINARY_OCCLUSION.
pub(crate) const OCLUSION: u32 = 0;
pub(crate) const BINARIA: u32 = 1;

/// Una consulta abierta: su monton, su indice y los pixeles que pasaron.
struct Abierta {
    monton: u64,
    indice: u32,
    pasan: u64,
}

struct Abiertas(UnsafeCell<Vec<Abierta>>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y una lista se
// ejecuta entera sin ceder el turno.
unsafe impl Sync for Abiertas {}
static ABIERTAS: Abiertas = Abiertas(UnsafeCell::new(Vec::new()));

fn abiertas() -> &'static mut Vec<Abierta> {
    // SAFETY: ver `Abiertas`.
    unsafe { &mut *ABIERTAS.0.get() }
}

pub(crate) fn reiniciar() {
    abiertas().clear();
}

/// Al empezar a ejecutar una lista: ninguna consulta abierta.
pub(crate) fn al_empezar_lista() {
    abiertas().clear();
}

/// Al acabarla: una consulta que sigue abierta no se cierra en otra lista.
pub(crate) fn al_acabar_lista() {
    if !abiertas().is_empty() {
        aviso("BeginQuery sin su EndQuery en la misma lista: en Windows es un error; la consulta no se escribe");
        abiertas().clear();
    }
}

/// `BeginQuery` de una consulta de oclusion, al ejecutarse.
pub(crate) fn abrir(monton: u64, indice: u32) {
    if abiertas().iter().any(|a| a.monton == monton && a.indice == indice) {
        aviso("BeginQuery de una consulta ya abierta: en Windows es un error; sigue contando la de antes");
        return;
    }
    abiertas().push(Abierta { monton, indice, pasan: 0 });
}

/// Si hay alguna consulta de oclusion abierta: el que dibuje tiene que
/// contar (`Lote::oclusion`).
pub(crate) fn hay_abierta() -> bool {
    !abiertas().is_empty()
}

/// Los pixeles que pasaron en un dibujo: a todas las abiertas.
pub(crate) fn sumar(pasan: u64) {
    for a in abiertas().iter_mut() {
        a.pasan += pasan;
    }
}

/// `EndQuery` de una consulta de oclusion, al ejecutarse: su resultado
/// (D3D12_QUERY_DATA: el numero, o 0/1 la BINARIA), o `None` si no estaba
/// abierta en esta lista.
pub(crate) fn cerrar(monton: u64, indice: u32, tipo: u32) -> Option<u64> {
    let v = abiertas();
    let i = v.iter().position(|a| a.monton == monton && a.indice == indice)?;
    let pasan = v.remove(i).pasan;
    Some(if tipo == BINARIA { (pasan > 0) as u64 } else { pasan })
}

/// `SetPredication`, al ejecutarse: si lo que venga detras se SALTA. `dir`
/// es el u64 que se mira (0: sin predicacion); `op` 0 es EQUAL_ZERO y 1
/// NOT_EQUAL_ZERO (comprobados al apuntar).
pub(crate) fn salta(dir: u64, op: u32) -> bool {
    if dir == 0 {
        return false;
    }
    // SAFETY: comprobado al apuntar: ocho bytes dentro de un bufer de la casa
    // (que no se libera).
    let v = unsafe { (dir as *const u64).read_unaligned() };
    if op == 0 {
        v == 0
    } else {
        v != 0
    }
}

/// Si la predicacion salta esta orden (ver la cabecera).
pub(crate) fn predicable(o: &Orden) -> bool {
    match o {
        Orden::Limpiar { .. } | Orden::LimpiarTexel { .. } | Orden::LimpiarUav { .. } | Orden::Dibujar { .. } | Orden::Region(_) | Orden::Bytes { .. } | Orden::Entero { .. } | Orden::Despachar { .. } | Orden::Indirecto { .. } => true,
        Orden::Atomica { .. } | Orden::Consulta { .. } | Orden::Empezar { .. } | Orden::Resolver { .. } | Orden::Escribir { .. } | Orden::Predicar { .. } => false,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_binaria_dice_cero_o_uno_y_la_otra_el_numero() {
        reiniciar();
        abrir(7, 0);
        abrir(7, 1);
        sumar(5);
        assert_eq!(cerrar(7, 0, BINARIA), Some(1));
        sumar(3);
        assert_eq!(cerrar(7, 1, OCLUSION), Some(8), "la abierta suma los dos dibujos");
        assert_eq!(cerrar(7, 1, OCLUSION), None, "ya cerrada");
        abrir(7, 2);
        assert_eq!(cerrar(7, 2, BINARIA), Some(0), "sin pixeles, oculta");
        assert!(!hay_abierta());
    }

    #[test]
    fn la_predicacion_lee_su_u64() {
        let v = [0u64, 3];
        assert!(!salta(0, 0), "sin bufer, nada se salta");
        assert!(salta(&v[0] as *const u64 as u64, 0), "EQUAL_ZERO con un 0");
        assert!(!salta(&v[1] as *const u64 as u64, 0));
        assert!(salta(&v[1] as *const u64 as u64, 1), "NOT_EQUAL_ZERO con un 3");
        assert!(!salta(&v[0] as *const u64 as u64, 1));
    }
}
