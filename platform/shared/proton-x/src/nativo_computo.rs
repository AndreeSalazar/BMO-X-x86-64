//! **El COMPUTO, traducido UNA vez a x86-64** (E2.3b, 05-10) -- lo que
//! EXPRIMIR pide de la CPU mientras la 3060 no corre computo.
//!
//! [carril]  VERDE     bytes de maquina; sellarlos y llamarlos es de la casa
//! [cuesta]  DATO      un salto mal puesto corre otra rama: el JUEZ es el
//!                     interprete, bit a bit (`proton-x-casa/tests/nativo.rs`)
//! [riesgo]  ESPEJO    las mismas cuentas que el interprete de PROMETEO
//!                     (`bmo_prometeo::interprete`), en el mismo orden:
//!                     IEEE-754 al mas cercano (MXCSR de D3D), los enteros
//!                     modulo 2^32, las conversiones de Rust
//! [consumo] DATO      una instruccion o tres por operacion, sobre la memoria
//!
//! nBodyGravity hace 10.000 x 10.000 interacciones por paso: el interprete,
//! a 1,4 millones por segundo sin optimizar, tardaria 73 s en cada uno. Esto
//! es lo de `nativo.rs` (el x86 sin saltos de los dibujos) con lo que el
//! computo necesita (y desde la VELOCIDAD, 05-10, tambien el cuerpo de los
//! dibujos que SALTAN: `nativo.rs` le pone delante su llamada):
//!
//! ```text
//!    la llamada   rdi = los registros del hilo (f32), rsi = el [`Contexto`],
//!                 rdx = el cbuffer; devuelve 0 (acabo), 1 (se paro en una
//!                 barrera: `reanudar` dice donde sigue) o 2 (descartado)
//!    los saltos   `si`, bucles, romper y continuar: saltos de verdad, con
//!                 sus destinos parcheados al cerrar cada uno
//!    la BARRERA   guarda su numero en `reanudar` y vuelve; al llamar otra
//!                 vez, el principio salta detras de ella. Los bucles no
//!                 tienen pila: en codigo de maquina son saltos, y todo el
//!                 estado del hilo vive en sus registros (en memoria)
//!    lo de fuera  la memoria compartida, los SRV y los UAV de bufer y los
//!                 ids del hilo, del contexto; lo que pasa de su vista da 0
//!                 al leer y no se escribe, como en el interprete
//!    lo que LLAMA (X2, 05-10) la matematica de `mates.rs` y, en un dibujo,
//!                 las texturas: un `call` a la funcion de Rust del
//!                 interprete (`nativo_llamadas`), los mismos bits; y el
//!                 cbuffer con fila CALCULADA, mirado contra su medida
//! ```
//!
//! Lo que no traduce (devuelve `None` y el Dispatch va por el interprete):
//! las texturas de un CS (su Dispatch no pone quien las lea) y sus buferes
//! tipados.

use alloc::vec::Vec;

use crate::bufer::Modo;
use crate::dxil::programa::{Comparacion, Conversion, Lectura, Op, OpEntera, Programa, Reg};
use crate::nativo_llamadas::{L_CB_BYTES, L_DATOS, L_MATE, L_TEXTURA};

/// Los SRV y los UAV que ve, como mucho (por ranura).
pub const VISTAS: usize = 8;

/// **Una vista de bufer** para el codigo traducido: sus bytes, su paso (0 si
/// no es estructurada), sus elementos (en una cruda, palabras de 4 bytes) y
/// (E2.4) su contador oculto, si lo tiene.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vista {
    pub datos: *mut u8,
    pub bytes: u64,
    pub paso: u32,
    pub elementos: u32,
    pub contador: *mut u32,
}

impl Vista {
    pub const NULA: Vista = Vista { datos: core::ptr::null_mut(), bytes: 0, paso: 0, elementos: 0, contador: core::ptr::null_mut() };
}

/// **Lo que ve la funcion traducida** (`rsi`): el codigo lo lee por sus
/// desplazamientos (`C_*`, comprobados en las pruebas).
#[repr(C)]
#[derive(Debug)]
pub struct Contexto {
    /// SV_DispatchThreadID (0..3), SV_GroupID (3..6), SV_GroupThreadID
    /// (6..9) y SV_GroupIndex (9).
    pub ids: [u32; 10],
    /// Donde sigue: 0 al empezar; `k` detras de la barrera `k`.
    pub reanudar: u32,
    pub n_compartida: u32,
    pub compartida: *mut u32,
    /// Las entradas y las salidas (`[f32; 4]` por elemento), si las hay.
    pub entradas: *const [f32; 4],
    pub salidas: *mut [f32; 4],
    /// X2 (05-10): a quien llama (la matematica, las texturas) y la medida
    /// del cbuffer: ver [`crate::nativo_llamadas::Llamadas`].
    pub llamadas: *const crate::nativo_llamadas::Llamadas,
    pub srv: [Vista; VISTAS],
    pub uav: [Vista; VISTAS],
}

const C_IDS: i32 = 0;
// Estos cinco, tambien de `nativo.rs`: el VS o el PS con saltos se traduce
// con esto, y su entrada pone un `Contexto` a medias en la pila (solo estos).
pub(crate) const C_REANUDAR: i32 = 40;
pub(crate) const C_COMPARTIDA: i32 = 48;
pub(crate) const C_ENTRADAS: i32 = 56;
pub(crate) const C_SALIDAS: i32 = 64;
pub(crate) const C_LLAMADAS: i32 = 72;
const C_SRV: i32 = 80;
const C_UAV: i32 = C_SRV + VISTA * VISTAS as i32;
/// Lo que mide una [`Vista`].
const VISTA: i32 = 32;
const V_DATOS: i32 = 0;
const V_BYTES: i32 = 8;
const V_PASO: i32 = 16;
const V_ELEMENTOS: i32 = 20;
const V_CONTADOR: i32 = 24;

// La firma de la funcion traducida es `unsafe extern "sysv64" fn(*mut f32,
// *mut Contexto, *const u8) -> u32` (el ABI de System V, el de los punteros
// de arriba, que sabe llamar el Rust soft-float de Ring 3). El tipo vive en
// la casa (`FuncionComputo`): esto es `capa: puro`, y sellar y llamar es suyo.

/// Lo que devuelve: acabo, se paro en una barrera, o el pixel se tiro.
pub const ACABO: u32 = 0;
pub const BARRERA: u32 = 1;
pub const DESCARTADO: u32 = 2;
/// X3 (06-10): un dibujo se paro en una DERIVADA (o en varias seguidas):
/// `reanudar` dice en cual ([`paradas`]). La casa resta los carriles del
/// cuadro de 2x2, pone el resultado en sus registros y lo vuelve a llamar.
/// A10 (06-10): y un hilo de computo, en una operacion de OLA (`Wave*`,
/// `Quad*`): [`despachar`] la resuelve con los carriles de su ola, como el
/// interprete (`dxil::carriles`), y lo vuelve a llamar.
pub const OLA: u32 = 3;

/// A10 (06-10): una operacion de ola de verdad (no una derivada), la que
/// en el computo se para para resolverse con su ola.
fn es_ola_de_computo(o: &Op) -> bool {
    matches!(o, Op::Ola { que, .. } if !matches!(que, crate::dxil::olas::Ola::Derivada { .. }))
}

