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
/// R11G11B10_FLOAT: lo que la 3060 aun no lee (LI2e).
const R11G11B10: u32 = 26;

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
    // LI2e (12-10): el RGBA8 ya lo lee la 3060; el R11G11B10_FLOAT, no.
    let (en, ia) = cubo(U8X4);
    assert_eq!(del_pso(&en, &ia, 28), Vec::<alloc::string::String>::new());
    let (en, ia) = cubo(R11G11B10);
    assert_eq!(del_pso(&en, &ia, 28), ["un formato de vertice que la 3060 no lee todavia (R11G11B10_FLOAT)"]);
    let (en, ia) = posicion(F4);
    assert!(del_pso(&en, &ia, 32).iter().any(|m| m == "SV_Position.z o .w en el de pixeles"));
}

/// *** La prueba del NO: `pso::traducir` para en la PRIMERA (el formato); el
/// censo dice las DOS.
#[test]
fn el_censo_dice_todas_no_la_primera() {
    let (en, ia) = posicion(R11G11B10);
    let m = del_pso(&en, &ia, 20);
    assert!(m.iter().any(|x| x == "SV_Position.z o .w en el de pixeles"), "{m:?}");
    assert!(m.iter().any(|x| x == "un formato de vertice que la 3060 no lee todavia (R11G11B10_FLOAT)"), "{m:?}");
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
    // LI2b (12-10): el hueco es de 4096; lo largo empieza en 4097.
    assert_eq!(tramo_largo(4097), "de 4097 a 8192 instrucciones");
    assert_eq!(tramo_largo(8192), "de 4097 a 8192 instrucciones");
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

/// *** LI2e (12-10), de punta a punta: el cubo con su NORMAL en
/// R16G16B16A16_SNORM o half y su COLOR en RGBA8 o BGRA8 -- la puerta, la
/// receta (el formato en los bits 16..24 de `componentes`), el pegado del
/// kernel con su juez, y el vigia: el de vertice en el simulador de la 3060
/// con lo que el pegamento lee, contra la CPU, bit a bit.
#[test]
fn los_formatos_de_vertice_van_a_la_3060() {
    use crate::puerta::{Blanco, Puerta};
    const SNORM16X4: u32 = 13;
    const HALF4: u32 = 10;
    const BGRA8: u32 = 87;
    for (normal, color) in [(SNORM16X4, U8X4), (HALF4, BGRA8)] {
        let ia = std::vec![e("POSITION", F3, 0), e("NORMAL", normal, 12), e("COLOR", color, 20)];
        let en = lote::enlazar(&dxil::leer(CUBO_VS).unwrap(), &dxil::leer(CUBO_PS).unwrap(), &ia).unwrap();
        let paso = 24usize;
        let mut vertices = std::vec![0u8; 3 * paso];
        for v in 0..3 {
            let b = &mut vertices[v * paso..(v + 1) * paso];
            for (k, x) in [0.25f32 * v as f32 - 0.5, 0.5 - 0.25 * v as f32, 0.5].iter().enumerate() {
                b[4 * k..4 * k + 4].copy_from_slice(&x.to_le_bytes());
            }
            for k in 12..24 {
                b[k] = (v * 37 + k * 11) as u8 ^ 0x5A;
            }
        }
        let cb = std::vec![0x3Fu8; 16 * en.vs.filas_cb.max(en.ps.filas_cb).max(1) as usize];
        let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], tijera: [0, 0, 1280, 720], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
        let l = lote::Lote { enlace: &en, entradas: &ia, vertices: &vertices, paso, ids: &[0, 1, 2], topologia: lote::Topologia::Lista, cb: &cb, reglas, limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
        let mut p = Puerta::nueva();
        let blanco = Blanco { va: 0x4000_0000, ancho: 1280, alto: 720, bgra: true, cadena: false, flotante: None, mas: crate::puerta::SIN_MAS };
        let n = p.preparar(&l, blanco).unwrap_or_else(|m| panic!("normal {normal}, color {color}: {m}"));
        let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel lee la receta");
        let formatos: Vec<u8> = r.elementos().iter().map(|x| x.formato).collect();
        assert!(formatos.contains(&(normal as u8)) && formatos.contains(&(color as u8)), "{formatos:?}");
        assert_eq!(p.revisados, 1, "el vigia lo reviso y cuadro con la CPU");
        // El pegado del kernel (el mismo `pegar`), con su juez: lleva las
        // conversiones -- la magia de SNORM o el HADD2 del half --.
        let mut t = alloc::boxed::Box::new(bmo_gpu_ga10x::receta::Taller::nuevo());
        bmo_gpu_ga10x::receta::pegar(&r, &mut t).unwrap_or_else(|x| panic!("{x:?}"));
        let vs: Vec<(u64, u64)> = t.vs[4 * bmo_gpu_ga10x::raster::SPH..t.bytes_vs].chunks(16).map(|w| (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap()))).collect();
        let magia = |m: u64| vs.iter().any(|w| w.0 & 0xFFF == 0x810 && w.0 >> 32 == m);
        if normal == SNORM16X4 {
            assert!(magia(0x4B40_0000) && magia(0x4B00_0000), "la magia de SNORM y de UNORM");
        } else {
            assert!(vs.iter().any(|w| w.0 & 0xFFF == 0x230), "el HADD2.F32 del half");
        }
    }
}
