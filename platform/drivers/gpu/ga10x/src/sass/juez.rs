//! **EL JUEZ DEL SASS** (J1 de `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`) --
//! si la 3060 calla, el juez habla.
//!
//! [carril]  VERDE     puro: lee palabras de 128 bits y dice SI o NO
//! [consumo] NADA      una pasada por programa, sin memoria dinamica
//!
//! # Por que existe
//!
//! Un programa de SM86 mal hecho no da error: cada instruccion lleva sus bits
//! de control -- cuantos ciclos esperar, que barrera encender al acabar una
//! carga, que barreras esperar -- y el hardware NO comprueba nada, obedece.
//! VERRANO V0 se quedo colgado en los VERTICES con INTR/EXCEPTION/STATUS a 0
//! (metal 25-09 y 26-09). El juez lee el programa ANTES de que llegue a la
//! 3060 y, si algo no cuadra, lo devuelve: `TOMA TU BODRIO`, con la regla, la
//! instruccion y el registro. Si todo cuadra: `PERFECTO Y PRECISO`.
//!
//! # De donde sale cada numero (nada adivinado)
//!
//! ```text
//!    donde va cada operando  NAK, Mesa 26.2.3: sm70_encode.rs
//!                            (encode_alu_base: formas 1..7; OpALd, OpASt,
//!                            OpIpa, OpLd, OpSt; set_instr_deps: bits 105..122)
//!    acoplada o desacoplada  NAK: sm80_instr_latencies.rs -- "la informacion
//!    y sus latencias         de planificacion de registros que dio NVIDIA",
//!                            para Ampere y Ada (la 3060 es Ampere)
//! ```
//!
//! Los bits de control (NAK `set_instr_deps`), desde el bit 105:
//!
//! ```text
//!    105..109  espera: ciclos antes de emitir la SIGUIENTE instruccion
//!    109       ceder (yield)
//!    110..113  barrera que ENCIENDE al acabar de escribir (7 = ninguna)
//!    113..116  barrera que ENCIENDE al acabar de LEER sus fuentes (7 = ninguna)
//!    116..122  mascara de barreras que ESPERA antes de empezar
//! ```
//!
//! # Las reglas (v1)
//!
//! ```text
//!    R1  dato leido antes de llegar   leer (o escribir encima de) un registro
//!                                     que carga una DESACOPLADA sin esperar su
//!                                     barrera -- o que no encendio ninguna
//!    R2  espera corta                 leer lo que escribe una ACOPLADA antes de
//!                                     su latencia (la tabla de Ampere)
//!    R3  barrera fantasma             esperar una barrera que nada encendio
//!    R4  fuente pisada                escribir un registro que una desacoplada
//!                                     aun LEE (encendio barrera de LECTURA) sin
//!                                     esperarla
//!    R5  la cabecera miente           LDG/STG sin DoesLoadOrStore (bit 26); un
//!                                     ALD/AST a un atributo que la SPH no
//!                                     declara; un registro >= REGISTROS - 2
//!                                     (en Volta y despues, DOS se gastan en el
//!                                     contador de programa)
//!    R6  final sucio                  EXIT con un AST aun leyendo sus datos
//!    R8  salto sucio                  (E6) un BRA con algo aun en vuelo: una
//!                                     desacoplada sin esperar, o una acoplada
//!                                     (o un predicado) que no habra llegado
//!                                     cuando se corra el destino
//!    R9  predicado antes de llegar    (E6) leer un predicado antes de 13
//!                                     ciclos como GUARDA, o de 4 como operando
//!                                     (SEL): lo que hace `ptxas`
//!    R0  no se                        una instruccion que el juez no conoce:
//!                                     tambien es NO (un juez que aprueba lo que
//!                                     no entiende no es un juez)
//! ```
//!
//! # [!] Lo que v1 NO mira, dicho
//!
//! - El acarreo de `IADD3` que lee `IMAD.X` (sus predicados de 77..90): sin
//!   regla; R9 mira los de FSETP, ISETP y SEL, y el guarda de todas.
//! - Los registros UNIFORMES (`UMOV`, `S2UR`...): solo sus barreras.
//!
//! # Los saltos (E6, 02-10)
//!
//! El programa se sigue leyendo en LINEA RECTA, y R8 es lo que lo hace valer
//! para cualquier camino -- en lo que fabrica BMO-X: los programas de la
//! tuberia grafica (con SPH: VERRANO, PROTON-X y su pegamento, los que sube
//! la puerta del kernel) y lo que se juzga con [`juzgar_drenado`]. Los de
//! computo de `ptxas` (el oro: `giro` salta con cargas en vuelo y las espera
//! en el destino) se siguen leyendo como en v1, sin juzgar sus saltos: en cada BRA todo tiene que haber llegado (lo de las
//! acopladas, contando la espera del propio BRA; las desacopladas,
//! esperadas). Asi quien llega a un destino SALTANDO lo encuentra todo hecho
//! -- nada que el juez, leyendo por debajo, no haya visto mas tarde --, y la
//! vuelta de un bucle tambien. R3 se calla si hay un salto hacia atras (una
//! barrera esperada en la cabeza puede venir de la vuelta anterior).

use crate::raster::SPH;

/// El registro cero (`RZ`): ni se lee ni se escribe.
const RZ: u8 = 255;

/// Las reglas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regla {
    /// Una instruccion que el juez no conoce.
    R0NoSe,
    R1DatoAntesDeLlegar,
    R2EsperaCorta,
    R3BarreraFantasma,
    R4FuentePisada,
    R5CabeceraMiente,
    R6FinalSucio,
    /// P3b4c: el cuerpo que manda una APP toca lo que no es suyo -- memoria,
    /// atributos, constantes, saltos o registros del pegamento.
    R7CuerpoAjeno,
    /// E6: un salto con algo aun en vuelo.
    R8SaltoSucio,
    /// E6: un predicado leido antes de llegar.
    R9PredicadoAntesDeLlegar,
}

impl Regla {
    /// Su nombre, para decirlo.
    pub const fn nombre(self) -> &'static str {
        match self {
            Regla::R0NoSe => "R0 no se: instruccion desconocida",
            Regla::R1DatoAntesDeLlegar => "R1 dato leido antes de llegar",
            Regla::R2EsperaCorta => "R2 espera corta",
            Regla::R3BarreraFantasma => "R3 barrera fantasma",
            Regla::R4FuentePisada => "R4 fuente pisada",
            Regla::R5CabeceraMiente => "R5 la cabecera miente",
            Regla::R6FinalSucio => "R6 final sucio",
            Regla::R7CuerpoAjeno => "R7 cuerpo ajeno: una app toca lo que no es suyo",
            Regla::R8SaltoSucio => "R8 salto sucio: un BRA con algo aun en vuelo",
            Regla::R9PredicadoAntesDeLlegar => "R9 predicado leido antes de llegar",
        }
    }
}

/// **TOMA TU BODRIO**: por que no, y donde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bodrio {
    pub regla: Regla,
    /// La instruccion (desde 0, sin contar la cabecera).
    pub instruccion: usize,
    /// El registro, o la barrera, o el atributo (segun la regla).
    pub que: u32,
    pub detalle: &'static str,
}

