//! **Un sombreador, traducido UNA vez a x86-64 con SSE** (P3b3b, 27-09).
//!
//! El Rust de Ring 3 (`x86_64-unknown-none`) no puede usar SSE: el target es
//! soft-float, y encenderlo lo retira rustc (issue #162235) y tumba a LLVM
//! (SIGILL, comprobado el 27-09). Asi que el interprete de `programa.rs` hace
//! cada `fmul` como una llamada a una rutina de software. INTI si emite SSE en
//! Ring 3 -- el kernel guarda x87+SSE por tarea (`xcr0 0x003`) --, y con sus
//! mismas reglas (`GRAMATICA.md`, 26-09: `acumula` son DOS redondeos, no FMA;
//! el orden del juez) esto traduce un [`Programa`] a bytes de maquina:
//!
//! ```text
//!    la llamada   rdi = los registros (f32), rsi = las entradas ([f32; 4]),
//!                 rdx = el cbuffer, rcx = las salidas ([f32; 4])
//!                 -- solo PUNTEROS: el ABI entero, el que Rust soft-float sabe
//!    el cuerpo    una instruccion ESCALAR de SSE por operacion, sobre
//!                 [rdi + 4 * registro]; solo xmm0..xmm2 (volatiles)
//!    el MXCSR     el suyo se guarda, se pone 0x1F80 (al mas cercano, sin
//!                 FTZ ni DAZ: IEEE-754 como el interprete) y se devuelve;
//!                 si su control ya es ese, ni se pone ni se quita (ver
//!                 [`mxcsr_al_entrar`]: sus banderas de excepcion no cuentan)
//!    devuelve     en eax: [`QUEDA`] (0), o [`DESCARTADO`] si el pixel se tiro
//! ```
//!
//! **Los que SALTAN** (la VELOCIDAD, 05-10): un VS o un PS con `si`, bucles,
//! enteros, comparaciones, conversiones, `discard` o arrays de registros no
//! cabe en lo de arriba (una fila de SSE sin saltos). Para esos, el cuerpo es
//! el del COMPUTO (`nativo_computo`: saltos de verdad, los enteros modulo
//! 2^32, las conversiones de Rust, su juez bit a bit) y delante va una
//! entrada con la MISMA llamada de arriba que le pone un `Contexto` a medias
//! en la pila (solo entradas, salidas, compartida nula y `reanudar`): asi la
//! casa los llama igual, y lo que el computo lee de mas (ids, vistas,
//! barreras) no esta en un dibujo -- se mira antes y, si esta, `None`.
//!
//! **Por que da los MISMOS bits que el interprete:** mulss, addss, subss,
//! divss y sqrtss son IEEE-754 con redondeo al mas cercano, igual que la
//! aritmetica de software; FMad son dos (`mulss` y `addss`), como el
//! interprete; el Dot suma de izquierda a derecha; Saturate, FMin y FMax dan
//! el otro operando si uno es NaN, como pide D3D. El interprete es el JUEZ de
//! esto: el banco corre los dos y compara bit a bit. (Un NaN puede salir con
//! otra carga util que el de software; como valor, es NaN igual.)
//!
//! Los bytes son puros: sellarlos (`MEM_OP_SELLAR`) y llamarlos es de la casa.

use alloc::vec::Vec;

use crate::dxil::programa::{Op, Programa, Reg};

/// El MXCSR de D3D: todas las excepciones tapadas, al mas cercano.
pub const MXCSR_D3D: u32 = 0x1F80;

/// Los bits de CONTROL del MXCSR (6..15: DAZ, las mascaras, el redondeo,
/// FTZ); los de abajo (0..5) son las banderas de excepcion, pegajosas.
const MXCSR_CONTROL: u32 = 0xFFC0;

