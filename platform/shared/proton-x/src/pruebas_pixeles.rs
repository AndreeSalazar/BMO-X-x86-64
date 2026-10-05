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
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde, por_instancia: None };
    let ia = [e("POSITION", 0), e("TEXCOORD", 16)];
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(ps).unwrap());
    (lote::enlazar(&vs, &ps, &ia).unwrap(), ia)
}

fn pintar(en: &Enlace, ia: &[ElementoIa], vertices: &[u8]) -> (Vec<u32>, trama::Cuenta) {
    pintar_con(en, ia, vertices, &mut [])
}

fn pintar_con(en: &Enlace, ia: &[ElementoIa], vertices: &[u8], otros: &mut [trama::Otro]) -> (Vec<u32>, trama::Cuenta) {
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
    let l = Lote { enlace: en, entradas: ia, vertices, paso: 32, ids: &[0, 1, 2, 2, 1, 3], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0 };
    let mut px = vec![0u32; 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros, flotante: None };
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
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
    let mut px = vec![0u32; 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None };
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
        trama::Otro { pixeles: Some(&mut sitio), bgra: false, flotante: None },
        trama::Otro { pixeles: None, bgra: false, flotante: None },
        trama::Otro { pixeles: Some(&mut normal), bgra: true, flotante: None },
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

/// Lo que el de pixeles escribe y no es un render target ni su profundidad
/// (SV_Coverage) se dice: el enlace no lo pinta a medias.
#[test]
fn una_salida_que_no_es_sv_target_se_dice() {
    let (vs, ps) = (dxil::leer(TEXTURA_VS).unwrap(), dxil::leer(GBUFFER_PS).unwrap());
    let mut ps = ps;
    ps.salidas[2].sistema = 66;
    ps.salidas[2].semantica = "SV_Coverage".into();
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde, por_instancia: None };
    let r = lote::enlazar(&vs, &ps, &[e("POSITION", 0), e("TEXCOORD", 16)]);
    assert_eq!(r.err().as_deref(), Some("el sombreador de pixeles escribe SV_Coverage3 (valor de sistema 66): todavia no"));
}

/// *** N5.11: la transparencia en la trama: un triangulo de alfa 0.5 sobre
/// un fondo azul (SRC_ALPHA, INV_SRC_ALPHA) deja la media, solo donde
/// cubre; con la mascara sin el azul, el azul de antes se queda.
#[test]
fn la_trama_mezcla_con_lo_que_ya_esta() {
    use crate::mezcla::{Mezcla, Mezclas, INV_ORIGEN_ALFA, ORIGEN_ALFA, SUMAR, UNO};
    let v = crate::pruebas::triangulo([1.0; 3], [1.0; 3], true);
    let pinta = |mascara: u8| {
        let m = Mezcla { encendida: true, origen: ORIGEN_ALFA, destino: INV_ORIGEN_ALFA, op: SUMAR, origen_a: UNO, destino_a: INV_ORIGEN_ALFA, op_a: SUMAR, mascara };
        let mut rt = [Mezcla::NINGUNA; 8];
        rt[0] = m;
        let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None, mezcla: Mezclas { rt, factor: [1.0; 4] }, z_del_sombreador: false };
        let fondo = rgba([0.0, 0.0, 1.0, 1.0]);
        let mut px = vec![fondo; 64];
        let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None };
        trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, |_, c| {
            c[0] = [1.0, 0.0, 0.0, 0.5];
            true
        });
        (px, fondo)
    };
    let (px, fondo) = pinta(0xF);
    // (1, 0, 0) * 0.5 + (0, 0, 1) * 0.5; alfa: 0.5 + 1 * 0.5 = 1.
    let medio = rgba([0.5, 0.0, 0.5, 1.0]);
    assert_eq!(px.iter().filter(|&&p| p == medio).count(), 28, "los 28 que cubre");
    assert_eq!(px.iter().filter(|&&p| p == fondo).count(), 64 - 28, "lo demas, el fondo");
    let (px, _) = pinta(0b1011);
    assert_eq!(px[0], rgba([0.5, 0.0, 1.0, 1.0]), "sin el azul en la mascara, el azul se queda");
}

const PROFUNDIDAD_PS: &[u8] = include_bytes!("../prueba/profundidad.dxil");

/// *** `profundidad.hlsl` (de `dxc`) escribe SV_Depth = x / 8: sobre una Z
/// limpiada a 0.5 con LESS, pasan solo las cuatro columnas de la izquierda
/// (su Z, la del sombreador; el triangulo esta a 0.9 y no cuenta), y esa Z
/// queda escrita. Antes el enlace se negaba.
#[test]
fn el_de_pixeles_escribe_su_profundidad() {
    let (en, ia) = enlace(PROFUNDIDAD_PS);
    assert_eq!((en.objetivos.as_slice(), en.profundidad_ps), (&[0, trama::PROFUNDIDAD as u8][..], Some(1)));
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: Some(trama::Profundidad { funcion: 2, escribir: true }), mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
    let vertices = cuadro(0.9, 1.0);
    let l = Lote { enlace: &en, entradas: &ia, vertices: &vertices, paso: 32, ids: &[0, 1, 2, 2, 1, 3], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: Some(0.5f32.to_bits()), limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0 };
    let (mut px, mut z) = (vec![0u32; 64], vec![0u32; 64]);
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: Some(&mut z), cadena: false, otros: &mut [], flotante: None };
    let c = lote::en_cpu(&l, &mut d).unwrap();
    assert_eq!((c.tapados, c.pasan), (32, 32), "con SV_Depth, la prueba DESPUES del sombreador: {c:?}");
    for k in 0..64 {
        let x = k % 8;
        if x < 4 {
            assert_eq!((px[k], f32::from_bits(z[k])), (0xFFFF_FFFF, (x as f32 + 0.5) / 8.0), "({x}, {})", k / 8);
        } else {
            assert_eq!((px[k], f32::from_bits(z[k])), (0, 0.5), "({x}, {})", k / 8);
        }
    }
}