/// Lo que el juez necesita saber del programa, ademas del codigo.
#[derive(Clone, Copy, Debug)]
pub struct Contexto<'a> {
    /// Los registros que la tarjeta le da (`SET_PIPELINE_REGISTER_COUNT` o el
    /// QMD): R0..R(n-1).
    pub registros: u32,
    /// La cabecera SPH, si es un programa de la tuberia grafica.
    pub sph: Option<&'a [u32; SPH]>,
}

/// **PERFECTO Y PRECISO**: lo que se comprobo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Veredicto {
    pub instrucciones: usize,
    /// Lecturas de registro comprobadas contra su escritura.
    pub lecturas: u32,
    /// Barreras esperadas (con algo pendiente o no).
    pub esperas: u32,
}

/// Como firma el juez lo que dice (el nombre lo puso el propietario el 26-09:
/// es el juez de V3b, `PLAN_VERRANO.md`).
pub const FIRMA: &str = "[BMO-X Juez V3b]";

/// **EL REMATE**, la linea que va DETRAS de un bodrio. Del propietario, el
/// 26-09: *"no es para humillar"* -- es para que un NO no se lea como un
/// aviso mas. Solo sale con un bodrio: lo PERFECTO Y PRECISO no se remata.
pub const REMATE: &str = "[BMO-X Juez V3b]: UN FRACASADO! UN FRACASADO!!!";

impl core::fmt::Display for Bodrio {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: TOMA TU BODRIO! {} en la instruccion {} ({}): {}", FIRMA, self.regla.nombre(), self.instruccion, self.que, self.detalle)
    }
}

impl core::fmt::Display for Veredicto {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: PERFECTO Y PRECISO: {} instrucciones, {} lecturas y {} esperas comprobadas", FIRMA, self.instrucciones, self.lecturas, self.esperas)
    }
}

// == Decodificar =============================================================

fn campo(lo: u64, hi: u64, desde: u32, bits: u32) -> u32 {
    let v = if desde >= 64 { hi >> (desde - 64) } else if desde + bits <= 64 { lo >> desde } else { lo >> desde | hi << (64 - desde) };
    (v & ((1u64 << bits) - 1)) as u32
}

/// Como se cronometra una instruccion (NAK `RegLatencySM80`, reducido).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clase {
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
struct Instr {
    op: u32,
    clase: Clase,
    /// Lo que escribe: (primer registro, cuantos).
    escribe: (u8, u8),
    /// Lo que lee: hasta cuatro tramos (registro, cuantos).
    lee: [(u8, u8); 4],
    espera: u32,
    bar_escritura: u32,
    bar_lectura: u32,
    mascara: u32,
    /// EXIT sin predicado (la ultima de verdad).
    fin: bool,
    /// BRA hacia atras (hay bucle).
    atras: bool,
    /// ALD/AST: (atributo base, cuantos) para R5.
    atributo: Option<(u32, u32)>,
    /// LDG/STG.
    memoria: bool,
    /// E6: los predicados que escribe (7 = ninguno).
    escribe_p: [u8; 2],
    /// E6: los que lee: (predicado, como guarda). 7 = ninguno.
    lee_p: [(u8, bool); 3],
    /// E6: un BRA.
    salto: bool,
}

const NADA: (u8, u8) = (RZ, 0);

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

fn decodificar(lo: u64, hi: u64) -> Option<Instr> {
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
    };
    // Los predicados de FSETP/ISETP (escriben 81..84 y 84..87; leen el que
    // combinan, 87..90, e ISETP el de .EX, 68..71) y de SEL (lee 87..90).
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
        // IPA: el atributo va en 64..72 (NO es un registro).
        0x126 => {
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 1);
            i.lee = [reg(s0, 1), reg(s1, 1), NADA, NADA];
        }
        // TEX con el asa en un registro (0x361: op 0x161, forma 1; P3b4c.8
        // T1, `texturas::tex`). SOLO la forma que se sabe: 2D, .LZ, los
        // cuatro canales seguidos (Rd2 = Rd + 2, mascara 0xF), sin
        // desplazamientos, comparacion, F16 ni predicado de salida, pares
        // alineados y un asa. Lo demas es otra instruccion para el juez: R0.
        0x161 if forma == 1 => {
            let forma_sabida = r(12, 4) == 7
                && r(59, 5) == 0b00111
                && r(64, 8) == rd + 2
                && r(72, 4) == 0xF
                && r(76, 5) == 0
                && r(81, 3) == 7
                && r(84, 3) == 1
                && r(87, 4) == 1
                && r(40, 19) == 0
                && r(91, 14) == 0
                && rd % 2 == 0
                && s0 % 2 == 0
                && rd as u8 != RZ
                && s0 as u8 != RZ
                && s1 as u8 != RZ;
            if !forma_sabida {
                return None;
            }
            i.clase = Clase::Agu;
            i.escribe = reg(rd, 4);
            i.lee = [reg(s0, 2), reg(s1, 1), NADA, NADA];
        }
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
        _ => return None,
    }
    Some(i)
}

// == R7: EL CUERPO DE UNA APP (P3b4c, 28-09) ================================

/// **Lo que puede traer el cuerpo de un sombreador que manda una APP.**
///
/// El juez de R0..R6 dice si un programa esta BIEN HECHO (registros,
/// barreras, esperas). No dice a QUE MEMORIA va: un `STG` bien cronometrado
/// a una VA cualquiera de la 3060 --el lienzo del escritorio, el contexto del
/// GR-- es PERFECTO Y PRECISO para R0..R6. Con programas del escritorio daba
/// igual: los fabrica el anfitrion y viajan dentro de `d.bex`. Con la puerta
/// de las apps (`CUBO_RECETA`) NO: el cuerpo lo fabrica la app.
///
/// Por eso la app manda solo el CUERPO y el kernel pone el pegamento (las
/// lecturas de los DATOS, los atributos, la salida): todas las direcciones
/// las decide el kernel. Y el cuerpo es una LISTA BLANCA, la de lo que emite
/// PROTON-X (`bmo-proton-x-sm86`) y nada mas:
///
/// ```text
///    FADD FMUL FFMA FMNMX MOV   con registros o inmediatos -- sin c[][]
///    MUFU                       con un registro
///    FSETP ISETP SEL IADD3      (E6) igual: registros o inmediatos
///    IMAD LOP3 SHF IMNMX        (E6c) los enteros: igual
///    I2F F2I                    (E6c) las conversiones: con un registro
///    BRA                        (E6) a una instruccion DEL CUERPO (de la 0 a
///                               su EXIT, que es donde sigue el pegamento)
///    EXIT                       la ultima, y solo ella
///    guarda                     PT siempre, salvo en BRA (su `@P`)
///    destino                    < registros: los del pegamento no se tocan
/// ```
///
/// Nada de LDG/STG/ALD/AST/IPA, ni un banco de constantes, ni un salto fuera.
///
/// [!] Un bucle que no sale cuelga la 3060 (E6): el juez no puede saber si
/// acaba. Lo que lo para es del kernel, no de esta lista.
pub fn juzgar_cuerpo_de_app(codigo: &[(u64, u64)], registros: u32) -> Result<(), Bodrio> {
    juzgar_cuerpo_con_asas(codigo, registros, 0)
}

