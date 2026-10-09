//! Las pruebas de la tarjeta de la CPU: cada operacion que dice saber, contra
//! el interprete de la casa en los valores que muerden; el juez diciendo que
//! NO a cada forma de romper el contrato de la llamada; y sus limites, dichos
//! sin emitir nada.

use super::*;
use bmo_prometeo::programa::{Comparacion, Conversion, OpEntera, Reg};

/// Un Programa de `entradas` valores y una salida.
fn programa(ops: Vec<Op>, iniciales: Vec<f32>, entradas: usize) -> Programa {
    let lee = if entradas >= 32 { u32::MAX } else { (1u32 << entradas) - 1 };
    Programa { ops, iniciales, entradas, salidas: 1, lee, filas_cb: 0, ranuras: Default::default(), computo: Default::default() }
}

/// La salida 0, componente 0, por el interprete de la casa.
fn casa(p: &Programa, ent: &[u32]) -> u32 {
    let e: Vec<[f32; 4]> = ent.iter().map(|b| [f32::from_bits(*b), 0.0, 0.0, 0.0]).collect();
    let mut s = [[0.0f32; 4]; 1];
    let mut regs = Vec::new();
    p.correr(&e, &[], &mut s, &mut regs);
    s[0][0].to_bits()
}

fn mismo(a: u32, b: u32) -> bool {
    a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

/// Los f32 que muerden, en bits.
const BORDES: [f32; 18] = [0.0, -0.0, 1.0, -1.0, 0.5, -2.5, 3.0, 0.1, 1.0e-38, f32::MIN_POSITIVE, 1.0e-45, -1.0e-45, f32::MAX, f32::MIN, 1.0e30, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
/// Los enteros que muerden.
const ENTEROS: [u32; 12] = [0, 1, 2, 7, 31, 32, 33, 0xFFFF_FFFF, 0xFFFF_FFFE, 0x8000_0000, 0x7FFF_FFFF, 12345];

/// Cada combinacion de `valores` para `n` entradas, por la CPU y por la casa.
fn contra_la_casa(nombre: &str, p: &Programa, valores: &[u32]) -> usize {
    let c = CPU.emitir(p, Para::Viaje).unwrap_or_else(|e| panic!("{nombre}: {e:?}"));
    CPU.juzgar(&c, Para::Viaje).unwrap_or_else(|e| panic!("{nombre}: el juez: {e}"));
    let n = p.entradas;
    let total = valores.len().pow(n as u32);
    for i in 0..total {
        let mut resto = i;
        let ent: Vec<u32> = (0..n)
            .map(|_| {
                let v = valores[resto % valores.len()];
                resto /= valores.len();
                v
            })
            .collect();
        let celdas: Vec<[u32; 4]> = ent.iter().map(|v| [*v, 0, 0, 0]).collect();
        let suya = CPU.simular(&c, &celdas, 1).unwrap_or_else(|e| panic!("{nombre} con {ent:x?}: {e}"))[0][0];
        let la_casa = casa(p, &ent);
        assert!(mismo(suya, la_casa), "{nombre} con {ent:x?}: la CPU {suya:#x}, la casa {la_casa:#x}");
    }
    total
}

fn f32s() -> Vec<u32> {
    BORDES.iter().map(|x| x.to_bits()).collect()
}

/// Dos entradas, `op`, la salida: el molde de una operacion binaria.
fn binaria(op: Op) -> Programa {
    programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 1, componente: 0 }, op, Op::Salida { s: 2, elemento: 0, componente: 0 }], vec![0.0; 3], 2)
}

fn unaria(op: Op) -> Programa {
    programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, op, Op::Salida { s: 1, elemento: 0, componente: 0 }], vec![0.0; 2], 1)
}

