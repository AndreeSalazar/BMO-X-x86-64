//! **Las excepciones estructuradas de Windows x64: lo que se DECIDE** (P4c,
//! 28-09).
//!
//! Lo que hace el despachador de Windows con una excepcion, dicho sin
//! punteros. La casa (`proton-x-casa/src/excepciones.rs`) pone la pila de
//! verdad, llama a los filtros y salta; aqui solo hay cuentas:
//!
//! ```text
//!    imagen_en        las cabeceras de una imagen YA en memoria: su medida y
//!                     su `.pdata` (el directorio 3)
//!    subir            un marco hacia quien llamo: con su RUNTIME_FUNCTION
//!                     (desenrollar::un_marco) o, sin ella, la regla de la
//!                     HOJA (rip = [rsp], rsp += 8); fuera de toda imagen, se
//!                     acabo la pila que se conoce
//!    ambitos          la SCOPE_TABLE que el compilador deja tras el
//!                     manejador `__C_specific_handler` (un `__try` por fila,
//!                     el de dentro primero)
//!    al_buscar        que `__except` mirar ahora (primera pasada)
//!    al_desenrollar   que `__finally` correr, o si ya se llego (segunda)
//!    Vectores         AddVectoredExceptionHandler: los que miran ANTES que
//!                     ningun marco, en su orden
//! ```
//!
//! Las formas (EXCEPTION_RECORD, DISPATCHER_CONTEXT, SCOPE_TABLE) son las de la
//! documentacion de Microsoft ("x64 exception handling") y las cabeceras
//! publicas de su SDK; las DOS pasadas tambien: la primera BUSCA quien la coge
//! (sin tocar la pila), la segunda DESENROLLA hasta el (corriendo los
//! `__finally` de en medio).

use alloc::vec::Vec;

use crate::desenrollar::{self, Contexto, Funcion, Marco, Memoria, NoDesenrolla, RSP};

// -- EXCEPTION_RECORD -------------------------------------------------------------

/// Lo que mide un EXCEPTION_RECORD de x64.
pub const REGISTRO_BYTES: usize = 0x98;
/// Cuantos parametros caben en `ExceptionInformation`.
pub const MAX_PARAMETROS: usize = 15;

/// No se puede seguir donde se paro (RaiseException con esta bandera).
pub const EXCEPTION_NONCONTINUABLE: u32 = 0x01;
/// Segunda pasada: se esta desenrollando.
pub const EXCEPTION_UNWINDING: u32 = 0x02;
/// Desenrollando, y ESTE marco es el destino.
pub const EXCEPTION_TARGET_UNWIND: u32 = 0x20;

const REG_CODIGO: usize = 0x00;
const REG_BANDERAS: usize = 0x04;
const REG_ANIDADO: usize = 0x08;
const REG_DIRECCION: usize = 0x10;
const REG_N: usize = 0x18;
const REG_INFO: usize = 0x20;

/// Un EXCEPTION_RECORD.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registro {
    pub codigo: u32,
    pub banderas: u32,
    /// Otro EXCEPTION_RECORD (una excepcion dentro de otra), o 0.
    pub anidado: u64,
    pub direccion: u64,
    /// Hasta 15; los de mas no caben y no se escriben.
    pub parametros: Vec<u64>,
}

fn pon32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn pon64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

fn lee32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap_or([0; 4]))
}

fn lee64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]))
}

impl Registro {
    /// En la forma de Windows (`b` mide al menos [`REGISTRO_BYTES`]).
    pub fn a_bytes(&self, b: &mut [u8]) {
        b[..REGISTRO_BYTES].fill(0);
        let n = self.parametros.len().min(MAX_PARAMETROS);
        pon32(b, REG_CODIGO, self.codigo);
        pon32(b, REG_BANDERAS, self.banderas);
        pon64(b, REG_ANIDADO, self.anidado);
        pon64(b, REG_DIRECCION, self.direccion);
        pon32(b, REG_N, n as u32);
        for (i, p) in self.parametros.iter().take(n).enumerate() {
            pon64(b, REG_INFO + 8 * i, *p);
        }
    }

