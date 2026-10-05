//! Las pruebas de N5.5 (05-10): el COMPUTO de D3D12, del sombreador de `dxc`
//! a un `Dispatch` corrido en la CPU.

use alloc::vec;
use alloc::vec::Vec;

use crate::bufer::{Bufer, Modo, Uav};
use crate::dxil::{self, programa};
use crate::textura::Recursos;

const COMPUTO: &[u8] = include_bytes!("../prueba/computo.dxil");

/// *** `computo.hlsl` (de `dxc`, cs_6_0, [numthreads(64, 1, 1)]): cada hilo
/// copia su elemento del SRV a la memoria COMPARTIDA del grupo, espera en la
/// barrera, y escribe en el UAV el de su ESPEJO dentro del grupo (63 - gi)
/// por `escala`, con `g * 1000 + gi` en la w. Cuatro grupos (256 hilos) y
/// `n` = 250: los 6 ultimos no escriben. Cada valor es el exacto de HLSL
/// (un producto por 2 y enteros chicos: sin redondeo que discutir). Sin la
/// barrera, o sin memoria compartida de verdad, el espejo leeria un hueco.
#[test]
fn un_dispatch_con_memoria_compartida_y_barrera_da_lo_de_hlsl() {
    let s = dxil::leer(COMPUTO).unwrap();
    assert_eq!(s.etapa, dxil::Etapa::Computo);
    assert_eq!(s.hilos, [64, 1, 1], "numthreads, de la PSV0");
    let p = programa::compilar(&s).unwrap();
    assert_eq!(p.computo, programa::Computo { hilos: [64, 1, 1], compartida: 256 }, "64 float4 compartidos: 256 palabras");
    assert_eq!(p.ranuras.uavs.len(), 1);
    assert!(p.ops.iter().any(|o| matches!(o, programa::Op::Barrera)));

    let entrada: Vec<u8> = (0..256u32).flat_map(|k| [k as f32, k as f32 + 0.5, -(k as f32), 7.0]).flat_map(f32::to_le_bytes).collect();
    let srv = [Some(Bufer { bytes: &entrada, formato: 0, paso: 16, elementos: 256 })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let mut salida: Vec<u8> = (0..256 * 4).flat_map(|_| (-1.0f32).to_le_bytes()).collect();
    let mut cb = [0u8; 16];
    cb[..4].copy_from_slice(&250u32.to_le_bytes());
    cb[4..8].copy_from_slice(&2.0f32.to_le_bytes());
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 256 })];
        assert_eq!(p.despachar([4, 1, 1], &cb, &rec, &mut uavs), 256, "4 grupos de 64 hilos");
    }
    let f = |k: usize, c: usize| f32::from_le_bytes(salida[k * 16 + c * 4..k * 16 + c * 4 + 4].try_into().unwrap());
    let mut mal = vec![];
    for k in 0..256usize {
        let (g, gi) = (k / 64, k % 64);
        let espejo = (g * 64 + 63 - gi) as f32;
        let esperado = if k < 250 { [espejo * 2.0, (espejo + 0.5) * 2.0, -espejo * 2.0, (g * 1000 + gi) as f32] } else { [-1.0; 4] };
        let visto = [f(k, 0), f(k, 1), f(k, 2), f(k, 3)];
        if visto != esperado {
            mal.push((k, visto, esperado));
        }
    }
    assert!(mal.is_empty(), "{} mal; los primeros: {:?}", mal.len(), &mal[..mal.len().min(4)]);
}

/// Y el mismo con UN grupo de mas de los que hay datos: lo que lee fuera del
/// SRV es 0 (D3D12), y su espejo lo escribe.
#[test]
fn leer_fuera_del_srv_da_cero_en_el_computo() {
    let p = programa::compilar(&dxil::leer(COMPUTO).unwrap()).unwrap();
    let entrada: Vec<u8> = (0..64u32).flat_map(|k| [k as f32, 1.0, 1.0, 1.0]).flat_map(f32::to_le_bytes).collect();
    let srv = [Some(Bufer { bytes: &entrada, formato: 0, paso: 16, elementos: 64 })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let mut salida = vec![0u8; 128 * 16];
    let mut cb = [0u8; 16];
    cb[..4].copy_from_slice(&128u32.to_le_bytes());
    cb[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 128 })];
    p.despachar([2, 1, 1], &cb, &rec, &mut uavs);
    let x = |k: usize| f32::from_le_bytes(uavs[0].as_ref().unwrap().bytes[k * 16..k * 16 + 4].try_into().unwrap());
    assert_eq!(x(0), 63.0, "grupo 0: el espejo de 0 es 63");
    assert_eq!(x(64), 0.0, "grupo 1: su espejo esta fuera de los 64 del SRV: 0");
    let _ = Modo::Estructurado;
}

