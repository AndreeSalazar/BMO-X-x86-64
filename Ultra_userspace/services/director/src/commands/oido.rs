//! **`oido`: el perfil de quien escucha, para todo lo que suena.**
//!
//! [consumo] NADA      no corre en reposo: solo cuando se teclea. Lo aplica
//!                     el maestro del kernel en cada trama (L6h)
//!
//! El propietario (03-10): *"mejorar el audifono, que se aplique en general,
//! y control"*. Lo aplica la ultima etapa, la que pasa TODO -- DOOM, la
//! musica de fondo, los avisos -- antes de la ganancia y del limite, asi que
//! lo que el tono suba tambien lo sujeta el limite.
//!
//! ```text
//!    oido                  como esta
//!    oido voz              para entender voces: agudos +6, medios +3, graves -2
//!    oido musica           graves +3, agudos +3
//!    oido plano            todo a cero: el maestro vuelve a ser un cable
//!    oido agudos N         -12..12 dB (y graves, medios igual)
//!    oido balance N        -100 solo izquierda .. +100 solo derecha
//!    oido mono si|no       los dos lados sumados
//!    oido izq | oido der   LA PRUEBA DE LOS LADOS: un aviso SOLO por ese
//!                          lado (03-10, "escucho solo por la derecha")
//! ```

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

fn numero(b: &[u8]) -> Option<i64> {
    let b = b.trim_ascii();
    let (signo, cifras) = match b.first() {
        Some(b'-') => (-1, &b[1..]),
        Some(b'+') => (1, &b[1..]),
        _ => (1, b),
    };
    if cifras.is_empty() || cifras.len() > 3 || !cifras.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(signo * cifras.iter().fold(0i64, |n, &c| n * 10 + (c - b'0') as i64))
}

fn mando(que: u64, valor: i64) -> bool {
    bmo::audio_mando(que, valor).is_some()
}

