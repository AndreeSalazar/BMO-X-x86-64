//! `compara lista.txt fotos/`: cada SVG de la lista pintado por el pintor de
//! la casa contra la foto del NAVEGADOR (`fotos/<k>.png`, 96 x 96 sobre
//! blanco). Dice cuanto se parecen: es el oraculo del lector.
use bmo_pinta::{Lienzo, Parada, Pieza, Tinta};

struct Im {
    px: Vec<u32>,
}
const L: i32 = 96;
impl Lienzo for Im {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        for j in y.max(0)..(y + h).min(L) {
            for i in x.max(0)..(x + w).min(L) {
                self.px[(j * L + i) as usize] = c;
            }
        }
    }
    fn mezclar(&mut self, x: i32, y: i32, c: u32, a: u8) {
        if (0..L).contains(&x) && (0..L).contains(&y) {
            let k = (y * L + x) as usize;
            self.px[k] = bmo_pinta::sobre(self.px[k], c, a);
        }
    }
}

fn a64(v: f64) -> i32 {
    ((v * 16.0).round() as i32) * 4
}

pub fn pintar(f: &[bmo_maqueta_dibujo::Figura]) -> Vec<u32> {
    let mut im = Im { px: vec![0xFFFFFF; (L * L) as usize] };
    let mut letra: Box<bmo_letra::LetraFija<65536, 1024>> = Box::new(bmo_letra::LetraFija::nueva());
    for fig in f {
        let cs: Vec<Vec<(i32, i32)>> = fig.caminos.iter().map(|c| c.iter().map(|p| (a64(p.0), a64(p.1))).collect()).collect();
        let refs: Vec<&[(i32, i32)]> = cs.iter().map(|c| c.as_slice()).collect();
        let paradas: Vec<Parada> = match &fig.tinta {
            bmo_maqueta_dibujo::Tinta::Liso(_) => Vec::new(),
            bmo_maqueta_dibujo::Tinta::Lineal { paradas, .. } | bmo_maqueta_dibujo::Tinta::Radial { paradas, .. } => {
                paradas.iter().map(|p| Parada { en: (p.en * 1000.0).round() as u16, c: p.c, alfa: (p.alfa * 255.0).round() as u8 }).collect()
            }
        };
        let pt = |p: (f64, f64)| (a64(p.0), a64(p.1));
        let tinta = match &fig.tinta {
            bmo_maqueta_dibujo::Tinta::Liso(c) => Tinta::Liso(*c),
            bmo_maqueta_dibujo::Tinta::Lineal { de, a, .. } => Tinta::Lineal { de: pt(*de), a: pt(*a), paradas: &paradas },
            bmo_maqueta_dibujo::Tinta::Radial { centro, eje_x, eje_y, .. } => Tinta::Radial { centro: pt(*centro), eje_x: pt(*eje_x), eje_y: pt(*eje_y), paradas: &paradas },
        };
        let pz = Pieza::Figura { caminos: &refs, cerrados: &fig.cerrados, pluma: a64(fig.pluma), tinta, alfa: (fig.alfa * 255.0).round() as u8, par_impar: fig.par_impar };
        bmo_pinta::pieza(&mut im, &mut *letra, &pz, 0, 0);
    }
    im.px
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let lista = std::fs::read_to_string(&a[1]).unwrap();
    let mut filas = Vec::new();
    for (k, ruta) in lista.lines().enumerate() {
        let Ok(png) = std::fs::read(format!("{}/{k}.png", a[2])) else { continue };
        let m = bmo_imagen::medir(&png).unwrap();
        let mut ref_ = vec![0u32; (m.ancho * m.alto) as usize];
        let mut taller = vec![0u8; bmo_imagen::TALLER];
        bmo_imagen::decodificar_con(&png, &mut ref_, &mut taller).unwrap();
        if m.ancho != 96 || m.alto != 96 {
            continue;
        }
        let texto = std::fs::read(ruta).unwrap();
        let Ok(s) = bmo_maqueta_dibujo::leer_fichero(ruta, &texto, bmo_maqueta_diag::Span::new(0, 1, 1, 1)) else { continue };
        let (f, e) = bmo_maqueta_dibujo::figuras(&s, &Default::default(), (0.0, 0.0, 96.0, 96.0));
        if !e.is_empty() {
            continue;
        }
        let nuestro = pintar(&f);
        if let Ok(dir) = std::env::var("VOLCAR") {
            let mut v = Vec::new();
            for (p, q) in nuestro.iter().zip(&ref_) {
                v.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8, (q >> 16) as u8, (q >> 8) as u8, *q as u8]);
            }
            std::fs::write(format!("{dir}/{k}.rgb"), v).unwrap();
        }
        let (mut igual, mut parecido, mut tinta) = (0, 0, 0);
        for (p, q) in nuestro.iter().zip(&ref_) {
            let d = [16, 8, 0].iter().map(|s| ((p >> s & 255) as i32 - (q >> s & 255) as i32).abs()).max().unwrap();
            let dibujado = (q & 0xFFFFFF) != 0xFFFFFF || (p & 0xFFFFFF) != 0xFFFFFF;
            if dibujado {
                tinta += 1;
                igual += (d <= 8) as i32;
                parecido += (d <= 40) as i32;
            }
        }
        let t = tinta.max(1) as f64;
        filas.push((igual as f64 * 100.0 / t, parecido as f64 * 100.0 / t, ruta.to_string()));
    }
    let n = filas.len() as f64;
    println!("{} dibujos: igual {:.2} %, parecido {:.2} % (de los pixeles con tinta)", filas.len(), filas.iter().map(|f| f.0).sum::<f64>() / n, filas.iter().map(|f| f.1).sum::<f64>() / n);
    filas.sort_by(|a, b| a.1.total_cmp(&b.1));
    for f in filas.iter().take(12) {
        println!("  {:6.2} {:6.2}  {}", f.0, f.1, f.2);
    }
}
