//! Las pruebas de la letra, en el anfitrion.

use super::*;

#[test]
fn todo_el_ascii_y_el_castellano_tienen_glifo_bien_escrito() {
    for c in 32u8..=126 {
        let g = glifos::glifo(c).unwrap_or_else(|| panic!("falta '{}'", c as char));
        trazo::valido(g.trazos).unwrap();
        assert!(g.avance > 0);
    }
    for c in [0xA1u8, 0xBF, 0xAA, 0xBA, 0xB0, 0xB7, 0xAC, 0xB4, 0xA8] {
        trazo::valido(glifos::glifo(c).unwrap().trazos).unwrap();
    }
    for c in [0xF1u8, 0xD1, 0xE1, 0xE9, 0xED, 0xF3, 0xFA, 0xFC, 0xC1, 0xC9, 0xCD, 0xD3, 0xDA, 0xDC, 0xE7, 0xC7] {
        assert!(partes(c).is_some(), "falta {c:#x}");
    }
}

#[test]
fn proporcional_la_i_no_ocupa_lo_que_la_m() {
    let mut l = Letra::nueva();
    let e = Estilo::normal(14);
    assert!(l.medir(b"iiii", e) * 3 < l.medir(b"mmmm", e));
    // Y la medida es la suma de los avances (sin kerning).
    assert_eq!(l.medir(b"", e), 0);
}

#[test]
fn la_negrita_es_mas_ancha_y_mas_negra() {
    let mut l = Letra::nueva();
    assert!(l.medir(b"Cartera", Estilo::negrita(14)) > l.medir(b"Cartera", Estilo::normal(14)));
    let tinta = |l: &mut Letra, e: Estilo| {
        let mut t = 0u32;
        l.escribir(b"H", e, 0, 20, |_, _, a| t += a as u32);
        t
    };
    assert!(tinta(&mut l, Estilo::negrita(14)) > tinta(&mut l, Estilo::normal(14)) * 5 / 4);
}

#[test]
fn la_base_y_la_mayuscula_caen_en_pixel_entero() {
    // Una H de 20 px: su palo tiene la tinta llena desde la fila de la
    // mayuscula (14 px por encima de la base) hasta la base, y nada debajo.
    let mut l = Letra::nueva();
    let mut filas = std::collections::BTreeMap::new();
    l.escribir(b"H", Estilo::normal(20), 0, 100, |_, y, a| {
        let m = filas.entry(y).or_insert(0u8);
        *m = (*m).max(a);
    });
    // La pluma es REDONDA: la ultima fila lleva casi toda la tinta, no toda.
    assert!(filas.get(&99).copied().unwrap_or(0) >= 200, "la fila de encima de la base: {:?}", filas.get(&99));
    assert!(filas.get(&100).copied().unwrap_or(0) < 40, "debajo de la base no hay tinta");
    assert!(filas.get(&86).copied().unwrap_or(0) >= 200, "la fila de la mayuscula: {:?}", filas.get(&86));
    assert!(filas.get(&85).copied().unwrap_or(0) < 40);
}

#[test]
fn utf8_y_latin1_dicen_lo_mismo() {
    let mut l = Letra::nueva();
    let e = Estilo::normal(16);
    assert_eq!(l.medir("ca\u{f1}\u{f3}n".as_bytes(), e), l.medir(b"ca\xF1\xF3n", e));
    // Lo que no es Latin-1 se dice '?', y no rompe.
    assert_eq!(l.medir("\u{20ac}".as_bytes(), e), l.medir(b"?", e));
}

#[test]
fn las_mayusculas_del_rotulo() {
    let mut l = Letra::nueva();
    let e = Estilo::normal(11).mayusculas();
    assert_eq!(l.medir(b"tus monederos", e), l.medir(b"TUS MONEDEROS", Estilo::normal(11)));
    let esp = Estilo::normal(11).espaciado(140);
    assert!(l.medir(b"ABC", esp) > l.medir(b"ABC", Estilo::normal(11)));
}

#[test]
fn recortar_con_puntos() {
    let mut l = Letra::nueva();
    let e = Estilo::normal(13);
    let w = l.escribir_cabe(b"la moneda de la casa, muy larga", e, 0, 20, 80, |_, _, _| {});
    assert!(w <= 80, "{w}");
}

#[test]
fn la_caja_de_una_linea_como_el_navegador() {
    // 14 px con line-height 20: (20 - 18.2)/2 + 14.35 = 15.25 -> 15.
    assert_eq!(base_en_caja(14, 20), 15);
    assert_eq!(alto_normal(14), 18);
}

#[test]
fn la_pluma_suaviza_y_no_pinta_dos_veces() {
    let mut max = 0u8;
    let mut medios = 0;
    pluma(&[(0, 0), (64 * 20, 64 * 7), (64 * 40, 0)], 128, false, |_, _, a| {
        max = max.max(a);
        if a > 0 && a < 255 {
            medios += 1;
        }
    });
    assert_eq!(max, 255);
    assert!(medios > 20, "una diagonal suave tiene pixeles a medias: {medios}");
}

#[test]
fn el_camino_de_la_maqueta_se_lee() {
    // La cola del gato: una C y una c encadenada con numeros pegados.
    let s = svg::camino("M196 250c40 0 52-44 26-58-14-8-26 6-16 16");
    assert_eq!(s.len(), 1);
    let p = &s[0].puntos;
    assert_eq!(p[0], (196 * 64, 250 * 64));
    // Fin: 196+40+... la segunda c parte de (222, 192) y acaba en (206, 208).
    assert_eq!(*p.last().unwrap(), (206 * 64, 208 * 64));
    // Dos subcaminos con M y lineas relativas; H, V y Z.
    let s = svg::camino("M110 260v-24M150 260v-24");
    assert_eq!(s.len(), 2);
    assert_eq!(s[1].puntos, vec![(150 * 64, 260 * 64), (150 * 64, 236 * 64)]);
    let s = svg::camino("M5 4h11l3 3v13H5zM8 9h8");
    assert!(s[0].cerrado && !s[1].cerrado);
    assert_eq!(s[0].puntos.last(), Some(&(5 * 64, 20 * 64)));
    // Decimales y q.
    let s = svg::camino("M98 122q7 6 14 0M1.5 .5L2 2");
    assert_eq!(*s[0].puntos.last().unwrap(), (112 * 64, 122 * 64));
    assert_eq!(s[1].puntos[0], (96, 32));
}

#[test]
fn rellenar_un_cuadrado_da_su_area() {
    let mut area = 0u32;
    svg::rellenar(&[vec![(0, 0), (640, 0), (640, 640), (0, 640)]], |_, _, a| area += a as u32);
    assert!((area as i32 - 100 * 255).abs() < 100 * 3, "{area}");
    // Medio pixel de borde: la tinta a medias.
    let mut medio = Vec::new();
    svg::rellenar(&[vec![(0, 0), (96, 0), (96, 64), (0, 64)]], |x, _, a| medio.push((x, a)));
    assert_eq!(medio, vec![(0, 255), (1, 127)]);
}
