//! **Las mates del CRT, de la casa** (tanda 1 de Cyberpunk, 29-09): lo que
//! el censo de `Cyberpunk2077.exe` y sus DLL pide de
//! `api-ms-win-crt-math-l1-1-0.dll` y de `msvcrt.dll`.
//!
//! ```text
//!    con el double en xmm0   sqrt sin cos tan asin acos atan atan2 log pow
//!                            fmod modf frexp trunc round ceilf floorf roundf
//!    clasificar              _dclass _dsign _dtest _fdclass
//!    enteros                 div, qsort, bsearch
//! ```
//!
//! La casa es soft-float (no sabe de xmm0), asi que cada una entra por un
//! trampolin de `global_asm!` que pasa los bits del double a los registros
//! enteros, llama a la de Rust y los devuelve a xmm0. Lo que pide calculo de
//! verdad (seno, logaritmo, potencia) lo hace el **x87** del propio CPU
//! (`fsin`, `fyl2x`, `f2xm1`...): exacto a 64 bits de mantisa, sin `libm`.
//! Lo raro de `pow` (base negativa, ceros, infinitos) va en Rust, antes.

use crate::dir;

// -- Los trampolines -------------------------------------------------------------------------
//
//    d_d   double f(double)            -> f_bits(bits) -> bits
//    d_dd  double f(double, double)    -> f_bits(bits, bits) -> bits
//    d_dp  double f(double, T*)        -> f_bits(bits, ptr) -> bits
//    i_d   int f(double)               -> f_bits(bits) -> int
//    f_f   float f(float)              -> f_bits(bits32) -> bits32
//    i_f   int f(float)                -> f_bits(bits32) -> int

macro_rules! trampolin {
    ($nombre:literal, $bits:literal, d_d) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "sub rsp, 40", "movq rcx, xmm0", concat!("call ", $bits), "movq xmm0, rax", "add rsp, 40", "ret");
    };
    ($nombre:literal, $bits:literal, d_dd) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "sub rsp, 40", "movq rcx, xmm0", "movq rdx, xmm1", concat!("call ", $bits), "movq xmm0, rax", "add rsp, 40", "ret");
    };
    ($nombre:literal, $bits:literal, d_dp) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "sub rsp, 40", "movq rcx, xmm0", concat!("call ", $bits), "movq xmm0, rax", "add rsp, 40", "ret");
    };
    ($nombre:literal, $bits:literal, i_d) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "movq rcx, xmm0", concat!("jmp ", $bits));
    };
    ($nombre:literal, $bits:literal, f_f) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "sub rsp, 40", "movd ecx, xmm0", concat!("call ", $bits), "movd xmm0, eax", "add rsp, 40", "ret");
    };
    ($nombre:literal, $bits:literal, i_f) => {
        core::arch::global_asm!(concat!(".globl ", $nombre), concat!($nombre, ":"), "movd ecx, xmm0", concat!("jmp ", $bits));
    };
}

trampolin!("proton_x_sin", "proton_x_sin_bits", d_d);
trampolin!("proton_x_cos", "proton_x_cos_bits", d_d);
trampolin!("proton_x_tan", "proton_x_tan_bits", d_d);
trampolin!("proton_x_asin", "proton_x_asin_bits", d_d);
trampolin!("proton_x_acos", "proton_x_acos_bits", d_d);
trampolin!("proton_x_atan", "proton_x_atan_bits", d_d);
trampolin!("proton_x_atan2", "proton_x_atan2_bits", d_dd);
trampolin!("proton_x_log", "proton_x_log_bits", d_d);
trampolin!("proton_x_pow", "proton_x_pow_bits", d_dd);
trampolin!("proton_x_fmod", "proton_x_fmod_bits", d_dd);
trampolin!("proton_x_modf", "proton_x_modf_bits", d_dp);
trampolin!("proton_x_frexp", "proton_x_frexp_bits", d_dp);
trampolin!("proton_x_round", "proton_x_round_bits", d_d);
trampolin!("proton_x_dclass", "proton_x_dclass_bits", i_d);
trampolin!("proton_x_dsign", "proton_x_dsign_bits", i_d);
trampolin!("proton_x_fdclass", "proton_x_fdclass_bits", i_f);
trampolin!("proton_x_roundf", "proton_x_roundf_bits", f_f);

