//! **LA HUELLA** -- lo que se lee en voz alta para hacerse amigos.
//!
//! [carril]  VERDE  solo cuentas sobre una clave que ya se tiene
//!
//! Una clave publica son 32 bytes que nadie compara a ojo. La huella es el
//! SHA-256 de la clave, en 16 grupos de 4 cifras hexadecimales: lo que las
//! dos pantallas muestran, y lo que dos personas leen en voz alta antes de
//! aceptarse. Es exactamente lo que dibuja la maqueta en AMIGOS.
//!
//! ```text
//!    7F3A 9C21 E04B 55D8 1A6C F2E9 0B47 C3D5 88A1 4E2F 6D90 B7C2 31FA 0E58 D4A7 9C16
//! ```
//!
//! ** Se hashea la clave en vez de mostrarla tal cual para que dos claves
//! parecidas no den huellas parecidas: cambiar un bit de la clave cambia
//! media huella, y eso es lo que un ojo cansado SI ve.

use bmo_cripto::sha256;

/// Caracteres de una huella: 16 grupos de 4 y 15 espacios.
pub const HUELLA: usize = 16 * 4 + 15;

/// **La huella de una clave publica**, en ASCII.
pub fn huella(publica: &[u8; 32]) -> [u8; HUELLA] {
    const CIFRAS: &[u8; 16] = b"0123456789ABCDEF";
    let h = sha256::hash(publica);
    let mut out = [b' '; HUELLA];
    let mut p = 0;
    for (i, par) in h.chunks(2).enumerate() {
        if i > 0 {
            p += 1;
        }
        for b in par {
            out[p] = CIFRAS[(b >> 4) as usize];
            out[p + 1] = CIFRAS[(b & 0xF) as usize];
            p += 2;
        }
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_forma_de_la_huella() {
        let h = huella(&[0u8; 32]);
        // SHA-256 de 32 ceros: 66687aad f862bd77 6c8fc18b 8e9f8e20 ...
        assert_eq!(&h[..19], b"6668 7AAD F862 BD77");
        assert_eq!(h.iter().filter(|&&c| c == b' ').count(), 15);
        assert!(h.iter().all(|c| c.is_ascii_hexdigit() || *c == b' '));
    }

    #[test]
    fn un_bit_cambia_media_huella() {
        let a = huella(&[0u8; 32]);
        let mut k = [0u8; 32];
        k[31] = 1;
        let b = huella(&k);
        let distintas = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
        assert!(distintas > 30, "solo {} de 64 cifras cambiaron", distintas);
    }
}
