//! **La division de enteros** (E6d, 02-10): `a / b` y `a % b`, con signo o
//! sin el, como `ptxas` (`ga10x/sombreadores/oro_division.ptx`) pero SIN
//! guardas -- en el cuerpo de una app solo un BRA lleva guarda (R7) --:
//! cada `@P0` es un SEL.
//!
//! ```text
//!    e ~ 2^32 / b, por abajo   I2F.U32.RP, MUFU.RCP, - 2 ulp, F2I.FTZ
//!    una vuelta de Newton      e += hi(e * (-b * e))
//!    q = hi(e * a)             r = a - q * b; q se queda corto en 2
//!                              como mucho: dos veces "si r >= b" (el
//!                              acarreo de `r - b`: IADD3 a P0, IADD3.X)
//!    con signo                 lo mismo con |a| y |b| (IABS); el
//!                              cociente, negado si a y b tienen signos
//!                              distintos, y el resto con el de a
//!    b = 0                     0xFFFFFFFF (lo de D3D; la 3060 pone ~b)
//! ```
//!
//! Lo que no da exacto el MUFU.RCP lo corrige la vuelta: la cuenta vale con
//! el inverso hasta un ULP por arriba (lo que se equivoca la 3060) y muchos
//! por abajo (`la_division_aguanta_un_inverso_aproximado`).
//!
//! # Lo que se ahorra (E6d, segunda parte)
//!
//! ```text
//!    LA PAREJA     `a / b` y `a % b` del mismo par en el mismo tramo recto
//!                  (lo que escribe `dxc` para `x / n` y `x % n`): UNA
//!                  cuenta, las dos salidas
//!    UNA CONSTANTE `b` que no cambia nunca: ni inverso ni comprobar el 0.
//!                  Potencia de 2, un desplazamiento; si no, la
//!                  multiplicacion "magica" de Granlund y Montgomery (la que
//!                  hace NVVM antes del PTX: `ptxas` no la hace, usa el
//!                  inverso tambien con una constante)
//! ```

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Op, OpEntera, Reg};
use bmo_sm86::codifica::{self as c, Cmp, Fuente, Mufu, RZ};

use super::planifica::Meta;
use super::saltos::{escritos, leidos};
use super::{Clase, Emisor, NoEmite, Valor};

/// Si `op` es una division o un resto.
pub(crate) fn es_division(op: OpEntera) -> bool {
    matches!(op, OpEntera::DivU | OpEntera::RemU | OpEntera::DivS | OpEntera::RemS)
}

/// **La pareja** de la division `i`: la operacion `j > i` del MISMO tramo
/// recto (sin un si ni un bucle por medio) que pide lo otro (el resto si `i`
/// es el cociente, y al reves) del mismo par, con nada entre medias que
/// escriba `a` o `b`, ni que lea o escriba el destino de `j` -- que se
/// escribe ya en `i` --. Que `i` lea ese destino (`q = x / n; x = x % n`)
/// si vale: `dividir` escribe al final.
pub(crate) fn pareja(ops: &[Op], i: usize) -> Option<usize> {
    let Op::Entera { d, a, b, op } = *ops.get(i)? else { return None };
    let otra = match op {
        OpEntera::DivU => OpEntera::RemU,
        OpEntera::RemU => OpEntera::DivU,
        OpEntera::DivS => OpEntera::RemS,
        OpEntera::RemS => OpEntera::DivS,
        _ => return None,
    };
    if d == a || d == b {
        return None;
    }
    let mut j = i + 1;
    let dj = loop {
        let o = ops.get(j)?;
        match *o {
            Op::Entera { d: dj, a: aj, b: bj, op: oj } if oj == otra && (aj, bj) == (a, b) && dj != d => break dj,
            Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle => return None,
            _ => {}
        }
        if escritos(o).0.iter().flatten().any(|&w| w == a || w == b) {
            return None;
        }
        j += 1;
    };
    let toca = |o: &Op| leidos(o).iter().flatten().chain(escritos(o).0.iter().flatten()).any(|&r| r == dj);
    if ops[i + 1..j].iter().any(toca) {
        return None;
    }
    Some(j)
}