pub(crate) fn oido(dsk: &mut Desktop, _p: &bmo::Pantalla, arg: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    let arg = arg.trim_ascii();
    let (palabra, resto) = match arg.iter().position(|&c| c == b' ') {
        Some(i) => (&arg[..i], &arg[i + 1..]),
        None => (arg, &b""[..]),
    };
    // ** LA PRUEBA DE LOS LADOS: un aviso solo por un lado, y lo que dice.
    let lado = match palabra {
        b"izq" | b"izquierda" | b"izquierdo" => Some((256u16, 0u16, &b"IZQUIERDA"[..])),
        b"der" | b"derecha" | b"derecho" => Some((0, 256, &b"DERECHA"[..])),
        _ => None,
    };
    if let Some((i, d, nombre)) = lado {
        match crate::desktop::musica::probar_lado(i, d) {
            Ok(()) => {
                s.with_ink(INK_GOOD);
                s.text(b"  suena un aviso SOLO por la ");
                s.text(nombre);
                s.text(b"\n");
                s.with_ink(INK_PLAIN);
                s.text(b"  si lo oyes por ese lado, BMO-X manda bien los dos lados.\n");
                s.text(b"  si NO se oye por ese lado: no es la mezcla, es el aparato, el cable o el\n");
                s.text(b"  conector (un auricular con microfono, de 4 polos, en un enchufe de 3 da un\n");
                s.text(b"  lado solo). `oido` dice que traia cada lado del aparato.\n");
                let o = bmo::info(bmo::INFO_AUDIO_OIDO);
                if (o >> 8) & 1 == 1 || o & 0xFF != 0 {
                    s.with_ink(INK_ERR);
                    s.text(b"  [!] el oido tiene MONO o BALANCE puesto: la prueba no es limpia (`oido plano`)\n");
                    s.with_ink(INK_PLAIN);
                }
            }
            Err(f) => {
                s.with_ink(INK_ERR);
                s.text(b"  ");
                s.text(f.texto());
                s.text(b"\n");
                s.with_ink(INK_PLAIN);
            }
        }
        return After::Settle;
    }
    let ok = match palabra {
        b"" => true,
        b"plano" => mando(bmo::AUDIO_MANDO_PLANO, 0),
        b"voz" => mando(bmo::AUDIO_MANDO_GRAVES, -2) && mando(bmo::AUDIO_MANDO_MEDIOS, 3) && mando(bmo::AUDIO_MANDO_AGUDOS, 6),
        b"musica" => mando(bmo::AUDIO_MANDO_GRAVES, 3) && mando(bmo::AUDIO_MANDO_MEDIOS, 0) && mando(bmo::AUDIO_MANDO_AGUDOS, 3),
        b"mono" => match resto.trim_ascii() {
            b"si" | b"1" | b"" => mando(bmo::AUDIO_MANDO_MONO, 1),
            b"no" | b"0" => mando(bmo::AUDIO_MANDO_MONO, 0),
            _ => false,
        },
        b"graves" | b"medios" | b"agudos" | b"balance" => {
            let que = match palabra {
                b"graves" => bmo::AUDIO_MANDO_GRAVES,
                b"medios" => bmo::AUDIO_MANDO_MEDIOS,
                b"agudos" => bmo::AUDIO_MANDO_AGUDOS,
                _ => bmo::AUDIO_MANDO_BALANCE,
            };
            match numero(resto) {
                Some(n) => mando(que, n),
                None => false,
            }
        }
        _ => false,
    };
    if !ok {
        s.with_ink(INK_ERR);
        s.text(b"  oido [voz|musica|plano] | graves|medios|agudos N (-12..12) | balance N (-100..100) | mono si|no | izq | der\n");
        s.text(b"  (si la orden era buena: solo lo puede mover el escritorio; mira `cabina`)\n");
        s.with_ink(INK_PLAIN);
        return After::Settle;
    }
    let o = bmo::info(bmo::INFO_AUDIO_OIDO);
    let b = |d: u32| ((o >> d) & 0xFF) as u8 as i8 as i64;
    let con_signo = |s: &mut crate::scene::output::Output, v: i64| {
        s.text(if v < 0 { b"-" } else if v > 0 { b"+" } else { b" " });
        s.dec(v.unsigned_abs());
    };
    s.with_ink(INK_GOOD);
    s.text(b"  el OIDO, para todo lo que suena\n");
    s.with_ink(INK_PLAIN);
    for (nombre, d, nota) in [
        (&b"  graves  "[..], 16u32, &b" dB  (100 Hz)"[..]),
        (b"  medios  ", 24, b" dB  (1 kHz)"),
        (b"  agudos  ", 32, b" dB  (3,5 kHz: la voz)"),
    ] {
        s.text(nombre);
        con_signo(s, b(d));
        s.text(nota);
        s.byte(b'\n');
    }
    s.text(b"  balance ");
    con_signo(s, b(0));
    s.text(b"  (-100 izquierda .. +100 derecha)\n  mono    ");
    s.text(if (o >> 8) & 1 == 1 { b"si\n" } else { b"no\n" });
    // ** Y LOS LADOS DEL APARATO (03-10): un solo lado puede no ser el oido
    // sino el aparato, que traia un canal al minimo o callado.
    let l = bmo::info(bmo::INFO_AUDIO_LADOS);
    if l >> 48 != 0 {
        let dbs = |v: u64, s: &mut crate::scene::output::Output| {
            let v = (v & 0xFFFF) as u16 as i16 as i64;
            con_signo(s, v / 256);
            s.text(b" dB");
        };
        s.text(b"  el aparato traia: izquierdo ");
        dbs(l, s);
        s.text(b", derecho ");
        dbs(l >> 16, s);
        if (l >> 32) & 3 != 0 {
            s.text(if (l >> 32) & 3 == 2 { b" (el DERECHO venia callado)" } else if (l >> 32) & 3 == 1 { b" (el IZQUIERDO venia callado)" } else { b" (los dos venian callados)" });
        }
        s.text(b"  -> igualados al reclamarlo\n");
        let otras = (l >> 36) & 0xF;
        if otras > 0 {
            s.text(b"  y ");
            s.dec(otras);
            s.text(b" unidad(es) de volumen mas en el camino del aparato, abiertas a 0 dB\n");
        }
    }
    if (o >> 48) & 1 == 0 {
        s.with_ink(INK_ERR);
        s.text(b"  [!] el tubo no va a 44,1 ni 48 kHz (o no hay tubo): el tono no se aplica; mono y balance si\n");
        s.with_ink(INK_PLAIN);
    }
    After::Settle
}
