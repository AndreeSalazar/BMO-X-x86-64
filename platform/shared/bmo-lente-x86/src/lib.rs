//! **LA LENTE DE x86-64** -- el kernel lee el binario PRECISO: prefijos,
//! REX, VEX, opcode, ModRM, SIB, desplazamiento e inmediato, y lo dice. LB3
//! de `docs/plan/PLAN_LA_LENTE_BINARIA.md`, A5 de `docs/plan/EL_FOCO.md`.
//!
//! generacion: nieto -- lee bytes y dice lo que son; no sabe de que programa
//! capa: puro -- ni un `unsafe` ni un aparato: bytes que da quien llama
//!
//! # El propietario, 14-10
//!
//! *"convertir mi Kernel que LEA PRECISO binarios 0 y 1"*; y antes:
//! *"CABINA + LENTE ambos se fusionan y entiende el idioma BINARIO no en
//! ASM"*. El ensamblador es una vista para personas: esto lee los BITS, y el
//! texto (`texto.rs`) es solo como se los muestra a una.
//!
//! # La regla: lo que no esta en la tabla se dice, no se adivina
//!
//! La tabla (`tabla.rs`) sale del SDM de Intel y del APM de AMD, recortada a
//! lo que los emisores de BMO-X escriben DE VERDAD. Una instruccion que no
//! esta es [`NoSe::FueraDeLaTabla`], con su mapa, su opcode y su extension:
//! nunca un largo inventado. Una lente que adivina es peor que no tener lente.
//!
//! # Como se sabe que lee bien: el programa corriendo
//!
//! El oraculo es el `metro` del emisor (`toolchain/tools/metro`): corre los
//! programas del banco -- C, C++, COBOL, Ada, INTI y TITAN++ -- en el
//! emulador, y por CADA instruccion que se ejecuto mira que la siguiente que
//! se ejecuto sea una de las que la lente dice que pueden venir ([`Sigue`]):
//! la de justo detras (eso juzga el LARGO) o el destino de un salto (eso juzga
//! el DESPLAZAMIENTO). Y las pruebas de este crate hacen la ida y vuelta con
//! el codificador de los emisores (`bmo_lower::x86`).

#![no_std]
#![forbid(unsafe_code)]

pub mod recorre;
pub mod tabla;
mod texto;

pub use tabla::{Ext, Fila, Forma, Pref};
pub use texto::Texto;

/// Lo mas que mide una instruccion de x86.
pub const LARGO_MAX: usize = 15;

/// En que mapa de opcodes esta la instruccion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapa {
    /// Un byte.
    Uno,
    /// `0F xx`.
    Dos,
    /// `0F 38 xx`.
    Tres38,
    /// `0F 3A xx`.
    Tres3A,
}

/// Los prefijos de grupo que se vieron.
pub const P66: u8 = 1;
pub const PF2: u8 = 2;
pub const PF3: u8 = 4;
pub const PLOCK: u8 = 8;

/// El segmento de la direccion: ninguno, `fs:` o `gs:`.
pub const SEG_FS: u8 = 1;
pub const SEG_GS: u8 = 2;

/// Lo que dice un prefijo VEX (`C5` de dos bytes, `C4` de tres).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vex {
    /// R, X y B ya en positivo (en el byte van negados), y W.
    pub r: bool,
    pub x: bool,
    pub b: bool,
    pub w: bool,
    /// El registro de mas (`vvvv`, ya en positivo).
    pub vvvv: u8,
    /// 256 bits (`ymm`) si es true.
    pub l: bool,
    /// El prefijo que lleva dentro: 0 ninguno, 1 `66`, 2 `F3`, 3 `F2`.
    pub pp: u8,
}

/// **Una instruccion leida.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instr {
    /// Cuantos bytes mide.
    pub largo: usize,
    pub prefijos: u8,
    /// El byte REX (0: no hay).
    pub rex: u8,
    pub vex: Option<Vex>,
    pub mapa: Mapa,
    pub op: u8,
    pub modrm: Option<u8>,
    pub sib: Option<u8>,
    pub disp: i64,
    /// Si la direccion es `[rip + disp]`.
    pub rip_relativa: bool,
    pub imm: i64,
    /// Cuantos bytes tenia el inmediato (0: no hay), o el desplazamiento de
    /// un salto relativo (`Rel8`, `Rel32`).
    pub imm_bytes: u8,
    /// La fila de la tabla: su nombre y su forma.
    pub fila: &'static Fila,
    /// `fs:` ([`SEG_FS`]), `gs:` ([`SEG_GS`]) o ninguno (0).
    pub segmento: u8,
}

