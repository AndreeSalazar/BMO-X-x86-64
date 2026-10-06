//! Las pruebas de E2.5 (05-10): las OLAS de verdad, con los sombreadores de
//! `prueba/olas_juez.hlsl` (los de `olas.exe`): el computo en olas de 32
//! hilos seguidos, y los pixeles en cuadros de 2x2 con sus ayudantes.

use alloc::vec;
use alloc::vec::Vec;

use crate::bufer::Uav;
use crate::dxil::{self, programa};
use crate::lote::{self, Lote, Topologia};
use crate::textura::Recursos;
use crate::trama;

const OLAS_CS: &[u8] = include_bytes!("../prueba/olas_cs.dxil");
const OLAS_VS: &[u8] = include_bytes!("../prueba/olas_vs.dxil");
const OLAS_PS: &[u8] = include_bytes!("../prueba/olas_ps.dxil");

/// Las palabras que escribe cada hilo de `CSOlas`.
const N: usize = 26;

/// **Lo que tiene que dar** el hilo `i` del grupo `g` de `CSOlas`, con olas
/// de 32 hilos seguidos (la de `i` son los `w0..w0 + 32`): la cuenta de
/// cada operacion, a mano, sobre los carriles de su ola. Es la misma que
/// hace `olas.cpp` para Windows.
pub fn esperado(g: u32, i: u32) -> [u32; N] {
    let w0 = i & !31;
    let ola = || w0..w0 + 32;
    let x = |j: u32| j.wrapping_mul(2654435761).wrapping_add(g);
    let f = |j: u32| j as f32 * 0.5;
    let cuenta = |p: &dyn Fn(u32) -> bool| ola().filter(|&j| p(j)).count() as u32;
    let mut o = [0u32; N];
    o[0] = 32;
    o[1] = i % 32;
    o[2] = (i % 32 == 0) as u32;
    o[3] = ola().sum();
    o[4] = ola().fold(1u32, |p, j| p.wrapping_mul(if j % 3 == 0 { 3 } else { 1 }));
    o[5] = ola().map(|j| j as i32 - 40).min().unwrap() as u32;
    o[6] = ola().map(x).max().unwrap();
    o[7] = ola().fold(u32::MAX, |s, j| s & (x(j) | 0x0F0F_0F0F));
    o[8] = ola().fold(0, |s, j| s | (x(j) & 0x00FF_00FF));
    o[9] = ola().fold(0, |s, j| s ^ x(j));
    o[10] = cuenta(&|j| j % 3 == 0);
    o[11] = ola().filter(|&j| j % 5 == 0 || j == 33).fold(0, |m, j| m | 1 << (j - w0));
    o[12] = 0;
    o[13] = x(w0 + 5);
    o[14] = x(w0);
    o[15] = (w0..i).sum();
    o[16] = (w0..i).fold(1u32, |p, j| p.wrapping_mul(if j % 4 == 1 { 3 } else { 1 }));
    o[17] = (w0..i).filter(|j| j % 2 == 1).count() as u32;
    // AllEqual(i / 8) nunca (una ola tiene 4 octavos), AllEqual(g) siempre,
    // AnyTrue(i == 37) en la ola de 32..63, AllTrue(i < 60) en la de 0..31.
    o[18] = 2 | if w0 == 32 { 4 } else { 8 };
    o[19] = ola().map(f).fold(0.0f32, |s, v| s + v).to_bits();
    o[20] = (w0..i).map(f).fold(0.0f32, |s, v| s + v).to_bits();
    o[21] = (1.0f32 - f(w0 + 31)).to_bits();
    o[22] = ola().map(|j| j as i32 - 40).max().unwrap() as u32;
    // En el si: los de i % 3 == 0 (su suma, y el primero de ellos), o los
    // demas (cuantos, y cuantos de ellos van antes).
    o[23] = if i % 3 == 0 {
        ola().filter(|j| j % 3 == 0).sum::<u32>() * 1000 + ola().find(|j| j % 3 == 0).unwrap()
    } else {
        cuenta(&|j| j % 3 != 0) * 1000 + (w0..i).filter(|j| j % 3 != 0).count() as u32
    };
    // Escalarizar: cada vuelta el primer activo dice su v y salen los que
    // lo tienen; las vueltas de `i`, las de su v.
    let v = |j: u32| (j * 7) % 5;
    let mut quedan: Vec<u32> = ola().collect();
    let mut vuelta = 0;
    while !quedan.is_empty() {
        vuelta += 1;
        let primero = v(quedan[0]);
        if v(i) == primero {
            o[24] = vuelta;
        }
        quedan.retain(|&j| v(j) != primero);
    }
    // El bucle: en la vuelta k siguen los de i % 4 > k.
    o[25] = (0..i % 4).map(|k| cuenta(&|j| j % 4 > k)).sum();
    o
}

