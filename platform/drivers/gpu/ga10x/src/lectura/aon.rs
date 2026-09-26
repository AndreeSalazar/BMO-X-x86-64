//! **LO QUE SOBREVIVE** -- las direcciones de la 3060 cuyo estado NO se borra
//! con un reinicio del PC, cada una con su motivo, y el UNICO sitio donde se
//! escriben sus numeros.
//!
//! [carril]  VERDE     constantes y lecturas puras: ni un registro se ESCRIBE
//! [estado]  AON WPR       el dominio que no se apaga (PGC6/BSI) y la WPR2
//!
//! # Por que existe (26-09)
//!
//! El booter de la 3060 dio 0x15 cinco veces. Tres hipotesis cayeron midiendo
//! (la WPR2 caliente; "reiniciaste sin cortar la corriente"; el bit
//! `boot_stage_3_handoff` puesto de antes), y las tres tenian lo mismo: una
//! suposicion sobre QUE ESTADO de la tarjeta sobrevive a QUE. Hasta hoy esas
//! direcciones estaban repartidas en cinco ficheros (el kernel, `correr`,
//! `descarga`, `identidad`, `secuenciador`), cada una con su comentario. Aqui
//! se juntan, y la regla A del guardian `la-3060` hace que no se vuelvan a
//! repartir: un numero de este dominio fuera de este fichero es un FAIL.
//!
//! # *** La regla que da sentido al fichero: BMO-X NUNCA ESCRIBE AQUI
//!
//! Estos registros los escriben el firmware de arranque de la tarjeta (GFW),
//! FWSEC, el booter y el GSP-RM, firmados por NVIDIA. Si BMO-X escribiera uno,
//! el siguiente arranque heredaria lo que BMO-X dejo, y el fallo apareceria
//! en OTRA sesion, lejos de su causa. Por eso aqui no hay ni una funcion que
//! escriba, y el guardian lo comprueba (regla A3).
//!
//! # Que sobrevive a que (lo MEDIDO en esta 3060, y lo que no se sabe)
//!
//! ```text
//!                         reinicio del PC     reinicio por el bus    cortar la
//!                         (sin cortar)        (el cargador, s1_cpu)  corriente
//!    registros del chip   se borran           se borran              se borran
//!    VRAM                 SOBREVIVE (*)       sin medir              se pierde
//!    WPR2 (0x1FA824/28)   SOBREVIVE           sin medir              cae
//!                         (metal 24-09 07:48: el GSP-RM de antes vivo)
//!    PGC6/BSI (0x118xxx)  sin medir: a las    sin medir              cae
//!                         19:59 del 25-09 el
//!                         bit 26 estaba ABAJO
//!                         antes del booter
//!    fusibles             fijos               fijos                  fijos
//!    VBIOS (ROM)          fija                fija                   fija
//! ```
//!
//! (*) Lo dice la WPR2 viva: el GSP-RM de la sesion anterior seguia en ella.
//!
//! "sin medir" es trabajo: cada casilla se cierra con una fila de `gpu` en un
//! arranque de cada tipo (la autopsia del booter ya apunta BSI y WPR2).

// == PGC6 / BSI: el dominio que no se apaga ==================================

/// `NV_PGC6_BSI_SECURE_SCRATCH_14`. Su bit 26 (`boot_stage_3_handoff`) lo
/// pone el GSP-RM al volver a la vida; `correr` lo espera en la fase 1 del
/// secuenciador (como nova-core, `check_reload_completed`). **Motivo:** es la
/// prueba de que el GSP-RM volvio, y la primera sospechosa del 0x15 (no lo
/// explico el 25-09 19:59: abajo antes del booter y aun asi 0x15).
pub const BSI_14: u32 = 0x0011_80F8;
/// El bit `boot_stage_3_handoff` de [`BSI_14`].
pub const BSI_14_HANDOFF: u32 = 1 << 26;

/// `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05[0]`: bits 0..7, el progreso del
/// firmware de arranque de la tarjeta; 0xFF = acabo. **Motivo:** ni FWSEC ni
/// el booter se tocan antes de 0xFF (nova-core, `gfw.rs`).
pub const GFW_PROGRESO: u32 = 0x0011_8234;
/// El progreso cuando el firmware de arranque acabo.
pub const GFW_ACABO: u32 = 0xFF;

/// La mascara de privilegio de [`GFW_PROGRESO`]: su bit 0 dice si la CPU lo
/// puede leer. **Motivo:** leerlo sin permiso da un error de PRI, no un 0.
pub const GFW_PLM: u32 = 0x0011_8128;

/// La VRAM en MiB, que escribe el firmware de arranque al acabar. **Motivo:**
/// es lo primero que dice si esta tarjeta es la 12G (ver `identidad`).
pub const VRAM_MIB: u32 = 0x0011_83A4;

/// Todo el dominio PGC6/BSI: lo que un registro de aqui significa lo dice su
/// constante; un numero que caiga en el rango y no tenga constante es de
/// alguien que no lo documento.
pub const PGC6_DESDE: u32 = 0x0011_8000;
pub const PGC6_HASTA: u32 = 0x0011_8FFF;

// == La WPR2: la region protegida que monta FWSEC y extiende el booter =======

/// `NV_PFB_PRI_MMU_WPR2_ADDR_LO`: el principio de la WPR2 (>> 4, en 4 KiB).
pub const WPR2_LO: u32 = 0x001F_A824;
/// `NV_PFB_PRI_MMU_WPR2_ADDR_HI`: su final; cero cuando no hay WPR2.
/// **Motivo:** la WPR2 SOBREVIVE a un reinicio sin cortar la corriente (metal
/// 24-09 07:48), y FWSEC-FRTS solo corre con ella vacia.
pub const WPR2_HI: u32 = 0x001F_A828;

/// Si `reg` cae en lo que este fichero guarda.
pub const fn es_de_aqui(reg: u32) -> bool {
    (reg >= PGC6_DESDE && reg <= PGC6_HASTA) || reg == WPR2_LO || reg == WPR2_HI
}

/// El bit de "el GSP-RM volvio", de una lectura de [`BSI_14`].
pub const fn handoff(bsi_14: u32) -> bool {
    bsi_14 & BSI_14_HANDOFF != 0
}

/// El progreso del GFW, de una lectura de [`GFW_PROGRESO`].
pub const fn gfw_acabo(progreso: u32) -> bool {
    progreso & 0xFF == GFW_ACABO
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_dominio_es_el_que_dice() {
        for r in [BSI_14, GFW_PROGRESO, GFW_PLM, VRAM_MIB, WPR2_LO, WPR2_HI] {
            assert!(es_de_aqui(r), "{r:#x}");
        }
        assert!(!es_de_aqui(0x0011_0040), "el MAILBOX0 del GSP no es de aqui");
        assert!(!es_de_aqui(0x0011_7FFF) && !es_de_aqui(0x0011_9000));
    }

    #[test]
    fn los_bits_se_leen_bien() {
        assert!(handoff(1 << 26) && !handoff(0) && !handoff(1 << 25));
        assert!(gfw_acabo(0xFF) && gfw_acabo(0x1234_56FF) && !gfw_acabo(0xFE));
    }
}
