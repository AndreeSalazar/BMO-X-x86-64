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
/// `a[j]` del cuerpo son la misma.
pub(super) fn quita_lo_repetido(code: &mut [I]) {
    use std::collections::BTreeSet;
    // (indice, largo) miradas; (base, indice, quien la tiene) leidas
    type Sabe = (BTreeSet<(V, V)>, BTreeSet<(V, V, V)>);
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
                        let sale = paso(&code[p], e.clone());
                        acc = Some(match acc {
                            None => sale,
                            Some(a) => (a.0.intersection(&sale.0).copied().collect(), a.1.intersection(&sale.1).copied().collect()),
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
            _ => {}
        }
    }
}
