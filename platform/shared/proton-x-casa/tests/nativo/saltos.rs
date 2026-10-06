//! **La VELOCIDAD** (05-10): los sombreadores de dibujo que SALTAN o hacen
//! cuentas ENTERAS, traducidos por `nativo::compilar` (el cuerpo del computo
//! con la llamada de los dibujos), contra el interprete, BIT A BIT: lo que
//! sale, y si el pixel queda. Los valores que muerden: NaN, -0, infinitos,
//! lo que no cabe en un entero (fptosi satura, NaN da 0), los bordes de i32
//! y u32 como bits, desplazamientos de 32 o mas (DXIL se queda con los 5 de
//! abajo), divisiones por 0 e i32::MIN / -1.

use super::{igual, sellar, Sombreador};
use bmo_proton_x::dxil::programa::{Comparacion, Conversion, Op, OpEntera, Programa};
use bmo_proton_x::dxil::{self, ejemplos};
use bmo_proton_x::{nativo, sm5};

const INST_VS: &[u8] = include_bytes!("../../../proton-x/prueba/instancias_vs.dxil");
const INST_ID: &[u8] = include_bytes!("../../../proton-x/prueba/instancias_id.dxil");
const INST_PS: &[u8] = include_bytes!("../../../proton-x/prueba/instancias_ps.dxil");
const HDR_VS: &[u8] = include_bytes!("../../../proton-x/prueba/hdr_vs.dxil");
const HDR_COLOR: &[u8] = include_bytes!("../../../proton-x/prueba/hdr_color.dxil");
const MIENTRAS: &[u8] = include_bytes!("../../../proton-x/prueba/mientras.dxil");
const ANIDADO: &[u8] = include_bytes!("../../../proton-x/prueba/anidado.dxil");
const ENTEROS: &[u8] = include_bytes!("../../../proton-x/prueba/enteros.dxil");
const DIVISION: &[u8] = include_bytes!("../../../proton-x/prueba/division.dxil");
const DESCARTE: &[u8] = include_bytes!("../../../proton-x/prueba/descarte.dxil");
const ARREGLOS: &[u8] = include_bytes!("../../../proton-x/prueba/arreglos.dxil");

pub(crate) fn de_dxc(d: &[u8]) -> Programa {
    let p = dxil::programa::compilar(&dxil::leer(d).unwrap()).unwrap();
    assert_eq!(p.forma(), Ok(()));
    p
}

/// Los valores que muerden: como floats, y como bits de enteros.
pub(crate) fn raros() -> Vec<f32> {
    let mut v = vec![
        0.0f32, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, -2.5, 3.0, 7.75, 100.0, -100.0, 1e-40, -1e-40, f32::MIN_POSITIVE, f32::MAX, f32::MIN, f32::INFINITY,
        f32::NEG_INFINITY, f32::NAN, -f32::NAN, 2147483520.0, 2147483648.0, -2147483648.0, -2147483904.0, 4294967040.0, 4294967296.0, 1e10, -1e10, 0.999_999_9,
    ];
    for b in [0u32, 1, 2, 3, 4, 5, 6, 7, 8, 31, 32, 33, 63, 64, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 0x0001_0000, 12345, 0x00FF_00FF] {
        v.push(f32::from_bits(b));
    }
    v
}

/// Un generador fijo (xorshift): siempre los mismos casos.
pub(crate) struct Azar(pub(crate) u64);