#[test]
fn la_aritmetica_de_f32_da_los_bits_del_interprete() {
    let (a, b, d): (Reg, Reg, Reg) = (0, 1, 2);
    let mut n = 0;
    for (nombre, op) in [("Mul", Op::Mul { d, a, b }), ("Add", Op::Add { d, a, b }), ("Sub", Op::Sub { d, a, b }), ("Div", Op::Div { d, a, b }), ("Min", Op::Min { d, a, b }), ("Max", Op::Max { d, a, b })] {
        n += contra_la_casa(nombre, &binaria(op), &f32s());
    }
    for (nombre, op) in [("Sqrt", Op::Sqrt { d: 1, a: 0 }), ("Rsqrt", Op::Rsqrt { d: 1, a: 0 }), ("Saturate", Op::Saturate { d: 1, a: 0 }), ("Abs", Op::Abs { d: 1, a: 0 })] {
        n += contra_la_casa(nombre, &unaria(op), &f32s());
    }
    let mad = programa(
        vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 1, componente: 0 }, Op::Entrada { d: 2, elemento: 2, componente: 0 }, Op::Mad { d: 3, a: 0, b: 1, c: 2 }, Op::Salida { s: 3, elemento: 0, componente: 0 }],
        vec![0.0; 4],
        3,
    );
    n += contra_la_casa("Mad", &mad, &f32s()[..10]);
    let dot = programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 1, componente: 0 },
            Op::Entrada { d: 2, elemento: 2, componente: 0 },
            Op::Dot { d: 3, n: 3, a: [0, 1, 2, 0], b: [2, 0, 1, 0] },
            Op::Salida { s: 3, elemento: 0, componente: 0 },
        ],
        vec![0.0; 4],
        3,
    );
    n += contra_la_casa("Dot", &dot, &f32s()[..10]);
    assert!(n > 2000, "{n} celdas");
}

/// ** Las mismas, por el OTRO camino de `nativo`: un Programa con una
/// operacion entera (aqui, una `Copia`) no va por la fila de SSE sin saltos
/// sino por el cuerpo del computo (`nativo_computo`), que las escribe a su
/// manera (su min/max, su `Rsqrt`...). Lo que la tarjeta dice saber, lo
/// sabe por los dos.
#[test]
fn la_aritmetica_de_f32_tambien_por_el_cuerpo_con_saltos() {
    let con_saltos = |op: Op, entradas: usize| {
        let mut ops: Vec<Op> = (0..entradas).map(|k| Op::Entrada { d: k as Reg, elemento: k as u8, componente: 0 }).collect();
        ops.push(op);
        ops.push(Op::Copia { d: 7, a: 6 });
        ops.push(Op::Salida { s: 7, elemento: 0, componente: 0 });
        programa(ops, vec![0.0; 8], entradas)
    };
    let (a, b, c, d): (Reg, Reg, Reg, Reg) = (0, 1, 2, 6);
    for (nombre, op) in [("Mul", Op::Mul { d, a, b }), ("Add", Op::Add { d, a, b }), ("Sub", Op::Sub { d, a, b }), ("Div", Op::Div { d, a, b }), ("Min", Op::Min { d, a, b }), ("Max", Op::Max { d, a, b })] {
        let p = con_saltos(op, 2);
        assert!(p.salta(), "{nombre}: por el cuerpo con saltos");
        contra_la_casa(&format!("{nombre} con saltos"), &p, &f32s());
    }
    for (nombre, op) in [("Sqrt", Op::Sqrt { d, a }), ("Rsqrt", Op::Rsqrt { d, a }), ("Saturate", Op::Saturate { d, a }), ("Abs", Op::Abs { d, a })] {
        contra_la_casa(&format!("{nombre} con saltos"), &con_saltos(op, 1), &f32s());
    }
    contra_la_casa("Mad con saltos", &con_saltos(Op::Mad { d, a, b, c }, 3), &f32s()[..10]);
    contra_la_casa("Dot con saltos", &con_saltos(Op::Dot { d, n: 3, a: [0, 1, 2, 0], b: [2, 0, 1, 0] }, 3), &f32s()[..10]);
}

