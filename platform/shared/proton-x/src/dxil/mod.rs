//! **Un sombreador de D3D12, leido** (P3b1, 27-09): el contenedor `DXBC`, sus
//! firmas de entrada y salida, y el programa DXIL de dentro.
//!
//! Lo que un juego D3D12 le da a `CreateGraphicsPipelineState` son estos
//! bytes, tal como los escupe `dxc` (el compilador de Microsoft): nadie los
//! traduce antes. Asi que PROTON-X tiene que saber leerlos, y lo primero es la
//! forma, sin ejecutar nada:
//!
//! ```text
//!    "DXBC" | hash (16) | version | medida | n partes | desplazamientos
//!    una parte: FourCC (4) | medida (4) | los bytes
//!       SFI0  banderas                    ISG1/ISGN  la firma de ENTRADA
//!       PSV0  lo que el runtime valida    OSG1/OSGN  la firma de SALIDA
//!       STAT  reflexion (con nombres)     HASH       la huella del programa
//!       DXIL  el programa: cabecera + bitcode de LLVM 3.7 (`bits.rs`)
//! ```
//!
//! De la cabecera de DXIL sale la ETAPA (vertice, pixel...) y el modelo
//! (6.0); del bitcode, el modulo (`Modulo`): que funciones hay, cuales son
//! declaraciones de `dx.op.*` (las operaciones de D3D) y cuantas instrucciones
//! tiene cada cuerpo. Que hace cada instruccion, y correrlo, es P3b3:
//! `programa.rs`.
//!
//! Las formas son las de la documentacion publica de DXIL
//! (DirectXShaderCompiler, `docs/DXIL.rst` y `DxilContainer.h`); el banco las
//! comprueba contra lo que dice `dxc -dumpbin` de los mismos bytes.

pub mod bits;
pub mod programa;
/// Los recursos de un sombreador con su espacio, de su PSV0 (03-10).
pub mod ranuras;
pub mod recursos;
/// E6 (02-10): programas de muestra con `si` y bucles (el banco del emisor).
pub mod ejemplos;
/// E6b (02-10): el grafo de bloques del DXIL, vuelto `si` y bucles.
mod estructura;
/// E6c (02-10): los enteros y las conversiones del DXIL.
mod enteros;
/// 03-10: las olas (un pixel por ola) y las derivadas.
mod olas;
/// El interprete de un `Programa` (partido de `programa.rs`, 03-10).
mod interprete;
/// N5.10: los arrays (alloca, GEP, load, store y las tablas globales).
mod arreglos;

use alloc::string::String;
use alloc::vec::Vec;

pub use bits::{Bloque, NoLee, Registro};

/// Por que un sombreador no se lee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSombreador {
    /// Ni `DXBC` ni un contenedor que cuadre.
    Contenedor(&'static str),
    /// No trae programa: ni `DXIL` ni `SHEX`/`SHDR`.
    SinDxil,
    /// El bitcode no se lee: donde y por que.
    Bitcode(NoLee),
    /// El bitcode se lee pero no es un modulo de DXIL.
    Modulo(&'static str),
}

/// La etapa de la tuberia, de la cabecera del programa DXIL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etapa {
    Pixel,
    Vertice,
    Geometria,
    Casco,
    Dominio,
    Computo,
    Otra(u32),
}

/// Un elemento de una firma: `POSITION0` en el registro 0, `xyz`...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elemento {
    pub semantica: String,
    pub indice: u32,
    /// El valor de sistema (0 ninguno, 1 SV_Position, 64 SV_Target...).
    pub sistema: u32,
    /// 3 = float.
    pub tipo: u32,
    pub registro: u32,
    pub mascara: u8,
}

/// Una funcion del modulo: su nombre, si es solo una declaracion, y cuantas
/// instrucciones tiene su cuerpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Funcion {
    pub nombre: String,
    pub declarada: bool,
    pub instrucciones: usize,
}

/// El modulo de LLVM de dentro, lo justo para saber que hay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modulo {
    /// Quien lo hizo: el `llvm.ident` de sus metadatos (`dxc(private) 1.8...`).
    /// En LLVM 3.7 no hay bloque IDENTIFICATION (llego en 3.8).
    pub productor: String,
    pub funciones: Vec<Funcion>,
    /// Los bloques de arriba, enteros, para quien los quiera recorrer (P3b3).
    pub bloques: Vec<Bloque>,
}

