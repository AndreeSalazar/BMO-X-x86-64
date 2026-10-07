//! # **LA ALARMA**: el reloj le quita el turno a quien no lo suelta
//!
//! capa: puro -- numeros que entran y numeros que salen, probado en el anfitrion
//!
//! [carril]  VERDE     decide si una alarma vale y si este tick la dispara
//! [cuesta]  DATO -- una alarma que salta dentro de su propia puerta pisaria
//!           el RIP que la puerta aun no copio: por eso NUNCA salta ahi
//!
//! La idea es la de Linux con un signal de reloj (`setitimer` + `SIGVTALRM`,
//! y la expropiacion asincrona de Go desde 2020, con `SIGURG`): el proceso
//! tiene hilos que el kernel no ve (los de la casa de PROTON-X, uno detras de
//! otro en una tarea), y uno que da vueltas sin llamar a nada se queda el
//! nucleo para siempre. El kernel no sabe de esos hilos; solo sabe AVISAR:
//!
//! ```text
//!    armar(puerta, buzon, ms)   cada `ms`, si el tick pilla a la tarea en
//!                               Ring 3 y FUERA de la puerta: el RIP que
//!                               llevaba, al buzon; y el RIP, a la puerta
//!    la puerta (Ring 3)         guarda TODO (registros, banderas, x87/SSE),
//!                               decide si cede el turno, lo devuelve todo
//!                               y salta al RIP del buzon
//! ```
//!
//! El kernel no escribe en la pila de nadie ni sabe que hay al otro lado:
//! cambia UN registro (el RIP del marco) y escribe UNA palabra (el buzon, por
//! su fisica). Lo que se decide esta aqui; el kernel (`task/alarma.rs`) solo
//! lo aplica.

#![cfg_attr(not(test), no_std)]

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoAlarma {
    /// La puerta o el buzon no son de Ring 3 (cero, o la mitad del kernel).
    FueraDeRing3 = 1,
    /// La puerta esta vacia o mide mas de [`PUERTA_MAXIMA`].
    PuertaMala = 2,
    /// El buzon no va a 8, o cae dentro de la puerta.
    BuzonMalo = 3,
    /// El periodo no esta entre [`MS_MINIMO`] y [`MS_MAXIMO`].
    PeriodoMalo = 4,
    /// El buzon no es una pagina de Ring 3 que la tarea pueda escribir.
    BuzonNoEscribible = 5,
}

/// El techo de Ring 3 (lo canonico de abajo).
pub const TECHO_RING3: u64 = 0x0000_8000_0000_0000;
/// Lo mas que mide una puerta: lo que salva y restaura cabe en una pagina.
pub const PUERTA_MAXIMA: u64 = 4096;
/// El periodo, en milisegundos: ni mas a menudo que el tick, ni tan raro
/// que no sirva.
pub const MS_MINIMO: u32 = 1;
pub const MS_MAXIMO: u32 = 1000;

/// **Una alarma armada**: la puerta `[inicio, fin)`, el buzon y cada cuantos
/// ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alarma {
    pub inicio: u64,
    pub fin: u64,
    pub buzon: u64,
    pub periodo: u64,
}

/// `arg2` de `TASK_OP_ALARMA`: lo que mide la puerta y el periodo.
pub const fn empaquetar(bytes_puerta: u32, ms: u32) -> u64 {
    bytes_puerta as u64 | (ms as u64) << 32
}

/// `(bytes de la puerta, ms)`.
pub const fn desempaquetar(p: u64) -> (u32, u32) {
    (p as u32, (p >> 32) as u32)
}

/// **Vale esta alarma?** `ms == 0` la APAGA (`Ok(None)`). `ticks_por_ms`:
/// los del reloj del kernel (1 a 1 kHz). La pagina del buzon la mira quien
/// llama (`NoAlarma::BuzonNoEscribible`).
pub fn validar(inicio: u64, buzon: u64, paquete: u64, ticks_por_ms: u64) -> Result<Option<Alarma>, NoAlarma> {
    let (bytes, ms) = desempaquetar(paquete);
    if ms == 0 {
        return Ok(None);
    }
    if !(MS_MINIMO..=MS_MAXIMO).contains(&ms) {
        return Err(NoAlarma::PeriodoMalo);
    }
    let fin = inicio.checked_add(bytes as u64).ok_or(NoAlarma::FueraDeRing3)?;
    if inicio == 0 || fin > TECHO_RING3 || buzon == 0 || buzon.checked_add(8).map_or(true, |b| b > TECHO_RING3) {
        return Err(NoAlarma::FueraDeRing3);
    }
    if bytes == 0 || bytes as u64 > PUERTA_MAXIMA {
        return Err(NoAlarma::PuertaMala);
    }
    if buzon % 8 != 0 || (buzon + 8 > inicio && buzon < fin) {
        return Err(NoAlarma::BuzonMalo);
    }
    Ok(Some(Alarma { inicio, fin, buzon, periodo: (ms as u64 * ticks_por_ms.max(1)).max(1) }))
}

