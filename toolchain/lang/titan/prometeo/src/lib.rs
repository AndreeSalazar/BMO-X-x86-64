//! # bmo-titan-prometeo -- PROMETEO de TITAN++: el emisor de GPU, para cualquier tarjeta
//!
//! Nivel 11 de TITAN++. Nacio como `bmo-titan-sm86` el 07-10, cuando el
//! propietario quito el SPIR-V (*"gpu - sm86 - luego el juez verifica
//! estrictamente y el 3060"*). Desde LB3 de `docs/plan/PLAN_LAS_LIBRERIAS.md`
//! (08-10) es la mitad de arriba de PROMETEO, la libreria general de la GPU:
//! la *"gpu general"* del propietario, *"que se llevara todo el emisor de GPU
//! para que aplique"*. **No nombra ninguna tarjeta**: las pide por el
//! contrato (`bmo_prometeo::Tarjeta`), y solo quien arma la herramienta
//! (`titan`) dice cuales hay -- hoy, la 3060. La mitad de abajo (el Programa y
//! el contrato) es `platform/shared/prometeo`.
//!
//! ```text
//!    la IR de una gpu fn   (bmo-titan-front: ya juzgada, ya calculada)
//!         |  programa()
//!         v
//!    el PROGRAMA de la casa   el mismo que sale de los DXIL y los SM5 de
//!         |                   PROTON-X, y el que corre el interprete
//!         |
//!         |  CADA tarjeta, AISLADA por completo (el propietario: "TODAS LAS
//!         |  GPU en emisor SON AISLADAS por completo luego el JUEZ procesa
//!         |  cada uno") --
//!         v
//!    su EMISOR      write(): el Programa a SU codigo, para su oraculo y
//!         |         para el viaje (la 3060: el MISMO emisor que usa
//!         |         Cyberpunk, E3..E6)
//!    su JUEZ        judge(): ESTRICTO, sobre lo que su emisor escribio
//!         |
//!    su SIMULADOR   run(): una celda por hilo
//!         v
//!    el ORACULO     las celdas que lleva el .bex
//!
//!    ... y el SUPREMO JUEZ no esta aqui: esta en la puerta de la GPU final
//!    (el kernel), y es el mismo juez de esa tarjeta, otra vez (LB8).
//! ```
//!
//! *** POR QUE SIN SPIR-V: el emisor de la 3060 que existe lee el `Programa`
//! (E3, 28-09). Con un escritor de SPIR-V aparte, cada cosa que aprendiera
//! TITAN++ (bucles, vecinos, computo) se aprenderia DOS veces, y lo que
//! abriera ILLAPA no le serviria a Cyberpunk. Con el Programa, es una vez.
//!
//! ** TRES RESPUESTAS PARA CADA CELDA, en cada build y en CADA tarjeta: su
//! simulador (sobre su codigo), el interprete de la casa (sobre el Programa)
//! y el calculo de TITAN++ (sobre la IR). Si dos no dan los mismos bits (o
//! los dos NaN), no hay `.bex`, y el NO dice la entrada y las respuestas.
//!
//! ** SIN SALTOS, como hacia el escritor de SPIR-V: cada `if` es un `Elige`
//! (una gpu fn es pura y sin bucles; los dos lados dan el mismo resultado
//! bit a bit). Los saltos de verdad (E6) llegaran con los bucles (IL1, LB5).
//!
//! > **08-10, LB5:** llegaron. Cada `for` de una gpu fn (con su N escrito,
//! > DL4) es un `Bucle` de verdad -- `RomperSi` en su cabeza y en cada
//! > `break`, `FinBucle` en su paso --, y lo que cruza la vuelta vive en su
//! > CASA, un registro que cada asignacion escribe con su predicado (ver
//! > `straight`). Los `if` siguen en linea recta, tambien dentro de un
//! > bucle; una gpu fn sin bucles sale como antes, byte a byte.
//!
//! > **08-10, a mitad de LB6:** y DIBUJA. Una gpu fn de VERTICE o de PIXEL
//! > (por su firma: `Forma`) sale como el Programa de la casa por elementos
//! > y componentes, el mismo que sale de los DXIL de PROTON-X; cada tarjeta
//! > la escribe, la juzga y la corre como a una de celdas, y la RTX 3060 12G
//! > ademas la PEGA a su tuberia y la mete en su sobre (`dibujo` de su
//! > crate). Y el escritor se parte en ficheros (el propietario: *"que
//! > administre archivos multiples bien organizado"*):
//! >
//! > ```text
//! >    lib.rs      las tarjetas, sus jueces y el oraculo: CADA tarjeta
//! >                escribe, juzga y corre; la bateria; las tres respuestas
//! >    escribe.rs  la gpu fn hecha Programa: el cuerpo, sus `if`, sus
//! >                bucles, sus llamadas en linea -- para todas las tarjetas
//! >    dibujo.rs   la que DIBUJA: sus elementos y su oraculo
//! > ```
//!
//! ** LOS BOOL, como D3D: dentro del Programa un bool es 0xFFFFFFFF o 0; en
//! las celdas, 1 o 0 (lo del calculo). Se convierte al entrar y al salir.
//!
//! ** LA DIVISION: una division entre una POTENCIA DE DOS es exacta como
//! multiplicacion por su inverso (`x / 2.0` y `x * 0.5` son el mismo numero
//! real, redondeado igual), y se escribe asi para cualquier tarjeta. La
//! general llega a la tarjeta como `Div`; la que no la hace exacta lo dice
//! como su LIMITE (la 3060: LI2g de `PLAN_EL_LIBRETO.md`), y es el NO del
//! programa, en su linea y su columna (LB1).

