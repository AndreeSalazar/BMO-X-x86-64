//! E6 (02-10): el emisor con `si` y bucles, contra la casa. Cada programa
//! se emite (con los dos ABI), se corre en el simulador y se compara BIT A
//! BIT con `Programa::correr` -- los cuatro de `dxil::ejemplos` y cientos
//! hechos al azar (si dentro de bucles dentro de si, romper desde un si,
//! variables que cruzan ramas, contadores enteros).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Op, Programa, Reg};

use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_de_app, juzgar_drenado, Contexto, RESERVADOS};

use crate::pruebas::{igual, igual_en_registros};
use crate::{emitir, emitir_con, Abi, Emitido, NoEmite};

const TECHO: u32 = 64;

/// Entradas con lo que suele romper: NaN, -0, negativos, grandes.
const ENTRADAS: [(f32, f32); 10] = [(2.0, 3.0), (3.0, 2.0), (1.9, 1.0), (0.5, 1.0), (-1.0, 0.25), (0.0, -0.0), (f32::NAN, 1.0), (1.0, f32::NAN), (1e30, -1e30), (7.25, 0.125)];

/// [!] Un bucle que no sale no sale en la casa NI en la 3060: con
/// `(NaN, 1)` el geometrico no llega nunca (`acc >= NaN` es falso), y con `y
/// <= 0` el anidado tampoco. A cada programa, las entradas con que ACABA.
const ACABA_GEOMETRICO: [(f32, f32); 7] = [(2.0, 3.0), (3.0, 2.0), (1.9, 1.0), (0.5, 1.0), (-1.0, 0.25), (0.0, -0.0), (1.5, 1.0)];
const ACABA_ANIDADO: [(f32, f32); 6] = [(2.0, 1.0), (0.5, 1.0), (3.0, 2.0), (7.25, 0.125), (0.0, 1.0), (-1.0, 0.5)];

fn los_dos(p: &Programa) {
    los_dos_con(p, &ENTRADAS);
}

/// El juez (J1, con R8 y R9 de E6): PERFECTO Y PRECISO con los saltos
/// drenados; y el cuerpo del ABI de registros -- el de la puerta de las apps
/// --, dentro de la lista blanca (R7).
fn juzgado(e: &Emitido, r: &Emitido) {
    for x in [e, r] {
        let v = juzgar_drenado(&x.codigo, &Contexto { registros: x.registros + RESERVADOS, sph: None });
        assert!(v.is_ok(), "{}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
    }
    assert_eq!(juzgar_cuerpo_de_app(&r.codigo, r.registros), Ok(()));
}

fn los_dos_con(p: &Programa, entradas: &[(f32, f32)]) {
    let e = emitir(p, TECHO).unwrap();
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap();
    juzgado(&e, &r);
    for &(x, y) in entradas {
        let ent = [[x, y, 0.0, 0.0]];
        igual(p, &e.codigo, &ent, &[]);
        igual_en_registros(p, &r, &ent, &[]);
    }
}

/// Las instrucciones por su opcode (los 9 bits de abajo).
fn cuantas(codigo: &[(u64, u64)], op: u64) -> usize {
    codigo.iter().filter(|w| w.0 & 0x1FF == op).count()
}

#[test]
fn un_si_emitido_da_los_bits_de_la_casa() {
    let p = ejemplos::si_sino();
    los_dos(&p);
    let e = emitir(&p, TECHO).unwrap();
    // La Compara va FUNDIDA a P0: un FSETP, ni un SEL; dos BRA (el del Si y
    // el del SiNo).
    assert_eq!((cuantas(&e.codigo, 0x00B), cuantas(&e.codigo, 0x007), cuantas(&e.codigo, 0x147)), (1, 0, 2));
}

#[test]
fn un_bucle_emitido_vuelve_y_sale() {
    let p = ejemplos::bucle_geometrico();
    los_dos_con(&p, &ACABA_GEOMETRICO);
    let e = emitir(&p, TECHO).unwrap();
    // Un salto hacia ATRAS (el del FinBucle) y uno hacia delante (el Romper).
    let atras = e.codigo.iter().filter(|w| w.0 & 0x1FF == 0x147 && w.1 & 0x2_0000 != 0).count();
    assert_eq!((cuantas(&e.codigo, 0x147), atras), (2, 1));
}

#[test]
fn un_contador_entero_y_elige_emitidos() {
    let p = ejemplos::bucle_entero();
    los_dos(&p);
    let e = emitir(&p, TECHO).unwrap();
    // ISETP (el contador), IADD3 (sumarle uno) y SEL (el Elige).
    assert!(cuantas(&e.codigo, 0x00C) >= 1 && cuantas(&e.codigo, 0x010) == 1 && cuantas(&e.codigo, 0x007) == 1);
}

#[test]
fn bucles_anidados_emitidos() {
    los_dos_con(&ejemplos::anidado(), &ACABA_ANIDADO);
}

#[test]
fn un_si_mal_cerrado_no_se_emite() {
    let mut p = ejemplos::si_sino();
    p.ops.retain(|o| !matches!(o, Op::FinSi));
    assert_eq!(emitir(&p, TECHO), Err(NoEmite::Forma(p.ops.len())));
}

// -- Al azar ----------------------------------------------------------------

/// Un generador de numeros chico y repetible.
struct Azar(u64);

impl Azar {
    fn n(&mut self, hasta: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % hasta as u64) as u32
    }
}

