//! **Programas de muestra de E6** (02-10): un `si`, un bucle que sale por su
//! `RomperSi`, un contador entero con `Elige`, y bucles anidados con un
//! `Romper` dentro de un `si`. Los corre el interprete en sus pruebas y son
//! el banco del emisor de la 3060 (`bmo-proton-x-sm86`): el mismo programa,
//! los mismos bits.

use alloc::vec;
use alloc::vec::Vec;

use super::programa::{Comparacion, Op, Programa, Reg};

/// Un programa con `n` registros, `iniciales` en los primeros, una entrada
/// de dos componentes (x, y) y una salida.
pub fn programa(ops: Vec<Op>, n: usize, iniciales: &[(Reg, f32)]) -> Programa {
    let mut ini = vec![0.0f32; n];
    for &(r, v) in iniciales {
        ini[r as usize] = v;
    }
    Programa { ops, iniciales: ini, entradas: 1, salidas: 1, lee: 1, filas_cb: 0, ranuras: Default::default() }
}

/// `z = x < y ? x * y : x + y`, con un `si` de verdad (no un `Elige`).
pub fn si_sino() -> Programa {
    use Op::*;
    programa(
        vec![
            Entrada { d: 0, elemento: 0, componente: 0 },
            Entrada { d: 1, elemento: 0, componente: 1 },
            Compara { d: 2, a: 0, b: 1, como: Comparacion::Menor, entero: false },
            Si { c: 2 },
            Mul { d: 3, a: 0, b: 1 },
            Copia { d: 5, a: 3 },
            SiNo,
            Add { d: 4, a: 0, b: 1 },
            Copia { d: 5, a: 4 },
            FinSi,
            Salida { s: 5, elemento: 0, componente: 0 },
        ],
        6,
        &[],
    )
}

/// `acc = 0; v = y; repetir { acc += v; v *= 0.5; si acc >= x, salir }`.
pub fn bucle_geometrico() -> Programa {
    use Op::*;
    programa(
        vec![
            Entrada { d: 0, elemento: 0, componente: 0 },
            Entrada { d: 1, elemento: 0, componente: 1 },
            Copia { d: 2, a: 7 },
            Copia { d: 3, a: 1 },
            Bucle,
            Add { d: 4, a: 2, b: 3 },
            Copia { d: 2, a: 4 },
            Mul { d: 5, a: 3, b: 8 },
            Copia { d: 3, a: 5 },
            Compara { d: 6, a: 2, b: 0, como: Comparacion::MayorIgual, entero: false },
            RomperSi { c: 6, si_cero: false },
            FinBucle,
            Salida { s: 2, elemento: 0, componente: 0 },
            Salida { s: 3, elemento: 0, componente: 1 },
        ],
        9,
        &[(7, 0.0), (8, 0.5)],
    )
}

/// `acc = 0; para i = 0 mientras i < 5: acc += x` -- un contador ENTERO
/// (sus bits, no un float), y dentro un `si` que se salta las vueltas
/// pares con `Elige`.
pub fn bucle_entero() -> Programa {
    use Op::*;
    let bits = |b: u32| f32::from_bits(b);
    programa(
        vec![
            Entrada { d: 0, elemento: 0, componente: 0 },
            Copia { d: 1, a: 10 },
            Copia { d: 2, a: 10 },
            Bucle,
            Compara { d: 3, a: 2, b: 11, como: Comparacion::MayorIgual, entero: true },
            RomperSi { c: 3, si_cero: false },
            Add { d: 4, a: 1, b: 0 },
            Copia { d: 1, a: 4 },
            SumaEntera { d: 5, a: 2, b: 12 },
            Copia { d: 2, a: 5 },
            FinBucle,
            // y = acc > 4x ? acc : -1 (con Elige).
            Mul { d: 6, a: 0, b: 13 },
            Compara { d: 7, a: 1, b: 6, como: Comparacion::Mayor, entero: false },
            Elige { d: 8, c: 7, a: 1, b: 14 },
            Salida { s: 1, elemento: 0, componente: 0 },
            Salida { s: 8, elemento: 0, componente: 1 },
            Salida { s: 2, elemento: 0, componente: 2 },
        ],
        15,
        &[(10, bits(0)), (11, bits(5)), (12, bits(1)), (13, 4.0), (14, -1.0)],
    )
}

