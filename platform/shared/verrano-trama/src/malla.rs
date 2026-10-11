//! **LAS MALLAS DE ESTUDIO** (VE3, 11-10): los triangulos de un modelo -- un
//! personaje, un coche, lo que sale de un `.obj` -- y el ARBOL que deja a
//! cada rayo mirar solo los que puede tocar (un BVH: cajas dentro de cajas).
//!
//! ```text
//!    el arbol       cada nodo es una caja; sus dos hijos parten sus
//!                   triangulos en dos; las hojas, 4 como mucho. Se parte
//!                   por donde menos cuesta (la heuristica de AREA, la SAH
//!                   de pbrt y de Cycles), probando 12 cortes por eje
//!    el rayo        entra en una caja solo si la cruza (las tres "losas");
//!                   del hijo mas cercano primero, y lo que queda detras del
//!                   golpe ya hallado no se mira
//!    un triangulo   Moller-Trumbore: el golpe y su sitio (u, v) dentro de
//!                   el, sin el plano
//!    la normal      la del triangulo (facetas), o SUAVE: la media de las
//!                   de sus vertices, mezclada por (u, v)
//! ```
//!
//! [carril]  VERDE     cuentas puras; ni un `unsafe`
//! [cuesta]  NADA      un arbol mal pierde golpes en la foto, nada mas
//!
//! Lo que cuesta en la CPU: al hacerla, n log n; cada rayo, ~log n cajas.
//! La prueba compara el arbol con mirar TODOS los triangulos: los mismos
//! golpes.

use alloc::vec;
use alloc::vec::Vec;

use crate::estudio::{cruz, mas, menos, por, punto, unidad, V3};
use bmo_prometeo::programa::raiz;

const EPS: f32 = 1e-4;
/// Triangulos por hoja, como mucho.
const HOJA: usize = 4;
/// Cortes que se prueban por eje.
const CORTES: usize = 12;

/// Un triangulo, listo para el rayo: un vertice, sus dos aristas, su normal
/// (la de las facetas) y la de cada vertice (la suave).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tri {
    a: V3,
    e1: V3,
    e2: V3,
    n: V3,
    na: V3,
    nb: V3,
    nc: V3,
}

/// Un nodo del arbol: su caja y, si es hoja, sus triangulos
/// (`primero..primero + cuenta`); si no, su hijo de la izquierda es el
/// SIGUIENTE nodo y el de la derecha, `primero`.
#[derive(Clone, Copy, Debug)]
struct Nodo {
    min: V3,
    max: V3,
    primero: u32,
    cuenta: u32,
}

/// **Una malla**: sus triangulos (ordenados como las hojas) y su arbol.
#[derive(Clone, Debug)]
pub struct Malla {
    tris: Vec<Tri>,
    nodos: Vec<Nodo>,
}

/// Lo que da un golpe a una malla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Golpe {
    pub t: f32,
    /// La normal para la luz (la suave, si la hay).
    pub n: V3,
    /// La del triangulo: de que lado se le dio.
    pub geo: V3,
}

fn min3(a: V3, b: V3) -> V3 {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}
fn max3(a: V3, b: V3) -> V3 {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}
fn area(min: V3, max: V3) -> f32 {
    let d = menos(max, min);
    if d[0] < 0.0 {
        return 0.0;
    }
    2.0 * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
}
const VACIA: (V3, V3) = ([f32::MAX; 3], [f32::MIN; 3]);

impl Malla {
    /// **La malla de unos vertices y sus caras** (cada una, tres indices).
    /// `suave`: la normal de cada vertice es la media de las de sus caras
    /// (por su area); si no, se ven las facetas.
    pub fn nueva(vertices: &[V3], caras: &[[u32; 3]], suave: bool) -> Malla {
        let mut normales = vec![[0.0f32; 3]; vertices.len()];
        if suave {
            for c in caras {
                let (a, b, cc) = (vertices[c[0] as usize], vertices[c[1] as usize], vertices[c[2] as usize]);
                // sin normalizar: pesa por su area
                let n = cruz(menos(b, a), menos(cc, a));
                for &k in c {
                    normales[k as usize] = mas(normales[k as usize], n);
                }
            }
        }
        let mut tris: Vec<Tri> = caras
            .iter()
            .filter_map(|c| {
                let (a, b, cc) = (vertices[c[0] as usize], vertices[c[1] as usize], vertices[c[2] as usize]);
                let (e1, e2) = (menos(b, a), menos(cc, a));
                let x = cruz(e1, e2);
                if punto(x, x) <= 0.0 {
                    return None; // un triangulo sin area no se ve
                }
                let n = unidad(x);
                let suya = |k: u32| {
                    let m = normales[k as usize];
                    if suave && punto(m, m) > 0.0 {
                        unidad(m)
                    } else {
                        n
                    }
                };
                Some(Tri { a, e1, e2, n, na: suya(c[0]), nb: suya(c[1]), nc: suya(c[2]) })
            })
            .collect();
        let mut nodos = Vec::with_capacity(2 * tris.len() / HOJA + 1);
        if !tris.is_empty() {
            let n = tris.len();
            construye(&mut tris, 0, n, &mut nodos);
        }
        Malla { tris, nodos }
    }