/// **La magia** para dividir un `u32` entre la constante `n` (ni 0 ni
/// potencia de 2): `(m, s, suma)`.
///
/// ```text
///    sin suma   q = hi(a * m) >> s           (si hay un m de 32 bits)
///    con suma   t = hi(a * m)                (m de 33 bits: 2^32 + m)
///               q = (((a - t) >> 1) + t) >> s
/// ```
///
/// Sin suma: `m = techo(2^p / n)` con `p = 32 + s` vale si `m * n - 2^p <=
/// 2^s` (el error de `a * m / 2^p` no llega a `1 / n` con `a < 2^32`); con
/// suma, la de Granlund y Montgomery (1994, figura 4.1) con `l = techo(log2
/// n)`: `m = suelo(2^32 (2^l - n) / n) + 1` y `s = l - 1`.
pub(crate) fn magia(n: u32) -> (u32, u32, bool) {
    let l = 32 - (n - 1).leading_zeros();
    for s in 0..l {
        let p = 32 + s;
        let m = ((1u128 << p) + n as u128 - 1) / n as u128;
        if m < 1 << 32 && m * n as u128 - (1u128 << p) <= 1u128 << s {
            return (m as u32, s, false);
        }
    }
    let m = ((1u64 << 32) * ((1u64 << l) - n as u64)) / n as u64 + 1;
    (m as u32, l - 1, true)
}

