//! ** TB1 de `docs/plan/PLAN_LA_TINTA.md` (10-10): TINTA, EL LIENZO MINIMO
//! (`Ultra_userspace/apps/tinta/`), ejecutado en el emulador con un
//! escritorio de mentira que le manda el raton y las teclas.
//!
//! ```text
//!    un trazo       el raton apretado de un sitio a otro: la linea sale
//!                   seguida, negra en el centro y con el borde SUAVE
//!    la goma        el boton derecho (o `g`): vuelve el papel
//!    el pincel      + y -: otro grosor
//!    el PNG         `s`: datos/tinta.png, leido aqui byte a byte -- la
//!                   firma, cada CRC-32, los bloques de zlib, su Adler-32 --
//!                   y sus pixeles, los MISMOS que la ventana
//!    la medida      lo que cuesta un trazo de lado a lado, en instrucciones
//! ```

use std::path::Path;

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_EV_RATON};
use bmo_lower::emu::{cargar_bex, run, Machine};

const PNG: &str = "datos/tinta.png";

fn tinta() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/tinta");
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn letra(c: u8) -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | c as u64
}

/// El raton en (x, y) con estos botones (1 izquierdo, 2 derecho).
fn raton(x: u64, y: u64, botones: u64) -> u64 {
    SUP_EV_RATON | 0x100 | 0x200 | botones | x << 16 | y << 32
}

/// TINTA lanzada por el escritorio, con la medida guardada (o ninguna), los
/// eventos y Esc.
fn con(medida: Option<&str>, eventos: &[u64]) -> Machine {
    let mut m = cargar_bex(&tinta()).expect("el .bex carga");
    m.padre = 7;
    if let Some(t) = medida {
        m.archivos.insert("datos/resolucion.txt".into(), t.as_bytes().to_vec());
    }
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(letra(27));
    let m = run(m, 1_500_000_000);
    assert!(m.exited, "{}", m.console);
    m
}

/// La ventana: `(ancho, alto, pixel(x, y))`, sin alfa.
fn pantalla(m: &Machine) -> (u64, u64, impl Fn(u64, u64) -> u32 + '_) {
    assert_eq!(m.ofertas.len(), 1, "UNA superficie: {:?}", m.ofertas);
    let (base, desde, _, destino) = m.ofertas[0];
    assert_eq!(destino, 7);
    let s = base + desde;
    let (w, h) = (m.read_u64(s + 4) as u32 as u64, m.read_u64(s + 8) as u32 as u64);
    (w, h, move |x: u64, y: u64| m.read_u64(s + SUP_CABECERA + 4 * (y * w + x)) as u32 & 0x00FF_FFFF)
}

const BLANCO: u32 = 0xFF_FFFF;
const NEGRO: u32 = 0;

/// El gris de un pixel de la ventana (los tres canales iguales).
fn gris(p: u32) -> u8 {
    let g = (p & 0xFF) as u8;
    assert_eq!(p, g as u32 * 0x01_0101, "un gris: {p:#08x}");
    g
}

#[test]
fn without_a_desktop_it_says_so() {
    let mut m = cargar_bex(&tinta()).unwrap();
    m.padre = 0;
    let m = run(m, 50_000_000);
    assert_eq!(m.console, "TINTA es una ventana: abrela desde el escritorio\n");
}

/// ** Un trazo de (40, 60) a (200, 60) con el pincel de 6: negro en la
/// linea, blanco lejos, y GRISES en el borde (el antialias). La medida es la
/// guardada (320 x 240: 220 de lienzo y la barra).
#[test]
fn a_stroke_is_ink_with_a_soft_edge() {
    let m = con(Some("320 240\n"), &[raton(40, 60, 1), raton(120, 60, 1), raton(200, 60, 1), raton(200, 60, 0)]);
    let (w, h, px) = pantalla(&m);
    assert_eq!((w, h), (320, 240));
    for x in [40, 80, 120, 160, 200] {
        assert_eq!(px(x, 60), NEGRO, "el centro de la linea en x = {x}");
        assert_eq!(px(x, 60 + 12), BLANCO, "lejos de la linea en x = {x}");
        assert_eq!(px(x, 60 - 12), BLANCO);
    }
    // el borde: de negro a blanco pasando por grises, y simetrico
    let columna: Vec<u8> = (50..71).map(|y| gris(px(120, y))).collect();
    assert!(columna.iter().any(|&g| g > 0 && g < 255), "hay grises en el borde: {columna:?}");
    let mut al_reves = columna.clone();
    al_reves.reverse();
    assert_eq!(columna, al_reves, "el borde es el mismo arriba y abajo");
    // fuera del trazo, papel
    assert_eq!(px(300, 200), BLANCO);
    assert_eq!(px(20, 60), BLANCO, "antes de empezar");
}

