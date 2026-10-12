//! ** 12-10: la MEZCLA por la 3060 (el censo de Cyberpunk: 12 de 34 lotes),
//! en el sombreador con los bits de la CPU (`bmo_gpu_ga10x::mezcla`), por la
//! puerta: el cuadro de pantalla completa (dos triangulos que comparten la
//! diagonal) mezcla; lo que se pisa, MIN, la Z o la limpieza, por la CPU.

extern crate std;

use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, Lote, Topologia};
use bmo_proton_x::mezcla as m;

use crate::puerta::{Blanco, Puerta};

const CUADRO_VS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_vs.dxil");
const COLOR_PS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_color.dxil");

/// La transparencia de siempre (SRC_ALPHA, INV_SRC_ALPHA).
const TRANSPARENCIA: m::Mezcla = m::Mezcla { encendida: true, origen: m::ORIGEN_ALFA, destino: m::INV_ORIGEN_ALFA, op: m::SUMAR, origen_a: m::UNO, destino_a: m::INV_ORIGEN_ALFA, op_a: m::SUMAR, mascara: 0xF };

/// La puerta con el cuadro de `ids`, la mezcla `mezcla` (en el render target
/// 0) y, si se pide, la Z o la limpieza: la receta que lee el kernel, o el
/// porque.
fn por_la_puerta(ids: &[u32], mezcla: m::Mezcla, con_z: bool, limpiar: bool, blanco: Blanco) -> Result<bmo_gpu_ga10x::receta::Receta<'static>, String> {
    let en = std::boxed::Box::leak(std::boxed::Box::new(lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap()));
    let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let mut mezclas = m::Mezclas::NINGUNA;
    mezclas.rt[0] = mezcla;
    mezclas.factor = [0.25, 0.5, 0.75, 1.0];
    let (w, h) = (blanco.ancho, blanco.alto);
    let profundidad = con_z.then_some(bmo_proton_x::trama::Profundidad { funcion: 2, escribir: true });
    let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0], tijera: [0, 0, w as i32, h as i32], descarte: 1, antihorario: false, profundidad, mezcla: mezclas, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: en, entradas: &[], vertices: &[], paso: 0, ids, topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: con_z.then_some(1.0f32.to_bits()), limpiar_rt: limpiar.then_some(0), recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, blanco)?;
    let caja: &'static [u8] = std::boxed::Box::leak(p.caja[..n].to_vec().into_boxed_slice());
    Ok(bmo_gpu_ga10x::receta::leer(caja).expect("el kernel lee la receta"))
}

fn blanco(flotante: Option<(u32, u32)>) -> Blanco {
    Blanco { va: 0x4000_0000, ancho: 256, alto: 256, bgra: true, cadena: false, flotante }
}

/// *** El cuadro de pantalla completa con transparencia, a 8 bits y en un
/// RGBA16F: la receta lleva la mezcla, el pegado del kernel lee el destino
/// (sus IPA de la posicion y su LDG) y su juez lo aprueba.
#[test]
fn el_cuadro_con_transparencia_va_a_la_3060() {
    for flotante in [None, Some((10u32, 4u32))] {
        let r = por_la_puerta(&[0, 1, 2, 3, 4, 5], TRANSPARENCIA, false, false, blanco(flotante)).unwrap_or_else(|e| panic!("{flotante:?}: {e}"));
        assert!(r.dibujo.mezcla.encendida && r.dibujo.mezcla.origen == m::ORIGEN_ALFA);
        assert_eq!(r.dibujo.mezcla.factor, [0.25f32, 0.5, 0.75, 1.0].map(f32::to_bits));
        let mut t = alloc::boxed::Box::new(bmo_gpu_ga10x::receta::Taller::nuevo());
        bmo_gpu_ga10x::receta::pegar(&r, &mut t).unwrap_or_else(|x| panic!("{x:?}"));
        let ps: Vec<(u64, u64)> = t.ps[4 * bmo_gpu_ga10x::raster::SPH..t.bytes_ps].chunks(16).map(|w| (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap()))).collect();
        let cuantas = |op: u64| ps.iter().filter(|w| w.0 & 0xFFF == op).count();
        assert!(cuantas(0x981) >= 1, "el LDG del destino");
        assert!(cuantas(0x326) >= 2, "las IPA de la posicion");
    }
}

