//! **LA LECTURA DEL JUEZ** (13-10): que instruccion es cada palabra de 128
//! bits, que registros y predicados lee y escribe, sus barreras y su espera.
//! Salio de `juez.rs` entera, como pieza: el juez JUZGA lo que esto LEE, y
//! juntos pasaban de las 1.000 lineas de codigo (L6a). Lo que no se sabe
//! leer es `None`, y para el juez eso es R0: "no se".

/// El registro cero (`RZ`): ni se lee ni se escribe.
pub(super) const RZ: u8 = 255;

fn campo(lo: u64, hi: u64, desde: u32, bits: u32) -> u32 {
    let v = if desde >= 64 { hi >> (desde - 64) } else if desde + bits <= 64 { lo >> desde } else { lo >> desde | hi << (64 - desde) };
    (v & ((1u64 << bits) - 1)) as u32
}

/// Como se cronometra una instruccion (NAK `RegLatencySM80`, reducido).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Clase {
    /// Acoplada de ALU: IADD3, LOP3, ISETP, MOV, SEL, LEA, SHF...
    Alu,
    /// Acoplada del FMA: IMAD, FFMA, FADD, FMUL.
    Fma,
    /// IMAD.WIDE / IMAD.HI: la mitad alta llega tarde.
    Ancha,
    /// CS2R.64, NOP: el despacho de 64 bits.
    Disp64,
    /// Desacoplada: MUFU, I2F, F2I, S2R... (necesita barrera).
    Desacoplada,
    /// Desacoplada de memoria o atributos: LDG, STG, ALD, AST, IPA.
    Agu,
    /// Sin registros de proposito general: saltos, NOP, uniformes.
    Nada,
}

/// Una instruccion leida.
#[derive(Clone, Copy, Debug)]
pub(super) struct Instr {
    pub(super) op: u32,
    pub(super) clase: Clase,
    /// Lo que escribe: (primer registro, cuantos).
    pub(super) escribe: (u8, u8),
    /// Lo que lee: hasta cuatro tramos (registro, cuantos).
    pub(super) lee: [(u8, u8); 4],
    pub(super) espera: u32,
    pub(super) bar_escritura: u32,
    pub(super) bar_lectura: u32,
    pub(super) mascara: u32,
    /// EXIT sin predicado (la ultima de verdad).
    pub(super) fin: bool,
    /// BRA hacia atras (hay bucle).
    pub(super) atras: bool,
    /// ALD/AST: (atributo base, cuantos) para R5.
    pub(super) atributo: Option<(u32, u32)>,
    /// LDG/STG.
    pub(super) memoria: bool,
    /// E6: los predicados que escribe (7 = ninguno).
    pub(super) escribe_p: [u8; 2],
    /// E6: los que lee: (predicado, como guarda). 7 = ninguno.
    pub(super) lee_p: [(u8, bool); 3],
    /// E6: un BRA.
    pub(super) salto: bool,
    /// DL12: un KILL (el `discard` de un programa de pixel).
    pub(super) mata: bool,
}

pub(super) const NADA: (u8, u8) = (RZ, 0);

fn reg(r: u32, n: u8) -> (u8, u8) {
    if r as u8 == RZ { NADA } else { (r as u8, n) }
}

/// ** Los registros que la tarjeta se queda (26-09, metal 06:33). En Volta y
/// despues, de los que se le dan a un programa, DOS se gastan en el contador
/// de programa (NAK `sm70.rs`, `hw_reserved_gprs`: "2 GPRs get burned for the
/// program counter", la nota de la tabla 2 del documento de Volta), y NAK
/// escribe `num_gprs + 2`. Con `REGISTROS = 16` el programa tiene R0..R13: el
/// de vertice de VERRANO V0 usaba R14 y la 3060 lo dijo con su Xid 13 --
/// "Graphics SM Warp Exception: Out Of Range Register". El juez v1 no lo sabia.
pub const RESERVADOS: u32 = 2;

/// Cuantos registros mueve un LDG/STG por su tipo (73..76).
fn por_tipo(t: u32) -> u8 {
    match t {
        5 => 2,
        6 => 4,
        _ => 1,
    }
}

