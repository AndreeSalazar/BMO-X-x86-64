//! **EL REGISTRO** -- lo que escribe un programa, cada linea distinta UNA
//! vez, con cuantas veces vino; y las SERIES de las graficas.
//!
//! generacion: hijo
//!
//! Pedido por el propietario (02-10): *"el printf en tiempo real pero que no
//! se repita lo que ya conoce, que CABINA lea todo para que registre y si se
//! repite se ignora pero pones (x1) hasta el infinito"*. Cyberpunk escribe
//! seis veces `GetCurrentDirectoryW: "D:\Cyberpunk 2077\bin\x64"` seguidas, y
//! cada repeticion empuja fuera de la pantalla una linea que si decia algo.
//!
//! ```text
//!    Registro    los bytes llegan a trozos (como los da la tuberia del
//!                programa); se parten en lineas; una linea NUEVA se apunta
//!                al final, una REPETIDA solo sube su cuenta (y su "ultima
//!                vez", para pintar lo que se repite AHORA)
//!    Serie       las ultimas N muestras de un numero, para una grafica
//! ```
//!
//! Sin `alloc` (el DIRECTOR no lo tiene): todo es de medida fija. Lo que no
//! cabe se CUENTA y se dice -- `sin_sitio` -- en vez de tirarse callado.

#![cfg_attr(not(test), no_std)]

pub mod serie;

pub use serie::Serie;

/// Lo mas largo que se guarda de una linea: lo de detras se corta (y se dice
/// con [`Entrada::cortada`]).
pub const LARGO: usize = 160;

/// **Una linea distinta**, con sus veces.
#[derive(Clone, Copy)]
pub struct Entrada {
    texto: [u8; LARGO],
    largo: u16,
    /// Si llego mas larga de [`LARGO`] y se corto.
    pub cortada: bool,
    /// Cuantas veces vino (1 la primera).
    pub veces: u32,
    /// El numero de linea (de todas las que llegaron, repetidas incluidas) de
    /// la primera vez y de la ultima.
    pub primera: u64,
    pub ultima: u64,
    huella: u64,
}

impl Entrada {
    const VACIA: Entrada = Entrada { texto: [0; LARGO], largo: 0, cortada: false, veces: 0, primera: 0, ultima: 0, huella: 0 };

    pub fn texto(&self) -> &[u8] {
        &self.texto[..self.largo as usize]
    }
}

/// FNV-1a: para buscar rapido; la igualdad se mira byte a byte.
fn huella(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))
}

/// **El registro**: `N` lineas distintas como mucho.
pub struct Registro<const N: usize> {
    entradas: [Entrada; N],
    usadas: usize,
    /// La linea que esta llegando (aun sin su `\n`).
    pendiente: [u8; LARGO],
    largo_pendiente: usize,
    cortada_pendiente: bool,
    /// Cuantas lineas llegaron, repetidas incluidas.
    pub lineas: u64,
    /// Lineas NUEVAS que ya no cupieron (el registro estaba lleno).
    pub sin_sitio: u64,
}

impl<const N: usize> Registro<N> {
    pub const fn nuevo() -> Self {
        Registro { entradas: [Entrada::VACIA; N], usadas: 0, pendiente: [0; LARGO], largo_pendiente: 0, cortada_pendiente: false, lineas: 0, sin_sitio: 0 }
    }

    /// Todo a cero (un programa nuevo).
    pub fn vaciar(&mut self) {
        self.usadas = 0;
        self.largo_pendiente = 0;
        self.cortada_pendiente = false;
        self.lineas = 0;
        self.sin_sitio = 0;
    }

    /// Cuantas lineas distintas hay.
    pub fn distintas(&self) -> usize {
        self.usadas
    }

    /// La distinta `i` (en el orden en que llegaron por primera vez).
    pub fn entrada(&self, i: usize) -> Option<&Entrada> {
        self.entradas[..self.usadas].get(i)
    }

    /// Cuantas lineas llegaron repetidas (lo que el registro se ahorro).
    pub fn repetidas(&self) -> u64 {
        self.entradas[..self.usadas].iter().map(|e| (e.veces - 1) as u64).sum()
    }