#[test]
fn comparar_elegir_y_los_enteros_dan_los_bits_del_interprete() {
    use Comparacion::*;
    let (a, b, d): (Reg, Reg, Reg) = (0, 1, 2);
    for como in [Menor, MenorIgual, Mayor, MayorIgual, Igual, Distinto] {
        contra_la_casa(&format!("Compara {como:?}"), &binaria(Op::Compara { d, a, b, como, entero: false }), &f32s());
    }
    for como in [Menor, MenorIgual, Mayor, MayorIgual, Igual, Distinto, MenorSinSigno, MenorIgualSinSigno, MayorSinSigno, MayorIgualSinSigno] {
        contra_la_casa(&format!("Compara entero {como:?}"), &binaria(Op::Compara { d, a, b, como, entero: true }), &ENTEROS);
    }
    use OpEntera::*;
    for op in [Resta, Mul, Shl, ShrL, ShrA, Y, O, OX, MinS, MaxS, MinU, MaxU, DivU, RemU, DivS, RemS] {
        contra_la_casa(&format!("Entera {op:?}"), &binaria(Op::Entera { d, a, b, op }), &ENTEROS);
    }
    contra_la_casa("SumaEntera", &binaria(Op::SumaEntera { d, a, b }), &ENTEROS);
    contra_la_casa("Copia", &unaria(Op::Copia { d: 1, a: 0 }), &f32s());
    for como in [Conversion::EnteroAFloat, Conversion::SinSignoAFloat] {
        contra_la_casa(&format!("Convierte {como:?}"), &unaria(Op::Convierte { d: 1, a: 0, como }), &ENTEROS);
    }
    for como in [Conversion::FloatAEntero, Conversion::FloatASinSigno] {
        contra_la_casa(&format!("Convierte {como:?}"), &unaria(Op::Convierte { d: 1, a: 0, como }), &f32s());
    }
    // Elige: c ? a : b
    let elige = programa(
        vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 1, componente: 0 }, Op::Entrada { d: 2, elemento: 2, componente: 0 }, Op::Elige { d: 3, c: 0, a: 1, b: 2 }, Op::Salida { s: 3, elemento: 0, componente: 0 }],
        vec![0.0; 4],
        3,
    );
    contra_la_casa("Elige", &elige, &[0, 1, 0xFFFF_FFFF, 0x3F80_0000, 0x7FC0_0000]);
}

/// ** Los saltos y los bucles del Programa (lo que llega con LB5): un `si`
/// con su `si no`, y un bucle que cuenta hasta lo que entra, con su romper.
#[test]
fn los_saltos_y_los_bucles_dan_los_bits_del_interprete() {
    // si a > 0: d = a * 2 si no: d = -a
    let si = programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Compara { d: 1, a: 0, b: 2, como: Comparacion::Mayor, entero: false },
            Op::Si { c: 1 },
            Op::Mul { d: 4, a: 0, b: 3 },
            Op::SiNo,
            Op::Sub { d: 4, a: 2, b: 0 },
            Op::FinSi,
            Op::Salida { s: 4, elemento: 0, componente: 0 },
        ],
        vec![0.0, 0.0, 0.0, 2.0, 0.0],
        1,
    );
    contra_la_casa("Si/SiNo", &si, &f32s());
    // s = 0; i = 0; bucle { romper si i >= n; s = s + i; i = i + 1 } (enteros)
    let bucle = programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Bucle,
            Op::Compara { d: 3, a: 2, b: 0, como: Comparacion::MayorIgualSinSigno, entero: true },
            Op::RomperSi { c: 3, si_cero: false },
            Op::SumaEntera { d: 1, a: 1, b: 2 },
            Op::SumaEntera { d: 2, a: 2, b: 4 },
            Op::FinBucle,
            Op::Salida { s: 1, elemento: 0, componente: 0 },
        ],
        vec![0.0, 0.0, 0.0, 0.0, f32::from_bits(1)],
        1,
    );
    contra_la_casa("Bucle", &bucle, &[0, 1, 2, 10, 100, 1000]);
}

