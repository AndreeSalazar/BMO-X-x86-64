//! OBS-BUILD (17-10, GCB2a de PLAN_GSP_CONTROL_BMO): con que commit se
//! compilaron el kernel y este DIRECTOR, en cada informe. Sin esto un parte
//! del metal no dice que binario corrio, y dos sesiones no se pueden comparar.
//! [consumo] NADA
use bmo_userland as bmo;
use crate::scene::output::Output;

/// La palabra de este DIRECTOR, de `build.rs` (`obs_build.rs` del kernel).
pub(crate) const DIRECTOR: u64 = palabra(env!("BMO_OBS_BUILD").as_bytes());

const fn palabra(b: &[u8]) -> u64 {
    let mut v: u64 = 0;
    let mut i = 0;
    while i < b.len() {
        assert!(b[i].is_ascii_digit(), "BMO_OBS_BUILD no es un numero");
        v = v * 10 + (b[i] - b'0') as u64;
        i += 1;
    }
    v
}

/// La del kernel que corre; 0 si no la sabe (o es anterior a OBS-BUILD).
pub(crate) fn kernel() -> u64 {
    bmo::info(bmo::INFO_BUILD)
}

/// `533e9d7f4a5b33`, `533e9d7f4a5b33 +cambios sin commit` o `desconocido`.
pub(crate) fn escribir(s: &mut Output, w: u64) {
    if w & bmo::BUILD_VALIDO == 0 {
        s.text(b"desconocido");
        return;
    }
    s.hex(w & bmo::BUILD_COMMIT, 14);
    if w & bmo::BUILD_SUCIO != 0 {
        s.text(b" +cambios sin commit");
    }
}

/// Las dos, y si son el mismo commit. Un build SUCIO no queda identificado
/// del todo por su commit: se dice, no se calla.
pub(crate) fn lineas(s: &mut Output, sangria: &[u8]) {
    let k = kernel();
    s.text(sangria); s.text(b"build      kernel "); escribir(s, k);
    s.text(b"; director "); escribir(s, DIRECTOR); s.byte(b'\n');
    s.text(sangria); s.text(b"           ");
    let validos = k & DIRECTOR & bmo::BUILD_VALIDO != 0;
    s.text(if !validos {
        b"sin comparar: falta una de las dos" as &[u8]
    } else if k & bmo::BUILD_COMMIT != DIRECTOR & bmo::BUILD_COMMIT {
        b"DISTINTOS: kernel y DIRECTOR no salieron del mismo commit"
    } else if (k | DIRECTOR) & bmo::BUILD_SUCIO != 0 {
        b"mismo commit, con cambios sin commit: el commit no identifica todo"
    } else {
        b"mismo commit, sin cambios"
    });
    s.byte(b'\n');
}