// Las de una instruccion, sin pasar por Rust: SSE2 y SSE4.1 (ROUNDSD/SS con
// modo 1 hacia -inf, 2 hacia +inf, 3 hacia cero).
core::arch::global_asm!(".globl proton_x_sqrt", "proton_x_sqrt:", "sqrtsd xmm0, xmm0", "ret");
core::arch::global_asm!(".globl proton_x_trunc", "proton_x_trunc:", "roundsd xmm0, xmm0, 3", "ret");
core::arch::global_asm!(".globl proton_x_ceilf", "proton_x_ceilf:", "roundss xmm0, xmm0, 2", "ret");
core::arch::global_asm!(".globl proton_x_floorf", "proton_x_floorf:", "roundss xmm0, xmm0, 1", "ret");

extern "C" {
    fn proton_x_sin();
    fn proton_x_cos();
    fn proton_x_tan();
    fn proton_x_asin();
    fn proton_x_acos();
    fn proton_x_atan();
    fn proton_x_atan2();
    fn proton_x_log();
    fn proton_x_pow();
    fn proton_x_fmod();
    fn proton_x_modf();
    fn proton_x_frexp();
    fn proton_x_round();
    fn proton_x_dclass();
    fn proton_x_dsign();
    fn proton_x_fdclass();
    fn proton_x_roundf();
    fn proton_x_sqrt();
    fn proton_x_trunc();
    fn proton_x_ceilf();
    fn proton_x_floorf();
}

// -- El x87 ----------------------------------------------------------------------------------

/// Una operacion del x87 sobre un double: se carga, `$op...`, se guarda.
macro_rules! x87_1 {
    ($x:expr, $($op:literal),+) => {{
        let mut v: u64 = $x;
        // SAFETY: solo la pila del x87 (vacia a la entrada y a la salida,
        // como pide la convencion de Windows) y `v`, que es nuestro.
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", $($op,)+ "fstp qword ptr [{p}]", p = in(reg) &mut v,
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _);
        }
        v
    }};
}

const NAN: u64 = 0x7FF8_0000_0000_0000;
const INF: u64 = 0x7FF0_0000_0000_0000;
const SIGNO: u64 = 1 << 63;
const UNO: u64 = 0x3FF0_0000_0000_0000;

fn es_nan(b: u64) -> bool {
    b & !SIGNO > INF
}

fn es_inf(b: u64) -> bool {
    b & !SIGNO == INF
}

#[no_mangle]
extern "win64" fn proton_x_sin_bits(x: u64) -> u64 {
    // FSIN solo reduce |x| < 2^63; mas alla (o inf) Windows da NaN.
    if es_nan(x) || (x & !SIGNO) >= 0x43E0_0000_0000_0000 {
        return NAN;
    }
    x87_1!(x, "fsin")
}

#[no_mangle]
extern "win64" fn proton_x_cos_bits(x: u64) -> u64 {
    if es_nan(x) || (x & !SIGNO) >= 0x43E0_0000_0000_0000 {
        return NAN;
    }
    x87_1!(x, "fcos")
}

#[no_mangle]
extern "win64" fn proton_x_tan_bits(x: u64) -> u64 {
    if es_nan(x) || (x & !SIGNO) >= 0x43E0_0000_0000_0000 {
        return NAN;
    }
    // FPTAN deja 1.0 encima de la tangente: fuera.
    x87_1!(x, "fptan", "fstp st(0)")
}

#[no_mangle]
extern "win64" fn proton_x_atan_bits(x: u64) -> u64 {
    x87_1!(x, "fld1", "fpatan")
}

#[no_mangle]
extern "win64" fn proton_x_asin_bits(x: u64) -> u64 {
    if es_nan(x) || (x & !SIGNO) > UNO {
        return NAN;
    }
    // asin x = atan2(x, sqrt(1 - x*x)).
    x87_1!(x, "fld st(0)", "fmul st(0), st(0)", "fld1", "fsubrp st(1), st(0)", "fsqrt", "fpatan")
}

#[no_mangle]
extern "win64" fn proton_x_acos_bits(x: u64) -> u64 {
    if es_nan(x) || (x & !SIGNO) > UNO {
        return NAN;
    }
    // acos x = atan2(sqrt(1 - x*x), x).
    x87_1!(x, "fld st(0)", "fmul st(0), st(0)", "fld1", "fsubrp st(1), st(0)", "fsqrt", "fxch st(1)", "fpatan")
}