/// Un bucle dentro de otro, un `Romper` dentro de un `si`, y un `si` sin
/// `SiNo`: `para i < 3 { para j { si j*y > x: romper; c += 1 } }`.
pub fn anidado() -> Programa {
    use Op::*;
    programa(
        vec![
            Entrada { d: 0, elemento: 0, componente: 0 },
            Entrada { d: 1, elemento: 0, componente: 1 },
            Copia { d: 2, a: 20 }, // i = 0.0
            Copia { d: 3, a: 20 }, // c = 0.0
            Bucle,
            Compara { d: 4, a: 2, b: 22, como: Comparacion::MayorIgual, entero: false },
            RomperSi { c: 4, si_cero: false },
            Copia { d: 5, a: 20 }, // j = 0.0
            Bucle,
            Mul { d: 6, a: 5, b: 1 },
            Compara { d: 7, a: 6, b: 0, como: Comparacion::Mayor, entero: false },
            Si { c: 7 },
            Romper,
            FinSi,
            Add { d: 8, a: 3, b: 21 },
            Copia { d: 3, a: 8 },
            Add { d: 9, a: 5, b: 21 },
            Copia { d: 5, a: 9 },
            FinBucle,
            Add { d: 10, a: 2, b: 21 },
            Copia { d: 2, a: 10 },
            FinBucle,
            Salida { s: 3, elemento: 0, componente: 0 },
        ],
        23,
        &[(20, 0.0), (21, 1.0), (22, 3.0)],
    )
}

// -- Sombreadores SM5 con saltos, armados palabra a palabra (FXC no esta) ---

use super::Elemento;
use alloc::string::String;

/// Una instruccion SM5: su codigo, banderas (`_sat`, `_nz`) y operandos.
fn ins(codigo: u32, banderas: u32, operandos: &[u32]) -> Vec<u32> {
    let mut v = vec![codigo | banderas | ((operandos.len() as u32 + 1) << 24)];
    v.extend_from_slice(operandos);
    v
}

/// Un destino `tipo#idx.mascara` (0 r#, 2 o#).
fn dst(tipo: u32, idx: u32, mascara: u32) -> [u32; 2] {
    [2 | mascara << 4 | tipo << 12 | 1 << 20, idx]
}

/// Una fuente de un componente `tipo#idx.c` (select1; 0 r#, 1 v#).
fn src(tipo: u32, idx: u32, c: u32) -> [u32; 2] {
    [2 | 2 << 2 | c << 4 | tipo << 12 | 1 << 20, idx]
}

/// `l(bits)`, un inmediato de un componente.
fn imm(bits: u32) -> [u32; 2] {
    [1 | 4 << 12, bits]
}

fn programa_sm5(cuerpo: &[Vec<u32>]) -> Vec<u32> {
    // ps_5_0, la medida, y `dcl_temps 1` (las declaraciones se saltan).
    let mut t = vec![0x50, 0, 104 | 2 << 24, 4];
    for i in cuerpo {
        t.extend_from_slice(i);
    }
    t.extend(ins(62, 0, &[])); // ret
    t[1] = t.len() as u32;
    t
}

fn firmas() -> (Vec<Elemento>, Vec<Elemento>) {
    let e = |semantica: &str, sistema| Elemento { semantica: String::from(semantica), indice: 0, sistema, tipo: 3, registro: 0, mascara: 0b0011 };
    (vec![e("TEXCOORD", 0)], vec![e("SV_Target", 64)])
}

fn cat(partes: &[&[u32]]) -> Vec<u32> {
    partes.concat()
}

