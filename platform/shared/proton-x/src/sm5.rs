//! **El bytecode de Shader Model 5, EJECUTABLE en la CPU** (P3c3, 28-09).
//!
//! Lo que compila FXC (`D3DCompile` con `vs_5_0` / `ps_5_0`, el de BMOX-12)
//! no es DXIL: es la parte `SHEX` (o `SHDR`, SM4) del mismo sobre DXBC, un
//! programa de una maquina virtual de registros de 4 componentes:
//!
//! ```text
//!    version     etapa << 16 | mayor << 4 | menor   (la de DXIL, igual)
//!    medida      en palabras, con estas dos
//!    una instruccion   su codigo (bits 0..10), saturar (bit 13), su medida
//!                      en palabras (bits 24..30), y sus operandos
//!    un operando       componentes (0..1), modo (2..3: mascara, swizzle o
//!                      uno), la mascara o el swizzle (4..11), el tipo
//!                      (12..19: r# v# o# l() cb#[]), las dimensiones del
//!                      indice (20..21), y el bit 31: un token mas con el
//!                      modificador (-x, |x|, -|x|)
//! ```
//!
//! No hay otro interprete: esto lo traduce al MISMO [`Programa`] que sale del
//! DXIL (`dxil::programa`), escalar, y el lote, la trama y el traductor a
//! x86-64 (`nativo`) no cambian. Cada componente escrito es un registro
//! nuevo: un `mov` no cuesta nada (el destino pasa a ser el mismo registro
//! que la fuente), y un swizzle tampoco.
//!
//! **Con saltos (E6, 02-10)**: `if_nz`/`if_z`, `else`, `endif`, `loop`,
//! `endloop`, `break`, `breakc_nz`/`breakc_z`, `continue`,
//! `continuec_nz`/`continuec_z`; y lo que los alimenta: `lt`
//! `ge` `eq` `ne` (floats; `ne` desordenada), `ilt` `ige` `ieq` `ine`,
//! `iadd` y `movc`. Un programa que salta no puede renombrar: tras un `if`
//! los dos caminos tienen que dejar el valor en el MISMO sitio. Asi que en
//! uno que salta cada componente de `r#` y de `o#` es una VARIABLE (un
//! registro fijo, que se escribe con `Copia`), y lo que se lee de `v#` y de
//! `cb0` va al PRINCIPIO del programa (si no, un camino que no paso por la
//! lectura no lo tendria). `switch`, `retc` y los enteros que no son esos se
//! dicen por su numero.
//!
//! Lo que se sabe hoy es lo que pide el cubo de BMOX-12 y poco mas: `add`
//! `mul` `mad` `div` `dp2` `dp3` `dp4` `rsq` `sqrt` `min` `max` `mov` y
//! `ret`, con `_sat` y los modificadores, sobre `r#`, `v#`, `o#`, `l()` y
//! `cb0[n]`; `sample` 2D; y los saltos de arriba. Lo demas se dice al leer,
//! con su numero: un segundo cbuffer, un indice relativo, una textura 3D.
//!
//! Los numeros, como en el DXIL: `mad` SIN fundir y `dp` de izquierda a
//! derecha (D3D deja las dos cosas al driver; asi las hace el juez). Lo que
//! no es D3D11, dicho: D3D11 pide llevar a cero los subnormales en la
//! aritmetica de 32 bits; aqui no (tampoco en el DXIL de la casa).
//!
//! Las formas son las publicas del SDK de Windows
//! (`d3d11TokenizedProgramFormat.hpp`); el banco las comprueba con lo que FXC
//! escribio para el cubo (`prueba/sombras/*.cso`) contra las huellas de la
//! 3060.

use alloc::vec::Vec;

use crate::dxil::programa::{Comparacion, Conversion, NoPrograma, Op, OpEntera, Programa, Reg};
use crate::dxil::Elemento;

