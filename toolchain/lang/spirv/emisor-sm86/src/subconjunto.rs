//! **E1: EL SUBCONJUNTO DE SPIR-V PARA LA 3060.** Un modulo leido y la etapa
//! que se espera de el: o cabe en lo que el emisor de SM86 promete traducir,
//! o el primer motivo por el que no, con su nombre y su palabra.
//!
//! Dos jueces, en fila, y el segundo no repite al primero:
//!
//! ```text
//!    1. el juez NEUTRO (S2), con la etapa pedida (`validate_stage`)
//!         tipos, valores, bloques, familias, clases de almacenamiento, y
//!         la etapa: un vertice entregado como pixel es `WrongStage`
//!         -> su NO sale tal cual, envuelto: `Reason::Spirv`
//!    2. el ABI `SM86_V1` (lo que el BSF promete de su codigo)
//!         buffers      la ranura k es `DescriptorSet 0, Binding k`, k < 16
//!         vertice      entra el numero de vertice (a[0x2fc]) y nada mas;
//!                      sale la posicion (a[0x70]) y el generico 0 (a[0x80])
//!         pixel        entra el generico 0 (IPA); sale el color 0 (R0..R3)
//!         computo      las invocaciones por S2R: `WorkgroupSize`, `WorkgroupId`,
//!                      `LocalInvocationId`, `GlobalInvocationId`,
//!                      `LocalInvocationIndex`; no `NumWorkgroups`
//!         atributos    flotantes de 32 bits (escalar o vector): AST e IPA
//!         aritmetica   entera y de coma flotante del nucleo; NO las
//!                      trascendentes (abajo)
//! ```
//!
//! **Las trascendentes, fuera, y por que.** `Sin`, `Cos`, `Exp`, `Log`, `Pow`
//! los define `bmo_spirv_front::math` a un ULP en doble (S3b): el oraculo y el
//! emisor de x86-64 dan los MISMOS bits. La 3060 las tiene en `MUFU`, que es
//! una aproximacion con su propio error. Aceptarlas seria tener dos
//! respuestas para el mismo sombreador; el dia que el emisor las haga con la
//! tabla de `math`, entran.
//!
//! **La FMA, solo si se pide.** SPIR-V deja contraer `a * b + c` en una FMA
//! (salvo `NoContraction`); el oraculo NO lo hace -- redondea dos veces --, y
//! el emisor de SM86 tampoco: una `FFMA` solo sale de un `GLSL.std.450 Fma`.
//! Por eso esto no RECHAZA nada por la FMA (en SPIR-V no se puede escribir una
//! FMA que nadie pidio), sino que la CUENTA: [`Fit::fused`] es el numero de
//! `FFMA` que el emisor (E3) puede poner, y ni una mas.
//!
//! [!] LO QUE NO COMPRUEBA, dicho:
//!
//! - que el emisor cumpla. Esto juzga la ENTRADA; que lo emitido haga lo
//!   mismo es de E3 (el oraculo contra un simulador) y de J1 (el juez del SASS);
//! - `Flat` contra `ScreenLinear`: el ABI interpola el generico 0 aunque la
//!   fuente diga `flat` (ver `ga10x/src/trabajos/tuberia.rs`); con los tres
//!   vertices del mismo color da lo mismo, y eso lo juzga el cubo, no esto;
//! - el computo en el metal con `SM86_V1`: el ABI se escribio para el cubo
//!   (vertice y pixel). Que los de computo quepan aqui dice que el emisor los
//!   podra traducir, no que ya haya un despacho que los corra.
//!
//! [consumo]  NADA   una pasada por peticion; sin estado entre llamadas

use core::fmt;

use bmo_spirv_front::table::{glsl, op};
use bmo_spirv_front::{glsl_info, interface, validate_stage, Error, GlslGroup, Instruction, Module, Stage, Verdict};

/// Las ranuras de la tabla de buffers de `SM86_V1`: la ranura `k` es la
/// direccion (u64) en `TABLA + 8k`, y el BSF guarda hasta 16 por objetivo.
pub const SLOTS: u32 = bmo_spirv_front::MAX_BINDINGS as u32;

const CLASE_INPUT: u32 = 1;
const CLASE_OUTPUT: u32 = 3;

const DEC_BUILTIN: u32 = 11;
const DEC_LOCATION: u32 = 30;

