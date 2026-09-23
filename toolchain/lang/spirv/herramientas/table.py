#!/usr/bin/env python3
"""table.py -- escribe `src/table/` desde la gramatica NORMATIVA de Khronos, o la coteja.

== Por que existe, y que se copia y que no ==

SPIR-V no se forkea: es una ESPECIFICACION, no un programa. Lo que se toma de
Khronos son los NUMEROS -- que `OpIAdd` es 128, que lleva tipo y resultado --
y esos numeros son el contrato, igual que los del ABI: inventarlos seria no
hablar SPIR-V. La fuente es `spirv.core.grammar.json` (licencia MIT, la misma
que genera la seccion binaria de la especificacion), que trae el SDK de Vulkan
del anfitrion.

Lo que NO se toma: el codigo de nadie. Ni SPIRV-Tools ni SPIRV-Cross se enlazan
ni se copian -- se leen para aprender las reglas, como OBS para LA MESA.

Y lo que decide ESTE fichero, que es el estudio: a que FAMILIA pertenece cada
instruccion (`FILAS`, abajo). Las que no se nombran alli entran igual, en la
familia `Otro`: el lector las RECORRE (sabe su forma por la gramatica) y el
juez las niega por su NOMBRE. Asi un `.spv` con algo raro dice "OpBitFieldInsert
fuera", y no "codigo 201". Lo que ni la gramatica conoce, el lector lo niega
con su numero.

== Como se usa ==

    py table.py --escribir    regenera src/table/ desde el SDK
    py table.py --cotejar     src/table/ dice lo mismo que el SDK? (sin SDK: lo dice y sale 0)

El banco NO necesita el SDK: `src/table/` va en el repo.
"""
import io
import json
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
TABLA_DIR = os.path.join(AQUI, "..", "src", "table")
SDK = os.environ.get("VULKAN_SDK", r"C:\VulkanSDK\1.4.350.0")
GRAMATICA = os.path.join(SDK, "Include", "spirv", "unified1", "spirv.core.grammar.json")
GRAMATICA_GLSL = os.path.join(SDK, "Include", "spirv", "unified1", "extinst.glsl.std.450.grammar.json")

# -- GLSL.std.450: las instrucciones extendidas, por GRUPO --------------------
# `Flotante`/`Entero`: el resultado y los operandos son del mismo tipo (escalar
# o vector) -- UNA o dos instrucciones de SSE. `Trascendente`: llegan con S3b,
# porque no existen en SSE y hay que escribirlas UNA vez para oraculo y emisor.
# Lo que no esta aqui, S2 lo niega con su numero.
GLSL = {
    "Float": "FAbs Floor Ceil Fract Sqrt InverseSqrt FMin FMax FClamp FMix Step Fma",
    "Int": "SAbs UMin SMin UMax SMax UClamp SClamp",
    "Transcendental": "Sin Cos Pow Exp Log",
}