/// **Por que no se leyo.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoSe {
    /// Los bytes se acaban a media instruccion.
    Corta,
    /// Mas de 15 bytes: x86 no tiene instrucciones asi.
    Larga,
    /// No esta en la tabla: el mapa, el opcode, la extension del ModRM (si
    /// lo hay) y los prefijos. Los bytes los tiene quien pregunto.
    FueraDeLaTabla { mapa: Mapa, op: u8, ext: Option<u8>, prefijos: u8, vex: bool },
}

/// **Que puede venir despues** de una instruccion que esta en `en`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sigue {
    /// La de justo detras.
    Siguiente(u64),
    /// Un salto condicional: la de detras, o el destino.
    Condicional { cae: u64, salta: u64 },
    /// Un salto incondicional a un destino conocido.
    Salta(u64),
    /// Una llamada a un destino conocido; vuelve a `vuelve`.
    Llama { destino: u64, vuelve: u64 },
    /// `ret`: a donde diga la pila.
    Vuelve,
    /// Un salto o una llamada por registro o memoria: no se sabe sin correr.
    Indirecto,
    /// `hlt`, `ud2`, `int3`: aqui se para.
    Para,
}

struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn u8(&mut self) -> Result<u8, NoSe> {
        let v = *self.b.get(self.i).ok_or(NoSe::Corta)?;
        self.i += 1;
        if self.i > LARGO_MAX {
            return Err(NoSe::Larga);
        }
        Ok(v)
    }

    fn le(&mut self, n: u8) -> Result<i64, NoSe> {
        let mut v: u64 = 0;
        for k in 0..n {
            v |= (self.u8()? as u64) << (8 * k);
        }
        // con signo, de su medida
        let s = 64 - 8 * n as u32;
        Ok(if n == 0 { 0 } else { ((v << s) as i64) >> s })
    }
}

/// El prefijo que ELIGE una instruccion de SSE: `F3` y `F2` mandan sobre `66`.
fn obligatorio(prefijos: u8) -> Pref {
    if prefijos & PF3 != 0 {
        Pref::F3
    } else if prefijos & PF2 != 0 {
        Pref::F2
    } else if prefijos & P66 != 0 {
        Pref::P66
    } else {
        Pref::Ninguno
    }
}

fn casa(f: &Fila, pref: Pref) -> bool {
    f.pref == Pref::Cualquiera || f.pref == pref
}