    pub fn de_bytes(b: &[u8]) -> Registro {
        let n = (lee32(b, REG_N) as usize).min(MAX_PARAMETROS);
        Registro {
            codigo: lee32(b, REG_CODIGO),
            banderas: lee32(b, REG_BANDERAS),
            anidado: lee64(b, REG_ANIDADO),
            direccion: lee64(b, REG_DIRECCION),
            parametros: (0..n).map(|i| lee64(b, REG_INFO + 8 * i)).collect(),
        }
    }
}

/// Donde van las banderas dentro de un EXCEPTION_RECORD (se cambian en su
/// sitio al pasar a la segunda pasada).
pub const REGISTRO_BANDERAS: usize = REG_BANDERAS;

// -- DISPATCHER_CONTEXT -----------------------------------------------------------

/// Lo que mide un DISPATCHER_CONTEXT de x64.
pub const DESPACHO_BYTES: usize = 0x50;
/// Donde va `ScopeIndex`: el manejador lo sube antes de correr un `__finally`.
pub const DESPACHO_INDICE: usize = 0x48;

/// Lo que el despachador le cuenta al manejador de un marco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Despacho {
    /// ControlPc: donde esta el marco (su rip).
    pub pc: u64,
    pub base_imagen: u64,
    /// La RUNTIME_FUNCTION (su direccion).
    pub funcion: u64,
    pub establecido: u64,
    /// TargetIp: adonde se desenrolla (segunda pasada).
    pub destino: u64,
    /// El CONTEXT de la excepcion (la direccion).
    pub contexto: u64,
    pub manejador: u64,
    /// HandlerData: lo que el compilador dejo tras el manejador.
    pub datos: u64,
    pub historia: u64,
    /// ScopeIndex: por que fila de la tabla de ambitos se va.
    pub indice: u32,
}

impl Despacho {
    pub fn a_bytes(&self, b: &mut [u8]) {
        b[..DESPACHO_BYTES].fill(0);
        for (i, v) in [self.pc, self.base_imagen, self.funcion, self.establecido, self.destino, self.contexto, self.manejador, self.datos, self.historia].iter().enumerate() {
            pon64(b, 8 * i, *v);
        }
        pon32(b, DESPACHO_INDICE, self.indice);
    }

    pub fn de_bytes(b: &[u8]) -> Despacho {
        Despacho {
            pc: lee64(b, 0x00),
            base_imagen: lee64(b, 0x08),
            funcion: lee64(b, 0x10),
            establecido: lee64(b, 0x18),
            destino: lee64(b, 0x20),
            contexto: lee64(b, 0x28),
            manejador: lee64(b, 0x30),
            datos: lee64(b, 0x38),
            historia: lee64(b, 0x40),
            indice: lee32(b, DESPACHO_INDICE),
        }
    }
}

// -- Las imagenes, y subir por la pila ---------------------------------------------

/// Una imagen en memoria y su `.pdata`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imagen {
    pub base: u64,
    pub tam: u32,
    pub pdata: u32,
    pub pdata_tam: u32,
}

impl Imagen {
    pub fn tiene(&self, d: u64) -> bool {
        d >= self.base && d - self.base < self.tam as u64
    }
}

const PE32_MAS: u16 = 0x20B;
const OPT_TAM_IMAGEN: u64 = 56;
const OPT_N_DIRECTORIOS: u64 = 108;
const OPT_DIRECTORIOS: u64 = 112;
const DIR_EXCEPCIONES: u32 = 3;

