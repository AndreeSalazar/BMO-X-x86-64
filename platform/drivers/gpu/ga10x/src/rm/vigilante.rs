//! **E7: EL VIGILANTE -- un trabajo de la 3060 que no vuelve** (el TDR de
//! BMO-X). Desde E6 el emisor pone bucles (`BRA` hacia atras) y el juez no
//! puede saber si un bucle acaba: un sombreador que no sale deja al motor
//! grafico girando PARA SIEMPRE. En Windows lo para el TDR (2 s y reinicia
//! la GPU entera); aqui, esto.
//!
//! capa: puro -- el plazo, la decision y el mensaje del corte; los registros
//! y la cola los toca el kernel (`gpu_trabajo.rs`, L8)
//!
//! [eje]     AISLAMIENTO -- se corta el canal de GR y NADA MAS: el de copia,
//!           la pantalla y el resto del sistema siguen
//!
//! # El corte, en dos pasos (de lo dirigido a lo grueso)
//!
//! ```text
//!    VENCIDO    un trabajo del GR lanzado que no pago su semaforo en su
//!               plazo ([`PLAZO_US`]; o el suyo, los cortos del kernel)
//!    1 PARAR    `STOP_CHANNEL` (0xA06F0112, bImmediate) sobre NUESTRO canal
//!               de GR (`control::Control::PararGr`): el RM lo saca del motor
//!               y de su lista; si no se deja sacar, le hace RC
//!               (`ctrla06fgpfifo.h`, r570.144)
//!    2 ESCALAR  si tras [`RESPUESTA_US`] el GR sigue OCUPADO y sin RC:
//!               `INTERNAL_RC_WATCHDOG_TIMEOUT` (0x20800A6A, sin parametros,
//!               sobre las asas INTERNAS del RM): lo que manda el RM de la CPU
//!               de NVIDIA cuando su vigilante ve la GPU colgada
//!               (`krcWatchdogRecovery_KERNEL`, `kernel_rc_watchdog_callback.c`)
//!               -- el GSP-RM hace la recuperacion
//!    ACABA      con el RC_TRIGGERED del canal en la cola del GSP (su Xid), o
//!               con el GR QUIETO (`NV_PGRAPH_STATUS`, 0x400700, bit 0), o
//!               sin ninguno tras los dos pasos: hay que reiniciar
//! ```
//!
//! En los tres casos el canal de GR queda fuera hasta reiniciar (parado y sin
//! su lista, o con RC): volver a levantarlo es E7b. Lo que importa ya: la
//! 3060 deja de girar, y se DICE en la cabina, como el juez dice un BODRIO.
//!
//! # Por que el RC_WATCHDOG_TIMEOUT va sobre las asas internas
//!
//! Sus banderas en `g_subdevice_nvoc.c` son 0xC0: `ROUTE_TO_PHYSICAL` e
//! `INTERNAL` ("only be allowed to be issued from RM itself"). En un sistema
//! con GSP, el "RM itself" de la CPU es quien habla con las asas que da
//! `GET_GSP_STATIC_INFO` (+0x640): las mismas con las que G0 ya pregunto los
//! buferes de GR en el metal. El contrato deja salir SOLO esta orden asi, con
//! sus parametros vacios ([`permitida`]).

use crate::control::{CABECERA_CONTROL, GSP_RM_CONTROL};
use crate::orden;

/// Lo que se espera a un trabajo del GR antes de darlo por VENCIDO: el
/// segundo de siempre (`FRACTAL_ESPERA_US` del kernel). Un trabajo bueno tarda
/// de 30 us a 800 ms (el fractal); el TDR de Windows da 2 s.
pub const PLAZO_US: u64 = 1_000_000;

/// Lo que se espera a cada paso del corte a que el GR se quede quieto o
/// llegue el RC_TRIGGERED, antes del siguiente.
pub const RESPUESTA_US: u64 = 250_000;

/// `NV2080_CTRL_CMD_INTERNAL_RC_WATCHDOG_TIMEOUT` (`ctrl2080internal.h`).
pub const RC_WATCHDOG_TIMEOUT: u32 = 0x2080_0A6A;

/// `NV_PGRAPH_STATUS`: que unidades del motor grafico siguen ocupadas; el
/// bit 0 (`STATE`) es "ocupado".
pub const ESTADO_GR: u32 = 0x0040_0700;

/// El motor grafico, ocupado (por su `NV_PGRAPH_STATUS`).
pub const fn ocupado(estado: u32) -> bool {
    estado & 1 != 0
}