use bmo_prometeo::programa::Op;
use bmo_prometeo::{Codigo, NoEmite, Para, Programa, Tarjeta};
use bmo_titan_front::calc::DeviceNo;
use bmo_titan_front::ir::{Function, Module};
use bmo_titan_front::{Code, Message};
use std::collections::HashMap;

mod dibujo;
mod escribe;

pub use bmo_titan_front::gpu::Forma;
pub use dibujo::{Celda, Dibujo};
pub use escribe::inverso_exacto;

/// El oraculo de lo que DIBUJA (LB6): su bateria, y sus celdas por la tarjeta
/// y por la casa.
pub mod dibuja {
    pub use crate::dibujo::{bateria, run, run_casa};
}

/// Lo que un valor ES dentro de una gpu fn: solo hay dos clases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    F32,
    Bool,
}

/// Un NO del escritor (no del programa: el frontend ya lo juzgo) -- salvo
/// cuando lleva su `Message`: entonces es lo que una tarjeta TODAVIA NO SABE
/// hacer, un limite dicho a proposito (la division general en la 3060, LI2g),
/// y es un NO del PROGRAMA, con su sitio en el modulo (LB1 de
/// `docs/plan/PLAN_LAS_LIBRERIAS.md`). El texto (`.0`) es el de siempre.
#[derive(Debug)]
pub struct Failure(pub String, pub Option<Message>);

impl Failure {
    /// Un fallo del escritor: nunca del programa.
    fn writer(why: String) -> Failure {
        Failure(why, None)
    }

    /// Lo que se le dice al calculo o al build: el limite como el NO del
    /// programa, o el fallo con el nombre de la gpu fn delante.
    pub fn no(self, name: &str) -> DeviceNo {
        match self.1 {
            Some(said) => DeviceNo::Limit(said),
            None => DeviceNo::Failure(format!("`gpu fn {}`: {}", name, self.0)),
        }
    }
}

/// ** LO QUE TITAN++ DICE DESPUES DEL LIMITE DE UNA TARJETA: el QUE y el POR
/// QUE son de la tarjeta (sus palabras); esto es lo de TITAN++, sea cual sea. La
/// division tiene su como: la de una potencia de dos ya llega como
/// multiplicacion (`inverso_exacto`).
const LIMITE_TAMBIEN: &str = "y una gpu fn da los MISMOS bits por su tarjeta, la casa y el calculo (L29)";
const DIVISION_HOY: &str = "Hoy solo divide entre una potencia de dos, que es una multiplicacion exacta; la general espera a LI2g de PLAN_EL_LIBRETO (DL10 de PLAN_LAS_LIBRERIAS)";
const DIVISION_COMO: &str = "entre una potencia de dos se escribe igual (x / 2.0, x / 0.25); las demas, todavia no";
const LIMITE_COMO: &str = "escribela con otras operaciones, o espera a que su tarjeta la sepa hacer";