/// **Lee UNA instruccion** del principio de `b`.
pub fn lee(b: &[u8]) -> Result<Instr, NoSe> {
    let mut l = Lector { b, i: 0 };
    let mut prefijos = 0u8;
    let mut segmento = 0u8;
    let mut byte = l.u8()?;
    loop {
        match byte {
            // `cs:` `ds:` `es:` `ss:`: en 64 bits no hacen nada (LLVM rellena
            // con `66 2E 0F 1F 84 ..`, un `nopw cs:[...]`). Se leen y se callan.
            0x2E | 0x3E | 0x26 | 0x36 => {}
            // `fs:` y `gs:` SI: suman su base (PROTON-X lee el bloque del hilo
            // de Windows con `gs:[0x30]`). Se apuntan y el texto los dice.
            0x64 => segmento = SEG_FS,
            0x65 => segmento = SEG_GS,
            0x66 => prefijos |= P66,
            0xF2 => prefijos |= PF2,
            0xF3 => prefijos |= PF3,
            0xF0 => prefijos |= PLOCK,
            _ => break,
        }
        byte = l.u8()?;
    }
    let mut rex = 0u8;
    let mut vex = None;
    let mut mapa = Mapa::Uno;
    let op;
    if byte == 0xC4 || byte == 0xC5 {
        // VEX: R X B y mmmmm (o solo R con `C5`), W vvvv L pp
        let (r, x, bb, m, w, segundo) = if byte == 0xC5 {
            let p = l.u8()?;
            (p & 0x80 == 0, false, false, 1u8, false, p)
        } else {
            let p1 = l.u8()?;
            let p2 = l.u8()?;
            (p1 & 0x80 == 0, p1 & 0x40 == 0, p1 & 0x20 == 0, p1 & 0x1F, p2 & 0x80 != 0, p2)
        };
        vex = Some(Vex { r, x, b: bb, w, vvvv: (!segundo >> 3) & 0xF, l: segundo & 4 != 0, pp: segundo & 3 });
        mapa = match m {
            1 => Mapa::Dos,
            2 => Mapa::Tres38,
            3 => Mapa::Tres3A,
            _ => return Err(NoSe::FueraDeLaTabla { mapa: Mapa::Uno, op: byte, ext: None, prefijos, vex: true }),
        };
        op = l.u8()?;
    } else {
        if (0x40..=0x4F).contains(&byte) {
            rex = byte;
            byte = l.u8()?;
        }
        if byte == 0x0F {
            let b2 = l.u8()?;
            match b2 {
                0x38 => {
                    mapa = Mapa::Tres38;
                    op = l.u8()?;
                }
                0x3A => {
                    mapa = Mapa::Tres3A;
                    op = l.u8()?;
                }
                _ => {
                    mapa = Mapa::Dos;
                    op = b2;
                }
            }
        } else {
            op = byte;
        }
    }
    let pref = match vex {
        Some(v) => [Pref::Ninguno, Pref::P66, Pref::F3, Pref::F2][v.pp as usize],
        None => obligatorio(prefijos),
    };
    let es_vex = vex.is_some();
    let fuera = |ext| NoSe::FueraDeLaTabla { mapa, op, ext, prefijos, vex: es_vex };
    // Las candidatas por mapa, opcode y prefijo; si alguna pide ModRM, se lee
    // (todas las de un mismo opcode lo piden o no lo piden).
    let primera = tabla::candidatas(mapa, op, es_vex).find(|f| casa(f, pref)).ok_or(fuera(None))?;
    let modrm = if primera.forma.modrm() { Some(l.u8()?) } else { None };
    let fila = match modrm {
        None => primera,
        Some(m) => {
            let reg = (m >> 3) & 7;
            tabla::candidatas(mapa, op, es_vex)
                .filter(|f| casa(f, pref))
                .find(|f| match f.ext {
                    Ext::No => true,
                    // Solo memoria (`ldmxcsr`, `fld qword`) o solo registro
                    // (`fxch st(i)`): el mismo `/n` es otra instruccion.
                    Ext::Es(n) => n == reg && !(f.forma.solo_memoria() && m >> 6 == 3) && !(f.forma.solo_registro() && m >> 6 != 3),
                    Ext::Byte(x) => x == m,
                })
                .ok_or(fuera(Some(reg)))?
        }
    };
    // La direccion: SIB y desplazamiento.
    let mut sib = None;
    let mut disp = 0;
    let mut rip_relativa = false;
    if let Some(m) = modrm {
        let (md, rm) = (m >> 6, m & 7);
        if md != 3 {
            if rm == 4 {
                let s = l.u8()?;
                sib = Some(s);
                if md == 0 && s & 7 == 5 {
                    disp = l.le(4)?;
                }
            } else if md == 0 && rm == 5 {
                rip_relativa = true;
                disp = l.le(4)?;
            }
            match md {
                1 => disp = l.le(1)?,
                2 => disp = l.le(4)?,
                _ => {}
            }
        }
    }
    let ancho = ancho_de(rex, prefijos, vex);
    let imm_bytes: u8 = match fila.forma {
        Forma::Rel8 | Forma::Acc8Imm8 | Forma::Imm8 | Forma::MovImm8 | Forma::MI8 | Forma::M8I8 | Forma::RMI8 | Forma::XI8 | Forma::MRI8 => 1,
        Forma::Rel32 | Forma::Imm => 4,
        Forma::AccImm | Forma::MI | Forma::RMI => if ancho == 2 { 2 } else { 4 },
        Forma::MovImm => match ancho {
            8 => 8,
            2 => 2,
            _ => 4,
        },
        _ => 0,
    };
    let imm = l.le(imm_bytes)?;
    Ok(Instr { largo: l.i, prefijos, rex, vex, mapa, op, modrm, sib, disp, rip_relativa, imm, imm_bytes, fila, segmento })
}

/// La medida del operando: 8 con REX.W (o VEX.W), 2 con `66`, si no 4.
fn ancho_de(rex: u8, prefijos: u8, vex: Option<Vex>) -> u8 {
    if rex & 8 != 0 || vex.is_some_and(|v| v.w) {
        8
    } else if prefijos & P66 != 0 {
        2
    } else {
        4
    }
}

