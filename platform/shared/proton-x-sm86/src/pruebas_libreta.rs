//! 9d (06-10): la LIBRETA de la 3060 -- el termometro que deja el cuerpo
//! (lo que corre el simulador es lo que cuenta la CPU), lo raro que apunta,
//! su ida y vuelta por el .bsf, y la puerta que revisa YA lo apuntado.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil;
use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::lote::{self, ElementoIa, Enlace, Lote, Topologia};

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};

use crate::libreta::{raro, registro, termometro};
use crate::puerta::{Blanco, Puerta};
use crate::simula::{correr, Maquina};
use crate::vivo::{a_bsf, comprobar, de_bsf, mapa};
use crate::{emitir_con, emitir_libreta, Abi};

const CUBO_VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");
const TEX_VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const TEX_PS: &[u8] = include_bytes!("../../proton-x/prueba/textura_ps.dxil");

fn e(s: &str, formato: u32, desde: u32) -> ElementoIa {
    ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None }
}

fn cubo() -> (Enlace, Vec<ElementoIa>) {
    let ia = std::vec![e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)];
    (lote::enlazar(&dxil::leer(CUBO_VS).unwrap(), &dxil::leer(CUBO_PS).unwrap(), &ia).unwrap(), ia)
}

fn textura() -> Enlace {
    let ia = std::vec![e("POSITION", 6, 0), e("TEXCOORD", 16, 12)];
    lote::enlazar(&dxil::leer(TEX_VS).unwrap(), &dxil::leer(TEX_PS).unwrap(), &ia).unwrap()
}

/// *** Con libreta, los cuatro sombreadores PASAN la comprobacion -- sus
/// salidas Y su termometro, tanda a tanda -- y cuentan uno mas por tanda;
/// el termometro va en el registro de detras de las salidas; y el juez de
/// la forma (el primer nivel) lo deja pasar.
#[test]
fn el_termometro_de_la_3060_es_el_de_la_cpu() {
    let mut limpios = 0;
    for en in [cubo().0, textura()] {
        for p in [&en.vs, &en.ps] {
            let sin = emitir_con(p, 64, Abi::Registros).unwrap();
            let con = emitir_libreta(p, 64, Abi::Registros, true).unwrap();
            assert_eq!((sin.termometro, con.termometro), (None, Some(registro(p.salidas))));
            assert!(con.registros > registro(p.salidas) as u32);
            assert_eq!(comprobar(p, &con, "prueba").unwrap(), comprobar(p, &sin, "prueba").unwrap() + 4, "una cuenta mas por tanda");
            // El primer nivel del juez (la FORMA): lo que pegaria el kernel.
            // Con libreta dice lo MISMO que sin ella (el TEX del de la
            // textura no es de la lista de una app: lo pone el pegamento).
            let v = juzgar_drenado(&con.codigo, &Contexto { registros: con.registros + RESERVADOS, sph: None });
            assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            let app = |x: &crate::Emitido| juzgar_cuerpo_de_app(&x.codigo, x.registros).map_err(|b| (b.regla, b.instruccion));
            assert_eq!(app(&con), app(&sin));
            limpios += app(&con).is_ok() as usize;
        }
    }
    assert_eq!(limpios, 3, "los dos del cubo y el de vertice de la textura, limpios del todo");
}

/// Un programa que saca sus cuatro entradas tal cual.
fn espejo() -> Programa {
    let mut ops: Vec<Op> = (0..4u8).map(|k| Op::Entrada { d: k as u16, elemento: 0, componente: k }).collect();
    ops.extend((0..4u8).map(|k| Op::Salida { s: k as u16, elemento: 0, componente: k }));
    Programa { ops, iniciales: std::vec![0.0; 4], entradas: 1, salidas: 1, lee: 1, filas_cb: 0, ranuras: Default::default(), computo: Default::default() }
}

/// *** Lo RARO se apunta: un infinito, un NaN, o una suma que se pasa de
/// f32::MAX; lo normal (con negativos, ceros y subnormales), no. Lo dice
/// el simulador de la 3060 y lo mismo la CPU.
#[test]
fn la_3060_apunta_lo_raro_y_solo_lo_raro() {
    let p = espejo();
    let x = emitir_libreta(&p, 16, Abi::Banco, true).unwrap();
    let t = x.termometro.unwrap() as usize;
    let casos: [([f32; 4], bool); 7] = [
        ([1.0, -2.0, 0.0, 3.5], false),
        ([-0.0, 1e-40, -1e30, 7.0], false),
        ([f32::INFINITY, 0.0, 0.0, 1.0], true),
        ([1.0, f32::NEG_INFINITY, 2.0, 3.0], true),
        ([1.0, 2.0, f32::NAN, 3.0], true),
        ([f32::MAX, f32::MAX, 0.0, 0.0], true),
        ([0.0, 0.0, 0.0, f32::from_bits(0xFFC0_1234)], true),
    ];
    for (ent, es_raro) in casos {
        let banco: Vec<u8> = ent.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
        let mut m = Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
        correr(&x.codigo, &mut m).unwrap();
        let cpu = termometro(&ent.map(f32::to_bits));
        assert_eq!(m.r[t], cpu, "{ent:?}: el de la 3060 y el de la CPU");
        assert_eq!(raro(m.r[t]), es_raro, "{ent:?}");
        if !es_raro {
            assert_eq!(m.r[t], 0, "{ent:?}: lo normal es +0");
        }
    }
}

