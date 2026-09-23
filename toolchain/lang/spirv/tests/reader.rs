//! El banco del LECTOR (S1 de PLAN_EL_SOMBREADOR).
//!
//! Tres clases de fila:
//!
//! 1. los cinco `.spv` de `pruebas/`, fabricados por `glslc` del SDK a partir
//!    del GLSL que tienen al lado: se leen ENTEROS, y lo que declaran es lo que
//!    el GLSL dice;
//! 2. una fila por motivo: cada forma de romper la cabecera y el flujo tiene
//!    su NO con su nombre;
//! 3. bytes hostiles a lo bruto: cortar en cada palabra y voltear cada byte.
//!    Lo que se comprueba ahi es que NUNCA hay panico -- el lector es la
//!    primera cosa que toca bytes de un tercero.

use bmo_spirv_front::{op_info, read, Family, Error, Reason, Section, MAGIC};

const SUMA: &[u8] = include_bytes!("../pruebas/suma.spv");
const SAXPY: &[u8] = include_bytes!("../pruebas/saxpy.spv");
const MANDELBROT: &[u8] = include_bytes!("../pruebas/mandelbrot.spv");
const COLORES: &[u8] = include_bytes!("../pruebas/colores.spv");
const FUERA: &[u8] = include_bytes!("../pruebas/fuera.spv");

const GL_COMPUTE: u32 = 5;
const CAP_SHADER: u32 = 1;

fn table() -> Vec<u32> {
    vec![0; 4096]
}