    pub fn triangulos(&self) -> usize {
        self.tris.len()
    }

    /// La caja de toda la malla.
    pub fn caja(&self) -> (V3, V3) {
        self.nodos.first().map_or(VACIA, |n| (n.min, n.max))
    }

    /// **El golpe mas cercano** de un rayo (`d` unidad) antes de `t_max`,
    /// por el ARBOL.
    pub fn golpea(&self, o: V3, d: V3, t_max: f32) -> Option<Golpe> {
        if self.nodos.is_empty() {
            return None;
        }
        let inv = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        let mut mejor: Option<(f32, usize, f32, f32)> = None;
        let mut lim = t_max;
        let mut pila = [0u32; 64];
        let mut alto = 1;
        while alto > 0 {
            alto -= 1;
            let nodo = &self.nodos[pila[alto] as usize];
            if cruza(nodo, o, inv, lim).is_none() {
                continue;
            }
            if nodo.cuenta > 0 {
                for k in nodo.primero as usize..(nodo.primero + nodo.cuenta) as usize {
                    if let Some((t, u, v)) = tri(&self.tris[k], o, d, lim) {
                        lim = t;
                        mejor = Some((t, k, u, v));
                    }
                }
                continue;
            }
            // los dos hijos: el mas cercano, ENCIMA de la pila (sale antes)
            let (i, dd) = (pila[alto] + 1, nodo.primero);
            let (ti, td) = (cruza(&self.nodos[i as usize], o, inv, lim), cruza(&self.nodos[dd as usize], o, inv, lim));
            let (lejos, cerca) = match (ti, td) {
                (Some(a), Some(b)) if a <= b => (Some(dd), Some(i)),
                (Some(_), Some(_)) => (Some(i), Some(dd)),
                (Some(_), None) => (None, Some(i)),
                (None, Some(_)) => (None, Some(dd)),
                (None, None) => (None, None),
            };
            for k in [lejos, cerca].into_iter().flatten() {
                if alto < pila.len() {
                    pila[alto] = k;
                    alto += 1;
                }
            }
        }
        mejor.map(|(t, k, u, v)| golpe_de(&self.tris[k], t, u, v))
    }

    /// El mismo golpe mirando TODOS los triangulos (el juez del arbol).
    pub fn golpea_todos(&self, o: V3, d: V3, t_max: f32) -> Option<Golpe> {
        let mut mejor: Option<(f32, usize, f32, f32)> = None;
        let mut lim = t_max;
        for (k, tr) in self.tris.iter().enumerate() {
            if let Some((t, u, v)) = tri(tr, o, d, lim) {
                lim = t;
                mejor = Some((t, k, u, v));
            }
        }
        mejor.map(|(t, k, u, v)| golpe_de(&self.tris[k], t, u, v))
    }
}

fn golpe_de(tr: &Tri, t: f32, u: f32, v: f32) -> Golpe {
    let n = unidad(mas(mas(por(tr.na, 1.0 - u - v), por(tr.nb, u)), por(tr.nc, v)));
    Golpe { t, n, geo: tr.n }
}

