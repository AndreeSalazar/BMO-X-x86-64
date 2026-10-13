//! **EL PARTE DEL CANAL MUERTO** (13-10): lo que el GSP-RM cuenta cuando mata
//! un canal, leido campo a campo y no adivinado.
//!
//! capa: puro -- se le dan las palabras de la cola del GSP; la memoria la lee
//! el kernel (L8)
//!
//! [eje]     CORRECCION -- cada campo, de `g_rpc-structures.h` de la 570.144
//!           (`rpc_rc_triggered_v17_02` y `rpc_os_error_log_v17_00`); los
//!           nombres de los Xid, de `nverror.h`; los del fallo de memoria, de
//!           `gv100/dev_fault.h` (`NV_PFAULT_FAULT_TYPE_*`)
//!
//! # Por que existe
//!
//! Hasta hoy BMO-X sabia del canal muerto UN numero, y lo adivinaba: "la
//! primera palabra de la 2 a la 7 que sea un Xid conocido". El 13-10 la 3060
//! murio en el primer lote de Cyberpunk 2077 y el Xid no llego a ninguna
//! parte que el propietario viera. Un fallo que no se puede decir no se puede
//! arreglar.
//!
//! # Los campos (`rpc_rc_triggered_v17_02`, alineacion natural de C)
//!
//! ```text
//!    +0   nv2080EngineType   el motor (1 = GR0)
//!    +4   chid               el canal
//!    +8   gfid               0 = la tarjeta entera (sin vGPU)
//!    +12  exceptLevel
//!    +16  exceptType         EL Xid (ROBUST_CHANNEL_*)
//!    +20  scope
//!    +24  partitionAttributionId (16 bits, y 2 de relleno)
//!    +28  mmuFaultAddrLo     la direccion del fallo de memoria (Xid 31)
//!    +32  mmuFaultAddrHi
//!    +36  mmuFaultType       NV_PFAULT_FAULT_TYPE_*
//!    +40  bCallbackNeeded    (8 bits, y 3 de relleno)
//!    +44  rcJournalBufferSize, y el diario detras
//! ```
//!
//! Comprobado contra el metal del 28-09: `00000001 00000002 00000000
//! 00000002 00000045` = motor GR, canal 2, gfid 0, nivel 2, Xid 0x45 = 69.
//! La cuenta vieja daba el mismo Xid aquel dia porque la primera palabra
//! "conocida" era justo la +16; con un nivel 13 o 31 habria dado otro.

use crate::rpc::{Mensaje, CABECERA};

/// `RC_TRIGGERED`: el GSP-RM mato un canal.
pub const RC_TRIGGERED: u32 = 0x1004;
/// `OS_ERROR_LOG`: el texto del error, como lo imprime el driver de NVIDIA.
pub const OS_ERROR_LOG: u32 = 0x1006;
/// Lo menos que mide un `RC_TRIGGERED` para leer su Xid (+16).
const RC_MIDE: usize = 20;
/// Y para leer tambien la direccion y el tipo del fallo de memoria (+28..+40).
const RC_FALLO: usize = 40;
/// Donde empieza el texto de un `OS_ERROR_LOG` (tras tipo, runlist y canal).
const LOG_TEXTO: usize = 12;
/// Cuanto texto se guarda (el de NVIDIA mide hasta 256).
pub const TEXTO: usize = 160;

/// **Un canal muerto, contado por el GSP-RM.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rc {
    pub motor: u32,
    pub chid: u32,
    pub gfid: u32,
    pub nivel: u32,
    /// El Xid (`exceptType`).
    pub xid: u32,
    pub alcance: u32,
    /// La direccion virtual del fallo de memoria (con sentido en un Xid 31).
    pub va: u64,
    /// `NV_PFAULT_FAULT_TYPE_*` (con sentido en un Xid 31).
    pub tipo_fallo: u32,
}

/// **El texto de un `OS_ERROR_LOG`**: el Xid, el canal y la frase.
#[derive(Clone, Copy, Debug)]
pub struct Texto {
    pub xid: u32,
    pub chid: u32,
    pub bytes: [u8; TEXTO],
    pub n: usize,
}

impl Texto {
    pub fn frase(&self) -> &[u8] {
        &self.bytes[..self.n]
    }
}

/// **Lo que se encontro en la cola**, recorrida sin moverla.
#[derive(Clone, Copy, Debug, Default)]
pub struct Parte {
    pub rc: Option<Rc>,
    pub texto: Option<Texto>,
}

/// Leer un `RC_TRIGGERED` de `datos` bytes; `palabra(o)` da los 4 bytes en
/// `o` contando desde el principio de los datos.
pub fn rc_de(mut palabra: impl FnMut(usize) -> u32, datos: usize) -> Option<Rc> {
    if datos < RC_MIDE {
        return None;
    }
    Some(Rc {
        motor: palabra(0),
        chid: palabra(4),
        gfid: palabra(8),
        nivel: palabra(12),
        xid: palabra(16),
        alcance: palabra(20),
        va: if datos >= RC_FALLO { palabra(28) as u64 | (palabra(32) as u64) << 32 } else { 0 },
        tipo_fallo: if datos >= RC_FALLO { palabra(36) } else { 0 },
    })
}