#[no_mangle]
extern "win64" fn proton_x_atan2_bits(y: u64, x: u64) -> u64 {
    let mut v = [y, x];
    // SAFETY: como `x87_1!`; FPATAN da atan(ST1/ST0) con el cuadrante bueno.
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", "fld qword ptr [{p} + 8]", "fpatan", "fstp qword ptr [{p}]", p = in(reg) &mut v,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _);
    }
    v[0]
}

#[no_mangle]
extern "win64" fn proton_x_log_bits(x: u64) -> u64 {
    if es_nan(x) {
        return x;
    }
    if x & !SIGNO == 0 {
        return INF | SIGNO;
    }
    if x & SIGNO != 0 {
        return NAN;
    }
    if x == INF {
        return INF;
    }
    // ln x = ln2 * log2 x.
    x87_1!(x, "fldln2", "fxch st(1)", "fyl2x")
}

/// `x^y` con x > 0 finito: 2^(y log2 x), la parte entera con FSCALE.
fn pow_positivo(x: u64, y: u64) -> u64 {
    let mut v = [x, y];
    // SAFETY: como `x87_1!`. F2XM1 pide |f| <= 1: FRNDINT deja |f| <= 0.5.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{p} + 8]",
            "fld qword ptr [{p}]",
            "fyl2x",
            "fld st(0)",
            "frndint",
            "fsub st(1), st(0)",
            "fxch st(1)",
            "f2xm1",
            "fld1",
            "faddp st(1), st(0)",
            "fscale",
            "fstp st(1)",
            "fstp qword ptr [{p}]",
            p = in(reg) &mut v,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _
        );
    }
    v[0]
}

/// Si el double `y` es entero, y si es impar.
fn entero_impar(y: u64) -> Option<bool> {
    let e = ((y >> 52) & 0x7FF) as i32 - 1023;
    if es_nan(y) || es_inf(y) {
        return None;
    }
    if y & !SIGNO == 0 {
        return Some(false);
    }
    if e < 0 {
        return None;
    }
    if e >= 53 {
        return Some(false);
    }
    let m = (y & ((1 << 52) - 1)) | (1 << 52);
    let fraccion = m & ((1u64 << (52 - e)) - 1);
    (fraccion == 0).then(|| (m >> (52 - e)) & 1 == 1)
}

#[no_mangle]
extern "win64" fn proton_x_pow_bits(x: u64, y: u64) -> u64 {
    // Las reglas de C99 (F.9.4.4), en el orden en que ganan.
    if y & !SIGNO == 0 || x == UNO {
        return UNO;
    }
    if es_nan(x) || es_nan(y) {
        return NAN;
    }
    let ax = x & !SIGNO;
    let neg = x & SIGNO != 0;
    let impar = entero_impar(y);
    if es_inf(y) {
        if ax == UNO {
            return UNO;
        }
        let grande = ax > UNO;
        return if grande == (y & SIGNO == 0) { INF } else { 0 };
    }
    if ax == 0 {
        let s = if neg && impar == Some(true) { SIGNO } else { 0 };
        return if y & SIGNO != 0 { INF | s } else { s };
    }
    if es_inf(x) {
        let s = if neg && impar == Some(true) { SIGNO } else { 0 };
        return if y & SIGNO != 0 { s } else { INF | s };
    }
    if neg {
        return match impar {
            None => NAN,
            Some(i) => pow_positivo(ax, y) | if i { SIGNO } else { 0 },
        };
    }
    pow_positivo(x, y)
}

#[no_mangle]
extern "win64" fn proton_x_fmod_bits(x: u64, y: u64) -> u64 {
    if es_nan(x) || es_nan(y) || es_inf(x) || y & !SIGNO == 0 {
        return NAN;
    }
    if es_inf(y) {
        return x;
    }
    let mut v = [y, x];
    // SAFETY: como `x87_1!`. FPREM se repite hasta que C2 dice que acabo.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{p}]",
            "fld qword ptr [{p} + 8]",
            "2:",
            "fprem",
            "fnstsw ax",
            "test ah, 4",
            "jnz 2b",
            "fstp st(1)",
            "fstp qword ptr [{p}]",
            p = in(reg) &mut v,
            out("ax") _,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _
        );
    }
    v[0]
}