/// Lo que va haciendo el generador.
struct Gen {
    az: Azar,
    ops: Vec<Op>,
    /// Lo que se puede leer: entradas, constantes, variables y lo calculado.
    legibles: Vec<Reg>,
    /// Las variables (se escriben con Copia).
    variables: Vec<Reg>,
    siguiente: Reg,
    /// Las constantes enteras 0, 1 y el tope de cada bucle (por su registro).
    cero: Reg,
    uno: Reg,
    topes: [Reg; 3],
}

impl Gen {
    fn nuevo(&mut self) -> Reg {
        self.siguiente += 1;
        self.siguiente
    }

    fn legible(&mut self) -> Reg {
        let k = self.az.n(self.legibles.len() as u32) as usize;
        self.legibles[k]
    }

    fn cuenta(&mut self) {
        // E6b: a veces una fila del cbuffer, donde caiga (dentro de un bucle,
        // como la repite `dxc`): con el ABI de registros es una precarga que
        // tiene que vivir hasta el fin del bucle.
        // Se usa ahi mismo, como hace `dxc` (cargar y usar en el bloque): un
        // valor no se lee por un camino que no paso por su definicion.
        let fila = if self.az.n(6) == 0 {
            let d = self.nuevo();
            for _ in 0..3 {
                self.nuevo();
            }
            self.ops.push(Op::Constantes { d, fila: self.az.n(3) as u16 });
            Some(d + self.az.n(4) as Reg)
        } else {
            None
        };
        let (a, b, c) = (fila.unwrap_or_else(|| self.legible()), self.legible(), self.legible());
        let d = self.nuevo();
        self.ops.push(match self.az.n(9) {
            0 => Op::Add { d, a, b },
            1 => Op::Mul { d, a, b },
            2 => Op::Sub { d, a, b },
            3 => Op::Min { d, a, b },
            4 => Op::Max { d, a, b },
            5 => Op::Mad { d, a, b, c },
            6 => Op::Saturate { d, a },
            7 => Op::Abs { d, a },
            _ => Op::Dot { d, n: 2, a: [a, b, 0, 0], b: [c, a, 0, 0] },
        });
        self.legibles.push(d);
        if self.az.n(2) == 0 {
            let v = self.variables[self.az.n(self.variables.len() as u32) as usize];
            self.ops.push(Op::Copia { d: v, a: d });
        }
    }

    fn compara(&mut self) -> Reg {
        let (a, b) = (self.legible(), self.legible());
        let d = self.nuevo();
        let como = [Comparacion::Menor, Comparacion::MenorIgual, Comparacion::Mayor, Comparacion::MayorIgual, Comparacion::Igual, Comparacion::Distinto][self.az.n(6) as usize];
        self.ops.push(Op::Compara { d, a, b, como, entero: false });
        d
    }