impl Modulo {
    /// La funcion con cuerpo (el punto de entrada del sombreador).
    pub fn entrada(&self) -> Option<&Funcion> {
        self.funciones.iter().find(|f| !f.declarada)
    }

    /// Las operaciones de D3D que usa (`dx.op.*`), sin repetir.
    pub fn operaciones(&self) -> Vec<&str> {
        self.funciones.iter().filter(|f| f.declarada && f.nombre.starts_with("dx.op.")).map(|f| f.nombre.as_str()).collect()
    }
}

/// Un sombreador leido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sombreador {
    pub etapa: Etapa,
    /// El modelo: (6, 0).
    pub modelo: (u32, u32),
    pub entradas: Vec<Elemento>,
    pub salidas: Vec<Elemento>,
    /// Las partes del contenedor, por su FourCC, en orden.
    pub partes: Vec<[u8; 4]>,
    /// El modulo de LLVM (vacio en un SM5).
    pub modulo: Modulo,
    /// Un sombreador de SM4/SM5 (FXC): las palabras de su `SHEX`/`SHDR`, que
    /// corre `crate::sm5` (P3c3). `None` en un DXIL.
    pub sm5: Option<Vec<u32>>,
    /// Sus recursos con su espacio (de la parte PSV0, 03-10); vacio si no la
    /// trae (FXC no la pone).
    pub recursos: Vec<recursos::Recurso>,
}

fn u32_en(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn cadena_c(d: &[u8], o: usize) -> String {
    d.get(o..).map(|r| r.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect()).unwrap_or_default()
}

/// Una firma (`ISG1`/`OSG1` de 32 bytes por elemento, o `ISGN`/`OSGN` de 24).
/// Los nombres van por desplazamiento desde el inicio de la parte.
fn firma(p: &[u8], ancha: bool) -> Result<Vec<Elemento>, NoSombreador> {
    let n = u32_en(p, 0).ok_or(NoSombreador::Contenedor("una firma sin cabecera"))? as usize;
    let desde = u32_en(p, 4).ok_or(NoSombreador::Contenedor("una firma sin cabecera"))? as usize;
    let (paso, base) = if ancha { (32, 4) } else { (24, 0) };
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let e = desde + i * paso;
        let campo = |k: usize| u32_en(p, e + base + 4 * k).ok_or(NoSombreador::Contenedor("una firma cortada"));
        let nombre = campo(0)? as usize;
        v.push(Elemento {
            semantica: cadena_c(p, nombre),
            indice: campo(1)?,
            sistema: campo(2)?,
            tipo: campo(3)?,
            registro: campo(4)?,
            mascara: p.get(e + base + 20).copied().ok_or(NoSombreador::Contenedor("una firma cortada"))?,
        });
    }
    Ok(v)
}

// Los bloques y registros de LLVM 3.7 que se miran aqui.
const MODULE: u64 = 8;
const METADATA: u64 = 15;
const METADATA_STRING: u64 = 1;
const METADATA_NODE: u64 = 3;
const METADATA_NAME: u64 = 4;
const METADATA_KIND: u64 = 6;
const METADATA_NAMED_NODE: u64 = 10;
const FUNCTION_BLOCK: u64 = 12;
const VALUE_SYMTAB: u64 = 14;
const MODULE_CODE_GLOBALVAR: u64 = 7;
const MODULE_CODE_FUNCTION: u64 = 8;
const MODULE_CODE_ALIAS: u64 = 9;
const VST_CODE_ENTRY: u64 = 1;
const VST_CODE_FNENTRY: u64 = 3;
const FUNC_CODE_DECLAREBLOCKS: u64 = 1;

fn texto(ops: &[u64]) -> String {
    ops.iter().map(|&c| c as u8 as char).collect()
}

/// Un metadato de LLVM 3.7, lo justo: cadenas y nodos (lo demas se cuenta
/// para que la numeracion cuadre, pero no se guarda).
enum Md {
    Cadena(String),
    Nodo(Vec<u64>),
    Otro,
}

