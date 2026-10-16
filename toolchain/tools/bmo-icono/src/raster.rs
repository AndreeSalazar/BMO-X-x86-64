//! **DE FORMAS A PIXELES**, suavizado: cada pixel se mira en una rejilla de
//! `MUESTRAS x MUESTRAS` puntos y se queda con la parte que cubre la forma.
//! Los colores van PREMULTIPLICADOS mientras se mezcla (el borde de una forma
//! clara sobre nada no se oscurece) y se sueltan al final, en el BICO.

use crate::ruta::{Matriz, P};
use crate::svg::{Degradado, Dibujo, Forma, Pintura, Rgba};

/// Puntos por lado y pixel: 8 x 8 = 64 tonos de borde, mas de lo que un ojo
/// distingue a 32 pixeles.
const MUESTRAS: usize = 8;

/// Un lienzo de `lado x lado` en RGBA premultiplicado de 0 a 1.
pub struct Lienzo {
    pub lado: u32,
    pub px: Vec<Rgba>,
}

impl Lienzo {
    pub fn nuevo(lado: u32) -> Self {
        Lienzo { lado, px: vec![[0.0; 4]; (lado * lado) as usize] }
    }

    /// `src` (premultiplicado) ENCIMA de lo que hay.
    fn encima(&mut self, i: usize, src: Rgba) {
        let d = &mut self.px[i];
        let k = 1.0 - src[3];
        for c in 0..4 {
            d[c] = src[c] + d[c] * k;
        }
    }

    /// **El BICO**: `"BICO"`, ancho y alto (u16) y los pixeles `0xAARRGGBB`
    /// en little-endian (B, G, R, A), SIN premultiplicar: el escritorio mezcla.
    pub fn bico(&self) -> Vec<u8> {
        let mut v = b"BICO".to_vec();
        v.extend_from_slice(&(self.lado as u16).to_le_bytes());
        v.extend_from_slice(&(self.lado as u16).to_le_bytes());
        for p in &self.px {
            let a = p[3].clamp(0.0, 1.0);
            let canal = |c: f64| if a > 0.0 { ((c / a).clamp(0.0, 1.0) * 255.0).round() as u32 } else { 0 };
            let a8 = (a * 255.0).round() as u32;
            let px = if a8 == 0 { 0 } else { (a8 << 24) | (canal(p[0]) << 16) | (canal(p[1]) << 8) | canal(p[2]) };
            v.extend_from_slice(&px.to_le_bytes());
        }
        v
    }
}

/// **Un dibujo SVG a `lado` pixeles**, centrado y sin deformar (lo que SVG
/// llama `xMidYMid meet`).
pub fn pintar(d: &Dibujo, lado: u32) -> Lienzo {
    let mut l = Lienzo::nuevo(lado);
    let [vx, vy, vw, vh] = d.vista;
    let s = (lado as f64 / vw).min(lado as f64 / vh);
    let (ox, oy) = ((lado as f64 - vw * s) / 2.0, (lado as f64 - vh * s) / 2.0);
    let vista = Matriz([s, 0.0, 0.0, s, ox - vx * s, oy - vy * s]);
    for f in &d.formas {
        let total = vista.por(&f.matriz);
        let Some(inversa) = total.inversa() else { continue };
        let caja = caja_local(f);
        let pinta = |p: &Pintura, opacidad: f64, en: P| -> Option<Rgba> {
            let c = match p {
                Pintura::Nada | Pintura::Ref(_) => return None,
                Pintura::Color(c) => *c,
                Pintura::Degradado(i) => color_de_degradado(&d.degradados[*i], inversa.aplica(en), caja),
            };
            let a = c[3] * opacidad;
            Some([c[0] * a, c[1] * a, c[2] * a, a])
        };
        let contornos: Vec<(Vec<P>, bool)> = f.contornos.iter().map(|(c, z)| (c.iter().map(|p| total.aplica(*p)).collect(), *z)).collect();
        if !matches!(f.relleno, Pintura::Nada) {
            let lados = aristas(&contornos, true);
            capa(&mut l, &lados, 0.0, |x, y| cubre_relleno(&lados, x, y, f.par_impar), |en| pinta(&f.relleno, f.opacidad_relleno, en));
        }
        if !matches!(f.trazo, Pintura::Nada) && f.ancho > 0.0 {
            let lados = aristas(&contornos, false);
            let medio = f.ancho * total.escala() / 2.0;
            capa(&mut l, &lados, medio, |x, y| cubre_trazo(&lados, x, y, medio), |en| pinta(&f.trazo, f.opacidad_trazo, en));
        }
    }
    l
}