/// *** E2.7: lo que cuenta una consulta de OCLUSION (`Cuenta::pasan`): los
/// pixeles que pasan la prueba de profundidad, escriban color o no. Un
/// triangulo que cubre los 64 pixeles a z = 0.5, sobre una Z con las tres
/// columnas de la izquierda a 0.25 (delante: LESS no pasa) y el resto a 1:
/// pasan 40. Sin escribir la Z (como la caja de PredicationQueries), una
/// segunda vez da lo mismo; escribiendola, la segunda ya no pasa ninguno.
#[test]
fn la_trama_cuenta_lo_que_pasa_como_una_consulta_de_oclusion() {
    let v: Vec<trama::Sombreado> = [[-1.0f32, -1.0], [-1.0, 3.0], [3.0, -1.0]].iter().map(|&[x, y]| trama::Sombreado { pos: [x, y, 0.5, 1.0], atributos: Vec::new() }).collect();
    let z0: Vec<u32> = (0..64).map(|k| if k % 8 < 3 { 0.25f32 } else { 1.0 }.to_bits()).collect();
    let pasan = |escribir: bool| {
        let (mut px, mut z) = (vec![0u32; 64], z0.clone());
        let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: Some(trama::Profundidad { funcion: 2, escribir }), mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false };
        let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: Some(&mut z), cadena: false, otros: &mut [], flotante: None };
        let mut ps = |_: &[[f32; 4]], c: &mut [[f32; 4]; trama::SALIDAS]| {
            c[0] = [1.0; 4];
            true
        };
        let a = trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, &mut ps);
        let b = trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, &mut ps);
        (a.pasan, a.tapados, b.pasan)
    };
    assert_eq!(pasan(false), (40, 24, 40));
    assert_eq!(pasan(true), (40, 24, 0), "con su Z escrita (0.5), LESS ya no deja pasar 0.5");
}

/// *** N5.16: un render target de FLOAT (RGBA16F) guarda lo que pasa de 1, y
/// la mezcla ADITIVA suma en float: 1.25 y luego 2.5 dan 3.75 (exacto en
/// half), donde uno de 8 bits se quedaba en 1. Lo que no cabe en un half se
/// redondea como en la GPU (1/3), y el otro render target, R11G11B10F, con
/// sus 6 bits de mantisa y sin negativos.
#[test]
fn un_render_target_de_float_guarda_mas_de_uno_y_suma_en_float() {
    let v: Vec<trama::Sombreado> = [[-1.0f32, -1.0], [-1.0, 3.0], [3.0, -1.0]].iter().map(|&[x, y]| trama::Sombreado { pos: [x, y, 0.5, 1.0], atributos: Vec::new() }).collect();
    let suma = crate::mezcla::Mezcla { encendida: true, origen: 2, destino: 2, origen_a: 2, destino_a: 2, ..crate::mezcla::Mezcla::NINGUNA };
    let mut mezcla = crate::mezcla::Mezclas::NINGUNA;
    mezcla.rt[0] = suma;
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 2.0, 2.0, 0.0, 1.0], tijera: [0, 0, 2, 2], descarte: 1, antihorario: false, profundidad: None, mezcla, z_del_sombreador: false };
    let (mut px, mut otro) = (vec![0u32; 16], vec![0u32; 16]);
    let mut otros = [trama::Otro { pixeles: Some(&mut otro), bgra: false, flotante: Some(26) }];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 2, alto: 2, bgra: false, z: None, cadena: false, otros: &mut otros, flotante: Some(10) };
    for c in [[1.25f32, 1.0 / 3.0, -2.0, 0.5], [2.5, 0.0, 0.0, 0.25]] {
        trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, |_, s| {
            s[0] = c;
            s[1] = [-1.0, 1.0 + 1.0 / 128.0, 70000.0, 0.0];
            true
        });
    }
    let leido = |p: &[u32], k: usize| [0, 1, 2, 3].map(|c| f32::from_bits(p[4 * k + c]));
    let tercio = crate::formato_ia::half(crate::formato_ia::a_half(1.0 / 3.0));
    for k in 0..4 {
        assert_eq!(leido(&px, k), [3.75, tercio, -2.0, 0.75], "texel {k}: sumado en float, cuantizado a half");
        // R11G11B10: 0 (sin negativos), 1 (6 bits), infinito (pasa del mayor).
        assert_eq!(leido(&otro, k), [0.0, 1.0, f32::INFINITY, 1.0], "texel {k}: el R11G11B10F, sin mezcla");
    }
}
