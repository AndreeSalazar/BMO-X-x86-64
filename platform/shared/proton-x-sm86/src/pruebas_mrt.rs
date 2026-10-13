//! ** LI2d (12-10): VARIOS RENDER TARGETS por la 3060, sin Z -- el G-buffer.
//! Hasta donde llega el banco (sin el metal): `gbuffer.hlsl` de dxc
//! (SV_Target0, 1 y 3: el 2 se salta) cruza la puerta, la receta y el pegado
//! del kernel con su juez; las ordenes ponen cada salida en SU ranura; y el
//! simulador de la 3060 da, en las tres salidas, los bits de la CPU.

extern crate std;

use alloc::vec::Vec;

use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, ElementoIa, Lote, Topologia};

use crate::puerta::{Blanco, Mas, Puerta, SIN_MAS};
use crate::{emitir_con, Abi};

const TEX_VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const GBUFFER_PS: &[u8] = include_bytes!("../../proton-x/prueba/gbuffer.dxil");

fn enlace() -> (lote::Enlace, Vec<ElementoIa>) {
    let e = |s: &str, desde| ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde, por_instancia: None };
    let ia = std::vec![e("POSITION", 0), e("TEXCOORD", 16)];
    (lote::enlazar(&dxil::leer(TEX_VS).unwrap(), &dxil::leer(GBUFFER_PS).unwrap(), &ia).unwrap(), ia)
}

/// Un triangulo (32 B por vertice: la posicion y la uv en float4).
fn vertices() -> Vec<u8> {
    [[-1.0f32, -1.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0], [3.0, -1.0, 0.5, 1.0, 2.0, 0.0, 0.0, 0.0], [-1.0, 3.0, 0.5, 1.0, 0.0, 2.0, 0.0, 0.0]].iter().flatten().flat_map(|f| f.to_bits().to_le_bytes()).collect()
}

const ANCHO: u32 = 64;
const ALTO: u32 = 16;

fn blanco(mas: crate::puerta::MasDelLote) -> Blanco {
    Blanco { va: 0x4000_0000, ancho: ANCHO, alto: ALTO, bgra: false, cadena: false, flotante: None, mas }
}

fn preparar(en: &lote::Enlace, ia: &[ElementoIa], v: &[u8], b: Blanco) -> Result<(Puerta, usize), alloc::string::String> {
    let cb = std::vec![0u8; 16];
    let reglas = bmo_proton_x::trama::Reglas { viewport: [0.0, 0.0, ANCHO as f32, ALTO as f32, 0.0, 1.0], tijera: [0, 0, ANCHO as i32, ALTO as i32], descarte: 1, antihorario: false, profundidad: None, mezcla: bmo_proton_x::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: None };
    let l = Lote { enlace: en, entradas: ia, vertices: v, paso: 32, ids: &[0, 1, 2], topologia: Topologia::Lista, cb: &cb, reglas, limpiar_z: None, limpiar_rt: None, recursos: bmo_proton_x::textura::Recursos::NINGUNO, oclusion: false, otros: &[], instancias: 1, primera_instancia: 0, base_vertice: 0, uavs: None };
    let mut p = Puerta::nueva();
    let n = p.preparar(&l, b)?;
    Ok((p, n))
}

