//! **Los procesadores que PROTON-X dice que hay** (02-10), en UN sitio: lo
//! leen `GetSystemInfo`, las afinidades, `GetLogicalProcessorInformation(Ex)`,
//! los CPU sets y el PEB, y todos tienen que decir lo mismo.
//!
//! Hasta el 02-10 la casa decia UN procesador, a proposito: sus hilos se
//! turnan en uno (M:1) y asi nadie esperaba paralelismo. Cyberpunk no lo
//! aguanta: monta una cola de trabajo por nucleo sin contar el principal, con
//! uno le salen CERO, y luego vacia "la ultima" con `cuantas - 1` = -1
//! (`Cyberpunk2077.exe+0x24d9a9`, y el fallo en `+0x24d8b3`). Ningun PC con
//! Windows que mueva un juego asi tiene un nucleo.
//!
//! Se dice la forma del Ryzen 5 5600X del propietario: 6 nucleos con SMT, 12
//! logicos, un paquete, un nodo NUMA, un grupo. Los hilos siguen turnandose
//! en uno: esto es lo que se CUENTA, no lo que corre a la vez. Un hilo del
//! `.exe` que espere a otro dando vueltas sin llamar a nadie se quedaria
//! colgado con uno o con doce: eso es de las esperas, no de esta cifra.

/// Nucleos fisicos.
pub const NUCLEOS: u32 = 6;
/// Logicos por nucleo (SMT).
pub const POR_NUCLEO: u32 = 2;
/// Procesadores logicos: lo que `dwNumberOfProcessors` dice.
pub const LOGICOS: u32 = NUCLEOS * POR_NUCLEO;
/// La mascara de afinidad de todo el sistema (grupo 0).
pub const MASCARA: u64 = (1u64 << LOGICOS) - 1;

/// La mascara de los logicos del nucleo `n`.
pub const fn mascara_del_nucleo(n: u32) -> u64 {
    ((1u64 << POR_NUCLEO) - 1) << (n * POR_NUCLEO)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_nucleos_cubren_la_mascara_sin_pisarse() {
        let mut todo = 0u64;
        for n in 0..NUCLEOS {
            assert_eq!(todo & mascara_del_nucleo(n), 0);
            todo |= mascara_del_nucleo(n);
        }
        assert_eq!(todo, MASCARA);
        assert_eq!(MASCARA.count_ones(), LOGICOS);
    }
}
