//! AVX2: cuatro `flotante64` de golpe.
//!
//! ** Y NO LAS EMITE NADIE SOLO. La seccion 13.7 del maestro decidio que INTI no
//! vectoriza, y eso no cambia: quien quiera cuatro a la vez **lo escribe**. Lo
//! que se prueba aqui es que se pueda escribir y que de el numero correcto.
//!
//! *** El bloque `crudo` que hace falta SE CUENTA, y sale en el manifiesto. Esa
//! es la diferencia con un compilador que vectoriza a tus espaldas: aqui el
//! sitio donde nadie comprueba tiene un numero.

use super::*;

/// Un banco de reales a mano: `a` en `base`, `b` en `base+32`, el resultado en
/// `base+64`. Todo alineado a 8, que es lo que `vmovupd` pide (no alineado).
fn con_reales(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa x86_64\nusa memoria\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n        a = base\n        b = base + 32\n        c = base + 64\n{}",
        cuerpo
    )
}

/// Escribe `v` como los bits de un `flotante64`.
fn pon(dir: &str, i: u64, v: f64) -> String {
    format!("        escribe_natural64({} + {}, {})\n", dir, i * 8, v.to_bits())
}

/// ***CUATRO SUMAS EN UNA INSTRUCCION, y las cuatro salen bien.***
#[test]
fn suma_de_cuatro_suma_los_cuatro() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, (i + 1) as f64);
        cuerpo += &pon("b", i, 10.0);
    }
    cuerpo += "        suma_de_cuatro(c, a, b)\n";
    // El tercero: 3 + 10 = 13.
    cuerpo += "        devuelve lee_natural64(c + 16)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 13.0, "el tercero de los cuatro");
}

/// Y el CUARTO tambien, que es el que se pierde si alguien emite `xmm` en vez de
/// `ymm`: una instruccion SSE haria dos y dejaria los otros dos intactos.
#[test]
fn el_cuarto_no_se_queda_atras() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, (i + 1) as f64);
        cuerpo += &pon("b", i, 100.0);
        cuerpo += &pon("c", i, 0.0);
    }
    cuerpo += "        suma_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c + 24)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 104.0, "4 + 100, y es el cuarto");
}

/// ***`funde_de_cuatro` ACUMULA: lee el destino antes de escribirlo.***
///
/// ** Es la operacion de la que esta hecho un producto de matrices, y la unica
/// de las cuatro que justifica las otras tres. El `231` de `vfmadd231pd` dice
/// exactamente eso: el acumulador es el destino.
#[test]
fn funde_de_cuatro_multiplica_y_acumula() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, 2.0);
        cuerpo += &pon("b", i, 3.0);
        cuerpo += &pon("c", i, 100.0);
    }
    // c += a * b  ->  100 + 6 = 106
    cuerpo += "        funde_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 106.0, "acumula, no pisa");

    // Y dos vueltas acumulan dos veces: 100 + 6 + 6 = 112.
    let mut dos = String::new();
    for i in 0..4 {
        dos += &pon("a", i, 2.0);
        dos += &pon("b", i, 3.0);
        dos += &pon("c", i, 100.0);
    }
    dos += "        funde_de_cuatro(c, a, b)\n";
    dos += "        funde_de_cuatro(c, a, b)\n";
    dos += "        devuelve lee_natural64(c)\n";
    let r = ejecuta_en(&con_reales(&dos), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 112.0);
}

/// Restar y multiplicar tambien, que si no la fila sobra.
#[test]
fn resta_y_producto_de_cuatro() {
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, 10.0);
        cuerpo += &pon("b", i, 4.0);
    }
    cuerpo += "        resta_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c + 8)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 6.0);

    let mut m = String::new();
    for i in 0..4 {
        m += &pon("a", i, 10.0);
        m += &pon("b", i, 4.0);
    }
    m += "        por_de_cuatro(c, a, b)\n";
    m += "        devuelve lee_natural64(c + 8)\n";
    let r = ejecuta_en(&con_reales(&m), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), 40.0);
}