    fn cuerpo(&mut self, hondo: u32, bucles: u32) {
        for _ in 0..1 + self.az.n(4) {
            match self.az.n(if hondo < 3 { 7 } else { 3 }) {
                0..=2 => self.cuenta(),
                3 | 4 => {
                    let c = self.compara();
                    // A veces la comparacion se usa dos veces: no se funde.
                    if self.az.n(4) == 0 {
                        let (a, b) = (self.legible(), self.legible());
                        let d = self.nuevo();
                        self.ops.push(Op::Elige { d, c, a, b });
                        self.legibles.push(d);
                    }
                    self.ops.push(Op::Si { c });
                    self.cuerpo(hondo + 1, bucles);
                    if bucles > 0 && self.az.n(5) == 0 {
                        self.ops.push(Op::Romper);
                    } else if bucles > 0 && self.az.n(5) == 0 {
                        // E6b: el contador se suma AL EMPEZAR la vuelta: un
                        // continue no hace el bucle eterno.
                        self.ops.push(Op::Continuar);
                    }
                    if self.az.n(2) == 0 {
                        self.ops.push(Op::SiNo);
                        self.cuerpo(hondo + 1, bucles);
                    }
                    self.ops.push(Op::FinSi);
                }
                5 => {
                    let c = self.compara();
                    let (a, b) = (self.legible(), self.legible());
                    let d = self.nuevo();
                    self.ops.push(Op::Elige { d, c, a, b });
                    self.legibles.push(d);
                }
                _ => {
                    // Un bucle con contador entero (acaba siempre), y a veces
                    // un RomperSi de una comparacion de floats.
                    let i = self.nuevo();
                    self.ops.push(Op::Copia { d: i, a: self.cero });
                    self.ops.push(Op::Bucle);
                    let c = self.nuevo();
                    let tope = self.topes[self.az.n(3) as usize];
                    self.ops.push(Op::Compara { d: c, a: i, b: tope, como: Comparacion::MayorIgual, entero: true });
                    self.ops.push(Op::RomperSi { c, si_cero: false });
                    let t = self.nuevo();
                    self.ops.push(Op::SumaEntera { d: t, a: i, b: self.uno });
                    self.ops.push(Op::Copia { d: i, a: t });
                    self.cuerpo(hondo + 1, bucles + 1);
                    if self.az.n(3) == 0 {
                        let c = self.compara();
                        self.ops.push(Op::RomperSi { c, si_cero: self.az.n(2) == 0 });
                    }
                    self.ops.push(Op::FinBucle);
                }
            }
        }
    }
}

/// Un programa al azar: dos entradas, cuatro variables (las cuatro salidas)
/// y lo que salga.
fn al_azar(semilla: u64) -> Programa {
    let mut g = Gen { az: Azar(semilla), ops: Vec::new(), legibles: Vec::new(), variables: vec![2, 3, 4, 5], siguiente: 40, cero: 10, uno: 11, topes: [12, 13, 14] };
    g.ops.push(Op::Entrada { d: 0, elemento: 0, componente: 0 });
    g.ops.push(Op::Entrada { d: 1, elemento: 0, componente: 1 });
    for v in 2..6 {
        g.ops.push(Op::Copia { d: v, a: 20 + v });
    }
    g.legibles = vec![0, 1, 2, 3, 4, 5, 20, 21, 22, 23];
    g.cuerpo(0, 0);
    for (k, v) in (2..6u16).enumerate() {
        g.ops.push(Op::Salida { s: v, elemento: 0, componente: k as u8 });
    }
    let n = g.siguiente as usize + 1;
    let bits = f32::from_bits;
    ejemplos::programa(
        g.ops,
        n,
        &[(10, bits(0)), (11, bits(1)), (12, bits(1)), (13, bits(2)), (14, bits(4)), (20, 0.5), (21, -2.0), (22, 3.0), (23, 1.25), (24, 0.0), (25, 1.0)],
    )
}