// Los codigos que se saben (D3D10_SB_OPCODE_TYPE).
const ADD: u32 = 0;
const BREAK: u32 = 2;
const BREAKC: u32 = 3;
const CONTINUE: u32 = 7;
const CONTINUEC: u32 = 8;
// E6c (02-10): los enteros, las conversiones y el `switch`.
const AND: u32 = 1;
const CASE: u32 = 6;
const DEFAULT: u32 = 10;
const ENDSWITCH: u32 = 23;
const FTOI: u32 = 27;
const FTOU: u32 = 28;
const IMAD: u32 = 35;
const IMAX: u32 = 36;
const IMIN: u32 = 37;
const IMUL: u32 = 38;
const INEG: u32 = 40;
const ISHL: u32 = 41;
const ISHR: u32 = 42;
const ITOF: u32 = 43;
const NOT: u32 = 59;
const OR: u32 = 60;
const SWITCH: u32 = 76;
const UDIV: u32 = 78;
const ULT: u32 = 79;
const UGE: u32 = 80;
const UMUL: u32 = 81;
const UMAD: u32 = 82;
const UMAX: u32 = 83;
const UMIN: u32 = 84;
const USHR: u32 = 85;
const UTOF: u32 = 86;
const XOR: u32 = 87;
/// D3D10_SB_OPERAND_TYPE_NULL: el destino alto de `imul`/`umul` que no se usa.
const NULO: u32 = 13;
/// Las instrucciones de enteros o de bits: sin modificadores de float.
const fn de_enteros(c: u32) -> bool {
    matches!(c, ILT | IGE | IEQ | INE | IADD | MOVC | AND | OR | XOR | NOT | INEG | IMUL | UMUL | IMAD | UMAD | ISHL | ISHR | USHR | IMIN | IMAX | UMIN | UMAX | ITOF | UTOF | ULT | UGE | UDIV)
}
const ELSE: u32 = 18;
const ENDIF: u32 = 21;
const ENDLOOP: u32 = 22;
const EQ: u32 = 24;
const GE: u32 = 29;
const IADD: u32 = 30;
const IF: u32 = 31;
const IEQ: u32 = 32;
const IGE: u32 = 33;
const ILT: u32 = 34;
const INE: u32 = 39;
const LOOP: u32 = 48;
const LT: u32 = 49;
const MOVC: u32 = 55;
const NE: u32 = 57;
/// D3D10_SB_INSTRUCTION_TEST_BOOLEAN (bit 18) de `if` y `breakc`: 1 = `_nz`.
const PRUEBA_NO_CERO: u32 = 1 << 18;
const DIV: u32 = 14;
const DP2: u32 = 15;
const DP3: u32 = 16;
const DP4: u32 = 17;
const MAD: u32 = 50;
const MIN: u32 = 51;
const MAX: u32 = 52;
const CUSTOMDATA: u32 = 53;
const MOV: u32 = 54;
const MUL: u32 = 56;
const RET: u32 = 62;
const RSQ: u32 = 68;
/// `sample dest, direccion, tN, sM` (D3D10_SB_OPCODE_SAMPLE).
const SAMPLE: u32 = 69;
const SQRT: u32 = 75;
// Las declaraciones que se miran (las demas se saltan por su medida).
const DCL_CONSTANT_BUFFER: u32 = 89;
const DCL_RESOURCE: u32 = 88;
const DCL_SAMPLER: u32 = 90;
const DCL_INDEXABLE_TEMP: u32 = 105;
/// De `dcl_resource` (88) a `dcl_global_flags` (106): las de SM4, que son las
/// que escribe FXC para un vertice o un pixel. Las de SM5 (teselado, computo,
/// UAV: desde la 143) se dicen por su numero.
const fn es_declaracion(c: u32) -> bool {
    c >= 88 && c <= 106
}

// Los tipos de operando.
const TEMP: u32 = 0;
const INPUT: u32 = 1;
const OUTPUT: u32 = 2;
const IMMEDIATE32: u32 = 4;
const CONSTANT_BUFFER: u32 = 8;
const SAMPLER: u32 = 6;
const RESOURCE: u32 = 7;
/// La dimension de `dcl_resource` (bits 11..15): 3 es TEXTURE2D.
const TEXTURA_2D: u32 = 3;

// Los modificadores de una fuente.
const NEG: u32 = 1;
const ABS: u32 = 2;
const ABSNEG: u32 = 3;

/// Un operando, leido: su tipo, sus indices, a que componente va cada una de
/// las 4 (`sel`), cuales escribe (`mascara`, si es destino) y su modificador.
struct Operando {
    tipo: u32,
    indices: [u32; 2],
    sel: [u8; 4],
    mascara: u8,
    modificador: u32,
    inmediato: [u32; 4],
}

fn palabra(t: &[u32], i: usize) -> Result<u32, NoPrograma> {
    t.get(i).copied().ok_or(NoPrograma::Forma("un programa SM5 cortado"))
}

/// Lee un operando desde `t[*i]` y avanza `*i`.
fn operando(t: &[u32], i: &mut usize) -> Result<Operando, NoPrograma> {
    let w = palabra(t, *i)?;
    *i += 1;
    let n = w & 3;
    let (mut sel, mut mascara) = ([0u8, 1, 2, 3], 0xFu8);
    if n == 2 {
        match (w >> 2) & 3 {
            0 => mascara = ((w >> 4) & 0xF) as u8,
            1 => sel = core::array::from_fn(|k| ((w >> (4 + 2 * k)) & 3) as u8),
            2 => sel = [((w >> 4) & 3) as u8; 4],
            _ => return Err(NoPrograma::Forma("un operando SM5 con un modo que no existe")),
        }
    } else if n == 1 {
        sel = [0; 4];
        mascara = 1;
    }
    let tipo = (w >> 12) & 0xFF;
    let dims = (w >> 20) & 3;
    let mut ext = w >> 31 != 0;
    let mut modificador = 0;
    while ext {
        let e = palabra(t, *i)?;
        *i += 1;
        if e & 0x3F == 1 {
            modificador = (e >> 6) & 0xFF;
        }
        ext = e >> 31 != 0;
    }
    let mut inmediato = [0u32; 4];
    if tipo == IMMEDIATE32 {
        let k = if n == 2 { 4 } else { 1 };
        for (x, j) in inmediato.iter_mut().zip(0..k) {
            *x = palabra(t, *i + j)?;
        }
        if k == 1 {
            inmediato = [inmediato[0]; 4];
        }
        *i += k;
    }
    let mut indices = [0u32; 2];
    for d in 0..dims as usize {
        match (w >> (22 + 3 * d)) & 7 {
            0 => {
                let x = palabra(t, *i)?;
                if let Some(v) = indices.get_mut(d) {
                    *v = x;
                }
                *i += 1;
            }
            _ => return Err(NoPrograma::Forma("un indice SM5 relativo o de 64 bits: todavia no")),
        }
    }
    Ok(Operando { tipo, indices, sel, mascara, modificador, inmediato })
}