/// **Los puntos donde se para el cuerpo** (X3, 06-10), en orden: el `k`
/// que deja en `reanudar` es la posicion `k - 1` de esta lista. Cada uno,
/// los indices de sus operaciones: una barrera, o (en un dibujo) una racha
/// de derivadas SEGUIDAS (`Op::Ola` con `Ola::Derivada`), que se resuelven
/// juntas y en su orden.
pub fn paradas(p: &Programa, dibujo: bool) -> Vec<Vec<usize>> {
    let deriva = |o: &Op| dibujo && matches!(o, Op::Ola { que: crate::dxil::olas::Ola::Derivada { .. }, .. });
    let mut v: Vec<Vec<usize>> = Vec::new();
    for (k, o) in p.ops.iter().enumerate() {
        // A10: en el computo, cada operacion de ola, su punto.
        if matches!(o, Op::Barrera) || (!dibujo && es_ola_de_computo(o)) {
            v.push(alloc::vec![k]);
        } else if deriva(o) {
            match v.last_mut() {
                Some(g) if g.last() == Some(&(k - 1)) && deriva(&p.ops[k - 1]) => g.push(k),
                _ => v.push(alloc::vec![k]),
            }
        }
    }
    v
}

// Los registros de x86-64 (su numero en ModRM, con el bit 3 en REX).
const RAX: u8 = 0;
const RCX: u8 = 1;
const RDX: u8 = 2;
const RBX: u8 = 3;
const RSP: u8 = 4;
const RSI: u8 = 6;
const RDI: u8 = 7;
const R8: u8 = 8;
const R9: u8 = 9;
const R10: u8 = 10;
const R11: u8 = 11;
const R12: u8 = 12;
const R13: u8 = 13;
const R14: u8 = 14;
const R15: u8 = 15;

// Las condiciones de Jcc / SETcc / CMOVcc.
const CC_B: u8 = 0x2;
const CC_AE: u8 = 0x3;
const CC_E: u8 = 0x4;
const CC_NE: u8 = 0x5;
const CC_BE: u8 = 0x6;
const CC_A: u8 = 0x7;
const CC_P: u8 = 0xA;
const CC_NP: u8 = 0xB;
const CC_L: u8 = 0xC;
const CC_GE: u8 = 0xD;
const CC_LE: u8 = 0xE;
const CC_G: u8 = 0xF;

// Las de SSE escalares (F3 0F op).
const MOVSS: u8 = 0x10;
const MOVSS_A_MEM: u8 = 0x11;
const SQRTSS: u8 = 0x51;
const ADDSS: u8 = 0x58;
const MULSS: u8 = 0x59;
const SUBSS: u8 = 0x5C;
const MINSS: u8 = 0x5D;
const DIVSS: u8 = 0x5E;
const MAXSS: u8 = 0x5F;

/// Donde estan los registros del hilo, el contexto, el cbuffer y la memoria
/// compartida durante toda la funcion (los guarda el prologo).
const REGS: u8 = RBX;
const CTX: u8 = R12;
const CB: u8 = R13;
const COMP: u8 = R14;

struct Emisor {
    b: Vec<u8>,
}

impl Emisor {
    fn rex(&mut self, w: bool, reg: u8, index: u8, base: u8) {
        let r = 0x40 | (w as u8) << 3 | (reg >> 3 & 1) << 2 | (index >> 3 & 1) << 1 | (base >> 3 & 1);
        if r != 0x40 {
            self.b.push(r);
        }
    }

    /// `[prefijo] [REX] opcode /reg [base + disp32]`.
    fn mem(&mut self, prefijo: Option<u8>, w: bool, opc: &[u8], reg: u8, base: u8, disp: i32) {
        if let Some(p) = prefijo {
            self.b.push(p);
        }
        self.rex(w, reg, 0, base);
        self.b.extend_from_slice(opc);
        self.b.push(0x80 | (reg & 7) << 3 | (base & 7));
        if base & 7 == RSP {
            self.b.push(0x24);
        }
        self.b.extend_from_slice(&disp.to_le_bytes());
    }

    /// `[prefijo] [REX] opcode /reg [base + index * 4 + disp32]`.
    fn mem_x4(&mut self, prefijo: Option<u8>, w: bool, opc: &[u8], reg: u8, base: u8, index: u8, disp: i32) {
        if let Some(p) = prefijo {
            self.b.push(p);
        }
        self.rex(w, reg, index, base);
        self.b.extend_from_slice(opc);
        self.b.push(0x80 | (reg & 7) << 3 | 4);
        self.b.push(2 << 6 | (index & 7) << 3 | (base & 7));
        self.b.extend_from_slice(&disp.to_le_bytes());
    }

    /// `[REX] opcode /reg [base + index * 1 + disp32]` (`w`: de 64 bits).
    fn mem_x1(&mut self, w: bool, opc: &[u8], reg: u8, base: u8, index: u8, disp: i32) {
        self.rex(w, reg, index, base);
        self.b.extend_from_slice(opc);
        self.b.push(0x80 | (reg & 7) << 3 | 4);
        self.b.push((index & 7) << 3 | (base & 7));
        self.b.extend_from_slice(&disp.to_le_bytes());
    }

    /// `[prefijo] [REX] opcode` entre registros (`reg` en ModRM.reg, `rm` en ModRM.rm).
    fn rr(&mut self, prefijo: Option<u8>, w: bool, opc: &[u8], reg: u8, rm: u8) {
        if let Some(p) = prefijo {
            self.b.push(p);
        }
        self.rex(w, reg, 0, rm);
        self.b.extend_from_slice(opc);
        self.b.push(0xC0 | (reg & 7) << 3 | (rm & 7));
    }

    // -- Lo de siempre ----------------------------------------------------------------

    /// `mov r32, [regs + 4 * x]`.
    fn cargar(&mut self, r: u8, x: Reg) {
        self.mem(None, false, &[0x8B], r, REGS, 4 * x as i32);
    }

    /// `mov [regs + 4 * x], r32`.
    fn guardar(&mut self, x: Reg, r: u8) {
        self.mem(None, false, &[0x89], r, REGS, 4 * x as i32);
    }

    /// `mov r32, imm32`.
    fn inmediato(&mut self, r: u8, v: u32) {
        if r >= 8 {
            self.b.push(0x41);
        }
        self.b.push(0xB8 + (r & 7));
        self.b.extend_from_slice(&v.to_le_bytes());
    }

    /// `movss xmm, [regs + 4 * x]` / `movss [regs + 4 * x], xmm`.
    fn cargar_x(&mut self, xmm: u8, x: Reg) {
        self.mem(Some(0xF3), false, &[0x0F, MOVSS], xmm, REGS, 4 * x as i32);
    }

    fn guardar_x(&mut self, x: Reg, xmm: u8) {
        self.mem(Some(0xF3), false, &[0x0F, MOVSS_A_MEM], xmm, REGS, 4 * x as i32);
    }

    /// `op xmm, [regs + 4 * x]` (escalar).
    fn sse_mem(&mut self, op: u8, xmm: u8, x: Reg) {
        self.mem(Some(0xF3), false, &[0x0F, op], xmm, REGS, 4 * x as i32);
    }

    fn sse(&mut self, op: u8, dst: u8, src: u8) {
        self.rr(Some(0xF3), false, &[0x0F, op], dst, src);
    }

    /// Un `u32` en un xmm (por eax).
    fn constante_x(&mut self, xmm: u8, bits: u32) {
        self.inmediato(RAX, bits);
        self.rr(Some(0x66), false, &[0x0F, 0x6E], xmm, RAX); // movd xmm, eax
    }

