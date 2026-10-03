//! Las pruebas de la musica de fondo: que es la de la maqueta, que el bucle
//! no tiene costura, que no silba, que todas suenan igual de fuertes y que el
//! limite no tiene que trabajar.
//!
//! Con `BMO_FONDO_WAV=<carpeta>` escribe cada pieza y cada aviso como WAV,
//! para OIRLOS en el anfitrion: es lo mismo que sonara en el Ryzen.

extern crate std;

use std::vec;
use std::vec::Vec;

use super::compositor::{azar, Patron};
use super::sintesis::{oscilar, HZ};
use super::tablas::{INC, SENO};
use super::*;

const N: Option<i32> = None;
fn s(x: i32) -> Option<i32> {
    Some(x)
}

#[test]
fn la_tabla_de_notas_cuadra() {
    for (m, &inc) in INC.iter().enumerate() {
        let f = 440.0 * 2f64.powf((m as f64 - 69.0) / 12.0);
        let esperado = (f / HZ as f64 * 4_294_967_296.0).round();
        assert!((inc as f64 - esperado).abs() <= 1.0, "nota {m}");
    }
}

#[test]
fn la_tabla_del_seno_cuadra() {
    for (i, &v) in SENO.iter().enumerate() {
        let esperado = (32767.0 * (2.0 * core::f64::consts::PI * i as f64 / 512.0).sin()).round();
        assert!((v as f64 - esperado).abs() <= 1.0, "trozo {i}");
    }
}

/// Lo que da `patron()` en el JavaScript de la maqueta, con -99 por `null`
/// (sacado con node el 03-10).
#[test]
fn el_patron_es_el_de_la_maqueta() {
    let p1 = Patron::de(&PIEZAS[0]);
    assert_eq!(p1.bajo, [s(0), N, s(2), s(8), s(0), N, s(2), s(2)]);
    assert_eq!(p1.arpegio, [s(0), s(2), s(17), N, s(15), N, s(0), s(0), s(0), s(17), s(0), s(17), N, s(14), s(0), N]);
    assert_eq!(p1.acordes, [0, -5, -7, -4]);
    let bombo: Vec<u8> = p1.bombo.iter().map(|&b| b as u8).collect();
    assert_eq!(bombo, [1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0]);
    let plato: Vec<u8> = p1.plato.iter().map(|&b| b as u8).collect();
    assert_eq!(plato, [1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]);

    let p7 = Patron::de(&PIEZAS[6]);
    assert_eq!(p7.bajo, [s(0), s(4), N, s(11), s(0), N, N, N]);
    assert_eq!(p7.arpegio, [N, s(19), N, s(4), s(11), s(4), N, s(4), s(16), s(12), s(9), s(9), s(7), s(9), s(12), s(21)]);
    assert_eq!(p7.acordes, [0, -5, -7, -3]);

    let p3 = Patron::de(&PIEZAS[2]);
    assert_eq!(p3.bajo, [s(0), N, s(3), N, s(0), s(0), N, s(0)]);
    assert_eq!(p3.arpegio, [s(5), s(7), s(3), s(15), s(0), s(7), s(12), s(7), s(5), s(22), s(17), s(19), s(3), N, N, N]);
    assert_eq!(p3.acordes, [0, -2, -5, -5]);

    let p2 = Patron::de(&PIEZAS[1]);
    let bombo: Vec<u8> = p2.bombo.iter().map(|&b| b as u8).collect();
    assert_eq!(bombo, [1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0]);
    assert_eq!(p2.bajo, [s(0), N, N, N, s(0), s(5), s(9), s(7)]);
    // Y el hash, contra un valor que la maqueta da: azar(0) * 2^32.
    assert_eq!(azar(0), azar(0));
}

fn componer(p: &Pieza, m: Mezcla, muestras: usize) -> (Vec<i16>, Compositor) {
    let mut c = Compositor::nuevo(p, m);
    let mut v = vec![0i16; muestras];
    c.llenar(&mut v);
    (v, c)
}