    /// **Lo que escribio el programa**, tal como llega: a trozos, con `\r\n`
    /// o `\n`. Cada linea entera se apunta; lo que quede sin `\n` espera al
    /// siguiente trozo.
    pub fn escribir(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match b {
                b'\n' => self.cerrar_linea(),
                b'\r' => {}
                _ if self.largo_pendiente < LARGO => {
                    self.pendiente[self.largo_pendiente] = b;
                    self.largo_pendiente += 1;
                }
                _ => self.cortada_pendiente = true,
            }
        }
    }

    /// Lo que quedo sin `\n` (el programa acabo a media linea), como linea.
    pub fn terminar(&mut self) {
        if self.largo_pendiente > 0 || self.cortada_pendiente {
            self.cerrar_linea();
        }
    }

    fn cerrar_linea(&mut self) {
        let (n, cortada) = (self.largo_pendiente, self.cortada_pendiente);
        self.largo_pendiente = 0;
        self.cortada_pendiente = false;
        // Sin los espacios del final: dos lineas que solo difieren en eso se
        // ven iguales, y son la misma.
        let mut fin = n;
        while fin > 0 && self.pendiente[fin - 1] == b' ' {
            fin -= 1;
        }
        let texto = self.pendiente;
        self.apuntar(&texto[..fin], cortada);
    }

    /// **Apuntar una linea** (sin su `\n`). Devuelve su indice y si es NUEVA;
    /// `None` si era nueva y no cupo.
    pub fn apuntar(&mut self, linea: &[u8], cortada: bool) -> Option<(usize, bool)> {
        self.lineas += 1;
        let linea = &linea[..linea.len().min(LARGO)];
        let h = huella(linea);
        let numero = self.lineas;
        if let Some(i) = self.entradas[..self.usadas].iter().position(|e| e.huella == h && e.texto() == linea) {
            let e = &mut self.entradas[i];
            e.veces = e.veces.saturating_add(1);
            e.ultima = numero;
            return Some((i, false));
        }
        if self.usadas == N {
            self.sin_sitio += 1;
            return None;
        }
        let e = &mut self.entradas[self.usadas];
        *e = Entrada::VACIA;
        e.texto[..linea.len()].copy_from_slice(linea);
        e.largo = linea.len() as u16;
        e.cortada = cortada;
        e.veces = 1;
        e.primera = numero;
        e.ultima = numero;
        e.huella = h;
        self.usadas += 1;
        Some((self.usadas - 1, true))
    }

    /// **La linea como se muestra**: su texto y, si se repitio, `  (xN)`.
    /// Devuelve cuantos bytes se escribieron en `dst`.
    pub fn con_veces(e: &Entrada, dst: &mut [u8]) -> usize {
        let mut n = 0;
        let mut poner = |s: &[u8], n: &mut usize| {
            for &b in s {
                if *n < dst.len() {
                    dst[*n] = b;
                    *n += 1;
                }
            }
        };
        poner(e.texto(), &mut n);
        if e.cortada {
            poner(b"...", &mut n);
        }
        if e.veces > 1 {
            poner(b"  (x", &mut n);
            let mut d = [0u8; 20];
            let k = decimal(e.veces as u64, &mut d);
            poner(&d[..k], &mut n);
            poner(b")", &mut n);
        }
        n
    }
}

impl<const N: usize> Default for Registro<N> {
    fn default() -> Self {
        Self::nuevo()
    }
}

/// `v` en decimal; devuelve cuantas cifras.
pub fn decimal(mut v: u64, d: &mut [u8; 20]) -> usize {
    let mut t = [0u8; 20];
    let mut k = 0;
    loop {
        t[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for i in 0..k {
        d[i] = t[k - 1 - i];
    }
    k
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ver<const N: usize>(r: &Registro<N>) -> Vec<String> {
        (0..r.distintas()).map(|i| {
            let mut b = [0u8; 200];
            let n = Registro::<N>::con_veces(r.entrada(i).unwrap(), &mut b);
            String::from_utf8(b[..n].to_vec()).unwrap()
        }).collect()
    }

    #[test]
    fn lo_repetido_sube_su_cuenta_y_no_se_vuelve_a_escribir() {
        let mut r = Registro::<8>::nuevo();
        // Llega a trozos, partido en cualquier sitio, con \r\n.
        r.escribir(b"PROTON-X: GetCurrentDirectoryW: \"D:\\x64\"\r\nPROTON-X: Get");
        r.escribir(b"CurrentDirectoryW: \"D:\\x64\"\r\nPROTON-X: otra\n");
        r.escribir(b"PROTON-X: GetCurrentDirectoryW: \"D:\\x64\"   \r\n");
        assert_eq!(ver(&r), ["PROTON-X: GetCurrentDirectoryW: \"D:\\x64\"  (x3)", "PROTON-X: otra"]);
        assert_eq!((r.lineas, r.distintas(), r.repetidas()), (4, 2, 2));
        let e = r.entrada(0).unwrap();
        assert_eq!((e.primera, e.ultima), (1, 4));
        // Lo que queda sin \n espera; al terminar, es una linea.
        r.escribir(b"a medias");
        assert_eq!(r.distintas(), 2);
        r.terminar();
        assert_eq!(ver(&r)[2], "a medias");
    }

    #[test]
    fn lleno_lo_nuevo_se_cuenta_y_lo_repetido_sigue_contando() {
        let mut r = Registro::<2>::nuevo();
        r.escribir(b"uno\ndos\ntres\nuno\ncuatro\n");
        assert_eq!(ver(&r), ["uno  (x2)", "dos"]);
        assert_eq!(r.sin_sitio, 2);
        // Una linea mas larga que LARGO se corta y lo dice.
        let mut l = Registro::<1>::nuevo();
        l.escribir(&[b'x'; LARGO + 5]);
        l.escribir(b"\n");
        let v = ver(&l);
        assert_eq!(v[0].len(), LARGO + 3);
        assert!(v[0].ends_with("..."));
        l.vaciar();
        assert_eq!((l.distintas(), l.lineas), (0, 0));
    }
}