/// ***Y PIDEN `crudo`, que es lo que las hace CONTABLES.***
///
/// ** Escriben 32 bytes en una direccion que da el programa, y nadie comprueba
/// que quepan. Es memoria cruda, igual que `escribe_natural64` -- por eso estan
/// en la misma lista y por eso su uso sale en el manifiesto con un numero.
///
/// *** Esa es la diferencia entera con un compilador que vectoriza a tus
/// espaldas: aqui **el sitio donde nadie comprueba se puede contar**.
#[test]
fn las_de_avx_piden_crudo_y_por_eso_se_cuentan() {
    let fuera = bmo_inti_front::comprobar(
        "perfil llano\nusa x86_64\n\nfuncion f(a es natural64)\n    suma_de_cuatro(a, a, a)\n",
    );
    assert!(
        fuera.codigos().contains(&"E0072"),
        "sin `crudo` tiene que denunciarse: {:?}",
        fuera.codigos()
    );

    let dentro = bmo_inti_front::comprobar(
        "perfil llano\nusa x86_64\n\nfuncion f(a es natural64)\n    crudo\n        suma_de_cuatro(a, a, a)\n",
    );
    assert!(dentro.codigos().is_empty(), "{:?}", dentro.codigos());
    assert_eq!(dentro.valor.bloques_crudo, 1, "y el bloque se CUENTA");
}

// ===================================================================
//  *** CUATRO flotante32 CON SSE: un vertice en un registro (2026-09-26)
// ===================================================================

/// Un programa `llano` con UNA funcion `prueba(base, n)` en `crudo`; la
/// memoria la prepara el banco (`maquina_en`).
fn con_cuatro32(cuerpo: &str) -> String {
    format!(
        "perfil llano\nusa x86_64\nusa memoria\n\nfuncion prueba(base es natural64, n es natural64) devuelve natural64\n    crudo\n{}        devuelve 0\n",
        cuerpo
    )
}

/// Cuatro `f32` en memoria, desde `dir`.
fn pon4(m: &mut Machine, dir: u64, v: [f32; 4]) {
    m.pon_u64(dir, v[0].to_bits() as u64 | (v[1].to_bits() as u64) << 32);
    m.pon_u64(dir + 8, v[2].to_bits() as u64 | (v[3].to_bits() as u64) << 32);
}

/// Cuatro `f32` de memoria, como BITS (la comparacion es bit a bit).
fn lee4(m: &Machine, dir: u64) -> [u32; 4] {
    let (lo, hi) = (m.read_u64(dir), m.read_u64(dir + 8));
    [lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32]
}

const B: u64 = 0x40000;

/// ***Las cuatro operaciones, los CUATRO carriles.*** El cuarto es el que se
/// pierde si alguien emite una instruccion escalar donde va una empaquetada.
#[test]
fn las_de_cuatro32_operan_los_cuatro_carriles() {
    let a = [1.5f32, -2.25, 1e30, 0.1];
    let b = [0.25f32, 4.0, -1e30, 0.2];
    for (nombre, f) in [
        ("suma_de_cuatro32", (|x: f32, y: f32| x + y) as fn(f32, f32) -> f32),
        ("resta_de_cuatro32", |x, y| x - y),
        ("por_de_cuatro32", |x, y| x * y),
    ] {
        let fuente = con_cuatro32(&format!("        {nombre}(base + 32, base, base + 16)\n"));
        let m = maquina_en(&fuente, "prueba", B, 0, |m| {
            pon4(m, B, a);
            pon4(m, B + 16, b);
        });
        let want: [u32; 4] = core::array::from_fn(|k| f(a[k], b[k]).to_bits());
        assert_eq!(lee4(&m, B + 32), want, "{nombre}");
    }
}

/// `reparte_de_cuatro32` copia UN numero en los cuatro carriles.
#[test]
fn reparte_llena_los_cuatro() {
    let fuente = con_cuatro32("        reparte_de_cuatro32(base + 16, base)\n");
    let m = maquina_en(&fuente, "prueba", B, 0, |m| pon4(m, B, [-3.5, 9.0, 9.0, 9.0]));
    assert_eq!(lee4(&m, B + 16), [(-3.5f32).to_bits(); 4]);
}

/// ***`acumula` NO es FMA: dos redondeos, los del juez.*** Con
/// `a = 1 + e`, `b = 1 - e` y el destino a `-1`, el producto exacto es
/// `1 - e^2`, que en `f32` no cabe: FMA lo usa entero y da `-e^2`; dos
/// redondeos lo pierden. Se compara contra la cuenta de Rust en `f32`
/// (`d + a * b`, dos redondeos), que es la del juez del cubo, y la prueba
/// exige tener un carril donde FMA daria otra cosa.
#[test]
fn acumula_redondea_dos_veces_como_el_juez() {
    let a = [1.0f32 + 1.0 / 4096.0, 1.1, 3.3, 1.0 + f32::EPSILON];
    let b = [1.0f32 - 1.0 / 4096.0, 0.7, 1e-8, 1.0 - f32::EPSILON];
    let d = [-1.0f32, 0.3, 7.0, -1.0];
    let fuente = con_cuatro32("        acumula_de_cuatro32(base + 32, base, base + 16)\n");
    let m = maquina_en(&fuente, "prueba", B, 0, |m| {
        pon4(m, B, a);
        pon4(m, B + 16, b);
        pon4(m, B + 32, d);
    });
    let dos: [u32; 4] = core::array::from_fn(|k| (d[k] + a[k] * b[k]).to_bits());
    let fma: [u32; 4] = core::array::from_fn(|k| a[k].mul_add(b[k], d[k]).to_bits());
    assert_eq!(lee4(&m, B + 32), dos, "acumula = destino + a*b con dos redondeos");
    assert_ne!(dos, fma, "la prueba tiene que tener un carril donde FMA da otra cosa");
}

