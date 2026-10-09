//! **LOS VALORES QUE MUEREN DEJAN SU SITIO** (LB7a, 09-10): el Programa de
//! una gpu fn grande -- la matriz de un fotograma del cubo son ~1500 valores
//! -- con sus registros RENUMERADOS para que dos valores que no viven a la
//! vez compartan uno. Cada registro es una palabra en la pila de quien llama
//! ([`crate::REGISTROS`]): el mismo calculo cabe en muchas menos.
//!
//! Solo se hace cuando el Programa no cabe tal cual: lo que ya cabia sale
//! como siempre, byte a byte.
//!
//! ```text
//!    un TEMPORAL   se escribe UNA vez, fuera de todo si y bucle, antes de
//!                  que nadie lo lea: vive de su escritura a su ultima
//!                  lectura, y despues su sitio es de otro
//!    un FIJO       lo demas -- una constante (nadie la escribe), una
//!                  variable (se escribe mas de una vez), lo que se lee
//!                  antes de escribirse o se escribe dentro de un si o de
//!                  un bucle --: tiene un sitio SUYO desde el principio, con
//!                  su valor inicial, hasta su ultimo uso
//!    los bucles    lo que vive al entrar en uno y se usa dentro, vive
//!                  hasta su fin: la vuelta siguiente lo vuelve a leer
//! ```
//!
//! El oraculo lo comprueba en cada build: la bateria compara esta tarjeta,
//! que corre el Programa compactado, con el interprete de la casa, que corre
//! el de siempre.

use bmo_prometeo::programa::{Op, Programa, Reg};

/// Lo que lee y lo que escribe una operacion de las que corre esta tarjeta
/// (`no_sabe` dice cuales); `None` con cualquier otra.
fn toca(op: &Op) -> Option<(Vec<Reg>, Vec<Reg>)> {
    Some(match *op {
        Op::Entrada { d, .. } => (vec![], vec![d]),
        Op::Salida { s, .. } => (vec![s], vec![]),
        Op::Mul { d, a, b } | Op::Add { d, a, b } | Op::Sub { d, a, b } | Op::Div { d, a, b } | Op::Min { d, a, b } | Op::Max { d, a, b } | Op::SumaEntera { d, a, b } => (vec![a, b], vec![d]),
        Op::Entera { d, a, b, .. } | Op::Compara { d, a, b, .. } => (vec![a, b], vec![d]),
        Op::Mad { d, a, b, c } | Op::Elige { d, c, a, b } => (vec![a, b, c], vec![d]),
        Op::Dot { d, n, a, b } => (a[..n as usize].iter().chain(&b[..n as usize]).copied().collect(), vec![d]),
        Op::Rsqrt { d, a } | Op::Sqrt { d, a } | Op::Saturate { d, a } | Op::Abs { d, a } | Op::Copia { d, a } | Op::Convierte { d, a, .. } => (vec![a], vec![d]),
        Op::Si { c } | Op::RomperSi { c, .. } => (vec![c], vec![]),
        Op::SiNo | Op::FinSi | Op::Bucle | Op::Romper | Op::Continuar | Op::FinBucle => (vec![], vec![]),
        _ => return None,
    })
}

/// La misma operacion con cada registro pasado por `f`.
fn renombrar(op: &Op, f: &dyn Fn(Reg) -> Reg) -> Op {
    match *op {
        Op::Entrada { d, elemento, componente } => Op::Entrada { d: f(d), elemento, componente },
        Op::Salida { s, elemento, componente } => Op::Salida { s: f(s), elemento, componente },
        Op::Mul { d, a, b } => Op::Mul { d: f(d), a: f(a), b: f(b) },
        Op::Add { d, a, b } => Op::Add { d: f(d), a: f(a), b: f(b) },
        Op::Sub { d, a, b } => Op::Sub { d: f(d), a: f(a), b: f(b) },
        Op::Div { d, a, b } => Op::Div { d: f(d), a: f(a), b: f(b) },
        Op::Min { d, a, b } => Op::Min { d: f(d), a: f(a), b: f(b) },
        Op::Max { d, a, b } => Op::Max { d: f(d), a: f(a), b: f(b) },
        Op::SumaEntera { d, a, b } => Op::SumaEntera { d: f(d), a: f(a), b: f(b) },
        Op::Entera { d, a, b, op } => Op::Entera { d: f(d), a: f(a), b: f(b), op },
        Op::Compara { d, a, b, como, entero } => Op::Compara { d: f(d), a: f(a), b: f(b), como, entero },
        Op::Mad { d, a, b, c } => Op::Mad { d: f(d), a: f(a), b: f(b), c: f(c) },
        Op::Elige { d, c, a, b } => Op::Elige { d: f(d), c: f(c), a: f(a), b: f(b) },
        Op::Dot { d, n, a, b } => Op::Dot { d: f(d), n, a: a.map(f), b: b.map(f) },
        Op::Rsqrt { d, a } => Op::Rsqrt { d: f(d), a: f(a) },
        Op::Sqrt { d, a } => Op::Sqrt { d: f(d), a: f(a) },
        Op::Saturate { d, a } => Op::Saturate { d: f(d), a: f(a) },
        Op::Abs { d, a } => Op::Abs { d: f(d), a: f(a) },
        Op::Copia { d, a } => Op::Copia { d: f(d), a: f(a) },
        Op::Convierte { d, a, como } => Op::Convierte { d: f(d), a: f(a), como },
        Op::Si { c } => Op::Si { c: f(c) },
        Op::RomperSi { c, si_cero } => Op::RomperSi { c: f(c), si_cero },
        otra => otra,
    }
}

