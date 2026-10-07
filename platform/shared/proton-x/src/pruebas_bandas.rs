//! Banco de `bandas` (H4.3): el mismo dibujo, entero y en franjas una
//! detras de otra (`en_orden`), da los MISMOS bytes. Repartidas en hilos de
//! verdad y con franjas que fallan: el banco de la casa (`tests/bandas.rs`).

use alloc::vec;
use alloc::vec::Vec;

use crate::bandas;
use crate::dxil;
use crate::lote::{self, ElementoIa, Lote, Topologia};
use crate::textura::Recursos;
use crate::trama;

const CUBO_VS: &[u8] = include_bytes!("../prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../prueba/cubo_ps.dxil");
const OLAS_VS: &[u8] = include_bytes!("../prueba/olas_vs.dxil");
const OLAS_PS: &[u8] = include_bytes!("../prueba/olas_ps.dxil");

/// El cubo del fotograma `f` con profundidad, sus limpiezas apuntadas (las
/// hace quien dibuja), en el destino de la referencia: entero, o en `n`
/// franjas en orden.
pub(crate) fn cubo(f: u32, n: Option<u32>) -> (Vec<u32>, Vec<u32>, trama::Cuenta) {
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let e = |s: &str, formato, desde| ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None };
    let entradas = [e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)];
    let enlace = lote::enlazar(&vs, &ps, &entradas).unwrap();
    let vertices: Vec<u8> = bmo_cubo::vertices().iter().flat_map(|v| v.pos.iter().chain(&v.normal).chain(&v.color).flat_map(|x| x.to_le_bytes())).collect();
    let ids: Vec<u32> = bmo_cubo::indices().iter().map(|&i| i as u32).collect();
    let b0 = crate::pruebas::cb_de(f);
    let cb = lote::juntar_constantes(&enlace.constantes, |_| Some(&b0[..]));
    let (w, h) = (bmo_cubo::referencia::ANCHO, bmo_cubo::referencia::ALTO);
    let reglas = trama::Reglas { viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0], tijera: [0, 0, w as i32, h as i32], descarte: 3, antihorario: false, profundidad: Some(trama::Profundidad { funcion: 2, escribir: true }), mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: &enlace, entradas: &entradas, vertices: &vertices, paso: 40, ids: &ids, topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: Some(1.0f32.to_bits()), limpiar_rt: Some(bmo_cubo::FONDO), recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    // Basura en los dos: lo que no limpie quien dibuja se veria.
    let mut px = vec![0xDEAD_BEEF; (w * h) as usize];
    let mut zs = vec![7u32; (w * h) as usize];
    let mut d = trama::Destino { pixeles: &mut px, ancho: w, alto: h, bgra: true, z: Some(&mut zs[..]), cadena: false, otros: &mut [], flotante: None, stencil: None };
    let cuenta = match n {
        None => lote::en_cpu(&l, &mut d).unwrap(),
        Some(n) => {
            assert!(bandas::se_parte(&l, &d, n, false));
            bandas::en_orden(&l, &mut d, n, &lote::en_cpu).unwrap()
        }
    };
    (px, zs, cuenta)
}

/// *** EL CUBO EN FRANJAS ES EL CUBO: los mismos pixeles, la misma
/// profundidad, los mismos triangulos y la misma cuenta de pixeles, con 2,
/// 3, 5, 12 y 64 franjas.
#[test]
fn el_cubo_en_franjas_da_los_mismos_bytes() {
    for f in [0u32, 30, 60] {
        let (px, zs, c) = cubo(f, None);
        assert_eq!(bmo_cubo::referencia::huella(&px), bmo_cubo::referencia::HUELLAS.iter().find(|h| h.0 == f).unwrap().1, "el entero da la huella de D3D12");
        for n in [2u32, 3, 5, 12, 64] {
            let (bpx, bzs, bc) = cubo(f, Some(n));
            assert!(bpx == px, "fotograma {f}, {n} franjas: los pixeles");
            assert!(bzs == zs, "fotograma {f}, {n} franjas: la profundidad");
            assert_eq!((bc.dibujados, bc.descartados, bc.recortados), (c.dibujados, c.descartados, c.recortados));
            assert_eq!((bc.pixeles, bc.tapados, bc.pasan), (c.pixeles, c.tapados, c.pasan));
        }
    }
}

