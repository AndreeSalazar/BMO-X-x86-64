//! **LOS CONTADORES DE ESTE ZEN 3, Y SI CUENTAN LO QUE DICE EL PAPEL** (C4
//! de `docs/plan/EL_FOCO.md`, LB4 de `PLAN_LA_LENTE_BINARIA.md`, 15-10).
//!
//! [carril]  AMARILLO  escribe PERF_CTL y lee PERF_CTR de ESTE nucleo, con
//!                     las interrupciones apagadas lo que dura un bucle
//! [consumo] NADA      solo cuando alguien pregunta (`INFO_CPU_CONTADOR`), y
//!                     la calibracion UNA vez por arranque (~3 ms)
//!
//! Lo que se decide (el catalogo, el `PERF_CTL`, el juez) es de
//! `bmo_contadores`, con banco. Aqui solo se escribe y se lee el MSR, y se
//! corren tres bucles que se saben de memoria:
//!
//! ```text
//!    recta         add; dec; jnz          3 instrucciones y 1 salto por vuelta
//!    azar          xorshift; test; jz     la mitad de los saltos, fallados
//!    persecucion   mov p, [p]             256 KiB en permutacion: la L1 falla
//! ```
//!
//! ** Solo en la CPU del PERFIL (`profile::extensiones`): un MSR que no existe
//! es un `#GP` del kernel. En otra CPU (o en una maquina virtual sin PMU) no
//! se toca nada y se dice.
//!
//! ** Solo LECTURA de lo que pasa y solo en el kernel (la regla del plan): un
//! contador mal puesto no rompe nada, pero un numero mal leido burla. Cada
//! fila dice su veredicto; un evento `PorConfirmar` no se cree hasta que su
//! fila diga `Cuenta` en el metal.

use bmo_contadores::{self as pc, Anillo, Control, Prueba};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    (hi as u64) << 32 | lo as u64
}

unsafe fn wrmsr(msr: u32, v: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") v as u32, in("edx") (v >> 32) as u32, options(nostack));
}

/// Si hay contadores extendidos (CPUID 0x80000001 ECX[23]).
pub fn extendidos() -> bool {
    // CPUID no tiene efectos, y toda CPU de 64 bits lo tiene.
    let max = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    max >= 0x8000_0001 && core::arch::x86_64::__cpuid(0x8000_0001).ecx >> pc::BIT_EXTENDIDOS & 1 != 0
}

/// Si se pueden tocar aqui: la CPU del perfil, con contadores extendidos.
pub fn se_pueden() -> bool {
    super::super::profile::extensiones().0 && extendidos()
}

/// El contador de la calibracion: el ULTIMO, para que los de abajo
/// (`CONTAR_DE_UN_PROGRAMA`) no se pisen con ella.
const DE_LA_CALIBRACION: u32 = 5;

/// **Contar** `evento` con el contador de la calibracion mientras corre `f`,
/// solo en Ring 0 y con las interrupciones apagadas (nada mas cuenta en medio).
fn contar(evento: &pc::Evento, f: impl FnOnce()) -> u64 {
    let (ctl, ctr) = pc::msr(DE_LA_CALIBRACION, true);
    let c = Control::de(evento, Anillo::Kernel);
    let flags: u64;
    // SAFETY: `se_pueden` dijo que estos MSR existen en ESTA CPU; ese contador
    // no lo usa nadie mas en el kernel, y se deja apagado y a cero.
    unsafe {
        asm!("pushfq", "pop {}", "cli", out(reg) flags, options(nomem));
        wrmsr(ctl, 0);
        wrmsr(ctr, 0);
        wrmsr(ctl, c.palabra());
    }
    f();
    let n = unsafe {
        wrmsr(ctl, c.apagado().palabra());
        let n = rdmsr(ctr) & pc::MASCARA_CUENTA;
        wrmsr(ctl, 0);
        wrmsr(ctr, 0);
        if flags & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
        n
    };
    n
}

#[inline(never)]
fn recta(n: u64) {
    // SAFETY: solo registros.
    unsafe {
        asm!("2:", "add {x}, 1", "dec {n}", "jnz 2b", n = inout(reg) n => _, x = inout(reg) 0u64 => _, options(nomem, nostack));
    }
}

#[inline(never)]
fn azar(n: u64) {
    // SAFETY: solo registros.
    unsafe {
        asm!(
            "2:",
            "mov {t}, {x}", "shl {t}, 13", "xor {x}, {t}",
            "mov {t}, {x}", "shr {t}, 7", "xor {x}, {t}",
            "mov {t}, {x}", "shl {t}, 17", "xor {x}, {t}",
            "test {x}, 1",
            "jz 3f",
            "inc {c}",
            "3:",
            "dec {n}",
            "jnz 2b",
            n = inout(reg) n => _, x = inout(reg) 0x9E37_79B9_7F4A_7C15u64 => _, t = out(reg) _, c = inout(reg) 0u64 => _,
            options(nomem, nostack)
        );
    }
}

/// El bloque de la persecucion: una linea de 64 B por entrada, y en su
/// primera palabra la direccion de la siguiente.
#[repr(C, align(4096))]
struct Bloque([[u64; 8]; pc::LINEAS]);
static mut BLOQUE: Bloque = Bloque([[0; 8]; pc::LINEAS]);

#[inline(never)]
fn persecucion(n: u64) {
    // SAFETY: la calibracion va de una en una (`CALIBRADO`), y el bloque es
    // solo suyo; cada direccion apunta a una linea del mismo bloque.
    unsafe {
        let b = &mut *core::ptr::addr_of_mut!(BLOQUE);
        let mut siguiente = [0u32; pc::LINEAS];
        pc::permutacion(&mut siguiente);
        for (i, s) in siguiente.iter().enumerate() {
            b.0[i][0] = b.0[*s as usize].as_ptr() as u64;
        }
        let p = b.0[0].as_ptr() as u64;
        asm!("2:", "mov {p}, [{p}]", "dec {n}", "jnz 2b", p = inout(reg) p => _, n = inout(reg) n => _, options(readonly, nostack));
    }
}

static CALIBRADO: AtomicBool = AtomicBool::new(false);
static mut FILAS: [u64; pc::CALIBRACION.len()] = [0; pc::CALIBRACION.len()];

/// **La calibracion**: cada fila de `bmo_contadores::CALIBRACION`, contada y
/// juzgada. UNA vez por arranque; despues, lo recordado.
pub fn calibracion() -> Option<&'static [u64; pc::CALIBRACION.len()]> {
    if !se_pueden() {
        return None;
    }
    // SAFETY: las filas se escriben UNA vez, aqui, antes de dar la bandera;
    // despues solo se leen.
    let filas = unsafe { &mut *core::ptr::addr_of_mut!(FILAS) };
    if !CALIBRADO.swap(true, Ordering::AcqRel) {
        for (k, &(p, e)) in pc::CALIBRACION.iter().enumerate() {
            let n = pc::vueltas(p);
            let ev = &pc::EVENTOS[e];
            let contado = match p {
                Prueba::Recta => contar(ev, || recta(n)),
                Prueba::Azar => contar(ev, || azar(n)),
                Prueba::Persecucion => contar(ev, || persecucion(n)),
            };
            filas[k] = pc::empaquetar(p, e, contado, pc::juzgar(p, e, n, contado));
        }
        let buenas = filas.iter().filter(|&&w| pc::desempaquetar(w).3 == 1).count();
        crate::ring0::cabina::info("contadores", "la calibracion de los contadores del Zen 3: filas que cuentan lo que dice el PPR", buenas as u64);
    }
    Some(filas)
}
