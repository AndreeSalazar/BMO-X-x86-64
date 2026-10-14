//! **F1: LAS TEXTURAS DE FLOAT EN LA 3060** (14-10, la puerta [1] del censo
//! de Cyberpunk: 13 de los 16 lotes que dibujaba la CPU) -- un `Sample` o un
//! `SampleLevel` de una 2D de floats (`Como::Flotantes4`, `Como::Flotante`)
//! con los MISMOS bits que la casa (`Plano::muestrear`).
//!
//! capa: puro -- instrucciones; la textura la presta el kernel
//!
//! [eje]     CORRECCION -- la unidad de texturas de la 3060 filtra floats con
//!           SUS cuentas (no se prometen); la casa, con pesos de 1/256 y una
//!           suma de f32 en su orden. El filtro va AQUI, con esas cuentas; a
//!           la unidad solo se le piden texeles enteros, con filtro PUNTO
//!
//! # Lo que pide cada mip `k` (lo de la casa a la derecha)
//!
//! ```text
//!    w, h       max(1, vista >> (k - vista))      `medidas_de`
//!    su, sv     u * w, v * h (FMUL)               `muestrear`
//!    PUNTO      i = suelo(su), j = suelo(sv)      `suelo(su) as i64`
//!               el texel (i, j): TEX.LZ PUNTO en su CENTRO, (i + 0.5) / w
//!    LINEAL     F = suelo((su - 0.5) * 256 + 0.5)  `fijo`, los dos redondeos
//!               i0 = suelo(F / 256), a = F - 256 i0
//!               p = ((256-a)(256-b), a(256-b), (256-a)b, ab) + 128 >> 8
//!               ((t0 + t1) + t2) + t3, tq = eq * pq   `.sum::<f32>()`
//!               (la suma de Rust empieza en -0.0: -0.0 + t0 es t0)
//!               por 1/256 (exacto: lo mismo que / 256.0)
//!    u o v NaN  ceros                             (el `return` de arriba)
//! ```
//!
//! La DIRECCION (repetir, espejo, sujetar) la hace la unidad con el texel
//! entero: el centro de `i` cae en `i` con su subtexel de 8 bits (D3D11.3
//! 7.18.6.1) y el metal ya dio IGUAL a la casa en las direcciones
//! (`gpu verrano textura`, 28-09). Un muestreador con BORDE, en la CPU
//! todavia (su color en un float no se ha medido; la puerta lo para).
//!
//! [!] Exacto mientras |u * w| y |v * h| < 2^22 texeles (el centro `i + 0.5`
//! y el F2I de 32 bits): la casa sigue en i64 mas alla. Ninguna textura de un
//! juego se muestrea a cuatro millones de vueltas.
//!
//! El camino sale de un salto UNIFORME (`@P4 BRA`, el bit 8 de la fila 1 de
//! la textura, que pone la puerta): una textura de 8 bits sigue por su TEX.
//! Cada TEX de aqui recalcula su asa DESPUES del destino del salto: R7 no
//! deja que un salto caiga entre el IMNMX que sujeta la mip y su TEX.

use alloc::vec::Vec;

use bmo_sm86::codifica::{self as c, Cmp, Fuente, Mufu, PT, RZ};

use super::planifica::Meta;
use super::{Clase, Emisor, NoEmite};

/// Los bits de la fila 1 de una textura (en la palabra del peso, por encima
/// de sus 8 bits): de float, y con filtro LINEAL.
pub const FLOTANTE: u32 = 1 << 8;
pub const LINEAL: u32 = 1 << 9;
/// Los predicados de aqui (P0 es el de la Compara fundida; P1..P3, los de
/// los UAV: no se cruzan con una lectura de textura).
const P_FLOAT: u8 = 4;
const P_LINEAL: u8 = 5;
const P_CMP: u8 = 6;

const fn f(x: f32) -> Fuente {
    Fuente::Imm(x.to_bits())
}

