//! **LA MATEMATICA EXACTA DE LA CASA, EN LA 3060** (E8 de
//! `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`, 09-10): de `Op::Mate`
//! (`bmo_prometeo::mates`), las que la casa cuenta EXACTAS -- los redondeos,
//! `frac`, que clase de numero es, los bits y los medios floats --, con los
//! MISMOS bits que `Mate::aplicar`, y solo con lo que la lista blanca de R7
//! ya deja (FADD, FMUL, FSETP, ISETP, SEL, IADD3, IMAD, LOP3, SHF, I2F, F2I):
//! el juez no cambia.
//!
//! ```text
//!    trunca          |x| < 2^23: F2I.TRUNC e I2F, con el signo de x (-0.5
//!                    da -0); si no -- enorme, infinito o NaN --, x tal cual
//!    suelo           trunca, y uno menos si se paso de x
//!    techo           -suelo(-x), el signo por sus bits
//!    redondo_par     |x| < 2^23: (|x| + 2^23) - 2^23 -- la suma redondea al
//!                    PAR -- con el signo de x; si no, x
//!    frac            x - suelo(x), la misma resta que la casa
//!    es_nan ...      FSETP (desordenada para el NaN), y la normal por su
//!                    exponente: 0xFFFFFFFF o 0
//!    los bits        contar (SWAR), invertir (cinco cambios), el primero por
//!                    abajo y por arriba (con su signo): 0xFFFFFFFF sin
//!                    ninguno
//!    medios floats   por sus campos, con los bordes de la casa: su NaN
//!                    (0x7E00), el infinito desde 65520, los subnormales al
//!                    par
//! ```
//!
//! Cada cuenta escribe su destino en la ULTIMA instruccion: lo de antes va a
//! registros de paso, y `x = f(x)` no pisa lo que aun lee. Los predicados son
//! P1..P3: el P0 es el de las comparaciones fundidas.
//!
//! Lo que la casa contaba en f64 por series -- seno, coseno, tangente, exp2,
//! log2, los arcos y los hiperbolicos -- no esta aqui: desde DL13 (09-10)
//! son RECETAS de f32 que la casa corre y la 3060 repite (`series.rs`).

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::Reg;
use bmo_proton_x::mates::Mate;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};

use super::planifica::Meta;
use super::{reg_de, Clase, Emisor, NoEmite};

const P1: u8 = 1;
const P2: u8 = 2;
const P3: u8 = 3;

/// 2^23 en f32: desde aqui todo f32 es entero.
const DOS_23: u32 = 0x4B00_0000;
const MENOS_DOS_23: u32 = 0xCB00_0000;
const MENOS_UNO: u32 = 0xBF80_0000;
const SIGNO: u32 = 0x8000_0000;
const INFINITO: u32 = 0x7F80_0000;
const TODOS: u32 = u32::MAX;
/// `!a` en una LOP3 de dos entradas (la tercera, RZ).
const NO: u8 = 0x0F;

/// Las que la casa cuenta exactas, y por eso se emiten.
pub(super) fn exacta(f: Mate) -> bool {
    !matches!(f, Mate::Sin | Mate::Cos | Mate::Tan | Mate::Exp2 | Mate::Log2 | Mate::Acos | Mate::Asin | Mate::Atan | Mate::Cosh | Mate::Senh | Mate::Tanh)
}