/// *** NO: un termometro mal emitido (el FMUL por 1 en vez de por 0) no
/// pasa la comprobacion, y lo dice.
#[test]
fn un_termometro_mal_emitido_no_pasa() {
    let (en, _) = cubo();
    let mut x = emitir_libreta(&en.ps, 64, Abi::Registros, true).unwrap();
    let k = x.codigo.len() - 2;
    let (lo, hi) = x.codigo[k];
    assert_eq!(lo & 0x1FF, 0x020, "el FMUL, justo antes del EXIT");
    x.codigo[k] = ((lo & 0xFFFF_FFFF) | (1.0f32.to_bits() as u64) << 32, hi);
    let m = comprobar(&en.ps, &x, "de pixel").unwrap_err();
    assert!(m.contains("termometro"), "{m}");
}

/// *** El .bsf lleva el termometro (ida y vuelta), y uno sin libreta sigue
/// saliendo sin el.
#[test]
fn el_termometro_va_y_vuelve_por_el_bsf() {
    let (en, _) = cubo();
    let (v, p) = (emitir_libreta(&en.vs, 64, Abi::Registros, true).unwrap(), emitir_libreta(&en.ps, 64, Abi::Registros, true).unwrap());
    let m = mapa(&en);
    let (v2, p2) = de_bsf(&a_bsf(&m, &v, &p), &m).unwrap();
    assert_eq!((v2.termometro, v2.precargas, v2.codigo), (v.termometro, v.precargas, v.codigo));
    assert_eq!((p2.termometro, p2.precargas), (p.termometro, p.precargas));
    let (s, t) = (emitir_con(&en.vs, 64, Abi::Registros).unwrap(), emitir_con(&en.ps, 64, Abi::Registros).unwrap());
    let (s2, t2) = de_bsf(&a_bsf(&m, &s, &t), &m).unwrap();
    assert_eq!((s2.termometro, t2.termometro), (None, None));
}

/// *** LA PUERTA APRENDE DE LA LIBRETA: el vigia revisa el primer lote de
/// un PSO y luego uno de cada 256; si el kernel dice que la libreta apunto
/// en un dibujo de ese PSO, su siguiente lote se revisa YA. Un PSO que no
/// es de la puerta no cuenta. Y la receta lleva los termometros.
#[test]
fn lo_apuntado_se_revisa_en_el_siguiente_lote() {
    let (en, ia) = cubo();
    let (otro, _) = cubo();
    let paso = 40usize;
    let vertices: Vec<u8> = (0..3 * paso / 4).flat_map(|k| (k as f32 * 0.125).to_bits().to_le_bytes()).collect();
    let cb = std::vec![0u8; 16 * en.vs.filas_cb.max(en.ps.filas_cb) as usize];
    let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0], tijera: [0, 0, 1280, 720], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: &en, entradas: &ia, vertices: &vertices, paso, ids: &[0, 1, 2], topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let b = Blanco { va: 0x4000_0000, ancho: 1280, alto: 720, bgra: false, cadena: false };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, b).expect("el cubo va a la 3060");
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).unwrap();
    assert!(r.termometro_vs.is_some() && r.termometro_ps.is_some(), "la receta lleva los dos termometros");
    assert_eq!(p.sin_libreta, 0, "y cabe con ellos");
    p.preparar(&l, b).unwrap();
    assert_eq!(p.revisados, 1, "el primero, y el segundo no (uno de cada 256)");
    assert!(!p.apunto(&Lote { enlace: &otro, ..l }), "un PSO que no es de la puerta");
    p.preparar(&l, b).unwrap();
    assert_eq!(p.revisados, 1);
    assert!(p.apunto(&l), "el cubo");
    p.preparar(&l, b).unwrap();
    assert_eq!((p.revisados, p.apuntados, p.corregidos), (2, 1, 0), "revisado YA, y cuadra");
    p.preparar(&l, b).unwrap();
    assert_eq!(p.revisados, 2, "y vuelve a su turno");
}