/// Leer un `OS_ERROR_LOG` de `datos` bytes. El texto acaba en su primer 0.
pub fn texto_de(mut palabra: impl FnMut(usize) -> u32, datos: usize) -> Option<Texto> {
    if datos < LOG_TEXTO + 1 {
        return None;
    }
    let mut t = Texto { xid: palabra(0), chid: palabra(8), bytes: [0; TEXTO], n: 0 };
    let largo = (datos - LOG_TEXTO).min(TEXTO);
    while t.n < largo {
        let o = LOG_TEXTO + t.n;
        let b = (palabra(o & !3) >> (8 * (o & 3))) as u8;
        if b == 0 {
            break;
        }
        // Lo que no es ASCII imprimible no se pinta: un punto.
        t.bytes[t.n] = if (0x20..0x7F).contains(&b) { b } else { b'.' };
        t.n += 1;
    }
    Some(t)
}

/// **Recorrer la cola del GSP SIN moverla** -- de `leido` (el `readPtr` de la
/// CPU) a `escrito` (el `writePtr` del GSP), en una cola de `paginas` -- y
/// dar el PRIMER `RC_TRIGGERED` del canal `chid` y el ULTIMO texto suyo.
/// `leer(pagina, desde)`: 4 bytes del mensaje que empieza en `pagina`,
/// contando desde su cabecera y dando la vuelta a la cola.
pub fn buscar(leer: &mut impl FnMut(u64, usize) -> u32, leido: u64, escrito: u64, paginas: u64, chid: u32) -> Parte {
    let mut parte = Parte::default();
    if paginas == 0 {
        return parte;
    }
    let (fin, mut p) = (escrito % paginas, leido % paginas);
    let mut vueltas = 0;
    while p != fin && vueltas < paginas {
        let mut h = [0u8; CABECERA];
        for k in 0..CABECERA / 4 {
            h[4 * k..4 * k + 4].copy_from_slice(&leer(p, 4 * k).to_le_bytes());
        }
        let m = Mensaje::de(&h);
        if !m.bien_formado() {
            break;
        }
        let datos = m.datos();
        if m.funcion == RC_TRIGGERED && parte.rc.is_none() {
            parte.rc = rc_de(|o| leer(p, CABECERA + o), datos).filter(|r| r.chid == chid);
        } else if m.funcion == OS_ERROR_LOG {
            if let Some(t) = texto_de(|o| leer(p, CABECERA + o), datos).filter(|t| t.chid == chid) {
                parte.texto = Some(t);
            }
        }
        p = (p + m.paginas as u64) % paginas;
        vueltas += m.paginas as u64;
    }
    parte
}

/// **El nombre de un Xid** (`nverror.h`, `ROBUST_CHANNEL_*`), en castellano
/// y con el de NVIDIA detras, para buscarlo.
pub const fn nombre_xid(x: u32) -> &'static str {
    match x {
        13 => "excepcion del motor grafico: un sombreador o un estado hizo algo prohibido (GR_EXCEPTION)",
        31 => "fallo de pagina: la 3060 leyo o escribio donde no tenia memoria (FIFO_ERROR_MMU_ERR_FLT)",
        32 => "error del PBDMA: una orden del empuje mal formada (PBDMA_ERROR)",
        43 => "el canal se paro para verificarlo (RESETCHANNEL_VERIF_ERROR)",
        44 => "fallo del motor grafico al cambiar de contexto (GR_FAULT_DURING_CTXSW)",
        45 => "el canal se quito a la fuerza (PREEMPTIVE_REMOVAL)",
        48 => "error doble de ECC en la memoria (GPU_ECC_DBE)",
        69 => "un metodo o un valor que el motor grafico no acepta (GR_CLASS_ERROR)",
        79 => "la tarjeta se cayo del bus (GPU_HAS_FALLEN_OFF_THE_BUS)",
        94 => "error contenido (CONTAINED_ERROR)",
        95 => "error NO contenido (UNCONTAINED_ERROR)",
        _ => "un Xid que BMO-X no nombra todavia (buscalo en nverror.h)",
    }
}

