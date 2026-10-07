//! **Las franjas repartidas entre nucleos** (H4.3 de
//! `PLAN_LOS_DOCE_DIRECTORES`, 07-10): `bmo_proton_x_casa::bandas` con
//! HILOS de verdad, y con franjas que "fallan" (las rehace quien reparte),
//! da los mismos bytes que el dibujo entero. Y su `Recuerdo` aguanta varios
//! a la vez.

use bmo_proton_x::lote::{self, ElementoIa, Lote, Topologia};
use bmo_proton_x::textura::Recursos;
use bmo_proton_x::{bandas as puro, dxil, trama};
use bmo_proton_x_casa::bandas;

const CUBO_VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");

/// Repartir en HILOS de verdad, uno por franja, a la vez.
fn en_hilos(n: u32, f: &(dyn Fn(u32) + Sync)) -> u64 {
    std::thread::scope(|s| {
        for k in 0..n {
            s.spawn(move || f(k));
        }
    });
    0
}

/// Las impares "fallan" sin pintar (en hilos las pares): las rehace quien
/// reparte.
fn con_fallos(n: u32, f: &(dyn Fn(u32) + Sync)) -> u64 {
    std::thread::scope(|s| {
        for k in (0..n).step_by(2) {
            s.spawn(move || f(k));
        }
    });
    (0..n).filter(|k| k % 2 == 1).fold(0, |m, k| m | 1 << k)
}

fn cb_de(f: u32) -> Vec<u8> {
    let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
    c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
}

/// El cubo del fotograma `f` con profundidad y sus limpiezas: entero, o en
/// `n` franjas repartidas por `repartir`.
fn cubo(f: u32, n: Option<(u32, bandas::Repartir)>) -> (Vec<u32>, Vec<u32>, trama::Cuenta) {
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let e = |s: &str, formato, desde| ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None };
    let entradas = [e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)];
    let enlace = lote::enlazar(&vs, &ps, &entradas).unwrap();
    let vertices: Vec<u8> = bmo_cubo::vertices().iter().flat_map(|v| v.pos.iter().chain(&v.normal).chain(&v.color).flat_map(|x| x.to_le_bytes())).collect();
    let ids: Vec<u32> = bmo_cubo::indices().iter().map(|&i| i as u32).collect();
    let b0 = cb_de(f);
    let cb = lote::juntar_constantes(&enlace.constantes, |_| Some(&b0[..]));
    let (w, h) = (bmo_cubo::referencia::ANCHO, bmo_cubo::referencia::ALTO);
    let reglas = trama::Reglas { viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0], tijera: [0, 0, w as i32, h as i32], descarte: 3, antihorario: false, profundidad: Some(trama::Profundidad { funcion: 2, escribir: true }), mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: &enlace, entradas: &entradas, vertices: &vertices, paso: 40, ids: &ids, topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: Some(1.0f32.to_bits()), limpiar_rt: Some(bmo_cubo::FONDO), recursos: Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut px = vec![0xDEAD_BEEF; (w * h) as usize];
    let mut zs = vec![7u32; (w * h) as usize];
    let mut d = trama::Destino { pixeles: &mut px, ancho: w, alto: h, bgra: true, z: Some(&mut zs[..]), cadena: false, otros: &mut [], flotante: None, stencil: None };
    let cuenta = match n {
        None => lote::en_cpu(&l, &mut d).unwrap(),
        Some((n, repartir)) => {
            assert!(puro::se_parte(&l, &d, n, false));
            bandas::dibujar(&l, &mut d, n, repartir, &lote::en_cpu).unwrap()
        }
    };
    (px, zs, cuenta)
}

/// *** EL CUBO EN FRANJAS POR HILOS ES EL CUBO: pixeles, profundidad y
/// cuentas, con 2, 3, 5, 12 y 64 franjas, en hilos y con fallos.
#[test]
fn el_cubo_en_franjas_por_hilos_da_los_mismos_bytes() {
    for f in [0u32, 30, 60] {
        let (px, zs, c) = cubo(f, None);
        assert_eq!(bmo_cubo::referencia::huella(&px), bmo_cubo::referencia::HUELLAS.iter().find(|h| h.0 == f).unwrap().1, "el entero da la huella de D3D12");
        for n in [2u32, 3, 5, 12, 64] {
            for (nombre, r) in [("en hilos", &en_hilos as bandas::Repartir), ("con fallos", &con_fallos)] {
                let (bpx, bzs, bc) = cubo(f, Some((n, r)));
                assert!(bpx == px, "fotograma {f}, {n} franjas {nombre}: los pixeles");
                assert!(bzs == zs, "fotograma {f}, {n} franjas {nombre}: la profundidad");
                assert_eq!((bc.dibujados, bc.descartados, bc.recortados), (c.dibujados, c.descartados, c.recortados), "{nombre}");
                assert_eq!((bc.pixeles, bc.tapados, bc.pasan), (c.pixeles, c.tapados, c.pasan), "{nombre}");
            }
        }
    }
}

/// El recuerdo: busca una vez por clave, lleno no recuerda mas (y da lo
/// mismo), y con ocho hilos a la vez no pierde ni repite nada.
#[test]
fn el_recuerdo_busca_una_vez_y_aguanta_varios_a_la_vez() {
    use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
    let r: bandas::Recuerdo<(u8, u32), u64, 4> = Default::default();
    let veces = AtomicU32::new(0);
    let buscar = |k: (u8, u32)| {
        veces.fetch_add(1, SeqCst);
        k.0 as u64 * 1000 + k.1 as u64
    };
    for _ in 0..3 {
        for k in 0..4u32 {
            assert_eq!(r.o_buscar((1, k), || buscar((1, k))), 1000 + k as u64);
        }
    }
    assert_eq!(veces.load(SeqCst), 4, "una vez por clave");
    assert_eq!(r.o_buscar((2, 9), || buscar((2, 9))), 2009);
    assert_eq!(r.o_buscar((2, 9), || buscar((2, 9))), 2009, "lleno: lo busca otra vez, y da lo mismo");
    assert_eq!(veces.load(SeqCst), 6);
    let r: bandas::Recuerdo<(u8, u32), u64, 256> = Default::default();
    std::thread::scope(|s| {
        for h in 0..8u32 {
            let r = &r;
            s.spawn(move || {
                for k in 0..200u32 {
                    let k = (k + h * 7) % 200;
                    assert_eq!(r.o_buscar((3, k), || 3000 + k as u64), 3000 + k as u64);
                }
            });
        }
    });
}