/// ```text
///    mov r0.x, l(0.0) ; mov r0.y, l(0)
///    loop
///      ige r0.z, r0.y, l(4) ; breakc_nz r0.z
///      lt r0.w, v0.x, v0.y
///      if_nz r0.w ; add r0.x, r0.x, v0.x ; else ; add r0.x, r0.x, v0.y ; endif
///      iadd r0.y, r0.y, l(1)
///    endloop
///    mov o0.x, r0.x ; movc o0.y, r0.z, l(1.0), l(2.0) ; ret
/// ```
/// o0.x = 4 * (x < y ? x : y); o0.y = 1 (r0.z sale cierto del bucle).
pub fn sm5_bucle() -> (Vec<u32>, Vec<Elemento>, Vec<Elemento>) {
    let uno = 1.0f32.to_bits();
    let t = programa_sm5(&[
        ins(54, 0, &cat(&[&dst(0, 0, 1), &imm(0)])),
        ins(54, 0, &cat(&[&dst(0, 0, 2), &imm(0)])),
        ins(48, 0, &[]),
        ins(33, 0, &cat(&[&dst(0, 0, 4), &src(0, 0, 1), &imm(4)])),
        ins(3, 1 << 18, &src(0, 0, 2)),
        ins(49, 0, &cat(&[&dst(0, 0, 8), &src(1, 0, 0), &src(1, 0, 1)])),
        ins(31, 1 << 18, &src(0, 0, 3)),
        ins(0, 0, &cat(&[&dst(0, 0, 1), &src(0, 0, 0), &src(1, 0, 0)])),
        ins(18, 0, &[]),
        ins(0, 0, &cat(&[&dst(0, 0, 1), &src(0, 0, 0), &src(1, 0, 1)])),
        ins(21, 0, &[]),
        ins(30, 0, &cat(&[&dst(0, 0, 2), &src(0, 0, 1), &imm(1)])),
        ins(22, 0, &[]),
        ins(54, 0, &cat(&[&dst(2, 0, 1), &src(0, 0, 0)])),
        ins(55, 0, &cat(&[&dst(2, 0, 2), &src(0, 0, 2), &imm(uno), &imm(2.0f32.to_bits())])),
    ]);
    let (e, s) = firmas();
    (t, e, s)
}

/// ```text
///    eq r0.x, v0.x, v0.y
///    if_z r0.x ; mov o0.x, l(5.0) ; else ; mov o0.x, l(7.0) ; endif
///    loop ; break ; endloop
///    mov o0.y, l(3.0) ; ret
/// ```
/// o0.x = x == y ? 7 : 5; o0.y = 3.
pub fn sm5_si_cero() -> (Vec<u32>, Vec<Elemento>, Vec<Elemento>) {
    let t = programa_sm5(&[
        ins(24, 0, &cat(&[&dst(0, 0, 1), &src(1, 0, 0), &src(1, 0, 1)])),
        ins(31, 0, &src(0, 0, 0)),
        ins(54, 0, &cat(&[&dst(2, 0, 1), &imm(5.0f32.to_bits())])),
        ins(18, 0, &[]),
        ins(54, 0, &cat(&[&dst(2, 0, 1), &imm(7.0f32.to_bits())])),
        ins(21, 0, &[]),
        ins(48, 0, &[]),
        ins(2, 0, &[]),
        ins(22, 0, &[]),
        ins(54, 0, &cat(&[&dst(2, 0, 2), &imm(3.0f32.to_bits())])),
    ]);
    let (e, s) = firmas();
    (t, e, s)
}

/// Un operando NULL (el destino alto de `imul`).
fn nulo() -> [u32; 1] {
    [13 << 12]
}

