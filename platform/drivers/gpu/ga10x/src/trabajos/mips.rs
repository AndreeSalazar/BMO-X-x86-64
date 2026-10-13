//! **G5: LAS MIPS EN STREAMING** (13-10, la puerta `[3]` del censo de
//! Cyberpunk) -- la 3060 lee CADA mip de una textura directo de la RAM del
//! juego, como ya leia la mip 0: un TIC por mip, sin copiar nada a la VRAM.
//!
//! capa: puro -- donde esta cada mip y su TIC; la IOMMU y la VRAM las toca el
//! kernel (L8)
//!
//! [eje]     CORRECCION -- `clb097tex.h` de open-gpu-doc: el TIC PITCH
//!           (`TEXHEAD_PITCH`) tiene UNA direccion y UN paso de fila; no hay
//!           sitio para decir donde empieza la mip 1. Las mips en un solo TIC
//!           piden la textura en BLOQUES (`TEXHEAD_BL`), y eso es copiarla:
//!           el propietario quiere streaming. Asi que cada mip es una textura
//!           PITCH de un nivel, y el SOMBREADOR elige la mip con las cuentas
//!           de la CPU (`proton-x-sm86::lecturas`).
//!
//! # Los 16 huecos de cada textura
//!
//! La textura `k` de una receta tiene los TIC `16 k .. 16 k + 15`:
//!
//! ```text
//!    hueco 0        la mip que mira la VISTA (lo de siempre: `Sample`, y el
//!                   asa de `gpu verrano textura`, `asa(0, 0)`, no cambia)
//!    huecos 1..14   la mip 0..13 del recurso (`Carga::Base` = el hueco 1)
//!    hueco 15       la NULA: un texel a ceros en la pagina de las piscinas
//! ```
//!
//! Un `SampleLevel` o un `Load` calcula `k` (la mip, ya con la de la vista
//! sumada), lo sujeta con `IMNMX.U32 k, 14` y suma la base: un `k` negativo o
//! de mas cae en el 15 (ceros, lo que da la CPU fuera del recurso). El juez
//! R7 lo exige asi (`sass::juez`, el asa calculada): ninguna cuenta del
//! sombreador sale de los 16 huecos de su textura.
//!
//! # Donde vive cada mip
//!
//! La casa guarda las mips SEGUIDAS y sin relleno (filas de `4 x ancho`). La
//! 3060 lee un PITCH con filas de 32 B desde 32 B: las mips grandes cumplen
//! y se leen TAL CUAL de la RAM del juego (prestada SOLO LECTURA en su
//! ranura); desde la primera que no cumple (una fila de 60 texeles, o la de
//! 4x4) hasta la ultima, la COLA, la puerta las pone con filas de 32 B en los
//! DATOS de la receta, que ya viajan a la VRAM en cada lote. La cola de una
//! textura de 1024x1024 son 224 bytes; la de una de 1920x1088, 11,6 KiB.

use crate::texturas::{tic, Imagen, PISCINAS_VA};

/// Huecos de TIC de cada textura.
pub const HUECOS: u32 = 16;
/// El hueco de la mip de la vista.
pub const HUECO_VISTA: u32 = 0;
/// El de la mip 0 del recurso (`Carga::Base`).
pub const HUECO_BASE: u32 = 1;
/// El de los ceros.
pub const HUECO_NULO: u32 = HUECOS - 1;
/// Lo mas que suma el sombreador a la base (`IMNMX.U32 k, TOPE`): del
/// hueco 1 al 15.
pub const TOPE: u32 = HUECO_NULO - HUECO_BASE;
/// Las mips que caben (las de una de 8192x8192).
pub const MAX_MIPS: u32 = TOPE;
/// El texel nulo: 32 bytes a cero al final de la pagina de las piscinas (la
/// pagina se pone a cero al mapearla y nadie escribe alli: los TIC acaban en
/// `TSC_DESDE`, los TSC en `TSC_DESDE + 4 x 32`).
pub const NULA: u64 = 0xFE0;
/// Lo mas que mide la cola en los DATOS.
pub const MAX_COLA: u32 = 32 << 10;

/// `(ancho, alto)` de la mip `j` de una `ancho x alto`.
pub const fn medidas(ancho: u32, alto: u32, j: u32) -> (u32, u32) {
    let w = if j >= 32 { 0 } else { ancho >> j };
    let h = if j >= 32 { 0 } else { alto >> j };
    (if w == 0 { 1 } else { w }, if h == 0 { 1 } else { h })
}