/// *** Lo que va por la CPU, cada uno con su porque.
#[test]
fn lo_que_no_mezcla_la_3060() {
    let no = |ids: &[u32], mezcla, z, limpiar| por_la_puerta(ids, mezcla, z, limpiar, blanco(None)).map(|_| ()).unwrap_err();
    assert!(no(&[0, 1, 2, 0, 1, 2], TRANSPARENCIA, false, false).contains("se pisan"), "el mismo triangulo dos veces");
    assert!(no(&[0, 1, 2, 3, 4, 5], m::Mezcla { op: m::MIN, ..TRANSPARENCIA }, false, false).contains("MIN o MAX"));
    assert!(no(&[0, 1, 2, 3, 4, 5], m::Mezcla { origen: m::ORIGEN_ALFA_SAT, ..TRANSPARENCIA }, false, false).contains("SRC_ALPHA_SAT"));
    assert!(no(&[0, 1, 2, 3, 4, 5], TRANSPARENCIA, true, false).contains("con la Z"));
    assert!(no(&[0, 1, 2, 3, 4, 5], TRANSPARENCIA, false, true).contains("limpieza"));
    // Sin mezcla, el mismo triangulo dos veces va como siempre.
    assert!(por_la_puerta(&[0, 1, 2, 0, 1, 2], m::Mezcla::NINGUNA, false, false, blanco(None)).is_ok());
    // Y solo la mascara (sin mezcla): tambien lee el destino, y va.
    let r = por_la_puerta(&[0, 1, 2], m::Mezcla { mascara: 0x7, ..m::Mezcla::NINGUNA }, false, false, blanco(None)).unwrap();
    assert_eq!(r.dibujo.mezcla.mascara, 0x7);
}

/// *** 12-10: la TIRA de triangulos va como LISTA, con los triangulos de la
/// CPU (`lote::triangulos`: el impar se da la vuelta). El cuadro en tira
/// (0, 1, 2, 5: dos triangulos que comparten la diagonal 1-2) con
/// transparencia: no se pisa, y va.
#[test]
fn la_tira_va_como_lista() {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let mut mezclas = m::Mezclas::NINGUNA;
    mezclas.rt[0] = TRANSPARENCIA;
    let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, 256.0, 256.0, 0.0, 1.0], tijera: [0, 0, 256, 256], descarte: 1, antihorario: false, profundidad: None, mezcla: mezclas, z_del_sombreador: false, stencil: None };
    let ids = [0u32, 1, 2, 5];
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &ids, topologia: Topologia::Tira, cb: &cb, reglas, limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, blanco(None)).unwrap_or_else(|e| panic!("{e}"));
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel lee la receta");
    let desde = r.dibujo.indices.expect("con indices") as usize;
    let leidos: Vec<u32> = (0..r.n).map(|i| u32::from_le_bytes(r.datos[desde + 4 * i..desde + 4 * i + 4].try_into().unwrap())).collect();
    assert_eq!(leidos, [0, 1, 2, 2, 1, 5], "el segundo, dado la vuelta: el de la CPU");
    assert_eq!(leidos.chunks(3).map(|t| [t[0], t[1], t[2]]).collect::<Vec<_>>(), lote::triangulos(&ids, Topologia::Tira));
    // Puntos o lineas, no.
    let lineas = Lote { topologia: Topologia::Lineas, ..l };
    assert!(Puerta::nueva().preparar(&lineas, blanco(None)).unwrap_err().contains("triangulos"));
}
