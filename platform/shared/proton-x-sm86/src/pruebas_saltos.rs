//! E6 (02-10): el emisor con `si` y bucles, contra la casa. Cada programa
//! se emite (con los dos ABI), se corre en el simulador y se compara BIT A
//! BIT con `Programa::correr` -- los cuatro de `dxil::ejemplos` y cientos
//! hechos al azar (si dentro de bucles dentro de si, romper desde un si,
//! variables que cruzan ramas, contadores enteros).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::dxil::ejemplos;
use bmo_proton_x::dxil::programa::{Comparacion, Conversion, Op, OpEntera, Programa, Reg};

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
    /// E6c: los ENTEROS que se pueden leer (contadores, constantes enteras, lo
    /// calculado con enteros). Como en un sombreador de verdad, una de
    /// enteros no lee los bits de un float: el signo y la carga de un NaN
    /// no se modelan, y verlos como entero cambiaria el camino.
    enteros: Vec<Reg>,
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

    fn entero(&mut self) -> Reg {
        let k = self.az.n(self.enteros.len() as u32) as usize;
        self.enteros[k]
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
            self.ops.push(Op::Constantes { d, fila: self.az.n(3) as u16, cb: 0 });
            Some(d + self.az.n(4) as Reg)
        } else {
            None
        };
        let (a, b, c) = (fila.unwrap_or_else(|| self.legible()), self.legible(), self.legible());
        let d = self.nuevo();
        // E6c: enteros y conversiones (sobre los bits de lo que haya); E6d:
        // y la division.
        const ENTERAS: [OpEntera; 16] = [OpEntera::Resta, OpEntera::Mul, OpEntera::Shl, OpEntera::ShrL, OpEntera::ShrA, OpEntera::Y, OpEntera::O, OpEntera::OX, OpEntera::MinS, OpEntera::MaxS, OpEntera::MinU, OpEntera::MaxU, OpEntera::DivU, OpEntera::RemU, OpEntera::DivS, OpEntera::RemS];
        const CONVERSIONES: [Conversion; 4] = [Conversion::EnteroAFloat, Conversion::SinSignoAFloat, Conversion::FloatAEntero, Conversion::FloatASinSigno];
        if self.az.n(4) == 0 {
            let como = CONVERSIONES[self.az.n(4) as usize];
            match self.az.n(3) {
                // De float a entero (un NaN, 0) o de entero a float.
                0 if matches!(como, Conversion::FloatAEntero | Conversion::FloatASinSigno) => {
                    self.ops.push(Op::Convierte { d, a, como });
                    self.enteros.push(d);
                }
                0 => {
                    let a = self.entero();
                    self.ops.push(Op::Convierte { d, a, como });
                    self.legibles.push(d);
                }
                _ => {
                    let (a, b) = (self.entero(), self.entero());
                    let op = ENTERAS[self.az.n(16) as usize];
                    self.ops.push(Op::Entera { d, a, b, op });
                    self.enteros.push(d);
                    // E6d: a veces su pareja (el resto del mismo par, o el
                    // cociente), detras: una cuenta para las dos.
                    let otra = match op {
                        OpEntera::DivU => Some(OpEntera::RemU),
                        OpEntera::RemU => Some(OpEntera::DivU),
                        OpEntera::DivS => Some(OpEntera::RemS),
                        OpEntera::RemS => Some(OpEntera::DivS),
                        _ => None,
                    };
                    if let Some(otra) = otra.filter(|_| self.az.n(2) == 0) {
                        let d2 = self.nuevo();
                        self.ops.push(Op::Entera { d: d2, a, b, op: otra });
                        self.enteros.push(d2);
                    }
                }
            }
            return;
        }
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
        // E6c: a veces de enteros (de su pozo), con o sin signo.
        let (como, entero) = match self.az.n(4) {
            0 => ([Comparacion::MenorSinSigno, Comparacion::MenorIgualSinSigno, Comparacion::MayorSinSigno, Comparacion::MayorIgualSinSigno][self.az.n(4) as usize], true),
            1 => (como, true),
            _ => (como, false),
        };
        let (a, b) = if entero { (self.entero(), self.entero()) } else { (a, b) };
        self.ops.push(Op::Compara { d, a, b, como, entero });
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
                    self.enteros.push(i);
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
    let mut g = Gen { az: Azar(semilla), ops: Vec::new(), legibles: Vec::new(), enteros: vec![10, 11, 12, 13, 14], variables: vec![2, 3, 4, 5], siguiente: 40, cero: 10, uno: 11, topes: [12, 13, 14] };
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
    for f in [ejemplos::sm5_bucle, ejemplos::sm5_si_cero, ejemplos::sm5_switch, ejemplos::sm5_enteros, ejemplos::sm5_division] {
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
const DXIL_ENTEROS: &[u8] = include_bytes!("../../proton-x/prueba/enteros.dxil");
const DXIL_DIVISION: &[u8] = include_bytes!("../../proton-x/prueba/division.dxil");

/// dxc -> el lector -> la estructura -> el emisor -> la 3060 de mentira: bit a
/// bit con la casa (los dos ABI), PERFECTO ante el juez y dentro de R7. El
/// cbuffer `k`, con el que los tres acaban (`proton-x`, `pruebas_saltos`).
#[test]
fn el_dxil_de_dxc_con_saltos_de_punta_a_punta() {
    let ks: [[f32; 4]; 3] = [[2.0, 1.0, 0.75, 3.0], [0.5, 1.0, 1.5, 100.0], [5.0, 1.0, -0.25, 1.0]];
    let uv: [(f32, f32); 6] = [(0.1, 0.9), (0.9, 0.1), (0.7, 0.7), (3.0, 0.25), (-0.5, 0.6), (0.0, -0.0)];
    for d in [DXIL_SALTOS, DXIL_ANIDADO, DXIL_MIENTRAS, DXIL_ENTEROS, DXIL_DIVISION] {
        let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(d).unwrap()).unwrap();
        assert!(p.salta());
        let e = emitir(&p, TECHO).unwrap();
        let r = emitir_con(&p, TECHO, Abi::Registros).unwrap();
        juzgado(&e, &r);
        for k in ks {
            // La fila 1 (los `int4 n` de `enteros.hlsl` y `division.hlsl`):
            // enteros.
            let n: [i32; 4] = [3, -50, 50, k[0] as i32];
            let cb: Vec<u8> = k.iter().flat_map(|v| v.to_le_bytes()).chain(n.iter().flat_map(|v| v.to_le_bytes())).collect();
            for (x, y) in uv {
                let ent = [[0.0; 4], [x, y, 0.0, 0.0]];
                igual(&p, &e.codigo, &ent, &cb);
                igual_en_registros(&p, &r, &ent, &cb);
            }
        }
    }
}

