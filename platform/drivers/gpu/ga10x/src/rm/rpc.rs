//! **LO QUE EL GSP DICE (L0c4a)** -- los mensajes de su cola, entendidos:
//! su cabecera, su suma de comprobacion y el nombre de cada tipo (r570.144).
//!
//! capa: puro -- recibe bytes de la cola y dice que mensaje es; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un mensaje mal partido es leer el siguiente a
//!           medias, y contestar a lo que el GSP no pregunto
//!
//! # Un mensaje (nova-core, `gsp/cmdq.rs` y `GSP_MSG_QUEUE_ELEMENT`)
//!
//! ```text
//!    +0   authTag[16], aad[16]         a 0 sin confidencialidad
//!    +32  checkSum    XOR rotado de TODO el mensaje: con el, da 0
//!    +36  seqNum      el numero del elemento en la cola
//!    +40  elemCount   cuantas paginas de 4 KiB de la cola ocupa
//!    +48  rpc_message_header_v03_00:
//!           header_version (3.0 = 0x03000000), signature "VRPC",
//!           length, function, rpc_result, rpc_result_private, sequence
//!    +80  los datos
//! ```
//!
//! Los del GSP son casi todos EVENTOS (0x1000 en adelante): lo que el GSP
//! cuenta sin que se le pregunte -- su secuenciador, sus avisos, su
//! `GSP_INIT_DONE`. Las RESPUESTAS llevan el numero de la funcion pedida.

/// Lo que mide la cabecera del elemento y la del RPC, juntas.
pub const CABECERA: usize = 80;
/// `NV_VGPU_MSG_SIGNATURE_VALID`: "VRPC".
pub const FIRMA_VRPC: u32 = 0x4350_5256;
/// `header_version` 3.0, la de r570.
pub const VERSION_3_0: u32 = 0x0300_0000;
/// Lo que mide la cabecera del RPC sola: `length` la CUENTA.
pub const CABECERA_RPC: usize = 32;

/// La cabecera de un mensaje, leida.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mensaje {
    pub suma: u32,
    pub numero: u32,
    pub paginas: u32,
    pub version: u32,
    pub firma: u32,
    pub largo: u32,
    pub funcion: u32,
    pub resultado: u32,
    pub resultado_privado: u32,
    pub secuencia: u32,
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl Mensaje {
    /// **Leer una cabecera** de los primeros 80 bytes de un elemento.
    pub fn de(b: &[u8; CABECERA]) -> Self {
        Mensaje {
            suma: u32_de(b, 32),
            numero: u32_de(b, 36),
            paginas: u32_de(b, 40),
            version: u32_de(b, 48),
            firma: u32_de(b, 52),
            largo: u32_de(b, 56),
            funcion: u32_de(b, 60),
            resultado: u32_de(b, 64),
            resultado_privado: u32_de(b, 68),
            secuencia: u32_de(b, 72),
        }
    }

    /// Tiene la forma de un mensaje de r570: firma, version, un `length` que
    /// al menos cubre su cabecera, y cabe en las paginas que dice ocupar
    /// (entre 1 y 16: 64 KiB es el tope de nova-core).
    pub fn bien_formado(&self) -> bool {
        self.firma == FIRMA_VRPC
            && self.version == VERSION_3_0
            && self.largo as usize >= CABECERA_RPC
            && (1..=16).contains(&self.paginas)
            && (CABECERA + self.datos()) as u64 <= self.paginas as u64 * 4096
    }

    /// **Los bytes de datos**, detras de los 80 de la cabecera.
    ///
    /// ** `length` CUENTA los 32 de la cabecera del RPC (nova-core,
    /// `payload_length`: "`rpc.length` includes the length of the RPC message
    /// header"). Hasta L0c4b2a (24-09) se leia como si no: la suma cubria 32
    /// bytes de mas, y dio 0 en los 835 del metal solo porque detras habia ceros.
    pub fn datos(&self) -> usize {
        (self.largo as usize).saturating_sub(CABECERA_RPC)
    }

    /// Los bytes que cubre la suma, cabecera incluida.
    pub fn bytes_sumados(&self) -> usize {
        CABECERA + self.datos()
    }
}

/// **La suma de comprobacion de nova-core** (`calculate_checksum`), por
/// trozos: cada byte rotado 8 x (su posicion mod 8) en un u64, todo con XOR, y
/// las dos mitades con XOR al final. Un mensaje entero, con su `checkSum`
/// puesto, da 0.
#[derive(Clone, Copy, Default)]
pub struct Suma {
    acc: u64,
    i: usize,
}

impl Suma {
    pub fn mas(&mut self, b: &[u8]) {
        for &x in b {
            self.acc ^= (x as u64).rotate_left(((self.i % 8) * 8) as u32);
            self.i += 1;
        }
    }

