//! Las pruebas del juez: los casos con nombre, y una FAT32 de mentira que
//! sigue los `.` y `..` como la de verdad, para comprobar que lo resuelto
//! nunca sube por encima de la raiz.

use super::*;

extern crate std;
use std::string::String;
use std::vec::Vec;

fn raiz(t: &str) -> Raiz {
    Raiz::para_hijo(None, t).unwrap()
}

fn r(raiz: &Raiz, pedida: &str) -> Result<String, NoRuta> {
    let mut d = [0u8; RUTA_MAX];
    raiz.resolver(pedida, &mut d).map(|n| String::from(core::str::from_utf8(&d[..n]).unwrap()))
}

#[test]
fn lo_normal_cae_dentro() {
    let h = raiz("hermes");
    assert_eq!(r(&h, "charla/nova.txt").as_deref(), Ok("hermes/charla/nova.txt"));
    assert_eq!(r(&h, "/charla/nova.txt").as_deref(), Ok("hermes/charla/nova.txt"), "la barra de delante no saca a la raiz del volumen");
    assert_eq!(r(&h, "charla\\nova.txt").as_deref(), Ok("hermes/charla/nova.txt"));
    assert_eq!(r(&h, "charla//nova.txt/").as_deref(), Ok("hermes/charla/nova.txt"));
    assert_eq!(r(&h, "").as_deref(), Ok("hermes"), "vacia es la raiz misma, no la del volumen");
    assert_eq!(r(&h, "///").as_deref(), Ok("hermes"));
    assert_eq!(r(&h, "fotos/atardecer en la sierra.jpg").as_deref(), Ok("hermes/fotos/atardecer en la sierra.jpg"));
    assert_eq!(r(&h, ".oculto/a.b.c").as_deref(), Ok("hermes/.oculto/a.b.c"), "un punto delante de un nombre no sube");
}

#[test]
fn las_tres_formas_de_salir_que_hay_en_el_arbol() {
    let h = raiz("hermes");
    assert_eq!(r(&h, "../sys/director.bex"), Err(NoRuta::Subir));
    assert_eq!(r(&h, "charla/../../sys"), Err(NoRuta::Subir));
    assert_eq!(r(&h, ".../sys"), Err(NoRuta::Subir), "to_8_3(\"...\") es `..` en FAT32");
    assert_eq!(r(&h, "./x"), Err(NoRuta::Subir));
    assert_eq!(r(&h, ".. ./sys"), Err(NoRuta::Subir), "to_8_3(\".. .\") tambien es `..`: lo encontro la prueba de abajo");
    assert_eq!(r(&h, ". ./sys"), Err(NoRuta::Subir));
    assert_eq!(r(&h, "d:Cyberpunk 2077/bin"), Err(NoRuta::Letra));
    assert_eq!(r(&h, "D:\\Juegos"), Err(NoRuta::Letra));
    assert_eq!(r(&h, "c:sys/director.bex"), Err(NoRuta::Letra), "FAT32 se come cualquier letra");
    assert_eq!(r(&h, "fotos/a:b"), Err(NoRuta::Letra));
}

#[test]
fn los_bordes_y_los_controles() {
    let h = raiz("hermes");
    assert_eq!(r(&h, " ../sys"), Err(NoRuta::Subir), "puntos y blancos: sube antes que borde");
    assert_eq!(r(&h, ".. /sys"), Err(NoRuta::Subir));
    assert_eq!(r(&h, "\u{A0}../sys"), Err(NoRuta::Subir), "un espacio duro tambien es blanco");
    assert_eq!(r(&h, " fotos/x"), Err(NoRuta::Borde));
    assert_eq!(r(&h, "fotos /x"), Err(NoRuta::Borde));
    assert_eq!(r(&h, "fotos/x\u{A0}"), Err(NoRuta::Borde));
    assert_eq!(r(&h, "a\0b"), Err(NoRuta::Control));
    assert_eq!(r(&h, "a\nb"), Err(NoRuta::Control));
    assert_eq!(r(&h, "a\x7Fb"), Err(NoRuta::Control));
}

#[test]
fn lo_que_no_cabe_no_se_corta() {
    let h = raiz("hermes");
    let larga: String = core::iter::repeat_n('a', RUTA_MAX).collect();
    assert_eq!(r(&h, &larga), Err(NoRuta::Larga), "cortar una ruta es abrir OTRA ruta");
    let justa: String = core::iter::repeat_n('a', RUTA_MAX - "hermes/".len()).collect();
    assert_eq!(r(&h, &justa).map(|s| s.len()), Ok(RUTA_MAX));
}

#[test]
fn la_raiz_de_un_hijo_nunca_es_mas_ancha() {
    let h = raiz("hermes");
    let e = Raiz::para_hijo(Some(&h), "entrantes").unwrap();
    assert_eq!(e.texto(), "hermes/entrantes");
    assert_eq!(Raiz::para_hijo(Some(&h), "/sys").unwrap().texto(), "hermes/sys", "la barra no sale del padre");
    assert_eq!(Raiz::para_hijo(Some(&h), ".."), Err(NoRuta::Subir));
    assert_eq!(Raiz::para_hijo(Some(&h), "d:"), Err(NoRuta::Letra));
    assert_eq!(Raiz::para_hijo(Some(&h), "").unwrap(), h, "sin pedir nada, la misma del padre");
    assert_eq!(Raiz::para_hijo(None, ""), Err(NoRuta::Raiz), "una raiz vacia no encierra");
    assert_eq!(Raiz::para_hijo(None, "///"), Err(NoRuta::Raiz));
    let larga: String = core::iter::repeat_n('x', RAIZ_MAX + 1).collect();
    assert_eq!(Raiz::para_hijo(None, &larga), Err(NoRuta::Raiz));
    assert_eq!(raiz("/hermes//").texto(), "hermes");
}

