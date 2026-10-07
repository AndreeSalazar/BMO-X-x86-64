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
    assert_eq!(p.computo, programa::Computo { hilos: [64, 1, 1], compartida: 256, temprana: false }, "64 float4 compartidos: 256 palabras");
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
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 256, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
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
    let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 128, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
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
            Op::EscribeUav { u: 0, modo: Modo::Estructurado, i: 0, desp: 2, z: 2, v: [3, 4, 5, 6], mascara: 0xF },
        ],
        iniciales,
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 0,
        ranuras: Ranuras { dinamicas: vec![l], uavs: vec![l], ..Ranuras::default() },
        computo: Computo { hilos: [2, 1, 1], compartida: 0, temprana: false },
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
            niveles: u32::MAX,
            lod_min: 0.0,
            vista: None,
        })
    };
    let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: Some(Dinamicas(&buscar)) };
    let mut salida = vec![0u8; 2 * 16];
    let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 2, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
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
    assert_eq!(p.computo, programa::Computo { hilos: [128, 1, 1], compartida: 512, temprana: false }, "128 float4 compartidos");
    let lecturas = p.ops.iter().filter(|o| matches!(o, programa::Op::LeeCompartida { .. })).count();
    assert_eq!(lecturas, 384, "128 interacciones de 3 floats");
    let (bucle, barreras) = (p.ops.iter().position(|o| matches!(o, programa::Op::Bucle)).unwrap(), p.ops.iter().enumerate().filter(|(_, o)| matches!(o, programa::Op::Barrera)).map(|(i, _)| i).collect::<Vec<_>>());
    let fin = p.ops.iter().position(|o| matches!(o, programa::Op::FinBucle)).unwrap();
    assert_eq!(barreras.len(), 2);
    assert!(barreras.iter().all(|&b| bucle < b && b < fin), "las dos, dentro del bucle de los tiles");
}

/// *** El CS de culling de D3D12ExecuteIndirect (Microsoft, MIT;
/// `prueba/muestras/indirect/`) se compila: su `Append` es un
/// `bufferUpdateCounter` (+1, el indice de antes) y dos `bufferStore` en ese
/// indice (la orden indirecta de 24 bytes: la direccion del CBV y los
/// argumentos del Draw), dentro de un `si`.
#[test]
fn el_cs_de_execute_indirect_se_compila_con_su_append() {
    let p = dxil::computo::preparar(include_bytes!("../prueba/muestras/indirect/compute.cso")).unwrap().programa;
    assert_eq!(p.computo.hilos, [128, 1, 1]);
    let contadores: Vec<_> = p.ops.iter().filter_map(|o| if let programa::Op::Contador { u, inc, .. } = *o { Some((u, inc)) } else { None }).collect();
    assert_eq!(contadores, [(0, 1)], "un Append: sube el contador del UAV u0");
    assert_eq!(p.ops.iter().filter(|o| matches!(o, programa::Op::EscribeUav { u: 0, .. })).count(), 2, "24 bytes: 8 y 16");
    assert!(p.ops.iter().any(|o| matches!(o, programa::Op::Si { .. })));
}

/// *** El CS de culling de D3D12ExecuteIndirect CORRIDO: dos ordenes, la 0
/// con su triangulo en el centro (pasa) y la 1 corrida 100 en x (fuera del
/// plano de culling, 0.5): el `Append` escribe SOLO la 0, en el indice 0, y
/// el contador queda en 1. Con la proyeccion identidad, la cuenta del CS es
/// a mano: x de -0.05 a 0.05 (la 0) y de 99.95 a 100.05 (la 1).
#[test]
fn el_cs_de_execute_indirect_deja_pasar_solo_lo_que_cae_dentro() {
    let p = dxil::computo::preparar(include_bytes!("../prueba/muestras/indirect/compute.cso")).unwrap().programa;
    // `cbv` (t0): velocity, offset, color, projection, padding (256 B).
    let mut cbv = vec![0u8; 2 * 256];
    for (k, x) in [0.0f32, 100.0].into_iter().enumerate() {
        let o = k * 256;
        cbv[o + 16..o + 20].copy_from_slice(&x.to_le_bytes()); // offset.x
        for i in 0..4 {
            let m = o + 48 + 16 * i + 4 * i; // la identidad
            cbv[m..m + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
    }
    // `inputCommands` (t1): la direccion del CBV (uint2) y los argumentos del Draw (uint4).
    let ordenes: Vec<u8> = (0..2u32).flat_map(|k| [0x1000 + k, 0, 3, 1, 0, 0]).flat_map(u32::to_le_bytes).collect();
    let srv = [Some(Bufer { bytes: &cbv, formato: 0, paso: 256, elementos: 2 }), Some(Bufer { bytes: &ordenes, formato: 0, paso: 24, elementos: 2 })];
    // Las ranuras: cada SRV en la posicion de su lugar (t0 y t1).
    let pos = |reg: u32| p.ranuras.texturas.iter().position(|l| l.registro == reg).unwrap();
    let mut buf: [Option<Bufer>; 2] = [None, None];
    buf[pos(0)] = srv[0];
    buf[pos(1)] = srv[1];
    let rec = Recursos { texturas: &[None, None], muestreadores: &[], buferes: &buf, dinamicas: None };
    let cb: Vec<u8> = [0.05f32, 1.0, 0.5, 2.0].iter().flat_map(|f| f.to_le_bytes()).collect();
    let mut salida = vec![0xEEu8; 2 * 24];
    let mut contador = 0u32;
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 24, elementos: 2, contador: Some(&mut contador), rebanadas: crate::bufer::Rebanadas::PLANA })];
        p.despachar([1, 1, 1], &cb, &rec, &mut uavs);
    }
    assert_eq!(contador, 1, "pasa una");
    assert_eq!(&salida[..24], &ordenes[..24], "la 0, en el indice 0");
    assert_eq!(&salida[24..], &[0xEE; 24], "nada mas");
}