impl Emisor<'_> {
    /// **`Op::Mate`**: las exactas, con los bits de la casa. `false` si es de
    /// las de series (no se emite: lo dice quien llama).
    pub(super) fn mate(&mut self, d: Reg, a: Reg, f: Mate, i: usize, paso: &mut Vec<u8>) -> Result<bool, NoEmite> {
        if !exacta(f) {
            // ** DL13 (09-10): las de series, por su receta (`series.rs`).
            return self.serie(d, a, f, i, paso);
        }
        let v = self.registro(a, paso)?;
        // Todo a registros de paso, y el destino lo escribe la ultima.
        let (w, clase, lee, p) = match f {
            Mate::Trunca => {
                let (t, p) = self.trunca_antes(v, paso)?;
                (c::sel(0, t, c::r(v), p, false, 0), Clase::Alu, [Some(t), Some(v), None], Some(p))
            }
            Mate::Suelo => {
                let (t, u, p) = self.suelo_antes(v, paso)?;
                (c::sel(0, u, c::r(t), p, false, 0), Clase::Alu, [Some(u), Some(t), None], Some(p))
            }
            Mate::Techo => {
                let n = self.paso(paso)?;
                self.en_alu(c::lop3(n, v, Fuente::Imm(SIGNO), c::OX, 0), n, [Some(v), None, None]);
                let s = self.suelo(n, paso)?;
                (c::lop3(0, s, Fuente::Imm(SIGNO), c::OX, 0), Clase::Alu, [Some(s), None, None], None)
            }
            Mate::RedondoPar => {
                self.fsetp(P1, Cmp::Lt, c::abs(v), Fuente::Imm(DOS_23));
                let t = self.paso(paso)?;
                self.en_fma(c::fadd(t, c::abs(v), Fuente::Imm(DOS_23), false, 0), t, [Some(v), None, None]);
                self.en_fma(c::fadd(t, c::r(t), Fuente::Imm(MENOS_DOS_23), false, 0), t, [Some(t), None, None]);
                let s = self.paso(paso)?;
                self.en_alu(c::lop3(s, v, Fuente::Imm(SIGNO), c::Y, 0), s, [Some(v), None, None]);
                self.en_alu(c::lop3(t, t, c::r(s), c::O, 0), t, [Some(t), Some(s), None]);
                (c::sel(0, t, c::r(v), P1, false, 0), Clase::Alu, [Some(t), Some(v), None], Some(P1))
            }
            Mate::Frac => {
                let s = self.suelo(v, paso)?;
                (c::fadd(0, c::r(v), c::neg(s), false, 0), Clase::Fma, [Some(v), Some(s), None], None)
            }
            Mate::EsNan => self.si_o_no(Cmp::Neu, c::r(v), c::r(v)),
            Mate::EsInf => self.si_o_no(Cmp::Eq, c::abs(v), Fuente::Imm(INFINITO)),
            Mate::EsFinito => self.si_o_no(Cmp::Lt, c::abs(v), Fuente::Imm(INFINITO)),
            Mate::EsNormal => {
                // Su exponente, ni 0 ni 255: (e - 1) < 254 sin signo.
                let e = self.paso(paso)?;
                self.en_alu(c::shr(e, v, Fuente::Imm(23), false, 0), e, [Some(v), None, None]);
                self.en_alu(c::lop3(e, e, Fuente::Imm(0xFF), c::Y, 0), e, [Some(e), None, None]);
                self.en_alu(c::iadd3(e, e, Fuente::Imm(TODOS), 0), e, [Some(e), None, None]);
                self.isetp(P1, Cmp::Lt, e, Fuente::Imm(254), true);
                (c::sel(0, RZ, Fuente::Imm(TODOS), P1, true, 0), Clase::Alu, [None; 3], Some(P1))
            }
            Mate::CuentaBits => {
                let r = self.cuenta_antes(v, paso)?;
                (c::shr(0, r, Fuente::Imm(24), false, 0), Clase::Alu, [Some(r), None, None], None)
            }
            Mate::InvierteBits => {
                let (a, b) = self.invierte_antes(v, paso)?;
                (c::lop3(0, a, c::r(b), c::O, 0), Clase::Alu, [Some(a), Some(b), None], None)
            }
            Mate::PrimerBitBajo => {
                // Lo de debajo del bit mas bajo, contado: v & -v, menos 1.
                let t = self.paso(paso)?;
                self.en_alu(c::iadd3(t, RZ, c::neg(v), 0), t, [Some(v), None, None]);
                self.en_alu(c::lop3(t, v, c::r(t), c::Y, 0), t, [Some(v), Some(t), None]);
                self.en_alu(c::iadd3(t, t, Fuente::Imm(TODOS), 0), t, [Some(t), None, None]);
                let n = self.cuenta(t, paso)?;
                self.isetp(P2, Cmp::Eq, v, c::r(RZ), false);
                (c::sel(0, n, Fuente::Imm(TODOS), P2, true, 0), Clase::Alu, [Some(n), None, None], Some(P2))
            }
            Mate::PrimerBitAlto => {
                let n = self.ceros_arriba(v, paso)?;
                self.isetp(P2, Cmp::Eq, v, c::r(RZ), false);
                (c::sel(0, n, Fuente::Imm(TODOS), P2, true, 0), Clase::Alu, [Some(n), None, None], Some(P2))
            }
            Mate::PrimerBitAltoConSigno => {
                // Un negativo se cuenta por su complemento: el primer 0.
                self.isetp(P3, Cmp::Lt, v, c::r(RZ), false);
                let b = self.paso(paso)?;
                self.en_alu(c::lop3(b, v, c::r(RZ), NO, 0), b, [Some(v), None, None]);
                self.sel(b, b, c::r(v), P3, false);
                let n = self.ceros_arriba(b, paso)?;
                self.isetp(P2, Cmp::Eq, b, c::r(RZ), false);
                (c::sel(0, n, Fuente::Imm(TODOS), P2, true, 0), Clase::Alu, [Some(n), None, None], Some(P2))
            }
            Mate::F16aF32 => self.de_medio(v, paso)?,
            Mate::F32aF16 => self.a_medio(v, paso)?,
            _ => return Ok(false),
        };
        let x = self.destino(d, i)?;
        // La ultima: la misma, con su destino (bits 16..24).
        let w = (w.0 & !(0xFF << 16) | (x as u64) << 16, w.1);
        let m = Meta::de(clase, Some(x), lee);
        self.poner_meta(w, Meta { lee_p: p.map(|p| (p, false)), ..m });
        Ok(true)
    }

    // -- los ladrillos ---------------------------------------------------

    fn paso(&mut self, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let t = self.pedir()?;
        paso.push(t);
        Ok(t)
    }

    fn en_alu(&mut self, w: (u64, u64), x: u8, lee: [Option<u8>; 3]) {
        self.poner(w, Clase::Alu, Some(x), lee);
    }

    fn en_fma(&mut self, w: (u64, u64), x: u8, lee: [Option<u8>; 3]) {
        self.poner(w, Clase::Fma, Some(x), lee);
    }

    /// I2F/F2I: desacopladas, con barrera (como en `Op::Convierte`).
    fn desacoplada(&mut self, w: (u64, u64), x: u8, a: u8) {
        self.poner(w, Clase::Mufu, Some(x), [Some(a), None, None]);
        self.mufus += 1;
    }

    fn fsetp(&mut self, p: u8, como: Cmp, a: Fuente, b: Fuente) {
        self.poner_meta(c::fsetp(p, como, a, b, 0), Meta { escribe_p: Some(p), ..Meta::de(Clase::Alu, None, [reg_de(a), reg_de(b), None]) });
    }

    fn isetp(&mut self, p: u8, como: Cmp, a: u8, b: Fuente, sin_signo: bool) {
        self.poner_meta(c::isetp(p, como, a, b, sin_signo, 0), Meta { escribe_p: Some(p), ..Meta::de(Clase::Alu, None, [Some(a), reg_de(b), None]) });
    }

    /// `x = p ? a : b`.
    fn sel(&mut self, x: u8, a: u8, b: Fuente, p: u8, negado: bool) {
        self.poner_meta(c::sel(x, a, b, p, negado, 0), Meta { lee_p: Some((p, false)), ..Meta::de(Clase::Alu, Some(x), [Some(a), reg_de(b), None]) });
    }

    /// Un si/no de una comparacion de floats: 0xFFFFFFFF o 0.
    fn si_o_no(&mut self, como: Cmp, a: Fuente, b: Fuente) -> ((u64, u64), Clase, [Option<u8>; 3], Option<u8>) {
        self.fsetp(P1, como, a, b);
        (c::sel(0, RZ, Fuente::Imm(TODOS), P1, true, 0), Clase::Alu, [None; 3], Some(P1))
    }

    /// trunca(v) sin escribirlo: el candidato (el entero, con el signo de v)
    /// y el predicado que lo elige (|v| < 2^23).
    fn trunca_antes(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<(u8, u8), NoEmite> {
        self.fsetp(P1, Cmp::Lt, c::abs(v), Fuente::Imm(DOS_23));
        let t = self.paso(paso)?;
        self.desacoplada(c::f2i(t, v, true, 0), t, v);
        self.desacoplada(c::i2f(t, t, true, 0), t, t);
        let s = self.paso(paso)?;
        self.en_alu(c::lop3(s, v, Fuente::Imm(SIGNO), c::Y, 0), s, [Some(v), None, None]);
        self.en_alu(c::lop3(t, t, c::r(s), c::O, 0), t, [Some(t), Some(s), None]);
        Ok((t, P1))
    }

    /// trunca(v), en un registro de paso.
    fn trunca(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let (t, p) = self.trunca_antes(v, paso)?;
        let r = self.paso(paso)?;
        self.sel(r, t, c::r(v), p, false);
        Ok(r)
    }

    /// suelo(v) sin escribirlo: trunca(v), trunca(v) - 1, y el predicado que
    /// elige el segundo (trunca(v) > v).
    fn suelo_antes(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<(u8, u8, u8), NoEmite> {
        let t = self.trunca(v, paso)?;
        self.fsetp(P2, Cmp::Gt, c::r(t), c::r(v));
        let u = self.paso(paso)?;
        self.en_fma(c::fadd(u, c::r(t), Fuente::Imm(MENOS_UNO), false, 0), u, [Some(t), None, None]);
        Ok((t, u, P2))
    }

    /// suelo(v), en un registro de paso.
    fn suelo(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let (t, u, p) = self.suelo_antes(v, paso)?;
        let r = self.paso(paso)?;
        self.sel(r, u, c::r(t), p, false);
        Ok(r)
    }

    /// Los bits de v, contados por pares, cuartetos y bytes; el ultimo paso
    /// (los cuatro bytes sumados en el de arriba) lo da quien llama: `>> 24`.
    fn cuenta_antes(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let t = self.paso(paso)?;
        let w = self.paso(paso)?;
        self.en_alu(c::shr(t, v, Fuente::Imm(1), false, 0), t, [Some(v), None, None]);
        self.en_alu(c::lop3(t, t, Fuente::Imm(0x5555_5555), c::Y, 0), t, [Some(t), None, None]);
        self.en_alu(c::iadd3(w, v, c::neg(t), 0), w, [Some(v), Some(t), None]);
        self.en_alu(c::lop3(t, w, Fuente::Imm(0x3333_3333), c::Y, 0), t, [Some(w), None, None]);
        self.en_alu(c::shr(w, w, Fuente::Imm(2), false, 0), w, [Some(w), None, None]);
        self.en_alu(c::lop3(w, w, Fuente::Imm(0x3333_3333), c::Y, 0), w, [Some(w), None, None]);
        self.en_alu(c::iadd3(w, w, c::r(t), 0), w, [Some(w), Some(t), None]);
        self.en_alu(c::shr(t, w, Fuente::Imm(4), false, 0), t, [Some(w), None, None]);
        self.en_alu(c::iadd3(w, w, c::r(t), 0), w, [Some(w), Some(t), None]);
        self.en_alu(c::lop3(w, w, Fuente::Imm(0x0F0F_0F0F), c::Y, 0), w, [Some(w), None, None]);
        self.en_fma(c::imad(w, w, Fuente::Imm(0x0101_0101), RZ, 0), w, [Some(w), None, None]);
        Ok(w)
    }

    /// Los bits de v contados, en un registro de paso.
    fn cuenta(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let w = self.cuenta_antes(v, paso)?;
        let r = self.paso(paso)?;
        self.en_alu(c::shr(r, w, Fuente::Imm(24), false, 0), r, [Some(w), None, None]);
        Ok(r)
    }

    /// v al reves, menos el ultimo cambio: sus dos mitades (a | b es el
    /// resultado).
    fn invierte_antes(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<(u8, u8), NoEmite> {
        let x = self.paso(paso)?;
        let a = self.paso(paso)?;
        let b = self.paso(paso)?;
        let mut de = v;
        let cambios = [(1, 0x5555_5555), (2, 0x3333_3333), (4, 0x0F0F_0F0F), (8, 0x00FF_00FF), (16, 0x0000_FFFF)];
        for (k, &(n, m)) in cambios.iter().enumerate() {
            // a = (de >> n) & m;  b = (de & m) << n
            self.en_alu(c::shr(a, de, Fuente::Imm(n), false, 0), a, [Some(de), None, None]);
            self.en_alu(c::lop3(a, a, Fuente::Imm(m), c::Y, 0), a, [Some(a), None, None]);
            self.en_alu(c::lop3(b, de, Fuente::Imm(m), c::Y, 0), b, [Some(de), None, None]);
            self.en_alu(c::shl(b, b, Fuente::Imm(n), 0), b, [Some(b), None, None]);
            if k + 1 < cambios.len() {
                self.en_alu(c::lop3(x, a, c::r(b), c::O, 0), x, [Some(a), Some(b), None]);
                de = x;
            }
        }
        Ok((a, b))
    }

    /// Los ceros por encima del bit mas alto de v (32 si v es 0): v se
    /// "derrama" hacia abajo y se cuentan los ceros que quedan.
    fn ceros_arriba(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let s = self.paso(paso)?;
        let t = self.paso(paso)?;
        let mut de = v;
        for n in [1, 2, 4, 8, 16] {
            self.en_alu(c::shr(t, de, Fuente::Imm(n), false, 0), t, [Some(de), None, None]);
            self.en_alu(c::lop3(s, de, c::r(t), c::O, 0), s, [Some(de), Some(t), None]);
            de = s;
        }
        self.en_alu(c::lop3(s, s, c::r(RZ), NO, 0), s, [Some(s), None, None]);
        self.cuenta(s, paso)
    }

    /// **Medio float a f32** (`de_medio` de la casa): los 16 bits de abajo.
    fn de_medio(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<((u64, u64), Clase, [Option<u8>; 3], Option<u8>), NoEmite> {
        let s = self.paso(paso)?;
        let e = self.paso(paso)?;
        let m = self.paso(paso)?;
        let n = self.paso(paso)?;
        let q = self.paso(paso)?;
        // el signo (al bit 31), el exponente y la mantisa
        self.en_alu(c::shl(s, v, Fuente::Imm(16), 0), s, [Some(v), None, None]);
        self.en_alu(c::lop3(s, s, Fuente::Imm(SIGNO), c::Y, 0), s, [Some(s), None, None]);
        self.en_alu(c::shr(e, v, Fuente::Imm(10), false, 0), e, [Some(v), None, None]);
        self.en_alu(c::lop3(e, e, Fuente::Imm(0x1F), c::Y, 0), e, [Some(e), None, None]);
        self.en_alu(c::lop3(m, v, Fuente::Imm(0x3FF), c::Y, 0), m, [Some(v), None, None]);
        // normal: s | (e + 112) << 23 | m << 13
        self.en_alu(c::iadd3(n, e, Fuente::Imm(112), 0), n, [Some(e), None, None]);
        self.en_alu(c::shl(n, n, Fuente::Imm(23), 0), n, [Some(n), None, None]);
        self.en_alu(c::shl(q, m, Fuente::Imm(13), 0), q, [Some(m), None, None]);
        self.en_alu(c::lop3(n, n, c::r(q), c::O, 0), n, [Some(n), Some(q), None]);
        self.en_alu(c::lop3(n, n, c::r(s), c::O, 0), n, [Some(n), Some(s), None]);
        // infinito o NaN: s | 0x7F800000, y con mantisa, | 0x400000 | m << 13
        self.isetp(P1, Cmp::Ne, m, c::r(RZ), false);
        self.en_alu(c::lop3(q, q, Fuente::Imm(0x40_0000), c::O, 0), q, [Some(q), None, None]);
        self.sel(q, q, c::r(RZ), P1, false);
        self.en_alu(c::lop3(q, q, Fuente::Imm(INFINITO), c::O, 0), q, [Some(q), None, None]);
        self.en_alu(c::lop3(q, q, c::r(s), c::O, 0), q, [Some(q), Some(s), None]);
        // subnormal (y cero): m * 2^-24, exacto, con su signo
        self.desacoplada(c::i2f(m, m, false, 0), m, m);
        self.en_fma(c::fmul(m, c::r(m), Fuente::Imm(0x3380_0000), false, 0), m, [Some(m), None, None]);
        self.en_alu(c::lop3(m, m, c::r(s), c::O, 0), m, [Some(m), Some(s), None]);
        // por el exponente: 0, el subnormal; 31, el infinito o NaN; si no, el normal
        self.isetp(P2, Cmp::Eq, e, c::r(RZ), false);
        self.sel(n, m, c::r(n), P2, false);
        self.isetp(P3, Cmp::Eq, e, Fuente::Imm(31), false);
        Ok((c::sel(0, q, c::r(n), P3, false, 0), Clase::Alu, [Some(q), Some(n), None], Some(P3)))
    }

    /// **f32 a medio float** (`a_medio` de la casa): al mas cercano, el
    /// empate al par; en los 16 bits de abajo.
    fn a_medio(&mut self, v: u8, paso: &mut Vec<u8>) -> Result<((u64, u64), Clase, [Option<u8>; 3], Option<u8>), NoEmite> {
        let s = self.paso(paso)?;
        let h = self.paso(paso)?;
        let m = self.paso(paso)?;
        let t = self.paso(paso)?;
        // el signo, al bit 15
        self.en_alu(c::shr(s, v, Fuente::Imm(16), false, 0), s, [Some(v), None, None]);
        self.en_alu(c::lop3(s, s, Fuente::Imm(0x8000), c::Y, 0), s, [Some(s), None, None]);
        // normal: h = (e - 112) << 10 | m >> 13, mas uno si lo que cae pasa de
        // la mitad, o es la mitad y h es impar: (resto + (h & 1)) > 0x1000
        self.en_alu(c::shr(h, v, Fuente::Imm(23), false, 0), h, [Some(v), None, None]);
        self.en_alu(c::lop3(h, h, Fuente::Imm(0xFF), c::Y, 0), h, [Some(h), None, None]);
        self.en_alu(c::iadd3(h, h, Fuente::Imm(112u32.wrapping_neg()), 0), h, [Some(h), None, None]);
        self.en_alu(c::shl(h, h, Fuente::Imm(10), 0), h, [Some(h), None, None]);
        self.en_alu(c::lop3(m, v, Fuente::Imm(0x7F_FFFF), c::Y, 0), m, [Some(v), None, None]);
        self.en_alu(c::shr(t, m, Fuente::Imm(13), false, 0), t, [Some(m), None, None]);
        self.en_alu(c::lop3(h, h, c::r(t), c::O, 0), h, [Some(h), Some(t), None]);
        self.en_alu(c::lop3(m, m, Fuente::Imm(0x1FFF), c::Y, 0), m, [Some(m), None, None]);
        self.en_alu(c::lop3(t, h, Fuente::Imm(1), c::Y, 0), t, [Some(h), None, None]);
        self.en_alu(c::iadd3(m, m, c::r(t), 0), m, [Some(m), Some(t), None]);
        self.isetp(P1, Cmp::Gt, m, Fuente::Imm(0x1000), true);
        self.en_alu(c::iadd3(t, h, Fuente::Imm(1), 0), t, [Some(h), None, None]);
        self.sel(h, t, c::r(h), P1, false);
        // subnormal: |v| * 2^24 al entero par (exacto: |v| < 2^-14)
        self.en_fma(c::fmul(t, c::abs(v), Fuente::Imm(0x4B80_0000), false, 0), t, [Some(v), None, None]);
        self.en_fma(c::fadd(t, c::r(t), Fuente::Imm(DOS_23), false, 0), t, [Some(t), None, None]);
        self.en_fma(c::fadd(t, c::r(t), Fuente::Imm(MENOS_DOS_23), false, 0), t, [Some(t), None, None]);
        self.desacoplada(c::f2i(t, t, false, 0), t, t);
        self.fsetp(P2, Cmp::Lt, c::abs(v), Fuente::Imm(0x3880_0000));
        self.sel(h, t, c::r(h), P2, false);
        // desde 65520, el infinito; el NaN, el de la casa
        self.fsetp(P3, Cmp::Ge, c::abs(v), Fuente::Imm(0x477F_F000));
        self.en_alu(c::mov(t, Fuente::Imm(0x7C00), 0), t, [None; 3]);
        self.sel(h, t, c::r(h), P3, false);
        self.fsetp(P1, Cmp::Neu, c::r(v), c::r(v));
        self.en_alu(c::mov(t, Fuente::Imm(0x7E00), 0), t, [None; 3]);
        self.sel(h, t, c::r(h), P1, false);
        Ok((c::lop3(0, h, c::r(s), c::O, 0), Clase::Alu, [Some(h), Some(s), None], None))
    }
}
