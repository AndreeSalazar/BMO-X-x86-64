//! ** Q0a1 (12-10): la puerta de los juegos dibuja OTRAS medidas. El metal
//! del 12-10 (Cyberpunk, ya con su ventana en 1280x720): 18 de 34 lotes los
//! paraba "otra medida", y eran sus render targets PROPIOS (256x1, 256x256).
//! El driver ya dibujaba cualquier medida que quepa (`Destino::valido`, el
//! viewport y el recorte de la suya); la puerta se abre, menos con la Z (la Z
//! y la sombra en bloque miden 1280x720).

extern crate std;

use alloc::string::String;

use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, Lote, Topologia};

use crate::puerta::{medida, Blanco, Flotante, NoMedida, Puerta};

const CUADRO_VS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_vs.dxil");
const COLOR_PS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_color.dxil");

fn blanco(ancho: u32, alto: u32) -> Blanco {
    Blanco { va: 0x4000_0000, ancho, alto, bgra: true, cadena: false, flotante: None, mas: crate::puerta::SIN_MAS }
}

/// El cuadro de pantalla completa en un blanco de `b`, con o sin la Z.
fn por_la_puerta(b: Blanco, con_z: bool) -> Result<bmo_gpu_ga10x::destino::Destino, String> {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let (w, h) = (b.ancho, b.alto);
    let profundidad = con_z.then_some(bmo_proton_x::trama::Profundidad { funcion: 2, escribir: true });
    let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0], tijera: [0, 0, w as i32, h as i32], descarte: 1, antihorario: false, profundidad, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[0, 1, 2, 3, 4, 5], topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: con_z.then_some(1.0f32.to_bits()), limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, b)?;
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel lee la receta");
    Ok(r.dibujo.destino.expect("con destino").1)
}

/// *** Las tablas de Cyberpunk (256x1, 256x256), la mitad de 1280x720 y Full
/// HD: la receta las lleva con SU medida, y el kernel las lee.
#[test]
fn los_render_targets_del_juego_van_a_la_3060() {
    for (w, h) in [(256, 1), (256, 256), (640, 360), (1920, 1080), (1280, 720)] {
        let d = por_la_puerta(blanco(w, h), false).unwrap_or_else(|m| panic!("{w}x{h}: {m}"));
        assert_eq!((d.ancho, d.alto, d.fila), (w, h, 4 * w), "{w}x{h}");
    }
}

/// *** Los NO, cada uno con su porque (y el censo los agrupa por el).
#[test]
fn lo_que_la_puerta_no_dibuja_todavia() {
    let m = por_la_puerta(blanco(256, 256), true).unwrap_err();
    assert!(m.contains("con la Z"), "{m}");
    assert!(por_la_puerta(blanco(1280, 720), true).is_ok(), "1280x720 con la Z, como siempre");
    let m = por_la_puerta(blanco(100, 100), false).unwrap_err();
    assert!(m.contains("128 B"), "{m}");
    let m = por_la_puerta(Blanco { va: 0x4000_0410, ..blanco(256, 256) }, false).unwrap_err();
    assert!(m.contains("no empieza a 128 B"), "{m}");
    assert_eq!(medida(&blanco(4096, 1100), Flotante::No, false), Err(NoMedida::Grande), "17,6 MiB: mas que el mapa");
    assert_eq!(medida(&blanco(8192, 1), Flotante::No, false), Err(NoMedida::Grande), "un lado de mas");
    assert_eq!(medida(&blanco(1920, 1080), Flotante::Rgba32, false), Err(NoMedida::Grande), "1920x1080 de float: 31,6 MiB");
    assert_eq!(medida(&blanco(0, 4), Flotante::No, false), Err(NoMedida::Grande));
    assert!(medida(&blanco(256, 256), Flotante::Rgba32, false).is_ok());
}

/// ** LI2b: `pso::pegado_vacio` (en el monton, a ceros) ES `Pegado::VACIO`.
#[test]
fn el_pegado_vacio_es_ceros() {
    let v = bmo_gpu_ga10x::pegamento::Pegado::VACIO;
    let b = crate::pso::pegado_vacio();
    assert_eq!((b.n, b.registros, b.sph, b.codigo.len()), (v.n, v.registros, v.sph, v.codigo.len()));
    assert!(b.codigo.iter().zip(v.codigo.iter()).all(|(a, b)| a == b));
}

/// ** Q0a1b (12-10): el render target a mitad de pagina (el mip de 80x45 de
/// Cyberpunk en ...d400): la receta lleva SU va entera, y el kernel la lee.
#[test]
fn el_render_target_a_mitad_de_pagina_va_a_la_3060() {
    let b = Blanco { va: 0x2_0040_d400, ..blanco(96, 45) };
    let d = por_la_puerta(b, false).expect("a 128 B, pasa");
    assert_eq!((d.ancho, d.alto, d.fila), (96, 45, 384));
    assert!(d.valido_en(b.va) && d.paginas_en(b.va) == (0x400 + 384 * 45u64).div_ceil(4096));
    let lleno = Blanco { va: 0x2_0040_0f80, ..blanco(4096, 1024) };
    assert_eq!(medida(&lleno, Flotante::No, false), Err(NoMedida::Grande), "lo que se mete en la pagina cuenta en el mapa");
}
