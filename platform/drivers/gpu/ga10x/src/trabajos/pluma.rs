//! **LA PLUMA (E1, 14-10): escribir en la VRAM por la ventana PRAMIN SIN
//! RELEER cada palabra**, y lo que ya esta escrito de los programas.
//!
//! capa: toca la tarjeta por `Registros` (la ventana PRAMIN del BAR0)
//!
//! [eje]     VELOCIDAD -- el metal del 14-10 (Cyberpunk): `preparar` 11,1 ms
//!           por lote, en frio en los seis. Era RELEER: cada palabra escrita
//!           se volvia a leer por el PCIe (~1 us cada lectura, lo medido en
//!           `volcado`), y un lote escribe hasta 64 KiB de datos, una pagina
//!           sustituta y dos programas -- unas 20.000 lecturas
//!
//! # Por que basta leer la primera y la ultima
//!
//! ```text
//!    PCIe, el orden de las transacciones: una LECTURA (non-posted) no
//!    adelanta a las ESCRITURAS (posted) que salieron antes por el mismo
//!    camino, y las escrituras no se adelantan entre ellas. Asi que la
//!    lectura de la ULTIMA palabra de un tramo vuelve despues de que todas
//!    las del tramo llegaron a la tarjeta; y la de la PRIMERA dice que la
//!    ventana estaba donde se creia al empezar (lo que fallaba de verdad:
//!    una ventana PRAMIN que no llega, `tuberia_vram`).
//! ```
//!
//! Lo que releer palabra a palabra cazaba de mas -- una escritura suelta
//! perdida en medio de un tramo -- no es algo que haga un PCIe vivo; y
//! el caliente (V1) ya escribia sin releer nada desde el 26-09. La prueba de
//! que el dibujo entero llego sigue siendo la de siempre: que la 3060 pague
//! su semaforo.
//!
//! # Lo que ya esta escrito
//!
//! Los dos programas de la tuberia viven en SU region (`vram::CODIGO`) y
//! solo los escribe el kernel por esta ventana. Si el que llega es el mismo
//! que ya esta (su [`huella`]), no se vuelve a escribir: un juego repite el
//! programa de vertice en muchos lotes seguidos aunque cambie el de pixel.
//! [`Escritos`] lo guarda quien tiene el cerrojo del GR (el kernel) y lo
//! olvida cuando alguien mas escribe alli (el anillo) o el canal se pierde.

use crate::vram::{ventana, VENTANA, VENTANA_REG};
use crate::Registros;

/// **La pluma**: la ventana se lee UNA vez al tomarla y se devuelve al
/// soltarla; entre medias se mueve solo cuando el tramo cambia de MiB.
pub struct Pluma<'a, R: Registros> {
    r: &'a mut R,
    antes: u32,
    base: u32,
}

impl<'a, R: Registros> Pluma<'a, R> {
    pub fn nueva(r: &'a mut R) -> Self {
        let antes = r.leer(VENTANA_REG);
        Self { r, antes, base: antes }
    }

    /// El registro de la palabra en `dir`, con la ventana movida si hace falta.
    fn en(&mut self, dir: u64) -> u32 {
        let (base, off) = ventana(dir);
        if base != self.base {
            self.r.escribir(VENTANA_REG, base);
            self.base = base;
        }
        VENTANA + off
    }

    /// Palabras seguidas desde `dir`, sin releer.
    pub fn palabras(&mut self, dir: u64, p: &[u32]) {
        for (k, &v) in p.iter().enumerate() {
            let reg = self.en(dir + 4 * k as u64);
            self.r.escribir(reg, v);
        }
    }

    /// Bytes (multiplo de 4) desde `dir`, sin releer.
    pub fn bytes(&mut self, dir: u64, b: &[u8]) {
        for (k, q) in b.chunks_exact(4).enumerate() {
            let reg = self.en(dir + 4 * k as u64);
            self.r.escribir(reg, u32::from_le_bytes([q[0], q[1], q[2], q[3]]));
        }
    }