const BUILTIN_POSITION: u32 = 0;
const BUILTIN_VERTEX_INDEX: u32 = 42;
/// Lo que una invocacion de computo lee de si misma con `S2R` (o, la
/// medida del grupo, como constante): `WorkgroupSize`, `WorkgroupId`,
/// `LocalInvocationId`, `GlobalInvocationId`, `LocalInvocationIndex`.
/// `NumWorkgroups` (24) NO: en NVIDIA sale de un banco de constantes que
/// `SM86_V1` no define.
const BUILTINS_DE_COMPUTO: [u32; 5] = [25, 26, 27, 28, 29];

/// Por que un modulo no cabe en el subconjunto de la 3060.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// El juez neutro (S2, con la etapa pedida) dijo que no: su motivo tal
    /// cual. Ahi caen la etapa equivocada (`WrongStage`), las instrucciones
    /// fuera del nucleo (`UnsupportedFamily`, con su nombre) y las clases de
    /// almacenamiento que no hay (`UnsupportedStorageClass`).
    Spirv(bmo_spirv_front::Reason),
    /// Mas de un punto de entrada: el BSF guarda UN programa por modulo.
    SeveralEntryPoints { count: usize },
    /// Un buffer que no cae en una ranura de `SM86_V1`: la tabla es UNA, sin
    /// conjuntos, y de 16.
    BufferOffSlot { set: u32, binding: u32 },
    /// Un `BuiltIn` que la etapa tiene, pero que `SM86_V1` no lleva a ningun
    /// sitio (en la variable, o el miembro de `gl_PerVertex` que se toca).
    BuiltInOffAbi { builtin: u32 },
    /// Una entrada de VERTICE por `Location`: `SM86_V1` no trae atributos de
    /// vertice; los datos van en un buffer.
    VertexInputByLocation { location: u32 },
    /// Una `Location` que no es la 0: `SM86_V1` tiene el generico 0 (a[0x80])
    /// y el color 0 (R0..R3), y nada mas.
    LocationOffAbi { location: u32 },
    /// Un atributo que no es un flotante de 32 bits (escalar o vector).
    AttributeNotFloat { id: u32 },
    /// Una trascendente de `GLSL.std.450`: la 3060 la da con `MUFU`, que no
    /// es la de `math`.
    Transcendental { number: u32 },
}

impl Reason {
    /// El motivo en palabras, sin los numeros.
    pub fn name(&self) -> &'static str {
        match self {
            Reason::Spirv(r) => r.name(),
            Reason::SeveralEntryPoints { .. } => "mas de un punto de entrada: el BSF guarda un programa por modulo",
            Reason::BufferOffSlot { .. } => "buffer fuera de las ranuras (DescriptorSet 0, Binding 0 a 15)",
            Reason::BuiltInOffAbi { .. } => "BuiltIn que el ABI no lleva a ningun sitio",
            Reason::VertexInputByLocation { .. } => "entrada de vertice por Location: el ABI no trae atributos de vertice, los datos van en un buffer",
            Reason::LocationOffAbi { .. } => "Location que no es la 0: el ABI tiene el generico 0 y el color 0",
            Reason::AttributeNotFloat { .. } => "atributo que no es un flotante de 32 bits (escalar o vector)",
            Reason::Transcendental { .. } => "trascendente: la 3060 la aproxima con MUFU y el oraculo la define a un ULP",
        }
    }
}

/// Un NO: el motivo y la palabra del fichero donde se vio.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Refusal {
    pub reason: Reason,
    /// Desplazamiento en PALABRAS desde el principio del fichero.
    pub word: usize,
}

impl Refusal {
    fn en(reason: Reason, word: usize) -> Refusal {
        Refusal { reason, word }
    }
}

impl From<Error> for Refusal {
    fn from(e: Error) -> Refusal {
        Refusal { reason: Reason::Spirv(e.reason), word: e.word }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Reason::Spirv(reason) = self.reason {
            return write!(f, "{}", Error { reason, word: self.word });
        }
        write!(f, "palabra {}: SM86_V1: {}", self.word, self.reason.name())?;
        match self.reason {
            Reason::SeveralEntryPoints { count } => write!(f, " ({} puntos de entrada)", count),
            Reason::BufferOffSlot { set, binding } => write!(f, " (set {}, binding {})", set, binding),
            Reason::BuiltInOffAbi { builtin } => write!(f, " (BuiltIn {})", builtin),
            Reason::VertexInputByLocation { location } | Reason::LocationOffAbi { location } => {
                write!(f, " (Location {})", location)
            }
            Reason::AttributeNotFloat { id } => write!(f, " (id {})", id),
            Reason::Transcendental { number } => match glsl_info(number) {
                Some(g) => write!(f, " ({})", g.name),
                None => write!(f, " (numero {})", number),
            },
            Reason::Spirv(_) => Ok(()),
        }
    }
}