#[test]
fn cientos_de_programas_al_azar_dan_los_bits_de_la_casa() {
    let (mut hechos, mut grandes, mut saltos) = (0, 0, 0);
    for semilla in 0..400u64 {
        let p = al_azar(semilla);
        assert_eq!(p.forma(), Ok(()), "semilla {semilla}");
        match emitir(&p, TECHO) {
            Err(NoEmite::Registros) => {
                grandes += 1;
                continue;
            }
            r => {
                let e = r.unwrap_or_else(|x| panic!("semilla {semilla}: {x:?}"));
                saltos += cuantas(&e.codigo, 0x147);
            }
        }
        let r = match emitir_con(&p, TECHO, Abi::Registros) {
            // Con el ABI de registros las precargas ocupan mas: puede no caber.
            Err(NoEmite::Registros) => {
                grandes += 1;
                continue;
            }
            r => r.unwrap_or_else(|x| panic!("semilla {semilla}: {x:?}")),
        };
        let e = emitir(&p, TECHO).unwrap();
        juzgado(&e, &r);
        let cb: Vec<u8> = (0..12).flat_map(|k| (0.25 * k as f32 - 1.0).to_le_bytes()).collect();
        for (x, y) in ENTRADAS {
            let ent = [[x, y, 0.0, 0.0]];
            igual(&p, &e.codigo, &ent, &cb);
            igual_en_registros(&p, &r, &ent, &cb);
        }
        hechos += 1;
    }
    // Que la prueba pruebe: casi todos se emiten, con muchos saltos.
    assert!(hechos >= 350 && saltos > 1000, "{hechos} emitidos, {grandes} sin registros, {saltos} saltos");
}



/// De punta a punta: un SM5 de FXC con `loop`, `breakc`, `if`/`else`,
/// `iadd`, `movc`, `if_z` y `break` -> el lector -> el emisor -> la 3060 de
/// mentira, bit a bit con la casa, y PERFECTO ante el juez.
#[test]
fn el_sm5_con_saltos_de_punta_a_punta() {
    for f in [ejemplos::sm5_bucle, ejemplos::sm5_si_cero] {
        let (t, ent, sal) = f();
        let p = bmo_proton_x::sm5::compilar(&t, &ent, &sal).unwrap();
        assert!(p.salta());
        los_dos(&p);
    }
}

// -- E6b: el DXIL de dxc con saltos, de punta a punta ------------------------

const DXIL_SALTOS: &[u8] = include_bytes!("../../proton-x/prueba/saltos.dxil");
const DXIL_ANIDADO: &[u8] = include_bytes!("../../proton-x/prueba/anidado.dxil");
const DXIL_MIENTRAS: &[u8] = include_bytes!("../../proton-x/prueba/mientras.dxil");

/// dxc -> el lector -> la estructura -> el emisor -> la 3060 de mentira: bit a
/// bit con la casa (los dos ABI), PERFECTO ante el juez y dentro de R7. El
/// cbuffer `k`, con el que los tres acaban (`proton-x`, `pruebas_saltos`).
#[test]
fn el_dxil_de_dxc_con_saltos_de_punta_a_punta() {
    let ks: [[f32; 4]; 3] = [[2.0, 1.0, 0.75, 3.0], [0.5, 1.0, 1.5, 100.0], [5.0, 1.0, -0.25, 1.0]];
    let uv: [(f32, f32); 6] = [(0.1, 0.9), (0.9, 0.1), (0.7, 0.7), (3.0, 0.25), (-0.5, 0.6), (0.0, -0.0)];
    for d in [DXIL_SALTOS, DXIL_ANIDADO, DXIL_MIENTRAS] {
        let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(d).unwrap()).unwrap();
        assert!(p.salta());
        let e = emitir(&p, TECHO).unwrap();
        let r = emitir_con(&p, TECHO, Abi::Registros).unwrap();
        juzgado(&e, &r);
        for k in ks {
            let cb: Vec<u8> = k.iter().flat_map(|v| v.to_le_bytes()).collect();
            for (x, y) in uv {
                let ent = [[0.0; 4], [x, y, 0.0, 0.0]];
                igual(&p, &e.codigo, &ent, &cb);
                igual_en_registros(&p, &r, &ent, &cb);
            }
        }
    }
}