/// Moller-Trumbore: (t, u, v) si el rayo da en el triangulo antes de `t_max`.
fn tri(tr: &Tri, o: V3, d: V3, t_max: f32) -> Option<(f32, f32, f32)> {
    let p = cruz(d, tr.e2);
    let det = punto(tr.e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = menos(o, tr.a);
    let u = punto(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cruz(s, tr.e1);
    let v = punto(d, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = punto(tr.e2, q) * inv;
    (t > EPS && t < t_max).then_some((t, u, v))
}

/// Donde entra el rayo en la caja del nodo (las tres losas), si entra antes
/// de `lim`.
fn cruza(n: &Nodo, o: V3, inv: V3, lim: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, lim);
    for k in 0..3 {
        let (a, b) = ((n.min[k] - o[k]) * inv[k], (n.max[k] - o[k]) * inv[k]);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        // `max`/`min` de f32 dejan pasar un NaN (0 * infinito): no corta
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

fn caja_tri(t: &Tri) -> (V3, V3) {
    let (b, c) = (mas(t.a, t.e1), mas(t.a, t.e2));
    (min3(t.a, min3(b, c)), max3(t.a, max3(b, c)))
}

fn centro(t: &Tri) -> V3 {
    por(mas(t.a, mas(mas(t.a, t.e1), mas(t.a, t.e2))), 1.0 / 3.0)
}

/// **Hace el nodo de `tris[desde..hasta]`** (y, debajo, los suyos), en
/// orden: el de la izquierda va justo detras.
fn construye(tris: &mut [Tri], desde: usize, hasta: usize, nodos: &mut Vec<Nodo>) {
    let yo = nodos.len();
    let (mut min, mut max) = VACIA;
    let (mut cmin, mut cmax) = VACIA;
    for t in &tris[desde..hasta] {
        let (a, b) = caja_tri(t);
        (min, max) = (min3(min, a), max3(max, b));
        let c = centro(t);
        (cmin, cmax) = (min3(cmin, c), max3(cmax, c));
    }
    nodos.push(Nodo { min, max, primero: desde as u32, cuenta: (hasta - desde) as u32 });
    let n = hasta - desde;
    if n <= HOJA {
        return;
    }
    // ** la SAH: por cada eje, 12 cubos de centros; el corte que menos cuesta
    // (area de cada lado por sus triangulos)
    let mut mejor: Option<(f32, usize, f32)> = None;
    for eje in 0..3 {
        let (lo, hi) = (cmin[eje], cmax[eje]);
        if hi - lo <= 1e-9 {
            continue;
        }
        let mut cubos = [(0usize, VACIA); CORTES];
        let escala = CORTES as f32 / (hi - lo);
        for t in &tris[desde..hasta] {
            let b = (((centro(t)[eje] - lo) * escala) as usize).min(CORTES - 1);
            let (a, z) = caja_tri(t);
            let c = &mut cubos[b];
            c.0 += 1;
            c.1 = (min3(c.1 .0, a), max3(c.1 .1, z));
        }
        for corte in 1..CORTES {
            let lado = |r: core::ops::Range<usize>| {
                r.fold((0usize, VACIA), |(k, (a, z)), i| (k + cubos[i].0, (min3(a, cubos[i].1 .0), max3(z, cubos[i].1 .1))))
            };
            let (ki, (ai, zi)) = lado(0..corte);
            let (kd, (ad, zd)) = lado(corte..CORTES);
            if ki == 0 || kd == 0 {
                continue;
            }
            let coste = area(ai, zi) * ki as f32 + area(ad, zd) * kd as f32;
            if mejor.is_none_or(|m| coste < m.0) {
                mejor = Some((coste, eje, lo + corte as f32 / escala));
            }
        }
    }
    // si partir no sale a cuenta (y caben), hoja
    let hoja = area(min, max) * n as f32;
    let medio = match mejor {
        Some((coste, eje, plano)) if coste < hoja || n > 4 * HOJA => {
            let mut i = desde;
            for j in desde..hasta {
                if centro(&tris[j])[eje] < plano {
                    tris.swap(i, j);
                    i += 1;
                }
            }
            i
        }
        _ if n <= 4 * HOJA => return,
        // todos los centros en el mismo sitio: por la mitad
        _ => desde + n / 2,
    };
    let medio = if medio == desde || medio == hasta { desde + n / 2 } else { medio };
    nodos[yo].cuenta = 0;
    construye(tris, desde, medio, nodos);
    let derecha = nodos.len();
    nodos[yo].primero = derecha as u32;
    construye(tris, medio, hasta, nodos);
}

// ---------------------------------------------------------------------------
// LAS FORMAS DE LA CASA (sin seno ni coseno de fuera)
// ---------------------------------------------------------------------------

/// Coseno y seno de un angulo pequenio (|x| <= pi/4), por su serie: con
/// estos terminos, el error no llega a 1e-7.
fn giro_pequenio(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let c = 1.0 - x2 / 2.0 * (1.0 - x2 / 12.0 * (1.0 - x2 / 30.0 * (1.0 - x2 / 56.0)));
    let s = x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))));
    (c, s)
}