/// **Una gpu fn escrita para UNA tarjeta**: su Programa, su codigo para el
/// oraculo y para el viaje, de donde salio, y quien lo escribio.
pub struct Kernel<'t> {
    pub name: String,
    /// El fichero del paquete donde esta la gpu fn, y su linea alli.
    pub file: String,
    pub line: usize,
    /// La clase de cada valor (la entrada `k` del Programa), y la del resultado.
    pub params: Vec<Kind>,
    pub ret: Kind,
    pub programa: Programa,
    /// El sitio del `.titan` de cada operacion del Programa.
    pub donde: Vec<(usize, usize)>,
    /// La tarjeta que lo escribio: la UNICA que lo juzga y lo simula.
    pub tarjeta: &'t dyn Tarjeta,
    /// Lo que corre su simulador (la 3060: entradas en c[1]).
    pub oraculo: Codigo,
    /// Lo que viajaria a la tarjeta (la 3060: entradas ya en registros).
    pub viaje: Codigo,
    /// ** LB6: lo que DIBUJA, si es de vertice o de pixel (`params` y `ret`
    /// son de las celdas: aqui, vacios).
    pub dibujo: Option<Dibujo>,
}

impl Kernel<'_> {
    /// El codigo que viaja, en bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.viaje.bytes
    }
}

/// **Todas las gpu fn de un modulo, en CADA tarjeta de `tarjetas`**,
/// escritas, juzgadas y comprobadas: lo que `titan build` pide antes de
/// escribir un `.bex`. Un NO aqui es del ESCRITOR o del juez, no del
/// programa -- salvo un `DeviceNo::Limit`: lo que una tarjeta todavia no
/// sabe, y eso SI es el NO del programa, en su sitio.
pub fn kernels<'t>(m: &Module, tarjetas: &[&'t dyn Tarjeta]) -> Result<Vec<Kernel<'t>>, DeviceNo> {
    let mut out = Vec::new();
    for (i, f) in m.functions.iter().enumerate().filter(|(_, f)| f.gpu) {
        if tarjetas.is_empty() {
            return Err(DeviceNo::Failure(format!("`gpu fn {}`: {}", f.name, SIN_TARJETAS)));
        }
        for t in tarjetas {
            let k = write(m, i, *t).map_err(|e| e.no(&f.name))?;
            judge(&k).map_err(DeviceNo::Failure)?;
            verify(m, i, &k).map_err(DeviceNo::Failure)?;
            out.push(k);
        }
    }
    Ok(out)
}

/// Lo que se dice si quien arma la herramienta no dio ninguna tarjeta.
const SIN_TARJETAS: &str = "no hay ninguna tarjeta: quien arma la herramienta no dio ninguna libreria de GPU, y una gpu fn no se escribe a ciegas";

/// La clase de un valor de una gpu fn de celdas: f32 o bool.
pub(crate) fn kind_of(t: &bmo_titan_front::tree::Ty) -> Result<Kind, Failure> {
    match t {
        bmo_titan_front::tree::Ty::F32 => Ok(Kind::F32),
        bmo_titan_front::tree::Ty::Bool => Ok(Kind::Bool),
        other => Err(Failure::writer(format!("una gpu fn con un `{}`: el frontend (gpu.rs) tenia que haberlo dicho", other.name()))),
    }
}

/// ** LA FORMA de la gpu fn `func` de `m`, por su firma (LB6): de celdas, de
/// vertice o de pixel -- la misma cuenta que el frontend (`gpu::forma`).
pub fn forma(m: &Module, func: usize) -> Forma {
    let f = &m.functions[func];
    let tys: Vec<&bmo_titan_front::tree::Ty> = f.params.iter().map(|(_, t)| t).collect();
    bmo_titan_front::gpu::forma(&m.types, &tys, f.ret.as_ref())
}

/// La gpu fn `func` de `m`, hecha el Programa de la casa, por su forma: el
/// Programa, el sitio de cada operacion, y lo que dibuja si dibuja.
fn escribir(m: &Module, func: usize) -> Result<(Programa, Vec<(usize, usize)>, Option<Dibujo>), Failure> {
    let f: &Function = &m.functions[func];
    if !f.gpu {
        return Err(Failure::writer(format!("`{}` no es una gpu fn", f.name)));
    }
    match forma(m, func) {
        Forma::Celda => escribe::celda(m, func).map(|(p, d)| (p, d, None)),
        forma => dibujo::programa(m, func, forma).map(|(p, d, x)| (p, d, Some(x))),
    }
}

/// **La gpu fn `func` de `m`, hecha el Programa de la casa.** Devuelve el
/// Programa y el sitio del `.titan` de cada una de sus operaciones. No sabe
/// de tarjetas: es lo mismo para todas.
pub fn programa(m: &Module, func: usize) -> Result<(Programa, Vec<(usize, usize)>), Failure> {
    escribir(m, func).map(|(p, d, _)| (p, d))
}