/// *** N5.3b: el PASO de cada bufer estructurado sale de `dx.resources`
/// (una vista en la raiz no lo lleva). Medido con lo que declaran sus HLSL:
/// el float4 de `computo.hlsl` (16), la Particle de nBodyGravity (dos
/// float4: 32) y las ordenes de ExecuteIndirect (uint2 + uint4: 24).
#[test]
fn el_paso_de_los_estructurados_sale_de_sus_metadatos() {
    let pasos = |cs: &[u8]| crate::dxil::computo::preparar(cs).unwrap().programa.ranuras.pasos;
    let mut c = pasos(include_bytes!("../prueba/computo.dxil"));
    c.sort_unstable();
    assert_eq!(c, [(false, 0, 0, 16), (true, 0, 0, 16)]);
    let mut n = pasos(include_bytes!("../prueba/muestras/nbody/nBodyGravityCS.cso"));
    n.sort_unstable();
    assert_eq!(n, [(false, 0, 0, 32), (true, 0, 0, 32)]);
    let i = pasos(include_bytes!("../prueba/muestras/indirect/compute.cso"));
    assert!(i.contains(&(false, 0, 1, 24)) && i.contains(&(true, 0, 0, 24)), "{i:?}");
}

/// **13 de la pila A (07-10): RawBufferLoad y RawBufferStore** (op 139 y
/// 140, SM 6.2), los de `crudo.hlsl` (de `dxc`, cs_6_2): Load y Load4 de un
/// ByteAddressBuffer, una fila de un StructuredBuffer, Store y Store2 en un
/// RWByteAddressBuffer y una fila de un RWStructuredBuffer. Cada valor es el
/// exacto de HLSL (enteros, y floats chicos sin redondeo que discutir); los
/// hilos de `n` en adelante no escriben.
#[test]
fn raw_buffer_load_y_store_dan_lo_de_hlsl() {
    let s = dxil::leer(include_bytes!("../prueba/crudo.dxil")).unwrap();
    let p = programa::compilar(&s).unwrap_or_else(|e| panic!("{e:?}"));
    let w = |k: u32| k.wrapping_mul(7).wrapping_add(1);
    let bytes: Vec<u8> = (0..160u32).flat_map(|k| w(k).to_le_bytes()).collect();
    let filas: Vec<u8> = (0..32u32).flat_map(|k| [k as f32, k as f32 + 0.25, 2.0 * k as f32, -0.5 * k as f32]).flat_map(f32::to_le_bytes).collect();
    // Las ranuras van en el orden del programa: t1 y t0, u1 y u0.
    assert_eq!(p.ranuras.texturas.iter().map(|l| l.registro).collect::<Vec<_>>(), [1, 0]);
    assert_eq!(p.ranuras.uavs.iter().map(|l| l.registro).collect::<Vec<_>>(), [1, 0]);
    let srv = [Some(Bufer { bytes: &filas, formato: 0, paso: 16, elementos: 32 }), Some(Bufer { bytes: &bytes, formato: 0, paso: 4, elementos: 160 })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let mut salida = vec![0xFFu8; 512];
    let mut pares = vec![0xFFu8; 32 * 8];
    let mut cb = [0u8; 16];
    cb[..4].copy_from_slice(&20u32.to_le_bytes());
    {
        let mut uavs = [
            Some(Uav { bytes: &mut pares, formato: 0, paso: 8, elementos: 32, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA }),
            Some(Uav { bytes: &mut salida, formato: 0, paso: 4, elementos: 128, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA }),
        ];
        assert_eq!(p.despachar([1, 1, 1], &cb, &rec, &mut uavs), 32);
    }
    let u = |b: &[u8], k: usize| u32::from_le_bytes(b[4 * k..4 * k + 4].try_into().unwrap());
    for i in 0..32u32 {
        let k = i as usize;
        let (a, bx, by, bz, bw) = (w(i), w(32 + 4 * i), w(33 + 4 * i), w(34 + 4 * i), w(35 + 4 * i));
        let (fx, fy, fw) = (i as f32, i as f32 + 0.25, -0.5 * i as f32);
        if i < 20 {
            assert_eq!(u(&salida, k), a.wrapping_mul(3).wrapping_add(1), "Store del hilo {i}");
            assert_eq!([u(&salida, 64 + 2 * k), u(&salida, 65 + 2 * k)], [bx ^ bw, (fy * 2.0).to_bits()], "Store2 del hilo {i}");
            assert_eq!([u(&pares, 2 * k), u(&pares, 2 * k + 1)], [by.wrapping_add(bz), (fx + fw).to_bits()], "la fila del hilo {i}");
        } else {
            assert_eq!([u(&salida, k), u(&salida, 64 + 2 * k), u(&pares, 2 * k)], [u32::MAX; 3], "el hilo {i} no escribe");
        }
    }
}

/// **18 de la pila A (07-10): IMad, UMad y los Interlocked de la memoria
/// COMPARTIDA** (`atomicrmw` de LLVM, la instruccion 38), los de
/// `compartido.hlsl` (de `dxc`, cs_6_0): dos grupos de 64. Lo que quedo en
/// cada contador del grupo; los `mad()` de cada hilo; los "antes" de la
/// suma, todos distintos y el mayor mas lo suyo es la suma; y UNO solo vio
/// el cambio sin hacer. Nada de eso depende del orden de los hilos.
#[test]
fn imad_umad_y_los_interlocked_de_la_compartida_dan_lo_de_hlsl() {
    let s = dxil::leer(include_bytes!("../prueba/compartido.dxil")).unwrap();
    let p = programa::compilar(&s).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(p.ops.iter().any(|o| matches!(o, programa::Op::AtomicoCompartido { .. })));
    let mut salida = vec![0xEEu8; 2 * 65 * 16];
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 130, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
        let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &[], dinamicas: None };
        assert_eq!(p.despachar([2, 1, 1], &[], &rec, &mut uavs), 128);
    }
    let fila = |k: usize| -> [u32; 4] { core::array::from_fn(|c| u32::from_le_bytes(salida[16 * k + 4 * c..16 * k + 4 * c + 4].try_into().unwrap())) };
    for g in 0..2u32 {
        let w = |gi: u32| gi * 5 + g * 1000 + 1;
        let suma: u32 = (0..64).map(w).sum();
        assert_eq!(fila(g as usize * 65 + 64), [suma, (200 - 3 * 63) as u32, 33, u32::MAX], "lo que quedo en el grupo {g}");
        let mut antes = Vec::new();
        let mut vieron_cero = 0;
        for gi in 0..64u32 {
            let f = fila((g * 65 + gi) as usize);
            assert_eq!([f[0], f[1]], [(200 - 3 * gi as i32) as u32, w(gi)], "mad() del hilo {gi}");
            antes.push((f[2], w(gi)));
            vieron_cero += f[3];
        }
        antes.sort_unstable();
        assert!(antes.windows(2).all(|x| x[0].0 < x[1].0), "cada InterlockedAdd vio otro antes");
        let (a, wa) = antes[63];
        assert_eq!(a + wa, suma, "el ultimo antes mas lo suyo es la suma");
        assert_eq!(vieron_cero, 1, "uno solo vio el InterlockedExchange sin hacer");
    }
}