/// **El MXCSR al entrar** (la VELOCIDAD, 05-10), con `[rsp]` y `[rsp+4]`
/// libres: el de quien llama a `[rsp]`, y el de D3D puesto SOLO si su
/// control es otro. Por que: un `ldmxcsr` que CAMBIA el MXCSR cuesta (en el
/// Xeon del banco, unos 50 ns por llamada: el VSId de instancias, de 33 a 87
/// ns), y en BMO-X quien llama es Rust soft-float, que no lo toca: su
/// control es siempre 0x1F80. Las banderas que deje el sombreador (inexacto,
/// NaN) se quedan: son de estado; el juego (codigo de Windows, en el mismo
/// hilo) podria leerlas con `_statusfp`, pero sus propias cuentas ya las
/// ponen a cada rato. Pisa eax.
pub(crate) fn mxcsr_al_entrar(b: &mut Vec<u8>) {
    b.extend_from_slice(&[0x0F, 0xAE, 0x1C, 0x24]); // stmxcsr [rsp]
    b.extend_from_slice(&[0x8B, 0x04, 0x24]); // mov eax, [rsp]
    b.push(0x25); // and eax, MXCSR_CONTROL
    b.extend_from_slice(&MXCSR_CONTROL.to_le_bytes());
    b.push(0x3D); // cmp eax, MXCSR_D3D
    b.extend_from_slice(&MXCSR_D3D.to_le_bytes());
    b.extend_from_slice(&[0x74, 13]); // je (detras de los dos de abajo: 8 + 5)
    b.extend_from_slice(&[0xC7, 0x44, 0x24, 0x04]); // mov dword [rsp+4], MXCSR_D3D
    b.extend_from_slice(&MXCSR_D3D.to_le_bytes());
    b.extend_from_slice(&[0x0F, 0xAE, 0x54, 0x24, 0x04]); // ldmxcsr [rsp+4]
}

/// **El MXCSR al salir**: el de quien llamo, de vuelta tal cual, si su
/// control era otro; si era el de D3D, nada (ver [`mxcsr_al_entrar`]). Pisa
/// ecx (no eax: lo que devuelve).
pub(crate) fn mxcsr_al_salir(b: &mut Vec<u8>) {
    b.extend_from_slice(&[0x0F, 0xAE, 0x5C, 0x24, 0x04]); // stmxcsr [rsp+4]
    b.extend_from_slice(&[0x8B, 0x0C, 0x24]); // mov ecx, [rsp]
    b.extend_from_slice(&[0x33, 0x4C, 0x24, 0x04]); // xor ecx, [rsp+4]
    b.extend_from_slice(&[0xF7, 0xC1]); // test ecx, MXCSR_CONTROL
    b.extend_from_slice(&MXCSR_CONTROL.to_le_bytes());
    b.extend_from_slice(&[0x74, 4]); // je (detras del ldmxcsr)
    b.extend_from_slice(&[0x0F, 0xAE, 0x14, 0x24]); // ldmxcsr [rsp]
}

// Los registros base de la llamada (numero de registro en ModRM).
const RDI: u8 = 7;
const RSI: u8 = 6;
const RDX: u8 = 2;
const RCX: u8 = 1;

struct Emisor {
    b: Vec<u8>,
}

impl Emisor {
    /// `F3 0F op /r` con memoria `[base + disp32]` (xmm0..7, base sin SIB).
    fn escalar_mem(&mut self, op: u8, xmm: u8, base: u8, disp: u32) {
        self.b.extend_from_slice(&[0xF3, 0x0F, op, 0x80 | (xmm << 3) | base]);
        self.b.extend_from_slice(&disp.to_le_bytes());
    }

    /// `F3 0F op /r` entre dos xmm.
    fn escalar_reg(&mut self, op: u8, dst: u8, src: u8) {
        self.b.extend_from_slice(&[0xF3, 0x0F, op, 0xC0 | (dst << 3) | src]);
    }

    /// `0F op /r` empaquetada entre dos xmm (and/andn/or).
    fn empaquetada(&mut self, op: u8, dst: u8, src: u8) {
        self.b.extend_from_slice(&[0x0F, op, 0xC0 | (dst << 3) | src]);
    }

    fn cargar(&mut self, xmm: u8, r: Reg) {
        self.escalar_mem(0x10, xmm, RDI, 4 * r as u32); // movss xmm, [rdi+4r]
    }

