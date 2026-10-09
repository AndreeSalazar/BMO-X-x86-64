//! ** E8b (09-10): los ARRAYS en la 3060 (`indexado.rs`) -- leer y escribir
//! con el indice calculado, en los dos ABI y juzgados: los bits de la casa,
//! tambien fuera del array (se lee 0 y no se escribe) y con los bits de un
//! negativo o de un NaN como indice.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};
use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Conversion, Op, Programa, Reg};

use crate::pruebas::{igual, igual_en_registros};
use crate::{emitir, emitir_con, Abi, Emitido, NoEmite};

const TECHO: u32 = 64;

fn bits(x: u32) -> f32 {
    f32::from_bits(x)
}

/// Los dos ABI, juzgados (R0..R6, y R7 el de registros).
fn emitidos(p: &Programa) -> (Emitido, Emitido) {
    let e = emitir(p, TECHO).unwrap_or_else(|x| panic!("{:?}", x));
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap_or_else(|x| panic!("{:?}", x));
    for x in [&e, &r] {
        let v = juzgar_drenado(&x.codigo, &Contexto { registros: x.registros + RESERVADOS, sph: None });
        assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(juzgar_cuerpo_de_app(&r.codigo, r.registros), Ok(()));
    (e, r)
}

fn los_dos(p: &Programa, entradas: &[[f32; 4]]) {
    let (e, r) = emitidos(p);
    for ent in entradas {
        igual(p, &e.codigo, &[*ent], &[]);
        igual_en_registros(p, &r, &[*ent], &[]);
    }
}

/// Indices de todas las clases: dentro, el borde, fuera, y los bits de un
/// negativo, de un float y de un NaN.
fn indices() -> Vec<u32> {
    let mut v: Vec<u32> = (0..12).collect();
    v.extend([0xFFFF_FFFF, 0x8000_0000, 0x7FFF_FFFF, 0x3F80_0000, 0x7FC0_0000, 1 << 16, 256]);
    v
}

/// ** Una TABLA constante (`static const float pesos[4]`): solo se lee.
#[test]
fn una_tabla_constante_leida_con_su_indice() {
    let p = ejemplos::programa(
        vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::LeeIndexado { d: 1, base: 10, n: 4, i: 0 }, Op::Salida { s: 1, elemento: 0, componente: 0 }],
        14,
        &[(10, 0.25), (11, -0.5), (12, 1.0), (13, f32::NAN)],
    );
    let ent: Vec<[f32; 4]> = indices().into_iter().map(|i| [bits(i), 0.0, 0.0, 0.0]).collect();
    los_dos(&p, &ent);
}

/// ** Un array LOCAL (`float a[6]`, a cero) escrito en un bucle
/// (`a[k] = x * k`) y leido con el indice de la entrada.
#[test]
fn un_array_local_escrito_en_un_bucle_y_leido() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 0, componente: 1 },
            Op::Copia { d: 2, a: 40 },
            Op::Bucle,
            Op::Compara { d: 3, a: 2, b: 41, como: Comparacion::MayorIgual, entero: true },
            Op::RomperSi { c: 3, si_cero: false },
            Op::Convierte { d: 4, a: 2, como: Conversion::EnteroAFloat },
            Op::Mul { d: 5, a: 0, b: 4 },
            Op::EscribeIndexado { base: 20, n: 6, i: 2, s: 5 },
            Op::SumaEntera { d: 6, a: 2, b: 42 },
            Op::Copia { d: 2, a: 6 },
            Op::FinBucle,
            Op::LeeIndexado { d: 7, base: 20, n: 6, i: 1 },
            Op::Salida { s: 7, elemento: 0, componente: 0 },
        ],
        43,
        &[(40, bits(0)), (41, bits(6)), (42, bits(1))],
    );
    let mut ent = Vec::new();
    for x in [1.5f32, -2.0, 0.0, f32::INFINITY, f32::NAN] {
        for i in indices() {
            ent.push([x, bits(i), 0.0, 0.0]);
        }
    }
    los_dos(&p, &ent);
}

/// ** El indice ESCRITO (el `a[2]` del DXIL): escribir y leer sin cadena.
#[test]
fn con_el_indice_escrito() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::EscribeIndexado { base: 20, n: 4, i: 50, s: 0 },
            Op::EscribeIndexado { base: 20, n: 4, i: 53, s: 0 },
            Op::LeeIndexado { d: 1, base: 20, n: 4, i: 51 },
            Op::LeeIndexado { d: 2, base: 20, n: 4, i: 52 },
            Op::LeeIndexado { d: 3, base: 20, n: 4, i: 53 },
            Op::Add { d: 4, a: 1, b: 2 },
            Op::Add { d: 5, a: 4, b: 3 },
            Op::Salida { s: 5, elemento: 0, componente: 0 },
        ],
        54,
        &[(23, 7.0), (50, bits(2)), (51, bits(2)), (52, bits(3)), (53, bits(9))],
    );
    let ent: Vec<[f32; 4]> = [1.0f32, -3.25, 1e30, f32::NAN].iter().map(|&x| [x, 0.0, 0.0, 0.0]).collect();
    los_dos(&p, &ent);
}

