//! **LA FILA DE ESTA CPU** (H4.0 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10): quien
//! es hermano SMT de quien, que nucleos comparten L3 y cuales son grandes o
//! chicos, leido del silicio. Lo decide `bmo_orquesta::topologia` (puro, con
//! banco: el 5600X, un Zen 2 de dos CCX y un Intel hibrido); aqui solo se
//! PREGUNTA.
//!
//! [carril]  AMARILLO  CPUID y la MADT; el orden en que se usan los obreros
//! [consumo] NADA      una vez por obrero al levantarse, y al repartir
//!
//! ```text
//!    los APIC   la MADT (`plat::madt::censo`)
//!    smt_bits   AMD 0x8000_001E EBX[15:8]; si no, 0x0B nivel 0 EAX[4:0]
//!    l3_bits    la cache de nivel 3: AMD 0x8000_001D, Intel 0x04
//!    el tipo    0x1A en un hibrido (CPUID 7 EDX[15]), lo lee CADA obrero
//!               en su nucleo al levantarse ([`apuntar`])
//! ```
//!
//! Sin un `if` por modelo: un Zen 2, un Zen 3 o un Intel hibrido son tres
//! filas distintas de los mismos tres numeros.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering::SeqCst};

use bmo_orquesta::topologia::{bits_de_cache, nivel_de_cache, smt_bits_amd, Fila, Medida, HILOS_MAXIMOS};

use super::ficha::MAX_OBREROS;

fn cpuid(hoja: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (a, b, c, d): (u32, u32, u32, u32);
    // SAFETY: CPUID no tiene efectos; rbx se guarda (lo usa LLVM).
    unsafe {
        core::arch::asm!(
            "mov {b:r}, rbx",
            "cpuid",
            "xchg {b:r}, rbx",
            b = out(reg) b,
            inout("eax") hoja => a,
            inout("ecx") sub => c,
            out("edx") d,
            options(nostack, preserves_flags),
        );
    }
    (a, b, c, d)
}

/// El tipo de nucleo de cada obrero (0: no es hibrido o no se sabe), y el
/// del BSP.
static TIPO: [AtomicU8; MAX_OBREROS] = [const { AtomicU8::new(0) }; MAX_OBREROS];
static TIPO_BSP: AtomicU8 = AtomicU8::new(0);

/// Un Intel hibrido (CPUID 7, EDX[15]).
fn hibrido() -> bool {
    cpuid(0, 0).0 >= 0x1A && cpuid(7, 0).3 & (1 << 15) != 0
}

/// Este nucleo, grande o chico (0x1A, EAX[31:24]).
fn tipo_de_aqui() -> u8 {
    if hibrido() { (cpuid(0x1A, 0).0 >> 24) as u8 } else { 0 }
}

/// **Lo llama cada obrero al levantarse**, en SU nucleo: su tipo (la hoja
/// 0x1A contesta por el nucleo que la ejecuta).
pub fn apuntar(indice: u32) {
    if let Some(t) = TIPO.get(indice as usize) {
        t.store(tipo_de_aqui(), SeqCst);
    }
    APUNTADOS.fetch_add(1, SeqCst);
}

/// Cuantos obreros dijeron ya su tipo: el orden guardado se rehace si sube.
static APUNTADOS: AtomicU32 = AtomicU32::new(0);

/// `(smt_bits, l3_bits)` de esta CPU.
fn bits() -> (u8, u8) {
    let amd = cpuid(0, 0).1 == 0x6874_7541; // "Auth"
    let ext = cpuid(0x8000_0000, 0).0;
    let smt = if amd && ext >= 0x8000_001E {
        smt_bits_amd(cpuid(0x8000_001E, 0).1)
    } else if cpuid(0, 0).0 >= 0x0B {
        (cpuid(0x0B, 0).0 & 0x1F) as u8
    } else {
        0
    };
    // El descriptor de cache de nivel 3: AMD 0x8000_001D (con TOPOEXT),
    // Intel 0x04. Sin el, una sola L3 para todos.
    let hoja = if amd && ext >= 0x8000_001D && cpuid(0x8000_0001, 0).2 & (1 << 22) != 0 {
        Some(0x8000_001D)
    } else if !amd && cpuid(0, 0).0 >= 4 {
        Some(4)
    } else {
        None
    };
    let mut l3 = 31;
    if let Some(h) = hoja {
        for sub in 0..8 {
            let eax = cpuid(h, sub).0;
            match nivel_de_cache(eax) {
                0 => break,
                3 => {
                    l3 = bits_de_cache(eax);
                    break;
                }
                _ => {}
            }
        }
    }
    (smt, l3)
}

