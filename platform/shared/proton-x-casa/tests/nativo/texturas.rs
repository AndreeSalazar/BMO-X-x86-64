//! **X2, la VELOCIDAD de los que MUESTREAN** (05-10): los sombreadores de
//! dibujo con texturas (`Sample`, `Load`, `Gather`, `SampleCmp`, los arrays
//! de texturas), con matematica (`sin`, `exp2`, los redondeos, los medios
//! floats...) o con un cbuffer de fila CALCULADA, traducidos por
//! `nativo::compilar`: el cuerpo LLAMA al Rust del interprete (la casa,
//! `textura_sysv`; `nativo_llamadas::mate_sysv`) y la fila se mira contra la
//! medida del cbuffer. Contra el interprete, BIT A BIT: lo que sale y si el
//! pixel queda, con texturas de 8 bits, de float con NaN, infinitos y -0, y
//! con mips, cada muestreador (punto, lineal, repetir, espejo, borde) y los
//! arrays de texturas buscados al correr.

use super::saltos::{raros, Azar};
use super::{igual, sellar};
use bmo_proton_x::bufer::Bufer;
use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::nativo;
use bmo_proton_x::nativo_llamadas::{Muestras, MATES};
use bmo_proton_x::textura::{Clase, Como, Dinamicas, Direccion, Filtro, Muestreador, Recursos, Textura};
use bmo_proton_x_casa::nativo::{llamadas, Sombreador};

const HDR_LEE: &[u8] = include_bytes!("../../../proton-x/prueba/hdr_lee.dxil");
const TEXTURA_PS: &[u8] = include_bytes!("../../../proton-x/prueba/textura_ps.dxil");
const FLOTANTE1_LEE: &[u8] = include_bytes!("../../../proton-x/prueba/flotante1_lee.dxil");
const VISTAS: &[u8] = include_bytes!("../../../proton-x/prueba/vistas.dxil");
const SOMBRAS: &[u8] = include_bytes!("../../../proton-x/prueba/sombras.dxil");
const ESPACIOS: &[u8] = include_bytes!("../../../proton-x/prueba/espacios.dxil");
const MATES_PS: &[u8] = include_bytes!("../../../proton-x/prueba/mates.dxil");
const LUCES: &[u8] = include_bytes!("../../../proton-x/prueba/luces.dxil");
const BUFERES: &[u8] = include_bytes!("../../../proton-x/prueba/buferes.dxil");
const GBUFFER: &[u8] = include_bytes!("../../../proton-x/prueba/gbuffer.dxil");
const HTEXTURE_PS: &[u8] = include_bytes!("../../../proton-x/prueba/muestras/htexture/shaders_PSMain.cso");
const DYNINDEX_PS: &[u8] = include_bytes!("../../../proton-x/prueba/muestras/dynindex/shader_mesh_dynamic_indexing_pixel.cso");
const NBODY_PS: &[u8] = include_bytes!("../../../proton-x/prueba/muestras/nbody/ParticleDraw_PS.cso");

/// Los que se juzgan: su nombre y sus bytes. Muestrean (Sample, Load,
/// SampleLevel, Gather, SampleCmp, desplazados, arrays de texturas por
/// indice, buferes), hacen matematica (mates, gbuffer, nbody) o leen una
/// fila calculada (luces).
const TODOS: [(&str, &[u8]); 13] = [
    ("hdr PSLee", HDR_LEE),
    ("textura_ps", TEXTURA_PS),
    ("flotante1_lee", FLOTANTE1_LEE),
    ("vistas", VISTAS),
    ("sombras", SOMBRAS),
    ("espacios", ESPACIOS),
    ("mates", MATES_PS),
    ("luces", LUCES),
    ("buferes", BUFERES),
    ("gbuffer", GBUFFER),
    ("htexture PSMain", HTEXTURE_PS),
    ("dynindex pixel", DYNINDEX_PS),
    ("nbody ParticleDraw_PS", NBODY_PS),
];