/// ***`funde_de_cuatro` SI es FMA, y el emulador ya no miente sobre ello.***
/// Hasta el 26-09 el emulador hacia `acc + a*b` en dos pasos: con
/// `(1+2^-30)(1-2^-30) - 1` daba `0`, y el Ryzen da `-2^-60`.
#[test]
fn funde_de_cuatro_redondea_una_vez_como_el_silicio() {
    let x = 1.0f64 + 2f64.powi(-30);
    let y = 1.0f64 - 2f64.powi(-30);
    let mut cuerpo = String::new();
    for i in 0..4 {
        cuerpo += &pon("a", i, x);
        cuerpo += &pon("b", i, y);
        cuerpo += &pon("c", i, -1.0);
    }
    cuerpo += "        funde_de_cuatro(c, a, b)\n";
    cuerpo += "        devuelve lee_natural64(c + 8)\n";
    let r = ejecuta_en(&con_reales(&cuerpo), "prueba", 0x40000, 0);
    assert_eq!(f64::from_bits(r), -(2f64.powi(-60)), "FMA: un solo redondeo");
}

/// ***LA PRUEBA DE ORO: el cubo de VERRANO, transformado por INTI, bit a bit
/// contra su juez.*** Los ocho vertices de `bmo_cubo`, por la matriz `wvp` de
/// los 360 angulos, con `reparte` + `por` + tres `acumula` por vertice (las
/// matrices van por columnas: ((c0*x + c1*y) + c2*z) + c3*w, el orden del
/// juez). Tienen que salir EXACTAMENTE los `clip` de
/// `bmo_cubo::mat::transformar`, que son los que VERRANO le manda a la 3060
/// y los que dan el IGUAL con D3D12.
#[test]
fn el_cubo_por_inti_es_el_del_juez_en_los_360() {
    use bmo_cubo::{angulo_de_fotograma, constantes, mat::transformar, vertices, NUM_VERTICES};
    // base: la matriz (64 B), los vertices (NUM_VERTICES x 16), el reparto
    // (16) y la salida (NUM_VERTICES x 16).
    let (mat, ver) = (B, B + 64);
    let rep = ver + 16 * NUM_VERTICES as u64;
    let sal = rep + 16;
    let mut cuerpo = String::new();
    for i in 0..NUM_VERTICES as u64 {
        let (v, o) = (ver + 16 * i, sal + 16 * i);
        cuerpo += &format!("        reparte_de_cuatro32({rep}, {v})\n        por_de_cuatro32({o}, {mat}, {rep})\n");
        for c in 1..4u64 {
            cuerpo += &format!("        reparte_de_cuatro32({rep}, {})\n        acumula_de_cuatro32({o}, {}, {rep})\n", v + 4 * c, mat + 16 * c);
        }
    }
    let fuente = con_cuatro32(&cuerpo);
    let vs = vertices();
    for f in 0..360 {
        let c = constantes(angulo_de_fotograma(f), 1280.0 / 720.0);
        let m = maquina_en(&fuente, "prueba", B, 0, |m| {
            for k in 0..4 {
                pon4(m, mat + 16 * k as u64, [c.wvp[4 * k], c.wvp[4 * k + 1], c.wvp[4 * k + 2], c.wvp[4 * k + 3]]);
            }
            for (i, v) in vs.iter().enumerate() {
                pon4(m, ver + 16 * i as u64, [v.pos[0], v.pos[1], v.pos[2], 1.0]);
            }
        });
        for (i, v) in vs.iter().enumerate() {
            let juez = transformar(&c.wvp, [v.pos[0], v.pos[1], v.pos[2], 1.0]).map(f32::to_bits);
            assert_eq!(lee4(&m, sal + 16 * i as u64), juez, "fotograma {f}, vertice {i}");
        }
    }
    // ** Y lo que se VIO al escribirla: en ESTA cuenta FMA daria lo mismo.
    // Los vertices del cubo son +-1 con w = 1, y multiplicar por +-1 es
    // exacto: no hay nada que redondear dos veces. Donde FMA SI cambia los
    // bits es multiplicando MATRICES (la `wvp` de cada angulo), y ahi es
    // donde `acumula` hace falta (`multiplicar_matrices_con_fma_da_otros_bits`).
}