    /// `n` palabras a cero desde `dir`, sin releer.
    pub fn ceros(&mut self, dir: u64, n: usize) {
        for k in 0..n {
            let reg = self.en(dir + 4 * k as u64);
            self.r.escribir(reg, 0);
        }
    }

    /// Una palabra de la VRAM.
    pub fn leer(&mut self, dir: u64) -> u32 {
        let reg = self.en(dir);
        self.r.leer(reg)
    }

    /// **Llego** el tramo de `n` palabras en `dir` cuya primera es `a` y
    /// cuya ultima es `z` (ver el modulo). Un tramo vacio llego.
    pub fn llego(&mut self, dir: u64, n: usize, a: u32, z: u32) -> bool {
        n == 0 || (self.leer(dir) == a && self.leer(dir + 4 * (n as u64 - 1)) == z)
    }

    /// [`palabras`](Self::palabras) y [`llego`](Self::llego).
    pub fn tramo(&mut self, dir: u64, p: &[u32]) -> bool {
        self.palabras(dir, p);
        p.is_empty() || self.llego(dir, p.len(), p[0], p[p.len() - 1])
    }

    /// [`bytes`](Self::bytes) y [`llego`](Self::llego).
    pub fn tramo_bytes(&mut self, dir: u64, b: &[u8]) -> bool {
        self.bytes(dir, b);
        let n = b.len() / 4;
        let w = |k: usize| u32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
        n == 0 || self.llego(dir, n, w(0), w(n - 1))
    }

    /// **Un programa en su sitio** (LI2b): sus bytes, y lo que queda de su
    /// ultima pagina a cero (lo de un programa anterior mas largo no queda
    /// detras del EXIT), sin tocar las paginas que no usa.
    pub fn programa(&mut self, dir: u64, b: &[u8]) -> bool {
        let fin = dir + b.len() as u64;
        let resto = (fin.next_multiple_of(0x1000) - fin) as usize / 4;
        self.ceros(fin, resto);
        self.tramo_bytes(dir, b) && (resto == 0 || self.leer(fin + 4 * (resto as u64 - 1)) == 0)
    }

    /// Una pagina (`vram::PALABRAS`) a cero.
    pub fn pagina_a_cero(&mut self, dir: u64) -> bool {
        self.ceros(dir, crate::vram::PALABRAS);
        self.llego(dir, crate::vram::PALABRAS, 0, 0)
    }
}

impl<R: Registros> Drop for Pluma<'_, R> {
    fn drop(&mut self) {
        if self.base != self.antes {
            self.r.escribir(VENTANA_REG, self.antes);
        }
    }
}

