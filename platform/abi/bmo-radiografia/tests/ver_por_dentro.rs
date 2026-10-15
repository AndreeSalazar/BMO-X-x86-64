//! La radiografia de `.bex` de verdad: ECCE (TITAN++, con ventana) y uno roto.

use std::path::Path;

use bmo_radiografia::{nombre_anexo, nombre_clase, radiografiar, Cual, Falta};

fn ecce() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Ultra_userspace/apps/ecce");
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn buffers() -> (Vec<u8>, Vec<u32>) {
    (vec![0u8; 128 * 1024], vec![0u32; 32 * 1024])
}

/// ** ECCE por dentro: la puerta dice que si, pide pantalla, trae codigo, y
/// la lente lo lee entero y lo DEMUESTRA (lo que el kernel hace al admitir).
#[test]
fn ecce_seen_from_inside() {
    let bex = ecce();
    let (mut visto, mut pila) = buffers();
    let r = radiografiar(&bex, &mut visto, &mut pila);
    assert_eq!(r.falta, None);
    assert!(r.admisible());
    // ECCE es una ventana: lo declara su MANIFIESTO (el `screen = true` del
    // Titan.toml), no la bandera de la cabecera. Y es texto, para leerlo.
    let (o, n) = r.manifiesto.expect("TITAN++ no escribe un .bex sin manifiesto");
    let texto = std::str::from_utf8(&bex[o as usize..(o + n) as usize]).expect("el manifiesto es texto");
    assert!(texto.contains("screen = true") && texto.contains("ECCE: el bloc de notas"), "{texto}");
    assert_eq!(r.bytes, bex.len() as u64);
    let codigo = r.regiones[0].expect("trae codigo");
    assert_eq!(codigo.cual, Cual::Codigo);
    assert!(codigo.file_size > 1000);
    assert!(r.entrada < codigo.file_size);
    let lente = r.lente.expect("la lente se paso");
    assert!(lente.leidas > 1000, "{lente:?}");
    assert!(lente.demostrado(), "{lente:?}");
    // Los papeles: algo pide, y cada anexo y clase tiene nombre.
    assert!(r.cuantos_anexos >= 1);
    for a in &r.anexos[..r.cuantos_anexos.min(16)] {
        assert_ne!(nombre_anexo(a.tipo), "");
    }
    for q in &r.requisitos[..r.cuantos_requisitos.min(16)] {
        assert_ne!(nombre_clase(q.clase), "");
    }
}

/// ** Lo que la puerta dice que NO, se ve con su nombre, y no se lee nada mas.
#[test]
fn a_broken_bex_says_why() {
    let (mut visto, mut pila) = buffers();
    let mut roto = ecce();
    roto[0] ^= 0xFF;
    let r = radiografiar(&roto, &mut visto, &mut pila);
    assert_eq!(r.falta, Some(Falta::OtroFormato));
    assert!(r.lente.is_none() && r.regiones.iter().all(|x| x.is_none()));
    assert!(!r.admisible());

    let entero = ecce();
    let cortado = &entero[..entero.len() / 2];
    let r = radiografiar(cortado, &mut visto, &mut pila);
    assert!(r.falta.is_some(), "medio .bex no pasa");
}

/// ** Sin sitio para el mapa de la lente, se dice (lente None) en vez de
/// leer a medias.
#[test]
fn without_room_the_lens_is_not_run() {
    let bex = ecce();
    let mut visto = vec![0u8; 8];
    let mut pila = vec![0u32; 8];
    let r = radiografiar(&bex, &mut visto, &mut pila);
    assert_eq!(r.falta, None);
    assert!(r.lente.is_none());
}