    pub fn valor(&self) -> u32 {
        ((self.acc >> 32) as u32) ^ (self.acc as u32)
    }
}

/// **El nombre de un tipo de mensaje** (`NV_VGPU_MSG_EVENT_*` y las funciones
/// que usa nova-core), sin el prefijo.
pub fn nombre(funcion: u32) -> &'static [u8] {
    match funcion {
        0 => b"NOP",
        1 => b"SET_GUEST_SYSTEM_INFO",
        2 => b"ALLOC_ROOT",
        3 => b"ALLOC_DEVICE",
        4 => b"ALLOC_MEMORY",
        5 => b"ALLOC_CTX_DMA",
        6 => b"ALLOC_CHANNEL_DMA",
        7 => b"MAP_MEMORY",
        8 => b"BIND_CTX_DMA",
        9 => b"ALLOC_OBJECT",
        10 => b"FREE",
        11 => b"LOG",
        47 => b"UNLOADING_GUEST_DRIVER",
        51 => b"GET_STATIC_INFO",
        65 => b"GET_GSP_STATIC_INFO",
        71 => b"CONTINUATION_RECORD",
        72 => b"GSP_SET_SYSTEM_INFO",
        73 => b"SET_REGISTRY",
        74 => b"GSP_INIT_POST_OBJGPU",
        76 => b"GSP_RM_CONTROL",
        0x1001 => b"GSP_INIT_DONE",
        0x1002 => b"GSP_RUN_CPU_SEQUENCER",
        0x1003 => b"POST_EVENT",
        0x1004 => b"RC_TRIGGERED",
        0x1005 => b"MMU_FAULT_QUEUED",
        0x1006 => b"OS_ERROR_LOG",
        0x1007 => b"RG_LINE_INTR",
        0x1008 => b"GPUACCT_PERFMON_UTIL_SAMPLES",
        0x1009 => b"SIM_READ",
        0x100A => b"SIM_WRITE",
        0x100B => b"SEMAPHORE_SCHEDULE_CALLBACK",
        0x100C => b"UCODE_LIBOS_PRINT",
        0x100D => b"VGPU_GSP_PLUGIN_TRIGGERED",
        0x100E => b"PERF_GPU_BOOST_SYNC_LIMITS_CALLBACK",
        0x100F => b"PERF_BRIDGELESS_INFO_UPDATE",
        0x1010 => b"VGPU_CONFIG",
        0x1011 => b"DISPLAY_MODESET",
        0x1012 => b"EXTDEV_INTR_SERVICE",
        0x1013..=0x1017 => b"NVLINK_INBAND_RECEIVED_DATA",
        0x1018 => b"TIMED_SEMAPHORE_RELEASE",
        0x1019 => b"NVLINK_IS_GPU_DEGRADED",
        0x101A => b"PFM_REQ_HNDLR_STATE_SYNC_CALLBACK",
        0x101B => b"NVLINK_FAULT_UP",
        0x101C => b"GSP_LOCKDOWN_NOTICE",
        0x101D => b"MIG_CI_CONFIG_UPDATE",
        0x101E => b"UPDATE_GSP_TRACE",
        0x101F => b"NVLINK_FATAL_ERROR_RECOVERY",
        0x1020 => b"GSP_POST_NOCAT_RECORD",
        0x1021 => b"FECS_ERROR",
        0x1022 => b"RECOVERY_ACTION",
        _ => b"?",
    }
}

/// `GSP_INIT_DONE`: lo que L0c4b tiene que llegar a ver.
pub const INIT_DONE: u32 = 0x1001;
/// El secuenciador: el GSP pide a la CPU que toque registros por el.
pub const SECUENCIADOR: u32 = 0x1002;

// -- L0c4b1: lo que se consume sin contestar ------------------------------------

/// `GSP_POST_NOCAT_RECORD`: un registro de diagnostico del GSP-RM.
pub const NOCAT: u32 = 0x1020;

/// **Se puede consumir sin contestar?** Los que el GSP CUENTA y no pide nada:
/// sus NOCAT, sus `LIBOS_PRINT` y su registro de errores. Todo lo demas --el
/// secuenciador, `GSP_INIT_DONE`, una respuesta-- se deja en la cola para
/// quien sepa contestarlo (L0c4b2).
pub const fn informativo(funcion: u32) -> bool {
    matches!(funcion, NOCAT | 0x100C | 0x1006)
}

