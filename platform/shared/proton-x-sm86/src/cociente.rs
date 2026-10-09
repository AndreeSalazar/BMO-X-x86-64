//! **LA DIVISION EXACTA DE f32** (DL10 de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 09-10, del propietario: *"Exactos, tambien en la 3060"*): `a / b` con los
//! bits de IEEE -- los de la casa (`x / y` de Rust), redondeado al mas
//! cercano, con subnormales, ceros con signo, infinitos y NaN --.
//!
//! Es la cuenta de `ptxas` para `div.rn.f32` (`bmo-sm86/oro_reales.ptx`),
//! rehecha con la lista blanca del cuerpo de una app (R7 del juez): sin
//! FCHK, sin el CALL a su camino lento y sin guardas -- cada `@P` es un BRA
//! hacia delante o un SEL --.
//!
//! ```text
//!    la ventana   |a| y |b| entre 2^-50 y 2^50: el inverso, el cociente, su
//!                 residuo y su medio ULP, todos normales
//!    la cuenta    sobre |a| y |b| (el signo, al final): r = MUFU.RCP(b); un
//!                 paso de Newton, r += r (1 - b r); q = a r; una correccion
//!                 con su residuo EXACTO (la FFMA hace a - b q sin redondear
//!                 el producto): q queda a menos de un ULP del cociente
//!    el redondeo  la prueba de TUCKERMAN, con residuos exactos: si a - b (q
//!                 + medio ULP) > 0, el bueno es el de arriba; si a - b (q -
//!                 medio ULP de abajo) < 0, el de abajo; si no, q. No depende
//!                 de cuanto se equivoque el MUFU (la cuenta de `ptxas`, si:
//!                 con un inverso a medio ULP mal redondeado, su empate cae
//!                 del lado malo -- 1 / 1.9999999 lo encontro la prueba)
//!    lo lento     fuera de la ventana, DENTRO del cuerpo:
//!                   NaN, infinitos y ceros, como IEEE
//!                   un subnormal, por 2^64 antes de seguir (exacto)
//!                   la misma cuenta sobre |a| y |b| llevados a [1, 2), y
//!                   el exponente despues: si se pasa, infinito; si se
//!                   queda corto, el cociente SUBNORMAL con su redondeo --
//!                   de q y del signo de su residuo: sus bits truncados y si
//!                   era exacto; el empate, al par
//! ```
//!
//! La cuenta es UNA para los dos caminos: lo lento lleva |a| y |b| a [1, 2)
//! y vuelve a ella con un salto hacia atras (cada salto drena: quien llega
//! lo encuentra todo hecho, `planifica.rs`).
//!
//! Cuesta instrucciones, no exactitud: ~105 de codigo, y al correr 36 en la
//! ventana y hasta 88 fuera (`lo_que_cuesta_una_division`). Y aguanta el
//! error del MUFU.RCP de la 3060: la prueba lo mueve (`Maquina::inverso_ulp`,
//! `pruebas_cociente.rs`).

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::Reg;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, Mufu, PT, RZ};

use super::planifica::Meta;
use super::{reg_de, Clase, Emisor, NoEmite};

/// La ventana de lo rapido, en los bits sin signo: de 2^-50 (0x2680_0000)
/// a 2^50 (0x5880_0000), sin llegar.
const VENTANA_DESDE: u32 = 0x2680_0000;
const VENTANA: u32 = 0x5880_0000 - VENTANA_DESDE;
/// 2^64: lleva un subnormal a los normales sin redondear.
const DOS_64: u32 = 0x5F80_0000;
const UNO: u32 = 0x3F80_0000;
const INFINITO: u32 = 0x7F80_0000;
/// El NaN de la 3060 (la carga de un NaN no se promete).
const NAN: u32 = 0x7FFF_FFFF;

/// Los registros de la cuenta, pedidos una vez para los dos caminos.
struct Cuenta {
    r0: u8,
    e1: u8,
    r1: u8,
    q0: u8,
    resto: u8,
    q1: u8,
    /// El cociente, redondeado (sin signo).
    q: u8,
}

