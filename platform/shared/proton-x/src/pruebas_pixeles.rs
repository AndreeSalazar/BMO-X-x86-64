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
    pintar_con(en, ia, vertices, &mut [])
}

fn pintar_con(en: &Enlace, ia: &[ElementoIa], vertices: &[u8], otros: &mut [trama::Otro]) -> (Vec<u32>, trama::Cuenta) {
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None };
    let l = Lote { enlace: en, entradas: ia, vertices, paso: 32, ids: &[0, 1, 2, 2, 1, 3], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO };
    let mut px = vec![0u32; 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros };
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
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros: &mut [] };
    let mut vistos = Vec::new();
    trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, Some(0), |e, _| {
        vistos.push(e[0]);
        true
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

const GBUFFER_PS: &[u8] = include_bytes!("../prueba/gbuffer.dxil");

fn rgba(c: [f32; 4]) -> u32 {
    let c = c.map(trama::unorm8);
    c[3] << 24 | c[2] << 16 | c[1] << 8 | c[0]
}

/// *** N5.8: `gbuffer.hlsl` (de `dxc`) escribe SV_Target0, 1 y 3 -- como el
/// G-buffer de Cyberpunk. Cada salida va a SU render target (el 3, al
/// `otros[2]`), el 2 sin vista no recibe nada, y cada uno con su orden de
/// bytes. Antes el enlace (y el PSO) se negaban enteros.
#[test]
fn el_g_buffer_pinta_cada_salida_en_su_render_target() {
    let (en, ia) = enlace(GBUFFER_PS);
    assert_eq!(en.objetivos, [0, 1, 3]);
    let (mut sitio, mut normal) = (vec![0u32; 64], vec![0u32; 64]);
    let mut otros = [
        trama::Otro { pixeles: Some(&mut sitio), bgra: false },
        trama::Otro { pixeles: None, bgra: false },
        trama::Otro { pixeles: Some(&mut normal), bgra: true },
    ];
    let (albedo, c) = pintar_con(&en, &ia, &cuadro(0.5, 2.0), &mut otros);
    assert_eq!(c.pixeles, 64, "{c:?}");
    assert!(albedo.iter().all(|&p| p == rgba([1.0, 0.0, 0.0, 1.0])), "el 0: rojo");
    // BGRA: el azul en el byte 0.
    assert!(normal.iter().all(|&p| p == 0x8000_00FF), "el 3, en BGRA (A R G B de arriba abajo): {:#x}", normal[0]);
    for y in 0..8u32 {
        for x in 0..8u32 {
            assert_eq!(sitio[(y * 8 + x) as usize], rgba([(x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0, 0.0, 1.0]), "el 1 en ({x}, {y})");
        }
    }
}

/// Lo que el de pixeles escribe y no es un render target (SV_Depth) se dice:
/// el enlace no lo pinta a medias.
#[test]
fn una_salida_que_no_es_sv_target_se_dice() {
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(GBUFFER_PS).unwrap());
    let mut ps = ps;
    ps.salidas[2].sistema = 65;
    ps.salidas[2].semantica = "SV_Depth".into();
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde };
    let r = lote::enlazar(&vs, &ps, &[e("POSITION", 0), e("TEXCOORD", 16)]);
    assert_eq!(r.err().as_deref(), Some("el sombreador de pixeles escribe SV_Depth3 (valor de sistema 65): todavia no"));
}
