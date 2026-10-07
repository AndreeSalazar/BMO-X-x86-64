//! **La CIFRA del mapa de la CPU** (A9b, 06-10): lo que la casa entiende de
//! un PSO -- su [`Enlace`]: los Programas compilados del DXIL o del SM5 y
//! como se cosen -- en bytes, y de vuelta. Con ella la CPU RECUERDA lo que
//! compilo (en BMO-X, en ESTRATOS: `proton-x/<juego>/mapas`) y el arranque
//! siguiente no vuelve a leer el DXIL, que es lo que mas cuesta al montar
//! miles de PSO. Es la otra mitad del paquete de VC1 (`PLAN_VERRANO.md`): la
//! de la GPU es el `.bsf` vivo (`bmo_proton_x_sm86::vivo`).
//!
//! ```text
//!    la cabecera   "CIFRA1" y dos ceros, y la HUELLA del codigo de este
//!                  crate (`build.rs`): si cambia una linea, lo cifrado
//!                  antes no se descifra (y se vuelve a compilar)
//!    el cuerpo     cada campo en su orden, little-endian; los Vec con su
//!                  medida delante; cada enum con su numero
//! ```
//!
//! Por que no se pierde nada al agregar algo: el cifrado de cada tipo es un
//! `match` (o un literal) EXHAUSTIVO -- una operacion o un campo nuevo no
//! compila hasta que se agrega aqui --, y la prueba pasa ida y vuelta
//! TODOS los sombreadores de `prueba/` (DXIL y SM5) y exige el mismo Enlace.
//!
//! capa: puro -- bytes que entran, bytes que salen

use alloc::string::String;
use alloc::vec::Vec;

use crate::bufer::{Atomo, Modo};
use crate::dxil::olas::{Numero, Ola};
use crate::dxil::programa::{Comparacion, Computo, Conversion, Lectura, Op, OpEntera, Programa};
use crate::dxil::ranuras::{Lugar, Ranuras};
use crate::dxil::recursos::Geometria;
use crate::lote::{Bloque, Enlace, EnlaceGs, Fuente};
use crate::mates::Mate;

/// La huella del codigo de este crate (ver `build.rs`).
pub const HUELLA_FUENTE: &str = env!("BMO_HUELLA_FUENTE");

/// La cabecera de lo cifrado.
const MAGIA: &[u8; 8] = b"CIFRA1\0\0";

/// Quien lee: los bytes y por donde va.
pub struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Lector<'a> {
    /// Para leer `b` desde el principio.
    pub fn nuevo(b: &'a [u8]) -> Self {
        Lector { b, i: 0 }
    }

    /// Si ya no queda nada por leer.
    pub fn al_final(&self) -> bool {
        self.i == self.b.len()
    }

    fn tomar_n(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
}

/// Lo que se cifra.
pub trait Cifra: Sized {
    fn poner(&self, o: &mut Vec<u8>);
    fn tomar(e: &mut Lector) -> Option<Self>;
}

macro_rules! numeros {
    ($($t:ty),*) => {$(
        impl Cifra for $t {
            fn poner(&self, o: &mut Vec<u8>) {
                o.extend_from_slice(&self.to_le_bytes());
            }
            fn tomar(e: &mut Lector) -> Option<Self> {
                Some(<$t>::from_le_bytes(e.tomar_n(core::mem::size_of::<$t>())?.try_into().ok()?))
            }
        }
    )*};
}
numeros!(u8, i8, u16, u32, u64);