/// Hacia cero, por los bits.
fn truncar(x: u64) -> u64 {
    let e = ((x >> 52) & 0x7FF) as i32 - 1023;
    if e >= 52 {
        return x;
    }
    if e < 0 {
        return x & SIGNO;
    }
    x & !((1u64 << (52 - e)) - 1)
}

#[no_mangle]
extern "win64" fn proton_x_modf_bits(x: u64, entera: *mut u64) -> u64 {
    let t = truncar(x);
    if !entera.is_null() {
        // SAFETY: el double del `.exe` donde va la parte entera.
        unsafe { entera.write_unaligned(t) };
    }
    if es_nan(x) {
        return x;
    }
    if es_inf(x) {
        return x & SIGNO;
    }
    let r = (f64::from_bits(x) - f64::from_bits(t)).to_bits();
    // La fraccion lleva el signo de x, tambien cuando es 0.
    (r & !SIGNO) | (x & SIGNO)
}

#[no_mangle]
extern "win64" fn proton_x_frexp_bits(x: u64, exp: *mut i32) -> u64 {
    let pon = |e: i32| {
        if !exp.is_null() {
            // SAFETY: el int del `.exe` donde va el exponente.
            unsafe { exp.write_unaligned(e) };
        }
    };
    let mut b = x;
    let mut ajuste = 0;
    if x & !SIGNO == 0 || es_nan(x) || es_inf(x) {
        pon(0);
        return x;
    }
    if (x >> 52) & 0x7FF == 0 {
        // Subnormal: por 2^54, y se descuenta.
        b = (f64::from_bits(x) * f64::from_bits(0x4350_0000_0000_0000)).to_bits();
        ajuste = -54;
    }
    let e = ((b >> 52) & 0x7FF) as i32 - 1022;
    pon(e + ajuste);
    (b & !(0x7FF << 52)) | (1022 << 52)
}

#[no_mangle]
extern "win64" fn proton_x_round_bits(x: u64) -> u64 {
    // La mitad, lejos del cero: trunc(x + 0.5 con el signo de x), salvo
    // cuando x ya es entero (sumar 0.5 a 2^52 - 1 se pasaria).
    let t = truncar(x);
    if t == x || es_nan(x) {
        return x;
    }
    let d = (f64::from_bits(x) - f64::from_bits(t)).to_bits() & !SIGNO;
    if d >= 0x3FE0_0000_0000_0000 {
        let uno = UNO | (x & SIGNO);
        (f64::from_bits(t) + f64::from_bits(uno)).to_bits()
    } else {
        t
    }
}

#[no_mangle]
extern "win64" fn proton_x_roundf_bits(x: u32) -> u32 {
    let e = ((x >> 23) & 0xFF) as i32 - 127;
    if e >= 23 || (x & 0x7FFF_FFFF) > 0x7F80_0000 {
        return x;
    }
    let s = x & 0x8000_0000;
    if e < -1 {
        return s;
    }
    if e == -1 {
        return s | 0x3F80_0000;
    }
    let m = (x & 0x7F_FFFF) | 0x80_0000;
    let corte = 23 - e as u32;
    let mut entero = (m + (1 << (corte - 1))) >> corte;
    // Reconstruir el float del entero (cabe en 24 bits).
    if entero == 0 {
        return s;
    }
    let mut exp = 127 + 23;
    while entero & 0x80_0000 == 0 {
        entero <<= 1;
        exp -= 1;
    }
    while entero > 0xFF_FFFF {
        entero >>= 1;
        exp += 1;
    }
    s | (exp << 23) | (entero & 0x7F_FFFF)
}

// -- Clasificar ------------------------------------------------------------------------------

const FP_INFINITE: i32 = 1;
const FP_NAN: i32 = 2;
const FP_NORMAL: i32 = -1;
const FP_SUBNORMAL: i32 = -2;
const FP_ZERO: i32 = 0;

#[no_mangle]
extern "win64" fn proton_x_dclass_bits(x: u64) -> i32 {
    match ((x >> 52) & 0x7FF, x & ((1 << 52) - 1)) {
        (0x7FF, 0) => FP_INFINITE,
        (0x7FF, _) => FP_NAN,
        (0, 0) => FP_ZERO,
        (0, _) => FP_SUBNORMAL,
        _ => FP_NORMAL,
    }
}

