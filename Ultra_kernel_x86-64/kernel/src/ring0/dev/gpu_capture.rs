//! Diario de diagnostico NVIDIA; no escribe registros ni gobierna el arranque.
//! [carril] VERDE -- lecturas de registros de la 3060 ya autorizada
//! [consumo] NADA -- una foto por fase; RAM fija, sin reservar ni esperar
//! [cuesta] MAQUINA -- 32 fotos de 296 bytes, mas contadores; RAM fija por arranque
use super::gpu_prestamo::Bar0;
use bmo_gpu_ga10x::{falcon as fa, Registros};
use bmo_nvidia_gsp_control::capture::{self as c, Journal, Record};
use core::sync::atomic::{AtomicU64, Ordering};

static JOURNAL: Journal = Journal::new();
static SESSION_DATE: AtomicU64 = AtomicU64::new(0);
static SESSION_TSC: AtomicU64 = AtomicU64::new(0);
static FRTS_ATTEMPT: AtomicU64 = AtomicU64::new(0);

fn build() -> u64 {
    // Mismo dato OBS-BUILD del kernel; no llama de dev hacia core/report.
    env!("BMO_OBS_BUILD")
        .bytes()
        .fold(0, |a, b| a * 10 + (b - b'0') as u64)
}

pub(crate) fn before_frts(r: &mut Bar0) {
    // Solo el propietario de la orden FWSEC inicializa la sesion; nunca un
    // lector INFO. Identificador observacional RTC/TSC, no UUID criptografico.
    if SESSION_TSC.load(Ordering::Acquire) == 0 {
        SESSION_DATE.store(super::clock::ahora(), Ordering::Relaxed);
        SESSION_TSC.store(
            crate::ring0::task::scheduler::rdtsc().max(1),
            Ordering::Release,
        );
    }
    let attempt = super::gpu_despertar::info_intento() + 1;
    FRTS_ATTEMPT.store(attempt, Ordering::Release);
    record(r, attempt, c::BEFORE_FRTS);
}

pub(crate) fn after_frts(r: &mut Bar0) {
    record(r, FRTS_ATTEMPT.load(Ordering::Acquire), c::AFTER_FRTS);
}

pub(crate) fn record(r: &mut Bar0, attempt: u64, phase: u64) {
    if JOURNAL.has(attempt, phase)
        || (phase == c::HALTED || phase == c::WAIT_EXPIRED)
            && (JOURNAL.has(attempt, c::HALTED) || JOURNAL.has(attempt, c::WAIT_EXPIRED))
    {
        return;
    }
    let session = [
        SESSION_DATE.load(Ordering::Acquire),
        SESSION_TSC.load(Ordering::Acquire),
    ];
    let mut photo = Record::new(
        session,
        build(),
        attempt,
        phase,
        crate::ring0::task::scheduler::rdtsc(),
    );
    use bmo_gpu_ga10x::{aon, arranque::boot_probe as p};
    // Solo direcciones existentes; ninguna lectura de DMEM ni selector TRACE.
    let regs = [
        fa::GSP + fa::MAILBOX0,
        fa::GSP + fa::MAILBOX1,
        fa::GSP + fa::CPUCTL,
        fa::GSP + fa::BCR_CTRL,
        fa::SEC2 + fa::MAILBOX0,
        fa::SEC2 + fa::MAILBOX1,
        fa::SEC2 + fa::CPUCTL,
        fa::SEC2 + fa::BCR_CTRL,
        aon::WPR2_LO,
        aon::WPR2_HI,
        aon::BSI_14,
        aon::GFW_PROGRESO,
        fa::SEC2 + fa::HWCFG2,
        fa::GSP + fa::HWCFG2,
        fa::SEC2 + fa::DMATRFCMD,
        fa::SEC2 + fa::ENGINE,
        fa::GSP + fa::ENGINE,
        p::ECC_L2TLB,
        p::ECC_HUBTLB,
        p::ECC_FILLUNIT,
    ];
    for (i, reg) in regs.into_iter().enumerate() {
        let raw = r.leer(reg);
        photo.set(i, raw as u64, bmo_gpu_ga10x::es_error_pri(raw));
    }
    for i in 0..3 {
        if let Some(v) = super::gpu_libos::capture_log(i as u64 * 65536) {
            photo.set(20 + i, v, false);
        }
    }
    photo.set(23, super::gpu_prestamo::eventos(), false);
    photo.set(24, super::gpu_despertar::info_g0(), false);
    let raw = r.leer(bmo_gpu_ga10x::BOOT_0);
    photo.set(25, raw as u64, bmo_gpu_ga10x::es_error_pri(raw));
    photo.0[7] = crate::ring0::task::scheduler::rdtsc();
    JOURNAL.publish(photo);
}

/// Selectores de la INFO existente: 64 cabecera, 65 palabra inmutable.
/// Los campos ausentes no se convierten en cero medido: presencia en cada Record.
pub fn info(sel: u64) -> u64 {
    let (value, half) = match (sel >> 8) & 255 {
        c::SELECTOR => (
            match (sel >> 16) & 255 {
                0 => c::MAGIC,
                1 => JOURNAL.count(),
                2 => JOURNAL.lost(),
                3 => SESSION_DATE.load(Ordering::Acquire),
                4 => SESSION_TSC.load(Ordering::Acquire),
                _ => return 0,
            },
            (sel >> 32) & 1,
        ),
        x if x == c::SELECTOR + 1 => {
            let part = (sel >> 24) as usize;
            let Some(v) = JOURNAL.word(((sel >> 16) & 255) as usize, part / 2) else {
                return 0;
            };
            (v, (part & 1) as u64)
        }
        x if x == c::SELECTOR + 3 => {
            let part = sel >> 16;
            let Some(v) = super::gpu_libos::capture_log((part / 2) * 8) else {
                return 0;
            };
            (v, part & 1)
        }
        _ => return 0,
    };
    c::TAG | (value >> (half * 32)) & 0xFFFF_FFFF
}