// -- E6c: cada operacion de enteros y cada conversion, sola -----------------

/// `x op y` (o `conv(x)`), con las entradas como BITS, en los dos ABI.
#[test]
fn cada_operacion_entera_sola() {
    let bits = [0u32, 1, 2, 3, 5, 7, 31, 32, 33, 1000, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 0xFFFF_FFFB, 0x3F80_0000, 0xBF80_0000, 0x4F00_0000, 0xCF00_0001, 0x7FC0_0000, 0x7F80_0000, 0xFF80_0000];
    let ops = [OpEntera::Resta, OpEntera::Mul, OpEntera::Shl, OpEntera::ShrL, OpEntera::ShrA, OpEntera::Y, OpEntera::O, OpEntera::OX, OpEntera::MinS, OpEntera::MaxS, OpEntera::MinU, OpEntera::MaxU, OpEntera::DivU, OpEntera::RemU, OpEntera::DivS, OpEntera::RemS];
    let mut programas: Vec<(std::string::String, Programa)> = Vec::new();
    for op in ops {
        let p = ejemplos::programa(
            vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }, Op::Entera { d: 2, a: 0, b: 1, op }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
            3,
            &[],
        );
        programas.push((std::format!("{op:?}"), p));
    }
    for como in [Conversion::EnteroAFloat, Conversion::SinSignoAFloat, Conversion::FloatAEntero, Conversion::FloatASinSigno] {
        let p = ejemplos::programa(vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Convierte { d: 2, a: 0, como }, Op::Salida { s: 2, elemento: 0, componente: 0 }], 3, &[]);
        programas.push((std::format!("{como:?}"), p));
    }
    for (nombre, p) in &programas {
        let e = emitir(p, TECHO).unwrap();
        let r = emitir_con(p, TECHO, Abi::Registros).unwrap();
        juzgado(&e, &r);
        for &x in &bits {
            for &y in &bits {
                let ent = [[f32::from_bits(x), f32::from_bits(y), 0.0, 0.0]];
                let mut casa = [[0.0f32; 4]; 1];
                p.correr(&ent, &[], &mut casa, &mut Vec::new());
                let banco: Vec<u8> = ent.iter().flat_map(|e| e.iter().flat_map(|v| v.to_le_bytes())).collect();
                let mut m = crate::simula::Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
                crate::simula::correr(&e.codigo, &mut m).unwrap();
                assert_eq!(m.r[0], casa[0][0].to_bits(), "{nombre} {x:#x} {y:#x}");
            }
        }
    }
}


