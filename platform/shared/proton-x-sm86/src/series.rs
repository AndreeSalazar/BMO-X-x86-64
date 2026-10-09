//! **LAS MATE DE SERIES EN LA 3060** (DL13 de `docs/plan/PLAN_LAS_LIBRERIAS.md`,
//! 09-10): seno, coseno, tangente, exp2, log2, los arcos y los hiperbolicos,
//! con los MISMOS bits que la casa -- por construccion: la receta es UNA
//! (`bmo_proton_x::cuentas`, de PROMETEO), la casa la corre y aqui se GRABA y se traduce,
//! cada cuenta a UNA instruccion --.
//!
//! ```text
//!    grabar     `Graba` es la tarjeta de la receta: cada cuenta, un paso
//!               con valores virtuales
//!    limpiar    lo que nadie lee, fuera (un registro escrito y soltado sin
//!               leer seria una carrera de escrituras)
//!    traducir   FFMA, FMUL, FADD, FSETP, ISETP, SEL, IADD3, IMAD, LOP3,
//!               SHF y MOV -- lo que R7 ya deja a un cuerpo de app: el juez
//!               no cambia --. Una constante va de inmediato donde la
//!               instruccion la acepta (la b); si no, un MOV, UNO por
//!               constante y receta
//!    registros  cada valor, en un registro de paso desde que se escribe
//!               hasta su ultima lectura; los predicados, P1..P6 (P0 es el
//!               de las comparaciones fundidas). El resultado lo escribe la
//!               ULTIMA instruccion, en su destino: `x = f(x)` no pisa lo que
//!               aun lee
//! ```
//!
//! Nada de MUFU, I2F ni F2I: sin unidad especial ni conversiones
//! desacopladas, no hay barreras ni bits que la casa no sepa repetir.

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::cuentas::{self, Bits, Como, Corre, Cuentas, E, F};
use bmo_proton_x::dxil::programa::Reg;
use bmo_proton_x::mates::Mate;
use bmo_sm86::codifica::{self as c, Cmp, Fuente, RZ};

use super::planifica::Meta;
use super::{reg_de, Clase, Emisor, NoEmite};

/// Un valor virtual (el 0 es la entrada).
type V = u16;

/// Un paso grabado.
#[derive(Clone, Copy, Debug)]
enum Paso {
    K { d: V, bits: u32 },
    Fma { d: V, a: F<V>, b: F<V>, c: F<V> },
    Mul { d: V, a: F<V>, b: F<V> },
    Suma { d: V, a: F<V>, b: F<V> },
    Compara { p: V, como: Como, a: F<V>, b: F<V> },
    CompEnt { p: V, como: Como, a: V, b: E<V> },
    Elige { d: V, p: V, a: E<V>, b: E<V> },
    Entera { d: V, a: V, b: E<V> },
    Imad { d: V, a: V, b: E<V>, c: V },
    Bits { d: V, que: Bits, a: V, b: E<V> },
    Corre { d: V, como: Corre, a: V, n: u32 },
}

/// **La tarjeta que graba**: cada cuenta, un paso.
#[derive(Default)]
pub(crate) struct Graba {
    pasos: Vec<Paso>,
    valores: V,
    predicados: V,
}

impl Graba {
    fn nuevo(&mut self) -> V {
        self.valores += 1;
        self.valores
    }
}

impl Cuentas for Graba {
    type V = V;
    type P = V;

    fn k(&mut self, bits: u32) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::K { d, bits });
        d
    }
    fn fma(&mut self, a: F<V>, b: F<V>, c: F<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Fma { d, a, b, c });
        d
    }
    fn mul(&mut self, a: F<V>, b: F<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Mul { d, a, b });
        d
    }
    fn suma(&mut self, a: F<V>, b: F<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Suma { d, a, b });
        d
    }
    fn compara(&mut self, como: Como, a: F<V>, b: F<V>) -> V {
        self.predicados += 1;
        let p = self.predicados;
        self.pasos.push(Paso::Compara { p, como, a, b });
        p
    }
    fn compara_entero(&mut self, como: Como, a: V, b: E<V>) -> V {
        self.predicados += 1;
        let p = self.predicados;
        self.pasos.push(Paso::CompEnt { p, como, a, b });
        p
    }
    fn elige(&mut self, p: V, a: E<V>, b: E<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Elige { d, p, a, b });
        d
    }
    fn entera(&mut self, a: V, b: E<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Entera { d, a, b });
        d
    }
    fn imad(&mut self, a: V, b: E<V>, c: V) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Imad { d, a, b, c });
        d
    }
    fn bits(&mut self, que: Bits, a: V, b: E<V>) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Bits { d, que, a, b });
        d
    }
    fn corre(&mut self, como: Corre, a: V, n: u32) -> V {
        let d = self.nuevo();
        self.pasos.push(Paso::Corre { d, como, a, n });
        d
    }
}

