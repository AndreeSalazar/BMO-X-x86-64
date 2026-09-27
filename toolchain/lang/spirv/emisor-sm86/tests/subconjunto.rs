//! El banco de E1 (PLAN_LA_LENGUA_DE_LA_3060): el subconjunto de SPIR-V que
//! el emisor de SM86 promete traducir.
//!
//! 1. los de verdad CABEN con su etapa: `cubo.vert.spv` como `Vertex`,
//!    `cubo.frag.spv` como `Fragment` (los que viajan en `cubo.bsf`) y los de
//!    computo del banco del lector como `GLCompute`;
//! 2. con la etapa equivocada, NO, y el motivo es `WrongStage`;
//! 3. una fila por regla: el MISMO modulo de verdad con UNA palabra cambiada
//!    (o una instruccion puesta), y el NO dice ESA regla y no otra.

use bmo_spirv_front::table::op;
use bmo_spirv_front::{read, Family, Reason as Spirv};
use bmo_spirv_sm86::{check, Fit, Reason, Refusal, Stage};

const VERT: &[u8] = include_bytes!("../../../../../platform/drivers/gpu/ga10x/sombreadores/cubo.vert.spv");
const FRAG: &[u8] = include_bytes!("../../../../../platform/drivers/gpu/ga10x/sombreadores/cubo.frag.spv");
const COMPUTO: [(&str, &[u8]); 7] = [
    ("suma", include_bytes!("../../pruebas/suma.spv")),
    ("saxpy", include_bytes!("../../pruebas/saxpy.spv")),
    ("mandelbrot", include_bytes!("../../pruebas/mandelbrot.spv")),
    ("colores", include_bytes!("../../pruebas/colores.spv")),
    ("collatz", include_bytes!("../../pruebas/collatz.spv")),
    ("bucle", include_bytes!("../../pruebas/bucle.spv")),
    ("division", include_bytes!("../../pruebas/division.spv")),
];
const COLORES: &[u8] = include_bytes!("../../pruebas/colores.spv");
const SUMA: &[u8] = include_bytes!("../../pruebas/suma.spv");
const TRASCENDENTES: &[u8] = include_bytes!("../../pruebas/trascendentes.spv");

fn juicio(bytes: &[u8], stage: Stage) -> Result<Fit, Refusal> {
    let mut ids = vec![0u32; 4096];
    let m = read(bytes, &mut ids).unwrap_or_else(|f| panic!("el lector no deberia negarlo: {}", f));
    check(&m, stage)
}

fn no(bytes: &[u8], stage: Stage) -> Refusal {
    juicio(bytes, stage).err().expect("se esperaba un NO")
}