/// **El Programa compactado**, o `None` si tiene una operacion que esta
/// tarjeta no corre (entonces lo dice `no_sabe`). Mismas operaciones, mismas
/// entradas y salidas, y cada valor en un registro que no pisa a ninguno vivo.
pub fn compactar(p: &Programa) -> Option<Programa> {
    let n = p.iniciales.len();
    let mut lecturas: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut escrituras: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut hondo = vec![0usize; p.ops.len()];
    let mut abiertos: Vec<(usize, bool)> = Vec::new();
    let mut bucles: Vec<(usize, usize)> = Vec::new();
    for (i, op) in p.ops.iter().enumerate() {
        hondo[i] = abiertos.len();
        let (lee, escribe) = toca(op)?;
        for r in lee {
            lecturas.get_mut(r as usize)?.push(i);
        }
        for r in escribe {
            escrituras.get_mut(r as usize)?.push(i);
        }
        match op {
            Op::Si { .. } => abiertos.push((i, false)),
            Op::Bucle => abiertos.push((i, true)),
            Op::FinSi => {
                abiertos.pop();
            }
            Op::FinBucle => {
                if let Some((b, true)) = abiertos.pop() {
                    bucles.push((b, i));
                }
            }
            _ => {}
        }
    }
    // Cada valor: si es fijo, y de donde a donde vive.
    let mut fijo = vec![false; n];
    let mut vive: Vec<Option<(usize, usize)>> = vec![None; n];
    for r in 0..n {
        let (l, e) = (&lecturas[r], &escrituras[r]);
        if l.is_empty() && e.is_empty() {
            continue;
        }
        let ultimo = l.iter().chain(e).copied().max().unwrap_or(0);
        fijo[r] = match (e.as_slice(), l.first()) {
            ([w], Some(&x)) => x <= *w || hondo[*w] > 0,
            ([w], None) => hondo[*w] > 0,
            _ => true,
        };
        let desde = if fijo[r] { 0 } else { e[0] };
        vive[r] = Some((desde, ultimo));
    }
    // Vivo al entrar en un bucle y usado dentro: hasta su fin.
    let mut cambia = true;
    while cambia {
        cambia = false;
        for &(b, f) in &bucles {
            for (r, v) in vive.iter_mut().enumerate() {
                if let Some((desde, hasta)) = v {
                    let dentro = lecturas[r].iter().chain(&escrituras[r]).any(|&x| x > b && x < f);
                    if *desde < b && dentro && *hasta < f {
                        *hasta = f;
                        cambia = true;
                    }
                }
            }
        }
    }
    // Los sitios: los fijos primero, uno cada uno; despues cada temporal, en
    // el primero libre cuando nace (libre: quien lo ocupaba ya murio ANTES).
    let mut sitio: Vec<Option<Reg>> = vec![None; n];
    let mut ocupado_hasta: Vec<usize> = Vec::new();
    let mut iniciales: Vec<f32> = Vec::new();
    for r in (0..n).filter(|&r| fijo[r] && vive[r].is_some()) {
        sitio[r] = Some(ocupado_hasta.len() as Reg);
        ocupado_hasta.push(vive[r].unwrap().1);
        iniciales.push(p.iniciales[r]);
    }
    let mut temporales: Vec<usize> = (0..n).filter(|&r| !fijo[r] && vive[r].is_some()).collect();
    temporales.sort_by_key(|&r| (vive[r].unwrap().0, r));
    for r in temporales {
        let (desde, hasta) = vive[r].unwrap();
        let s = match ocupado_hasta.iter().position(|&h| h < desde) {
            Some(s) => s,
            None => {
                ocupado_hasta.push(0);
                iniciales.push(0.0);
                ocupado_hasta.len() - 1
            }
        };
        ocupado_hasta[s] = hasta;
        sitio[r] = Some(s as Reg);
    }
    if ocupado_hasta.len() > Reg::MAX as usize {
        return None;
    }
    let mapa = |r: Reg| sitio.get(r as usize).copied().flatten().unwrap_or(0);
    let ops = p.ops.iter().map(|op| renombrar(op, &mapa)).collect();
    Some(Programa { ops, iniciales, ..p.clone() })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_prometeo::programa::Comparacion;

    fn programa(ops: Vec<Op>, iniciales: Vec<f32>, entradas: usize) -> Programa {
        Programa { ops, iniciales, entradas, salidas: 1, lee: (1 << entradas) - 1, filas_cb: 0, ranuras: Default::default(), computo: Default::default() }
    }

    fn correr(p: &Programa, x: f32) -> u32 {
        let mut s = [[0.0f32; 4]; 1];
        let mut regs = Vec::new();
        p.correr(&[[x, 0.0, 0.0, 0.0]], &[], &mut s, &mut regs);
        s[0][0].to_bits()
    }

    /// ** Una cadena de mil sumas: mil temporales que viven de uno en uno
    /// caben en un punado de sitios, y dan lo mismo en el interprete.
    #[test]
    fn a_chain_of_temporaries_shares_its_places() {
        let mut ops = vec![Op::Entrada { d: 1, elemento: 0, componente: 0 }];
        let mut previo = 1;
        for k in 0..1000u16 {
            let d = 2 + k;
            ops.push(Op::Add { d, a: previo, b: 0 });
            ops.push(Op::Mul { d: d + 1000, a: d, b: d });
            previo = d;
        }
        ops.push(Op::Salida { s: previo, elemento: 0, componente: 0 });
        let mut iniciales = vec![0.0; 2002];
        iniciales[0] = 0.5;
        let p = programa(ops, iniciales, 1);
        let c = compactar(&p).unwrap();
        assert!(c.iniciales.len() <= 4, "{} sitios", c.iniciales.len());
        for x in [0.0f32, 1.5, -3.25, 1.0e30, f32::NAN] {
            let (a, b) = (correr(&p, x), correr(&c, x));
            assert!(a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan()), "{x}");
        }
    }

    /// ** UN BUCLE, una variable y un `si`: lo que vive al entrar al bucle y
    /// se lee dentro no se pisa entre vueltas, y la variable tiene su sitio
    /// con su valor inicial. `r = 1; x5 veces: t = r * x; r = t + 1`, y un
    /// `si` que cambia la salida.
    #[test]
    fn a_loop_keeps_what_it_reads_and_a_variable_keeps_its_place() {
        let ops = vec![
            Op::Entrada { d: 1, elemento: 0, componente: 0 },
            Op::Mul { d: 6, a: 1, b: 1 },
            Op::Bucle,
            Op::Compara { d: 7, a: 3, b: 4, como: Comparacion::MayorIgual, entero: false },
            Op::RomperSi { c: 7, si_cero: false },
            Op::Mul { d: 8, a: 2, b: 6 },
            Op::Add { d: 9, a: 8, b: 5 },
            Op::Copia { d: 2, a: 9 },
            Op::Add { d: 10, a: 3, b: 5 },
            Op::Copia { d: 3, a: 10 },
            Op::FinBucle,
            Op::Compara { d: 11, a: 2, b: 0, como: Comparacion::Mayor, entero: false },
            Op::Si { c: 11 },
            Op::Sub { d: 12, a: 2, b: 5 },
            Op::Copia { d: 2, a: 12 },
            Op::FinSi,
            Op::Salida { s: 2, elemento: 0, componente: 0 },
        ];
        let mut iniciales = vec![0.0; 13];
        iniciales[0] = 100.0;
        iniciales[2] = 1.0;
        iniciales[4] = 5.0;
        iniciales[5] = 1.0;
        let p = programa(ops, iniciales, 1);
        let c = compactar(&p).unwrap();
        assert!(c.iniciales.len() < p.iniciales.len());
        for x in [0.0f32, 0.5, -1.0, 3.0, 1.0e10] {
            assert_eq!(correr(&p, x), correr(&c, x), "{x}");
        }
    }

    /// Lo que esta tarjeta no corre no se compacta.
    #[test]
    fn what_this_card_does_not_run_is_not_compacted() {
        let p = programa(vec![Op::IdHilo { d: 0, que: 0, c: 0 }, Op::Salida { s: 0, elemento: 0, componente: 0 }], vec![0.0], 0);
        assert!(compactar(&p).is_none());
    }
}