/// La llamada de los dibujos con sus `Llamadas` (la de la casa).
fn con_llamadas(f: super::Sombreador) -> Sombreador {
    // SAFETY: la misma funcion; la firma de cinco es la de verdad (el quinto
    // solo lo lee el que muestrea, hace matematica o lee la fila calculada).
    unsafe { core::mem::transmute::<super::Sombreador, Sombreador>(f) }
}

/// Las texturas del banco: de 8 bits al azar (RGBA, BGRA, sRGB con otro
/// mapeo), de float con los valores que muerden, una de un float, una con
/// tres mips y un array de dos capas. `'static`: las busca tambien
/// `Dinamicas`.
fn texturas() -> Vec<Option<Textura<'static>>> {
    let mut z = Azar(0x9E37_79B9_7F4A_7C15);
    let mut palabras = |n: usize| -> &'static [u32] { Box::leak((0..n).map(|_| z.siguiente() as u32).collect::<Vec<_>>().into_boxed_slice()) };
    let raros = raros();
    let floats: &'static [u32] = Box::leak((0..4 * 5 * 3).map(|k| raros[k * 7 % raros.len()].to_bits()).collect::<Vec<_>>().into_boxed_slice());
    let uno: &'static [u32] = Box::leak((0..6 * 6).map(|k| raros[k * 5 % raros.len()].to_bits()).collect::<Vec<_>>().into_boxed_slice());
    let plana = |texeles, ancho, alto, como| Textura { texeles, ancho, alto, como, srgb: false, mapeo: Textura::MAPEO, mips: 1, capas: 1, hondo: 1, clase: Clase::Plana, mip: 0, capa: 0, niveles: u32::MAX, lod_min: 0.0 };
    vec![
        Some(Textura::rgba(palabras(8 * 8), 8, 8, false)),
        Some(plana(floats, 5, 3, Como::Flotantes4)),
        Some(Textura { mips: 3, ..Textura::rgba(palabras(8 * 8 + 4 * 4 + 2 * 2), 8, 8, true) }),
        Some(plana(uno, 6, 6, Como::Flotante)),
        Some(Textura { srgb: true, mapeo: 0x0A0B_1A, ..Textura::rgba(palabras(7 * 3), 7, 3, false) }),
        Some(Textura { capas: 2, clase: Clase::Array, ..Textura::rgba(palabras(2 * 4 * 4), 4, 4, false) }),
        None,
        Some(Textura::rgba(palabras(1), 1, 1, false)),
    ]
}

/// Los muestreadores: cada filtro y cada direccion, el borde de color y uno
/// de comparacion (menor o igual).
fn muestreadores() -> Vec<Option<Muestreador>> {
    let m = |filtro, u, v, comparacion| Some(Muestreador { filtro, u, v, borde: [0.25, -0.0, 1.0, f32::NAN], comparacion, lod: bmo_proton_x::textura::Lod::DE_SIEMPRE });
    vec![
        m(Filtro::Punto, Direccion::Repetir, Direccion::Repetir, 0),
        m(Filtro::Lineal, Direccion::Espejo, Direccion::Sujetar, 0),
        m(Filtro::Lineal, Direccion::Borde, Direccion::EspejoUnaVez, 0),
        m(Filtro::Punto, Direccion::Sujetar, Direccion::Borde, 4),
        None,
    ]
}

/// Los buferes de las mismas ranuras (un SRV es uno u otro: aqui hay de
/// los dos, para que cada lectura encuentre algo).
fn buferes() -> Vec<Option<Bufer<'static>>> {
    let mut z = Azar(0x0123_4567_89AB_CDEF);
    let bytes: &'static [u8] = Box::leak((0..256).map(|_| z.siguiente() as u8).collect::<Vec<_>>().into_boxed_slice());
    vec![Some(Bufer { bytes, formato: 2, paso: 16, elementos: 16 }), Some(Bufer { bytes: &bytes[..100], formato: 0, paso: 12, elementos: 9 }), None, Some(Bufer { bytes, formato: 0, paso: 0, elementos: 64 })]
}