/// **VENCIDO**: se lanzo, no se pago, y paso su plazo. Lo que no se lanzo no
/// esta en la 3060; lo que se pago ya volvio, aunque saliera mal.
pub const fn vencido(lanzado: bool, pagado: bool, us: u64, plazo: u64) -> bool {
    lanzado && !pagado && us >= plazo
}

/// Un paso del corte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paso {
    /// `STOP_CHANNEL` sobre nuestro canal de GR.
    Parar,
    /// `INTERNAL_RC_WATCHDOG_TIMEOUT` sobre las asas internas.
    Escalar,
}

/// Como acabo un corte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Final {
    /// El GSP-RM hizo RC del canal de GR: su Xid (0 si no es uno conocido).
    Rc(u32),
    /// El motor grafico se quedo QUIETO tras ese paso, sin RC.
    Quieto(Paso),
    /// Ni RC ni quieto tras los dos pasos: la 3060 sigue girando. Reiniciar.
    Sigue,
}

/// Lo que toca tras mirar un paso del corte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tras {
    /// Seguir mirando.
    Esperar,
    /// Pasar al paso siguiente.
    Siguiente(Paso),
    /// Acabado.
    Acabo(Final),
}

/// **Mirar un paso**: `rc` = el Xid del RC_TRIGGERED del canal de GR en la
/// cola del GSP, si ya esta; `estado` = `NV_PGRAPH_STATUS`; `us` = desde que
/// se pidio el paso.
pub const fn tras(paso: Paso, rc: Option<u32>, estado: u32, us: u64) -> Tras {
    if let Some(xid) = rc {
        return Tras::Acabo(Final::Rc(xid));
    }
    if !ocupado(estado) {
        return Tras::Acabo(Final::Quieto(paso));
    }
    if us < RESPUESTA_US {
        return Tras::Esperar;
    }
    match paso {
        Paso::Parar => Tras::Siguiente(Paso::Escalar),
        Paso::Escalar => Tras::Acabo(Final::Sigue),
    }
}

/// Un final en 64 bits, para la cabina y el escritorio: `tipo | xid << 8`
/// (tipo 1 = RC, 2 = quieto tras PARAR, 3 = quieto tras ESCALAR, 4 = sigue).
pub const fn empaquetar(f: Final) -> u64 {
    match f {
        Final::Rc(xid) => 1 | (xid as u64) << 8,
        Final::Quieto(Paso::Parar) => 2,
        Final::Quieto(Paso::Escalar) => 3,
        Final::Sigue => 4,
    }
}

/// Lo de [`empaquetar`], de vuelta (`None` si no es un final).
pub const fn desempaquetar(v: u64) -> Option<Final> {
    match v & 0xFF {
        1 => Some(Final::Rc((v >> 8) as u32)),
        2 => Some(Final::Quieto(Paso::Parar)),
        3 => Some(Final::Quieto(Paso::Escalar)),
        4 => Some(Final::Sigue),
        _ => None,
    }
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **El paso 2**: `INTERNAL_RC_WATCHDOG_TIMEOUT` sobre el cliente y el
/// subdispositivo INTERNOS del RM, sin parametros.
pub fn escalar(hueco: &mut [u8], numero: u32, cliente: u32, subdispositivo: u32) -> Option<usize> {
    if cliente == 0 || subdispositivo == 0 {
        return None;
    }
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL, |d| {
        poner(d, 0, cliente);
        poner(d, 4, subdispositivo);
        poner(d, 8, RC_WATCHDOG_TIMEOUT);
        // status, paramsSize y flags a 0: sin parametros.
    })
}

