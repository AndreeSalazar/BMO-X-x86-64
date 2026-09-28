//! **Desenrollar la pila de x64 como Windows** (P4c, 27-09).
//!
//! En Windows x64 no hay cadena de marcos: cada funcion (que no sea hoja)
//! trae en `.pdata` una RUNTIME_FUNCTION y en `.xdata` su UNWIND_INFO -- la
//! lista de lo que hizo su prologo, al reves --. Con eso, desde el contexto de
//! un marco se sabe el del marco que lo llamo: es lo que hacen las
//! excepciones (SEH, C++), los depuradores y los perfiles.
//!
//! ```text
//!    RUNTIME_FUNCTION (12 B)   inicio, fin, UNWIND_INFO (RVA)
//!    UNWIND_INFO               version:3 banderas:5 | medida del prologo |
//!                              n de codigos | registro de marco:4 desp:4 |
//!                              codigos (u16) | manejador (RVA) + sus datos,
//!                              o la RUNTIME_FUNCTION encadenada
//!    los codigos, al reves     PUSH_NONVOL  ALLOC_LARGE  ALLOC_SMALL
//!                              SET_FPREG  SAVE_NONVOL(_FAR)
//!                              SAVE_XMM128(_FAR)  PUSH_MACHFRAME  EPILOG (v2)
//! ```
//!
//! Solo cuentas: la memoria (la pila, la imagen) la lee quien llama por
//! [`Memoria`]. Las formas son las de la documentacion de Microsoft ("x64
//! exception handling"); el banco del anfitrion las comprueba con lo que
//! `clang` escribe en `.pdata` y `.xdata`, y en el Ryzen contra Windows (el
//! mismo `seh.exe`).
//!
//! **Lo que no hace, dicho:** si la excepcion cae DENTRO de un epilogo
//! (entre el `add rsp` y el `ret`), Windows lo detecta desensamblando y no
//! aplica los codigos; aqui no. Con `RaiseException` (una llamada) no pasa.

use alloc::vec::Vec;

/// Los registros de un marco: los 16 enteros en el orden de la codificacion
/// x86 (rax rcx rdx rbx rsp rbp rsi rdi r8..r15, que es tambien el del CONTEXT
/// de Windows), rip, los 16 xmm y el MXCSR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contexto {
    pub gp: [u64; 16],
    pub rip: u64,
    pub xmm: [u128; 16],
    pub mxcsr: u32,
}

pub const RAX: usize = 0;
pub const RBX: usize = 3;
pub const RSP: usize = 4;
pub const RBP: usize = 5;
pub const RSI: usize = 6;
pub const RDI: usize = 7;

/// El CONTEXT de Windows x64 (0x4D0 bytes): donde va cada cosa.
pub const CONTEXT_BYTES: usize = 0x4D0;
const CTX_FLAGS: usize = 0x30;
const CTX_MXCSR: usize = 0x34;
const CTX_GP: usize = 0x78;
const CTX_RIP: usize = 0xF8;
const CTX_FLTSAVE_MXCSR: usize = 0x118;
const CTX_XMM: usize = 0x1A0;
/// CONTEXT_AMD64 | CONTROL | INTEGER | FLOATING_POINT.
const CONTEXT_FULL: u32 = 0x0010_000B;

impl Contexto {
    /// Escribirlo en la forma del CONTEXT de Windows (lo que lee un filtro).
    pub fn a_context(&self, c: &mut [u8]) {
        c[..CONTEXT_BYTES].fill(0);
        c[CTX_FLAGS..CTX_FLAGS + 4].copy_from_slice(&CONTEXT_FULL.to_le_bytes());
        self.sobre_context(c);
    }