// ---- a la tarjeta, y su juez ----------------------------------------------------------

/// El sitio del `.titan` de una gpu fn: (fichero, linea de su `gpu fn`).
fn sitio(m: &Module, f: &Function) -> (String, usize) {
    let (file_k, fn_line) = m.sources.place(f.line).unwrap_or((0, f.line));
    let file = if m.sources.0.is_empty() { "main.titan".to_string() } else { m.sources.path(file_k).to_string() };
    (file, fn_line)
}

/// **Escribe la gpu fn `func` de `m` para `tarjeta`**: el Programa, y su
/// codigo en esa tarjeta para su oraculo y para el viaje. Lo que la tarjeta
/// no sabe se dice en el `.titan` -- fichero, linea y columna --: un LIMITE
/// suyo es el NO del programa; un fallo de su emisor, un fallo.
pub fn write<'t>(m: &Module, func: usize, tarjeta: &'t dyn Tarjeta) -> Result<Kernel<'t>, Failure> {
    let f = &m.functions[func];
    let (programa, donde, dibujo) = escribir(m, func)?;
    let (params, ret) = match &dibujo {
        // Lo que dibuja no tiene celdas: sus elementos los dice `dibujo`.
        Some(_) => (Vec::new(), Kind::F32),
        None => (f.params.iter().map(|(_, t)| kind_of(t)).collect::<Result<_, _>>()?, kind_of(f.ret.as_ref().expect("escribir() lo miro"))?),
    };
    let (file, line) = sitio(m, f);
    let en = |para| {
        tarjeta.emitir(&programa, para).map_err(|e| {
            // DONDE, en las lineas del MODULO (las del `at` de la IR: el
            // paquete entero seguido), o la de la gpu fn; el texto dice la
            // del fichero (08-10: antes salia la del modulo con el nombre del
            // fichero, que solo cuadra en la raiz).
            let op = match &e {
                NoEmite::Limite { op, .. } => Some(*op),
                NoEmite::Fallo { op, .. } => *op,
            };
            let at = op.and_then(|i| donde.get(i).copied()).unwrap_or((f.line, 1));
            let (l, c) = (m.sources.place(at.0).map(|(_, l)| l).unwrap_or(at.0), at.1);
            match e {
                NoEmite::Limite { op, que, por_que } => {
                    // ** UN LIMITE, no un fallo: lo que la tarjeta todavia no
                    // sabe, dicho a proposito. Es el NO del programa.
                    let division = matches!(programa.ops.get(op), Some(Op::Div { .. }));
                    let why = if division { format!("{}, {}. {}", por_que, LIMITE_TAMBIEN, DIVISION_HOY) } else { format!("{}, {}", por_que, LIMITE_TAMBIEN) };
                    let said = Message::new(Code::GpuBody, at.0, at.1, &que, &why, if division { DIVISION_COMO } else { LIMITE_COMO });
                    Failure(format!("{}, linea {}, columna {} (gpu fn `{}`): {}", file, l, c, f.name, why), Some(said))
                }
                NoEmite::Fallo { por_que, .. } => Failure::writer(format!("{}, linea {}, columna {} (gpu fn `{}`): {}", file, l, c, f.name, por_que)),
            }
        })
    };
    let oraculo = en(Para::Oraculo)?;
    let viaje = en(Para::Viaje)?;
    Ok(Kernel { name: f.name.clone(), file, line, params, ret, programa, donde, tarjeta, oraculo, viaje, dibujo })
}

/// **SU juez, ESTRICTO**, sobre los dos codigos: el de la tarjeta que lo
/// escribio y ningun otro (la 3060: las esperas y las barreras de Ampere, los
/// registros, los saltos, y la regla de un cuerpo de app). Su NO, en sus
/// palabras y en la linea de la gpu fn.
pub fn judge(k: &Kernel) -> Result<(), String> {
    let no = |por: String| format!("{}, linea {} (gpu fn `{}`): el juez de {} dijo que no -- {}", k.file, k.line, k.name, k.tarjeta.ficha().nombre, por);
    k.tarjeta.juzgar(&k.oraculo, Para::Oraculo).map_err(&no)?;
    k.tarjeta.juzgar(&k.viaje, Para::Viaje).map_err(&no)
}

// ---- el oraculo: el simulador de la tarjeta, contra la casa y el calculo --------------

/// La celda `i` de cada valor, como la entrada del Programa (componente 0),
/// en bits.
fn entradas(cells: &[Vec<u32>], i: usize) -> Vec<[u32; 4]> {
    cells.iter().map(|c| [c[i], 0, 0, 0]).collect()
}

