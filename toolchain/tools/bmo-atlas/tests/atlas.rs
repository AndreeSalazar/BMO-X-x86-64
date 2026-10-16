//! ** LE1 de PL18 (`docs/plan/PLAN_ECCE.md`, 16-10): EL ATLAS DE UNA LETRA.
//!
//! ```text
//!    la casa       224 letras de Latin-1, con tinta las que la tienen; el
//!                  BLET se lee de vuelta y cuadra byte a byte
//!    un TrueType   uno HECHO AQUI, a mano (ninguna letra de terceros entra
//!                  al arbol): un cuadrado, un glifo COMPUESTO de el movido,
//!                  y un circulo de curvas cuadraticas
//!    el juez       la tinta que pinta es el AREA del contorno: lo que cubre
//!                  cada pixel se mide, no se adivina
//! ```

use bmo_atlas::{atlas, atlas_de_casa, pinta, ttf, Atlas, CUANTOS, PRIMERO};

/// Lee un BLET de vuelta: `(medida, glifos (avance, x, y, ancho, alto, alfa))`.
fn leer(b: &[u8]) -> (u8, Vec<(u16, i16, i16, u16, u16, Vec<u8>)>) {
    assert_eq!(&b[..4], b"BLET");
    assert_eq!(b[4], 1);
    let u16_en = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    assert_eq!(u16_en(14), PRIMERO);
    assert_eq!(u16_en(16), CUANTOS);
    let tabla = 18 + 16 * CUANTOS as usize;
    let mut v = Vec::new();
    let mut fin = tabla;
    for i in 0..CUANTOS as usize {
        let o = 18 + 16 * i;
        let (ancho, alto) = (u16_en(o + 6), u16_en(o + 8));
        let donde = u32::from_le_bytes([b[o + 12], b[o + 13], b[o + 14], b[o + 15]]) as usize;
        let n = ancho as usize * alto as usize;
        let alfa = b[tabla + donde..tabla + donde + n].to_vec();
        fin = fin.max(tabla + donde + n);
        v.push((u16_en(o), u16_en(o + 2) as i16, u16_en(o + 4) as i16, ancho, alto, alfa));
    }
    assert_eq!(fin, b.len(), "el alfa acaba donde acaba el fichero");
    (b[5], v)
}

fn tinta(a: &[u8]) -> u64 {
    a.iter().map(|&x| x as u64).sum()
}

/// ** La letra de la casa: cada letra de Latin-1, con tinta (la "a", la "n"
/// con tilde, la "A" con acento), el espacio sin tinta pero con avance, y
/// las de 127 a 159 (no son letras) vacias. Y el BLET cuadra.
#[test]
fn the_house_letter_makes_a_whole_latin1_atlas() {
    let a = atlas_de_casa(16, bmo_letra::Peso::Normal).unwrap();
    let (medida, g) = leer(&a.bytes());
    assert_eq!(medida, 16);
    assert_eq!(g.len(), 224);
    let de = |c: u8| &g[(c - 32) as usize];
    for c in [b'a', b'H', 0xF1, 0xC1, b'?', 0xBF] {
        assert!(tinta(&de(c).5) > 0, "la {c} no tiene tinta");
        assert!(de(c).0 > 0, "la {c} no avanza");
    }
    assert_eq!(tinta(&de(b' ').5), 0);
    assert!(de(b' ').0 > 0, "el espacio avanza");
    assert_eq!(tinta(&de(130).5), 0, "el 130 no es una letra");
    // La H esta ENCIMA de la base, y la "g" baja de ella.
    assert!(de(b'H').2 > 0 && de(b'H').2 as i32 - de(b'H').4 as i32 >= -1);
    assert!(de(b'g').2 as i32 - (de(b'g').4 as i32) < -1, "la g baja de la base");
    // En negrita, mas tinta.
    let n = atlas_de_casa(16, bmo_letra::Peso::Negrita).unwrap();
    assert!(tinta(&n.glifos[(b'H' - 32) as usize].alfa) > tinta(&de(b'H').5));
}