/// Los `n` puntos de la circunferencia unidad, girando de 2pi/n en 2pi/n
/// (y volviendo a la unidad cada vez, para no ir torciendose).
fn circulo(n: usize) -> Vec<(f32, f32)> {
    let (c, s) = giro_pequenio(2.0 * core::f32::consts::PI / n as f32);
    let mut p = (1.0f32, 0.0f32);
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(p);
        let q = (p.0 * c - p.1 * s, p.0 * s + p.1 * c);
        let r = raiz(q.0 * q.0 + q.1 * q.1);
        p = (q.0 / r, q.1 / r);
    }
    v
}

/// **Una icosfera**: el icosaedro, partido `nivel` veces (cada triangulo
/// en 4) y llevado a la esfera. Nivel 0: 20 caras; 4: 5120.
pub fn icosfera(centro: V3, radio: f32, nivel: u32) -> (Vec<V3>, Vec<[u32; 3]>) {
    let t = (1.0 + raiz(5.0)) / 2.0;
    let mut v: Vec<V3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|&p| unidad(p))
    .collect();
    let mut caras: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..nivel {
        // el punto medio de cada arista, una vez (un mapa pequenio: ordenado)
        let mut medios: Vec<((u32, u32), u32)> = Vec::new();
        let mut medio = |a: u32, b: u32, v: &mut Vec<V3>| -> u32 {
            let k = (a.min(b), a.max(b));
            match medios.binary_search_by(|e| e.0.cmp(&k)) {
                Ok(i) => medios[i].1,
                Err(i) => {
                    v.push(unidad(por(mas(v[a as usize], v[b as usize]), 0.5)));
                    let id = v.len() as u32 - 1;
                    medios.insert(i, (k, id));
                    id
                }
            }
        };
        let mut nuevas = Vec::with_capacity(caras.len() * 4);
        for &[a, b, c] in &caras {
            let (ab, bc, ca) = (medio(a, b, &mut v), medio(b, c, &mut v), medio(c, a, &mut v));
            nuevas.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        caras = nuevas;
    }
    (v.iter().map(|&p| mas(centro, por(p, radio))).collect(), caras)
}

/// **Una dona** (un toro) tumbada (su eje, +y): radio `grande` hasta el
/// centro del tubo, `chico` el del tubo; `n` y `m` vueltas.
pub fn dona(centro: V3, grande: f32, chico: f32, n: usize, m: usize) -> (Vec<V3>, Vec<[u32; 3]>) {
    let (cn, cm) = (circulo(n), circulo(m));
    let mut v = Vec::with_capacity(n * m);
    for &(c, s) in &cn {
        for &(cc, ss) in &cm {
            let r = grande + chico * cc;
            v.push(mas(centro, [r * c, chico * ss, r * s]));
        }
    }
    let mut caras = Vec::with_capacity(2 * n * m);
    for i in 0..n {
        for j in 0..m {
            let k = |i: usize, j: usize| ((i % n) * m + j % m) as u32;
            // hacia fuera: la normal, de (u x v) en este orden
            caras.push([k(i, j), k(i, j + 1), k(i + 1, j)]);
            caras.push([k(i + 1, j), k(i, j + 1), k(i + 1, j + 1)]);
        }
    }
    (v, caras)
}

