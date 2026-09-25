//! **EL BUZON** -- una pagina donde el escritorio deja sus cajas sucias, y como
//! la lee el kernel sin creerla.
//!
//! [carril]  ROJO      el unico sitio donde el kernel lee cajas que escribio Ring 3
//! [cuesta]  DATO      una caja mal leida manda a la GPU a copiar fuera de la pantalla
//! [riesgo]  AJENO     esta memoria la escribe el proceso, y puede cambiarla
//!                     MIENTRAS el kernel la lee
//!
//! # La forma (palabras de 32 bits; cabe de sobra en UNA pagina)
//!
//! ```text
//!    palabra   quien la ESCRIBE   que
//!    0         el kernel          MAGIA `BGPU` (a cero: revocado)
//!    1         el kernel          VERSION
//!    2         el kernel          ESTADO: 0 abierto, si no el `radar::Motivo`
//!    3         el kernel          ENVIADO: la ultima tanda con el timbre tocado
//!    4         el kernel          PAGADO: la ultima que la GPU pago
//!    5         el kernel          ancho | alto << 16 de la pantalla
//!    8         el proceso         CERRADO: el numero de la tanda lista
//!    9         el proceso         N: cuantas cajas trae
//!    16..      el proceso         CAJAS: `x | y << 16`, `w | h << 16`, hasta 64
//! ```
//!
//! # *** Las tres reglas del lado del kernel ([`Lado`])
//!
//! 1. **Su propio numero lo guarda en SU memoria.** ENVIADO en el buzon es
//!    informativo: si el proceso lo pisa, al kernel no le importa.
//! 2. **El kernel COPIA las palabras una vez y juzga la copia.** Una caja leida
//!    dos veces puede valer dos cosas, y la segunda es la que ataca.
//! 3. **Un numero que salta o una caja fuera de la pantalla son una mentira**,
//!    no un estado. No se "arreglan": se devuelve [`Mal`] y el radar revoca.
//!
//! # El protocolo del escritorio ([`escribir`])
//!
//! Dibuja en el lienzo, escribe las cajas y N, y **el numero el ultimo** (con
//! una barrera Release delante: es lo que dice "ya esta"). No cierra la
//! siguiente hasta que ENVIADO diga la suya, y no vuelve a pintar en el lienzo
//! hasta que PAGADO la diga: la GPU lo esta leyendo.

/// `BGPU`, leido en little-endian. Un buzon revocado lo tiene a cero.
pub const MAGIA: u32 = u32::from_le_bytes(*b"BGPU");
pub const VERSION: u32 = 1;
/// Las cajas de una tanda. Mas que eso, el escritorio manda la pantalla entera.
pub const MAX_CAJAS: usize = 64;

pub mod campo {
    pub const MAGIA: usize = 0;
    pub const VERSION: usize = 1;
    pub const ESTADO: usize = 2;
    pub const ENVIADO: usize = 3;
    pub const PAGADO: usize = 4;
    pub const MEDIDAS: usize = 5;
    pub const CERRADO: usize = 8;
    pub const N: usize = 9;
    pub const CAJAS: usize = 16;
}

/// Las palabras que el kernel copia en cada latido.
pub const PALABRAS: usize = campo::CAJAS + 2 * MAX_CAJAS;

/// La medida de la pantalla, la unica que el buzon necesita.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Medidas {
    pub ancho: u32,
    pub alto: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Caja {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Caja {
    pub const fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }

    fn cabe(&self, m: Medidas) -> bool {
        self.w > 0 && self.h > 0 && self.x < m.ancho && self.y < m.alto && self.w <= m.ancho - self.x && self.h <= m.alto - self.y
    }
}

/// Lo que el kernel saca del buzon: YA copiado y juzgado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tanda {
    pub numero: u32,
    pub n: usize,
    pub cajas: [Caja; MAX_CAJAS],
}

impl Tanda {
    pub fn cajas(&self) -> &[Caja] {
        &self.cajas[..self.n]
    }

    /// Los bytes que movera la GPU (4 por pixel).
    pub fn bytes(&self) -> u64 {
        self.cajas().iter().map(|c| c.area() * 4).sum()
    }
}

/// **Por que el buzon miente.** Cada una revoca AL MOMENTO.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mal {
    /// La copia no llega a [`PALABRAS`]: el buzon no es el que se mapeo.
    BuzonCorto,
    /// CERRADO no es el siguiente de la ultima enviada: el escritorio no espero.
    NumeroImposible,
    /// N es cero o mas de [`MAX_CAJAS`].
    CajasImposibles,
    /// Una caja vacia o que se sale de la pantalla.
    CajaFuera,
}

/// El siguiente numero de tanda; el cero no existe (es "ninguna todavia").
pub const fn siguiente(n: u32) -> u32 {
    let s = n.wrapping_add(1);
    if s == 0 {
        1
    } else {
        s
    }
}

