#!/usr/bin/env python3
"""censo_naga.py -- la MATRIZ: el lector y el juez contra el banco de pruebas de Naga.

Naga (el traductor de sombreadores de wgpu, MIT/Apache-2.0) guarda en
`naga/tests/in/` cientos de sombreadores de verdad: WGSL, GLSL y SPIR-V en
texto. Es la mejor fuente de "lo que el mundo escribe" que hay, y se usa como
ACATS para Ada: una matriz contra la que MEDIR, no codigo que copiar.

** NADA DE NAGA ENTRA EN EL REPO. El corpus vive en
`BMO-externo/naga-corpus` (clon ralo de `naga/tests/in`), los `.spv` que se
fabrican van a `BMO-externo/naga-corpus/fabricados`, y aqui solo queda esta
herramienta y el numero que da.

Tres fabricantes, del anfitrion, ninguno enlazado:
    .spvasm      -> spirv-as   (Vulkan SDK)
    .wgsl        -> naga       (`cargo install naga-cli`)
    .comp/.vert/.frag -> glslc (Vulkan SDK)

    py censo_naga.py [--corpus DIR]
"""
import argparse
import os
import shutil
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.normpath(os.path.join(AQUI, "..", "..", "..", ".."))
EXTERNO = os.path.normpath(os.path.join(RAIZ, "..", "BMO-externo", "naga-corpus"))
SDK = os.environ.get("VULKAN_SDK", r"C:\VulkanSDK\1.4.350.0")
EXE = ".exe" if os.name == "nt" else ""
SPIRV_AS = os.path.join(SDK, "Bin", "spirv-as" + EXE)
GLSLC = os.path.join(SDK, "Bin", "glslc" + EXE)
NAGA = shutil.which("naga") or os.path.expanduser(os.path.join("~", ".cargo", "bin", "naga" + EXE))


def fabricar(corpus, salida):
    os.makedirs(salida, exist_ok=True)
    for f in os.listdir(salida):
        if f.endswith(".spv"):
            os.remove(os.path.join(salida, f))
    cuenta = {}
    for sub, ext, herramienta in [
        ("spv", ".spvasm", "spirv-as"),
        ("wgsl", ".wgsl", "naga"),
        ("glsl", ".comp", "glslc"),
        ("glsl", ".vert", "glslc"),
        ("glsl", ".frag", "glslc"),
    ]:
        d = os.path.join(corpus, sub)
        if not os.path.isdir(d):
            continue
        for nombre in sorted(os.listdir(d)):
            if not nombre.endswith(ext):
                continue
            fuente = os.path.join(d, nombre)
            destino = os.path.join(salida, "%s-%s.spv" % (sub, nombre[: -len(ext)]))
            if herramienta == "spirv-as":
                orden = [SPIRV_AS, fuente, "-o", destino]
            elif herramienta == "naga":
                orden = [NAGA, fuente, destino]
            else:
                orden = [GLSLC, "--target-env=vulkan1.0", "-O0", fuente, "-o", destino]
            r = subprocess.run(orden, capture_output=True, text=True)
            ok = r.returncode == 0 and os.path.exists(destino)
            c = cuenta.setdefault(herramienta, [0, 0])
            c[0 if ok else 1] += 1
    return cuenta


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.join(EXTERNO, "naga", "tests", "in"))
    a = ap.parse_args()
    if not os.path.isdir(a.corpus):
        sys.exit("censo_naga.py: no esta el corpus (%s). Se clona ralo fuera del repo." % a.corpus)
    for h in (SPIRV_AS, GLSLC, NAGA):
        if not os.path.exists(h):
            sys.exit("censo_naga.py: falta %s" % h)
    salida = os.path.join(EXTERNO, "fabricados")
    cuenta = fabricar(a.corpus, salida)
    print("fabricados (salieron / el fabricante los nego):")
    for h, (bien, mal) in sorted(cuenta.items()):
        print("  %-9s %4d / %d" % (h, bien, mal))
    print()
    subprocess.run(["cargo", "run", "-q", "-p", "bmo-spirv-front", "--example", "censo", "--", salida],
                   cwd=RAIZ, check=True)


if __name__ == "__main__":
    main()