/// **El ORACULO**: el codigo de la tarjeta, corrido por SU simulador, un hilo
/// por celda. `cells[k]` son las celdas del valor `k` (sus bits).
pub fn run(k: &Kernel, cells: &[Vec<u32>]) -> Result<Vec<u32>, String> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let s = k.tarjeta.simular(&k.oraculo, &entradas(cells, i), 1)?;
        let first = s.first().ok_or_else(|| format!("el simulador de {} no devolvio la salida de `{}`", k.tarjeta.ficha().nombre, k.name))?;
        out.push(first[0]);
    }
    Ok(out)
}

/// La misma gpu fn por el INTERPRETE de la casa, sobre el Programa.
pub fn run_casa(k: &Kernel, cells: &[Vec<u32>]) -> Vec<u32> {
    let n = cells.first().map(|c| c.len()).unwrap_or(0);
    let mut regs = Vec::new();
    (0..n)
        .map(|i| {
            let e: Vec<[f32; 4]> = entradas(cells, i).iter().map(|c| c.map(f32::from_bits)).collect();
            let mut s = [[0.0f32; 4]; 1];
            k.programa.correr(&e, &[], &mut s, &mut regs);
            s[0][0].to_bits()
        })
        .collect()
}

/// ** SI ESTA BIEN: las celdas que mas fallan en una cuenta de coma flotante.
const BORDES: [f32; 18] = [
    0.0, -0.0,                       // el signo del cero: 1/0 y 1/-0 no son lo mismo
    1.0, -1.0, 0.5, -2.5, 3.0,       // los de todos los dias
    0.1,                             // el que no cabe en base 2
    1.0e-38, f32::MIN_POSITIVE,      // los normales mas chicos
    1.0e-45, -1.0e-45,               // los subnormales: donde se pierde precision
    f32::MAX, f32::MIN, 1.0e30,      // los que desbordan al sumar
    f32::NAN, f32::INFINITY, f32::NEG_INFINITY,
];

/// Las entradas de la bateria: el producto entero si cabe en 4096 casos; si
/// no, cada valor recorre los bordes a su propio paso.
pub fn battery(params: &[Kind]) -> Vec<Vec<u32>> {
    battery_de(params, 0)
}

/// ** LO QUE CORRE LA BATERIA, como mucho (LB5): sus celdas por la OBRA de
/// una (la de `gpu.rs`, que con cada `range` escrito es la misma en todas).
/// Una gpu fn sin bucles (obra de decenas) se prueba como siempre, hasta
/// 4096 casos; una en el tope de una celda (65536), en 18 -- cada valor
/// sigue pasando por sus 18 bordes --, y su bateria no tarda mucho mas que
/// la de las otras.
pub const BATERIA_OBRA: u64 = 1 << 20;

/// La bateria de una gpu fn de esta `obra`: el producto entero si cabe; si
/// no, cada valor recorre los bordes a su propio paso, en tantas celdas como
/// deje [`BATERIA_OBRA`] (nunca menos que los bordes).
pub fn battery_de(params: &[Kind], obra: u64) -> Vec<Vec<u32>> {
    let column = |k: Kind| -> Vec<u32> { if k == Kind::F32 { BORDES.iter().map(|x| x.to_bits()).collect() } else { vec![0, 1] } };
    let columns: Vec<Vec<u32>> = params.iter().map(|k| column(*k)).collect();
    let total: usize = columns.iter().map(|c| c.len()).product();
    let tope = (BATERIA_OBRA / obra.max(1)).clamp(BORDES.len() as u64, 4096) as usize;
    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); params.len()];
    if total <= tope {
        for i in 0..total {
            let mut rest = i;
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[rest % c.len()]);
                rest /= c.len();
            }
        }
    } else {
        for i in 0..tope {
            for (j, c) in columns.iter().enumerate() {
                cells[j].push(c[(i * (2 * j + 1) + j) % c.len()]);
            }
        }
    }
    cells
}