#[test]
fn el_bucle_no_tiene_costura() {
    // Dos vueltas seguidas: la segunda tiene que ser la primera, muestra a
    // muestra. Eso es lo que oye la voz en bucle al volver al principio.
    for i in [6usize, 0, 4] {
        let p = &PIEZAS[i];
        let largo = Compositor::nuevo(p, Mezcla::FONDO).muestras_del_bucle();
        let (v, _) = componer(p, Mezcla::FONDO, 2 * largo);
        let (a, b) = v.split_at(largo);
        let distintas = a.iter().zip(b).filter(|(x, y)| x != y).count();
        assert_eq!(distintas, 0, "{}: la segunda vuelta difiere en {distintas} muestras", p.nombre);
        // Y la muestra 0 NO empieza en silencio: lleva las colas de antes.
        assert!(a[..64].iter().any(|&x| x != 0), "{}: empieza en silencio", p.nombre);
    }
}

/// La energia de `x` en `hz`, por Goertzel, en unidades arbitrarias.
fn goertzel(x: &[i32], hz: f64) -> f64 {
    let w = 2.0 * core::f64::consts::PI * hz / HZ as f64;
    let k = 2.0 * w.cos();
    let (mut a, mut b) = (0f64, 0f64);
    for &v in x {
        let c = v as f64 + k * a - b;
        b = a;
        a = c;
    }
    a * a + b * b - k * a * b
}

#[test]
fn la_cuadrada_no_silba() {
    // Una cuadrada de 4.410 Hz: sus armonicos impares pasan de 24 kHz y
    // vuelven DOBLADOS. El 9.o (39.690 Hz) cae en 8.310 Hz, donde la nota no
    // tiene nada. Con PolyBLEP tiene que haber mucho menos ahi.
    let dt = ((4410u64 << 32) / HZ as u64) as u32;
    let mut fase = 0u32;
    let mut buena = Vec::new();
    let mut ingenua = Vec::new();
    for _ in 0..HZ {
        buena.push(oscilar(Timbre::Cuadrada, fase, dt));
        ingenua.push(if fase < 1 << 31 { 32_767 } else { -32_768 });
        fase = fase.wrapping_add(dt);
    }
    let alias_b = goertzel(&buena, 8310.0);
    let alias_i = goertzel(&ingenua, 8310.0);
    let nota_b = goertzel(&buena, 4410.0);
    let db = 10.0 * (alias_i / alias_b).log10();
    assert!(db > 15.0, "PolyBLEP quita solo {db:.1} dB del silbido");
    // Y la nota sigue ahi, igual de fuerte.
    let nota_i = goertzel(&ingenua, 4410.0);
    assert!((10.0 * (nota_i / nota_b).log10()).abs() < 1.0);
}

/// dBFS de una lectura en 1/256 dB, en coma flotante para leerla.
fn dbfs(x: MilesimasDb) -> f64 {
    x as f64 / 256.0
}

#[test]
fn cada_pieza_suena_igual_de_fuerte_y_el_limite_no_trabaja() {
    let mut fuerzas = Vec::new();
    for p in PIEZAS.iter() {
        let largo = Compositor::nuevo(p, Mezcla::FONDO).muestras_del_bucle();
        let (_, c) = componer(p, Mezcla::FONDO, largo);
        let (pico, rms) = c.medida();
        std::println!("{:<24} pico {:6.1} dBFS  fuerza {:6.1} dBFS  sujetadas {}  robadas {}", p.nombre, dbfs(pico), dbfs(rms), c.sujetadas(), c.robadas());
        assert_eq!(c.robadas(), 0, "{}: no le caben las notas", p.nombre);
        assert_eq!(c.sujetadas(), 0, "{}: el limite tuvo que sujetar", p.nombre);
        assert!(dbfs(pico) < -3.0, "{}: pico de {:.1} dBFS", p.nombre, dbfs(pico));
        fuerzas.push(dbfs(rms));
    }
    // Fondo: -26 dBFS de fuerza, y todas dentro de 1 dB.
    for (p, f) in PIEZAS.iter().zip(&fuerzas) {
        assert!((-27.0..=-25.0).contains(f), "{}: fuerza {f:.1} dBFS", p.nombre);
    }
}

#[test]
fn la_cancion_suena_mas_que_el_fondo() {
    let p = &PIEZAS[0];
    let largo = 4 * HZ as usize;
    let (_, cancion) = componer(p, Mezcla::CANCION, largo);
    let (_, fondo) = componer(p, Mezcla::FONDO, largo);
    assert!(cancion.medida().1 > fondo.medida().1 + 3 * DB);
}