/// ***Por que `acumula` y no `funde`, medido***: la `wvp` de cada angulo es
/// `(proyeccion * vista) * mundo` (`bmo_cubo::constantes`). En el angulo 0 el
/// mundo es la identidad, asi que su `wvp` ES `proyeccion * vista`; con ella,
/// la `wvp` de cada angulo hecha con FMA sale con OTROS bits en alguno. Y con
/// esos bits el cubo ya no es el de D3D12.
#[test]
fn multiplicar_matrices_con_fma_da_otros_bits() {
    use bmo_cubo::mat::{mul, rotacion_x, rotacion_y};
    use bmo_cubo::{angulo_de_fotograma, constantes};
    let aspecto = 1280.0 / 720.0;
    let pv = constantes(angulo_de_fotograma(0), aspecto).wvp;
    let distintos = (0..360)
        .filter(|&f| {
            let a = angulo_de_fotograma(f);
            let mundo = mul(&rotacion_y(a), &rotacion_x(a * 0.7));
            let juez = constantes(a, aspecto).wvp;
            assert_eq!(mul(&pv, &mundo).map(f32::to_bits), juez.map(f32::to_bits), "la wvp es pv * mundo");
            (0..16).any(|i| {
                let (c, r) = (i / 4, i % 4);
                let (x, y) = (&pv, &mundo);
                let fma = x[12 + r].mul_add(y[c * 4 + 3], x[8 + r].mul_add(y[c * 4 + 2], x[4 + r].mul_add(y[c * 4 + 1], x[r] * y[c * 4])));
                fma.to_bits() != juez[i].to_bits()
            })
        })
        .count();
    assert!(distintos > 0, "con FMA la wvp de algun angulo tiene que salir distinta");
    std::eprintln!("con FMA, {distintos} de 360 angulos dan otra wvp");
}


/// ***LA TANDA ENTERA EN INTI** (`inti/ejemplos/tanda.inti`), fotograma a
/// fotograma contra `bmo_cubo::tanda::de_fotograma`: cuantas caras, y de cada
/// una sus tres vertices en recorte y su color, BIT A BIT. Es lo que VERRANO
/// le manda a la 3060.
#[test]
fn la_tanda_de_inti_es_la_del_juez_en_los_360() {
    use bmo_cubo::{angulo_de_fotograma, constantes, indices, tanda::de_fotograma, vertices};
    let fuente = include_str!("../../../ejemplos/tanda.inti");
    let (w, h) = (1280u32, 720u32);
    let vs = vertices();
    let is = indices();
    for f in 0..360 {
        let c = constantes(angulo_de_fotograma(f), w as f32 / h as f32);
        let m = maquina_en(fuente, "tanda", B, 0, |m| {
            for k in 0..4 {
                pon4(m, B + 16 * k, [c.wvp[4 * k as usize], c.wvp[4 * k as usize + 1], c.wvp[4 * k as usize + 2], c.wvp[4 * k as usize + 3]]);
                pon4(m, B + 64 + 16 * k, [c.world[4 * k as usize], c.world[4 * k as usize + 1], c.world[4 * k as usize + 2], c.world[4 * k as usize + 3]]);
            }
            pon4(m, B + 128, c.luz);
            pon4(m, B + 144, [w as f32 * 0.5, h as f32 * 0.5, 256.0, 0.0]);
            for (i, v) in vs.iter().enumerate() {
                pon4(m, B + 160 + 16 * i as u64, [v.pos[0], v.pos[1], v.pos[2], 1.0]);
                pon4(m, B + 544 + 16 * i as u64, [v.normal[0], v.normal[1], v.normal[2], 0.0]);
                pon4(m, B + 928 + 16 * i as u64, v.color);
            }
            for k in 0..is.len() / 2 {
                m.pon_u64(B + 1312 + 8 * k as u64, is[2 * k] as u64 | (is[2 * k + 1] as u64) << 32);
            }
        });
        let juez = de_fotograma(f, w, h).expect("la tanda de Rust cabe");
        assert_eq!(m.regs[0] as i64, juez.n as i64, "fotograma {f}: cuantas caras");
        for (t, tri) in juez.tris().iter().enumerate() {
            let cara = B + 2440 + 64 * t as u64;
            for v in 0..3 {
                assert_eq!(lee4(&m, cara + 16 * v as u64), tri.clip[v].map(f32::to_bits), "fotograma {f}, cara {t}, vertice {v}");
            }
            assert_eq!(lee4(&m, cara + 48), tri.color.map(f32::to_bits), "fotograma {f}, cara {t}: el color");
        }
    }
}