const ARREGLOS: &[u8] = include_bytes!("../../proton-x/prueba/arreglos.dxil");

/// *** `arreglos.hlsl` (de `dxc`; el de N5.10, lo que pedian los pixeles de
/// Cyberpunk): un array local con indice calculado, uno de dos dimensiones
/// escrito en un bucle y dos tablas globales (float e int) -- de punta a
/// punta en la 3060: el lector, el emisor, el juez y el simulador, con los
/// bits de la casa en los dos ABI --. Antes iba por la CPU.
#[test]
fn el_pixel_de_dxc_con_arrays_de_punta_a_punta() {
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(ARREGLOS).unwrap()).unwrap();
    assert!(p.ops.iter().any(|o| matches!(o, Op::LeeIndexado { .. })) && p.ops.iter().any(|o| matches!(o, Op::EscribeIndexado { .. })));
    let (e, r) = emitidos(&p);
    for i in indices() {
        for x in [[1.0f32, 2.0, 3.0, 4.0], [-0.5, f32::NAN, 1e30, -0.0], [0.1, 0.2, 0.3, 0.4]] {
            let ent = [[0.0; 4], [bits(i), 0.0, 0.0, 0.0], x];
            igual(&p, &e.codigo, &ent, &[]);
            igual_en_registros(&p, &r, &ent, &[]);
        }
    }
    std::eprintln!("arreglos.hlsl: {} instrucciones ({} con el ABI de registros), {} registros", e.codigo.len(), r.codigo.len(), e.registros);
}

/// Por la PUERTA del kernel: el cuerpo (ABI de registros) con el pegamento de
/// pixel del driver, y el juez de programas. Cuantas instrucciones, o lo que
/// dice el pegamento.
fn por_la_puerta(p: &Programa, genericos: &[Option<u8>]) -> Result<usize, bmo_gpu_ga10x::pegamento::NoPega> {
    use bmo_gpu_ga10x::pegamento::{self, Datos};
    use bmo_gpu_ga10x::sass::juez;
    use bmo_gpu_ga10x::tuberia;
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap();
    let pegado = pegamento::pixel(&r.codigo, r.registros, &crate::pso::cargas(&r), Datos { filas: p.filas_cb as u32, paso: 0, elementos: &[] }, genericos)?;
    let mut b = vec![0u8; tuberia::HUECO];
    let n = pegado.bytes(&mut b);
    let v = juez::juzgar_programa(&b[..n], tuberia::REGISTROS).unwrap_or_else(|x| panic!("{x}"));
    assert!(v.instrucciones <= juez::MAX_INSTRUCCIONES);
    Ok(v.instrucciones)
}

/// ** LA PUERTA de 128: una tabla leida con su indice y un array local
/// escrito en un bucle, pegados, CABEN y el juez de programas los da por
/// buenos. `arreglos.hlsl` entero -- siete accesos con indice calculado, dos
/// bucles y la division entre 3 -- no cabe: el pegamento lo dice
/// (`NoPega::Instrucciones`) y su PSO va por la CPU, como antes de E8b.
#[test]
fn los_arrays_y_la_puerta_de_128() {
    use bmo_gpu_ga10x::pegamento::NoPega;
    let tabla = ejemplos::programa(
        vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::LeeIndexado { d: 1, base: 10, n: 4, i: 0 }, Op::Salida { s: 1, elemento: 0, componente: 0 }],
        14,
        &[(10, 0.25), (11, -0.5), (12, 1.0), (13, 2.0)],
    );
    let bucle = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 0, componente: 1 },
            Op::Copia { d: 2, a: 40 },
            Op::Bucle,
            Op::Compara { d: 3, a: 2, b: 41, como: Comparacion::MayorIgual, entero: true },
            Op::RomperSi { c: 3, si_cero: false },
            Op::Convierte { d: 4, a: 2, como: Conversion::EnteroAFloat },
            Op::Mul { d: 5, a: 0, b: 4 },
            Op::EscribeIndexado { base: 20, n: 6, i: 2, s: 5 },
            Op::SumaEntera { d: 6, a: 2, b: 42 },
            Op::Copia { d: 2, a: 6 },
            Op::FinBucle,
            Op::LeeIndexado { d: 7, base: 20, n: 6, i: 1 },
            Op::Salida { s: 7, elemento: 0, componente: 0 },
        ],
        43,
        &[(40, bits(0)), (41, bits(6)), (42, bits(1))],
    );
    for (nombre, p) in [("una tabla", &tabla), ("un array en un bucle", &bucle)] {
        let n = por_la_puerta(p, &[Some(0)]).unwrap_or_else(|x| panic!("{nombre}: {x:?}"));
        std::eprintln!("{nombre}: {n} instrucciones pegado");
    }
    let dxc = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(ARREGLOS).unwrap()).unwrap();
    assert_eq!(por_la_puerta(&dxc, &[None, Some(0), Some(1)]), Err(NoPega::Instrucciones));
}

