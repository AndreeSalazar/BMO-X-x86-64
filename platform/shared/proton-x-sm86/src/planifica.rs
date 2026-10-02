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
//!
//! # Los saltos (E6, 02-10)
//!
//! ```text
//!    un predicado (FSETP/ISETP)   quien lo lee como GUARDA (`@P0 BRA`) sale
//!                                 13 ciclos despues; como operando (SEL), 5
//!                                 -- `ptxas` pone 13 y 4 (`oro_saltos.ptx`)
//!    un BRA DRENA                 sale cuando todo lo escrito por una
//!                                 acoplada ya llego (6, lo mas largo de la
//!                                 tabla) y esperando TODAS las barreras
//!                                 abiertas; y deja 5 ciclos detras, como
//!                                 `ptxas`
//! ```
//!
//! Con eso, quien llega a un destino por un salto lo encuentra TODO hecho, y
//! la cuenta en linea recta (la de aqui y la del juez) vale para cualquier
//! camino: el que cae por debajo la hace; el que salta no tiene nada que
//! esperar. Cuesta ciclos en cada salto; lo correcto primero (E5).

use crate::Clase;

/// Lectura tras escritura, en ciclos: la de `juez.rs` (`latencia`), para las
/// clases que emite el emisor.
pub fn latencia(escritor: Clase, lector: Clase) -> u32 {
    use Clase::*;
    match (lector, escritor) {
        (Alu | Nada, Alu) => 4,
        (Alu | Nada, Fma | Ancha) => 5,
        (Fma, Alu) => 5,
        (Fma, Fma | Ancha) => 4,
        (Ancha, Alu) => 5,
        (Ancha, Fma) => 4,
        (Ancha, Ancha) => 6,
        (Mufu, _) => 4,
        // El TEX lee sus fuentes como las de memoria del juez (Agu): 5.
        (Tex, _) => 5,
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
    /// Cuantos registros escribe desde `escribe` (el TEX, cuatro).
    pub escribe_n: u8,
    /// E6: el predicado que escribe (FSETP/ISETP).
    pub escribe_p: Option<u8>,
    /// E6: el predicado que lee, y si como GUARDA (`@P0 BRA`) o operando (SEL).
    pub lee_p: Option<(u8, bool)>,
    /// E6: un BRA (drena todo antes de salir).
    pub salto: bool,
}

impl Meta {
    /// Una sin predicados ni salto.
    pub const fn de(clase: Clase, escribe: Option<u8>, lee: [Option<u8>; 3]) -> Self {
        Meta { clase, escribe, lee, lee_salidas: 0, escribe_n: 1, escribe_p: None, lee_p: None, salto: false }
    }
}

/// Lo que tarda un predicado en llegar a quien lo lee como guarda, y como
/// operando (las de `ptxas`, ver arriba).
pub const PREDICADO_GUARDA: u32 = 13;
pub const PREDICADO_OPERANDO: u32 = 5;
/// Lo mas largo de la tabla del juez: lo que espera un BRA a lo escrito.
pub const DRENAR: u32 = 6;
/// Lo que `ptxas` deja detras de un BRA.
pub const TRAS_SALTO: u32 = 5;

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
    // E6: el ciclo en que se escribio cada predicado.
    let mut predicado: [Option<u32>; 7] = [None; 7];
    let mut espera_mascara = alloc::vec![0u64; n];
    let mut barrera_de = alloc::vec![7u64; n];
    let mut t = 0u32;
    for (j, m) in metas.iter().enumerate() {
        let mut listo = if j == 0 { 0 } else { t + 1 };
        let mut mascara = 0u64;
        let leidos = m.lee.iter().flatten().copied().chain(0..m.lee_salidas);
        let escritos = m.escribe.into_iter().flat_map(|r| r..r.saturating_add(m.escribe_n.max(1)));
        for r in leidos.clone().chain(escritos.clone()) {
            if let Some(b) = pendiente[r as usize].take() {
                mascara |= 1 << b;
            }
        }
        // Un salto espera TODO lo que este en vuelo, y que todo haya llegado.
        if m.salto {
            for p in pendiente.iter_mut() {
                if let Some(b) = p.take() {
                    mascara |= 1 << b;
                }
            }
            for &(c, _) in escrito.iter().flatten() {
                listo = listo.max(c + DRENAR);
            }
        }
        if let Some((p, guarda)) = m.lee_p {
            if let Some(c) = predicado.get(p as usize).copied().flatten() {
                listo = listo.max(c + if guarda { PREDICADO_GUARDA } else { PREDICADO_OPERANDO });
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
        if let Some(p) = m.escribe_p {
            if let Some(x) = predicado.get_mut(p as usize) {
                *x = Some(listo);
            }
        }
        // Detras de un salto, `TRAS_SALTO` ciclos (el siguiente sale en t + 1).
        if m.salto {
            t = listo + TRAS_SALTO - 1;
        }
        espera_mascara[j] = mascara;
        if m.escribe.is_some() {
            if matches!(m.clase, Clase::Mufu | Clase::Tex) {
                let b = libres.iter().position(|&l| l).unwrap_or(0);
                libres[b] = false;
                barrera_de[j] = b as u64;
                for r in escritos {
                    pendiente[r as usize] = Some(b as u8);
                    escrito[r as usize] = None;
                }
            } else {
                for r in escritos {
                    escrito[r as usize] = Some((listo, m.clase));
                }
            }
        }
    }
    let controles = (0..n)
        .map(|j| {
            let mut espera = if j + 1 < n { (ciclo[j + 1] - ciclo[j]).clamp(1, 15) } else { 5 } as u64;
            // ** Una barrera tarda UN ciclo en encenderse: quien la enciende
            // espera 2, o el que la espera justo detras no espera nada (metal
            // 28-09: el MUFU.RSQ con espera 1 y su lector leyendo lo viejo,
            // al azar segun cuantos warps hubiera; `ptxas` pone 2).
            if barrera_de[j] != 7 {
                espera = espera.max(2);
            }
            espera | BIT4 | barrera_de[j] << 5 | 7 << 8 | espera_mascara[j] << 11
        })
        .collect();
    (controles, ciclo.last().copied().unwrap_or(0) + 1)
}