/// Las olas: los cuadros de 2x2 no se parten (filas pares: las derivadas y
/// las pruebas de cuadro dan lo mismo), pero CUANTOS van en cada ola si
/// cambia -- por eso un de pixeles con olas propias no se parte.
#[test]
fn los_cuadros_no_se_parten_y_las_olas_propias_no_se_reparten() {
    let (vs, ps) = (dxil::leer(OLAS_VS).unwrap(), dxil::leer(OLAS_PS).unwrap());
    let en = lote::enlazar(&vs, &ps, &[]).unwrap();
    assert!(en.ps.usa_olas() && en.ps.olas_propias());
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 64.0, 64.0, 0.0, 1.0], tijera: [0, 0, 64, 64], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let ids = [0u32, 1, 2, 3, 4, 5];
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &ids, topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: Some(0), recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let pinta = |n: Option<u32>| {
        let mut px = vec![0x1234_5678u32; 64 * 64];
        let mut d = trama::Destino { pixeles: &mut px, ancho: 64, alto: 64, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
        let c = match n {
            None => lote::en_cpu(&l, &mut d).unwrap(),
            Some(n) => {
                assert!(!bandas::se_parte(&l, &d, n, false), "olas propias: no se reparte");
                // Forzado, para ver lo que cambiaria.
                bandas::en_orden(&l, &mut d, n, &lote::en_cpu).unwrap()
            }
        };
        (px, c.pixeles)
    };
    let entero = pinta(None);
    assert!(entero.1 > 0);
    for n in [2u32, 3, 4] {
        let (px, pixeles) = pinta(Some(n));
        assert_eq!(pixeles, entero.1);
        // R = x ^ 1 y G = y ^ 1 (el vecino de cuadro) y B (las pruebas de
        // cuadro): iguales. A (los activos de la ola): no siempre.
        assert!(px.iter().zip(&entero.0).all(|(a, b)| a & 0x00FF_FFFF == b & 0x00FF_FFFF), "{n} franjas: los cuadros enteros");
    }
}

/// Las filas de las franjas: pares, seguidas, sin huecos, hasta abajo.
#[test]
fn las_franjas_cubren_el_destino_en_filas_pares() {
    for alto in [32u32, 33, 64, 100, 720, 1080] {
        for n in 1..=12 {
            let mut y = 0;
            for k in 0..n {
                let (y0, y1) = bandas::filas(k, n, alto);
                assert_eq!(y0, y, "alto {alto}, {n} franjas, la {k} empieza donde acabo la anterior");
                assert_eq!(y0 % 2, 0);
                assert!(y1 >= y0);
                y = y1;
            }
            assert_eq!(y, alto, "alto {alto}, {n} franjas: hasta abajo");
        }
    }
}

/// Lo que no se parte: con UAV, con dinamicas sin garantia, un destino
/// bajo, o una sola parte.
#[test]
fn lo_que_no_se_parte() {
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let e = |s: &str, formato, desde| ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None };
    let en = lote::enlazar(&vs, &ps, &[e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)]).unwrap();
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 64.0, 64.0, 0.0, 1.0], tijera: [0, 0, 64, 64], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let uavs: lote::Uavs = Default::default();
    let busca = |_: u8, _: u32| None;
    let mut l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut px = vec![0u32; 64 * 64];
    let d = trama::Destino { pixeles: &mut px, ancho: 64, alto: 64, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    assert!(bandas::se_parte(&l, &d, 4, false));
    assert!(!bandas::se_parte(&l, &d, 1, false), "una parte");
    l.uavs = Some(&uavs);
    assert!(!bandas::se_parte(&l, &d, 4, true), "con UAV");
    l.uavs = None;
    l.recursos.dinamicas = Some(crate::textura::Dinamicas(&busca, None));
    assert!(!bandas::se_parte(&l, &d, 4, false), "dinamicas sin garantia");
    assert!(bandas::se_parte(&l, &d, 4, true), "dinamicas que aguantan");
    let mut bajo = vec![0u32; 64 * 16];
    let d = trama::Destino { pixeles: &mut bajo, ancho: 64, alto: 16, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    assert!(!bandas::se_parte(&l, &d, 4, true), "16 filas");
}

/// Cuantas partes le compensan a un dibujo: las que llena su rectangulo de
/// verdad. La pantalla entera, todas; un cuadro chico o una tijera
/// estrecha, una (no se reparte).
#[test]
fn las_partes_dependen_de_lo_que_pinta_el_dibujo() {
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let e = |s: &str, formato, desde| ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None };
    let en = lote::enlazar(&vs, &ps, &[e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)]).unwrap();
    let lote_con = |viewport: [f32; 6], tijera: [i32; 4]| {
        let reglas = trama::Reglas { viewport, tijera, descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
        Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None }
    };
    let mut px = vec![0u32; 1280 * 720];
    let d = trama::Destino { pixeles: &mut px, ancho: 1280, alto: 720, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    let todo = lote_con([0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], [0, 0, 1280, 720]);
    assert_eq!(bandas::partes_utiles(&todo, &d, 12), 12, "1280x720 son 57 cuadros: los 12 nucleos");
    assert_eq!(bandas::partes_utiles(&todo, &d, 6), 6, "y con 6, 6");
    let chico = lote_con([100.0, 100.0, 64.0, 64.0, 0.0, 1.0], [0, 0, 1280, 720]);
    assert_eq!(bandas::partes_utiles(&chico, &d, 12), 1, "64x64: no se reparte");
    let tijera = lote_con([0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], [0, 0, 1280, 40]);
    assert_eq!(bandas::partes_utiles(&tijera, &d, 12), 4, "una tijera de 1280x40: 51200 pixeles, 4 partes");
    let medio = lote_con([0.0, 0.0, 640.0, 360.0, 0.0, 1.0], [0, 0, 1280, 720]);
    assert_eq!(bandas::partes_utiles(&medio, &d, 12), 12, "640x360: 15 cuadros, los 12");
}
