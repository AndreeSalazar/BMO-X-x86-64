//! GSP-CONTROL-BMO: resumen derivado de consultas existentes, sin ordenes nuevas.
//! [consumo] NADA
use bmo_nvidia_gsp_control::{diagnose, Identity, Snapshot, State};
use bmo_userland as bmo;
use super::tabla::{campo, section};
use crate::scene::output::Output;

pub(crate) fn report(s: &mut Output) {
    // Dos lecturas delimitan una consulta; no es una captura atomica de la GPU.
    let before = bmo::info(bmo::INFO_GPU_DESPIERTO);
    let chip = bmo::info(bmo::INFO_GPU_CHIP);
    let firmware = bmo::info(bmo::INFO_GPU_GSP);
    let hash = bmo::info(bmo::INFO_GPU_GSP_HASH);
    let libos = bmo::info(bmo::INFO_GPU_LIBOS);
    let init_done = super::gspinit::observado();
    let after = bmo::info(bmo::INFO_GPU_DESPIERTO);
    // GCB2a: el commit del kernel y el numero del intento del booter.
    let build = super::obs_build::kernel();
    let attempt = bmo::info(bmo::INFO_GPU_INTENTO);
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
        stable: before == after,
    });
    section(s, b"GSP-CONTROL-BMO -- observador, etapa 1");
    campo(s, b"estado");
    s.text(match state {
        Some(State::Unprepared) => b"SinPreparar" as &[u8],
        Some(State::Prepared) => b"Preparado",
        Some(State::Starting) => b"Arrancando",
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
    campo(s, b"init_done");
    s.text(match init_done { Some(true) => b"recibido" as &[u8],
        Some(false) => b"no recibido en el intento local", None => b"sin observacion local" });
    s.text(b"; motivo 0x"); s.hex(reason as u64, 2);
    s.text(b"; SEC2 MAILBOX0 0x"); s.hex(after >> bmo::DESPIERTO_BUZON_SHIFT, 8);
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
