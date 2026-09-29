//! **P3b4c: EL EJECUTOR DE LA 3060** (`Plataforma::dibujar`) -- cada lote de
//! D3D12 del `.exe`, a la 3060 por la PUERTA ESTRECHA del kernel
//! (`IOMMU_OP_GPU_DIBUJAR`); y si no se puede, en la CPU con los sombreadores
//! nativos de la casa, DICIENDO por que.
//!
//! La CPU dirige: traduce el PSO una vez (`bmo_proton_x_sm86::puerta`), copia
//! los datos del lote a la receta y llama. La 3060 dibuja en el back buffer
//! de la casa (la RAM de esta app), y el `Present` de siempre lo lleva a la
//! ventana.
//!
//! ```text
//!    la puerta dice NO    se dice UNA vez por motivo (EL REGISTRO), y ese
//!                         lote va por la CPU
//!    el kernel dice NO    tres seguidos: la 3060 se deja para el resto de
//!                         la vida del proceso (sin escritorio que la haya
//!                         preparado no va a cambiar), y se dice
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::lote::{Lote, NoDibuja};
use bmo_proton_x::trama::{Cuenta, Destino};
use bmo_proton_x_sm86::puerta::{self, Blanco, Puerta};
use bmo_userland as bmo;

struct Estado {
    puerta: Option<Puerta>,
    /// Los motivos ya dichos (cada uno una vez).
    dichos: Vec<String>,
    /// NO seguidos del kernel; a 3, la 3060 se deja.
    negados: u32,
    apagada: bool,
    /// Lotes dibujados por la 3060 (para decir el primero).
    por_la_3060: u64,
}

struct Celda(core::cell::UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Celda {}
static ESTADO: Celda = Celda(core::cell::UnsafeCell::new(Estado { puerta: None, dichos: Vec::new(), negados: 0, apagada: false, por_la_3060: 0 }));

fn decir(e: &mut Estado, motivo: String) {
    if e.dichos.iter().any(|d| *d == motivo) {
        return;
    }
    bmo::consola("PROTON-X: este lote va por la CPU: ");
    bmo::consola(&motivo);
    bmo::consola("\n");
    e.dichos.push(motivo);
}

/// **El ejecutor** (`Plataforma::dibujar`).
pub fn dibujar(l: &Lote, d: &mut Destino) -> Result<Cuenta, NoDibuja> {
    // SAFETY: ver `Celda`.
    let e = unsafe { &mut *ESTADO.0.get() };
    if !e.apagada {
        let blanco = Blanco { va: d.pixeles.as_ptr() as u64, ancho: d.ancho, alto: d.alto, bgra: d.bgra };
        let p = e.puerta.get_or_insert_with(Puerta::nueva);
        match p.preparar(l, blanco) {
            Ok(_) => {
                let caja = p.caja.as_ptr() as u64;
                match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIBUJAR, caja) {
                    Ok(r) if puerta::sano(r) => {
                        p.despues(l, true);
                        e.negados = 0;
                        if e.por_la_3060 == 0 {
                            bmo::consola("PROTON-X: la 3060 dibuja los lotes (P3b4c: la puerta estrecha, la receta VRN2)\n");
                        }
                        e.por_la_3060 += 1;
                        let (_, tris, _, _) = puerta::desempaquetar(r);
                        return Ok(Cuenta { dibujados: tris, ..Cuenta::default() });
                    }
                    Ok(r) => {
                        p.despues(l, false);
                        decir(e, alloc::format!("la 3060 no pago el dibujo entero (contesto {r:#x})"));
                    }
                    Err(m) => {
                        p.despues(l, false);
                        e.negados += 1;
                        decir(e, alloc::format!("el kernel dijo que no a la receta (motivo {m}; el porque, en `cabina fallos`)"));
                        if e.negados >= 3 {
                            e.apagada = true;
                            bmo::consola("PROTON-X: tres NO seguidos del kernel: la 3060 se deja; el resto, por la CPU\n");
                        }
                    }
                }
            }
            Err(motivo) => {
                p.despues(l, false);
                decir(e, motivo);
            }
        }
    }
    bmo_proton_x_casa::nativo::dibujar(l, d)
}
