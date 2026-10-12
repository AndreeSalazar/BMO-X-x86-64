//! LI0 (12-10): el censo dice TODAS las puertas de un PSO, no la primera.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, ElementoIa, Enlace};

use crate::censo::del_pso;

const CUBO_VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");
const TEX_VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const POSICION_PS: &[u8] = include_bytes!("../../proton-x/prueba/posicion.dxil");

/// R32G32B32A32_FLOAT, R32G32B32_FLOAT y R8G8B8A8_UNORM.
const F4: u32 = 2;
const F3: u32 = 6;
const U8X4: u32 = 28;

fn e(s: &str, formato: u32, desde: u32) -> ElementoIa {
    ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None }
}

fn cubo(color: u32) -> (Enlace, Vec<ElementoIa>) {
    let ia = std::vec![e("POSITION", F3, 0), e("NORMAL", F3, 12), e("COLOR", color, 24)];
    (lote::enlazar(&dxil::leer(CUBO_VS).unwrap(), &dxil::leer(CUBO_PS).unwrap(), &ia).unwrap(), ia)
}

fn posicion(tex: u32) -> (Enlace, Vec<ElementoIa>) {
    let ia = std::vec![e("POSITION", F4, 0), e("TEXCOORD", tex, 16)];
    (lote::enlazar(&dxil::leer(TEX_VS).unwrap(), &dxil::leer(POSICION_PS).unwrap(), &ia).unwrap(), ia)
}

#[test]
fn el_cubo_pasa_todas() {
    let (en, ia) = cubo(F4);
    assert_eq!(del_pso(&en, &ia, 40), Vec::<alloc::string::String>::new());
}

#[test]
fn cada_puerta_se_cuenta_sola() {
    let (en, ia) = cubo(U8X4);
    assert_eq!(del_pso(&en, &ia, 28), ["un formato de vertice que no es float de 32 bits"]);
    let (en, ia) = posicion(F4);
    assert!(del_pso(&en, &ia, 32).iter().any(|m| m == "SV_Position en el de pixeles"));
}

/// *** La prueba del NO: `pso::traducir` para en la PRIMERA (el formato); el
/// censo dice las DOS.
#[test]
fn el_censo_dice_todas_no_la_primera() {
    let (en, ia) = posicion(U8X4);
    let m = del_pso(&en, &ia, 20);
    assert!(m.iter().any(|x| x == "SV_Position en el de pixeles"), "{m:?}");
    assert!(m.iter().any(|x| x == "un formato de vertice que no es float de 32 bits"), "{m:?}");
    let una = crate::pso::traducir(&en, &ia, 20).unwrap_err();
    assert!(alloc::format!("{una:?}").contains("formato"), "traducir dice solo la primera: {una:?}");
}