/// *** `CSOlas` (de `dxc`, cs_6_0, 64 hilos por grupo, 2 grupos): cada
/// operacion de ola sobre su ola de 32, dentro de un si, en el bucle de
/// escalarizar y en uno con salidas a distintas vueltas, bit a bit. Con
/// "un hilo por ola" (antes del 05-10) no compilaba (la papeleta, 116) y
/// contaba 1 carril.
#[test]
fn el_computo_va_en_olas_de_32_hilos_seguidos() {
    let p = programa::compilar(&dxil::leer(OLAS_CS).unwrap()).unwrap();
    assert!(p.usa_olas());
    assert_eq!(crate::nativo_computo::compilar(&p), None, "las olas, por el interprete");
    let mut salida = vec![0xEEu8; 128 * N * 4];
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 4, elementos: (128 * N) as u32, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
        assert_eq!(p.despachar([2, 1, 1], &[], &Recursos::NINGUNO, &mut uavs), 128);
    }
    let mut mal = Vec::new();
    for t in 0..128u32 {
        let quiero = esperado(t / 64, t % 64);
        for (k, &q) in quiero.iter().enumerate() {
            let o = (t as usize * N + k) * 4;
            let visto = u32::from_le_bytes(salida[o..o + 4].try_into().unwrap());
            if visto != q {
                mal.push((t, k, visto, q));
            }
        }
    }
    assert!(mal.is_empty(), "{} mal (hilo, palabra, visto, quiero); los primeros: {:x?}", mal.len(), &mal[..mal.len().min(6)]);
}

/// Pintar con `VSOlas` y `PSOlas` los vertices `ids` en 64 x 64.
fn pintar(ids: &[u32]) -> (Vec<u32>, trama::Cuenta) {
    let (vs, ps) = (dxil::leer(OLAS_VS).unwrap(), dxil::leer(OLAS_PS).unwrap());
    let en = lote::enlazar(&vs, &ps, &[]).unwrap();
    assert!(en.ps.usa_olas() && en.pos_ps == Some(0));
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 64.0, 64.0, 0.0, 1.0], tijera: [0, 0, 64, 64], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids, topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: Some(0), recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, uavs: None };
    let mut px = vec![0u32; 64 * 64];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 64, alto: 64, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    let c = lote::en_cpu(&l, &mut d).unwrap();
    (px, c)
}