/// E6c: un `switch` con un caso compartido, uno que CAE al siguiente y su
/// `default`; y `ftoi`, `ftou`, `imul`, `ushr`, `itof`, `utof`, `ineg`:
///
/// ```text
///    ftoi r0.x, v0.x ; ftou r0.y, v0.y ; mov o0.y, l(0.0)
///    switch r0.x
///      case 0: mov o0.x, l(10.0) ; break
///      case 1: case 2: imul null, r0.z, r0.x, l(3) ; itof o0.x, r0.z
///      case 3: ushr r0.w, r0.y, l(1) ; utof o0.y, r0.w ; break
///      default: ineg r0.z, r0.x ; itof o0.x, r0.z ; break
///    endswitch
/// ```
pub fn sm5_switch() -> (Vec<u32>, Vec<Elemento>, Vec<Elemento>) {
    let t = programa_sm5(&[
        ins(27, 0, &cat(&[&dst(0, 0, 1), &src(1, 0, 0)])),
        ins(28, 0, &cat(&[&dst(0, 0, 2), &src(1, 0, 1)])),
        ins(54, 0, &cat(&[&dst(2, 0, 2), &imm(0)])),
        ins(76, 0, &src(0, 0, 0)),
        ins(6, 0, &imm(0)),
        ins(54, 0, &cat(&[&dst(2, 0, 1), &imm(10.0f32.to_bits())])),
        ins(2, 0, &[]),
        ins(6, 0, &imm(1)),
        ins(6, 0, &imm(2)),
        ins(38, 0, &cat(&[&nulo(), &dst(0, 0, 4), &src(0, 0, 0), &imm(3)])),
        ins(43, 0, &cat(&[&dst(2, 0, 1), &src(0, 0, 2)])),
        ins(6, 0, &imm(3)),
        ins(85, 0, &cat(&[&dst(0, 0, 8), &src(0, 0, 1), &imm(1)])),
        ins(86, 0, &cat(&[&dst(2, 0, 2), &src(0, 0, 3)])),
        ins(2, 0, &[]),
        ins(10, 0, &[]),
        ins(40, 0, &cat(&[&dst(0, 0, 4), &src(0, 0, 0)])),
        ins(43, 0, &cat(&[&dst(2, 0, 1), &src(0, 0, 2)])),
        ins(2, 0, &[]),
        ins(23, 0, &[]),
    ]);
    let (e, s) = firmas();
    (t, e, s)
}