    fn guardar(&mut self, r: Reg, xmm: u8) {
        self.escalar_mem(0x11, xmm, RDI, 4 * r as u32); // movss [rdi+4r], xmm
    }

    fn op_mem(&mut self, op: u8, xmm: u8, r: Reg) {
        self.escalar_mem(op, xmm, RDI, 4 * r as u32);
    }

    /// Un `u32` en un xmm: `mov eax, imm32` y `movd xmm, eax`.
    fn constante(&mut self, xmm: u8, bits: u32) {
        self.b.push(0xB8);
        self.b.extend_from_slice(&bits.to_le_bytes());
        self.b.extend_from_slice(&[0x66, 0x0F, 0x6E, 0xC0 | (xmm << 3)]);
    }

    /// FMin/FMax de D3D: `minss/maxss` con el orden (b, a), y si `a` es NaN,
    /// `b`. xmm0 = b op a; xmm1 = mascara de "a es NaN"; mezcla.
    fn min_max(&mut self, op: u8, d: Reg, a: Reg, b: Reg) {
        self.cargar(0, b);
        self.op_mem(op, 0, a); // xmm0 = (b < a) ? b : a   (o >)
        self.cargar(1, a);
        self.b.extend_from_slice(&[0xF3, 0x0F, 0xC2, 0xC9, 0x03]); // cmpunordss xmm1, xmm1
        self.cargar(2, b);
        self.empaquetada(0x54, 2, 1); // andps  xmm2, xmm1   (b si a es NaN)
        self.empaquetada(0x55, 1, 0); // andnps xmm1, xmm0   (lo otro si no)
        self.empaquetada(0x56, 1, 2); // orps   xmm1, xmm2
        self.guardar(d, 1);
    }
}

pub const MOVSS: u8 = 0x10;
pub const MOVSS_A_MEM: u8 = 0x11;
pub const SQRTSS: u8 = 0x51;
pub const ADDSS: u8 = 0x58;
pub const MULSS: u8 = 0x59;
pub const SUBSS: u8 = 0x5C;
pub const MINSS: u8 = 0x5D;
pub const DIVSS: u8 = 0x5E;
pub const MAXSS: u8 = 0x5F;

/// Lo que devuelve un sombreador traducido: el pixel queda (o el vertice).
pub const QUEDA: u32 = 0;
/// O se tiro (`discard`): el del computo, que es quien lo emite.
pub const DESCARTADO: u32 = crate::nativo_computo::DESCARTADO;

/// **Por que [`compilar`] no traduce `p`**, dicho para quien lo lea (el
/// aviso de la casa), o `None` si lo traduce. La lista es la de `compilar`.
pub fn por_que_no(p: &Programa) -> Option<&'static str> {
    if p.muestrea() {
        return Some("muestrea una textura");
    }
    // Los saltos, los enteros, `discard` y los arrays de registros ya los
    // sabe (por el cuerpo del computo): no son motivo.
    p.ops.iter().find_map(|o| match o {
        Op::Mate { .. } => Some("usa la matematica (exp, log, sin...)"),
        Op::ConstantesEn { .. } => Some("lee un cbuffer con fila calculada"),
        // 05-10: un UAV en un dibujo: interpretado, nunca perdido.
        Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } | Op::Contador { .. } | Op::Atomico { .. } => Some("lee o escribe un UAV"),
        Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } => Some("es de computo"),
        Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. } => Some("es de geometria"),
        _ => None,
    })
}

/// **Traducir un programa a x86-64.** Una funcion entera, independiente de
/// donde caiga (solo usa sus cuatro punteros): se puede copiar a otro bloque.
pub fn compilar(p: &Programa) -> Option<Vec<u8>> {
    // Un programa que MUESTREA una textura no se traduce todavia: el
    // muestreo (filtros, direcciones) va por el interprete (`textura`).
    if p.muestrea() {
        return None;
    }
    // Lo sin saltos, por la fila de SSE; lo demas, por el cuerpo del computo.
    directo(p).or_else(|| con_saltos(p))
}

