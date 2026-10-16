//! ** B6 de `docs/plan/PLAN_TITAN_AFINADO.md`: lo que ya se hizo no se
//! repite. Aparte de `e2/mod.rs` (L6a: aquel pasaba de 1.000 lineas) porque
//! contesta otra pregunta: no baja ni reparte ni emite, MIRA el codigo ya
//! bajado y quita lo que sobra.

use super::{usa_y_define, Opnd, I, V};

/// ** B6: LO QUE YA SE HIZO NO SE REPITE (las "expresiones disponibles" de
/// GCC y LLVM). Por cada sitio del codigo, lo que se sabe seguro venga de
/// donde venga: que la celda `i` de una lista ya se miro (`i < largo`), y
/// que registro tiene ya el valor de `l[i]`. Se sabe al leerla o escribirla,
/// y se olvida cuando `i`, el largo o quien la tiene cambian, o cuando se
/// escribe en esa lista (en cualquier celda). Donde se sabe, la celda no se
/// mira otra vez y no se lee otra vez: `while j >= 0 and a[j] > v` y el
/// `a[j]` del cuerpo son la misma. Y M4a: si `a * a` ya cupo en 64 bits,
/// `|a| <= 3.037.000.499`, y `a + 1` (o `a - K` chico) cabe: `while d * d
/// <= k` ... `d = d + 1` no mira el desborde de la suma. Y M1b: lo que un
/// salto deja saber en cada camino (`if n < 2` ... por el otro, `n >= 2`):
/// lo menos y lo mas que vale, y `n - 1` no desborda.
pub(super) fn quita_lo_repetido(code: &mut [I]) {
    use std::collections::BTreeSet;
    // (indice, largo) miradas; (base, indice, quien la tiene) leidas; los
    // que su cuadrado ya cupo (M4a)
    // y lo menos y lo mas que vale cada uno, por los saltos (M1b)
    type Sabe = (BTreeSet<(V, V)>, BTreeSet<(V, V, V)>, BTreeSet<V>, std::collections::BTreeMap<V, (i64, i64)>);
    let n = code.len();
    let donde: std::collections::HashMap<usize, usize> = code.iter().enumerate().filter_map(|(k, i)| if let I::Label(l) = i { Some((*l, k)) } else { None }).collect();
    let mut antes: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, i) in code.iter().enumerate() {
        let sig: Vec<usize> = match i {
            I::Jmp(l) => vec![donde[l]],
            I::Br(_, _, _, l) => vec![donde[l], k + 1],
            I::Ret(_) | I::Sale(..) => Vec::new(),
            _ => vec![k + 1],
        };
        for t in sig {
            if t < n {
                antes[t].push(k);
            }
        }
    }
    let paso = |i: &I, mut s: Sabe| -> Sabe {
        if let (_, Some(d)) = usa_y_define(i) {
            s.0.retain(|&(a, b)| a != d && b != d);
            s.1.retain(|&(a, b, c)| a != d && b != d && c != d);
            s.2.remove(&d);
            s.3.remove(&d);
        }
        match i {
            I::Idx { d, base, len, i, byte: false, .. } => {
                s.0.insert((*i, *len));
                if d != i && d != base {
                    s.1.insert((*base, *i, *d));
                }
            }
            I::Idx { len, i, .. } => {
                s.0.insert((*i, *len));
            }
            I::Pon { base, len, i, v, byte, .. } => {
                s.0.insert((*i, *len));
                s.1.retain(|&(b, _, _)| b != *base);
                if let (Opnd::V(v), false) = (v, byte) {
                    s.1.insert((*base, *i, *v));
                }
            }
            I::Bin("*", d, a, Opnd::V(b), _, _) if a == b && d != a => {
                s.2.insert(*a);
            }
            _ => {}
        }
        s
    };
    // hacia delante, hasta que nada cambie: lo que se sabe al entrar en cada
    // instruccion es lo que se sabe en TODAS las que llevan a ella
    let mut entra: Vec<Option<Sabe>> = vec![None; n];
    entra[0] = Some(Default::default());
    loop {
        let mut cambio = false;
        for k in 0..n {
            let nuevo: Option<Sabe> = if k == 0 {
                Some(Default::default())
            } else {
                let mut acc: Option<Sabe> = None;
                for &p in &antes[k] {
                    if let Some(e) = &entra[p] {
                        let mut sale = paso(&code[p], e.clone());
                        if let I::Br(op, a, Opnd::K(c), l) = &code[p] {
                            let salta = donde[l] == k;
                            if salta != (k == p + 1) {
                                if let Some(r) = rango(op, *c, salta) {
                                    let v = sale.3.entry(*a).or_insert((i64::MIN, i64::MAX));
                                    *v = (v.0.max(r.0), v.1.min(r.1));
                                }
                            }
                        }
                        acc = Some(match acc {
                            None => sale,
                            Some(a) => {
                                let r = a.3.iter().filter_map(|(v, x)| sale.3.get(v).map(|y| (*v, (x.0.min(y.0), x.1.max(y.1))))).collect();
                                (a.0.intersection(&sale.0).copied().collect(), a.1.intersection(&sale.1).copied().collect(), a.2.intersection(&sale.2).copied().collect(), r)
                            }
                        });
                    }
                }
                acc
            };
            if nuevo != entra[k] {
                entra[k] = nuevo;
                cambio = true;
            }
        }
        if !cambio {
            break;
        }
    }
    for k in 0..n {
        let Some(s) = entra[k].clone() else { continue };
        match &mut code[k] {
            I::Idx { d, base, len, i, byte: false, .. } if s.1.iter().any(|&(b, x, _)| b == *base && x == *i) && s.0.contains(&(*i, *len)) => {
                let quien = s.1.iter().find(|&&(b, x, _)| b == *base && x == *i).unwrap().2;
                code[k] = I::Mov(*d, quien);
            }
            I::Idx { len, i, mira, .. } | I::Pon { len, i, mira, .. } if s.0.contains(&(*i, *len)) => *mira = false,
            // `i + 1` de un i que ya se miro (`i < largo <= el mayor`): cabe
            I::Bin("+", _, a, Opnd::K(1), _, seguro) if s.0.iter().any(|&(x, _)| x == *a) => *seguro = true,
            // `a + K` de un `a` cuyo cuadrado cupo: |a| < 2^32, y |K| < 2^32
            I::Bin("+" | "-", _, a, Opnd::K(k), _, seguro) if s.2.contains(a) && k.unsigned_abs() < 1 << 32 => *seguro = true,
            // M1b: `a + K` o `a - K` que, con lo que el salto dejo saber, cabe
            I::Bin(op @ ("+" | "-"), _, a, Opnd::K(k), _, seguro) if s.3.contains_key(a) => {
                let (lo, hi) = s.3[a];
                let k = if *op == "-" { k.checked_neg() } else { Some(*k) };
                if k.is_some_and(|k| lo.checked_add(k).is_some() && hi.checked_add(k).is_some()) {
                    *seguro = true;
                }
            }
            // un divisor que vale 1 o mas: ni cero ni -1
            I::Bin("/" | "%", _, _, Opnd::V(b), _, seguro) if s.3.get(b).is_some_and(|r| r.0 >= 1) => *seguro = true,
            _ => {}
        }
    }
}

/// Lo que deja saber `a op c` (o, si no `salta`, que no se cumplio): lo menos
/// y lo mas que vale `a`.
fn rango(op: &str, c: i64, salta: bool) -> Option<(i64, i64)> {
    let op = if salta {
        op
    } else {
        match op {
            "<" => ">=",
            "<=" => ">",
            ">" => "<=",
            ">=" => "<",
            "==" => "!=",
            _ => "==",
        }
    };
    match op {
        "<" => Some((i64::MIN, c.checked_sub(1)?)),
        "<=" => Some((i64::MIN, c)),
        ">" => Some((c.checked_add(1)?, i64::MAX)),
        ">=" => Some((c, i64::MAX)),
        "==" => Some((c, c)),
        _ => None,
    }
}