/// ** Lo que no sabe correr, lo DICE: el limite en la operacion que es, con
/// su porque, y ni un byte emitido.
#[test]
fn lo_que_no_sabe_lo_dice_en_su_operacion_y_no_emite() {
    let casos = [
        (Op::Mate { d: 1, a: 0, f: bmo_prometeo::mates::Mate::Sin }, "llama al interprete"),
        (Op::IdHilo { d: 1, que: 0, c: 0 }, "es de computo"),
        (Op::LeeUav { d: 1, u: 0, modo: bmo_prometeo::bufer::Modo::Crudo, i: 0, desp: 0, z: 0 }, "memoria de fuera"),
        (Op::Constantes { d: 1, fila: 0, cb: 0 }, "cbuffer"),
    ];
    for (op, dice) in casos {
        match CPU.emitir(&unaria(op.clone()), Para::Viaje) {
            Err(NoEmite::Limite { op: 1, por_que, .. }) => assert!(por_que.contains(dice), "{op:?}: {por_que}"),
            otro => panic!("{op:?}: el limite en la operacion 1, no {otro:?}"),
        }
    }
}

/// Un codigo hecho a mano, para el juez.
fn codigo(bytes: &[u8]) -> Codigo {
    Codigo { bytes: bytes.to_vec(), instrucciones: 1, registros: 4 }
}

/// ** EL JUEZ DICE QUE NO (L4: una regla se prueba diciendo que no): cada
/// forma de romper el contrato de la llamada, con su motivo.
#[test]
fn el_juez_dice_que_no_a_cada_forma_de_romper_la_llamada() {
    let casos: [(&[u8], &str); 6] = [
        (&[0x31, 0xDB, 0xC3], "no conserva rbx"),                                  // xor ebx, ebx; ret
        (&[0xEB, 0xFE], "no vuelve"),                                              // jmp $
        (&[0xB8, 1, 0, 0, 0, 0xC3], "eax = 1"),                                    // mov eax, 1; ret
        (&[0x89, 0x87, 0, 0, 0x10, 0, 0x31, 0xC0, 0xC3], "escribe fuera"),         // mov [rdi+0x100000], eax; xor eax, eax; ret
        (&[0x31, 0xC0, 0x89, 0x06, 0xC3], "escribe en sus entradas"),              // xor eax, eax; mov [rsi], eax; ret
        (&[0x48, 0x83, 0xEC, 0x08, 0x31, 0xC0, 0xC3], "no vuelve"),                // sub rsp, 8; ...; ret (a otro sitio)
    ];
    for (bytes, dice) in casos {
        match CPU.juzgar(&codigo(bytes), Para::Viaje) {
            Err(por_que) => assert!(por_que.contains(dice), "{bytes:x?}: {por_que}"),
            Ok(()) => panic!("{bytes:x?}: el juez tenia que decir que NO ({dice})"),
        }
    }
    // Y uno que cumple: `xor eax, eax; ret`.
    assert_eq!(CPU.juzgar(&codigo(&[0x31, 0xC0, 0xC3]), Para::Viaje), Ok(()));
    // Sus techos.
    assert!(CPU.juzgar(&Codigo { bytes: vec![0x31, 0xC0, 0xC3], instrucciones: 1, registros: REGISTROS + 1 }, Para::Viaje).is_err());
    assert!(CPU.juzgar(&codigo(&[]), Para::Viaje).is_err());
}

/// ** El prologo pone CADA registro: un cuerpo que lee una constante da lo
/// mismo aunque el sitio que le dan venga sucio (el emulador lo da a cero;
/// aqui se mira que el codigo no dependa de eso: la constante es 0.5).
#[test]
fn el_prologo_pone_los_registros_y_la_celda_corre_lo_que_dice() {
    let p = programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Mul { d: 2, a: 0, b: 1 }, Op::Salida { s: 2, elemento: 0, componente: 0 }], vec![0.0, 0.5, 0.0], 1);
    let c = CPU.emitir(&p, Para::Viaje).unwrap();
    assert_eq!(&c.bytes[..2], &[0xC7, 0x87], "empieza por el prologo");
    assert_eq!(c.registros, 3);
    assert!(c.instrucciones >= 4, "{} instrucciones", c.instrucciones);
    assert_eq!(CPU.simular(&c, &[[3.0f32.to_bits(), 0, 0, 0]], 1).unwrap()[0][0], 1.5f32.to_bits());
    // lo mismo para el oraculo y para el viaje
    assert_eq!(CPU.emitir(&p, Para::Oraculo).unwrap(), c);
}
