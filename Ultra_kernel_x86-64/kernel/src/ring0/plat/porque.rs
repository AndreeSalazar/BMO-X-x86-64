//! **POR QUE SE REINICIO LA VEZ ANTERIOR** (11-10, V10 de
//! `PLAN_LOS_DOCE_DIRECTORES`): lo que el CHIPSET y la CPU guardan del ultimo
//! reinicio, leido una vez al arrancar y dicho en CABINA (y por ella en el
//! `CAIDA.TXT` y el DIARIO).
//!
//! [carril]  AMARILLO  es un instrumento: lee y lo dice, no escribe nada
//! [consumo] NADA      una vez, al arrancar
//!
//! El tablero (`cabina/tablero.rs`) dice QUE hacia cada nucleo cuando la
//! maquina murio; esto dice COMO murio, y son preguntas distintas. Un triple
//! fallo, una `#MC` con `CR4.MCE` a cero (los obreros: el trampolin no lo
//! pone) y un "sync flood" del data fabric reinician IGUAL de golpe, sin
//! pantalla, y solo el hardware sabe cual fue:
//!
//! ```text
//!    el FCH       S5_RESET_STATUS (MMIO 0xFED8_0300 + 0xC0): un bit por
//!                 motivo de reinicio. Solo AMD familia 17h en adelante
//!    los bancos   MCi_STATUS (MSR 0x401 + 4*i): un error de maquina que
//!    MCA          la CPU guardo; sobreviven a un reinicio en caliente
//! ```
//!
//! *** LA TABLA DE BITS DEL FCH NO SE VIO EN EL METAL TODAVIA: es la que usa
//! Linux (`arch/x86/kernel/cpu/amd.c`). Se comprueba sola: despues de un
//! `reboot` de BMO-X (0x6 a `0xCF9`) tiene que salir el bit 19. Si sale otro,
//! la tabla no vale para esta placa y lo que diga no se cree. Por eso se dice
//! tambien el valor CRUDO.
//!
//! Solo se lee: no se limpia nada (el registro del FCH se limpia escribiendo
//! unos, y eso no se hace sin haberlo visto antes en el metal).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::mm::HIGH_MEM_BASE;

/// Lo leido, para `INFO_REINICIO_FCH` (crudo; `u64::MAX` = no se leyo) e
/// `INFO_REINICIO_MCA` (con error `[0..32)`, bancos `[32..64)`).
static FCH: AtomicU64 = AtomicU64::new(u64::MAX);
static MCA: AtomicU64 = AtomicU64::new(0);

/// El `S5_RESET_STATUS` leido al arrancar (`u64::MAX` = no se leyo).
pub fn fch() -> u64 {
    FCH.load(Ordering::Relaxed)
}

/// Los bancos MCA al arrancar: con error `[0..32)`, cuantos hay `[32..64)`.
pub fn mca() -> u64 {
    MCA.load(Ordering::Relaxed)
}

/// El registro del FCH con el motivo del ultimo reinicio (fisico).
const S5_RESET_STATUS: u64 = 0xFED8_0300 + 0xC0;

/// Los motivos, por bit (la tabla de Linux; ver arriba).
const MOTIVOS: [(u32, &str); 20] = [
    (0, "el pin THERMTRIP: la CPU se paso de temperatura"),
    (1, "el boton de encendido, 4 segundos"),
    (2, "el pin SHUTDOWN_L"),
    (4, "apagado remoto (ASF)"),
    (9, "el limite termico interno de la CPU"),
    (16, "el pin RESET_L: el boton de reset"),
    (17, "un reset de PCI pedido por software"),
    (18, "0x4 en 0xCF9"),
    (19, "0x6 en 0xCF9: un reboot pedido (el de BMO-X)"),
    (20, "0xE en 0xCF9"),
    (21, "un cambio de estado ACPI"),
    (22, "el pin KB_RST_L (el 8042)"),
    (23, "APAGADO INTERNO de la CPU: triple fallo, o #MC sin CR4.MCE"),
    (24, "no arranco antes del plazo del firmware"),
    (25, "el perro guardian del hardware"),
    (26, "reset remoto (ASF)"),
    (27, "SYNC FLOOD del data fabric: un error NO corregido"),
    (29, "el FCH y el MP1 no se pusieron de acuerdo"),
    (30, "un error de paridad"),
    (31, "un sync flood pedido por software"),
];

