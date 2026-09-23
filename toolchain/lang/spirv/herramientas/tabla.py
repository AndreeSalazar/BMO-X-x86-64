#!/usr/bin/env python3
"""tabla.py -- escribe `src/tabla.rs` desde la gramatica NORMATIVA de Khronos, o la coteja.

== Por que existe, y que se copia y que no ==

SPIR-V no se forkea: es una ESPECIFICACION, no un programa. Lo que se toma de
Khronos son los NUMEROS -- que `OpIAdd` es 128, que lleva tipo y resultado --
y esos numeros son el contrato, igual que los del ABI: inventarlos seria no
hablar SPIR-V. La fuente es `spirv.core.grammar.json` (licencia MIT, la misma
que genera la seccion binaria de la especificacion), que trae el SDK de Vulkan
del anfitrion.

Lo que NO se toma: el codigo de nadie. Ni SPIRV-Tools ni SPIRV-Cross se enlazan
ni se copian -- se leen para aprender las reglas, como OBS para LA MESA.

Y lo que decide ESTE fichero, que es el estudio: QUE instrucciones tienen fila
(`FILAS`, abajo) y a que FAMILIA pertenece cada una. Una instruccion sin fila
no la lee el lector: la niega nombrando su codigo.

== Como se usa ==

    py tabla.py --escribir    regenera src/tabla.rs desde el SDK
    py tabla.py --cotejar     src/tabla.rs dice lo mismo que el SDK? (sin SDK: lo dice y sale 0)

El banco NO necesita el SDK: `tabla.rs` va en el repo.
"""
import io
import json
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
TABLA_RS = os.path.join(AQUI, "..", "src", "tabla.rs")
SDK = os.environ.get("VULKAN_SDK", r"C:\VulkanSDK\1.4.350.0")
GRAMATICA = os.path.join(SDK, "Include", "spirv", "unified1", "spirv.core.grammar.json")

