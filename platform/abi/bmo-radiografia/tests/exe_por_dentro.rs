//! **LX1: un `.exe` de Windows por dentro**, contra el cargador de PROTON-X.
//!
//! Los `.exe` son los de `platform/shared/proton-x/prueba/` (MSVC, de verdad).
//! El lector de la radiografia no tiene monton; el de PROTON-X si. Los dos
//! tienen que ver las MISMAS secciones y las MISMAS importaciones, y el mismo
//! veredicto: si uno dice otra cosa, uno de los dos miente.

use std::path::Path;

use bmo_proton_x::cargar::{self, Funcion};
use bmo_radiografia::{lente, pe};

fn los_exe() -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/proton-x/prueba");
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .expect("la carpeta de pruebas de PROTON-X")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "exe"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect();
    v.sort();
    assert!(v.len() > 20, "pocos .exe de prueba: {}", v.len());
    v
}

#[test]
fn las_secciones_y_lo_que_pide_son_las_de_proton_x() {
    let mut importaciones = 0usize;
    for (nombre, d) in los_exe() {
        let suyo = bmo_proton_x::pe::leer(&d);
        let mio = pe::leer(&d);
        let (suyo, mio) = match (suyo, mio) {
            (Ok(s), Ok(m)) => (s, m),
            (Err(_), Err(_)) => continue,
            (s, m) => panic!("{nombre}: PROTON-X dice {:?} y la radiografia {:?}", s.map(|_| ()), m.map(|_| ())),
        };
        assert_eq!(mio.base, suyo.base, "{nombre}: la base");
        assert_eq!(mio.entrada, suyo.entrada, "{nombre}: la entrada");
        assert_eq!(mio.secciones().len(), suyo.secciones.len(), "{nombre}: cuantas secciones");
        for (a, b) in mio.secciones().iter().zip(&suyo.secciones) {
            assert_eq!(a.nombre(), b.nombre.as_bytes(), "{nombre}");
            assert_eq!((a.rva, a.tam_virtual, a.desde, a.tam_fichero), (b.rva, b.tam_virtual, b.desde, b.tam_en_fichero), "{nombre}: {}", b.nombre);
        }
        let img = cargar::colocar(&suyo, &d, suyo.base).expect("PROTON-X lo coloca");
        let esperado: Vec<(String, String, u32)> = cargar::importaciones(&suyo, &img)
            .expect("las importaciones de PROTON-X")
            .into_iter()
            .map(|i| {
                let f = match i.funcion {
                    Funcion::Nombre(n) => n,
                    Funcion::Ordinal(o) => format!("#{o}"),
                };
                (i.dll, f, i.ranura)
            })
            .collect();
        let mut visto = Vec::new();
        mio.importaciones(&d, |i| {
            let f = match i.nombre {
                Some(n) => String::from_utf8_lossy(n).into_owned(),
                None => format!("#{}", i.ordinal),
            };
            visto.push((String::from_utf8_lossy(i.dll).into_owned(), f, i.ranura));
        })
        .unwrap_or_else(|e| panic!("{nombre}: {e:?}"));
        assert_eq!(visto, esperado, "{nombre}: lo que pide");
        importaciones += visto.len();
    }
    assert!(importaciones > 500, "solo {importaciones} importaciones en todos");
}

/// El BMOX-12 de EPICX (P3c): la lente lo lee desde la entrada y desde cada
/// funcion de `.pdata`, cada instruccion tiene su frase, y un `call [rip+x]`
/// a la IAT se sabe A QUIEN llama.
#[test]
fn el_bmox12_por_dentro_en_castellano() {
    let d = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shared/proton-x/prueba/bmox12.exe")).unwrap();
    let p = pe::leer(&d).expect("PROTON-X lo acepta");
    let c = *p.codigo().expect("su .text");
    assert_eq!(c.nombre(), b".text");
    let mut visto = vec![0u8; c.tam_fichero as usize / 8 + 1];
    let mut pila = vec![0u32; 1 << 16];
    let mut usadas = pe::Extensiones::default();
    let r = p.lente(&d, &mut visto, &mut pila, &mut usadas).expect("la lente");
    assert!(!r.desborda);
    // Con `.pdata` se lee casi todo el `.text`, no solo lo de la entrada.
    assert!(r.bytes * 10 > c.tam_fichero as usize * 6, "solo {} de {} bytes", r.bytes, c.tam_fichero);
    // SSE2 es la base de x86-64 y no cuenta. Lo que SI se ve: el CRT de
    // MSVC pregunta con `xgetbv` si el sistema guarda AVX (CR4.OSXSAVE).
    assert!(usadas.iter().any(|e| e == "CR4.OSXSAVE"), "lo que usa: {:?}", usadas.iter().collect::<Vec<_>>());
    // Cada instruccion leida, en castellano; y las llamadas a Windows, con nombre.
    let codigo = c.crudos(&d);
    let (mut sin_frase, mut a_windows) = (0usize, 0usize);
    for o in 0..codigo.len() {
        if visto[o / 8] & (1 << (o % 8)) == 0 {
            continue;
        }
        let Ok(i) = lente::lee(&codigo[o..(o + lente::LARGO_MAX).min(codigo.len())]) else { continue };
        let en = p.base + c.rva as u64 + o as u64;
        if lente::castellano::explica(&i, en).as_str().contains("sin frase") {
            sin_frase += 1;
        }
        if let Some(m) = i.memoria().filter(|m| m.rip_relativa) {
            let destino = (c.rva as u64 + o as u64 + i.largo as u64).wrapping_add(m.disp as u64) as u32;
            if (i.fila.nombre == "call" || i.fila.nombre == "jmp") && p.importada_en(&d, destino).is_some() {
                a_windows += 1;
            }
        }
    }
    assert_eq!(sin_frase, 0, "instrucciones sin frase en castellano");
    assert!(a_windows > 10, "solo {a_windows} llamadas a Windows reconocidas");
}

/// ** Y TODOS los `.exe` de prueba: cada instruccion que la lente alcanza
/// tiene su frase en castellano. Lo que MSVC escribe y la frase no sabe
/// decir sale aqui con su nombre.
#[test]
fn todo_el_codigo_de_windows_tiene_su_frase() {
    let mut faltan = std::collections::BTreeMap::<String, usize>::new();
    let mut leidas = 0usize;
    for (_, d) in los_exe() {
        let Ok(p) = pe::leer(&d) else { continue };
        let Some(c) = p.codigo().copied() else { continue };
        let mut visto = vec![0u8; c.tam_fichero as usize / 8 + 1];
        let mut pila = vec![0u32; 1 << 16];
        let mut usadas = pe::Extensiones::default();
        let Some(_) = p.lente(&d, &mut visto, &mut pila, &mut usadas) else { continue };
        let codigo = c.crudos(&d);
        for o in 0..codigo.len() {
            if visto[o / 8] & (1 << (o % 8)) == 0 {
                continue;
            }
            let Ok(i) = lente::lee(&codigo[o..(o + lente::LARGO_MAX).min(codigo.len())]) else { continue };
            leidas += 1;
            if lente::castellano::explica(&i, 0).as_str().contains("sin frase") {
                *faltan.entry(i.fila.nombre.to_string()).or_default() += 1;
            }
        }
    }
    assert!(leidas > 100_000, "solo {leidas} instrucciones");
    assert!(faltan.is_empty(), "sin frase en castellano: {faltan:?}");
}