/// **R7 con TEXTURAS** (P3b4c.8 T2, 29-09): lo mismo, y un `TEX` (la unica
/// forma que el juez sabe, `texturas::tex`) SOLO si su asa esta en un
/// registro de `asas` (mascara de bits: los que carga el PEGAMENTO del
/// kernel con `asa(k, k)`), y ninguna instruccion del cuerpo escribe en
/// ellos. Asi la app elige las coordenadas y a donde van los cuatro
/// canales, pero NO que TIC ni que TSC lee la 3060: un asa suya podria
/// apuntar a una piscina entera de descriptores que no son suyos.
pub fn juzgar_cuerpo_con_asas(codigo: &[(u64, u64)], registros: u32, asas: u64) -> Result<(), Bodrio> {
    let ajeno = |k: usize, que: u32, detalle: &'static str| Err(Bodrio { regla: Regla::R7CuerpoAjeno, instruccion: k, que, detalle });
    let Some(ultima) = codigo.len().checked_sub(1) else {
        return ajeno(0, 0, "un cuerpo vacio: le falta su EXIT");
    };
    for (k, &(lo, hi)) in codigo.iter().enumerate() {
        let op = (lo & 0x1FF) as u32;
        let forma = (lo >> 9 & 7) as u32;
        if lo >> 12 & 0xF != 7 && op != 0x147 {
            return ajeno(k, op, "con predicado: en el cuerpo de una app solo un BRA lleva guarda");
        }
        let formas: &[u32] = match op {
            // FADD: inmediato en la forma 2 (y c[][] en la 3).
            0x021 => &[1, 2],
            // FMUL, FFMA, FMNMX, MOV, y (E6) FSETP, ISETP, SEL, IADD3:
            // inmediato en la 4 (y c[][] en la 5).
            0x020 | 0x023 | 0x009 | 0x002 | 0x00B | 0x00C | 0x007 | 0x010 | 0x024 | 0x012 | 0x019 | 0x017 => &[1, 4],
            0x106 | 0x105 => &[1],
            // E6d: la division -- IMAD.HI.U32 (con c = RZ: el par no se
            // usa) e IABS, de registros.
            0x027 | 0x013 => &[1],
            // E6: un salto, si cae DENTRO del cuerpo (de 0 a su EXIT).
            0x147 => {
                let d = (((hi & 0x3_FFFF) << 32 | lo >> 32) << 14) as i64 >> 14;
                let destino = 16 * (k as i64 + 1) + d;
                if d % 16 != 0 || destino < 0 || destino > 16 * ultima as i64 {
                    return ajeno(k, op, "un salto que sale del cuerpo: solo de la 0 a su EXIT");
                }
                &[4]
            }
            0x108 => &[1],
            // TEX: con un asa que puso el kernel (abajo).
            0x161 if asas != 0 => &[1],
            0x14D if k == ultima => &[4],
            0x14D => return ajeno(k, op, "un EXIT antes del final: el pegamento de detras no correria"),
            _ => return ajeno(k, op, "fuera de la lista blanca de una app (memoria, atributos, saltos...)"),
        };
        if !formas.contains(&forma) {
            return ajeno(k, forma, "una forma de operando fuera de la lista: c[][] es memoria");
        }
        if k == ultima && op != 0x14D {
            return ajeno(k, op, "el cuerpo tiene que acabar en EXIT");
        }
        let Some(i) = decodificar(lo, hi) else {
            return ajeno(k, op, "el juez no sabe leerla");
        };
        let (r, n) = i.escribe;
        if n > 0 && r as u32 + n as u32 > registros {
            return ajeno(k, r as u32, "escribe un registro que no es del cuerpo: los del pegamento son del kernel");
        }
        if n > 0 && (r as u32..r as u32 + n as u32).any(|x| x < 64 && asas >> x & 1 == 1) {
            return ajeno(k, r as u32, "pisa el registro del ASA de una textura: el asa la pone el kernel");
        }
        if op == 0x161 {
            let asa = (lo >> 32 & 0xFF) as u32;
            if asa >= 64 || asas >> asa & 1 == 0 {
                return ajeno(k, asa, "un TEX con un asa que no puso el kernel");
            }
        }
    }
    Ok(())
}

/// E6: lo que tarda un predicado de FSETP/ISETP en poder leerse como GUARDA
/// y como operando (SEL), y lo que un BRA espera a lo escrito antes de que
/// corra su destino -- lo mas largo de la tabla de abajo. Los dos primeros,
/// de `ptxas` (`oro_saltos.ptx`): nunca pone menos.
pub const PREDICADO_GUARDA: u32 = 13;
pub const PREDICADO_OPERANDO: u32 = 4;
pub const DRENAR: u32 = 6;

/// La latencia de lectura tras escritura (NAK `RegLatencySM80::read_after_write`,
/// reducida a estas clases; ante la duda, la mayor).
fn latencia(escritor: Clase, lector: Clase) -> u32 {
    use Clase::*;
    match (lector, escritor) {
        (Alu | Nada, Alu) => 4,
        (Alu | Nada, Fma | Ancha) => 5,
        (Fma, Alu) => 5,
        (Fma, Fma | Ancha) => 4,
        (Ancha, Alu) => 5,
        (Ancha, Fma) => 4,
        (Ancha, Ancha) => 6,
        (_, Disp64) => 6,
        (Desacoplada, _) => 4,
        (Agu, _) => 5,
        _ => 1,
    }
}

// == Juzgar ==================================================================

/// Lo que se sabe de un registro mientras se lee el programa.
#[derive(Clone, Copy)]
struct Registro {
    /// Una desacoplada lo esta escribiendo: su barrera (7 = ninguna).
    escribe_bar: u8,
    pendiente: bool,
    /// Una acoplada lo escribio: en que ciclo y de que clase.
    ciclo: u32,
    clase: Clase,
    acoplado: bool,
    /// Una desacoplada aun lo LEE: su barrera de lectura (7 = ninguna).
    leido_bar: u8,
    leido: bool,
    /// Quien lo lee es un AST (para R6).
    leido_ast: bool,
    /// La barrera de ESCRITURA de quien lo lee (7 = ninguna).
    leido_wbar: u8,
}

const LIMPIO: Registro = Registro { escribe_bar: 7, pendiente: false, ciclo: 0, clase: Clase::Nada, acoplado: false, leido_bar: 7, leido: false, leido_ast: false, leido_wbar: 7 };

fn no(regla: Regla, instruccion: usize, que: u32, detalle: &'static str) -> Result<Veredicto, Bodrio> {
    Err(Bodrio { regla, instruccion, que, detalle })
}

/// **JUZGAR** un programa. `Ok` = PERFECTO Y PRECISO; `Err` = TOMA TU BODRIO.
/// Con SPH (un programa de la tuberia, de BMO-X) los saltos se juzgan (R8).
pub fn juzgar(codigo: &[(u64, u64)], ctx: &Contexto) -> Result<Veredicto, Bodrio> {
    juzgar_con(codigo.len(), |k| codigo[k], ctx, ctx.sph.is_some())
}

/// **JUZGAR con los saltos drenados** (E6): R8 siempre, tenga SPH o no. Lo
/// que emite PROTON-X lo cumple por construccion (`planifica`: un BRA espera
/// a todo); un cuerpo suelto se juzga asi.
pub fn juzgar_drenado(codigo: &[(u64, u64)], ctx: &Contexto) -> Result<Veredicto, Bodrio> {
    juzgar_con(codigo.len(), |k| codigo[k], ctx, true)
}

