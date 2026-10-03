//! Las pruebas del saludo: primero contra los vectores de fuera, despues
//! contra lo que haria un vecino con mala idea.

use super::*;
use crate::vectores::{Vector, IK, XX};

extern crate alloc;
use alloc::vec::Vec;

fn de_hex(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    (0..b.len() / 2)
        .map(|i| {
            let hi = (b[i * 2] as char).to_digit(16).unwrap() as u8;
            let lo = (b[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
            (hi << 4) | lo
        })
        .collect()
}

fn clave(s: &str) -> [u8; CLAVE] {
    let v = de_hex(s);
    let mut k = [0u8; CLAVE];
    k.copy_from_slice(&v);
    k
}

fn patron_de(v: &Vector) -> Patron {
    if v.nombre.contains("_IK_") { Patron::IK } else { Patron::XX }
}

/// Las dos puntas del vector, recien creadas.
fn puntas(v: &Vector) -> (Saludo, Saludo) {
    let p = patron_de(v);
    let prologo = de_hex(v.prologo);
    let remota = v.ini_remota.map(clave);
    let ini = Saludo::nuevo(p, Papel::Inicia, &prologo, &clave(v.ini_estatica), &clave(v.ini_efimera), remota.as_ref()).unwrap();
    let res = Saludo::nuevo(p, Papel::Responde, &prologo, &clave(v.res_estatica), &clave(v.res_efimera), None).unwrap();
    (ini, res)
}

/// **Un vector entero**: cada mensaje que escribe una punta tiene que ser,
/// byte a byte, el del vector, y la otra tiene que sacar la misma carga.
fn correr(v: &Vector) {
    let (mut ini, mut res) = puntas(v);
    let pasos = patron_de(v).pasos();
    let mut buf = [0u8; 1024];
    let mut fuera = [0u8; 1024];
    let mut canales: Option<(Canal, Canal)> = None;
    for (i, (carga, cifrado)) in v.mensajes.iter().enumerate() {
        let carga = de_hex(carga);
        let esperado = de_hex(cifrado);
        let habla_ini = i % 2 == 0;
        if i < pasos {
            let (a, b) = if habla_ini { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
            let n = a.escribir(&carga, &mut buf).unwrap();
            assert_eq!(&buf[..n], &esperado[..], "{} mensaje {}: el cifrado no es el del vector", v.nombre, i);
            let m = b.leer(&buf[..n], &mut fuera).unwrap();
            assert_eq!(&fuera[..m], &carga[..], "{} mensaje {}: la carga no vuelve", v.nombre, i);
            if i + 1 == pasos {
                assert_eq!(ini.hash().to_vec(), de_hex(v.hash_saludo), "{}: handshake_hash", v.nombre);
                assert_eq!(res.hash(), ini.hash());
                canales = Some((ini.clone().partir().unwrap(), res.clone().partir().unwrap()));
            }
        } else {
            let (ci, cr) = canales.as_mut().unwrap();
            let (a, b) = if habla_ini { (ci, cr) } else { (cr, ci) };
            let n = a.sellar(&carga, &mut buf).unwrap();
            assert_eq!(&buf[..n], &esperado[..], "{} mensaje {}: el transporte no es el del vector", v.nombre, i);
            let m = b.abrir(&buf[..n], &mut fuera).unwrap();
            assert_eq!(&fuera[..m], &carga[..]);
        }
    }
}

#[test]
fn el_vector_xx_byte_a_byte() {
    correr(&XX);
}

#[test]
fn el_vector_ik_byte_a_byte() {
    correr(&IK);
}

#[test]
fn cada_punta_sabe_la_clave_fija_de_la_otra() {
    let (mut ini, mut res) = puntas(&XX);
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    for i in 0..3 {
        let (a, o) = if i % 2 == 0 { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
        let n = a.escribir(&[], &mut b).unwrap();
        o.leer(&b[..n], &mut f).unwrap();
    }
    assert_eq!(ini.remota(), Some(x25519::secreto_a_publico(&clave(XX.res_estatica))));
    assert_eq!(res.remota(), Some(x25519::secreto_a_publico(&clave(XX.ini_estatica))));
}

#[test]
fn la_medida_vacia_es_la_de_verdad() {
    for v in [&XX, &IK] {
        let p = patron_de(v);
        let (mut ini, mut res) = puntas(v);
        let mut b = [0u8; 256];
        let mut f = [0u8; 256];
        for i in 0..p.pasos() {
            let (a, o) = if i % 2 == 0 { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
            let n = a.escribir(&[], &mut b).unwrap();
            assert_eq!(Some(n), p.medida_vacia(i), "{} paso {}", v.nombre, i);
            o.leer(&b[..n], &mut f).unwrap();
        }
        assert_eq!(p.medida_vacia(p.pasos()), None);
    }
    assert_eq!(Patron::XX.medida_vacia(0), Some(32));
    assert_eq!(Patron::XX.medida_vacia(1), Some(96));
    assert_eq!(Patron::XX.medida_vacia(2), Some(64));
    assert_eq!(Patron::IK.medida_vacia(0), Some(96));
    assert_eq!(Patron::IK.medida_vacia(1), Some(48));
}

/// **El vecino de la LAN se pone en medio.** Hace un XX con cada punta, con
/// SU clave fija. Los dos saludos terminan -- Noise no puede saber quien es
/// amigo -- pero cada punta ve una clave fija que no es la que espera, y eso
/// es lo que mira la lista de amigos. Y un IK contra el impostor no termina.
#[test]
fn el_que_se_pone_en_medio_no_es_el_amigo() {
    let ini_s = clave(XX.ini_estatica);
    let res_s = clave(XX.res_estatica);
    let malo_s = [0x42u8; 32];
    let malo_pub = x25519::secreto_a_publico(&malo_s);
    // El que inicia cree hablar con el que responde, y habla con el malo.
    let mut ini = Saludo::nuevo(Patron::XX, Papel::Inicia, b"", &ini_s, &[7u8; 32], None).unwrap();
    let mut malo = Saludo::nuevo(Patron::XX, Papel::Responde, b"", &malo_s, &[9u8; 32], None).unwrap();
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    for i in 0..3 {
        let (a, o) = if i % 2 == 0 { (&mut ini, &mut malo) } else { (&mut malo, &mut ini) };
        let n = a.escribir(&[], &mut b).unwrap();
        o.leer(&b[..n], &mut f).unwrap();
    }
    assert_eq!(ini.remota(), Some(malo_pub));
    assert_ne!(ini.remota(), Some(x25519::secreto_a_publico(&res_s)), "la lista de amigos lo pilla aqui");

    // Con IK, el que inicia mete la publica de su amigo en el hash desde el
    // principio: el malo no puede ni abrir el primer mensaje.
    let res_pub = x25519::secreto_a_publico(&res_s);
    let mut ini = Saludo::nuevo(Patron::IK, Papel::Inicia, b"", &ini_s, &[7u8; 32], Some(&res_pub)).unwrap();
    let mut malo = Saludo::nuevo(Patron::IK, Papel::Responde, b"", &malo_s, &[9u8; 32], None).unwrap();
    let n = ini.escribir(&[], &mut b).unwrap();
    assert_eq!(malo.leer(&b[..n], &mut f), Err(Rechazo::Etiqueta));
    assert_eq!(malo.leer(&b[..n], &mut f), Err(Rechazo::Roto), "un saludo roto no se reintenta");
}

/// Un solo bit cambiado en cualquier sitio de cualquier mensaje con clave
/// tumba el saludo. En el primer mensaje de XX no hay clave todavia: el
/// cambio no se nota ahi, se nota en el segundo, porque el hash ya no cuadra.
#[test]
fn un_bit_tocado_en_el_saludo_no_pasa() {
    for v in [&XX, &IK] {
        let p = patron_de(v);
        for paso in 0..p.pasos() {
            let medida = p.medida_vacia(paso).unwrap();
            for byte in 0..medida {
                let (mut ini, mut res) = puntas(v);
                let mut b = [0u8; 256];
                let mut f = [0u8; 256];
                let mut fallo = false;
                for i in 0..p.pasos() {
                    let (a, o) = if i % 2 == 0 { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
                    let Ok(n) = a.escribir(&[], &mut b) else {
                        fallo = true;
                        break;
                    };
                    if i == paso {
                        b[byte] ^= 0x01;
                    }
                    if o.leer(&b[..n], &mut f).is_err() {
                        fallo = true;
                        break;
                    }
                }
                assert!(fallo, "{} paso {} byte {}: un bit tocado y el saludo termino", v.nombre, paso, byte);
            }
        }
    }
}

#[test]
fn los_turnos_y_el_orden() {
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    let (_, mut res) = puntas(&XX);
    assert_eq!(res.escribir(&[], &mut b), Err(Rechazo::FueraDeTurno), "el que responde no empieza");
    let (mut ini, _) = puntas(&XX);
    assert_eq!(ini.leer(&[0u8; 32], &mut f), Err(Rechazo::FueraDeTurno));
    let (mut ini, mut res) = puntas(&XX);
    let n = ini.escribir(&[], &mut b).unwrap();
    assert_eq!(res.leer(&b[..n - 1], &mut f), Err(Rechazo::Corto));
    let (ini, _) = puntas(&XX);
    assert!(matches!(ini.partir(), Err(Rechazo::SaludoSinTerminar)));
}

#[test]
fn ik_sin_la_remota_no_empieza_y_xx_no_la_lleva() {
    let k = [1u8; 32];
    assert!(matches!(Saludo::nuevo(Patron::IK, Papel::Inicia, b"", &k, &k, None), Err(Rechazo::FaltaRemota)));
    assert!(matches!(Saludo::nuevo(Patron::XX, Papel::Inicia, b"", &k, &k, Some(&k)), Err(Rechazo::SobraRemota)));
    assert!(matches!(Saludo::nuevo(Patron::IK, Papel::Responde, b"", &k, &k, Some(&k)), Err(Rechazo::SobraRemota)));
}

/// RFC 7748: una publica de orden chico da un secreto de ceros. Una efimera de
/// ceros en el cable es la forma mas simple de mandarla.
#[test]
fn una_efimera_de_ceros_es_clave_debil() {
    let (_, mut res) = puntas(&XX);
    let mut f = [0u8; 64];
    // El primer mensaje de XX es solo `e`: lo lee sin DH, asi que pasa...
    assert_eq!(res.leer(&[0u8; 32], &mut f), Ok(0));
    // ...y el DH `ee` del segundo, al escribirlo, da ceros.
    let mut b = [0u8; 256];
    assert_eq!(res.escribir(&[], &mut b), Err(Rechazo::ClaveDebil));
}

#[test]
fn el_transporte_no_se_deja_repetir_ni_tocar() {
    let (mut ini, mut res) = puntas(&IK);
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    for i in 0..2 {
        let (a, o) = if i % 2 == 0 { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
        let n = a.escribir(&[], &mut b).unwrap();
        o.leer(&b[..n], &mut f).unwrap();
    }
    let mut ci = ini.partir().unwrap();
    let mut cr = res.partir().unwrap();
    let n = ci.sellar(b"hola", &mut b).unwrap();
    let copia = b[..n].to_vec();
    assert_eq!(cr.abrir(&copia, &mut f), Ok(4));
    assert_eq!(&f[..4], b"hola");
    // El mismo mensaje otra vez: el contador ya subio y no abre.
    assert_eq!(cr.abrir(&copia, &mut f), Err(Rechazo::Etiqueta));
    // Uno tocado no abre y NO mueve el contador: el siguiente bueno si abre.
    let n = ci.sellar(b"adios", &mut b).unwrap();
    let mut tocado = b[..n].to_vec();
    tocado[0] ^= 0x80;
    assert_eq!(cr.abrir(&tocado, &mut f), Err(Rechazo::Etiqueta));
    assert_eq!(cr.abrir(&b[..n], &mut f), Ok(5));
    assert_eq!((ci.enviados(), cr.recibidos()), (2, 2));
    // Y lo que manda una punta no lo abre ella misma: son dos claves.
    let n = ci.sellar(b"eco", &mut b).unwrap();
    assert_eq!(ci.abrir(&b[..n], &mut f), Err(Rechazo::Etiqueta));
    assert_eq!(cr.hash(), ci.hash());
}

#[test]
fn las_medidas_del_transporte() {
    let (mut ini, mut res) = puntas(&IK);
    let mut b = [0u8; 256];
    let mut f = [0u8; 256];
    for i in 0..2 {
        let (a, o) = if i % 2 == 0 { (&mut ini, &mut res) } else { (&mut res, &mut ini) };
        let n = a.escribir(&[], &mut b).unwrap();
        o.leer(&b[..n], &mut f).unwrap();
    }
    let mut ci = ini.partir().unwrap();
    let mut cr = res.partir().unwrap();
    let grande = alloc::vec![0u8; MENSAJE_MAX - ETIQUETA + 1];
    let mut sitio = alloc::vec![0u8; MENSAJE_MAX + 16];
    assert_eq!(ci.sellar(&grande, &mut sitio), Err(Rechazo::Largo));
    let justo = &grande[..MENSAJE_MAX - ETIQUETA];
    let n = ci.sellar(justo, &mut sitio).unwrap();
    assert_eq!(n, MENSAJE_MAX);
    let mut salida = alloc::vec![0u8; MENSAJE_MAX];
    assert_eq!(cr.abrir(&sitio[..n], &mut salida), Ok(MENSAJE_MAX - ETIQUETA));
    assert_eq!(ci.sellar(b"x", &mut [0u8; 4]), Err(Rechazo::SinSitio));
    assert_eq!(cr.abrir(&[0u8; 15], &mut f), Err(Rechazo::Corto));
}