    /// Escribir SOLO lo que este contexto sabe (los enteros, rip, los xmm y el
    /// MXCSR) sobre un CONTEXT que ya existe: las banderas, los segmentos y
    /// lo demas de quien lo dio se quedan como estaban (RtlVirtualUnwind).
    pub fn sobre_context(&self, c: &mut [u8]) {
        c[CTX_MXCSR..CTX_MXCSR + 4].copy_from_slice(&self.mxcsr.to_le_bytes());
        c[CTX_FLTSAVE_MXCSR..CTX_FLTSAVE_MXCSR + 4].copy_from_slice(&self.mxcsr.to_le_bytes());
        for (i, r) in self.gp.iter().enumerate() {
            c[CTX_GP + 8 * i..CTX_GP + 8 * i + 8].copy_from_slice(&r.to_le_bytes());
        }
        c[CTX_RIP..CTX_RIP + 8].copy_from_slice(&self.rip.to_le_bytes());
        for (i, x) in self.xmm.iter().enumerate() {
            c[CTX_XMM + 16 * i..CTX_XMM + 16 * i + 16].copy_from_slice(&x.to_le_bytes());
        }
    }

    /// Leerlo de un CONTEXT de Windows.
    pub fn de_context(c: &[u8]) -> Contexto {
        let u = |o: usize| u64::from_le_bytes(c[o..o + 8].try_into().unwrap_or([0; 8]));
        Contexto {
            gp: core::array::from_fn(|i| u(CTX_GP + 8 * i)),
            rip: u(CTX_RIP),
            xmm: core::array::from_fn(|i| u128::from_le_bytes(c[CTX_XMM + 16 * i..CTX_XMM + 16 * i + 16].try_into().unwrap_or([0; 16]))),
            mxcsr: u32::from_le_bytes(c[CTX_MXCSR..CTX_MXCSR + 4].try_into().unwrap_or([0; 4])),
        }
    }
}

/// Quien lee la memoria: la pila y la imagen. `None` si esa direccion no es
/// legible (y entonces el desenrollado para, en vez de leer basura).
pub trait Memoria {
    fn u64_en(&self, dir: u64) -> Option<u64>;
    fn u32_en(&self, dir: u64) -> Option<u32>;
    fn u16_en(&self, dir: u64) -> Option<u16>;
    fn u8_en(&self, dir: u64) -> Option<u8>;
    fn u128_en(&self, dir: u64) -> Option<u128> {
        Some(self.u64_en(dir)? as u128 | (self.u64_en(dir + 8)? as u128) << 64)
    }
}

/// Una memoria que es un trozo de bytes que empieza en `base` (el banco).
pub struct Trozo<'a> {
    pub base: u64,
    pub bytes: &'a [u8],
}

impl Memoria for Trozo<'_> {
    fn u64_en(&self, d: u64) -> Option<u64> {
        let o = d.checked_sub(self.base)? as usize;
        Some(u64::from_le_bytes(self.bytes.get(o..o + 8)?.try_into().ok()?))
    }
    fn u32_en(&self, d: u64) -> Option<u32> {
        let o = d.checked_sub(self.base)? as usize;
        Some(u32::from_le_bytes(self.bytes.get(o..o + 4)?.try_into().ok()?))
    }
    fn u16_en(&self, d: u64) -> Option<u16> {
        let o = d.checked_sub(self.base)? as usize;
        Some(u16::from_le_bytes(self.bytes.get(o..o + 2)?.try_into().ok()?))
    }
    fn u8_en(&self, d: u64) -> Option<u8> {
        let o = d.checked_sub(self.base)? as usize;
        self.bytes.get(o).copied()
    }
}

/// Una RUNTIME_FUNCTION, en RVA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Funcion {
    pub inicio: u32,
    pub fin: u32,
    pub desenrollar: u32,
    /// Donde esta la entrada misma (lo que devuelve RtlLookupFunctionEntry).
    pub dir: u64,
}

/// **La RUNTIME_FUNCTION que cubre `rva`**, por busqueda binaria en `.pdata`
/// (ordenada por inicio, como la deja el enlazador).
pub fn buscar(m: &dyn Memoria, base: u64, pdata_rva: u32, pdata_tam: u32, rva: u32) -> Option<Funcion> {
    let n = pdata_tam as usize / 12;
    let (mut lo, mut hi) = (0usize, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        let e = base + pdata_rva as u64 + 12 * mid as u64;
        let (ini, fin) = (m.u32_en(e)?, m.u32_en(e + 4)?);
        if rva < ini {
            hi = mid;
        } else if rva >= fin {
            lo = mid + 1;
        } else {
            return Some(Funcion { inicio: ini, fin, desenrollar: m.u32_en(e + 8)?, dir: e });
        }
    }
    None
}