/// **Con saltos** (la VELOCIDAD, 05-10): el cuerpo de `nativo_computo` con
/// una entrada que habla la llamada de los dibujos:
///
/// ```text
///    sub  rsp, 88             el Contexto a medias (72 bytes), a 16
///    mov  [rsp+56], rsi       sus entradas
///    mov  [rsp+64], rcx       sus salidas
///    mov  qword [rsp+48], 0   sin memoria compartida
///    mov  dword [rsp+40], 0   desde el principio
///    mov  rsi, rsp            rdi (registros) y rdx (cbuffer), tal cual
///    call cuerpo              eax: 0 acabo, 2 descartado
///    add  rsp, 88
///    ret
/// ```
fn con_saltos(p: &Programa) -> Option<Vec<u8>> {
    use crate::nativo_computo::{C_COMPARTIDA, C_ENTRADAS, C_REANUDAR, C_SALIDAS};
    // Lo que lee del Contexto que aqui no se pone (ids, vistas, barreras,
    // compartida) o lo de la geometria: eso no es de un vertice ni un pixel.
    let de_fuera = |o: &Op| matches!(o, Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } | Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } | Op::Contador { .. } | Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. });
    if p.ops.iter().any(de_fuera) {
        return None;
    }
    let cuerpo = crate::nativo_computo::compilar(p)?;
    let disp8 = |c: i32| c as u8;
    let mut b = Vec::with_capacity(cuerpo.len() + 48);
    b.extend_from_slice(&[0x48, 0x83, 0xEC, 0x58]); // sub rsp, 88
    b.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, disp8(C_ENTRADAS)]); // mov [rsp+56], rsi
    b.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, disp8(C_SALIDAS)]); // mov [rsp+64], rcx
    b.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, disp8(C_COMPARTIDA), 0, 0, 0, 0]); // mov qword [rsp+48], 0
    b.extend_from_slice(&[0xC7, 0x44, 0x24, disp8(C_REANUDAR), 0, 0, 0, 0]); // mov dword [rsp+40], 0
    b.extend_from_slice(&[0x48, 0x89, 0xE6]); // mov rsi, rsp
    b.extend_from_slice(&[0xE8, 5, 0, 0, 0]); // call cuerpo (detras de add y ret)
    b.extend_from_slice(&[0x48, 0x83, 0xC4, 0x58]); // add rsp, 88
    b.push(0xC3); // ret
    b.extend_from_slice(&cuerpo);
    Some(b)
}

// Los cuatro campos que pone la entrada de [`con_saltos`] caben en sus 88
// bytes de pila y su desplazamiento, en un byte (`disp8`).
const _: () = {
    use crate::nativo_computo::{C_COMPARTIDA, C_ENTRADAS, C_REANUDAR, C_SALIDAS};
    assert!(C_REANUDAR + 4 <= 88 && C_COMPARTIDA + 8 <= 88 && C_ENTRADAS + 8 <= 88 && C_SALIDAS + 8 <= 88 && C_SALIDAS < 128);
};

