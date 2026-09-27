//! **El flujo de bits de LLVM** (P3b1, 27-09): lo que hay dentro de la parte
//! `DXIL` de un sombreador de D3D12.
//!
//! DXIL es bitcode de LLVM 3.7. No es un formato de bytes: es un flujo de BITS
//! (del menos al mas significativo de cada palabra de 32) con bloques anidados
//! y registros, y cada registro se escribe con una ABREVIATURA que el propio
//! flujo define antes de usarla. Esto lo lee entero, sin saber todavia que
//! significa cada registro: eso es `modulo.rs`.
//!
//! ```text
//!    id de abreviatura (ancho del bloque)
//!    0  END_BLOCK        alinear a 32 y salir del bloque
//!    1  ENTER_SUBBLOCK   id (vbr8), ancho nuevo (vbr4), alinear, palabras (32)
//!    2  DEFINE_ABBREV    n (vbr5) operandos: literal, Fixed(w), VBR(w),
//!                        Array, Char6 o Blob
//!    3  UNABBREV_RECORD  codigo (vbr6), n (vbr6), n operandos (vbr6)
//!    4+ un registro con la abreviatura id - 4
//! ```
//!
//! El bloque BLOCKINFO (id 0) no es contenido: define abreviaturas para OTROS
//! bloques (`SETBID`), y se aplican a todo bloque de ese id que venga despues.
//! La referencia es la documentacion de LLVM ("LLVM Bitcode File Format").
//!
//! Cada NO dice donde: el bit en el que el flujo dejo de tener sentido.

use alloc::vec::Vec;

/// Un registro: su codigo, sus operandos y, si la abreviatura lo tenia, un
/// blob de bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registro {
    pub codigo: u64,
    pub ops: Vec<u64>,
    pub blob: Option<Vec<u8>>,
}

/// Un bloque: su id, sus registros y sus bloques hijos, en orden.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bloque {
    pub id: u64,
    pub registros: Vec<Registro>,
    pub bloques: Vec<Bloque>,
}

impl Bloque {
    /// El primer hijo con este id.
    pub fn hijo(&self, id: u64) -> Option<&Bloque> {
        self.bloques.iter().find(|b| b.id == id)
    }
}

/// Por que un flujo no se lee, y en que bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoLee {
    /// No empieza por `BC` 0xC0DE.
    Magia,
    /// Se acabo antes de tiempo.
    Corto { bit: usize },
    /// Una abreviatura que no se definio.
    Abreviatura { bit: usize, id: u64 },
    /// Una codificacion de operando que no existe (ni 1..5).
    Codificacion { bit: usize, cual: u64 },
    /// Un Array o un Blob donde no pueden ir (el ultimo operando, sin elemento).
    Forma { bit: usize },
    /// END_BLOCK sin bloque abierto.
    FinSuelto { bit: usize },
}

/// Un operando de una abreviatura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Literal(u64),
    Fijo(u32),
    Vbr(u32),
    Array,
    Char6,
    Blob,
}

type Abreviatura = Vec<Op>;

struct Lector<'a> {
    d: &'a [u8],
    bit: usize,
}

impl Lector<'_> {
    fn fijo(&mut self, ancho: u32) -> Result<u64, NoLee> {
        if ancho == 0 {
            return Ok(0);
        }
        if self.bit + ancho as usize > self.d.len() * 8 {
            return Err(NoLee::Corto { bit: self.bit });
        }
        let mut v = 0u64;
        for i in 0..ancho as usize {
            let b = self.bit + i;
            v |= (((self.d[b / 8] >> (b % 8)) & 1) as u64) << i;
        }
        self.bit += ancho as usize;
        Ok(v)
    }

    fn vbr(&mut self, ancho: u32) -> Result<u64, NoLee> {
        let alto = 1u64 << (ancho - 1);
        let mut v = 0u64;
        let mut desplazar = 0;
        loop {
            let trozo = self.fijo(ancho)?;
            v |= (trozo & (alto - 1)).checked_shl(desplazar).unwrap_or(0);
            if trozo & alto == 0 {
                return Ok(v);
            }
            desplazar += ancho - 1;
            if desplazar > 64 {
                return Err(NoLee::Forma { bit: self.bit });
            }
        }
    }

    fn alinear32(&mut self) {
        self.bit = self.bit.div_ceil(32) * 32;
    }

    fn fin(&self) -> bool {
        self.bit + 32 > self.d.len() * 8
    }
}

const CHAR6: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._";

/// Las abreviaturas que BLOCKINFO define para cada id de bloque.
#[derive(Default)]
struct Info {
    por_bloque: Vec<(u64, Vec<Abreviatura>)>,
}

impl Info {
    fn de(&self, id: u64) -> Vec<Abreviatura> {
        self.por_bloque.iter().find(|(b, _)| *b == id).map(|(_, a)| a.clone()).unwrap_or_default()
    }

    fn poner(&mut self, id: u64, a: Abreviatura) {
        match self.por_bloque.iter_mut().find(|(b, _)| *b == id) {
            Some((_, v)) => v.push(a),
            None => self.por_bloque.push((id, alloc::vec![a])),
        }
    }
}

