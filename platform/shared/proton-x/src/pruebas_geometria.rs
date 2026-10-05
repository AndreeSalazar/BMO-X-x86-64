//! Las pruebas del sombreador de GEOMETRIA (E2.3b, 05-10): los tres de
//! `ParticleDraw.hlsl` de D3D12nBodyGravity (Microsoft, MIT), tal como los
//! compila su proyecto (`prueba/muestras/nbody/`): el de vertices lee la
//! particula de un StructuredBuffer por SV_VertexID, el GS hace de cada
//! PUNTO un cuadro (una tira de 4 vertices) y el de pixeles pinta un
//! degradado redondo.

use alloc::vec;
use alloc::vec::Vec;

use crate::bufer::Bufer;
use crate::dxil;
use crate::lote::{self, ElementoIa, Lote, Topologia};
use crate::trama;

const VS: &[u8] = include_bytes!("../prueba/muestras/nbody/ParticleDraw_VS.cso");
const GS: &[u8] = include_bytes!("../prueba/muestras/nbody/ParticleDraw_GS.cso");
const PS: &[u8] = include_bytes!("../prueba/muestras/nbody/ParticleDraw_PS.cso");

/// Lo que la PSV0 del GS dice de el, y el enlace de los tres.
#[test]
fn el_gs_de_nbody_es_de_puntos_a_una_tira_de_cuatro() {
    let gs = dxil::leer(GS).unwrap();
    assert_eq!(gs.etapa, dxil::Etapa::Geometria);
    assert_eq!(gs.geometria, Some(dxil::recursos::Geometria { entrada: 1, salida: 5, maximo: 4 }));
    let p = dxil::programa::compilar(&gs).unwrap();
    assert_eq!(p.ops.iter().filter(|o| matches!(o, dxil::programa::Op::Emite { flujo: 0 })).count(), 1, "un Emite, dentro del bucle");
    assert!(p.ops.iter().any(|o| matches!(o, dxil::programa::Op::Corta { flujo: 0 })));
}

/// Las tiras de un GS en triangulos: cada uno nuevo con los dos de antes,
/// los impares dados la vuelta, y una tira cortada no se une con la otra.
#[test]
fn las_tiras_del_gs_dan_sus_triangulos() {
    let mut t = dxil::Tiras { salidas: 1, maximo: 16, ..Default::default() };
    t.vertices = vec![[0.0; 4]; 7];
    t.cortes = vec![4];
    assert_eq!(t.triangulos(), [[0, 1, 2], [2, 1, 3], [4, 5, 6]]);
    t.cortar();
    t.cortar();
    assert_eq!(t.cortes, [4, 7], "cortar dos veces seguidas es una vez");
}

