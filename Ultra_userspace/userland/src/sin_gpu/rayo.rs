//! **VOLCAR DETRAS DEL RAYO** -- no copiar donde la tarjeta esta leyendo.
//!
//! # Por que existe (2026-09-23, E1 de `docs/plan/PLAN_LA_3060.md`)
//!
//! Sin page flip, el volcado copia pixeles al mismo framebuffer que el escaner
//! de video esta leyendo. Si el rayo pasa por una caja mientras se copia, el
//! cuadro sale PARTIDO: arriba lo nuevo y abajo lo viejo (el tearing).
//!
//! El Ryzen dijo el 23-09 que la linea que barre la 3060 se lee por MMIO sin
//! firmware (`gpu`: SE PUEDE). Con eso, antes de copiar cada caja se pregunta
//! al kernel (`INFO_GPU_ESPERA`) cuanto falta para poder copiarla sin que el
//! rayo la cruce, y se espera eso:
//!
//! ```text
//!    <= 1 ms      girando: es menos que un latido
//!    >  1 ms      durmiendo por el LATIDO lo que pase del milisegundo, y
//!                 girando solo el final -- girar 10 ms le quitaria el CPU a
//!                 DOOM, que corre a la misma prioridad
//! ```
//!
//! Lo que tarda la copia POR FILA lo mide este fichero sobre sus propias
//! copias: el kernel sabe donde va el rayo; lo que cuesta mover pixeles lo sabe
//! quien los mueve.
//!
//! # Por que vive en `sin_gpu/`
//!
//! Porque cumple las dos reglas de la carpeta: existe solo porque no hay page
//! flip, y el dia que lo haya (M1 del plan) se borra entero -- con flip no se
//! copia nada, y lo que no se copia no se puede partir.
//!
//! Sin grafica que leer (otra placa, o el kernel no midio el rayo) la primera
//! pregunta contesta "no valida" y el rayo se APAGA: no vuelve a preguntar, y
//! el volcado es el de siempre.

use core::cell::Cell;

use crate::*;

/// Por debajo de esto se gira; por encima se duerme por el latido.
const GIRAR_MAX_NS: u64 = 1_000_000;

/// Lo que ha pasado al esperar al rayo. Lo pinta `gpu`.
#[derive(Clone, Copy, Default)]
pub struct CuentasRayo {
    /// Cajas que se preguntaron.
    pub preguntas: u64,
    /// De esas, las que tuvieron que esperar.
    pub esperas: u64,
    /// Lo esperado en total, y la peor espera, en ns.
    pub esperado_ns: u64,
    pub peor_ns: u64,
    /// Cajas cuya copia dura mas que lo que el rayo tarda en volver a ellas
    /// incluso esperando: esas se pueden partir, y solo el flip las arregla.
    pub no_caben: u64,
    /// Lo que cuesta copiar una fila, medido, en ns.
    pub ns_fila: u32,
    /// El rayo se pudo mirar (hay grafica y el kernel lo midio).
    pub activo: bool,
}

const SIN_PREGUNTAR: u8 = 0;
const ACTIVO: u8 = 1;
const APAGADO: u8 = 2;

pub(crate) struct Rayo {
    estado: Cell<u8>,
    tsc_hz: Cell<u64>,
    latido: Cell<u64>,
    cuentas: Cell<CuentasRayo>,
}

impl Rayo {
    pub(crate) const fn nuevo() -> Self {
        Self {
            estado: Cell::new(SIN_PREGUNTAR),
            tsc_hz: Cell::new(0),
            latido: Cell::new(0),
            cuentas: Cell::new(CuentasRayo {
                preguntas: 0,
                esperas: 0,
                esperado_ns: 0,
                peor_ns: 0,
                no_caben: 0,
                ns_fila: 0,
                activo: false,
            }),
        }
    }

    pub(crate) fn cuentas(&self) -> CuentasRayo {
        let mut c = self.cuentas.get();
        c.activo = self.estado.get() == ACTIVO;
        c
    }

    /// **Antes de copiar las filas `[y0, y1)`**: espera lo que haga falta.
    /// Devuelve el TSC de cuando se empieza a copiar, para [`Rayo::despues`].
    pub(crate) fn antes(&self, y0: u32, y1: u32) -> u64 {
        if self.estado.get() == SIN_PREGUNTAR {
            let hz = info(INFO_TSC_HZ);
            self.tsc_hz.set(hz);
            self.estado.set(if hz == 0 { APAGADO } else { ACTIVO });
        }
        if self.estado.get() != ACTIVO {
            return ciclos();
        }
        let mut c = self.cuentas.get();
        let sel = INFO_GPU_ESPERA
            | (y0 as u64 & GPU_ESPERA_FILAS_MASK) << GPU_ESPERA_Y0_SHIFT
            | (y1 as u64 & GPU_ESPERA_FILAS_MASK) << GPU_ESPERA_Y1_SHIFT
            | (c.ns_fila as u64).min(GPU_ESPERA_NS_FILA_MASK) << GPU_ESPERA_NS_FILA_SHIFT;
        let r = info(sel);
        if r & GPU_ESPERA_VALIDA == 0 {
            // No hay rayo que mirar: se apaga, y el volcado es el de siempre.
            self.estado.set(APAGADO);
            return ciclos();
        }
        c.preguntas += 1;
        if r & GPU_ESPERA_NO_CABE != 0 {
            c.no_caben += 1;
        }
        let ns = r & GPU_ESPERA_NS_MASK;
        if ns > 0 {
            self.esperar(ns);
            c.esperas += 1;
            c.esperado_ns += ns;
            c.peor_ns = c.peor_ns.max(ns);
        }
        self.cuentas.set(c);
        ciclos()
    }

    /// **Despues de copiar `filas` filas desde `t0`**: lo que cuesta una fila,
    /// con memoria (tres cuartos lo de antes, un cuarto lo de ahora) para que
    /// una copia rara no mueva la cuenta de golpe.
    pub(crate) fn despues(&self, filas: u32, t0: u64) {
        let hz = self.tsc_hz.get();
        if self.estado.get() != ACTIVO || filas == 0 || hz == 0 {
            return;
        }
        let ns = (ciclos().saturating_sub(t0) as u128 * 1_000_000_000 / hz as u128 / filas as u128) as u64;
        let ns = ns.min(u32::MAX as u64) as u32;
        let mut c = self.cuentas.get();
        c.ns_fila = if c.ns_fila == 0 { ns } else { (c.ns_fila as u64 * 3 / 4 + ns as u64 / 4) as u32 };
        self.cuentas.set(c);
    }

    fn esperar(&self, ns: u64) {
        let hz = self.tsc_hz.get();
        let fin = ciclos().saturating_add((ns as u128 * hz as u128 / 1_000_000_000) as u64);
        if ns > GIRAR_MAX_NS {
            if self.latido.get() == 0 {
                self.latido.set(latido_tomar().unwrap_or(0));
            }
            let h = self.latido.get();
            if h != 0 {
                // Dormir latido a latido hasta que quede menos de uno.
                let margen = (GIRAR_MAX_NS as u128 * hz as u128 / 1_000_000_000) as u64;
                while ciclos().saturating_add(margen) < fin {
                    let Some(visto) = latido_cuenta(h) else { break };
                    latido_esperar(h, visto, 0);
                }
            }
        }
        while ciclos() < fin {
            core::hint::spin_loop();
        }
    }
}