/// Un caso por los dos caminos: el interprete con `rec`, el traducido con
/// sus llamadas sobre los mismos `rec`. `cb` tal cual (sin rellenar: quien
/// llama lo rellena si lee filas fijas).
fn un_caso(p: &Programa, f: Sombreador, ent: &[[f32; 4]], cb: &[u8], rec: &Recursos) -> Result<(), String> {
    let (mut s1, mut s2) = (vec![[0.0f32; 4]; p.salidas], vec![[0.0f32; 4]; p.salidas]);
    let queda1 = p.correr_con(ent, cb, rec, &mut s1, &mut Vec::new());
    let mut regs = p.iniciales.clone();
    let mut m = Muestras { programa: p, recursos: rec, elegida: None };
    let ll = llamadas(&mut m, cb.len());
    let cbp = if cb.is_empty() { [0u8; 16].as_ptr() } else { cb.as_ptr() };
    // SAFETY: `f` es la traduccion de `p`; todo lo que apunta, de aqui.
    let r = unsafe { f(regs.as_mut_ptr(), ent.as_ptr(), cbp, s2.as_mut_ptr(), &ll) };
    let queda2 = match r {
        nativo::QUEDA => true,
        nativo::DESCARTADO => false,
        otro => return Err(format!("devolvio {otro}")),
    };
    let bits = |v: &[[f32; 4]]| v.iter().map(|e| e.map(f32::to_bits)).collect::<Vec<_>>();
    if queda1 != queda2 {
        return Err(format!("queda: interpretado {queda1}, nativo {queda2} (entradas {:x?})", bits(ent)));
    }
    if queda1 && !s1.iter().zip(&s2).all(|(a, b)| (0..4).all(|k| igual(a[k], b[k]))) {
        return Err(format!("interpretado {:x?}, nativo {:x?} (entradas {:x?})", bits(&s1), bits(&s2), bits(ent)));
    }
    Ok(())
}

/// Un valor de entrada: un raro, unos bits cualesquiera, una coordenada de
/// textura (de -1.5 a 2.5, donde caen los texeles, los bordes y las
/// vueltas) o un entero chico (un `Load`, un indice de array).
fn valor(z: &mut Azar, raros: &[f32]) -> f32 {
    let x = z.siguiente();
    match x % 4 {
        0 | 1 => z.valor(raros),
        2 => ((x >> 16) as u32 % 4096) as f32 / 1024.0 - 1.5,
        _ => f32::from_bits((x >> 20) as u32 % 40),
    }
}