/// **Un metadato con nombre** (`llvm.ident`, `dx.shaderModel`...): la lista de
/// nodos a la que apunta. Cada registro de METADATA define un metadato nuevo
/// y numerado, salvo NAME, KIND y NAMED_NODE; un NODE apunta a otros con
/// `id + 1` (0 es "nada").
fn metadatos_con_nombre(m: &Bloque) -> Vec<(String, Vec<Md>)> {
    let mut todos: Vec<Md> = Vec::new();
    let mut nombrados: Vec<(String, Vec<u64>)> = Vec::new();
    let mut nombre = None;
    for b in m.bloques.iter().filter(|b| b.id == METADATA) {
        for r in &b.registros {
            match r.codigo {
                METADATA_NAME => nombre = Some(texto(&r.ops)),
                METADATA_NAMED_NODE => {
                    if let Some(n) = nombre.take() {
                        nombrados.push((n, r.ops.clone()));
                    }
                }
                METADATA_KIND => {}
                METADATA_STRING => todos.push(Md::Cadena(texto(&r.ops))),
                METADATA_NODE => todos.push(Md::Nodo(r.ops.clone())),
                _ => todos.push(Md::Otro),
            }
        }
    }
    // Cada nombre apunta a nodos; un nodo cuyo primer elemento es una cadena
    // se da como esa cadena (`!{!"dxc..."}` -> "dxc...").
    let cadena_de = |id: u64| -> Md {
        match todos.get(id as usize) {
            Some(Md::Nodo(v)) => match v.first().and_then(|&x| x.checked_sub(1)).and_then(|x| todos.get(x as usize)) {
                Some(Md::Cadena(c)) => Md::Cadena(c.clone()),
                _ => Md::Nodo(v.clone()),
            },
            Some(Md::Cadena(c)) => Md::Cadena(c.clone()),
            _ => Md::Otro,
        }
    };
    nombrados.into_iter().map(|(n, ids)| (n, ids.iter().map(|&i| cadena_de(i)).collect())).collect()
}

/// **El modulo**, de los bloques del bitcode. Los valores globales se numeran
/// en el orden de sus registros (variables, funciones, alias); la tabla de
/// simbolos del modulo les pone nombre; los cuerpos (FUNCTION_BLOCK) van en el
/// orden de las funciones que NO son declaracion.
fn modulo(bloques: Vec<Bloque>) -> Result<Modulo, NoSombreador> {
    let m = bloques.iter().find(|b| b.id == MODULE).ok_or(NoSombreador::Modulo("sin MODULE_BLOCK"))?;
    let productor = metadatos_con_nombre(m)
        .into_iter()
        .find(|(n, _)| n == "llvm.ident")
        .and_then(|(_, v)| v.into_iter().find_map(|md| if let Md::Cadena(c) = md { Some(c) } else { None }))
        .unwrap_or_default();
    let mut globales: Vec<Option<usize>> = Vec::new();
    let mut funciones = Vec::new();
    for r in &m.registros {
        match r.codigo {
            MODULE_CODE_GLOBALVAR | MODULE_CODE_ALIAS => globales.push(None),
            MODULE_CODE_FUNCTION => {
                // [tipo, convencion, es_prototipo, ...]
                let declarada = r.ops.get(2).copied().unwrap_or(1) != 0;
                globales.push(Some(funciones.len()));
                funciones.push(Funcion { nombre: String::new(), declarada, instrucciones: 0 });
            }
            _ => {}
        }
    }
    if let Some(vst) = m.hijo(VALUE_SYMTAB) {
        for r in &vst.registros {
            let (id, nombre) = match r.codigo {
                VST_CODE_ENTRY => (r.ops.first().copied(), r.ops.get(1..).map(texto)),
                VST_CODE_FNENTRY => (r.ops.first().copied(), r.ops.get(2..).map(texto)),
                _ => (None, None),
            };
            if let (Some(id), Some(nombre)) = (id, nombre) {
                if let Some(Some(f)) = globales.get(id as usize) {
                    funciones[*f].nombre = nombre;
                }
            }
        }
    }
    let cuerpos: Vec<usize> = m.bloques.iter().filter(|b| b.id == FUNCTION_BLOCK).map(|b| b.registros.iter().filter(|r| r.codigo != FUNC_CODE_DECLAREBLOCKS).count()).collect();
    let mut con_cuerpo = funciones.iter_mut().filter(|f| !f.declarada);
    for n in cuerpos {
        let f = con_cuerpo.next().ok_or(NoSombreador::Modulo("mas cuerpos que funciones definidas"))?;
        f.instrucciones = n;
    }
    Ok(Modulo { productor, funciones, bloques })
}