impl Emisor<'_> {
    /// `d = suelo(x)` (`textura::suelo`): F2I hacia cero, de vuelta, y uno
    /// menos si se paso. Exacto con |x| < 2^31.
    fn suelo_f(&mut self, d: u8, x: u8) -> Result<(), NoEmite> {
        let t = self.pedir()?;
        self.poner(c::f2i(t, x, true, 0), Clase::Mufu, Some(t), [Some(x), None, None]);
        self.poner(c::i2f(d, t, true, 0), Clase::Mufu, Some(d), [Some(t), None, None]);
        self.mufus += 2;
        self.poner_meta(c::fsetp(P_CMP, Cmp::Gt, c::r(d), c::r(x), 0), Meta { escribe_p: Some(P_CMP), ..Meta::de(Clase::Alu, None, [Some(d), Some(x), None]) });
        self.poner(c::fadd(t, c::r(d), f(-1.0), false, 0), Clase::Fma, Some(t), [Some(d), None, None]);
        self.poner_meta(c::sel(d, t, c::r(d), P_CMP, false, 0), Meta { lee_p: Some((P_CMP, false)), ..Meta::de(Clase::Alu, Some(d), [Some(t), Some(d), None]) });
        self.soltar(t);
        Ok(())
    }

    /// `d = (x + desde) * inverso`: el centro de un texel, normalizado.
    fn centro(&mut self, d: u8, x: u8, desde: f32, inverso: u8) {
        self.poner(c::fadd(d, c::r(x), f(desde), false, 0), Clase::Fma, Some(d), [Some(x), None, None]);
        self.poner(c::fmul(d, c::r(d), c::r(inverso), false, 0), Clase::Fma, Some(d), [Some(d), Some(inverso), None]);
    }

    /// `s = coordenada * max(1, de >> kr)` y `inv` = el inverso de esa
    /// medida (un RCP: solo apunta al CENTRO de un texel, ver el modulo).
    fn escala(&mut self, s: u8, inv: u8, coordenada: u8, de: u8, kr: u8) -> Result<(), NoEmite> {
        let n = self.pedir()?;
        self.poner(c::f2i(n, de, true, 0), Clase::Mufu, Some(n), [Some(de), None, None]);
        self.poner(c::shr(n, n, c::r(kr), false, 0), Clase::Alu, Some(n), [Some(n), Some(kr), None]);
        self.poner(c::imnmx(n, n, Fuente::Imm(1), true, true, 0), Clase::Alu, Some(n), [Some(n), None, None]);
        self.poner(c::i2f(n, n, true, 0), Clase::Mufu, Some(n), [Some(n), None, None]);
        self.poner(c::mufu(inv, Mufu::Rcp, n, 0), Clase::Mufu, Some(inv), [Some(n), None, None]);
        self.poner(c::fmul(s, c::r(coordenada), c::r(n), false, 0), Clase::Fma, Some(s), [Some(coordenada), Some(n), None]);
        self.mufus += 3;
        self.soltar(n);
        Ok(())
    }

    /// `(i0, a)` de `fijo(s - 0.5)` de la casa: `i0` en float, `a` (0..255)
    /// en entero. Suelta `s`.
    fn fijo(&mut self, i0: u8, a: u8, s: u8) -> Result<(), NoEmite> {
        let x = self.pedir()?;
        self.poner(c::fadd(x, c::r(s), f(-0.5), false, 0), Clase::Fma, Some(x), [Some(s), None, None]);
        self.poner(c::fmul(x, c::r(x), f(256.0), false, 0), Clase::Fma, Some(x), [Some(x), None, None]);
        self.poner(c::fadd(x, c::r(x), f(0.5), false, 0), Clase::Fma, Some(x), [Some(x), None, None]);
        // F en `s`, i0 = suelo(F / 256), y a = F - 256 i0 (exacto).
        self.suelo_f(s, x)?;
        self.poner(c::fmul(x, c::r(s), f(1.0 / 256.0), false, 0), Clase::Fma, Some(x), [Some(s), None, None]);
        self.suelo_f(i0, x)?;
        self.poner(c::ffma(x, c::r(i0), f(-256.0), c::r(s), false, 0), Clase::Fma, Some(x), [Some(i0), None, Some(s)]);
        self.poner(c::f2i(a, x, true, 0), Clase::Mufu, Some(a), [Some(x), None, None]);
        self.mufus += 1;
        self.soltar(x);
        self.soltar(s);
        Ok(())
    }

    /// **`q..q+4` = la mip `k` (la `kr` desde la de la vista) de una de
    /// floats en (u, v) = `ra`** (ver el modulo). `m1`, la fila 1. Lo de
    /// paso se suelta al acabar de usarlo: el peor momento lleva 15.
    pub(super) fn mip_flotante(&mut self, q: u8, ra: u8, base: u8, k: u8, kr: u8, m1: u8) -> Result<(), NoEmite> {
        let (su, sv, iw, ih) = (self.pedir()?, self.pedir()?, self.pedir()?, self.pedir()?);
        self.escala(su, iw, ra, m1 + 2, kr)?;
        self.escala(sv, ih, ra + 1, m1 + 3, kr)?;
        let par = self.bloque(2, false)?;
        // LINEAL o PUNTO: el filtro del muestreador (uniforme).
        let t = self.pedir()?;
        self.poner(c::lop3(t, m1 + 1, Fuente::Imm(LINEAL), c::Y, 0), Clase::Alu, Some(t), [Some(m1 + 1), None, None]);
        self.poner_meta(c::isetp(P_LINEAL, Cmp::Ne, t, c::r(RZ), false, 0), Meta { escribe_p: Some(P_LINEAL), ..Meta::de(Clase::Alu, None, [Some(t), None, None]) });
        self.soltar(t);
        let a_lineal = self.saltar(P_LINEAL);
        // PUNTO: el texel (suelo(su), suelo(sv)), en su centro.
        {
            let (fi, fj) = (self.pedir()?, self.pedir()?);
            self.suelo_f(fi, su)?;
            self.suelo_f(fj, sv)?;
            self.centro(par, fi, 0.5, iw);
            self.centro(par + 1, fj, 0.5, ih);
            self.soltar(fi);
            self.soltar(fj);
            let mut v = Vec::new();
            let asa = self.asa_de_mip(base, k, &mut v)?;
            self.leer_lz(q, par, asa, false);
            for x in v {
                self.soltar(x);
            }
        }
        let al_final = self.saltar(PT);
        // LINEAL: `fijo` en x y en y, los pesos de la casa y la suma.
        let destino = self.codigo.len();
        self.parchear(a_lineal, destino);
        let (i0, j0, a, b) = (self.pedir()?, self.pedir()?, self.pedir()?, self.pedir()?);
        self.fijo(i0, a, su)?;
        self.fijo(j0, b, sv)?;
        // Los pesos: (x + 128) >> 8 de cada producto, en float.
        let mut pesos = [0u8; 4];
        {
            // 256 - a = (a ^ 255) + 1, con a en 0..255.
            let (na, nb) = (self.pedir()?, self.pedir()?);
            for (n, x) in [(na, a), (nb, b)] {
                self.poner(c::lop3(n, x, Fuente::Imm(255), c::OX, 0), Clase::Alu, Some(n), [Some(x), None, None]);
                self.poner(c::iadd3(n, n, Fuente::Imm(1), 0), Clase::Alu, Some(n), [Some(n), None, None]);
            }
            for (p, (x, y)) in pesos.iter_mut().zip([(na, nb), (a, nb), (na, b), (a, b)]) {
                *p = self.pedir()?;
                self.poner(c::imad(*p, x, c::r(y), RZ, 0), Clase::Fma, Some(*p), [Some(x), Some(y), None]);
                self.poner(c::iadd3(*p, *p, Fuente::Imm(128), 0), Clase::Alu, Some(*p), [Some(*p), None, None]);
                self.poner(c::shr(*p, *p, Fuente::Imm(8), false, 0), Clase::Alu, Some(*p), [Some(*p), None, None]);
                self.poner(c::i2f(*p, *p, true, 0), Clase::Mufu, Some(*p), [Some(*p), None, None]);
                self.mufus += 1;
            }
            for x in [na, nb, a, b] {
                self.soltar(x);
            }
        }
        let mut v = Vec::new();
        let asa = self.asa_de_mip(base, k, &mut v)?;
        let e = self.bloque(4, false)?;
        for (n, (dx, dy)) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].into_iter().enumerate() {
            self.centro(par, i0, dx + 0.5, iw);
            self.centro(par + 1, j0, dy + 0.5, ih);
            self.leer_lz(e, par, asa, false);
            for j in 0..4u8 {
                let (x, y) = (q + j, e + j);
                if n == 0 {
                    self.poner(c::fmul(x, c::r(y), c::r(pesos[0]), false, 0), Clase::Fma, Some(x), [Some(y), Some(pesos[0]), None]);
                } else {
                    self.poner(c::fmul(y, c::r(y), c::r(pesos[n]), false, 0), Clase::Fma, Some(y), [Some(y), Some(pesos[n]), None]);
                    self.poner(c::fadd(x, c::r(x), c::r(y), false, 0), Clase::Fma, Some(x), [Some(x), Some(y), None]);
                }
            }
        }
        for j in 0..4u8 {
            let x = q + j;
            self.poner(c::fmul(x, c::r(x), f(1.0 / 256.0), false, 0), Clase::Fma, Some(x), [Some(x), None, None]);
        }
        for x in v.into_iter().chain(pesos).chain([i0, j0, iw, ih, par, par + 1]).chain(e..e + 4) {
            self.soltar(x);
        }
        let destino = self.codigo.len();
        self.parchear(al_final, destino);
        // u o v NaN: ceros, como la casa.
        for x in [ra, ra + 1] {
            self.poner_meta(c::fsetp(P_CMP, Cmp::Neu, c::r(x), c::r(x), 0), Meta { escribe_p: Some(P_CMP), ..Meta::de(Clase::Alu, None, [Some(x), None, None]) });
            for j in 0..4u8 {
                let y = q + j;
                self.poner_meta(c::sel(y, y, c::r(RZ), P_CMP, true, 0), Meta { lee_p: Some((P_CMP, false)), ..Meta::de(Clase::Alu, Some(y), [Some(y), None, None]) });
            }
        }
        Ok(())
    }

    /// El salto a lo de float si la textura lo es (`@P4 BRA`); devuelve el
    /// salto, que se parchea donde empieza ese camino.
    pub(super) fn si_flotante(&mut self, m1: u8, paso: &mut Vec<u8>) -> Result<usize, NoEmite> {
        let t = self.pedir()?;
        paso.push(t);
        self.poner(c::lop3(t, m1 + 1, Fuente::Imm(FLOTANTE), c::Y, 0), Clase::Alu, Some(t), [Some(m1 + 1), None, None]);
        self.poner_meta(c::isetp(P_FLOAT, Cmp::Ne, t, c::r(RZ), false, 0), Meta { escribe_p: Some(P_FLOAT), ..Meta::de(Clase::Alu, None, [Some(t), None, None]) });
        Ok(self.saltar(P_FLOAT))
    }
}