/// *** `PSOlas` sobre el destino entero: cada pixel lee el x de su vecino
/// de cuadro (R = x ^ 1), el y del otro (G = y ^ 1), y sus ocho pruebas de
/// ola salen (B = 255); A, los activos de su ola: 32 en los cuadros llenos
/// (8 cuadros de un triangulo), menos en la diagonal que parte los dos.
#[test]
fn los_pixeles_van_en_cuadros_y_en_olas() {
    let (px, c) = pintar(&[0, 1, 2, 3, 4, 5]);
    assert_eq!((c.pixeles, c.sombreados, c.tirados), (4096, 4096, 0), "{c:?}");
    let mut activos = [0u32; 33];
    for y in 0..64u32 {
        for x in 0..64u32 {
            let p = px[(y * 64 + x) as usize];
            assert_eq!(p & 0xFF_FFFF, (x ^ 1) | (y ^ 1) << 8 | 255 << 16, "pixel ({x}, {y}): {p:08x}");
            activos[(p >> 24).min(32) as usize] += 1;
        }
    }
    assert_eq!(activos[0], 0);
    // Mas de la mitad en olas llenas (las de la diagonal, en olas con
    // cuadros a medias: los dos triangulos la parten).
    assert!(activos[32] > 2048, "{activos:?}");
}

/// *** Un triangulo que cubre SOLO el pixel (2, 2): su cuadro corre con
/// tres AYUDANTES (3, 2), (2, 3) y (3, 3): el pixel lee sus x e y (R = G =
/// 3), y la ola tiene UN activo (los ayudantes no cuentan: A = 1). Nada mas
/// se pinta.
#[test]
fn un_pixel_solo_lee_a_sus_ayudantes() {
    let (px, c) = pintar(&[6, 7, 8]);
    assert_eq!((c.pixeles, c.sombreados), (1, 1), "{c:?}");
    for (k, &p) in px.iter().enumerate() {
        let quiero = if k == 2 * 64 + 2 { 3 | 3 << 8 | 255 << 16 | 1 << 24 } else { 0 };
        assert_eq!(p, quiero, "pixel ({}, {}): {p:08x}", k % 64, k / 64);
    }
}

/// Los cuadros no cambian lo que entra a un pixel: un triangulo con
/// atributos que cambian, pintado pixel a pixel y en cuadros (con un
/// sombreador "de olas" que solo copia), da los MISMOS bits: la
/// interpolacion es la de `trama::Tri`, la misma para los dos.
#[test]
fn en_cuadros_entra_lo_mismo_que_pixel_a_pixel() {
    let v = vec![
        trama::Sombreado { pos: [-0.9, 0.8, 0.25, 1.0], atributos: vec![[0.1, 0.2, 0.3, 0.4]] },
        trama::Sombreado { pos: [2.4, 1.6, 0.5, 2.0], atributos: vec![[0.9, 0.5, 0.0, 1.0]] },
        trama::Sombreado { pos: [-0.5, -3.0, 1.5, 3.0], atributos: vec![[0.3, 0.7, 0.6, 0.2]] },
    ];
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 32.0, 32.0, 0.0, 1.0], tijera: [0, 0, 32, 32], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let mut uno = Vec::new();
    let mut px = vec![0u32; 32 * 32];
    let mut d = trama::Destino { pixeles: &mut px, ancho: 32, alto: 32, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, |e, c| {
        uno.push(e[0]);
        c[0] = e[0];
        true
    });
    let mut cuadros = Vec::new();
    let mut ola = |o: &mut [crate::cuadros::Carril]| {
        for c in o.iter_mut() {
            if !c.ayudante {
                cuadros.push(c.entrada[0]);
            }
            c.colores[0] = c.entrada[0];
            c.queda = true;
        }
    };
    let mut px2 = vec![0u32; 32 * 32];
    let mut d = trama::Destino { pixeles: &mut px2, ancho: 32, alto: 32, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: None };
    trama::dibujar_en_olas(&reglas, trama::Efectos::default(), &v, &[[0, 1, 2]], &mut d, None, &mut ola);
    assert!(uno.len() > 100);
    let bits = |v: &Vec<[f32; 4]>| {
        let mut b: Vec<[u32; 4]> = v.iter().map(|x| x.map(f32::to_bits)).collect();
        b.sort();
        b
    };
    assert_eq!(bits(&uno), bits(&cuadros));
    assert_eq!(px, px2);
}