/// El elemento de una firma que lleva el componente `c` del registro `r`, y
/// su componente DENTRO del elemento (como en el DXIL).
fn elemento(firma: &[Elemento], r: u32, c: u8) -> Option<(u8, u8)> {
    let i = firma.iter().position(|e| e.registro == r && e.mascara & (1 << c) != 0)?;
    let primero = firma[i].mascara.trailing_zeros() as u8;
    Some((i as u8, c - primero))
}

struct Traductor<'a> {
    entradas: &'a [Elemento],
    salidas: &'a [Elemento],
    p: Programa,
    /// Lo que vale ahora cada componente de cada `r#` y de cada `o#`.
    temps: Vec<[Option<Reg>; 4]>,
    outs: Vec<[Option<Reg>; 4]>,
    /// Las ya leidas: `v#` por (registro, componente), `cb0[n]` y `l()`.
    leidas: Vec<(u32, u8, Reg)>,
    filas: Vec<(u32, Reg)>,
    literales: Vec<(u32, Reg)>,
    /// E6: el programa salta -- `r#` y `o#` son variables (ver arriba).
    variables: bool,
    /// E6: cuantos `if`/`loop` hay abiertos.
    hondo: usize,
    /// E6c: lo abierto que es bucle (`false`) o `switch` (`true`): un
    /// `continue` dentro de un `switch` todavia no.
    construcciones: Vec<bool>,
    /// E6c: los `switch` abiertos.
    switches: Vec<Interruptor>,
}

/// Un `switch` abierto: se hace un BUCLE de una vuelta (su `break` es el
/// Romper de ese bucle) y cada tramo de `case` un `Si` de `m`, que dice si
/// ya se entro (asi el que cae al siguiente caso, cae): `m |= (x == v)` en
/// cada `case`, `m |= defecto` en el `default`, y `defecto` es que `x` no es
/// ninguno de los casos (se miran todos al abrir).
struct Interruptor {
    x: Reg,
    m: Reg,
    defecto: Reg,
    /// Hay un `Si` de tramo abierto.
    abierto: bool,
    /// Tras un `case`/`default`: la siguiente instruccion abre el tramo.
    pendiente: bool,
}