/// ** Un array que se sale de los registros del Programa no se emite -- la
/// casa solo toca el elemento del indice; aqui se miran todos --: va por la
/// CPU, y el emisor no se cae.
#[test]
fn un_array_mas_alla_de_los_registros_no_se_emite() {
    for op in [Op::LeeIndexado { d: 1, base: 2, n: 8, i: 0 }, Op::EscribeIndexado { base: 2, n: 8, i: 0, s: 0 }] {
        let p = ejemplos::programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, op, Op::Salida { s: 1, elemento: 0, componente: 0 }], 4, &[]);
        assert_eq!(emitir(&p, TECHO).err(), Some(NoEmite::Operacion(1)), "{:?}", op);
        assert_eq!(emitir_con(&p, TECHO, Abi::Registros).err(), Some(NoEmite::Operacion(1)), "{:?}", op);
    }
}

/// ** El indice es un ELEMENTO del mismo array (`a[a[0]] = x`, `a[a[1]]`):
/// la casa mira sus bits UNA vez, antes de escribir; la cadena tambien --
/// si no, el elemento que cambia en medio la desvia: con `a[0] = 0` y
/// `x = 3`, `a[a[0]] = x` deja `a[0] = 3` y despues escribiria `a[3]` --.
#[test]
fn el_indice_es_un_elemento_del_mismo_array() {
    let p = ejemplos::programa(
        vec![
            Op::Entrada { d: 0, elemento: 0, componente: 0 },
            Op::Entrada { d: 1, elemento: 0, componente: 1 },
            Op::EscribeIndexado { base: 20, n: 4, i: 50, s: 1 },
            Op::EscribeIndexado { base: 20, n: 4, i: 20, s: 0 },
            Op::LeeIndexado { d: 2, base: 20, n: 4, i: 21 },
            Op::LeeIndexado { d: 3, base: 20, n: 4, i: 20 },
            Op::LeeIndexado { d: 4, base: 20, n: 4, i: 53 },
            Op::LeeIndexado { d: 21, base: 20, n: 4, i: 21 },
            Op::LeeIndexado { d: 5, base: 20, n: 4, i: 51 },
            Op::Salida { s: 2, elemento: 0, componente: 0 },
            Op::Salida { s: 3, elemento: 0, componente: 1 },
            Op::Salida { s: 4, elemento: 0, componente: 2 },
            Op::Salida { s: 5, elemento: 0, componente: 3 },
        ],
        54,
        &[(21, bits(2)), (22, bits(1)), (23, bits(0)), (50, bits(0)), (51, bits(1)), (53, bits(3))],
    );
    let mut ent = Vec::new();
    for x in [0u32, 1, 2, 3, 4, 0xFFFF_FFFF, 0x3FC0_0000] {
        for i in [0u32, 1, 2, 3, 4, 7, 0x8000_0000] {
            ent.push([bits(x), bits(i), 0.0, 0.0]);
        }
    }
    los_dos(&p, &ent);
}

/// Un azar chico.
struct Azar(u64);

impl Azar {
    fn n(&mut self, m: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % m as u64) as u32
    }
}

