//! El banco del JUEZ (S2 de PLAN_EL_SOMBREADOR).
//!
//! 1. los cuatro sombreadores del subconjunto CABEN, y `outside` cae nombrando
//!    la familia;
//! 2. una fila por motivo, sobre un modulo minimo armado a mano al que se le
//!    rompe UNA cosa;
//! 3. el juez tampoco entra en panico con bytes volteados que el lector acepta.

use bmo_spirv_front::table::op;
use bmo_spirv_front::{census, validate, read, Error, Family, Reason, Verdict, MAGIC};

const SUMA: &[u8] = include_bytes!("../pruebas/suma.spv");
const SAXPY: &[u8] = include_bytes!("../pruebas/saxpy.spv");
const MANDELBROT: &[u8] = include_bytes!("../pruebas/mandelbrot.spv");
const COLORES: &[u8] = include_bytes!("../pruebas/colores.spv");
const FUERA: &[u8] = include_bytes!("../pruebas/fuera.spv");

fn juicio_de(bytes: &[u8]) -> Result<Verdict, Error> {
    let mut ids = vec![0u32; 4096];
    let m = read(bytes, &mut ids).unwrap_or_else(|f| panic!("el lector no deberia negarlo: {}", f));
    validate(&m)
}

// ---- 1. los de verdad ------------------------------------------------------

#[test]
fn los_cuatro_del_subconjunto_caben() {
    for (name, bytes, functions) in
        [("suma", SUMA, 1), ("saxpy", SAXPY, 1), ("mandelbrot", MANDELBROT, 1), ("colores", COLORES, 2)]
    {
        let v = juicio_de(bytes).unwrap_or_else(|f| panic!("{}: {}", name, f));
        assert_eq!(v.entry_points, 1, "{}", name);
        assert_eq!(v.functions, functions, "{}", name);
        assert!(v.blocks >= 1, "{}", name);
    }
}

#[test]
fn fuera_cae_en_lo_primero_que_aparece_la_memoria_de_grupo() {
    // ** El juez da el PRIMER no en el orden del fichero. En `outside.spv` el
    // puntero a `Workgroup` (la `shared uint group`) va antes que la imagen: es
    // lo que glslc escribe primero. Las demas las nombra el censo.
    let f = juicio_de(FUERA).err().expect("fuera.spv NO cabe");
    assert_eq!(f.reason, Reason::UnsupportedStorageClass { class: 4 });
    assert_eq!(
        format!("{}", f),
        format!(
            "palabra {}: clase de almacenamiento fuera del subconjunto (Workgroup: memoria de grupo, pide hilos)",
            f.word
        )
    );
}

#[test]
fn una_imagen_cae_nombrando_su_familia() {
    let f = reason(|p| p.tipos.extend(ins(op::OpTypeImage, &[20, 7, 1, 0, 0, 0, 2, 4])));
    assert_eq!(f, Reason::UnsupportedFamily { family: Family::Image, opcode: op::OpTypeImage });
}

#[test]
fn el_censo_de_fuera_cuenta_las_tres_familias() {
    let mut ids = vec![0u32; 4096];
    let m = read(FUERA, &mut ids).unwrap();
    let c = census(&m);
    for fam in [Family::Image, Family::Atomic, Family::Barrier] {
        assert!(c.per_family[fam.index()] > 0, "{:?}", fam);
    }
    assert!(c.outside() >= 5);
    // Y los cuatro buenos no tienen nada fuera.
    for bytes in [SUMA, SAXPY, MANDELBROT, COLORES] {
        let mut ids = vec![0u32; 4096];
        assert_eq!(census(&read(bytes, &mut ids).unwrap()).outside(), 0);
    }
}

// ---- 2. el modulo minimo, y una fila por motivo ----------------------------

fn ins(opcode: u16, ops: &[u32]) -> Vec<u32> {
    let mut v = vec![((ops.len() as u32 + 1) << 16) | opcode as u32];
    v.extend_from_slice(ops);
    v
}

fn cad(s: &str) -> Vec<u32> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn con(a: &[u32], b: Vec<u32>) -> Vec<u32> {
    let mut v = a.to_vec();
    v.extend(b);
    v
}

