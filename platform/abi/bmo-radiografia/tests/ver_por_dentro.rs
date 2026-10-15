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

/// ** El traductor a castellano sabe decir CADA instruccion de ECCE: ni una
/// sale "(sin frase todavia)". Una que entre nueva en el emisor y no tenga
/// frase pone esto rojo.
#[test]
fn every_instruction_of_ecce_has_a_spanish_sentence() {
    let bex = ecce();
    let (mut visto, mut pila) = buffers();
    let r = radiografiar(&bex, &mut visto, &mut pila);
    let c = r.regiones[0].unwrap();
    let code = &bex[c.file_offset as usize..(c.file_offset + c.file_size) as usize];
    let (mut en, mut n) = (0usize, 0usize);
    let mut sin = Vec::new();
    while en < code.len() {
        let Ok(i) = bmo_lente_x86::lee(&code[en..]) else {
            en += 1;
            continue;
        };
        let f = bmo_lente_x86::castellano::explica(&i, en as u64);
        if f.as_str().contains("sin frase") {
            sin.push(String::from(f.as_str()));
        }
        n += 1;
        en += i.largo;
    }
    assert!(n > 10_000, "{n}");
    assert!(sin.is_empty(), "{} sin frase: {:?}", sin.len(), &sin[..sin.len().min(5)]);
}

/// ** La MESA DE KATANAS de un `.ibx`: cada regla con su nombre y su trampa,
/// y la que apunta fuera del codigo se dice.
#[test]
fn the_katanas_of_an_inti_binary_are_named() {
    use bmo_abi::bmo_abi::bef::katanas::{construir, Katana};
    use bmo_abi::bmo_abi::bef2::{Escritor, ANEXO_KATANAS};
    // Codigo: un `ret` y dos trampas (`ud2`), como las que pone INTI.
    let codigo = vec![0xC3, 0x0F, 0x0B, 0x0F, 0x0B];
    let mesa = construir(&[
        Katana { codigo: 1001, offset: 1, longitud: 2 },
        Katana { codigo: 1003, offset: 3, longitud: 2 },
    ])
    .unwrap();
    let mut e = Escritor::ejecutable();
    e.codigo(codigo).entrada(0).anexo(ANEXO_KATANAS, mesa);
    let bex = e.construir().unwrap();
    let (mut visto, mut pila) = buffers();
    let r = radiografiar(&bex, &mut visto, &mut pila);
    assert_eq!(r.falta, None);
    assert_eq!(r.katanas_ok, Some(true));
    assert_eq!(r.cuantas_katanas, 2);
    assert_eq!((r.katanas[0].codigo, r.katanas[0].offset, r.katanas[0].dentro), (1001, 1, true));
    assert_eq!(bmo_radiografia::nombre_regla(r.katanas[1].codigo), "dividir entre cero ATRAPA");
    // La trampa, dicha en castellano por la lente.
    let i = bmo_lente_x86::lee(&bex[r.regiones[0].unwrap().file_offset as usize + 1..]).unwrap();
    assert!(bmo_lente_x86::castellano::explica(&i, 1).as_str().contains("trampa"));
    // Sin katanas (ECCE es de TITAN++), no hay mesa.
    let r = radiografiar(&ecce(), &mut visto, &mut pila);
    assert_eq!(r.katanas_ok, None);
}