// -- Un TrueType hecho a mano --

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// Un glifo simple: sus contornos, cada punto `(x, y, sobre_la_curva)`.
fn simple(contornos: &[&[(i16, i16, bool)]]) -> Vec<u8> {
    let mut v = Vec::new();
    be16(&mut v, contornos.len() as u16);
    v.extend_from_slice(&[0; 8]);
    let mut n = 0u16;
    for c in contornos {
        n += c.len() as u16;
        be16(&mut v, n - 1);
    }
    be16(&mut v, 0);
    for c in contornos {
        for p in c.iter() {
            v.push(if p.2 { 1 } else { 0 });
        }
    }
    let (mut ax, mut ay) = (0i16, 0i16);
    let mut ys = Vec::new();
    for c in contornos {
        for p in c.iter() {
            be16(&mut v, (p.0 - ax) as u16);
            ax = p.0;
            be16(&mut ys, (p.1 - ay) as u16);
            ay = p.1;
        }
    }
    v.extend_from_slice(&ys);
    v
}

/// **El TrueType de la prueba** (1000 unidades por em): el glifo 1 es un
/// cuadrado de 500, el 2 es el 1 COMPUESTO y movido 500 a la derecha, el 3
/// un circulo de radio 400 de cuatro curvas cuadraticas. La "A" es el 1, la
/// "B" el 2 y la "C" el 3; el resto, el 0 (vacio).
fn ttf_de_prueba() -> Vec<u8> {
    let cuadrado = simple(&[&[(0, 0, true), (0, 500, true), (500, 500, true), (500, 0, true)]]);
    let mut compuesto = Vec::new();
    be16(&mut compuesto, 0xFFFF);
    compuesto.extend_from_slice(&[0; 8]);
    be16(&mut compuesto, 0x0003); // ARG_1_AND_2_ARE_WORDS | ARGS_ARE_XY_VALUES
    be16(&mut compuesto, 1);
    be16(&mut compuesto, 500);
    be16(&mut compuesto, 0);
    let r = 400i16;
    let circulo = simple(&[&[(500, 500 + r, true), (500 + r, 500 + r, false), (500 + r, 500, true), (500 + r, 500 - r, false), (500, 500 - r, true), (500 - r, 500 - r, false), (500 - r, 500, true), (500 - r, 500 + r, false)]]);
    let glifos: Vec<Vec<u8>> = vec![Vec::new(), cuadrado, compuesto, circulo];
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for g in &glifos {
        be16(&mut loca, (glyf.len() / 2) as u16);
        glyf.extend_from_slice(g);
        if glyf.len() % 2 == 1 {
            glyf.push(0);
        }
    }
    be16(&mut loca, (glyf.len() / 2) as u16);
    let mut head = vec![0u8; 54];
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let mut maxp = vec![0u8; 6];
    maxp[4..6].copy_from_slice(&4u16.to_be_bytes());
    let mut hhea = vec![0u8; 36];
    hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
    hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
    hhea[34..36].copy_from_slice(&4u16.to_be_bytes());
    let mut hmtx = Vec::new();
    for av in [500u16, 500, 1100, 1000] {
        be16(&mut hmtx, av);
        be16(&mut hmtx, 0);
    }
    // cmap de formato 4: 65..=67 -> 1..=3 (delta -64), y el 0xFFFF de cierre.
    let mut sub = Vec::new();
    for x in [4u16, 32, 0, 4, 4, 1, 0, 67, 0xFFFF, 0, 65, 0xFFFF, (-64i16) as u16, 1, 0, 0] {
        be16(&mut sub, x);
    }
    let mut cmap = Vec::new();
    for x in [0u16, 1, 3, 1] {
        be16(&mut cmap, x);
    }
    cmap.extend_from_slice(&12u32.to_be_bytes());
    cmap.extend_from_slice(&sub);
    let tablas: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"cmap", cmap), (b"glyf", glyf), (b"head", head), (b"hhea", hhea), (b"hmtx", hmtx), (b"loca", loca), (b"maxp", maxp)];
    let mut f = Vec::new();
    f.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    be16(&mut f, tablas.len() as u16);
    f.extend_from_slice(&[0; 6]);
    let mut sitio = 12 + 16 * tablas.len();
    let mut datos = Vec::new();
    for (nombre, t) in &tablas {
        f.extend_from_slice(&nombre[..]);
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(&(sitio as u32).to_be_bytes());
        f.extend_from_slice(&(t.len() as u32).to_be_bytes());
        datos.extend_from_slice(t);
        while datos.len() % 4 != 0 {
            datos.push(0);
        }
        sitio = 12 + 16 * tablas.len() + datos.len();
    }
    f.extend_from_slice(&datos);
    f
}

