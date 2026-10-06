//! **Aplanar la escena**: cada pieza a sus figuras del pintor.
//!
//! ```text
//!    el relleno   el contorno llevado al lienzo y aplanado; su regla
//!    la pluma     redonda y en un parecido (giro + escala igual): la
//!                 pluma de la casa, con su grosor escalado. Si no -- una
//!                 punta recta, una esquina en pico, o un dibujo estirado
//!                 que haria la pluma ELIPTICA --: el contorno del trazo,
//!                 calculado en las unidades de la figura y llevado al
//!                 lienzo, que se rellena `nonzero`
//!    el orden     relleno y luego pluma, como SVG (`paint-order` al reves)
//! ```

use crate::escena::{self, Ajuste, Pieza, Pintura};
use crate::geo::{self, Matriz, P};
use crate::pluma::{self, Esquina, Punta};
use crate::{degradado, Falla, Figura, Herencia, Svg, Tinta};
use bmo_maqueta_diag::Error;

/// Lo mas que se separa un tramo recto de su curva, en pixeles.
pub const TOLERANCIA: f64 = 0.15;

pub fn piezas_en(svg: &Svg, h: &Herencia, caja: (f64, f64, f64, f64), ajuste: &dyn Fn(&crate::Elemento) -> Ajuste) -> (Vec<Pieza>, Vec<(usize, String)>) {
    escena::piezas(svg, &h.estado(), &svg.encaje(caja), ajuste)
}

/// Los tramos de cada curva de cada pieza, en el lienzo.
pub fn conteos(piezas: &[Pieza]) -> Vec<Vec<Vec<usize>>> {
    piezas.iter().map(|p| geo::conteos(&p.contorno.transformado(&p.m), TOLERANCIA)).collect()
}

fn tinta(svg: &Svg, ids: &std::collections::HashMap<String, &crate::Elemento>, p: &Pieza, pintura: &Pintura) -> Result<Option<Tinta>, String> {
    match pintura {
        Pintura::Color(c) => Ok(Some(Tinta::Liso(*c))),
        Pintura::Degradado(id) => {
            let (vw, vh) = svg.vista_o_medida().map_or((100.0, 100.0), |v| (v[2], v[3]));
            degradado::tinta(ids, &svg.hoja, id, p.contorno.caja_exacta(), &p.m, vw, vh, p.color)
        }
    }
}

fn llevar(m: &Matriz, c: &[Vec<P>]) -> Vec<Vec<P>> {
    c.iter().map(|s| s.iter().map(|&q| m.punto(q)).collect()).collect()
}

/// Si una pieza se sale de la caja `(x, y, w, h)` del dibujo (sus puntos de
/// control, con media pluma): entonces se recorta por ella.
fn se_sale(p: &Pieza, (x, y, w, h): (f64, f64, f64, f64)) -> bool {
    let media = p.linea.as_ref().map_or(0.0, |l| l.ancho / 2.0 * p.m.estira());
    let c = p.contorno.transformado(&p.m);
    c.subs.iter().any(|s| {
        std::iter::once(s.inicio)
            .chain(s.segs.iter().flat_map(|g| match *g {
                geo::Seg::Linea(a) => vec![a],
                geo::Seg::Cuadratica(a, b) => vec![a, b],
                geo::Seg::Cubica(a, b, c) => vec![a, b, c],
            }))
            .any(|(px, py)| px - media < x || py - media < y || px + media > x + w || py + media > y + h)
    })
}