impl Azar {
    pub(crate) fn siguiente(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Medio de las veces un raro; medio, unos bits cualesquiera.
    pub(crate) fn valor(&mut self, raros: &[f32]) -> f32 {
        let x = self.siguiente();
        if x & 1 == 0 {
            raros[(x >> 8) as usize % raros.len()]
        } else {
            f32::from_bits((x >> 32) as u32)
        }
    }
}

/// El cbuffer como lo pone la casa: lo que lea el programa, con ceros.
fn relleno(p: &Programa, cb: &[u8]) -> Vec<u8> {
    let mut v = cb.to_vec();
    v.resize(v.len().max(p.filas_cb as usize * 16).max(16), 0);
    v
}

/// **Un caso por los dos caminos**: si el pixel queda y, si queda, cada
/// salida. `Err` dice el primero distinto.
fn un_caso(p: &Programa, f: Sombreador, ent: &[[f32; 4]], cb: &[u8]) -> Result<(), String> {
    let cb = relleno(p, cb);
    let (mut s1, mut s2) = (vec![[0.0f32; 4]; p.salidas], vec![[0.0f32; 4]; p.salidas]);
    let queda1 = p.correr(ent, &cb, &mut s1, &mut Vec::new());
    let mut regs = p.iniciales.clone();
    let r = f(regs.as_mut_ptr(), ent.as_ptr(), cb.as_ptr(), s2.as_mut_ptr());
    let queda2 = match r {
        nativo::QUEDA => true,
        nativo::DESCARTADO => false,
        otro => return Err(format!("devolvio {otro} (entradas {ent:?})")),
    };
    if queda1 != queda2 {
        return Err(format!("queda: interpretado {queda1}, nativo {queda2} (entradas {:?})", ent.iter().map(|e| e.map(f32::to_bits)).collect::<Vec<_>>()));
    }
    if !queda1 {
        return Ok(());
    }
    for (i, (a, b)) in s1.iter().zip(&s2).enumerate() {
        for k in 0..4 {
            if !igual(a[k], b[k]) {
                return Err(format!(
                    "salida {i}.{k}: interpretado {:#010x}, nativo {:#010x} (entradas {:?}, cb {:?})",
                    a[k].to_bits(),
                    b[k].to_bits(),
                    ent.iter().map(|e| e.map(f32::to_bits)).collect::<Vec<_>>(),
                    &cb[..cb.len().min(32)]
                ));
            }
        }
    }
    Ok(())
}

/// Lo traduce `nativo` (con saltos: no por la fila de SSE) y no hay motivo
/// que decir.
fn traducir(nombre: &str, p: &Programa) -> Sombreador {
    assert_eq!(nativo::por_que_no(p), None, "{nombre}: sin motivo para no traducirlo");
    sellar(&nativo::compilar(p).unwrap_or_else(|| panic!("{nombre}: se traduce")))
}

/// `n` casos al azar: cada componente de cada entrada, y del cbuffer si
/// `cb_al_azar`, un raro o unos bits cualesquiera.
fn al_azar(nombre: &str, p: &Programa, n: usize, cb_al_azar: bool) -> usize {
    let f = traducir(nombre, p);
    let raros = raros();
    let mut z = Azar(0x2545_F491_4F6C_DD1D ^ nombre.len() as u64);
    let mut malos = Vec::new();
    let filas = (p.filas_cb as usize).max(1);
    for _ in 0..n {
        let ent: Vec<[f32; 4]> = (0..p.entradas.max(1)).map(|_| [z.valor(&raros), z.valor(&raros), z.valor(&raros), z.valor(&raros)]).collect();
        let cb: Vec<u8> = if cb_al_azar { (0..filas * 4).flat_map(|_| z.valor(&raros).to_le_bytes()).collect() } else { Vec::new() };
        if let Err(m) = un_caso(p, f, &ent, &cb) {
            malos.push(m);
        }
    }
    assert!(malos.is_empty(), "{nombre}: {} distintos de {n}; el primero: {}", malos.len(), malos[0]);
    n
}

/// *** Los sombreadores de `instancias.exe` y `hdr.exe` (SV_InstanceID y
/// SV_VertexID a float, desplazamientos y `&` de enteros): se traducen, y
/// dan los bits del interprete. Los ids chicos, todos; y miles al azar.
#[test]
fn los_vs_de_instancias_y_hdr_con_enteros_nativos_dan_los_bits_del_interprete() {
    for (nombre, d) in [("instancias VSInst", INST_VS), ("instancias VSId", INST_ID), ("instancias PSColor", INST_PS), ("hdr VSCuadro", HDR_VS), ("hdr PSColor", HDR_COLOR)] {
        let p = de_dxc(d);
        al_azar(nombre, &p, 20_000, true);
        // Cada id de 0 a 63 en cada elemento (el que lleve el id, lo lee).
        let f = traducir(nombre, &p);
        for id in 0u32..64 {
            let ent = vec![[f32::from_bits(id), 0.25, 0.5, 1.0]; p.entradas.max(1)];
            un_caso(&p, f, &ent, &[]).unwrap_or_else(|m| panic!("{nombre}, id {id}: {m}"));
        }
    }
}

/// *** Los PS de `dxc` con saltos (`while`, `for` anidados con `break` y
/// `continue`, `switch`, enteros con y sin signo, divisiones por un cbuffer,
/// `clip` y `discard`, arrays con indice calculado): los bits y el
/// descarte del interprete. `mientras` no acaba con x <= -0.25 (en los dos
/// caminos: `pruebas_saltos.rs`), asi que su x va de 0 a 100.
#[test]
fn los_ps_de_dxc_con_saltos_nativos_dan_los_bits_del_interprete() {
    for (nombre, d) in [("anidado", ANIDADO), ("enteros", ENTEROS), ("division", DIVISION), ("descarte", DESCARTE), ("arreglos", ARREGLOS)] {
        al_azar(nombre, &de_dxc(d), 20_000, true);
    }
    let p = de_dxc(MIENTRAS);
    let f = traducir("mientras", &p);
    let raros = raros();
    for k in [[100.0f32, 0.25, 0.0, 0.0], [7.5, 1.0, 0.0, 0.0], [f32::INFINITY, 0.0, 0.0, 0.0], [f32::NAN, 0.5, 0.0, 0.0]] {
        let cb: Vec<u8> = k.iter().flat_map(|v| v.to_le_bytes()).collect();
        for x in [0.0f32, -0.0, 1e-40, 0.1, 0.5, 1.0, 3.0, 99.0, 1e30, f32::INFINITY, f32::NAN] {
            if k[0].is_infinite() && x.is_finite() {
                continue; // x < infinito, siempre: no acaba (los dos).
            }
            for &y in &raros {
                un_caso(&p, f, &[[0.0; 4], [x, y, 0.0, 0.0]], &cb).unwrap_or_else(|m| panic!("mientras: {m}"));
            }
        }
    }
}

/// *** Los de `ejemplos` (los de E6, a mano y de SM5): un si, un bucle que
/// sale por su RomperSi, un contador entero, bucles anidados, y los de
/// `fxc` (loop, if_z, switch, enteros, udiv, discard).
#[test]
fn los_saltos_de_ejemplos_y_de_fxc_nativos_dan_los_bits_del_interprete() {
    let raros = raros();
    for (nombre, p) in [("si_sino", ejemplos::si_sino()), ("bucle_entero", ejemplos::bucle_entero())] {
        let f = traducir(nombre, &p);
        for &x in &raros {
            for &y in &raros {
                un_caso(&p, f, &[[x, y, 0.0, 0.0]], &[]).unwrap_or_else(|m| panic!("{nombre}: {m}"));
            }
        }
    }
    // Los que solo acaban con algunos pares (como en `nativo_computo.rs`).
    let f = traducir("bucle_geometrico", &ejemplos::bucle_geometrico());
    for y in [0.25f32, 0.5, 1.0, 2.0, 3.0, 100.0, 1e-3] {
        for x in [-1e10f32, -1.0, -0.0, 0.0, 0.5 * y, y, 1.5 * y, 1.9 * y] {
            un_caso(&ejemplos::bucle_geometrico(), f, &[[x, y, 0.0, 0.0]], &[]).unwrap();
        }
    }
    let p = ejemplos::anidado();
    let f = traducir("anidado", &p);
    for y in [0.125f32, 0.5, 1.0, 2.0, 3.0] {
        for x in [-1e10f32, -1.0, 0.0, 0.5, 2.0, 7.25, 100.0] {
            un_caso(&p, f, &[[x, y, 0.0, 0.0]], &[]).unwrap();
        }
    }
    for (nombre, (t, e, s)) in [
        ("sm5_bucle", ejemplos::sm5_bucle()),
        ("sm5_si_cero", ejemplos::sm5_si_cero()),
        ("sm5_switch", ejemplos::sm5_switch()),
        ("sm5_enteros", ejemplos::sm5_enteros()),
        ("sm5_division", ejemplos::sm5_division()),
        ("sm5_descarte", ejemplos::sm5_descarte()),
    ] {
        al_azar(nombre, &sm5::compilar(&t, &e, &s).unwrap(), 20_000, false);
    }
}

/// *** Cada operacion entera, cada comparacion (float y entera), cada
/// conversion, `Elige`, `discard` y los arrays de registros (con indices
/// fuera), sobre cada par de valores raros, por la llamada de los dibujos.
#[test]
fn cada_operacion_entera_nativa_da_los_bits_del_interprete() {
    use OpEntera::*;
    let enteras = [Resta, Mul, Shl, ShrL, ShrA, Y, O, OX, MinS, MaxS, MinU, MaxU, DivU, RemU, DivS, RemS];
    use Comparacion::*;
    let comparaciones = [Menor, MenorIgual, Mayor, MayorIgual, Igual, Distinto, MenorSinSigno, MenorIgualSinSigno, MayorSinSigno, MayorIgualSinSigno];
    let conversiones = [Conversion::EnteroAFloat, Conversion::SinSignoAFloat, Conversion::FloatAEntero, Conversion::FloatASinSigno];
    let mut ops = vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }];
    let mut d = 2u16;
    let mut resultados = Vec::new();
    let mut nuevo = |ops: &mut Vec<Op>, op: Op, d: &mut u16| {
        ops.push(op);
        resultados.push(*d);
        *d += 1;
    };
    for &o in &enteras {
        nuevo(&mut ops, Op::Entera { d, a: 0, b: 1, op: o }, &mut d);
    }
    for &c in &comparaciones {
        for entero in [false, true] {
            nuevo(&mut ops, Op::Compara { d, a: 0, b: 1, como: c, entero }, &mut d);
        }
    }
    for &c in &conversiones {
        nuevo(&mut ops, Op::Convierte { d, a: 0, como: c }, &mut d);
    }
    nuevo(&mut ops, Op::SumaEntera { d, a: 0, b: 1 }, &mut d);
    nuevo(&mut ops, Op::Elige { d, c: 0, a: 1, b: 0 }, &mut d);
    nuevo(&mut ops, Op::Copia { d, a: 1 }, &mut d);
    // Un array de 4 registros: se escribe y se lee con los bits de x y de
    // y como indice (fuera de 0..4: no se escribe, y se lee 0).
    let base = d;
    d += 4;
    ops.push(Op::EscribeIndexado { base, n: 4, i: 0, s: 1 });
    nuevo(&mut ops, Op::LeeIndexado { d, base, n: 4, i: 1 }, &mut d);
    for k in 0..4 {
        resultados.push(base + k);
    }
    // Y se tira si y es NaN (despues de escribir algo: no cuenta).
    let nan = d;
    ops.push(Op::Compara { d: nan, a: 1, b: 1, como: Distinto, entero: false });
    d += 1;
    let mut todo = ops.clone();
    for (k, &r) in resultados.iter().enumerate() {
        todo.push(Op::Salida { s: r, elemento: (k / 4) as u8, componente: (k % 4) as u8 });
        if k == 3 {
            todo.push(Op::Descarta { c: nan });
        }
    }
    let salidas = resultados.len().div_ceil(4);
    let mut iniciales = vec![0.0f32; d as usize];
    // El array empieza con algo que se vea.
    for k in 0..4 {
        iniciales[(base + k) as usize] = 10.0 + k as f32;
    }
    let p = Programa { ops: todo, iniciales, entradas: 1, salidas, lee: 1, filas_cb: 0, ranuras: Default::default(), computo: Default::default() };
    let f = traducir("a mano", &p);
    let raros = raros();
    let mut malos = Vec::new();
    for &x in &raros {
        for &y in &raros {
            if let Err(m) = un_caso(&p, f, &[[x, y, 0.0, 0.0]], &[]) {
                malos.push(m);
            }
        }
    }
    assert!(malos.is_empty(), "{} distintos; los primeros:\n{}", malos.len(), malos.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
    // Que el descarte se probo de verdad: con un NaN, el pixel se va.
    let mut regs = p.iniciales.clone();
    let mut s = vec![[0.0f32; 4]; salidas];
    assert_eq!(f(regs.as_mut_ptr(), [[1.0, f32::NAN, 0.0, 0.0]].as_ptr(), [0u8; 16].as_ptr(), s.as_mut_ptr()), nativo::DESCARTADO);
    assert_eq!(f(regs.as_mut_ptr(), [[1.0, 2.0, 0.0, 0.0]].as_ptr(), [0u8; 16].as_ptr(), s.as_mut_ptr()), nativo::QUEDA);
}