// == la FAT32 de mentira =====================================================

/// El nombre en 8.3 como lo hace `ring0/fsys/fs.rs::to_8_3`: el ULTIMO punto
/// separa. Es la funcion que convierte `...` en `..`.
fn a_8_3(t: &str) -> Option<[u8; 11]> {
    let b = t.as_bytes();
    let mut out = [b' '; 11];
    let (stem, ext) = match b.iter().rposition(|&c| c == b'.') {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (b, &b[0..0]),
    };
    if b.is_empty() || stem.is_empty() || stem.len() > 8 || ext.len() > 3 {
        return None;
    }
    out[..stem.len()].copy_from_slice(stem);
    out[8..8 + ext.len()].copy_from_slice(ext);
    Some(out)
}

/// **Cuantos niveles por debajo de la raiz del volumen** acaba una ruta,
/// siguiendo `.` y `..` como las entradas de FAT32 y quitando la letra de
/// unidad como `fs.rs`. `None` si en algun momento sube por encima del volumen.
fn profundidad_fat(ruta: &str) -> Option<i32> {
    let mut p = ruta;
    if p.len() >= 2 && p.as_bytes()[1] == b':' {
        p = &p[2..];
    }
    let mut d: i32 = 0;
    for t in p.split(['/', '\\']).filter(|t| !t.is_empty()) {
        match a_8_3(t) {
            Some(n) if &n == b".          " => {}
            Some(n) if &n == b"..         " => {
                d -= 1;
                if d < 0 {
                    return None;
                }
            }
            _ => d += 1,
        }
    }
    Some(d)
}

#[test]
fn la_fat_de_mentira_es_la_de_verdad() {
    assert_eq!(a_8_3("..").map(|n| n[0..2] == *b". "), Some(true), "`..` en 8.3 es `.`: el ultimo punto separa");
    assert_eq!(&a_8_3("...").unwrap(), b"..         ", "y `...` es `..`");
    assert_eq!(&a_8_3(".. .").unwrap(), b"..         ", "y `.. .` tambien");
    assert_eq!(profundidad_fat("hermes/.../..."), None);
    assert_eq!(profundidad_fat("hermes/charla"), Some(2));
}

/// **Cien mil rutas hechas a mala idea**: con piezas que se sabe que suben,
/// letras, bordes y basura. Lo que el juez acepta tiene que caer, en la FAT32
/// de mentira, a la profundidad de la raiz o mas abajo -- nunca encima.
#[test]
fn nada_de_lo_que_acepta_sube_por_encima_de_la_raiz() {
    const PIEZAS: &[&str] = &[
        "..", "...", "....", ".", " ..", ".. ", "x", "sys", "hermes", "d:", "c:", "a:b", "/", "\\", "//",
        "fotos", "\u{A0}", " ", "\t", "\0", "director.bex", ". .", "a.b", ".oculto", "\u{202E}", "~",
    ];
    let h = raiz("hermes/charla");
    let base = profundidad_fat(h.texto()).unwrap();
    let mut semilla: u64 = 0x5EED_0003;
    let mut aceptadas = 0;
    for _ in 0..100_000 {
        let mut ruta = String::new();
        semilla ^= semilla << 13;
        semilla ^= semilla >> 7;
        semilla ^= semilla << 17;
        let piezas = 1 + (semilla % 6) as usize;
        let mut s = semilla;
        for _ in 0..piezas {
            ruta.push_str(PIEZAS[(s % PIEZAS.len() as u64) as usize]);
            s /= PIEZAS.len() as u64;
            if s % 3 != 0 {
                ruta.push(if s % 2 == 0 { '/' } else { '\\' });
            }
            s /= 3;
        }
        if let Ok(t) = r(&h, &ruta) {
            aceptadas += 1;
            let d = profundidad_fat(&t);
            assert!(d.is_some_and(|d| d >= base), "{:?} -> {:?} sube (profundidad {:?}, la raiz {})", ruta, t, d, base);
            assert!(t == h.texto() || t.starts_with("hermes/charla/"), "{:?} -> {:?} no empieza por la raiz", ruta, t);
            assert!(!t.contains(':'));
        }
    }
    assert!(aceptadas > 1000, "solo {} aceptadas: la prueba no prueba nada", aceptadas);
}

#[test]
fn los_hijos_de_hijos_siguen_dentro() {
    let mut actual = raiz("hermes");
    let mut nombres: Vec<String> = Vec::new();
    for i in 0..5 {
        let n = std::format!("n{}", i);
        actual = Raiz::para_hijo(Some(&actual), &n).unwrap();
        nombres.push(n);
        assert!(actual.texto().starts_with("hermes/"));
    }
    assert_eq!(actual.texto(), "hermes/n0/n1/n2/n3/n4");
}