    /// `setcc al; movzx eax, al; neg eax`: 0xFFFFFFFF si se cumple, 0 si no.
    fn booleano(&mut self, cc: u8) {
        self.b.extend_from_slice(&[0x0F, 0x90 | cc, 0xC0]);
        self.b.extend_from_slice(&[0x0F, 0xB6, 0xC0]);
        self.b.extend_from_slice(&[0xF7, 0xD8]);
    }

    /// `jmp rel32` a ninguna parte: devuelve donde parchearlo.
    fn salto(&mut self) -> usize {
        self.b.push(0xE9);
        self.b.extend_from_slice(&[0; 4]);
        self.b.len() - 4
    }

    /// `jcc rel32` a ninguna parte.
    fn salto_si(&mut self, cc: u8) -> usize {
        self.b.extend_from_slice(&[0x0F, 0x80 | cc]);
        self.b.extend_from_slice(&[0; 4]);
        self.b.len() - 4
    }

    /// El salto de `en` va a `destino`.
    fn parchear(&mut self, en: usize, destino: usize) {
        let rel = destino as i64 - (en as i64 + 4);
        self.b[en..en + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    /// El salto de `en` va a aqui.
    fn aqui(&mut self, en: usize) {
        let d = self.b.len();
        self.parchear(en, d);
    }

    /// `jmp` a `destino` (hacia atras).
    fn saltar_a(&mut self, destino: usize) {
        let en = self.salto();
        self.parchear(en, destino);
    }

    fn push(&mut self, r: u8) {
        if r >= 8 {
            self.b.push(0x41);
        }
        self.b.push(0x50 + (r & 7));
    }

    fn pop(&mut self, r: u8) {
        if r >= 8 {
            self.b.push(0x41);
        }
        self.b.push(0x58 + (r & 7));
    }

    /// FMin/FMax de D3D: `minss/maxss` con el orden (b, a), y si `a` es NaN,
    /// `b` (lo de `nativo.rs`).
    fn min_max(&mut self, op: u8, d: Reg, a: Reg, b: Reg) {
        self.cargar_x(0, b);
        self.sse_mem(op, 0, a);
        self.cargar_x(1, a);
        self.b.extend_from_slice(&[0xF3, 0x0F, 0xC2, 0xC9, 0x03]); // cmpunordss xmm1, xmm1
        self.cargar_x(2, b);
        self.rr(None, false, &[0x0F, 0x54], 2, 1); // andps  xmm2, xmm1
        self.rr(None, false, &[0x0F, 0x55], 1, 0); // andnps xmm1, xmm0
        self.rr(None, false, &[0x0F, 0x56], 1, 2); // orps   xmm1, xmm2
        self.guardar_x(d, 1);
    }

    /// `eax` = el `u32` de una vista del contexto (`campo` dentro de ella).
    fn campo(&mut self, r: u8, w: bool, vista: i32, campo: i32) {
        self.mem(None, w, &[0x8B], r, CTX, vista + campo);
    }

    /// **Leer 4 palabras de un bufer** a `d..d+4`: las que caen enteras en
    /// [desde, hasta) (rax y r10, en bytes), de `rdx` (los datos); las demas,
    /// 0. Lo de `Bufer::palabras`.
    fn cuatro_palabras(&mut self, d: Reg) {
        for k in 0..4i32 {
            // ecx = 0; rcx' = rax + 4k + 4; si rcx' <= r10: ecx = [rdx + rax + 4k]
            self.rr(None, false, &[0x31], RCX, RCX); // xor ecx, ecx
            self.mem(None, true, &[0x8D], R11, RAX, 4 * k + 4); // lea r11, [rax + 4k + 4]
            self.rr(None, true, &[0x39], R10, R11); // cmp r11, r10
            let fuera = self.salto_si(CC_A);
            self.mem_x1(false, &[0x8B], RCX, RDX, RAX, 4 * k); // mov ecx, [rdx + rax + 4k]
            self.aqui(fuera);
            self.guardar(d + k as Reg, RCX);
        }
    }

    /// **La vista de un bufer en `[desde, hasta)`** (rax y r10, en bytes) y
    /// sus datos en rdx, para el elemento `i` y el desplazamiento `desp`
    /// (registros), segun su modo. Si el elemento no esta, salta a lo que
    /// devuelve (lo que la llama pone detras).
    fn ventana(&mut self, vista: i32, modo: Modo, i: Reg, desp: Reg) -> Option<Vec<usize>> {
        let mut fuera = Vec::new();
        self.cargar(RAX, i); // eax = i (rax, sin signo)
        self.cargar(RSI, desp); // esi = desp
        match modo {
            Modo::Estructurado => {
                self.campo(R8, false, vista, V_ELEMENTOS);
                self.rr(None, false, &[0x39], R8, RAX); // cmp eax, r8d
                fuera.push(self.salto_si(CC_AE));
                self.campo(R9, false, vista, V_PASO);
                self.rr(None, false, &[0x85], R9, R9); // test r9d, r9d
                fuera.push(self.salto_si(CC_E));
                self.rr(None, true, &[0x0F, 0xAF], RAX, R9); // imul rax, r9: el principio del elemento
                self.mem_x1(true, &[0x8D], R10, RAX, R9, 0); // lea r10, [rax + r9]: su final
            }
            Modo::Crudo => {
                // hasta = elementos * 4
                self.campo(R10, false, vista, V_ELEMENTOS);
                self.b.extend_from_slice(&[0x49, 0xC1, 0xE2, 0x02]); // shl r10, 2
            }
            Modo::Tipado | Modo::Textura => return None,
        }
        // hasta = min(hasta, bytes)
        self.campo(R11, true, vista, V_BYTES);
        self.rr(None, true, &[0x39], R11, R10); // cmp r10, r11
        self.rr(None, true, &[0x0F, 0x40 | CC_A], R10, R11); // cmova r10, r11
        // desde += desp
        self.rr(None, true, &[0x01], RSI, RAX); // add rax, rsi
        self.campo(RDX, true, vista, V_DATOS);
        Some(fuera)
    }

    // -- Lo que LLAMA (X2, 05-10): `nativo_llamadas` ---------------------------------

    /// `mov rax, [r12 + llamadas]; call [rax + desp]`. Lo que vive en un
    /// registro de los que se pisan (rax, rcx, rdx, rsi, rdi, r8..r11, los
    /// xmm) no sobrevive: aqui todo esta en memoria o en rbx, r12..r14.
    fn llamar(&mut self, desp: i32) {
        self.mem(None, true, &[0x8B], RAX, CTX, C_LLAMADAS);
        self.mem(None, false, &[0xFF], 2, RAX, desp); // call qword [rax + desp]
    }

    /// `d = mate(cual, a)`, la de `Mate::aplicar` (edi, esi -> eax).
    fn mate(&mut self, d: Reg, a: Reg, cual: u32) {
        self.inmediato(RDI, cual);
        self.cargar(RSI, a);
        self.llamar(L_MATE);
        self.guardar(d, RAX);
    }

    /// La operacion `k` (una lectura de textura) por la llamada de la casa:
    /// `textura(datos, registros, k)` (rdi, rsi, edx).
    fn textura(&mut self, k: u32) {
        self.mem(None, true, &[0x8B], RAX, CTX, C_LLAMADAS);
        self.mem(None, true, &[0x8B], RDI, RAX, L_DATOS);
        self.rr(None, true, &[0x89], REGS, RSI); // mov rsi, rbx
        self.inmediato(RDX, k);
        self.mem(None, false, &[0xFF], 2, RAX, L_TEXTURA);
    }

    /// `d..d+4` = la fila `fila + i` del cbuffer si `i < filas`, cada
    /// palabra solo si cabe en su medida (`Llamadas::cb_bytes`); lo demas,
    /// 0. Lo del interprete en `ConstantesEn`.
    fn constantes_en(&mut self, d: Reg, fila: u16, filas: u16, i: Reg) {
        self.cargar(RAX, i); // rax = i, sin signo
        self.b.push(0x3D); // cmp eax, filas
        self.b.extend_from_slice(&(filas as u32).to_le_bytes());
        let fuera = self.salto_si(CC_AE);
        self.mem(None, true, &[0x8D], RAX, RAX, fila as i32); // lea rax, [rax + fila]
        self.b.extend_from_slice(&[0x48, 0xC1, 0xE0, 0x04]); // shl rax, 4: en bytes
        self.mem(None, true, &[0x8B], R10, CTX, C_LLAMADAS);
        self.mem(None, true, &[0x8B], R10, R10, L_CB_BYTES); // r10 = hasta
        self.rr(None, true, &[0x89], CB, RDX); // mov rdx, r13
        self.cuatro_palabras(d);
        let listo = self.salto();
        self.aqui(fuera);
        self.rr(None, false, &[0x31], RCX, RCX);
        for k in 0..4 {
            self.guardar(d + k, RCX);
        }
        self.aqui(listo);
    }
}

/// Los registros que algo escribe (para saber cuales son constantes).
fn escritos(op: &Op, mut f: impl FnMut(Reg)) {
    let cuatro = |d: Reg, f: &mut dyn FnMut(Reg)| (0..4).for_each(|k| f(d + k));
    match *op {
        Op::Constantes { d, .. } | Op::ConstantesEn { d, .. } | Op::Muestra { d, .. } | Op::Lee { d, .. } | Op::LeeUav { d, .. } | Op::MedidasUav { d, .. } => cuatro(d, &mut f),
        Op::Entrada { d, .. }
        | Op::EntradaDe { d, .. }
        | Op::Mul { d, .. }
        | Op::Add { d, .. }
        | Op::Sub { d, .. }
        | Op::Div { d, .. }
        | Op::Mad { d, .. }
        | Op::Dot { d, .. }
        | Op::Rsqrt { d, .. }
        | Op::Sqrt { d, .. }
        | Op::Saturate { d, .. }
        | Op::Abs { d, .. }
        | Op::Mate { d, .. }
        | Op::Min { d, .. }
        | Op::Max { d, .. }
        | Op::IdHilo { d, .. }
        | Op::LeeCompartida { d, .. }
        | Op::Compara { d, .. }
        | Op::Elige { d, .. }
        | Op::Copia { d, .. }
        | Op::SumaEntera { d, .. }
        | Op::Entera { d, .. }
        | Op::Convierte { d, .. }
        | Op::Contador { d, .. }
        | Op::Atomico { d, .. }
        | Op::AtomicoCompartido { d, .. }
        // X3: la escribe la casa al pararse (no es una constante).
        | Op::Ola { d, .. }
        | Op::LeeIndexado { d, .. } => f(d),
        Op::EscribeIndexado { base, n, .. } => (0..n).for_each(|k| f(base + k)),
        _ => {}
    }
}

/// **Traducir un programa de computo** a x86-64. `None`: algo que todavia no
/// sabe (va por el interprete). Sus texturas, no: su Dispatch no pone quien
/// las lea (`Llamadas::textura` a 0).
pub fn compilar(p: &Programa) -> Option<Vec<u8>> {
    compilar_con(p, false)
}

/// **Las ranuras de UAV que van por la LLAMADA** (06-10): las que el
/// codigo no sabe tocar solo -- las de una textura (`Modo::Textura`: sus
/// rebanadas, sus formatos) y las que tienen atomicos o `GetDimensions` --
/// van ENTERAS por `operar_uav` del interprete (cada operacion sobre
/// ellas, tambien las sencillas: asi nadie mas toca su memoria). Quien
/// despacha se las da a la llamada y no al `Contexto` (`Vista::NULA`).
pub fn uavs_llamados(p: &Programa) -> Vec<bool> {
    let mut v = alloc::vec![false; VISTAS];
    for o in &p.ops {
        let u = match *o {
            Op::Atomico { u, .. } | Op::MedidasUav { u, .. } => u,
            Op::LeeUav { u, modo: Modo::Textura, .. } | Op::EscribeUav { u, modo: Modo::Textura, .. } => u,
            _ => continue,
        };
        if let Some(x) = v.get_mut(u as usize) {
            *x = true;
        }
    }
    v
}

/// **El cuerpo de un dibujo** (`nativo.rs` le pone delante su llamada): lo
/// de [`compilar`], y sus texturas por la llamada de la casa (X2, 05-10).
pub(crate) fn compilar_dibujo(p: &Programa) -> Option<Vec<u8>> {
    compilar_con(p, true)
}

fn compilar_con(p: &Programa, dibujo: bool) -> Option<Vec<u8>> {
    // Los registros que nadie escribe son sus iniciales: constantes.
    let mut escrito = alloc::vec![false; p.iniciales.len()];
    for op in &p.ops {
        escritos(op, |r| {
            if let Some(x) = escrito.get_mut(r as usize) {
                *x = true;
            }
        });
    }
    let constante = |r: Reg| (!escrito.get(r as usize).copied().unwrap_or(true)).then(|| p.iniciales[r as usize].to_bits());
    // 06-10: en el computo, las ranuras de UAV que van por la llamada. A10
    // (06-10): en un dibujo, TODAS (su entrada no pone vistas): cada
    // operacion de UAV, por `operar_uav` del interprete (la casa la llama).
    let llamados = if dibujo { alloc::vec![false; VISTAS] } else { uavs_llamados(p) };
    let llamado = |u: u8| dibujo || llamados.get(u as usize).copied().unwrap_or(false);
    // Las barreras y (X3, en un dibujo) las rachas de derivadas: cada una,
    // un punto donde para y por donde sigue.
    let puntos = paradas(p, dibujo);
    let barreras = puntos.len();
    // A10: llevar las vueltas de los bucles (solo el computo con olas).
    let vueltas = !dibujo && p.olas_propias();
    if vueltas && p.ops.iter().any(|o| matches!(o, Op::Ola { que: crate::dxil::olas::Ola::Derivada { .. }, .. })) {
        // Derivadas en el computo: por el interprete.
        return None;
    }
    let mut punto_de = alloc::vec![None; p.ops.len()];
    for (n, g) in puntos.iter().enumerate() {
        punto_de[g[0]] = Some(n + 1);
    }
    let mut e = Emisor { b: Vec::with_capacity(24 * p.ops.len() + 256) };
    // Prologo: los cinco que hay que conservar, el MXCSR, y los cuatro
    // punteros de la funcion en sus sitios.
    for r in [RBX, R12, R13, R14, R15] {
        e.push(r);
    }
    e.b.extend_from_slice(&[0x48, 0x83, 0xEC, 0x10]); // sub rsp, 16
    crate::nativo::mxcsr_al_entrar(&mut e.b); // el de D3D, si no lo es ya
    e.rr(None, true, &[0x89], RDI, REGS); // mov rbx, rdi
    e.rr(None, true, &[0x89], RSI, CTX); // mov r12, rsi
    e.rr(None, true, &[0x89], RDX, CB); // mov r13, rdx
    e.mem(None, true, &[0x8B], COMP, CTX, C_COMPARTIDA); // mov r14, [r12 + compartida]
    // Seguir detras de la barrera `reanudar`.
    let mut reanudar = Vec::with_capacity(barreras);
    if barreras > 0 {
        e.mem(None, false, &[0x8B], RAX, CTX, C_REANUDAR);
        for k in 1..=barreras as u32 {
            e.b.push(0x3D); // cmp eax, imm32
            e.b.extend_from_slice(&k.to_le_bytes());
            reanudar.push(e.salto_si(CC_E));
        }
    }
    // Los saltos por parchear: los `si` abiertos (su salto) y los bucles
    // abiertos (su principio y sus `romper`).
    let mut sis: Vec<usize> = Vec::new();
    let mut bucles: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut al_final: Vec<usize> = Vec::new();
    for (k, op) in p.ops.iter().enumerate() {
        match *op {
            // X2 (05-10): en un dibujo, TODA lectura de textura (y de bufer:
            // su entrada no pone vistas) por la llamada de la casa; la
            // matematica, por `Mate::aplicar`; y el cbuffer con fila calculada.
            // 06-10: y en el computo tambien (su Dispatch pone las llamadas),
            // salvo los buferes, que el computo lee solo (abajo).
            Op::Muestra { .. } | Op::EligeTextura { .. } => e.textura(k as u32),
            Op::Lee { como, .. } if dibujo || !matches!(como, Lectura::Bufer(_)) => e.textura(k as u32),
            // 06-10: lo de una ranura de UAV llamada, por la misma llamada.
            Op::LeeUav { u, .. } | Op::EscribeUav { u, .. } | Op::Atomico { u, .. } | Op::MedidasUav { u, .. } | Op::Contador { u, .. } if llamado(u) => e.textura(k as u32),
            // A10 (06-10): una OLA en el computo (`Wave*`, `Quad*`): se para
            // (OLA) y `despachar` la resuelve con los carriles de su ola; quien
            // va antes lo dicen sus VUELTAS (ver `Op::Bucle`).
            Op::Ola { .. } if !dibujo && es_ola_de_computo(op) => {
                let n = punto_de[k]?;
                e.mem(None, false, &[0xC7], 0, CTX, C_REANUDAR); // mov dword [r12 + reanudar], n
                e.b.extend_from_slice(&(n as u32).to_le_bytes());
                e.inmediato(RAX, OLA);
                al_final.push(e.salto());
                let aqui = e.b.len();
                e.parchear(reanudar[n - 1], aqui);
            }
            // X3 (06-10): una racha de derivadas para el carril (OLA); la casa
            // resta los del cuadro y lo vuelve a llamar. Las de despues de la
            // primera de la racha, ya resueltas con ella.
            Op::Ola { que: crate::dxil::olas::Ola::Derivada { .. }, .. } if dibujo => {
                if let Some(n) = punto_de[k] {
                    e.mem(None, false, &[0xC7], 0, CTX, C_REANUDAR); // mov dword [r12 + reanudar], n
                    e.b.extend_from_slice(&(n as u32).to_le_bytes());
                    e.inmediato(RAX, OLA);
                    al_final.push(e.salto());
                    let aqui = e.b.len();
                    e.parchear(reanudar[n - 1], aqui);
                }
            }
            Op::Mate { d, a, f } => e.mate(d, a, crate::nativo_llamadas::indice_mate(f)?),
            Op::ConstantesEn { d, fila, filas, i, .. } => e.constantes_en(d, fila, filas, i),
            Op::Mul { d, a, b } | Op::Add { d, a, b } | Op::Sub { d, a, b } | Op::Div { d, a, b } => {
                let x = match op {
                    Op::Mul { .. } => MULSS,
                    Op::Add { .. } => ADDSS,
                    Op::Sub { .. } => SUBSS,
                    _ => DIVSS,
                };
                e.cargar_x(0, a);
                e.sse_mem(x, 0, b);
                e.guardar_x(d, 0);
            }
            Op::Mad { d, a, b, c } => {
                // Dos redondeos, como el interprete.
                e.cargar_x(0, a);
                e.sse_mem(MULSS, 0, b);
                e.sse_mem(ADDSS, 0, c);
                e.guardar_x(d, 0);
            }
            Op::Dot { d, n, a, b } => {
                e.cargar_x(0, a[0]);
                e.sse_mem(MULSS, 0, b[0]);
                for k in 1..n as usize {
                    e.cargar_x(1, a[k]);
                    e.sse_mem(MULSS, 1, b[k]);
                    e.sse(ADDSS, 0, 1);
                }
                e.guardar_x(d, 0);
            }
            Op::Sqrt { d, a } => {
                e.sse_mem(SQRTSS, 0, a);
                e.guardar_x(d, 0);
            }
            Op::Rsqrt { d, a } => {
                e.sse_mem(SQRTSS, 1, a);
                e.constante_x(0, 1.0f32.to_bits());
                e.sse(DIVSS, 0, 1);
                e.guardar_x(d, 0);
            }
            Op::Saturate { d, a } => {
                e.cargar_x(0, a);
                e.constante_x(1, 0);
                e.sse(MAXSS, 0, 1);
                e.constante_x(1, 1.0f32.to_bits());
                e.sse(MINSS, 0, 1);
                e.guardar_x(d, 0);
            }
            Op::Abs { d, a } => {
                e.cargar(RAX, a);
                e.b.extend_from_slice(&[0x25]); // and eax, imm32
                e.b.extend_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
                e.guardar(d, RAX);
            }
            Op::Min { d, a, b } => e.min_max(MINSS, d, a, b),
            Op::Max { d, a, b } => e.min_max(MAXSS, d, a, b),
            Op::Compara { d, a, b, como, entero: false } => {
                e.cargar_x(0, a);
                e.cargar_x(1, b);
                use Comparacion::*;
                match como {
                    Menor | MenorSinSigno => {
                        e.rr(None, false, &[0x0F, 0x2E], 1, 0); // ucomiss xmm1, xmm0
                        e.booleano(CC_A);
                    }
                    MenorIgual | MenorIgualSinSigno => {
                        e.rr(None, false, &[0x0F, 0x2E], 1, 0);
                        e.booleano(CC_AE);
                    }
                    Mayor | MayorSinSigno => {
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1); // ucomiss xmm0, xmm1
                        e.booleano(CC_A);
                    }
                    MayorIgual | MayorIgualSinSigno => {
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1);
                        e.booleano(CC_AE);
                    }
                    Igual | Distinto => {
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1);
                        // Igual: ZF y no PF. Distinto: no ZF o PF.
                        let (z, p_, junta) = if como == Igual { (CC_E, CC_NP, 0x20) } else { (CC_NE, CC_P, 0x08) };
                        e.b.extend_from_slice(&[0x0F, 0x90 | z, 0xC0]); // setcc al
                        e.b.extend_from_slice(&[0x0F, 0x90 | p_, 0xC1]); // setcc cl
                        e.b.extend_from_slice(&[junta, 0xC8]); // and/or al, cl
                        e.b.extend_from_slice(&[0x0F, 0xB6, 0xC0, 0xF7, 0xD8]); // movzx eax, al; neg eax
                    }
                }
                e.guardar(d, RAX);
            }
            Op::Compara { d, a, b, como, entero: true } => {
                e.cargar(RAX, a);
                e.cargar(RCX, b);
                e.rr(None, false, &[0x39], RCX, RAX); // cmp eax, ecx
                use Comparacion::*;
                let cc = match como {
                    Menor => CC_L,
                    MenorIgual => CC_LE,
                    Mayor => CC_G,
                    MayorIgual => CC_GE,
                    Igual => CC_E,
                    Distinto => CC_NE,
                    MenorSinSigno => CC_B,
                    MenorIgualSinSigno => CC_BE,
                    MayorSinSigno => CC_A,
                    MayorIgualSinSigno => CC_AE,
                };
                e.booleano(cc);
                e.guardar(d, RAX);
            }
            Op::Elige { d, c, a, b } => {
                e.cargar(RAX, a);
                e.cargar(RCX, b);
                e.cargar(RDX, c);
                e.rr(None, false, &[0x85], RDX, RDX); // test edx, edx
                e.rr(None, false, &[0x0F, 0x40 | CC_E], RAX, RCX); // cmovz eax, ecx
                e.guardar(d, RAX);
            }
            Op::Copia { d, a } => {
                e.cargar(RAX, a);
                e.guardar(d, RAX);
            }
            Op::SumaEntera { d, a, b } => {
                e.cargar(RAX, a);
                e.mem(None, false, &[0x03], RAX, REGS, 4 * b as i32); // add eax, [b]
                e.guardar(d, RAX);
            }
            Op::Entera { d, a, b, op: o } => {
                e.cargar(RAX, a);
                e.cargar(RCX, b);
                match o {
                    OpEntera::Resta => e.rr(None, false, &[0x29], RCX, RAX),
                    OpEntera::Mul => e.rr(None, false, &[0x0F, 0xAF], RAX, RCX),
                    OpEntera::Shl => e.b.extend_from_slice(&[0xD3, 0xE0]), // shl eax, cl
                    OpEntera::ShrL => e.b.extend_from_slice(&[0xD3, 0xE8]), // shr eax, cl
                    OpEntera::ShrA => e.b.extend_from_slice(&[0xD3, 0xF8]), // sar eax, cl
                    OpEntera::Y => e.rr(None, false, &[0x21], RCX, RAX),
                    OpEntera::O => e.rr(None, false, &[0x09], RCX, RAX),
                    OpEntera::OX => e.rr(None, false, &[0x31], RCX, RAX),
                    OpEntera::MinS | OpEntera::MaxS | OpEntera::MinU | OpEntera::MaxU => {
                        e.rr(None, false, &[0x39], RAX, RCX); // cmp ecx, eax
                        let cc = match o {
                            OpEntera::MinS => CC_L,
                            OpEntera::MaxS => CC_G,
                            OpEntera::MinU => CC_B,
                            _ => CC_A,
                        };
                        e.rr(None, false, &[0x0F, 0x40 | cc], RAX, RCX); // cmovcc eax, ecx
                    }
                    OpEntera::DivU | OpEntera::RemU | OpEntera::DivS | OpEntera::RemS => {
                        // Por 0: 0xFFFFFFFF (D3D). i32::MIN / -1: lo que
                        // Rust da (wrapping): -a y resto 0; idiv daria #DE.
                        let resto = matches!(o, OpEntera::RemU | OpEntera::RemS);
                        e.rr(None, false, &[0x85], RCX, RCX);
                        let no_cero = e.salto_si(CC_NE);
                        e.inmediato(RAX, u32::MAX);
                        let listo1 = e.salto();
                        e.aqui(no_cero);
                        let mut listos = alloc::vec![listo1];
                        if matches!(o, OpEntera::DivS | OpEntera::RemS) {
                            e.b.extend_from_slice(&[0x83, 0xF9, 0xFF]); // cmp ecx, -1
                            let no_menos_uno = e.salto_si(CC_NE);
                            if resto {
                                e.rr(None, false, &[0x31], RAX, RAX); // xor eax, eax
                            } else {
                                e.b.extend_from_slice(&[0xF7, 0xD8]); // neg eax
                            }
                            listos.push(e.salto());
                            e.aqui(no_menos_uno);
                            e.b.push(0x99); // cdq
                            e.b.extend_from_slice(&[0xF7, 0xF9]); // idiv ecx
                        } else {
                            e.rr(None, false, &[0x31], RDX, RDX); // xor edx, edx
                            e.b.extend_from_slice(&[0xF7, 0xF1]); // div ecx
                        }
                        if resto {
                            e.rr(None, false, &[0x89], RDX, RAX); // mov eax, edx
                        }
                        for l in listos {
                            e.aqui(l);
                        }
                    }
                }
                e.guardar(d, RAX);
            }
            Op::Convierte { d, a, como } => {
                match como {
                    Conversion::EnteroAFloat => {
                        e.mem(Some(0xF3), false, &[0x0F, 0x2A], 0, REGS, 4 * a as i32); // cvtsi2ss xmm0, [a]
                        e.guardar_x(d, 0);
                    }
                    Conversion::SinSignoAFloat => {
                        e.cargar(RAX, a); // zero-extiende a rax
                        e.rr(Some(0xF3), true, &[0x0F, 0x2A], 0, RAX); // cvtsi2ss xmm0, rax
                        e.guardar_x(d, 0);
                    }
                    Conversion::FloatAEntero => {
                        // Rust `as i32`: NaN 0, lo que no cabe al limite.
                        e.cargar_x(0, a);
                        e.rr(None, false, &[0x31], RCX, RCX); // xor ecx, ecx
                        e.rr(None, false, &[0x0F, 0x2E], 0, 0); // ucomiss xmm0, xmm0
                        let nan = e.salto_si(CC_P);
                        e.inmediato(RCX, i32::MAX as u32);
                        e.constante_x(1, 2147483648.0f32.to_bits());
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1); // ucomiss xmm0, xmm1
                        let grande = e.salto_si(CC_AE);
                        e.rr(Some(0xF3), false, &[0x0F, 0x2C], RCX, 0); // cvttss2si ecx, xmm0
                        e.aqui(nan);
                        e.aqui(grande);
                        e.guardar(d, RCX);
                    }
                    Conversion::FloatASinSigno => {
                        // Rust `as u32`: NaN y lo de <= 0, 0; >= 2^32, el maximo.
                        e.cargar_x(0, a);
                        e.rr(None, false, &[0x31], RCX, RCX);
                        e.constante_x(1, 0);
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1); // ucomiss xmm0, 0
                        let cero = e.salto_si(CC_BE); // NaN (CF) o <= 0
                        e.inmediato(RCX, u32::MAX);
                        e.constante_x(1, 4294967296.0f32.to_bits());
                        e.rr(None, false, &[0x0F, 0x2E], 0, 1);
                        let grande = e.salto_si(CC_AE);
                        e.rr(Some(0xF3), true, &[0x0F, 0x2C], RCX, 0); // cvttss2si rcx, xmm0
                        e.aqui(cero);
                        e.aqui(grande);
                        e.guardar(d, RCX);
                    }
                }
            }
            // -- Los saltos --------------------------------------------------------------
            Op::Si { c } => {
                e.cargar(RAX, c);
                e.rr(None, false, &[0x85], RAX, RAX);
                sis.push(e.salto_si(CC_E));
            }
            Op::SiNo => {
                let fin = e.salto();
                let si = sis.pop()?;
                e.aqui(si);
                sis.push(fin);
            }
            Op::FinSi => {
                let s = sis.pop()?;
                e.aqui(s);
            }
            Op::Bucle => {
                // A10: con olas en el computo, la VUELTA de cada bucle abierto
                // (como `Pausa::vueltas` del interprete): 0 al entrar, +1 cada
                // vez que vuelve a empezar. Va en los registros de mas del hilo
                // (`vueltas_desde`): `despachar` la lee en cada ola.
                if vueltas {
                    e.mem(None, false, &[0xC7], 0, REGS, 4 * (p.iniciales.len() + bucles.len()) as i32); // mov dword [rbx + vuelta], 0
                    e.b.extend_from_slice(&0u32.to_le_bytes());
                }
                bucles.push((e.b.len(), Vec::new()));
            }
            Op::FinBucle => {
                if vueltas {
                    e.mem(None, false, &[0x83], 0, REGS, 4 * (p.iniciales.len() + bucles.len() - 1) as i32); // add dword [rbx + vuelta], 1
                    e.b.push(1);
                }
                let (principio, romper) = bucles.pop()?;
                e.saltar_a(principio);
                for r in romper {
                    e.aqui(r);
                }
            }
            Op::Romper => {
                let s = e.salto();
                bucles.last_mut()?.1.push(s);
            }
            Op::RomperSi { c, si_cero } => {
                e.cargar(RAX, c);
                e.rr(None, false, &[0x85], RAX, RAX);
                let s = e.salto_si(if si_cero { CC_E } else { CC_NE });
                bucles.last_mut()?.1.push(s);
            }
            Op::Continuar => {
                if vueltas {
                    e.mem(None, false, &[0x83], 0, REGS, 4 * (p.iniciales.len() + bucles.len() - 1) as i32); // add dword [rbx + vuelta], 1
                    e.b.push(1);
                }
                let principio = bucles.last()?.0;
                e.saltar_a(principio);
            }
            Op::Descarta { c } => {
                e.cargar(RAX, c);
                e.rr(None, false, &[0x85], RAX, RAX);
                let sigue = e.salto_si(CC_E);
                e.inmediato(RAX, DESCARTADO);
                al_final.push(e.salto());
                e.aqui(sigue);
            }
            // -- Arrays de registros y cbuffers -------------------------------------------
            Op::LeeIndexado { d, base, n, i } => {
                e.cargar(RAX, i);
                e.rr(None, false, &[0x31], RCX, RCX);
                e.b.push(0x3D); // cmp eax, n
                e.b.extend_from_slice(&(n as u32).to_le_bytes());
                let fuera = e.salto_si(CC_AE);
                e.mem_x4(None, false, &[0x8B], RCX, REGS, RAX, 4 * base as i32); // mov ecx, [rbx + rax*4 + 4base]
                e.aqui(fuera);
                e.guardar(d, RCX);
            }
            Op::EscribeIndexado { base, n, i, s } => {
                e.cargar(RAX, i);
                e.b.push(0x3D);
                e.b.extend_from_slice(&(n as u32).to_le_bytes());
                let fuera = e.salto_si(CC_AE);
                e.cargar(RCX, s);
                e.mem_x4(None, false, &[0x89], RCX, REGS, RAX, 4 * base as i32);
                e.aqui(fuera);
            }
            Op::Constantes { d, fila, .. } => {
                for k in 0..4u16 {
                    e.mem(Some(0xF3), false, &[0x0F, MOVSS], 0, CB, fila as i32 * 16 + 4 * k as i32);
                    e.guardar_x(d + k, 0);
                }
            }
            Op::Entrada { d, elemento, componente } => {
                e.mem(None, true, &[0x8B], RAX, CTX, C_ENTRADAS);
                e.mem(Some(0xF3), false, &[0x0F, MOVSS], 0, RAX, (elemento as i32 * 4 + componente as i32) * 4);
                e.guardar_x(d, 0);
            }
            Op::Salida { s, elemento, componente } => {
                e.mem(None, true, &[0x8B], RAX, CTX, C_SALIDAS);
                e.cargar_x(0, s);
                e.mem(Some(0xF3), false, &[0x0F, MOVSS_A_MEM], 0, RAX, (elemento as i32 * 4 + componente as i32) * 4);
            }
            // -- El computo ------------------------------------------------------------
            Op::IdHilo { d, que, c } => {
                let i = if que < 3 { que as i32 * 3 + c as i32 } else { 9 };
                e.mem(None, false, &[0x8B], RAX, CTX, C_IDS + 4 * i);
                e.guardar(d, RAX);
            }
            Op::Barrera => {
                let n = punto_de[k]?;
                e.mem(None, false, &[0xC7], 0, CTX, C_REANUDAR); // mov dword [r12 + reanudar], n
                e.b.extend_from_slice(&(n as u32).to_le_bytes());
                e.inmediato(RAX, BARRERA);
                al_final.push(e.salto());
                let aqui = e.b.len();
                e.parchear(reanudar[n - 1], aqui);
            }
            Op::LeeCompartida { d, base, n, i } => {
                match constante(i) {
                    Some(k) if k < n => e.mem(None, false, &[0x8B], RCX, COMP, 4 * (base + k) as i32),
                    Some(_) => e.rr(None, false, &[0x31], RCX, RCX),
                    None => {
                        e.cargar(RAX, i);
                        e.rr(None, false, &[0x31], RCX, RCX);
                        e.b.push(0x3D);
                        e.b.extend_from_slice(&n.to_le_bytes());
                        let fuera = e.salto_si(CC_AE);
                        e.mem_x4(None, false, &[0x8B], RCX, COMP, RAX, 4 * base as i32);
                        e.aqui(fuera);
                    }
                }
                e.guardar(d, RCX);
            }
            Op::EscribeCompartida { base, n, i, s } => {
                e.cargar(RCX, s);
                match constante(i) {
                    Some(k) if k < n => e.mem(None, false, &[0x89], RCX, COMP, 4 * (base + k) as i32),
                    Some(_) => {}
                    None => {
                        e.cargar(RAX, i);
                        e.b.push(0x3D);
                        e.b.extend_from_slice(&n.to_le_bytes());
                        let fuera = e.salto_si(CC_AE);
                        e.mem_x4(None, false, &[0x89], RCX, COMP, RAX, 4 * base as i32);
                        e.aqui(fuera);
                    }
                }
            }
            Op::Lee { d, t, como: Lectura::Bufer(modo), c, .. } => {
                if t as usize >= VISTAS {
                    return None;
                }
                // Primero los indices (d puede ser uno de ellos), luego los ceros.
                let fuera = e.ventana(C_SRV + VISTA * t as i32, modo, c[0], c[1])?;
                e.cuatro_palabras(d);
                let listo = e.salto();
                for f in fuera {
                    e.aqui(f);
                }
                e.rr(None, false, &[0x31], RCX, RCX);
                for k in 0..4 {
                    e.guardar(d + k, RCX);
                }
                e.aqui(listo);
            }
            Op::LeeUav { d, u, modo, i, desp, .. } => {
                if u as usize >= VISTAS {
                    return None;
                }
                let fuera = e.ventana(C_UAV + VISTA * u as i32, modo, i, desp)?;
                e.cuatro_palabras(d);
                let listo = e.salto();
                for f in fuera {
                    e.aqui(f);
                }
                e.rr(None, false, &[0x31], RCX, RCX);
                for k in 0..4 {
                    e.guardar(d + k, RCX);
                }
                e.aqui(listo);
            }
            Op::EscribeUav { u, modo, i, desp, v, mascara, .. } => {
                if u as usize >= VISTAS {
                    return None;
                }
                let fuera = e.ventana(C_UAV + VISTA * u as i32, modo, i, desp)?;
                for (k, &r) in v.iter().enumerate() {
                    if mascara & (1 << k) == 0 {
                        continue;
                    }
                    let k = k as i32;
                    e.mem(None, true, &[0x8D], R11, RAX, 4 * k + 4); // lea r11, [rax + 4k + 4]
                    e.rr(None, true, &[0x39], R10, R11); // cmp r11, r10
                    let no = e.salto_si(CC_A);
                    e.cargar(RCX, r);
                    e.mem_x1(false, &[0x89], RCX, RDX, RAX, 4 * k); // mov [rdx + rax + 4k], ecx
                    e.aqui(no);
                }
                for f in fuera {
                    e.aqui(f);
                }
            }
            // E2.4: el contador oculto del UAV (sin el, 0 y nada se mueve).
            Op::Contador { d, u, inc } => {
                if u as usize >= VISTAS {
                    return None;
                }
                e.campo(RCX, true, C_UAV + VISTA * u as i32, V_CONTADOR); // rcx = el contador
                e.rr(None, false, &[0x31], RAX, RAX); // xor eax, eax
                e.rr(None, true, &[0x85], RCX, RCX); // test rcx, rcx
                let sin = e.salto_si(CC_E);
                e.mem(None, false, &[0x8B], RAX, RCX, 0); // mov eax, [rcx]
                e.mem(None, false, &[0x8D], RDX, RAX, inc as i32); // lea edx, [rax + inc]
                e.mem(None, false, &[0x89], RDX, RCX, 0); // mov [rcx], edx
                if inc < 0 {
                    e.rr(None, false, &[0x89], RDX, RAX); // al bajar, el de despues
                }
                e.aqui(sin);
                e.guardar(d, RAX);
            }
            // Lo que no sabe: por el interprete (y, 05-10, los Interlocked;
            // E2.5, las olas: aqui cada hilo corre solo).
            // 18 de la pila A (07-10): el Interlocked de la compartida, tambien.
            Op::Lee { .. } | Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. } | Op::MedidasUav { .. } | Op::Atomico { .. } | Op::AtomicoCompartido { .. } | Op::Ola { .. } => return None,
        }
    }
    if !sis.is_empty() || !bucles.is_empty() {
        return None;
    }
    // El final: acabo (y la proxima vez, desde el principio).
    e.mem(None, false, &[0xC7], 0, CTX, C_REANUDAR);
    e.b.extend_from_slice(&0u32.to_le_bytes());
    e.inmediato(RAX, ACABO);
    let fin = e.b.len();
    for s in al_final {
        e.parchear(s, fin);
    }
    // Epilogo: el MXCSR de quien llamo, y los cinco.
    crate::nativo::mxcsr_al_salir(&mut e.b);
    e.b.extend_from_slice(&[0x48, 0x83, 0xC4, 0x10]); // add rsp, 16
    for r in [R15, R14, R13, R12, RBX] {
        e.pop(r);
    }
    e.b.push(0xC3);
    Some(e.b)
}

