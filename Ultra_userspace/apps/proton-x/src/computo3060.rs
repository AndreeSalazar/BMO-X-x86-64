//! **LI2f: EL COMPUTO DEL JUEGO POR LA 3060** (13-10) -- el `Plataforma::
//! computar` de BMO-X: un `Dispatch` de D3D12 va a la puerta estrecha del
//! computo (`IOMMU_OP_GPU_COMPUTO`) si se puede, y si no, la casa lo corre
//! en la CPU como siempre.
//!
//! ```text
//!    el Programa  emitido UNA vez por PSO (`emitir_con`, `Abi::Registros`) y
//!                 guardado; lo que no se emite (un UAV con tipo, la barrera
//!                 que no se demuestra uniforme...) se dice una vez y va por
//!                 la CPU para siempre
//!    la receta    VRC1 (`receta_computo`): sus buferes son la memoria del
//!                 juego TAL CUAL (el kernel la presta), el cbuffer, sus filas
//!    el kernel    la copia, la juzga, presta, despacha y devuelve: `Ok` = los
//!                 UAV ya tienen lo escrito
//! ```
//!
//! Solo con `--3060` (el modo 3060): sin el, nada cambia.
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: lo dira el primer `Dispatch`.
//! Un NO del kernel con el canal VIVO deja los UAV como estaban (no se
//! lanzo); uno que se lanzo y no volvio, no: el canal queda MUERTO y la CPU
//! lo corre sobre lo que la 3060 dejara a medias.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::bufer::{Bufer, Uav};
use bmo_proton_x::dxil::programa::Programa;
use bmo_proton_x_sm86::{computo_app, emitir_con, vrc1, Abi, Emitido, Precarga, CONTADOR, SRV};
use bmo_userland as bmo;

/// Lo emitido de un PSO, por su clave (`None`: no se emite, por la CPU).
struct Guardado {
    clave: (usize, usize, [u32; 3], u32),
    emitido: Option<Box<Emitido>>,
}

struct Estado {
    guardados: Vec<Guardado>,
    /// Los motivos ya dichos.
    dichos: Vec<String>,
    /// Despachos por la 3060 (para decir el primero).
    hechos: u64,
    /// NO seguidos del kernel; a 3, el computo se deja.
    negados: u32,
    /// La caja de la receta (un bloque del proceso).
    caja: Vec<u8>,
}

struct Celda(core::cell::UnsafeCell<Estado>);
// SAFETY: solo se toca con `EN_MARCHA` tomado (`computar`).
// [hilos] hecho -- su cerrojo (`EN_MARCHA`): un segundo hilo que despacha a la vez va por la CPU
unsafe impl Sync for Celda {}
/// El cerrojo del estado: uno despacha por la 3060 a la vez.
static EN_MARCHA: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static ESTADO: Celda = Celda(core::cell::UnsafeCell::new(Estado { guardados: Vec::new(), dichos: Vec::new(), hechos: 0, negados: 0, caja: Vec::new() }));

fn decir(e: &mut Estado, motivo: String) {
    if !e.dichos.contains(&motivo) {
        bmo::consola(&alloc::format!("PROTON-X: un computo va por la CPU: {motivo}\n"));
        e.dichos.push(motivo);
    }
}

/// La clave de un Programa: su direccion (la del PSO, que vive lo que el
/// PSO) y lo que lo distingue si otro PSO cayera en la misma.
fn clave(p: &Programa) -> (usize, usize, [u32; 3], u32) {
    (p as *const Programa as usize, p.ops.len(), p.computo.hilos, p.computo.compartida)
}

/// **`Plataforma::computar`**: con el cerrojo tomado; si otro hilo esta
/// despachando, este va por la CPU.
pub fn computar(p: &Programa, grupos: [u32; 3], cb: &[u8], buferes: &[Option<Bufer>], uavs: &mut [Option<Uav>]) -> bool {
    use core::sync::atomic::Ordering;
    if !crate::la3060::en_modo_3060() || crate::la3060::apagada() || EN_MARCHA.swap(true, Ordering::AcqRel) {
        return false;
    }
    // SAFETY: `EN_MARCHA` tomado arriba: nadie mas toca el estado.
    let r = con_el_estado(unsafe { &mut *ESTADO.0.get() }, p, grupos, cb, buferes, uavs);
    EN_MARCHA.store(false, Ordering::Release);
    r
}