/// Lo que lee un paso (valores) y lo que escribe (valor o predicado).
fn lee_f(f: F<V>, v: &mut Vec<V>) {
    if let F::R(x) | F::Menos(x) | F::Abs(x) = f {
        v.push(x);
    }
}

fn lee_e(e: E<V>, v: &mut Vec<V>) {
    if let E::R(x) | E::Menos(x) = e {
        v.push(x);
    }
}

impl Paso {
    fn lee(&self) -> Vec<V> {
        let mut v = Vec::new();
        match *self {
            Paso::K { .. } => {}
            Paso::Fma { a, b, c, .. } => {
                lee_f(a, &mut v);
                lee_f(b, &mut v);
                lee_f(c, &mut v);
            }
            Paso::Mul { a, b, .. } | Paso::Suma { a, b, .. } | Paso::Compara { a, b, .. } => {
                lee_f(a, &mut v);
                lee_f(b, &mut v);
            }
            Paso::CompEnt { a, b, .. } | Paso::Entera { a, b, .. } | Paso::Bits { a, b, .. } => {
                v.push(a);
                lee_e(b, &mut v);
            }
            Paso::Elige { a, b, .. } => {
                lee_e(a, &mut v);
                lee_e(b, &mut v);
            }
            Paso::Imad { a, b, c, .. } => {
                v.push(a);
                lee_e(b, &mut v);
                v.push(c);
            }
            Paso::Corre { a, .. } => v.push(a),
        }
        v
    }

    fn lee_p(&self) -> Option<V> {
        match *self {
            Paso::Elige { p, .. } => Some(p),
            _ => None,
        }
    }

    /// El valor que escribe (los predicados, aparte).
    fn escribe(&self) -> Option<V> {
        match *self {
            Paso::K { d, .. }
            | Paso::Fma { d, .. }
            | Paso::Mul { d, .. }
            | Paso::Suma { d, .. }
            | Paso::Elige { d, .. }
            | Paso::Entera { d, .. }
            | Paso::Imad { d, .. }
            | Paso::Bits { d, .. }
            | Paso::Corre { d, .. } => Some(d),
            _ => None,
        }
    }

    fn escribe_p(&self) -> Option<V> {
        match *self {
            Paso::Compara { p, .. } | Paso::CompEnt { p, .. } => Some(p),
            _ => None,
        }
    }
}

/// **Grabar la receta de `f`** y dejar solo lo que llega al resultado: los
/// pasos, y el valor que es el resultado.
fn grabar(f: Mate) -> Option<(Vec<Paso>, V)> {
    let mut g = Graba::default();
    let res = cuentas::receta(&mut g, f, 0)?;
    // Lo que se lee, de atras adelante (los predicados con su propio
    // espacio: se marcan aparte).
    let mut vivo = vec![false; g.valores as usize + 1];
    let mut vivo_p = vec![false; g.predicados as usize + 1];
    vivo[res as usize] = true;
    let mut quedan = Vec::new();
    for p in g.pasos.iter().rev() {
        let util = p.escribe().is_some_and(|d| vivo[d as usize]) || p.escribe_p().is_some_and(|q| vivo_p[q as usize]);
        if !util {
            continue;
        }
        for x in p.lee() {
            vivo[x as usize] = true;
        }
        if let Some(q) = p.lee_p() {
            vivo_p[q as usize] = true;
        }
        quedan.push(*p);
    }
    quedan.reverse();
    Some((quedan, res))
}

/// **Cuanto cuesta una receta**: las instrucciones que emite (los MOV de sus
/// constantes incluidos).
pub fn instrucciones(f: Mate) -> Option<usize> {
    let (pasos, _) = grabar(f)?;
    Some(pasos.len() + constantes_en_registro(&pasos).len())
}

