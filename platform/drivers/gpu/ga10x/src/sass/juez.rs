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
//!    R0  no se                        una instruccion que el juez no conoce:
//!                                     tambien es NO (un juez que aprueba lo que
//!                                     no entiende no es un juez)
//! ```
//!
//! # [!] Lo que v1 NO mira, dicho
//!
//! - Los PREDICADOS (el acarreo de `IADD3` que lee `IMAD.X`): sin reglas aun.
//! - Los registros UNIFORMES (`UMOV`, `S2UR`...): solo sus barreras.
//! - Los SALTOS hacia atras: el programa se lee en linea recta; lo que
//!   depende de la vuelta anterior de un bucle no se juzga (y R3 se calla si
//!   hay un salto hacia atras). Es conservador en la linea recta: lo que dice
//!   NO, es NO.

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

impl core::fmt::Display for Bodrio {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "TOMA TU BODRIO: {} en la instruccion {} ({}): {}", self.regla.nombre(), self.instruccion, self.que, self.detalle)
    }
}

impl core::fmt::Display for Veredicto {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PERFECTO Y PRECISO: {} instrucciones, {} lecturas y {} esperas comprobadas", self.instrucciones, self.lecturas, self.esperas)
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
    };
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
        // Sin registros: NOP, BSSY, BSYNC, EXIT, BRA; y los UNIFORMES.
        0x118 | 0x145 | 0x141 | 0x82 | 0x90 | 0x99 | 0x1C3 => i.clase = Clase::Nada,
        0x14D => {
            i.clase = Clase::Nada;
            // Sin predicado: PT (7) y no negado (bits 12..16).
            i.fin = lo >> 12 & 0xF == 7;
        }
        0x147 => {
            i.clase = Clase::Nada;
            // El desplazamiento, con signo, en 34..82: negativo = hacia atras.
            i.atras = r(81, 1) == 1;
        }
        _ => return None,
    }
    Some(i)
}

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
pub fn juzgar(codigo: &[(u64, u64)], ctx: &Contexto) -> Result<Veredicto, Bodrio> {
    let mut regs = [LIMPIO; 256];
    let mut encendidas = 0u32;
    let mut ciclo = 0u32;
    let mut v = Veredicto::default();
    // Un salto hacia atras ANTES del primer EXIT sin predicado: un bucle. El
    // `BRA .` de relleno que va detras del EXIT no cuenta (no se alcanza).
    let mut hay_bucle = false;
    for i in codigo.iter().filter_map(|&(lo, hi)| decodificar(lo, hi)) {
        hay_bucle |= i.atras;
        if i.fin {
            break;
        }
    }
    for (k, &(lo, hi)) in codigo.iter().enumerate() {
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
        if i.bar_escritura < 6 {
            encendidas |= 1 << i.bar_escritura;
        }
        if i.bar_lectura < 6 {
            encendidas |= 1 << i.bar_lectura;
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

/// Cuantas instrucciones caben en un programa que se juzga en bytes: las
/// que caben en el hueco mas grande de la tuberia.
pub const MAX_INSTRUCCIONES: usize = 256;

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
    let mut codigo = [(0u64, 0u64); MAX_INSTRUCCIONES];
    for (k, c) in codigo[..n].iter_mut().enumerate() {
        let b = cabecera + 16 * k;
        *c = (u64le(b), u64le(b + 8));
    }
    juzgar(&codigo[..n], &Contexto { registros, sph: Some(&sph) })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::sass::corpus::{ORO, SOSPECHOSOS};
    use crate::{color3d, cubo as cu, raster, tuberia as tu};

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