/// Los tramos de los contornos; con `cerrar`, cada uno vuelve a su inicio
/// (un relleno siempre se cierra; un trazo, solo si dijo `Z`).
fn aristas(contornos: &[(Vec<P>, bool)], cerrar: bool) -> Vec<(P, P)> {
    let mut v = Vec::new();
    for (c, z) in contornos {
        for w in c.windows(2) {
            v.push((w[0], w[1]));
        }
        if (cerrar || *z) && c.len() > 2 {
            v.push((c[c.len() - 1], c[0]));
        }
    }
    v
}

/// Una forma (relleno o trazo) sobre el lienzo: muestra a muestra.
fn capa(l: &mut Lienzo, lados: &[(P, P)], margen: f64, cubre: impl Fn(f64, f64) -> bool, color: impl Fn(P) -> Option<Rgba>) {
    if lados.is_empty() {
        return;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (a, b) in lados {
        for p in [a, b] {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    let lado = l.lado as i64;
    let rango = |a: f64, b: f64| ((a - margen).floor().max(0.0) as i64, ((b + margen).ceil() as i64).min(lado));
    let (px0, px1) = rango(x0, x1);
    let (py0, py1) = rango(y0, y1);
    let n = (MUESTRAS * MUESTRAS) as f64;
    for py in py0..py1 {
        for px in px0..px1 {
            let mut suma = [0.0; 4];
            for j in 0..MUESTRAS {
                for i in 0..MUESTRAS {
                    let x = px as f64 + (i as f64 + 0.5) / MUESTRAS as f64;
                    let y = py as f64 + (j as f64 + 0.5) / MUESTRAS as f64;
                    if cubre(x, y) {
                        if let Some(c) = color((x, y)) {
                            for k in 0..4 {
                                suma[k] += c[k];
                            }
                        }
                    }
                }
            }
            if suma[3] > 0.0 {
                l.encima((py * lado + px) as usize, suma.map(|v| v / n));
            }
        }
    }
}

/// Dentro de un relleno: la regla de la vuelta (`nonzero`) o par-impar.
fn cubre_relleno(lados: &[(P, P)], x: f64, y: f64, par_impar: bool) -> bool {
    let mut vueltas = 0i32;
    for &(a, b) in lados {
        let cruz = (b.0 - a.0) * (y - a.1) - (x - a.0) * (b.1 - a.1);
        if a.1 <= y && b.1 > y && cruz > 0.0 {
            vueltas += 1;
        } else if b.1 <= y && a.1 > y && cruz < 0.0 {
            vueltas -= 1;
        }
    }
    if par_impar { vueltas % 2 != 0 } else { vueltas != 0 }
}

/// Dentro de un trazo: a menos de medio ancho de algun tramo (puntas y codos
/// redondos, como los de Lucide).
fn cubre_trazo(lados: &[(P, P)], x: f64, y: f64, medio: f64) -> bool {
    let m2 = medio * medio;
    lados.iter().any(|&(a, b)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (ex, ey) = (a.0 + t * dx - x, a.1 + t * dy - y);
        ex * ex + ey * ey <= m2
    })
}

/// La caja de la forma en SUS coordenadas (para `objectBoundingBox`).
fn caja_local(f: &Forma) -> [f64; 4] {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (c, _) in &f.contornos {
        for p in c {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    [x0, y0, (x1 - x0).max(1e-9), (y1 - y0).max(1e-9)]
}

/// El color de un degradado en un punto de la forma (sus coordenadas).
fn color_de_degradado(g: &Degradado, local: P, caja: [f64; 4]) -> Rgba {
    let p = if g.caja { ((local.0 - caja[0]) / caja[2], (local.1 - caja[1]) / caja[3]) } else { local };
    let p = g.matriz.inversa().map(|m| m.aplica(p)).unwrap_or(p);
    let t = if g.radial {
        let r = g.p[2].max(1e-9);
        let (dx, dy) = (p.0 - g.p[0], p.1 - g.p[1]);
        (dx * dx + dy * dy).sqrt() / r
    } else {
        let (dx, dy) = (g.p[2] - g.p[0], g.p[3] - g.p[1]);
        let l2 = dx * dx + dy * dy;
        if l2 > 0.0 { ((p.0 - g.p[0]) * dx + (p.1 - g.p[1]) * dy) / l2 } else { 0.0 }
    };
    let t = t.clamp(0.0, 1.0);
    let s = &g.paradas;
    if t <= s[0].0 {
        return s[0].1;
    }
    for w in s.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let k = if b > a { (t - a) / (b - a) } else { 1.0 };
            return [0, 1, 2, 3].map(|i| ca[i] + (cb[i] - ca[i]) * k);
        }
    }
    s[s.len() - 1].1
}

/// **Una foto (PNG o JPG ya decodificado) a `lado` pixeles**, centrada y sin
/// deformar, por PROMEDIO DE AREA: cada pixel de salida es la media de los
/// de entrada que le caen debajo (con su parte de los del borde), con el alfa
/// premultiplicado para que lo transparente no tina el borde.
pub fn encajar(px: &[u32], ancho: u32, alto: u32, lado: u32) -> Lienzo {
    let mut l = Lienzo::nuevo(lado);
    let s = (lado as f64 / ancho as f64).min(lado as f64 / alto as f64);
    let (w, h) = (ancho as f64 * s, alto as f64 * s);
    let (ox, oy) = ((lado as f64 - w) / 2.0, (lado as f64 - h) / 2.0);
    let fuente = |x: u32, y: u32| -> Rgba {
        let c = px[(y * ancho + x) as usize];
        let a = (c >> 24) as f64 / 255.0;
        [((c >> 16) & 0xFF) as f64 / 255.0 * a, ((c >> 8) & 0xFF) as f64 / 255.0 * a, (c & 0xFF) as f64 / 255.0 * a, a]
    };
    for py in 0..lado {
        for qx in 0..lado {
            // El trozo de la foto que cae bajo este pixel, en pixeles de la foto.
            let (fx0, fx1) = (((qx as f64 - ox) / s).max(0.0), ((qx as f64 + 1.0 - ox) / s).min(ancho as f64));
            let (fy0, fy1) = (((py as f64 - oy) / s).max(0.0), ((py as f64 + 1.0 - oy) / s).min(alto as f64));
            if fx1 <= fx0 || fy1 <= fy0 {
                continue;
            }
            let mut suma = [0.0; 4];
            let mut y = fy0.floor();
            while y < fy1 {
                let ky = (y + 1.0).min(fy1) - y.max(fy0);
                let mut x = fx0.floor();
                while x < fx1 {
                    let kx = (x + 1.0).min(fx1) - x.max(fx0);
                    let c = fuente(x as u32, y as u32);
                    for k in 0..4 {
                        suma[k] += c[k] * kx * ky;
                    }
                    x += 1.0;
                }
                y += 1.0;
            }
            // Dividido por el area del PIXEL (no del trozo): en el borde de una
            // foto que no llena el cuadro, lo que falta es transparente.
            let area = s * s;
            l.px[(py * lado + qx) as usize] = suma.map(|v| (v * area).min(1.0));
        }
    }
    l
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_circulo_tiene_borde_suave_y_centro_lleno() {
        let d = crate::svg::leer(r##"<svg viewBox="0 0 32 32"><circle cx="16" cy="16" r="10" fill="#fff"/></svg>"##).unwrap();
        let l = pintar(&d, 32);
        let a = |x: u32, y: u32| l.px[(y * 32 + x) as usize][3];
        assert!(a(16, 16) > 0.999);
        assert_eq!(a(1, 1), 0.0);
        // En el borde hay medios tonos: en la fila 13 el circulo acaba en x = 25,7.
        let borde = a(25, 13);
        assert!(borde > 0.1 && borde < 0.9, "borde = {borde}");
    }

    #[test]
    fn el_trazo_pinta_solo_la_linea() {
        let d = crate::svg::leer(r##"<svg viewBox="0 0 32 32"><line x1="4" y1="16" x2="28" y2="16" stroke="red" stroke-width="2"/></svg>"##).unwrap();
        let l = pintar(&d, 32);
        assert!(l.px[(15 * 32 + 16) as usize][3] > 0.99 && l.px[(16 * 32 + 16) as usize][3] > 0.99);
        assert_eq!(l.px[(10 * 32 + 16) as usize][3], 0.0);
    }

    #[test]
    fn par_impar_deja_el_agujero() {
        let d = crate::svg::leer(r##"<svg viewBox="0 0 32 32"><path fill-rule="evenodd" d="M2 2h28v28h-28zM10 10h12v12h-12z"/></svg>"##).unwrap();
        let l = pintar(&d, 32);
        assert_eq!(l.px[(16 * 32 + 16) as usize][3], 0.0);
        assert!(l.px[(4 * 32 + 4) as usize][3] > 0.999);
    }

    #[test]
    fn la_foto_de_cuatro_a_dos_promedia() {
        // 4x4: mitad izquierda blanca opaca, derecha transparente.
        let px: Vec<u32> = (0..16).map(|i| if i % 4 < 2 { 0xFFFF_FFFF } else { 0 }).collect();
        let l = encajar(&px, 4, 4, 2);
        assert!((l.px[0][3] - 1.0).abs() < 1e-9 && l.px[1][3] == 0.0);
        let b = l.bico();
        assert_eq!(&b[..8], b"BICO\x02\x00\x02\x00");
        assert_eq!(u32::from_le_bytes([b[8], b[9], b[10], b[11]]), 0xFFFF_FFFF);
    }
}