impl Traductor<'_> {
    fn nuevo(&mut self) -> Result<Reg, NoPrograma> {
        let r = Reg::try_from(self.p.iniciales.len()).map_err(|_| NoPrograma::Forma("un programa SM5 con demasiados registros"))?;
        self.p.iniciales.push(0.0);
        Ok(r)
    }

    /// Una lectura de `v#` o `cb0`: dentro de un `if` o un `loop`, AL
    /// PRINCIPIO del programa (lo que lee no cambia; asi todo camino la ve).
    fn fija(&mut self, op: Op) {
        if self.hondo > 0 {
            self.p.ops.insert(0, op);
        } else {
            self.p.ops.push(op);
        }
    }

    fn literal(&mut self, bits: u32) -> Result<Reg, NoPrograma> {
        if let Some(&(_, r)) = self.literales.iter().find(|x| x.0 == bits) {
            return Ok(r);
        }
        let r = self.nuevo()?;
        self.p.iniciales[r as usize] = f32::from_bits(bits);
        self.literales.push((bits, r));
        Ok(r)
    }

    /// Un componente de una fuente, sin el modificador.
    fn componente(&mut self, o: &Operando, c: u8) -> Result<Reg, NoPrograma> {
        let r = o.indices[0];
        match o.tipo {
            TEMP => self.temps.get(r as usize).and_then(|t| t[c as usize]).ok_or(NoPrograma::Forma("un r# que se lee antes de escribirse")),
            INPUT => {
                if let Some(&(_, _, x)) = self.leidas.iter().find(|x| x.0 == r && x.1 == c) {
                    return Ok(x);
                }
                let (e, k) = elemento(self.entradas, r, c).ok_or(NoPrograma::Forma("un v# que la firma de entrada no tiene"))?;
                let d = self.nuevo()?;
                self.fija(Op::Entrada { d, elemento: e, componente: k });
                self.p.lee |= 1 << e;
                self.leidas.push((r, c, d));
                Ok(d)
            }
            OUTPUT => self.outs.get(r as usize).and_then(|t| t[c as usize]).ok_or(NoPrograma::Forma("un o# que se lee antes de escribirse")),
            IMMEDIATE32 => self.literal(o.inmediato[c as usize]),
            CONSTANT_BUFFER => {
                if o.indices[0] != 0 {
                    return Err(NoPrograma::Forma("un cbuffer que no es b0: todavia no"));
                }
                let fila = o.indices[1];
                let base = match self.filas.iter().find(|x| x.0 == fila) {
                    Some(&(_, b)) => b,
                    None => {
                        let b = self.nuevo()?;
                        for _ in 1..4 {
                            self.nuevo()?;
                        }
                        let f = u16::try_from(fila).map_err(|_| NoPrograma::Forma("una fila de cbuffer imposible"))?;
                        self.fija(Op::Constantes { d: b, fila: f });
                        self.p.filas_cb = self.p.filas_cb.max(f + 1);
                        self.filas.push((fila, b));
                        b
                    }
                };
                Ok(base + c as Reg)
            }
            _ => Err(NoPrograma::Forma("un operando SM5 de un tipo que no se lee todavia (textura, sampler, indexable...)")),
        }
    }

    /// La componente `k` de una fuente: su swizzle y su modificador.
    fn fuente(&mut self, o: &Operando, k: usize) -> Result<Reg, NoPrograma> {
        let x = self.componente(o, o.sel[k])?;
        let x = if o.modificador == ABS || o.modificador == ABSNEG {
            let d = self.nuevo()?;
            self.p.ops.push(Op::Abs { d, a: x });
            d
        } else {
            x
        };
        if o.modificador == NEG || o.modificador == ABSNEG {
            // -x = x * -1: exacto, y -0 sale -0.
            let menos = self.literal((-1.0f32).to_bits())?;
            let d = self.nuevo()?;
            self.p.ops.push(Op::Mul { d, a: x, b: menos });
            return Ok(d);
        }
        Ok(x)
    }

    /// **Una operacion** (no `mov` ni `dp`) sobre las fuentes `s`: su
    /// resultado, en un registro nuevo.
    fn operar(&mut self, codigo: u32, s: &[Reg]) -> Result<Reg, NoPrograma> {
        let x = self.nuevo()?;
        let entera = |op| Op::Entera { d: x, a: s[0], b: s.get(1).copied().unwrap_or(s[0]), op };
        let compara = |como, entero| Op::Compara { d: x, a: s[0], b: s[1], como, entero };
        let convierte = |como| Op::Convierte { d: x, a: s[0], como };
        let op = match codigo {
            ADD => Op::Add { d: x, a: s[0], b: s[1] },
            MUL => Op::Mul { d: x, a: s[0], b: s[1] },
            DIV => Op::Div { d: x, a: s[0], b: s[1] },
            MIN => Op::Min { d: x, a: s[0], b: s[1] },
            MAX => Op::Max { d: x, a: s[0], b: s[1] },
            MAD => Op::Mad { d: x, a: s[0], b: s[1], c: s[2] },
            RSQ => Op::Rsqrt { d: x, a: s[0] },
            SQRT => Op::Sqrt { d: x, a: s[0] },
            IADD => Op::SumaEntera { d: x, a: s[0], b: s[1] },
            MOVC => Op::Elige { d: x, c: s[0], a: s[1], b: s[2] },
            LT => compara(Comparacion::Menor, false),
            GE => compara(Comparacion::MayorIgual, false),
            EQ => compara(Comparacion::Igual, false),
            NE => compara(Comparacion::Distinto, false),
            ILT => compara(Comparacion::Menor, true),
            IGE => compara(Comparacion::MayorIgual, true),
            IEQ => compara(Comparacion::Igual, true),
            INE => compara(Comparacion::Distinto, true),
            ULT => compara(Comparacion::MenorSinSigno, true),
            UGE => compara(Comparacion::MayorIgualSinSigno, true),
            AND => entera(OpEntera::Y),
            OR => entera(OpEntera::O),
            XOR => entera(OpEntera::OX),
            IMUL | UMUL => entera(OpEntera::Mul),
            ISHL => entera(OpEntera::Shl),
            ISHR => entera(OpEntera::ShrA),
            USHR => entera(OpEntera::ShrL),
            IMIN => entera(OpEntera::MinS),
            IMAX => entera(OpEntera::MaxS),
            UMIN => entera(OpEntera::MinU),
            UMAX => entera(OpEntera::MaxU),
            ITOF => convierte(Conversion::EnteroAFloat),
            UTOF => convierte(Conversion::SinSignoAFloat),
            FTOI => convierte(Conversion::FloatAEntero),
            FTOU => convierte(Conversion::FloatASinSigno),
            // not x = x ^ 0xFFFFFFFF; ineg x = 0 - x.
            NOT => {
                let todos = self.literal(u32::MAX)?;
                Op::Entera { d: x, a: s[0], b: todos, op: OpEntera::OX }
            }
            INEG => {
                let cero = self.literal(0)?;
                Op::Entera { d: x, a: cero, b: s[0], op: OpEntera::Resta }
            }
            // imad/umad: a * b + c (los 32 de abajo: igual con o sin signo).
            _ => {
                let t = self.nuevo()?;
                self.p.ops.push(Op::Entera { d: t, a: s[0], b: s[1], op: OpEntera::Mul });
                Op::SumaEntera { d: x, a: t, b: s[2] }
            }
        };
        self.p.ops.push(op);
        Ok(x)
    }

    /// E6c: tras un `case`/`default`, la primera instruccion abre su tramo.
    fn abrir_tramo(&mut self) {
        if let Some(s) = self.switches.last_mut().filter(|s| s.pendiente) {
            s.pendiente = false;
            s.abierto = true;
            let m = s.m;
            self.p.ops.push(Op::Si { c: m });
            self.hondo += 1;
        }
    }

    fn cerrar_tramo(&mut self) -> Result<(), NoPrograma> {
        let s = self.switches.last_mut().ok_or(NoPrograma::Forma("un case/default/endswitch SM5 fuera de un switch"))?;
        if s.abierto {
            s.abierto = false;
            self.p.ops.push(Op::FinSi);
            self.hondo -= 1;
        }
        Ok(())
    }

    /// `switch x` con estos casos: `m = 0`, `defecto = x no es ninguno`, y el
    /// bucle de una vuelta.
    fn abrir_switch(&mut self, x: Reg, casos: &[u32]) -> Result<(), NoPrograma> {
        let cero = self.literal(0)?;
        let m = self.nuevo()?;
        self.p.ops.push(Op::Copia { d: m, a: cero });
        let mut defecto = self.literal(u32::MAX)?;
        for &v in casos {
            let lit = self.literal(v)?;
            let distinto = self.nuevo()?;
            self.p.ops.push(Op::Compara { d: distinto, a: x, b: lit, como: Comparacion::Distinto, entero: true });
            let y = self.nuevo()?;
            self.p.ops.push(Op::Entera { d: y, a: defecto, b: distinto, op: OpEntera::Y });
            defecto = y;
        }
        self.p.ops.push(Op::Bucle);
        self.hondo += 1;
        self.construcciones.push(true);
        self.switches.push(Interruptor { x, m, defecto, abierto: false, pendiente: false });
        Ok(())
    }

    /// `case v` (o `default` con `None`): `m |= (x == v)` (o `m |= defecto`).
    fn caso(&mut self, v: Option<u32>) -> Result<(), NoPrograma> {
        self.cerrar_tramo()?;
        let (x, m, defecto) = match self.switches.last() {
            Some(s) => (s.x, s.m, s.defecto),
            None => return Err(NoPrograma::Forma("un case/default SM5 fuera de un switch")),
        };
        let entra = match v {
            Some(v) => {
                let lit = self.literal(v)?;
                let e = self.nuevo()?;
                self.p.ops.push(Op::Compara { d: e, a: x, b: lit, como: Comparacion::Igual, entero: true });
                e
            }
            None => defecto,
        };
        let o = self.nuevo()?;
        self.p.ops.push(Op::Entera { d: o, a: m, b: entra, op: OpEntera::O });
        self.p.ops.push(Op::Copia { d: m, a: o });
        if let Some(s) = self.switches.last_mut() {
            s.pendiente = true;
        }
        Ok(())
    }

    /// `endswitch`: el tramo, y salir del bucle de una vuelta.
    fn cerrar_switch(&mut self) -> Result<(), NoPrograma> {
        self.cerrar_tramo()?;
        self.switches.pop();
        if self.construcciones.pop() != Some(true) {
            return Err(NoPrograma::Forma("un endswitch SM5 sin su switch"));
        }
        self.p.ops.push(Op::Romper);
        self.p.ops.push(Op::FinBucle);
        self.hondo -= 1;
        Ok(())
    }

    /// Escribe el componente `k` del destino (con `_sat` si toca).
    fn escribir(&mut self, o: &Operando, k: usize, x: Reg, saturar: bool) -> Result<(), NoPrograma> {
        let x = if saturar {
            let d = self.nuevo()?;
            self.p.ops.push(Op::Saturate { d, a: x });
            d
        } else {
            x
        };
        let r = o.indices[0] as usize;
        let banco = match o.tipo {
            TEMP => &mut self.temps,
            OUTPUT => &mut self.outs,
            _ => return Err(NoPrograma::Forma("un destino SM5 que no es r# ni o#")),
        };
        if banco.len() <= r {
            if r > 4096 {
                return Err(NoPrograma::Forma("un registro SM5 imposible"));
            }
            banco.resize(r + 1, [None; 4]);
        }
        if !self.variables {
            banco[r][k] = Some(x);
            return Ok(());
        }
        // E6: la variable de ese componente (la primera vez, una nueva), y
        // el valor copiado en ella.
        let v = match banco[r][k] {
            Some(v) => v,
            None => {
                let v = self.nuevo()?;
                let banco = if o.tipo == TEMP { &mut self.temps } else { &mut self.outs };
                banco[r][k] = Some(v);
                v
            }
        };
        self.p.ops.push(Op::Copia { d: v, a: x });
        Ok(())
    }
}

