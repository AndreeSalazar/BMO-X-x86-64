//! **El registro de lo que se dibuja** (28-09, lo pidio el propietario: *"LOG
//! en FPS y todos los datos"*; ver PLAN_VERRANO 2c, EL REGISTRO).
//!
//! Lo puro: se le dan los tiempos de cada fotograma y, cada segundo, da UNA
//! linea con lo que paso en ese segundo:
//!
//! ```text
//!    [registro] 7 fps  fotograma 131/142/170 ms  dibujar 130 ms  presentar 4 ms  (fotogramas 30..36)
//!               |      |min/medio/max            |medio          |medio           |cuales
//! ```
//!
//! Solo enteros (en BMO-X la casa es soft-float en Ring 3, y aqui no hace
//! falta un decimal). La casa pone el reloj (`Plataforma::ahora_ns`) y
//! escribe la linea donde escribe todo (la consola de BMO-X, la salida del
//! banco); VERRANO usara la misma forma.

use alloc::string::String;

const SEGUNDO: u64 = 1_000_000_000;

/// Un segundo de fotogramas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registro {
    /// Cuando empezo el segundo que se cuenta, y el ultimo Present.
    desde: u64,
    ultimo: Option<u64>,
    /// El numero del primer fotograma del segundo, y cuantos van en total.
    primero: u64,
    total: u64,
    /// Lo de este segundo: fotogramas, y las sumas y extremos en ns.
    n: u64,
    min: u64,
    max: u64,
    suma: u64,
    dibujar: u64,
    presentar: u64,
    /// Lo dibujado desde el ultimo Present (se suma a su fotograma).
    dibujando: u64,
}

fn ms(ns: u64) -> u64 {
    (ns + 500_000) / 1_000_000
}

impl Registro {
    /// Tiempo de dibujar (ExecuteCommandLists) desde el ultimo Present.
    pub fn dibujo(&mut self, ns: u64) {
        self.dibujando += ns;
    }

    /// **Un Present** que termino en `ahora` y costo `presentar` ns. Si se
    /// cumplio un segundo, devuelve su linea.
    pub fn presente(&mut self, ahora: u64, presentar: u64) -> Option<String> {
        let Some(antes) = self.ultimo.replace(ahora) else {
            // El primero abre la cuenta: no hay fotograma anterior que medir.
            self.desde = ahora;
            self.primero = self.total;
            self.total += 1;
            self.dibujando = 0;
            return None;
        };
        let f = ahora.saturating_sub(antes);
        if self.n == 0 || f < self.min {
            self.min = f;
        }
        self.max = self.max.max(f);
        self.suma += f;
        self.dibujar += core::mem::take(&mut self.dibujando);
        self.presentar += presentar;
        self.n += 1;
        self.total += 1;
        if ahora.saturating_sub(self.desde) < SEGUNDO {
            return None;
        }
        let n = self.n;
        let linea = alloc::format!(
            "[registro] {} fps  fotograma {}/{}/{} ms  dibujar {} ms  presentar {} ms  (fotogramas {}..{})\n",
            n * SEGUNDO / ahora.saturating_sub(self.desde).max(1),
            ms(self.min),
            ms(self.suma / n),
            ms(self.max),
            ms(self.dibujar / n),
            ms(self.presentar / n),
            self.primero,
            self.total - 1,
        );
        *self = Registro { desde: ahora, ultimo: Some(ahora), primero: self.total, total: self.total, ..Registro::default() };
        Some(linea)
    }
}