#[no_mangle]
extern "win64" fn proton_x_dsign_bits(x: u64) -> i32 {
    // El UCRT da el bit de signo en su sitio (0x8000 de la palabra alta).
    ((x >> 48) & 0x8000) as i32
}

/// `_dtest(const double*)`: el double llega por puntero, no en xmm0.
extern "win64" fn dtest(p: *const u64) -> i32 {
    // SAFETY: un double del `.exe`.
    proton_x_dclass_bits(unsafe { p.read_unaligned() })
}

#[no_mangle]
extern "win64" fn proton_x_fdclass_bits(x: u32) -> i32 {
    match ((x >> 23) & 0xFF, x & 0x7F_FFFF) {
        (0xFF, 0) => FP_INFINITE,
        (0xFF, _) => FP_NAN,
        (0, 0) => FP_ZERO,
        (0, _) => FP_SUBNORMAL,
        _ => FP_NORMAL,
    }
}

// -- Enteros ---------------------------------------------------------------------------------

/// `div(a, b)`: un `div_t` (cociente, resto) de 8 bytes vuelve en rax.
extern "win64" fn div(a: i32, b: i32) -> u64 {
    if b == 0 {
        crate::aviso("div: entre cero (en Windows, excepcion de division)");
        return 0;
    }
    let q = a.wrapping_div(b);
    let r = a.wrapping_rem(b);
    (q as u32 as u64) | ((r as u32 as u64) << 32)
}

type Comparar = extern "win64" fn(*const u8, *const u8) -> i32;

/// `qsort`: por monticulo, en su sitio (qsort no promete estabilidad) y
/// sin pedir memoria. O(n log n) siempre: un juego no va a notar el peor
/// caso de un quicksort ingenuo, y este no lo tiene.
extern "win64" fn qsort(base: *mut u8, n: usize, tam: usize, cmp: Comparar) {
    if base.is_null() || n < 2 || tam == 0 {
        return;
    }
    let el = |i: usize| unsafe { base.add(i * tam) };
    let cambiar = |i: usize, j: usize| {
        if i != j {
            // SAFETY: dos elementos distintos del arreglo del `.exe`.
            unsafe { core::ptr::swap_nonoverlapping(el(i), el(j), tam) };
        }
    };
    let hundir = |mut i: usize, fin: usize| loop {
        let mut mayor = i;
        for h in [2 * i + 1, 2 * i + 2] {
            if h < fin && cmp(el(h), el(mayor)) > 0 {
                mayor = h;
            }
        }
        if mayor == i {
            break;
        }
        cambiar(i, mayor);
        i = mayor;
    };
    for i in (0..n / 2).rev() {
        hundir(i, n);
    }
    for fin in (1..n).rev() {
        cambiar(0, fin);
        hundir(0, fin);
    }
}

