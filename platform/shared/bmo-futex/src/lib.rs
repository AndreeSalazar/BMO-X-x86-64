//! # **EL FUTEX**: esperar en una palabra de Ring 3, y despertar a N
//!
//! capa: puro -- numeros que entran y numeros que salen, probado en el anfitrion
//!
//! [carril]  VERDE     decide la llave, el plazo y si una peticion vale
//! [cuesta]  DATO -- dos palabras con la misma llave solo se despiertan de
//!           mas (quien despierta vuelve a mirar su palabra); una llave que
//!           mezclara dos PROCESOS seria un despertar que cruza la frontera
//!
//! La idea es la de Linux (`futex(2)`, 2002), y es la que hace que Proton
//! corra Cyberpunk: un cerrojo LIBRE se toma en Ring 3 con una atomica, sin
//! pedirle nada a nadie; solo cuando hay que ESPERAR se llama al kernel:
//!
//! ```text
//!    esperar(dir, visto, plazo)   si la palabra en `dir` ya no vale `visto`,
//!                                 vuelve en el acto (alguien la cambio:
//!                                 no hay que dormir); si vale, el hilo
//!                                 duerme en la llave de (proceso, dir)
//!    despertar(dir, n)            despierta a n de los que duermen ahi
//! ```
//!
//! La comparacion y el dormir van juntos bajo el cerrojo del planificador
//! (`wait_current_checked`): un `despertar` entre el "vale" y el "duermo" no
//! se puede perder. Aqui esta todo lo que se DECIDE; el kernel
//! (`syscall/op_futex.rs`) solo lee la palabra y duerme.

#![cfg_attr(not(test), no_std)]

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoFutex {
    /// La direccion no va a 4 (una palabra de 32 bits).
    Desalineada = 1,
    /// No es de Ring 3 (cero, o de la mitad del kernel), o no esta mapeada.
    NoEsRing3 = 2,
    /// La palabra ya no valia lo visto: no se durmio (EAGAIN de Linux).
    Cambio = 3,
}

/// El techo de Ring 3 (lo canonico de abajo).
pub const TECHO_RING3: u64 = 0x0000_8000_0000_0000;

/// La marca de las llaves de futex, en el byte de arriba: ninguna otra
/// espera del kernel lo usa (0x7E el enterrador, 0xD1 el disco, 0x4D la
/// memoria, 0x0B el VBLANK, y las chicas de la red y el latido).
pub const MARCA: u64 = 0xFE00_0000_0000_0000;

/// Sin plazo: `ms` de [`empaquetar`] que dice "para siempre".
pub const SIEMPRE: u32 = u32::MAX;

/// **La llave de `dir` en el proceso `pid`.** La direccion entra entera
/// (47 bits, alineada a 4: los dos de abajo sobran) y el proceso en los 8
/// bits que quedan. Dos procesos cuyo pid coincide en esos 8 bits y que
/// esperan en la MISMA direccion se despertarian de mas el uno al otro:
/// cada uno vuelve a mirar su palabra, nada se rompe.
pub fn llave(pid: u32, dir: u64) -> Result<u64, NoFutex> {
    if dir == 0 || dir >= TECHO_RING3 {
        return Err(NoFutex::NoEsRing3);
    }
    if dir % 4 != 0 {
        return Err(NoFutex::Desalineada);
    }
    Ok(MARCA | ((pid as u64 & 0xFF) << 47) | (dir >> 2))
}

/// La puerta lleva dos numeros: el segundo de `esperar` es lo visto (32
/// bits) y el plazo en milisegundos ([`SIEMPRE`] = sin plazo).
pub fn empaquetar(visto: u32, ms: u32) -> u64 {
    visto as u64 | (ms as u64) << 32
}

/// Lo de [`empaquetar`], al reves: `(visto, ms)`.
pub fn desempaquetar(v: u64) -> (u32, u32) {
    (v as u32, (v >> 32) as u32)
}

/// **El plazo en ciclos** para `ms` milisegundos desde `ahora` con un reloj
/// de `hz`: 0 es "sin plazo" (lo que entiende el planificador). Un plazo de
/// 0 ms es "ya": un ciclo despues de ahora, para que no se lea como
/// "nunca". Saturado: un plazo enorme no da la vuelta.
pub fn plazo(ahora: u64, ms: u32, hz: u64) -> u64 {
    if ms == SIEMPRE {
        return 0;
    }
    let ciclos = (ms as u128 * hz as u128 / 1000).min(u64::MAX as u128) as u64;
    ahora.saturating_add(ciclos.max(1))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La llave: de Ring 3, alineada, distinta por direccion y por
    /// proceso, y con su marca (no choca con las demas esperas del kernel).
    #[test]
    fn la_llave_separa_direcciones_y_procesos() {
        let a = llave(7, 0x20_0000_1000).unwrap();
        assert_eq!(a >> 56, 0xFE, "su marca");
        assert_ne!(a, llave(7, 0x20_0000_1004).unwrap(), "otra palabra");
        assert_ne!(a, llave(8, 0x20_0000_1000).unwrap(), "otro proceso");
        assert_eq!(a, llave(7, 0x20_0000_1000).unwrap(), "la misma, la misma llave");
        assert_eq!(llave(7, 0x7FFF_FFFF_FFFC).map(|k| k >> 56), Ok(0xFE), "la ultima palabra de Ring 3");
        for k in [0x7E4E_A000_0000_0001u64, 0xD15C_0000_0000_0001, 0x4D45_4D00_0000_0000, 0x0B1A_0000_0000_0001, 0x52_4544, 0x1A71D0] {
            assert_ne!(k >> 56, 0xFE, "las otras llaves del kernel no llevan la marca");
        }
    }

    #[test]
    fn lo_que_no_es_una_palabra_de_ring3_dice_por_que() {
        assert_eq!(llave(1, 0), Err(NoFutex::NoEsRing3));
        assert_eq!(llave(1, 0xFFFF_8000_0000_0000), Err(NoFutex::NoEsRing3), "del kernel");
        assert_eq!(llave(1, TECHO_RING3), Err(NoFutex::NoEsRing3));
        assert_eq!(llave(1, 0x1002), Err(NoFutex::Desalineada));
    }

    #[test]
    fn el_plazo_y_el_paquete() {
        assert_eq!(desempaquetar(empaquetar(0xDEAD_BEEF, 250)), (0xDEAD_BEEF, 250));
        assert_eq!(plazo(1000, SIEMPRE, 3_700_000_000), 0, "sin plazo");
        assert_eq!(plazo(1000, 0, 3_700_000_000), 1001, "0 ms es ya, no nunca");
        assert_eq!(plazo(1000, 10, 3_700_000_000), 1000 + 37_000_000);
        assert_eq!(plazo(u64::MAX - 5, 1000, 3_700_000_000), u64::MAX, "saturado");
    }
}