/// **19 de la pila A (07-10): un array COMPARTIDO de structs y uno de
/// vectores**, los de `estructuras.hlsl` (de `dxc`, cs_6_0): dxc los aplana
/// (`[192 x float]`, `[64 x i32]`, `[128 x float]`); cada hilo lee el de su
/// espejo tras la barrera. Lo que no aplana dxc (un struct o un vector que
/// llega entero) lo juzga `dxil::arreglos::pruebas`.
#[test]
fn un_array_compartido_de_structs_y_de_vectores_da_lo_de_hlsl() {
    let s = dxil::leer(include_bytes!("../prueba/estructuras.dxil")).unwrap();
    let p = programa::compilar(&s).unwrap_or_else(|e| panic!("{e:?}"));
    let mut salida = vec![0u8; 64 * 16];
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 64, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
        let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &[], dinamicas: None };
        assert_eq!(p.despachar([1, 1, 1], &[], &rec, &mut uavs), 64);
    }
    for gi in 0..64usize {
        let q = (63 - gi) as f32;
        let f: [f32; 4] = core::array::from_fn(|c| f32::from_le_bytes(salida[16 * gi + 4 * c..16 * gi + 4 * c + 4].try_into().unwrap()));
        assert_eq!(f, [q + q * 0.5, -q, ((63 - gi) * 3 + 1) as f32, (q + 0.25) * (100.0 - q)], "el hilo {gi}");
    }
}