# -- LAS FILAS: el estudio. Familia -> nombres. --------------------------------
# `Nucleo`   el subconjunto que S2 acepta (PLAN_EL_SOMBREADOR, seccion 2).
# Las demas familias se LEEN (el lector tiene que poder recorrer un modulo
# entero) y S2 las niega nombrando la familia.
FILAS = {
    "Nucleo": """
        Nop Undef SourceContinued Source SourceExtension Name MemberName String
        Line NoLine ModuleProcessed Extension ExtInstImport ExtInst MemoryModel
        EntryPoint ExecutionMode ExecutionModeId Capability
        TypeVoid TypeBool TypeInt TypeFloat TypeVector TypeArray TypeRuntimeArray
        TypeStruct TypePointer TypeFunction
        ConstantTrue ConstantFalse Constant ConstantComposite ConstantNull
        SpecConstantTrue SpecConstantFalse SpecConstant SpecConstantComposite
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
    # `switch` se lee y S2 lo niega: control de flujo con tabla, despues.
    "Salto": "Switch Kill",
    "Imagen": """
        TypeImage TypeSampler TypeSampledImage SampledImage Image
        ImageSampleImplicitLod ImageSampleExplicitLod ImageFetch ImageRead
        ImageWrite ImageQuerySizeLod ImageQuerySize ImageTexelPointer
    """,
    "Atomico": """
        AtomicLoad AtomicStore AtomicExchange AtomicCompareExchange
        AtomicIIncrement AtomicIDecrement AtomicIAdd AtomicISub AtomicSMin
        AtomicUMin AtomicSMax AtomicUMax AtomicAnd AtomicOr AtomicXor
    """,
    "Barrera": "ControlBarrier MemoryBarrier",
    "Matriz": """
        TypeMatrix MatrixTimesScalar VectorTimesMatrix MatrixTimesVector
        MatrixTimesMatrix OuterProduct Transpose
    """,
    "Derivada": "DPdx DPdy Fwidth",
}

# -- LA SECCION de la disposicion logica (especificacion, 2.4) -----------------
SECCION_FIJA = {
    "Capability": "Capacidad",
    "Extension": "Extension",
    "ExtInstImport": "Importacion",
    "MemoryModel": "Modelo",
    "EntryPoint": "Entrada",
    "ExecutionMode": "Modo", "ExecutionModeId": "Modo",
    "String": "Fuente", "SourceExtension": "Fuente", "Source": "Fuente",
    "SourceContinued": "Fuente",
    "Name": "Nombre", "MemberName": "Nombre",
    "ModuleProcessed": "Procesado",
    # Pueden ir en la seccion de tipos (globales) Y dentro de una funcion.
    "Variable": "Flexible", "Undef": "Flexible", "Line": "Flexible",
    "NoLine": "Flexible", "Nop": "Flexible",
    "Function": "Funcion",
    "FunctionEnd": "FinFuncion",
}
CLASE_A_SECCION = {
    "Annotation": "Anotacion",
    "Type-Declaration": "Tipo",
    "Constant-Creation": "Tipo",
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


def filas_de(g, enums_valor):
    por_nombre = {i["opname"]: i for i in g["instructions"]}
    fuera = []
    for familia, texto in FILAS.items():
        for corto in texto.split():
            nombre = "Op" + corto
            i = por_nombre.get(nombre)
            if i is None:
                sys.exit("tabla.py: %s no existe en la gramatica" % nombre)
            ops = i.get("operands", [])
            tipo = any(o["kind"] == "IdResultType" for o in ops)
            resultado = any(o["kind"] == "IdResult" for o in ops)
            # Palabras minimas: la cabecera + cada operando obligatorio (una
            # cadena ocupa al menos una palabra).
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
            seccion = SECCION_FIJA.get(corto) or CLASE_A_SECCION.get(i["class"], "Cuerpo")
            fuera.append((i["opcode"], nombre, tipo, resultado, minimo, cadena, seccion, familia))
    fuera.sort()
    for a, b in zip(fuera, fuera[1:]):
        if a[0] == b[0]:
            sys.exit("tabla.py: codigo repetido %d (%s, %s)" % (a[0], a[1], b[1]))
    return fuera


def rust(filas, g):
    o = []
    o.append("//! La tabla de instrucciones de SPIR-V que este lector conoce.")
    o.append("//!")
    o.append("//! ** GENERADA por `herramientas/tabla.py --escribir` desde")
    o.append("//! `spirv.core.grammar.json` (Khronos, licencia MIT) version %d.%d rev %d. No se"
             % (g["major_version"], g["minor_version"], g["revision"]))
    o.append("//! edita a mano: QUE filas hay y su familia lo decide `FILAS` en el script;")
    o.append("//! los numeros son de la especificacion. `--cotejar` comprueba que coinciden.")
    o.append("//!")
    o.append("//! [consumo]  NADA   es una tabla constante: no gasta ni en reposo ni corriendo")
    o.append("")
    o.append("use crate::{Familia, Fila, Seccion};")
    o.append("")
    o.append("/// Ordenada por codigo: se busca por biseccion.")
    o.append("pub const TABLA: &[Fila] = &[")
    for (cod, nombre, tipo, res, minimo, cadena, seccion, familia) in filas:
        o.append("    Fila { codigo: %d, nombre: \"%s\", tipo: %s, resultado: %s, minimo: %d, "
                 "cadena: %d, seccion: Seccion::%s, familia: Familia::%s },"
                 % (cod, nombre, "true" if tipo else "false", "true" if res else "false",
                    minimo, cadena, seccion, familia))
    o.append("];")
    o.append("")
    return "\n".join(o)


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ("--escribir", "--cotejar"):
        sys.exit(__doc__)
    if not os.path.exists(GRAMATICA):
        print("tabla.py: sin el SDK de Vulkan (%s) no hay contra que cotejar; la tabla del repo manda" % GRAMATICA)
        return 0
    g, ev = cargar()
    texto = rust(filas_de(g, ev), g)
    if sys.argv[1] == "--escribir":
        with io.open(TABLA_RS, "w", encoding="utf-8", newline="\n") as f:
            f.write(texto)
        print("tabla.py: escrita %s" % os.path.normpath(TABLA_RS))
        return 0
    with io.open(TABLA_RS, encoding="utf-8") as f:
        actual = f.read().replace("\r\n", "\n")
    if actual != texto:
        print("tabla.py: src/tabla.rs NO coincide con la gramatica del SDK -- regenerala con --escribir")
        return 1
    print("tabla.py: clean -- src/tabla.rs coincide con la gramatica de Khronos")
    return 0


if __name__ == "__main__":
    sys.exit(main())
