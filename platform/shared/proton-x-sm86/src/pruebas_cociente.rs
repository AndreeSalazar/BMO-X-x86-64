//! DL10 (09-10): la division EXACTA de f32, en el simulador de la 3060,
//! contra la de IEEE (`x / y` de Rust, la de la casa): los mismos bits -- o
//! NaN los dos -- en cada clase de numero, con el MUFU.RCP de la casa y con
//! el MOVIDO como se equivoca la 3060; y el juez dice que si a lo que viaja.

extern crate std;

use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_proton_x::dxil::programa::{Comparacion, Op, Programa};

use crate::pruebas::mismos;
use crate::simula::{correr, Maquina};
use crate::{emitir, emitir_con, vivo, Abi};

/// `a / b` de sus dos entradas, a su salida.
fn division() -> Programa {
    Programa {
        ops: alloc::vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 1, componente: 0 }, Op::Div { d: 2, a: 0, b: 1 }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
        iniciales: alloc::vec![0.0; 3],
        entradas: 2,
        salidas: 1,
        lee: 0b11,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    }
}

/// `a / b` en el simulador (el ABI del banco), con el inverso movido `ulp`.
fn en_la_3060(codigo: &[(u64, u64)], a: u32, b: u32, ulp: i32) -> u32 {
    let banco: Vec<u8> = [a, 0, 0, 0, b, 0, 0, 0].iter().flat_map(|x| x.to_le_bytes()).collect();
    let mut m = Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
    m.inverso_ulp = ulp;
    correr(codigo, &mut m).unwrap_or_else(|e| panic!("{a:#x} / {b:#x}: {e:?}"));
    m.r[0]
}

struct Azar(u64);

impl Azar {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    /// Un f32 con el campo de exponente `e` (de 0 a 255) y signo y mantisa al azar.
    fn con(&mut self, e: u32) -> u32 {
        (self.bits() & 0x8000_0000) | e.min(255) << 23 | (self.bits() & 0x7F_FFFF)
    }
}

/// Los pares que mas cuestan: cada clase contra cada clase, los bordes de la
/// ventana, los cocientes que se salen por arriba y por abajo, y los
/// subnormales con su empate.
fn pares() -> Vec<(u32, u32)> {
    let mut z = Azar(0xd1_0010);
    let bordes = [
        0u32, 0x8000_0000, 1, 0x8000_0001, 2, 3, 5, 0x007F_FFFF, 0x0080_0000, 0x0080_0001, 0x00FF_FFFF, 0x0100_0000,
        0x1FFF_FFFF, 0x2000_0000, 0x2000_0001, 0x3F80_0000, 0xBF80_0000, 0x4040_0000, 0x3DCC_CCCD, 0x3F7F_FFFF, 0x3F80_0001,
        0x5EFF_FFFF, 0x5F00_0000, 0x5F00_0001, 0x7F7F_FFFF, 0xFF7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0000, 0xFFC0_0001, 0x7F80_0001,
    ];
    let mut v = Vec::new();
    for &a in &bordes {
        for &b in &bordes {
            v.push((a, b));
        }
    }
    // Los subnormales y sus empates: k * 2^-149 entre potencias de dos y
    // entre impares chicos.
    for k in 1..200u32 {
        for b in [2.0f32, 4.0, 8.0, 16.0, 1.0e10, 3.0, 5.0, 0.75, 1.5, 6.0, 1.0 / 3.0] {
            v.push((k, b.to_bits()));
            v.push((k | 0x8000_0000, b.to_bits()));
        }
    }
    for _ in 0..200_000 {
        // al azar entero (todas las clases, pocas especiales)
        v.push((z.bits(), z.bits()));
        // dentro de la ventana
        let (ea, eb) = (64 + z.bits() % 127, 64 + z.bits() % 127);
        v.push((z.con(ea), z.con(eb)));
        // cocientes que se quedan cortos: subnormales y casi
        let ea = z.bits() % 100;
        let eb = ea + 100 + z.bits() % 60;
        v.push((z.con(ea), z.con(eb)));
        // y que se pasan
        let (ea, eb) = (190 + z.bits() % 65, z.bits() % 60);
        v.push((z.con(ea), z.con(eb)));
        // con un subnormal de un lado o del otro
        let (sa, e) = (z.bits() & 0x807F_FFFF, z.bits() % 255);
        v.push((sa, z.con(e)));
        let (e, sb) = (z.bits() % 255, z.bits() & 0x807F_FFFF);
        v.push((z.con(e), sb));
        // la misma mantisa: el cociente, una potencia de dos
        let e = 1 + z.bits() % 254;
        let a = z.con(e);
        let e = 1 + z.bits() % 254;
        v.push((a, (a & 0x807F_FFFF) | e << 23));
    }
    v
}

/// ** LOS BITS DE IEEE, en cada clase: el simulador de la 3060 sobre lo que
/// emite el emisor da `a / b` de Rust -- o NaN los dos --.
#[test]
fn la_division_da_los_bits_de_ieee() {
    let e = emitir(&division(), 64).expect("la division se emite");
    let mut vistos = 0;
    for (a, b) in pares() {
        let quiere = f32::from_bits(a) / f32::from_bits(b);
        let da = en_la_3060(&e.codigo, a, b, 0);
        assert!(mismos(da, quiere), "{a:#010x} / {b:#010x}: {da:#010x}, y no {:#010x}", quiere.to_bits());
        vistos += 1;
    }
    assert!(vistos > 1_000_000, "{vistos}");
}