impl Emisor<'_> {
    /// `n` temporales de esta operacion (se devuelven al acabarla).
    fn temporales<const N: usize>(&mut self, paso: &mut Vec<u8>) -> Result<[u8; N], NoEmite> {
        let mut t = [0u8; N];
        for x in t.iter_mut() {
            *x = self.pedir()?;
            paso.push(*x);
        }
        Ok(t)
    }

    /// Una de enteros de dos fuentes, en la ALU.
    fn alu(&mut self, w: (u64, u64), x: u8, a: u8, b: Fuente) {
        self.poner(w, Clase::Alu, Some(x), [Some(a), reg_de(b), None]);
    }

    /// `x = a & m`, `x = a | b`.
    fn y_con(&mut self, x: u8, a: u8, m: u32) {
        self.alu(c::lop3(x, a, Fuente::Imm(m), c::Y, 0), x, a, Fuente::Imm(m));
    }

    fn o_con(&mut self, x: u8, a: u8, b: Fuente) {
        self.alu(c::lop3(x, a, b, c::O, 0), x, a, b);
    }

    /// `x = a + b` (enteros, modulo 2^32).
    fn mas(&mut self, x: u8, a: u8, b: Fuente) {
        self.alu(c::iadd3(x, a, b, 0), x, a, b);
    }

    /// `P0 = a <como> b`, de enteros.
    fn pregunta(&mut self, como: Cmp, a: u8, b: Fuente, sin_signo: bool) {
        self.poner_meta(c::isetp(0, como, a, b, sin_signo, 0), Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, None, [Some(a), reg_de(b), None]) });
    }

    /// `P0 = a <como> b`, de floats.
    fn pregunta_real(&mut self, como: Cmp, a: u8, b: Fuente) {
        self.poner_meta(c::fsetp(0, como, c::r(a), b, 0), Meta { escribe_p: Some(0), ..Meta::de(Clase::Alu, None, [Some(a), reg_de(b), None]) });
    }

    /// `x = P0 ? a : b` (o `!P0`, con `negado`).
    fn elige_p0(&mut self, x: u8, a: u8, b: Fuente, negado: bool) {
        self.poner_meta(c::sel(x, a, b, 0, negado, 0), Meta { lee_p: Some((0, false)), ..Meta::de(Clase::Alu, Some(x), [Some(a), reg_de(b), None]) });
    }

    /// `x = a * b + c`, con UN redondeo, al mas cercano.
    fn fma(&mut self, x: u8, a: Fuente, b: Fuente, cc: u8) {
        self.poner(c::ffma(x, a, b, c::r(cc), false, 0), Clase::Fma, Some(x), [reg_de(a), reg_de(b), Some(cc)]);
    }

    /// **La cuenta** de `a / b`, los dos POSITIVOS (registros), al cociente
    /// redondeado al mas cercano en `k.q`: el inverso del MUFU con un paso de
    /// Newton, una correccion, y la prueba de Tuckerman (ver la cabecera).
    fn nucleo(&mut self, a: u8, b: u8, uno: u8, k: &Cuenta) {
        self.poner(c::mufu(k.r0, Mufu::Rcp, b, 0), Clase::Mufu, Some(k.r0), [Some(b), None, None]);
        self.mufus += 1;
        self.fma(k.e1, c::neg(b), c::r(k.r0), uno);
        self.fma(k.r1, c::r(k.r0), c::r(k.e1), k.r0);
        self.poner(c::fmul(k.q0, c::r(a), c::r(k.r1), false, 0), Clase::Fma, Some(k.q0), [Some(a), Some(k.r1), None]);
        self.fma(k.resto, c::neg(b), c::r(k.q0), a);
        self.fma(k.q1, c::r(k.r1), c::r(k.resto), k.q0);
        // El residuo exacto de q1; su medio ULP (h) y el de debajo (h, o h/2
        // si q1 es una potencia de dos).
        self.fma(k.resto, c::neg(b), c::r(k.q1), a);
        let (h, m, hb, d) = (k.r1, k.e1, k.q0, k.r0);
        self.y_con(h, k.q1, 0x7F80_0000);
        self.mas(h, h, Fuente::Imm((24u32 << 23).wrapping_neg()));
        self.y_con(m, k.q1, 0x007F_FFFF);
        self.pregunta(Cmp::Eq, m, Fuente::Imm(0), true);
        self.mas(hb, h, Fuente::Imm((1u32 << 23).wrapping_neg()));
        self.elige_p0(hb, hb, c::r(h), false);
        // Arriba si a - b (q1 + h) > 0; abajo si a - b (q1 - hb) < 0.
        self.fma(d, c::neg(b), c::r(h), k.resto);
        self.pregunta_real(Cmp::Gt, d, Fuente::Imm(0));
        self.mas(m, k.q1, Fuente::Imm(1));
        self.elige_p0(k.q, m, c::r(k.q1), false);
        self.fma(d, c::r(b), c::r(hb), k.resto);
        self.pregunta_real(Cmp::Lt, d, Fuente::Imm(0));
        self.mas(m, k.q1, Fuente::Imm(u32::MAX));
        self.elige_p0(k.q, m, c::r(k.q), false);
    }

    /// **`d = a / b`**, exacto (ver la cabecera). `d` se escribe al final de
    /// cada camino: puede ser `a` o `b` (una variable, `x = x / y`).
    pub(crate) fn cociente(&mut self, d: Reg, a: Reg, b: Reg, i: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let (ra, rb) = (self.registro(a, paso)?, self.registro(b, paso)?);
        let x = self.destino(d, i)?;
        let [aa, bb, s, t, u, uno, e] = self.temporales(paso)?;
        let [r0, e1, r1, q0, resto, q1, q] = self.temporales(paso)?;
        let k = Cuenta { r0, e1, r1, q0, resto, q1, q };
        // |a|, |b|, el signo del cociente, y 1.0 para el paso de Newton.
        self.y_con(aa, ra, 0x7FFF_FFFF);
        self.y_con(bb, rb, 0x7FFF_FFFF);
        self.alu(c::lop3(s, ra, c::r(rb), c::OX, 0), s, ra, c::r(rb));
        self.y_con(s, s, 0x8000_0000);
        self.poner(c::mov(uno, Fuente::Imm(UNO), 0), Clase::Alu, Some(uno), [None; 3]);
        // ** LA VENTANA: las dos en [2^-50, 2^50), o a lo lento. `t` lo
        // recuerda hasta despues de la cuenta.
        self.mas(t, aa, Fuente::Imm(VENTANA_DESDE.wrapping_neg()));
        self.mas(u, bb, Fuente::Imm(VENTANA_DESDE.wrapping_neg()));
        self.alu(c::imnmx(t, t, c::r(u), true, false, 0), t, t, c::r(u));
        self.pregunta(Cmp::Ge, t, Fuente::Imm(VENTANA), true);
        let lento = self.saltar(0);
        // ** LA CUENTA, UNA para los dos caminos: sobre `aa` y `bb` -- |a| y
        // |b| en lo rapido; llevados a [1, 2) en lo lento, que vuelve aqui
        // (un salto hacia atras) con `t` fuera de la ventana.
        let cuenta = self.codigo.len();
        self.nucleo(aa, bb, uno, &k);
        self.pregunta(Cmp::Ge, t, Fuente::Imm(VENTANA), true);
        let tras_lo_lento = self.saltar(0);
        // Lo rapido: el signo, y ya.
        self.o_con(x, q, c::r(s));
        let mut al_final = alloc::vec![self.saltar(PT)];
        // ** LO LENTO.
        let aqui = self.codigo.len();
        self.parchear(lento, aqui);
        // NaN, infinitos y ceros: aparte (|x| - 1 >= 0x7F7FFFFF sin signo, en
        // cualquiera de los dos).
        self.mas(t, aa, Fuente::Imm(u32::MAX));
        self.mas(u, bb, Fuente::Imm(u32::MAX));
        self.alu(c::imnmx(t, t, c::r(u), true, false, 0), t, t, c::r(u));
        self.pregunta(Cmp::Ge, t, Fuente::Imm(0x7F7F_FFFF), true);
        let especial = self.saltar(0);
        // Un subnormal, por 2^64 (exacto); su exponente lo descuenta.
        for (v, sesgo, en) in [(aa, (-64i32) as u32, e), (bb, 64, u)] {
            self.pregunta(Cmp::Lt, v, Fuente::Imm(0x0080_0000), true);
            self.poner(c::fmul(t, c::r(v), Fuente::Imm(DOS_64), false, 0), Clase::Fma, Some(t), [Some(v), None, None]);
            self.elige_p0(v, t, c::r(v), false);
            self.elige_p0(en, RZ, Fuente::Imm(sesgo), true);
        }
        self.mas(e, e, c::r(u));
        // Los exponentes (e = ea - eb + lo descontado), y a y b a [1, 2), en
        // su sitio: los lee la cuenta.
        self.alu(c::shr(t, aa, Fuente::Imm(23), false, 0), t, aa, Fuente::Imm(23));
        self.alu(c::shr(u, bb, Fuente::Imm(23), false, 0), u, bb, Fuente::Imm(23));
        self.mas(e, e, c::r(t));
        self.mas(e, e, c::neg(u));
        for v in [aa, bb] {
            self.y_con(v, v, 0x007F_FFFF);
            self.o_con(v, v, Fuente::Imm(UNO));
        }
        // A la cuenta, con `t` fuera de la ventana: vuelve a lo de abajo.
        self.poner(c::mov(t, Fuente::Imm(u32::MAX), 0), Clase::Alu, Some(t), [None; 3]);
        let atras = self.saltar(PT);
        self.parchear(atras, cuenta);
        // ** TRAS LA CUENTA LENTA: el campo de exponente del cociente, dentro
        // de [1, 254]: normal.
        let aqui = self.codigo.len();
        self.parchear(tras_lo_lento, aqui);
        let (an, bn) = (aa, bb);
        self.alu(c::shr(t, q, Fuente::Imm(23), false, 0), t, q, Fuente::Imm(23));
        self.mas(t, t, c::r(e));
        self.mas(u, t, Fuente::Imm(u32::MAX));
        self.pregunta(Cmp::Ge, u, Fuente::Imm(254), true);
        let fuera = self.saltar(0);
        self.poner(c::imad(u, e, Fuente::Imm(0x80_0000), q, 0), Clase::Fma, Some(u), [Some(e), Some(q), None]);
        self.o_con(x, u, c::r(s));
        al_final.push(self.saltar(PT));
        // Por arriba: infinito.
        let aqui = self.codigo.len();
        self.parchear(fuera, aqui);
        self.pregunta(Cmp::Gt, t, Fuente::Imm(254), false);
        let corto = self.saltar(8);
        self.o_con(x, s, Fuente::Imm(INFINITO));
        al_final.push(self.saltar(PT));
        // ** Por abajo: el SUBNORMAL. De q y del signo de su residuo exacto:
        // sus bits truncados (q, o el de debajo si el cociente se queda por
        // debajo de q) y si era exacto.
        let aqui = self.codigo.len();
        self.parchear(corto, aqui);
        let (qz, inexacto, corre, vuelve, m, kk, l, sube) = (q0, r0, r1, q1, aa, t, u, bb);
        self.fma(resto, c::neg(bn), c::r(q), an);
        self.pregunta_real(Cmp::Lt, resto, Fuente::Imm(0));
        self.mas(e1, q, Fuente::Imm(u32::MAX));
        self.elige_p0(qz, e1, c::r(q), false);
        self.pregunta_real(Cmp::Ne, resto, Fuente::Imm(0));
        self.elige_p0(inexacto, RZ, Fuente::Imm(1), true);
        // Su campo, el de qz: se corre 1 - campo (de 1 en adelante), y lo
        // que se pierde, a la izquierda (31 + campo).
        self.alu(c::shr(corre, qz, Fuente::Imm(23), false, 0), corre, qz, Fuente::Imm(23));
        self.mas(corre, corre, c::r(e));
        self.mas(vuelve, corre, Fuente::Imm(31));
        self.mas(corre, corre, Fuente::Imm(u32::MAX));
        self.mas(corre, RZ, c::neg(corre));
        self.y_con(m, qz, 0x007F_FFFF);
        self.o_con(m, m, Fuente::Imm(0x0080_0000));
        self.alu(c::shr(kk, m, c::r(corre), false, 0), kk, m, c::r(corre));
        self.alu(c::shl(l, m, c::r(vuelve), 0), l, m, c::r(vuelve));
        // Sube si lo perdido pasa de la mitad, o es la mitad y (no era exacto
        // o queda impar): con `l + (impar | inexacto) > 0x8000_0000`.
        self.y_con(sube, kk, 1);
        self.o_con(sube, sube, c::r(inexacto));
        self.mas(l, l, c::r(sube));
        self.pregunta(Cmp::Gt, l, Fuente::Imm(0x8000_0000), true);
        self.elige_p0(sube, RZ, Fuente::Imm(1), true);
        self.mas(kk, kk, c::r(sube));
        self.o_con(x, kk, c::r(s));
        al_final.push(self.saltar(PT));
        // ** NaN, infinitos y ceros: el infinito, salvo lo que diga cada uno
        // -- de lo menos a lo mas fuerte --.
        let aqui = self.codigo.len();
        self.parchear(especial, aqui);
        self.o_con(x, s, Fuente::Imm(INFINITO));
        // 0 / b y a / inf: cero.
        self.pregunta(Cmp::Eq, aa, Fuente::Imm(0), true);
        self.elige_p0(x, s, c::r(x), false);
        self.pregunta(Cmp::Eq, bb, Fuente::Imm(INFINITO), true);
        self.elige_p0(x, s, c::r(x), false);
        // 0 / 0 e inf / inf -- aqui, |a| == |b| solo puede ser eso, o el
        // mismo NaN --, y un NaN: NaN.
        self.pregunta(Cmp::Eq, aa, c::r(bb), true);
        self.elige_p0(x, x, Fuente::Imm(NAN), true);
        self.alu(c::imnmx(t, aa, c::r(bb), true, false, 0), t, aa, c::r(bb));
        self.pregunta(Cmp::Gt, t, Fuente::Imm(INFINITO), true);
        self.elige_p0(x, x, Fuente::Imm(NAN), true);
        let fin = self.codigo.len();
        for k in al_final {
            self.parchear(k, fin);
        }
        self.p0 = None;
        Ok(())
    }
}
