//! **`3d`: el modo 3D, para todo lo que suena (S7).**
//!
//! [consumo] NADA      no corre en reposo: solo cuando se teclea. Lo aplica
//!                     el maestro del kernel en cada trama (L6h)
//!
//! El propietario (03-10): *"se puede aplicar global si ponemos un Modo 3D,
//! no? pero PRO, EPICO [...] y 4D?"*. Global si: lo hace el maestro, antes
//! del oido, a DOOM, a la musica de fondo y a los avisos por igual.
//!
//! ```text
//!    3d                 como esta
//!    3d cerca           dos altavoces virtuales delante (+-30 grados)
//!    3d sala            y una habitacion: el sonido sale de la cabeza
//!    3d amplio          los altavoces a +-60 grados, con sala
//!    3d orbita [N]      el "4D": la escena gira, una vuelta cada N s (8)
//!    3d apagado         el estereo tal cual
//! ```

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

const MODOS: [(&[u8], &[u8]); 5] = [
    (b"apagado", b"el estereo tal cual, dentro de la cabeza"),
    (b"cerca", b"dos altavoces virtuales delante, a +-30 grados"),
    (b"sala", b"y una habitacion: ocho reflejos, el sonido sale FUERA"),
    (b"amplio", b"los altavoces a +-60 grados, con sala"),
    (b"orbita", b"el \"4D\": la escena entera gira alrededor de la cabeza"),
];

fn numero(b: &[u8]) -> Option<i64> {
    let b = b.trim_ascii();
    if b.is_empty() || b.len() > 2 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0i64, |n, &c| n * 10 + (c - b'0') as i64))
}

pub(crate) fn espacio(dsk: &mut Desktop, _p: &bmo::Pantalla, arg: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    let arg = arg.trim_ascii();
    let (palabra, resto) = match arg.iter().position(|&c| c == b' ') {
        Some(i) => (&arg[..i], &arg[i + 1..]),
        None => (arg, &b""[..]),
    };
    if !palabra.is_empty() {
        let Some(modo) = MODOS.iter().position(|(n, _)| *n == palabra) else {
            s.with_ink(INK_ERR);
            s.text(b"  3d [apagado|cerca|sala|amplio|orbita [segundos]]\n");
            s.with_ink(INK_PLAIN);
            return After::Settle;
        };
        let mut ok = bmo::audio_mando(bmo::AUDIO_MANDO_3D, modo as i64).is_some();
        if let Some(n) = numero(resto) {
            ok &= bmo::audio_mando(bmo::AUDIO_MANDO_3D_VUELTA, n).is_some();
        }
        if !ok {
            s.with_ink(INK_ERR);
            s.text(b"  el kernel no lo acepto: solo lo mueve el escritorio (mira `cabina`)\n");
            s.with_ink(INK_PLAIN);
            return After::Settle;
        }
    }
    let e = bmo::info(bmo::INFO_AUDIO_ESPACIO);
    let modo = (e & 0xFF) as usize;
    s.with_ink(INK_GOOD);
    s.text(b"  el 3D, para todo lo que suena: ");
    s.text(MODOS.get(modo).map(|m| m.0).unwrap_or(b"?"));
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    for (i, (n, que)) in MODOS.iter().enumerate() {
        s.text(if i == modo { b"  > " } else { b"    " });
        s.text(n);
        for _ in n.len()..9 {
            s.byte(b' ');
        }
        s.text(que);
        if i == 4 {
            s.text(b" (una vuelta cada ");
            s.dec((e >> 8) & 0xFF);
            s.text(b" s)");
        }
        s.byte(b'\n');
    }
    if (e >> 48) & 1 == 0 {
        s.with_ink(INK_ERR);
        s.text(b"  [!] el tubo no va a 44,1 ni 48 kHz (o no hay tubo): el 3D no se aplica\n");
        s.with_ink(INK_PLAIN);
    }
    s.text(b"  arriba/abajo y un detras preciso piden HRTF, que no esta (PLAN_EL_SONIDO S7.3)\n");
    After::Settle
}
