//! **APAGAR EL GSP EN ORDEN (L0c5)** -- lo que hace falta para que el
//! siguiente arranque encuentre la 3060 limpia: el GSP-RM se despide, FWSEC
//! cierra lo suyo, y el booter de DESCARGA baja la WPR2.
//!
//! capa: puro -- el mensaje, los registros y lo que tienen que decir; los
//! falcons y la cola los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- el orden de nouveau (`tu102_gsp_fini` +
//!           `r535_gsp_fini`, Linux) y del RM de NVIDIA (`kgspTeardown_TU102`)
//!
//! # Por que (metal 24-09: tres veces 0x15)
//!
//! El booter devolvio `0x15` tres veces, siempre despues de una sesion con el
//! GSP-RM corriendo; tras cortar la corriente, arranco a la primera. nova-core
//! (`gsp/boot.rs`) lo dice claro: sin el paquete de descarga, *"The GPU will
//! need to be reset before the driver can bind again"*. BMO-X nunca apagaba
//! el GSP: su WPR2 y su RM seguian vivos para el arranque siguiente.
//!
//! # El orden
//!
//! ```text
//!    1  RPC UNLOADING_GUEST_DRIVER (47): 8 B a cero (arranque normal, sin
//!       suspender, nivel 0); el GSP-RM contesta
//!    2  esperar (hasta 2 s) el MAILBOX0 del falcon del GSP = 0x80000000:
//!       el GSP-RM se ha suspendido
//!    3  reset del falcon del GSP
//!    4  FWSEC-SB: el mismo FWSEC de FRTS, con la orden 0x19 (sin region);
//!       su error, en los 16 bits bajos de 0x1400 + 0x15 * 4
//!    5  el booter de DESCARGA en el SEC2, con MAILBOX0 = MAILBOX1 = 0xFF;
//!       acaba con la WPR2 ya no esta (0x1FA828 = 0)
//! ```

use crate::orden;

/// `NV_VGPU_MSG_FUNCTION_UNLOADING_GUEST_DRIVER`.
pub const UNLOADING_GUEST_DRIVER: u32 = 47;
/// `rpc_unloading_guest_driver_v1F_07`: `bInPMTransition` (u8),
/// `bGc6Entering` (u8), relleno, `newLevel` (u32).
pub const BYTES: usize = 8;

/// **El mensaje**: la RPC con sus 8 B a cero -- una descarga normal.
pub fn pedir(hueco: &mut [u8], numero: u32) -> Option<usize> {
    orden::componer(hueco, numero, UNLOADING_GUEST_DRIVER, BYTES, |_| {})
}

/// Los datos que el contrato deja salir: EXACTAMENTE estos.
pub const DATOS: [u8; BYTES] = [0; BYTES];

/// El MAILBOX0 del falcon del GSP cuando el GSP-RM se ha suspendido
/// (`LIBOS_INTERRUPT_PROCESSOR_SUSPENDED`, bit 31; nouveau compara igual).
pub const SUSPENDIDO: u32 = 0x8000_0000;
/// Cuanto se espera a que se suspenda (nouveau: 2 s).
pub const ESPERA_SUSPENDIDO_MS: u64 = 2000;

/// La orden de FWSEC para cerrar (`NVFW_FALCON_APPIF_DMEMMAPPER_CMD_SB`).
pub const ORDEN_SB: u32 = 0x19;
/// Donde deja FWSEC-SB su error (`NV_PBUS_SW_SCRATCH(0x15)`).
pub const SB_ERROR: u32 = 0x0000_1400 + 0x15 * 4;

/// Si FWSEC-SB acabo bien: los 16 bits bajos a cero.
pub const fn sb_bien(v: u32) -> bool {
    v & 0xFFFF == 0
}

/// Lo que va en los buzones del SEC2 para una descarga normal.
pub const BUZON_DESCARGA: u32 = 0xFF;
/// `NV_PFB_PRI_MMU_WPR2_ADDR_HI`: cero cuando ya no hay WPR2.
pub const WPR2_HI: u32 = 0x001F_A828;

/// Si el booter de descarga hizo lo suyo: la WPR2 abajo. Es lo UNICO que
/// mira nouveau (`tu102_gsp_booter_unload`); su MAILBOX0 se muestra, no juzga.
pub const fn descargado(wpr2_hi: u32) -> bool {
    wpr2_hi == 0
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, CABECERA};

    #[test]
    fn el_mensaje() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 30).unwrap();
        assert_eq!(n, CABECERA + BYTES);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero, m.datos()), (UNLOADING_GUEST_DRIVER, 30, BYTES));
        assert_eq!(h[CABECERA..n], DATOS);
        // El contrato la deja salir.
        assert_eq!(crate::contrato::permitido(&h[..n]), Ok(UNLOADING_GUEST_DRIVER));
    }

    #[test]
    fn el_contrato_no_deja_otra_descarga() {
        let mut h = [0u8; 4096];
        let n = pedir(&mut h, 30).unwrap();
        // Una suspension (bInPMTransition = 1) no: no hay con que volver.
        h[CABECERA] = 1;
        assert!(crate::contrato::permitido(&h[..n]).is_err());
    }

    #[test]
    fn lo_que_tienen_que_decir() {
        assert_eq!(SB_ERROR, 0x1454);
        assert!(sb_bien(0) && sb_bien(0xABCD_0000) && !sb_bien(1));
        assert!(descargado(0) && !descargado(0x2F4));
    }
}