/// Las constantes que tienen que ir en un registro (no caben de inmediato
/// donde estan): sus bits, cada una UNA vez.
fn constantes_en_registro(pasos: &[Paso]) -> Vec<u32> {
    let mut ks: Vec<u32> = Vec::new();
    let mut pon = |b: u32| {
        if !ks.contains(&b) {
            ks.push(b);
        }
    };
    for p in pasos {
        for b in a_registro(p) {
            pon(b);
        }
    }
    ks
}

/// Los bits de una constante de float con su modificador.
fn k_de(f: F<V>) -> Option<u32> {
    match f {
        F::K(b) => Some(b),
        _ => None,
    }
}

/// Lo que de un paso va a registro aunque sea constante (despues de dar la
/// vuelta a lo que conmuta): las de la `a` y la `c`.
fn a_registro(p: &Paso) -> Vec<u32> {
    let mut v = Vec::new();
    match *p {
        Paso::Fma { a, b, c, .. } => {
            let (a, _) = conmuta_f(a, b);
            v.extend(k_de(a));
            v.extend(k_de(c));
        }
        Paso::Mul { a, b, .. } | Paso::Suma { a, b, .. } => {
            let (a, _) = conmuta_f(a, b);
            v.extend(k_de(a));
        }
        Paso::Compara { a, b, .. } => {
            let (a, _) = conmuta_f(a, b);
            v.extend(k_de(a));
        }
        Paso::Elige { a, b, .. } => {
            if let (E::K(x), E::K(_)) = (a, b) {
                v.push(x);
            }
        }
        Paso::CompEnt { b: E::Menos(_), .. } => {}
        _ => {}
    }
    v
}

/// Si `a` es constante y `b` no, al reves (para FFMA, FMUL, FADD y FSETP).
fn conmuta_f(a: F<V>, b: F<V>) -> (F<V>, F<V>) {
    if matches!(a, F::K(_)) && !matches!(b, F::K(_)) {
        (b, a)
    } else {
        (a, b)
    }
}

/// La comparacion con los operandos al reves.
fn espejo(como: Como) -> Como {
    match como {
        Como::Lt => Como::Gt,
        Como::Le => Como::Ge,
        Como::Gt => Como::Lt,
        Como::Ge => Como::Le,
        x => x,
    }
}

fn cmp(como: Como) -> Cmp {
    match como {
        Como::Lt => Cmp::Lt,
        Como::Le => Cmp::Le,
        Como::Gt => Cmp::Gt,
        Como::Ge => Cmp::Ge,
        Como::Eq => Cmp::Eq,
        Como::Ne => Cmp::Ne,
        Como::Neu => Cmp::Neu,
    }
}

/// Los predicados que usa una receta: P1..P6.
const PREDICADOS: [u8; 6] = [1, 2, 3, 4, 5, 6];

/// Donde vive cada valor virtual mientras se emite.
struct Sitios {
    reg: Vec<Option<u8>>,
    pred: Vec<Option<u8>>,
    /// Los registros de las constantes (por sus bits).
    ks: Vec<(u32, u8)>,
}