pub const UNW_FLAG_EHANDLER: u8 = 1;
pub const UNW_FLAG_UHANDLER: u8 = 2;
const UNW_FLAG_CHAININFO: u8 = 4;

const UWOP_PUSH_NONVOL: u8 = 0;
const UWOP_ALLOC_LARGE: u8 = 1;
const UWOP_ALLOC_SMALL: u8 = 2;
const UWOP_SET_FPREG: u8 = 3;
const UWOP_SAVE_NONVOL: u8 = 4;
const UWOP_SAVE_NONVOL_FAR: u8 = 5;
const UWOP_EPILOG: u8 = 6;
const UWOP_SAVE_XMM128: u8 = 8;
const UWOP_SAVE_XMM128_FAR: u8 = 9;
const UWOP_PUSH_MACHFRAME: u8 = 10;

/// Por que un marco no se desenrolla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDesenrolla {
    /// La memoria no se deja leer ahi.
    Memoria(u64),
    /// Una UNWIND_INFO de una version que no es 1 ni 2.
    Version(u8),
    /// Un codigo de desenrollado que no existe.
    Codigo(u8),
    /// La pila no sube: el marco de quien llamo no queda por encima del de
    /// ahora (un contexto roto; seguir seria dar vueltas).
    NoSube(u64),
}

/// Lo que se sabe del marco al desenrollarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Marco {
    /// El "establisher frame": el rsp (o el registro de marco) de su cuerpo.
    pub establecido: u64,
    /// El manejador de lenguaje (VA) y sus datos (VA), si los pidio `tipo`.
    pub manejador: Option<u64>,
    pub datos: u64,
}

/// Cuantos huecos de u16 ocupa un codigo.
fn huecos(op: u8, info: u8) -> usize {
    match op {
        UWOP_ALLOC_LARGE => {
            if info == 0 {
                2
            } else {
                3
            }
        }
        UWOP_SAVE_NONVOL | UWOP_SAVE_XMM128 | UWOP_EPILOG => 2,
        UWOP_SAVE_NONVOL_FAR | UWOP_SAVE_XMM128_FAR => 3,
        _ => 1,
    }
}