fn de(a: &Atlas, c: u8) -> &bmo_atlas::Glifo {
    &a.glifos[(c - 32) as usize]
}

/// ** El cuadrado a 20 px por em: 10 x 10 pixeles LLENOS, sobre la base; su
/// avance, 500 unidades = 10 px (640 en 1/64). Las medidas de la linea, de
/// `hhea`.
#[test]
fn a_truetype_square_is_drawn_full_and_on_the_baseline() {
    let a = atlas(&ttf_de_prueba(), 20).unwrap();
    assert_eq!((a.ascenso, a.descenso, a.linea), (16, -4, 20));
    let g = de(&a, b'A');
    assert_eq!((g.ancho, g.alto, g.x, g.y), (10, 10, 0, 10));
    assert!(g.alfa.iter().all(|&x| x == 255), "{:?}", g.alfa);
    assert_eq!(g.avance, 640);
}

/// ** El glifo COMPUESTO es el cuadrado movido 500 unidades (10 px): misma
/// tinta, diez pixeles a la derecha. Asi sale una "a" con tilde.
#[test]
fn a_composite_glyph_is_its_piece_moved() {
    let a = atlas(&ttf_de_prueba(), 20).unwrap();
    let (q, b) = (de(&a, b'A'), de(&a, b'B'));
    assert_eq!(b.x, q.x + 10);
    assert_eq!(tinta(&b.alfa), tinta(&q.alfa));
    assert_eq!(b.avance as u32, 1100 * 64 * 20 / 1000);
}

/// ** EL JUEZ DEL BORDE: la tinta del circulo (curvas cuadraticas) es el
/// AREA de su contorno, en pixeles, a cada medida -- el borde suave es la
/// parte del pixel que queda dentro, medida --. Y una letra que falta sale
/// como "?" si la hay (aqui no: vacia).
#[test]
fn the_ink_of_a_curve_is_its_area() {
    let f = ttf_de_prueba();
    let t = ttf::Ttf::leer(&f).unwrap();
    for medida in [12u8, 20, 33, 64] {
        let a = atlas(&f, medida).unwrap();
        let g = de(&a, b'C');
        let s = medida as f64 / 1000.0;
        let tramos = pinta::aplanar(&t.contornos(3).unwrap(), |x, y| (x * s, -y * s));
        let area: f64 = tramos.iter().map(|&((ax, ay), (bx, by))| ax * by - bx * ay).sum::<f64>().abs() / 2.0;
        let pintada = tinta(&g.alfa) as f64 / 255.0;
        assert!((pintada - area).abs() / area < 0.01, "a {medida} px: tinta {pintada:.2} y area {area:.2}");
        // Hay borde suave: no todo es 0 o 255.
        assert!(g.alfa.iter().any(|&x| x > 0 && x < 255));
    }
    let a = atlas(&f, 20).unwrap();
    assert_eq!(tinta(&de(&a, b'z').alfa), 0, "sin 'z' ni '?': vacia");
}

/// ** Lo que no es un TrueType, o esta roto, se dice: nunca un indice fuera.
#[test]
fn a_broken_font_says_why() {
    assert!(atlas(b"ho", 16).unwrap_err().contains("se acaba"));
    assert!(atlas(b"hola", 16).unwrap_err().contains("no es un TrueType"));
    let mut otto = ttf_de_prueba();
    otto[..4].copy_from_slice(b"OTTO");
    assert!(atlas(&otto, 16).unwrap_err().contains("CFF"));
    let mut corto = ttf_de_prueba();
    corto.truncate(200);
    assert!(atlas(&corto, 16).is_err());
    assert!(atlas(&ttf_de_prueba(), 200).unwrap_err().contains("medida"));
}
