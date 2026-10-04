//! **EL MOVIMIENTO** (P3b, 04-10): de un estado a otro, trazo a trazo.
//!
//! La transicion se pinta en el ESCRITORIO, en cada fotograma, y alli no hay
//! maquetacion ni arbol: solo trazos. Asi que aqui, en el anfitrion, se
//! empareja cada trazo del estado de salida con el suyo del de llegada, y el
//! aparato solo los mezcla (`bmo_pinta::entre_piezas`).
//!
//! ```text
//!    las cajas    por lo que SON (su camino de spans desde la raiz), no por
//!                 su sitio en la lista: una que pasa a absoluta sigue siendo
//!                 ella
//!    los trazos   por su RANURA (brillo, fondo, borde, texto...): el fondo
//!                 con el fondo
//!    lo que no    un resplandor que aparece NACE de alcance 0; un color que
//!    casa         pasa a degradado se vuelve degradado de un color; lo demas
//!                 que solo esta en un lado SALTA a mitad, como hace CSS con
//!                 lo discreto
//!    el tiempo    la `transition` de la caja en el estado de LLEGADA
//! ```
//!
//! La foto de una transicion del anfitrion (`foto::foto_en`) y el codigo que
//! pinta el escritorio (`rust.rs`) salen de ESTA lista: la foto es el
//! oraculo de lo que hace el aparato.

use std::collections::{BTreeMap, HashMap};

use bmo_maqueta_layout::{Frame, Laid, Rect};

use crate::orden::{es_suave, nombre_de, trazos_de_estilo, Ranura, Trazo};

/// **Un trazo en movimiento**: el de salida, el de llegada (alguno puede
/// faltar) y como se mueve.
#[derive(Clone, PartialEq, Debug)]
pub struct Par {
    pub a: Option<Trazo>,
    pub b: Option<Trazo>,
    pub retraso: u32,
    pub dura: u32,
    pub curva: [i32; 4],
    /// De que caja es, para el comentario del codigo generado.
    pub de: String,
}

/// Lo que tarda todo: el par que mas tarde acaba.
pub fn duracion(pares: &[Par]) -> u32 {
    pares.iter().map(|p| p.retraso + p.dura).max().unwrap_or(0)
}

/// **Los pares de la transicion de `a` a `b`**, en orden de pintado.
pub fn pares(a: &Laid, b: &Laid) -> Vec<Par> {
    let mut llegada: HashMap<Vec<usize>, &Frame> = HashMap::new();
    indexar(&b.root, &mut Vec::new(), &mut llegada);
    let mut out = Vec::new();
    recorrer(&a.root, &llegada, &mut Vec::new(), &mut out);
    out
}

fn indexar<'a>(f: &'a Frame, camino: &mut Vec<usize>, out: &mut HashMap<Vec<usize>, &'a Frame>) {
    camino.push(f.span.start);
    out.insert(camino.clone(), f);
    for c in &f.children {
        indexar(c, camino, out);
    }
    camino.pop();
}

fn recorrer(fa: &Frame, llegada: &HashMap<Vec<usize>, &Frame>, camino: &mut Vec<usize>, out: &mut Vec<Par>) {
    camino.push(fa.span.start);
    let fb = llegada.get(camino.as_slice()).copied().unwrap_or(fa);
    caja(fa, fb, out);
    for c in &fa.children {
        recorrer(c, llegada, camino, out);
    }
    camino.pop();
}

/// **Los pares de UNA caja** entre dos estilos (H8): el reposo y el
/// `:hover` de la misma caja, que tienen la misma geometria.
pub fn pares_caja(fa: &Frame, fb: &Frame) -> Vec<Par> {
    let mut out = Vec::new();
    caja(fa, fb, &mut out);
    out
}

fn caja(fa: &Frame, fb: &Frame, out: &mut Vec<Par>) {
    let suave = es_suave(&fa.style) || es_suave(&fb.style);
    let mut ranuras: BTreeMap<Ranura, (Option<Trazo>, Option<Trazo>)> = BTreeMap::new();
    for (r, t) in trazos_de_estilo(fa, &fa.style, suave) {
        ranuras.entry(r).or_default().0 = Some(t);
    }
    for (r, t) in trazos_de_estilo(fb, &fb.style, suave) {
        ranuras.entry(r).or_default().1 = Some(t);
    }
    let (retraso, dura, curva) = match fb.style.transicion {
        Some(t) => (t.retraso, t.ms, t.curva),
        None => (0, 0, [0, 0, 1000, 1000]),
    };
    let de = nombre_de(fb);
    let (ra, rb) = ((fa.rect, fa.style.border_radius), (fb.rect, fb.style.border_radius));
    for (_, (a, b)) in ranuras {
        let (a, b) = casar(a, b, ra, rb);
        out.push(Par { a, b, retraso, dura, curva, de: de.clone() });
    }
}

/// Que los dos trazos de una ranura se puedan mezclar, si se puede. `ca` y
/// `cb` son la caja (y su radio) en cada estado.
fn casar(a: Option<Trazo>, b: Option<Trazo>, ca: (Rect, u32), cb: (Rect, u32)) -> (Option<Trazo>, Option<Trazo>) {
    match (a, b) {
        // Un resplandor que aparece nace de NADA (alcance 0, fuerza 0) en la
        // caja de SALIDA, y crece con ella; uno que se va se apaga en la de
        // llegada. Nacer en la de llegada lo pintaria alrededor de una caja
        // que todavia no esta ahi.
        (None, Some(Trazo::Resplandor { r, radio, alcance, argb })) => (
            Some(Trazo::Resplandor { r: ca.0, radio: ca.1, alcance: 0, argb: argb & 0x00FF_FFFF }),
            Some(Trazo::Resplandor { r, radio, alcance, argb }),
        ),
        (Some(Trazo::Resplandor { r, radio, alcance, argb }), None) => (
            Some(Trazo::Resplandor { r, radio, alcance, argb }),
            Some(Trazo::Resplandor { r: cb.0, radio: cb.1, alcance: 0, argb: argb & 0x00FF_FFFF }),
        ),
        // Un color liso que pasa a degradado: degradado de un solo color.
        (Some(Trazo::Caja { r, radio, color }), Some(d @ Trazo::Degradado { vertical, .. })) => {
            (Some(Trazo::Degradado { r, radio, de: color, a: color, vertical }), Some(d))
        }
        (Some(d @ Trazo::Degradado { vertical, .. }), Some(Trazo::Caja { r, radio, color })) => {
            (Some(d), Some(Trazo::Degradado { r, radio, de: color, a: color, vertical }))
        }
        otro => otro,
    }
}