/// Lo que el subconjunto dice de un modulo que cabe.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fit {
    pub stage: Stage,
    /// Lo que conto el juez neutro.
    pub verdict: Verdict,
    /// Las ranuras que usa: el bit `k` es `Binding k` del set 0.
    pub slots: u32,
    /// Los genericos que escribe (vertice) o lee (pixel), o el color que deja
    /// (pixel): el bit `n` es `Location n`. Con `SM86_V1`, a lo sumo el 0.
    pub attributes: u32,
    /// Cuantas `GLSL.std.450 Fma` pide el SPIR-V: las UNICAS `FFMA` que el
    /// emisor puede poner.
    pub fused: usize,
}

/// **Juzga un modulo leido como la etapa `stage`, para la 3060.** Primero el
/// juez neutro con esa etapa; despues el ABI `SM86_V1`. El primer NO, con su
/// palabra; o lo que se conto.
pub fn check(m: &Module, stage: Stage) -> Result<Fit, Refusal> {
    let verdict = validate_stage(m, stage)?;
    let entradas = m.entry_points().len();
    if entradas != 1 {
        let segunda = m.instructions().filter(|i| i.opcode == op::OpEntryPoint).nth(1).map(|i| i.offset);
        return Err(Refusal::en(Reason::SeveralEntryPoints { count: entradas }, segunda.unwrap_or(5)));
    }
    let mut fit = Fit { stage, verdict, slots: 0, attributes: 0, fused: 0 };

    // -- Los buffers, cada uno en su ranura ----------------------------------
    let it = interface(m)?;
    for b in it.bindings() {
        if b.set != 0 || b.binding >= SLOTS {
            let donde = m.def(b.variable).map(|d| d.offset).unwrap_or(5);
            return Err(Refusal::en(Reason::BufferOffSlot { set: b.set, binding: b.binding }, donde));
        }
        fit.slots |= 1 << b.binding;
    }

    // -- Las entradas y las salidas ------------------------------------------
    for ins in m.instructions().take_while(|i| i.opcode != op::OpFunction) {
        if ins.opcode != op::OpVariable || !matches!(ins.op(3), CLASE_INPUT | CLASE_OUTPUT) {
            continue;
        }
        variable(m, stage, &ins, &mut fit).map_err(|r| Refusal::en(r, ins.offset))?;
    }

    // -- Los cuerpos -----------------------------------------------------------
    let glsl_id = m.glsl450();
    for ins in m.instructions().skip_while(|i| i.opcode != op::OpFunction) {
        cuerpo(m, &ins, glsl_id, &mut fit).map_err(|r| Refusal::en(r, ins.offset))?;
    }
    Ok(fit)
}

/// Una variable `Input` u `Output` global, contra el ABI de su etapa.
fn variable(m: &Module, stage: Stage, ins: &Instruction, fit: &mut Fit) -> Result<(), Reason> {
    let (id, class) = (ins.op(2), ins.op(3));
    if let Some(b) = decoracion(m, id, DEC_BUILTIN) {
        let vale = match (stage, class) {
            (Stage::Vertex, CLASE_INPUT) => b == BUILTIN_VERTEX_INDEX,
            (Stage::Vertex, _) => b == BUILTIN_POSITION,
            (Stage::GLCompute, CLASE_INPUT) => BUILTINS_DE_COMPUTO.contains(&b),
            _ => false,
        };
        return if vale { Ok(()) } else { Err(Reason::BuiltInOffAbi { builtin: b }) };
    }
    let Some(location) = decoracion(m, id, DEC_LOCATION) else {
        // Sin `BuiltIn` ni `Location`, el juez neutro solo deja pasar un
        // bloque de `BuiltIn` por miembro (`gl_PerVertex`): se juzga el
        // miembro que se TOCA, en el cuerpo.
        return Ok(());
    };
    if stage == Stage::Vertex && class == CLASE_INPUT {
        return Err(Reason::VertexInputByLocation { location });
    }
    if location != 0 {
        return Err(Reason::LocationOffAbi { location });
    }
    let apuntado = m.def(ins.op(1)).map(|p| p.op(3)).unwrap_or(0);
    if !es_flotante(m, apuntado) {
        return Err(Reason::AttributeNotFloat { id });
    }
    fit.attributes |= 1 << location;
    Ok(())
}