/// *** De punta a punta: el 1 a la ranura 1 y el 3 a la ranura 2 (en la 3060
/// los huecos de D3D no existen), cada uno con su orden de bytes; la SPH con
/// las tres salidas; el kernel lo pega y lo juzga; las ordenes ponen cada
/// ranura; y el simulador da las tres salidas con los bits de la CPU.
#[test]
fn el_g_buffer_de_dxc_va_a_la_3060() {
    let (en, ia) = enlace();
    assert_eq!(en.objetivos, [0, 1, 3]);
    let v = vertices();
    let mut mas = SIN_MAS;
    mas[0] = Some(Mas { va: 0x4100_0000, bgra: false, flotante: None });
    mas[2] = Some(Mas { va: 0x4200_0400, bgra: true, flotante: None });
    let (p, n) = preparar(&en, &ia, &v, blanco(mas)).unwrap_or_else(|x| panic!("{x}"));
    let r = bmo_gpu_ga10x::receta::leer(&p.caja[..n]).expect("el kernel la lee");
    let d = r.dibujo;
    assert_eq!(d.objetivos(), 3);
    let (va1, d1) = d.mas()[0];
    let (va2, d2) = d.mas()[1];
    assert_eq!((va1, d1.ancho, d1.alto, d1.rgb), (0x4100_0000, ANCHO, ALTO, true), "la ranura 1: el SV_Target1, R8G8B8A8");
    assert_eq!((va2, d2.rgb), (0x4200_0400, false), "la ranura 2: el SV_Target3, en BGRA y a mitad de pagina");
    // El kernel: pega, juzga, y la SPH dice las tres salidas.
    let mut t = alloc::boxed::Box::new(bmo_gpu_ga10x::receta::Taller::nuevo());
    bmo_gpu_ga10x::receta::pegar(&r, &mut t).unwrap_or_else(|e| panic!("{e:?}"));
    let sph18 = u32::from_le_bytes(t.ps[72..76].try_into().unwrap());
    assert_eq!(sph18 & 0xFFFF, 0x0FFF, "OmapTarget de tres");
    // Las ordenes: cada ranura donde la deja la IOMMU, y el CT_SELECT.
    let q = bmo_gpu_ga10x::receta::paquete(&r, &t, 1);
    let gop = bmo_gpu_ga10x::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let o = bmo_gpu_ga10x::tuberia::ordenes_dibujo(&bmo_gpu_ga10x::cubo::ventana(&gop).unwrap(), 1, true, q.dibujo);
    let w = &o.o[..o.n];
    for (k, va) in [(1u32, va1), (2, va2)] {
        let cab = bmo_gpu_ga10x::copia::cabecera_en(0, bmo_gpu_ga10x::tresde::set_color_target_a(k), 8);
        let i = w.iter().position(|&x| x == cab).unwrap_or_else(|| panic!("la ranura {k}"));
        let en_3060 = bmo_gpu_ga10x::destino::va_de(k as usize) + va % 4096;
        assert_eq!(&w[i + 1..i + 3], &[(en_3060 >> 32) as u32, en_3060 as u32], "la ranura {k}");
    }
    let sel = bmo_gpu_ga10x::copia::cabecera_en(0, bmo_gpu_ga10x::tresde::SET_CT_SELECT, 1);
    assert_eq!(w[w.iter().rposition(|&x| x == sel).unwrap() + 1], bmo_gpu_ga10x::tresde::ct_select(3));
    // El simulador de la 3060 contra la CPU, en las TRES salidas.
    let e = emitir_con(&en.ps, crate::puerta::REGISTROS, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    crate::vivo::comprobar(&en.ps, &e, "gbuffer").unwrap_or_else(|x| panic!("{x}"));
}

/// *** Lo que no va, cada uno con su porque.
#[test]
fn lo_que_el_g_buffer_no_lleva_todavia() {
    let (en, ia) = enlace();
    let v = vertices();
    let todos = |m1: Option<Mas>, m3: Option<Mas>| {
        let mut mas = SIN_MAS;
        mas[0] = m1;
        mas[2] = m3;
        mas
    };
    let bien = Some(Mas { va: 0x4100_0000, bgra: false, flotante: None });
    let m = preparar(&en, &ia, &v, blanco(todos(bien, None))).err().unwrap();
    assert!(m.contains("render target 3 es un RTV nulo"), "{m}");
    // Un RGBA16F en el 1: el pegamento redondea solo el 0.
    let half = Some(Mas { va: 0x4100_0000, bgra: false, flotante: Some((10, 4)) });
    let m = preparar(&en, &ia, &v, blanco(todos(half, bien))).err().unwrap();
    assert!(m.contains("se redondea"), "{m}");
    // Un RGBA32F en el 1 (sin redondeo) SI.
    let f32x4 = Some(Mas { va: 0x4100_0000, bgra: false, flotante: Some((2, 4)) });
    assert!(preparar(&en, &ia, &v, blanco(todos(f32x4, bien))).is_ok());
    // Fuera de 128 B.
    let torcido = Some(Mas { va: 0x4100_0010, bgra: false, flotante: None });
    let m = preparar(&en, &ia, &v, blanco(todos(torcido, bien))).err().unwrap();
    assert!(m.contains("render target 1"), "{m}");
}