/// **Puede salir?** Solo esta orden, sin parametros (`paramsSize` 0 y nada
/// detras), con asas distintas de 0. `d` son los datos del mensaje.
pub fn permitida(d: &[u8]) -> bool {
    if d.len() != CABECERA_CONTROL {
        return false;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    u(0) != 0 && u(4) != 0 && u(8) == RC_WATCHDOG_TIMEOUT && u(12) == 0 && u(16) == 0 && u(20) == 0
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn vencido_es_lanzado_sin_pagar_y_fuera_de_plazo() {
        assert!(vencido(true, false, PLAZO_US, PLAZO_US));
        assert!(!vencido(true, false, PLAZO_US - 1, PLAZO_US));
        // Pagado justo al borde: volvio (aunque saliera mal).
        assert!(!vencido(true, true, PLAZO_US + 5, PLAZO_US));
        // Lo que no se lanzo no esta en la 3060.
        assert!(!vencido(false, false, 10 * PLAZO_US, PLAZO_US));
        // Un plazo propio: los cortos del kernel (100 ms).
        assert!(vencido(true, false, 100_000, 100_000));
    }

    #[test]
    fn los_pasos_del_corte() {
        let ocupada = 0x0000_0081;
        let quieta = 0x0000_0080;
        // Esperando: ni RC, ni quieto, ni plazo.
        assert_eq!(tras(Paso::Parar, None, ocupada, RESPUESTA_US - 1), Tras::Esperar);
        // El RC manda, aunque siga ocupada (el reset va detras).
        assert_eq!(tras(Paso::Parar, Some(8), ocupada, 10), Tras::Acabo(Final::Rc(8)));
        assert_eq!(tras(Paso::Escalar, Some(0), ocupada, 10), Tras::Acabo(Final::Rc(0)));
        // Quieta tras parar: sin escalar.
        assert_eq!(tras(Paso::Parar, None, quieta, 10), Tras::Acabo(Final::Quieto(Paso::Parar)));
        // Ocupada pasado el plazo: escalar; y tras escalar, sigue.
        assert_eq!(tras(Paso::Parar, None, ocupada, RESPUESTA_US), Tras::Siguiente(Paso::Escalar));
        assert_eq!(tras(Paso::Escalar, None, ocupada, RESPUESTA_US), Tras::Acabo(Final::Sigue));
        assert_eq!(tras(Paso::Escalar, None, quieta, RESPUESTA_US + 1), Tras::Acabo(Final::Quieto(Paso::Escalar)));
    }

    #[test]
    fn el_final_va_y_vuelve() {
        for f in [Final::Rc(69), Final::Rc(0), Final::Quieto(Paso::Parar), Final::Quieto(Paso::Escalar), Final::Sigue] {
            assert_eq!(desempaquetar(empaquetar(f)), Some(f));
        }
        assert_eq!(desempaquetar(0), None);
        assert_eq!(empaquetar(Final::Rc(109)), 1 | 109 << 8);
    }

    #[test]
    fn el_escalon_es_la_orden_de_nvidia_sin_parametros() {
        let mut h = [0u8; 4096];
        assert_eq!(escalar(&mut h, 9, 0, 5), None);
        assert_eq!(escalar(&mut h, 9, 0xC1D0_0001, 0), None);
        let n = escalar(&mut h, 9, 0xC1D0_0001, 0x5C00_0003).unwrap();
        assert_eq!(n, CABECERA + CABECERA_CONTROL);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.datos()), (GSP_RM_CONTROL, CABECERA_CONTROL));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        let d = &h[CABECERA..n];
        assert!(permitida(d));
        // Con un byte de parametros de mas, o con otra orden, o sin asa: NO.
        let mut largo = [0u8; CABECERA_CONTROL + 4];
        largo[..CABECERA_CONTROL].copy_from_slice(d);
        assert!(!permitida(&largo));
        let mut otra = [0u8; CABECERA_CONTROL];
        otra.copy_from_slice(d);
        otra[8..12].copy_from_slice(&0x2080_0A32u32.to_le_bytes());
        assert!(!permitida(&otra));
        otra.copy_from_slice(d);
        otra[16..20].copy_from_slice(&4u32.to_le_bytes());
        assert!(!permitida(&otra));
        otra.copy_from_slice(d);
        otra[0..4].fill(0);
        assert!(!permitida(&otra));
    }

    #[test]
    fn el_contrato_deja_salir_el_corte_y_nada_parecido() {
        use crate::contrato::{permitido, No};
        let mut h = [0u8; 4096];
        let n = escalar(&mut h, 9, 0xC1D0_0001, 0x5C00_0003).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(GSP_RM_CONTROL));
        // PararGr va por la lista de `Control`, con su bImmediate exacto.
        let n = crate::control::pedir(&mut h, 10, crate::control::Control::PararGr).unwrap();
        assert_eq!(permitido(&h[..n]), Ok(GSP_RM_CONTROL));
        // bImmediate = 0 (esperar a que se quede quieto: no se quedara): NO.
        h[CABECERA + CABECERA_CONTROL] = 0;
        let mut s = Suma::default();
        h[32..36].fill(0);
        s.mas(&h[..n]);
        h[32..36].copy_from_slice(&s.valor().to_le_bytes());
        assert_eq!(permitido(&h[..n]), Err(No::Control));
    }
}