/// **Las cabeceras de una imagen ya colocada** (`colocar` las copia y el
/// cargador de Windows tambien): su medida y su `.pdata`. `None` si en `base`
/// no hay un PE32+.
pub fn imagen_en(m: &dyn Memoria, base: u64) -> Option<Imagen> {
    if m.u16_en(base)? != 0x5A4D {
        return None;
    }
    let pe = base + m.u32_en(base + 0x3C)? as u64;
    if m.u32_en(pe)? != 0x0000_4550 {
        return None;
    }
    let opt = pe + 24;
    if m.u16_en(opt)? != PE32_MAS {
        return None;
    }
    let tam = m.u32_en(opt + OPT_TAM_IMAGEN)?;
    let (pdata, pdata_tam) = if m.u32_en(opt + OPT_N_DIRECTORIOS)? > DIR_EXCEPCIONES {
        let d = opt + OPT_DIRECTORIOS + 8 * DIR_EXCEPCIONES as u64;
        (m.u32_en(d)?, m.u32_en(d + 4)?)
    } else {
        (0, 0)
    };
    Some(Imagen { base, tam, pdata, pdata_tam })
}

/// La imagen que tiene `pc`.
pub fn imagen_de(imagenes: &[Imagen], pc: u64) -> Option<Imagen> {
    imagenes.iter().copied().find(|i| i.tiene(pc))
}

/// **`RtlLookupFunctionEntry`**: la RUNTIME_FUNCTION de `pc` en su imagen.
/// `None` tambien si esta en una imagen pero en una funcion HOJA (sin marco).
pub fn funcion_de(m: &dyn Memoria, im: &Imagen, pc: u64) -> Option<Funcion> {
    if im.pdata_tam == 0 || !im.tiene(pc) {
        return None;
    }
    desenrollar::buscar(m, im.base, im.pdata, im.pdata_tam, (pc - im.base) as u32)
}

/// Lo que hay un marco mas arriba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subida {
    /// El pc no es de ninguna imagen conocida: la pila de Windows se acaba
    /// aqui (quien llamo a la entrada, o un marco de la casa). El contexto no
    /// cambia.
    Fuera,
    /// Una funcion hoja: sin RUNTIME_FUNCTION, su vuelta esta en [rsp].
    Hoja,
    /// Un marco con su RUNTIME_FUNCTION, ya desenrollado. `pc` es donde
    /// estaba; `marco`, su establisher y su manejador (si lo pidio `tipo`).
    Marco { pc: u64, imagen: Imagen, funcion: Funcion, marco: Marco },
}

/// **Un marco hacia arriba**: el contexto `c` pasa a ser el de quien llamo.
/// `tipo`: UNW_FLAG_EHANDLER al buscar, UNW_FLAG_UHANDLER al desenrollar.
pub fn subir(m: &dyn Memoria, imagenes: &[Imagen], c: &mut Contexto, tipo: u8) -> Result<Subida, NoDesenrolla> {
    let (pc, rsp) = (c.rip, c.gp[RSP]);
    let Some(im) = imagen_de(imagenes, pc) else { return Ok(Subida::Fuera) };
    let s = match funcion_de(m, &im, pc) {
        None => {
            c.rip = m.u64_en(rsp).ok_or(NoDesenrolla::Memoria(rsp))?;
            c.gp[RSP] = rsp + 8;
            Subida::Hoja
        }
        Some(f) => {
            let marco = desenrollar::un_marco(m, im.base, &f, c, tipo)?;
            Subida::Marco { pc, imagen: im, funcion: f, marco }
        }
    };
    if c.gp[RSP] <= rsp {
        return Err(NoDesenrolla::NoSube(c.gp[RSP]));
    }
    Ok(s)
}

// -- __C_specific_handler: la tabla de ambitos -------------------------------------

/// Una fila de la SCOPE_TABLE (RVAs): un `__try`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ambito {
    pub inicio: u32,
    pub fin: u32,
    /// El filtro de un `__except` (o [`FILTRO_EJECUTAR`]), o el bloque de un
    /// `__finally`.
    pub manejador: u32,
    /// Donde sigue un `__except` (su bloque); 0 si es un `__finally`.
    pub destino: u32,
}

/// Un filtro que es la constante EXCEPTION_EXECUTE_HANDLER: el compilador no
/// escribe una funcion, pone un 1.
pub const FILTRO_EJECUTAR: u32 = 1;

