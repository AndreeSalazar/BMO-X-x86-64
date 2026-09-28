//! **Los bits de control por REGLA** (E4 de `PLAN_LA_LENGUA_DE_LA_3060`).
//!
//! Hasta E3 cada instruccion esperaba 6 ciclos "por si acaso". Aqui se
//! calcula lo justo, con la MISMA tabla con la que el juez (J1, `juez.rs`
//! del driver, de NAK `sm80_instr_latencies.rs`) lo va a juzgar:
//!
//! ```text
//!    acoplada (FADD, FMUL, FMNMX, MOV)   quien lee su resultado sale, como
//!                                        pronto, `latencia(escritor, lector)`
//!                                        ciclos despues: la espera de cada
//!                                        instruccion es lo que falta para eso
//!    desacoplada (MUFU)                  ENCIENDE una de las 6 barreras al
//!                                        escribir; el primero que lee (o
//!                                        pisa) su resultado la ESPERA
//! ```
//!
//! Si esta tabla fuera mas corta que la del juez, el juez lo diria (R2): las
//! pruebas del driver juzgan todo lo que sale de aqui.

use crate::Clase;

/// Lectura tras escritura, en ciclos: la de `juez.rs` (`latencia`), para las
/// clases que emite el emisor.
pub fn latencia(escritor: Clase, lector: Clase) -> u32 {
    use Clase::*;
    match (lector, escritor) {
        (Alu | Nada, Alu) => 4,
        (Alu | Nada, Fma) => 5,
        (Fma, Alu) => 5,
        (Fma, Fma) => 4,
        (Mufu, _) => 4,
        _ => 1,
    }
}

/// Lo que el planificador necesita de cada instruccion.
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub clase: Clase,
    pub escribe: Option<u8>,
    pub lee: [Option<u8>; 3],
    /// Y ademas R0..R(n-1): el EXIT, que entrega las salidas (en un
    /// programa de pixel, el color lo lee la 3060 AL SALIR; metal 28-09).
    pub lee_salidas: u8,
}

/// El bit 4, que `ptxas` pone en todas (ver `bmo_sm86::codifica::ALU`).
const BIT4: u64 = 1 << 4;
/// **Planificar**: el control de cada instruccion (espera, barreras), y los
/// ciclos que tarda el programa en salir entero.
pub fn planificar(metas: &[Meta]) -> (alloc::vec::Vec<u64>, u32) {
    let n = metas.len();
    // El ciclo en que sale cada una, y quien escribio cada registro.
    let mut ciclo = alloc::vec![0u32; n];
    let mut escrito: [Option<(u32, Clase)>; 256] = [None; 256];
    // Barrera pendiente por registro (lo escribe un MUFU aun en vuelo).
    let mut pendiente: [Option<u8>; 256] = [None; 256];
    let mut libres = [true; 6];
    let mut espera_mascara = alloc::vec![0u64; n];
    let mut barrera_de = alloc::vec![7u64; n];
    let mut t = 0u32;
    for (j, m) in metas.iter().enumerate() {
        let mut listo = if j == 0 { 0 } else { t + 1 };
        let mut mascara = 0u64;
        let leidos = m.lee.iter().flatten().copied().chain(0..m.lee_salidas);
        for r in leidos.clone().chain(m.escribe) {
            if let Some(b) = pendiente[r as usize].take() {
                mascara |= 1 << b;
            }
        }
        // Lo esperado se libera (y ya nadie mas lo tiene pendiente).
        for b in 0..6u8 {
            if mascara & 1 << b != 0 {
                libres[b as usize] = true;
                for p in pendiente.iter_mut() {
                    if *p == Some(b) {
                        *p = None;
                    }
                }
            }
        }
        for r in leidos {
            if let Some((c, clase)) = escrito[r as usize] {
                listo = listo.max(c + latencia(clase, m.clase));
            }
        }
        ciclo[j] = listo;
        t = listo;
        espera_mascara[j] = mascara;
        if let Some(r) = m.escribe {
            if m.clase == Clase::Mufu {
                let b = libres.iter().position(|&l| l).unwrap_or(0);
                libres[b] = false;
                pendiente[r as usize] = Some(b as u8);
                barrera_de[j] = b as u64;
                escrito[r as usize] = None;
            } else {
                escrito[r as usize] = Some((listo, m.clase));
            }
        }
    }
    let controles = (0..n)
        .map(|j| {
            let espera = if j + 1 < n { (ciclo[j + 1] - ciclo[j]).clamp(1, 15) } else { 5 } as u64;
            espera | BIT4 | barrera_de[j] << 5 | 7 << 8 | espera_mascara[j] << 11
        })
        .collect();
    (controles, ciclo.last().copied().unwrap_or(0) + 1)
}