/// E6c: los de bits y los de comparar sin signo, a cuatro salidas:
///
/// ```text
///    ftoi r0.x, v0.x ; ftoi r0.y, v0.y
///    and r1.x, r0.x, l(6) ; or r1.y, r0.x, r0.y ; xor r1.z, r1.x, r1.y
///    not r1.w, r1.z ; itof o0.x, r1.w
///    imin r2.x, r0.x, r0.y ; imax r2.y, r0.x, r0.y ; umin r2.z, r0.x, r0.y
///    umax r2.w, r0.x, r0.y ; iadd r2.x, r2.x, r2.y ; iadd r2.x, r2.x, r2.z
///    iadd r2.x, r2.x, r2.w ; itof o0.y, r2.x
///    ishr r3.x, r0.x, l(1) ; imad r3.y, r0.x, r0.y, l(7) ; iadd r3.y, r3.y, r3.x
///    itof o0.z, r3.y
///    ult r3.z, r0.x, r0.y ; uge r3.w, r0.x, r0.y ; and r3.z, r3.z, l(1)
///    and r3.w, r3.w, l(2) ; or r3.z, r3.z, r3.w ; itof o0.w, r3.z
/// ```
pub fn sm5_enteros() -> (Vec<u32>, Vec<Elemento>, Vec<Elemento>) {
    let t = programa_sm5(&[
        ins(27, 0, &cat(&[&dst(0, 0, 1), &src(1, 0, 0)])),
        ins(27, 0, &cat(&[&dst(0, 0, 2), &src(1, 0, 1)])),
        ins(1, 0, &cat(&[&dst(0, 1, 1), &src(0, 0, 0), &imm(6)])),
        ins(60, 0, &cat(&[&dst(0, 1, 2), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(87, 0, &cat(&[&dst(0, 1, 4), &src(0, 1, 0), &src(0, 1, 1)])),
        ins(59, 0, &cat(&[&dst(0, 1, 8), &src(0, 1, 2)])),
        ins(43, 0, &cat(&[&dst(2, 0, 1), &src(0, 1, 3)])),
        ins(37, 0, &cat(&[&dst(0, 2, 1), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(36, 0, &cat(&[&dst(0, 2, 2), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(84, 0, &cat(&[&dst(0, 2, 4), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(83, 0, &cat(&[&dst(0, 2, 8), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(30, 0, &cat(&[&dst(0, 2, 1), &src(0, 2, 0), &src(0, 2, 1)])),
        ins(30, 0, &cat(&[&dst(0, 2, 1), &src(0, 2, 0), &src(0, 2, 2)])),
        ins(30, 0, &cat(&[&dst(0, 2, 1), &src(0, 2, 0), &src(0, 2, 3)])),
        ins(43, 0, &cat(&[&dst(2, 0, 2), &src(0, 2, 0)])),
        ins(42, 0, &cat(&[&dst(0, 3, 1), &src(0, 0, 0), &imm(1)])),
        ins(35, 0, &cat(&[&dst(0, 3, 2), &src(0, 0, 0), &src(0, 0, 1), &imm(7)])),
        ins(30, 0, &cat(&[&dst(0, 3, 2), &src(0, 3, 1), &src(0, 3, 0)])),
        ins(43, 0, &cat(&[&dst(2, 0, 4), &src(0, 3, 1)])),
        ins(79, 0, &cat(&[&dst(0, 3, 4), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(80, 0, &cat(&[&dst(0, 3, 8), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(1, 0, &cat(&[&dst(0, 3, 4), &src(0, 3, 2), &imm(1)])),
        ins(1, 0, &cat(&[&dst(0, 3, 8), &src(0, 3, 3), &imm(2)])),
        ins(60, 0, &cat(&[&dst(0, 3, 4), &src(0, 3, 2), &src(0, 3, 3)])),
        ins(43, 0, &cat(&[&dst(2, 0, 8), &src(0, 3, 2)])),
    ]);
    let (e, mut s) = firmas();
    s[0].mascara = 0xF;
    (t, e, s)
}

/// E6d: `udiv` con sus dos destinos, con uno solo (el otro null) y leyendo
/// lo que escribe:
///
/// ```text
///    ftou r0.x, v0.x ; ftou r0.y, v0.y
///    udiv r1.x, r1.y, r0.x, r0.y ; udiv r1.z, null, r0.x, l(7)
///    udiv null, r1.w, r0.y, l(10) ; udiv r0.x, r0.y, r0.y, r0.x
///    iadd r1.w, r1.w, r0.x ; iadd r1.w, r1.w, r0.y
///    utof o0.x, r1.x ; utof o0.y, r1.y ; utof o0.z, r1.z ; utof o0.w, r1.w
/// ```
pub fn sm5_division() -> (Vec<u32>, Vec<Elemento>, Vec<Elemento>) {
    let t = programa_sm5(&[
        ins(28, 0, &cat(&[&dst(0, 0, 1), &src(1, 0, 0)])),
        ins(28, 0, &cat(&[&dst(0, 0, 2), &src(1, 0, 1)])),
        ins(78, 0, &cat(&[&dst(0, 1, 1), &dst(0, 1, 2), &src(0, 0, 0), &src(0, 0, 1)])),
        ins(78, 0, &cat(&[&dst(0, 1, 4), &nulo(), &src(0, 0, 0), &imm(7)])),
        ins(78, 0, &cat(&[&nulo(), &dst(0, 1, 8), &src(0, 0, 1), &imm(10)])),
        ins(78, 0, &cat(&[&dst(0, 0, 1), &dst(0, 0, 2), &src(0, 0, 1), &src(0, 0, 0)])),
        ins(30, 0, &cat(&[&dst(0, 1, 8), &src(0, 1, 3), &src(0, 0, 0)])),
        ins(30, 0, &cat(&[&dst(0, 1, 8), &src(0, 1, 3), &src(0, 0, 1)])),
        ins(86, 0, &cat(&[&dst(2, 0, 1), &src(0, 1, 0)])),
        ins(86, 0, &cat(&[&dst(2, 0, 2), &src(0, 1, 1)])),
        ins(86, 0, &cat(&[&dst(2, 0, 4), &src(0, 1, 2)])),
        ins(86, 0, &cat(&[&dst(2, 0, 8), &src(0, 1, 3)])),
    ]);
    let (e, mut s) = firmas();
    s[0].mascara = 0xF;
    (t, e, s)
}