// El Dispatch con la funcion traducida (y A10, sus olas) vive en
// `nativo_despacho.rs` (L6a, 06-10); aqui, el mismo nombre.
pub use crate::nativo_despacho::despachar;

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los desplazamientos que lee el codigo son los de la estructura.
    #[test]
    fn el_contexto_tiene_los_desplazamientos_del_codigo() {
        assert_eq!(core::mem::offset_of!(Contexto, ids) as i32, C_IDS);
        assert_eq!(core::mem::offset_of!(Contexto, reanudar) as i32, C_REANUDAR);
        assert_eq!(core::mem::offset_of!(Contexto, compartida) as i32, C_COMPARTIDA);
        assert_eq!(core::mem::offset_of!(Contexto, entradas) as i32, C_ENTRADAS);
        assert_eq!(core::mem::offset_of!(Contexto, salidas) as i32, C_SALIDAS);
        assert_eq!(core::mem::offset_of!(Contexto, llamadas) as i32, C_LLAMADAS);
        assert_eq!(core::mem::offset_of!(Contexto, srv) as i32, C_SRV);
        assert_eq!(core::mem::offset_of!(Contexto, uav) as i32, C_UAV);
        assert_eq!(core::mem::size_of::<Vista>() as i32, VISTA);
        assert_eq!(core::mem::offset_of!(Vista, contador) as i32, V_CONTADOR);
        assert_eq!(core::mem::offset_of!(Vista, datos) as i32, V_DATOS);
        assert_eq!(core::mem::offset_of!(Vista, bytes) as i32, V_BYTES);
        assert_eq!(core::mem::offset_of!(Vista, paso) as i32, V_PASO);
        assert_eq!(core::mem::offset_of!(Vista, elementos) as i32, V_ELEMENTOS);
    }

    /// Lo que no sabe, lo dice (`None`), y lo que sabe sale entero.
    #[test]
    fn traduce_el_computo_y_no_lo_que_no_sabe() {
        let cs = crate::dxil::computo::preparar(include_bytes!("../prueba/computo.dxil")).unwrap();
        assert!(compilar(&cs.programa).is_some());
        let mut p = cs.programa.clone();
        // X2 (05-10): la matematica ya la llama; y (06-10) las texturas de
        // un CS tambien, por la llamada de su Dispatch.
        p.ops.push(Op::Mate { d: 0, a: 0, f: crate::mates::Mate::Exp2 });
        assert!(compilar(&p).is_some(), "la matematica, por `Mate::aplicar`");
        p.ops.push(Op::Muestra { d: 0, t: 0, s: 0, u: 0, v: 0, g: None });
        assert!(compilar(&p).is_some(), "las texturas de un CS, por la llamada de la casa");
        assert!(compilar_dibujo(&p).is_some(), "las de un dibujo, tambien");
        // 06-10: un atomico o una textura en un UAV: su ranura, ENTERA por la
        // llamada (`uavs_llamados`), y lo demas sigue en el codigo.
        let mut q = cs.programa.clone();
        q.ops.push(Op::Atomico { d: 0, u: 1, modo: crate::bufer::Modo::Crudo, i: 0, desp: 0, z: 0, como: crate::bufer::Atomo::Suma, v: 0, igual: 0 });
        assert!(compilar(&q).is_some(), "un Interlocked, por la llamada");
        assert_eq!(uavs_llamados(&q)[..2], [false, true], "solo su ranura");
        // A10 (06-10): una ola, ya si (se para y la resuelve `despachar`).
        p.ops.push(Op::Ola { d: 0, a: 0, b: 0, que: crate::dxil::olas::Ola::Indice });
        assert!(compilar(&p).is_some(), "una ola fuera de bucles, traducida");
        let mut b = cs.programa.clone();
        b.ops.extend([Op::Bucle, Op::Ola { d: 0, a: 0, b: 0, que: crate::dxil::olas::Ola::Indice }, Op::Romper, Op::FinBucle]);
        assert!(compilar(&b).is_some(), "y dentro de un bucle (con sus vueltas)");
    }
}