/// **Sin saltos**: una fila de SSE, sobre los cuatro punteros. `None` si
/// tiene algo que esto no sabe (lo de [`con_saltos`] o lo que nadie sabe).
fn directo(p: &Programa) -> Option<Vec<u8>> {
    if p.salta() {
        return None;
    }
    let mut e = Emisor { b: Vec::with_capacity(16 * p.ops.len() + 64) };
    // Prologo: el MXCSR de quien llama, a la pila; el de D3D, puesto.
    e.b.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]); // sub rsp, 8
    mxcsr_al_entrar(&mut e.b);
    for op in &p.ops {
        match *op {
            Op::Entrada { d, elemento, componente } => {
                e.escalar_mem(MOVSS, 0, RSI, (elemento as u32 * 4 + componente as u32) * 4);
                e.guardar(d, 0);
            }
            Op::Salida { s, elemento, componente } => {
                e.cargar(0, s);
                e.escalar_mem(MOVSS_A_MEM, 0, RCX, (elemento as u32 * 4 + componente as u32) * 4);
            }
            Op::Constantes { d, fila, .. } => {
                for k in 0..4u16 {
                    e.escalar_mem(MOVSS, 0, RDX, fila as u32 * 16 + 4 * k as u32);
                    e.guardar(d + k, 0);
                }
            }
            Op::Mul { d, a, b } | Op::Add { d, a, b } | Op::Sub { d, a, b } | Op::Div { d, a, b } => {
                let x = match op {
                    Op::Mul { .. } => MULSS,
                    Op::Add { .. } => ADDSS,
                    Op::Sub { .. } => SUBSS,
                    _ => DIVSS,
                };
                e.cargar(0, a);
                e.op_mem(x, 0, b);
                e.guardar(d, 0);
            }
            Op::Mad { d, a, b, c } => {
                // Dos redondeos: el `acumula` de INTI, no su `funde`.
                e.cargar(0, a);
                e.op_mem(MULSS, 0, b);
                e.op_mem(ADDSS, 0, c);
                e.guardar(d, 0);
            }
            Op::Dot { d, n, a, b } => {
                e.cargar(0, a[0]);
                e.op_mem(MULSS, 0, b[0]);
                for k in 1..n as usize {
                    e.cargar(1, a[k]);
                    e.op_mem(MULSS, 1, b[k]);
                    e.escalar_reg(ADDSS, 0, 1);
                }
                e.guardar(d, 0);
            }
            Op::Sqrt { d, a } => {
                e.op_mem(SQRTSS, 0, a);
                e.guardar(d, 0);
            }
            Op::Rsqrt { d, a } => {
                e.op_mem(SQRTSS, 1, a);
                e.constante(0, 1.0f32.to_bits());
                e.escalar_reg(DIVSS, 0, 1);
                e.guardar(d, 0);
            }
            Op::Saturate { d, a } => {
                // max(x, 0): un NaN o un cero da el segundo, +0; y min(., 1).
                e.cargar(0, a);
                e.constante(1, 0);
                e.escalar_reg(MAXSS, 0, 1);
                e.constante(1, 1.0f32.to_bits());
                e.escalar_reg(MINSS, 0, 1);
                e.guardar(d, 0);
            }
            Op::Abs { d, a } => {
                e.cargar(0, a);
                e.constante(1, 0x7FFF_FFFF);
                e.empaquetada(0x54, 0, 1); // andps
                e.guardar(d, 0);
            }
            Op::Min { d, a, b } => e.min_max(MINSS, d, a, b),
            Op::Max { d, a, b } => e.min_max(MAXSS, d, a, b),
            // N5.6: la matematica va por el interprete hasta que esto la sepa.
            Op::Mate { .. } => return None,
            // N5.7 y N5.10: el que tira pixeles y los arrays, por `con_saltos`
            // (los cbuffers con fila calculada, por el interprete).
            Op::Descarta { .. } => return None,
            Op::LeeIndexado { .. } | Op::EscribeIndexado { .. } | Op::ConstantesEn { .. } => return None,
            // N5.5: el computo, por el interprete.
            Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } | Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } => return None,
            // E2.3b: el sombreador de geometria, por el interprete.
            Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. } => return None,
            // E2.4: el contador de un UAV, por el interprete; y (05-10) sus
            // Interlocked.
            Op::Contador { .. } | Op::Atomico { .. } => return None,
            Op::Muestra { .. } | Op::Lee { .. } | Op::EligeTextura { .. } => unreachable!("mirado en `compilar`: `muestrea`"),
            Op::Compara { .. } | Op::Elige { .. } | Op::Copia { .. } | Op::SumaEntera { .. } | Op::Entera { .. } | Op::Convierte { .. } | Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle => unreachable!("mirado arriba: `salta`"),
        }
    }
    // Epilogo: el MXCSR de quien llamo, de vuelta, y el pixel queda.
    e.b.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax (QUEDA)
    mxcsr_al_salir(&mut e.b);
    e.b.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
    e.b.push(0xC3); // ret
    Some(e.b)
}
