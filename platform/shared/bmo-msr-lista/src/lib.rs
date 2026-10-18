//! **LOS MSR QUE UNA TAREA PUEDE LEER** -- PP3 de
//! `docs/plan/PLAN_LAS_PUERTAS_PROFUNDAS.md` (18-10, el propietario: *"dale con
//! PP3 primero"*).
//!
//! [carril]  VERDE     decide; no lee ningun registro (eso es del perfil del
//!                     Ryzen, en Ring 0, y del emulador en el anfitrion)
//! [consumo] NADA      una busqueda en una tabla de doce filas
//!
//! `rdmsr` es CPL 0 siempre: el silicio no tiene puerta para el. La puerta es
//! una OPERACION (`TASK_OP_LEE_MSR`): la tarea pide un numero, el kernel mira
//! esta lista, y solo si esta -- y si corre en la CPU del perfil, donde se sabe
//! que ese registro EXISTE: un MSR que no existe es un `#GP` del kernel -- lo
//! lee y devuelve sus 64 bits. Nunca escribe.
//!
//! ```text
//!    la BLANCA   lo que se puede leer: lo que dice como es y como va este
//!                silicio, y no lo que hace otra tarea ni como se manda
//!    la NEGRA    lo que NUNCA, con su motivo: se escribe ANTES que la blanca,
//!                y el banco comprueba que no se tocan
//! ```
//!
//! [!] Lo que esta lista NO lleva, a proposito: los contadores de ENERGIA en
//! crudo (`0xC001_0299`..`029B`). Leidos seguido, con su grano fino, son un
//! canal lateral conocido (Platypus, 2020: la energia de RAPL deja ver una
//! clave), y por eso Linux los cerro a root ese mismo ano. El kernel ya da la
//! energia ACUMULADA por segundo (`INFO_CPU_UJ_*`): para medir vatios basta.

#![no_std]

/// **Una fila de la lista blanca**: el registro, su nombre en el manual de
/// AMD (PPR del Family 19h) y lo que dice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fila {
    pub msr: u32,
    pub nombre: &'static str,
    pub que_dice: &'static str,
}

/// **LA LISTA BLANCA** del Ryzen 5 5600X (Zen 3, Family 19h): solo lectura.
pub const BLANCA: &[Fila] = &[
    Fila { msr: 0x0000_008B, nombre: "PATCH_LEVEL", que_dice: "la version del microcodigo cargado" },
    Fila { msr: 0x0000_00E7, nombre: "MPERF", que_dice: "este nucleo, al ritmo nominal (el del TSC)" },
    Fila { msr: 0x0000_00E8, nombre: "APERF", que_dice: "este nucleo, al ritmo REAL: APERF / MPERF es su frecuencia" },
    Fila { msr: 0xC001_0061, nombre: "PStateCurLim", que_dice: "los P-states que este nucleo tiene permitidos ahora" },
    Fila { msr: 0xC001_0063, nombre: "PStateStat", que_dice: "el P-state en que esta este nucleo" },
    Fila { msr: 0xC001_0064, nombre: "PStateDef0", que_dice: "la frecuencia y el voltaje del P-state 0" },
    Fila { msr: 0xC001_0065, nombre: "PStateDef1", que_dice: "la frecuencia y el voltaje del P-state 1" },
    Fila { msr: 0xC001_0066, nombre: "PStateDef2", que_dice: "la frecuencia y el voltaje del P-state 2" },
    Fila { msr: 0xC001_0067, nombre: "PStateDef3", que_dice: "la frecuencia y el voltaje del P-state 3" },
    Fila { msr: 0xC001_0068, nombre: "PStateDef4", que_dice: "la frecuencia y el voltaje del P-state 4" },
    Fila { msr: 0xC001_0069, nombre: "PStateDef5", que_dice: "la frecuencia y el voltaje del P-state 5" },
    Fila { msr: 0xC001_006A, nombre: "PStateDef6", que_dice: "la frecuencia y el voltaje del P-state 6" },
    Fila { msr: 0xC001_006B, nombre: "PStateDef7", que_dice: "la frecuencia y el voltaje del P-state 7" },
];

