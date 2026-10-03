//! **El cerrojo del monton**: un giro sobre un atomico. Lo puro no enlaza
//! `bmo-abi` (y su `BmoSpinLock`), y un monton no necesita mas: nadie lo
//! retiene mientras espera nada.

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Cerrojo(AtomicBool);

impl Cerrojo {
    pub(crate) const fn new() -> Self {
        Cerrojo(AtomicBool::new(false))
    }

    pub(crate) fn lock(&self) {
        while self.0.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            spin_loop();
        }
    }

    pub(crate) fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}
