//! **A10 (06-10): los vertices y los pixeles que tocan UAV, TRADUCIDOS**
//! (`nativo::compilar`: cada operacion de UAV por la llamada de la casa,
//! `dibujo_sysv`, a `operar_uav` del interprete), contra el interprete
//! (`correr_con_uavs`), BIT A BIT: lo que sale, si el pixel queda y lo que
//! queda en los cuatro UAV DESPUES DE CADA UNO. Los de `uavorden.hlsl`
//! dependen del ORDEN: cada pixel ve lo que dejaron los de antes (el
//! contador, la lista, el lienzo de float que se lee y se escribe, la tabla
//! con tipo), y un `discard` a medias deja lo de antes de el.

use core::cell::RefCell;

use super::saltos::{de_dxc, raros, Azar};
use bmo_proton_x::bufer::{Rebanadas, Uav};
use bmo_proton_x::dxil::programa::Programa;
use bmo_proton_x::nativo;
use bmo_proton_x::nativo_llamadas::{Llamadas, Muestras};
use bmo_proton_x::textura::Recursos;
use bmo_proton_x_casa::nativo::{llamadas_dibujo, LlamadoDibujo};

const PS: &[u8] = include_bytes!("../../../proton-x/prueba/uavorden_ps.dxil");
const VS: &[u8] = include_bytes!("../../../proton-x/prueba/uavorden_vs.dxil");

/// El de cinco punteros (las `Llamadas`: las lee el que toca UAV).
type Sombreador = extern "sysv64" fn(*mut f32, *const [f32; 4], *const u8, *mut [f32; 4], *const Llamadas) -> u32;

/// Los cuatro UAV de `uavorden.hlsl` (u0..u3) en las ranuras de `p`,
/// empezando con los mismos bytes al azar.
fn uavs(p: &Programa, semilla: u64) -> RefCell<Vec<Option<Uav<'static>>>> {
    let mut z = Azar(semilla);
    let mut bytes = |n: usize| -> &'static mut [u8] { Vec::leak((0..n).map(|_| z.siguiente() as u8).collect()) };
    let plana = Rebanadas::PLANA;
    let todos = [
        Uav { bytes: bytes(16), formato: 0, paso: 0, elementos: 4, contador: None, rebanadas: plana },
        Uav { bytes: bytes(16 * 8), formato: 0, paso: 8, elementos: 16, contador: None, rebanadas: plana },
        // RGBA32F de 8 x 8.
        Uav { bytes: bytes(64 * 16), formato: 2, paso: 8, elementos: 64, contador: None, rebanadas: plana },
        // R32_UINT.
        Uav { bytes: bytes(64 * 4), formato: 42, paso: 0, elementos: 64, contador: None, rebanadas: plana },
    ];
    let mut v: Vec<Option<Uav<'static>>> = (0..p.ranuras.uavs.len()).map(|_| None).collect();
    for (r, u) in todos.into_iter().enumerate() {
        if let Some(k) = p.ranuras.uavs.iter().position(|l| l.registro == r as u32) {
            v[k] = Some(u);
        }
    }
    RefCell::new(v)
}

/// Los bytes de todos los UAV, para comparar.
fn foto(u: &RefCell<Vec<Option<Uav<'static>>>>) -> Vec<Vec<u8>> {
    u.borrow().iter().map(|x| x.as_ref().map_or(Vec::new(), |x| x.bytes.to_vec())).collect()
}