fn cpuid(hoja: u32) -> (u32, u32, u32, u32) {
    // CPUID existe en todo x86-64 y no tiene efectos.
    let r = core::arch::x86_64::__cpuid(hoja);
    (r.eax, r.ebx, r.ecx, r.edx)
}

fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: quien llama solo pide MSR que la CPU dice tener (MCG_CAP y
    // los bancos por debajo de su cuenta).
    unsafe { core::arch::asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack)) };
    (hi as u64) << 32 | lo as u64
}

/// AMD de la familia 17h (Zen) en adelante: los que tienen ese registro.
fn es_zen() -> bool {
    let (_, b, c, d) = cpuid(0);
    if (b, d, c) != (0x6874_7541, 0x6974_6E65, 0x444D_4163) {
        return false;
    }
    let (a, _, _, _) = cpuid(1);
    let base = (a >> 8) & 0xF;
    let familia = if base == 0xF { base + ((a >> 20) & 0xFF) } else { base };
    familia >= 0x17
}

/// **Decirlo**, una vez, al arrancar (en el BSP, con el physmap listo).
pub fn decir() {
    use crate::ring0::cabina::{info, warn};
    if es_zen() {
        // SAFETY: lectura de 32 bits del MMIO del FCH por el physmap (como
        // el LAPIC en `smp/lapic.rs`); leerlo no cambia nada.
        let v = unsafe { core::ptr::read_volatile((HIGH_MEM_BASE + S5_RESET_STATUS) as *const u32) };
        FCH.store(v as u64, Ordering::Relaxed);
        if v == 0 || v == u32::MAX {
            info("reinicio", "el FCH no dice motivo (S5_RESET_STATUS crudo)", v as u64);
        } else {
            warn("reinicio", "el FCH guarda el motivo del ultimo reinicio; crudo", v as u64);
            for (bit, que) in MOTIVOS.iter() {
                if v >> bit & 1 != 0 {
                    warn("reinicio", que, *bit as u64);
                }
            }
        }
    }
    // Los bancos MCA (CPUID.1:EDX bit 14).
    let (_, _, _, d) = cpuid(1);
    if d & (1 << 14) == 0 {
        return;
    }
    let cap = rdmsr(0x179);
    let bancos = (cap & 0xFF).min(64) as u32;
    let mut guardados = 0u64;
    for i in 0..bancos {
        let st = rdmsr(0x401 + 4 * i);
        if st >> 63 == 0 {
            continue;
        }
        guardados += 1;
        warn("reinicio", "MCA: un banco guarda un ERROR de maquina; banco", i as u64);
        warn("reinicio", "  ...status (61 = no corregido, 57 = contexto roto)", st);
        if st >> 58 & 1 != 0 {
            warn("reinicio", "  ...direccion", rdmsr(0x402 + 4 * i));
        }
    }
    MCA.store(guardados | (bancos as u64) << 32, Ordering::Relaxed);
    if guardados == 0 {
        info("reinicio", "MCA: ningun error de maquina guardado; bancos", bancos as u64);
    }
}

/// **`CR4.MCE` en el BSP** (11-10, V12), DESPUES de leer los bancos: un error
/// de maquina pasa a ser la #MC (vector 18, IST3 de `s1_cpu`, que ya estaba
/// puesta para eso) en vez de un apagado del PC. El BSP heredaba lo que
/// dejara el firmware; nadie lo ponia. Se dice si ya estaba.
pub fn armar_mce() {
    let (_, _, _, d) = cpuid(1);
    if d & (1 << 7) == 0 {
        return;
    }
    let cr4: u64;
    // SAFETY: CR4 del BSP; solo se pone el bit 6 (MCE), con su IST3 puesta.
    unsafe { core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack)) };
    if cr4 & (1 << 6) != 0 {
        crate::ring0::cabina::info("reinicio", "CR4.MCE del BSP: ya estaba (el firmware)", 1);
        return;
    }
    unsafe { core::arch::asm!("mov cr4, {}", in(reg) cr4 | 1 << 6, options(nostack)) };
    crate::ring0::cabina::info("reinicio", "CR4.MCE del BSP: puesto (una #MC ya no apaga el PC)", 1);
}