/// Las mips de una cadena entera (hasta 1x1).
pub const fn todas(ancho: u32, alto: u32) -> u32 {
    let m = if ancho > alto { ancho } else { alto };
    32 - m.leading_zeros()
}

/// La fila de la mip `j` en la RAM de la app: la de la mip 0 la dice la app
/// (`fila`: la casa, `4 x ancho`; la prueba de `gpu verrano textura`, 32 con
/// relleno); las demas, sin relleno, como las guarda la casa.
pub const fn fila_de(ancho: u32, alto: u32, fila: u32, j: u32) -> u32 {
    if j == 0 {
        fila
    } else {
        4 * medidas(ancho, alto, j).0
    }
}

/// Bytes de la mip `j` en la RAM de la app.
pub const fn bytes_de(ancho: u32, alto: u32, fila: u32, j: u32) -> u64 {
    fila_de(ancho, alto, fila, j) as u64 * medidas(ancho, alto, j).1 as u64
}

/// Donde empieza la mip `j` (bytes desde la mip 0) en la RAM de la app.
pub const fn desde(ancho: u32, alto: u32, fila: u32, j: u32) -> u64 {
    let mut s = 0;
    let mut k = 0;
    while k < j {
        s += bytes_de(ancho, alto, fila, k);
        k += 1;
    }
    s
}

/// La fila de la mip `j` en la COLA: alineada a 32.
pub const fn fila_en_cola(ancho: u32, alto: u32, j: u32) -> u32 {
    (4 * medidas(ancho, alto, j).0).next_multiple_of(32)
}

/// **Desde que mip va todo en la cola**: la primera cuya fila o cuyo
/// principio (en la RAM de la app, `va` la de la mip 0) no cae en 32 B.
pub const fn cola_desde(va: u64, ancho: u32, alto: u32, fila: u32, mips: u32) -> u32 {
    let mut j = 0;
    while j < mips {
        if fila_de(ancho, alto, fila, j) % 32 != 0 || (va + desde(ancho, alto, fila, j)) % 32 != 0 {
            return j;
        }
        j += 1;
    }
    mips
}

/// Donde empieza la mip `j` (de la cola) dentro de la cola.
pub const fn en_cola(ancho: u32, alto: u32, cola: u32, j: u32) -> u64 {
    let mut s = 0;
    let mut k = cola;
    while k < j {
        s += fila_en_cola(ancho, alto, k) as u64 * medidas(ancho, alto, k).1 as u64;
        k += 1;
    }
    s
}

/// Lo que mide la cola de `cola` a `mips`.
pub const fn bytes_cola(ancho: u32, alto: u32, cola: u32, mips: u32) -> u64 {
    en_cola(ancho, alto, cola, mips)
}

/// **El TIC de cada hueco** de la textura de la ranura `en_ranura` (la VA
/// de la 3060 donde empieza su pagina prestada, mas el desplazamiento de su
/// mip 0 en ella), con la cola en `en_datos` (la VA de la 3060 en los DATOS).
#[allow(clippy::too_many_arguments)]
pub fn tics(en_ranura: u64, en_datos: u64, ancho: u32, alto: u32, fila: u32, mips: u32, vista: u32, cola: u32, bgra: bool) -> Option<[[u32; 8]; HUECOS as usize]> {
    let nula = tic(&Imagen { va: PISCINAS_VA + NULA, ancho: 1, alto: 1, fila: 32, bgra: false })?;
    let mut t = [nula; HUECOS as usize];
    let de = |j: u32| -> Option<[u32; 8]> {
        let (w, h) = medidas(ancho, alto, j);
        let i = if j < cola {
            Imagen { va: en_ranura + desde(ancho, alto, fila, j), ancho: w, alto: h, fila: fila_de(ancho, alto, fila, j), bgra }
        } else {
            Imagen { va: en_datos + en_cola(ancho, alto, cola, j), ancho: w, alto: h, fila: fila_en_cola(ancho, alto, j), bgra }
        };
        tic(&i)
    };
    t[HUECO_VISTA as usize] = de(vista)?;
    for j in 0..mips {
        t[(HUECO_BASE + j) as usize] = de(j)?;
    }
    Some(t)
}

