//! E8f y E8g (DL18 y DL17, 09-10): el juez con el LDC de la app y con las
//! formas de TEX y TLD -- lo que lee cada una, cuando, y lo que R7 le deja
//! a un cuerpo --, cada regla con su bodrio. Salieron de `juez.rs` (10-10)
//! moviendo texto, para que el juez no pase de la linea de L6a.

use bmo_sm86::codifica::RZ;

use super::juez::{conoce, juzgar_cuerpo_con, juzgar_cuerpo_con_asas, juzgar_cuerpo_de_app, juzgar_drenado, BancoDeApp, Contexto, Permisos, Regla};

/// ** E8g (DL17): las formas de TEX y TLD -- lo que lee cada una (sus
/// coordenadas y, con .LL, el nivel en Rb+1) y lo que NO se sabe: R0 --;
/// y R7 deja el TLD como el TEX, con el asa del kernel.
#[test]
fn e8g_las_formas_de_tex_y_tld() {
    extern crate std;
    use bmo_sm86::codifica::{self as c, DimTex, Fuente, NivelTex};
    let ctl = |espera: u64, wbar: u64, mascara: u64| espera | 1 << 4 | wbar << 5 | 7 << 8 | mascara << 11;
    let ctx = Contexto { registros: 24, sph: None };
    let fin = c::exit(ctl(5, 7, 1));
    // Un TEX 3D con su nivel: u, v, w en R8..R10, el asa en R4 y el nivel en R5.
    let poner = |r: u8, v: u32| c::mov(r, Fuente::Imm(v), ctl(1, 7, 0));
    let tex3d = c::tex_forma(12, 8, 4, DimTex::D3, NivelTex::De, ctl(2, 0, 0));
    let antes = [poner(4, 0), poner(8, 0), poner(9, 0), poner(10, 0), c::mov(5, Fuente::Imm(0), ctl(6, 7, 0))];
    let bien: std::vec::Vec<_> = antes.iter().copied().chain([tex3d, fin]).collect();
    assert!(juzgar_drenado(&bien, &ctx).is_ok(), "{:?}", juzgar_drenado(&bien, &ctx));
    // El nivel (R5) escrito justo antes, sin esperar su latencia: R2.
    let mut corto = bien.clone();
    corto[4] = c::mov(5, Fuente::Imm(0), ctl(1, 7, 0));
    assert_eq!(juzgar_drenado(&corto, &ctx).unwrap_err().regla, Regla::R2EsperaCorta, "Rb+1 tambien se lee");
    // El resultado leido sin esperar su barrera: R1.
    let mut pronto = bien.clone();
    pronto.pop();
    pronto.push(c::fadd(0, c::r(12), c::r(15), false, ctl(6, 7, 0)));
    pronto.push(c::exit(ctl(5, 7, 0)));
    assert_eq!(juzgar_drenado(&pronto, &ctx).unwrap_err().regla, Regla::R1DatoAntesDeLlegar);
    // ** Las fuentes las toma la unidad de texturas cuando las toma: pisar
    // el nivel o una coordenada sin esperar NADA es R4; esperando la
    // barrera del resultado (las fuentes ya se leyeron) o la de LECTURA
    // del TEX, no.
    let pisa = |tex: (u64, u64), mascara: u64| -> std::vec::Vec<(u64, u64)> { antes.iter().copied().chain([tex, c::mov(9, Fuente::Imm(1), ctl(1, 7, mascara)), c::exit(ctl(5, 7, 1))]).collect() };
    assert_eq!(juzgar_drenado(&pisa(tex3d, 0), &ctx).unwrap_err().regla, Regla::R4FuentePisada, "la v del TEX, pisada al salir");
    assert!(juzgar_drenado(&pisa(tex3d, 1), &ctx).is_ok(), "tras su resultado");
    let con_lectura = c::tex_forma(12, 8, 4, DimTex::D3, NivelTex::De, ctl(2, 0, 0) & !(7 << 8) | 1 << 8);
    assert!(juzgar_drenado(&pisa(con_lectura, 2), &ctx).is_ok(), "tras su barrera de lectura");
    assert_eq!(juzgar_drenado(&pisa(con_lectura, 0), &ctx).unwrap_err().regla, Regla::R4FuentePisada);
    // Lo que no se sabe es R0: un .LZ fuera del 2D, un .SCR fuera del 2D,
    // un .CL en un TEX, unas coordenadas desalineadas.
    let (lo, hi) = c::tex_forma(12, 8, 4, DimTex::D2, NivelTex::Cero, 0);
    let con_dim = |d: u64| (lo & !(0xF << 60) | 1 << 59 | d << 61, hi);
    assert!(!conoce(con_dim(2).0, con_dim(2).1), ".LZ en 3D");
    let (lo3, hi3) = c::tex_forma(12, 8, 4, DimTex::D3, NivelTex::De, 0);
    assert!(!conoce(lo3 | 1 << 60, hi3), ".SCR en 3D");
    assert!(!conoce(lo3, hi3 | 1 << 13), ".CL en un TEX");
    let (lo9, hi9) = c::tex_forma(12, 9, 4, DimTex::D3, NivelTex::De, 0);
    assert!(!conoce(lo9, hi9), "tres coordenadas desde R9");
    let (lob, hib) = c::tex_forma(12, 8, 5, DimTex::D3, NivelTex::De, 0);
    assert!(!conoce(lob, hib), "el par (asa, nivel) desde R5");
    let (tl, th) = c::tld(12, 8, 4, DimTex::Array2D, NivelTex::De, 0);
    assert!(conoce(tl, th) && !conoce(tl, th & !(1 << 13)), "un TLD con .LL lleva su .CL");
    // R7: el TLD con el asa del kernel, SI; con otra, NO.
    let cuerpo = [c::tld(12, 8, 4, DimTex::Array2D, NivelTex::De, 1), c::exit(1)];
    assert_eq!(juzgar_cuerpo_con_asas(&cuerpo, 16, 1 << 4), Ok(()));
    assert_eq!(juzgar_cuerpo_con_asas(&cuerpo, 16, 1 << 6).unwrap_err().regla, Regla::R7CuerpoAjeno);
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo, 16).unwrap_err().regla, Regla::R7CuerpoAjeno, "sin asas, ni TEX ni TLD");
}

