//! **EL JUEZ (S2): un modulo leido cabe en el subconjunto, o por que no.**
//!
//! El lector (S1) garantiza la FORMA. El juez mira el SIGNIFICADO, en una
//! pasada y sin pedir memoria: los tipos se preguntan a la tabla de ids que ya
//! lleno el lector (`Modulo::def`), asi que no hace falta ninguna tabla nueva.
//!
//! Lo que comprueba, en el orden en que aparece en el fichero:
//!
//! - el modulo: solo `Shader`, `Logical` + `GLSL450`, solo `GLSL.std.450`,
//!   etapa `GLCompute` con `LocalSize`, y el punto de entrada es `void main()`;
//! - cada instruccion es de la familia `Nucleo` (el resto se niega NOMBRANDO la
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

use crate::tabla::op;
use crate::{fila, glsl, Fallo, Familia, GrupoGlsl, Instr, Modulo, Motivo, Seccion};

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
pub struct Veredicto {
    pub entradas: usize,
    pub funciones: usize,
    pub bloques: usize,
    pub instrucciones: usize,
}

/// Cuantas instrucciones hay de cada familia, en el orden de `Familia::TODAS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Censo {
    pub por_familia: [usize; 9],
}

impl Censo {
    /// Cuantas de fuera del nucleo.
    pub fn fuera(&self) -> usize {
        self.por_familia[1..].iter().sum()
    }
}