/// `juzgar`, leyendo la palabra `k` con `palabra` (sin copiar el programa:
/// el kernel juzga los bytes del paquete donde estan, sin 2 KiB en su pila).
fn juzgar_con(n: usize, palabra: impl Fn(usize) -> (u64, u64), ctx: &Contexto, drenados: bool) -> Result<Veredicto, Bodrio> {
    let codigo = (0..n).map(&palabra);
    let mut regs = [LIMPIO; 256];
    let mut encendidas = 0u32;
    // El ciclo en que se emitio quien encendio cada barrera de ESCRITURA la
    // ultima vez (las de lectura no: el AST de NAK con su EXIT detras, a 1
    // ciclo, corrio en el metal).
    let mut encendida_en = [None::<u32>; 6];
    let mut ciclo = 0u32;
    // E6: el ciclo en que se escribio cada predicado.
    let mut pred = [None::<u32>; 7];
    let mut v = Veredicto::default();
    // Un salto hacia atras ANTES del primer EXIT sin predicado: un bucle. El
    // `BRA .` de relleno que va detras del EXIT no cuenta (no se alcanza).
    let mut hay_bucle = false;
    for i in codigo.clone().filter_map(|(lo, hi)| decodificar(lo, hi)) {
        hay_bucle |= i.atras;
        if i.fin {
            break;
        }
    }
    for (k, (lo, hi)) in codigo.enumerate() {
        let Some(i) = decodificar(lo, hi) else {
            return no(Regla::R0NoSe, k, (lo & 0xFFF) as u32, "el juez no conoce este opcode: primero se le da su regla, despues se aprueba");
        };
        v.instrucciones = k + 1;
        // Las barreras que espera: primero, antes de tocar nada.
        for b in 0..6u32 {
            if i.mascara & 1 << b == 0 {
                continue;
            }
            v.esperas += 1;
            if encendidas & 1 << b == 0 && !hay_bucle {
                return no(Regla::R3BarreraFantasma, k, b, "espera una barrera que ninguna instruccion anterior enciende");
            }
            // ** Una barrera tarda UN ciclo en encenderse (metal 28-09, y
            // `ptxas` lo respeta: su MUFU que alguien espera sale con espera
            // 2). Esperarla al ciclo siguiente no espera nada: se lee lo
            // viejo cuando no hay otros warps que metan ciclos por medio --
            // los pixeles de `gpu verrano bmox12` mal al azar por bloques.
            if encendida_en[b as usize].is_some_and(|c| ciclo.saturating_sub(c) < 2) {
                return no(Regla::R3BarreraFantasma, k, b, "espera una barrera de escritura encendida el ciclo anterior: aun no esta encendida (quien la enciende necesita espera 2)");
            }
            for r in regs.iter_mut() {
                if r.pendiente && r.escribe_bar as u32 == b {
                    r.pendiente = false;
                }
                if r.leido && (r.leido_bar as u32 == b || r.leido_wbar as u32 == b) {
                    r.leido = false;
                }
            }
        }
        // R5: la cabecera y los registros.
        for &(r, n) in i.lee.iter().chain(core::iter::once(&i.escribe)) {
            if n > 0 && r as u32 + n as u32 + RESERVADOS > ctx.registros {
                return no(Regla::R5CabeceraMiente, k, r as u32 + n as u32 - 1, "usa un registro que la tarjeta no le da (REGISTROS menos los 2 del contador de programa)");
            }
        }
        if let Some(h) = ctx.sph {
            if i.memoria && h[0] & 1 << 26 == 0 {
                return no(Regla::R5CabeceraMiente, k, 26, "LDG/STG y la SPH no dice DoesLoadOrStore (bit 26)");
            }
            if let Some((a, n)) = i.atributo {
                // VTG: entradas desde el bit 160, salidas desde el 400 (una por palabra).
                let base = if i.op == 0x121 { 160 } else { 400 };
                for c in 0..n {
                    let bit = base + (a / 4 + c) as usize;
                    if bit >= SPH * 32 || h[bit / 32] & 1 << (bit % 32) == 0 {
                        return no(Regla::R5CabeceraMiente, k, a + 4 * c, "ALD/AST a un atributo que la SPH no declara");
                    }
                }
            }
        }
        // R9: los predicados que lee (el guarda, y los de SEL/FSETP/ISETP).
        for &(p, guarda) in &i.lee_p {
            if let Some(c) = pred.get(p as usize).copied().flatten() {
                if ciclo.saturating_sub(c) < if guarda { PREDICADO_GUARDA } else { PREDICADO_OPERANDO } {
                    return no(Regla::R9PredicadoAntesDeLlegar, k, p as u32, if guarda { "un guarda (@P) leido antes de 13 ciclos de su FSETP/ISETP" } else { "un predicado leido como operando antes de 4 ciclos" });
                }
            }
        }
        // R8: en un BRA, todo tiene que haber llegado cuando corra el destino
        // (este ciclo mas su espera): asi el que llega saltando no encuentra
        // nada que la lectura en linea recta no haya visto.
        if i.salto && drenados {
            let llega = ciclo + i.espera.max(1);
            if let Some(x) = (0..256).find(|&x| regs[x].pendiente) {
                return no(Regla::R8SaltoSucio, k, x as u32, "un BRA con una desacoplada aun cargando: esperar su barrera antes de saltar");
            }
            if let Some(x) = (0..256).find(|&x| regs[x].leido) {
                return no(Regla::R8SaltoSucio, k, x as u32, "un BRA con una desacoplada aun leyendo sus fuentes: esperar su barrera antes de saltar");
            }
            if let Some(x) = (0..256).find(|&x| regs[x].acoplado && llega.saturating_sub(regs[x].ciclo) < DRENAR) {
                return no(Regla::R8SaltoSucio, k, x as u32, "un BRA que llega antes de que este escrito lo de una acoplada (6 ciclos con la espera del BRA)");
            }
            if let Some(p) = (0..7).find(|&p| pred[p].is_some_and(|c| llega.saturating_sub(c) < PREDICADO_GUARDA)) {
                return no(Regla::R8SaltoSucio, k, p as u32, "un BRA que llega antes de que este escrito un predicado (13 ciclos)");
            }
        }
        // Lo que lee.
        for &(r, n) in i.lee.iter() {
            for x in r as usize..r as usize + n as usize {
                let e = regs[x];
                v.lecturas += 1;
                if e.pendiente {
                    return no(
                        Regla::R1DatoAntesDeLlegar,
                        k,
                        x as u32,
                        if e.escribe_bar == 7 { "lee lo que carga una desacoplada que NO enciende barrera" } else { "lee lo que carga una desacoplada sin esperar su barrera" },
                    );
                }
                if e.acoplado && ciclo.saturating_sub(e.ciclo) < latencia(e.clase, i.clase) {
                    return no(Regla::R2EsperaCorta, k, x as u32, "lee lo que escribe una acoplada antes de su latencia (tabla de Ampere)");
                }
            }
        }
        // Lo que escribe.
        let (r, n) = i.escribe;
        for x in r as usize..r as usize + n as usize {
            let e = regs[x];
            if e.pendiente {
                return no(Regla::R1DatoAntesDeLlegar, k, x as u32, "escribe encima de lo que una desacoplada aun esta cargando");
            }
            if e.leido {
                return no(Regla::R4FuentePisada, k, x as u32, "escribe un registro que una desacoplada aun esta leyendo");
            }
            let desacoplada = matches!(i.clase, Clase::Desacoplada | Clase::Agu);
            regs[x] = Registro {
                escribe_bar: i.bar_escritura as u8,
                pendiente: desacoplada,
                ciclo,
                clase: i.clase,
                acoplado: !desacoplada,
                ..regs[x]
            };
        }
        // Una desacoplada que LEE y enciende barrera de LECTURA: sus fuentes
        // quedan leidas hasta esa barrera (o hasta la de escritura: si el
        // resultado llego, las fuentes ya se leyeron). Sin barrera de lectura
        // las lee al emitirse: la tabla de Ampere da 1 ciclo de WAR a una
        // desacoplada (NAK `write_after_read`), y `ptxas` cuenta con ello.
        if matches!(i.clase, Clase::Desacoplada | Clase::Agu) && i.bar_lectura < 6 {
            for &(r, n) in i.lee.iter() {
                for x in r as usize..r as usize + n as usize {
                    regs[x].leido = true;
                    regs[x].leido_bar = i.bar_lectura as u8;
                    regs[x].leido_ast = i.op == 0x122;
                    regs[x].leido_wbar = i.bar_escritura as u8;
                }
            }
        }
        for &p in &i.escribe_p {
            if let Some(x) = pred.get_mut(p as usize) {
                *x = Some(ciclo);
            }
        }
        if i.bar_escritura < 6 {
            encendidas |= 1 << i.bar_escritura;
            encendida_en[i.bar_escritura as usize] = Some(ciclo);
        }
        if i.bar_lectura < 6 {
            encendidas |= 1 << i.bar_lectura;
        }
        // ** R2 en el EXIT de un programa de PIXEL (28-09, metal): el EXIT
        // LEE el color de cada destino que la SPH declara (`OmapTarget`, 4
        // bits por destino desde el 576: R4t..R4t+3). `gpu verrano bmox12`
        // lo escribia 2 ciclos antes de un EXIT que salia a los 5 de una FMUL:
        // pixeles con el color a medio escribir, al azar por bloques. El juez
        // no lo miraba.
        if i.fin {
            if let Some(h) = ctx.sph.filter(|h| h[0] & 0x1F == 2) {
                for x in 0..32usize {
                    let bit = 576 + x;
                    if h[bit / 32] & 1 << (bit % 32) == 0 {
                        continue;
                    }
                    let e = regs[x];
                    v.lecturas += 1;
                    if e.pendiente {
                        return no(Regla::R1DatoAntesDeLlegar, k, x as u32, "EXIT de pixel: el color lo carga una desacoplada sin esperar su barrera");
                    }
                    if e.acoplado && ciclo.saturating_sub(e.ciclo) < latencia(e.clase, i.clase) {
                        return no(Regla::R2EsperaCorta, k, x as u32, "EXIT de pixel: lee el color (R0..R3) antes de la latencia de quien lo escribio");
                    }
                }
            }
        }
        // R6: el final, con un AST aun leyendo.
        if i.fin {
            if let Some(x) = (0..256).find(|&x| regs[x].leido && regs[x].leido_ast) {
                return no(Regla::R6FinalSucio, k, x as u32, "EXIT con un AST aun leyendo sus datos: falta esperar su barrera");
            }
            break;
        }
        ciclo += i.espera.max(1);
    }
    Ok(v)
}