extern "win64" fn bsearch(clave: *const u8, base: *const u8, n: usize, tam: usize, cmp: Comparar) -> *const u8 {
    let (mut a, mut b) = (0usize, n);
    while a < b {
        let m = a + (b - a) / 2;
        // SAFETY: un elemento del arreglo del `.exe`.
        let e = unsafe { base.add(m * tam) };
        match cmp(clave, e) {
            0 => return e,
            c if c < 0 => b = m,
            _ => a = m + 1,
        }
    }
    core::ptr::null()
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "sqrt" => dir!(proton_x_sqrt),
        "sin" => dir!(proton_x_sin),
        "cos" => dir!(proton_x_cos),
        "tan" => dir!(proton_x_tan),
        "asin" => dir!(proton_x_asin),
        "acos" => dir!(proton_x_acos),
        "atan" => dir!(proton_x_atan),
        "atan2" => dir!(proton_x_atan2),
        "log" => dir!(proton_x_log),
        "pow" => dir!(proton_x_pow),
        "fmod" => dir!(proton_x_fmod),
        "modf" => dir!(proton_x_modf),
        "frexp" => dir!(proton_x_frexp),
        "trunc" => dir!(proton_x_trunc),
        "round" => dir!(proton_x_round),
        "roundf" => dir!(proton_x_roundf),
        "ceilf" => dir!(proton_x_ceilf),
        "floorf" => dir!(proton_x_floorf),
        "_dclass" => dir!(proton_x_dclass),
        "_dsign" => dir!(proton_x_dsign),
        "_dtest" => dir!(dtest),
        "_fdclass" => dir!(proton_x_fdclass),
        "div" => dir!(div),
        "qsort" => dir!(qsort),
        "bsearch" => dir!(bsearch),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;

    fn b(x: f64) -> u64 {
        x.to_bits()
    }

    #[test]
    fn pow_sigue_las_reglas_de_c99() {
        assert_eq!(proton_x_pow_bits(b(2.0), b(10.0)), b(1024.0));
        assert_eq!(proton_x_pow_bits(b(-2.0), b(3.0)), b(-8.0));
        assert!(es_nan(proton_x_pow_bits(b(-2.0), b(0.5))));
        assert_eq!(proton_x_pow_bits(b(0.0), b(-1.0)), INF);
        assert_eq!(proton_x_pow_bits(b(-0.0), b(-3.0)), INF | SIGNO);
        assert_eq!(proton_x_pow_bits(b(f64::NAN), b(0.0)), UNO);
        assert!((f64::from_bits(proton_x_pow_bits(b(9.0), b(0.5))) - 3.0).abs() < 1e-15);
    }

    #[test]
    fn el_x87_da_el_seno_el_logaritmo_y_el_resto() {
        assert!((f64::from_bits(proton_x_sin_bits(b(1.0))) - 1f64.sin()).abs() < 1e-15);
        assert!((f64::from_bits(proton_x_log_bits(b(10.0))) - 10f64.ln()).abs() < 1e-15);
        assert!((f64::from_bits(proton_x_atan2_bits(b(1.0), b(-1.0))) - 1f64.atan2(-1.0)).abs() < 1e-15);
        assert!((f64::from_bits(proton_x_asin_bits(b(0.5))) - 0.5f64.asin()).abs() < 1e-15);
        assert!((f64::from_bits(proton_x_acos_bits(b(-0.5))) - (-0.5f64).acos()).abs() < 1e-15);
        assert_eq!(proton_x_fmod_bits(b(7.5), b(2.0)), b(1.5));
        assert_eq!(proton_x_fmod_bits(b(-7.5), b(2.0)), b(-1.5));
    }

    #[test]
    fn redondear_y_partir() {
        assert_eq!(proton_x_round_bits(b(2.5)), b(3.0));
        assert_eq!(proton_x_round_bits(b(-2.5)), b(-3.0));
        assert_eq!(proton_x_round_bits(b(0.49999999999999994)), b(0.0));
        assert_eq!(proton_x_roundf_bits(2.5f32.to_bits()), 3f32.to_bits());
        assert_eq!(proton_x_roundf_bits((-0.4f32).to_bits()), (-0f32).to_bits());
        let mut e = 0;
        assert_eq!(proton_x_frexp_bits(b(8.0), &mut e), b(0.5));
        assert_eq!(e, 4);
        let mut t = 0u64;
        assert_eq!(proton_x_modf_bits(b(-3.75), &mut t), b(-0.75));
        assert_eq!(t, b(-3.0));
        assert_eq!(proton_x_dclass_bits(b(0.0)), FP_ZERO);
        assert_eq!(proton_x_dclass_bits(INF), FP_INFINITE);
    }

    #[test]
    fn qsort_y_bsearch() {
        extern "win64" fn cmp(a: *const u8, b: *const u8) -> i32 {
            unsafe { (a as *const i32).read_unaligned().cmp(&(b as *const i32).read_unaligned()) as i32 }
        }
        let mut v = [5i32, -1, 9, 3, 3, 0, 7, 100, -50, 2];
        qsort(v.as_mut_ptr() as *mut u8, v.len(), 4, cmp);
        assert_eq!(v, [-50, -1, 0, 2, 3, 3, 5, 7, 9, 100]);
        let k = 7i32;
        let p = bsearch(&k as *const i32 as *const u8, v.as_ptr() as *const u8, v.len(), 4, cmp);
        assert_eq!(p, &v[7] as *const i32 as *const u8);
        assert_eq!(div(-7, 2), (-3i32 as u32 as u64) | ((-1i32 as u32 as u64) << 32));
    }
}