/// **Las figuras de unas piezas** con unos tramos dados. Con `podar`, lo
/// que no se ve (opacidad 0) no sale; para animar sale todo, para que los
/// pasos tengan la misma forma.
///
/// ** `caja` es la del `<svg>` en el lienzo: lo que se sale de ella NO se ve,
/// como en el navegador (`overflow: hidden` de un `<svg>`). Solo se recorta
/// la pieza que de verdad se sale; las demas salen como siempre.
pub fn aplanar(svg: &Svg, piezas: &[Pieza], n: &[Vec<Vec<usize>>], podar: bool, caja: (f64, f64, f64, f64)) -> (Vec<Figura>, Vec<(usize, String)>) {
    let ids = svg.ids();
    let mut out = Vec::new();
    let mut fallas = Vec::new();
    let marco = [(caja.0, caja.1), (caja.0 + caja.2, caja.1), (caja.0 + caja.2, caja.1 + caja.3), (caja.0, caja.1 + caja.3)];
    for (k, p) in piezas.iter().enumerate() {
        let tramos = &n[k];
        let mut con_marco;
        let p = if se_sale(p, caja) {
            con_marco = p.clone();
            con_marco.recorte = Some(match &p.recorte {
                Some(r) => geo::convexo(&geo::recortar(&marco, r)).unwrap_or_default(),
                None => marco.to_vec(),
            });
            &con_marco
        } else {
            p
        };
        // Con recorte, todo sale como contorno y se recorta: una pluma
        // redonda tambien (su contorno redondo).
        let recorta = |caminos: Vec<Vec<P>>| -> Vec<Vec<P>> {
            match &p.recorte {
                None => caminos,
                Some(c) if c.is_empty() => Vec::new(),
                Some(c) => caminos.iter().map(|s| geo::recortar(s, c)).filter(|s| s.len() > 2).collect(),
            }
        };
        let mut relleno = None;
        let mut linea = Vec::new();
        if let Some(r) = &p.relleno {
            match tinta(svg, &ids, p, &r.pintura) {
                Ok(Some(t)) => {
                    let (caminos, _) = geo::aplanar_con(&p.contorno.transformado(&p.m), tramos);
                    let caminos = recorta(caminos);
                    let cerrados = vec![true; caminos.len()];
                    relleno = Some(Figura { caminos, cerrados, pluma: 0.0, tinta: t, alfa: r.alfa, par_impar: r.par_impar, de: p.de.clone() });
                }
                Ok(None) => {}
                Err(m) => fallas.push((p.pos, m)),
            }
        }
        if let Some(l) = &p.linea {
            match tinta(svg, &ids, p, &l.pintura) {
                Ok(Some(t)) => {
                    let (mut cs, mut ks) = geo::aplanar_con(&p.contorno, tramos);
                    if let Some((patron, desfase)) = &l.discontinuo {
                        cs = pluma::discontinuo(&cs, &ks, patron, *desfase);
                        ks = vec![false; cs.len()];
                    }
                    if l.punta == Punta::Redonda && l.esquina == Esquina::Redonda && p.m.parecido() && p.recorte.is_none() {
                        let escala = p.m.det().abs().sqrt();
                        linea.push(Figura { caminos: llevar(&p.m, &cs), cerrados: ks, pluma: l.ancho * escala, tinta: t, alfa: l.alfa, par_impar: false, de: p.de.clone() });
                    } else {
                        let mut polis = Vec::new();
                        for (c, cerrado) in cs.iter().zip(&ks) {
                            polis.extend(pluma::contorno(c, *cerrado, l.ancho, l.punta, l.esquina, l.limite));
                        }
                        let caminos = recorta(llevar(&p.m, &polis));
                        let cerrados = vec![true; caminos.len()];
                        linea.push(Figura { caminos, cerrados, pluma: 0.0, tinta: t, alfa: l.alfa, par_impar: false, de: p.de.clone() });
                    }
                }
                Ok(None) => {}
                Err(m) => fallas.push((p.pos, m)),
            }
        }
        let mut dos: Vec<Figura> = Vec::new();
        if p.linea_primero {
            dos.extend(linea);
            dos.extend(relleno);
        } else {
            dos.extend(relleno);
            dos.extend(linea);
        }
        out.extend(dos.into_iter().filter(|f| !podar || (f.alfa > 0.0 && f.caminos.iter().any(|c| !c.is_empty()))));
    }
    (out, fallas)
}

/// Un rechazo de la escena, en la forma de MAQUETA.
pub fn errores(svg: &Svg, fallas: &[(usize, String)]) -> Vec<Error> {
    let mut vistas = std::collections::HashSet::new();
    fallas
        .iter()
        .filter(|f| vistas.insert((*f).clone()))
        .map(|(pos, m)| svg.error(&Falla::en(*pos, 1, m, "lo que el pintor no hace igual que el navegador, no se pinta a medias.", "ver LA_MAQUETA_EXIGE.md, 2e.")))
        .collect()
}

pub fn en(svg: &Svg, h: &Herencia, caja: (f64, f64, f64, f64), ajuste: &dyn Fn(&crate::Elemento) -> Ajuste, n: Option<&[Vec<Vec<usize>>]>, podar: bool) -> (Vec<Figura>, Vec<Error>) {
    let (piezas, mut fallas) = piezas_en(svg, h, caja, ajuste);
    let propios;
    let n = match n {
        Some(n) => n,
        None => {
            propios = conteos(&piezas);
            &propios
        }
    };
    let (figs, mut f2) = aplanar(svg, &piezas, n, podar, caja);
    fallas.append(&mut f2);
    (figs, errores(svg, &fallas))
}