fn definir(l: &mut Lector) -> Result<Abreviatura, NoLee> {
    let n = l.vbr(5)?;
    let mut a = Vec::with_capacity(n as usize);
    for _ in 0..n {
        if l.fijo(1)? == 1 {
            a.push(Op::Literal(l.vbr(8)?));
            continue;
        }
        let bit = l.bit;
        a.push(match l.fijo(3)? {
            1 => Op::Fijo(l.vbr(5)? as u32),
            2 => Op::Vbr(l.vbr(5)? as u32),
            3 => Op::Array,
            4 => Op::Char6,
            5 => Op::Blob,
            cual => return Err(NoLee::Codificacion { bit, cual }),
        });
    }
    Ok(a)
}

fn escalar(l: &mut Lector, op: Op) -> Result<u64, NoLee> {
    match op {
        Op::Literal(v) => Ok(v),
        Op::Fijo(w) => l.fijo(w),
        Op::Vbr(w) => l.vbr(w),
        Op::Char6 => Ok(CHAR6[l.fijo(6)? as usize] as u64),
        Op::Array | Op::Blob => Err(NoLee::Forma { bit: l.bit }),
    }
}

/// Un registro con abreviatura: el primer operando es el codigo.
fn abreviado(l: &mut Lector, a: &Abreviatura) -> Result<Registro, NoLee> {
    let mut vals = Vec::new();
    let mut blob = None;
    let mut i = 0;
    while i < a.len() {
        match a[i] {
            Op::Array => {
                let elemento = *a.get(i + 1).ok_or(NoLee::Forma { bit: l.bit })?;
                let n = l.vbr(6)?;
                for _ in 0..n {
                    vals.push(escalar(l, elemento)?);
                }
                i += 2;
            }
            Op::Blob => {
                let n = l.vbr(6)? as usize;
                l.alinear32();
                let desde = l.bit / 8;
                let bytes = l.d.get(desde..desde + n).ok_or(NoLee::Corto { bit: l.bit })?;
                blob = Some(bytes.to_vec());
                l.bit += n * 8;
                l.alinear32();
                i += 1;
            }
            op => {
                vals.push(escalar(l, op)?);
                i += 1;
            }
        }
    }
    if vals.is_empty() {
        return Err(NoLee::Forma { bit: l.bit });
    }
    let codigo = vals.remove(0);
    Ok(Registro { codigo, ops: vals, blob })
}

/// **Un bloque**, con el ancho de abreviatura que le toca, hasta su END_BLOCK.
fn bloque(l: &mut Lector, info: &mut Info, id: u64, ancho: u32) -> Result<Bloque, NoLee> {
    let mut b = Bloque { id, ..Bloque::default() };
    let mut abrevs = info.de(id);
    // En BLOCKINFO: el bloque al que van las abreviaturas que se definan.
    let mut para: Option<u64> = None;
    loop {
        let bit = l.bit;
        match l.fijo(ancho)? {
            0 => {
                l.alinear32();
                return Ok(b);
            }
            1 => {
                let hijo = l.vbr(8)?;
                let nuevo = l.vbr(4)? as u32;
                l.alinear32();
                let _palabras = l.fijo(32)?;
                let h = bloque(l, info, hijo, nuevo)?;
                // BLOCKINFO no es contenido: ya dejo sus abreviaturas en `info`.
                if hijo != 0 {
                    b.bloques.push(h);
                }
            }
            2 => {
                let a = definir(l)?;
                match (id, para) {
                    (0, Some(destino)) => info.poner(destino, a),
                    _ => abrevs.push(a),
                }
            }
            3 => {
                let codigo = l.vbr(6)?;
                let n = l.vbr(6)?;
                let mut ops = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    ops.push(l.vbr(6)?);
                }
                if id == 0 && codigo == 1 {
                    para = ops.first().copied();
                }
                b.registros.push(Registro { codigo, ops, blob: None });
            }
            n => {
                let a = abrevs.get(n as usize - 4).ok_or(NoLee::Abreviatura { bit, id: n })?.clone();
                let r = abreviado(l, &a)?;
                if id == 0 && r.codigo == 1 {
                    para = r.ops.first().copied();
                }
                b.registros.push(r);
            }
        }
    }
}

/// **Leer un flujo de bitcode entero**: los bloques de arriba (en DXIL,
/// IDENTIFICATION y MODULE).
pub fn leer(d: &[u8]) -> Result<Vec<Bloque>, NoLee> {
    if d.get(..4) != Some(&[b'B', b'C', 0xC0, 0xDE]) {
        return Err(NoLee::Magia);
    }
    let mut l = Lector { d, bit: 32 };
    let mut info = Info::default();
    let mut arriba = Vec::new();
    while !l.fin() {
        let bit = l.bit;
        match l.fijo(2)? {
            1 => {
                let id = l.vbr(8)?;
                let ancho = l.vbr(4)? as u32;
                l.alinear32();
                let _palabras = l.fijo(32)?;
                let b = bloque(&mut l, &mut info, id, ancho)?;
                if id != 0 {
                    arriba.push(b);
                }
            }
            0 => return Err(NoLee::FinSuelto { bit }),
            otro => return Err(NoLee::Abreviatura { bit, id: otro }),
        }
    }
    Ok(arriba)
}
