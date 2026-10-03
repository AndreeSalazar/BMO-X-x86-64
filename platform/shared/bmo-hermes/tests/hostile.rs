//! **HOSTILE HERMES** -- the other end of a HERMES conversation is another
//! machine, and every byte it sends reaches the PUERTA. Garbage and mutations
//! of good messages through the four readers: the message reader, the frame
//! splitter, the handshake and the open conversation.
//!
//! Checked: nothing panics, and nothing garbled opens. See `bmo-hostile` for
//! what is not checked.

use bmo_hermes::marco::{enmarcar, Lector};
use bmo_hermes::noise::{Papel, Patron, Saludo};
use bmo_hermes::trama::{escribir, leer, Mensaje, Que};
use bmo_hostile::{attack, DEFAULT_SEED};

const CASES: u32 = 20_000;

fn good_messages() -> Vec<Vec<u8>> {
    let suma = [0x11u8; 32];
    let datos = [0x22u8; 300];
    let ms = [
        Mensaje::Texto("hola nova, DOOM II esta noche?"),
        Mensaje::Zumbido,
        Mensaje::Guino(2),
        Mensaje::Oferta { id: 9, bytes: 4_404_019, suma: &suma, nombre: "sierra.jpg" },
        Mensaje::Si(9),
        Mensaje::No(9),
        Mensaje::Trozo { id: 9, numero: 3, datos: &datos },
        Mensaje::Pide { que: Que::Pagina, nombre: "inicio" },
        Mensaje::Reaccion { tuyo: true, numero: 4, emoji: "\u{1F44D}\u{1F3FD}" },
    ];
    let mut b = [0u8; 1024];
    ms.iter()
        .map(|m| {
            let n = escribir(m, &mut b).unwrap();
            b[..n].to_vec()
        })
        .collect()
}

#[test]
fn a_message_never_panics_and_what_it_accepts_writes_back_the_same() {
    let good = good_messages();
    let samples: Vec<&[u8]> = good.iter().map(|v| v.as_slice()).collect();
    attack("trama::leer", DEFAULT_SEED, CASES, &samples, 700, |b| {
        if let Ok(m) = leer(b) {
            // Whatever is accepted must be exactly re-writable: no two byte
            // strings mean the same message.
            let mut out = vec![0u8; b.len() + 8];
            let n = escribir(&m, &mut out).expect("accepted but not writable");
            assert_eq!(&out[..n], b, "accepted bytes do not round-trip");
        }
    });
}

#[test]
fn the_frame_splitter_never_panics() {
    let good = good_messages();
    let mut stream = Vec::new();
    let mut b = [0u8; 1100];
    for m in &good {
        let n = enmarcar(m, &mut b).unwrap();
        stream.extend_from_slice(&b[..n]);
    }
    attack("marco::Lector", DEFAULT_SEED ^ 1, CASES, &[&stream], 4000, |bytes| {
        let mut l = Box::new(Lector::nuevo());
        let mut d = vec![0u8; 65_535];
        let mut resto = bytes;
        loop {
            let n = l.meter(resto);
            resto = &resto[n..];
            match l.siguiente(&mut d) {
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }
    });
}

/// The responder side of both handshakes, fed garbage and mutated first
/// messages. A first message that is not exactly right must never be accepted
/// past the first DH: with a key, the tag stops it.
#[test]
fn the_handshake_never_panics_on_a_hostile_first_message() {
    let ini_s = [0x31u8; 32];
    let res_s = [0x52u8; 32];
    let res_pub = bmo_cripto::x25519::secreto_a_publico(&res_s);
    for patron in [Patron::XX, Patron::IK] {
        let remota = if patron == Patron::IK { Some(&res_pub) } else { None };
        let mut ini = Saludo::nuevo(patron, Papel::Inicia, b"HERMES/1", &ini_s, &[0x77u8; 32], remota).unwrap();
        let mut good = [0u8; 256];
        let n = ini.escribir(&[], &mut good).unwrap();
        let good = good[..n].to_vec();
        attack("noise::Saludo::leer", DEFAULT_SEED ^ 2, CASES / 4, &[&good], 300, |b| {
            let mut res = Saludo::nuevo(patron, Papel::Responde, b"HERMES/1", &res_s, &[0x99u8; 32], None).unwrap();
            let mut out = [0u8; 400];
            let r = res.leer(b, &mut out);
            if patron == Patron::IK && r.is_ok() {
                assert_eq!(b, &good[..], "IK accepted a first message that is not the real one");
            }
        });
    }
}

#[test]
fn an_open_conversation_never_opens_garbage() {
    let ini_s = [0x31u8; 32];
    let res_s = [0x52u8; 32];
    let res_pub = bmo_cripto::x25519::secreto_a_publico(&res_s);
    let mut ini = Saludo::nuevo(Patron::IK, Papel::Inicia, b"HERMES/1", &ini_s, &[0x77u8; 32], Some(&res_pub)).unwrap();
    let mut res = Saludo::nuevo(Patron::IK, Papel::Responde, b"HERMES/1", &res_s, &[0x99u8; 32], None).unwrap();
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    let n = ini.escribir(&[], &mut b).unwrap();
    res.leer(&b[..n], &mut f).unwrap();
    let n = res.escribir(&[], &mut b).unwrap();
    ini.leer(&b[..n], &mut f).unwrap();
    let mut ci = ini.partir().unwrap();
    let cr = res.partir().unwrap();
    let mut sellado = [0u8; 600];
    let n = ci.sellar(&good_messages()[0], &mut sellado).unwrap();
    let good = sellado[..n].to_vec();
    attack("noise::Canal::abrir", DEFAULT_SEED ^ 3, CASES, &[&good], 700, |bytes| {
        let mut c = cr.clone();
        let mut out = vec![0u8; bytes.len().max(1)];
        if c.abrir(bytes, &mut out).is_ok() {
            assert_eq!(bytes, &good[..], "a conversation opened bytes that were never sealed");
        }
    });
}