# -- LAS FILAS: el estudio. Familia -> nombres. --------------------------------
# `Nucleo`   el subconjunto que S2 acepta (PLAN_EL_SOMBREADOR, seccion 2).
# Las demas familias se LEEN (el lector tiene que poder recorrer un modulo
# entero) y S2 las niega nombrando la familia.
FILAS = {
    "Core": """
        Nop Undef SourceContinued Source SourceExtension Name MemberName String
        Line NoLine ModuleProcessed Extension ExtInstImport ExtInst MemoryModel
        EntryPoint ExecutionMode ExecutionModeId Capability
        TypeVoid TypeBool TypeInt TypeFloat TypeVector TypeArray TypeRuntimeArray
        TypeStruct TypePointer TypeFunction
        ConstantTrue ConstantFalse Constant ConstantComposite ConstantNull
        Function FunctionParameter FunctionEnd FunctionCall
        Variable Load Store CopyMemory AccessChain InBoundsAccessChain ArrayLength
        Decorate MemberDecorate DecorationGroup GroupDecorate GroupMemberDecorate
        DecorateId DecorateString MemberDecorateString
        VectorExtractDynamic VectorInsertDynamic VectorShuffle CompositeConstruct
        CompositeExtract CompositeInsert CopyObject
        ConvertFToU ConvertFToS ConvertSToF ConvertUToF UConvert SConvert FConvert
        Bitcast
        SNegate FNegate IAdd FAdd ISub FSub IMul FMul UDiv SDiv FDiv UMod SRem SMod
        FRem FMod VectorTimesScalar Dot
        Any All IsNan IsInf LogicalEqual LogicalNotEqual LogicalOr LogicalAnd
        LogicalNot Select IEqual INotEqual UGreaterThan SGreaterThan
        UGreaterThanEqual SGreaterThanEqual ULessThan SLessThan ULessThanEqual
        SLessThanEqual FOrdEqual FUnordEqual FOrdNotEqual FUnordNotEqual
        FOrdLessThan FUnordLessThan FOrdGreaterThan FUnordGreaterThan
        FOrdLessThanEqual FUnordLessThanEqual FOrdGreaterThanEqual
        FUnordGreaterThanEqual
        ShiftRightLogical ShiftRightArithmetic ShiftLeftLogical BitwiseOr
        BitwiseXor BitwiseAnd Not
        Phi LoopMerge SelectionMerge Label Branch BranchConditional Return
        ReturnValue Unreachable
    """,
    # Las constantes que se fijan al crear el pipeline: sin VERRANO no hay quien
    # las fije, y S2 las niega.
    "Specialization": "SpecConstantTrue SpecConstantFalse SpecConstant SpecConstantComposite SpecConstantOp",
    # `switch` se lee y S2 lo niega: control de flujo con tabla, despues.
    "ControlFlow": "Switch Kill",
    "Image": """
        TypeImage TypeSampler TypeSampledImage SampledImage Image
        ImageSampleImplicitLod ImageSampleExplicitLod ImageFetch ImageRead
        ImageWrite ImageQuerySizeLod ImageQuerySize ImageTexelPointer
    """,
    "Atomic": """
        AtomicLoad AtomicStore AtomicExchange AtomicCompareExchange
        AtomicIIncrement AtomicIDecrement AtomicIAdd AtomicISub AtomicSMin
        AtomicUMin AtomicSMax AtomicUMax AtomicAnd AtomicOr AtomicXor
    """,
    "Barrier": "ControlBarrier MemoryBarrier",
    "Matrix": """
        TypeMatrix MatrixTimesScalar VectorTimesMatrix MatrixTimesVector
        MatrixTimesMatrix OuterProduct Transpose
    """,
    "Derivative": "DPdx DPdy Fwidth",
}

# -- LA SECCION de la disposicion logica (especificacion, 2.4) -----------------
SECCION_FIJA = {
    "Capability": "Capability",
    "Extension": "Extension",
    "ExtInstImport": "Import",
    "MemoryModel": "MemoryModel",
    "EntryPoint": "EntryPoint",
    "ExecutionMode": "ExecutionMode", "ExecutionModeId": "ExecutionMode",
    "String": "Source", "SourceExtension": "Source", "Source": "Source",
    "SourceContinued": "Source",
    "Name": "Name", "MemberName": "Name",
    "ModuleProcessed": "ModuleProcessed",
    # Pueden ir en la seccion de tipos (globales) Y dentro de una funcion.
    "Variable": "Flexible", "Undef": "Flexible", "Line": "Flexible",
    "NoLine": "Flexible", "Nop": "Flexible",
    "Function": "Function",
    "FunctionEnd": "FunctionEnd",
}
CLASE_A_SECCION = {
    "Annotation": "Annotation",
    "Type-Declaration": "Type",
    "Constant-Creation": "Type",
}

# Operandos que ocupan UNA palabra seguro. Los demas (LiteralString, Pair*,
# LiteralContextDependentNumber...) cortan la cuenta de posicion fija.
UNA_PALABRA = {"IdResultType", "IdResult", "IdRef", "IdScope", "IdMemorySemantics",
               "LiteralInteger", "LiteralExtInstInteger", "LiteralSpecConstantOpInteger"}


def cargar():
    with io.open(GRAMATICA, encoding="utf-8") as f:
        g = json.load(f)
    enums_valor = {k["kind"] for k in g["operand_kinds"] if k["category"] == "ValueEnum"}
    return g, enums_valor