impl Emisor<'_> {
    /// Un temporal de esta operacion (se devuelve al acabarla).
    fn temporal(&mut self, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let t = self.pedir()?;
        paso.push(t);
        Ok(t)
    }

    /// `ISETP P0, a, b`.
    fn a_p0(&mut self, como: Cmp, a: u8, b: Fuente, sin_signo: bool) {
        let lee_b = if let Fuente::R { r, .. } = b { Some(r) } else { None };
        self.poner_meta(c::isetp(0, como, a, b, sin_signo, 0), Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, None, [Some(a), lee_b, None]) });
    }

    /// `SEL x, a, b, P0`: `a` si P0, `b` si no.
    fn segun_p0(&mut self, x: u8, a: u8, b: Fuente) {
        let lee_b = if let Fuente::R { r, .. } = b { Some(r) } else { None };
        self.poner_meta(c::sel(x, a, b, 0, false, 0), Meta { lee_p: Some((0, false)), ..Meta::de(Clase::Alu, Some(x), [Some(a), lee_b, None]) });
    }

    /// `x = -a` (de enteros).
    fn negar(&mut self, x: u8, a: u8) {
        self.poner(c::iadd3(x, RZ, c::neg(a), 0), Clase::Alu, Some(x), [Some(a), None, None]);
    }

    /// `x = |a|` si `con_signo`; si no, `a` tal cual.
    fn sin_signo(&mut self, a: u8, con_signo: bool, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        if !con_signo {
            return Ok(a);
        }
        let x = self.temporal(paso)?;
        self.poner(c::iabs(x, a, 0), Clase::Alu, Some(x), [Some(a), None, None]);
        Ok(x)
    }

    /// **Dividir** `a` entre `b`: el cociente a `cociente`, el resto a
    /// `resto` (uno de los dos, o los dos: la pareja). Los resultados van a
    /// sus registros AL FINAL, o cuando ya nada lee `a`: un destino puede
    /// ser `a` (una variable, `x = x / 3`).
    pub(crate) fn dividir(&mut self, a: Reg, b: Reg, con_signo: bool, cociente: Option<Reg>, resto: Option<Reg>, i: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        if let Valor::Imm(n) = self.valor(b) {
            return self.dividir_por(a, n, con_signo, cociente, resto, i, paso);
        }
        let (ra, rb) = (self.registro(a, paso)?, self.registro(b, paso)?);
        let ua = self.sin_signo(ra, con_signo, paso)?;
        let ub = self.sin_signo(rb, con_signo, paso)?;
        let [e, nb, q, r, k] = [(); 5].map(|_| self.pedir());
        let [e, nb, q, r, k] = [e?, nb?, q?, r?, k?];
        paso.extend([e, nb, q, r, k]);
        // El inverso, por abajo.
        self.poner(c::i2f_arriba(e, ub, false, 0), Clase::Mufu, Some(e), [Some(ub), None, None]);
        self.negar(nb, ub);
        self.poner(c::mufu(e, Mufu::Rcp, e, 0), Clase::Mufu, Some(e), [Some(e), None, None]);
        self.poner(c::iadd3(e, e, Fuente::Imm(0x0fff_fffe), 0), Clase::Alu, Some(e), [Some(e), None, None]);
        self.poner(c::f2i_ftz(e, e, false, 0), Clase::Mufu, Some(e), [Some(e), None, None]);
        self.mufus += 3;
        // La vuelta de Newton.
        self.poner(c::imad(k, nb, c::r(e), RZ, 0), Clase::Fma, Some(k), [Some(nb), Some(e), None]);
        self.poner(c::imad_hi(k, e, c::r(k), 0), Clase::Ancha, Some(k), [Some(e), Some(k), None]);
        self.poner(c::iadd3(e, e, c::r(k), 0), Clase::Alu, Some(e), [Some(e), Some(k), None]);
        // El cociente y el resto, y las dos correcciones: `k = r - b` con su
        // ACARREO a P0 (que es `r >= b`), `r = P0 ? k : r` y `q += P0`
        // (IADD3.X): tres instrucciones, sin ISETP. Solo lo que se pide: el
        // resto no necesita el cociente, y la segunda vez el cociente solo
        // el acarreo.
        self.poner(c::imad_hi(q, e, c::r(ua), 0), Clase::Ancha, Some(q), [Some(e), Some(ua), None]);
        self.poner(c::imad(r, nb, c::r(q), ua, 0), Clase::Fma, Some(r), [Some(nb), Some(q), Some(ua)]);
        for vuelta in 0..2 {
            let con_r = vuelta == 0 || resto.is_some();
            let dk = if con_r { k } else { RZ };
            let w = c::iadd3_acarreo(dk, 0, r, c::neg(ub), 0);
            self.poner_meta(w, Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, con_r.then_some(k), [Some(r), Some(ub), None]) });
            if con_r {
                self.segun_p0(r, k, c::r(r));
            }
            if cociente.is_some() {
                self.poner_meta(c::iadd3_x(q, q, c::r(RZ), 0, 0), Meta { lee_p: Some((0, false)), ..Meta::de(Clase::Alu, Some(q), [Some(q), None, None]) });
            }
        }
        // Los signos: el cociente, si a ^ b < 0; el resto, si a < 0.
        if con_signo {
            if cociente.is_some() {
                self.poner(c::lop3(k, ra, c::r(rb), c::OX, 0), Clase::Alu, Some(k), [Some(ra), Some(rb), None]);
                self.a_p0(Cmp::Lt, k, c::r(RZ), false);
                self.negar(nb, q);
                self.segun_p0(q, nb, c::r(q));
            }
            if resto.is_some() {
                self.a_p0(Cmp::Lt, ra, c::r(RZ), false);
                self.negar(nb, r);
                self.segun_p0(r, nb, c::r(r));
            }
        }
        // Entre 0: todo unos. Y a sus registros.
        self.a_p0(Cmp::Ne, rb, c::r(RZ), true);
        for (d, v) in [(cociente, q), (resto, r)] {
            if let Some(d) = d {
                let x = self.destino(d, i)?;
                self.segun_p0(x, v, Fuente::Imm(u32::MAX));
            }
        }
        self.p0 = None;
        Ok(())
    }

    /// Entre la CONSTANTE `n` (los bits: con signo, un i32).
    fn dividir_por(&mut self, a: Reg, n: u32, con_signo: bool, cociente: Option<Reg>, resto: Option<Reg>, i: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        // Entre 0: todo unos, sin mirar `a`.
        if n == 0 {
            for d in [cociente, resto].into_iter().flatten() {
                let x = self.destino(d, i)?;
                self.poner(c::mov(x, Fuente::Imm(u32::MAX), 0), Clase::Alu, Some(x), [None; 3]);
            }
            return Ok(());
        }
        let ra = self.registro(a, paso)?;
        let negativo = con_signo && (n as i32) < 0;
        let m = if con_signo { (n as i32).unsigned_abs() } else { n };
        // El cociente va directo a su registro, salvo si ese es el de `a` y
        // falta el resto (que lee `a`): entonces a un temporal y un MOV.
        let xq = match cociente {
            Some(d) => Some(self.destino(d, i)?),
            None => None,
        };
        let qd = match xq {
            Some(x) if !(x == ra && resto.is_some()) => x,
            _ => self.temporal(paso)?,
        };
        let ua = self.sin_signo(ra, con_signo, paso)?;
        // El cociente sin signo: en `qd` si no hay signo que poner.
        let uq = if con_signo { self.temporal(paso)? } else { qd };
        if m == 1 {
            self.poner(c::mov(uq, c::r(ua), 0), Clase::Alu, Some(uq), [Some(ua), None, None]);
        } else if m.is_power_of_two() {
            self.poner(c::shr(uq, ua, Fuente::Imm(m.trailing_zeros()), false, 0), Clase::Alu, Some(uq), [Some(ua), None, None]);
        } else {
            let (mm, s, suma) = magia(m);
            let t = self.temporal(paso)?;
            self.poner(c::imad_hi(t, ua, Fuente::Imm(mm), 0), Clase::Ancha, Some(t), [Some(ua), None, None]);
            if suma {
                let u = self.temporal(paso)?;
                self.poner(c::iadd3(u, ua, c::neg(t), 0), Clase::Alu, Some(u), [Some(ua), Some(t), None]);
                self.poner(c::shr(u, u, Fuente::Imm(1), false, 0), Clase::Alu, Some(u), [Some(u), None, None]);
                self.poner(c::iadd3(u, u, c::r(t), 0), Clase::Alu, Some(u), [Some(u), Some(t), None]);
                self.poner(c::shr(uq, u, Fuente::Imm(s), false, 0), Clase::Alu, Some(uq), [Some(u), None, None]);
            } else if s > 0 {
                self.poner(c::shr(uq, t, Fuente::Imm(s), false, 0), Clase::Alu, Some(uq), [Some(t), None, None]);
            } else {
                self.poner(c::mov(uq, c::r(t), 0), Clase::Alu, Some(uq), [Some(t), None, None]);
            }
        }
        // Con signo: negado si a < 0, y otra vez si n < 0.
        if con_signo {
            let nq = self.temporal(paso)?;
            self.negar(nq, uq);
            self.a_p0(Cmp::Lt, ra, c::r(RZ), false);
            let (si, no) = if negativo { (uq, nq) } else { (nq, uq) };
            self.segun_p0(qd, si, c::r(no));
        }
        // El resto: a - q * n, de una IMAD (con o sin signo, el mismo).
        if let Some(d) = resto {
            let x = self.destino(d, i)?;
            self.poner(c::imad(x, qd, Fuente::Imm(n.wrapping_neg()), ra, 0), Clase::Fma, Some(x), [Some(qd), Some(ra), None]);
        }
        if let Some(x) = xq.filter(|&x| x != qd) {
            self.poner(c::mov(x, c::r(qd), 0), Clase::Alu, Some(x), [Some(qd), None, None]);
        }
        self.p0 = None;
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::magia;

    /// La magia, contra la division de Rust: cada divisor chico, y los
    /// grandes al azar, con los `a` que mas cuestan (los multiplos y sus
    /// vecinos, y los de arriba del todo).
    #[test]
    fn la_magia_divide() {
        let mut x = 0x0123_4567_89ab_cdefu64;
        let mut azar = || {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (x >> 32) as u32
        };
        let divisores: alloc::vec::Vec<u32> = (3..3000).chain((0..3000).map(|_| azar() >> (azar() % 32))).chain([0x7FFF_FFFF, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 641, 6700417]).filter(|n| *n > 2 && !n.is_power_of_two()).collect();
        for n in divisores {
            let (m, s, suma) = magia(n);
            let q = |a: u32| {
                let t = ((a as u64 * m as u64) >> 32) as u32;
                if suma {
                    (((a - t) >> 1) + t) >> s
                } else {
                    t >> s
                }
            };
            let k = u32::MAX / n;
            let mut prueba = [0, 1, n - 1, n, n.wrapping_add(1), u32::MAX, u32::MAX - 1, k.wrapping_mul(n), k.wrapping_mul(n).wrapping_sub(1)].to_vec();
            for _ in 0..40 {
                let v = azar();
                prueba.extend([v, (v / n).wrapping_mul(n), (v / n).wrapping_mul(n).wrapping_sub(1)]);
            }
            for a in prueba {
                assert_eq!(q(a), a / n, "{a} / {n}: m {m:#x} s {s} suma {suma}");
            }
        }
    }
}
