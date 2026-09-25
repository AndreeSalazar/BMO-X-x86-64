//! **BMO PASE GPU** -- lo mismo que el GATE RED, para la 3060: la burocracia se
//! paga UNA vez, despues no hay puerta, y un radar mira desde fuera.
//!
//! generacion: nieto -- la forma del buzon y los veredictos; no sabe que es una GPU
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el kernel y Ring 3
//!
//! # El propietario, 25-09
//!
//! *"pero la GPU puede tener lo mismo una sola vez la burocracia y le viene el
//! radar eso para exprimir con todo?"* -- si, y es la misma ruta de
//! `docs/identidad/LA_RUTA.md` que ya vive en `bmo-puerta-red`:
//!
//! ```text
//!    1. DECLARAR   INVOKE(GPU_PASE_ABRIR, lienzo)                 [`pase`]
//!    2. JUZGAR     el kernel pregunta en orden, UNA vez: pantalla, IOMMU,
//!                  BAR1, timbre, lienzo. Presta el lienzo PARA QUEDARSE
//!    3. CORRER     el escritorio deja sus cajas sucias en el buzon y sube
//!                  un numero. CERO syscalls por fotograma          [`buzon`]
//!    4. VIGILAR    en cada VBLANK, desde fuera: el buzon, la valla de la
//!                  3060 y unas muestras de vez en cuando           [`radar`]
//!                  -> si no cuadra, se REVOCA y vuelve el camino de siempre
//! ```
//!
//! # *** Por que el latido es el VBLANK y no 4 ms
//!
//! La red late cada 4 ms porque las tramas llegan cuando quieren. La pantalla
//! no: el monitor la barre 60 veces por segundo y lo que se copie entre dos
//! barridos no se ve antes. El aviso ya existe (E2, `dev/vblank.rs`, el MSI 50),
//! asi que el latido cae EXACTAMENTE cuando sirve: una tanda por barrido, sin
//! girar, sin esperar, sin tearing.
//!
//! # *** Lo que se gana, dicho sin inflar
//!
//! Un syscall son ~1000 ciclos (`syscall/mod.rs`): con 20 cajas por fotograma
//! son ~5 us. **Eso NO es lo que se gana.** Lo que se gana es lo que hoy gira:
//!
//! ```text
//!    la valla (ESPERAR)       la CPU gira hasta que la 3060 paga   -> 0: la mira el latido
//!    el ritmo                 fotogramas que el monitor nunca ve   -> uno por barrido
//!    la comprobacion          muestras en el camino del fotograma  -> el radar, 1 de cada 16
//! ```
//!
//! # *** Lo que Ring 3 NUNCA toca: las ordenes de la 3060
//!
//! El escritorio escribe CAJAS; el kernel las lee UNA vez, las juzga y construye
//! el empuje en memoria suya. Lo que la 3060 lee del proceso son PIXELES (el
//! lienzo, prestado de solo lectura): cambiarlos a destiempo estropea la
//! pantalla del propio proceso y nada mas -- y el radar lo nota.
//!
//! # *** NEUTRO: la GPU de hoy es la 3060, el crate no lo sabe
//!
//! Ni un nombre de fabricante en el codigo: lo de cada tarjeta lo pone su
//! `Motor` en el kernel (`dev/pase_gpu.rs` define el rasgo; el de la 3060
//! vive en `dev/gpu_trabajo/pase_nv.rs`). El guardian `la-3060` (regla N) lo
//! vigila del lado del kernel.
//!
//! # *** Revocar no apaga la pantalla
//!
//! Un pase revocado devuelve el lienzo y deja el motivo en el buzon. El
//! escritorio lo lee y vuelve al camino con burocracia (`CAJA` y `ESPERAR`, que
//! sigue ahi) o al volcado por la CPU. Revocar es perder el atajo, no la vista.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod buzon;
pub mod orden;
pub mod pase;
pub mod radar;
