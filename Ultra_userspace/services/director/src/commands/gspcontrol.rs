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
    let valid = after & bmo::DESPIERTO_VALIDO != 0;
    let reason = ((after >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF) as u32;
    let identity = Identity {
        device: (chip & bmo::GPU_HALLADA != 0).then_some(chip),
        firmware: (firmware & bmo::GSP_VALIDO != 0).then_some(hash),
        build: None,
        attempt: None,
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
    s.text(b"; build e intento: NO DISPONIBLES\n");
    campo(s, b"limite");
    s.text(b"foto derivada; sin historia ligada ni permiso para recuperar. Siguiente: protocolo G0-MODO.\n");
}

fn optional(s: &mut Output, value: Option<u64>) {
    if let Some(v) = value { s.text(b"0x"); s.hex(v, 16); }
    else { s.text(b"desconocido"); }
}
