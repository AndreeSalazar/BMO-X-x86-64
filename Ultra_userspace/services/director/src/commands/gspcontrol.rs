//! GSP-CONTROL-BMO: resumen derivado de consultas existentes, sin ordenes nuevas.
//! [consumo] NADA
use bmo_nvidia_gsp_control::{diagnose, Identity, Snapshot, State};
use bmo_nvidia_gsp_control::booter::{self, CaptureLink, Evidence, Mailbox};
use bmo_userland as bmo;
use super::tabla::{campo, section};
use crate::scene::output::Output;

pub(crate) fn report(s: &mut Output) {
    // Dos lecturas delimitan una consulta; no es una captura atomica de la GPU.
    let attempt_before = bmo::info(bmo::INFO_GPU_INTENTO);
    let before = bmo::info(bmo::INFO_GPU_DESPIERTO);
    let chip = bmo::info(bmo::INFO_GPU_CHIP);
    let firmware = bmo::info(bmo::INFO_GPU_GSP);
    let hash = bmo::info(bmo::INFO_GPU_GSP_HASH);
    let libos = bmo::info(bmo::INFO_GPU_LIBOS);
    // El supervisor guarda finalizacion de inicializacion; el rotulo distingue
    // la observacion GMCAPI 615 del evento INIT_DONE 570.
    let es_615 = firmware & bmo::GSP_ES_615 != 0;
    let init_done = if es_615 { super::gsp615::listo().then_some(true) } else { super::gspinit::observado() };
    // Solo las caches existentes; no se piden nuevas lecturas del chip.
    let stop = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 5 << 8);
    let stop_mailboxes = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 6 << 8);
    let stop_controls = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 8 << 8);
    let after = bmo::info(bmo::INFO_GPU_DESPIERTO);
    // GCB2a: el commit del kernel y el numero del intento del booter.
    let build = super::obs_build::kernel();
    let attempt = bmo::info(bmo::INFO_GPU_INTENTO);
    let stable = before == after && attempt_before == attempt;
    let valid = after & bmo::DESPIERTO_VALIDO != 0;
    let reason = ((after >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF) as u32;
    let identity = Identity {
        device: (chip & bmo::GPU_HALLADA != 0).then_some(chip),
        firmware: (firmware & bmo::GSP_VALIDO != 0).then_some(hash),
        build: (build & bmo::BUILD_VALIDO != 0).then_some(build),
        attempt: (attempt != 0).then_some(attempt),
    };
    let state = diagnose(Snapshot {
        prepared: (libos & bmo::LIBOS_VALIDO != 0)
            .then_some(libos & bmo::LIBOS_COMPROBADO != 0),
        started: valid.then_some(after & bmo::DESPIERTO_GSP_ARRANCADO != 0),
        init_done,
        fault: (valid && reason != 0).then_some(reason),
        stable,
    });
    let halted = valid && after & bmo::DESPIERTO_SEC2_PARADO != 0;
    let code = (after >> bmo::DESPIERTO_BUZON_SHIFT) as u32;
    let completion = booter::diagnose(Evidence {
        stable,
        started: valid.then_some(after & bmo::DESPIERTO_SEC2_ARRANCADO != 0),
        halted: valid.then_some(halted),
        mailbox0: halted.then_some(if bmo_gpu_ga10x::es_error_pri(code) {
            Mailbox::Unreadable(code)
        } else { Mailbox::Readable(code) }),
        host_rejected: valid && reason == bmo::IOMMU_NO_BOOTER_MAL,
    });
    section(s, b"GSP-CONTROL-BMO -- observador, etapa 1");
    campo(s, b"estado");
    s.text(match state {
        Some(State::Unprepared) => b"SinPreparar" as &[u8],
        Some(State::Prepared) => b"Preparado",
        Some(State::Starting) => b"Arrancando",
        Some(State::Ready) if es_615 => b"Inicializacion observada: GSP_INIT GMCAPI respondido; motores y salud no demostrados",
        Some(State::Ready) => b"Operativo observado: INIT_DONE recibido; salud actual no demostrada",
        Some(State::Failed) => b"Fallido: hay motivo registrado; conservar autopsia",
        Some(State::RecoveryBlocked) => b"RecuperacionBloqueada",
        None => b"Desconocido: faltan datos, se contradicen o cambiaron",
    });
    s.byte(b'\n');
    campo(s, b"evidencia");
    s.text(b"LIBOS 0x"); s.hex(libos, 16);
    s.text(b"; despertar 0x"); s.hex(before, 16);
    s.text(b" -> 0x"); s.hex(after, 16); s.byte(b'\n');
    campo(s, if es_615 { b"gmc_init" } else { b"init_done" });
    s.text(match init_done { Some(true) => b"recibido" as &[u8],
        Some(false) => b"no recibido en el intento local", None => b"sin observacion local" });
    s.text(b"; motivo 0x"); s.hex(reason as u64, 2);
    if halted {
        s.text(b"; SEC2 MAILBOX0 0x"); s.hex(after >> bmo::DESPIERTO_BUZON_SHIFT, 8);
    } else {
        s.text(b"; SEC2 sin parada observada: retorno final no disponible");
    }
    s.byte(b'\n');
    campo(s, b"retorno booter");
    s.text(completion.explanation()); s.byte(b'\n');
    // La foto no tiene sello de intento. Despues del primero no la unimos
    // al MAILBOX0 vivo, aunque el numero y el ultimo motivo sean iguales.
    campo(s, b"foto parada");
    let link = booter::link_capture(stable, identity.attempt, stop >> 63 != 0);
    s.text(match link {
        CaptureLink::Changing => b"consulta cambio; no atribuir la foto" as &[u8],
        CaptureLink::Absent => b"sin primera foto de parada; cero ausente no es un registro medido",
        CaptureLink::ThisAttempt => b"primera foto observada; atribuible al intento 1",
        CaptureLink::Unattributed => b"primera foto del arranque SIN sello de intento; no atribuir al intento actual",
    });
    if link == CaptureLink::ThisAttempt {
        s.text(b"; intervalo CPU observado "); s.dec((stop >> 32) & 0x7FFF_FFFF);
        s.text(b" us desde marca previa a STARTCPU hasta observar parada; incluye preparacion/sondeo");
        s.text(b"; GSP MAILBOX0/1 0x"); s.hex(stop_mailboxes as u32 as u64, 8);
        s.text(b"/0x"); s.hex(stop_mailboxes >> 32, 8);
        s.text(b"; CPUCTL GSP/SEC2 0x"); s.hex(stop_controls as u32 as u64, 8);
        s.text(b"/0x"); s.hex(stop_controls >> 32, 8);
        if bmo_gpu_ga10x::es_error_pri(stop_mailboxes as u32)
            || bmo_gpu_ga10x::es_error_pri((stop_mailboxes >> 32) as u32) {
            s.text(b"; buzon GSP con error PRI: crudo de lectura, no respuesta del firmware");
        }
    }
    s.byte(b'\n');
    campo(s, b"identidad");
    s.text(b"tarjeta "); optional(s, identity.device);
    s.text(b"; firmware-huella "); optional(s, identity.firmware);
    s.text(b"; build "); super::obs_build::escribir(s, build);
    s.text(b"; intento ");
    match identity.attempt {
        Some(n) => { s.dec(n); s.text(b" desde el arranque"); }
        None => s.text(b"ninguno: el booter no se solto en este arranque"),
    }
    s.byte(b'\n');
    super::g0::filas(s);
    campo(s, b"limite");
    s.text(if identity.complete() {
        b"identidad completa; foto derivada, sin historia de eventos ni permiso para recuperar.\n" as &[u8]
    } else {
        b"identidad incompleta; foto derivada, sin historia ni permiso para recuperar.\n"
    });
}

fn optional(s: &mut Output, value: Option<u64>) {
    if let Some(v) = value { s.text(b"0x"); s.hex(v, 16); }
    else { s.text(b"desconocido"); }
}
