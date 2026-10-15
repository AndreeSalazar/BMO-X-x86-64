"""llvm-obedece -- el guardian que hace que LLVM pida PERMISO (B4, 14-10).

El propietario, 14-10: "puedes hacer que ese LLVM de rust TENGA OBLIGACION
que tengas control? porque no quiero tener choque con LLVM luego que decida
sino que yo decido por motivos".

Que decide quien
================

    PERFIL/CPU.txt, columna `llvm`   lo que el COMPILADOR puede elegir solo
                                     (un `+feature`, un `#[target_feature]`)
    PERFIL/CPU.txt, columna `mano`   lo que la CASA puede escribir a mano;
                                     eso lo juzgan `sem-asm` y `bex-link`

Este guardian mira la primera. LLVM elige instrucciones por tres puertas, y
las tres estan aqui:

    1. la CPU del objetivo     `-C target-cpu=...` o `"cpu"` en un objetivo
                               .json. Tiene que ser `x86-64`, la generica: una
                               CPU con nombre (`native`, `znver3`) le deja
                               elegir TODO lo de esa CPU sin preguntar
    2. las extensiones         `-C target-feature=+x` o `"features"`: cada `+x`
                               es base de x86-64 o esta `llvm: si`
    3. una funcion suelta      `#[target_feature(enable = "x")]`: lo mismo

Un `+x` que este guardian no sabe nombrar NO pasa: lo que no se entiende no
se deja entrar (se le pone nombre aqui, en LLVM_A_PERFIL, y se decide).

Y lo que LLVM emite de verdad lo juzga `bex-link` con la lente, en el binario
(PARA el build si hay algo no decidido): esto cierra las puertas, aquello
mira lo que paso por ellas.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
sys.path.insert(0, os.path.join(RAIZ, "toolchain", "tools", "perfil-campos"))
import perfil_campos  # noqa: E402  -- el MISMO lector de la tabla: una fuente

# De nombre de LLVM a fila de PERFIL/CPU.txt.
LLVM_A_PERFIL = {
    "sse3": "SSE3",
    "ssse3": "SSSE3",
    "sse4.1": "SSE4.1",
    "sse4.2": "SSE4.2",
    "popcnt": "POPCNT",
    "lzcnt": "LZCNT",
    "bmi": "BMI1",
    "bmi2": "BMI2",
    "avx": "AVX",
    "avx2": "AVX2",
    "fma": "FMA",
    "rdrnd": "RDRAND",
    "rdseed": "RDSEED",
}
# Lo que x86-64 trae siempre, o lo que no es una instruccion: no se decide.
BASE = {"sse", "sse2", "x87", "fxsr", "cmov", "cx8", "crt-static", "soft-float"}
CPU_GENERICA = "x86-64"

FUERA = ("target", ".git", "node_modules")


def ficheros():
    for dirpath, dirs, files in os.walk(RAIZ):
        dirs[:] = [d for d in dirs if d not in FUERA]
        for f in files:
            yield os.path.join(dirpath, f)


def decision():
    with open(os.path.join(RAIZ, "PERFIL", "CPU.txt"), "r", encoding="utf-8") as fh:
        filas = perfil_campos.extensiones_del_perfil(fh.read())
    return {n: llvm for n, _, _, _, llvm in filas}


def juzga_features(texto, donde, llvm, quejas):
    for f in [x.strip() for x in texto.split(",") if x.strip()]:
        if f.startswith("-"):
            continue  # quitar nunca da a LLVM nada que elegir
        nombre = f.lstrip("+").rstrip(".")  # el punto de una frase en un comentario
        if nombre in BASE:
            continue
        fila = LLVM_A_PERFIL.get(nombre)
        if fila is None:
            quejas.append("%s: `+%s` -- este guardian no sabe que fila de PERFIL/CPU.txt es; "
                          "se le pone nombre en LLVM_A_PERFIL y se decide" % (donde, nombre))
        elif llvm.get(fila) != "si":
            quejas.append("%s: `+%s` deja a LLVM elegir %s y PERFIL/CPU.txt dice `llvm: no`" % (donde, nombre, fila))


def juzga_cpu(cpu, donde, quejas):
    if cpu != CPU_GENERICA:
        quejas.append("%s: la CPU del objetivo es `%s`, no `%s` -- con ella LLVM elige todo lo que esa "
                      "CPU sabe sin preguntar al perfil" % (donde, cpu, CPU_GENERICA))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    llvm = decision()
    quejas = []
    mirados = {"objetivos": 0, "config": 0, "fuentes": 0}
    for ruta in ficheros():
        rel = os.path.relpath(ruta, RAIZ).replace(os.sep, "/")
        nombre = os.path.basename(ruta)
        if nombre.endswith(".json") or nombre == "config.toml" or nombre.endswith((".ps1", ".sh", ".cmd")):
            try:
                with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                    t = fh.read()
            except OSError:
                continue
            if nombre.endswith(".json") and '"llvm-target"' in t:
                mirados["objetivos"] += 1
                m = re.search(r'"cpu"\s*:\s*"([^"]*)"', t)
                juzga_cpu(m.group(1) if m else "(sin decir)", rel, quejas)
                m = re.search(r'"features"\s*:\s*"([^"]*)"', t)
                if m:
                    juzga_features(m.group(1), rel, llvm, quejas)
            elif nombre == "config.toml" or nombre.endswith((".ps1", ".sh", ".cmd")):
                if nombre == "config.toml":
                    mirados["config"] += 1
                for m in re.finditer(r"target-cpu=([\w.\-]+)", t):
                    juzga_cpu(m.group(1), rel, quejas)
                for m in re.finditer(r"target-feature=([+\-\w.,]+)", t):
                    juzga_features(m.group(1), rel, llvm, quejas)
        elif nombre.endswith(".rs"):
            try:
                with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                    t = fh.read()
            except OSError:
                continue
            mirados["fuentes"] += 1
            for m in re.finditer(r'#\[target_feature\(\s*enable\s*=\s*"([^"]+)"', t):
                juzga_features(",".join("+" + x for x in m.group(1).split(",")), rel, llvm, quejas)
    if quejas:
        print("LLVM elige algo que el propietario no decidio:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  Lo que LLVM puede elegir lo dice la columna `llvm` de PERFIL/CPU.txt.")
        print("  Cambiarla es una decision del propietario, con su commit y su motivo.")
        return 1 if args.check else 0
    permitidas = sorted(n for n, v in llvm.items() if v == "si")
    print("clean: LLVM obedece -- %d objetivo(s) .json, %d config.toml y %d fuente(s) .rs mirados; "
          "CPU `%s` y ninguna extension fuera de lo base%s" % (
              mirados["objetivos"], mirados["config"], mirados["fuentes"], CPU_GENERICA,
              "" if not permitidas else " y de lo decidido (" + ", ".join(permitidas) + ")"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