/// **El lado del kernel.** Guarda SU numero; el del buzon no lo cree.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Lado {
    enviado: u32,
}

impl Lado {
    pub const fn nuevo() -> Self {
        Self { enviado: 0 }
    }

    pub const fn enviado(&self) -> u32 {
        self.enviado
    }

    /// **Un latido.** `copia` = las palabras del buzon, copiadas UNA vez;
    /// `pagado` = lo que la GPU dice en SU semaforo (no lo que dice el buzon).
    ///
    /// `Ok(None)`: nada nuevo, o la anterior aun no esta pagada (la GPU sigue
    /// leyendo el lienzo; se mira en el latido siguiente). `Ok(Some)`: la tanda,
    /// y el numero ya cuenta como enviado.
    pub fn leer(&mut self, copia: &[u32], m: Medidas, pagado: u32) -> Result<Option<Tanda>, Mal> {
        if copia.len() < PALABRAS {
            return Err(Mal::BuzonCorto);
        }
        let cerrado = copia[campo::CERRADO];
        if cerrado == self.enviado {
            return Ok(None);
        }
        if cerrado != siguiente(self.enviado) {
            return Err(Mal::NumeroImposible);
        }
        if pagado != self.enviado {
            return Ok(None);
        }
        let n = copia[campo::N] as usize;
        if n == 0 || n > MAX_CAJAS {
            return Err(Mal::CajasImposibles);
        }
        let mut t = Tanda { numero: cerrado, n, cajas: [Caja::default(); MAX_CAJAS] };
        for (k, c) in t.cajas[..n].iter_mut().enumerate() {
            let (a, b) = (copia[campo::CAJAS + 2 * k], copia[campo::CAJAS + 2 * k + 1]);
            *c = Caja { x: a & 0xFFFF, y: a >> 16, w: b & 0xFFFF, h: b >> 16 };
            if !c.cabe(m) {
                return Err(Mal::CajaFuera);
            }
        }
        // Hacer MENOS: si lo sucio pasa de media pantalla, una copia entera sale
        // mas barata que muchas ordenes (y el bus va al techo con una sola).
        let entera = m.ancho as u64 * m.alto as u64;
        if t.cajas().iter().map(Caja::area).sum::<u64>() * 2 > entera {
            t.n = 1;
            t.cajas[0] = Caja { x: 0, y: 0, w: m.ancho, h: m.alto };
        }
        self.enviado = cerrado;
        Ok(Some(t))
    }
}

/// **El buzon recien abierto**, como lo deja el kernel.
pub fn formar(pal: &mut [u32], m: Medidas) {
    pal.iter_mut().for_each(|w| *w = 0);
    pal[campo::MAGIA] = MAGIA;
    pal[campo::VERSION] = VERSION;
    pal[campo::MEDIDAS] = m.ancho | m.alto << 16;
}

/// **Revocado**: sin magia y con el motivo. Lo demas se queda como estaba.
pub fn revocar(pal: &mut [u32], motivo: u32) {
    pal[campo::MAGIA] = 0;
    pal[campo::ESTADO] = motivo;
}