def forma(i, enums_valor, familia):
    """La fila de una instruccion de la gramatica."""
    nombre = i["opname"]
    corto = nombre[2:]
    ops = i.get("operands", [])
    tipo = any(o["kind"] == "IdResultType" for o in ops)
    resultado = any(o["kind"] == "IdResult" for o in ops)
    # Palabras minimas: la cabecera + cada operando obligatorio (una cadena
    # ocupa al menos una palabra).
    minimo = 1 + sum(1 for o in ops if "quantifier" not in o)
    # Donde empieza la cadena, si su sitio es FIJO.
    cadena = 0
    pos = 1
    for o in ops:
        if "quantifier" in o:
            break
        if o["kind"] == "LiteralString":
            cadena = pos
            break
        if o["kind"] in UNA_PALABRA or o["kind"] in enums_valor:
            pos += 1
            continue
        break
    seccion = SECCION_FIJA.get(corto) or CLASE_A_SECCION.get(i["class"], "Body")
    return (i["opcode"], nombre, tipo, resultado, minimo, cadena, seccion, familia)


def filas_de(g, enums_valor):
    por_nombre = {i["opname"]: i for i in g["instructions"]}
    fuera = []
    for familia, texto in FILAS.items():
        for corto in texto.split():
            nombre = "Op" + corto
            i = por_nombre.get(nombre)
            if i is None:
                sys.exit("table.py: %s no existe en la gramatica" % nombre)
            fuera.append(forma(i, enums_valor, familia))
    # ** Y TODAS LAS DEMAS, en la familia `Otro` (2026-09-23, tras la primera
    # medida contra el banco de Naga: 34 de 228 ficheros ni se leian, y el NO
    # decia un numero). La gramatica repite codigos con alias de extension
    # (`OpDecorateStringGOOGLE` = `OpDecorateString`): gana el primero visto.
    vistos = {f[0] for f in fuera}
    for i in g["instructions"]:
        if i["opcode"] in vistos:
            continue
        vistos.add(i["opcode"])
        fuera.append(forma(i, enums_valor, "Other"))
    fuera.sort()
    for a, b in zip(fuera, fuera[1:]):
        if a[0] == b[0]:
            sys.exit("table.py: codigo repetido %d (%s, %s)" % (a[0], a[1], b[1]))
    return fuera


def filas_glsl():
    with io.open(GRAMATICA_GLSL, encoding="utf-8") as f:
        g = json.load(f)
    por_nombre = {i["opname"]: i for i in g["instructions"]}
    fuera = []
    for grupo, texto in GLSL.items():
        for nombre in texto.split():
            i = por_nombre.get(nombre)
            if i is None:
                sys.exit("table.py: GLSL.std.450 no tiene %s" % nombre)
            fuera.append((i["opcode"], nombre, len(i["operands"]), grupo))
    fuera.sort()
    return fuera


def cabecera(g, que):
    return [
        "//! %s" % que,
        "//!",
        "//! ** GENERADO por `herramientas/table.py --escribir` desde la gramatica de",
        "//! Khronos (licencia MIT), SPIR-V %d.%d rev %d. No se edita a mano: las"
        % (g["major_version"], g["minor_version"], g["revision"]),
        "//! familias las decide `FILAS` en el script; los numeros son de la",
        "//! especificacion. `--cotejar` comprueba que coinciden.",
        "//!",
        "//! [consumo]  NADA   datos constantes: no gastan ni en reposo ni corriendo",
        "",
    ]