/// Un modulo a mano: la cabecera con `bound` y luego las palabras dadas.
fn modulo(bound: u32, cuerpo: &[u32]) -> Vec<u8> {
    let mut w = vec![MAGIC, 0x0001_0000, 0, bound, 0];
    w.extend_from_slice(cuerpo);
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn ins(opcode: u16, ops: &[u32]) -> Vec<u32> {
    let mut v = vec![((ops.len() as u32 + 1) << 16) | opcode as u32];
    v.extend_from_slice(ops);
    v
}

/// Una cadena de SPIR-V: bytes, su cero, rellena a palabras.
fn cadena(s: &str) -> Vec<u32> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// El minimo valido: `OpCapability Shader` + `OpMemoryModel Logical GLSL450`.
fn min_words() -> Vec<u32> {
    let mut v = ins(17, &[CAP_SHADER]);
    v.extend(ins(14, &[0, 1]));
    v
}

fn no(bytes: &[u8]) -> Error {
    match read(bytes, &mut table()) {
        Ok(_) => panic!("se esperaba un NO"),
        Err(f) => f,
    }
}

// ---- 1. los sombreadores de verdad -----------------------------------------

fn comprobar(bytes: &[u8], local_size: [u32; 3], functions: usize) {
    let mut ids = table();
    let m = read(bytes, &mut ids).unwrap_or_else(|f| panic!("{}", f));
    assert_eq!(m.header.major, 1);
    assert!(m.capabilities().contains(&CAP_SHADER));
    assert_eq!(m.memory_model, (0, 1), "Logical + GLSL450");
    assert!(m.glsl450().is_some(), "glslc siempre importa GLSL.std.450");
    assert_eq!(m.entry_points().len(), 1);
    let e = m.entry_points()[0];
    assert_eq!(e.model, GL_COMPUTE);
    assert_eq!(e.name, b"main");
    assert_eq!(e.local_size, Some(local_size));
    assert_eq!(m.functions, functions);
    // El punto de entrada es una OpFunction.
    assert_eq!(m.def(e.id).map(|i| i.opcode), Some(54));
    // El recorrido ve las mismas instrucciones que la lectura, y termina justo
    // en el final del fichero.
    let mut n = 0;
    let mut fin = 5;
    for i in m.instructions() {
        assert_eq!(i.offset, fin);
        fin += i.words as usize;
        n += 1;
    }
    assert_eq!(n, m.instruction_count);
    assert_eq!(fin * 4, bytes.len());
    // Todo id que una instruccion define apunta de vuelta a ella.
    for i in m.instructions() {
        if let Some(id) = i.result_id() {
            assert_eq!(m.def(id).map(|d| d.offset), Some(i.offset));
        }
    }
}

#[test]
fn suma_se_lee_entera() {
    comprobar(SUMA, [64, 1, 1], 1);
}

#[test]
fn saxpy_se_lee_entera() {
    comprobar(SAXPY, [64, 1, 1], 1);
}

#[test]
fn mandelbrot_se_lee_entero() {
    comprobar(MANDELBROT, [8, 8, 1], 1);
}

#[test]
fn colores_se_lee_entero_con_su_funcion_auxiliar() {
    comprobar(COLORES, [64, 1, 1], 2);
}

#[test]
fn fuera_se_lee_entero_aunque_use_lo_que_s2_negara() {
    // El lector recorre imagenes, atomicos y barreras: negarlos es del juez.
    comprobar(FUERA, [16, 16, 1], 1);
    let mut ids = table();
    let m = read(FUERA, &mut ids).unwrap();
    let familias: Vec<Family> = m.instructions().filter_map(|i| op_info(i.opcode)).map(|f| f.family).collect();
    for f in [Family::Image, Family::Atomic, Family::Barrier] {
        assert!(familias.contains(&f), "{:?} tiene que aparecer en fuera.spv", f);
    }
}

#[test]
fn los_nombres_de_depuracion_se_leen() {
    let mut ids = table();
    let m = read(SAXPY, &mut ids).unwrap();
    let nombres: Vec<&[u8]> = m.instructions().filter(|i| i.opcode == 5).filter_map(|i| i.string(2)).collect();
    assert!(nombres.contains(&&b"main"[..]));
    let miembros: Vec<&[u8]> = m.instructions().filter(|i| i.opcode == 6).filter_map(|i| i.string(3)).collect();
    assert!(miembros.contains(&&b"alfa"[..]), "{:?}", miembros);
}

#[test]
fn la_tabla_esta_ordenada_y_sin_repetidos() {
    let t = bmo_spirv_front::table::TABLE;
    for par in t.windows(2) {
        assert!(par[0].opcode < par[1].opcode, "{} / {}", par[0].name, par[1].name);
    }
    assert_eq!(op_info(128).map(|f| f.name), Some("OpIAdd"));
    assert_eq!(op_info(54).map(|f| f.section), Some(Section::Function));
    assert!(op_info(9999).is_none());
}

// ---- 2. una fila por motivo ------------------------------------------------

#[test]
fn corto() {
    assert_eq!(no(&[0; 16]).reason, Reason::Short { bytes: 16 });
}

#[test]
fn no_multiplo_de_4() {
    let mut b = modulo(10, &min_words());
    b.push(0);
    assert!(matches!(no(&b).reason, Reason::NotWordAligned { .. }));
}

#[test]
fn no_es_spirv() {
    let mut b = modulo(10, &min_words());
    b[0] = 0x7F;
    assert!(matches!(no(&b).reason, Reason::NotSpirv { .. }));
}

#[test]
fn al_reves() {
    let mut b = modulo(10, &min_words());
    b[..4].copy_from_slice(&MAGIC.to_be_bytes());
    assert_eq!(no(&b).reason, Reason::BigEndian);
}

#[test]
fn version() {
    for v in [0x0002_0000u32, 0x0001_0700, 0x0001_0001, 0x0101_0000] {
        let mut b = modulo(10, &min_words());
        b[4..8].copy_from_slice(&v.to_le_bytes());
        assert_eq!(no(&b).reason, Reason::Version { word: v }, "0x{:08x}", v);
    }
    // 1.6 si vale.
    let mut b = modulo(10, &min_words());
    b[4..8].copy_from_slice(&0x0001_0600u32.to_le_bytes());
    assert!(read(&b, &mut table()).is_ok());
}

#[test]
fn bound_cero() {
    assert_eq!(no(&modulo(0, &min_words())).reason, Reason::ZeroBound);
}

#[test]
fn esquema_no_cero() {
    let mut b = modulo(10, &min_words());
    b[16..20].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(no(&b).reason, Reason::NonZeroSchema { schema: 7 });
}

#[test]
fn tabla_de_ids_chica() {
    let b = modulo(100, &min_words());
    let f = read(&b, &mut [0u32; 10]).err().unwrap();
    assert_eq!(f.reason, Reason::IdTableTooSmall { bound: 100, capacity: 10 });
}

#[test]
fn instruccion_vacia() {
    let mut c = min_words();
    c.push(17); // medida 0, codigo 17
    let f = no(&modulo(10, &c));
    assert_eq!(f.reason, Reason::EmptyInstruction);
    // La cabecera (5) + OpCapability (2) + OpMemoryModel (3).
    assert_eq!(f.word, 5 + 5);
}

#[test]
fn se_sale_del_fichero() {
    let mut c = min_words();
    c.push((9 << 16) | 17);
    assert_eq!(no(&modulo(10, &c)).reason, Reason::PastEnd { words: 9, remaining: 1 });
}

#[test]
fn sin_fila() {
    let mut c = min_words();
    c.extend(ins(4999, &[]));
    assert_eq!(no(&modulo(10, &c)).reason, Reason::UnknownOpcode { opcode: 4999 });
}

#[test]
fn corta() {
    // OpMemoryModel necesita 3 palabras.
    let mut c = ins(17, &[CAP_SHADER]);
    c.extend(ins(14, &[0]));
    assert_eq!(no(&modulo(10, &c)).reason, Reason::TooShort { opcode: 14, words: 2, min_words: 3 });
}

#[test]
fn id_fuera_por_cero_y_por_bound() {
    for id in [0u32, 10, 11] {
        let mut c = min_words();
        c.extend(ins(19, &[id])); // OpTypeVoid %id
        assert_eq!(no(&modulo(10, &c)).reason, Reason::IdOutOfBound { id }, "id {}", id);
    }
}

#[test]
fn id_repetido() {
    let mut c = min_words();
    c.extend(ins(19, &[3]));
    c.extend(ins(20, &[3]));
    assert_eq!(no(&modulo(10, &c)).reason, Reason::DuplicateId { id: 3 });
}

#[test]
fn cadena_sin_cero() {
    let mut c = min_words();
    // OpName %1 "abcd" SIN el cero: cuatro letras llenan la palabra.
    c.extend(ins(5, &[1, u32::from_le_bytes(*b"abcd")]));
    assert_eq!(no(&modulo(10, &c)).reason, Reason::UnterminatedString { opcode: 5 });
}

#[test]
fn modelo_ninguno_o_dos() {
    let sin = ins(17, &[CAP_SHADER]);
    assert_eq!(no(&modulo(10, &sin)).reason, Reason::MemoryModelCount { count: 0 });
    let mut dos = min_words();
    dos.extend(ins(14, &[0, 1]));
    assert_eq!(no(&modulo(10, &dos)).reason, Reason::MemoryModelCount { count: 2 });
}

#[test]
fn fuera_de_orden() {
    // Una capacidad DESPUES del modelo de memoria.
    let mut c = min_words();
    c.extend(ins(17, &[CAP_SHADER]));
    assert_eq!(no(&modulo(10, &c)).reason, Reason::OutOfOrder { opcode: 17 });
}

#[test]
fn funciones_mal_anidadas() {
    let mut c = min_words();
    c.extend(ins(19, &[1])); // void
    c.extend(ins(33, &[2, 1])); // fn() -> void
    let func = ins(54, &[1, 3, 0, 2]);
    let mut dos = c.clone();
    dos.extend(func.clone());
    dos.extend(ins(54, &[1, 4, 0, 2]));
    assert_eq!(no(&modulo(10, &dos)).reason, Reason::NestedFunction);

    let mut fin = c.clone();
    fin.extend(ins(56, &[]));
    assert_eq!(no(&modulo(10, &fin)).reason, Reason::StrayFunctionEnd);

    let mut abierta = c.clone();
    abierta.extend(func);
    abierta.extend(ins(248, &[5])); // OpLabel
    assert_eq!(no(&modulo(10, &abierta)).reason, Reason::UnterminatedFunction);
}

#[test]
fn cuerpo_fuera_de_funcion() {
    let mut c = min_words();
    c.extend(ins(253, &[])); // OpReturn
    assert_eq!(no(&modulo(10, &c)).reason, Reason::BodyOutsideFunction { opcode: 253 });
}

#[test]
fn modo_sin_entrada() {
    let mut c = min_words();
    c.extend(ins(16, &[4, 17, 1, 1, 1])); // LocalSize para %4, que no es entrada
    assert_eq!(no(&modulo(10, &c)).reason, Reason::ModeWithoutEntryPoint { id: 4 });
}

#[test]
fn local_size_corto() {
    let mut c = min_words();
    let mut e = vec![GL_COMPUTE, 4];
    e.extend(cadena("main"));
    c.extend(ins(15, &e));
    c.extend(ins(16, &[4, 17, 8]));
    assert!(matches!(no(&modulo(10, &c)).reason, Reason::TooShort { opcode: 16, .. }));
}

#[test]
fn demasiadas_capacidades_es_un_techo_dicho() {
    let mut c = Vec::new();
    for _ in 0..33 {
        c.extend(ins(17, &[CAP_SHADER]));
    }
    c.extend(ins(14, &[0, 1]));
    assert!(matches!(no(&modulo(10, &c)).reason, Reason::TooMany { what: "capacidades", .. }));
}

#[test]
fn el_fallo_se_explica_con_sus_numeros() {
    let mut c = min_words();
    c.extend(ins(4999, &[]));
    let texto = format!("{}", no(&modulo(10, &c)));
    assert_eq!(texto, "palabra 10: instruccion que este lector no conoce (codigo 4999)");
}

// ---- 3. bytes hostiles -----------------------------------------------------

#[test]
fn cortar_en_cada_palabra_nunca_es_panico() {
    for bytes in [SUMA, SAXPY, MANDELBROT, COLORES, FUERA] {
        let entero = read(bytes, &mut table()).unwrap().functions;
        for fin in (0..bytes.len()).step_by(4) {
            // Cortar ENTRE funciones (o antes de la primera) da un modulo legal
            // mas chico -- `colores` tiene dos funciones y se puede cortar tras
            // la primera. Lo que no puede pasar es que un corte que parte una
            // instruccion o deja una funcion abierta se lea: si se lee, el
            // recorrido acaba justo en el corte y no tiene mas funciones.
            let corte = &bytes[..fin];
            if let Ok(m) = read(corte, &mut table()) {
                let fin_rec: usize = m.instructions().map(|i| i.words as usize).sum::<usize>() + 5;
                assert_eq!(fin_rec * 4, fin, "cortado en {}", fin);
                assert!(m.functions <= entero);
            }
        }
    }
}

#[test]
fn voltear_cada_byte_nunca_es_panico() {
    for bytes in [SUMA, COLORES, FUERA] {
        let mut b = bytes.to_vec();
        for i in 0..b.len() {
            b[i] ^= 0xFF;
            let _ = read(&b, &mut table());
            b[i] ^= 0xFF;
        }
    }
}

#[test]
fn ninguna_palabra_imposible_en_la_cabecera_de_medida() {
    // Todas las medidas posibles de la primera instruccion, con el resto intacto.
    let mut b = SUMA.to_vec();
    let w = u32::from_le_bytes([b[20], b[21], b[22], b[23]]);
    for medida in 0..=u16::MAX as u32 {
        let nueva = (medida << 16) | (w & 0xFFFF);
        b[20..24].copy_from_slice(&nueva.to_le_bytes());
        let _ = read(&b, &mut table());
    }
}