/// **`RtlVirtualUnwind`**: el contexto `c` de un marco (su rip dentro de la
/// funcion `f`) pasa a ser el de quien la llamo. `tipo` son las banderas de
/// manejador que interesan (EHANDLER al buscar, UHANDLER al desenrollar).
pub fn un_marco(m: &dyn Memoria, base: u64, f: &Funcion, c: &mut Contexto, tipo: u8) -> Result<Marco, NoDesenrolla> {
    let leer8 = |d: u64| m.u8_en(d).ok_or(NoDesenrolla::Memoria(d));
    let leer16 = |d: u64| m.u16_en(d).ok_or(NoDesenrolla::Memoria(d));
    let leer32 = |d: u64| m.u32_en(d).ok_or(NoDesenrolla::Memoria(d));
    let leer64 = |d: u64| m.u64_en(d).ok_or(NoDesenrolla::Memoria(d));
    let desde = c.rip.wrapping_sub(base + f.inicio as u64);
    let mut info_rva = f.desenrollar;
    let mut primero = true;
    let mut marco = Marco::default();
    let mut maquina = false;
    loop {
        let ui = base + info_rva as u64;
        let (vf, prologo, n, fr) = (leer8(ui)?, leer8(ui + 1)? as u64, leer8(ui + 2)? as usize, leer8(ui + 3)?);
        let (version, banderas) = (vf & 7, vf >> 3);
        if version != 1 && version != 2 {
            return Err(NoDesenrolla::Version(version));
        }
        let (reg_marco, desp_marco) = ((fr & 0xF) as usize, (fr >> 4) as u64 * 16);
        // Dentro del prologo del primer marco, solo cuenta lo ya hecho.
        let en_prologo = primero && banderas & UNW_FLAG_CHAININFO == 0 && desde < prologo;
        let codigos: Vec<u16> = (0..n).map(|i| leer16(ui + 4 + 2 * i as u64)).collect::<Result<_, _>>()?;
        if primero {
            // El establisher frame, antes de tocar nada.
            let puso_marco = reg_marco != 0
                && (!en_prologo || {
                    let mut i = 0;
                    let mut hecho = false;
                    while i < codigos.len() {
                        let (off, op) = (codigos[i] as u8 as u64, (codigos[i] >> 8) as u8 & 0xF);
                        if op == UWOP_SET_FPREG && off <= desde {
                            hecho = true;
                        }
                        i += huecos(op, (codigos[i] >> 12) as u8);
                    }
                    hecho
                });
            marco.establecido = if puso_marco { c.gp[reg_marco].wrapping_sub(desp_marco) } else { c.gp[RSP] };
        }
        let base_marco = marco.establecido;
        let mut i = 0;
        while i < codigos.len() {
            let (off, op, info) = (codigos[i] as u8 as u64, (codigos[i] >> 8) as u8 & 0xF, (codigos[i] >> 12) as u8);
            let paso = huecos(op, info);
            let hueco = |k: usize| codigos.get(i + k).copied().unwrap_or(0) as u64;
            if en_prologo && off > desde {
                i += paso;
                continue;
            }
            match op {
                UWOP_PUSH_NONVOL => {
                    c.gp[info as usize] = leer64(c.gp[RSP])?;
                    c.gp[RSP] += 8;
                }
                UWOP_ALLOC_LARGE => c.gp[RSP] += if info == 0 { hueco(1) * 8 } else { hueco(1) | hueco(2) << 16 },
                UWOP_ALLOC_SMALL => c.gp[RSP] += info as u64 * 8 + 8,
                UWOP_SET_FPREG => c.gp[RSP] = c.gp[reg_marco].wrapping_sub(desp_marco),
                UWOP_SAVE_NONVOL => c.gp[info as usize] = leer64(base_marco + hueco(1) * 8)?,
                UWOP_SAVE_NONVOL_FAR => c.gp[info as usize] = leer64(base_marco + (hueco(1) | hueco(2) << 16))?,
                UWOP_SAVE_XMM128 => c.xmm[info as usize] = m.u128_en(base_marco + hueco(1) * 16).ok_or(NoDesenrolla::Memoria(base_marco))?,
                UWOP_SAVE_XMM128_FAR => c.xmm[info as usize] = m.u128_en(base_marco + (hueco(1) | hueco(2) << 16)).ok_or(NoDesenrolla::Memoria(base_marco))?,
                UWOP_EPILOG => {}
                UWOP_PUSH_MACHFRAME => {
                    if info == 1 {
                        c.gp[RSP] += 8;
                    }
                    c.rip = leer64(c.gp[RSP])?;
                    c.gp[RSP] = leer64(c.gp[RSP] + 24)?;
                    maquina = true;
                }
                otro => return Err(NoDesenrolla::Codigo(otro)),
            }
            i += paso;
        }
        // Detras de los codigos (alineados a 4 bytes): manejador o cadena.
        let detras = ui + 4 + 2 * ((n as u64 + 1) & !1);
        if primero && !en_prologo && banderas & tipo & (UNW_FLAG_EHANDLER | UNW_FLAG_UHANDLER) != 0 {
            marco.manejador = Some(base + leer32(detras)? as u64);
            marco.datos = detras + 4;
        }
        if banderas & UNW_FLAG_CHAININFO == 0 {
            break;
        }
        info_rva = leer32(detras + 8)?;
        primero = false;
    }
    if !maquina {
        c.rip = leer64(c.gp[RSP])?;
        c.gp[RSP] += 8;
    }
    Ok(marco)
}