/// Los trozos de un modulo de computo minimo. Ids fijos:
///
/// ```text
///   %1 GLSL.std.450   %2 void   %3 fn()->void   %4 main   %5 su bloque
///   %6 uint   %7 float   %8 bool   %9 vec2   %10 int
///   %11 = 1u   %12 = 1.0   %13 = true   %14 = vec2(1.0, 1.0)
/// ```
///
/// Cada prueba cambia UN trozo. Los ids que agrega empiezan en %20.
struct Partes {
    caps: Vec<u32>,
    exts: Vec<u32>,
    imports: Vec<u32>,
    model: Vec<u32>,
    entrada: Vec<u32>,
    modos: Vec<u32>,
    anot: Vec<u32>,
    tipos: Vec<u32>,
    func: Vec<u32>,
    cuerpo: Vec<u32>,
    fin: Vec<u32>,
    cola: Vec<u32>,
}

fn partes() -> Partes {
    let mut tipos = Vec::new();
    for t in [
        ins(op::OpTypeVoid, &[2]),
        ins(op::OpTypeFunction, &[3, 2]),
        ins(op::OpTypeInt, &[6, 32, 0]),
        ins(op::OpTypeFloat, &[7, 32]),
        ins(op::OpTypeBool, &[8]),
        ins(op::OpTypeVector, &[9, 7, 2]),
        ins(op::OpTypeInt, &[10, 32, 1]),
        ins(op::OpConstant, &[6, 11, 1]),
        ins(op::OpConstant, &[7, 12, 0x3F80_0000]),
        ins(op::OpConstantTrue, &[8, 13]),
        ins(op::OpConstantComposite, &[9, 14, 12, 12]),
    ] {
        tipos.extend(t);
    }
    Partes {
        caps: ins(op::OpCapability, &[1]),
        exts: vec![],
        imports: con(&[], ins(op::OpExtInstImport, &con(&[1], cad("GLSL.std.450")))),
        model: ins(op::OpMemoryModel, &[0, 1]),
        entrada: ins(op::OpEntryPoint, &con(&[5, 4], cad("main"))),
        modos: ins(op::OpExecutionMode, &[4, 17, 1, 1, 1]),
        anot: vec![],
        tipos,
        func: con(&ins(op::OpFunction, &[2, 4, 0, 3]), ins(op::OpLabel, &[5])),
        cuerpo: vec![],
        fin: con(&ins(op::OpReturn, &[]), ins(op::OpFunctionEnd, &[])),
        cola: vec![],
    }
}

fn armar(p: &Partes) -> Vec<u8> {
    let mut w = vec![MAGIC, 0x0001_0000, 0, 100, 0];
    for t in [&p.caps, &p.exts, &p.imports, &p.model, &p.entrada, &p.modos, &p.anot, &p.tipos, &p.func, &p.cuerpo, &p.fin, &p.cola] {
        w.extend_from_slice(t);
    }
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn juicio(cambia: impl FnOnce(&mut Partes)) -> Result<Verdict, Error> {
    let mut p = partes();
    cambia(&mut p);
    juicio_de(&armar(&p))
}

fn reason(cambia: impl FnOnce(&mut Partes)) -> Reason {
    juicio(cambia).err().expect("se esperaba un NO").reason
}

fn cuerpo(c: &[Vec<u32>]) -> impl FnOnce(&mut Partes) + '_ {
    move |p| {
        for i in c {
            p.cuerpo.extend(i);
        }
    }
}

#[test]
fn el_minimo_cabe() {
    let v = juicio(|_| {}).unwrap();
    assert_eq!((v.entry_points, v.functions, v.blocks), (1, 1, 1));
}