/// *** Cada sombreador que muestrea, hace matematica o lee una fila
/// calculada: se traduce (sin motivo que decir) y da los bits del
/// interprete en 20.000 casos al azar, con las texturas del banco, sus
/// arrays buscados al correr y el cbuffer al azar.
#[test]
fn los_que_muestrean_nativos_dan_los_bits_del_interprete() {
    let (tex, mue, buf) = (texturas(), muestreadores(), buferes());
    let tex_dinamicas = tex.clone();
    let buscar = move |rango: u8, registro: u32| -> Option<Textura<'static>> { tex_dinamicas.get((registro as usize + rango as usize) % 9).copied().flatten() };
    let rec = Recursos { texturas: &tex, muestreadores: &mue, buferes: &buf, dinamicas: Some(Dinamicas(&buscar)) };
    let raros = raros();
    for (nombre, d) in TODOS {
        let p = super::saltos::de_dxc(d);
        assert_eq!(nativo::por_que_no(&p), None, "{nombre}: sin motivo para no traducirlo");
        let f = con_llamadas(sellar(&nativo::compilar(&p).unwrap_or_else(|| panic!("{nombre}: se traduce"))));
        let mut z = Azar(0x2545_F491_4F6C_DD1D ^ nombre.len() as u64);
        // El cbuffer: lo que lean sus filas fijas (las calculadas, hasta 64
        // filas: lo de fuera, 0 en los dos).
        let fijas = p.ops.iter().filter_map(|o| if let Op::Constantes { fila, .. } = o { Some(*fila as usize + 1) } else { None }).max().unwrap_or(0);
        let filas = (p.filas_cb as usize).clamp(1, 64).max(fijas);
        let mut malos = Vec::new();
        for _ in 0..20_000 {
            let ent: Vec<[f32; 4]> = (0..p.entradas.max(1)).map(|_| [valor(&mut z, &raros), valor(&mut z, &raros), valor(&mut z, &raros), valor(&mut z, &raros)]).collect();
            let cb: Vec<u8> = (0..filas * 4).flat_map(|_| valor(&mut z, &raros).to_le_bytes()).collect();
            if let Err(m) = un_caso(&p, f, &ent, &cb, &rec) {
                malos.push(m);
            }
        }
        assert!(malos.is_empty(), "{nombre}: {} distintos de 20000; el primero: {}", malos.len(), malos[0]);
    }
}

/// Las funciones de `mates.rs`, una por salida, sobre la entrada x; y los
/// cbuffers con fila calculada por los bits de y: `(fila, filas)` con la
/// fila de base 0, 1 y 3 (tras el enlace, la del bloque aplanado) y menos o
/// mas filas que las que hay.
fn mates_y_filas() -> Programa {
    let mut ops = vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }];
    let mut d = 2u16;
    let mut resultados = Vec::new();
    for &f in &MATES {
        ops.push(Op::Mate { d, a: 0, f });
        resultados.push(d);
        d += 1;
    }
    for (fila, filas) in [(0u16, 4096u16), (1, 4096), (3, 2), (0, 0), (2, 5)] {
        ops.push(Op::ConstantesEn { d, fila, filas, i: 1, cb: 0 });
        resultados.extend(d..d + 4);
        d += 4;
    }
    // Y una que se lee sobre su propio indice (d = i: el indice, antes).
    ops.push(Op::Copia { d, a: 1 });
    ops.push(Op::ConstantesEn { d, fila: 0, filas: 4096, i: d, cb: 0 });
    resultados.extend(d..d + 4);
    d += 4;
    for (k, &r) in resultados.iter().enumerate() {
        ops.push(Op::Salida { s: r, elemento: (k / 4) as u8, componente: (k % 4) as u8 });
    }
    Programa { ops, iniciales: vec![0.0; d as usize], entradas: 1, salidas: resultados.len().div_ceil(4), lee: 1, filas_cb: 0, ranuras: Default::default(), computo: Default::default() }
}

/// Los valores que muerden a la matematica: los de `saltos` y los de los
/// senos y las exponenciales (pi, lo que pasa de 2^24, los bordes de exp2
/// y de los medios floats).
fn raros_de_mates() -> Vec<f32> {
    let mut v = raros();
    v.extend([core::f32::consts::PI, -core::f32::consts::FRAC_PI_2, 1e7, 16777216.0, 1e-7, 127.0, 128.0, -126.0, -149.0, -150.0, 65504.0, 65520.0, 6e-8, 0.333_333_34, 88.7, -87.3, 1e38, 2.5e-39]);
    for b in [0x3C00u32, 0x7C00, 0xFC00, 0x7E00, 0x8001, 0x03FF, 0x0001_3C00] {
        v.push(f32::from_bits(b));
    }
    v
}