/// **Salta en este tick?** Solo si el tick pillo a la tarea en Ring 3
/// (`cs` con RPL 3), ya toco (`ahora >= proxima`), y el RIP NO esta en la
/// puerta: dentro, la puerta aun no ha copiado el buzon (o ya lo restauro
/// todo y va a saltar), y una alarma ahi lo pisaria.
pub fn salta(a: &Alarma, cs: u64, rip: u64, ahora: u64, proxima: u64) -> bool {
    cs & 3 == 3 && ahora >= proxima && !(rip >= a.inicio && rip < a.fin)
}

/// La proxima vez, contada desde ahora (si el tick llega tarde, no se
/// acumulan alarmas atrasadas: una, y el periodo otra vez).
pub fn proxima(a: &Alarma, ahora: u64) -> u64 {
    ahora.saturating_add(a.periodo)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUERTA: u64 = 0x1000_2000;
    const BUZON: u64 = 0x1000_8000;

    fn buena() -> Alarma {
        validar(PUERTA, BUZON, empaquetar(256, 4), 1).unwrap().unwrap()
    }

    #[test]
    fn una_alarma_buena_y_la_que_apaga() {
        let a = buena();
        assert_eq!(a, Alarma { inicio: PUERTA, fin: PUERTA + 256, buzon: BUZON, periodo: 4 });
        assert_eq!(validar(PUERTA, BUZON, empaquetar(256, 0), 1), Ok(None));
        assert_eq!(desempaquetar(empaquetar(123, 456)), (123, 456));
        // A 2 ticks por ms, 4 ms son 8 ticks.
        assert_eq!(validar(PUERTA, BUZON, empaquetar(256, 4), 2).unwrap().unwrap().periodo, 8);
    }

    #[test]
    fn el_no_de_cada_cosa() {
        let k = 0xFFFF_8000_0000_0000;
        assert_eq!(validar(0, BUZON, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(k, BUZON, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(PUERTA, k, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(TECHO_RING3 - 16, BUZON, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(PUERTA, 0, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(u64::MAX - 8, BUZON, empaquetar(256, 4), 1), Err(NoAlarma::FueraDeRing3));
        assert_eq!(validar(PUERTA, BUZON, empaquetar(0, 4), 1), Err(NoAlarma::PuertaMala));
        assert_eq!(validar(PUERTA, BUZON, empaquetar(4097, 4), 1), Err(NoAlarma::PuertaMala));
        assert_eq!(validar(PUERTA, BUZON + 4, empaquetar(256, 4), 1), Err(NoAlarma::BuzonMalo));
        // El buzon DENTRO de la puerta, y pegado por abajo (sus 8 bytes la pisan).
        assert_eq!(validar(PUERTA, PUERTA + 8, empaquetar(256, 4), 1), Err(NoAlarma::BuzonMalo));
        assert_eq!(validar(PUERTA + 4, PUERTA, empaquetar(256, 4), 1), Err(NoAlarma::BuzonMalo));
        // Justo antes y justo despues, si.
        assert!(validar(PUERTA, PUERTA - 8, empaquetar(256, 4), 1).is_ok());
        assert!(validar(PUERTA, PUERTA + 256, empaquetar(256, 4), 1).is_ok());
        assert_eq!(validar(PUERTA, BUZON, empaquetar(256, 1001), 1), Err(NoAlarma::PeriodoMalo));
    }

    #[test]
    fn salta_solo_en_ring3_fuera_de_la_puerta_y_a_su_hora() {
        let a = buena();
        let fuera = 0x4000_0000;
        assert!(salta(&a, 0x23, fuera, 10, 10));
        assert!(salta(&a, 0x23, fuera, 11, 10));
        // Antes de su hora, no.
        assert!(!salta(&a, 0x23, fuera, 9, 10));
        // Un tick del kernel (la tarea en una puerta), no.
        assert!(!salta(&a, 0x08, fuera, 10, 10));
        // Dentro de la puerta, en el primer y en el ultimo byte, no; justo
        // despues, si.
        assert!(!salta(&a, 0x23, a.inicio, 10, 10));
        assert!(!salta(&a, 0x23, a.fin - 1, 10, 10));
        assert!(salta(&a, 0x23, a.fin, 10, 10));
        assert!(salta(&a, 0x23, a.inicio - 1, 10, 10));
    }

    #[test]
    fn un_tick_tarde_no_acumula() {
        let a = buena();
        assert_eq!(proxima(&a, 100), 104);
        assert_eq!(proxima(&a, u64::MAX - 1), u64::MAX);
    }
}
