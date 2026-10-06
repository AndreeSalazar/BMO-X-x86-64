//! Las pruebas de los UAV escritos desde un DIBUJO (05-10): los
//! sombreadores de `uavpixel.hlsl` (los del juez `uavpixel.exe`, de `dxc`),
//! en un cuadro de pantalla completa de 8x8 sin bufer de vertices.
//!
//! Hasta hoy lo que un sombreador de vertices o de pixeles escribia en un UAV
//! se perdia (lo decia un aviso de la casa) y lo que leia era 0.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::bufer::Uav;
use crate::dxil;
use crate::lote::{self, Enlace, Lote, Topologia};
use crate::trama;

const VS: &[u8] = include_bytes!("../prueba/uavpixel_vs.dxil");
const POSICION: &[u8] = include_bytes!("../prueba/uavpixel_pos.dxil");
const CUENTA: &[u8] = include_bytes!("../prueba/uavpixel_cuenta.dxil");
const TEMPRANA: &[u8] = include_bytes!("../prueba/uavpixel_temprana.dxil");

/// DXGI_FORMAT_R32_UINT: el de la textura de A y el del bufer de C.
const R32_UINT: u32 = 42;

fn enlace(ps: &[u8]) -> Enlace {
    lote::enlazar(&dxil::leer(VS).unwrap(), &dxil::leer(ps).unwrap(), &[]).unwrap()
}

/// Memoria que vive lo que la prueba (el lote lleva UAV de `'static`, como
/// los de la casa).
fn memoria(palabras: usize) -> &'static mut [u8] {
    Vec::leak(vec![0u8; 4 * palabras])
}

/// El UAV de la ranura de `(espacio 0, registro)` del enlace, en su sitio.
fn poner(en: &Enlace, uavs: &mut [Option<Uav<'static>>], registro: u32, u: Uav<'static>) {
    let k = en.ranuras.uavs.iter().position(|l| l.registro == registro).expect("el enlace lo lee");
    uavs[k] = Some(u);
}

/// Dibujar el cuadro (a z = 0.5) con LESS sobre una Z limpiada a `z`, si
/// hay; lo que cuenta la trama.
fn dibujar(en: &Enlace, uavs: &RefCell<Vec<Option<Uav<'static>>>>, z: Option<f32>) -> trama::Cuenta {
    let profundidad = z.map(|_| trama::Profundidad { funcion: 2, escribir: true });
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: en, entradas: &[], vertices: &[], paso: 0, ids: &[0, 1, 2, 3, 4, 5], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, uavs: Some(uavs) };
    let (mut px, mut zs) = (vec![0u32; 64], vec![z.unwrap_or(1.0).to_bits(); 64]);
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: z.map(|_| &mut zs[..]), cadena: false, otros: &mut [], flotante: None, stencil: None };
    lote::en_cpu(&l, &mut d).unwrap()
}

/// *** A y C del juez: cada pixel escribe su posicion en SU texel de un
/// `RWTexture2D<uint>`, y cada vertice 100 + su numero en SU elemento de un
/// `RWBuffer<uint>`. Antes, la textura y el bufer seguian a cero.
#[test]
fn cada_pixel_y_cada_vertice_escriben_su_uav() {
    let en = enlace(POSICION);
    assert!(en.ps.toca_uav() && en.vs.toca_uav());
    let (tex, buf) = (memoria(64), memoria(8));
    let mut v: Vec<Option<Uav>> = (0..en.ranuras.uavs.len()).map(|_| None).collect();
    poner(&en, &mut v, 1, Uav { bytes: tex, formato: R32_UINT, paso: 8, elementos: 64, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA });
    poner(&en, &mut v, 3, Uav { bytes: buf, formato: R32_UINT, paso: 0, elementos: 8, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA });
    let uavs = RefCell::new(v);
    let c = dibujar(&en, &uavs, None);
    assert_eq!(c.pixeles, 64, "{c:?}");
    let v = uavs.into_inner();
    let palabras = |u: &Option<Uav>| u.as_ref().unwrap().bytes.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect::<Vec<u32>>();
    let k = |r: u32| en.ranuras.uavs.iter().position(|l| l.registro == r).unwrap();
    let t = palabras(&v[k(1)]);
    for y in 0..8u32 {
        for x in 0..8u32 {
            assert_eq!(t[(y * 8 + x) as usize], y << 16 | x, "texel ({x}, {y})");
        }
    }
    assert_eq!(palabras(&v[k(3)]), [100, 101, 102, 103, 104, 105, 0, 0], "seis vertices, cada uno en el suyo; los dos de detras, sin tocar");
}

/// El contador de B, tras dibujar el cuadro con `ps` (sin Z, o sobre una
/// Z de 0.25: delante del cuadro, que esta a 0.5).
fn cuenta(ps: &[u8], con_z: bool) -> (u32, trama::Cuenta) {
    let en = enlace(ps);
    let c = memoria(1);
    let mut v: Vec<Option<Uav>> = (0..en.ranuras.uavs.len()).map(|_| None).collect();
    poner(&en, &mut v, 2, Uav { bytes: c, formato: 0, paso: 0, elementos: 1, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA });
    let uavs = RefCell::new(v);
    let cuenta = dibujar(&en, &uavs, con_z.then_some(0.25));
    let v = uavs.into_inner();
    let b = &v[en.ranuras.uavs.iter().position(|l| l.registro == 2).unwrap()].as_ref().unwrap().bytes;
    (u32::from_le_bytes([b[0], b[1], b[2], b[3]]), cuenta)
}