/// ** E8f (DL18): el LDC es una DESACOPLADA -- quien lee lo que carga
/// espera su barrera (R1) -- y R7 lo deja solo del banco que ato el
/// kernel, con el indice sujeto justo antes y sin un salto que lo
/// esquive. Cada cerrojo, con su NO.
#[test]
fn e8f_el_ldc_de_la_app_y_sus_tres_cerrojos() {
    use bmo_sm86::codifica::{self as c, Fuente};
    let ctl = |espera: u64, wbar: u64, mascara: u64| espera | 1 << 4 | wbar << 5 | 7 << 8 | mascara << 11;
    let fin = c::exit(ctl(5, 7, 0));
    // R1: el dato del LDC, leido sin esperar su barrera.
    let carga = c::ldc(0, 3, RZ, 0x10, false, ctl(2, 0, 0));
    let ctx = Contexto { registros: 8, sph: None };
    let bien = [carga, c::fadd(1, c::r(0), c::r(0), false, ctl(6, 7, 1)), fin];
    assert!(juzgar_drenado(&bien, &ctx).is_ok(), "{:?}", juzgar_drenado(&bien, &ctx));
    let mal = [carga, c::fadd(1, c::r(0), c::r(0), false, ctl(6, 7, 0)), fin];
    assert_eq!(juzgar_drenado(&mal, &ctx).unwrap_err().regla, Regla::R1DatoAntesDeLlegar);
    // R7: el banco 3, de 128 bytes (8 filas).
    let banco = Some(BancoDeApp { numero: 3, bytes: 128 });
    let p = Permisos { asas: 0, banco };
    let sujeta = |tope: u32| c::imnmx(0, 1, Fuente::Imm(tope), false, false, 1);
    let ldc = |b: u8, ra: u8, desp: u16| c::ldc(2, b, ra, desp, true, 1);
    let ldc2 = c::ldc(4, 3, 0, 0x8, true, 1);
    // La fila del indice, sus dos mitades: tope 0x70 + 8 + 8 = 128. SI.
    assert_eq!(juzgar_cuerpo_con(&[sujeta(0x70), ldc(3, 0, 0), ldc2, fin], 8, p), Ok(()));
    // Sin indice: c[3][0x78] y ocho bytes, SI; c[3][0x7c] y ocho, NO.
    assert_eq!(juzgar_cuerpo_con(&[ldc(3, RZ, 0x78), fin], 8, p), Ok(()));
    assert_eq!(juzgar_cuerpo_con(&[ldc(3, RZ, 0x7c), fin], 8, p).unwrap_err().regla, Regla::R7CuerpoAjeno);
    let no = |codigo: &[(u64, u64)], p: Permisos| juzgar_cuerpo_con(codigo, 8, p).unwrap_err();
    // Sin banco atado (la de siempre), o con permisos sin banco: NO.
    assert_eq!(juzgar_cuerpo_de_app(&[sujeta(0x70), ldc(3, 0, 0), fin], 8).unwrap_err().instruccion, 1);
    assert_eq!(no(&[sujeta(0x70), ldc(3, 0, 0), fin], Permisos::default()).instruccion, 1);
    // Otro banco.
    assert_eq!(no(&[sujeta(0x70), ldc(0, 0, 0), fin], p).que, 0);
    // El indice sin sujetar (un MOV), o sujeto con un tope que se pasa.
    assert_eq!(no(&[c::mov(0, c::r(1), 1), ldc(3, 0, 0), fin], p).instruccion, 0);
    assert_eq!(juzgar_cuerpo_con(&[sujeta(0x78), ldc(3, 0, 0), fin], 8, p), Ok(()), "0x78 + 8 = 128: cabe justo");
    assert_eq!(no(&[sujeta(0x7c), ldc(3, 0, 0), fin], p).instruccion, 1);
    assert_eq!(no(&[sujeta(0x78), ldc(3, 0, 0), ldc2, fin], p).instruccion, 2);
    // Sujeto, pero algo lo pisa despues (el IMNMX ya no es el ultimo).
    assert_eq!(no(&[sujeta(0x70), c::iadd3(0, 0, Fuente::Imm(64), 1), ldc(3, 0, 0), fin], p).instruccion, 1);
    // El MAYOR, o con signo (un negativo pasaria por pequenyo), NO.
    assert_eq!(no(&[c::imnmx(0, 1, Fuente::Imm(0x70), true, false, 1), ldc(3, 0, 0), fin], p).instruccion, 0);
    assert_eq!(no(&[c::imnmx(0, 1, Fuente::Imm(0x70), false, true, 1), ldc(3, 0, 0), fin], p).instruccion, 0);
    // Un salto que cae en el LDC, saltandose el IMNMX: NO, aunque venga
    // de detras (un bucle).
    let salto = |desde: i64, hasta: i64| c::bra(7, 16 * (hasta - desde - 1), 1);
    assert_eq!(no(&[c::mov(0, c::r(1), 1), sujeta(0x70), ldc(3, 0, 0), salto(3, 2), fin], p).instruccion, 3);
    assert_eq!(no(&[salto(0, 2), sujeta(0x70), ldc(3, 0, 0), fin], p).instruccion, 0);
    // Un salto al IMNMX mismo, SI: lo corre.
    assert_eq!(juzgar_cuerpo_con(&[sujeta(0x70), ldc(3, 0, 0), salto(2, 0), fin], 8, p), Ok(()));
}