/// **Los textos legibles de unos datos**: tramos de 4 o mas caracteres ASCII
/// imprimibles, en orden, hasta `fuera.len()`. Cada uno como `(desde, hasta)`.
/// Un NOCAT lleva el nombre de su fuente y de lo que paso en claro.
pub fn textos(datos: &[u8], fuera: &mut [(usize, usize)]) -> usize {
    let (mut n, mut ini) = (0, None);
    for (i, &c) in datos.iter().chain(core::iter::once(&0)).enumerate() {
        let imprimible = (0x20..0x7F).contains(&c);
        match (imprimible, ini) {
            (true, None) => ini = Some(i),
            (false, Some(a)) => {
                if i - a >= 4 && n < fuera.len() {
                    fuera[n] = (a, i);
                    n += 1;
                }
                ini = None;
            }
            _ => {}
        }
    }
    n
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;

    fn mensaje(funcion: u32, datos: &[u8]) -> std::vec::Vec<u8> {
        let mut m = vec![0u8; 4096];
        m[36..40].copy_from_slice(&7u32.to_le_bytes());
        m[40..44].copy_from_slice(&1u32.to_le_bytes());
        m[48..52].copy_from_slice(&VERSION_3_0.to_le_bytes());
        m[52..56].copy_from_slice(&FIRMA_VRPC.to_le_bytes());
        m[56..60].copy_from_slice(&((CABECERA_RPC + datos.len()) as u32).to_le_bytes());
        m[60..64].copy_from_slice(&funcion.to_le_bytes());
        m[80..80 + datos.len()].copy_from_slice(datos);
        let mut s = Suma::default();
        s.mas(&m[..80 + datos.len()]);
        m[32..36].copy_from_slice(&s.valor().to_le_bytes());
        m
    }

    #[test]
    fn un_mensaje_con_su_suma_da_cero() {
        let m = mensaje(SECUENCIADOR, b"hola GSP");
        let c = Mensaje::de(m[..80].try_into().unwrap());
        assert!(c.bien_formado());
        assert_eq!((c.funcion, c.numero, c.paginas, c.largo, c.datos()), (0x1002, 7, 1, 40, 8));
        let mut s = Suma::default();
        s.mas(&m[..c.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        // Por trozos da lo mismo que de una vez.
        let mut t = Suma::default();
        t.mas(&m[..13]);
        t.mas(&m[13..c.bytes_sumados()]);
        assert_eq!(t.valor(), 0);
    }

    #[test]
    fn un_byte_cambiado_no_da_cero() {
        let mut m = mensaje(INIT_DONE, &[1, 2, 3, 4]);
        m[81] ^= 0x10;
        let c = Mensaje::de(m[..80].try_into().unwrap());
        let mut s = Suma::default();
        s.mas(&m[..c.bytes_sumados()]);
        assert_ne!(s.valor(), 0);
    }

    #[test]
    fn lo_que_no_tiene_forma_de_mensaje() {
        let mut m = mensaje(INIT_DONE, &[]);
        m[52] = b'X';
        assert!(!Mensaje::de(m[..80].try_into().unwrap()).bien_formado(), "sin VRPC");
        let mut m = mensaje(INIT_DONE, &[]);
        m[56..60].copy_from_slice(&5000u32.to_le_bytes());
        assert!(!Mensaje::de(m[..80].try_into().unwrap()).bien_formado(), "no cabe en su pagina");
        assert!(!Mensaje::de(&[0; 80]).bien_formado(), "una pagina a cero");
        let mut m = mensaje(INIT_DONE, &[]);
        m[56..60].copy_from_slice(&31u32.to_le_bytes());
        assert!(!Mensaje::de(m[..80].try_into().unwrap()).bien_formado(), "un length que no cubre ni su cabecera");
    }

    #[test]
    fn los_que_se_consumen_sin_contestar() {
        assert!(informativo(NOCAT) && informativo(0x100C) && informativo(0x1006));
        assert!(!informativo(SECUENCIADOR) && !informativo(INIT_DONE) && !informativo(73));
    }

    #[test]
    fn los_textos_de_un_nocat() {
        let d = b"\x01\x00GSP-RM\x00\x02ab\x00falta registro\xff\x00XYZW";
        let mut t = [(0, 0); 4];
        let n = textos(d, &mut t);
        let dichos: std::vec::Vec<&[u8]> = t[..n].iter().map(|&(a, b)| &d[a..b]).collect();
        assert_eq!(dichos, [&b"GSP-RM"[..], b"falta registro", b"XYZW"], "ab es corto; el ultimo acaba con los datos");
        let mut uno = [(0, 0); 1];
        assert_eq!(textos(d, &mut uno), 1, "no pasa de lo que cabe");
    }

    #[test]
    fn los_nombres() {
        assert_eq!(nombre(0x1001), b"GSP_INIT_DONE");
        assert_eq!(nombre(0x1002), b"GSP_RUN_CPU_SEQUENCER");
        assert_eq!(nombre(73), b"SET_REGISTRY");
        assert_eq!(nombre(0x9999), b"?");
    }
}