/// **El nombre de un tipo de fallo de memoria** (`NV_PFAULT_FAULT_TYPE_*`).
pub const fn nombre_fallo(t: u32) -> &'static str {
    match t {
        0x0 => "sin PDE (la tabla de paginas no tiene esa rama)",
        0x1 => "PDE de medida equivocada",
        0x2 => "sin PTE (la pagina no esta mapeada)",
        0x3 => "fuera del limite de la VA",
        0x4 => "bloque de instancia sin atar",
        0x5 => "violacion de privilegio",
        0x6 => "escritura en una pagina de solo lectura",
        0x7 => "lectura de una pagina de solo escritura",
        0x8 => "violacion de la mascara de pitch",
        0x9 => "creacion de trabajo",
        0xA => "apertura no soportada",
        0xB => "fallo de compresion",
        0xC => "clase de memoria (kind) no soportada",
        0xD => "violacion de region",
        0xE => "memoria envenenada",
        0xF => "violacion atomica",
        _ => "un tipo de fallo desconocido",
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cola(mensajes: &[(u32, &[u32])]) -> ([u32; 1024], u64) {
        // Una cola de 4 paginas de 1 KiB de palabras: cada mensaje en su pagina.
        let mut c = [0u32; 1024];
        for (i, (f, datos)) in mensajes.iter().enumerate() {
            let base = i * 256;
            let mut h = [0u8; CABECERA];
            let largo = (crate::rpc::CABECERA_RPC + 4 * datos.len()) as u32;
            for (o, v) in [(40, 1u32), (48, crate::rpc::VERSION_3_0), (52, crate::rpc::FIRMA_VRPC), (56, largo), (60, *f)] {
                h[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            for k in 0..CABECERA / 4 {
                c[base + k] = u32::from_le_bytes([h[4 * k], h[4 * k + 1], h[4 * k + 2], h[4 * k + 3]]);
            }
            for (k, d) in datos.iter().enumerate() {
                c[base + CABECERA / 4 + k] = *d;
            }
        }
        (c, mensajes.len() as u64)
    }

    #[test]
    fn el_metal_del_28_09_da_motor_canal_y_xid_69() {
        let crudo = [1u32, 2, 0, 2, 0x45, 0, 0, 0, 0, 0, 0, 0];
        let r = rc_de(|o| crudo[o / 4], 48).unwrap();
        assert_eq!((r.motor, r.chid, r.gfid, r.nivel, r.xid), (1, 2, 0, 2, 69));
        assert!(nombre_xid(r.xid).contains("GR_CLASS_ERROR"));
    }

    #[test]
    fn un_fallo_de_pagina_dice_su_direccion_y_su_tipo() {
        let crudo = [1u32, 2, 0, 2, 31, 1, 0, 0xDEAD_B000, 0x3, 0x2, 0, 0];
        let r = rc_de(|o| crudo[o / 4], 48).unwrap();
        assert_eq!(r.va, 0x3_DEAD_B000);
        assert_eq!(r.tipo_fallo, 2);
        assert!(nombre_fallo(r.tipo_fallo).contains("PTE"));
        let corto = rc_de(|o| crudo[o / 4], 36).unwrap();
        assert_eq!((corto.xid, corto.va, corto.tipo_fallo), (31, 0, 0), "corto: el Xid si, el fallo no");
        assert!(rc_de(|o| crudo[o / 4], 16).is_none(), "sin su Xid, nada");
    }

    #[test]
    fn el_texto_acaba_en_su_cero() {
        let mut d = [0u32; 8];
        d[0] = 13;
        d[2] = 2;
        d[3] = u32::from_le_bytes(*b"Out ");
        d[4] = u32::from_le_bytes(*b"Of R");
        d[5] = u32::from_le_bytes([b'a', b'n', 0x01, 0]);
        let t = texto_de(|o| d[o / 4], 32).unwrap();
        assert_eq!((t.xid, t.chid), (13, 2));
        assert_eq!(t.frase(), b"Out Of Ran.");
    }

    #[test]
    fn la_cola_da_el_rc_de_su_canal_y_su_ultimo_texto() {
        let otro: &[u32] = &[1, 5, 0, 2, 13, 0, 0, 0, 0, 0, 0, 0];
        let suyo: &[u32] = &[1, 2, 0, 2, 31, 0, 0, 0x1000, 0, 2, 0, 0];
        let log: &[u32] = &[31, 0, 2, u32::from_le_bytes(*b"MMU "), u32::from_le_bytes(*b"Faul"), u32::from_le_bytes([b't', 0, 0, 0])];
        let (c, n) = cola(&[(RC_TRIGGERED, otro), (0x1020, &[0; 4]), (RC_TRIGGERED, suyo), (OS_ERROR_LOG, log)]);
        let mut leer = |p: u64, o: usize| c[(p as usize * 256 + o / 4) % 1024];
        let parte = buscar(&mut leer, 0, n, 8, 2);
        let rc = parte.rc.expect("el del canal 2");
        assert_eq!((rc.xid, rc.va, rc.tipo_fallo), (31, 0x1000, 2));
        assert_eq!(parte.texto.unwrap().frase(), b"MMU Fault");
        let nada = buscar(&mut leer, 0, n, 8, 7);
        assert!(nada.rc.is_none() && nada.texto.is_none());
    }
}