/// **La fila de esta CPU** (con los obreros que ya dijeron su tipo).
pub fn fila() -> Option<Fila> {
    let censo = super::super::madt::censo()?;
    let ids = censo.ids();
    let mut tipos = [0u8; HILOS_MAXIMOS];
    let bsp = super::tramp::apic_id();
    if TIPO_BSP.load(SeqCst) == 0 {
        TIPO_BSP.store(tipo_de_aqui(), SeqCst);
    }
    for (k, &apic) in ids.iter().enumerate().take(HILOS_MAXIMOS) {
        tipos[k] = if apic == bsp {
            TIPO_BSP.load(SeqCst)
        } else {
            super::ficha::indice_de(apic).map_or(0, |i| TIPO[i as usize].load(SeqCst))
        };
    }
    let (smt_bits, l3_bits) = bits();
    Some(Fila::leer(&Medida { apics: ids, smt_bits, l3_bits, tipos: &tipos[..ids.len().min(HILOS_MAXIMOS)] }))
}

// == EL ORDEN DE LOS OBREROS, guardado ====================================
//
// `ring3::repartir` lo pide en CADA dibujo: la MADT no se recorre por
// dibujo. Se rehace cuando cambia cuantos obreros se dieron de alta o
// dijeron su tipo.

static ORDEN: [AtomicU32; MAX_OBREROS] = [const { AtomicU32::new(u32::MAX) }; MAX_OBREROS];
static ORDEN_N: AtomicU32 = AtomicU32::new(0);
static ORDEN_DE: AtomicU32 = AtomicU32::new(u32::MAX);
static DICHO: AtomicBool = AtomicBool::new(false);

/// **Los obreros en el orden en que se usan** (indices de `ficha`): un
/// nucleo fisico distinto para cada uno primero, luego los hermanos SMT
/// (`bmo_orquesta::topologia::Fila::orden`). Los que la fila no situa, al
/// final, en su orden de llegada.
pub fn orden(sale: &mut [u32; MAX_OBREROS]) -> usize {
    let altas = super::crew::ENTRARON.load(SeqCst) | APUNTADOS.load(SeqCst) << 16;
    if ORDEN_DE.load(SeqCst) != altas {
        rehacer();
        ORDEN_DE.store(altas, SeqCst);
    }
    let n = ORDEN_N.load(SeqCst) as usize;
    for (k, s) in sale.iter_mut().enumerate().take(n) {
        *s = ORDEN[k].load(SeqCst);
    }
    n
}

fn rehacer() {
    let mut puestos = 0usize;
    let poner = |i: u32, puestos: &mut usize| {
        if *puestos < MAX_OBREROS && !(0..*puestos).any(|k| ORDEN[k].load(SeqCst) == i) {
            ORDEN[*puestos].store(i, SeqCst);
            *puestos += 1;
        }
    };
    if let Some(f) = fila() {
        let mut apics = [0u32; HILOS_MAXIMOS];
        let n = f.orden(super::tramp::apic_id(), &mut apics);
        for &apic in &apics[..n] {
            if let Some(i) = super::ficha::indice_de(apic) {
                poner(i, &mut puestos);
            }
        }
        if !DICHO.swap(true, SeqCst) {
            crate::ring0::cabina::count("smp", "topologia: nucleos fisicos", f.nucleos() as u64);
            crate::ring0::cabina::count("smp", "topologia: hilos", f.hilos().len() as u64);
            crate::ring0::cabina::count("smp", "topologia: grupos de L3", f.l3s() as u64);
            crate::ring0::cabina::count("smp", "topologia: nucleos chicos", f.chicos() as u64);
        }
    }
    for i in 0..MAX_OBREROS as u32 {
        if super::ficha::retrato(i).apic != u32::MAX {
            poner(i, &mut puestos);
        }
    }
    ORDEN_N.store(puestos as u32, SeqCst);
}
