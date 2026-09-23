//! **EL JUEZ (S2): un modulo leido cabe en el subconjunto, o por que no.**
//!
//! El lector (S1) garantiza la FORMA. El juez mira el SIGNIFICADO, en una
//! pasada y sin pedir memoria: los tipos se preguntan a la tabla de ids que ya
//! lleno el lector (`Module::def`), asi que no hace falta ninguna tabla nueva.
//!
//! Lo que comprueba, en el orden en que aparece en el fichero:
//!
//! - el modulo: solo `Shader`, `Logical` + `GLSL450`, solo `GLSL.std.450`,
//!   etapa `GLCompute` con `LocalSize`, y el punto de entrada es `void main()`;
//! - cada instruccion es de la familia `Core` (el resto se niega NOMBRANDO la
//!   familia: imagen, atomico, barrera...);
//! - los tipos: enteros y flotantes de 32 bits, vectores de 2 a 4, clases de
//!   almacenamiento del subconjunto, `BuiltIn` de computo, buffers con
//!   `Binding` y `DescriptorSet`;
//! - los valores: definidos ANTES de usarse y en la misma funcion (salvo donde
//!   SPIR-V deja ir hacia delante: `OpPhi`, los saltos, `OpFunctionCall`);
//! - los tipos de cada instruccion cuadran;
//! - los bloques: empiezan por `OpLabel`, terminan en un salto o un retorno,
//!   `OpPhi` al principio, `OpVariable` al principio del primero, la fusion
//!   justo antes de su salto.
//!
//! [!] LO QUE NO COMPRUEBA, dicho para que nadie lo de por hecho:
//!
//! - **dominancia.** "Definido antes" es por ORDEN EN EL FICHERO, que SPIR-V
//!   exige pero que no basta: un valor de una rama del `if` usado despues del
//!   `if` pasa. El oraculo (S3) lo caza en ejecucion; el juez completo pide el
//!   arbol de dominadores, y eso es memoria que hoy no se pide.
//! - **la estructura entera.** Un salto condicional sin fusion propia se acepta
//!   si uno de sus destinos es la salida o la continuacion de ALGUN bucle de la
//!   funcion (un `break`/`continue`), no necesariamente del que lo encierra.
//!
//! [consumo]  NADA   una pasada por peticion; sin estado entre llamadas

use crate::table::op;
use crate::{op_info, glsl_info, Error, Family, GlslGroup, Instruction, Module, Reason, Section};

const CAP_SHADER: u32 = 1;
const GL_COMPUTE: u32 = 5;
const MODO_LOCAL_SIZE: u32 = 17;
const DIR_LOGICAL: u32 = 0;
const MEM_GLSL450: u32 = 1;

const CLASE_INPUT: u32 = 1;
const CLASE_UNIFORM: u32 = 2;
const CLASE_PRIVATE: u32 = 6;
const CLASE_FUNCTION: u32 = 7;
const CLASE_STORAGE_BUFFER: u32 = 12;

const DEC_BUILTIN: u32 = 11;
const DEC_BINDING: u32 = 33;
const DEC_DESCRIPTOR_SET: u32 = 34;

/// `NumWorkgroups`, `WorkgroupSize`, `WorkgroupId`, `LocalInvocationId`,
/// `GlobalInvocationId`, `LocalInvocationIndex`: lo que una invocacion de
/// computo puede preguntar de si misma.
const BUILTINS_DE_COMPUTO: [u32; 6] = [24, 25, 26, 27, 28, 29];

/// Las extensiones que el subconjunto acepta. La unica: la que deja usar la
/// clase `StorageBuffer` en SPIR-V 1.0-1.2 (la emite Naga).
const EXTENSIONES: [&[u8]; 1] = [b"SPV_KHR_storage_buffer_storage_class"];

/// Lo que el juez dice de un modulo que cabe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub entry_points: usize,
    pub functions: usize,
    pub blocks: usize,
    pub instruction_count: usize,
}

/// Cuantas instrucciones hay de cada familia, en el orden de `Family::ALL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Census {
    pub per_family: [usize; 9],
}

impl Census {
    /// Cuantas de fuera del nucleo.
    pub fn outside(&self) -> usize {
        self.per_family[1..].iter().sum()
    }
}