impl Emisor<'_> {
    /// **`Op::Mate` de series**: su receta, grabada y traducida. `false` si
    /// `f` no es de series.
    pub(super) fn serie(&mut self, d: Reg, a: Reg, f: Mate, i: usize, paso: &mut Vec<u8>) -> Result<bool, NoEmite> {
        let Some((pasos, res)) = grabar(f) else {
            return Ok(false);
        };
        let entrada = self.registro(a, paso)?;
        let n = pasos.iter().filter_map(|p| p.escribe()).max().unwrap_or(0) as usize + 1;
        let np = pasos.iter().filter_map(|p| p.escribe_p()).max().unwrap_or(0) as usize + 1;
        // La ultima lectura de cada valor, de cada predicado y de cada
        // constante en registro.
        let mut ultimo = vec![usize::MAX; n];
        let mut ultimo_p = vec![usize::MAX; np];
        let mut ultimo_k: Vec<(u32, usize)> = Vec::new();
        for (k, p) in pasos.iter().enumerate() {
            for x in p.lee() {
                ultimo[x as usize] = k;
            }
            if let Some(q) = p.lee_p() {
                ultimo_p[q as usize] = k;
            }
            for b in a_registro(p) {
                match ultimo_k.iter_mut().find(|(x, _)| *x == b) {
                    Some(u) => u.1 = k,
                    None => ultimo_k.push((b, k)),
                }
            }
        }
        let mut s = Sitios { reg: vec![None; n], pred: vec![None; np], ks: Vec::new() };
        s.reg[0] = Some(entrada);
        let mut libres_p: Vec<u8> = PREDICADOS.to_vec();
        let fin = pasos.len() - 1;
        debug_assert_eq!(pasos[fin].escribe(), Some(res), "la ultima escribe el resultado");
        for (k, p) in pasos.iter().enumerate() {
            // Las constantes que esta pide en registro, con su MOV (una vez).
            for b in a_registro(p) {
                if !s.ks.iter().any(|(x, _)| *x == b) {
                    let t = self.pedir()?;
                    self.poner(c::mov(t, Fuente::Imm(b), 0), Clase::Alu, Some(t), [None; 3]);
                    s.ks.push((b, t));
                }
            }
            // Los registros que mueren aqui (sus valores y sus constantes).
            let mut muere: Vec<u8> = Vec::new();
            for x in p.lee() {
                if x != 0 && ultimo[x as usize] == k {
                    let r = s.reg[x as usize].expect("un valor sin registro");
                    if !muere.contains(&r) {
                        muere.push(r);
                    }
                }
            }
            for (b, u) in &ultimo_k {
                if *u == k {
                    let r = s.ks.iter().find(|(x, _)| x == b).map(|(_, r)| *r).expect("una constante sin registro");
                    if !muere.contains(&r) {
                        muere.push(r);
                    }
                }
            }
            // El destino: en la ultima, el del resultado; si no, uno de paso
            // -- pedido DESPUES de soltar lo que muere: puede ser el de una
            // fuente, que se lee antes de escribirse --.
            let dest = match p.escribe() {
                Some(_) if k == fin => Some(self.destino(d, i)?),
                Some(_) => {
                    for &r in &muere {
                        self.soltar(r);
                    }
                    muere.clear();
                    Some(self.pedir()?)
                }
                None => None,
            };
            let pdest = match p.escribe_p() {
                Some(q) => {
                    let x = *libres_p.first().ok_or(NoEmite::Registros)?;
                    libres_p.remove(0);
                    s.pred[q as usize] = Some(x);
                    Some(x)
                }
                None => None,
            };
            self.traducir(p, &s, dest, pdest);
            if let Some(dv) = p.escribe() {
                s.reg[dv as usize] = dest;
            }
            for &r in &muere {
                self.soltar(r);
            }
            if let Some(q) = p.lee_p() {
                if ultimo_p[q as usize] == k {
                    libres_p.push(s.pred[q as usize].expect("un predicado sin sitio"));
                    libres_p.sort_unstable();
                }
            }
        }
        Ok(true)
    }

    /// Un paso, en su instruccion (con sus fuentes ya en registros).
    fn traducir(&mut self, p: &Paso, s: &Sitios, dest: Option<u8>, pdest: Option<u8>) {
        let reg = |x: V| s.reg[x as usize].expect("un valor sin registro");
        let kreg = |b: u32| s.ks.iter().find(|(x, _)| *x == b).map(|(_, r)| *r).expect("una constante sin registro");
        // Una fuente de float donde va un registro (`a` o `c`).
        let fr = |f: F<V>| -> Fuente {
            match f {
                F::R(x) => c::r(reg(x)),
                F::Menos(x) => c::neg(reg(x)),
                F::Abs(x) => c::abs(reg(x)),
                F::K(b) => c::r(kreg(b)),
            }
        };
        // Una fuente de float donde cabe un inmediato (`b`).
        let fb = |f: F<V>| -> Fuente {
            match f {
                F::K(b) => Fuente::Imm(b),
                x => fr(x),
            }
        };
        let eb = |e: E<V>| -> Fuente {
            match e {
                E::R(x) => c::r(reg(x)),
                E::Menos(x) => c::neg(reg(x)),
                E::K(b) => Fuente::Imm(b),
            }
        };
        let lee3 = |fs: [Fuente; 3]| -> [Option<u8>; 3] { [reg_de(fs[0]), reg_de(fs[1]), reg_de(fs[2])] };
        match *p {
            Paso::K { bits, .. } => {
                let x = dest.unwrap();
                self.poner(c::mov(x, Fuente::Imm(bits), 0), Clase::Alu, Some(x), [None; 3]);
            }
            Paso::Fma { a, b, c: cc, .. } => {
                let x = dest.unwrap();
                let (a, b) = conmuta_f(a, b);
                let (fa, fbb, fc) = (fr(a), fb(b), fr(cc));
                self.poner(c::ffma(x, fa, fbb, fc, false, 0), Clase::Fma, Some(x), lee3([fa, fbb, fc]));
            }
            Paso::Mul { a, b, .. } => {
                let x = dest.unwrap();
                let (a, b) = conmuta_f(a, b);
                let (fa, fbb) = (fr(a), fb(b));
                self.poner(c::fmul(x, fa, fbb, false, 0), Clase::Fma, Some(x), lee3([fa, fbb, c::r(RZ)]));
            }
            Paso::Suma { a, b, .. } => {
                let x = dest.unwrap();
                let (a, b) = conmuta_f(a, b);
                let (fa, fbb) = (fr(a), fb(b));
                self.poner(c::fadd(x, fa, fbb, false, 0), Clase::Fma, Some(x), lee3([fa, fbb, c::r(RZ)]));
            }
            Paso::Compara { como, a, b, .. } => {
                let q = pdest.unwrap();
                let (como, a, b) = if matches!(a, F::K(_)) && !matches!(b, F::K(_)) { (espejo(como), b, a) } else { (como, a, b) };
                let (fa, fbb) = (fr(a), fb(b));
                self.poner_meta(c::fsetp(q, cmp(como), fa, fbb, 0), Meta { escribe_p: Some(q), ..Meta::de(Clase::Alu, None, lee3([fa, fbb, c::r(RZ)])) });
            }
            Paso::CompEnt { como, a, b, .. } => {
                let q = pdest.unwrap();
                let fbb = eb(b);
                self.poner_meta(c::isetp(q, cmp(como), reg(a), fbb, false, 0), Meta { escribe_p: Some(q), ..Meta::de(Clase::Alu, None, [Some(reg(a)), reg_de(fbb), None]) });
            }
            Paso::Elige { p: q, a, b, .. } => {
                let x = dest.unwrap();
                let pr = s.pred[q as usize].expect("un predicado sin sitio");
                // SEL Rd, Ra, b, P: si `a` es constante y `b` no, al reves con
                // el predicado negado; si las dos lo son, `a` en su registro.
                let (ra, fbb, negado) = match (a, b) {
                    (E::K(_), E::R(y)) => (reg(y), eb(a), true),
                    (E::K(ka), E::K(_)) => (kreg(ka), eb(b), false),
                    (E::R(y), _) => (reg(y), eb(b), false),
                    (E::Menos(_), _) | (E::K(_), E::Menos(_)) => unreachable!("SEL no niega"),
                };
                self.poner_meta(c::sel(x, ra, fbb, pr, negado, 0), Meta { lee_p: Some((pr, false)), ..Meta::de(Clase::Alu, Some(x), [Some(ra), reg_de(fbb), None]) });
            }
            Paso::Entera { a, b, .. } => {
                let x = dest.unwrap();
                let fbb = eb(b);
                self.poner(c::iadd3(x, reg(a), fbb, 0), Clase::Alu, Some(x), [Some(reg(a)), reg_de(fbb), None]);
            }
            Paso::Imad { a, b, c: cc, .. } => {
                let x = dest.unwrap();
                let fbb = eb(b);
                self.poner(c::imad(x, reg(a), fbb, reg(cc), 0), Clase::Fma, Some(x), [Some(reg(a)), reg_de(fbb), Some(reg(cc))]);
            }
            Paso::Bits { que, a, b, .. } => {
                let x = dest.unwrap();
                let fbb = eb(b);
                let lut = match que {
                    Bits::Y => c::Y,
                    Bits::O => c::O,
                    Bits::Ox => c::OX,
                };
                self.poner(c::lop3(x, reg(a), fbb, lut, 0), Clase::Alu, Some(x), [Some(reg(a)), reg_de(fbb), None]);
            }
            Paso::Corre { como, a, n, .. } => {
                let x = dest.unwrap();
                let w = match como {
                    Corre::Izquierda => c::shl(x, reg(a), Fuente::Imm(n), 0),
                    Corre::Derecha => c::shr(x, reg(a), Fuente::Imm(n), false, 0),
                    Corre::DerechaConSigno => c::shr(x, reg(a), Fuente::Imm(n), true, 0),
                };
                self.poner(w, Clase::Alu, Some(x), [Some(reg(a)), None, None]);
            }
        }
    }
}