/// *** Cada funcion de la matematica (`MATES`: las 27 de `Op::Mate`) y el
/// cbuffer con fila calculada, por la llamada de los dibujos: los bits del
/// interprete sobre cada par de valores raros (x la funcion, y la fila
/// como bits: 0, 1, 2, 3, 31, 2^31, 0xFFFFFFFF...) y 20.000 al azar, con
/// cbuffers de 0, 4, 20, 37, 64, 100 y 160 bytes SIN rellenar: lo que pasa
/// de su medida, 0 en los dos (la palabra que no cabe ENTERA, tambien).
#[test]
fn cada_mate_y_cada_fila_calculada_nativa_da_los_bits_del_interprete() {
    let p = mates_y_filas();
    assert_eq!(nativo::por_que_no(&p), None);
    let f = con_llamadas(sellar(&nativo::compilar(&p).expect("se traduce")));
    let raros = raros_de_mates();
    let mut z = Azar(0x5DEE_CE66_D1CE_4E5B);
    let cbs: Vec<Vec<u8>> = [0usize, 4, 20, 37, 64, 100, 160].iter().map(|&n| (0..n).map(|_| z.siguiente() as u8).collect()).collect();
    let mut malos = Vec::new();
    let mut casos = 0;
    for &x in &raros {
        for &y in &raros {
            let cb = &cbs[casos % cbs.len()];
            casos += 1;
            if let Err(m) = un_caso(&p, f, &[[x, y, 0.0, 0.0]], cb, &Recursos::NINGUNO) {
                malos.push(m);
            }
        }
    }
    for _ in 0..20_000 {
        let (x, y) = (z.valor(&raros), f32::from_bits(z.siguiente() as u32 % 16));
        let cb = &cbs[casos % cbs.len()];
        casos += 1;
        if let Err(m) = un_caso(&p, f, &[[x, y, 0.0, 0.0]], cb, &Recursos::NINGUNO) {
            malos.push(m);
        }
    }
    assert!(malos.is_empty(), "{} distintos de {casos}; los primeros:\n{}", malos.len(), malos.iter().take(4).cloned().collect::<Vec<_>>().join("\n"));
}

/// El MXCSR de quien llama no cambia la matematica (la llamada corre con el
/// de D3D, que pone la entrada) ni el muestreo, y su control vuelve como
/// estaba: hacia cero, DAZ y FTZ.
#[test]
fn el_mxcsr_de_quien_llama_no_cuenta_en_lo_que_se_llama() {
    let p = mates_y_filas();
    let f = con_llamadas(sellar(&nativo::compilar(&p).unwrap()));
    let (tex, mue, buf) = (texturas(), muestreadores(), buferes());
    let rec = Recursos { texturas: &tex, muestreadores: &mue, buferes: &buf, dinamicas: None };
    let lee = super::saltos::de_dxc(TEXTURA_PS);
    let fl = con_llamadas(sellar(&nativo::compilar(&lee).unwrap()));
    let cb: Vec<u8> = (0..160u32).map(|k| (k * 37) as u8).collect();
    for suyo in [0x7F80u32, 0x9FC0, 0x1FBF] {
        for (x, y) in [(1e-40f32, 1.0f32), (0.1, 2.0), (core::f32::consts::PI, 3.0), (-87.3, 0.0), (0.37, 0.61)] {
            for (q, g) in [(&p, f), (&lee, fl)] {
                let ent = vec![[x, y, 0.0, 0.0]; q.entradas.max(1)];
                let (mut s1, mut s2) = (vec![[0.0f32; 4]; q.salidas], vec![[0.0f32; 4]; q.salidas]);
                q.correr_con(&ent, &cb, &rec, &mut s1, &mut Vec::new());
                let mut regs = q.iniciales.clone();
                let mut m = Muestras { programa: q, recursos: &rec, elegida: None };
                let ll = llamadas(&mut m, cb.len());
                let (mut antes, mut despues) = (0u32, 0u32);
                // SAFETY: el MXCSR del hilo, puesto y devuelto aqui mismo; `g`
                // es la traduccion de `q` y lo que apunta, de aqui.
                unsafe {
                    core::arch::asm!("stmxcsr [{}]", in(reg) &mut antes);
                    core::arch::asm!("ldmxcsr [{}]", in(reg) &suyo);
                    g(regs.as_mut_ptr(), ent.as_ptr(), cb.as_ptr(), s2.as_mut_ptr(), &ll);
                    core::arch::asm!("stmxcsr [{}]", in(reg) &mut despues);
                    core::arch::asm!("ldmxcsr [{}]", in(reg) &antes);
                }
                assert_eq!(despues & 0xFFC0, suyo & 0xFFC0, "el control de quien llama, devuelto");
                for (a, b) in s1.iter().zip(&s2) {
                    assert!((0..4).all(|k| igual(a[k], b[k])), "MXCSR {suyo:#x}, ({x:e}, {y:e}): {a:?} y {b:?}");
                }
            }
        }
    }
}