/// El MXCSR de quien llama no cambia las cuentas, por los DOS caminos (la
/// fila de SSE y el cuerpo del computo), y su control vuelve como estaba:
/// redondeo hacia cero, DAZ y FTZ, el de D3D con todas las banderas puestas.
/// Si el control de quien llama es otro, vuelve el MXCSR exacto; si es el
/// de D3D, ni se toca (las banderas del sombreador se quedan: no cuentan).
#[test]
fn el_mxcsr_de_quien_llama_no_cuenta_por_ningun_camino() {
    use Op::*;
    let suma = Programa {
        ops: vec![Entrada { d: 0, elemento: 0, componente: 0 }, Entrada { d: 1, elemento: 0, componente: 1 }, Add { d: 2, a: 0, b: 1 }, Salida { s: 2, elemento: 0, componente: 0 }],
        iniciales: vec![0.0; 3],
        entradas: 1,
        salidas: 1,
        lee: 1,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    };
    assert!(!suma.salta());
    let si_sino = ejemplos::si_sino();
    let (fs, fc) = (sellar(&nativo::compilar(&suma).unwrap()), traducir("si_sino", &si_sino));
    // x >= y: x + y en los dos. 1 + 0.75 de un ulp: al mas cercano, 1 +
    // 2^-23 (hacia cero, 1). 1e-40 + 1e-40: subnormal (con DAZ o FTZ, 0).
    let y = 1.5 * f32::EPSILON / 2.0;
    let casos = [(1.0f32, y, 0x3F80_0001u32), (1e-40, 1e-40, (1e-40f32 + 1e-40).to_bits())];
    assert_ne!(casos[1].2, 0);
    for (camino, f, p) in [("directo", fs, &suma), ("con saltos", fc, &si_sino)] {
        for suyo in [0x7F80u32, 0x9FC0, 0x1FBF, 0x1F80] {
            for &(x, y, bits) in &casos {
                let mut regs = p.iniciales.clone();
                let mut s = [[0.0f32; 4]];
                let (mut antes, mut despues) = (0u32, 0u32);
                // SAFETY: el MXCSR del hilo, puesto y devuelto aqui mismo.
                unsafe {
                    core::arch::asm!("stmxcsr [{}]", in(reg) &mut antes);
                    core::arch::asm!("ldmxcsr [{}]", in(reg) &suyo);
                }
                f(regs.as_mut_ptr(), [[x, y, 0.0, 0.0]].as_ptr(), [0u8; 16].as_ptr(), s.as_mut_ptr());
                unsafe {
                    core::arch::asm!("stmxcsr [{}]", in(reg) &mut despues);
                    core::arch::asm!("ldmxcsr [{}]", in(reg) &antes);
                }
                assert_eq!(s[0][0].to_bits(), bits, "{camino}, MXCSR {suyo:#x}: ({x:e}, {y:e}), IEEE al mas cercano");
                assert_eq!(despues & 0xFFC0, suyo & 0xFFC0, "{camino}: el control de quien llama, devuelto");
                if suyo & 0xFFC0 != 0x1F80 {
                    assert_eq!(despues, suyo, "{camino}: otro control: el MXCSR exacto");
                }
            }
        }
    }
}