/// Si el juez sabe leer esta instruccion (si no, seria R0 "no se"). Con
/// ella el banco comprueba que todo lo que fabrica el codificador (E2,
/// `bmo-sm86`) el juez lo puede juzgar.
pub fn conoce(lo: u64, hi: u64) -> bool {
    decodificar(lo, hi).is_some()
}

pub(super) fn decodificar(lo: u64, hi: u64) -> Option<Instr> {
    let op = (lo & 0x1FF) as u32;
    let forma = (lo >> 9 & 7) as u32;
    let control = (hi >> 41) as u32;
    let r = |d, b| campo(lo, hi, d, b);
    let rd = r(16, 8);
    let (s0, s1, s2) = (r(24, 8), r(32, 8), r(64, 8));
    // Las fuentes de una de ALU, segun su forma (encode_alu_base). [!] Una
    // fuente AUSENTE deja su campo a 0 -- que es R0 --, asi que cada opcode
    // dice cuantas fuentes tiene de verdad:
    //    tres (IADD3, LOP3, IMAD, FFMA, SHF, LEA): src0; src1 en 32..40 (forma
    //         1) o en 64..72 (formas 2, 3, 7); src2 en 64..72 (1, 4, 5, 6)
    //    dos  (ISETP, IMNMX, SEL, FADD...): src0; src1 en 32..40 si forma 1
    //    una  (MOV, IABS): solo src1 en 32..40 si forma 1
    let alu = |fuentes: u8, ancha_c: bool| -> [(u8, u8); 4] {
        let src0 = if fuentes >= 2 { reg(s0, 1) } else { NADA };
        let src1 = if forma == 1 { reg(s1, 1) } else if fuentes == 3 && matches!(forma, 2 | 3 | 7) { reg(s2, 1) } else { NADA };
        let src2 = if fuentes == 3 && matches!(forma, 1 | 4 | 5 | 6) { reg(s2, if ancha_c { 2 } else { 1 }) } else { NADA };
        [src0, src1, src2, NADA]
    };
    let mut i = Instr {
        op,
        clase: Clase::Alu,
        escribe: NADA,
        lee: [NADA; 4],
        espera: control & 0xF,
        bar_escritura: control >> 5 & 7,
        bar_lectura: control >> 8 & 7,
        mascara: control >> 11 & 0x3F,
        fin: false,
        atras: false,
        atributo: None,
        memoria: false,
        escribe_p: [7; 2],
        // El guarda (12..16) de cualquiera: PT (7) no se lee.
        lee_p: [((lo >> 12 & 7) as u8, true), (7, false), (7, false)],
        salto: false,
        mata: false,
    };
    // Los predicados de FSETP/ISETP (escriben 81..84 y 84..87; leen el que
    // combinan, 87..90, e ISETP el de .EX, 68..71), de SEL (lee 87..90) y
    // de IADD3 (sus acarreos).
    let pred = |desde: u32| r(desde, 3) as u8;
    match op {
        0x0B | 0x0C => {
            i.escribe_p = [pred(81), pred(84)];
            i.lee_p[1] = (pred(87), false);
            if op == 0x0C {
                i.lee_p[2] = (pred(68), false);
            }
        }
        0x07 => i.lee_p[1] = (pred(87), false),
        // E6d: IADD3 escribe sus acarreos (81..84 y 84..87; PT si no); con
        // .X (el 74) los SUMA: lee 87..90 y 77..80 como operandos.
        0x10 => {
            i.escribe_p = [pred(81), pred(84)];
            if r(74, 1) == 1 {
                i.lee_p[1] = (pred(87), false);
                i.lee_p[2] = (pred(77), false);
            }
        }
        _ => {}
    }
    match op {
        // Acopladas de ALU con destino.
        0x10 | 0x12 | 0x11 | 0x19 | 0x16 => {
            i.escribe = reg(rd, 1);
            i.lee = alu(3, false);
        }
        0x17 | 0x07 | 0x09 => {
            i.escribe = reg(rd, 1);
            i.lee = alu(2, false);
        }
        0x13 | 0x02 => {
            i.escribe = reg(rd, 1);
            i.lee = alu(1, false);
        }
        // Comparaciones: el destino es un predicado.
        0x0C | 0x0B => i.lee = alu(2, false),
        // FMA: IMAD y FFMA (tres), FADD y FMUL (dos).
        0x24 | 0x23 | 0x21 | 0x20 => {
            i.clase = Clase::Fma;
            i.escribe = reg(rd, 1);
            i.lee = alu(if op == 0x24 || op == 0x23 { 3 } else { 2 }, false);
        }
        // IMAD.WIDE: escribe un par y su C es un par. IMAD.HI: una.
        0x25 | 0x27 => {
            i.clase = Clase::Ancha;
            i.escribe = reg(rd, if op == 0x25 { 2 } else { 1 });
            i.lee = alu(3, op == 0x25);
        }
        // CS2R: .32 o .64 (bit 80).
        0x05 => {
            let dos = r(80, 1) == 1;
            i.clase = if dos { Clase::Disp64 } else { Clase::Alu };
            i.escribe = reg(rd, if dos { 2 } else { 1 });
        }
        // Desacopladas: MUFU, I2F, F2I (la fuente en el sitio de src1).
        0x108 | 0x106 | 0x105 => {
            i.clase = Clase::Desacoplada;
            i.escribe = reg(rd, 1);
            i.lee = [if forma == 1 { reg(s1, 1) } else { NADA }, NADA, NADA, NADA];
        }
        // ** L2 (14-10): SHFL -- desacoplada como un LDG (NAK la cuenta
        // `DecoupledAgu`: barrera para lo que escribe y para lo que lee) --;
        // la fuente en 24..32, la lane en 32..40 (formas 1 y 2: 0x389, 0x589)
        // y la c en 64..72 (1 y 4: 0x389, 0x989); en la 0xF89 (7), las dos
        // inmediatas. Escribe su predicado de "dentro" (81..84).
        0x189 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), if matches!(forma, 1 | 2) { reg(s1, 1) } else { NADA }, if matches!(forma, 1 | 4) { reg(s2, 1) } else { NADA }, NADA];
            i.escribe_p = [pred(81), 7];
        }
        // L2: FSWZADD (0x822), del FMA como FADD: el vecino en 24..32 y el
        // propio en 64..72.
        0x022 if forma == 4 => {
            i.clase = Clase::Fma;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), reg(s2, 1), NADA, NADA];
        }
        // S2R.
        0x119 => {
            i.clase = Clase::Desacoplada;
            i.escribe = reg(rd, 1);
        }
        // LDG: destino por su tipo; la direccion, un par si es .E (bit 72).
        0x181 => {
            i.clase = Clase::Agu;
            i.memoria = true;
            i.escribe = reg(rd, por_tipo(r(73, 3)));
            i.lee = [reg(s0, if r(72, 1) == 1 { 2 } else { 1 }), NADA, NADA, NADA];
        }
        // STG: la direccion y el dato.
        0x186 => {
            i.clase = Clase::Agu;
            i.memoria = true;
            i.lee = [reg(s0, if r(72, 1) == 1 { 2 } else { 1 }), reg(s1, por_tipo(r(73, 3))), NADA, NADA];
        }
        // ALD: destino (74..76 + 1 componentes), vertice en 32..40, desplazamiento en 24..32.
        0x121 => {
            let n = r(74, 2) + 1;
            i.clase = Clase::Agu;
            i.escribe = reg(rd, n as u8);
            i.lee = [reg(s0, 1), reg(s1, 1), NADA, NADA];
            i.atributo = Some((r(40, 10), n));
        }
        // AST: el dato en 32..40, el vertice en 64..72, desplazamiento en 24..32.
        0x122 => {
            let n = r(74, 2) + 1;
            i.clase = Clase::Agu;
            i.lee = [reg(s1, n as u8), reg(s2, 1), reg(s0, 1), NADA];
            i.atributo = Some((r(40, 10), n));
        }
        // ** LI2d-HDR (12-10): el redondeo al half de la salida de un render
        // target de float (`pegamento::Redondeo`). SOLO las formas de `ptxas`
        // (sm_86) para `cvt.rn.f16.f32` y `cvt.f32.f16`:
        //    F2FP.PACK_AB Rd, RZ, Rb        0x723E, Ra = RZ, nada en 40..64,
        //                                   0xFF en los 41 bits bajos de hi
        //    HADD2.F32 Rd, -RZ, Rb.H0_H0    0x7230, Ra = RZ, el -RZ (0x20 en
        //                                   56..64), 0x4100 en hi
        // Las dos, una fuente (Rb) y del FMA para la latencia.
        0x03E if forma == 1 && s0 == RZ as u32 && lo >> 40 == 0 && hi & ((1 << 41) - 1) == 0xFF => {
            i.clase = Clase::Fma;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s1, 1), NADA, NADA, NADA];
        }
        0x030 if forma == 1 && s0 == RZ as u32 && lo >> 40 == 0x20_0000 && hi & ((1 << 41) - 1) == 0x4100 => {
            i.clase = Clase::Fma;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s1, 1), NADA, NADA, NADA];
        }
        // IPA: el atributo va en 64..72 (NO es un registro).
        0x126 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), reg(s1, 1), NADA, NADA];
        }
        // TEX con el asa en un registro (0x361: op 0x161, forma 1; P3b4c.8
        // T1, `texturas::tex`) y, desde E8g (DL17, 09-10), TLD (0x367: op
        // 0x167). SOLO las formas que se saben -- las de
        // `bmo_sm86::codifica::{tex_forma, tld}`, sus palabras de `ptxas`
        // (`ORO_TEX`) --:
        //
        //    dim (61..64)  1 2D, 2 3D, 3 CUBE, 5 ARRAY_2D, 7 ARRAY_CUBE: 2, 3,
        //                  3, 3 y 4 coordenadas en Ra.. (alineadas a 2 o a 4)
        //    nivel (87..)  1 .LZ (solo 2D): el asa en Rb; 3 .LL: el asa y el
        //                  nivel en Rb, Rb+1 (el par alineado)
        //    .B y .SCR     el 59 siempre; el 60 solo en 2D
        //    .CL (77)      en un TLD con .LL, y solo ahi
        //
        // Los cuatro canales seguidos (Rd2 = Rd + 2, mascara 0xF), sin
        // desplazamientos, comparacion, F16 ni predicado de salida. Lo demas
        // es otra instruccion para el juez: R0.
        0x161 | 0x167 if forma == 1 => {
            let (dim, nivel, tld) = (r(61, 3), r(87, 3), op == 0x167);
            let coordenadas = match dim {
                1 => 2,
                2 | 3 | 5 => 3,
                7 => 4,
                _ => return None,
            };
            let alinea = if coordenadas == 2 { 2 } else { 4 };
            let (n_b, con_nivel) = (if nivel == 3 { 2 } else { 1 }, nivel == 3);
            let forma_sabida = r(12, 4) == 7
                && r(59, 1) == 1
                && r(60, 1) == (dim == 1) as u32
                && (nivel == 3 || nivel == 1 && dim == 1)
                && r(77, 1) == (tld && con_nivel) as u32
                && r(64, 8) == rd + 2
                && r(72, 4) == 0xF
                && r(76, 1) == 0
                && r(78, 3) == 0
                && r(81, 3) == 7
                && r(84, 3) == 1
                && r(90, 1) == 0
                && r(40, 19) == 0
                && r(91, 14) == 0
                && rd % 2 == 0
                && s0 % alinea == 0
                && s1 % n_b == 0
                && rd + 4 <= RZ as u32
                && s0 + coordenadas <= RZ as u32
                && s1 + n_b <= RZ as u32;
            if !forma_sabida {
                return None;
            }
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 4);
            i.lee = [reg(s0, coordenadas as u8), reg(s1, n_b as u8), NADA, NADA];
        }
        // ** E8f (DL18, 09-10): LDC, una o dos palabras de un banco de
        // constantes con el indice en Ra (en bytes; RZ, sin indice). SOLO la
        // forma de `bmo_sm86::codifica::ldc` (sus palabras de `ptxas`): forma
        // 5, 32 o 64 bits (73..76), nada en 32..40 ni en 59..64, ni modo, ni
        // nada mas en la alta; el par alineado. Desacoplada: su barrera.
        0x182 if forma == 5 => {
            let ancho = r(73, 3);
            let doble = ancho == 5;
            let sabida = r(12, 4) == 7 && (ancho == 4 || doble) && r(32, 8) == 0 && r(59, 5) == 0 && hi & ((1 << 41) - 1) == (ancho as u64) << 9 && (!doble || rd % 2 == 0 && rd < RZ as u32 - 1);
            if !sabida {
                return None;
            }
            i.clase = Clase::Desacoplada;
            i.escribe = reg(rd, if doble { 2 } else { 1 });
            i.lee = [reg(s0, 1), NADA, NADA, NADA];
        }
        // ** LI2f (12-10): la compartida del grupo -- LDS, STS, ATOMS y
        // ATOMS.CAS --, SOLO en las formas de `bmo_sm86::computo` (de oro):
        // [Ra] sin inmediato, 32 bits; lo demas, R0.
        0x184 if forma == 4 && lo >> 40 == 0 && hi & ((1 << 41) - 1) == 0x800 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), NADA, NADA, NADA];
        }
        0x188 if forma == 1 && lo >> 40 == 0 && hi & ((1 << 41) - 1) == 0x800 => {
            i.clase = Clase::Agu;
            i.lee = [reg(s0, 1), reg(s1, 1), NADA, NADA];
        }
        0x18C if forma == 1 && lo >> 40 == 0 && hi & ((1 << 41) - 1) & !(0xF << 23 | 1 << 9) == 0 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), reg(s1, 1), NADA, NADA];
        }
        0x18D if forma == 1 && lo >> 40 == 0 && hi & ((1 << 41) - 1) & !0xFF == 0 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), reg(s1, 1), reg(s2, 1), NADA];
        }
        // ** LI2f: ATOMG.E y ATOMG.E.CAS (32 bits, .STRONG.GPU), sus formas:
        // lo de antes en Rd, la direccion un par, el valor (y en el CAS, el
        // nuevo en 64..72).
        0x1A8 if forma == 4 && lo >> 40 == 0 && hi & ((1 << 41) - 1) & !(0xF << 23 | 1 << 9) == 0x081e_e1c4 & !(0xF << 23) => {
            i.clase = Clase::Agu;
            i.memoria = true;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 2), reg(s1, 1), NADA, NADA];
        }
        0x1A9 if forma == 1 && lo >> 40 == 0 && hi & ((1 << 41) - 1) & !0xFF == 0x001e_e100 => {
            i.clase = Clase::Agu;
            i.memoria = true;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 2), reg(s1, 1), reg(s2, 1), NADA];
        }
        // ** LI2f (12-10): BAR.SYNC 0x0, la barrera del grupo de un computo
        // (`bmo_sm86::computo::bar_sync`, de oro), y solo esa forma.
        0x11D if lo == 0x7b1d && hi & ((1 << 41) - 1) == 1 << 16 => i.clase = Clase::Nada,
        // Sin registros: NOP, BSSY, BSYNC, EXIT, BRA; y los UNIFORMES.
        0x118 | 0x145 | 0x141 | 0x82 | 0x90 | 0x99 | 0x1C3 => i.clase = Clase::Nada,
        0x14D => {
            i.clase = Clase::Nada;
            // Sin predicado: PT (7) y no negado (bits 12..16).
            i.fin = lo >> 12 & 0xF == 7;
        }
        0x147 => {
            i.clase = Clase::Nada;
            i.salto = true;
            // El desplazamiento, con signo, en 34..82: negativo = hacia atras.
            i.atras = r(81, 1) == 1;
        }
        // ** DL12 (09-10): KILL, el `discard` de un programa de pixel. SOLO la
        // forma que se sabe (`bmo_sm86::codifica::kill`, leida por `nvdisasm`):
        // su predicado PT en 87..91 y nada mas que el guarda y el control. Lo
        // demas (`KILL P0`...) es otra instruccion para el juez: R0.
        0x15B if lo & !0xF000 == 0x95B && hi & ((1 << 41) - 1) == 7 << 23 => {
            i.clase = Clase::Nada;
            i.mata = true;
        }
        _ => return None,
    }
    Some(i)
}