/// ** AGUANTA EL MUFU DE LA 3060: con el inverso movido hasta 4 ULP a cada
/// lado (la 3060 se equivoca en uno), los mismos bits.
#[test]
fn la_division_aguanta_un_inverso_aproximado() {
    let e = emitir(&division(), 64).unwrap();
    let todos = pares();
    for ulp in [-4, -3, -2, -1, 1, 2, 3, 4] {
        for &(a, b) in todos.iter().step_by(7) {
            let quiere = f32::from_bits(a) / f32::from_bits(b);
            let da = en_la_3060(&e.codigo, a, b, ulp);
            assert!(mismos(da, quiere), "{a:#010x} / {b:#010x} con el inverso a {ulp} ulp: {da:#010x}, y no {:#010x}", quiere.to_bits());
        }
    }
}

/// ** LO QUE VIAJA, juzgado: el juez de la 3060 (esperas, barreras,
/// registros) en los dos ABI, la regla de un cuerpo de app (R7) en el de
/// registros, y los bits contra la casa con las entradas ya en registros
/// (`vivo::comprobar`).
#[test]
fn el_juez_acepta_la_division() {
    let p = division();
    for abi in [Abi::Banco, Abi::Registros] {
        let e = emitir_con(&p, 64, abi).unwrap();
        juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("{abi:?}: {b}"));
        if abi == Abi::Registros {
            juzgar_cuerpo_de_app(&e.codigo, e.registros).unwrap_or_else(|b| panic!("R7: {b}"));
            assert!(vivo::comprobar(&p, &e, "la division").unwrap() > 0);
        }
    }
}

/// ** ENTRE UNA CONSTANTE, y un nombre que es el cociente de si mismo
/// (`x = x / y` en un bucle): la misma cuenta, los mismos bits.
#[test]
fn la_division_entre_una_constante_y_en_una_variable() {
    // a / 3.0, con 3.0 en los iniciales.
    let p = Programa {
        ops: alloc::vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Div { d: 2, a: 0, b: 1 }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
        iniciales: alloc::vec![0.0, 3.0, 0.0],
        entradas: 1,
        salidas: 1,
        lee: 1,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    };
    let e = emitir(&p, 64).unwrap();
    let mut z = Azar(7);
    for _ in 0..50_000 {
        let a = z.bits();
        let da = en_la_3060(&e.codigo, a, 0, 0);
        assert!(mismos(da, f32::from_bits(a) / 3.0), "{a:#x} / 3");
    }
    // x = 1000; tres vueltas de x = x / 7 (x, una variable: su registro de
    // todo el programa es el de `a` y el de `d`).
    let p = Programa {
        ops: alloc::vec![
            Op::Copia { d: 0, a: 1 },
            Op::Copia { d: 3, a: 4 },
            Op::Bucle,
            Op::Compara { d: 7, a: 3, b: 5, como: Comparacion::MayorIgual, entero: false },
            Op::RomperSi { c: 7, si_cero: false },
            Op::Div { d: 0, a: 0, b: 2 },
            Op::Add { d: 3, a: 3, b: 6 },
            Op::FinBucle,
            Op::Salida { s: 0, elemento: 0, componente: 0 },
        ],
        iniciales: alloc::vec![0.0, 1000.0, 7.0, 0.0, 0.0, 3.0, 1.0, 0.0],
        entradas: 0,
        salidas: 1,
        lee: 0,
        filas_cb: 0,
        ranuras: Default::default(),
        computo: Default::default(),
    };
    let e = emitir(&p, 64).unwrap();
    let mut quiere = 1000.0f32;
    for _ in 0..3 {
        quiere /= 7.0;
    }
    assert_eq!(en_la_3060(&e.codigo, 0, 0, 0), quiere.to_bits());
    let mut regs = Vec::new();
    let mut s = [[0.0f32; 4]; 1];
    p.correr(&[], &[], &mut s, &mut regs);
    assert_eq!(s[0][0].to_bits(), quiere.to_bits(), "la casa");
}

/// ** LO QUE CUESTA, medido (09-10): una division son 105 instrucciones de
/// codigo -- lo rapido, lo lento y lo raro, cada uno en su sitio; con las
/// dos que suben sus entradas del banco -- y, al correr, de 26 a 88: 36 en
/// la ventana. Es lo que pesa en la obra de una celda de TITAN++
/// (`OBRA_DIVISION`, 24: hasta ~3.6 instrucciones por unidad, como lo
/// demas).
#[test]
fn lo_que_cuesta_una_division() {
    let e = emitir(&division(), 64).unwrap();
    // El mismo programa con un producto: lo que no es la division.
    let mut sin = division();
    sin.ops[2] = Op::Mul { d: 2, a: 0, b: 1 };
    let base = emitir(&sin, 64).unwrap();
    let pasos = |codigo: &[(u64, u64)], a: u32, b: u32| {
        let banco: Vec<u8> = [a, 0, 0, 0, b, 0, 0, 0].iter().flat_map(|x| x.to_le_bytes()).collect();
        correr(codigo, &mut Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]])).unwrap()
    };
    let (n, fijos) = (e.codigo.len() - base.codigo.len() + 1, pasos(&base.codigo, 0, 0) - 1);
    assert!(n <= 106, "{n} instrucciones");
    let (mut menos, mut mas) = (usize::MAX, 0);
    for (a, b) in pares().into_iter().step_by(3) {
        let p = pasos(&e.codigo, a, b) - fijos;
        (menos, mas) = (menos.min(p), mas.max(p));
    }
    assert!(menos >= 20 && mas <= 88, "de {menos} a {mas} pasos");
    assert_eq!(pasos(&e.codigo, 1.5f32.to_bits(), 3.0f32.to_bits()) - fijos, 36, "en la ventana");
}