/// *** Dos particulas: la 0 en (-25, 0, 0) con velo.w = 0 (el rojo del de
/// vertices) y la 1 en (25, 25, 0) con velo.w = 9 (el color del vertice).
/// Con g_mWorldViewProj = 0.02 en la diagonal y g_mInvView la identidad, el
/// GS hace de cada una un cuadro de 2 * 10 * 0.02 = 0.4 de NDC: 6.4 pixeles
/// en 32x32, centrado en (8, 16) y en (24, 8). Cada pixel de dentro es su
/// color con el alfa del degradado, 2 * clamp(0.5 - |(0.5, 0.5) - uv|, 0,
/// 0.5) (sin mezcla), y los de fuera, lo que habia. El juez sale de la
/// geometria, no de la casa: el alfa con 1 de margen (la UV se interpola en
/// float), el color exacto.
#[test]
fn el_gs_de_nbody_hace_de_cada_punto_un_cuadro_con_su_degradado() {
    let ia = [ElementoIa { semantica: "COLOR".into(), indice: 0, formato: 2, ranura: 0, desde: 0, por_instancia: None }];
    let (vs, gs, ps) = (dxil::leer(VS).unwrap(), dxil::leer(GS).unwrap(), dxil::leer(PS).unwrap());
    let en = lote::enlazar_con_gs(&vs, Some(&gs), Some(&ps), &ia).unwrap();
    assert!(en.gs.is_some());
    // Los vertices: solo su color.
    let azul = [0.2f32, 0.6, 1.0, 1.0];
    let vertices: Vec<u8> = [[0.5f32, 0.5, 0.5, 1.0], azul].iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
    // Las particulas (pos, velo), lo que lee el de vertices por SV_VertexID.
    let particulas: Vec<u8> = [[-25.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], [25.0, 25.0, 0.0, 1.0, 0.0, 0.0, 0.0, 9.0]].iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
    let srv = [Some(Bufer { bytes: &particulas, formato: 0, paso: 32, elementos: 2 })];
    // cb0: la vista y proyeccion (filas 0..3) y la inversa de la vista (4..7).
    let mut cb = vec![0u8; 8 * 16];
    let mut poner = |fila: usize, col: usize, v: f32| cb[fila * 16 + col * 4..fila * 16 + col * 4 + 4].copy_from_slice(&v.to_le_bytes());
    for k in 0..3 {
        poner(k, k, 0.02);
        poner(4 + k, k, 1.0);
    }
    poner(3, 3, 1.0);
    poner(7, 3, 1.0);
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 32.0, 32.0, 0.0, 1.0], tijera: [0, 0, 32, 32], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
    let rec = crate::textura::Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let l = Lote { enlace: &en, entradas: &ia, vertices: &vertices, paso: 16, ids: &[0, 1], topologia: Topologia::Puntos, cb: &cb, reglas, limpiar_z: None, limpiar_rt: None, recursos: rec, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0 };
    let mut px = vec![0x0102_0304u32; 32 * 32];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 32, alto: 32, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None };
    let c = lote::en_cpu(&l, &mut d).unwrap();
    assert_eq!(c.recortados, 0);
    let rojo = [1.0f32, 0.1, 0.1, 1.0];
    let mut dentro = 0;
    for y in 0..32u32 {
        for x in 0..32u32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let p = px[(y * 32 + x) as usize];
            let mut esperado = None;
            for (centro, color) in [((8.0f32, 16.0f32), rojo), ((24.0, 8.0), azul)] {
                let (u, v) = ((fx - (centro.0 - 3.2)) / 6.4, (fy - (centro.1 - 3.2)) / 6.4);
                // A menos de un pixel del borde, la regla de arriba a la
                // izquierda decide: no se mira.
                if (u * 6.4).abs() < 0.05 || ((1.0 - u) * 6.4).abs() < 0.05 || (v * 6.4).abs() < 0.05 || ((1.0 - v) * 6.4).abs() < 0.05 {
                    esperado = Some(None);
                    continue;
                }
                if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                    let alfa = 2.0 * (0.5 - ((0.5 - u) * (0.5 - u) + (0.5 - v) * (0.5 - v)).sqrt()).clamp(0.0, 0.5);
                    esperado = Some(Some((color, alfa)));
                }
            }
            match esperado {
                Some(None) => {}
                None => assert_eq!(p, 0x0102_0304, "pixel ({x}, {y}): fuera de los dos cuadros, lo que habia"),
                Some(Some((color, alfa))) => {
                    dentro += 1;
                    let rgb = [color[0], color[1], color[2]].map(trama::unorm8);
                    assert_eq!(p & 0xFF_FFFF, rgb[2] << 16 | rgb[1] << 8 | rgb[0], "pixel ({x}, {y}): el color de su particula");
                    let a = (p >> 24) as i32;
                    assert!((a - trama::unorm8(alfa) as i32).abs() <= 1, "pixel ({x}, {y}): alfa {a}, y el degradado da {}", trama::unorm8(alfa));
                }
            }
        }
    }
    assert!(dentro >= 2 * 25, "dos cuadros de unos 6 x 6 pixeles: {dentro}");
}