/// **Cuenta de que familias es un modulo**, sin parar en el primer NO.
pub fn census(m: &Module) -> Census {
    let mut c = Census { per_family: [0; 9] };
    for ins in m.instructions() {
        if let Some(f) = op_info(ins.opcode) {
            c.per_family[f.family.index()] += 1;
        }
    }
    c
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Escalar {
    Bool,
    Int,
    Float,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fase {
    /// Fuera de toda funcion.
    Fuera,
    /// Tras `OpFunction`, leyendo parametros.
    Parametros,
    /// Tras un salto o retorno: lo siguiente es `OpLabel` o `OpFunctionEnd`.
    EntreBloques,
    /// Justo tras `OpLabel`: aun valen `OpPhi` (y `OpVariable` en el primero).
    Cabeza,
    Body,
}

#[derive(Clone, Copy)]
struct Function {
    ini: usize,
    fin: usize,
    ret: u32,
    tipo_fn: u32,
    params: u32,
    vistos: u32,
    blocks: usize,
}

struct Juez<'m, 'a, 'b> {
    m: &'m Module<'a, 'b>,
    glsl_id: Option<u32>,
    primera_funcion: usize,
    /// La palabra de la instruccion que se esta juzgando.
    cur: usize,
    f: Option<Function>,
    fase: Fase,
    merge: Option<u16>,
    blocks: usize,
}

/// **Juzga un modulo leido.** El primer motivo por el que no cabe, con la
/// palabra donde se vio; o lo que se conto de el.
pub fn validate(m: &Module) -> Result<Verdict, Error> {
    let primera_funcion = m
        .instructions()
        .find(|i| i.opcode == op::OpFunction)
        .map(|i| i.offset)
        .unwrap_or(usize::MAX);
    let mut j = Juez {
        m,
        glsl_id: m.glsl450(),
        primera_funcion,
        cur: 0,
        f: None,
        fase: Fase::Fuera,
        merge: None,
        blocks: 0,
    };
    for ins in m.instructions() {
        j.cur = ins.offset;
        j.una(&ins).map_err(|reason| Error { reason, word: ins.offset })?;
    }
    if m.entry_points().is_empty() {
        return Err(Error { reason: Reason::NoEntryPoint, word: 5 });
    }
    for e in m.entry_points() {
        let donde = m
            .instructions()
            .find(|i| i.opcode == op::OpEntryPoint && i.op(2) == e.id)
            .map(|i| i.offset)
            .unwrap_or(5);
        if e.local_size.is_none() {
            return Err(Error { reason: Reason::NoLocalSize, word: donde });
        }
        if !j.es_void_main(e.id) {
            return Err(Error { reason: Reason::EntryPointNotVoidMain, word: donde });
        }
    }
    Ok(Verdict {
        entry_points: m.entry_points().len(),
        functions: m.functions,
        blocks: j.blocks,
        instruction_count: m.instruction_count,
    })
}

fn si(cond: bool, reason: Reason) -> Result<(), Reason> {
    if cond {
        Ok(())
    } else {
        Err(reason)
    }
}

impl<'m, 'a, 'b> Juez<'m, 'a, 'b> {
    // ---- preguntas sobre ids -----------------------------------------------

    fn def(&self, id: u32) -> Result<Instruction<'a>, Reason> {
        self.m.def(id).ok_or(Reason::Undefined { id })
    }

    /// `id` es un TIPO definido antes de aqui.
    fn tipo(&self, id: u32) -> Result<Instruction<'a>, Reason> {
        let d = self.def(id)?;
        let es = (op::OpTypeVoid..=op::OpTypeFunction).contains(&d.opcode);
        si(es && d.offset < self.cur, Reason::NotAType { id })?;
        Ok(d)
    }

    /// Un tipo que puede guardar un dato: ni `void` ni una funcion.
    fn tipo_dato(&self, id: u32) -> Result<(), Reason> {
        let d = self.tipo(id)?;
        si(d.opcode != op::OpTypeVoid && d.opcode != op::OpTypeFunction, Reason::NotAType { id })
    }

    /// `(escalar, componentes)` de un escalar o un vector.
    fn num(&self, t: u32) -> Option<(Escalar, u32)> {
        let d = self.m.def(t)?;
        match d.opcode {
            op::OpTypeBool => Some((Escalar::Bool, 1)),
            op::OpTypeInt => Some((Escalar::Int, 1)),
            op::OpTypeFloat => Some((Escalar::Float, 1)),
            op::OpTypeVector => {
                let (e, _) = self.num(d.op(2))?;
                Some((e, d.op(3)))
            }
            _ => None,
        }
    }

    /// El tipo de componente de un vector, o el propio escalar.
    fn componente(&self, t: u32) -> u32 {
        match self.m.def(t) {
            Some(d) if d.opcode == op::OpTypeVector => d.op(2),
            _ => t,
        }
    }

    /// `(class, apuntado)` de un tipo puntero.
    fn puntero(&self, t: u32) -> Option<(u32, u32)> {
        let d = self.m.def(t)?;
        if d.opcode == op::OpTypePointer {
            Some((d.op(2), d.op(3)))
        } else {
            None
        }
    }

    /// El tipo de un VALOR definido antes, en esta funcion o global.
    fn valor(&self, id: u32) -> Result<u32, Reason> {
        let d = self.def(id)?;
        let f = op_info(d.opcode).ok_or(Reason::NotAValue { id })?;
        si(f.has_result_type && d.opcode != op::OpFunction, Reason::NotAValue { id })?;
        si(d.offset < self.cur, Reason::UsedBeforeDefined { id })?;
        if d.offset >= self.primera_funcion {
            let ini = self.f.map(|f| f.ini).unwrap_or(usize::MAX);
            si(d.offset > ini, Reason::UsedBeforeDefined { id })?;
        }
        Ok(d.op(1))
    }

    /// Como `valor`, pero puede estar mas adelante en la misma funcion (`OpPhi`).
    fn valor_adelante(&self, id: u32) -> Result<u32, Reason> {
        let d = self.def(id)?;
        let f = op_info(d.opcode).ok_or(Reason::NotAValue { id })?;
        si(f.has_result_type && d.opcode != op::OpFunction, Reason::NotAValue { id })?;
        if d.offset >= self.primera_funcion {
            let (ini, fin) = self.f.map(|f| (f.ini, f.fin)).unwrap_or((0, 0));
            si(d.offset > ini && d.offset < fin, Reason::UsedBeforeDefined { id })?;
        }
        Ok(d.op(1))
    }

    /// Una constante definida antes; devuelve su tipo.
    fn constante(&self, id: u32) -> Result<u32, Reason> {
        let d = self.def(id)?;
        let es = matches!(
            d.opcode,
            op::OpConstantTrue | op::OpConstantFalse | op::OpConstant | op::OpConstantComposite | op::OpConstantNull
        );
        si(es && d.offset < self.cur, Reason::NotAConstant { id })?;
        Ok(d.op(1))
    }

    /// El valor de una constante entera escalar.
    fn constante_entera(&self, id: u32) -> Result<u32, Reason> {
        let t = self.constante(id)?;
        let d = self.def(id)?;
        si(d.opcode == op::OpConstant && self.num(t) == Some((Escalar::Int, 1)), Reason::NotAConstant { id })?;
        Ok(d.op(3))
    }

    /// Una etiqueta de la funcion actual (puede estar mas adelante).
    fn etiqueta(&self, id: u32) -> Result<(), Reason> {
        let d = self.def(id)?;
        let (ini, fin) = self.f.map(|f| (f.ini, f.fin)).unwrap_or((0, 0));
        si(d.opcode == op::OpLabel && d.offset > ini && d.offset < fin, Reason::NotALabel { id })
    }

    /// El valor de una decoracion `dec` sobre `id`, si la tiene.
    fn decoracion(&self, id: u32, dec: u32) -> Option<u32> {
        for i in self.m.instructions() {
            if let Some(f) = op_info(i.opcode) {
                if f.section > Section::Annotation {
                    break;
                }
            }
            if i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == dec {
                return Some(i.op(3));
            }
        }
        None
    }

    /// Paso de un compuesto por un indice literal.
    fn paso(&self, t: u32, idx: u32, opcode: u16) -> Result<u32, Reason> {
        let outside = Reason::IndexOutOfRange { opcode };
        let d = self.m.def(t).ok_or(outside)?;
        match d.opcode {
            op::OpTypeVector if idx < d.op(3) => Ok(d.op(2)),
            op::OpTypeArray if idx < self.constante_entera(d.op(3))? => Ok(d.op(2)),
            op::OpTypeStruct if (idx as usize) + 2 < d.words as usize => Ok(d.op(2 + idx as usize)),
            _ => Err(outside),
        }
    }

    fn es_void_main(&self, id: u32) -> bool {
        let Some(f) = self.m.def(id) else { return false };
        let Some(tf) = self.m.def(f.op(4)) else { return false };
        let void = self.m.def(f.op(1)).map(|d| d.opcode) == Some(op::OpTypeVoid);
        f.opcode == op::OpFunction && void && tf.words == 3
    }

    // ---- una instruccion ---------------------------------------------------

    fn una(&mut self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        let f = op_info(opcode).ok_or(Reason::UnknownOpcode { opcode })?;
        if f.family != Family::Core {
            return Err(Reason::UnsupportedFamily { family: f.family, opcode });
        }
        match f.section {
            Section::Capability => {
                si(ins.op(1) == CAP_SHADER, Reason::UnsupportedCapability { capability: ins.op(1) })
            }
            Section::Extension => {
                let e = ins.string(1).unwrap_or(&[]);
                si(EXTENSIONES.contains(&e), Reason::UnsupportedExtension)
            }
            Section::Import => {
                si(ins.string(2) == Some(&b"GLSL.std.450"[..]), Reason::UnsupportedImport)
            }
            Section::MemoryModel => si(
                ins.op(1) == DIR_LOGICAL && ins.op(2) == MEM_GLSL450,
                Reason::UnsupportedMemoryModel { addressing: ins.op(1), memory: ins.op(2) },
            ),
            Section::EntryPoint => si(ins.op(1) == GL_COMPUTE, Reason::UnsupportedStage { model: ins.op(1) }),
            Section::ExecutionMode => si(
                opcode == op::OpExecutionMode && ins.op(2) == MODO_LOCAL_SIZE,
                Reason::UnsupportedMode { mode: ins.op(2) },
            ),
            Section::Source | Section::Name | Section::ModuleProcessed => Ok(()),
            Section::Annotation => match opcode {
                op::OpDecorationGroup | op::OpGroupDecorate | op::OpGroupMemberDecorate => {
                    Err(Reason::UnsupportedInstruction {
                        opcode,
                        why: "grupos de decoraciones: obsoletos desde SPIR-V 1.5",
                    })
                }
                _ => Ok(()),
            },
            Section::Type => self.tipo_o_constante(ins),
            Section::Flexible => self.flexible(ins),
            Section::Function => self.abrir(ins),
            Section::FunctionEnd => self.cerrar(),
            Section::Body => self.cuerpo(ins),
        }
    }

    fn class(&self, class: u32) -> Result<(), Reason> {
        si(
            matches!(class, CLASE_INPUT | CLASE_UNIFORM | CLASE_PRIVATE | CLASE_FUNCTION | CLASE_STORAGE_BUFFER),
            Reason::UnsupportedStorageClass { class },
        )
    }

    fn tipo_o_constante(&self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        let no = Reason::TypeMismatch { opcode };
        match opcode {
            op::OpTypeVoid | op::OpTypeBool => Ok(()),
            op::OpTypeInt => {
                si(ins.op(2) == 32, Reason::UnsupportedType { why: "entero que no es de 32 bits" })?;
                si(ins.op(3) <= 1, no)
            }
            op::OpTypeFloat => si(ins.op(2) == 32, Reason::UnsupportedType { why: "flotante que no es de 32 bits" }),
            op::OpTypeVector => {
                self.tipo(ins.op(2))?;
                si(self.num(ins.op(2)).map(|x| x.1) == Some(1), no)?;
                si((2..=4).contains(&ins.op(3)), Reason::UnsupportedType { why: "vector que no es de 2, 3 o 4" })
            }
            op::OpTypeArray => {
                self.tipo_dato(ins.op(2))?;
                si(self.constante_entera(ins.op(3))? > 0, no)
            }
            op::OpTypeRuntimeArray => self.tipo_dato(ins.op(2)),
            op::OpTypeStruct => {
                for k in 2..ins.words as usize {
                    self.tipo_dato(ins.op(k))?;
                }
                Ok(())
            }
            op::OpTypePointer => {
                self.class(ins.op(2))?;
                self.tipo(ins.op(3)).map(|_| ())
            }
            op::OpTypeFunction => {
                self.tipo(ins.op(2))?;
                for k in 3..ins.words as usize {
                    self.tipo_dato(ins.op(k))?;
                }
                Ok(())
            }
            op::OpConstantTrue | op::OpConstantFalse => {
                self.tipo(ins.op(1))?;
                si(self.num(ins.op(1)) == Some((Escalar::Bool, 1)), no)
            }
            op::OpConstant => {
                self.tipo(ins.op(1))?;
                let n = self.num(ins.op(1));
                let escalar = n == Some((Escalar::Int, 1)) || n == Some((Escalar::Float, 1));
                si(escalar && ins.words == 4, no)
            }
            op::OpConstantComposite => {
                self.tipo(ins.op(1))?;
                self.compuesto(ins, true)
            }
            op::OpConstantNull => self.tipo(ins.op(1)).map(|_| ()),
            _ => Err(Reason::UnsupportedInstruction { opcode, why: "tipo que el juez no conoce" }),
        }
    }

    /// `OpCompositeConstruct` / `OpConstantComposite`: las partes, desde la
    /// palabra 3, llenan el tipo de resultado.
    fn compuesto(&self, ins: &Instruction, constantes: bool) -> Result<(), Reason> {
        let no = Reason::TypeMismatch { opcode: ins.opcode };
        let d = self.def(ins.op(1))?;
        let partes = ins.words as usize - 3;
        let tipo_parte = |k: usize| {
            if constantes {
                self.constante(ins.op(3 + k))
            } else {
                self.valor(ins.op(3 + k))
            }
        };
        match d.opcode {
            op::OpTypeVector => {
                let c = d.op(2);
                let mut total = 0;
                for k in 0..partes {
                    let t = tipo_parte(k)?;
                    if t == c {
                        total += 1;
                    } else if !constantes && self.componente(t) == c && t != c {
                        total += self.num(t).map(|x| x.1).unwrap_or(0);
                    } else {
                        return Err(no);
                    }
                }
                si(total == d.op(3), no)
            }
            op::OpTypeArray => {
                si(partes as u32 == self.constante_entera(d.op(3))?, no)?;
                for k in 0..partes {
                    si(tipo_parte(k)? == d.op(2), no)?;
                }
                Ok(())
            }
            op::OpTypeStruct => {
                si(partes + 2 == d.words as usize, no)?;
                for k in 0..partes {
                    si(tipo_parte(k)? == d.op(2 + k), no)?;
                }
                Ok(())
            }
            _ => Err(no),
        }
    }

    fn flexible(&mut self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        let dentro = self.f.is_some();
        match opcode {
            op::OpLine | op::OpNoLine | op::OpNop => Ok(()),
            op::OpUndef => {
                if dentro {
                    self.en_bloque(opcode)?;
                    self.fase = Fase::Body;
                }
                self.tipo(ins.op(1)).map(|_| ())
            }
            op::OpVariable if !dentro => self.variable_global(ins),
            op::OpVariable => {
                let primer = self.f.map(|f| f.blocks == 1).unwrap_or(false);
                si(self.fase == Fase::Cabeza && primer, Reason::MisplacedVariable)?;
                let no = Reason::TypeMismatch { opcode };
                let (class, apuntado) = self.puntero(ins.op(1)).ok_or(no)?;
                si(class == CLASE_FUNCTION && ins.op(3) == CLASE_FUNCTION, no)?;
                if ins.words > 4 {
                    si(self.constante(ins.op(4))? == apuntado, no)?;
                }
                Ok(())
            }
            _ => Err(Reason::UnsupportedInstruction { opcode, why: "instruccion flexible que el juez no conoce" }),
        }
    }

    fn variable_global(&self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        let no = Reason::TypeMismatch { opcode };
        let id = ins.op(2);
        let (clase_p, apuntado) = self.puntero(ins.op(1)).ok_or(no)?;
        let class = ins.op(3);
        si(class == clase_p, no)?;
        si(class != CLASE_FUNCTION, Reason::MisplacedVariable)?;
        self.class(class)?;
        if ins.words > 4 {
            si(self.constante(ins.op(4))? == apuntado, no)?;
        }
        match class {
            CLASE_INPUT => {
                let b = self.decoracion(id, DEC_BUILTIN).ok_or(Reason::InputWithoutBuiltIn { id })?;
                si(BUILTINS_DE_COMPUTO.contains(&b), Reason::UnsupportedBuiltIn { builtin: b })
            }
            CLASE_UNIFORM | CLASE_STORAGE_BUFFER => si(
                self.decoracion(id, DEC_BINDING).is_some() && self.decoracion(id, DEC_DESCRIPTOR_SET).is_some(),
                Reason::NoBinding { id },
            ),
            _ => Ok(()),
        }
    }

    fn abrir(&mut self, ins: &Instruction) -> Result<(), Reason> {
        let no = Reason::TypeMismatch { opcode: ins.opcode };
        let ret = ins.op(1);
        let tf = self.tipo(ins.op(4))?;
        si(tf.opcode == op::OpTypeFunction && tf.op(2) == ret, no)?;
        let fin = self
            .m
            .instructions_from(ins.offset)
            .find(|i| i.opcode == op::OpFunctionEnd)
            .map(|i| i.offset)
            .unwrap_or(usize::MAX);
        self.f = Some(Function {
            ini: ins.offset,
            fin,
            ret,
            tipo_fn: ins.op(4),
            params: tf.words as u32 - 3,
            vistos: 0,
            blocks: 0,
        });
        self.fase = Fase::Parametros;
        Ok(())
    }

    fn cerrar(&mut self) -> Result<(), Reason> {
        let f = self.f.take().ok_or(Reason::StrayFunctionEnd)?;
        match self.fase {
            Fase::EntreBloques => {}
            Fase::Parametros => return Err(Reason::NoBody),
            _ => return Err(Reason::UnterminatedBlock),
        }
        si(f.blocks > 0, Reason::NoBody)?;
        self.fase = Fase::Fuera;
        Ok(())
    }

    /// La instruccion va dentro de un bloque abierto, y si hay una fusion
    /// pendiente, es su salto.
    fn en_bloque(&self, opcode: u16) -> Result<(), Reason> {
        si(matches!(self.fase, Fase::Cabeza | Fase::Body), Reason::OutsideBlock { opcode })?;
        match self.merge {
            None => Ok(()),
            Some(op::OpLoopMerge) => {
                si(opcode == op::OpBranch || opcode == op::OpBranchConditional, Reason::MisplacedMerge)
            }
            Some(_) => si(opcode == op::OpBranchConditional, Reason::MisplacedMerge),
        }
    }

    fn cuerpo(&mut self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        match opcode {
            op::OpFunctionParameter => {
                let mut f = self.f.ok_or(Reason::OutsideBlock { opcode })?;
                si(self.fase == Fase::Parametros && f.vistos < f.params, Reason::OutsideBlock { opcode })?;
                let tf = self.def(f.tipo_fn)?;
                si(ins.op(1) == tf.op(3 + f.vistos as usize), Reason::TypeMismatch { opcode })?;
                f.vistos += 1;
                self.f = Some(f);
                Ok(())
            }
            op::OpLabel => {
                let mut f = self.f.ok_or(Reason::OutsideBlock { opcode })?;
                match self.fase {
                    Fase::Parametros => si(
                        f.vistos == f.params,
                        Reason::TypeMismatch { opcode: op::OpFunctionParameter },
                    )?,
                    Fase::EntreBloques => {}
                    _ => return Err(Reason::UnterminatedBlock),
                }
                f.blocks += 1;
                self.f = Some(f);
                self.blocks += 1;
                self.fase = Fase::Cabeza;
                self.merge = None;
                Ok(())
            }
            _ => {
                self.en_bloque(opcode)?;
                if opcode == op::OpPhi {
                    si(self.fase == Fase::Cabeza, Reason::MisplacedPhi)?;
                } else {
                    self.fase = Fase::Body;
                }
                self.significado(ins)?;
                match opcode {
                    op::OpBranch
                    | op::OpBranchConditional
                    | op::OpReturn
                    | op::OpReturnValue
                    | op::OpUnreachable => {
                        self.fase = Fase::EntreBloques;
                        self.merge = None;
                    }
                    op::OpLoopMerge | op::OpSelectionMerge => self.merge = Some(opcode),
                    _ => {}
                }
                Ok(())
            }
        }
    }

    /// Los tipos de una instruccion de cuerpo.
    fn significado(&self, ins: &Instruction) -> Result<(), Reason> {
        let opcode = ins.opcode;
        let no = Reason::TypeMismatch { opcode };
        let r = ins.op(1);
        let n_ops = ins.words as usize;
        use Escalar::{Bool, Float, Int};
        match opcode {
            // -- enteros --
            op::OpIAdd
            | op::OpISub
            | op::OpIMul
            | op::OpUDiv
            | op::OpSDiv
            | op::OpUMod
            | op::OpSRem
            | op::OpSMod
            | op::OpShiftRightLogical
            | op::OpShiftRightArithmetic
            | op::OpShiftLeftLogical
            | op::OpBitwiseOr
            | op::OpBitwiseXor
            | op::OpBitwiseAnd => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Int, no)?;
                si(self.num(self.valor(ins.op(3))?) == Some((Int, n)), no)?;
                si(self.num(self.valor(ins.op(4))?) == Some((Int, n)), no)
            }
            op::OpSNegate | op::OpNot => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Int && self.num(self.valor(ins.op(3))?) == Some((Int, n)), no)
            }
            // -- flotantes --
            op::OpFAdd | op::OpFSub | op::OpFMul | op::OpFDiv | op::OpFRem | op::OpFMod => {
                si(self.num(r).map(|x| x.0) == Some(Float), no)?;
                si(self.valor(ins.op(3))? == r && self.valor(ins.op(4))? == r, no)
            }
            op::OpFNegate => {
                si(self.num(r).map(|x| x.0) == Some(Float), no)?;
                si(self.valor(ins.op(3))? == r, no)
            }
            op::OpVectorTimesScalar => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Float && n > 1 && self.valor(ins.op(3))? == r, no)?;
                si(self.valor(ins.op(4))? == self.componente(r), no)
            }
            op::OpDot => {
                si(self.num(r) == Some((Float, 1)), no)?;
                let a = self.valor(ins.op(3))?;
                si(a == self.valor(ins.op(4))? && self.componente(a) == r && a != r, no)
            }
            // -- comparaciones --
            op::OpIEqual
            | op::OpINotEqual
            | op::OpUGreaterThan
            | op::OpSGreaterThan
            | op::OpUGreaterThanEqual
            | op::OpSGreaterThanEqual
            | op::OpULessThan
            | op::OpSLessThan
            | op::OpULessThanEqual
            | op::OpSLessThanEqual => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Bool, no)?;
                si(self.num(self.valor(ins.op(3))?) == Some((Int, n)), no)?;
                si(self.num(self.valor(ins.op(4))?) == Some((Int, n)), no)
            }
            op::OpFOrdEqual
            | op::OpFUnordEqual
            | op::OpFOrdNotEqual
            | op::OpFUnordNotEqual
            | op::OpFOrdLessThan
            | op::OpFUnordLessThan
            | op::OpFOrdGreaterThan
            | op::OpFUnordGreaterThan
            | op::OpFOrdLessThanEqual
            | op::OpFUnordLessThanEqual
            | op::OpFOrdGreaterThanEqual
            | op::OpFUnordGreaterThanEqual => {
                let (e, n) = self.num(r).ok_or(no)?;
                let a = self.valor(ins.op(3))?;
                si(e == Bool && self.num(a) == Some((Float, n)), no)?;
                si(self.valor(ins.op(4))? == a, no)
            }
            op::OpIsNan | op::OpIsInf => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Bool && self.num(self.valor(ins.op(3))?) == Some((Float, n)), no)
            }
            // -- logicas --
            op::OpLogicalEqual | op::OpLogicalNotEqual | op::OpLogicalOr | op::OpLogicalAnd => {
                si(self.num(r).map(|x| x.0) == Some(Bool), no)?;
                si(self.valor(ins.op(3))? == r && self.valor(ins.op(4))? == r, no)
            }
            op::OpLogicalNot => {
                si(self.num(r).map(|x| x.0) == Some(Bool) && self.valor(ins.op(3))? == r, no)
            }
            op::OpAny | op::OpAll => {
                si(self.num(r) == Some((Bool, 1)), no)?;
                let (e, n) = self.num(self.valor(ins.op(3))?).ok_or(no)?;
                si(e == Bool && n > 1, no)
            }
            op::OpSelect => {
                let (_, n) = self.num(r).ok_or(no)?;
                let c = self.num(self.valor(ins.op(3))?).ok_or(no)?;
                si(c == (Bool, 1) || c == (Bool, n), no)?;
                si(self.valor(ins.op(4))? == r && self.valor(ins.op(5))? == r, no)
            }
            // -- conversiones --
            op::OpConvertFToU | op::OpConvertFToS => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Int && self.num(self.valor(ins.op(3))?) == Some((Float, n)), no)
            }
            op::OpConvertSToF | op::OpConvertUToF => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Float && self.num(self.valor(ins.op(3))?) == Some((Int, n)), no)
            }
            op::OpUConvert | op::OpSConvert => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Int && self.num(self.valor(ins.op(3))?) == Some((Int, n)), no)
            }
            op::OpFConvert => {
                let (e, n) = self.num(r).ok_or(no)?;
                si(e == Float && self.num(self.valor(ins.op(3))?) == Some((Float, n)), no)
            }
            op::OpBitcast => {
                let (e, n) = self.num(r).ok_or(no)?;
                let (ea, na) = self.num(self.valor(ins.op(3))?).ok_or(no)?;
                si(e != Bool && ea != Bool && n == na, no)
            }
            op::OpCopyObject => si(self.valor(ins.op(3))? == r, no),
            // -- compuestos --
            op::OpCompositeConstruct => {
                self.tipo(r)?;
                self.compuesto(ins, false)
            }
            op::OpCompositeExtract => {
                si(n_ops > 4, no)?;
                let mut t = self.valor(ins.op(3))?;
                for k in 4..n_ops {
                    t = self.paso(t, ins.op(k), opcode)?;
                }
                si(t == r, no)
            }
            op::OpCompositeInsert => {
                si(n_ops > 5, no)?;
                let objeto = self.valor(ins.op(3))?;
                let mut t = self.valor(ins.op(4))?;
                si(t == r, no)?;
                for k in 5..n_ops {
                    t = self.paso(t, ins.op(k), opcode)?;
                }
                si(t == objeto, no)
            }
            op::OpVectorShuffle => {
                let a = self.valor(ins.op(3))?;
                let b = self.valor(ins.op(4))?;
                let (_, na) = self.num(a).ok_or(no)?;
                let (_, nb) = self.num(b).ok_or(no)?;
                let (_, nr) = self.num(r).ok_or(no)?;
                let c = self.componente(r);
                si(a != c && b != c && self.componente(a) == c && self.componente(b) == c, no)?;
                si(nr as usize == n_ops - 5 && nr > 1, no)?;
                for k in 5..n_ops {
                    let i = ins.op(k);
                    si(i < na + nb || i == u32::MAX, Reason::IndexOutOfRange { opcode })?;
                }
                Ok(())
            }
            op::OpVectorExtractDynamic => {
                let v = self.valor(ins.op(3))?;
                si(v != r && self.componente(v) == r, no)?;
                si(self.num(self.valor(ins.op(4))?) == Some((Int, 1)), no)
            }
            op::OpVectorInsertDynamic => {
                si(self.valor(ins.op(3))? == r && self.componente(r) != r, no)?;
                si(self.valor(ins.op(4))? == self.componente(r), no)?;
                si(self.num(self.valor(ins.op(5))?) == Some((Int, 1)), no)
            }
            // -- memoria --
            op::OpLoad => {
                let (_, apuntado) = self.puntero(self.valor(ins.op(3))?).ok_or(no)?;
                si(apuntado == r, no)
            }
            op::OpStore => {
                let (class, apuntado) = self.puntero(self.valor(ins.op(1))?).ok_or(no)?;
                si(class != CLASE_INPUT, Reason::WriteToInput)?;
                si(self.valor(ins.op(2))? == apuntado, no)
            }
            op::OpCopyMemory => {
                let (class, a) = self.puntero(self.valor(ins.op(1))?).ok_or(no)?;
                let (_, b) = self.puntero(self.valor(ins.op(2))?).ok_or(no)?;
                si(class != CLASE_INPUT, Reason::WriteToInput)?;
                si(a == b, no)
            }
            op::OpAccessChain | op::OpInBoundsAccessChain => {
                let (class, mut t) = self.puntero(self.valor(ins.op(3))?).ok_or(no)?;
                for k in 4..n_ops {
                    let idx = ins.op(k);
                    si(self.num(self.valor(idx)?) == Some((Int, 1)), no)?;
                    let d = self.m.def(t).ok_or(no)?;
                    t = match d.opcode {
                        op::OpTypeVector | op::OpTypeArray | op::OpTypeRuntimeArray => d.op(2),
                        op::OpTypeStruct => {
                            let v = self.constante_entera(idx)? as usize;
                            si(v + 2 < d.words as usize, Reason::IndexOutOfRange { opcode })?;
                            d.op(2 + v)
                        }
                        _ => return Err(Reason::IndexOutOfRange { opcode }),
                    };
                }
                si(self.puntero(r) == Some((class, t)), no)
            }
            op::OpArrayLength => {
                si(self.num(r) == Some((Int, 1)), no)?;
                let (_, s) = self.puntero(self.valor(ins.op(3))?).ok_or(no)?;
                let d = self.m.def(s).ok_or(no)?;
                let miembro = ins.op(4) as usize;
                si(d.opcode == op::OpTypeStruct && miembro + 3 == d.words as usize, no)?;
                let ultimo = self.m.def(d.op(2 + miembro)).map(|x| x.opcode);
                si(ultimo == Some(op::OpTypeRuntimeArray), no)
            }
            // -- funciones --
            op::OpFunctionCall => {
                let callee = self.def(ins.op(3))?;
                si(callee.opcode == op::OpFunction && callee.op(1) == r, no)?;
                let tf = self.def(callee.op(4))?;
                si(tf.words as usize - 3 == n_ops - 4, no)?;
                for k in 0..n_ops - 4 {
                    si(self.valor(ins.op(4 + k))? == tf.op(3 + k), no)?;
                }
                Ok(())
            }
            op::OpExtInst => {
                si(Some(ins.op(3)) == self.glsl_id, Reason::UnsupportedImport)?;
                let number = ins.op(4);
                let g = glsl_info(number).ok_or(Reason::UnsupportedGlsl { number })?;
                si(g.group != GlslGroup::Transcendental, Reason::GlslLater { number })?;
                si(n_ops - 5 == g.operands as usize, no)?;
                let (e, n) = self.num(r).ok_or(no)?;
                for k in 5..n_ops {
                    let t = self.valor(ins.op(k))?;
                    match g.group {
                        GlslGroup::Float => si(e == Float && t == r, no)?,
                        _ => si(e == Int && self.num(t) == Some((Int, n)), no)?,
                    }
                }
                Ok(())
            }
            // -- flujo --
            op::OpPhi => {
                self.tipo(r)?;
                let pares = n_ops - 3;
                si(pares >= 2 && pares % 2 == 0, no)?;
                let mut k = 3;
                while k < n_ops {
                    si(self.valor_adelante(ins.op(k))? == r, no)?;
                    self.etiqueta(ins.op(k + 1))?;
                    k += 2;
                }
                Ok(())
            }
            op::OpLoopMerge => {
                self.etiqueta(ins.op(1))?;
                self.etiqueta(ins.op(2))
            }
            op::OpSelectionMerge => self.etiqueta(ins.op(1)),
            op::OpBranch => self.etiqueta(ins.op(1)),
            op::OpBranchConditional => {
                si(self.num(self.valor(ins.op(1))?) == Some((Bool, 1)), no)?;
                si(n_ops == 4 || n_ops == 6, no)?;
                let (a, b) = (ins.op(2), ins.op(3));
                self.etiqueta(a)?;
                self.etiqueta(b)?;
                if self.merge.is_none() && a != b {
                    si(self.sale_de_un_bucle(a) || self.sale_de_un_bucle(b), Reason::Unstructured)?;
                }
                Ok(())
            }
            op::OpReturn => {
                let ret = self.f.map(|f| f.ret).unwrap_or(0);
                si(self.m.def(ret).map(|d| d.opcode) == Some(op::OpTypeVoid), no)
            }
            op::OpReturnValue => {
                let ret = self.f.map(|f| f.ret).unwrap_or(0);
                si(self.valor(ins.op(1))? == ret, no)
            }
            op::OpUnreachable => Ok(()),
            _ => Err(Reason::UnsupportedInstruction { opcode, why: "instruccion del nucleo que el juez aun no mira" }),
        }
    }

    /// `destino` es la salida o la continuacion de algun bucle de esta funcion.
    fn sale_de_un_bucle(&self, destino: u32) -> bool {
        let Some(f) = self.f else { return false };
        self.m
            .instructions_from(f.ini)
            .take_while(|i| i.offset < f.fin)
            .any(|i| i.opcode == op::OpLoopMerge && (i.op(1) == destino || i.op(2) == destino))
    }
}