/// ** La goma (boton derecho) sobre la linea vuelve el papel; `+` engorda el
/// pincel; `g` hace del izquierdo una goma.
#[test]
fn the_eraser_and_the_brush_size() {
    let m = con(Some("320 240\n"), &[raton(40, 60, 1), raton(200, 60, 1), raton(200, 60, 0), raton(120, 40, 2), raton(120, 80, 2), raton(120, 80, 0)]);
    let (_, _, px) = pantalla(&m);
    assert_eq!(px(120, 60), BLANCO, "la goma borro el cruce");
    assert_eq!(px(60, 60), NEGRO, "y nada mas");
    // + cuatro veces: radio 10; la linea es mas gruesa
    let mut ev: Vec<u64> = (0..4).map(|_| letra(b'+')).collect();
    ev.extend([raton(40, 100, 1), raton(200, 100, 1), raton(200, 100, 0)]);
    let m = con(Some("320 240\n"), &ev);
    let (_, _, px) = pantalla(&m);
    assert_eq!(px(120, 100 + 8), NEGRO, "radio 10: a 8 del centro, tinta");
    let m = con(Some("320 240\n"), &[raton(40, 100, 1), raton(200, 100, 1), raton(200, 100, 0)]);
    let (_, _, px) = pantalla(&m);
    assert_eq!(px(120, 100 + 8), BLANCO, "radio 6: a 8 del centro, papel");
    // g: el izquierdo borra
    let m = con(Some("320 240\n"), &[raton(40, 60, 1), raton(200, 60, 1), raton(200, 60, 0), letra(b'g'), raton(120, 40, 1), raton(120, 80, 1), raton(120, 80, 0)]);
    let (_, _, px) = pantalla(&m);
    assert_eq!(px(120, 60), BLANCO);
}

// -- leer el PNG, sin bibliotecas ------------------------------------------

fn crc32(datos: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in datos {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// `(ancho, alto, grises)` de un PNG de grises sin comprimir, comprobando
/// cada CRC, cada bloque de zlib y su Adler-32.
fn lee_png(f: &[u8]) -> (u32, u32, Vec<u8>) {
    assert_eq!(&f[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "la firma");
    let mut i = 8;
    let (mut w, mut h, mut idat) = (0, 0, Vec::new());
    loop {
        let n = be32(&f[i..]) as usize;
        let tipo = &f[i + 4..i + 8];
        let datos = &f[i + 8..i + 8 + n];
        assert_eq!(be32(&f[i + 8 + n..]), crc32(&f[i + 4..i + 8 + n]), "el CRC de {}", String::from_utf8_lossy(tipo));
        match tipo {
            b"IHDR" => {
                (w, h) = (be32(datos), be32(&datos[4..]));
                assert_eq!(&datos[8..13], &[8, 0, 0, 0, 0], "8 bits, grises");
            }
            b"IDAT" => idat.extend_from_slice(datos),
            b"IEND" => break,
            otro => panic!("un trozo que no se espera: {:?}", otro),
        }
        i += 12 + n;
    }
    assert_eq!(i + 12, f.len(), "nada detras de IEND");
    assert_eq!(&idat[..2], &[0x78, 0x01], "la cabecera de zlib");
    let mut crudo = Vec::new();
    let mut k = 2;
    loop {
        let fin = idat[k] & 1 == 1;
        assert_eq!(idat[k] & 6, 0, "bloques stored");
        let len = u16::from_le_bytes([idat[k + 1], idat[k + 2]]) as usize;
        let nlen = u16::from_le_bytes([idat[k + 3], idat[k + 4]]) as usize;
        assert_eq!(len ^ 0xFFFF, nlen);
        crudo.extend_from_slice(&idat[k + 5..k + 5 + len]);
        k += 5 + len;
        if fin {
            break;
        }
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &crudo {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    assert_eq!(be32(&idat[k..]), b << 16 | a, "el Adler-32");
    assert_eq!(k + 4, idat.len());
    let fila = w as usize + 1;
    assert_eq!(crudo.len(), fila * h as usize);
    let mut px = Vec::new();
    for y in 0..h as usize {
        assert_eq!(crudo[y * fila], 0, "el filtro de la fila {y}");
        px.extend_from_slice(&crudo[y * fila + 1..(y + 1) * fila]);
    }
    (w, h, px)
}

/// ** `s` guarda datos/tinta.png: un PNG VALIDO (cada CRC, cada bloque, la
/// suma de Adler) con el lienzo entero -- y cada pixel es el de la ventana.
#[test]
fn s_saves_a_png_with_the_pixels_of_the_window() {
    let m = con(Some("160 120\n"), &[raton(20, 30, 1), raton(140, 70, 1), raton(140, 70, 0), letra(b's')]);
    let f = m.archivos.get(PNG).expect("datos/tinta.png se guardo");
    let (w, h, png) = lee_png(f);
    assert_eq!((w, h), (160, 100), "el lienzo: la ventana sin la barra");
    let (_, _, px) = pantalla(&m);
    let mut tinta = 0;
    for y in 0..h as u64 {
        for x in 0..w as u64 {
            let g = png[(y * w as u64 + x) as usize];
            assert_eq!(g, gris(px(x, y)), "el pixel ({x}, {y})");
            tinta += (g < 255) as usize;
        }
    }
    assert!(tinta > 500, "el trazo esta en el PNG: {tinta} pixeles con tinta");
}

/// ** LA MEDIDA DE TB1: un trazo de lado a lado (de x = 10 a x = 630 de un
/// lienzo de 640) con el pincel de 6, en instrucciones de E1 -- la resta de
/// TINTA con el trazo y sin el --. Su techo: si sube, TINTA se volvio mas
/// lenta. El metal lo dice en ms en la barra (`director.ms()`).
#[test]
fn what_a_stroke_across_costs() {
    let sin = con(None, &[]).pasos;
    let m = con(None, &[raton(10, 200, 1), raton(630, 200, 1), raton(630, 200, 0)]);
    let (w, _, px) = pantalla(&m);
    assert_eq!(w, 640, "sin medida guardada, 640 x 400");
    assert_eq!(px(320, 200), NEGRO);
    let trazo = m.pasos - sin;
    eprintln!("un trazo de 620 pixeles con el pincel de 6: {} instrucciones de E1 (~{} us a 3 por ns)", trazo, trazo / 3000);
    assert!(trazo < 9_000_000, "{trazo} instrucciones");
}