#[test]
fn un_cuerpo_con_de_todo_cabe() {
    // %20 = 1u + 1u; %21 = 1.0 * 1.0; %22 = %20 < 1u; %23 = select; %24 = vec2 * 1.0;
    // %25 = shuffle; %26 = extract; %27 = fclamp; %28 = dot
    juicio(cuerpo(&[
        ins(op::OpIAdd, &[6, 20, 11, 11]),
        ins(op::OpFMul, &[7, 21, 12, 12]),
        ins(op::OpULessThan, &[8, 22, 20, 11]),
        ins(op::OpSelect, &[7, 23, 22, 21, 12]),
        ins(op::OpVectorTimesScalar, &[9, 24, 14, 23]),
        ins(op::OpVectorShuffle, &[9, 25, 24, 14, 1, 2]),
        ins(op::OpCompositeExtract, &[7, 26, 25, 0]),
        ins(op::OpExtInst, &[7, 27, 1, 43, 26, 12, 21]),
        ins(op::OpDot, &[7, 28, 24, 25]),
        ins(op::OpConvertFToU, &[6, 29, 28]),
        ins(op::OpIAdd, &[6, 30, 29, 11]),
    ]))
    .unwrap_or_else(|f| panic!("{}", f));
}

#[test]
fn capacidad_fuera() {
    assert_eq!(reason(|p| p.caps.extend(ins(op::OpCapability, &[10]))), Reason::UnsupportedCapability { capability: 10 });
}

#[test]
fn extension_fuera_y_la_que_si_vale() {
    assert_eq!(reason(|p| p.exts = ins(op::OpExtension, &cad("SPV_KHR_de_mentira"))), Reason::UnsupportedExtension);
    assert!(juicio(|p| p.exts = ins(op::OpExtension, &cad("SPV_KHR_storage_buffer_storage_class"))).is_ok());
}

#[test]
fn importacion_fuera() {
    assert_eq!(
        reason(|p| p.imports.extend(ins(op::OpExtInstImport, &con(&[20], cad("OpenCL.std"))))),
        Reason::UnsupportedImport
    );
}

#[test]
fn modelo_fuera() {
    assert_eq!(reason(|p| p.model = ins(op::OpMemoryModel, &[0, 0])), Reason::UnsupportedMemoryModel { addressing: 0, memory: 0 });
}

#[test]
fn sin_entrada() {
    assert_eq!(
        reason(|p| {
            p.entrada.clear();
            p.modos.clear();
        }),
        Reason::NoEntryPoint
    );
}

#[test]
fn etapa_fuera() {
    assert_eq!(reason(|p| p.entrada = ins(op::OpEntryPoint, &con(&[0, 4], cad("main")))), Reason::UnsupportedStage { model: 0 });
}

#[test]
fn sin_local_size() {
    assert_eq!(reason(|p| p.modos.clear()), Reason::NoLocalSize);
}

#[test]
fn modo_fuera() {
    assert_eq!(reason(|p| p.modos.extend(ins(op::OpExecutionMode, &[4, 7]))), Reason::UnsupportedMode { mode: 7 });
}

#[test]
fn grupos_de_decoraciones_fuera() {
    let m = reason(|p| p.anot = ins(op::OpDecorationGroup, &[20]));
    assert!(matches!(m, Reason::UnsupportedInstruction { opcode: op::OpDecorationGroup, .. }));
}

#[test]
fn ext_inst_fuera_y_luego() {
    // Round (1) no esta; Sin (13) llega con S3b.
    assert_eq!(reason(cuerpo(&[ins(op::OpExtInst, &[7, 20, 1, 1, 12])])), Reason::UnsupportedGlsl { number: 1 });
    assert_eq!(reason(cuerpo(&[ins(op::OpExtInst, &[7, 20, 1, 13, 12])])), Reason::GlslLater { number: 13 });
}

#[test]
fn tipos_fuera() {
    assert!(matches!(reason(|p| p.tipos.extend(ins(op::OpTypeInt, &[20, 64, 0]))), Reason::UnsupportedType { .. }));
    assert!(matches!(reason(|p| p.tipos.extend(ins(op::OpTypeFloat, &[20, 64]))), Reason::UnsupportedType { .. }));
    assert!(matches!(reason(|p| p.tipos.extend(ins(op::OpTypeVector, &[20, 7, 5]))), Reason::UnsupportedType { .. }));
}

#[test]
fn clase_fuera_workgroup() {
    assert_eq!(reason(|p| p.tipos.extend(ins(op::OpTypePointer, &[20, 4, 6]))), Reason::UnsupportedStorageClass { class: 4 });
}