fn palabras(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn bytes(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Donde empieza la primera instruccion `opcode` cuyos operandos EMPIEZAN por
/// `ops`. Asi una mutacion nombra lo que cambia, no un numero de palabra.
fn donde(w: &[u32], opcode: u16, ops: &[u32]) -> usize {
    let mut i = 5;
    while i < w.len() {
        let n = (w[i] >> 16) as usize;
        if w[i] as u16 == opcode && w[i + 1..].starts_with(ops) {
            return i;
        }
        i += n.max(1);
    }
    panic!("no esta {:?} {:?}", opcode, ops)
}

/// El modulo con la palabra `k` (contada desde la cabecera de la instruccion
/// encontrada) cambiada a `valor`.
fn cambia(original: &[u8], opcode: u16, ops: &[u32], k: usize, valor: u32) -> Vec<u8> {
    let mut w = palabras(original);
    let i = donde(&w, opcode, ops);
    w[i + k] = valor;
    bytes(&w)
}

fn ins(opcode: u16, ops: &[u32]) -> Vec<u32> {
    let mut v = vec![((ops.len() as u32 + 1) << 16) | opcode as u32];
    v.extend_from_slice(ops);
    v
}

// ---- 1. los de verdad ------------------------------------------------------

#[test]
fn el_cubo_cabe_con_su_etapa() {
    let v = juicio(VERT, Stage::Vertex).unwrap_or_else(|f| panic!("cubo.vert: {}", f));
    // La ranura 0 (los vertices), el generico 0 (el color), ni una FMA.
    assert_eq!((v.stage, v.slots, v.attributes, v.fused), (Stage::Vertex, 1, 1, 0));
    assert_eq!((v.verdict.entry_points, v.verdict.functions, v.verdict.blocks), (1, 1, 1));
    let p = juicio(FRAG, Stage::Fragment).unwrap_or_else(|f| panic!("cubo.frag: {}", f));
    // Sin buffers; lee el generico 0 y deja el color 0: los dos en la Location 0.
    assert_eq!((p.stage, p.slots, p.attributes, p.fused), (Stage::Fragment, 0, 1, 0));
}

#[test]
fn el_computo_del_lector_cabe() {
    for (nombre, spv) in COMPUTO {
        let v = juicio(spv, Stage::GLCompute).unwrap_or_else(|f| panic!("{}: {}", nombre, f));
        assert!(v.slots != 0, "{}: toca al menos un buffer", nombre);
        assert_eq!((v.attributes, v.fused), (0, 0), "{}", nombre);
    }
}

// ---- 2. la etapa equivocada ------------------------------------------------

#[test]
fn con_la_etapa_equivocada_no() {
    let f = no(VERT, Stage::Fragment);
    assert_eq!(f.reason, Reason::Spirv(Spirv::WrongStage { expected: 4, found: 0 }));
    assert_eq!(
        format!("{}", f),
        format!("palabra {}: el punto de entrada no es de la etapa que se pidio (se pidio Fragment, es Vertex)", f.word)
    );
    assert_eq!(no(FRAG, Stage::Vertex).reason, Reason::Spirv(Spirv::WrongStage { expected: 0, found: 4 }));
    assert_eq!(no(VERT, Stage::GLCompute).reason, Reason::Spirv(Spirv::WrongStage { expected: 5, found: 0 }));
    assert_eq!(no(SUMA, Stage::Vertex).reason, Reason::Spirv(Spirv::WrongStage { expected: 0, found: 5 }));
}

// ---- 3. una fila por regla -------------------------------------------------

#[test]
fn una_instruccion_fuera_del_nucleo_cae_con_su_nombre() {
    // El `OpReturn` del de pixel cambiado por `OpKill` (una palabra los dos).
    let mut w = palabras(FRAG);
    let i = donde(&w, op::OpReturn, &[]);
    w[i] = (1 << 16) | op::OpKill as u32;
    let f = no(&bytes(&w), Stage::Fragment);
    assert_eq!(f.reason, Reason::Spirv(Spirv::UnsupportedFamily { family: Family::ControlFlow, opcode: op::OpKill }));
    assert!(format!("{}", f).ends_with("(OpKill)"), "{}", f);
}

#[test]
fn una_clase_de_almacenamiento_fuera() {
    // `gl_PerVertex` en `Workgroup` en vez de `Output`.
    let malo = cambia(VERT, op::OpTypePointer, &[12, 3], 2, 4);
    assert_eq!(no(&malo, Stage::Vertex).reason, Reason::Spirv(Spirv::UnsupportedStorageClass { class: 4 }));
}

#[test]
fn un_buffer_fuera_de_las_ranuras() {
    // `DescriptorSet 1`: la tabla de SM86_V1 no tiene conjuntos.
    let malo = cambia(VERT, op::OpDecorate, &[20, 34], 3, 1);
    let f = no(&malo, Stage::Vertex);
    assert_eq!(f.reason, Reason::BufferOffSlot { set: 1, binding: 0 });
    assert_eq!(format!("{}", f), format!("palabra {}: SM86_V1: buffer fuera de las ranuras (DescriptorSet 0, Binding 0 a 15) (set 1, binding 0)", f.word));
    // `Binding 16`: la tabla es de 16.
    let malo = cambia(VERT, op::OpDecorate, &[20, 33], 3, 16);
    assert_eq!(no(&malo, Stage::Vertex).reason, Reason::BufferOffSlot { set: 0, binding: 16 });
    // Y el 15 si cabe: la ranura 15.
    let bueno = cambia(VERT, op::OpDecorate, &[20, 33], 3, 15);
    assert_eq!(juicio(&bueno, Stage::Vertex).unwrap().slots, 1 << 15);
}

#[test]
fn un_builtin_que_el_abi_no_lleva() {
    // `InstanceIndex` en vez de `VertexIndex`: la etapa lo tiene, SM86_V1 no.
    let malo = cambia(VERT, op::OpDecorate, &[22, 11], 3, 43);
    assert_eq!(no(&malo, Stage::Vertex).reason, Reason::BuiltInOffAbi { builtin: 43 });
    // El miembro de `gl_PerVertex` que se toca es `PointSize`, no `Position`.
    let mut w = palabras(VERT);
    let a = donde(&w, op::OpMemberDecorate, &[11, 0, 11]);
    let b = donde(&w, op::OpMemberDecorate, &[11, 1, 11]);
    (w[a + 4], w[b + 4]) = (1, 0);
    assert_eq!(no(&bytes(&w), Stage::Vertex).reason, Reason::BuiltInOffAbi { builtin: 1 });
    // `FragCoord` en el de pixel, en vez del generico 0.
    let malo = cambia(FRAG, op::OpDecorate, &[11, 30], 2, 11);
    let malo = cambia(&malo, op::OpDecorate, &[11, 11], 3, 15);
    assert_eq!(no(&malo, Stage::Fragment).reason, Reason::BuiltInOffAbi { builtin: 15 });
    // `NumWorkgroups` en el de computo, en vez de `GlobalInvocationId`.
    let malo = cambia(SUMA, op::OpDecorate, &[11, 11], 3, 24);
    assert_eq!(no(&malo, Stage::GLCompute).reason, Reason::BuiltInOffAbi { builtin: 24 });
}

#[test]
fn una_entrada_de_vertice_por_location() {
    // `gl_VertexIndex` pasa a ser `layout(location = 0) in int`.
    let malo = cambia(VERT, op::OpDecorate, &[22, 11], 2, 30);
    let malo = cambia(&malo, op::OpDecorate, &[22, 30], 3, 0);
    assert_eq!(no(&malo, Stage::Vertex).reason, Reason::VertexInputByLocation { location: 0 });
}

#[test]
fn una_location_que_no_es_la_cero() {
    let malo = cambia(VERT, op::OpDecorate, &[29, 30], 3, 1);
    assert_eq!(no(&malo, Stage::Vertex).reason, Reason::LocationOffAbi { location: 1 });
    let malo = cambia(FRAG, op::OpDecorate, &[9, 30], 3, 1);
    assert_eq!(no(&malo, Stage::Fragment).reason, Reason::LocationOffAbi { location: 1 });
}

#[test]
fn un_atributo_que_no_es_flotante() {
    // `OpTypeFloat %6 32` (3 palabras) pasa a `OpTypeBool %6` + `OpNop`: el
    // color es un `bvec4`, que el juez neutro deja y el ABI no mueve.
    let mut w = palabras(FRAG);
    let i = donde(&w, op::OpTypeFloat, &[6, 32]);
    w[i] = (2 << 16) | op::OpTypeBool as u32;
    w[i + 2] = (1 << 16) | op::OpNop as u32;
    assert_eq!(no(&bytes(&w), Stage::Fragment).reason, Reason::AttributeNotFloat { id: 9 });
}

#[test]
fn las_trascendentes_fuera_y_la_fma_contada() {
    // El de verdad: su primera trascendente es `Sin` (13).
    let f = no(TRASCENDENTES, Stage::GLCompute);
    assert_eq!(f.reason, Reason::Transcendental { number: 13 });
    assert!(format!("{}", f).ends_with("(Sin)"), "{}", f);
    // Y en `colores`, el `FAbs` (4) pasado a `Exp` (27).
    let malo = cambia(COLORES, op::OpExtInst, &[6, 44, 1], 4, 27);
    assert_eq!(no(&malo, Stage::GLCompute).reason, Reason::Transcendental { number: 27 });
    // `FMix` (46, tres flotantes) pasado a `Fma` (50): cabe, y se CUENTA.
    let fma = cambia(COLORES, op::OpExtInst, &[46, 61, 1], 4, 50);
    assert_eq!(juicio(&fma, Stage::GLCompute).unwrap().fused, 1);
    let fma = cambia(&fma, op::OpExtInst, &[46, 68, 1], 4, 50);
    assert_eq!(juicio(&fma, Stage::GLCompute).unwrap().fused, 2);
}

#[test]
fn dos_puntos_de_entrada() {
    // Un segundo `OpEntryPoint Vertex %4 "otro"` justo detras del primero.
    let mut w = palabras(VERT);
    let i = donde(&w, op::OpEntryPoint, &[0, 4]);
    let n = (w[i] >> 16) as usize;
    let otro = u32::from_le_bytes(*b"otro");
    let extra = ins(op::OpEntryPoint, &[0, 4, otro, 0, 13, 22, 29]);
    w.splice(i + n..i + n, extra);
    let f = no(&bytes(&w), Stage::Vertex);
    assert_eq!(f.reason, Reason::SeveralEntryPoints { count: 2 });
    assert_eq!(f.word, i + n);
}