/// Mas filas que esto no es una tabla: es basura.
const MAX_AMBITOS: u32 = 4096;

impl Ambito {
    pub fn cubre(&self, rva: u32) -> bool {
        self.inicio <= rva && rva < self.fin
    }
}

/// **La SCOPE_TABLE** en `datos` (el HandlerData de un marco con
/// `__C_specific_handler`): un u32 con cuantas, y cuatro u32 por fila.
pub fn ambitos(m: &dyn Memoria, datos: u64) -> Option<Vec<Ambito>> {
    let n = m.u32_en(datos)?;
    if n > MAX_AMBITOS {
        return None;
    }
    (0..n as u64)
        .map(|i| {
            let f = datos + 4 + 16 * i;
            Some(Ambito { inicio: m.u32_en(f)?, fin: m.u32_en(f + 4)?, manejador: m.u32_en(f + 8)?, destino: m.u32_en(f + 12)? })
        })
        .collect()
}

/// **Primera pasada**: la siguiente fila desde `desde` que es un `__except`
/// y cubre `pc` (RVA). Las de dentro van antes, asi que el orden de la tabla
/// es el orden en que se pregunta.
pub fn al_buscar(t: &[Ambito], desde: usize, pc: u32) -> Option<usize> {
    (desde..t.len()).find(|&i| t[i].destino != 0 && t[i].cubre(pc))
}

/// Lo que toca en la segunda pasada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlDesenrollar {
    /// Correr este `__finally` (la fila), y seguir desde la de despues.
    Finally(usize),
    /// Esta fila es el `__except` al que se va: lo de fuera no se toca.
    Llego(usize),
    /// Nada mas en este marco.
    Nada,
}

/// **Segunda pasada**: desde la fila `desde`, que `__finally` que cubra `pc`
/// hay que correr. `destino` (RVA) solo en el marco destino
/// (EXCEPTION_TARGET_UNWIND): al llegar a su `__except` se para.
pub fn al_desenrollar(t: &[Ambito], desde: usize, pc: u32, destino: Option<u32>) -> AlDesenrollar {
    for (i, a) in t.iter().enumerate().skip(desde) {
        if !a.cubre(pc) {
            continue;
        }
        if destino.is_some() && destino == Some(a.destino) {
            return AlDesenrollar::Llego(i);
        }
        if a.destino == 0 {
            return AlDesenrollar::Finally(i);
        }
    }
    AlDesenrollar::Nada
}

// -- AddVectoredExceptionHandler --------------------------------------------------

/// Los manejadores vectorizados: miran cada excepcion ANTES que ningun marco.
#[derive(Debug, Default)]
pub struct Vectores {
    /// (asa, manejador), en el orden en que se llaman.
    lista: Vec<(u64, u64)>,
    ultima: u64,
}

/// Las asas que se dan: no son punteros, y ninguna es 0 (el fallo).
const ASA_VECTOR: u64 = 0x5EC0_0000;

impl Vectores {
    pub const fn nuevos() -> Vectores {
        Vectores { lista: Vec::new(), ultima: 0 }
    }

    /// `primero != 0`: delante de todos; si no, detras.
    pub fn poner(&mut self, primero: bool, manejador: u64) -> u64 {
        self.ultima += 1;
        let asa = ASA_VECTOR + self.ultima;
        if primero {
            self.lista.insert(0, (asa, manejador));
        } else {
            self.lista.push((asa, manejador));
        }
        asa
    }

    /// `false` si esa asa no esta.
    pub fn quitar(&mut self, asa: u64) -> bool {
        let n = self.lista.len();
        self.lista.retain(|&(a, _)| a != asa);
        self.lista.len() != n
    }

    /// Los manejadores, en el orden en que se les pregunta.
    pub fn en_orden(&self) -> Vec<u64> {
        self.lista.iter().map(|&(_, h)| h).collect()
    }
}