/// Una instruccion de cuerpo: las trascendentes, las FMA pedidas y los
/// miembros de `gl_PerVertex` que se tocan.
fn cuerpo(m: &Module, ins: &Instruction, glsl_id: Option<u32>, fit: &mut Fit) -> Result<(), Reason> {
    match ins.opcode {
        op::OpExtInst if Some(ins.op(3)) == glsl_id => {
            let number = ins.op(4);
            if glsl_info(number).map(|g| g.group) == Some(GlslGroup::Transcendental) {
                return Err(Reason::Transcendental { number });
            }
            if number == glsl::Fma {
                fit.fused += 1;
            }
            Ok(())
        }
        op::OpAccessChain | op::OpInBoundsAccessChain => match bloque(m, ins.op(3)) {
            Some(s) if ins.words > 4 => {
                let k = m.def(ins.op(4)).filter(|c| c.opcode == op::OpConstant).map(|c| c.op(3));
                let b = k.and_then(|k| builtin_de_miembro(m, s, k));
                match b {
                    Some(BUILTIN_POSITION) => Ok(()),
                    Some(b) => Err(Reason::BuiltInOffAbi { builtin: b }),
                    None => Err(Reason::Spirv(bmo_spirv_front::Reason::NoLocation { id: ins.op(3) })),
                }
            }
            _ => Ok(()),
        },
        // El bloque ENTERO de una vez: todos sus miembros, y el ABI solo lleva
        // la posicion.
        op::OpLoad | op::OpStore | op::OpCopyMemory => {
            let punteros: [u32; 2] = match ins.opcode {
                op::OpLoad => [ins.op(3), 0],
                op::OpStore => [ins.op(1), 0],
                _ => [ins.op(1), ins.op(2)],
            };
            for p in punteros {
                if let Some(s) = bloque(m, p) {
                    let d = m.def(s).map(|d| d.words as u32).unwrap_or(2);
                    for k in 0..d.saturating_sub(2) {
                        match builtin_de_miembro(m, s, k) {
                            Some(BUILTIN_POSITION) => {}
                            Some(b) => return Err(Reason::BuiltInOffAbi { builtin: b }),
                            None => return Err(Reason::Spirv(bmo_spirv_front::Reason::NoLocation { id: p })),
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Si `ptr` es una variable `Input`/`Output` sin `BuiltIn` ni `Location`
/// -- o sea, un bloque de `BuiltIn` por miembro --, el id de su struct.
fn bloque(m: &Module, ptr: u32) -> Option<u32> {
    let v = m.def(ptr).filter(|d| d.opcode == op::OpVariable)?;
    if !matches!(v.op(3), CLASE_INPUT | CLASE_OUTPUT) {
        return None;
    }
    if decoracion(m, ptr, DEC_BUILTIN).is_some() || decoracion(m, ptr, DEC_LOCATION).is_some() {
        return None;
    }
    let s = m.def(v.op(1))?.op(3);
    m.def(s).filter(|d| d.opcode == op::OpTypeStruct).map(|_| s)
}

/// Un flotante (de 32 bits: el juez neutro no deja otros) o un vector de ellos.
fn es_flotante(m: &Module, t: u32) -> bool {
    match m.def(t) {
        Some(d) if d.opcode == op::OpTypeFloat => true,
        Some(d) if d.opcode == op::OpTypeVector => m.def(d.op(2)).map(|c| c.opcode) == Some(op::OpTypeFloat),
        _ => false,
    }
}

fn decoracion(m: &Module, id: u32, dec: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == dec)
        .map(|i| if i.words > 3 { i.op(3) } else { 0 })
}

fn builtin_de_miembro(m: &Module, s: u32, k: u32) -> Option<u32> {
    m.instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .find(|i| i.opcode == op::OpMemberDecorate && i.op(1) == s && i.op(2) == k && i.op(3) == DEC_BUILTIN)
        .map(|i| i.op(4))
}