/// La textura elegida con un indice dinamico (N5.4, `EligeTextura`) ANTES de
/// una barrera sigue elegida DESPUES: el hilo se para en la barrera y vuelve
/// a buscar la misma (registro 7 del rango 0). Hecho a mano: dos hilos,
/// cada uno escribe en su elemento del UAV el texel que lee tras la barrera.
/// Probado que dice NO: con la elegida olvidada en la barrera, lee ceros.
#[test]
fn la_textura_elegida_antes_de_la_barrera_sigue_elegida_despues() {
    use crate::textura::{Clase, Como, Dinamicas, Textura};
    use programa::{Computo, Lectura, Lugar, Op, Programa, Ranuras, DINAMICA};
    static TEXEL: [u32; 1] = [0x4080_C0FF]; // R 0xFF, G 0xC0, B 0x80, A 0x40
    let l = Lugar { espacio: 0, registro: 0, vista: 0 };
    let mut iniciales = vec![0.0f32; 8];
    iniciales[1] = f32::from_bits(7);
    let p = Programa {
        ops: vec![
            Op::IdHilo { d: 0, que: 3, c: 0 },
            Op::EligeTextura { i: 1, rango: 0 },
            Op::Barrera,
            Op::Lee { d: 3, t: DINAMICA, s: 0, como: Lectura::Carga { enteros: false }, c: [2, 2, 2, 2], nivel: 2, desp: [0; 3] },
            Op::EscribeUav { u: 0, modo: Modo::Estructurado, i: 0, desp: 2, v: [3, 4, 5, 6], mascara: 0xF },
        ],
        iniciales,
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 0,
        ranuras: Ranuras { dinamicas: vec![l], uavs: vec![l], ..Ranuras::default() },
        computo: Computo { hilos: [2, 1, 1], compartida: 0 },
    };
    let buscar = |rango: u8, registro: u32| {
        (rango == 0 && registro == 7).then_some(Textura {
            texeles: &TEXEL,
            ancho: 1,
            alto: 1,
            como: Como::Rgba8,
            srgb: false,
            mapeo: Textura::MAPEO,
            mips: 1,
            capas: 1,
            hondo: 1,
            clase: Clase::Plana,
            mip: 0,
            capa: 0,
        })
    };
    let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: Some(Dinamicas(&buscar)) };
    let mut salida = vec![0u8; 2 * 16];
    let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 2 })];
    assert_eq!(p.despachar([1, 1, 1], &[], &rec, &mut uavs), 2);
    let quiero: Vec<u8> = [1.0f32, 192.0 / 255.0, 128.0 / 255.0, 64.0 / 255.0].iter().flat_map(|f| f.to_le_bytes()).collect();
    assert_eq!(&salida[..16], &quiero[..], "hilo 0: el texel de la textura 7");
    assert_eq!(&salida[16..], &quiero[..], "hilo 1: igual");
}

/// *** El CS de nBodyGravity (Microsoft, MIT; `prueba/muestras/nbody/`) se
/// compila: sus 384 lecturas de la memoria compartida desenrolladas son
/// `getelementptr` CONSTANTES (del bloque de constantes, no instrucciones:
/// antes, "un load de algo que no es un array"), y sus dos barreras van
/// dentro de un bucle.
#[test]
fn el_cs_de_nbody_se_compila_con_sus_getelementptr_constantes() {
    let p = dxil::computo::preparar(include_bytes!("../prueba/muestras/nbody/nBodyGravityCS.cso")).unwrap().programa;
    assert_eq!(p.computo, programa::Computo { hilos: [128, 1, 1], compartida: 512 }, "128 float4 compartidos");
    let lecturas = p.ops.iter().filter(|o| matches!(o, programa::Op::LeeCompartida { .. })).count();
    assert_eq!(lecturas, 384, "128 interacciones de 3 floats");
    let (bucle, barreras) = (p.ops.iter().position(|o| matches!(o, programa::Op::Bucle)).unwrap(), p.ops.iter().enumerate().filter(|(_, o)| matches!(o, programa::Op::Barrera)).map(|(i, _)| i).collect::<Vec<_>>());
    let fin = p.ops.iter().position(|o| matches!(o, programa::Op::FinBucle)).unwrap();
    assert_eq!(barreras.len(), 2);
    assert!(barreras.iter().all(|&b| bucle < b && b < fin), "las dos, dentro del bucle de los tiles");
}
