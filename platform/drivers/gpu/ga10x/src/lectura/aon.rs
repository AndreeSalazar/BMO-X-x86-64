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

/// OpenRM 615.78.08, GA100 dev_fb.h: campos VAL 31:4, paginas de 4 KiB.
pub const MMU_LOCK_LO: u32 = 0x001F_A82C;
pub const MMU_LOCK_HI: u32 = 0x001F_A830;
pub const MMU_LOCK_PLM: u32 = 0x001F_A7C8;

/// Una region MMU leida, no una autorizacion para modificar WPR2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandadoMmu {
    pub desde: u64,
    pub hasta: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoCandadoMmu {
    Plm,
    Pri,
}

/// `None` significa hi < lo, no lectura fallida. Cero/cero es una region
/// valida segun OpenRM; el llamador conserva la validez por separado.
pub fn candado_mmu(plm: u32, lo: u32, hi: u32) -> Result<Option<CandadoMmu>, NoCandadoMmu> {
    if crate::es_error_pri(plm) || plm & 1 == 0 {
        return Err(NoCandadoMmu::Plm);
    }
    if crate::es_error_pri(lo) || crate::es_error_pri(hi) {
        return Err(NoCandadoMmu::Pri);
    }
    let desde = u64::from(lo >> 4) << 12;
    let hasta = u64::from(hi >> 4) << 12;
    Ok((hasta >= desde).then_some(CandadoMmu { desde, hasta }))
}

/// Si `reg` cae en lo que este fichero guarda.
pub const fn es_de_aqui(reg: u32) -> bool {
    (reg >= PGC6_DESDE && reg <= PGC6_HASTA) || reg == WPR2_LO || reg == WPR2_HI
        || reg == MMU_LOCK_LO || reg == MMU_LOCK_HI || reg == MMU_LOCK_PLM
}

/// El bit de "el GSP-RM volvio", de una lectura de [`BSI_14`].
pub const fn handoff(bsi_14: u32) -> bool {
    bsi_14 & BSI_14_HANDOFF != 0
}

/// El progreso del GFW, de una lectura de [`GFW_PROGRESO`].
pub const fn gfw_acabo(progreso: u32) -> bool {
    !crate::es_error_pri(progreso) && progreso & 0xFF == GFW_ACABO
}

/// Solo una lectura autorizada y sin PRI tiene un progreso de ocho bits.
/// `None` no se convierte en cero medido por INFO_FB.
pub const fn gfw_leido(plm: u32, progreso: u32) -> Option<u8> {
    if crate::es_error_pri(plm) || plm & 1 == 0 || crate::es_error_pri(progreso) {
        None
    } else {
        Some((progreso & 0xFF) as u8)
    }
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

    #[test]
    fn un_error_pri_que_acaba_en_ff_no_es_gfw_completado() {
        for dato in [0xFFFF_FFFF, 0xBAD0_00FF, 0xBADF_FFFF] {
            assert!(!gfw_acabo(dato));
            assert_eq!(gfw_leido(1, dato), None);
            assert_eq!(gfw_leido(dato, 0xFF), None);
        }
        assert_eq!(gfw_leido(0, 0xFF), None);
        assert_eq!(gfw_leido(1, 0), Some(0));
        assert_eq!(gfw_leido(1, 0xFF), Some(0xFF));
    }

    #[test]
    fn el_candado_distingue_cero_ausencia_y_error() {
        assert_eq!(candado_mmu(1, 0, 0), Ok(Some(CandadoMmu { desde: 0, hasta: 0 })));
        assert_eq!(candado_mmu(1, 0x20, 0x10), Ok(None));
        for plm in [0, 0xBAD0_0001, 0xFFFF_FFFF] {
            assert_eq!(candado_mmu(plm, 0, 0), Err(NoCandadoMmu::Plm));
        }
        for pri in [0xBADF_1002, 0xFFFF_FFFF] {
            assert_eq!(candado_mmu(1, pri, 0), Err(NoCandadoMmu::Pri));
            assert_eq!(candado_mmu(1, 0, pri), Err(NoCandadoMmu::Pri));
        }
    }

    #[test]
    fn el_candado_usa_40_bits_y_no_confunde_las_banderas_con_paginas() {
        let c = candado_mmu(1, 0x02F4_000F, 0x02FF_EE0F).unwrap().unwrap();
        assert_eq!((c.desde, c.hasta), (0x2F4000000, 0x2FFEE0000));
        assert!(es_de_aqui(MMU_LOCK_LO) && es_de_aqui(MMU_LOCK_HI) && es_de_aqui(MMU_LOCK_PLM));
    }
}