const _: () = assert!(HUECO_BASE + MAX_MIPS == HUECO_NULO && TOPE == 14);
const _: () = assert!(NULA >= crate::texturas::TSC_DESDE + (crate::texturas::MAX_TEXTURAS * crate::texturas::BYTES) as u64 && NULA + 32 <= 0x1000);
// Los TIC de las cuatro texturas caben antes de los TSC.
const _: () = assert!(crate::texturas::MAX_TEXTURAS as u64 * HUECOS as u64 * crate::texturas::BYTES as u64 <= crate::texturas::TSC_DESDE);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cadena_como_la_guarda_la_casa() {
        assert_eq!(todas(1024, 1024), 11);
        assert_eq!(todas(1920, 1088), 11);
        assert_eq!(todas(1, 1), 1);
        assert_eq!(medidas(1920, 1088, 6), (30, 17));
        assert_eq!(medidas(4, 4, 5), (1, 1));
        assert_eq!(desde(4, 4, 16, 1), 64);
        assert_eq!(desde(4, 4, 16, 2), 64 + 16);
        // La mip 0 con su relleno (la de `gpu verrano textura`).
        assert_eq!(desde(4, 4, 32, 1), 128);
    }

    #[test]
    fn la_cola_empieza_donde_la_fila_deja_de_caer_en_32() {
        // 1024x1024 desde una pagina: hasta la de 8x8 en la RAM del juego; la
        // de 4x4, 2x2 y 1x1 en la cola, 224 bytes (filas de 32).
        assert_eq!(cola_desde(0x1000_0000, 1024, 1024, 4096, 11), 8);
        assert_eq!(bytes_cola(1024, 1024, 8, 11), 32 * 4 + 32 * 2 + 32);
        // 1920x1088: la de 60 texeles (240 B por fila) ya no cae.
        assert_eq!(cola_desde(0x1000_0000, 1920, 1088, 7680, 11), 5);
        assert_eq!(bytes_cola(1920, 1088, 5, 11), 256 * 34 + 128 * 17 + 64 * 8 + 32 * 4 + 32 * 2 + 32);
        // Una mip 0 que no empieza en 32: todo a la cola.
        assert_eq!(cola_desde(0x1000_0010, 64, 64, 256, 7), 0);
        // Sin mips de mas: la textura entera en la RAM del juego.
        assert_eq!(cola_desde(0x1000_0000, 256, 256, 1024, 1), 1);
        // La 4x4 de `gpu verrano textura`: con su relleno, en la RAM.
        assert_eq!(cola_desde(0x4000_0000, 4, 4, 32, 1), 1);
        assert_eq!(cola_desde(0x4000_0000, 4, 4, 16, 1), 0);
    }

    #[test]
    fn los_16_huecos() {
        let t = tics(0xA_0000_0000, 0x2_0001_0400, 64, 32, 256, 7, 2, cola_desde(0, 64, 32, 256, 7), true).unwrap();
        let va = |x: &[u32; 8]| x[1] as u64 | ((x[2] & 0xFFFF) as u64) << 32;
        let medida = |x: &[u32; 8]| ((x[4] & 0xFFFF) + 1, (x[5] & 0xFFFF) + 1, x[3] << 5);
        // El hueco 0 es la mip de la vista (la 2: 16x8, en la RAM).
        assert_eq!(t[0], t[(HUECO_BASE + 2) as usize]);
        assert_eq!((va(&t[0]), medida(&t[0])), (0xA_0000_0000 + 64 * 32 * 4 + 32 * 16 * 4, (16, 8, 64)));
        // La 3 (8x4) aun en la RAM; la 4 (4x2) ya en la cola, con filas de 32.
        assert_eq!(cola_desde(0, 64, 32, 256, 7), 4);
        assert_eq!((va(&t[5]), medida(&t[5])), (0x2_0001_0400, (4, 2, 32)));
        assert_eq!((va(&t[6]), medida(&t[6])), (0x2_0001_0400 + 64, (2, 1, 32)));
        assert_eq!((va(&t[7]), medida(&t[7])), (0x2_0001_0400 + 96, (1, 1, 32)));
        // Del 8 al 15, la nula: 1x1 en la pagina de las piscinas.
        for h in 8..16 {
            assert_eq!((va(&t[h]), medida(&t[h])), (PISCINAS_VA + NULA, (1, 1, 32)), "hueco {h}");
        }
        // BGRA cruza R y B en cada mip, no en la nula.
        assert_eq!(t[1][0] >> 19 & 7, crate::texturas::DE_B);
        assert_eq!(t[15][0] >> 19 & 7, crate::texturas::DE_R);
    }
}