/// **La huella de un programa**: FNV-1a de 64 bits de sus bytes y su
/// medida; nunca 0 (el 0 de [`Escritos`] es "no se sabe").
pub fn huella(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b.iter().chain(&(b.len() as u64).to_le_bytes()) {
        h = (h ^ x as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h.max(1)
}

/// **Lo que ya esta en la VRAM** de los dos programas de la tuberia: sus
/// huellas, 0 si no se sabe. Lo guarda el kernel con el cerrojo del GR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Escritos {
    pub vs: u64,
    pub ps: u64,
}

impl Escritos {
    /// Nada se sabe (otro escribio en la region, o el canal se perdio).
    pub const NADA: Self = Self { vs: 0, ps: 0 };
}

/// **Un programa, si no esta ya**: `ya` dice el que esta; se pone a 0 antes
/// de escribir (si falla a medias, no se sabe) y a su huella cuando llego.
pub fn programa_si_falta<R: Registros>(p: &mut Pluma<'_, R>, dir: u64, b: &[u8], ya: &mut u64) -> bool {
    let h = huella(b);
    if *ya == h {
        return true;
    }
    *ya = 0;
    let bien = p.programa(dir, b);
    if bien {
        *ya = h;
    }
    bien
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;

    /// La VRAM por la ventana, contando lecturas y movimientos de ventana.
    #[derive(Default)]
    struct Vram {
        m: std::collections::BTreeMap<u64, u32>,
        ventana: u32,
        lecturas: usize,
        movidas: usize,
    }
    impl Registros for Vram {
        fn leer(&mut self, reg: u32) -> u32 {
            self.lecturas += 1;
            if reg == VENTANA_REG {
                return self.ventana;
            }
            *self.m.get(&(((self.ventana as u64) << 16) + (reg - VENTANA) as u64)).unwrap_or(&0)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            if reg == VENTANA_REG {
                self.ventana = v;
                self.movidas += 1;
                return;
            }
            self.m.insert(((self.ventana as u64) << 16) + (reg - VENTANA) as u64, v);
        }
    }

    #[test]
    fn sesenta_y_cuatro_kib_con_tres_lecturas() {
        let mut r = Vram { ventana: 0x77, ..Default::default() };
        let b: std::vec::Vec<u8> = (0..64 * 1024).map(|k| (k * 13 + 1) as u8).collect();
        {
            let mut p = Pluma::nueva(&mut r);
            assert!(p.tramo_bytes(crate::vram::DATOS, &b));
        }
        assert_eq!(r.lecturas, 3, "la ventana, la primera y la ultima");
        assert_eq!((r.ventana, r.movidas), (0x77, 2), "la ventana, una vez alli y de vuelta");
        for k in 0..b.len() / 4 {
            assert_eq!(r.m[&(crate::vram::DATOS + 4 * k as u64)], u32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap()));
        }
    }

    #[test]
    fn un_tramo_que_cruza_el_mib() {
        let mut r = Vram::default();
        let dir = (1 << 20) - 8;
        {
            let mut p = Pluma::nueva(&mut r);
            assert!(p.tramo(dir, &[1, 2, 3, 4]));
        }
        assert_eq!([r.m[&dir], r.m[&(dir + 4)], r.m[&(dir + 8)], r.m[&(dir + 12)]], [1, 2, 3, 4]);
        assert_eq!(r.ventana, 0);
    }

    #[test]
    fn lo_que_no_llega_al_principio_o_al_final_no_llego() {
        // Una VRAM que pierde lo escrito en un tramo (lo de `tuberia_vram`).
        struct Sorda(Vram, core::ops::Range<u64>);
        impl Registros for Sorda {
            fn leer(&mut self, reg: u32) -> u32 {
                self.0.leer(reg)
            }
            fn escribir(&mut self, reg: u32, v: u32) {
                let dir = ((self.0.ventana as u64) << 16) + reg.wrapping_sub(VENTANA) as u64;
                if reg == VENTANA_REG || !self.1.contains(&dir) {
                    self.0.escribir(reg, v);
                }
            }
        }
        let d = crate::vram::DATOS;
        for (sorda, llega) in [(d..d + 4, false), (d + 8..d + 12, false), (d + 4..d + 8, true), (0..0, true)] {
            let mut r = Sorda(Vram::default(), sorda.clone());
            assert_eq!(Pluma::nueva(&mut r).tramo(d, &[5, 6, 7]), llega, "{sorda:x?}");
        }
    }

    #[test]
    fn el_programa_que_ya_esta_no_se_escribe() {
        let mut r = Vram::default();
        let a = [0x11u8; 640];
        let mut ya = 0;
        assert!(programa_si_falta(&mut Pluma::nueva(&mut r), crate::tuberia::VS, &a, &mut ya));
        assert_eq!(ya, huella(&a));
        let escritas = r.m.len();
        r.m.clear();
        assert!(programa_si_falta(&mut Pluma::nueva(&mut r), crate::tuberia::VS, &a, &mut ya));
        assert!(r.m.is_empty(), "el mismo: nada");
        // Otro, aunque empiece igual: entero, con su cola a cero.
        let b = [0x11u8; 644];
        assert!(programa_si_falta(&mut Pluma::nueva(&mut r), crate::tuberia::VS, &b, &mut ya));
        assert_eq!((ya, r.m.len()), (huella(&b), escritas), "{} palabras", r.m.len());
        assert_ne!(huella(&a), huella(&[0x11u8; 636]));
    }
}