/// `%20` = puntero Input a uint, `%21` = la variable.
fn entrada_uint(p: &mut Partes) {
    p.tipos.extend(ins(op::OpTypePointer, &[20, 1, 6]));
    p.tipos.extend(ins(op::OpVariable, &[20, 21, 1]));
}

#[test]
fn entrada_sin_builtin_y_builtin_fuera() {
    assert_eq!(reason(entrada_uint), Reason::InputWithoutBuiltIn { id: 21 });
    assert_eq!(
        reason(|p| {
            entrada_uint(p);
            p.anot = ins(op::OpDecorate, &[21, 11, 0]); // BuiltIn Position
        }),
        Reason::UnsupportedBuiltIn { builtin: 0 }
    );
    assert!(juicio(|p| {
        entrada_uint(p);
        p.anot = ins(op::OpDecorate, &[21, 11, 29]); // LocalInvocationIndex
    })
    .is_ok());
}

#[test]
fn escritura_en_entrada() {
    assert_eq!(
        reason(|p| {
            entrada_uint(p);
            p.anot = ins(op::OpDecorate, &[21, 11, 29]);
            p.cuerpo = ins(op::OpStore, &[21, 11]);
        }),
        Reason::WriteToInput
    );
}

#[test]
fn buffer_sin_binding() {
    let buffer = |p: &mut Partes| {
        p.tipos.extend(ins(op::OpTypeStruct, &[20, 6]));
        p.tipos.extend(ins(op::OpTypePointer, &[21, 2, 20]));
        p.tipos.extend(ins(op::OpVariable, &[21, 22, 2]));
    };
    assert_eq!(reason(buffer), Reason::NoBinding { id: 22 });
    assert!(juicio(|p| {
        buffer(p);
        p.anot = con(&ins(op::OpDecorate, &[22, 33, 0]), ins(op::OpDecorate, &[22, 34, 0]));
    })
    .is_ok());
}

#[test]
fn no_es_tipo_no_es_valor_no_es_constante() {
    assert_eq!(reason(|p| p.tipos.extend(ins(op::OpTypeVector, &[20, 11, 2]))), Reason::NotAType { id: 11 });
    assert_eq!(reason(cuerpo(&[ins(op::OpIAdd, &[6, 20, 6, 11])])), Reason::NotAValue { id: 6 });
    assert_eq!(reason(|p| p.tipos.extend(ins(op::OpTypeArray, &[20, 6, 7]))), Reason::NotAConstant { id: 7 });
}

#[test]
fn no_es_etiqueta() {
    assert_eq!(
        reason(|p| p.fin = con(&ins(op::OpBranch, &[11]), ins(op::OpFunctionEnd, &[]))),
        Reason::NotALabel { id: 11 }
    );
}

#[test]
fn no_definido_y_uso_antes_de_definir() {
    assert_eq!(reason(cuerpo(&[ins(op::OpIAdd, &[6, 20, 90, 11])])), Reason::Undefined { id: 90 });
    assert_eq!(
        reason(cuerpo(&[ins(op::OpIAdd, &[6, 20, 21, 11]), ins(op::OpIAdd, &[6, 21, 11, 11])])),
        Reason::UsedBeforeDefined { id: 21 }
    );
}

#[test]
fn los_tipos_no_cuadran() {
    // Una suma entera con resultado flotante; una flotante con un entero.
    assert_eq!(reason(cuerpo(&[ins(op::OpIAdd, &[7, 20, 11, 11])])), Reason::TypeMismatch { opcode: op::OpIAdd });
    assert_eq!(reason(cuerpo(&[ins(op::OpFAdd, &[7, 20, 12, 11])])), Reason::TypeMismatch { opcode: op::OpFAdd });
}

#[test]
fn indice_fuera() {
    assert_eq!(
        reason(cuerpo(&[ins(op::OpCompositeExtract, &[7, 20, 14, 5])])),
        Reason::IndexOutOfRange { opcode: op::OpCompositeExtract }
    );
}

#[test]
fn sin_cuerpo() {
    // Una segunda funcion sin ningun bloque.
    assert_eq!(
        reason(|p| p.cola = con(&ins(op::OpFunction, &[2, 20, 0, 3]), ins(op::OpFunctionEnd, &[]))),
        Reason::NoBody
    );
}