/// **Cuenta de que familias es un modulo**, sin parar en el primer NO.
pub fn censo(m: &Modulo) -> Censo {
    let mut c = Censo { por_familia: [0; 9] };
    for ins in m.recorrer() {
        if let Some(f) = fila(ins.codigo) {
            c.por_familia[f.familia.indice()] += 1;
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
    Cuerpo,
}

#[derive(Clone, Copy)]
struct Funcion {
    ini: usize,
    fin: usize,
    ret: u32,
    tipo_fn: u32,
    params: u32,
    vistos: u32,
    bloques: usize,
}

struct Juez<'m, 'a, 'b> {
    m: &'m Modulo<'a, 'b>,
    glsl_id: Option<u32>,
    primera_funcion: usize,
    /// La palabra de la instruccion que se esta juzgando.
    cur: usize,
    f: Option<Funcion>,
    fase: Fase,
    merge: Option<u16>,
    bloques: usize,
}

/// **Juzga un modulo leido.** El primer motivo por el que no cabe, con la
/// palabra donde se vio; o lo que se conto de el.
pub fn juzgar(m: &Modulo) -> Result<Veredicto, Fallo> {
    let primera_funcion = m
        .recorrer()
        .find(|i| i.codigo == op::OpFunction)
        .map(|i| i.desde)
        .unwrap_or(usize::MAX);
    let mut j = Juez {
        m,
        glsl_id: m.glsl450(),
        primera_funcion,
        cur: 0,
        f: None,
        fase: Fase::Fuera,
        merge: None,
        bloques: 0,
    };
    for ins in m.recorrer() {
        j.cur = ins.desde;
        j.una(&ins).map_err(|motivo| Fallo { motivo, palabra: ins.desde })?;
    }
    if m.entradas().is_empty() {
        return Err(Fallo { motivo: Motivo::SinEntrada, palabra: 5 });
    }
    for e in m.entradas() {
        let donde = m
            .recorrer()
            .find(|i| i.codigo == op::OpEntryPoint && i.op(2) == e.id)
            .map(|i| i.desde)
            .unwrap_or(5);
        if e.local.is_none() {
            return Err(Fallo { motivo: Motivo::SinLocalSize, palabra: donde });
        }
        if !j.es_void_main(e.id) {
            return Err(Fallo { motivo: Motivo::EntradaNoCuadra, palabra: donde });
        }
    }
    Ok(Veredicto {
        entradas: m.entradas().len(),
        funciones: m.funciones,
        bloques: j.bloques,
        instrucciones: m.instrucciones,
    })
}

fn si(cond: bool, motivo: Motivo) -> Result<(), Motivo> {
    if cond {
        Ok(())
    } else {
        Err(motivo)
    }
}

impl<'m, 'a, 'b> Juez<'m, 'a, 'b> {
    // ---- preguntas sobre ids -----------------------------------------------

    fn def(&self, id: u32) -> Result<Instr<'a>, Motivo> {
        self.m.def(id).ok_or(Motivo::NoDefinido { id })
    }

    /// `id` es un TIPO definido antes de aqui.
    fn tipo(&self, id: u32) -> Result<Instr<'a>, Motivo> {
        let d = self.def(id)?;
        let es = (op::OpTypeVoid..=op::OpTypeFunction).contains(&d.codigo);
        si(es && d.desde < self.cur, Motivo::NoEsTipo { id })?;
        Ok(d)
    }

    /// Un tipo que puede guardar un dato: ni `void` ni una funcion.
    fn tipo_dato(&self, id: u32) -> Result<(), Motivo> {
        let d = self.tipo(id)?;
        si(d.codigo != op::OpTypeVoid && d.codigo != op::OpTypeFunction, Motivo::NoEsTipo { id })
    }

    /// `(escalar, componentes)` de un escalar o un vector.
    fn num(&self, t: u32) -> Option<(Escalar, u32)> {
        let d = self.m.def(t)?;
        match d.codigo {
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
            Some(d) if d.codigo == op::OpTypeVector => d.op(2),
            _ => t,
        }
    }

    /// `(clase, apuntado)` de un tipo puntero.
    fn puntero(&self, t: u32) -> Option<(u32, u32)> {
        let d = self.m.def(t)?;
        if d.codigo == op::OpTypePointer {
            Some((d.op(2), d.op(3)))
        } else {
            None
        }
    }

    /// El tipo de un VALOR definido antes, en esta funcion o global.
    fn valor(&self, id: u32) -> Result<u32, Motivo> {
        let d = self.def(id)?;
        let f = fila(d.codigo).ok_or(Motivo::NoEsValor { id })?;
        si(f.tipo && d.codigo != op::OpFunction, Motivo::NoEsValor { id })?;
        si(d.desde < self.cur, Motivo::UsoAntesDeDefinir { id })?;
        if d.desde >= self.primera_funcion {
            let ini = self.f.map(|f| f.ini).unwrap_or(usize::MAX);
            si(d.desde > ini, Motivo::UsoAntesDeDefinir { id })?;
        }
        Ok(d.op(1))
    }

    /// Como `valor`, pero puede estar mas adelante en la misma funcion (`OpPhi`).
    fn valor_adelante(&self, id: u32) -> Result<u32, Motivo> {
        let d = self.def(id)?;
        let f = fila(d.codigo).ok_or(Motivo::NoEsValor { id })?;
        si(f.tipo && d.codigo != op::OpFunction, Motivo::NoEsValor { id })?;
        if d.desde >= self.primera_funcion {
            let (ini, fin) = self.f.map(|f| (f.ini, f.fin)).unwrap_or((0, 0));
            si(d.desde > ini && d.desde < fin, Motivo::UsoAntesDeDefinir { id })?;
        }
        Ok(d.op(1))
    }

    /// Una constante definida antes; devuelve su tipo.
    fn constante(&self, id: u32) -> Result<u32, Motivo> {
        let d = self.def(id)?;
        let es = matches!(
            d.codigo,
            op::OpConstantTrue | op::OpConstantFalse | op::OpConstant | op::OpConstantComposite | op::OpConstantNull
        );
        si(es && d.desde < self.cur, Motivo::NoEsConstante { id })?;
        Ok(d.op(1))
    }

    /// El valor de una constante entera escalar.
    fn constante_entera(&self, id: u32) -> Result<u32, Motivo> {
        let t = self.constante(id)?;
        let d = self.def(id)?;
        si(d.codigo == op::OpConstant && self.num(t) == Some((Escalar::Int, 1)), Motivo::NoEsConstante { id })?;
        Ok(d.op(3))
    }

    /// Una etiqueta de la funcion actual (puede estar mas adelante).
    fn etiqueta(&self, id: u32) -> Result<(), Motivo> {
        let d = self.def(id)?;
        let (ini, fin) = self.f.map(|f| (f.ini, f.fin)).unwrap_or((0, 0));
        si(d.codigo == op::OpLabel && d.desde > ini && d.desde < fin, Motivo::NoEsEtiqueta { id })
    }

    /// El valor de una decoracion `dec` sobre `id`, si la tiene.
    fn decoracion(&self, id: u32, dec: u32) -> Option<u32> {
        for i in self.m.recorrer() {
            if let Some(f) = fila(i.codigo) {
                if f.seccion > Seccion::Anotacion {
                    break;
                }
            }
            if i.codigo == op::OpDecorate && i.op(1) == id && i.op(2) == dec {
                return Some(i.op(3));
            }
        }
        None
    }

    /// Paso de un compuesto por un indice literal.
    fn paso(&self, t: u32, idx: u32, codigo: u16) -> Result<u32, Motivo> {
        let fuera = Motivo::IndiceFuera { codigo };
        let d = self.m.def(t).ok_or(fuera)?;
        match d.codigo {
            op::OpTypeVector if idx < d.op(3) => Ok(d.op(2)),
            op::OpTypeArray if idx < self.constante_entera(d.op(3))? => Ok(d.op(2)),
            op::OpTypeStruct if (idx as usize) + 2 < d.palabras as usize => Ok(d.op(2 + idx as usize)),
            _ => Err(fuera),
        }
    }

    fn es_void_main(&self, id: u32) -> bool {
        let Some(f) = self.m.def(id) else { return false };
        let Some(tf) = self.m.def(f.op(4)) else { return false };
        let void = self.m.def(f.op(1)).map(|d| d.codigo) == Some(op::OpTypeVoid);
        f.codigo == op::OpFunction && void && tf.palabras == 3
    }

    // ---- una instruccion ---------------------------------------------------

    fn una(&mut self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        let f = fila(codigo).ok_or(Motivo::SinFila { codigo })?;
        if f.familia != Familia::Nucleo {
            return Err(Motivo::FamiliaFuera { familia: f.familia, codigo });
        }
        match f.seccion {
            Seccion::Capacidad => {
                si(ins.op(1) == CAP_SHADER, Motivo::CapacidadFuera { capacidad: ins.op(1) })
            }
            Seccion::Extension => {
                let e = ins.cadena(1).unwrap_or(&[]);
                si(EXTENSIONES.contains(&e), Motivo::ExtensionFuera)
            }
            Seccion::Importacion => {
                si(ins.cadena(2) == Some(&b"GLSL.std.450"[..]), Motivo::ImportacionFuera)
            }
            Seccion::Modelo => si(
                ins.op(1) == DIR_LOGICAL && ins.op(2) == MEM_GLSL450,
                Motivo::ModeloFuera { direccionamiento: ins.op(1), memoria: ins.op(2) },
            ),
            Seccion::Entrada => si(ins.op(1) == GL_COMPUTE, Motivo::EtapaFuera { modelo: ins.op(1) }),
            Seccion::Modo => si(
                codigo == op::OpExecutionMode && ins.op(2) == MODO_LOCAL_SIZE,
                Motivo::ModoFuera { modo: ins.op(2) },
            ),
            Seccion::Fuente | Seccion::Nombre | Seccion::Procesado => Ok(()),
            Seccion::Anotacion => match codigo {
                op::OpDecorationGroup | op::OpGroupDecorate | op::OpGroupMemberDecorate => {
                    Err(Motivo::InstruccionFuera {
                        codigo,
                        porque: "grupos de decoraciones: obsoletos desde SPIR-V 1.5",
                    })
                }
                _ => Ok(()),
            },
            Seccion::Tipo => self.tipo_o_constante(ins),
            Seccion::Flexible => self.flexible(ins),
            Seccion::Funcion => self.abrir(ins),
            Seccion::FinFuncion => self.cerrar(),
            Seccion::Cuerpo => self.cuerpo(ins),
        }
    }

    fn clase(&self, clase: u32) -> Result<(), Motivo> {
        si(
            matches!(clase, CLASE_INPUT | CLASE_UNIFORM | CLASE_PRIVATE | CLASE_FUNCTION | CLASE_STORAGE_BUFFER),
            Motivo::ClaseFuera { clase },
        )
    }

    fn tipo_o_constante(&self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        let no = Motivo::TipoNoCuadra { codigo };
        match codigo {
            op::OpTypeVoid | op::OpTypeBool => Ok(()),
            op::OpTypeInt => {
                si(ins.op(2) == 32, Motivo::TipoFuera { porque: "entero que no es de 32 bits" })?;
                si(ins.op(3) <= 1, no)
            }
            op::OpTypeFloat => si(ins.op(2) == 32, Motivo::TipoFuera { porque: "flotante que no es de 32 bits" }),
            op::OpTypeVector => {
                self.tipo(ins.op(2))?;
                si(self.num(ins.op(2)).map(|x| x.1) == Some(1), no)?;
                si((2..=4).contains(&ins.op(3)), Motivo::TipoFuera { porque: "vector que no es de 2, 3 o 4" })
            }
            op::OpTypeArray => {
                self.tipo_dato(ins.op(2))?;
                si(self.constante_entera(ins.op(3))? > 0, no)
            }
            op::OpTypeRuntimeArray => self.tipo_dato(ins.op(2)),
            op::OpTypeStruct => {
                for k in 2..ins.palabras as usize {
                    self.tipo_dato(ins.op(k))?;
                }
                Ok(())
            }
            op::OpTypePointer => {
                self.clase(ins.op(2))?;
                self.tipo(ins.op(3)).map(|_| ())
            }
            op::OpTypeFunction => {
                self.tipo(ins.op(2))?;
                for k in 3..ins.palabras as usize {
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
                si(escalar && ins.palabras == 4, no)
            }
            op::OpConstantComposite => {
                self.tipo(ins.op(1))?;
                self.compuesto(ins, true)
            }
            op::OpConstantNull => self.tipo(ins.op(1)).map(|_| ()),
            _ => Err(Motivo::InstruccionFuera { codigo, porque: "tipo que el juez no conoce" }),
        }
    }

    /// `OpCompositeConstruct` / `OpConstantComposite`: las partes, desde la
    /// palabra 3, llenan el tipo de resultado.
    fn compuesto(&self, ins: &Instr, constantes: bool) -> Result<(), Motivo> {
        let no = Motivo::TipoNoCuadra { codigo: ins.codigo };
        let d = self.def(ins.op(1))?;
        let partes = ins.palabras as usize - 3;
        let tipo_parte = |k: usize| {
            if constantes {
                self.constante(ins.op(3 + k))
            } else {
                self.valor(ins.op(3 + k))
            }
        };
        match d.codigo {
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
                si(partes + 2 == d.palabras as usize, no)?;
                for k in 0..partes {
                    si(tipo_parte(k)? == d.op(2 + k), no)?;
                }
                Ok(())
            }
            _ => Err(no),
        }
    }

    fn flexible(&mut self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        let dentro = self.f.is_some();
        match codigo {
            op::OpLine | op::OpNoLine | op::OpNop => Ok(()),
            op::OpUndef => {
                if dentro {
                    self.en_bloque(codigo)?;
                    self.fase = Fase::Cuerpo;
                }
                self.tipo(ins.op(1)).map(|_| ())
            }
            op::OpVariable if !dentro => self.variable_global(ins),
            op::OpVariable => {
                let primer = self.f.map(|f| f.bloques == 1).unwrap_or(false);
                si(self.fase == Fase::Cabeza && primer, Motivo::VariableFueraDeSitio)?;
                let no = Motivo::TipoNoCuadra { codigo };
                let (clase, apuntado) = self.puntero(ins.op(1)).ok_or(no)?;
                si(clase == CLASE_FUNCTION && ins.op(3) == CLASE_FUNCTION, no)?;
                if ins.palabras > 4 {
                    si(self.constante(ins.op(4))? == apuntado, no)?;
                }
                Ok(())
            }
            _ => Err(Motivo::InstruccionFuera { codigo, porque: "instruccion flexible que el juez no conoce" }),
        }
    }

    fn variable_global(&self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        let no = Motivo::TipoNoCuadra { codigo };
        let id = ins.op(2);
        let (clase_p, apuntado) = self.puntero(ins.op(1)).ok_or(no)?;
        let clase = ins.op(3);
        si(clase == clase_p, no)?;
        si(clase != CLASE_FUNCTION, Motivo::VariableFueraDeSitio)?;
        self.clase(clase)?;
        if ins.palabras > 4 {
            si(self.constante(ins.op(4))? == apuntado, no)?;
        }
        match clase {
            CLASE_INPUT => {
                let b = self.decoracion(id, DEC_BUILTIN).ok_or(Motivo::EntradaSinBuiltIn { id })?;
                si(BUILTINS_DE_COMPUTO.contains(&b), Motivo::BuiltInFuera { builtin: b })
            }
            CLASE_UNIFORM | CLASE_STORAGE_BUFFER => si(
                self.decoracion(id, DEC_BINDING).is_some() && self.decoracion(id, DEC_DESCRIPTOR_SET).is_some(),
                Motivo::SinBinding { id },
            ),
            _ => Ok(()),
        }
    }

    fn abrir(&mut self, ins: &Instr) -> Result<(), Motivo> {
        let no = Motivo::TipoNoCuadra { codigo: ins.codigo };
        let ret = ins.op(1);
        let tf = self.tipo(ins.op(4))?;
        si(tf.codigo == op::OpTypeFunction && tf.op(2) == ret, no)?;
        let fin = self
            .m
            .recorrer_desde(ins.desde)
            .find(|i| i.codigo == op::OpFunctionEnd)
            .map(|i| i.desde)
            .unwrap_or(usize::MAX);
        self.f = Some(Funcion {
            ini: ins.desde,
            fin,
            ret,
            tipo_fn: ins.op(4),
            params: tf.palabras as u32 - 3,
            vistos: 0,
            bloques: 0,
        });
        self.fase = Fase::Parametros;
        Ok(())
    }

    fn cerrar(&mut self) -> Result<(), Motivo> {
        let f = self.f.take().ok_or(Motivo::FinSinFuncion)?;
        match self.fase {
            Fase::EntreBloques => {}
            Fase::Parametros => return Err(Motivo::SinCuerpo),
            _ => return Err(Motivo::BloqueSinTerminar),
        }
        si(f.bloques > 0, Motivo::SinCuerpo)?;
        self.fase = Fase::Fuera;
        Ok(())
    }

    /// La instruccion va dentro de un bloque abierto, y si hay una fusion
    /// pendiente, es su salto.
    fn en_bloque(&self, codigo: u16) -> Result<(), Motivo> {
        si(matches!(self.fase, Fase::Cabeza | Fase::Cuerpo), Motivo::FueraDeBloque { codigo })?;
        match self.merge {
            None => Ok(()),
            Some(op::OpLoopMerge) => {
                si(codigo == op::OpBranch || codigo == op::OpBranchConditional, Motivo::MergeFueraDeSitio)
            }
            Some(_) => si(codigo == op::OpBranchConditional, Motivo::MergeFueraDeSitio),
        }
    }

    fn cuerpo(&mut self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        match codigo {
            op::OpFunctionParameter => {
                let mut f = self.f.ok_or(Motivo::FueraDeBloque { codigo })?;
                si(self.fase == Fase::Parametros && f.vistos < f.params, Motivo::FueraDeBloque { codigo })?;
                let tf = self.def(f.tipo_fn)?;
                si(ins.op(1) == tf.op(3 + f.vistos as usize), Motivo::TipoNoCuadra { codigo })?;
                f.vistos += 1;
                self.f = Some(f);
                Ok(())
            }
            op::OpLabel => {
                let mut f = self.f.ok_or(Motivo::FueraDeBloque { codigo })?;
                match self.fase {
                    Fase::Parametros => si(
                        f.vistos == f.params,
                        Motivo::TipoNoCuadra { codigo: op::OpFunctionParameter },
                    )?,
                    Fase::EntreBloques => {}
                    _ => return Err(Motivo::BloqueSinTerminar),
                }
                f.bloques += 1;
                self.f = Some(f);
                self.bloques += 1;
                self.fase = Fase::Cabeza;
                self.merge = None;
                Ok(())
            }
            _ => {
                self.en_bloque(codigo)?;
                if codigo == op::OpPhi {
                    si(self.fase == Fase::Cabeza, Motivo::PhiFueraDeSitio)?;
                } else {
                    self.fase = Fase::Cuerpo;
                }
                self.significado(ins)?;
                match codigo {
                    op::OpBranch
                    | op::OpBranchConditional
                    | op::OpReturn
                    | op::OpReturnValue
                    | op::OpUnreachable => {
                        self.fase = Fase::EntreBloques;
                        self.merge = None;
                    }
                    op::OpLoopMerge | op::OpSelectionMerge => self.merge = Some(codigo),
                    _ => {}
                }
                Ok(())
            }
        }
    }

    /// Los tipos de una instruccion de cuerpo.
    fn significado(&self, ins: &Instr) -> Result<(), Motivo> {
        let codigo = ins.codigo;
        let no = Motivo::TipoNoCuadra { codigo };
        let r = ins.op(1);
        let n_ops = ins.palabras as usize;
        use Escalar::{Bool, Float, Int};
        match codigo {
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
                    t = self.paso(t, ins.op(k), codigo)?;
                }
                si(t == r, no)
            }
            op::OpCompositeInsert => {
                si(n_ops > 5, no)?;
                let objeto = self.valor(ins.op(3))?;
                let mut t = self.valor(ins.op(4))?;
                si(t == r, no)?;
                for k in 5..n_ops {
                    t = self.paso(t, ins.op(k), codigo)?;
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
                    si(i < na + nb || i == u32::MAX, Motivo::IndiceFuera { codigo })?;
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
                let (clase, apuntado) = self.puntero(self.valor(ins.op(1))?).ok_or(no)?;
                si(clase != CLASE_INPUT, Motivo::EscrituraEnEntrada)?;
                si(self.valor(ins.op(2))? == apuntado, no)
            }
            op::OpCopyMemory => {
                let (clase, a) = self.puntero(self.valor(ins.op(1))?).ok_or(no)?;
                let (_, b) = self.puntero(self.valor(ins.op(2))?).ok_or(no)?;
                si(clase != CLASE_INPUT, Motivo::EscrituraEnEntrada)?;
                si(a == b, no)
            }
            op::OpAccessChain | op::OpInBoundsAccessChain => {
                let (clase, mut t) = self.puntero(self.valor(ins.op(3))?).ok_or(no)?;
                for k in 4..n_ops {
                    let idx = ins.op(k);
                    si(self.num(self.valor(idx)?) == Some((Int, 1)), no)?;
                    let d = self.m.def(t).ok_or(no)?;
                    t = match d.codigo {
                        op::OpTypeVector | op::OpTypeArray | op::OpTypeRuntimeArray => d.op(2),
                        op::OpTypeStruct => {
                            let v = self.constante_entera(idx)? as usize;
                            si(v + 2 < d.palabras as usize, Motivo::IndiceFuera { codigo })?;
                            d.op(2 + v)
                        }
                        _ => return Err(Motivo::IndiceFuera { codigo }),
                    };
                }
                si(self.puntero(r) == Some((clase, t)), no)
            }
            op::OpArrayLength => {
                si(self.num(r) == Some((Int, 1)), no)?;
                let (_, s) = self.puntero(self.valor(ins.op(3))?).ok_or(no)?;
                let d = self.m.def(s).ok_or(no)?;
                let miembro = ins.op(4) as usize;
                si(d.codigo == op::OpTypeStruct && miembro + 3 == d.palabras as usize, no)?;
                let ultimo = self.m.def(d.op(2 + miembro)).map(|x| x.codigo);
                si(ultimo == Some(op::OpTypeRuntimeArray), no)
            }
            // -- funciones --
            op::OpFunctionCall => {
                let callee = self.def(ins.op(3))?;
                si(callee.codigo == op::OpFunction && callee.op(1) == r, no)?;
                let tf = self.def(callee.op(4))?;
                si(tf.palabras as usize - 3 == n_ops - 4, no)?;
                for k in 0..n_ops - 4 {
                    si(self.valor(ins.op(4 + k))? == tf.op(3 + k), no)?;
                }
                Ok(())
            }
            op::OpExtInst => {
                si(Some(ins.op(3)) == self.glsl_id, Motivo::ImportacionFuera)?;
                let numero = ins.op(4);
                let g = glsl(numero).ok_or(Motivo::ExtInstFuera { numero })?;
                si(g.grupo != GrupoGlsl::Trascendente, Motivo::ExtInstLuego { numero })?;
                si(n_ops - 5 == g.operandos as usize, no)?;
                let (e, n) = self.num(r).ok_or(no)?;
                for k in 5..n_ops {
                    let t = self.valor(ins.op(k))?;
                    match g.grupo {
                        GrupoGlsl::Flotante => si(e == Float && t == r, no)?,
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
                    si(self.sale_de_un_bucle(a) || self.sale_de_un_bucle(b), Motivo::SaltoSinEstructura)?;
                }
                Ok(())
            }
            op::OpReturn => {
                let ret = self.f.map(|f| f.ret).unwrap_or(0);
                si(self.m.def(ret).map(|d| d.codigo) == Some(op::OpTypeVoid), no)
            }
            op::OpReturnValue => {
                let ret = self.f.map(|f| f.ret).unwrap_or(0);
                si(self.valor(ins.op(1))? == ret, no)
            }
            op::OpUnreachable => Ok(()),
            _ => Err(Motivo::InstruccionFuera { codigo, porque: "instruccion del nucleo que el juez aun no mira" }),
        }
    }

    /// `destino` es la salida o la continuacion de algun bucle de esta funcion.
    fn sale_de_un_bucle(&self, destino: u32) -> bool {
        let Some(f) = self.f else { return false };
        self.m
            .recorrer_desde(f.ini)
            .take_while(|i| i.desde < f.fin)
            .any(|i| i.codigo == op::OpLoopMerge && (i.op(1) == destino || i.op(2) == destino))
    }
}
