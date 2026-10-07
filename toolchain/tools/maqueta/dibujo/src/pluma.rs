//! **La pluma** (S4): el discontinuo, y el CONTORNO de un trazo que no es
//! de pluma redonda.
//!
//! La pluma de la casa es redonda y el pintor la hace exacta. Una pluma
//! `butt`/`square` o unas esquinas `miter`/`bevel` se convierten aqui, al
//! compilar, en el contorno del trazo: un poligono que se rellena con la
//! regla `nonzero` (la que une sin agujeros lo que se pisa -- un trazo que
//! se cruza consigo mismo no se vacia en el cruce).
//!
//! ** Cada esquina y cada punta dan SIEMPRE los mismos puntos (una esquina
//! en pico que pasa de `stroke-miterlimit` y se corta, tambien): dos pasos
//! de una animacion (S7) no cambian de forma porque una esquina se doble mas.

use crate::geo::P;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Punta {
    Recta,
    Redonda,
    Cuadrada,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Esquina {
    Pico,
    Redonda,
    Cortada,
}

/// Los puntos de un arco de punta o esquina redonda en un contorno.
const ARCO: usize = 9;

/// **El discontinuo**: los trozos de `caminos` que caen en los "trazos" de
/// `patron` (unidades del dibujo), empezando `desfase` dentro del patron.
pub fn discontinuo(caminos: &[Vec<P>], cerrados: &[bool], patron: &[f64], desfase: f64) -> Vec<Vec<P>> {
    let mut patron = patron.to_vec();
    if patron.len() % 2 == 1 {
        patron.extend_from_slice(&patron.clone());
    }
    let total: f64 = patron.iter().sum();
    if total <= 0.0 || patron.iter().any(|v| *v < 0.0) {
        return caminos.to_vec();
    }
    let mut out = Vec::new();
    for (k, c) in caminos.iter().enumerate() {
        let mut pts = c.clone();
        if cerrados.get(k).copied().unwrap_or(false) && pts.len() > 1 {
            pts.push(pts[0]);
        }
        // Donde empieza el patron: el desfase, dentro de un ciclo.
        let mut d = desfase.rem_euclid(total);
        let mut i = 0usize;
        while d >= patron[i] {
            d -= patron[i];
            i = (i + 1) % patron.len();
        }
        let mut queda = patron[i] - d;
        let mut pinta = i % 2 == 0;
        let mut actual: Vec<P> = if pinta { vec![pts[0]] } else { Vec::new() };
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let largo = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let mut hecho = 0.0;
            while largo - hecho > queda {
                hecho += queda;
                let t = hecho / largo;
                let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                if pinta {
                    actual.push(p);
                    out.push(std::mem::take(&mut actual));
                } else {
                    actual = vec![p];
                }
                pinta = !pinta;
                i = (i + 1) % patron.len();
                queda = patron[i];
            }
            queda -= largo - hecho;
            if pinta {
                actual.push(b);
            }
        }
        if pinta && actual.len() > 1 {
            out.push(actual);
        }
    }
    out
}

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
fn suma(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
fn por(a: P, k: f64) -> P {
    (a.0 * k, a.1 * k)
}
fn unidad(a: P) -> P {
    let l = (a.0 * a.0 + a.1 * a.1).sqrt();
    if l < 1e-12 {
        (0.0, 0.0)
    } else {
        (a.0 / l, a.1 / l)
    }
}
fn normal(d: P) -> P {
    (-d.1, d.0)
}
fn cruz(a: P, b: P) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// Un arco de `ARCO` puntos alrededor de `c`, de `a` a `b`, por el camino
/// CORTO (el de fuera de una esquina, y cada cuarto de una punta redonda).
fn arco(c: P, a: P, b: P) -> Vec<P> {
    use std::f64::consts::{PI, TAU};
    let r = ((a.0 - c.0).powi(2) + (a.1 - c.1).powi(2)).sqrt();
    let t0 = (a.1 - c.1).atan2(a.0 - c.0);
    let mut dt = (b.1 - c.1).atan2(b.0 - c.0) - t0;
    if dt > PI {
        dt -= TAU;
    }
    if dt <= -PI {
        dt += TAU;
    }
    (0..ARCO).map(|k| {
        let t = t0 + dt * k as f64 / (ARCO - 1) as f64;
        (c.0 + r * t.cos(), c.1 + r * t.sin())
    }).collect()
}

/// Las direcciones de cada tramo; un tramo de largo cero toma la del vecino.
fn direcciones(p: &[P], cerrado: bool) -> Vec<P> {
    let n = p.len();
    let tramos = if cerrado { n } else { n - 1 };
    let mut d: Vec<P> = (0..tramos).map(|k| unidad(sub(p[(k + 1) % n], p[k]))).collect();
    for k in 0..d.len() {
        if d[k] == (0.0, 0.0) {
            if let Some(v) = d[..k].iter().rev().chain(d[k..].iter()).find(|v| **v != (0.0, 0.0)) {
                d[k] = *v;
            }
        }
    }
    d
}

/// Una esquina en `p` entre las direcciones `d0` y `d1`, por el lado `s`.
fn esquina(out: &mut Vec<P>, p: P, d0: P, d1: P, s: f64, h: f64, tipo: Esquina, limite: f64) {
    let (a, b) = (suma(p, por(normal(d0), s * h)), suma(p, por(normal(d1), s * h)));
    let fuera = s * cruz(d0, d1) < 0.0;
    let n = match tipo {
        Esquina::Redonda => ARCO,
        _ => 3,
    };
    if !fuera {
        // Por dentro: por el vertice, que nunca deja hueco.
        out.push(a);
        out.extend(std::iter::repeat(p).take(n - 2));
        out.push(b);
        return;
    }
    match tipo {
        Esquina::Redonda => out.extend(arco(p, a, b)),
        Esquina::Pico | Esquina::Cortada => {
            let coseno = (d0.0 * d1.0 + d0.1 * d1.1).clamp(-1.0, 1.0);
            let medio = ((1.0 + coseno) / 2.0).sqrt();
            let pico_ok = tipo == Esquina::Pico && medio > 1e-9 && 1.0 / medio <= limite;
            let centro = if pico_ok {
                // La interseccion de las dos orillas.
                let den = cruz(d0, d1);
                if den.abs() < 1e-12 {
                    por(suma(a, b), 0.5)
                } else {
                    let t = cruz(sub(b, a), d1) / den;
                    suma(a, por(d0, t))
                }
            } else {
                por(suma(a, b), 0.5)
            };
            out.extend([a, centro, b]);
        }
    }
}

/// Una punta en `p` hacia `d` (la direccion de SALIDA del trazo), de la
/// orilla `+` a la `-`.
fn punta(out: &mut Vec<P>, p: P, d: P, h: f64, tipo: Punta) {
    let n = normal(d);
    let (a, b) = (suma(p, por(n, h)), suma(p, por(n, -h)));
    match tipo {
        Punta::Recta => out.extend([a, b]),
        Punta::Cuadrada => out.extend([a, suma(a, por(d, h)), suma(b, por(d, h)), b]),
        Punta::Redonda => {
            let lejos = suma(p, por(d, h));
            let mut v = arco(p, a, lejos);
            v.extend(arco(p, lejos, b).into_iter().skip(1));
            out.extend(v);
        }
    }
}

/// **El contorno de un trazo** de ancho `ancho` por una polilinea. Uno o dos
/// poligonos (dos si el camino se cierra: la orilla de fuera y la de dentro
/// al reves), para rellenar con `nonzero`.
pub fn contorno(p: &[P], cerrado: bool, ancho: f64, cab: Punta, esq: Esquina, limite: f64) -> Vec<Vec<P>> {
    let h = ancho / 2.0;
    if p.is_empty() || h <= 0.0 {
        return Vec::new();
    }
    let cerrado = cerrado && p.len() > 2;
    let todo_igual = p.iter().all(|q| (q.0 - p[0].0).abs() < 1e-12 && (q.1 - p[0].1).abs() < 1e-12);
    if p.len() == 1 || todo_igual {
        // Un punto: la punta sola, como el navegador (nada si es recta).
        let mut v = Vec::new();
        match cab {
            Punta::Recta => return Vec::new(),
            _ => {
                punta(&mut v, p[0], (1.0, 0.0), h, cab);
                punta(&mut v, p[0], (-1.0, 0.0), h, cab);
            }
        }
        return vec![v];
    }
    let d = direcciones(p, cerrado);
    let n = p.len();
    let orilla = |s: f64| -> Vec<P> {
        let mut v = Vec::new();
        if cerrado {
            for k in 0..n {
                let (d0, d1) = (d[(k + d.len() - 1) % d.len()], d[k % d.len()]);
                esquina(&mut v, p[k], d0, d1, s, h, esq, limite);
            }
        } else {
            v.push(suma(p[0], por(normal(d[0]), s * h)));
            for k in 1..n - 1 {
                esquina(&mut v, p[k], d[k - 1], d[k], s, h, esq, limite);
            }
            v.push(suma(p[n - 1], por(normal(d[n - 2]), s * h)));
        }
        v
    };
    if cerrado {
        let a = orilla(1.0);
        let mut b = orilla(-1.0);
        b.reverse();
        return vec![a, b];
    }
    let mut v = orilla(1.0);
    v.pop();
    punta(&mut v, p[n - 1], d[n - 2], h, cab);
    let mut atras = orilla(-1.0);
    atras.pop();
    atras.remove(0);
    atras.reverse();
    v.extend(atras);
    let mut ini = Vec::new();
    punta(&mut ini, p[0], por(d[0], -1.0), h, cab);
    v.extend(ini);
    vec![v]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area(v: &[P]) -> f64 {
        let n = v.len();
        (0..n).map(|k| cruz(v[k], v[(k + 1) % n])).sum::<f64>() / 2.0
    }

    #[test]
    fn un_trazo_recto_de_punta_recta_es_su_rectangulo() {
        let c = contorno(&[(0.0, 0.0), (10.0, 0.0)], false, 2.0, Punta::Recta, Esquina::Pico, 4.0);
        assert_eq!(c.len(), 1);
        assert!((area(&c[0]).abs() - 20.0).abs() < 1e-9, "{}", area(&c[0]));
    }

    #[test]
    fn la_punta_cuadrada_alarga_media_pluma_por_cada_lado() {
        let c = contorno(&[(0.0, 0.0), (10.0, 0.0)], false, 2.0, Punta::Cuadrada, Esquina::Pico, 4.0);
        assert!((area(&c[0]).abs() - 24.0).abs() < 1e-9, "{}", area(&c[0]));
    }

    #[test]
    fn la_esquina_en_pico_llega_a_la_punta_y_la_cortada_no() {
        let l = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        let pico = contorno(&l, false, 2.0, Punta::Recta, Esquina::Pico, 4.0);
        let corte = contorno(&l, false, 2.0, Punta::Recta, Esquina::Cortada, 4.0);
        let maxx = |c: &[Vec<P>]| c[0].iter().map(|p| p.0).fold(f64::MIN, f64::max);
        assert!((maxx(&pico) - 11.0).abs() < 1e-9);
        assert_eq!(pico[0].len(), corte[0].len(), "la misma forma, para animar");
        // El corte se queda por dentro del pico.
        assert!(area(&corte[0]).abs() < area(&pico[0]).abs());
    }

    #[test]
    fn el_discontinuo_corta_en_su_patron() {
        let t = discontinuo(&[vec![(0.0, 0.0), (10.0, 0.0)]], &[false], &[2.0, 3.0], 0.0);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], vec![(0.0, 0.0), (2.0, 0.0)]);
        assert_eq!(t[1], vec![(5.0, 0.0), (7.0, 0.0)]);
        let d = discontinuo(&[vec![(0.0, 0.0), (10.0, 0.0)]], &[false], &[2.0, 3.0], 1.0);
        assert_eq!(d[0], vec![(0.0, 0.0), (1.0, 0.0)]);
    }

    /// Las vueltas de unos poligonos alrededor de `q` (la regla `nonzero`).
    fn vueltas(polis: &[Vec<P>], q: P) -> i32 {
        let mut w = 0;
        for v in polis {
            for k in 0..v.len() {
                let (a, b) = (v[k], v[(k + 1) % v.len()]);
                if (a.1 <= q.1) != (b.1 <= q.1) {
                    let x = a.0 + (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1);
                    if x > q.0 {
                        w += if b.1 > a.1 { 1 } else { -1 };
                    }
                }
            }
        }
        w
    }

    #[test]
    fn un_camino_cerrado_es_un_anillo_con_su_agujero() {
        let c = contorno(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true, 2.0, Punta::Recta, Esquina::Pico, 4.0);
        assert_eq!(c.len(), 2);
        assert!(area(&c[0]) * area(&c[1]) < 0.0, "las dos orillas en sentidos contrarios");
        assert_eq!(vueltas(&c, (5.0, 5.0)), 0, "el centro es agujero");
        for q in [(0.5, 5.0), (-0.5, 5.0), (5.0, 10.5), (9.5, 0.5), (10.9, -0.9)] {
            assert_ne!(vueltas(&c, q), 0, "{q:?} esta en el trazo");
        }
        for q in [(1.5, 5.0), (-1.5, 5.0), (11.5, 11.5)] {
            assert_eq!(vueltas(&c, q), 0, "{q:?} esta fuera");
        }
    }
}

#[cfg(test)]
mod recorte {
    use crate::geo::{area, convexo, recortar};

    #[test]
    fn un_cuadrado_recortado_por_otro_es_su_interseccion() {
        let a = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let c = convexo(&[(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)]).unwrap();
        assert!((area(&recortar(&a, &c)).abs() - 25.0).abs() < 1e-9);
        assert!(convexo(&[(0.0, 0.0), (10.0, 0.0), (5.0, 2.0), (10.0, 10.0), (0.0, 10.0)]).is_none(), "una flecha no es convexa");
    }
}
