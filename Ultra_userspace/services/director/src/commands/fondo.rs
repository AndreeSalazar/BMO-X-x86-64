//! **`fondo`: la musica de fondo, desde Ejecutar.**
//!
//! [consumo] NADA      no corre en reposo: solo cuando se teclea. Lo que
//!                     suena despues lo cuenta `desktop::musica` (L6h)
//!
//! ```text
//!    fondo               la enciende (o dice que suena)
//!    fondo <pieza>       esa pieza, por su nombre ("sierra al atardecer")
//!    fondo siguiente     la siguiente de las tranquilas
//!    fondo vol N         el volumen, 0..100
//!    fondo aviso [que]   un aviso encima, en su sitio (mensaje, conecta,
//!                        zumbido, juez, captura, arranque, error,
//!                        advertencia, pregunta, hecho, llega, seva): para
//!                        OIR como se agacha la musica y de donde viene
//!    fondo lista         las diez piezas
//!    fondo apagar        la apaga y suelta el banco y el tubo
//! ```

use bmo_fondo::{buscar, Aviso, PIEZAS, TRANQUILAS};
use bmo_userland as bmo;

use super::After;
use crate::desktop::musica;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

fn numero(b: &[u8]) -> Option<u32> {
    let b = b.trim_ascii();
    if b.is_empty() || b.len() > 3 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0u32, |n, &c| n * 10 + (c - b'0') as u32))
}

fn aviso_de(b: &[u8]) -> Option<Aviso> {
    Some(match b {
        b"" | b"mensaje" => Aviso::Mensaje,
        b"conecta" => Aviso::Conecta,
        b"zumbido" => Aviso::Zumbido,
        b"juez" => Aviso::Juez,
        b"captura" => Aviso::Captura,
        b"arranque" => Aviso::Arranque,
        b"error" => Aviso::Error,
        b"advertencia" => Aviso::Advertencia,
        b"pregunta" => Aviso::Pregunta,
        b"hecho" => Aviso::Hecho,
        b"llega" => Aviso::Llega,
        b"seva" | b"se-va" => Aviso::SeVa,
        _ => return None,
    })
}

pub(crate) fn fondo(dsk: &mut Desktop, _p: &bmo::Pantalla, arg: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    let arg = arg.trim_ascii();
    let hecho = match arg {
        b"lista" => {
            for (i, p) in PIEZAS.iter().enumerate() {
                s.text(if TRANQUILAS.contains(&i) { b"  * " } else { b"    " });
                s.text(p.nombre.as_bytes());
                s.text(b"  (");
                s.dec(p.bpm as u64);
                s.text(b" pulsos)\n");
            }
            s.text(b"  * = de las tranquilas, las que suenan de fondo por turno\n");
            return After::Settle;
        }
        b"apagar" | b"para" | b"off" => {
            musica::apagar();
            s.text(b"  la musica de fondo, apagada: banco y tubo devueltos\n");
            return After::Settle;
        }
        b"siguiente" | b"sig" => musica::siguiente(),
        b"" => match musica::que_suena() {
            Some(_) => Ok(()),
            None => musica::siguiente(),
        },
        _ => {
            if let Some(n) = arg.strip_prefix(b"vol").map(|r| r.strip_prefix(b"umen").unwrap_or(r)).and_then(numero) {
                musica::volumen(n);
                s.text(b"  volumen de fondo ");
                s.dec(n.min(100) as u64);
                s.text(b"\n");
                return After::Settle;
            }
            if let Some(que) = arg.strip_prefix(b"aviso") {
                let Some(a) = aviso_de(que.trim_ascii()) else {
                    s.with_ink(INK_ERR);
                    s.text(b"  avisos: mensaje, conecta, zumbido, juez, captura, arranque,\n");
                    s.text(b"          error, advertencia, pregunta, hecho, llega, seva (cada uno en su sitio, en 3D)\n");
                    s.with_ink(INK_PLAIN);
                    return After::Settle;
                };
                if musica::que_suena().is_none() {
                    s.text(b"  los avisos suenan con el fondo encendido: `fondo` primero\n");
                    return After::Settle;
                }
                musica::avisar(a);
                s.text(b"  aviso encima: la musica baja 15 dB en 30 ms y vuelve sola\n");
                return After::Settle;
            }
            match buscar(arg) {
                Some(i) => musica::tocar(i),
                None => {
                    s.with_ink(INK_ERR);
                    s.text(b"  no hay una pieza con ese nombre: `fondo lista`\n");
                    s.with_ink(INK_PLAIN);
                    return After::Settle;
                }
            }
        }
    };
    match (hecho, musica::que_suena()) {
        (Ok(()), Some((i, vol, ms))) => {
            s.with_ink(INK_GOOD);
            s.text(b"  de fondo: ");
            s.text(PIEZAS[i].nombre.as_bytes());
            s.with_ink(INK_PLAIN);
            s.text(b"  (volumen ");
            s.dec(vol as u64);
            s.text(b", compuesta aqui en ");
            s.dec(ms);
            s.text(b" ms)\n  se agacha sola bajo un aviso o un juego. `fondo siguiente`, `fondo vol N`, `fondo apagar`\n");
        }
        (Ok(()), None) => s.text(b"  no suena nada\n"),
        (Err(f), _) => {
            s.with_ink(INK_ERR);
            s.text(b"  ");
            s.text(f.texto());
            s.text(b"\n");
            s.with_ink(INK_PLAIN);
        }
    }
    After::Settle
}