// -- E6d: la division ---------------------------------------------------------

/// 32 bits al azar (`n` da 31 como mucho).
fn palabra(az: &mut Azar) -> u32 {
    az.n(1 << 16) << 16 | az.n(1 << 16)
}

/// La division del emisor, contra la casa, con muchos pares: los bordes
/// (0, 1, potencias de 2 y sus vecinos, i32::MIN, -1...) y al azar.
#[test]
fn la_division_da_lo_de_la_casa() {
    let mut az = Azar(0x00D1_7151_0000_0001);
    let mut bits: Vec<u32> = Vec::new();
    for k in 0..32 {
        let p = 1u32 << k;
        bits.extend([p, p.wrapping_sub(1), p.wrapping_add(1), p.wrapping_neg(), p.wrapping_neg().wrapping_sub(1)]);
    }
    bits.extend([0, 3, 7, 10, 641, 6700417, 0x5555_5555, 0xAAAA_AAAB, 0xFFFF_FFFF, 0x7FFF_FFFF, 0x8000_0001]);
    for _ in 0..40 {
        bits.push(palabra(&mut az));
    }
    for op in [OpEntera::DivU, OpEntera::RemU, OpEntera::DivS, OpEntera::RemS] {
        let p = ejemplos::programa(
            vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }, Op::Entera { d: 2, a: 0, b: 1, op }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
            3,
            &[],
        );
        let e = emitir(&p, TECHO).unwrap();
        let r = emitir_con(&p, TECHO, Abi::Registros).unwrap();
        juzgado(&e, &r);
        for &x in &bits {
            for &y in &bits {
                let banco: Vec<u8> = [x, y, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
                let mut m = crate::simula::Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
                crate::simula::correr(&e.codigo, &mut m).unwrap();
                assert_eq!(m.r[0], op.hacer(x, y), "{op:?} {x:#x} {y:#x}");
            }
        }
    }
}

/// El simulador hace MUFU.RCP exacto; la 3060 se equivoca en un ULP. La
/// cuenta de `dividir` (la misma, paso a paso) con el inverso movido un ULP
/// arriba o hasta 64 abajo: el cociente sale igual. (Con 2 arriba ya no:
/// ese es el margen de las -2 ULP de `0x0ffffffe`.)
#[test]
fn la_division_aguanta_un_inverso_aproximado() {
    let dividir = |a: u32, b: u32, ulp: i32| -> u32 {
        let fb = {
            let y = b as f32;
            if (y as f64) < b as f64 { f32::from_bits(y.to_bits() + 1) } else { y }
        };
        let inv = f32::from_bits((1.0 / fb).to_bits().wrapping_add_signed(ulp));
        let e = f32::from_bits(inv.to_bits().wrapping_add(0x0fff_fffe)) as u32;
        let t = b.wrapping_neg().wrapping_mul(e);
        let e = e.wrapping_add(((e as u64 * t as u64) >> 32) as u32);
        let mut q = ((e as u64 * a as u64) >> 32) as u32;
        let mut r = a.wrapping_sub(q.wrapping_mul(b));
        for _ in 0..2 {
            if r >= b {
                r -= b;
                q += 1;
            }
        }
        q
    };
    let mut az = Azar(0x00D1_7151_0000_0002);
    for _ in 0..50_000 {
        let a = palabra(&mut az);
        let b = match az.n(3) {
            0 => az.n(256) + 1,
            1 => (palabra(&mut az) >> az.n(32)).max(1),
            _ => (palabra(&mut az)).max(1),
        };
        for ulp in [-64, -16, -4, -3, -2, -1, 0, 1] {
            assert_eq!(dividir(a, b, ulp), a / b, "{a} / {b}, {ulp} ulp");
        }
    }
}