fn con_el_estado(e: &mut Estado, p: &Programa, grupos: [u32; 3], cb: &[u8], buferes: &[Option<Bufer>], uavs: &mut [Option<Uav>]) -> bool {
    if e.negados >= 3 {
        return false;
    }
    let k = clave(p);
    let i = match e.guardados.iter().position(|g| g.clave == k) {
        Some(i) => i,
        None => {
            let emitido = match emitir_con(p, bmo_proton_x_sm86::puerta::REGISTROS, Abi::Registros) {
                Ok(x) => Some(Box::new(x)),
                Err(x) => {
                    decir(e, alloc::format!("el emisor de la 3060 no puede con su CS ({x:?})"));
                    None
                }
            };
            e.guardados.push(Guardado { clave: k, emitido });
            e.guardados.len() - 1
        }
    };
    let Some(em) = e.guardados[i].emitido.as_deref() else { return false };
    // Los buferes de la receta: la memoria del juego, por ranura.
    let mut lista: Vec<vrc1::Bufer> = Vec::new();
    for q in &em.precargas {
        let Precarga::Uav { u, .. } = *q else { continue };
        if lista.iter().any(|b| b.ranura == u) {
            continue;
        }
        let (va, bytes, elementos, paso, modo_ok) = if u & CONTADOR != 0 && u & SRV == 0 {
            // El contador oculto de un UAV: su palabra, como un crudo de una.
            match uavs.get((u & !CONTADOR) as usize) {
                Some(Some(Uav { contador: Some(c), .. })) => (&**c as *const u32 as u64, 4, 1, 0, true),
                _ => (0, 0, 0, 0, false),
            }
        } else if u & SRV != 0 {
            match buferes.get((u & !SRV) as usize) {
                Some(Some(b)) => (b.bytes.as_ptr() as u64, b.bytes.len(), b.elementos, b.paso, b.formato == 0),
                _ => (0, 0, 0, 0, false),
            }
        } else {
            match uavs.get(u as usize) {
                Some(Some(w)) => (w.bytes.as_ptr() as u64, w.bytes.len(), w.elementos, w.paso, w.formato == 0),
                _ => (0, 0, 0, 0, false),
            }
        };
        if !modo_ok || bytes < 4 || bytes > u32::MAX as usize || va % 4 != 0 {
            decir(e, alloc::format!("un bufer (ranura {u:#x}) que la 3060 no puede ver: sin atar, con formato, o sin alinear"));
            return false;
        }
        lista.push(vrc1::Bufer { va, bytes: bytes as u32, elementos, paso, ranura: u });
    }
    let filas: Vec<[u32; 4]> = cb.chunks(16).map(|c| core::array::from_fn(|j| c.get(4 * j..4 * j + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])))).collect();
    let medida = vrc1::MAX_RECETA.min(vrc1::CABECERA + 16 + 16 * em.codigo.len() + 16 * filas.len() + 24 * lista.len() + 4 * em.precargas.len());
    if e.caja.len() < medida {
        e.caja = alloc::vec![0; medida];
    }
    if bmo_proton_x_sm86::receta_computo(&mut e.caja, em, p, grupos, &lista, &filas).is_none() {
        decir(e, String::from("la receta de computo no cabe"));
        return false;
    }
    let va = e.caja.as_ptr() as u64;
    match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COMPUTO, va) {
        Ok(v) => {
            let (us, _, _) = computo_app::desempaquetar(v);
            e.negados = 0;
            e.hechos += 1;
            if e.hechos == 1 {
                bmo::consola(&alloc::format!("PROTON-X: el PRIMER computo del juego por la 3060 ({} grupos de {} hilos, {} instrucciones): {us} us\n", grupos[0] * grupos[1] * grupos[2], p.computo.hilos.iter().product::<u32>(), em.codigo.len()));
            }
            true
        }
        Err(m) if m == bmo::IOMMU_NO_CANAL_MUERTO => {
            crate::la3060::apagar();
            bmo::consola("PROTON-X: el kernel dice que el canal de GR de la 3060 esta MUERTO: el computo, por la CPU\n");
            false
        }
        Err(m) => {
            e.negados += 1;
            let porque = crate::la3060::porque_del_kernel();
            decir(e, alloc::format!("el kernel dijo que no al computo (motivo {m}): {porque}"));
            if e.negados >= 3 {
                bmo::consola("PROTON-X: tres NO seguidos del kernel al computo: el computo se deja; por la CPU\n");
            }
            false
        }
    }
}
