//! Las pruebas del pintor, sobre una imagen en memoria.

extern crate std;
use super::*;
use std::vec;
use std::vec::Vec;

struct Imagen {
    w: i32,
    h: i32,
    px: Vec<Color>,
}

impl Imagen {
    fn nueva(w: i32, h: i32) -> Imagen {
        Imagen { w, h, px: vec![0; (w * h) as usize] }
    }
    fn en(&self, x: i32, y: i32) -> Color {
        self.px[(y * self.w + x) as usize]
    }
    /// La "tinta" total del canal verde, en 1/255 de pixel.
    fn tinta(&self) -> u64 {
        self.px.iter().map(|&c| (c >> 8 & 255) as u64).sum()
    }
}

impl Lienzo for Imagen {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for j in y.max(0)..(y + h).min(self.h) {
            for i in x.max(0)..(x + w).min(self.w) {
                self.px[(j * self.w + i) as usize] = c;
            }
        }
    }
    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let k = (y * self.w + x) as usize;
            self.px[k] = entre(self.px[k], c, alfa as u32 * 256 / 255);
        }
    }
}

const VERDE: Color = 0x0000_FF00;

#[test]
fn la_caja_redonda_es_simetrica_y_su_area_es_la_de_la_cuenta() {
    let mut im = Imagen::nueva(60, 40);
    caja(&mut im, 5, 5, 50, 30, 10, VERDE);
    // Simetrica en los dos ejes.
    for y in 0..40 {
        for x in 0..60 {
            assert_eq!(im.en(x, y), im.en(59 - x, y), "({x},{y})");
            assert_eq!(im.en(x, y), im.en(x, 39 - y), "({x},{y})");
        }
    }
    // Area = 50*30 - (4 - pi) * 10^2 = 1414,16 pixeles.
    let area = im.tinta() as f64 / 255.0;
    assert!((area - 1414.16).abs() < 3.0, "{area}");
    // El centro, macizo; la esquina de verdad, vacia.
    assert_eq!(im.en(30, 20), VERDE);
    assert_eq!(im.en(5, 5), 0);
}

#[test]
fn el_borde_no_toca_lo_de_dentro() {
    let mut im = Imagen::nueva(60, 40);
    borde(&mut im, 5, 5, 50, 30, 10, 1, VERDE);
    assert_eq!(im.en(30, 20), 0, "lo de dentro, intacto");
    assert_eq!(im.en(30, 5), VERDE, "el borde de arriba, entero");
    // El perimetro de la caja redonda: 2*(50+30) - (8 - 2*pi) * 10 = 142,8.
    let largo = im.tinta() as f64 / 255.0;
    assert!((largo - 142.8).abs() < 6.0, "{largo}");
}

#[test]
fn el_resplandor_cae_y_no_entra() {
    let mut im = Imagen::nueva(80, 80);
    resplandor(&mut im, 30, 30, 20, 20, 4, 14, 0xFF00_FF00);
    assert_eq!(im.en(40, 40), 0, "dentro no se pinta");
    let cerca = im.en(29, 40) >> 8 & 255;
    let lejos = im.en(20, 40) >> 8 & 255;
    assert!(cerca > lejos && lejos > 0, "{cerca} {lejos}");
    assert_eq!(im.en(10, 40), 0, "mas alla del alcance, nada");
}

#[test]
fn el_degradado_va_de_un_color_al_otro() {
    let mut im = Imagen::nueva(101, 4);
    degradado(&mut im, 0, 0, 101, 4, 0, 0x0000_0000, 0x0000_FF00, false);
    assert_eq!(im.en(0, 1) >> 8 & 255, 0);
    assert_eq!(im.en(100, 1) >> 8 & 255, 255);
    let medio = im.en(50, 1) >> 8 & 255;
    assert!((126..=129).contains(&medio), "{medio}");
}

#[test]
fn el_trazo_suaviza_y_no_pinta_dos_veces_las_juntas() {
    // Una V: la junta del vertice no puede salir mas oscura que el trazo.
    let mut im = Imagen::nueva(40, 40);
    let p = [(5 * 64, 5 * 64), (20 * 64, 30 * 64), (35 * 64, 5 * 64)];
    trazo(&mut im, &p, false, 128, VERDE);
    let max = im.px.iter().map(|&c| c >> 8 & 255).max().unwrap();
    assert_eq!(max, 255);
    let medios = im.px.iter().filter(|&&c| (1..255).contains(&(c >> 8 & 255))).count();
    assert!(medios > 30, "una diagonal suave tiene pixeles a medias: {medios}");
}

#[test]
fn el_relleno_de_un_cuadrado_da_su_area_y_medio_pixel_es_medio() {
    let mut im = Imagen::nueva(20, 20);
    let cuadrado = [(2 * 64, 2 * 64), (12 * 64, 2 * 64), (12 * 64, 12 * 64), (2 * 64, 12 * 64)];
    relleno(&mut im, &[&cuadrado], VERDE);
    assert_eq!(im.tinta() / 255, 100);
    let mut im = Imagen::nueva(4, 2);
    relleno(&mut im, &[&[(0, 0), (96, 0), (96, 64), (0, 64)]], VERDE);
    assert_eq!(im.en(0, 0), VERDE);
    assert!((126..=128).contains(&(im.en(1, 0) >> 8 & 255)), "medio pixel, media tinta");
}

#[test]
fn la_letra_cae_en_su_caja_como_en_el_navegador() {
    let mut im = Imagen::nueva(80, 30);
    let mut f: std::boxed::Box<bmo_letra::LetraFija<65536, 1024>> = std::boxed::Box::new(bmo_letra::LetraFija::nueva());
    let w = letra(&mut im, &mut *f, 2, 2, 20, b"Hola", VERDE, Estilo::normal(14));
    assert!(w > 20 && w < 40, "{w}");
    // La base de una caja de 20 con letra de 14 cae en la fila 2 + 15.
    let filas: Vec<i32> = (0..30).filter(|&y| (0..80).any(|x| im.en(x, y) != 0)).collect();
    assert_eq!(*filas.last().unwrap(), 2 + 15 - 1, "la ultima fila con tinta es la de encima de la base");
}

/// La raiz rapida da EXACTAMENTE el suelo de la raiz: cada numero hasta dos
/// millones, los cuadrados y sus vecinos hasta 2^40, y los bordes de u64.
#[test]
fn la_raiz_rapida_es_la_raiz_exacta() {
    let bien = |n: u64, r: u64| r * r <= n && (r + 1).checked_mul(r + 1).map_or(true, |q| q > n);
    for n in 0..2_000_000u64 {
        assert!(bien(n, super::raiz(n)), "{n}");
    }
    for k in 1..(1u64 << 20) {
        for n in [k * k - 1, k * k, k * k + 1] {
            assert!(bien(n, super::raiz(n)), "{n}");
        }
    }
    for n in [u64::MAX, u64::MAX - 1, 1 << 63, (1 << 62) + 7] {
        assert!(bien(n, super::raiz(n)), "{n}");
    }
}