/// Los mismos bits, o los dos NaN: una tarjeta no promete la carga de un NaN.
pub(crate) fn same_cell(a: u32, b: u32, k: Kind) -> bool {
    a == b || (k == Kind::F32 && f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
}

/// **La comparacion de cada build**: estas celdas por la tarjeta (su
/// simulador), por la casa y por el calculo; la primera que no da lo mismo,
/// dicha con la entrada y las tres respuestas.
fn compare(m: &Module, func: usize, k: &Kernel, cells: &[Vec<u32>], what: &str) -> Result<Vec<u32>, String> {
    let suya = run(k, cells)?;
    let casa = run_casa(k, cells);
    let calc = bmo_titan_front::calc::run_gpu(m, func, cells).map_err(|e| format!("el calculo: {}", e.what))?;
    for i in 0..suya.len() {
        if !same_cell(suya[i], calc[i], k.ret) || !same_cell(casa[i], calc[i], k.ret) {
            let show = |bits: u32, kind: Kind| if kind == Kind::F32 { format!("{:?}", f32::from_bits(bits)) } else { (bits != 0).to_string() };
            let input: Vec<String> = cells.iter().zip(&k.params).map(|(c, kind)| show(c[i], *kind)).collect();
            return Err(format!(
                "{}, linea {} (gpu fn `{}`): con {} ({}) {} da {}, la casa {} y el calculo {} -- una cuenta, varias respuestas: no hay .bex",
                k.file, k.line, k.name, what, input.join(", "), k.tarjeta.ficha().nombre, show(suya[i], k.ret), show(casa[i], k.ret), show(calc[i], k.ret)
            ));
        }
    }
    Ok(suya)
}

/// **La bateria de bordes** de una gpu fn, por los tres: sus celdas, o (LB6)
/// los elementos de lo que dibuja.
pub fn verify(m: &Module, func: usize, k: &Kernel) -> Result<usize, String> {
    if let Some(d) = &k.dibujo {
        return dibujo::verify(m, func, k, d);
    }
    let cells = battery_de(&k.params, m.functions[func].obra);
    compare(m, func, k, &cells, "la bateria de bordes")?;
    Ok(cells.first().map(|c| c.len()).unwrap_or(0))
}

/// ** EL ORACULO COMO `Device` DEL CALCULO: cada gpu fn se escribe UNA vez en
/// cada tarjeta, se juzga, pasa la bateria, y sus celdas las calculan las
/// tarjetas (las mismas en todas, o no hay `.bex`). Es lo que `titan build`
/// le da al calculo: los resultados que lleva el `.bex` son los del codigo
/// que viajaria a la tarjeta (las de la primera).
pub struct Oracle<'t> {
    tarjetas: Vec<&'t dyn Tarjeta>,
    written: HashMap<usize, Vec<Kernel<'t>>>,
}

impl<'t> Oracle<'t> {
    /// El oraculo, con las tarjetas que diga quien arma la herramienta.
    pub fn new(tarjetas: &[&'t dyn Tarjeta]) -> Self {
        Oracle { tarjetas: tarjetas.to_vec(), written: HashMap::new() }
    }

    /// Cuantas gpu fn escribio (y juzgo) hasta ahora.
    pub fn written(&self) -> usize {
        self.written.len()
    }
}

impl bmo_titan_front::calc::Device for Oracle<'_> {
    fn run(&mut self, m: &Module, func: usize, cells: Vec<Vec<u32>>) -> Result<Vec<u32>, DeviceNo> {
        if !self.written.contains_key(&func) {
            if self.tarjetas.is_empty() {
                return Err(DeviceNo::Failure(SIN_TARJETAS.to_string()));
            }
            let mut ks = Vec::with_capacity(self.tarjetas.len());
            for t in &self.tarjetas {
                // Un limite de la tarjeta es el NO del programa; lo demas, un fallo.
                let k = write(m, func, *t).map_err(|e| match e.1 {
                    Some(said) => DeviceNo::Limit(said),
                    None => DeviceNo::Failure(e.0),
                })?;
                judge(&k).map_err(DeviceNo::Failure)?;
                verify(m, func, &k).map_err(DeviceNo::Failure)?;
                ks.push(k);
            }
            self.written.insert(func, ks);
        }
        // Las celdas REALES del programa, por cada tarjeta, la casa y el calculo.
        let mut first = None;
        for k in &self.written[&func] {
            if k.dibujo.is_some() {
                // El calculo no la llama (`gpu_call` lo dice): esto es la red.
                return Err(DeviceNo::Failure(format!("`gpu fn {}` dibuja: no se llama, la pone a dibujar VERRANO (LB7)", k.name)));
            }
            let celdas = compare(m, func, k, &cells, "las celdas del programa").map_err(DeviceNo::Failure)?;
            first.get_or_insert(celdas);
        }
        Ok(first.expect("al menos una tarjeta: se miro al escribir"))
    }
}

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_bucles;
#[cfg(test)]
mod pruebas_dibujo;