/// **Lo que dice un `.obj`** (Wavefront, el de todos los modelos): sus
/// vertices (`v x y z`) y sus caras (`f a b c ...`, con `a/t/n` o sin, y
/// los indices negativos -- desde el final --); una cara de mas de tres
/// lados se parte en abanico. Lo demas (texturas, materiales) se salta.
/// `None` si una linea que importa no se entiende.
pub fn desde_obj(texto: &str) -> Option<(Vec<V3>, Vec<[u32; 3]>)> {
    let mut v: Vec<V3> = Vec::new();
    let mut caras = Vec::new();
    for linea in texto.lines() {
        let mut partes = linea.split_whitespace();
        match partes.next() {
            Some("v") => {
                let mut p = [0.0f32; 3];
                for x in &mut p {
                    *x = partes.next()?.parse().ok()?;
                }
                v.push(p);
            }
            Some("f") => {
                let idx: Option<Vec<u32>> = partes
                    .map(|p| {
                        let i: i64 = p.split('/').next()?.parse().ok()?;
                        let i = if i < 0 { v.len() as i64 + i } else { i - 1 };
                        (0..v.len() as i64).contains(&i).then_some(i as u32)
                    })
                    .collect();
                let idx = idx?;
                if idx.len() < 3 {
                    return None;
                }
                for k in 1..idx.len() - 1 {
                    caras.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    Some((v, caras))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::estudio::Azar;

    fn direccion(az: &mut Azar) -> V3 {
        loop {
            let p = [2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0, 2.0 * az.uno() - 1.0];
            let l = punto(p, p);
            if l > 1e-4 && l < 1.0 {
                return unidad(p);
            }
        }
    }

    /// *** EL ARBOL NO SE PIERDE NADA: miles de rayos al azar (desde fuera
    /// y desde dentro de la dona), por el arbol y mirando TODOS los
    /// triangulos, dan los MISMOS golpes. Y el arbol mira mucho menos.
    #[test]
    fn the_tree_finds_the_same_hits_as_looking_at_everything() {
        let (v, c) = dona([0.1, 0.0, -0.2], 0.6, 0.25, 48, 24);
        let m = Malla::nueva(&v, &c, true);
        assert_eq!(m.triangulos(), 2 * 48 * 24);
        let mut az = Azar::nuevo(11);
        let mut golpes = 0;
        for k in 0..3000 {
            let o = if k % 2 == 0 { por(direccion(&mut az), 2.0) } else { por(direccion(&mut az), 0.5) };
            // hacia el centro, con algo de azar; o a cualquier lado
            let d = if k % 3 == 0 { direccion(&mut az) } else { unidad(mas(por(o, -1.0), por(direccion(&mut az), 0.6))) };
            let (a, b) = (m.golpea(o, d, f32::MAX), m.golpea_todos(o, d, f32::MAX));
            match (a, b) {
                (Some(a), Some(b)) => {
                    assert!((a.t - b.t).abs() < 1e-5, "rayo {k}: arbol {a:?}, todos {b:?}");
                    golpes += 1;
                }
                (None, None) => {}
                _ => panic!("rayo {k}: arbol {a:?}, todos {b:?}"),
            }
        }
        assert!(golpes > 1000, "{golpes} golpes");
        // el arbol tiene sus hojas pequenias
        assert!(m.nodos.iter().all(|n| n.cuenta as usize <= 4 * HOJA));
    }

    /// La icosfera ES una esfera: los rayos que le dan son los que le dan a
    /// la esfera de verdad (salvo un borde fino), y su normal suave es casi
    /// la de la esfera.
    #[test]
    fn the_icosphere_is_a_sphere() {
        let (centro, radio) = ([0.2, 0.1, -0.3], 0.5);
        let (v, c) = icosfera(centro, radio, 4);
        assert_eq!(c.len(), 20 * 256);
        let m = Malla::nueva(&v, &c, true);
        let mut az = Azar::nuevo(5);
        let (mut distintos, mut normal_peor) = (0, 1.0f32);
        let n = 4000;
        for _ in 0..n {
            let o = mas(centro, por(direccion(&mut az), 2.0));
            let d = unidad(mas(menos(centro, o), por(direccion(&mut az), 0.6)));
            // la esfera de verdad
            let oc = menos(o, centro);
            let b = punto(oc, d);
            let disc = b * b - (punto(oc, oc) - radio * radio);
            let de_verdad = disc > 0.0;
            match m.golpea(o, d, f32::MAX) {
                Some(g) if de_verdad => {
                    let p = mas(o, por(d, g.t));
                    normal_peor = normal_peor.min(punto(g.n, unidad(menos(p, centro))));
                }
                None if !de_verdad => {}
                _ => distintos += 1,
            }
        }
        assert!(distintos * 100 < n, "{distintos} de {n} distintos");
        assert!(normal_peor > 0.999, "la peor normal: {normal_peor}");
    }

    /// El `.obj`: un cuadrado (dos triangulos), los indices `a/t/n` y los
    /// negativos; lo que no se entiende, `None`.
    #[test]
    fn an_obj_file_is_read() {
        let texto = "# un cuadrado\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nf 1/1/1 2/1/1 3/1/1 4/1/1\nf -4 -3 -2\n";
        let (v, c) = desde_obj(texto).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(c, vec![[0, 1, 2], [0, 2, 3], [0, 1, 2]]);
        assert!(desde_obj("v 0 0\n").is_none());
        assert!(desde_obj("v 0 0 0\nf 1 2 3\n").is_none());
    }

    /// El circulo de la casa, sin seno ni coseno de fuera: el punto de un
    /// cuarto de vuelta es (0, 1).
    #[test]
    fn the_house_circle_turns_right() {
        let c = circulo(48);
        assert!((c[12].0).abs() < 1e-5 && (c[12].1 - 1.0).abs() < 1e-5, "{:?}", c[12]);
    }
}