/// El bucle de `salidas.hlsl`, en Rust: por donde salio (1 lo encontro, 2
/// la suma se paso, 3 acabo), en que vuelta, con que suma, y lo encontrado.
fn salidas_esperado(datos: &[u32], gi: u32, buscado: u32, tope: u32) -> [u32; 4] {
    let mut suma = 0u32;
    let mut k = 0;
    while k < datos.len() {
        let v = datos[k] ^ gi;
        if v == buscado {
            return [1, k as u32, suma, v];
        }
        suma = suma.wrapping_add(v);
        if suma > tope {
            break;
        }
        k += 1;
    }
    [if suma > tope { 2 } else { 3 }, k as u32, suma, 0]
}

/// **16 y 21 de la pila A (07-10): un bucle con TRES salidas** -- su
/// condicion, un `break` y un `return` --, el de `salidas.hlsl` (de `dxc`):
/// el CS (64 hilos) y el de pixeles (64 pixeles) dan lo del bucle en Rust,
/// y salen por las tres. Antes: "un bucle con mas de una salida: todavia
/// no" (la prueba que dice NO, con el estructurador de antes).
#[test]
fn un_bucle_con_tres_salidas_da_lo_de_hlsl_en_el_cs_y_en_el_de_pixeles() {
    let datos: Vec<u32> = (0..32u32).map(|k| (k * 37 + 11) & 63).collect();
    let (buscado, tope) = (5u32, 1000u32);
    let bytes: Vec<u8> = datos.iter().flat_map(|v| v.to_le_bytes()).collect();
    let srv = [Some(Bufer { bytes: &bytes, formato: 0, paso: 4, elementos: 32 })];
    let rec = Recursos { texturas: &[None], muestreadores: &[], buferes: &srv, dinamicas: None };
    let mut cb = [0u8; 16];
    for (k, v) in [32u32, buscado, tope].iter().enumerate() {
        cb[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
    let esperado: Vec<[u32; 4]> = (0..64).map(|gi| salidas_esperado(&datos, gi, buscado, tope)).collect();
    let mut vistas = [false; 4];
    for e in &esperado {
        vistas[e[0] as usize] = true;
    }
    assert_eq!(vistas, [false, true, true, true], "el juez sale por las tres");
    // El CS.
    let cs = programa::compilar(&dxil::leer(include_bytes!("../prueba/salidas_cs.dxil")).unwrap()).unwrap_or_else(|e| panic!("{e:?}"));
    let mut salida = vec![0xEEu8; 64 * 16];
    {
        let mut uavs = [Some(Uav { bytes: &mut salida, formato: 0, paso: 16, elementos: 64, contador: None, rebanadas: crate::bufer::Rebanadas::PLANA })];
        assert_eq!(cs.despachar([1, 1, 1], &cb, &rec, &mut uavs), 64);
    }
    for gi in 0..64usize {
        let f: [u32; 4] = core::array::from_fn(|c| u32::from_le_bytes(salida[16 * gi + 4 * c..16 * gi + 4 * c + 4].try_into().unwrap()));
        assert_eq!(f, esperado[gi], "el hilo {gi} del CS");
    }
    // El de pixeles: la x del pixel es el xor; lo que sale, en floats.
    let ps = programa::compilar(&dxil::leer(include_bytes!("../prueba/salidas_ps.dxil")).unwrap()).unwrap_or_else(|e| panic!("{e:?}"));
    let mut regs = Vec::new();
    for gi in 0..64u32 {
        let mut sal = vec![[0.0f32; 4]; ps.salidas];
        ps.correr_con(&[[gi as f32 + 0.5, 0.5, 0.5, 1.0]], &cb, &rec, &mut sal, &mut regs);
        let e = esperado[gi as usize];
        assert_eq!(sal[0], [e[0] as f32, e[1] as f32, e[2] as f32, e[3] as f32], "el pixel {gi}");
    }
}