/// **LA LISTA NEGRA**: lo que no se lee nunca por la puerta, con su motivo.
/// No hace falta para decidir (lo que no esta en la blanca ya es NO): esta
/// para que el banco PRUEBE que ninguna de estas llego a la blanca, y para que
/// quien la amplie lea por que no.
pub const NEGRA: &[(u32, &str)] = &[
    (0x0000_0010, "el TSC: `rdtsc` ya lo da en Ring 3, y escribirlo mueve el tiempo de todos"),
    (0x0000_001B, "APIC_BASE: donde vive el APIC del kernel"),
    (0x0000_0048, "SPEC_CTRL: las defensas contra Spectre que el kernel puso"),
    (0x0000_0049, "PRED_CMD: una orden, no un dato"),
    (0xC000_0080, "EFER: el modo largo y el NX de toda la maquina"),
    (0xC000_0081, "STAR: los selectores de la puerta del kernel"),
    (0xC000_0082, "LSTAR: DONDE entra el kernel en cada syscall"),
    (0xC000_0083, "CSTAR: la entrada de 32 bits del kernel"),
    (0xC000_0084, "SFMASK: que banderas apaga la puerta"),
    (0xC000_0100, "FS_BASE: la de la tarea que corre (ya la tiene `rdfsbase`)"),
    (0xC000_0101, "GS_BASE: la del kernel en este nucleo"),
    (0xC000_0102, "KERNEL_GS_BASE: la del kernel guardada en `swapgs`"),
    (0xC000_0103, "TSC_AUX: el numero de nucleo que pone el kernel"),
    (0xC001_0010, "SYSCFG: la configuracion de la memoria (y su cifrado)"),
    (0xC001_0015, "HWCR: los interruptores del procesador"),
    (0xC001_0062, "PStateCtl: MANDA el P-state; leerlo no dice nada que PStateStat no diga"),
    (0xC001_0299, "RAPL_PWR_UNIT: va con la energia en crudo (ver la cabecera)"),
    (0xC001_029A, "CORE_ENERGY_STAT: energia en crudo, un canal lateral (Platypus)"),
    (0xC001_029B, "PKG_ENERGY_STAT: energia en crudo, un canal lateral (Platypus)"),
];

/// Los contadores de rendimiento (`PERF_CTL` y `PERF_CTR` de Zen,
/// `0xC001_0200`..`020B`): cuentan lo que hace OTRA tarea si el kernel no los
/// limpia. Son de `TASK_OP_CONTAR`, no de esta puerta.
pub const CONTADORES: core::ops::RangeInclusive<u32> = 0xC001_0200..=0xC001_020B;

/// **Por que NO** -- el motivo que lleva el NO de la puerta (`BmoStatus::negado`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum No {
    /// El registro no esta en la lista blanca.
    FueraDeLaLista = 1,
    /// La maquina no es la CPU del perfil: no se sabe si el registro existe,
    /// y un MSR que no existe es un `#GP` del kernel.
    OtraCpu = 2,
}

/// **La decision**: la fila de `msr` si se puede leer en esta maquina.
/// `cpu_del_perfil`: lo que dice el kernel (`profile::extensiones().0`).
/// Lo de fuera de 32 bits (el `arg0` de la puerta es de 64) es NO.
pub fn decidir(msr: u64, cpu_del_perfil: bool) -> Result<&'static Fila, No> {
    let msr = u32::try_from(msr).map_err(|_| No::FueraDeLaLista)?;
    let fila = BLANCA.iter().find(|f| f.msr == msr).ok_or(No::FueraDeLaLista)?;
    if !cpu_del_perfil {
        return Err(No::OtraCpu);
    }
    Ok(fila)
}

#[cfg(test)]
mod banco {
    use super::*;

    #[test]
    fn nothing_black_and_no_counter_is_on_the_white_list() {
        for f in BLANCA {
            assert!(!NEGRA.iter().any(|(m, _)| *m == f.msr), "{} esta en las dos listas", f.nombre);
            assert!(!CONTADORES.contains(&f.msr), "{} es un contador de rendimiento", f.nombre);
        }
    }

    #[test]
    fn every_row_is_once_and_has_its_words() {
        for (i, f) in BLANCA.iter().enumerate() {
            assert!(BLANCA[i + 1..].iter().all(|g| g.msr != f.msr), "{:#x} dos veces", f.msr);
            assert!(!f.nombre.is_empty() && !f.que_dice.is_empty());
        }
        for (i, (m, por_que)) in NEGRA.iter().enumerate() {
            assert!(NEGRA[i + 1..].iter().all(|(n, _)| n != m), "{:#x} dos veces en la negra", m);
            assert!(!por_que.is_empty());
        }
    }

    #[test]
    fn the_door_reads_the_list_on_the_profile_cpu_and_says_why_not() {
        assert_eq!(decidir(0x8B, true).map(|f| f.nombre), Ok("PATCH_LEVEL"));
        assert_eq!(decidir(0xC001_0064, true).map(|f| f.nombre), Ok("PStateDef0"));
        // fuera de la lista: lo negro, lo que no existe, lo que no cabe en 32 bits
        assert_eq!(decidir(0xC000_0082, true), Err(No::FueraDeLaLista));
        assert_eq!(decidir(0xC001_029A, true), Err(No::FueraDeLaLista));
        assert_eq!(decidir(0xC001_0200, true), Err(No::FueraDeLaLista));
        assert_eq!(decidir(0x1_0000_008B, true), Err(No::FueraDeLaLista));
        // en otra CPU (o en QEMU), ni lo de la lista
        assert_eq!(decidir(0x8B, false), Err(No::OtraCpu));
        // y lo de fuera sigue diciendo que esta fuera, aunque sea otra CPU
        assert_eq!(decidir(0xC000_0080, false), Err(No::FueraDeLaLista));
    }
}