/// La etapa, de la version (igual en DXIL y en SM4/SM5).
fn etapa(version: u32) -> Etapa {
    match version >> 16 {
        0 => Etapa::Pixel,
        1 => Etapa::Vertice,
        2 => Etapa::Geometria,
        3 => Etapa::Casco,
        4 => Etapa::Dominio,
        5 => Etapa::Computo,
        e => Etapa::Otra(e),
    }
}

/// **Leer un sombreador** de D3D12: el contenedor entero, sus firmas y su
/// programa.
pub fn leer(d: &[u8]) -> Result<Sombreador, NoSombreador> {
    if d.get(..4) != Some(b"DXBC") {
        return Err(NoSombreador::Contenedor("no empieza por DXBC"));
    }
    let total = u32_en(d, 24).ok_or(NoSombreador::Contenedor("sin medida"))? as usize;
    if total > d.len() {
        return Err(NoSombreador::Contenedor("dice medir mas de lo que trae"));
    }
    let n = u32_en(d, 28).ok_or(NoSombreador::Contenedor("sin numero de partes"))? as usize;
    let mut partes = Vec::with_capacity(n);
    let (mut entradas, mut salidas, mut programa, mut shex) = (Vec::new(), Vec::new(), None, None);
    let mut tabla = Vec::new();
    for i in 0..n {
        let o = u32_en(d, 32 + 4 * i).ok_or(NoSombreador::Contenedor("una parte sin desplazamiento"))? as usize;
        let cc: [u8; 4] = d.get(o..o + 4).and_then(|b| b.try_into().ok()).ok_or(NoSombreador::Contenedor("una parte fuera"))?;
        let tam = u32_en(d, o + 4).ok_or(NoSombreador::Contenedor("una parte fuera"))? as usize;
        let p = d.get(o + 8..o + 8 + tam).ok_or(NoSombreador::Contenedor("una parte pasa del final"))?;
        match &cc {
            b"ISG1" => entradas = firma(p, true)?,
            b"OSG1" => salidas = firma(p, true)?,
            b"ISGN" => entradas = firma(p, false)?,
            b"OSGN" => salidas = firma(p, false)?,
            b"DXIL" => programa = Some(p),
            b"SHEX" | b"SHDR" => shex = Some(p),
            b"PSV0" => tabla = recursos::de_psv0(p).ok_or(NoSombreador::Contenedor("una PSV0 que no se lee"))?,
            _ => {}
        }
        partes.push(cc);
    }
    if let (None, Some(p)) = (programa, shex) {
        // SM4/SM5 (P3c3): la misma version que DXIL, y el programa entero en
        // palabras.
        let t: Vec<u32> = p.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let version = *t.first().ok_or(NoSombreador::Contenedor("SHEX sin version"))?;
        if t.get(1).map_or(true, |&n| n as usize > t.len() || n < 2) {
            return Err(NoSombreador::Contenedor("SHEX con una medida que no cuadra"));
        }
        let modulo = Modulo { productor: String::new(), funciones: Vec::new(), bloques: Vec::new() };
        return Ok(Sombreador { etapa: etapa(version), modelo: ((version >> 4) & 0xF, version & 0xF), entradas, salidas, partes, modulo, sm5: Some(t), recursos: tabla });
    }
    let p = programa.ok_or(NoSombreador::SinDxil)?;
    // Cabecera del programa: version (etapa << 16 | mayor << 4 | menor), medida
    // en palabras, y la de DXIL: "DXIL", su version, desde donde y cuanto.
    let version = u32_en(p, 0).ok_or(NoSombreador::Contenedor("DXIL sin cabecera"))?;
    if p.get(8..12) != Some(b"DXIL") {
        return Err(NoSombreador::Contenedor("la parte DXIL no dice DXIL"));
    }
    let desde = u32_en(p, 16).ok_or(NoSombreador::Contenedor("DXIL sin cabecera"))? as usize;
    let tam = u32_en(p, 20).ok_or(NoSombreador::Contenedor("DXIL sin cabecera"))? as usize;
    let bc = p.get(8 + desde..8 + desde + tam).ok_or(NoSombreador::Contenedor("el bitcode pasa del final"))?;
    let bloques = bits::leer(bc).map_err(NoSombreador::Bitcode)?;
    Ok(Sombreador {
        etapa: etapa(version),
        modelo: ((version >> 4) & 0xF, version & 0xF),
        entradas,
        salidas,
        partes,
        modulo: modulo(bloques)?,
        sm5: None,
        recursos: tabla,
    })
}
