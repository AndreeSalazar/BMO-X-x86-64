//! **LAS SERIES DE `<math.h>`, bit a bit contra el oraculo** (2026-09-26).
//!
//! Escalon 4 de la escalera al jefe final (`docs/plan/PLAN_VERRANO.md`, 2d):
//! Quake por software pide `sin`, `cos`, `atan2`, `pow`... y BMO C no los
//! tenia, a proposito: una serie sin juez da numeros que parecen bien.
//!
//! El juez es `bmo_spirv_front::math`, el MISMO que decide que da un seno en
//! un sombreador. Dos pruebas, y las dos hacen falta:
//!
//! - **la tabla**: los coeficientes de `math.h` (en BITS) son los de
//!   `math::table`. Si alguien corrige uno alla y no aqui, se pone rojo con el
//!   nombre delante, y la linea que hay que pegar;
//! - **las cuentas**: un programa de C corre las ocho funciones sobre cientos
//!   de entradas --ceros con signo, infinitos, NaN, subnormales, angulos de
//!   Quake-- en el emulador, y cada resultado tiene que ser el del oraculo,
//!   bit a bit (un NaN, NaN).

use super::*;
use bmo_spirv_front::math as m;
use bmo_spirv_front::math::table as t;

const MATH_H: &str = include_str!("../../../../../forge/sem-asm/tables/standards/C/math.h");

/// Los bits de la constante `#define BMO_M_<nombre> 0x...ULL`.
fn define(nombre: &str) -> u64 {
    let clave = format!("#define BMO_M_{nombre} 0x");
    let l = MATH_H.lines().find(|l| l.starts_with(&clave)).unwrap_or_else(|| panic!("math.h no define BMO_M_{nombre}"));
    u64::from_str_radix(l[clave.len()..].trim().trim_end_matches("ULL"), 16).unwrap()
}

/// Los bits del arreglo `bmo_m_<nombre>[N] = { ... }`.
fn arreglo(nombre: &str) -> Vec<u64> {
    let clave = format!("bmo_m_{nombre}[");
    let i = MATH_H.find(&clave).unwrap_or_else(|| panic!("math.h no tiene bmo_m_{nombre}"));
    let cuerpo = &MATH_H[i..];
    let (a, b) = (cuerpo.find('{').unwrap(), cuerpo.find('}').unwrap());
    cuerpo[a + 1..b]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| u64::from_str_radix(s.trim_start_matches("0x").trim_end_matches("ULL"), 16).unwrap())
        .collect()
}

#[test]
fn la_tabla_de_math_h_es_la_del_oraculo() {
    let sueltas = [
        ("FRAC_PI_2_HI", t::FRAC_PI_2_HI),
        ("FRAC_PI_2_LO", t::FRAC_PI_2_LO),
        ("FRAC_2_PI", t::FRAC_2_PI),
        ("LN2_HI", t::LN2_HI),
        ("LN2_LO", t::LN2_LO),
        ("INV_LN2", t::INV_LN2),
        ("NEAREST", t::NEAREST),
        ("EXP_MAX", t::EXP_MAX),
        ("EXP_MIN", t::EXP_MIN),
        ("SQRT_2", t::SQRT_2),
        ("FRAC_PI_6", t::FRAC_PI_6),
        ("SQRT_3", t::SQRT_3),
        ("TAN_PI_12", t::TAN_PI_12),
    ];
    for (n, v) in sueltas {
        assert_eq!(define(n), v.to_bits(), "BMO_M_{n}: en math.h va\n#define BMO_M_{n} 0x{:016X}ULL", v.to_bits());
    }
    let series: [(&str, &[f64]); 5] = [("SIN", &t::SIN), ("COS", &t::COS), ("EXP", &t::EXP), ("LN", &t::LN), ("ATAN", &t::ATAN)];
    for (n, v) in series {
        let bits: Vec<u64> = v.iter().map(|x| x.to_bits()).collect();
        assert_eq!(arreglo(n), bits, "bmo_m_{n} no es math::table::{n}");
    }
}

/// Las entradas: lo que Quake mide, y los bordes de C99.
fn entradas() -> Vec<(f64, f64)> {
    let especiales = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        2.0,
        1e-300,
        -1e-300,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1e300,
        -1e300,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        core::f64::consts::PI,
        core::f64::consts::FRAC_PI_2,
        -core::f64::consts::FRAC_PI_4,
        0.267_949_192_431_122_7,
        3.0,
        -3.0,
        710.0,
        -746.0,
        1e5,
    ];
    let mut v = Vec::new();
    for &a in &especiales {
        for &b in &[0.0, -0.0, 1.0, -2.0, 0.5, 3.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -7.25] {
            v.push((a, b));
        }
    }
    // Angulos de Quake (grados a radianes), y un barrido ancho y denso.
    let mut x = -1000.0f64;
    while x < 1000.0 {
        v.push((x, 0.37 - x * 0.61));
        x += 7.389_056;
    }
    let mut x = -4.0f64;
    while x < 4.0 {
        v.push((x, x * x - 1.5));
        x += 0.061_3;
    }
    v
}