/// `n` vertices o pixeles seguidos por los dos caminos, cada uno con sus
/// UAV (empiezan iguales): tras cada uno, lo mismo. Las entradas, raras o
/// al azar; `ent` las arregla (las del pixel quieren x, y en el lienzo).
fn en_orden(nombre: &str, p: &Programa, n: usize, ent: impl Fn(&mut Azar, &[f32]) -> Vec<[f32; 4]>) -> (usize, usize) {
    assert_eq!(nativo::por_que_no(p), None, "{nombre}: sin motivo para no traducirlo");
    let f: Sombreador = {
        let g = super::sellar(&nativo::compilar(p).unwrap_or_else(|| panic!("{nombre}: se traduce")));
        // SAFETY: la misma funcion; la firma de cinco es la de verdad.
        unsafe { core::mem::transmute::<super::Sombreador, Sombreador>(g) }
    };
    let (ua, ub) = (uavs(p, 0x5EED_0A10), uavs(p, 0x5EED_0A10));
    let rec = Recursos::NINGUNO;
    let mut llamado = LlamadoDibujo { muestras: Muestras { programa: p, recursos: &rec, elegida: None }, uavs: Some(&ub) };
    let ll = llamadas_dibujo(&mut llamado, 16);
    let cb = [0u8; 16];
    let raros = raros();
    let mut z = Azar(0xA10A_10A1_0A10_A10A ^ nombre.len() as u64);
    let (mut quedan, mut tirados) = (0, 0);
    for k in 0..n {
        let e = ent(&mut z, &raros);
        let (mut s1, mut s2) = (vec![[0.0f32; 4]; p.salidas], vec![[0.0f32; 4]; p.salidas]);
        let q1 = p.correr_con_uavs(&e, &cb, &rec, &mut s1, &mut Vec::new(), &mut ua.borrow_mut());
        let mut regs = p.iniciales.clone();
        let q2 = match f(regs.as_mut_ptr(), e.as_ptr(), cb.as_ptr(), s2.as_mut_ptr(), &ll) {
            nativo::QUEDA => true,
            nativo::DESCARTADO => false,
            otro => panic!("{nombre} {k}: devolvio {otro}"),
        };
        let bits = |s: &[[f32; 4]]| s.iter().map(|x| x.map(|v| if v.is_nan() { 0x7FC0_0000 } else { v.to_bits() })).collect::<Vec<_>>();
        assert_eq!(q1, q2, "{nombre} {k}: si queda (entradas {e:?})");
        if q1 {
            assert_eq!(bits(&s1), bits(&s2), "{nombre} {k}: lo que sale (entradas {e:?})");
            quedan += 1;
        } else {
            tirados += 1;
        }
        assert_eq!(foto(&ua), foto(&ub), "{nombre} {k}: lo que queda en los UAV (entradas {e:?})");
    }
    (quedan, tirados)
}

/// *** El de pixeles de `uavorden.hlsl`, 3000 seguidos: contador,
/// lista, lienzo de float leido y escrito, tabla con tipo, un `discard`
/// a medias, un bucle de `InterlockedMax` y `GetDimensions`.
#[test]
fn los_pixeles_con_uav_traducidos_dejan_los_bits_del_interprete() {
    let p = de_dxc(PS);
    assert!(p.toca_uav() && !p.usa_olas());
    let (quedan, tirados) = en_orden("uavorden PSOrden", &p, 3_000, |z, raros| {
        let x = (z.siguiente() % 9) as f32 + 0.5;
        let y = (z.siguiente() % 9) as f32 + 0.5;
        vec![[x, y, 0.5, 1.0], [z.valor(raros), z.valor(raros), z.valor(raros), z.valor(raros)]]
    });
    assert!(quedan > 500 && tirados > 500, "los dos caminos del discard: {quedan} quedan, {tirados} tirados");
}

/// *** El de vertices: un contador que suma y su elemento de la tabla.
#[test]
fn los_vertices_con_uav_traducidos_dejan_los_bits_del_interprete() {
    let p = de_dxc(VS);
    assert!(p.toca_uav());
    let (quedan, _) = en_orden("uavorden VSOrden", &p, 2_000, |z, raros| vec![[z.valor(raros), z.valor(raros), z.valor(raros), z.valor(raros)], [f32::from_bits(z.siguiente() as u32), 0.0, 0.0, 0.0]]);
    assert_eq!(quedan, 2_000);
}

/// *** Lo que sigue sin traducirse: un pixel que DERIVA y toca UAV (un
/// cuadro de 2x2 que se separa se rehace en el interprete, y lo escrito
/// quedaria dos veces). Lo dice, y `compilar_cuadros` no lo da.
#[test]
fn el_pixel_que_deriva_y_toca_uav_lo_dice() {
    use bmo_proton_x::dxil::olas;
    let mut p = de_dxc(PS);
    // Cuatro derivadas al principio, a registros nuevos (nadie las lee).
    let mut extra = Vec::new();
    olas::gradientes(&mut extra, p.iniciales.len() as _, 0, 1);
    p.iniciales.resize(p.iniciales.len() + 4, 0.0);
    p.ops.splice(0..0, extra);
    assert!(p.usa_olas());
    assert_eq!(nativo::por_que_no(&p), Some("deriva y lee o escribe un UAV (un cuadro de 2x2 que se separa no se puede rehacer)"));
    assert!(nativo::compilar_cuadros(&p).is_none());
}