/// Cuantas instrucciones caben en un programa que se juzga en bytes: las de
/// un hueco de VERRANO (`tuberia::HUECO`, 2 KiB de codigo tras su SPH). 128
/// desde E5 (28-09, dicho que si): lo que PROTON-X emite para BMOX-12, con
/// su pegamento, no cabia en 64. El kernel lo juzga donde esta, sin copia.
pub const MAX_INSTRUCCIONES: usize = 128;

/// **JUZGAR UN PROGRAMA TAL COMO VIAJA** -- la cabecera SPH (128 B) y
/// detras las instrucciones, en bytes: lo que guarda el BSF (`kind` SM86) y lo
/// que el kernel sube. Asi el que toma un programa de un sobre lo juzga ANTES
/// de mandarlo, con los registros que de verdad le va a dar la tarjeta.
///
/// Un programa que no se sostiene como bytes (cabecera corta, instrucciones a
/// medias, demasiadas) tambien es BODRIO: R5, la cabecera miente sobre lo que
/// trae.
pub fn juzgar_programa(bytes: &[u8], registros: u32) -> Result<Veredicto, Bodrio> {
    let cabecera = 4 * SPH;
    if bytes.len() < cabecera + 16 || (bytes.len() - cabecera) % 16 != 0 {
        return no(Regla::R5CabeceraMiente, 0, bytes.len() as u32, "no es una cabecera SPH y instrucciones enteras de 128 bits");
    }
    let n = (bytes.len() - cabecera) / 16;
    if n > MAX_INSTRUCCIONES {
        return no(Regla::R5CabeceraMiente, MAX_INSTRUCCIONES, n as u32, "mas instrucciones de las que caben en un hueco de la tuberia");
    }
    let u32le = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let u64le = |i: usize| u32le(i) as u64 | (u32le(i + 4) as u64) << 32;
    let mut sph = [0u32; SPH];
    for (k, w) in sph.iter_mut().enumerate() {
        *w = u32le(4 * k);
    }
    juzgar_con(n, |k| (u64le(cabecera + 16 * k), u64le(cabecera + 16 * k + 8)), &Contexto { registros, sph: Some(&sph) }, true)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::sass::corpus::{ORO, SOSPECHOSOS};
    use crate::{color3d, cubo as cu, raster, tuberia as tu};

    /// E2: el juez conoce TODO lo que fabrica el codificador (`bmo-sm86`):
    /// las palabras de oro de `ptxas` y las leidas por `nvdisasm`. Ninguna es
    /// R0 "no se".
    #[test]
    fn el_juez_conoce_lo_que_fabrica_el_codificador() {
        for (texto, lo, hi) in bmo_sm86::codifica::ORO.iter().chain(bmo_sm86::codifica::LEIDAS) {
            assert!(conoce(*lo, *hi), "{texto}");
        }
    }

    /// E3: lo que EMITE el emisor de PROTON-X para los cuatro sombreadores
    /// del cubo (los DXIL de dxc y los SM5 de BMOX-12) es PERFECTO Y PRECISO,
    /// con los registros que dice que usa (mas los DOS de Volta).
    #[test]
    fn lo_emitido_por_el_emisor_de_proton_x_es_perfecto() {
        extern crate std;
        use bmo_proton_x::dxil::{self, programa::compilar};
        let raiz = "../../../shared/proton-x/prueba/";
        for f in ["cubo_vs.dxil", "cubo_ps.dxil", "sombras/f3ef42a0.cso", "sombras/4d67f5e4.cso"] {
            let d = std::fs::read(std::format!("{raiz}{f}")).unwrap();
            // Los dos ABI: el del banco (E3) y el de registros precargados (E5).
            for abi in [bmo_proton_x_sm86::Abi::Banco, bmo_proton_x_sm86::Abi::Registros] {
                let e = bmo_proton_x_sm86::emitir_con(&compilar(&dxil::leer(&d).unwrap()).unwrap(), 64, abi).unwrap();
                let ctx = Contexto { registros: e.registros + RESERVADOS, sph: None };
                let v = juzgar(&e.codigo, &ctx);
                assert!(v.is_ok(), "{f} {abi:?}: {}", v.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            }
        }
    }

    /// **R7 (P3b4c)**: los cuerpos que emite PROTON-X con el ABI de
    /// registros (el de la puerta de las apps) pasan la lista blanca; lo que
    /// toca memoria, constantes, atributos o registros del pegamento, NO.
    #[test]
    fn r7_el_cuerpo_de_una_app_no_toca_lo_ajeno() {
        extern crate std;
        use bmo_proton_x::dxil::{self, programa::compilar};
        use bmo_sm86::codifica::{self as c, Fuente};
        let raiz = "../../../shared/proton-x/prueba/";
        for f in ["cubo_vs.dxil", "cubo_ps.dxil", "sombras/f3ef42a0.cso", "sombras/4d67f5e4.cso"] {
            let d = std::fs::read(std::format!("{raiz}{f}")).unwrap();
            let e = bmo_proton_x_sm86::emitir_con(&compilar(&dxil::leer(&d).unwrap()).unwrap(), 64, bmo_proton_x_sm86::Abi::Registros).unwrap();
            assert_eq!(juzgar_cuerpo_de_app(&e.codigo, e.registros), Ok(()), "{f}");
        }
        let fin = c::exit(0);
        let bueno = [c::mov(0, Fuente::Imm(0x3F80_0000), 1), c::fadd(1, c::r(0), c::r(0), false, 1), fin];
        assert_eq!(juzgar_cuerpo_de_app(&bueno, 4), Ok(()));
        let r7 = |codigo: &[(u64, u64)], registros: u32| juzgar_cuerpo_de_app(codigo, registros).unwrap_err();
        // Un banco de constantes es memoria.
        let b = r7(&[c::mov(0, Fuente::C { banco: 0, desp: 0x10 }, 1), fin], 4);
        assert_eq!((b.regla, b.instruccion), (Regla::R7CuerpoAjeno, 0));
        // Un LDG (lo que lee el pegamento), y un STG: fuera de la lista.
        for op in [0x181u64, 0x186] {
            let w = (op | 1 << 9 | 7 << 12, 0);
            assert!(conoce(w.0, w.1));
            assert_eq!(r7(&[w, fin], 64).que, op as u32);
        }
        // Un registro del pegamento (el cuerpo dice 4: R4 no es suyo).
        assert_eq!(r7(&[c::mov(4, c::r(0), 1), fin], 4).que, 4);
        // Sin EXIT, o con uno a medias, o vacio.
        assert_eq!(r7(&[c::mov(0, c::r(1), 1)], 4).instruccion, 0);
        assert_eq!(r7(&[fin, c::mov(0, c::r(1), 1), fin], 4).instruccion, 0);
        assert_eq!(r7(&[], 4).regla, Regla::R7CuerpoAjeno);
        // Con predicado (P0): no.
        let (lo, hi) = c::mov(0, c::r(1), 1);
        assert_eq!(r7(&[(lo & !(0xF << 12), hi), fin], 4).instruccion, 0);
    }

    /// E5: la puerta del kernel acepta 128 instrucciones (un HUECO de
    /// VERRANO entero) y ni una mas; y el paquete con los dos huecos llenos
    /// se sostiene y cabe en la caja del escritorio.
    #[test]
    fn la_puerta_de_128_y_su_hueco() {
        extern crate std;
        use std::vec::Vec;
        let bytes = |n: usize| -> Vec<u8> {
            let mut b: Vec<u8> = tu::vertice().iter().flat_map(|w| w.to_le_bytes()).collect();
            b.resize(4 * SPH + 16 * n, 0);
            b
        };
        assert!(juzgar_programa(&bytes(MAX_INSTRUCCIONES), raster::REGISTROS).is_ok());
        let b = juzgar_programa(&bytes(MAX_INSTRUCCIONES + 1), raster::REGISTROS).unwrap_err();
        assert_eq!((b.regla, b.que), (Regla::R5CabeceraMiente, MAX_INSTRUCCIONES as u32 + 1));
        assert_eq!(MAX_INSTRUCCIONES, 128);
        assert_eq!(tu::HUECO, 128 + 2048);
        let ps: Vec<u8> = {
            let mut b: Vec<u8> = tu::pixel().iter().flat_map(|w| w.to_le_bytes()).collect();
            b.resize(tu::HUECO, 0);
            b
        };
        let vs = bytes(MAX_INSTRUCCIONES);
        // V0: los vertices que caben en los DATOS (64 KiB de 32 B), y uno
        // (tres) mas, no.
        let v0 = tu::DATOS_MAX / tu::BYTES_VERTICE / 3 * 3;
        let v = std::vec![tu::Vertice::default(); v0];
        let mut caja = std::vec![0u8; tu::MAX_PAQUETE];
        let n = tu::escribir_paquete(&mut caja, 1, &vs, &ps, &v).unwrap();
        let p = tu::leer(&caja[..n]).unwrap();
        assert_eq!((p.vs.len(), p.ps.len(), p.n), (tu::HUECO, tu::HUECO, v0));
        let mas = std::vec![tu::Vertice::default(); v0 + 3];
        assert_eq!(tu::escribir_paquete(&mut std::vec![0u8; tu::MAX_PAQUETE + 128], 1, &vs, &ps, &mas), None);
        // E5: los datos mas grandes que caben, y los dos huecos llenos.
        let datos = std::vec![7u8; tu::DATOS_MAX];
        let n = tu::escribir_paquete_datos(&mut caja, 1, &vs, &ps, 18, &datos).unwrap();
        assert_eq!(n, tu::MAX_PAQUETE);
        let p = tu::leer(&caja).unwrap();
        assert_eq!((p.vertices.len(), p.n), (tu::DATOS_MAX, 18));
        // Datos de mas, o que no son enteros de 16: no.
        let mut caja3 = std::vec![0u8; tu::MAX_PAQUETE + 16];
        assert_eq!(tu::escribir_paquete_datos(&mut caja3, 1, &vs, &ps, 18, &std::vec![0u8; tu::DATOS_MAX + 16]), None);
        assert_eq!(tu::escribir_paquete_datos(&mut caja3, 1, &vs, &ps, 18, &[0u8; 40]), None);
        // Uno mas, no: ni se escribe, ni se lee (la cabecera lo dice).
        let mut grande = vs.clone();
        grande.extend_from_slice(&[0; 16]);
        let mut caja2 = std::vec![0u8; tu::MAX_PAQUETE + 16];
        assert_eq!(tu::escribir_paquete(&mut caja2, 1, &grande, &ps, &v), None);
        caja2[12..16].copy_from_slice(&(tu::HUECO as u32 + 16).to_le_bytes());
        assert_eq!(tu::medida(&caja2), None);
    }

    /// J1 (a): TODO lo que ya corrio en el metal es PERFECTO Y PRECISO. Si no,
    /// el que esta mal es el juez.
    #[test]
    fn el_oro_pasa() {
        for p in ORO.iter() {
            let v = juzgar(p.codigo, &p.contexto());
            assert!(v.is_ok(), "{} ({}): {:?}", p.nombre, p.origen, v);
            assert!(v.unwrap().instrucciones > 0);
        }
    }

    fn verrano() -> [(u64, u64); tu::INSTR_VS] {
        tu::codigo_vs()
    }

    fn ctx_verrano(sph: &[u32; SPH]) -> Contexto<'_> {
        Contexto { registros: raster::REGISTROS, sph: Some(sph) }
    }

    fn regla(c: &[(u64, u64)], ctx: &Contexto) -> Regla {
        juzgar(c, ctx).expect_err("tenia que ser un bodrio").regla
    }

    /// Cambia los bits de control de una instruccion (desde el bit 105).
    fn control(i: (u64, u64), c: u64) -> (u64, u64) {
        (i.0, cu::con_control(i.1, c))
    }

    /// P3b4c.8 T1: el TEX es una desacoplada -- sus cuatro canales llegan
    /// con su barrera, y leerlos antes es R1. Y solo la forma que se sabe.
    #[test]
    fn el_tex_se_espera() {
        use crate::texturas::tex;
        use crate::tuberia::{carga, espera, iadd3_acarreo};
        let ctx = Contexto { registros: 64, sph: None };
        let bien = [tex(4, 0, 2, carga(0)), iadd3_acarreo(8, 0, 4, 7, espera(1)), cu::EXIT];
        // R4..R7 los escribe el TEX; R10 y R11, no.
        assert!(juzgar(&bien, &ctx).is_ok(), "{:?}", juzgar(&bien, &ctx));
        let mut mal = bien;
        mal[1] = iadd3_acarreo(8, 0, 10, 11, cu::ALU);
        assert!(juzgar(&mal, &ctx).is_ok(), "lo que no es del TEX no espera");
        mal[1] = iadd3_acarreo(8, 0, 6, 9, cu::ALU);
        assert_eq!(regla(&mal, &ctx), Regla::R1DatoAntesDeLlegar, "el canal B antes de llegar");
        // Otra forma (sin .LZ, mascara 0x7, 1D, desplazamientos): R0.
        for (bit, hi) in [(87, true), (72, true), (61, false), (76, true)] {
            let mut t = bien;
            if hi { t[0].1 ^= 1 << (bit - 64) } else { t[0].0 ^= 1 << bit }
            assert_eq!(regla(&t, &ctx), Regla::R0NoSe, "bit {bit}");
        }
        // Un par sin alinear, tampoco.
        assert_eq!(regla(&[tex(5, 0, 2, carga(0)), cu::EXIT], &ctx), Regla::R0NoSe);
    }

    /// P3b4c.8 T2: el TEX en el cuerpo de una app, solo con el asa del
    /// kernel, y sin pisarla.
    #[test]
    fn r7_el_tex_con_el_asa_del_kernel() {
        use crate::texturas::tex;
        use crate::tuberia::carga;
        // R0..R3 el color; las coordenadas en R4, R5; el asa en R6.
        let cuerpo = [tex(0, 4, 6, carga(0)), control(cu::EXIT, 1 | 1 << 4 | 7 << 5 | 7 << 8 | 1 << 11)];
        assert_eq!(juzgar_cuerpo_con_asas(&cuerpo, 8, 1 << 6), Ok(()));
        let r7 = |c: &[(u64, u64)], asas: u64| juzgar_cuerpo_con_asas(c, 8, asas).unwrap_err().regla;
        assert_eq!(r7(&cuerpo, 0), Regla::R7CuerpoAjeno, "sin asas del kernel, un TEX no");
        assert_eq!(r7(&cuerpo, 1 << 7), Regla::R7CuerpoAjeno, "el asa en otro registro");
        assert_eq!(r7(&[tex(4, 0, 6, carga(0)), cu::EXIT], 1 << 6), Regla::R7CuerpoAjeno, "los canales encima del asa");
        assert_eq!(juzgar_cuerpo_de_app(&cuerpo, 8).unwrap_err().regla, Regla::R7CuerpoAjeno, "la de siempre, sin texturas");
    }

    /// J1 (b): cada regla tiene su programa roto a proposito, y el juez lo
    /// rechaza con ESA regla y no con otra.
    #[test]
    fn cada_regla_tiene_su_bodrio() {
        let sph = tu::sph_vertice();
        let ctx = ctx_verrano(&sph);
        assert!(juzgar(&verrano(), &ctx).is_ok(), "el de partida pasa");

        // R0: un opcode que no conoce.
        let mut c = verrano();
        c[0] = (0x0000_0000_0000_71FF, c[0].1);
        assert_eq!(regla(&c, &ctx), Regla::R0NoSe);

        // R1: el AST de la posicion sin esperar la barrera de los LDG.
        let mut c = verrano();
        c[17] = control(c[17], 1 | 1 << 4 | 7 << 5 | 1 << 8);
        assert_eq!(regla(&c, &ctx), Regla::R1DatoAntesDeLlegar);

        // R1 tambien: un LDG que no enciende ninguna barrera.
        let mut c = verrano();
        c[9] = control(c[9], 2 | 1 << 4 | 7 << 5 | 7 << 8);
        assert_eq!(regla(&c, &ctx), Regla::R1DatoAntesDeLlegar);

        // R2: el IMAD.SHL con 1 ciclo y el IADD3 que lee su resultado detras
        // (Ampere pide 5 de un FMA a una ALU).
        let mut c = verrano();
        c[6] = control(c[6], 1 | 1 << 4 | 7 << 5 | 7 << 8 | 1 << 11);
        assert_eq!(regla(&c, &ctx), Regla::R2EsperaCorta);

        // R3: esperar la barrera 5, que nadie enciende.
        let mut c = verrano();
        c[2] = control(c[2], cu::ALU | 1 << 5 << 11);
        assert_eq!(regla(&c, &ctx), Regla::R3BarreraFantasma);

        // R4: escribir R4 mientras el AST de la posicion aun lo lee.
        let mut c = verrano();
        c[18] = cu::mov(4, 0);
        assert_eq!(regla(&c, &ctx), Regla::R4FuentePisada);

        // R5: la cabecera sin DoesLoadOrStore (la de T2a).
        let sin_bit = color3d::sph_vertice();
        assert_eq!(regla(&verrano(), &ctx_verrano(&sin_bit)), Regla::R5CabeceraMiente);
        // R5: un AST al generico 0 con una cabecera que no lo declara.
        let mut sin_generico = raster::sph_vertice();
        sin_generico[0] |= 1 << 26;
        assert_eq!(regla(&verrano(), &ctx_verrano(&sin_generico)), Regla::R5CabeceraMiente);
        // R5: R14 con solo 8 registros.
        assert_eq!(regla(&verrano(), &Contexto { registros: 8, sph: Some(&sph) }), Regla::R5CabeceraMiente);

        // R6: EXIT sin esperar la lectura de los AST.
        let mut c = verrano();
        c[19] = cu::EXIT;
        assert_eq!(regla(&c, &ctx), Regla::R6FinalSucio);
    }

    /// ** LO QUE SE LE ENVIA A LA 3060 (26-09, pedido del propietario: "fijate
    /// en lo que le envia para configurar eso antes"). El juez no se fia de un
    /// numero escrito a mano en el corpus: saca los registros de las ORDENES
    /// y de los QMD de verdad -- `SET_PIPELINE_REGISTER_COUNT` de cada hueco
    /// del pipeline y `REGISTER_COUNT_V` (bits 648..656) de cada QMD -- y juzga
    /// cada programa con ESE numero. Si alguien sube un programa sin subir lo
    /// que se envia (o al reves), esto dice TOMA TU BODRIO antes que la 3060.
    #[test]
    fn con_lo_que_se_le_envia() {
        use crate::raster::{set_pipeline_shader, PIXEL, VERTICE};
        use crate::sombreador::{leer_campo, QMD_PALABRAS};
        use crate::{blur, escena, fractal, giro, lienzo, pantalla, sombreador, triangulo, video};

        // Los registros que las ordenes dan al hueco `j` del pipeline.
        fn enviados(o: &[u32], j: u32) -> u32 {
            let quiero = set_pipeline_shader(j) + 0x0c;
            let mut i = 0;
            while i < o.len() {
                let (n, m) = ((o[i] >> 16 & 0x1FFF) as usize, (o[i] & 0xFFF) << 2);
                for k in 0..n {
                    if m + 4 * k as u32 == quiero && i + 1 + k < o.len() {
                        return o[i + 1 + k];
                    }
                }
                i += 1 + n;
            }
            panic!("las ordenes no dan registros al hueco {j}")
        }

        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = cu::ventana(&gop).unwrap();
        let (x5, verrano_o) = (cu::ordenes(&v, 1), tu::ordenes(&v, 6));
        let t1c = raster::ordenes();
        let (sv, sp) = (tu::sph_vertice(), tu::sph_pixel());
        let (rv, rp) = (raster::sph_vertice(), raster::sph_pixel());
        let (cv, cp) = (color3d::sph_vertice(), color3d::sph_pixel());
        let t = cu::Triangulo { clip: [[0; 4]; 3], color: [0; 4] };
        let graficos: [(&str, &[u32], u32, &[(u64, u64)], &[u32; SPH]); 8] = [
            ("T1c vertice", &t1c[..], VERTICE, &raster::CODIGO_VS, &rv),
            ("T1c pixel", &t1c[..], PIXEL, &raster::CODIGO_PS, &rp),
            ("T2a vertice", &t1c[..], VERTICE, &color3d::CODIGO_VS, &cv),
            ("T2a pixel", &t1c[..], PIXEL, &color3d::CODIGO_PS, &cp),
            ("X5 vertice", &x5.o[..x5.n], VERTICE, &cu::codigo_vs(&t), &rv),
            ("X5 pixel", &x5.o[..x5.n], PIXEL, &cu::codigo_ps(&t), &rp),
            ("VERRANO vertice", &verrano_o.o[..verrano_o.n], VERTICE, &tu::codigo_vs(), &sv),
            ("VERRANO pixel", &verrano_o.o[..verrano_o.n], PIXEL, &tu::codigo_ps(), &sp),
        ];
        for (nombre, o, j, codigo, sph) in graficos {
            let r = enviados(o, j);
            let v = juzgar(codigo, &Contexto { registros: r, sph: Some(sph) });
            assert!(v.is_ok(), "{nombre} con los {r} registros que se le envian: {}", v.unwrap_err());
        }

        let registros = |q: &[u32; QMD_PALABRAS]| leer_campo(q, 656, 648) as u32;
        let computo: [(&str, [u32; QMD_PALABRAS], &[(u64, u64)]); 9] = [
            ("sombreador", sombreador::qmd(), &sombreador::CODIGO),
            ("lienzo", lienzo::qmd(), &lienzo::CODIGO),
            ("blur", blur::qmd(), &blur::CODIGO),
            ("fractal", fractal::qmd(), &fractal::CODIGO),
            ("triangulo", triangulo::qmd(), &triangulo::CODIGO),
            ("escena", escena::qmd(), &escena::CODIGO),
            ("giro", giro::qmd(), &giro::CODIGO),
            ("pantalla", pantalla::qmd(&gop), &pantalla::CODIGO),
            ("video", video::qmd(&video::Formato { ancho: 640, alto: 360 }), &video::CODIGO),
        ];
        for (nombre, q, codigo) in computo {
            let r = registros(&q);
            assert!(r > 0, "{nombre}: el QMD no da registros");
            let v = juzgar(codigo, &Contexto { registros: r, sph: None });
            assert!(v.is_ok(), "{nombre} con los {r} registros de su QMD: {}", v.unwrap_err());
        }
    }

    /// J1 (c): el de vertice de VERRANO V0 tal como se colgo (con R14) y
    /// como quedo (con R1). Con la regla de los 2 registros del contador de
    /// programa, el juez caza el de V0 -- lo que la 3060 dijo con su Xid 13
    /// ("Out Of Range Register", metal 26-09 06:33) -- y aprueba el arreglado.
    /// Como lo dice: firmado, y el remate solo detras de un bodrio. En ASCII:
    /// la fuente del metal es de 8x16 y no tiene emojis.
    #[test]
    fn como_lo_dice() {
        extern crate std;
        let b = juez_verrano_bodrio();
        let t = std::format!("{b}");
        assert!(t.starts_with("[BMO-X Juez V3b]: TOMA TU BODRIO! R5"), "{t}");
        assert!(REMATE.is_ascii() && FIRMA.is_ascii() && t.is_ascii());
        let v = juzgar(&verrano(), &ctx_verrano(&tu::sph_vertice())).unwrap();
        assert!(std::format!("{v}").starts_with("[BMO-X Juez V3b]: PERFECTO Y PRECISO: "));
    }

    fn juez_verrano_bodrio() -> Bodrio {
        let mut b = [0u8; 4 * tu::PALABRAS_VS];
        let n = tu::bytes(&tu::vertice(), &mut b);
        juzgar_programa(&b[..n - 8], raster::REGISTROS).unwrap_err()
    }

    /// Los bytes del BSF dicen lo mismo que las instrucciones: el programa
    /// de VERRANO tal como viaja es PERFECTO Y PRECISO, y uno cortado es
    /// BODRIO.
    #[test]
    fn en_bytes_como_viaja() {
        let mut b = [0u8; 4 * tu::PALABRAS_VS];
        let n = tu::bytes(&tu::vertice(), &mut b);
        assert!(juzgar_programa(&b[..n], raster::REGISTROS).is_ok());
        let mut b = [0u8; 4 * tu::PALABRAS_PS];
        let n = tu::bytes(&tu::pixel(), &mut b);
        assert!(juzgar_programa(&b[..n], raster::REGISTROS).is_ok());
        assert_eq!(juzgar_programa(&b[..n - 8], raster::REGISTROS).unwrap_err().regla, Regla::R5CabeceraMiente);
        assert_eq!(juzgar_programa(&b[..4 * SPH], raster::REGISTROS).unwrap_err().regla, Regla::R5CabeceraMiente);
    }

    #[test]
    fn verrano_segun_el_juez() {
        let sph = tu::sph_vertice();
        let ctx = ctx_verrano(&sph);
        let mut v0 = verrano();
        v0[6] = tu::imad_shl(14, 0, tu::BYTES_VERTICE as u32, tu::espera(1 << 0));
        v0[7] = tu::iadd3_acarreo(2, 0, 2, 14, tu::espera(1 << 2));
        let b = juzgar(&v0, &ctx).expect_err("el de V0 es un bodrio");
        assert_eq!((b.regla, b.instruccion, b.que), (Regla::R5CabeceraMiente, 6, 14));
        for p in SOSPECHOSOS.iter() {
            assert!(juzgar(p.codigo, &p.contexto()).is_ok(), "{}: {:?}", p.nombre, juzgar(p.codigo, &p.contexto()));
        }
    }
}