/// **Traducir el `SHEX`/`SHDR`** (`t`: sus palabras, con la version y la
/// medida) al programa escalar de la casa, con las firmas del sobre.
pub fn compilar(t: &[u32], entradas: &[Elemento], salidas: &[Elemento]) -> Result<Programa, NoPrograma> {
    let medida = (palabra(t, 1)? as usize).min(t.len());
    let p = Programa { ops: Vec::new(), iniciales: Vec::new(), entradas: entradas.len(), salidas: salidas.len(), lee: 0, filas_cb: 0 };
    // E6: si salta, antes de leer nada (lo de antes del primer `if` tambien
    // va a sus variables).
    let variables = salta(t, medida);
    let mut tr = Traductor { entradas, salidas, p, temps: Vec::new(), outs: Vec::new(), leidas: Vec::new(), filas: Vec::new(), literales: Vec::new(), variables, hondo: 0, construcciones: Vec::new(), switches: Vec::new() };
    let mut i = 2;
    let mut acabado = false;
    while i < medida {
        let w = t[i];
        let codigo = w & 0x7FF;
        let largo = if codigo == CUSTOMDATA { palabra(t, i + 1)? as usize } else { ((w >> 24) & 0x7F) as usize };
        if largo == 0 {
            return Err(NoPrograma::Forma("una instruccion SM5 de medida 0"));
        }
        let fin = i + largo;
        if fin > medida {
            return Err(NoPrograma::Forma("una instruccion SM5 pasa del final"));
        }
        if acabado {
            // Tras el `ret` del principal solo hay subrutinas o nada.
            return Err(NoPrograma::Forma("un programa SM5 con algo detras del ret (subrutinas): todavia no"));
        }
        // E6c: la primera instruccion tras un `case`/`default` abre su tramo.
        if !matches!(codigo, CASE | DEFAULT | ENDSWITCH) && !es_declaracion(codigo) {
            tr.abrir_tramo();
        }
        match codigo {
            CUSTOMDATA => return Err(NoPrograma::Forma("un programa SM5 con datos propios (icb): todavia no")),
            DCL_CONSTANT_BUFFER => {
                let mut j = i + 1;
                let o = operando(t, &mut j)?;
                if o.indices[0] != 0 {
                    return Err(NoPrograma::Forma("un cbuffer que no es b0: todavia no"));
                }
            }
            // ** Texturas (29-09): 2D y ya. El muestreador se declara con su
            // modo (default o comparacion): la comparacion, todavia no.
            DCL_RESOURCE if (w >> 11) & 0x1F != TEXTURA_2D => return Err(NoPrograma::Forma("una textura SM5 que no es 2D (1D, 3D, cubo, array, buffer...): todavia no")),
            DCL_RESOURCE => {}
            DCL_SAMPLER if (w >> 11) & 0xF != 0 => return Err(NoPrograma::Forma("un muestreador SM5 de comparacion: todavia no")),
            DCL_SAMPLER => {}
            DCL_INDEXABLE_TEMP => return Err(NoPrograma::Forma("un sombreador SM5 con x# (registros indexables): todavia no")),
            c if es_declaracion(c) => {}
            RET if tr.hondo > 0 => return Err(NoPrograma::Forma("un ret dentro de un if o un loop: todavia no")),
            RET => acabado = true,
            // ** E6: los saltos.
            CONTINUE | CONTINUEC if tr.construcciones.last() == Some(&true) => return Err(NoPrograma::Forma("un continue SM5 dentro de un switch: todavia no")),
            IF | BREAKC | CONTINUEC => {
                let mut j = i + 1;
                let c = operando(t, &mut j)?;
                if j != fin {
                    return Err(NoPrograma::Forma("un if/breakc SM5 que no mide lo que dice"));
                }
                let x = tr.fuente(&c, 0)?;
                let no_cero = w & PRUEBA_NO_CERO != 0;
                if codigo == BREAKC {
                    tr.p.ops.push(Op::RomperSi { c: x, si_cero: !no_cero });
                } else if codigo == CONTINUEC {
                    // `continuec_nz x`: si x { continue }; `_z`: si x {} sino { continue }.
                    tr.p.ops.push(Op::Si { c: x });
                    if !no_cero {
                        tr.p.ops.push(Op::SiNo);
                    }
                    tr.p.ops.push(Op::Continuar);
                    tr.p.ops.push(Op::FinSi);
                } else {
                    // `if_z x`: si x == 0 (los bits): el si de "x es cero".
                    let c = if no_cero {
                        x
                    } else {
                        let cero = tr.literal(0)?;
                        let d = tr.nuevo()?;
                        tr.p.ops.push(Op::Compara { d, a: x, b: cero, como: Comparacion::Igual, entero: true });
                        d
                    };
                    tr.p.ops.push(Op::Si { c });
                    tr.hondo += 1;
                }
            }
            ELSE => tr.p.ops.push(Op::SiNo),
            ENDIF | ENDLOOP => {
                tr.hondo = tr.hondo.checked_sub(1).ok_or(NoPrograma::Forma("un endif/endloop SM5 sin su if/loop"))?;
                if codigo == ENDLOOP && tr.construcciones.pop() != Some(false) {
                    return Err(NoPrograma::Forma("un endloop SM5 sin su loop"));
                }
                tr.p.ops.push(if codigo == ENDIF { Op::FinSi } else { Op::FinBucle });
            }
            LOOP => {
                tr.p.ops.push(Op::Bucle);
                tr.hondo += 1;
                tr.construcciones.push(false);
            }
            // ** E6c: el `switch`.
            SWITCH => {
                let mut j = i + 1;
                let o = operando(t, &mut j)?;
                let x = tr.fuente(&o, 0)?;
                tr.abrir_switch(x, &casos_del_switch(t, fin, medida)?)?;
            }
            CASE => {
                let mut j = i + 1;
                let o = operando(t, &mut j)?;
                tr.caso(Some(o.inmediato[0]))?;
            }
            DEFAULT => tr.caso(None)?,
            ENDSWITCH => tr.cerrar_switch()?,
            BREAK => tr.p.ops.push(Op::Romper),
            CONTINUE => tr.p.ops.push(Op::Continuar),
            // ** E6d: `udiv cociente, resto, a, b` -- cualquiera de los dos
            // destinos puede ser null. Se lee todo antes de escribir nada.
            UDIV => {
                if w & 0x2000 != 0 {
                    return Err(NoPrograma::Forma("un udiv SM5 con _sat"));
                }
                let mut j = i + 1;
                while t[j - 1] >> 31 != 0 && j < fin {
                    j += 1;
                }
                let dq = operando(t, &mut j)?;
                let dr = operando(t, &mut j)?;
                let (a, b) = (operando(t, &mut j)?, operando(t, &mut j)?);
                if j != fin {
                    return Err(NoPrograma::Forma("una instruccion SM5 que no mide lo que dice"));
                }
                if a.modificador != 0 || b.modificador != 0 {
                    return Err(NoPrograma::Forma("un modificador en una instruccion SM5 de enteros o de bits: todavia no"));
                }
                let mut hechos = Vec::with_capacity(8);
                for (d, op) in [(&dq, OpEntera::DivU), (&dr, OpEntera::RemU)] {
                    if d.tipo == NULO {
                        continue;
                    }
                    for k in (0..4).filter(|k| d.mascara & (1 << k) != 0) {
                        let (ra, rb) = (tr.fuente(&a, k)?, tr.fuente(&b, k)?);
                        let x = tr.nuevo()?;
                        tr.p.ops.push(Op::Entera { d: x, a: ra, b: rb, op });
                        hechos.push((d, k, x));
                    }
                }
                for (d, k, x) in hechos {
                    tr.escribir(d, k, x, false)?;
                }
            }
            ADD | MUL | DIV | MIN | MAX | MAD | MOV | RSQ | SQRT | DP2 | DP3 | DP4 | LT | GE | EQ | NE | ILT | IGE | IEQ | INE | IADD | MOVC | AND | OR | XOR | NOT | INEG | IMUL | UMUL | IMAD | UMAD | ISHL | ISHR | USHR | IMIN | IMAX | UMIN | UMAX | ITOF | UTOF | FTOI | FTOU | ULT | UGE => {
                let saturar = w & 0x2000 != 0;
                let mut j = i + 1;
                while t[j - 1] >> 31 != 0 && j < fin {
                    j += 1; // los tokens extendidos del codigo
                }
                let mut d = operando(t, &mut j)?;
                // `imul`/`umul` escriben DOS: la mitad alta (que no se usa:
                // null) y la baja.
                if matches!(codigo, IMUL | UMUL) {
                    if d.tipo != NULO {
                        return Err(NoPrograma::Forma("un imul/umul SM5 que usa la mitad alta: todavia no"));
                    }
                    d = operando(t, &mut j)?;
                }
                let n = match codigo {
                    MAD | MOVC | IMAD | UMAD => 3,
                    MOV | RSQ | SQRT | NOT | INEG | ITOF | UTOF | FTOI | FTOU => 1,
                    _ => 2,
                };
                let mut f = Vec::with_capacity(n);
                for _ in 0..n {
                    f.push(operando(t, &mut j)?);
                }
                // Un `-` o un `|x|` de float no es el de un entero (ni el de
                // los bits de `movc`): todavia no.
                if de_enteros(codigo) && f.iter().any(|o| o.modificador != 0) {
                    return Err(NoPrograma::Forma("un modificador en una instruccion SM5 de enteros o de bits: todavia no"));
                }
                if j != fin {
                    return Err(NoPrograma::Forma("una instruccion SM5 que no mide lo que dice"));
                }
                let punto = match codigo {
                    DP2 => 2,
                    DP3 => 3,
                    DP4 => 4,
                    _ => 0,
                };
                if punto > 0 {
                    let mut a = [0; 4];
                    let mut b = [0; 4];
                    for k in 0..punto {
                        a[k] = tr.fuente(&f[0], k)?;
                        b[k] = tr.fuente(&f[1], k)?;
                    }
                    let x = tr.nuevo()?;
                    tr.p.ops.push(Op::Dot { d: x, n: punto as u8, a, b });
                    for k in (0..4).filter(|k| d.mascara & (1 << k) != 0) {
                        tr.escribir(&d, k, x, saturar)?;
                    }
                } else {
                    // Primero se leen TODAS las fuentes: `mul r0.xy, r0.yx, ...`
                    // lee el r0 de antes.
                    let mut hechos = Vec::with_capacity(4);
                    for k in (0..4).filter(|k| d.mascara & (1 << k) != 0) {
                        let s: Vec<Reg> = f.iter().map(|o| tr.fuente(o, k)).collect::<Result<_, _>>()?;
                        let x = if codigo == MOV && !tr.variables {
                            s[0]
                        } else if codigo == MOV {
                            // E6: con variables el `mov` COPIA ya lo que lee
                            // (`mov r0.xy, r0.yx` lee el r0 de antes).
                            let x = tr.nuevo()?;
                            tr.p.ops.push(Op::Copia { d: x, a: s[0] });
                            x
                        } else {
                            tr.operar(codigo, &s)?
                        };
                        hechos.push((k, x));
                    }
                    for (k, x) in hechos {
                        tr.escribir(&d, k, x, saturar)?;
                    }
                }
            }
            SAMPLE => {
                if w >> 31 != 0 {
                    // Un token extendido: los desplazamientos (aoffimmi).
                    let e = palabra(t, i + 1)?;
                    if e & 0x3F == 1 && (e >> 9) & 0xFFF != 0 {
                        return Err(NoPrograma::Forma("un sample SM5 con desplazamiento (aoffimmi): todavia no"));
                    }
                }
                let saturar = w & 0x2000 != 0;
                let mut j = i + 1;
                while t[j - 1] >> 31 != 0 && j < fin {
                    j += 1;
                }
                let d = operando(t, &mut j)?;
                let dir = operando(t, &mut j)?;
                let res = operando(t, &mut j)?;
                let smp = operando(t, &mut j)?;
                if j != fin || res.tipo != RESOURCE || smp.tipo != SAMPLER {
                    return Err(NoPrograma::Forma("un sample SM5 que no es (destino, direccion, tN, sM)"));
                }
                let (Ok(tn), Ok(sn)) = (u8::try_from(res.indices[0]), u8::try_from(smp.indices[0])) else {
                    return Err(NoPrograma::Forma("un sample SM5 con un registro imposible"));
                };
                let (u, v) = (tr.fuente(&dir, 0)?, tr.fuente(&dir, 1)?);
                let x = tr.nuevo()?;
                for _ in 0..3 {
                    tr.nuevo()?;
                }
                tr.p.ops.push(Op::Muestra { d: x, t: tn, s: sn, u, v });
                // El swizzle del recurso dice que canal va a cada componente.
                for k in (0..4).filter(|k| d.mascara & (1 << k) != 0) {
                    tr.escribir(&d, k, x + res.sel[k] as Reg, saturar)?;
                }
            }
            c => return Err(NoPrograma::Sm5(c)),
        }
        i = fin;
    }
    if !acabado {
        return Err(NoPrograma::Forma("un programa SM5 sin ret"));
    }
    if tr.hondo != 0 {
        return Err(NoPrograma::Forma("un if o un loop SM5 sin cerrar"));
    }
    // Las salidas: lo ultimo escrito en cada o#.
    for (r, comps) in core::mem::take(&mut tr.outs).into_iter().enumerate() {
        for (c, x) in comps.iter().enumerate() {
            if let Some(s) = *x {
                let (e, k) = elemento(tr.salidas, r as u32, c as u8).ok_or(NoPrograma::Forma("un o# que la firma de salida no tiene"))?;
                tr.p.ops.push(Op::Salida { s, elemento: e, componente: k });
            }
        }
    }
    tr.p.forma().map_err(|_| NoPrograma::Forma("los if/else/loop/break SM5 no casan"))?;
    Ok(tr.p)
}

