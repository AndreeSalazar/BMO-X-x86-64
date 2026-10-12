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
    assert!(del_pso(&en, &ia, 32).iter().any(|m| m == "SV_Position.z o .w en el de pixeles"));
}

/// *** La prueba del NO: `pso::traducir` para en la PRIMERA (el formato); el
/// censo dice las DOS.
#[test]
fn el_censo_dice_todas_no_la_primera() {
    let (en, ia) = posicion(U8X4);
    let m = del_pso(&en, &ia, 20);
    assert!(m.iter().any(|x| x == "SV_Position.z o .w en el de pixeles"), "{m:?}");
    assert!(m.iter().any(|x| x == "un formato de vertice que no es float de 32 bits"), "{m:?}");
    let una = crate::pso::traducir(&en, &ia, 20).unwrap_err();
    assert!(alloc::format!("{una:?}").contains("formato"), "traducir dice solo la primera: {una:?}");
}

/// ** LI2a (12-10): `posicion.dxil` lee x, y, z y w: sigue por la CPU, y
/// ahora dice POR QUE (la z y la w), no "SV_Position" entero.
#[test]
fn la_z_y_la_w_siguen_por_la_cpu() {
    let (en, ia) = posicion(F4);
    assert!(crate::pso::posicion_zw(&en));
    let m = del_pso(&en, &ia, 32);
    assert!(m.iter().any(|x| x == "SV_Position.z o .w en el de pixeles"), "{m:?}");
    assert!(!m.iter().any(|x| x == "SV_Position en el de pixeles"), "{m:?}");
}

/// *** LI2a: el MISMO sombreador leyendo solo x e y (sus lecturas de z y w
/// cambiadas por x e y) VA a la 3060: el de pixeles pegado lleva el IPA de
/// `a[0x70]` y de `a[0x74]` y los bits `ImapPositionX/Y` (188, 189) de su
/// SPH, y el juez lo da por bueno (`traducir` juzga los dos programas).
#[test]
fn la_x_y_la_y_van_a_la_3060() {
    use bmo_gpu_ga10x::pegamento::{ATRIBUTO_POSICION, BIT_IMAP_POSICION_X};
    use bmo_proton_x::dxil::programa::Op;
    let (mut en, ia) = posicion(F4);
    let p = en.pos_ps.unwrap();
    for o in en.ps.ops.iter_mut() {
        if let Op::Entrada { elemento, componente, .. } = o {
            if *elemento as usize == p && *componente >= 2 {
                *componente -= 2;
            }
        }
    }
    assert!(!crate::pso::posicion_zw(&en));
    assert_eq!(del_pso(&en, &ia, 32), Vec::<alloc::string::String>::new());
    let t = crate::pso::traducir(&en, &ia, 32).expect("va a la 3060");
    let palabra = |k: usize| u32::from_le_bytes(t.ps[4 * k..4 * k + 4].try_into().unwrap());
    for c in 0..2u32 {
        let b = BIT_IMAP_POSICION_X + c;
        assert!(palabra(b as usize / 32) & 1 << (b % 32) != 0, "falta el bit {b} en la SPH");
    }
    let sph = 4 * bmo_gpu_ga10x::raster::SPH;
    let ipas: Vec<u64> = t.ps[sph..]
        .chunks(16)
        .map(|w| (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap())))
        .filter(|(lo, _)| lo & 0xFFF == 0x326)
        .map(|(_, hi)| hi & 0xFF)
        .collect();
    assert!(ipas.contains(&ATRIBUTO_POSICION) && ipas.contains(&(ATRIBUTO_POSICION + 1)), "IPA de la posicion: {ipas:x?}");
}

#[test]
fn el_programa_largo_dice_cuanto_mide() {
    use crate::censo::tramo_largo;
    assert_eq!(tramo_largo(129), "de 129 a 256 instrucciones");
    assert_eq!(tramo_largo(256), "de 129 a 256 instrucciones");
    assert_eq!(tramo_largo(257), "de 257 a 512 instrucciones");
    assert_eq!(tramo_largo(2000), "de 1025 a 2048 instrucciones");
    assert_eq!(tramo_largo(100_000), "mas de 8192 instrucciones");
}

#[test]
fn el_simulador_dice_que_palabra_no_sabe() {
    use crate::simula::{decir, NoSimula};
    let codigo = [(0u64, 0u64), (0x0000_0000_0000_0b5e, 0x0000_0000_0000_7000)];
    let s = decir(&codigo, NoSimula::Instruccion(1));
    assert!(s.contains("la instruccion 1 de 2") && s.contains("op 0x15e") && s.contains("forma 5"), "{s}");
    assert!(decir(&codigo, NoSimula::Instruccion(9)).contains("fuera del programa"));
    assert_eq!(decir(&codigo, NoSimula::SinExit), "SinExit");
}

/// Metal 12-10: casi ningun lote de Cyberpunk lo para UNA puerta. El censo
/// dice tambien las combinaciones, por el numero de cada puerta.
#[test]
fn el_censo_dice_las_puertas_juntas() {
    use alloc::string::String;
    let mut c = crate::censo::Censo::nuevo();
    let p = |v: &[&str]| v.iter().map(|s| String::from(*s)).collect::<Vec<_>>();
    for _ in 0..3 {
        c.contar(p(&["hdr", "largo"]));
    }
    c.contar(p(&["largo", "hdr"]));
    c.contar(p(&["largo"]));
    c.contar(p(&[]));
    let j = c.juntas_que_mas(5);
    assert_eq!(j[0].0, 4, "el orden no cuenta: {j:?}");
    assert_eq!(j[0].1, &p(&["hdr", "largo"]));
    assert_eq!((j[1].0, j[1].1.len()), (1, 1));
    let r = c.resumen(10);
    // `largo` para 5 lotes ([1]) y `hdr` 4 ([2]): la combinacion, por numero.
    assert!(r.contains("juntas: 4 lotes se abren con [1] + [2]"), "{r}");
    assert!(r.contains("de 6 lotes, 1 irian"), "{r}");
}
