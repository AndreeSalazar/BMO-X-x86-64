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
//! Lo que tarda la copia lo mide este fichero sobre sus propias copias: el
//! kernel sabe donde va el rayo; lo que cuesta mover pixeles lo sabe quien los
//! mueve.
//!
//! ** Se mide POR PIXEL, no por fila (2026-09-23). La primera version media
//! nanosegundos por fila y los mezclaba entre cajas: la fila del cursor (16
//! px) y la de la pantalla entera (1920) entraban en la misma media. El `save`
//! de las 15:52 dio `131 ns/fila`, que no es la fila de NINGUNA caja: la
//! pantalla entera le pedia al kernel un plazo con una copia que se quedaba
//! corta, y el cursor uno que le sobraba. Ahora se guarda el coste de un
//! pixel y cada caja pide con su ancho.
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
/// Lo mas que se cree una espera: un cuadro a 50 Hz. A 60 Hz, un cuadro
/// son 16,7 ms; mas que esto no puede pedirlo una cuenta sana.
const ESPERA_MAX_NS: u64 = 20_000_000;
/// Lo mas que se pregunta por una caja. Una espera sana acaba en una o dos.
const VUELTAS_MAX: u32 = 4;

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
    /// Cajas cuya espera se PERDIO (se desperto tarde y el rayo ya volvia a
    /// estar encima) y que se copiaron igual, en vez de esperar otro cuadro
    /// entero. Pueden haber salido partidas.
    pub rendidas: u64,
    /// Lo que cuesta copiar UN pixel, medido, en picosegundos (una fila de
    /// 1920 son `ps_px * 1920 / 1000` ns).
    pub ps_px: u32,
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
                rendidas: 0,
                ps_px: 0,
                activo: false,
            }),
        }
    }

    pub(crate) fn cuentas(&self) -> CuentasRayo {
        let mut c = self.cuentas.get();
        c.activo = self.estado.get() == ACTIVO;
        c
    }

    /// **Antes de copiar las filas `[y0, y1)` de `ancho` pixeles**: espera lo
    /// que haga falta. Devuelve el TSC de cuando se empieza a copiar, para
    /// [`Rayo::despues`].
    pub(crate) fn antes(&self, y0: u32, y1: u32, ancho: u32) -> u64 {
        if self.estado.get() == SIN_PREGUNTAR {
            let hz = info(INFO_TSC_HZ);
            self.tsc_hz.set(hz);
            self.estado.set(if hz == 0 { APAGADO } else { ACTIVO });
        }
        if self.estado.get() != ACTIVO {
            return ciclos();
        }
        let mut c = self.cuentas.get();
        let ns_fila = (c.ps_px as u64 * ancho as u64).div_ceil(1000);
        let sel = INFO_GPU_ESPERA
            | (y0 as u64 & GPU_ESPERA_FILAS_MASK) << GPU_ESPERA_Y0_SHIFT
            | (y1 as u64 & GPU_ESPERA_FILAS_MASK) << GPU_ESPERA_Y1_SHIFT
            | ns_fila.min(GPU_ESPERA_NS_FILA_MASK) << GPU_ESPERA_NS_FILA_SHIFT;
        let mut r = info(sel);
        if r & GPU_ESPERA_VALIDA == 0 {
            // No hay rayo que mirar: se apaga, y el volcado es el de siempre.
            self.estado.set(APAGADO);
            return ciclos();
        }
        c.preguntas += 1;
        if r & GPU_ESPERA_NO_CABE != 0 {
            c.no_caben += 1;
        }
        // ** Y tras esperar, se VUELVE A PREGUNTAR (2026-09-23): lo que se
        // espera de mas de 1 ms se duerme por el latido, y un latido que llega
        // tarde deja al rayo en otro sitio. Copiar con la respuesta vieja seria
        // copiar a ciegas.
        //
        // ** Pero una espera PERDIDA no se paga con otro cuadro (2026-09-24).
        // El Ryzen dio `la peor 42945 us`: dos cuadros y medio para UNA caja.
        // La pantalla entera solo tiene 666 us de VBLANK para empezar, y si el
        // que copia despierta tarde -- el hilo del bus USB puede tener el CPU
        // hasta 3 ms por vuelta -- el rayo ya esta otra vez encima, y esperar
        // de nuevo era otro cuadro, y otro. Ahora: si lo que falta tras la
        // primera espera es menos de un milisegundo, se gira; si no, se copia
        // YA y se cuenta como RENDIDA. Un cuadro partido de vez en cuando se ve
        // y se cuenta; 43 ms de retraso en cada caja, no se ve y se sufre.
        let mut esperado = 0u64;
        let mut vueltas = 0;
        loop {
            let ns = r & GPU_ESPERA_NS_MASK;
            if ns == 0 || r & GPU_ESPERA_VALIDA == 0 {
                break;
            }
            if vueltas > 0 && ns > GIRAR_MAX_NS {
                c.rendidas += 1;
                break;
            }
            // ** Y NUNCA MAS DE UN CUADRO (2026-09-24): una respuesta que pide
            // esperar mas de lo que tarda el rayo en dar una vuelta es un
            // fallo de la cuenta, no una espera. El Ryzen durmio 4,29 s por
            // una asi. Se copia ya y se cuenta.
            if ns > ESPERA_MAX_NS {
                c.rendidas += 1;
                break;
            }
            // ** Y NUNCA MAS DE CUATRO VUELTAS (2026-09-24): toda espera tiene
            // tope, pase lo que pase en el hardware. Si el rayo se parara y
            // el kernel no lo viera, esto seguiria girando para siempre -- y
            // el escritorio se quedaria en su intro, que es lo que paso.
            if vueltas == VUELTAS_MAX {
                c.rendidas += 1;
                break;
            }
            self.esperar(ns);
            esperado += ns;
            vueltas += 1;
            r = info(sel);
        }
        if esperado > 0 {
            c.esperas += 1;
            c.esperado_ns += esperado;
            c.peor_ns = c.peor_ns.max(esperado);
        }
        self.cuentas.set(c);
        ciclos()
    }

    /// **Despues de copiar `filas` filas de `ancho` pixeles desde `t0`**: lo
    /// que cuesta un pixel, con memoria (tres cuartos lo de antes, un cuarto
    /// lo de ahora) para que una copia rara no mueva la cuenta de golpe.
    pub(crate) fn despues(&self, filas: u32, ancho: u32, t0: u64) {
        let hz = self.tsc_hz.get();
        let px = filas as u128 * ancho as u128;
        if self.estado.get() != ACTIVO || px == 0 || hz == 0 {
            return;
        }
        let ps = (ciclos().saturating_sub(t0) as u128 * 1_000_000_000_000 / hz as u128 / px) as u64;
        let ps = ps.min(u32::MAX as u64) as u32;
        let mut c = self.cuentas.get();
        c.ps_px = if c.ps_px == 0 { ps } else { (c.ps_px as u64 * 3 / 4 + ps as u64 / 4) as u32 };
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