/// **El lado del escritorio**: escribe las cajas y el numero EL ULTIMO.
/// `None` si no toca todavia (ENVIADO no llego a su ultima tanda) o si las
/// cajas no caben; `Some(numero)` de la tanda cerrada.
pub fn escribir(pal: &mut [u32], mi_ultimo: u32, cajas: &[Caja]) -> Option<u32> {
    if pal.len() < PALABRAS || pal[campo::MAGIA] != MAGIA || pal[campo::ENVIADO] != mi_ultimo {
        return None;
    }
    if cajas.is_empty() || cajas.len() > MAX_CAJAS {
        return None;
    }
    for (k, c) in cajas.iter().enumerate() {
        pal[campo::CAJAS + 2 * k] = c.x | c.y << 16;
        pal[campo::CAJAS + 2 * k + 1] = c.w | c.h << 16;
    }
    pal[campo::N] = cajas.len() as u32;
    let numero = siguiente(mi_ultimo);
    pal[campo::CERRADO] = numero;
    Some(numero)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const M: Medidas = Medidas { ancho: 1920, alto: 1080 };

    fn buzon() -> [u32; PALABRAS] {
        let mut b = [0; PALABRAS];
        formar(&mut b, M);
        b
    }

    fn caja(x: u32, y: u32, w: u32, h: u32) -> Caja {
        Caja { x, y, w, h }
    }

    #[test]
    fn la_forma_cabe_en_una_pagina() {
        assert!(PALABRAS * 4 <= 4096);
        assert_eq!(MAGIA, 0x5550_4742);
        let b = buzon();
        assert_eq!(b[campo::MEDIDAS], 1920 | 1080 << 16);
    }

    #[test]
    fn una_tanda_va_y_vuelve() {
        let mut b = buzon();
        let mut k = Lado::nuevo();
        assert_eq!(k.leer(&b, M, 0), Ok(None), "nada cerrado todavia");
        let n = escribir(&mut b, 0, &[caja(10, 20, 30, 40), caja(0, 0, 8, 16)]).unwrap();
        assert_eq!(n, 1);
        let t = k.leer(&b, M, 0).unwrap().unwrap();
        assert_eq!((t.numero, t.cajas()), (1, &[caja(10, 20, 30, 40), caja(0, 0, 8, 16)][..]));
        assert_eq!(t.bytes(), (30 * 40 + 8 * 16) * 4);
        assert_eq!(k.enviado(), 1);
        assert_eq!(k.leer(&b, M, 0), Ok(None), "la misma no se envia dos veces");
    }

    /// El escritorio no cierra la siguiente hasta que ENVIADO diga la suya.
    #[test]
    fn el_escritorio_espera_a_enviado() {
        let mut b = buzon();
        assert_eq!(escribir(&mut b, 0, &[caja(0, 0, 1, 1)]), Some(1));
        assert_eq!(escribir(&mut b, 1, &[caja(0, 0, 1, 1)]), None, "ENVIADO sigue en 0");
        b[campo::ENVIADO] = 1;
        assert_eq!(escribir(&mut b, 1, &[caja(0, 0, 1, 1)]), Some(2));
        assert_eq!(escribir(&mut b, 1, &[]), None, "sin cajas no hay tanda");
        assert_eq!(escribir(&mut b, 1, &[caja(0, 0, 1, 1); MAX_CAJAS + 1]), None);
    }

    /// *** Mientras la GPU no pague la anterior, la nueva espera (no es mentira).
    #[test]
    fn sin_pagar_la_anterior_se_espera_al_latido_siguiente() {
        let mut b = buzon();
        let mut k = Lado::nuevo();
        escribir(&mut b, 0, &[caja(0, 0, 1, 1)]);
        k.leer(&b, M, 0).unwrap().unwrap();
        b[campo::ENVIADO] = 1;
        escribir(&mut b, 1, &[caja(0, 0, 2, 2)]);
        assert_eq!(k.leer(&b, M, 0), Ok(None), "la 1 aun no pagada");
        assert_eq!(k.leer(&b, M, 1).unwrap().unwrap().numero, 2);
    }

    #[test]
    fn un_numero_que_salta_es_mentira() {
        let mut b = buzon();
        b[campo::CERRADO] = 5;
        b[campo::N] = 1;
        assert_eq!(Lado::nuevo().leer(&b, M, 0), Err(Mal::NumeroImposible));
    }

    #[test]
    fn el_cero_no_existe_ni_al_dar_la_vuelta() {
        assert_eq!(siguiente(0), 1);
        assert_eq!(siguiente(u32::MAX), 1);
    }

    #[test]
    fn cajas_imposibles_y_cajas_fuera() {
        let mut b = buzon();
        b[campo::CERRADO] = 1;
        b[campo::N] = 0;
        assert_eq!(Lado::nuevo().leer(&b, M, 0), Err(Mal::CajasImposibles));
        b[campo::N] = MAX_CAJAS as u32 + 1;
        assert_eq!(Lado::nuevo().leer(&b, M, 0), Err(Mal::CajasImposibles));
        for c in [caja(0, 0, 0, 5), caja(1900, 0, 21, 1), caja(0, 1080, 1, 1), caja(0xFFFF, 0, 0xFFFF, 1)] {
            let mut b = buzon();
            escribir(&mut b, 0, &[c]);
            assert_eq!(Lado::nuevo().leer(&b, M, 0), Err(Mal::CajaFuera), "{c:?}");
        }
        let mut b = buzon();
        escribir(&mut b, 0, &[caja(1900, 1070, 20, 10)]);
        assert!(Lado::nuevo().leer(&b, M, 0).unwrap().is_some(), "justo en la esquina cabe");
    }

    #[test]
    fn un_buzon_corto_es_otro_buzon() {
        assert_eq!(Lado::nuevo().leer(&[0; 10], M, 0), Err(Mal::BuzonCorto));
    }

    /// Mas de media pantalla sucia: UNA copia entera.
    #[test]
    fn mucho_sucio_se_vuelve_una_copia_entera() {
        let mut b = buzon();
        escribir(&mut b, 0, &[caja(0, 0, 1920, 400), caja(0, 400, 1920, 200)]);
        let t = Lado::nuevo().leer(&b, M, 0).unwrap().unwrap();
        assert_eq!(t.cajas(), &[caja(0, 0, 1920, 1080)]);
        assert_eq!(t.bytes(), 1920 * 1080 * 4);
    }

    #[test]
    fn revocado_se_lee_sin_magia_y_con_motivo() {
        let mut b = buzon();
        revocar(&mut b, 7);
        assert_eq!((b[campo::MAGIA], b[campo::ESTADO]), (0, 7));
        assert_eq!(escribir(&mut b, 0, &[caja(0, 0, 1, 1)]), None, "sobre un buzon revocado no se escribe");
    }
}