/// Un programa al azar con dos arrays -- uno que se escribe (A, a cero) y
/// una tabla constante (B) -- leidos y escritos con indices escritos, de la
/// entrada, del contador de un bucle y de los elementos de A, dentro de
/// `si` y de bucles.
fn al_azar(semilla: u64) -> Programa {
    let mut z = Azar(semilla);
    let (a, na) = (100 as Reg, 1 + z.n(8) as u16);
    let (b, nb) = (120 as Reg, 1 + z.n(8) as u16);
    // 200..: valores; 300..: constantes de indice; 0/1 las entradas
    let mut ops = vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }];
    let mut iniciales: Vec<(Reg, f32)> = (0..nb).map(|k| (b + k, (k as f32 - 2.0) * 1.25)).collect();
    let mut siguiente: Reg = 200;
    let mut cte: Reg = 300;
    // Los valores: los floats de la entrada y tambien bits chicos (el
    // indice de la entrada, las constantes de indice), que escritos en A
    // sirven de indice despues.
    let mut valores: Vec<Reg> = vec![0, 1];
    let nuevo = |s: &mut Reg| {
        *s += 1;
        *s
    };
    // El indice: el contador, la entrada, un ELEMENTO de A (sus bits; el
    // de `a[a[0]]`), o una constante.
    let indice = |z: &mut Azar, cte: &mut Reg, iniciales: &mut Vec<(Reg, f32)>, contador: Option<Reg>| -> Reg {
        match (z.n(4), contador) {
            (0, Some(k)) => k,
            (1, _) => 1,
            (2, _) => a + z.n(na as u32) as Reg,
            _ => {
                *cte += 1;
                iniciales.push((*cte, bits(z.n(10))));
                *cte
            }
        }
    };
    let cuerpo = |ops: &mut Vec<Op>, z: &mut Azar, contador: Option<Reg>, siguiente: &mut Reg, cte: &mut Reg, iniciales: &mut Vec<(Reg, f32)>, valores: &mut Vec<Reg>| {
        for _ in 0..1 + z.n(5) {
            let i = indice(z, cte, iniciales, contador);
            if i > 300 {
                valores.push(i);
            }
            match z.n(4) {
                0 => {
                    let s = valores[z.n(valores.len() as u32) as usize];
                    ops.push(Op::EscribeIndexado { base: a, n: na, i, s });
                }
                1 => {
                    let d = nuevo(siguiente);
                    ops.push(Op::LeeIndexado { d, base: a, n: na, i });
                    valores.push(d);
                }
                2 => {
                    let d = nuevo(siguiente);
                    ops.push(Op::LeeIndexado { d, base: b, n: nb, i });
                    valores.push(d);
                }
                _ => {
                    let (x, y) = (valores[z.n(valores.len() as u32) as usize], valores[z.n(valores.len() as u32) as usize]);
                    let d = nuevo(siguiente);
                    ops.push(Op::Add { d, a: x, b: y });
                    valores.push(d);
                }
            }
        }
    };
    cuerpo(&mut ops, &mut z, None, &mut siguiente, &mut cte, &mut iniciales, &mut valores);
    // un si sobre la entrada
    let c = nuevo(&mut siguiente);
    ops.push(Op::Compara { d: c, a: 0, b: valores[valores.len() - 1], como: Comparacion::Menor, entero: false });
    ops.push(Op::Si { c });
    let mut dentro = valores.clone();
    cuerpo(&mut ops, &mut z, None, &mut siguiente, &mut cte, &mut iniciales, &mut dentro);
    ops.push(Op::FinSi);
    // un bucle con su contador entero, que indexa
    let k = 250 as Reg;
    iniciales.extend([(251, bits(0)), (252, bits(1 + z.n(na as u32 + 2))), (253, bits(1))]);
    ops.push(Op::Copia { d: k, a: 251 });
    ops.push(Op::Bucle);
    let fin = nuevo(&mut siguiente);
    ops.push(Op::Compara { d: fin, a: k, b: 252, como: Comparacion::MayorIgual, entero: true });
    ops.push(Op::RomperSi { c: fin, si_cero: false });
    let mut en_bucle = valores.clone();
    cuerpo(&mut ops, &mut z, Some(k), &mut siguiente, &mut cte, &mut iniciales, &mut en_bucle);
    let k1 = nuevo(&mut siguiente);
    ops.push(Op::SumaEntera { d: k1, a: k, b: 253 });
    ops.push(Op::Copia { d: k, a: k1 });
    ops.push(Op::FinBucle);
    // lo que sale: A en la entrada, A en 0, y el ultimo valor de fuera
    let (s0, s1) = (nuevo(&mut siguiente), nuevo(&mut siguiente));
    ops.push(Op::LeeIndexado { d: s0, base: a, n: na, i: 1 });
    ops.push(Op::LeeIndexado { d: s1, base: a, n: na, i: 251 });
    ops.push(Op::Salida { s: s0, elemento: 0, componente: 0 });
    ops.push(Op::Salida { s: s1, elemento: 0, componente: 1 });
    ops.push(Op::Salida { s: valores[valores.len() - 1], elemento: 0, componente: 2 });
    ejemplos::programa(ops, 400, &iniciales)
}

/// *** CIENTOS DE PROGRAMAS al azar con arrays: los bits de la casa en los
/// dos ABI, y el juez dice que si.
#[test]
fn cientos_de_programas_con_arrays() {
    let mut hechos = 0;
    for semilla in 0..300u64 {
        let p = al_azar(semilla);
        assert_eq!(p.forma(), Ok(()), "semilla {semilla}");
        let ent: Vec<[f32; 4]> = [(1.5f32, 0u32), (-2.0, 1), (0.25, 3), (7.0, 5), (f32::NAN, 2), (3.0, 0xFFFF_FFFF), (-0.0, 9)].iter().map(|&(x, i)| [x, bits(i), 0.0, 0.0]).collect();
        los_dos(&p, &ent);
        hechos += 1;
    }
    assert_eq!(hechos, 300);
}
