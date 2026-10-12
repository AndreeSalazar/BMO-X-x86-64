//! LI2e (12-10): SV_VertexID y SV_InstanceID, de la 3060. El censo de
//! Cyberpunk: 16 lotes de 34 los paraba "un valor de sistema en el de
//! vertice". Lo que valen lo dice la CPU (`lote::entrada`); la 3060 los saca
//! del `vid` que le da la puerta.

extern crate std;

use alloc::vec::Vec;

use bmo_gpu_ga10x::pegamento::Sistema;
use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, Lote, Topologia};

use crate::puerta::{Blanco, Puerta};

/// Un cuadro de pantalla completa SIN bufer de vertices (como los de
/// Cyberpunk): `VSCuadro(uint id : SV_VertexID)` y el color de la raiz.
const CUADRO_VS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_vs.dxil");
const COLOR_PS: &[u8] = include_bytes!("../../proton-x/prueba/flotante1_color.dxil");

fn reglas() -> bmo_proton_x::trama::Reglas {
    bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], tijera: [0, 0, 1280, 720], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None }
}

const BLANCO: Blanco = Blanco { va: 0x4000_0000, ancho: 1280, alto: 720, bgra: false, cadena: false };

/// La puerta con el cuadro de `ids` y vertice `base`: la receta, sus
/// indices en los DATOS, y si el vigia lo reviso contra la CPU.
fn por_la_puerta(ids: &[u32], base: u32) -> (Vec<u32>, Sistema, u32, usize) {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids, topologia: Topologia::Lista, cb: &cb, reglas: reglas(), limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: base, uavs: None };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, BLANCO).unwrap_or_else(|m| panic!("el cuadro de SV_VertexID va a la 3060: {m}"));
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel lee la receta");
    let desde = r.dibujo.indices.expect("con indices") as usize;
    let indices = (0..ids.len()).map(|i| u32::from_le_bytes(r.datos[desde + 4 * i..desde + 4 * i + 4].try_into().unwrap())).collect();
    (indices, r.elementos()[0].sistema, r.paso, p.revisados)
}

/// *** El cuadro va a la 3060: la receta lleva el elemento de SISTEMA, paso
/// 0 y los indices tal cual; el pegado de prueba (el del kernel) y su juez
/// pasan, y el vigia lo revisa contra la CPU, bit a bit.
#[test]
fn el_cuadro_de_sv_vertexid_va_a_la_3060() {
    let (indices, sistema, paso, revisados) = por_la_puerta(&[0, 1, 2, 3, 4, 5], 0);
    assert_eq!(indices, [0, 1, 2, 3, 4, 5]);
    assert_eq!((sistema, paso), (Sistema::Vertice, 0));
    assert_eq!(revisados, 1, "el vigia lo reviso y cuadro con la CPU");
}

/// *** Con vertice BASE: la CPU dice que SV_VertexID es el id MENOS el base
/// (`lote::entrada`). La puerta escribe los indices sin el base, y el vigia
/// -- el simulador de la 3060 con ese `vid` contra la CPU -- cuadra.
#[test]
fn con_vertice_base_el_vid_es_el_de_la_cpu() {
    let (indices, _, _, revisados) = por_la_puerta(&[3, 4, 5, 6, 7, 8], 3);
    assert_eq!(indices, [0, 1, 2, 3, 4, 5], "sin el vertice base");
    assert_eq!(revisados, 1);
    // Y lo de la CPU, para que no quede en el aire.
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[], topologia: Topologia::Lista, cb: &[], reglas: reglas(), limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 3, uavs: None };
    assert_eq!(lote::entrada(&l, en.desde_ia[0], 5, 0)[0].to_bits(), 2);
}

/// *** La prueba del NO: un indice por debajo del vertice base (un
/// BaseVertexLocation negativo) no se reescribe a ciegas: va por la CPU.
#[test]
fn un_base_negativo_va_por_la_cpu() {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let cb = std::vec![0u8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[0, 1, 2], topologia: Topologia::Lista, cb: &cb, reglas: reglas(), limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 2, uavs: None };
    let m = Puerta::nueva().preparar(&l, BLANCO).unwrap_err();
    assert!(m.contains("vertice base"), "{m}");
}

/// El censo ya no lo cuenta como puerta cerrada.
#[test]
fn el_censo_deja_pasar_el_valor_de_sistema() {
    let en = lote::enlazar(&dxil::leer(CUADRO_VS).unwrap(), &dxil::leer(COLOR_PS).unwrap(), &[]).unwrap();
    let m = crate::censo::del_pso(&en, &[], 0);
    assert!(!m.iter().any(|x| x.contains("valor de sistema")), "{m:?}");
}