impl Cifra for bool {
    fn poner(&self, o: &mut Vec<u8>) {
        o.push(*self as u8);
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        match u8::tomar(e)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

/// Un f32 por sus BITS (la carga de un NaN incluida).
impl Cifra for f32 {
    fn poner(&self, o: &mut Vec<u8>) {
        self.to_bits().poner(o);
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        Some(f32::from_bits(u32::tomar(e)?))
    }
}

impl Cifra for usize {
    fn poner(&self, o: &mut Vec<u8>) {
        (*self as u64).poner(o);
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        usize::try_from(u64::tomar(e)?).ok()
    }
}

impl<T: Cifra> Cifra for Vec<T> {
    fn poner(&self, o: &mut Vec<u8>) {
        (self.len() as u32).poner(o);
        self.iter().for_each(|x| x.poner(o));
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        let n = u32::tomar(e)? as usize;
        // Nada mide menos de un byte: una medida mayor que lo que queda miente.
        if n > e.b.len() - e.i {
            return None;
        }
        (0..n).map(|_| T::tomar(e)).collect()
    }
}

impl Cifra for String {
    fn poner(&self, o: &mut Vec<u8>) {
        self.as_bytes().to_vec().poner(o);
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        String::from_utf8(Vec::<u8>::tomar(e)?).ok()
    }
}

impl<T: Cifra> Cifra for Option<T> {
    fn poner(&self, o: &mut Vec<u8>) {
        match self {
            None => o.push(0),
            Some(x) => {
                o.push(1);
                x.poner(o);
            }
        }
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        match u8::tomar(e)? {
            0 => Some(None),
            1 => Some(Some(T::tomar(e)?)),
            _ => None,
        }
    }
}

impl<T: Cifra + Copy + Default, const N: usize> Cifra for [T; N] {
    fn poner(&self, o: &mut Vec<u8>) {
        self.iter().for_each(|x| x.poner(o));
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        let mut a = [T::default(); N];
        for x in a.iter_mut() {
            *x = T::tomar(e)?;
        }
        Some(a)
    }
}

macro_rules! tuplas {
    ($(($($t:ident),+)),*) => {$(
        #[allow(non_snake_case)]
        impl<$($t: Cifra),+> Cifra for ($($t,)+) {
            fn poner(&self, o: &mut Vec<u8>) {
                let ($($t,)+) = self;
                $($t.poner(o);)+
            }
            fn tomar(e: &mut Lector) -> Option<Self> {
                Some(($($t::tomar(e)?,)+))
            }
        }
    )*};
}
tuplas!((A), (A, B), (A, B, C), (A, B, C, D), (A, B, C, D, E), (A, B, C, D, E, F), (A, B, C, D, E, F, G), (A, B, C, D, E, F, G, H), (A, B, C, D, E, F, G, H, I));

/// Un enum SIN datos: su posicion en la lista. El `match` de debajo obliga a
/// que la lista los tenga TODOS.
macro_rules! unidades {
    ($t:ident: $($v:ident),+ $(,)?) => {
        impl Cifra for $t {
            fn poner(&self, o: &mut Vec<u8>) {
                const TODOS: &[$t] = &[$($t::$v),+];
                match *self { $($t::$v)|+ => {} }
                o.push(TODOS.iter().position(|x| x == self).expect("en la lista") as u8);
            }
            fn tomar(e: &mut Lector) -> Option<Self> {
                const TODOS: &[$t] = &[$($t::$v),+];
                TODOS.get(u8::tomar(e)? as usize).copied()
            }
        }
    };
}
unidades!(Modo: Tipado, Estructurado, Crudo, Textura);
unidades!(Atomo: Suma, Y, O, Xor, MinConSigno, MaxConSigno, MinSinSigno, MaxSinSigno, Cambia, CambiaSiIgual);
unidades!(Numero: Float, Entero, SinSigno);
unidades!(Comparacion: Menor, MenorIgual, Mayor, MayorIgual, Igual, Distinto, MenorSinSigno, MenorIgualSinSigno, MayorSinSigno, MayorIgualSinSigno);
unidades!(OpEntera: Resta, Mul, Shl, ShrL, ShrA, Y, O, OX, MinS, MaxS, MinU, MaxU, DivU, RemU, DivS, RemS);
unidades!(Conversion: EnteroAFloat, SinSignoAFloat, FloatAEntero, FloatASinSigno);
unidades!(
    Mate: Sin, Cos, Tan, Exp2, Log2, Frac, RedondoPar, Suelo, Techo, Trunca, F16aF32, F32aF16, Acos, Asin, Atan, Cosh, Senh, Tanh, EsNan, EsInf, EsFinito, EsNormal, InvierteBits, CuentaBits,
    PrimerBitBajo, PrimerBitAlto, PrimerBitAltoConSigno
);

/// Un struct: sus campos en orden. Tomarlo es un LITERAL: un campo que
/// falte en la lista no compila.
macro_rules! estructura {
    ($t:ident { $($f:ident),+ $(,)? }) => {
        impl Cifra for $t {
            fn poner(&self, o: &mut Vec<u8>) {
                $(self.$f.poner(o);)+
            }
            fn tomar(e: &mut Lector) -> Option<Self> {
                Some($t { $($f: Cifra::tomar(e)?),+ })
            }
        }
    };
}
estructura!(Lugar { espacio, registro, vista });
estructura!(Ranuras { texturas, muestreadores, cbuffers, dinamicas, uavs, pasos });
estructura!(Computo { hilos, compartida, temprana });
estructura!(Bloque { fila, filas });
estructura!(Geometria { entrada, salida, maximo });
estructura!(Programa { ops, iniciales, entradas, salidas, lee, filas_cb, ranuras, computo });
estructura!(EnlaceGs { programa, desde_vs, info });
estructura!(Enlace { vs, ps, desde_ia, posicion, desde_vs, pos_ps, objetivos, profundidad_ps, ranuras, constantes, gs });

impl Cifra for Fuente {
    fn poner(&self, o: &mut Vec<u8>) {
        match *self {
            Fuente::Ia(i) => {
                o.push(0);
                i.poner(o);
            }
            Fuente::Vertice => o.push(1),
            Fuente::Instancia => o.push(2),
        }
    }
    fn tomar(e: &mut Lector) -> Option<Self> {
        Some(match u8::tomar(e)? {
            0 => Fuente::Ia(usize::tomar(e)?),
            1 => Fuente::Vertice,
            2 => Fuente::Instancia,
            _ => return None,
        })
    }
}

/// Un enum CON datos: cada variante, su numero y sus campos (en una tupla).
/// Poner y tomar salen de la MISMA lista: no pueden ir en otro orden.
macro_rules! variantes {
    ($t:ident: $($n:literal $v:ident $({ $($f:ident),+ })? $(( $($g:ident),+ ))?;)+) => {
        impl Cifra for $t {
            fn poner(&self, o: &mut Vec<u8>) {
                match *self {
                    $($t::$v $({ $($f),+ })? $(( $($g),+ ))? => {
                        o.push($n);
                        $( ($($f,)+).poner(o); )?
                        $( ($($g,)+).poner(o); )?
                    })+
                }
            }
            fn tomar(e: &mut Lector) -> Option<Self> {
                Some(match u8::tomar(e)? {
                    $($n => {
                        $( let ($($f,)+) = Cifra::tomar(e)?; )?
                        $( let ($($g,)+) = Cifra::tomar(e)?; )?
                        $t::$v $({ $($f),+ })? $(( $($g),+ ))?
                    })+
                    _ => return None,
                })
            }
        }
    };
}

variantes!(Lectura:
    0 Muestra;
    1 Nivel;
    2 Carga { enteros };
    3 Medidas;
    4 Bufer(m);
    5 MedidasBufer(m);
    6 Junta { canal };
    7 Compara;
    8 JuntaCompara { canal };
    9 Gradientes { compara };
    10 Lod { sujeta };
);

variantes!(Ola:
    0 EsPrimero;
    1 Indice;
    2 AlgunoCierto;
    3 TodosCiertos;
    4 TodosIguales { float };
    5 Papeleta;
    6 LeerCarril;
    7 LeerPrimero;
    8 Activa { op, num };
    9 Bits { op };
    10 Prefijo { op, num };
    11 CuentaBits;
    12 PrefijoBits;
    13 Cuadro;
    14 Cruza(k);
    15 Derivada { y, fina, muestra };
);

variantes!(Op:
    0 Entrada { d, elemento, componente };
    1 Salida { s, elemento, componente };
    2 Constantes { d, fila, cb };
    3 Mul { d, a, b };
    4 Add { d, a, b };
    5 Sub { d, a, b };
    6 Div { d, a, b };
    7 Mad { d, a, b, c };
    8 Dot { d, n, a, b };
    9 Rsqrt { d, a };
    10 Sqrt { d, a };
    11 Saturate { d, a };
    12 Abs { d, a };
    13 Mate { d, a, f };
    14 Min { d, a, b };
    15 Max { d, a, b };
    16 Muestra { d, t, s, u, v, g };
    17 EligeTextura { i, rango };
    18 IdHilo { d, que, c };
    19 Barrera;
    20 LeeCompartida { d, base, n, i };
    21 EscribeCompartida { base, n, i, s };
    22 EscribeUav { u, modo, i, desp, z, v, mascara };
    23 LeeUav { d, u, modo, i, desp, z };
    24 MedidasUav { d, u, modo };
    25 Contador { d, u, inc };
    26 Atomico { d, u, modo, i, desp, z, como, v, igual };
    27 EntradaDe { d, vertice, elemento, componente };
    28 Emite { flujo };
    29 Corta { flujo };
    30 Lee { d, t, s, como, c, nivel, desp };
    31 Compara { d, a, b, como, entero };
    32 Elige { d, c, a, b };
    33 Copia { d, a };
    34 SumaEntera { d, a, b };
    35 Entera { d, a, b, op };
    36 Convierte { d, a, como };
    37 Si { c };
    38 SiNo;
    39 FinSi;
    40 Bucle;
    41 RomperSi { c, si_cero };
    42 Romper;
    43 Continuar;
    44 FinBucle;
    45 Descarta { c };
    46 LeeIndexado { d, base, n, i };
    47 EscribeIndexado { base, n, i, s };
    48 ConstantesEn { d, fila, filas, i, cb };
    49 Ola { d, a, b, que };
    50 AtomicoCompartido { d, base, n, i, v, como };
);

/// **Cifrar un Enlace**: la cabecera, la huella del codigo y el Enlace.
pub fn cifrar(en: &Enlace) -> Vec<u8> {
    let mut o = Vec::with_capacity(4096);
    o.extend_from_slice(MAGIA);
    String::from(HUELLA_FUENTE).poner(&mut o);
    en.poner(&mut o);
    o
}

/// **Descifrar un Enlace**: solo si la cabecera, la huella del codigo de
/// AHORA y cada byte cuadran (lo que sobre al final, tambien es `None`).
pub fn descifrar(b: &[u8]) -> Option<Enlace> {
    let mut e = Lector { b, i: 0 };
    if e.tomar_n(8)? != MAGIA || String::tomar(&mut e)? != HUELLA_FUENTE {
        return None;
    }
    let en = Enlace::tomar(&mut e)?;
    (e.i == b.len()).then_some(en)
}

/// **Cifrar lo compilado de un PSO** (lo que guarda la casa): los nombres
/// de sus funciones de entrada y su Enlace.
pub fn cifrar_compilado(nombres: &(String, String), en: &Enlace) -> Vec<u8> {
    let mut o = cifrar(en);
    nombres.poner(&mut o);
    o
}

/// Lo contrario de [`cifrar_compilado`], con las mismas reglas que
/// [`descifrar`].
pub fn descifrar_compilado(b: &[u8]) -> Option<((String, String), Enlace)> {
    let mut e = Lector { b, i: 0 };
    if e.tomar_n(8)? != MAGIA || String::tomar(&mut e)? != HUELLA_FUENTE {
        return None;
    }
    let en = Enlace::tomar(&mut e)?;
    let nombres = <(String, String)>::tomar(&mut e)?;
    e.al_final().then_some((nombres, en))
}