/// E6c: los valores de los `case` de un `switch` (los de SU nivel), desde
/// la instruccion `desde` (la de detras del `switch`) hasta su `endswitch`.
fn casos_del_switch(t: &[u32], desde: usize, medida: usize) -> Result<Vec<u32>, NoPrograma> {
    let (mut i, mut hondo, mut casos) = (desde, 0usize, Vec::new());
    while i < medida {
        let w = t[i];
        let codigo = w & 0x7FF;
        let largo = ((w >> 24) & 0x7F) as usize;
        if largo == 0 {
            break;
        }
        match codigo {
            SWITCH => hondo += 1,
            ENDSWITCH if hondo == 0 => return Ok(casos),
            ENDSWITCH => hondo -= 1,
            CASE if hondo == 0 => {
                let mut j = i + 1;
                casos.push(operando(t, &mut j)?.inmediato[0]);
            }
            _ => {}
        }
        i += largo;
    }
    Err(NoPrograma::Forma("un switch SM5 sin su endswitch"))
}

/// E6: si el programa (de la palabra 2 a `medida`) tiene algun salto.
fn salta(t: &[u32], medida: usize) -> bool {
    let mut i = 2;
    while i < medida {
        let w = t[i];
        let codigo = w & 0x7FF;
        if matches!(codigo, IF | LOOP | BREAK | BREAKC | CONTINUE | CONTINUEC | ELSE | ENDIF | ENDLOOP | SWITCH) {
            return true;
        }
        let largo = if codigo == CUSTOMDATA { t.get(i + 1).copied().unwrap_or(0) as usize } else { ((w >> 24) & 0x7F) as usize };
        if largo == 0 {
            return false;
        }
        i += largo;
    }
    false
}
