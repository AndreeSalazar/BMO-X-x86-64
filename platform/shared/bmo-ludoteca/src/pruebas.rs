//! El banco de J0: cada regla de la lista blanca, el camino y el juez.

use super::*;

const SUMA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn hex(s: &str) -> [u8; 32] {
    suma(s.as_bytes()).unwrap()
}

#[test]
fn las_tres_lineas_se_leen() {
    assert_eq!(
        leer(b"JUEGO cyberpunk2077 gog Cyberpunk 2077\n"),
        Ok(Linea::Juego { id: "cyberpunk2077", tienda: Tienda::Gog, titulo: "Cyberpunk 2077" })
    );
    assert_eq!(
        leer(format!("FICHERO doom2 3 {SUMA_ABC} DOOM II/base/DOOM2.WAD\r\n").as_bytes()),
        Ok(Linea::Fichero { id: "doom2", nombre: "DOOM II/base/DOOM2.WAD", bytes: 3, suma: hex(SUMA_ABC) })
    );
    assert_eq!(leer(b"MOTOR doom2 doom"), Ok(Linea::Motor { id: "doom2", motor: Motor::Doom }));
}

#[test]
fn la_lista_blanca_dice_por_que() {
    let s = SUMA_ABC;
    let casos: &[(&str, Falla)] = &[
        ("HOLA mundo", Falla::Verbo),
        ("JUEGO", Falla::Verbo),
        ("JUEGO x gog", Falla::Campos),
        ("JUEGO Mayus gog T", Falla::Id),
        ("JUEGO aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa gog T", Falla::Id),
        ("JUEGO x origin T", Falla::Tienda),
        ("JUEGO x GOG T", Falla::Tienda),
        ("JUEGO x gog  T", Falla::Titulo),
        ("FICHERO x 3 S /etc/passwd", Falla::Nombre),
        ("FICHERO x 3 S a/../b", Falla::Nombre),
        ("FICHERO x 3 S a//b", Falla::Nombre),
        ("FICHERO x 3 S a  b", Falla::Nombre),
        ("FICHERO x 3 S a /b", Falla::Nombre),
        ("FICHERO x 3 S a;b", Falla::Nombre),
        ("FICHERO x 03 S a", Falla::Bytes),
        ("FICHERO x 1099511627777 S a", Falla::Bytes),
        ("FICHERO x -1 S a", Falla::Bytes),
        ("FICHERO x 3 S", Falla::Campos),
        ("MOTOR x unreal", Falla::Motor),
    ];
    for (l, f) in casos {
        let l = l.replace(" S", &format!(" {s}"));
        assert_eq!(leer(l.as_bytes()).map(|_| ()), Err(*f), "{l}");
    }
    let mayus = format!("FICHERO x 3 {} a", SUMA_ABC.to_uppercase());
    assert_eq!(leer(mayus.as_bytes()).map(|_| ()), Err(Falla::Suma));
    assert_eq!(leer(&[b'J'; 257]).map(|_| ()), Err(Falla::Largo));
    assert_eq!(leer("JUEGO x gog Caf\u{e9}".as_bytes()).map(|_| ()), Err(Falla::NoAscii));
    assert_eq!(leer(b"JUEGO x gog a\tb").map(|_| ()), Err(Falla::NoAscii));
}

const LUDOTECA: &str = "\
# la ludoteca de prueba
JUEGO doom2 gog DOOM II
FICHERO doom2 3 ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad DOOM II/base/DOOM2.WAD
JUEGO quake gog Quake
MOTOR quake quake
JUEGO cyberpunk2077 gog Cyberpunk 2077
FICHERO cyberpunk2077 59945608 ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad bin/x64/Cyberpunk2077.exe

JUEGO witcher3 steam The Witcher 3
";

#[test]
fn cada_juego_tiene_su_camino() {
    let l = Ludoteca::cargar(LUDOTECA.as_bytes()).unwrap();
    assert_eq!(l.juegos.len(), 4);
    assert_eq!(l.camino("doom2"), Some(Camino::Nativo(Motor::Doom)));
    assert_eq!(l.camino("quake"), Some(Camino::Nativo(Motor::Quake)));
    assert_eq!(l.camino("cyberpunk2077"), Some(Camino::ProtonX));
    assert_eq!(l.camino("witcher3"), Some(Camino::Pendiente));
    assert_eq!(l.camino("no-esta"), None);
}

#[test]
fn la_ludoteca_entera_tiene_reglas() {
    assert_eq!(Ludoteca::cargar(b"JUEGO a gog A\nJUEGO a gog B\n").unwrap_err(), (2, Falla::Repetido));
    assert_eq!(Ludoteca::cargar(b"MOTOR a doom\n").unwrap_err(), (1, Falla::Huerfano));
    let huerfano = format!("JUEGO a gog A\nFICHERO b 3 {SUMA_ABC} x\n");
    assert_eq!(Ludoteca::cargar(huerfano.as_bytes()).unwrap_err(), (2, Falla::Huerfano));
    assert_eq!(Ludoteca::cargar(b"JUEGO a gog A\nBASURA\n").unwrap_err(), (2, Falla::Verbo));
}

#[test]
fn el_dato_delata_al_motor_sin_mirar_mayusculas() {
    assert_eq!(Motor::por_dato("DOOM2.WAD"), Some(Motor::Doom));
    assert_eq!(Motor::por_dato("x/Id1/Pak0.pak"), Some(Motor::Quake));
    assert_eq!(Motor::por_dato("nodoom2.wad"), None, "acabar en el nombre no basta: va detras de una /");
    assert_eq!(Motor::Hexen.nombre(), "hexen");
}

#[test]
fn el_juez_de_la_suma() {
    let f = Fichero { id: "x".into(), nombre: "a".into(), bytes: 3, suma: hex(SUMA_ABC) };
    assert_eq!(juzgar(&f, b"abc"), Veredicto::Bien);
    assert_eq!(juzgar(&f, b"abd"), Veredicto::Suma);
    assert_eq!(juzgar(&f, b"ab"), Veredicto::Largo);
    let mut j = Juez::para(&f);
    assert!(j.mete(b"a") && j.mete(b"bc"));
    assert_eq!(j.veredicto(), Veredicto::Bien);
    let mut j = Juez::para(&f);
    assert!(!j.mete(b"abcd"), "pasarse del largo se dice en el trozo");
    assert_eq!(j.veredicto(), Veredicto::Largo);
}

#[test]
fn las_tiendas_globales_van_y_vuelven() {
    let mut n = 0;
    for t in Tienda::todas() {
        let l = format!("JUEGO x {} T", t.corto());
        assert_eq!(leer(l.as_bytes()), Ok(Linea::Juego { id: "x", tienda: t, titulo: "T" }));
        assert!(!t.nombre().is_empty());
        n += 1;
    }
    assert_eq!(n, 12);
}