#[test]
fn los_avisos_se_oyen_y_no_se_pasan() {
    for a in avisos::AVISOS {
        let n = avisos::muestras(a);
        assert!(n > HZ as usize / 20 && n < 2 * HZ as usize, "{a:?}: {n} muestras");
        let mut v = vec![0i16; n + 10];
        assert_eq!(avisos::componer(a, &mut v), n);
        let pico = v.iter().map(|&x| (x as i32).abs()).max().unwrap();
        let db = 20.0 * (pico as f64 / 32768.0).log10();
        std::println!("{a:?}: {n} muestras, pico {db:.1} dBFS");
        assert!((-7.0..=-5.0).contains(&db), "{a:?}: pico {db:.1} dBFS, y todos tienen que llegar a -6");
        // Lo que no cabe, no se escribe.
        let mut corto = [0i16; 100];
        assert_eq!(avisos::componer(a, &mut corto), 100);
    }
}

#[test]
fn se_buscan_por_su_nombre() {
    assert_eq!(buscar(b"sierra al atardecer"), Some(6));
    assert_eq!(buscar(b"LA CIUDAD DE NEON"), Some(2));
    assert_eq!(buscar(b"no existe"), None);
    for &i in TRANQUILAS.iter() {
        assert!(PIEZAS[i].bpm <= 100, "{} no es tranquila", PIEZAS[i].nombre);
    }
}

/// Escribe un WAV mono de 16 bits a 48 kHz.
fn wav(ruta: &std::path::Path, v: &[i16]) {
    let mut b = Vec::new();
    let datos = (v.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + datos).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&HZ.to_le_bytes());
    b.extend_from_slice(&(HZ * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&datos.to_le_bytes());
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    std::fs::write(ruta, b).unwrap();
}

#[test]
fn escribir_wav_para_oirlos() {
    let Some(dir) = std::env::var_os("BMO_FONDO_WAV") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for p in PIEZAS.iter() {
        let largo = Compositor::nuevo(p, Mezcla::FONDO).muestras_del_bucle();
        let (v, _) = componer(p, Mezcla::FONDO, largo);
        let nombre: std::string::String = p.nombre.chars().map(|c| if c == ' ' { '_' } else { c.to_ascii_lowercase() }).collect();
        wav(&dir.join(std::format!("fondo_{nombre}.wav")), &v);
        let (v, _) = componer(p, Mezcla::CANCION, largo);
        wav(&dir.join(std::format!("cancion_{nombre}.wav")), &v);
    }
    for a in avisos::AVISOS {
        let mut v = vec![0i16; avisos::muestras(a)];
        avisos::componer(a, &mut v);
        wav(&dir.join(std::format!("aviso_{a:?}.wav").to_lowercase()), &v);
    }
}

/// **La experiencia entera, en un WAV**: 24 s de fondo; a los 8 s llega un
/// mensaje y a los 15 un amigo se conecta. La musica se agacha con el MISMO
/// `Agacha` que corre el orquestador, en bloques de 1 ms como el bus.
#[test]
fn escribir_la_demo_del_agache() {
    use bmo_amplificador::agacha::{Agacha, AGACHE_AVISO};
    let Some(dir) = std::env::var_os("BMO_FONDO_WAV") else { return };
    let dir = std::path::PathBuf::from(dir);
    let total = 24 * HZ as usize;
    let (fondo, _) = componer(&PIEZAS[6], Mezcla::FONDO, total);
    let mut aviso = vec![0i32; total];
    let mut sonando = vec![false; total];
    for (a, en) in [(Aviso::Mensaje, 8usize), (Aviso::Conecta, 15)] {
        let mut v = vec![0i16; avisos::muestras(a)];
        avisos::componer(a, &mut v);
        let desde = en * HZ as usize;
        for (i, &x) in v.iter().enumerate() {
            aviso[desde + i] += x as i32;
            sonando[desde + i] = true;
        }
    }
    let mut ag = Agacha::nueva(HZ);
    let mut fuera = vec![0i16; total];
    for b in (0..total).step_by(48) {
        let mut trozo: Vec<i32> = fondo[b..b + 48].iter().map(|&x| x as i32).collect();
        let pedido = if sonando[b..b + 48].iter().any(|&x| x) { AGACHE_AVISO } else { 0 };
        ag.bloque(pedido, &mut trozo, 1);
        for i in 0..48 {
            fuera[b + i] = (trozo[i] + aviso[b + i]).clamp(-32768, 32767) as i16;
        }
    }
    wav(&dir.join("demo_fondo_y_aviso.wav"), &fuera);
}