/// **La direccion de memoria de una instruccion**: `[base + indice*escala +
/// disp]`, con los registros por su NUMERO de x86 (0 `rax`, 1 `rcx`, 2 `rdx`,
/// 3 `rbx`, 4 `rsp`, 5 `rbp`, 6 `rsi`, 7 `rdi`, 8..15 `r8`..`r15`).
///
/// Es lo que necesita la autopsia (B2): si la instruccion que fallo leia
/// `[rdi+0x18c0]`, el culpable es lo que tenia `rdi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memoria {
    pub base: Option<u8>,
    pub indice: Option<u8>,
    pub escala: u8,
    pub disp: i64,
    /// `[rip + disp]`: la direccion no depende de ningun registro.
    pub rip_relativa: bool,
    /// `fs:`/`gs:`: a la direccion se le suma esa base (que no esta en los
    /// registros enteros).
    pub segmento: u8,
}

/// **El `call` que acaba justo donde acaban `antes`** -- lo que hay antes de
/// una direccion de retorno. x86 es de largo variable y no se puede leer
/// hacia atras: se prueba desde el final, del mas corto al mas largo, hasta dar
/// con una instruccion ENTERA que sea un `call`. Devuelve la instruccion y
/// cuantos bytes mide. Lo usa la autopsia cuando el `rip` no se lee (un salto
/// a un puntero nulo): el `call` que salto dice de donde salio el nulo.
pub fn el_call_antes(antes: &[u8]) -> Option<(Instr, usize)> {
    (2..=antes.len().min(LARGO_MAX)).find_map(|k| {
        let i = lee(&antes[antes.len() - k..]).ok()?;
        (i.largo == k && i.fila.nombre == "call").then_some((i, k))
    })
}

/// El nombre de 64 bits del registro `n` (0..15).
pub fn nombre_r64(n: u8) -> &'static str {
    texto::R64[(n & 15) as usize]
}

impl Instr {
    /// **Si toca memoria, como forma la direccion.** `None` si sus
    /// operandos son solo registros (o no tiene ModRM).
    pub fn memoria(&self) -> Option<Memoria> {
        let m = self.modrm?;
        let (md, rm) = (m >> 6, m & 7);
        if md == 3 {
            return None;
        }
        let (b, x) = match self.vex {
            Some(v) => (v.b as u8, v.x as u8),
            None => (self.rex & 1, (self.rex >> 1) & 1),
        };
        let mut d = Memoria { base: None, indice: None, escala: 1, disp: self.disp, rip_relativa: false, segmento: self.segmento };
        if rm == 4 {
            let s = self.sib?;
            let i = ((s >> 3) & 7) | (x << 3);
            if i != 4 {
                d.indice = Some(i);
                d.escala = 1 << (s >> 6);
            }
            if !(md == 0 && s & 7 == 5) {
                d.base = Some((s & 7) | (b << 3));
            }
        } else if md == 0 && rm == 5 {
            d.rip_relativa = true;
        } else {
            d.base = Some(rm | (b << 3));
        }
        Some(d)
    }

    /// La medida del operando de esta instruccion.
    pub fn ancho(&self) -> u8 {
        ancho_de(self.rex, self.prefijos, self.vex)
    }

    /// **Que puede venir despues**, si esta en `en`.
    pub fn sigue(&self, en: u64) -> Sigue {
        let detras = en.wrapping_add(self.largo as u64);
        let destino = detras.wrapping_add(self.imm as u64);
        match (self.mapa, self.op, self.fila.forma) {
            (_, _, Forma::Rel8 | Forma::Rel32) => match (self.mapa, self.op) {
                (Mapa::Uno, 0xE8) => Sigue::Llama { destino, vuelve: detras },
                (Mapa::Uno, 0xE9 | 0xEB) => Sigue::Salta(destino),
                _ => Sigue::Condicional { cae: detras, salta: destino },
            },
            (Mapa::Uno, 0xC3, _) => Sigue::Vuelve,
            (Mapa::Uno, 0xFF, _) if matches!(self.fila.ext, Ext::Es(2) | Ext::Es(4)) => Sigue::Indirecto,
            (Mapa::Uno, 0xF4 | 0xCC, _) | (Mapa::Dos, 0x0B, _) => Sigue::Para,
            _ => Sigue::Siguiente(detras),
        }
    }

    /// **Como se le muestra a una persona**: Intel, con los destinos de los
    /// saltos ya sumados si se sabe donde esta (`en`).
    pub fn texto(&self, en: u64) -> Texto {
        texto::escribe(self, en)
    }
}

#[cfg(test)]
mod pruebas;