def rust(filas, g):
    """Los ficheros de `src/table/`, como `{ruta relativa: texto}`.

    ** Partidos por OFICIO (2026-09-23): en uno solo eran 1.820 lineas y L6a
    no deja entrar un modulo nuevo de mas de 1.000. Una fabrica en tiempo de
    compilacion obligaria a meter la gramatica de Khronos en el repo, y la
    regla de esta carpeta es no mezclar. Asi que cada fichero hace UNA cosa:
    las filas, los nombres que el juez mira, y GLSL.std.450.
    """
    fuera = {}

    o = ["//! Las tablas de SPIR-V: la forma de cada instruccion, sus nombres y las de",
         "//! `GLSL.std.450`. GENERADO por `herramientas/table.py`; ver cada fichero.",
         "//!",
         "//! [consumo]  NADA   datos constantes",
         "",
         "mod rows;",
         "pub mod glsl;",
         "pub mod op;",
         "",
         "pub use rows::TABLE;",
         "pub use glsl::GLSL450;",
         ""]
    fuera["mod.rs"] = "\n".join(o)

    o = cabecera(g, "La FORMA de cada instruccion de la gramatica: lo que el lector necesita para recorrerla.")
    o.append("use crate::{Family, OpInfo, Section};")
    o.append("")
    o.append("/// Ordenada por codigo: se busca por biseccion.")
    o.append("pub const TABLE: &[OpInfo] = &[")
    for (cod, nombre, tipo, res, minimo, cadena, seccion, familia) in filas:
        o.append("    OpInfo { opcode: %d, name: \"%s\", has_result_type: %s, has_result: %s, min_words: %d, "
                 "string_word: %d, section: Section::%s, family: Family::%s },"
                 % (cod, nombre, "true" if tipo else "false", "true" if res else "false",
                    minimo, cadena, seccion, familia))
    o.append("];")
    o.append("")
    fuera["rows.rs"] = "\n".join(o)

    # ** Los codigos con el NOMBRE DE LA ESPECIFICACION, para que el juez no
    # teclee ni un numero: `op::OpIAdd` y no `128`. Solo los que tienen familia
    # propia (las de `Otro` no las nombra nadie: el juez las niega por fila).
    o = cabecera(g, "Los codigos de las instrucciones que el juez MIRA, con el nombre de la especificacion.")
    o.append("#![allow(non_upper_case_globals)]")
    o.append("")
    for (cod, nombre, _t, _r, _m, _c, _s, familia) in filas:
        if familia != "Other":
            o.append("pub const %s: u16 = %d;" % (nombre, cod))
    o.append("")
    fuera["op.rs"] = "\n".join(o)

    glsl = filas_glsl()
    o = cabecera(g, "`GLSL.std.450`: las instrucciones extendidas que el juez conoce, y sus numeros.")
    o.append("#![allow(non_upper_case_globals)]")
    o.append("")
    o.append("use crate::{GlslGroup, GlslInfo};")
    o.append("")
    o.append("/// De `extinst.glsl.std.450.grammar.json`. Ordenadas por numero.")
    o.append("pub const GLSL450: &[GlslInfo] = &[")
    for (num, nombre, ops, grupo) in glsl:
        o.append("    GlslInfo { number: %d, name: \"%s\", operands: %d, group: GlslGroup::%s },"
                 % (num, nombre, ops, grupo))
    o.append("];")
    o.append("")
    for (num, nombre, _o, _g) in glsl:
        o.append("pub const %s: u32 = %d;" % (nombre, num))
    o.append("")
    fuera["glsl.rs"] = "\n".join(o)
    return fuera


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ("--escribir", "--cotejar"):
        sys.exit(__doc__)
    if not os.path.exists(GRAMATICA):
        print("table.py: sin el SDK de Vulkan (%s) no hay contra que cotejar; la tabla del repo manda" % GRAMATICA)
        return 0
    g, ev = cargar()
    ficheros = rust(filas_de(g, ev), g)
    if sys.argv[1] == "--escribir":
        os.makedirs(TABLA_DIR, exist_ok=True)
        for nombre, texto in ficheros.items():
            with io.open(os.path.join(TABLA_DIR, nombre), "w", encoding="utf-8", newline="\n") as f:
                f.write(texto)
        print("table.py: escritos %d ficheros en %s" % (len(ficheros), os.path.normpath(TABLA_DIR)))
        return 0
    malos = []
    for nombre, texto in ficheros.items():
        ruta = os.path.join(TABLA_DIR, nombre)
        actual = io.open(ruta, encoding="utf-8").read().replace("\r\n", "\n") if os.path.exists(ruta) else None
        if actual != texto:
            malos.append(nombre)
    if malos:
        print("table.py: src/table/%s NO coincide con la gramatica del SDK -- regenera con --escribir"
              % ", ".join(malos))
        return 1
    print("table.py: clean -- src/table/ coincide con la gramatica de Khronos")
    return 0


if __name__ == "__main__":
    sys.exit(main())
