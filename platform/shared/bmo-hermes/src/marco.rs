//! **EL MARCO** -- como se cortan los mensajes de Noise en un flujo de TCP.
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina
//! [cuesta]  DATO      un marco mal cortado pega dos mensajes o parte uno
//! [riesgo]  AJENO     la cabecera la escribe la otra punta
//!
//! TCP entrega bytes, no mensajes. Cada mensaje de Noise va precedido de su
//! medida en dos bytes big-endian, que es lo que recomienda la especificacion
//! de Noise y por lo que un mensaje mide como mucho 65.535 bytes.
//!
//! ```text
//!    [medida: u16 BE][mensaje de Noise: medida bytes]
//! ```
//!
//! [`Lector`] junta lo que llega del cable en un buffer fijo y entrega los
//! marcos enteros. Sin monton: el buffer es el de un marco y su cabecera.
//!
//! [!] Una medida de 0 se rechaza: ningun mensaje de HERMES/1 esta vacio, y un
//! marco vacio repetido seria una forma de hacer girar a la PUERTA sin coste.

use crate::noise::MENSAJE_MAX;
use crate::Rechazo;

/// Bytes de la cabecera de un marco.
pub const CABECERA: usize = 2;

/// **Pone la cabecera** a `msg` en `dst`. Devuelve cuantos bytes van al cable.
pub fn enmarcar(msg: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
    if msg.is_empty() {
        return Err(Rechazo::Vacio);
    }
    if msg.len() > MENSAJE_MAX {
        return Err(Rechazo::Largo);
    }
    let n = CABECERA + msg.len();
    let d = dst.get_mut(..n).ok_or(Rechazo::SinSitio)?;
    d[..CABECERA].copy_from_slice(&(msg.len() as u16).to_be_bytes());
    d[CABECERA..].copy_from_slice(msg);
    Ok(n)
}

/// **Junta el flujo y entrega marcos enteros.**
pub struct Lector {
    buf: [u8; CABECERA + MENSAJE_MAX],
    largo: usize,
}

impl Default for Lector {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Lector {
    pub const fn nuevo() -> Self {
        Lector { buf: [0; CABECERA + MENSAJE_MAX], largo: 0 }
    }

    /// **Mete bytes del cable.** Devuelve cuantos tomo: si el buffer esta
    /// lleno, toma menos, y lo que sobra se vuelve a meter despues de sacar
    /// el marco que espera.
    pub fn meter(&mut self, bytes: &[u8]) -> usize {
        let cabe = (self.buf.len() - self.largo).min(bytes.len());
        self.buf[self.largo..self.largo + cabe].copy_from_slice(&bytes[..cabe]);
        self.largo += cabe;
        cabe
    }

    /// **El siguiente marco entero**, copiado en `dst`; `Ok(None)` si todavia
    /// no ha llegado entero.
    pub fn siguiente(&mut self, dst: &mut [u8]) -> Result<Option<usize>, Rechazo> {
        if self.largo < CABECERA {
            return Ok(None);
        }
        let m = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
        if m == 0 {
            return Err(Rechazo::Vacio);
        }
        if self.largo < CABECERA + m {
            return Ok(None);
        }
        dst.get_mut(..m).ok_or(Rechazo::SinSitio)?.copy_from_slice(&self.buf[CABECERA..CABECERA + m]);
        self.buf.copy_within(CABECERA + m..self.largo, 0);
        self.largo -= CABECERA + m;
        Ok(Some(m))
    }

    /// Cuantos bytes esperan dentro, sin formar marco todavia.
    pub fn pendientes(&self) -> usize {
        self.largo
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate alloc;
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    fn flujo() -> (Vec<u8>, Vec<Vec<u8>>) {
        let msgs: Vec<Vec<u8>> = [1usize, 2, 300, 32, 96, 64, 7].iter().enumerate().map(|(i, &n)| alloc::vec![i as u8 + 1; n]).collect();
        let mut f = alloc::vec![0u8; 0];
        let mut b = [0u8; 400];
        for m in &msgs {
            let n = enmarcar(m, &mut b).unwrap();
            f.extend_from_slice(&b[..n]);
        }
        (f, msgs)
    }

    /// TCP puede partir el flujo por donde quiera: de byte en byte, a trozos
    /// de siete, o todo de golpe. Los marcos tienen que salir iguales.
    #[test]
    fn un_flujo_partido_de_cualquier_forma_da_los_mismos_marcos() {
        let (f, msgs) = flujo();
        for corte in [1usize, 2, 3, 7, 64, 1000] {
            let mut l = Box::new(Lector::nuevo());
            let mut salida: Vec<Vec<u8>> = Vec::new();
            let mut d = [0u8; 400];
            for trozo in f.chunks(corte) {
                assert_eq!(l.meter(trozo), trozo.len());
                while let Some(n) = l.siguiente(&mut d).unwrap() {
                    salida.push(d[..n].to_vec());
                }
            }
            assert_eq!(salida, msgs, "cortando de {} en {}", corte, corte);
            assert_eq!(l.pendientes(), 0);
        }
    }

    #[test]
    fn las_medidas_del_marco() {
        let mut b = alloc::vec![0u8; CABECERA + MENSAJE_MAX + 1];
        assert_eq!(enmarcar(&[], &mut b), Err(Rechazo::Vacio));
        assert_eq!(enmarcar(&alloc::vec![1u8; MENSAJE_MAX + 1], &mut b), Err(Rechazo::Largo));
        assert_eq!(enmarcar(&alloc::vec![1u8; MENSAJE_MAX], &mut b), Ok(CABECERA + MENSAJE_MAX));
        assert_eq!(enmarcar(&[1, 2, 3], &mut [0u8; 4]), Err(Rechazo::SinSitio));
        let mut l = Box::new(Lector::nuevo());
        l.meter(&[0, 0, 9]);
        assert_eq!(l.siguiente(&mut [0u8; 8]), Err(Rechazo::Vacio), "un marco de cero no se entrega");
        let mut l = Box::new(Lector::nuevo());
        l.meter(&[0, 5, 1, 2, 3, 4, 5]);
        assert_eq!(l.siguiente(&mut [0u8; 4]), Err(Rechazo::SinSitio));
    }

    /// El buffer lleno no pierde nada: toma lo que cabe, y lo demas entra
    /// en cuanto sale el marco que estaba esperando.
    #[test]
    fn el_buffer_lleno_no_pierde_bytes() {
        let mut b = alloc::vec![0u8; CABECERA + MENSAJE_MAX];
        let n = enmarcar(&alloc::vec![7u8; MENSAJE_MAX], &mut b).unwrap();
        let mut f = b[..n].to_vec();
        f.extend_from_slice(&[0, 1, 9]);
        let mut l = Box::new(Lector::nuevo());
        let tomado = l.meter(&f);
        assert_eq!(tomado, CABECERA + MENSAJE_MAX);
        let mut d = alloc::vec![0u8; MENSAJE_MAX];
        assert_eq!(l.siguiente(&mut d), Ok(Some(MENSAJE_MAX)));
        assert_eq!(l.meter(&f[tomado..]), 3);
        assert_eq!(l.siguiente(&mut d), Ok(Some(1)));
        assert_eq!(d[0], 9);
    }
}