/// *** B del juez: `InterlockedAdd(0, 1)` en cada pixel cubierto: 64. El
/// sombreador no lee nada (los 64 le dan lo mismo): con la memoria del
/// ultimo pixel de la trama corria UNA vez y el contador daba 1.
#[test]
fn cada_pixel_cubierto_suma_uno() {
    let (n, c) = cuenta(CUENTA, false);
    assert_eq!((n, c.sombreados), (64, 64), "{c:?}");
}

/// D3D: con UAV y sin `[earlydepthstencil]`, la profundidad se prueba
/// DESPUES del sombreador: un pixel tapado tambien suma (64), aunque no
/// pinte. Con `[earlydepthstencil]`, antes: ninguno suma.
#[test]
fn la_profundidad_va_despues_salvo_con_earlydepthstencil() {
    // El cuadro esta a z = 0.5 y la Z, a 0.25: LESS no deja pasar ninguno.
    let (n, c) = cuenta(CUENTA, true);
    assert_eq!((n, c.tapados, c.pasan), (64, 64, 0), "tarde: {c:?}");
    assert!(enlace(TEMPRANA).ps.computo.temprana, "las banderas de dx.entryPoints");
    assert!(!enlace(CUENTA).ps.computo.temprana);
    let (n, c) = cuenta(TEMPRANA, true);
    assert_eq!((n, c.tapados, c.sombreados), (0, 64, 0), "temprana: {c:?}");
}

/// El cuadro con `ps` (que suma 1 en el contador de B) y un STENCIL de una
/// cara con `funcion` y las operaciones `falla` y `pasa` (referencia 1),
/// sobre un plano a 0 y sin Z: el contador, la cuenta y el plano.
fn con_stencil(ps: &[u8], funcion: u8, falla: u8, pasa: u8) -> (u32, trama::Cuenta, Vec<u8>) {
    use crate::stencil::{Cara, Stencil, KEEP};
    let en = enlace(ps);
    let c = memoria(1);
    let mut v: Vec<Option<Uav>> = (0..en.ranuras.uavs.len()).map(|_| None).collect();
    poner(&en, &mut v, 2, Uav { bytes: c, formato: 0, paso: 0, elementos: 1, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA });
    let uavs = RefCell::new(v);
    let cara = Cara { falla, falla_z: KEEP, pasa, funcion, lectura: 0xFF, escritura: 0xFF, referencia: 1 };
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad: None, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: Some(Stencil { delante: cara, detras: cara }) };
    let l = Lote { enlace: &en, entradas: &[], vertices: &[], paso: 0, ids: &[0, 1, 2, 3, 4, 5], topologia: Topologia::Lista, cb: &[], reglas, limpiar_z: None, limpiar_rt: None, recursos: crate::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, uavs: Some(&uavs) };
    let (mut px, mut plano) = (vec![0u32; 64], vec![0u8; 64]);
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: false, z: None, cadena: false, otros: &mut [], flotante: None, stencil: Some(&mut plano) };
    let cuenta = lote::en_cpu(&l, &mut d).unwrap();
    let v = uavs.into_inner();
    let b = &v[en.ranuras.uavs.iter().position(|l| l.registro == 2).unwrap()].as_ref().unwrap().bytes;
    (u32::from_le_bytes([b[0], b[1], b[2], b[3]]), cuenta, plano)
}

/// *** El STENCIL con UAV (al juntar N5.3d y N5.12b, 05-10): como la
/// profundidad, el stencil va DESPUES del sombreador si escribe UAV (todos
/// suman y el que no pasa hace su operacion de fallo), y ANTES con
/// `[earlydepthstencil]` (no corre el que no pasa; su operacion ya se
/// escribio). La de paso es INCR a proposito: aplicada dos veces daria 2.
#[test]
fn el_stencil_con_uav_va_despues_salvo_con_earlydepthstencil() {
    use crate::stencil::{INCR, INCR_SAT, KEEP};
    const EQUAL: u8 = 3;
    const ALWAYS: u8 = 8;
    // Falla (EQUAL 1 contra 0): tarde, los 64 suman y quedan a 1 (INCR_SAT).
    let (n, c, p) = con_stencil(CUENTA, EQUAL, INCR_SAT, KEEP);
    assert_eq!((n, c.tapados, c.pasan), (64, 64, 0), "tarde, falla: {c:?}");
    assert!(p.iter().all(|&b| b == 1), "tarde, falla: {p:?}");
    // Falla con [earlydepthstencil]: ninguno corre, y el fallo ya esta escrito.
    let (n, c, p) = con_stencil(TEMPRANA, EQUAL, INCR_SAT, KEEP);
    assert_eq!((n, c.tapados, c.sombreados), (0, 64, 0), "temprana, falla: {c:?}");
    assert!(p.iter().all(|&b| b == 1), "temprana, falla: {p:?}");
    // Pasa (ALWAYS): los 64 suman y INCR se aplica UNA vez, en los dos modos.
    for ps in [CUENTA, TEMPRANA] {
        let (n, c, p) = con_stencil(ps, ALWAYS, KEEP, INCR);
        assert_eq!((n, c.pasan), (64, 64), "pasa: {c:?}");
        assert!(p.iter().all(|&b| b == 1), "INCR una vez: {p:?}");
    }
}