/// **Lo que se gana** (no juzga: lo dice). Cada sombreador, `n` veces por
/// el interprete y por el codigo traducido, con las mismas entradas; de
/// cinco rondas, la mejor de cada uno (la maquina del banco la comparten
/// otros: la peor ronda mide a los otros).
///
/// [!] Aqui el interprete es Rust CON SSE (en el metal, soft-float: mucho
/// mas lento), asi que en `--release` esto mide lo de menos.
#[test]
fn lo_que_tarda_cada_camino() {
    let n = 100_000;
    for (nombre, p) in [("hdr VSCuadro", de_dxc(HDR_VS)), ("instancias VSId", de_dxc(INST_ID)), ("instancias VSInst", de_dxc(INST_VS)), ("division (bucle)", de_dxc(DIVISION)), ("anidado (bucles)", de_dxc(ANIDADO))] {
        let f = traducir(nombre, &p);
        let cb = relleno(&p, &[3u8, 0, 0, 0, 7, 0, 0, 0, 10, 0, 0, 0, 1, 0, 0, 0].repeat(4));
        let ent: Vec<Vec<[f32; 4]>> = (0..64u32).map(|i| vec![[f32::from_bits(i % 6), i as f32 / 64.0, 0.75, 1.0]; p.entradas.max(1)]).collect();
        let mut s = vec![[0.0f32; 4]; p.salidas];
        let mut regs = Vec::new();
        let (mut interpretado, mut traducido) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for k in 0..n {
                p.correr(&ent[k % 64], &cb, &mut s, &mut regs);
            }
            interpretado = interpretado.min(t.elapsed());
            let t = std::time::Instant::now();
            for k in 0..n {
                regs.clear();
                regs.extend_from_slice(&p.iniciales);
                f(regs.as_mut_ptr(), ent[k % 64].as_ptr(), cb.as_ptr(), s.as_mut_ptr());
            }
            traducido = traducido.min(t.elapsed());
        }
        eprintln!("{nombre}: {n} veces, interpretado {interpretado:?}, traducido {traducido:?} ({:.1} veces)", interpretado.as_secs_f64() / traducido.as_secs_f64());
    }
}

