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
//!    la 3060 no paga      A LA PRIMERA: el canal GR tomo un Xid y esta
//!                         muerto hasta reiniciar; insistir costo 1 s por
//!                         lote en el metal (28-09). Se dice la escalera
//! ```
//!
//! # POR PARTES (29-09)
//!
//! El metal (28-09 22:36): BMOX-12 por la 3060 a 59-90 fps con `dibujar`
//! 10-15 ms, y la 3060 dibujando en ~1 ms. Para saber DONDE se va el resto,
//! una linea por segundo:
//!
//! ```text
//!    [3060] N lotes/s; por lote: puerta P us (la receta, en esta app),
//!           kernel K us = 3060 A + preparar B + sombra C + resto R
//! ```
//!
//! `A` es la espera del semaforo tras el timbre (el dibujo); `B`, escribir
//! ordenes y datos en la VRAM; `C`, las copias de la sombra por el motor de
//! copia; `R`, todo lo demas del syscall: prestar y devolver el back buffer y
//! las texturas por la IOMMU, pegar y juzgar los programas, las tablas.

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
    /// Lo medido desde la ultima linea `[3060]` (ver POR PARTES).
    partes: Partes,
}

/// Las sumas de un segundo: lotes y ns/us de cada parte.
#[derive(Clone, Copy, Default)]
struct Partes {
    desde_ns: u64,
    lotes: u64,
    puerta_ns: u64,
    kernel_ns: u64,
    tarjeta_us: u64,
    preparar_us: u64,
    sombra_us: u64,
}

impl Partes {
    /// Suma un lote; pasado un segundo, dice la linea y empieza otra.
    fn sumar(&mut self, puerta_ns: u64, kernel_ns: u64, r: u64, ahora: u64) {
        if self.desde_ns == 0 {
            self.desde_ns = ahora;
        }
        let (us, _, _, _) = puerta::desempaquetar(r);
        self.lotes += 1;
        self.puerta_ns += puerta_ns;
        self.kernel_ns += kernel_ns;
        self.tarjeta_us += us as u64;
        self.preparar_us += puerta::preparado(r).1 as u64;
        self.sombra_us += puerta::copia_us(r) as u64;
        let pasado = ahora.saturating_sub(self.desde_ns);
        if pasado >= 1_000_000_000 {
            let n = self.lotes.max(1);
            let (k, a, b, c) = (self.kernel_ns / 1000 / n, self.tarjeta_us / n, self.preparar_us / n, self.sombra_us / n);
            bmo::consola(&alloc::format!(
                "[3060] {} lotes/s; por lote: puerta {} us, kernel {k} us = 3060 {a} + preparar {b} + sombra {c} + resto {}\n",
                self.lotes * 1_000_000_000 / pasado.max(1),
                self.puerta_ns / 1000 / n,
                k.saturating_sub(a + b + c),
            ));
            *self = Partes { desde_ns: ahora, ..Partes::default() };
        }
    }
}

struct Celda(core::cell::UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Celda {}
static ESTADO: Celda = Celda(core::cell::UnsafeCell::new(Estado { puerta: None, dichos: Vec::new(), negados: 0, apagada: false, por_la_3060: 0, partes: Partes { desde_ns: 0, lotes: 0, puerta_ns: 0, kernel_ns: 0, tarjeta_us: 0, preparar_us: 0, sombra_us: 0 } }));

fn decir(e: &mut Estado, motivo: String) {
    if e.dichos.iter().any(|d| *d == motivo) {
        return;
    }
    bmo::consola("PROTON-X: este lote va por la CPU: ");
    bmo::consola(&motivo);
    bmo::consola("\n");
    e.dichos.push(motivo);
}

/// **Un dibujo que la 3060 no pago**, dicho entero: cuanto espero el kernel,
/// hasta que escalon llego, y lo que se hace.
fn no_pagado(r: u64) -> String {
    let (us, tris, etapas, lanzado) = puerta::desempaquetar(r);
    let si = |b: u32| if etapas & b != 0 { "SI" } else { "NO" };
    let donde = match (lanzado, etapas) {
        (false, _) => "no se lanzo: el kernel no pudo poner la receta en el canal",
        (true, 0) => "no pago NI el estado: el canal GR ya estaba muerto (un Xid anterior; el `gsp aviso` de `gpu verrano` lo dice)",
        (true, 1) => "el estado SI, los vertices NO: se paro en el programa de VERTICES",
        (true, _) => "los vertices SI, el dibujo NO: se paro al RASTERIZAR o en el programa de PIXEL (un Xid 69 = un metodo o un valor que el motor no acepta)",
    };
    alloc::format!(
        "PROTON-X: la 3060 NO PAGO un lote de {tris} triangulo(s): el kernel espero {} ms; escalera: estado {} vertices {} dibujo {} -> {donde}.\n\
         PROTON-X: un canal GR que no paga queda MUERTO hasta reiniciar: la 3060 se deja YA (cada lote mas costaria otro segundo); el resto, por la CPU (contesto {r:#x})\n",
        us / 1000,
        si(1),
        si(2),
        si(4),
    )
}

/// **El ejecutor** (`Plataforma::dibujar`).
pub fn dibujar(l: &Lote, d: &mut Destino) -> Result<Cuenta, NoDibuja> {
    // SAFETY: ver `Celda`.
    let e = unsafe { &mut *ESTADO.0.get() };
    if !e.apagada {
        let blanco = Blanco { va: d.pixeles.as_ptr() as u64, ancho: d.ancho, alto: d.alto, bgra: d.bgra };
        let p = e.puerta.get_or_insert_with(Puerta::nueva);
        let t0 = crate::plataforma::ahora_ns();
        match p.preparar(l, blanco) {
            Ok(_) => {
                let caja = p.caja.as_ptr() as u64;
                let t1 = crate::plataforma::ahora_ns();
                let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIBUJAR, caja);
                let t2 = crate::plataforma::ahora_ns();
                match r {
                    Ok(r) if puerta::sano(r) => {
                        e.partes.sumar(t1 - t0, t2 - t1, r, t2);
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
                        // El metal (28-09): cada NO pagado costo 1 s entero
                        // (el tope del kernel) y 0 fps. Un canal GR que no
                        // paga tomo una excepcion (Xid): esta MUERTO hasta
                        // reiniciar y cada lote mas esperaria otro segundo.
                        // Se deja a la PRIMERA.
                        p.despues(l, false);
                        e.apagada = true;
                        bmo::consola(&no_pagado(r));
                    }
                    Err(m) if m == bmo::IOMMU_NO_CANAL_MUERTO => {
                        p.despues(l, false);
                        e.apagada = true;
                        bmo::consola("PROTON-X: el kernel dice que el canal de GR de la 3060 esta MUERTO (tomo un Xid; el numero, en `cabina`): la 3060 se deja YA; el resto, por la CPU. Reinicia para recuperarla\n");
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