/// Corre `p` emitido con los dos ABI (y juzgado) y compara los BITS de la
/// salida 0 con los de la casa (sin la tolerancia de NaN: 0xFFFFFFFF es un
/// NaN y aqui es un resultado). Devuelve lo emitido con el del banco.
fn bits_exactos(p: &Programa, pares: &[(u32, u32)]) -> Emitido {
    let e = emitir(p, TECHO).unwrap();
    let r = emitir_con(p, TECHO, Abi::Registros).unwrap();
    juzgado(&e, &r);
    for &(x, y) in pares {
        let ent = [[f32::from_bits(x), f32::from_bits(y), 0.0, 0.0]];
        let mut casa = [[0.0f32; 4]; 1];
        p.correr(&ent, &[], &mut casa, &mut Vec::new());
        let banco: Vec<u8> = [x, y, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut m = crate::simula::Maquina::nueva([&[], &banco, &[], &[], &[], &[], &[], &[]]);
        crate::simula::correr(&e.codigo, &mut m).unwrap();
        for k in 0..4 {
            assert_eq!(m.r[k], casa[0][k].to_bits(), "salida {k} con ({x:#x}, {y:#x}): {:?}", p.ops);
        }
    }
    e
}

/// Los `a` de las pruebas de division: los bordes y unos al azar.
fn dividendos() -> Vec<u32> {
    let mut az = Azar(0x00D1_7151_0000_0003);
    let mut v = std::vec![0u32, 1, 2, 3, 6, 7, 8, 9, 10, 99, 100, 101, 640, 641, 642, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 0xFFFF_FFF9, 0xFFFF_FFF6, 0xCCCC_CCCC, 0x2492_4924];
    for _ in 0..60 {
        v.push(palabra(&mut az));
    }
    v
}

/// E6d: entre una CONSTANTE (0, 1, -1, potencias de 2, i32::MIN, las de
/// magia de 32 y de 33 bits, negativas): los bits de la casa, sin I2F ni
/// MUFU (ni inverso ni comprobar el 0).
#[test]
fn la_division_por_una_constante() {
    let constantes = [0u32, 1, 2, 3, 4, 5, 6, 7, 10, 25, 64, 100, 641, 1000, 6700417, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFF, 0xFFFF_FFFE, 0xFFFF_FFF9, 0xFFFF_FFF6, 0xFFFF_FC18, 0xCCCC_CCCD];
    let pares: Vec<(u32, u32)> = dividendos().into_iter().map(|x| (x, 0)).collect();
    for op in [OpEntera::DivU, OpEntera::RemU, OpEntera::DivS, OpEntera::RemS] {
        for n in constantes {
            let p = ejemplos::programa(
                vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entera { d: 2, a: 0, b: 1, op }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
                3,
                &[(1, f32::from_bits(n))],
            );
            let e = bits_exactos(&p, &pares);
            assert_eq!((cuantas(&e.codigo, 0x106), e.mufus), (0, 0), "{op:?} entre {n:#x}");
        }
    }
}

/// E6d: `a / b` y `a % b` del mismo par son UNA cuenta (un I2F), en los dos
/// ordenes, con algo por medio, con una constante, y con `x = x % b` (el
/// destino de la pareja es lo que se divide); y NO se funden si algo
/// escribe `a` entre medias o hay un `si` por medio.
#[test]
fn la_pareja_es_una_cuenta() {
    use Op::*;
    let pares: Vec<(u32, u32)> = dividendos().iter().flat_map(|&x| [(x, 7), (x, 0), (x, 0xFFFF_FFF9), (x, x.rotate_left(7) >> 3), (x, 1), (x, 0x8000_0000)]).collect();
    let ent = || std::vec![Entrada { d: 0, elemento: 0, componente: 0 }, Entrada { d: 1, elemento: 0, componente: 1 }];
    let sal = |a: Reg, b: Reg| [Salida { s: a, elemento: 0, componente: 0 }, Salida { s: b, elemento: 0, componente: 1 }];
    for (div, rem) in [(OpEntera::DivU, OpEntera::RemU), (OpEntera::DivS, OpEntera::RemS)] {
        // Cociente y resto; resto y cociente con una suma en medio.
        let mut a = ent();
        a.extend([Entera { d: 2, a: 0, b: 1, op: div }, Entera { d: 3, a: 0, b: 1, op: rem }]);
        a.extend(sal(2, 3));
        let mut b = ent();
        b.extend([Entera { d: 3, a: 0, b: 1, op: rem }, SumaEntera { d: 4, a: 0, b: 1 }, Entera { d: 2, a: 0, b: 1, op: div }, Salida { s: 4, elemento: 0, componente: 2 }]);
        b.extend(sal(2, 3));
        // Entre la constante -7 (el registro 5): sin inverso.
        let mut k = ent();
        k.extend([Entera { d: 2, a: 0, b: 5, op: div }, Entera { d: 3, a: 0, b: 5, op: rem }]);
        k.extend(sal(2, 3));
        // `q = x / b; x = x % b`: x es una variable (se escribe dos veces).
        let mut x = ent();
        x.extend([Copia { d: 4, a: 0 }, Entera { d: 2, a: 4, b: 1, op: div }, Entera { d: 4, a: 4, b: 1, op: rem }]);
        x.extend(sal(2, 4));
        let mut xk = ent();
        xk.extend([Copia { d: 4, a: 0 }, Entera { d: 2, a: 4, b: 5, op: div }, Entera { d: 4, a: 4, b: 5, op: rem }]);
        xk.extend(sal(2, 4));
        for (nombre, ops, inversos) in [("cociente y resto", a, 1), ("resto y cociente", b, 1), ("entre -7", k, 0), ("x = x % b", x, 1), ("x = x % -7", xk, 0)] {
            let p = ejemplos::programa(ops, 6, &[(5, f32::from_bits(0xFFFF_FFF9))]);
            let e = bits_exactos(&p, &pares);
            assert_eq!(cuantas(&e.codigo, 0x106), inversos, "{nombre} {div:?}");
        }
        // Algo escribe `a` entre medias: dos cuentas.
        let mut w = ent();
        w.extend([Copia { d: 4, a: 0 }, Entera { d: 2, a: 4, b: 1, op: div }, SumaEntera { d: 4, a: 4, b: 1 }, Entera { d: 3, a: 4, b: 1, op: rem }]);
        w.extend(sal(2, 3));
        // Un `si` por medio: dos cuentas.
        let mut s = ent();
        s.extend([Entera { d: 2, a: 0, b: 1, op: div }, Compara { d: 4, a: 0, b: 1, como: Comparacion::Menor, entero: true }, Si { c: 4 }, FinSi, Entera { d: 3, a: 0, b: 1, op: rem }]);
        s.extend(sal(2, 3));
        for (nombre, ops) in [("a escrita en medio", w), ("un si en medio", s)] {
            let p = ejemplos::programa(ops, 6, &[]);
            let e = bits_exactos(&p, &pares);
            assert_eq!(cuantas(&e.codigo, 0x106), 2, "{nombre} {div:?}");
        }
    }
}

/// E6d: `division.hlsl` entero (siete divisiones: tres parejas y una entre
/// la constante 4), pegado con el pegamento de pixel del driver, CABE en la
/// puerta de 128 y el juez de programas lo da por bueno.
#[test]
fn el_dxil_de_division_cabe_en_la_puerta() {
    use bmo_gpu_ga10x::pegamento::{self, Datos};
    use bmo_gpu_ga10x::sass::juez;
    use bmo_gpu_ga10x::tuberia;
    let p = bmo_proton_x::dxil::programa::compilar(&bmo_proton_x::dxil::leer(DXIL_DIVISION).unwrap()).unwrap();
    let r = emitir_con(&p, 64, Abi::Registros).unwrap();
    // Siete divisiones: tres parejas fundidas (la del bucle tambien) y `a /
    // 4` sin inverso: tres I2F.RP (el inverso de la division; los demas I2F
    // son conversiones), no siete.
    assert_eq!(r.codigo.iter().filter(|w| w.0 & 0x1FF == 0x106 && w.1 >> 14 & 3 == 2).count(), 3);
    let genericos = [None, Some(0u8)];
    let pegado = pegamento::pixel(&r.codigo, r.registros, &crate::pso::cargas(&r), Datos { filas: p.filas_cb as u32, paso: 0, elementos: &[] }, &genericos).unwrap();
    let mut b = vec![0u8; tuberia::HUECO];
    let n = pegado.bytes(&mut b);
    let v = juez::juzgar_programa(&b[..n], tuberia::REGISTROS);
    let v = v.unwrap_or_else(|x| panic!("{x}"));
    assert!(v.instrucciones <= juez::MAX_INSTRUCCIONES);
    std::eprintln!("division.hlsl: cuerpo {} y pegado {} instrucciones", r.codigo.len(), v.instrucciones);
}