/// **Lo que se gana con los que muestrean** (no juzga: lo dice), como
/// `saltos::lo_que_tarda_cada_camino`: `n` veces por los dos caminos, la
/// mejor de cinco rondas. El muestreo es el MISMO Rust en los dos: lo que
/// se gana es todo lo de alrededor (las cuentas, los saltos, el despacho
/// del interprete).
#[test]
fn lo_que_tarda_cada_camino_con_texturas() {
    let n = 100_000;
    let (tex, mue, buf) = (texturas(), muestreadores(), buferes());
    let tex_dinamicas = tex.clone();
    let buscar = move |rango: u8, registro: u32| -> Option<Textura<'static>> { tex_dinamicas.get((registro as usize + rango as usize) % 9).copied().flatten() };
    let rec = Recursos { texturas: &tex, muestreadores: &mue, buferes: &buf, dinamicas: Some(Dinamicas(&buscar)) };
    for (nombre, d) in [("hdr PSLee", HDR_LEE), ("htexture PSMain", HTEXTURE_PS), ("dynindex pixel", DYNINDEX_PS), ("mates", MATES_PS), ("luces", LUCES), ("gbuffer", GBUFFER)] {
        let p = super::saltos::de_dxc(d);
        let f = con_llamadas(sellar(&nativo::compilar(&p).unwrap()));
        let cb: Vec<u8> = (0..1024u32).flat_map(|k| ((k % 7) as f32 * 0.125).to_le_bytes()).collect();
        let ent: Vec<Vec<[f32; 4]>> = (0..64u32).map(|i| vec![[i as f32 / 64.0, 1.0 - i as f32 / 80.0, 0.5, f32::from_bits(i % 4)]; p.entradas.max(1)]).collect();
        let mut s = vec![[0.0f32; 4]; p.salidas];
        let mut regs = Vec::new();
        let mut m = Muestras { programa: &p, recursos: &rec, elegida: None };
        let pm: *mut Muestras = &mut m;
        let ll = llamadas(pm, cb.len());
        let (mut interpretado, mut traducido) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for k in 0..n {
                p.correr_con(&ent[k % 64], &cb, &rec, &mut s, &mut regs);
            }
            interpretado = interpretado.min(t.elapsed());
            let t = std::time::Instant::now();
            for k in 0..n {
                regs.clear();
                regs.extend_from_slice(&p.iniciales);
                // SAFETY: `f` es la traduccion de `p`; lo que apunta, de aqui.
                unsafe {
                    (*pm).elegida = None;
                    f(regs.as_mut_ptr(), ent[k % 64].as_ptr(), cb.as_ptr(), s.as_mut_ptr(), &ll);
                }
            }
            traducido = traducido.min(t.elapsed());
        }
        eprintln!("{nombre}: {n} veces, interpretado {interpretado:?}, traducido {traducido:?} ({:.1} veces)", interpretado.as_secs_f64() / traducido.as_secs_f64());
    }
}
