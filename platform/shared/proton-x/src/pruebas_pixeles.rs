//! Las pruebas de lo que el sombreador de pixeles LEE y ESCRIBE mas alla de
//! un color (03-10): SV_Position (N5.9) y varios render targets (N5.8). Un
//! cuadro de pantalla completa de 8x8 con el de vertices de `textura.hlsl`.

use alloc::vec;
use alloc::vec::Vec;

use crate::dxil;
use crate::lote::{self, ElementoIa, Enlace, Lote, Topologia};
use crate::pruebas::TEXTURA_VS;
use crate::trama;

const POSICION_PS: &[u8] = include_bytes!("../prueba/posicion.dxil");

/// El cuadro: (-1, 1) (1, 1) (-1, -1) (1, -1) en NDC, con `z` y `w` en
/// recorte (x e y por w), en dos triangulos horarios.
fn cuadro(z: f32, w: f32) -> Vec<u8> {
    let mut v = Vec::new();
    for (x, y) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        for c in [x * w, y * w, z * w, w, 0.0, 0.0, 0.0, 0.0] {
            v.extend_from_slice(&c.to_le_bytes());
        }
    }
    v
}

fn enlace(ps: &[u8]) -> (Enlace, [ElementoIa; 2]) {
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde };
    let ia = [e("POSITION", 0), e("TEXCOORD", 16)];
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(ps).unwrap());
    (lote::enlazar(&vs, &ps, &ia).unwrap(), ia)
}

fn pintar(en: &Enlace, ia: &[ElementoIa], vertices: &[u8]) -> (Vec<u32>, trama::Cuenta) {
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None };
    let l = Lote { enlace: en, entradas: ia, vertices, paso: 32, ids: &[0, 1, 2, 2, 1, 3], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO };
    let mut px = vec![0u32; 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false };
    let c = lote::en_cpu(&l, &mut d).unwrap();
    (px, c)
}

/// *** N5.9: `posicion.hlsl` (de `dxc`) lee SV_Position: cada pixel ve SU
/// centro en pantalla (x + 0.5, y + 0.5), la z del viewport y la w de
/// recorte. Antes el enlace se negaba entero.
#[test]
fn el_de_pixeles_lee_su_sv_position() {
    let (en, ia) = enlace(POSICION_PS);
    assert_eq!(en.pos_ps, Some(0));
    assert_eq!(en.desde_vs, [None]);
    let (px, c) = pintar(&en, &ia, &cuadro(0.5, 2.0));
    assert_eq!(c.pixeles, 64, "{c:?}");
    assert_eq!(c.sombreados, 64, "con SV_Position cada pixel es distinto: {c:?}");
    for y in 0..8u32 {
        for x in 0..8u32 {
            let rgba = [(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.5, 0.5].map(trama::unorm8);
            let esperado = rgba[3] << 24 | rgba[2] << 16 | rgba[1] << 8 | rgba[0];
            assert_eq!(px[(y * 8 + x) as usize], esperado, "pixel ({x}, {y})");
        }
    }
}

/// La w de SV_Position es la de RECORTE, con perspectiva: con w distinta por
/// vertice, en el centro de la pantalla no es la media de las w sino la
/// inversa de la media de las 1/w (y la z sigue lineal en pantalla).
#[test]
fn la_w_de_sv_position_va_con_perspectiva() {
    let v = vec![
        trama::Sombreado { pos: [-1.0, 1.0, 0.0, 1.0], atributos: vec![[0.0; 4]] },
        trama::Sombreado { pos: [3.0 * 4.0, 1.0 * 4.0, 0.0, 4.0], atributos: vec![[0.0; 4]] },
        trama::Sombreado { pos: [-1.0 * 4.0, -3.0 * 4.0, 0.0, 4.0], atributos: vec![[0.0; 4]] },
    ];
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None };
    let mut px = vec![0u32; 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false };
    let mut vistos = Vec::new();
    trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, Some(0), |e| {
        vistos.push(e[0]);
        Some([0.0; 4])
    });
    assert_eq!(vistos.len(), 64, "el triangulo cubre la pantalla");
    let p = vistos.iter().find(|p| p[0] == 0.5 && p[1] == 0.5).unwrap();
    // En (0.5, 0.5) los pesos de pantalla son (15/16, 1/32, 1/32): casi
    // el vertice de w = 1.
    assert!(p[3] > 1.0 && p[3] < 1.2, "{p:?}");
    let ultimo = vistos.iter().find(|p| p[0] == 7.5 && p[1] == 7.5).unwrap();
    let pesos = |x: f32, y: f32| {
        // El vertice 0 esta en (0, 0); el 1 en (16, 0); el 2 en (0, 16).
        let (b1, b2) = (x / 16.0, y / 16.0);
        1.0 / ((1.0 - b1 - b2) / 1.0 + b1 / 4.0 + b2 / 4.0)
    };
    assert!((ultimo[3] - pesos(7.5, 7.5)).abs() < 1e-5, "{ultimo:?} contra {}", pesos(7.5, 7.5));
}