#[test]
fn fuera_de_bloque_y_bloque_sin_terminar() {
    assert_eq!(
        reason(|p| p.fin = [ins(op::OpReturn, &[]), ins(op::OpIAdd, &[6, 20, 11, 11]), ins(op::OpFunctionEnd, &[])].concat()),
        Reason::OutsideBlock { opcode: op::OpIAdd }
    );
    assert_eq!(reason(cuerpo(&[ins(op::OpLabel, &[20])])), Reason::UnterminatedBlock);
    assert_eq!(reason(|p| p.fin = ins(op::OpFunctionEnd, &[])), Reason::UnterminatedBlock);
}

#[test]
fn phi_y_variable_fuera_de_sitio() {
    assert_eq!(
        reason(cuerpo(&[ins(op::OpIAdd, &[6, 20, 11, 11]), ins(op::OpPhi, &[6, 21, 11, 5])])),
        Reason::MisplacedPhi
    );
    assert_eq!(
        reason(|p| {
            p.tipos.extend(ins(op::OpTypePointer, &[20, 7, 6]));
            p.cuerpo = con(&ins(op::OpIAdd, &[6, 21, 11, 11]), ins(op::OpVariable, &[20, 22, 7]));
        }),
        Reason::MisplacedVariable
    );
}

#[test]
fn merge_fuera_de_sitio_y_salto_sin_estructura() {
    // SelectionMerge seguido de algo que no es su salto.
    assert_eq!(
        reason(|p| {
            p.cuerpo = con(&ins(op::OpSelectionMerge, &[5, 0]), ins(op::OpIAdd, &[6, 21, 11, 11]));
        }),
        Reason::MisplacedMerge
    );
    // Un if sin fusion: dos destinos distintos y ningun bucle.
    assert_eq!(
        reason(|p| {
            p.fin = [
                ins(op::OpBranchConditional, &[13, 20, 21]),
                ins(op::OpLabel, &[20]),
                ins(op::OpReturn, &[]),
                ins(op::OpLabel, &[21]),
                ins(op::OpReturn, &[]),
                ins(op::OpFunctionEnd, &[]),
            ]
            .concat();
        }),
        Reason::Unstructured
    );
    // El mismo if CON su fusion cabe.
    assert!(juicio(|p| {
        p.fin = [
            ins(op::OpSelectionMerge, &[21, 0]),
            ins(op::OpBranchConditional, &[13, 20, 21]),
            ins(op::OpLabel, &[20]),
            ins(op::OpBranch, &[21]),
            ins(op::OpLabel, &[21]),
            ins(op::OpReturn, &[]),
            ins(op::OpFunctionEnd, &[]),
        ]
        .concat();
    })
    .is_ok());
}

#[test]
fn entrada_no_cuadra() {
    // main recibe un parametro.
    assert_eq!(
        reason(|p| {
            p.tipos.extend(ins(op::OpTypeFunction, &[20, 2, 6]));
            p.func = [ins(op::OpFunction, &[2, 4, 0, 20]), ins(op::OpFunctionParameter, &[6, 21]), ins(op::OpLabel, &[5])]
                .concat();
        }),
        Reason::EntryPointNotVoidMain
    );
}

#[test]
fn las_constantes_de_especializacion_se_nombran() {
    assert_eq!(
        reason(|p| p.tipos.extend(ins(op::OpSpecConstant, &[6, 20, 7]))),
        Reason::UnsupportedFamily { family: Family::Specialization, opcode: op::OpSpecConstant }
    );
}

// ---- 3. bytes hostiles, tambien para el juez -------------------------------

#[test]
fn voltear_cada_byte_tampoco_tumba_al_juez() {
    for bytes in [SAXPY, MANDELBROT, COLORES] {
        let mut b = bytes.to_vec();
        let mut juzgados = 0;
        for i in 0..b.len() {
            b[i] ^= 0xFF;
            let mut ids = vec![0u32; 1 << 16];
            if let Ok(m) = read(&b, &mut ids) {
                let _ = validate(&m);
                let _ = census(&m);
                juzgados += 1;
            }
            b[i] ^= 0xFF;
        }
        assert!(juzgados > 0, "ningun volteo paso el lector: el juez no se probo");
    }
}