/// *** En olas se pone cada pixel IGUAL que pixel a pixel (al juntar E2.5
/// con N5.3d y N5.12b, 05-10): con profundidad (LESS, que escribe),
/// STENCIL (EQUAL 1; falla INCR, que cambia: corre para saber si lo tira;
/// falla_z DECR_SAT; pasa REPLACE) y `discard`, en los cuatro modos (sin
/// UAV, con UAV -- todo despues --, `[earlydepthstencil]` con y sin UAV):
/// los mismos colores, la misma Z, el mismo plano de stencil y la misma
/// cuenta. Los ayudantes no escriben ni cuentan nada.
#[test]
fn en_olas_la_profundidad_el_stencil_y_la_cuenta_son_los_de_siempre() {
    use crate::stencil::{Cara, Stencil, DECR_SAT, INCR, REPLACE};
    let v = vec![
        trama::Sombreado { pos: [-0.9, 0.8, 0.25, 1.0], atributos: vec![[0.1, 0.2, 0.3, 0.4]] },
        trama::Sombreado { pos: [2.4, 1.6, 0.5, 2.0], atributos: vec![[0.9, 0.5, 0.0, 1.0]] },
        trama::Sombreado { pos: [-0.5, -3.0, 1.5, 3.0], atributos: vec![[0.3, 0.7, 0.6, 0.2]] },
    ];
    let cara = Cara { falla: INCR, falla_z: DECR_SAT, pasa: REPLACE, funcion: 3, lectura: 0xFF, escritura: 0xFF, referencia: 1 };
    let profundidad = Some(trama::Profundidad { funcion: 2, escribir: true });
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 32.0, 32.0, 0.0, 1.0], tijera: [0, 0, 32, 32], descarte: 1, antihorario: false, profundidad, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: Some(Stencil { delante: cara, detras: cara }) };
    let queda = |e: &[[f32; 4]]| e[0][0] <= 0.6;
    for (uav, temprana) in [(false, false), (true, false), (false, true), (true, true)] {
        let efectos = trama::Efectos { uav, temprana, referencia: false };
        let pintar = |olas: bool| {
            let mut px = vec![0u32; 32 * 32];
            let mut zs: Vec<u32> = (0..32 * 32u32).map(|i| if (i % 32 + i / 32) % 3 == 0 { 0.3f32 } else { 1.0 }.to_bits()).collect();
            let mut plano: Vec<u8> = (0..32 * 32u32).map(|i| ((i % 32) * 7 + i / 32) as u8 % 3).collect();
            let mut d = trama::Destino { pixeles: &mut px, ancho: 32, alto: 32, bgra: false, z: Some(&mut zs), cadena: false, otros: &mut [], flotante: None, stencil: Some(&mut plano) };
            let mut ayudantes = 0;
            let c = if olas {
                let mut ola = |o: &mut [crate::cuadros::Carril]| {
                    for c in o.iter_mut() {
                        ayudantes += c.ayudante as u32;
                        c.colores[0] = c.entrada[0];
                        c.queda = queda(&c.entrada);
                    }
                };
                trama::dibujar_en_olas(&reglas, efectos, &v, &[[0, 1, 2]], &mut d, None, &mut ola)
            } else {
                trama::dibujar_con(&reglas, efectos, &v, &[[0, 1, 2]], &mut d, None, |e, c| {
                    c[0] = e[0];
                    queda(e)
                })
            };
            (px, zs, plano, [c.dibujados as u64, c.pixeles, c.tapados, c.tirados, c.pasan], ayudantes)
        };
        let (a, b) = (pintar(false), pintar(true));
        assert!(b.4 > 0, "hay ayudantes");
        assert!(a.3[4] > 50 && a.3[2] > 50, "pasan y tapados: {:?}", a.3);
        assert_eq!((a.0, a.1, a.2, a.3), (b.0, b.1, b.2, b.3), "uav {uav}, temprana {temprana}");
    }
}