fn bits(v: f64) -> String {
    format!("0x{:016X}ULL", v.to_bits())
}

/// Lo que el oraculo dice de `(x, y)`, en el orden del programa de C.
fn oraculo(x: f64, y: f64) -> [f64; 8] {
    let (s, c) = m::sin_cos_f64(x);
    [s, c, m::tan_f64(x), m::atan_f64(x), m::atan2_f64(x, y), m::exp_f64(x), m::ln_f64(x), m::pow_c_f64(x, y)]
}

const NOMBRES: [&str; 8] = ["sin", "cos", "tan", "atan", "atan2", "exp", "log", "pow"];

/// Un programa de C con `e` entradas: una linea por entrada, las ocho
/// funciones en hexadecimal.
fn programa(e: &[(f64, f64)]) -> String {
    let xs: Vec<String> = e.iter().map(|p| bits(p.0)).collect();
    let ys: Vec<String> = e.iter().map(|p| bits(p.1)).collect();
    format!(
        "#include <stdio.h>\n#include <math.h>\n\
         static const unsigned long long X[{n}] = {{ {xs} }};\n\
         static const unsigned long long Y[{n}] = {{ {ys} }};\n\
         int main() {{ int i; for (i = 0; i < {n}; i++) {{\n\
           double x = bmo_m_d(X[i]); double y = bmo_m_d(Y[i]);\n\
           printf(\"%llx %llx %llx %llx %llx %llx %llx %llx\\n\", bmo_m_bits(sin(x)), bmo_m_bits(cos(x)), bmo_m_bits(tan(x)),\n\
             bmo_m_bits(atan(x)), bmo_m_bits(atan2(x, y)), bmo_m_bits(exp(x)), bmo_m_bits(log(x)), bmo_m_bits(pow(x, y)));\n\
         }} return 0; }}\n",
        n = e.len(),
        xs = xs.join(", "),
        ys = ys.join(", ")
    )
}

#[test]
fn las_series_de_math_h_dan_los_bits_del_oraculo() {
    let e = entradas();
    let mut malos = Vec::new();
    // De 40 en 40: el banco corta a las 500.000 instrucciones por programa, y
    // cada entrada son ocho series y un `printf`.
    for (tanda, trozo) in e.chunks(40).enumerate() {
        let src = programa(trozo);
        let salida = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {
                let bef = compile_with_preprocessor(&src, std::path::Path::new("serie.c"), CStandard::C11).expect("math.h compila");
                ejecutar_bef(&bef)
            })
            .unwrap()
            .join()
            .expect("el programa de las series corre");
        let lineas: Vec<&str> = salida.lines().collect();
        assert_eq!(lineas.len(), trozo.len(), "tanda {tanda}: una linea por entrada; salio:\n{salida}");
        for (i, (l, &(x, y))) in lineas.iter().zip(trozo).enumerate() {
            let de_c: Vec<u64> = l.split_whitespace().map(|h| u64::from_str_radix(h, 16).unwrap()).collect();
            for (k, esperado) in oraculo(x, y).iter().enumerate() {
                let c = f64::from_bits(de_c[k]);
                let bien = c.to_bits() == esperado.to_bits() || (c.is_nan() && esperado.is_nan());
                if !bien {
                    malos.push(format!("#{} {}({x:e}, {y:e}): C {c:e} ({:#x}), oraculo {esperado:e}", tanda * 40 + i, NOMBRES[k], de_c[k]));
                }
            }
        }
    }
    assert!(malos.is_empty(), "{} de {} resultados distintos:\n{}", malos.len(), e.len() * 8, malos[..malos.len().min(20)].join("\n"));
}

/// Y las de `float`: la de doble y UN redondeo, como `math::sin` del
/// sombreador. Un `sinf` de C y un `sin` de GLSL dan el mismo `f32`.
#[test]
fn sinf_es_el_seno_del_sombreador() {
    let xs = [0.0f32, 0.5, -1.25, 3.0, 100.0, -0.001];
    let src = format!(
        "#include <stdio.h>\n#include <math.h>\nint main() {{ float x[6] = {{ {} }}; int i;\n\
         for (i = 0; i < 6; i++) {{ float s = sinf(x[i]); float c = cosf(x[i]); printf(\"%x %x\\n\", *(unsigned int *)&s, *(unsigned int *)&c); }}\n\
         return 0; }}\n",
        xs.iter().map(|x| format!("{x:?}f")).collect::<Vec<_>>().join(", ")
    );
    let bef = compile_with_preprocessor(&src, std::path::Path::new("sinf.c"), CStandard::C11).expect("compila");
    let salida = ejecutar_bef(&bef);
    for (l, &x) in salida.lines().zip(&xs) {
        let v: Vec<u32> = l.split_whitespace().map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
        assert_eq!(v, [m::sin(x).to_bits(), m::cos(x).to_bits()], "sinf/cosf({x})");
    }
    assert_eq!(salida.lines().count(), xs.len(), "{salida}");
}
